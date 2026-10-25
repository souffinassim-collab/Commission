use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use serde::Deserialize;
use tauri::State;
use crate::calculation::CalculationEngine;
use crate::db::{DatabaseManager, ImportAEnregistrer};
use crate::export::ExcelExporter;
use crate::models::{
    Anomalie, Bordereau, BrancheParam, CalculateMultiBordereauRequest, DbInfo, ExcelClientSummary, ExcelScanResult,
    Intermediaire, UploadedFileResult,
};
use crate::parser::{
    extract_pages, CirisPdfParser, ExcelProductionParser, ExcelSelection, FlottePdfParser, PeriodeBordereau,
};

/// Fichier analysé en attente d'enregistrement (contenu conservé jusqu'à l'enregistrement du bordereau)
pub struct FichierEnAttente {
    pub contenu: Vec<u8>,
    pub nom: String,
}

pub struct AppState {
    pub db: Mutex<DatabaseManager>,
    pub en_attente: Mutex<HashMap<String, FichierEnAttente>>,
}

impl AppState {
    pub fn new(db: DatabaseManager) -> Self {
        Self { db: Mutex::new(db), en_attente: Mutex::new(HashMap::new()) }
    }
}

/// Contexte de l'import : chaque fichier est rattaché à l'intermédiaire et à la période du bordereau en cours
#[derive(Debug, Clone, Deserialize)]
pub struct ImportContext {
    /// Intermédiaire du bordereau ; absent si les fichiers sont importés avant de l'identifier
    #[serde(default)]
    pub intermediaire_id: Option<i64>,
    pub exercice: i32,
    pub mois: Option<i32>,
    /// Bordereau déjà enregistré en cours de modification (ses propres fichiers ne sont pas des doublons)
    #[serde(default)]
    pub bordereau_id: Option<i64>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{:02x}", b)).collect()
}

fn decode_base64_payload(data: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    let clean_b64 = match data.find(',') {
        Some(idx) => &data[idx + 1..],
        None => data,
    };
    let trimmed: String = clean_b64.chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD
        .decode(&trimmed)
        .map_err(|e| format!("Erreur décodage base64: {}", e))
}

fn file_name_of(path: &str) -> String {
    PathBuf::from(path).file_name().and_then(|n| n.to_str()).unwrap_or("fichier").to_string()
}

/// Analyse d'un fichier selon son type (détection sur le contenu pour les PDF)
fn parse_file_auto_bytes(
    bytes: &[u8],
    file_name: &str,
    selection: &ExcelSelection,
    periode: Option<PeriodeBordereau>,
    known_codes: &[String],
) -> Result<UploadedFileResult, String> {
    let lower = file_name.to_lowercase();
    if lower.ends_with(".xlsx") || lower.ends_with(".xls") {
        ExcelProductionParser::parse_from_bytes(bytes, file_name, selection, periode)
    } else if lower.ends_with(".pdf") {
        let doc = lopdf::Document::load_mem(bytes).map_err(|e| format!("Erreur lecture PDF: {:?}", e))?;
        let pages = extract_pages(&doc);
        if CirisPdfParser::is_ciris(&pages) {
            CirisPdfParser::parse_pages(&pages, file_name, known_codes)
        } else {
            let res = FlottePdfParser::parse_pages(&pages, file_name, known_codes);
            if res.items.is_empty() && res.anomalies.is_empty() {
                Err(format!(
                    "« {} » n'est ni un rapport CIRIS ni un PDF de flotte reconnu (aucune garantie trouvée).",
                    file_name
                ))
            } else {
                Ok(res)
            }
        }
    } else {
        Err(format!("Format non supporté pour le fichier: {}", file_name))
    }
}

/// Contrôles du fichier par rapport au bordereau en cours (intermédiaire, période, doublon).
/// Les résultats remplacent `anomalies_contexte` ; sans intermédiaire, seul le doublon est contrôlé.
fn validate_against_context(
    db: &DatabaseManager,
    res: &mut UploadedFileResult,
    ctx: &ImportContext,
) -> Result<(), String> {
    res.anomalies_contexte.clear();
    // Rapport CIRIS (totaux sans N° de police) : contrôle sur le fichier lui-même. Les autres fichiers
    // peuvent servir à plusieurs bordereaux ; le contrôle porte alors sur les polices (voir le calcul).
    if res.file_type == "CIRIS_PDF" {
        if let Some((_, num)) = db.find_hash_usage(&res.hash, ctx.bordereau_id)? {
            res.anomalies_contexte.push(Anomalie::erreur(format!(
                "Ce fichier a déjà été importé dans le bordereau {} : il ne peut pas être commissionné deux fois.",
                num
            )));
        }
    }
    let inter = match ctx.intermediaire_id {
        Some(id) => db.get_intermediaire(id)?.ok_or_else(|| format!("Intermédiaire #{} introuvable.", id))?,
        None => return Ok(()),
    };
    if !inter.actif {
        res.anomalies_contexte.push(Anomalie::erreur(format!(
            "L'intermédiaire {} ({}) est désactivé.",
            inter.nom, inter.code
        )));
    }

    if res.file_type == "CIRIS_PDF" {
        match res.code.as_deref() {
            Some(code) if code.trim() != inter.code.trim() => res.anomalies_contexte.push(Anomalie::erreur(format!(
                "Ce rapport CIRIS appartient à l'agence {} ({}) et non à l'intermédiaire sélectionné {} ({}).",
                code,
                res.courtier_ou_client.clone().unwrap_or_default(),
                inter.code,
                inter.nom
            ))),
            _ => {}
        }
        if let Some(ex) = res.exercice {
            if ex != ctx.exercice {
                res.anomalies_contexte.push(Anomalie::erreur(format!(
                    "Période du rapport ({}) différente de l'exercice du bordereau ({}).",
                    res.periode.clone().unwrap_or_default(),
                    ctx.exercice
                )));
            }
        }
        if let (Some(m), Some(cm)) = (res.mois, ctx.mois) {
            if m != cm {
                res.anomalies_contexte.push(Anomalie::erreur(format!(
                    "Période du rapport ({}) différente du mois du bordereau ({:02}).",
                    res.periode.clone().unwrap_or_default(),
                    cm
                )));
            }
        }
    } else {
        let annee = res.exercice.or_else(|| res.periode.as_deref().and_then(|p| p.get(..4)).and_then(|y| y.parse().ok()));
        if let Some(y) = annee {
            if y != ctx.exercice {
                res.anomalies_contexte.push(Anomalie::avertissement(format!(
                    "Le fichier porte sur l'exercice {} alors que le bordereau est sur {}.",
                    y, ctx.exercice
                )));
            }
        }
    }
    Ok(())
}

/// Relance les contrôles de contexte (après identification de l'intermédiaire ou changement de période)
#[tauri::command]
pub fn revalider_fichiers(
    fichiers: Vec<UploadedFileResult>,
    ctx: ImportContext,
    state: State<'_, AppState>,
) -> Result<Vec<UploadedFileResult>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut out = fichiers;
    for f in out.iter_mut() {
        validate_against_context(&db, f, &ctx)?;
    }
    Ok(out)
}

fn analyse_et_met_en_attente(
    state: &State<'_, AppState>,
    bytes: Vec<u8>,
    file_name: &str,
    selection: &ExcelSelection,
    ctx: &ImportContext,
) -> Result<UploadedFileResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let known = db.known_sous_codes()?;
    let periode = Some(PeriodeBordereau { exercice: ctx.exercice, mois: ctx.mois });
    let mut res = parse_file_auto_bytes(&bytes, file_name, selection, periode, &known)?;
    res.hash = sha256_hex(&bytes);
    res.taille = bytes.len() as i64;
    validate_against_context(&db, &mut res, ctx)?;
    drop(db);
    state
        .en_attente
        .lock()
        .map_err(|e| e.to_string())?
        .insert(res.id.clone(), FichierEnAttente { contenu: bytes, nom: file_name.to_string() });
    Ok(res)
}

// ---------------------------------------------------------------------------------------------
// Base de données
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub fn get_db_info(state: State<'_, AppState>) -> Result<DbInfo, String> {
    state.db.lock().map_err(|e| e.to_string())?.get_info()
}

#[tauri::command]
pub fn backup_database(state: State<'_, AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    Ok(db.create_pre_migration_backup()?.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------------------------
// Paramètres
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub fn get_branches(state: State<'_, AppState>) -> Result<Vec<BrancheParam>, String> {
    state.db.lock().map_err(|e| e.to_string())?.get_all_branches()
}

#[tauri::command]
pub fn update_branche(branche: BrancheParam, state: State<'_, AppState>) -> Result<(), String> {
    state.db.lock().map_err(|e| e.to_string())?.save_branche(&branche).map(|_| ())
}

#[tauri::command]
pub fn save_branche(branche: BrancheParam, state: State<'_, AppState>) -> Result<i64, String> {
    state.db.lock().map_err(|e| e.to_string())?.save_branche(&branche)
}

#[tauri::command]
pub fn delete_branche(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.db.lock().map_err(|e| e.to_string())?.delete_branche(id)
}

#[tauri::command]
pub fn get_categories(state: State<'_, AppState>) -> Result<Vec<crate::models::Categorie>, String> {
    state.db.lock().map_err(|e| e.to_string())?.get_categories()
}

#[tauri::command]
pub fn save_categorie(categorie: crate::models::Categorie, state: State<'_, AppState>) -> Result<i64, String> {
    state.db.lock().map_err(|e| e.to_string())?.save_categorie(&categorie)
}

#[tauri::command]
pub fn delete_categorie(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.db.lock().map_err(|e| e.to_string())?.delete_categorie(id)
}

#[tauri::command]
pub fn affecter_categorie(branche_ids: Vec<i64>, categorie_id: Option<i64>, state: State<'_, AppState>) -> Result<(), String> {
    state.db.lock().map_err(|e| e.to_string())?.affecter_categorie(&branche_ids, categorie_id)
}

#[tauri::command]
pub fn reset_branches_to_default(state: State<'_, AppState>) -> Result<Vec<BrancheParam>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.reset_branches_to_default()?;
    db.get_all_branches()
}

// ---------------------------------------------------------------------------------------------
// Intermédiaires
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub fn get_intermediaires(state: State<'_, AppState>) -> Result<Vec<Intermediaire>, String> {
    state.db.lock().map_err(|e| e.to_string())?.get_all_intermediaires()
}

#[tauri::command]
pub fn save_intermediaire(item: Intermediaire, state: State<'_, AppState>) -> Result<i64, String> {
    state.db.lock().map_err(|e| e.to_string())?.save_intermediaire(&item)
}

#[tauri::command]
pub fn delete_intermediaire(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.db.lock().map_err(|e| e.to_string())?.delete_intermediaire(id)
}

// ---------------------------------------------------------------------------------------------
// Import de fichiers
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub fn parse_file_auto(
    file_path: String,
    selection: Option<ExcelSelection>,
    ctx: ImportContext,
    state: State<'_, AppState>,
) -> Result<UploadedFileResult, String> {
    let bytes = fs::read(&file_path).map_err(|e| format!("Impossible de lire le fichier: {}", e))?;
    analyse_et_met_en_attente(&state, bytes, &file_name_of(&file_path), &selection.unwrap_or_default(), &ctx)
}

#[tauri::command]
pub fn parse_file_auto_base64(
    base64_data: String,
    file_name: String,
    selection: Option<ExcelSelection>,
    ctx: ImportContext,
    state: State<'_, AppState>,
) -> Result<UploadedFileResult, String> {
    let bytes = decode_base64_payload(&base64_data)?;
    analyse_et_met_en_attente(&state, bytes, &file_name, &selection.unwrap_or_default(), &ctx)
}

/// Registre Excel : analyse (points de vente) ; le classeur reste en mémoire pour la sélection
fn analyser_et_garder(state: &State<'_, AppState>, bytes: Vec<u8>, file_name: &str) -> Result<ExcelScanResult, String> {
    let mut scan = ExcelProductionParser::scan_from_bytes(&bytes, file_name)?;
    scan.staging_id = format!("classeur-{}", chrono::Local::now().timestamp_micros());
    state
        .en_attente
        .lock()
        .map_err(|e| e.to_string())?
        .insert(scan.staging_id.clone(), FichierEnAttente { contenu: bytes, nom: file_name.to_string() });
    Ok(scan)
}

fn classeur_en_attente(state: &State<'_, AppState>, staging_id: &str) -> Result<(Vec<u8>, String), String> {
    let map = state.en_attente.lock().map_err(|e| e.to_string())?;
    let f = map
        .get(staging_id)
        .ok_or_else(|| "Le classeur n'est plus en mémoire : réimportez le fichier.".to_string())?;
    Ok((f.contenu.clone(), f.nom.clone()))
}

#[tauri::command]
pub fn analyser_classeur(file_path: String, state: State<'_, AppState>) -> Result<ExcelScanResult, String> {
    let bytes = fs::read(&file_path).map_err(|e| format!("Impossible de lire le fichier: {}", e))?;
    analyser_et_garder(&state, bytes, &file_name_of(&file_path))
}

#[tauri::command]
pub fn analyser_classeur_base64(
    base64_data: String,
    file_name: String,
    state: State<'_, AppState>,
) -> Result<ExcelScanResult, String> {
    let bytes = decode_base64_payload(&base64_data)?;
    analyser_et_garder(&state, bytes, &file_name)
}

/// Clients d'un point de vente du classeur analysé (un par code, sans doublon)
#[tauri::command]
pub fn clients_point_vente(
    staging_id: String,
    point_vente: String,
    state: State<'_, AppState>,
) -> Result<Vec<ExcelClientSummary>, String> {
    let (bytes, _) = classeur_en_attente(&state, &staging_id)?;
    ExcelProductionParser::clients_from_bytes(&bytes, &point_vente)
}

/// Import de la sélection faite dans le classeur analysé (point de vente, clients éventuels)
#[tauri::command]
pub fn importer_classeur(
    staging_id: String,
    selection: ExcelSelection,
    ctx: ImportContext,
    state: State<'_, AppState>,
) -> Result<UploadedFileResult, String> {
    let (bytes, nom) = classeur_en_attente(&state, &staging_id)?;
    analyse_et_met_en_attente(&state, bytes, &nom, &selection, &ctx)
}

/// Abandon de l'import en cours : libère les fichiers en attente
#[tauri::command]
pub fn clear_pending_files(ids: Vec<String>, state: State<'_, AppState>) -> Result<(), String> {
    let mut map = state.en_attente.lock().map_err(|e| e.to_string())?;
    for id in ids {
        map.remove(&id);
    }
    Ok(())
}

#[tauri::command]
pub fn calculate_multi_bordereau(
    req: CalculateMultiBordereauRequest,
    state: State<'_, AppState>,
) -> Result<Bordereau, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let branches = db.get_all_branches()?;
    let deja = db.polices_commissionnees(req.bordereau_id)?;
    Ok(CalculationEngine::compute_multi_bordereau(&req, &branches, &deja))
}

// ---------------------------------------------------------------------------------------------
// Bordereaux
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub fn save_bordereau(
    bordereau: Bordereau,
    fichiers: Option<Vec<UploadedFileResult>>,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    let mut bordereau = bordereau;
    if let Some(a) = bordereau.anomalies_bloquantes().first() {
        return Err(format!("Enregistrement impossible : {}", a.message));
    }
    // Contrôles de contexte rejoués côté serveur : intermédiaire, période, fichier déjà commissionné
    let mut fichiers = fichiers;
    if let Some(list) = fichiers.as_mut() {
        let ctx = ImportContext {
            intermediaire_id: bordereau.intermediaire_id,
            exercice: bordereau.exercice,
            mois: bordereau.mois,
            bordereau_id: bordereau.id,
        };
        let db = state.db.lock().map_err(|e| e.to_string())?;
        for f in list.iter_mut() {
            validate_against_context(&db, f, &ctx)?;
        }
    }
    // Cohérence montants/totaux à partir des taux figés dans chaque ligne (le barème n'est pas relu)
    CalculationEngine::recompute_bordereau(&mut bordereau);

    let en_attente = state.en_attente.lock().map_err(|e| e.to_string())?;
    let imports: Option<Vec<ImportAEnregistrer>> = fichiers.as_ref().map(|list| {
        list.iter()
            .map(|f| ImportAEnregistrer { fichier: f, contenu: en_attente.get(&f.id).map(|p| p.contenu.clone()) })
            .collect()
    });
    drop(en_attente);

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let id = db.save_bordereau(&bordereau, imports.as_deref())?;
    drop(db);

    if let Some(list) = fichiers {
        let mut map = state.en_attente.lock().map_err(|e| e.to_string())?;
        for f in list {
            map.remove(&f.id);
        }
    }
    Ok(id)
}

#[tauri::command]
pub fn get_bordereaux(state: State<'_, AppState>) -> Result<Vec<Bordereau>, String> {
    state.db.lock().map_err(|e| e.to_string())?.get_all_bordereaux()
}

#[tauri::command]
pub fn get_bordereau(id: i64, state: State<'_, AppState>) -> Result<Option<Bordereau>, String> {
    state.db.lock().map_err(|e| e.to_string())?.get_bordereau_by_id(id)
}

#[tauri::command]
pub fn update_bordereau_statut(id: i64, statut: String, state: State<'_, AppState>) -> Result<(), String> {
    state.db.lock().map_err(|e| e.to_string())?.update_bordereau_statut(id, &statut)
}

#[tauri::command]
pub fn delete_bordereau(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.db.lock().map_err(|e| e.to_string())?.delete_bordereau(id)
}

// ---------------------------------------------------------------------------------------------
// Pièces jointes / fichiers rattachés
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub fn add_piece_jointe(bordereau_id: i64, file_path: String, state: State<'_, AppState>) -> Result<i64, String> {
    let bytes = fs::read(&file_path).map_err(|e| format!("Impossible de lire le fichier: {}", e))?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_piece_jointe(bordereau_id, &file_name_of(&file_path), &sha256_hex(&bytes), &bytes)
}

#[tauri::command]
pub fn add_piece_jointe_base64(
    bordereau_id: i64,
    file_name: String,
    base64_data: String,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    let bytes = decode_base64_payload(&base64_data)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_piece_jointe(bordereau_id, &file_name, &sha256_hex(&bytes), &bytes)
}

#[tauri::command]
pub fn delete_piece_jointe(fichier_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.db.lock().map_err(|e| e.to_string())?.delete_piece_jointe(fichier_id)
}

fn safe_name(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect()
}

/// Extrait un fichier rattaché dans Exports/<numéro de bordereau>/ et l'ouvre avec l'application associée
#[tauri::command]
pub fn ouvrir_fichier(fichier_id: i64, state: State<'_, AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let (nom, bordereau_id, contenu) = db.get_fichier_contenu(fichier_id)?;
    let numero = db
        .get_bordereau_by_id(bordereau_id)?
        .map(|b| b.numero_bordereau)
        .unwrap_or_else(|| format!("bordereau_{}", bordereau_id));
    let dir = db.exports_directory().join(safe_name(&numero));
    drop(db);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(safe_name(&nom));
    fs::write(&path, contenu).map_err(|e| format!("Écriture impossible : {}", e))?;
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|e| format!("Ouverture impossible : {}", e))?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn export_excel(
    bordereau: Bordereau,
    export_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let default_name = format!("Bordereau_{}.xlsx", safe_name(&bordereau.numero_bordereau));
    // Par défaut dans le dossier "Exports" à côté de BDD (et non le répertoire courant,
    // qui en mode dev est src-tauri et relancerait l'application via le watcher)
    let target_path = match export_path {
        Some(p) => PathBuf::from(p),
        None => {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            db.exports_directory().join(default_name)
        }
    };
    ExcelExporter::export_bordereau_to_file(&bordereau, &target_path)
        .map_err(|e| format!("Erreur export Excel: {:?}", e))?;
    Ok(target_path.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------------------------
// Détail de production : facture de l'intermédiaire, rapprochement et écarts
// ---------------------------------------------------------------------------------------------

/// Détail de production à confirmer (détail par police, commission calculée / déclarée, écarts)
#[tauri::command]
pub fn detail_production(bordereau: Bordereau) -> Result<crate::detail::DetailProduction, String> {
    Ok(crate::detail::construire(&bordereau))
}

/// Enregistre la facture de l'intermédiaire (numéro, date, commission HT déclarée par ligne)
#[tauri::command]
pub fn enregistrer_facture_intermediaire(
    bordereau_id: i64,
    facture: Option<crate::models::FactureIntermediaire>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.db.lock().map_err(|e| e.to_string())?.enregistrer_facture(bordereau_id, facture.as_ref())
}

/// Export Excel du détail de production (même présentation que le modèle du service commercial)
#[tauri::command]
pub fn export_detail_production(bordereau: Bordereau, state: State<'_, AppState>) -> Result<String, String> {
    let d = crate::detail::construire(&bordereau);
    let path = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.exports_directory().join(format!("Detail_production_{}.xlsx", safe_name(&bordereau.numero_bordereau)))
    };
    ExcelExporter::export_detail_production(&d, &path).map_err(|e| format!("Erreur export Excel: {:?}", e))?;
    Ok(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controles_contexte_import_avant_intermediaire() {
        let Ok(bytes) = std::fs::read("../Import/05-2025.PDF") else { return };
        let dir = std::env::temp_dir().join(format!("commissions_ctx_{}", std::process::id()));
        let db = DatabaseManager::with_path(dir.join("BDD").join("commissions.db")).unwrap();
        let known = db.known_sous_codes().unwrap();
        let mut f = parse_file_auto_bytes(&bytes, "05-2025.PDF", &ExcelSelection::default(), None, &known).unwrap();
        f.hash = sha256_hex(&bytes);
        let nouvel = |code: &str, t: &str| {
            db.save_intermediaire(&Intermediaire {
                id: None,
                type_intermediaire: t.to_string(),
                code: code.to_string(),
                nom: format!("INTER {}", code),
                succursale: "S".to_string(),
                departement: "D".to_string(),
                contact: None,
                adresse: None,
                tva_applicable: 0.19,
                actif: true,
            })
            .unwrap()
        };
        let aga = nouvel("492", "AGA");
        let autre = nouvel("500", "AGA");
        let ctx = |id: Option<i64>, mois: Option<i32>| ImportContext { intermediaire_id: id, exercice: 2025, mois, bordereau_id: None };

        // Import sans intermédiaire : aucun contrôle bloquant de contexte
        validate_against_context(&db, &mut f, &ctx(None, Some(5))).unwrap();
        assert!(f.anomalies_contexte.is_empty());
        // Bon intermédiaire (agence 492 du rapport) et bonne période
        validate_against_context(&db, &mut f, &ctx(Some(aga), Some(5))).unwrap();
        assert!(f.anomalies_contexte.is_empty(), "{:?}", f.anomalies_contexte);
        // Mauvais intermédiaire et mauvais mois : signalés, puis effacés lors d'une nouvelle validation correcte
        validate_against_context(&db, &mut f, &ctx(Some(autre), Some(6))).unwrap();
        assert_eq!(f.anomalies_contexte.iter().filter(|a| a.est_bloquante()).count(), 2);
        validate_against_context(&db, &mut f, &ctx(Some(aga), Some(5))).unwrap();
        assert!(f.anomalies_contexte.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_auto_detection_des_pdf() {
        let known = crate::parser::tests::known();
        if let Ok(bytes) = std::fs::read("../Import/05-2025.PDF") {
            let res = parse_file_auto_bytes(&bytes, "05-2025.PDF", &ExcelSelection::default(), None, &known).expect("parse CIRIS");
            assert_eq!(res.file_type, "CIRIS_PDF");
            assert_eq!(res.items.len(), 17);
        }
        if let Ok(bytes) = std::fs::read("../Import/RCCDVF.pdf") {
            let res = parse_file_auto_bytes(&bytes, "RCCDVF.pdf", &ExcelSelection::default(), None, &known).expect("parse flotte");
            assert_eq!(res.file_type, "FLOTTE_PDF");
        }
    }
}

use std::sync::Mutex;
use std::fs;
use std::path::PathBuf;
use chrono::Local;
use tauri::State;
use crate::db::DatabaseManager;
use crate::models::{BrancheParam, Intermediaire, Bordereau, CirisPdfExtractedData, DbInfo};
use crate::calculation::CalculationEngine;
use crate::parser::CirisPdfParser;
use crate::export::ExcelExporter;

pub struct AppState {
    pub db: Mutex<DatabaseManager>,
}

#[tauri::command]
pub fn get_db_info(state: State<'_, AppState>) -> Result<DbInfo, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_info().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn backup_database(state: State<'_, AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path = db.create_pre_migration_backup().map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn get_branches(state: State<'_, AppState>) -> Result<Vec<BrancheParam>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_all_branches().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_branche(branche: BrancheParam, state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.update_branche(&branche).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_intermediaires(state: State<'_, AppState>) -> Result<Vec<Intermediaire>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_all_intermediaires().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_intermediaire(item: Intermediaire, state: State<'_, AppState>) -> Result<i64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.save_intermediaire(&item).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn parse_ciris_pdf(file_path: String) -> Result<CirisPdfExtractedData, String> {
    let bytes = fs::read(&file_path).map_err(|e| format!("Impossible de lire le fichier: {}", e))?;
    CirisPdfParser::parse_from_bytes(&bytes)
}

#[tauri::command]
pub fn parse_ciris_pdf_base64(base64_data: String) -> Result<CirisPdfExtractedData, String> {
    use base64::Engine;

    // Si l'utilisateur dépose le fichier depuis le frontend en base64 (avec préfixe data:application/pdf;base64,...)
    let clean_b64 = if let Some(idx) = base64_data.find(',') {
        &base64_data[idx + 1..]
    } else {
        &base64_data
    };

    let trimmed: String = clean_b64.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&trimmed)
        .map_err(|e| format!("Erreur décodage base64: {}", e))?;

    CirisPdfParser::parse_from_bytes(&bytes)
}

#[tauri::command]
pub fn calculate_bordereau_from_pdf(
    extracted_data: CirisPdfExtractedData,
    type_intermediaire: String,
    tva_taux: f64,
    succursale: String,
    departement: String,
    state: State<'_, AppState>,
) -> Result<Bordereau, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let branches = db.get_all_branches().map_err(|e| e.to_string())?;

    let mut lignes = Vec::new();
    let mut total_prime_nette = 0.0;
    let mut total_commission_apport = 0.0;
    let mut total_commission_gestion = 0.0;

    for item in &extracted_data.items {
        let matched = CalculationEngine::match_branche(item, &branches);
        let ligne = CalculationEngine::compute_ligne(item, matched, &type_intermediaire, tva_taux);
        
        total_prime_nette += ligne.prime_nette;
        total_commission_apport += ligne.montant_apport;
        total_commission_gestion += ligne.montant_gestion;
        lignes.push(ligne);
    }

    total_prime_nette = CalculationEngine::round_cents(total_prime_nette);
    total_commission_apport = CalculationEngine::round_cents(total_commission_apport);
    total_commission_gestion = CalculationEngine::round_cents(total_commission_gestion);
    let total_commission_ht = CalculationEngine::round_cents(total_commission_apport + total_commission_gestion);
    let total_tva = CalculationEngine::round_cents(total_commission_ht * tva_taux);
    let total_commission_ttc = CalculationEngine::round_cents(total_commission_ht + total_tva);

    let num_bordereau = format!(
        "BORD-{}-{}-{:02}",
        type_intermediaire,
        extracted_data.exercice,
        extracted_data.mois
    );

    let now = Local::now().format("%Y-%m-%d").to_string();

    Ok(Bordereau {
        id: None,
        intermediaire_id: None,
        intermediaire_nom: extracted_data.agence_nom.clone(),
        intermediaire_code: extracted_data.agence_code.clone(),
        type_intermediaire,
        numero_bordereau: num_bordereau,
        exercice: extracted_data.exercice,
        mois: Some(extracted_data.mois),
        code_client: None,
        succursale,
        departement,
        tva_taux,
        date_generation: now,
        total_prime_nette,
        total_commission_apport,
        total_commission_gestion,
        total_commission_ht,
        total_tva,
        total_commission_ttc,
        statut: "BROUILLON".to_string(),
        lignes,
    })
}

#[tauri::command]
pub fn save_bordereau(bordereau: Bordereau, state: State<'_, AppState>) -> Result<i64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.save_bordereau(&bordereau).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_bordereaux(state: State<'_, AppState>) -> Result<Vec<Bordereau>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_all_bordereaux().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_bordereau(id: i64, state: State<'_, AppState>) -> Result<Option<Bordereau>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_bordereau_by_id(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_bordereau(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_bordereau(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_excel(bordereau: Bordereau, export_path: Option<String>) -> Result<String, String> {
    let default_name = format!(
        "Bordereau_{}_{}_{:02}_{}.xlsx",
        bordereau.type_intermediaire,
        bordereau.intermediaire_code,
        bordereau.mois.unwrap_or(0),
        bordereau.exercice
    );

    let target_path = if let Some(p) = export_path {
        PathBuf::from(p)
    } else {
        PathBuf::from(default_name)
    };

    ExcelExporter::export_bordereau_to_file(&bordereau, &target_path)
        .map_err(|e| format!("Erreur export Excel: {:?}", e))?;

    Ok(target_path.to_string_lossy().to_string())
}

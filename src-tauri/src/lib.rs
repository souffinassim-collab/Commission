pub mod models;
pub mod db;
pub mod calculation;
pub mod parser;
pub mod export;
pub mod detail;
pub mod commands;

use db::DatabaseManager;
use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_manager = DatabaseManager::new()
        .unwrap_or_else(|e| panic!("Échec d'initialisation de la base SQLite dans ./BDD : {}", e));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new(db_manager))
        .invoke_handler(tauri::generate_handler![
            get_db_info,
            backup_database,
            get_branches,
            update_branche,
            save_branche,
            delete_branche,
            reset_branches_to_default,
            get_categories,
            save_categorie,
            delete_categorie,
            affecter_categorie,
            get_intermediaires,
            save_intermediaire,
            delete_intermediaire,
            analyser_classeur,
            analyser_classeur_base64,
            clients_point_vente,
            importer_classeur,
            parse_file_auto,
            parse_file_auto_base64,
            clear_pending_files,
            revalider_fichiers,
            calculate_multi_bordereau,
            save_bordereau,
            get_bordereaux,
            get_bordereau,
            update_bordereau_statut,
            delete_bordereau,
            add_piece_jointe,
            add_piece_jointe_base64,
            delete_piece_jointe,
            ouvrir_fichier,
            export_excel,
            detail_production,
            export_detail_production,
            enregistrer_facture_intermediaire,
        ])
        .run(tauri::generate_context!())
        .expect("Erreur lors de l'exécution de l'application Tauri");
}

#[cfg(test)]
mod tests {
    use super::*;
    use calculation::CalculationEngine;
    use db::ImportAEnregistrer;
    use export::ExcelExporter;
    use models::{BrancheParam, CalculateMultiBordereauRequest, Intermediaire, UploadedFileResult};
    use parser::{CirisPdfParser, ExcelProductionParser, FlottePdfParser};

    /// Base SQLite jetable propre à chaque test (ne touche jamais BDD/commissions.db)
    fn test_db() -> DatabaseManager {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "commissions_test_{}_{}_{}",
            std::process::id(),
            n,
            chrono::Local::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        DatabaseManager::with_path(dir.join("BDD").join("commissions.db")).expect("DB Init failed")
    }

    /// Intermédiaire du rapport 05-2025 (agence 492), créé s'il n'existe pas encore dans la base de test
    fn inter_by_code(db: &DatabaseManager, code: &str) -> Intermediaire {
        if let Some(i) = db.get_all_intermediaires().unwrap().into_iter().find(|i| i.code == code) {
            return i;
        }
        let id = db
            .save_intermediaire(&Intermediaire {
                id: None,
                type_intermediaire: "AGA".to_string(),
                code: code.to_string(),
                nom: "AGA ABDELLAH".to_string(),
                succursale: "SUCCURSALE SUD".to_string(),
                departement: "Département Commercial".to_string(),
                contact: None,
                adresse: None,
                tva_applicable: 0.19,
                actif: true,
            })
            .unwrap();
        db.get_intermediaire(id).unwrap().unwrap()
    }

    fn new_courtier(db: &DatabaseManager, code: &str) -> Intermediaire {
        let id = db
            .save_intermediaire(&Intermediaire {
                id: None,
                type_intermediaire: "COURTIER".to_string(),
                code: code.to_string(),
                nom: format!("COURTIER {}", code),
                succursale: "SUCCURSALE SUD".to_string(),
                departement: "Département Commercial".to_string(),
                contact: None,
                adresse: None,
                tva_applicable: 0.19,
                actif: true,
            })
            .unwrap();
        db.get_intermediaire(id).unwrap().unwrap()
    }

    fn ciris_file(db: &DatabaseManager) -> Option<(UploadedFileResult, Vec<u8>)> {
        let bytes = std::fs::read("../Import/05-2025.PDF").ok()?;
        let mut f = CirisPdfParser::parse_from_bytes(&bytes, "05-2025.PDF", &db.known_sous_codes().unwrap()).unwrap();
        f.hash = format!("hash-{}", bytes.len());
        Some((f, bytes))
    }

    fn req_for(inter: &Intermediaire, files: Vec<UploadedFileResult>, exercice: i32, mois: Option<i32>) -> CalculateMultiBordereauRequest {
        CalculateMultiBordereauRequest {
            files,
            bordereau_id: None,
            intermediaire_id: inter.id,
            type_intermediaire: inter.type_intermediaire.clone(),
            intermediaire_code: inter.code.clone(),
            intermediaire_nom: inter.nom.clone(),
            code_client: None,
            succursale: inter.succursale.clone(),
            departement: inter.departement.clone(),
            tva_taux: inter.tva_applicable,
            exercice,
            mois,
            cumuler_par_branche: true,
        }
    }

    #[test]
    fn test_db_init_migrations_et_parametres_initiaux() {
        let db = test_db();
        let branches = db.get_all_branches().unwrap();
        assert!(branches.len() >= 70);
        assert_eq!(db.get_info().unwrap().version, 8);
        let vrc = branches.iter().find(|b| b.sous_code == "VRC").unwrap();
        assert_eq!(vrc.alias_import, "VP,VO,VR");
        assert_eq!(vrc.groupe_bordereau, "RC AUTOMOBILE");
        let vinv = branches.iter().find(|b| b.sous_code == "VINV").unwrap();
        assert_eq!(vinv.sous_libelle, "Incendie de Véhicule");
        // Réinitialisation : mêmes paramètres initiaux
        db.reset_branches_to_default().unwrap();
        let again = db.get_all_branches().unwrap();
        assert_eq!(again.len(), branches.len());
        assert!(again.iter().any(|b| b.sous_code == "VRC" && b.alias_import == "VP,VO,VR"));
    }

    /// Migration de la base réelle (copie) : les bordereaux existants sont relus sans changement de taux
    #[test]
    fn test_migration_base_existante_sans_modification() {
        for base in ["../BDD/commissions.db", "../exe/BDD/commissions.db"] {
            verifier_migration_base_reelle(base);
        }
    }

    /// Applique toutes les migrations sur une COPIE de la base indiquée et vérifie que les bordereaux
    /// existants sont relus à l'identique (taux, montants) et rattachés à leur intermédiaire.
    fn verifier_migration_base_reelle(chemin: &str) {
        let src = std::path::Path::new(chemin);
        if !src.exists() {
            return;
        }
        let before = rusqlite::Connection::open(src).unwrap();
        let mut st = before
            .prepare("SELECT bordereau_id, code_garantie, taux_apport, taux_gestion, montant_ttc FROM bordereau_lignes ORDER BY id")
            .unwrap();
        let avant: Vec<(i64, String, f64, f64, f64)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        drop(st);
        let dir = std::env::temp_dir().join(format!(
            "commissions_migration_{}_{}",
            std::process::id(),
            chemin.replace(['.', '/', '\\'], "_")
        ));
        std::fs::create_dir_all(dir.join("BDD")).unwrap();
        let target = dir.join("BDD").join("commissions.db");
        let _ = std::fs::remove_file(&target);
        before
            .execute_batch(&format!("VACUUM INTO '{}'", target.to_string_lossy().replace('\'', "''")))
            .unwrap();
        drop(before);

        let db = DatabaseManager::with_path(&target).expect("migration");
        assert_eq!(db.get_info().unwrap().version, 8);
        for b in db.get_all_bordereaux().unwrap() {
            let full = db.get_bordereau_by_id(b.id.unwrap()).unwrap().unwrap();
            assert!(full.intermediaire_id.is_some(), "bordereau {} rattaché à son intermédiaire", full.numero_bordereau);
            for l in &full.lignes {
                let a = avant
                    .iter()
                    .find(|x| x.0 == full.id.unwrap() && x.1 == l.code_garantie && x.4 == l.montant_ttc)
                    .expect("ligne inchangée");
                assert_eq!((a.2, a.3), (l.taux_apport, l.taux_gestion));
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_exclusion_polices_non_encaissees() {
        let db = test_db();
        let Ok(xls) = std::fs::read("../Import/Divers.xlsx") else { return };
        let excel = ExcelProductionParser::parse_from_bytes(&xls, "Divers.xlsx", &crate::parser::ExcelSelection { point_vente: Some("347".into()), clients: vec!["347024203/0".into()] }, None).unwrap();
        let courtier = new_courtier(&db, "CRT-347");
        let mut b = CalculationEngine::compute_multi_bordereau(&req_for(&courtier, vec![excel], 2025, None), &db.get_all_branches().unwrap(), &std::collections::HashMap::new());
        let ht_avant = b.total_commission_ht;
        let idx = b.lignes.iter().position(|l| l.libelle_garantie == "AUTOMOBILE FACULTATIF").unwrap();
        let prime_avant = b.lignes[idx].prime_nette;

        // Police non encaissée : exclue par le commercial
        for c in b.lignes[idx].contributions.iter_mut() {
            if c.police.as_deref() == Some("347/VF/304/0/1") {
                c.exclue = true;
            }
        }
        CalculationEngine::recompute_bordereau(&mut b);
        assert_eq!(b.lignes[idx].prime_nette, CalculationEngine::round_cents(prime_avant - 128648.76));
        assert_eq!(b.total_commission_ht, CalculationEngine::round_cents(ht_avant - 12864.88));

        // Enregistrement puis réouverture : l'exclusion et les montants sont conservés
        let id = db.save_bordereau(&b, None).unwrap();
        let r = db.get_bordereau_by_id(id).unwrap().unwrap();
        let l = r.lignes.iter().find(|l| l.libelle_garantie == "AUTOMOBILE FACULTATIF").unwrap();
        assert!(l.contributions.iter().any(|c| c.exclue && c.police.as_deref() == Some("347/VF/304/0/1")));
        assert_eq!(l.prime_nette, b.lignes[idx].prime_nette);
        assert_eq!(r.total_commission_ttc, b.total_commission_ttc);
        let xlsx = std::env::temp_dir().join(format!("test_polices_exclues_{}.xlsx", std::process::id()));
        assert!(ExcelExporter::export_bordereau_to_file(&r, &xlsx).is_ok());
        let _ = std::fs::remove_file(xlsx);

        // Toutes les polices d'une ligne exclues : la ligne ne compte plus
        let mut all = r.clone();
        let j = all.lignes.iter().position(|l| l.libelle_garantie == "CATNAT").unwrap();
        for c in all.lignes[j].contributions.iter_mut() {
            c.exclue = true;
        }
        CalculationEngine::recompute_bordereau(&mut all);
        assert_eq!(all.lignes[j].prime_nette, 0.0);
        assert!(!all.lignes[j].est_active());
    }

    /// Une police déjà commissionnée dans un bordereau enregistré n'est pas recomptée (signalée),
    /// et l'enregistrement d'un doublon est refusé. Un même registre sert à plusieurs bordereaux.
    #[test]
    fn test_police_deja_commissionnee_non_recomptee() {
        use parser::{ExcelSelection, PeriodeBordereau};
        let db = test_db();
        let Some(bytes) = parser::tests::fichier_test("PROD2026.XLS") else { return };
        let aga = inter_by_code(&db, "492");
        let periode = Some(PeriodeBordereau { exercice: 2026, mois: Some(5) });
        let sel = ExcelSelection { point_vente: Some("492".into()), clients: vec![] };
        let mut f = ExcelProductionParser::parse_from_bytes(&bytes, "PROD2026.XLS", &sel, periode).unwrap();
        f.hash = "prod2026".to_string();
        let branches = db.get_all_branches().unwrap();

        let mut b = CalculationEngine::compute_multi_bordereau(&req_for(&aga, vec![f.clone()], 2026, Some(5)), &branches, &db.polices_commissionnees(None).unwrap());
        // Codes branche sans alias (ex: RD, RN) : lignes non paramétrées, exclues ici pour le test
        for l in b.lignes.iter_mut().filter(|l| l.non_parametre) {
            l.desactive = true;
        }
        CalculationEngine::recompute_bordereau(&mut b);
        assert!(b.total_prime_nette > 0.0);
        let imports = [ImportAEnregistrer { fichier: &f, contenu: Some(bytes.clone()) }];
        let id = db.save_bordereau(&b, Some(&imports)).unwrap();

        // Même registre pour un autre point de vente : autorisé (plus de blocage sur l'empreinte du fichier)
        let autre = inter_by_code(&db, "445");
        let sel445 = ExcelSelection { point_vente: Some("445".into()), clients: vec![] };
        let mut f445 = ExcelProductionParser::parse_from_bytes(&bytes, "PROD2026.XLS", &sel445, periode).unwrap();
        f445.hash = "prod2026".to_string();
        let mut b445 = CalculationEngine::compute_multi_bordereau(&req_for(&autre, vec![f445.clone()], 2026, Some(5)), &branches, &db.polices_commissionnees(None).unwrap());
        for l in b445.lignes.iter_mut().filter(|l| l.non_parametre) {
            l.desactive = true;
        }
        CalculationEngine::recompute_bordereau(&mut b445);
        // Contenu identique (même empreinte) : stocké une seule fois en base
        let imports445 = [ImportAEnregistrer { fichier: &f445, contenu: Some(bytes.clone()) }];
        let id445 = db.save_bordereau(&b445, Some(&imports445)).unwrap();
        assert_ne!(id, id445);
        let c1 = db.list_fichiers(id).unwrap()[0].id.unwrap();
        let c2 = db.list_fichiers(id445).unwrap()[0].id.unwrap();
        assert_eq!(db.get_fichier_contenu(c1).unwrap().2.len(), db.get_fichier_contenu(c2).unwrap().2.len());

        // Facture saisie sur le premier bordereau : elle est citée comme origine de la double facturation
        db.enregistrer_facture(id, Some(&models::FactureIntermediaire { numero: "F-492-05".into(), date: "10/06/2026".into(), ..Default::default() }))
            .unwrap();
        // Réimport du point de vente 492 pour mai : toutes les polices sont déjà facturées
        let deja = db.polices_commissionnees(None).unwrap();
        let mut again = CalculationEngine::compute_multi_bordereau(&req_for(&aga, vec![f.clone()], 2026, Some(5)), &branches, &deja);
        assert_eq!(again.total_prime_nette, 0.0);
        assert!(again
            .anomalies
            .iter()
            .any(|a| a.message.contains("Double facturation") && a.message.contains(&b.numero_bordereau) && a.message.contains("F-492-05")));
        // Les polices restent visibles, verrouillées (déjà facturées) et exclues du calcul
        let contribs: Vec<&models::LigneContribution> = again.lignes.iter().flat_map(|l| l.contributions.iter()).collect();
        assert!(!contribs.is_empty());
        assert!(contribs.iter().all(|c| c.exclue && c.deja_facturee.as_deref().unwrap_or("").contains("F-492-05")));
        assert!(again.lignes.iter().all(|l| !l.est_active()));
        // Tentative de refacturation (réintégration forcée) : refusée par la base
        for l in again.lignes.iter_mut() {
            for c in l.contributions.iter_mut() {
                c.exclue = false;
            }
        }
        CalculationEngine::recompute_bordereau(&mut again);
        assert!(again.total_prime_nette > 0.0);
        for l in again.lignes.iter_mut().filter(|l| l.non_parametre) {
            l.desactive = true;
        }
        assert!(db.save_bordereau(&again, None).unwrap_err().contains("Double facturation refusée"));
        let mut doublon = b.clone();
        doublon.id = None;
        assert!(db.save_bordereau(&doublon, None).unwrap_err().contains("Double facturation refusée"));
        // Le bordereau lui-même peut être ré-enregistré (ses propres polices ne sont pas des doublons)
        assert_eq!(db.save_bordereau(&db.get_bordereau_by_id(id).unwrap().unwrap(), None).unwrap(), id);

        // Suppression : le contenu partagé reste tant qu'un bordereau l'utilise
        db.delete_bordereau(id).unwrap();
        assert_eq!(db.get_fichier_contenu(c2).unwrap().2.len(), bytes.len());
    }

    /// Reproduit le modèle « Détail a confirmé.xlsx » (docs/) : courtier 715 CAPASSURANCE, exercice 2026,
    /// TVA 9 %, police VF ventilée (RC / assistance / dommages) et facture du courtier saisie.
    #[test]
    fn test_detail_production_modele_service_commercial() {
        use models::{BordereauLigne, FactureIntermediaire, LigneContribution};
        let db = test_db();
        let mut courtier = new_courtier(&db, "715");
        courtier.nom = "CAPASSURANCE".to_string();
        courtier.tva_applicable = 0.09;
        db.save_intermediaire(&courtier).unwrap();
        let courtier = db.get_intermediaire(courtier.id.unwrap()).unwrap().unwrap();

        let contrib_g = |police: &str, branche: &str, garantie: Option<&str>, montant: f64, cp: f64, ttc: f64| LigneContribution {
            fichier_source: "Registre.xls".to_string(),
            police: Some(police.to_string()),
            client: Some("OPGI TAMANRASSET".to_string()),
            montant,
            exclue: false,
            client_code: Some("350021468/0".to_string()),
            branche: Some(branche.to_string()),
            date_emission: Some("30/12/2025".to_string()),
            date_effet: Some("01/01/2026".to_string()),
            date_echeance: Some("31/12/2026".to_string()),
            cp: Some(cp),
            prime_ttc: Some(ttc),
            deja_facturee: None,
            garantie: garantie.map(|g| g.to_string()),
        };
        let contrib = |police: &str, branche: &str, montant: f64, cp: f64, ttc: f64| contrib_g(police, branche, None, montant, cp, ttc);

        let ligne = |libelle: &str, taux: f64, contributions: Vec<LigneContribution>| BordereauLigne {
            id: None,
            bordereau_id: None,
            branche_id: None,
            code_garantie: libelle.to_string(),
            libelle_garantie: libelle.to_string(),
            branche_nom: libelle.to_string(),
            prime_nette: 0.0,
            taux_apport: taux,
            taux_gestion: 0.0,
            montant_apport: 0.0,
            montant_gestion: 0.0,
            montant_ht: 0.0,
            montant_tva: 0.0,
            montant_ttc: 0.0,
            non_parametre: false,
            desactive: false,
            contributions,
            source_code: None,
            source_code_ministeriel: None,
            source_type: None,
        };
        let vf = "350/VF/349/0/0";
        let mut b = models::Bordereau {
            id: None,
            intermediaire_id: courtier.id,
            intermediaire_nom: courtier.nom.clone(),
            intermediaire_code: courtier.code.clone(),
            type_intermediaire: "COURTIER".to_string(),
            numero_bordereau: "BORD-COURTIER-715-2026".to_string(),
            exercice: 2026,
            mois: None,
            code_client: Some("350021468/0".to_string()),
            succursale: "Succursale Sud".to_string(),
            departement: "Département commercial".to_string(),
            tva_taux: 0.09,
            date_generation: "2026-10-25".to_string(),
            total_prime_nette: 0.0,
            total_commission_apport: 0.0,
            total_commission_gestion: 0.0,
            total_commission_ht: 0.0,
            total_tva: 0.0,
            total_commission_ttc: 0.0,
            statut: "BROUILLON".to_string(),
            lignes: vec![
                ligne("RC AUTO", 0.05, vec![contrib_g(vf, "VF", Some("VRC"), 8493.14, 300.0, 192638.23)]),
                ligne("GARANTIES FACULT", 0.10, vec![contrib_g(vf, "VF", Some("VTR"), 133015.38, 300.0, 192638.23)]),
                ligne("ASSISTANSE", 0.075, vec![contrib_g(vf, "VF", Some("VASS"), 15000.0, 300.0, 192638.23)]),
                ligne(
                    "INCENDIE",
                    0.08,
                    vec![contrib("350/IM/5/0/0", "IM", 1805528.28, 500.0, 2149893.65), contrib("350/IN/27/0/0", "IN", 43824.51, 500.0, 53466.17)],
                ),
                ligne(
                    "CAT-NAT",
                    0.05,
                    vec![
                        contrib("350/CN/3909/0/0", "CN", 107163.67, 150.0, 107353.67),
                        contrib("350/CN/3910/0/0", "CN", 4092.74, 150.0, 4282.74),
                        contrib("350/CN/3911/0/0", "CN", 3308.31, 150.0, 3498.31),
                        contrib("350/CN/3912/0/0", "CN", 7002.19, 150.0, 7192.19),
                        contrib("350/CN/3913/0/0", "CN", 2886.58, 150.0, 3076.58),
                        contrib("350/CN/3914/0/0", "CN", 27957.02, 150.0, 28147.02),
                    ],
                ),
                ligne("RC G", 0.08, vec![contrib("350/RG/27/0/0", "RG", 80000.0, 500.0, 96035.0)]),
                ligne("VOL", 0.10, vec![contrib("350/VM/17/0/0", "VM", 17620.0, 500.0, 21762.8)]),
                ligne("T-Informatique", 0.08, vec![contrib("350/TI/23/0/0", "TI", 15165.0, 500.0, 18961.35)]),
            ],
            anomalies: vec![],
            fichiers: vec![],
            facture: None,
            client_libelle: None,
        };
        CalculationEngine::recompute_bordereau(&mut b);

        // Commission calculée par branche (colonne COM-HT du modèle)
        let ht: Vec<f64> = b.lignes.iter().map(|l| l.montant_ht).collect();
        assert_eq!(ht, vec![424.66, 13301.54, 1125.0, 147948.22, 7620.53, 6400.0, 1762.0, 1213.2]);

        // Facture du courtier (colonne VOTR-COM-HT du modèle)
        let declare = [424.66, 13301.54, 1125.0, 147948.22, 7620.53, 6400.0, 1762.0, 1213.2];
        let mut facture = FactureIntermediaire { numero: "F-2026-12".into(), date: "15/10/2026".into(), ..Default::default() };
        for (l, v) in b.lignes.iter().zip(declare.iter()) {
            facture.montants_ht.insert(detail::cle_ligne(l), *v);
        }
        b.facture = Some(facture.clone());
        let d = detail::construire(&b);
        assert_eq!(d.polices.len(), 12);
        assert_eq!(d.total_prime_nette, 2271056.82);
        assert_eq!(d.total_prime_lignes, 2271056.82);
        assert_eq!(d.controle, 0.0);
        // La police VF est ventilée sur RC / facultatif / assistance, CP et TTC repris une seule fois
        // Colonnes de l'état : VRC | VRCV | ASSISTANSE | Dommages (= reste de la prime nette)
        assert_eq!(d.titre, "Détail de production courtier");
        assert_eq!(d.colonnes_ventilation, vec!["VRC", "VRCV", "ASSISTANSE", "Dommages"]);
        let pvf = d.polices.iter().find(|p| p.police == vf).unwrap();
        assert_eq!(pvf.prime_nette, 156508.52);
        assert_eq!(pvf.ventilation, vec![Some(8493.14), None, Some(15000.0), Some(133015.38)]);
        assert_eq!(d.total_ventilation, vec![8493.14, 0.0, 15000.0, 133015.38]);
        // Polices non auto : pas de ventilation
        assert!(d.polices.iter().filter(|p| p.police != vf).all(|p| p.ventilation.iter().all(|v| v.is_none())));
        assert_eq!(pvf.prime_ttc, Some(192638.23));
        assert_eq!(d.total_cp, 3700.0);
        assert_eq!(d.clients, vec!["OPGI TAMANRASSET 350021468/0".to_string()]);
        assert_eq!(d.ht_calcule, 179795.15);
        assert_eq!(d.ht_declare, 179795.15);
        assert_eq!(d.ecart_ht, 0.0);
        assert_eq!(d.tva_calculee, 16181.56);
        assert_eq!(d.ttc_calcule, 195976.71);
        assert_eq!(d.ecart_ttc, 0.0);
        assert_eq!(d.lignes_non_saisies, 0);

        // Écart : le courtier déclare 100 DA de plus sur l'incendie
        let mut f2 = facture.clone();
        *f2.montants_ht.get_mut(&detail::cle_ligne(&b.lignes[3])).unwrap() += 100.0;
        let mut b2 = b.clone();
        b2.facture = Some(f2);
        let d2 = detail::construire(&b2);
        assert_eq!(d2.lignes[3].ecart, Some(100.0));
        assert_eq!(d2.ecart_ht, 100.0);
        assert_eq!(d2.ecart_tva, 9.0);
        assert_eq!(d2.ecart_ttc, 109.0);

        // Enregistrement, saisie de la facture, relecture, export Excel du modèle
        let id = db.save_bordereau(&b, None).unwrap();
        let mut mauvaise = b2.facture.clone().unwrap();
        mauvaise.date = "2026-10-15".into();
        assert!(db.enregistrer_facture(id, Some(&mauvaise)).unwrap_err().contains("JJ/MM/AAAA"));
        db.enregistrer_facture(id, b2.facture.as_ref()).unwrap();
        let relu = db.get_bordereau_by_id(id).unwrap().unwrap();
        assert_eq!(relu.client_libelle.as_deref(), Some("350021468/0 OPGI TAMANRASSET"));
        assert_eq!(relu.facture.as_ref().unwrap().numero, "F-2026-12");
        let dr = detail::construire(&relu);
        assert_eq!(dr.ecart_ht, 100.0);
        assert_eq!(dr.polices.len(), 12);
        let xlsx = std::env::temp_dir().join(format!("detail_production_{}.xlsx", std::process::id()));
        ExcelExporter::export_detail_production(&dr, &xlsx).unwrap();
        if let Ok(copie) = std::env::var("COPIE_ETAT_DETAIL") {
            ExcelExporter::export_detail_production(&detail::construire(&b), &copie).unwrap();
        }
        assert!(xlsx.exists());
        let _ = std::fs::remove_file(xlsx);
        // Facture verrouillée une fois le bordereau réglé
        db.update_bordereau_statut(id, "VALIDE").unwrap();
        db.enregistrer_facture(id, b2.facture.as_ref()).unwrap();
        db.update_bordereau_statut(id, "REGLE").unwrap();
        assert!(db.enregistrer_facture(id, None).is_err());
    }

    #[test]
    fn test_crud_categories_parametres() {
        let db = test_db();
        let cats = db.get_categories().unwrap();
        let noms: Vec<&str> = cats.iter().map(|c| c.libelle.as_str()).collect();
        assert_eq!(noms, vec!["Automobile", "Transport & Maritime", "Agricole", "Incendie & Risques Divers", "Responsabilité Civile"]);
        let branches = db.get_all_branches().unwrap();
        let cat = |libelle: &str| cats.iter().find(|c| c.libelle == libelle).unwrap().id;
        // Classement par code ministériel : les garanties auto (bris de glace, vol…) restent en Automobile
        for sc in ["VBDG", "VINV", "VOLV", "VVA", "VRC"] {
            assert_eq!(branches.iter().find(|b| b.sous_code == sc).unwrap().categorie_id, cat("Automobile"), "{}", sc);
        }
        assert_eq!(branches.iter().find(|b| b.code == "8.1.1.0").unwrap().categorie_id, cat("Incendie & Risques Divers"));
        assert_eq!(branches.iter().find(|b| b.code == "13.1").unwrap().categorie_id, cat("Responsabilité Civile"));
        assert!(branches.iter().filter(|b| b.branche.to_lowercase().contains("agricole")).all(|b| b.categorie_id == cat("Agricole")));

        // Create / Update / doublon
        let id = db.save_categorie(&models::Categorie { id: None, libelle: "Engineering".into(), ordre: 60, nb_branches: 0 }).unwrap();
        assert!(db.save_categorie(&models::Categorie { id: None, libelle: "engineering".into(), ordre: 0, nb_branches: 0 }).is_err());
        assert!(db.save_categorie(&models::Categorie { id: None, libelle: " ".into(), ordre: 0, nb_branches: 0 }).is_err());
        db.save_categorie(&models::Categorie { id: Some(id), libelle: "Engineering & Chantiers".into(), ordre: 60, nb_branches: 0 }).unwrap();
        // Affectation de lignes
        let ec: Vec<i64> = branches.iter().filter(|b| b.code.starts_with("9.5") || b.code.starts_with("9.4")).map(|b| b.id.unwrap()).collect();
        db.affecter_categorie(&ec, Some(id)).unwrap();
        let c = db.get_categories().unwrap().into_iter().find(|c| c.id == Some(id)).unwrap();
        assert_eq!(c.libelle, "Engineering & Chantiers");
        assert_eq!(c.nb_branches, ec.len() as i64);
        // Ligne de paramètre : catégorie inexistante refusée
        let mut b = branches[0].clone();
        b.categorie_id = Some(9999);
        assert!(db.save_branche(&b).is_err());
        // Delete : les lignes deviennent « sans catégorie », taux inchangés
        let avant = db.get_all_branches().unwrap();
        db.delete_categorie(id).unwrap();
        let apres = db.get_all_branches().unwrap();
        for i in &ec {
            let a = avant.iter().find(|b| b.id == Some(*i)).unwrap();
            let p = apres.iter().find(|b| b.id == Some(*i)).unwrap();
            assert_eq!(p.categorie_id, None);
            assert_eq!(p.taux_apport_courtier, a.taux_apport_courtier);
        }
        assert!(db.delete_categorie(id).is_err());
    }

    #[test]
    fn test_libelle_client_et_dates() {
        use models::{date_valide, libelle_clients};
        assert_eq!(
            libelle_clients([(Some("350021468/0"), Some("OPGI TAMANRASSET")), (Some("350021468/0"), Some("OPGI TAMANRASSET"))].into_iter()),
            Some("350021468/0 OPGI TAMANRASSET".to_string())
        );
        assert_eq!(
            libelle_clients([(Some("A/0"), Some("X")), (Some("B/0"), Some("Y"))].into_iter()),
            Some("A/0 X & B/0 Y".to_string())
        );
        let beaucoup: Vec<(String, String)> = (0..7).map(|i| (format!("C{}", i), format!("N{}", i))).collect();
        assert_eq!(libelle_clients(beaucoup.iter().map(|(c, n)| (Some(c.as_str()), Some(n.as_str())))), Some("7 clients".to_string()));
        assert_eq!(libelle_clients(std::iter::empty()), None);
        for ok in ["", "01/01/2026", "29/02/2024", "31/12/2026"] {
            assert!(date_valide(ok), "{}", ok);
        }
        for ko in ["2026-01-01", "1/1/2026", "31/02/2026", "29/02/2026", "32/01/2026", "01/13/2026", "01/01/26"] {
            assert!(!date_valide(ko), "{}", ko);
        }
    }

    #[test]
    fn test_crud_parametres() {
        let db = test_db();
        let initial = db.get_all_branches().unwrap().len();
        let mut b = BrancheParam {
            id: None,
            code: "99.9".to_string(),
            branche: "BRANCHE TEST".to_string(),
            sous_code: "vtest".to_string(),
            sous_libelle: "Garantie test".to_string(),
            taux_apport_courtier: 0.12,
            taux_apport_aga: 0.08,
            taux_gestion_aga: 0.04,
            alias_import: " xx , yy ".to_string(),
            groupe_bordereau: "GROUPE TEST".to_string(),
            ordre_bordereau: 5,
            categorie_id: None,
        };
        // Create
        let id = db.save_branche(&b).unwrap();
        let created = db.get_all_branches().unwrap().into_iter().find(|x| x.id == Some(id)).unwrap();
        assert_eq!(created.sous_code, "VTEST");
        assert_eq!(created.alias_import, "XX,YY");
        // Update
        b.id = Some(id);
        b.taux_apport_courtier = 0.15;
        db.save_branche(&b).unwrap();
        let updated = db.get_all_branches().unwrap().into_iter().find(|x| x.id == Some(id)).unwrap();
        assert_eq!(updated.taux_apport_courtier, 0.15);
        // Validations
        let mut dup = b.clone();
        dup.id = None;
        dup.alias_import = String::new();
        assert!(db.save_branche(&dup).unwrap_err().contains("existe déjà"));
        let mut dup_alias = b.clone();
        dup_alias.id = None;
        dup_alias.sous_code = "VTEST2".to_string();
        dup_alias.alias_import = "YY".to_string();
        assert!(db.save_branche(&dup_alias).unwrap_err().contains("alias"));
        let mut bad = b.clone();
        bad.id = None;
        bad.sous_code = "VTEST3".to_string();
        bad.alias_import = String::new();
        bad.taux_apport_aga = 1.5;
        assert!(db.save_branche(&bad).is_err());
        bad.taux_apport_aga = 0.1;
        bad.code = " ".to_string();
        assert!(db.save_branche(&bad).is_err());
        // Même sous-code sur une autre branche (code ministériel différent) : autorisé
        let mut autre = b.clone();
        autre.id = None;
        autre.code = "99.8".to_string();
        autre.alias_import = String::new();
        let autre_id = db.save_branche(&autre).unwrap();
        // Delete
        db.delete_branche(id).unwrap();
        db.delete_branche(autre_id).unwrap();
        assert_eq!(db.get_all_branches().unwrap().len(), initial);
        assert!(db.delete_branche(id).is_err());
    }

    #[test]
    fn test_crud_intermediaires() {
        let db = test_db();
        let c = new_courtier(&db, "CRT-900");
        assert_eq!(c.type_intermediaire, "COURTIER");
        // Code en double refusé (insensible à la casse)
        let mut dup = c.clone();
        dup.id = None;
        dup.code = "crt-900".to_string();
        assert!(db.save_intermediaire(&dup).unwrap_err().contains("déjà utilisé"));
        // Type invalide refusé
        let mut bad = c.clone();
        bad.type_intermediaire = "AUTRE".to_string();
        assert!(db.save_intermediaire(&bad).is_err());
        // Update
        let mut upd = c.clone();
        upd.nom = "NOUVEAU NOM".to_string();
        upd.actif = false;
        db.save_intermediaire(&upd).unwrap();
        let reread = db.get_intermediaire(c.id.unwrap()).unwrap().unwrap();
        assert_eq!(reread.nom, "NOUVEAU NOM");
        assert!(!reread.actif);
        // Delete (aucun bordereau rattaché)
        db.delete_intermediaire(c.id.unwrap()).unwrap();
        assert!(db.get_intermediaire(c.id.unwrap()).unwrap().is_none());
        assert!(db.delete_intermediaire(c.id.unwrap()).is_err());
    }

    #[test]
    fn test_05_2025_calcul_aga_exact() {
        let db = test_db();
        let Some((file, _)) = ciris_file(&db) else { return };
        let aga = inter_by_code(&db, "492");
        let b = CalculationEngine::compute_multi_bordereau(&req_for(&aga, vec![file], 2025, Some(5)), &db.get_all_branches().unwrap(), &std::collections::HashMap::new());
        for l in &b.lignes {
            println!("{} | {} | {:.2} | {} | {} | {:.2}", l.code_garantie, l.branche_nom, l.prime_nette, l.taux_apport, l.taux_gestion, l.montant_apport);
        }
        assert!(b.anomalies.iter().all(|a| !a.est_bloquante()), "{:?}", b.anomalies);
        assert_eq!(b.total_prime_nette, 2272025.22);
        assert_eq!(b.total_commission_apport, 181094.77);
        assert_eq!(b.total_commission_gestion, 109637.19);
        assert_eq!(b.total_commission_ht, 290731.96);
        assert_eq!(b.total_tva, 55239.07);
        assert_eq!(b.total_commission_ttc, 345971.03);
        assert_eq!(b.numero_bordereau, "BORD-AGA-492-2025-05");
        assert_eq!(b.intermediaire_id, aga.id);

        let temp = std::env::temp_dir().join(format!("test_bordereau_{}.xlsx", std::process::id()));
        assert!(ExcelExporter::export_bordereau_to_file(&b, &temp).is_ok());
        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn test_crud_bordereau_fichiers_et_reouverture_sans_modification_des_taux() {
        let db = test_db();
        let Some((file, bytes)) = ciris_file(&db) else { return };
        let aga = inter_by_code(&db, "492");
        let branches = db.get_all_branches().unwrap();
        let mut computed = CalculationEngine::compute_multi_bordereau(&req_for(&aga, vec![file.clone()], 2025, Some(5)), &branches, &std::collections::HashMap::new());

        // Désactivation d'une ligne : exclue des totaux
        computed.lignes[0].desactive = true;
        CalculationEngine::recompute_bordereau(&mut computed);
        let excluded_ht = computed.lignes[0].montant_ht;
        assert_eq!(computed.total_commission_ht, CalculationEngine::round_cents(290731.96 - excluded_ht));

        // Create avec le fichier source rattaché
        let imports = [ImportAEnregistrer { fichier: &file, contenu: Some(bytes.clone()) }];
        let id = db.save_bordereau(&computed, Some(&imports)).unwrap();
        let saved = db.get_bordereau_by_id(id).unwrap().unwrap();
        assert_eq!(saved.intermediaire_id, aga.id);
        assert_eq!(saved.fichiers.len(), 1);
        assert_eq!(saved.fichiers[0].type_piece, "IMPORT");
        assert_eq!(saved.fichiers[0].taille, bytes.len() as i64);
        assert_eq!(db.get_fichier_contenu(saved.fichiers[0].id.unwrap()).unwrap().2, bytes);

        // Le même fichier ne peut pas être rattaché à un second bordereau
        let mut other = computed.clone();
        other.id = None;
        let err = db.save_bordereau(&other, Some(&imports)).unwrap_err();
        assert!(err.contains("déjà rattaché"), "{}", err);
        // Sans fichier, un second bordereau de même numéro reçoit un suffixe
        let id2 = db.save_bordereau(&other, None).unwrap();
        assert_eq!(db.get_bordereau_by_id(id2).unwrap().unwrap().numero_bordereau, format!("{}-2", computed.numero_bordereau));
        db.delete_bordereau(id2).unwrap();

        // Modification du barème APRÈS enregistrement
        for mut b in db.get_all_branches().unwrap() {
            b.taux_apport_aga = 0.99;
            b.taux_gestion_aga = 0.99;
            db.save_branche(&b).unwrap();
        }

        // Read : taux et montants identiques à l'enregistrement
        let reopened = db.get_bordereau_by_id(id).unwrap().unwrap();
        assert_eq!(reopened.lignes.len(), computed.lignes.len());
        for (a, b) in reopened.lignes.iter().zip(computed.lignes.iter()) {
            assert_eq!(a.code_garantie, b.code_garantie);
            assert_eq!(a.taux_apport, b.taux_apport);
            assert_eq!(a.taux_gestion, b.taux_gestion);
            assert_eq!(a.prime_nette, b.prime_nette);
            assert_eq!(a.montant_ttc, b.montant_ttc);
            assert_eq!(a.desactive, b.desactive);
            assert_eq!(a.contributions.len(), b.contributions.len());
        }
        assert_eq!(reopened.total_commission_ttc, computed.total_commission_ttc);

        // Update (réactivation d'une ligne) : recalcul avec les taux figés, fichiers conservés
        let mut upd = reopened.clone();
        upd.lignes[0].desactive = false;
        CalculationEngine::recompute_bordereau(&mut upd);
        assert_eq!(db.save_bordereau(&upd, None).unwrap(), id);
        let after = db.get_bordereau_by_id(id).unwrap().unwrap();
        assert_eq!(after.total_commission_ht, 290731.96);
        assert_eq!(after.fichiers.len(), 1);

        // Statuts : VALIDE verrouille les modifications, REGLE interdit la suppression
        assert!(db.update_bordereau_statut(id, "REGLE").is_err());
        db.update_bordereau_statut(id, "VALIDE").unwrap();
        assert!(db.save_bordereau(&upd, None).unwrap_err().contains("VALIDE"));
        db.update_bordereau_statut(id, "REGLE").unwrap();
        assert!(db.delete_bordereau(id).is_err());

        // Pièces jointes : ajout, lecture, suppression ; un import source ne se supprime pas seul
        let pj = db.add_piece_jointe(id, "bordereau_signe.pdf", "h", b"%PDF-test").unwrap();
        let fichiers = db.list_fichiers(id).unwrap();
        assert_eq!(fichiers.len(), 2);
        assert_eq!(db.get_fichier_contenu(pj).unwrap().2, b"%PDF-test".to_vec());
        let import_id = fichiers.iter().find(|f| f.type_piece == "IMPORT").unwrap().id.unwrap();
        assert!(db.delete_piece_jointe(import_id).is_err());
        db.delete_piece_jointe(pj).unwrap();
        assert!(db.add_piece_jointe(9999, "x.pdf", "h", b"x").is_err());

        // L'intermédiaire ne peut plus être supprimé ni changer de code
        assert!(db.delete_intermediaire(aga.id.unwrap()).is_err());
        let mut code_change = aga.clone();
        code_change.code = "493".to_string();
        assert!(db.save_intermediaire(&code_change).is_err());

        // Delete : retour en VALIDE puis suppression (lignes et fichiers en cascade)
        db.update_bordereau_statut(id, "VALIDE").unwrap();
        db.delete_bordereau(id).unwrap();
        assert!(db.get_bordereau_by_id(id).unwrap().is_none());
        assert!(db.list_fichiers(id).unwrap().is_empty());
        assert!(db.find_hash_usage(&file.hash, None).unwrap().is_none());
    }

    #[test]
    fn test_garantie_non_parametree_bloque_l_enregistrement() {
        let db = test_db();
        let Some((mut file, _)) = ciris_file(&db) else { return };
        // Suppression du paramètre VASS : la garantie devient non paramétrée
        let vass = db.get_all_branches().unwrap().into_iter().find(|b| b.sous_code == "VASS").unwrap();
        db.delete_branche(vass.id.unwrap()).unwrap();
        file.anomalies.clear();
        let aga = inter_by_code(&db, "492");
        let mut b = CalculationEngine::compute_multi_bordereau(&req_for(&aga, vec![file], 2025, Some(5)), &db.get_all_branches().unwrap(), &std::collections::HashMap::new());
        let np = b.lignes.iter().position(|l| l.non_parametre).expect("ligne non paramétrée");
        assert_eq!(b.lignes[np].taux_apport, 0.0);
        assert_eq!(b.lignes[np].source_code.as_deref(), Some("VASS"));
        assert!(b.anomalies.iter().any(|a| a.est_bloquante() && a.message.contains("VASS")));
        assert!(db.save_bordereau(&b, None).unwrap_err().contains("sans taux"));
        assert!(!b.anomalies_bloquantes().is_empty());
        // Ligne exclue par le commercial : l'enregistrement devient possible
        b.lignes[np].desactive = true;
        CalculationEngine::recompute_bordereau(&mut b);
        assert!(b.anomalies_bloquantes().is_empty());
        assert!(db.save_bordereau(&b, None).is_ok());
    }

    #[test]
    fn test_bordereau_sans_intermediaire_ou_incoherent_refuse() {
        let db = test_db();
        let Some((file, _)) = ciris_file(&db) else { return };
        let aga = inter_by_code(&db, "492");
        let mut b = CalculationEngine::compute_multi_bordereau(&req_for(&aga, vec![file], 2025, Some(5)), &db.get_all_branches().unwrap(), &std::collections::HashMap::new());
        let mut sans = b.clone();
        sans.intermediaire_id = None;
        assert!(db.save_bordereau(&sans, None).is_err());
        let c = new_courtier(&db, "CRT-1");
        b.intermediaire_id = c.id;
        assert!(db.save_bordereau(&b, None).unwrap_err().contains("Incohérence"));
    }

    #[test]
    fn test_multi_fichiers_courtier_cumul() {
        let db = test_db();
        let known = db.known_sous_codes().unwrap();
        let (Ok(pdf), Ok(xls)) = (std::fs::read("../Import/RCCDVF.pdf"), std::fs::read("../Import/Divers.xlsx")) else { return };
        let flotte = FlottePdfParser::parse_from_bytes(&pdf, "RCCDVF.pdf", &known).unwrap();
        assert_eq!(flotte.total_prime_nette, 764317.19);
        let excel = ExcelProductionParser::parse_from_bytes(&xls, "Divers.xlsx", &crate::parser::ExcelSelection { point_vente: Some("347".into()), clients: vec!["347024203/0".into()] }, None).unwrap();
        assert_eq!(excel.items.len(), 10);
        let courtier = new_courtier(&db, "CRT-347");
        let branches = db.get_all_branches().unwrap();

        // Police 347/VF/304/0/0 présente dans les deux fichiers : comptée une seule fois (PDF importé en premier),
        // signalée sans bloquer l'enregistrement
        let b = CalculationEngine::compute_multi_bordereau(&req_for(&courtier, vec![flotte.clone(), excel.clone()], 2025, None), &branches, &std::collections::HashMap::new());
        assert_eq!(b.total_prime_nette, CalculationEngine::round_cents(flotte.total_prime_nette + excel.total_prime_nette - 764317.19));
        assert!(b.anomalies_bloquantes().is_empty(), "{:?}", b.anomalies);
        let avert: Vec<_> = b.anomalies.iter().filter(|a| a.message.contains("présente dans deux fichiers")).collect();
        assert_eq!(avert.len(), 1, "{:?}", b.anomalies);
        assert!(avert[0].message.contains("347/VF/304/0/0") && avert[0].message.contains("RCCDVF.pdf"));
        // Ordre inverse : c'est la ligne Excel qui est retenue, le total est identique
        let inv = CalculationEngine::compute_multi_bordereau(&req_for(&courtier, vec![excel.clone(), flotte.clone()], 2025, None), &branches, &std::collections::HashMap::new());
        assert_eq!(inv.total_prime_nette, b.total_prime_nette);

        // Excel seul : chaque code branche prend le taux du paramètre auquel il est rattaché (alias d'import)
        let e = CalculationEngine::compute_multi_bordereau(&req_for(&courtier, vec![excel], 2025, None), &branches, &std::collections::HashMap::new());
        assert!(e.anomalies_bloquantes().is_empty(), "{:?}", e.anomalies);
        let get = |label: &str| e.lignes.iter().find(|l| l.libelle_garantie == label).unwrap_or_else(|| panic!("{}", label));
        assert_eq!(get("RC AUTOMOBILE").prime_nette, 1593.59);
        assert_eq!(get("RC AUTOMOBILE").taux_apport, 0.05);
        assert_eq!(get("AUTOMOBILE FACULTATIF").prime_nette, 892965.95);
        assert_eq!(get("AUTOMOBILE FACULTATIF").taux_apport, 0.10);
        assert_eq!(get("CATNAT").prime_nette, 1609023.58);
        assert_eq!(get("INCENDIE").prime_nette, 2985474.57);
        assert_eq!(get("INCENDIE").taux_apport, 0.08);
        assert_eq!(get("Bris de Machine").prime_nette, 1823923.02);

        // PDF de flotte seul, sans cumul : une ligne par garantie avec le barème courtier
        let mut req = req_for(&courtier, vec![flotte], 2025, None);
        req.cumuler_par_branche = false;
        let detail = CalculationEngine::compute_multi_bordereau(&req, &branches, &std::collections::HashMap::new());
        assert!(detail.lignes.iter().any(|l| l.code_garantie == "VRC"));
        assert_eq!(detail.total_prime_nette, 764317.19);
        assert!(detail.anomalies_bloquantes().is_empty());
    }
}

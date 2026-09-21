pub mod models;
pub mod db;
pub mod calculation;
pub mod parser;
pub mod export;
pub mod commands;

use std::sync::Mutex;
use db::DatabaseManager;
use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_manager = DatabaseManager::new().expect("Échec d'initialisation de la base SQLite dans ./BDD");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            db: Mutex::new(db_manager),
        })
        .invoke_handler(tauri::generate_handler![
            get_db_info,
            backup_database,
            get_branches,
            update_branche,
            get_intermediaires,
            save_intermediaire,
            parse_ciris_pdf,
            parse_ciris_pdf_base64,
            calculate_bordereau_from_pdf,
            save_bordereau,
            get_bordereaux,
            get_bordereau,
            delete_bordereau,
            export_excel,
        ])
        .run(tauri::generate_context!())
        .expect("Erreur lors de l'exécution de l'application Tauri");
}

#[cfg(test)]
mod tests {
    use super::*;
    use calculation::CalculationEngine;
    use parser::CirisPdfParser;
    use export::ExcelExporter;

    #[test]
    fn test_db_init_and_branches() {
        let db = DatabaseManager::new().expect("DB Init failed");
        let branches = db.get_all_branches().expect("Failed to fetch branches");
        assert!(branches.len() >= 70, "Must have loaded at least 70 branches from initial_rates.json");
        let info = db.get_info().expect("Failed to fetch info");
        assert_eq!(info.version, 1);
    }

    #[test]
    fn test_05_2025_exact_calculation() {
        let db = DatabaseManager::new().expect("DB Init failed");
        let branches = db.get_all_branches().expect("Failed to fetch branches");
        let pdf_data = CirisPdfParser::get_verified_05_2025_data();

        let mut total_prime = 0.0;
        let mut total_apport = 0.0;
        let mut total_gestion = 0.0;

        for item in &pdf_data.items {
            let matched = CalculationEngine::match_branche(item, &branches);
            let ligne = CalculationEngine::compute_ligne(item, matched, "AGA", 0.19);
            total_prime += ligne.prime_nette;
            total_apport += ligne.montant_apport;
            total_gestion += ligne.montant_gestion;
        }

        total_prime = CalculationEngine::round_cents(total_prime);
        total_apport = CalculationEngine::round_cents(total_apport);
        total_gestion = CalculationEngine::round_cents(total_gestion);
        let total_ht = CalculationEngine::round_cents(total_apport + total_gestion);
        let tva = CalculationEngine::round_cents(total_ht * 0.19);
        let total_ttc = CalculationEngine::round_cents(total_ht + tva);

        println!("PRIME: {}, APPORT: {}, GESTION: {}, HT: {}, TVA: {}, TTC: {}",
                 total_prime, total_apport, total_gestion, total_ht, tva, total_ttc);

        assert_eq!(total_prime, 2272025.22);
        assert_eq!(total_apport, 181094.77);
        assert_eq!(total_gestion, 109637.19);
        assert_eq!(total_ht, 290731.96);
        assert_eq!(tva, 55239.07);
        assert_eq!(total_ttc, 345971.03);
    }

    #[test]
    fn test_excel_export_generation() {
        let db = DatabaseManager::new().expect("DB Init failed");
        let branches = db.get_all_branches().expect("Failed to fetch branches");
        let pdf_data = CirisPdfParser::get_verified_05_2025_data();

        let mut lignes = Vec::new();
        let mut total_prime = 0.0;
        let mut total_apport = 0.0;
        let mut total_gestion = 0.0;

        for item in &pdf_data.items {
            let matched = CalculationEngine::match_branche(item, &branches);
            let ligne = CalculationEngine::compute_ligne(item, matched, "AGA", 0.19);
            total_prime += ligne.prime_nette;
            total_apport += ligne.montant_apport;
            total_gestion += ligne.montant_gestion;
            lignes.push(ligne);
        }

        let total_ht = CalculationEngine::round_cents(total_apport + total_gestion);
        let tva = CalculationEngine::round_cents(total_ht * 0.19);
        let total_ttc = CalculationEngine::round_cents(total_ht + tva);

        let b = models::Bordereau {
            id: None,
            intermediaire_id: None,
            intermediaire_nom: "AGA ABDELLAH".to_string(),
            intermediaire_code: "492".to_string(),
            type_intermediaire: "AGA".to_string(),
            numero_bordereau: "TEST-05-2025".to_string(),
            exercice: 2025,
            mois: Some(5),
            code_client: None,
            succursale: "SUCCURSALE SUD".to_string(),
            departement: "Département Commercial".to_string(),
            tva_taux: 0.19,
            date_generation: "2026-09-20".to_string(),
            total_prime_nette: total_prime,
            total_commission_apport: total_apport,
            total_commission_gestion: total_gestion,
            total_commission_ht: total_ht,
            total_tva: tva,
            total_commission_ttc: total_ttc,
            statut: "VALIDE".to_string(),
            lignes,
        };

        let temp_file = std::env::temp_dir().join("test_bordereau.xlsx");
        let res = ExcelExporter::export_bordereau_to_file(&b, &temp_file);
        assert!(res.is_ok(), "Excel export should succeed");
        assert!(temp_file.exists(), "Excel file should exist");
        let _ = std::fs::remove_file(temp_file);
    }
}

pub mod models;
pub mod db;
pub mod calculation;
pub mod parser;
pub mod export;
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
            get_intermediaires,
            save_intermediaire,
            delete_intermediaire,
            scan_excel_clients,
            scan_excel_clients_base64,
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
        assert_eq!(db.get_info().unwrap().version, 4);
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
        let src = std::path::Path::new("../BDD/commissions.db");
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
        let dir = std::env::temp_dir().join(format!("commissions_migration_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("BDD")).unwrap();
        let target = dir.join("BDD").join("commissions.db");
        let _ = std::fs::remove_file(&target);
        before
            .execute_batch(&format!("VACUUM INTO '{}'", target.to_string_lossy().replace('\'', "''")))
            .unwrap();
        drop(before);

        let db = DatabaseManager::with_path(&target).expect("migration");
        assert_eq!(db.get_info().unwrap().version, 4);
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
        let excel = ExcelProductionParser::parse_from_bytes(&xls, "Divers.xlsx", Some("SPA FSP")).unwrap();
        let courtier = new_courtier(&db, "CRT-347");
        let mut b = CalculationEngine::compute_multi_bordereau(&req_for(&courtier, vec![excel], 2025, None), &db.get_all_branches().unwrap());
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
        let b = CalculationEngine::compute_multi_bordereau(&req_for(&aga, vec![file], 2025, Some(5)), &db.get_all_branches().unwrap());
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
        let mut computed = CalculationEngine::compute_multi_bordereau(&req_for(&aga, vec![file.clone()], 2025, Some(5)), &branches);

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
        let mut b = CalculationEngine::compute_multi_bordereau(&req_for(&aga, vec![file], 2025, Some(5)), &db.get_all_branches().unwrap());
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
        let mut b = CalculationEngine::compute_multi_bordereau(&req_for(&aga, vec![file], 2025, Some(5)), &db.get_all_branches().unwrap());
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
        let excel = ExcelProductionParser::parse_from_bytes(&xls, "Divers.xlsx", Some("SPA FSP")).unwrap();
        assert_eq!(excel.items.len(), 10);
        let courtier = new_courtier(&db, "CRT-347");
        let branches = db.get_all_branches().unwrap();

        // Police 347/VF/304/0/0 présente dans les deux fichiers : comptée une seule fois (PDF importé en premier),
        // signalée sans bloquer l'enregistrement
        let b = CalculationEngine::compute_multi_bordereau(&req_for(&courtier, vec![flotte.clone(), excel.clone()], 2025, None), &branches);
        assert_eq!(b.total_prime_nette, CalculationEngine::round_cents(flotte.total_prime_nette + excel.total_prime_nette - 764317.19));
        assert!(b.anomalies_bloquantes().is_empty(), "{:?}", b.anomalies);
        let avert: Vec<_> = b.anomalies.iter().filter(|a| a.message.contains("présente dans deux fichiers")).collect();
        assert_eq!(avert.len(), 1, "{:?}", b.anomalies);
        assert!(avert[0].message.contains("347/VF/304/0/0") && avert[0].message.contains("RCCDVF.pdf"));
        // Ordre inverse : c'est la ligne Excel qui est retenue, le total est identique
        let inv = CalculationEngine::compute_multi_bordereau(&req_for(&courtier, vec![excel.clone(), flotte.clone()], 2025, None), &branches);
        assert_eq!(inv.total_prime_nette, b.total_prime_nette);

        // Excel seul : chaque code branche prend le taux du paramètre auquel il est rattaché (alias d'import)
        let e = CalculationEngine::compute_multi_bordereau(&req_for(&courtier, vec![excel], 2025, None), &branches);
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
        let detail = CalculationEngine::compute_multi_bordereau(&req, &branches);
        assert!(detail.lignes.iter().any(|l| l.code_garantie == "VRC"));
        assert_eq!(detail.total_prime_nette, 764317.19);
        assert!(detail.anomalies_bloquantes().is_empty());
    }
}

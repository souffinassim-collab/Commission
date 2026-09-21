use std::fs;
use std::path::{Path, PathBuf};
use chrono::Local;
use rusqlite::{params, Connection, Result};
use crate::models::{BrancheParam, Intermediaire, Bordereau, BordereauLigne, DbInfo};

pub struct DatabaseManager {
    db_path: PathBuf,
}

impl DatabaseManager {
    pub fn new() -> Result<Self> {
        let bdd_dir = Self::get_bdd_directory();
        if !bdd_dir.exists() {
            fs::create_dir_all(&bdd_dir).unwrap_or_default();
        }

        let backups_dir = bdd_dir.join("backups");
        if !backups_dir.exists() {
            fs::create_dir_all(&backups_dir).unwrap_or_default();
        }

        let db_path = bdd_dir.join("commissions.db");
        let manager = Self { db_path };
        manager.run_migrations()?;
        Ok(manager)
    }

    fn get_bdd_directory() -> PathBuf {
        // En mode dev (exécuté depuis target/debug ou target/release sous src-tauri),
        // on place BDD à la racine du projet pour éviter que le watcher de Tauri dev
        // ne détecte l'écriture dans SQLite et ne relance l'application en boucle infinie !
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let exe_str = exe_dir.to_string_lossy().to_lowercase();
                if exe_str.contains("target\\debug") || exe_str.contains("target/debug")
                    || exe_str.contains("target\\release") || exe_str.contains("target/release") {
                    let mut cur = exe_dir.to_path_buf();
                    while let Some(parent) = cur.parent() {
                        if cur.file_name().and_then(|n| n.to_str()) == Some("src-tauri") {
                            return parent.join("BDD");
                        }
                        cur = parent.to_path_buf();
                    }
                }
                // En mode standalone/portable (ex: Gestion_Commissions_Portable.exe à côté de son dossier BDD)
                return exe_dir.join("BDD");
            }
        }
        PathBuf::from("BDD")
    }

    pub fn get_connection(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;"
        )?;
        Ok(conn)
    }

    fn run_migrations(&self) -> Result<()> {
        let mut conn = Connection::open(&self.db_path)?;

        // Création table de suivi des migrations
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL
            );",
            [],
        )?;

        // Sauvegarde automatique pré-migration si la base contient déjà des tables
        if self.has_any_user_tables(&conn)? {
            if let Err(e) = self.create_pre_migration_backup() {
                eprintln!("Avertissement sauvegarde pré-migration: {:?}", e);
            }
        }

        let current_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Migration V1: Schéma initial + injection des taux Excel de référence
        if current_version < 1 {
            let tx = conn.transaction()?;
            
            tx.execute(
                "CREATE TABLE IF NOT EXISTS branches (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    code TEXT NOT NULL,
                    branche TEXT NOT NULL,
                    sous_code TEXT DEFAULT '',
                    sous_libelle TEXT DEFAULT '',
                    taux_apport_courtier REAL NOT NULL DEFAULT 0.0,
                    taux_apport_aga REAL NOT NULL DEFAULT 0.0,
                    taux_gestion_aga REAL NOT NULL DEFAULT 0.0,
                    created_at TEXT DEFAULT (datetime('now', 'localtime'))
                );",
                [],
            )?;

            tx.execute(
                "CREATE TABLE IF NOT EXISTS intermediaires (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    type_intermediaire TEXT NOT NULL CHECK(type_intermediaire IN ('AGA', 'COURTIER')),
                    code TEXT UNIQUE NOT NULL,
                    nom TEXT NOT NULL,
                    succursale TEXT NOT NULL DEFAULT 'SUCCURSALE SUD',
                    departement TEXT NOT NULL DEFAULT 'Département Commercial',
                    contact TEXT,
                    adresse TEXT,
                    tva_applicable REAL NOT NULL DEFAULT 0.19,
                    actif INTEGER NOT NULL DEFAULT 1,
                    created_at TEXT DEFAULT (datetime('now', 'localtime'))
                );",
                [],
            )?;

            tx.execute(
                "CREATE TABLE IF NOT EXISTS bordereaux (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    intermediaire_id INTEGER REFERENCES intermediaires(id),
                    intermediaire_nom TEXT NOT NULL,
                    intermediaire_code TEXT NOT NULL,
                    type_intermediaire TEXT NOT NULL,
                    numero_bordereau TEXT UNIQUE NOT NULL,
                    exercice INTEGER NOT NULL,
                    mois INTEGER,
                    code_client TEXT,
                    succursale TEXT NOT NULL,
                    departement TEXT NOT NULL,
                    tva_taux REAL NOT NULL DEFAULT 0.19,
                    date_generation TEXT NOT NULL,
                    total_prime_nette REAL NOT NULL DEFAULT 0.0,
                    total_commission_apport REAL NOT NULL DEFAULT 0.0,
                    total_commission_gestion REAL NOT NULL DEFAULT 0.0,
                    total_commission_ht REAL NOT NULL DEFAULT 0.0,
                    total_tva REAL NOT NULL DEFAULT 0.0,
                    total_commission_ttc REAL NOT NULL DEFAULT 0.0,
                    statut TEXT NOT NULL DEFAULT 'BROUILLON',
                    created_at TEXT DEFAULT (datetime('now', 'localtime'))
                );",
                [],
            )?;

            tx.execute(
                "CREATE TABLE IF NOT EXISTS bordereau_lignes (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    bordereau_id INTEGER NOT NULL REFERENCES bordereaux(id) ON DELETE CASCADE,
                    branche_id INTEGER REFERENCES branches(id),
                    code_garantie TEXT NOT NULL,
                    libelle_garantie TEXT NOT NULL,
                    branche_nom TEXT NOT NULL,
                    prime_nette REAL NOT NULL DEFAULT 0.0,
                    taux_apport REAL NOT NULL DEFAULT 0.0,
                    taux_gestion REAL NOT NULL DEFAULT 0.0,
                    montant_apport REAL NOT NULL DEFAULT 0.0,
                    montant_gestion REAL NOT NULL DEFAULT 0.0,
                    montant_ht REAL NOT NULL DEFAULT 0.0,
                    montant_tva REAL NOT NULL DEFAULT 0.0,
                    montant_ttc REAL NOT NULL DEFAULT 0.0
                );",
                [],
            )?;

            // Seed initial des branches depuis le JSON extrait
            let initial_rates_json = include_str!("../../initial_rates.json");
            if let Ok(rates) = serde_json::from_str::<Vec<serde_json::Value>>(initial_rates_json) {
                for item in rates {
                    let code = item.get("code").and_then(|v| v.as_str()).unwrap_or("");
                    let branche = item.get("branche").and_then(|v| v.as_str()).unwrap_or("");
                    let sous_code = item.get("sous_code").and_then(|v| v.as_str()).unwrap_or("");
                    let sous_libelle = item.get("sous_libelle").and_then(|v| v.as_str()).unwrap_or("");
                    let t_courtier = item.get("taux_apport_courtier").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let t_app_aga = item.get("taux_apport_aga").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let t_gest_aga = item.get("taux_gestion_aga").and_then(|v| v.as_f64()).unwrap_or(0.0);

                    tx.execute(
                        "INSERT INTO branches (code, branche, sous_code, sous_libelle, taux_apport_courtier, taux_apport_aga, taux_gestion_aga)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![code, branche, sous_code, sous_libelle, t_courtier, t_app_aga, t_gest_aga],
                    )?;
                }
            }

            // Seed de l'intermédiaire réel extrait du PDF (492 - AGA ABDELLAH)
            tx.execute(
                "INSERT OR IGNORE INTO intermediaires (type_intermediaire, code, nom, succursale, departement, tva_applicable, actif)
                 VALUES ('AGA', '492', 'AGA ABDELLAH', 'SUCCURSALE SUD', 'Département Commercial', 0.19, 1)",
                [],
            )?;

            let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            tx.execute(
                "INSERT INTO _migrations (version, name, applied_at) VALUES (1, 'initial_schema_and_rates', ?1)",
                params![now],
            )?;

            tx.commit()?;
            println!("Migration V1 exécutée avec succès !");
        }

        Ok(())
    }

    fn has_any_user_tables(&self, conn: &Connection) -> Result<bool> {
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='branches'",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        Ok(count > 0)
    }

    pub fn create_pre_migration_backup(&self) -> std::io::Result<PathBuf> {
        if !self.db_path.exists() {
            return Ok(PathBuf::new());
        }
        let now = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_dir = self.db_path.parent().unwrap_or(Path::new(".")).join("backups");
        fs::create_dir_all(&backup_dir)?;
        let backup_filename = format!("backup_commissions_{}.db", now);
        let backup_path = backup_dir.join(backup_filename);
        fs::copy(&self.db_path, &backup_path)?;
        Ok(backup_path)
    }

    pub fn get_info(&self) -> Result<DbInfo> {
        let conn = self.get_connection()?;
        let version: i32 = conn
            .query_row("SELECT COALESCE(MAX(version), 0) FROM _migrations", [], |row| row.get(0))
            .unwrap_or(0);
        let total_branches: i64 = conn
            .query_row("SELECT COUNT(*) FROM branches", [], |row| row.get(0))
            .unwrap_or(0);
        let total_intermediaires: i64 = conn
            .query_row("SELECT COUNT(*) FROM intermediaires", [], |row| row.get(0))
            .unwrap_or(0);
        let total_bordereaux: i64 = conn
            .query_row("SELECT COUNT(*) FROM bordereaux", [], |row| row.get(0))
            .unwrap_or(0);

        let backups_dir = self.db_path.parent().unwrap_or(Path::new(".")).join("backups");
        let mut last_backup = None;
        if let Ok(entries) = fs::read_dir(backups_dir) {
            let mut latest_time = None;
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if latest_time.map_or(true, |t| modified > t) {
                            latest_time = Some(modified);
                            last_backup = Some(entry.file_name().to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        Ok(DbInfo {
            db_path: self.db_path.to_string_lossy().to_string(),
            version,
            total_branches,
            total_intermediaires,
            total_bordereaux,
            last_backup,
        })
    }

    // CRUD Branches
    pub fn get_all_branches(&self) -> Result<Vec<BrancheParam>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, code, branche, sous_code, sous_libelle, taux_apport_courtier, taux_apport_aga, taux_gestion_aga 
             FROM branches ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(BrancheParam {
                id: Some(row.get(0)?),
                code: row.get(1)?,
                branche: row.get(2)?,
                sous_code: row.get(3)?,
                sous_libelle: row.get(4)?,
                taux_apport_courtier: row.get(5)?,
                taux_apport_aga: row.get(6)?,
                taux_gestion_aga: row.get(7)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn update_branche(&self, b: &BrancheParam) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE branches SET 
                code = ?1, 
                branche = ?2, 
                sous_code = ?3, 
                sous_libelle = ?4, 
                taux_apport_courtier = ?5, 
                taux_apport_aga = ?6, 
                taux_gestion_aga = ?7 
             WHERE id = ?8",
            params![
                b.code,
                b.branche,
                b.sous_code,
                b.sous_libelle,
                b.taux_apport_courtier,
                b.taux_apport_aga,
                b.taux_gestion_aga,
                b.id
            ],
        )?;
        Ok(())
    }

    // CRUD Intermédiaires
    pub fn get_all_intermediaires(&self) -> Result<Vec<Intermediaire>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, type_intermediaire, code, nom, succursale, departement, contact, adresse, tva_applicable, actif 
             FROM intermediaires ORDER BY nom ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let actif_int: i32 = row.get(9)?;
            Ok(Intermediaire {
                id: Some(row.get(0)?),
                type_intermediaire: row.get(1)?,
                code: row.get(2)?,
                nom: row.get(3)?,
                succursale: row.get(4)?,
                departement: row.get(5)?,
                contact: row.get(6)?,
                adresse: row.get(7)?,
                tva_applicable: row.get(8)?,
                actif: actif_int != 0,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn save_intermediaire(&self, item: &Intermediaire) -> Result<i64> {
        let conn = self.get_connection()?;
        if let Some(id) = item.id {
            conn.execute(
                "UPDATE intermediaires SET 
                    type_intermediaire = ?1,
                    code = ?2,
                    nom = ?3,
                    succursale = ?4,
                    departement = ?5,
                    contact = ?6,
                    adresse = ?7,
                    tva_applicable = ?8,
                    actif = ?9
                 WHERE id = ?10",
                params![
                    item.type_intermediaire,
                    item.code,
                    item.nom,
                    item.succursale,
                    item.departement,
                    item.contact,
                    item.adresse,
                    item.tva_applicable,
                    if item.actif { 1 } else { 0 },
                    id
                ],
            )?;
            Ok(id)
        } else {
            conn.execute(
                "INSERT INTO intermediaires 
                 (type_intermediaire, code, nom, succursale, departement, contact, adresse, tva_applicable, actif)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    item.type_intermediaire,
                    item.code,
                    item.nom,
                    item.succursale,
                    item.departement,
                    item.contact,
                    item.adresse,
                    item.tva_applicable,
                    if item.actif { 1 } else { 0 }
                ],
            )?;
            Ok(conn.last_insert_rowid())
        }
    }

    // CRUD Bordereaux
    pub fn save_bordereau(&self, b: &Bordereau) -> Result<i64> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let bordereau_id: i64 = if let Some(id) = b.id {
            tx.execute(
                "UPDATE bordereaux SET 
                    intermediaire_id = ?1,
                    intermediaire_nom = ?2,
                    intermediaire_code = ?3,
                    type_intermediaire = ?4,
                    numero_bordereau = ?5,
                    exercice = ?6,
                    mois = ?7,
                    code_client = ?8,
                    succursale = ?9,
                    departement = ?10,
                    tva_taux = ?11,
                    date_generation = ?12,
                    total_prime_nette = ?13,
                    total_commission_apport = ?14,
                    total_commission_gestion = ?15,
                    total_commission_ht = ?16,
                    total_tva = ?17,
                    total_commission_ttc = ?18,
                    statut = ?19
                 WHERE id = ?20",
                params![
                    b.intermediaire_id,
                    b.intermediaire_nom,
                    b.intermediaire_code,
                    b.type_intermediaire,
                    b.numero_bordereau,
                    b.exercice,
                    b.mois,
                    b.code_client,
                    b.succursale,
                    b.departement,
                    b.tva_taux,
                    b.date_generation,
                    b.total_prime_nette,
                    b.total_commission_apport,
                    b.total_commission_gestion,
                    b.total_commission_ht,
                    b.total_tva,
                    b.total_commission_ttc,
                    b.statut,
                    id
                ],
            )?;
            // Nettoyage des anciennes lignes
            tx.execute("DELETE FROM bordereau_lignes WHERE bordereau_id = ?1", params![id])?;
            id
        } else {
            tx.execute(
                "INSERT INTO bordereaux 
                 (intermediaire_id, intermediaire_nom, intermediaire_code, type_intermediaire, numero_bordereau, 
                  exercice, mois, code_client, succursale, departement, tva_taux, date_generation,
                  total_prime_nette, total_commission_apport, total_commission_gestion, total_commission_ht,
                  total_tva, total_commission_ttc, statut)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                params![
                    b.intermediaire_id,
                    b.intermediaire_nom,
                    b.intermediaire_code,
                    b.type_intermediaire,
                    b.numero_bordereau,
                    b.exercice,
                    b.mois,
                    b.code_client,
                    b.succursale,
                    b.departement,
                    b.tva_taux,
                    b.date_generation,
                    b.total_prime_nette,
                    b.total_commission_apport,
                    b.total_commission_gestion,
                    b.total_commission_ht,
                    b.total_tva,
                    b.total_commission_ttc,
                    b.statut
                ],
            )?;
            tx.last_insert_rowid()
        };

        for l in &b.lignes {
            tx.execute(
                "INSERT INTO bordereau_lignes 
                 (bordereau_id, branche_id, code_garantie, libelle_garantie, branche_nom, prime_nette,
                  taux_apport, taux_gestion, montant_apport, montant_gestion, montant_ht, montant_tva, montant_ttc)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    bordereau_id,
                    l.branche_id,
                    l.code_garantie,
                    l.libelle_garantie,
                    l.branche_nom,
                    l.prime_nette,
                    l.taux_apport,
                    l.taux_gestion,
                    l.montant_apport,
                    l.montant_gestion,
                    l.montant_ht,
                    l.montant_tva,
                    l.montant_ttc
                ],
            )?;
        }

        tx.commit()?;
        Ok(bordereau_id)
    }

    pub fn get_all_bordereaux(&self) -> Result<Vec<Bordereau>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, intermediaire_id, intermediaire_nom, intermediaire_code, type_intermediaire,
                    numero_bordereau, exercice, mois, code_client, succursale, departement,
                    tva_taux, date_generation, total_prime_nette, total_commission_apport,
                    total_commission_gestion, total_commission_ht, total_tva, total_commission_ttc, statut
             FROM bordereaux ORDER BY id DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Bordereau {
                id: Some(row.get(0)?),
                intermediaire_id: row.get(1)?,
                intermediaire_nom: row.get(2)?,
                intermediaire_code: row.get(3)?,
                type_intermediaire: row.get(4)?,
                numero_bordereau: row.get(5)?,
                exercice: row.get(6)?,
                mois: row.get(7)?,
                code_client: row.get(8)?,
                succursale: row.get(9)?,
                departement: row.get(10)?,
                tva_taux: row.get(11)?,
                date_generation: row.get(12)?,
                total_prime_nette: row.get(13)?,
                total_commission_apport: row.get(14)?,
                total_commission_gestion: row.get(15)?,
                total_commission_ht: row.get(16)?,
                total_tva: row.get(17)?,
                total_commission_ttc: row.get(18)?,
                statut: row.get(19)?,
                lignes: Vec::new(),
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_bordereau_by_id(&self, id: i64) -> Result<Option<Bordereau>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, intermediaire_id, intermediaire_nom, intermediaire_code, type_intermediaire,
                    numero_bordereau, exercice, mois, code_client, succursale, departement,
                    tva_taux, date_generation, total_prime_nette, total_commission_apport,
                    total_commission_gestion, total_commission_ht, total_tva, total_commission_ttc, statut
             FROM bordereaux WHERE id = ?1",
        )?;

        let mut b = match stmt.query_row(params![id], |row| {
            Ok(Bordereau {
                id: Some(row.get(0)?),
                intermediaire_id: row.get(1)?,
                intermediaire_nom: row.get(2)?,
                intermediaire_code: row.get(3)?,
                type_intermediaire: row.get(4)?,
                numero_bordereau: row.get(5)?,
                exercice: row.get(6)?,
                mois: row.get(7)?,
                code_client: row.get(8)?,
                succursale: row.get(9)?,
                departement: row.get(10)?,
                tva_taux: row.get(11)?,
                date_generation: row.get(12)?,
                total_prime_nette: row.get(13)?,
                total_commission_apport: row.get(14)?,
                total_commission_gestion: row.get(15)?,
                total_commission_ht: row.get(16)?,
                total_tva: row.get(17)?,
                total_commission_ttc: row.get(18)?,
                statut: row.get(19)?,
                lignes: Vec::new(),
            })
        }) {
            Ok(val) => val,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e),
        };

        // Charger les lignes
        let mut l_stmt = conn.prepare(
            "SELECT id, bordereau_id, branche_id, code_garantie, libelle_garantie, branche_nom,
                    prime_nette, taux_apport, taux_gestion, montant_apport, montant_gestion,
                    montant_ht, montant_tva, montant_ttc
             FROM bordereau_lignes WHERE bordereau_id = ?1 ORDER BY id ASC",
        )?;

        let l_rows = l_stmt.query_map(params![id], |row| {
            Ok(BordereauLigne {
                id: Some(row.get(0)?),
                bordereau_id: Some(row.get(1)?),
                branche_id: row.get(2)?,
                code_garantie: row.get(3)?,
                libelle_garantie: row.get(4)?,
                branche_nom: row.get(5)?,
                prime_nette: row.get(6)?,
                taux_apport: row.get(7)?,
                taux_gestion: row.get(8)?,
                montant_apport: row.get(9)?,
                montant_gestion: row.get(10)?,
                montant_ht: row.get(11)?,
                montant_tva: row.get(12)?,
                montant_ttc: row.get(13)?,
            })
        })?;

        for lr in l_rows {
            b.lignes.push(lr?);
        }

        Ok(Some(b))
    }

    pub fn delete_bordereau(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM bordereaux WHERE id = ?1", params![id])?;
        Ok(())
    }
}

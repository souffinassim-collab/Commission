use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension};
use crate::calculation::alias_list;
use crate::models::{
    Anomalie, Bordereau, BordereauLigne, BrancheParam, Categorie, DbInfo, FichierBordereau, Intermediaire,
    UploadedFileResult,
};

/// Résultat des opérations BDD : les erreurs sont des messages lisibles par l'utilisateur
pub type DbResult<T> = std::result::Result<T, String>;

fn db_err(e: rusqlite::Error) -> String {
    format!("Erreur base de données : {}", e)
}

pub struct DatabaseManager {
    db_path: PathBuf,
}

/// Dernière version de schéma connue (à incrémenter à chaque nouvelle migration)
const LATEST_SCHEMA_VERSION: i32 = 7;

pub const STATUTS_BORDEREAU: [&str; 3] = ["BROUILLON", "VALIDE", "REGLE"];

/// Catégories proposées à l'origine et règle de classement initial (premier segment du code ministériel)
const CATEGORIES_INITIALES: [(&str, &str); 5] = [
    ("Automobile", "code LIKE '3.%'"),
    ("Transport & Maritime", "code LIKE '4.%' OR code LIKE '5.%' OR code LIKE '6.%' OR code LIKE '7.%'"),
    ("Agricole", "LOWER(branche) LIKE '%agricole%'"),
    (
        "Incendie & Risques Divers",
        "code LIKE '2.%' OR code LIKE '8.%' OR code LIKE '9.%' OR code LIKE '16.%' OR code = '16'",
    ),
    ("Responsabilité Civile", "code LIKE '10.%' OR code LIKE '11.%' OR code LIKE '12.%' OR code LIKE '13.%'"),
];

/// Fichier d'import à rattacher à un bordereau lors de l'enregistrement
pub struct ImportAEnregistrer<'a> {
    pub fichier: &'a UploadedFileResult,
    /// Contenu du fichier ; `None` si le fichier est déjà enregistré pour ce bordereau (même empreinte)
    pub contenu: Option<Vec<u8>>,
}

impl DatabaseManager {
    pub fn new() -> DbResult<Self> {
        let bdd_dir = Self::get_bdd_directory();
        fs::create_dir_all(bdd_dir.join("backups")).unwrap_or_default();
        let manager = Self { db_path: bdd_dir.join("commissions.db") };
        manager.run_migrations()?;
        Ok(manager)
    }

    /// Ouvre (ou crée) une base à un emplacement explicite — utilisé par les tests
    /// pour ne jamais écrire dans la base de production.
    #[allow(dead_code)]
    pub fn with_path<P: AsRef<Path>>(db_path: P) -> DbResult<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent).unwrap_or_default();
        }
        let manager = Self { db_path };
        manager.run_migrations()?;
        Ok(manager)
    }

    /// Dossier d'export (à côté du dossier BDD)
    pub fn exports_directory(&self) -> PathBuf {
        let base = self
            .db_path
            .parent()
            .and_then(|bdd| bdd.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let dir = base.join("Exports");
        fs::create_dir_all(&dir).unwrap_or_default();
        dir
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

    pub fn get_connection(&self) -> DbResult<Connection> {
        let conn = Connection::open(&self.db_path).map_err(db_err)?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )
        .map_err(db_err)?;
        Ok(conn)
    }

    // -----------------------------------------------------------------------------------------
    // Migrations
    // -----------------------------------------------------------------------------------------

    fn run_migrations(&self) -> DbResult<()> {
        let mut conn = Connection::open(&self.db_path).map_err(db_err)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS _migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL
            );",
            [],
        )
        .map_err(db_err)?;

        let current_version: i32 = conn
            .query_row("SELECT COALESCE(MAX(version), 0) FROM _migrations", [], |row| row.get(0))
            .unwrap_or(0);

        // Sauvegarde automatique pré-migration uniquement si une migration va réellement s'appliquer
        if current_version < LATEST_SCHEMA_VERSION && Self::has_any_user_tables(&conn) {
            if let Err(e) = self.create_pre_migration_backup() {
                eprintln!("Avertissement sauvegarde pré-migration: {}", e);
            }
        }
        let now = || Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        // Migration V1: Schéma initial + injection des taux de référence
        if current_version < 1 {
            let tx = conn.transaction().map_err(db_err)?;
            tx.execute_batch(
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
                );
                CREATE TABLE IF NOT EXISTS intermediaires (
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
                );
                CREATE TABLE IF NOT EXISTS bordereaux (
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
                );
                CREATE TABLE IF NOT EXISTS bordereau_lignes (
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
            )
            .map_err(db_err)?;
            Self::insert_default_rates(&tx)?;
            tx.execute(
                "INSERT INTO _migrations (version, name, applied_at) VALUES (1, 'initial_schema_and_rates', ?1)",
                params![now()],
            )
            .map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }

        // Migration V2: Ajout colonne 'desactive' dans bordereau_lignes
        if current_version < 2 {
            let tx = conn.transaction().map_err(db_err)?;
            if !Self::column_exists(&tx, "bordereau_lignes", "desactive")? {
                tx.execute("ALTER TABLE bordereau_lignes ADD COLUMN desactive INTEGER NOT NULL DEFAULT 0;", [])
                    .map_err(db_err)?;
            }
            tx.execute(
                "INSERT INTO _migrations (version, name, applied_at) VALUES (2, 'add_desactive_column', ?1)",
                params![now()],
            )
            .map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }

        // Migration V3: détail des contributions et indicateur "non paramétré" des lignes
        if current_version < 3 {
            let tx = conn.transaction().map_err(db_err)?;
            if !Self::column_exists(&tx, "bordereau_lignes", "non_parametre")? {
                tx.execute("ALTER TABLE bordereau_lignes ADD COLUMN non_parametre INTEGER NOT NULL DEFAULT 0;", [])
                    .map_err(db_err)?;
                tx.execute(
                    "UPDATE bordereau_lignes SET non_parametre = CASE WHEN branche_id IS NULL OR (taux_apport = 0 AND taux_gestion = 0) THEN 1 ELSE 0 END",
                    [],
                )
                .map_err(db_err)?;
            }
            if !Self::column_exists(&tx, "bordereau_lignes", "contributions_json")? {
                tx.execute("ALTER TABLE bordereau_lignes ADD COLUMN contributions_json TEXT;", [])
                    .map_err(db_err)?;
            }
            tx.execute(
                "INSERT INTO _migrations (version, name, applied_at) VALUES (3, 'add_lignes_contributions_non_parametre', ?1)",
                params![now()],
            )
            .map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }

        // Migration V4: paramètres d'import / de regroupement, fichiers rattachés aux bordereaux,
        // anomalies et rattachement des bordereaux existants à leur intermédiaire
        if current_version < 4 {
            let tx = conn.transaction().map_err(db_err)?;
            for (col, ddl) in [
                ("alias_import", "ALTER TABLE branches ADD COLUMN alias_import TEXT NOT NULL DEFAULT '';"),
                ("groupe_bordereau", "ALTER TABLE branches ADD COLUMN groupe_bordereau TEXT NOT NULL DEFAULT '';"),
                ("ordre_bordereau", "ALTER TABLE branches ADD COLUMN ordre_bordereau INTEGER NOT NULL DEFAULT 999;"),
            ] {
                if !Self::column_exists(&tx, "branches", col)? {
                    tx.execute(ddl, []).map_err(db_err)?;
                }
            }
            if !Self::column_exists(&tx, "bordereaux", "anomalies_json")? {
                tx.execute("ALTER TABLE bordereaux ADD COLUMN anomalies_json TEXT;", []).map_err(db_err)?;
            }
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS bordereau_fichiers (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    bordereau_id INTEGER NOT NULL REFERENCES bordereaux(id) ON DELETE CASCADE,
                    type_piece TEXT NOT NULL CHECK(type_piece IN ('IMPORT', 'PIECE_JOINTE')),
                    nom_fichier TEXT NOT NULL,
                    type_fichier TEXT NOT NULL,
                    hash TEXT NOT NULL,
                    taille INTEGER NOT NULL DEFAULT 0,
                    date_ajout TEXT NOT NULL,
                    client_code TEXT,
                    client_nom TEXT,
                    periode TEXT,
                    total_prime_nette REAL NOT NULL DEFAULT 0.0,
                    nb_lignes INTEGER NOT NULL DEFAULT 0,
                    anomalies_json TEXT,
                    contenu BLOB NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_bordereau_fichiers_bordereau ON bordereau_fichiers(bordereau_id);
                CREATE INDEX IF NOT EXISTS idx_bordereau_fichiers_hash ON bordereau_fichiers(hash);
                -- Intermédiaires absents du répertoire : fiche créée à partir des données du bordereau
                INSERT INTO intermediaires (type_intermediaire, code, nom, succursale, departement, tva_applicable, actif)
                SELECT b.type_intermediaire, b.intermediaire_code, MAX(b.intermediaire_nom), MAX(b.succursale),
                       MAX(b.departement), MAX(b.tva_taux), 1
                FROM bordereaux b
                WHERE b.type_intermediaire IN ('AGA', 'COURTIER')
                  AND NOT EXISTS (SELECT 1 FROM intermediaires i WHERE i.code = b.intermediaire_code)
                GROUP BY b.intermediaire_code, b.type_intermediaire;
                UPDATE bordereaux SET intermediaire_id = (
                    SELECT i.id FROM intermediaires i WHERE i.code = bordereaux.intermediaire_code
                ) WHERE intermediaire_id IS NULL;",
            )
            .map_err(db_err)?;
            Self::fix_default_labels(&tx)?;
            Self::apply_default_mappings(&tx)?;
            tx.execute(
                "INSERT INTO _migrations (version, name, applied_at) VALUES (4, 'import_mappings_fichiers_liens', ?1)",
                params![now()],
            )
            .map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }

        // Migration V5: contenu des fichiers stocké une seule fois (un même registre peut servir à plusieurs
        // bordereaux, un par point de vente) + sélection retenue dans le fichier
        if current_version < 5 {
            let tx = conn.transaction().map_err(db_err)?;
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS fichiers_contenu (
                    hash TEXT PRIMARY KEY,
                    contenu BLOB NOT NULL
                );",
            )
            .map_err(db_err)?;
            if Self::column_exists(&tx, "bordereau_fichiers", "contenu")? {
                tx.execute_batch(
                    "INSERT OR IGNORE INTO fichiers_contenu (hash, contenu) SELECT hash, contenu FROM bordereau_fichiers;
                     ALTER TABLE bordereau_fichiers DROP COLUMN contenu;",
                )
                .map_err(db_err)?;
            }
            if !Self::column_exists(&tx, "bordereau_fichiers", "selection")? {
                tx.execute("ALTER TABLE bordereau_fichiers ADD COLUMN selection TEXT;", []).map_err(db_err)?;
            }
            tx.execute(
                "INSERT INTO _migrations (version, name, applied_at) VALUES (5, 'contenu_fichiers_mutualise_selection', ?1)",
                params![now()],
            )
            .map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }

        // Migration V6: facture de l'intermédiaire saisie pour confirmation (rapprochement avec le calcul)
        if current_version < 6 {
            let tx = conn.transaction().map_err(db_err)?;
            if !Self::column_exists(&tx, "bordereaux", "facture_json")? {
                tx.execute("ALTER TABLE bordereaux ADD COLUMN facture_json TEXT;", []).map_err(db_err)?;
            }
            tx.execute(
                "INSERT INTO _migrations (version, name, applied_at) VALUES (6, 'facture_intermediaire', ?1)",
                params![now()],
            )
            .map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }

        // Migration V7: catégories de paramètres gérées en base (remplacent le filtrage par mots-clés)
        if current_version < 7 {
            let tx = conn.transaction().map_err(db_err)?;
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS categories (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    libelle TEXT NOT NULL UNIQUE,
                    ordre INTEGER NOT NULL DEFAULT 0
                );",
            )
            .map_err(db_err)?;
            if !Self::column_exists(&tx, "branches", "categorie_id")? {
                tx.execute(
                    "ALTER TABLE branches ADD COLUMN categorie_id INTEGER REFERENCES categories(id) ON DELETE SET NULL;",
                    [],
                )
                .map_err(db_err)?;
            }
            let nb: i64 = tx.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0)).map_err(db_err)?;
            if nb == 0 {
                for (i, lib) in CATEGORIES_INITIALES.iter().enumerate() {
                    tx.execute("INSERT INTO categories (libelle, ordre) VALUES (?1, ?2)", params![lib.0, (i as i32 + 1) * 10])
                        .map_err(db_err)?;
                }
            }
            Self::apply_default_categories(&tx)?;
            tx.execute(
                "INSERT INTO _migrations (version, name, applied_at) VALUES (7, 'categories_parametres', ?1)",
                params![now()],
            )
            .map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }

        Ok(())
    }

    /// Classement initial des lignes sans catégorie, d'après le code ministériel (premier segment) ;
    /// ne concerne que les catégories encore présentes sous leur libellé d'origine.
    fn apply_default_categories(conn: &Connection) -> DbResult<()> {
        for (libelle, condition) in CATEGORIES_INITIALES.iter() {
            let id: Option<i64> = conn
                .query_row("SELECT id FROM categories WHERE libelle = ?1", params![libelle], |r| r.get(0))
                .optional()
                .map_err(db_err)?;
            if let Some(id) = id {
                conn.execute(
                    &format!(
                        "UPDATE branches SET categorie_id = ?1 WHERE categorie_id IS NULL AND ({})",
                        condition
                    ),
                    params![id],
                )
                .map_err(db_err)?;
            }
        }
        Ok(())
    }

    fn insert_default_rates(conn: &Connection) -> DbResult<()> {
        let json = include_str!("../../initial_rates.json");
        let rates: Vec<serde_json::Value> =
            serde_json::from_str(json).map_err(|e| format!("initial_rates.json invalide : {}", e))?;
        for item in rates {
            let s = |k: &str| item.get(k).and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            let f = |k: &str| item.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
            conn.execute(
                "INSERT INTO branches (code, branche, sous_code, sous_libelle, taux_apport_courtier, taux_apport_aga, taux_gestion_aga)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    s("code"),
                    s("branche"),
                    s("sous_code").to_uppercase(),
                    s("sous_libelle"),
                    f("taux_apport_courtier"),
                    f("taux_apport_aga"),
                    f("taux_gestion_aga")
                ],
            )
            .map_err(db_err)?;
        }
        Ok(())
    }

    /// Corrige les libellés mal encodés du barème d'origine
    fn fix_default_labels(conn: &Connection) -> DbResult<()> {
        conn.execute_batch(
            "UPDATE branches SET sous_libelle = 'Incendie de Véhicule' WHERE sous_code = 'VINV' AND sous_libelle LIKE '%©%';
             UPDATE branches SET sous_libelle = 'Vol de Véhicule' WHERE sous_code = 'VOLV' AND sous_libelle LIKE '%©%';",
        )
        .map_err(db_err)
    }

    /// Paramètres initiaux d'import et de regroupement (données de départ, modifiables ensuite dans
    /// l'écran Paramètres). N'écrase jamais une valeur déjà saisie.
    fn apply_default_mappings(conn: &Connection) -> DbResult<()> {
        // Regroupements du bordereau courtier (libellé de ligne, ordre)
        let groupes_par_sous_code: [(&[&str], &str, i32); 5] = [
            (&["VRC", "VRCR", "VRCE"], "RC AUTOMOBILE", 10),
            (&["VASS"], "ASSISTANCE AUTOMOBILE", 20),
            (&["VTR", "VOLV", "VINV", "VDR", "VBDG", "VPT", "VVA", "VDC", "VVI"], "AUTOMOBILE FACULTATIF", 30),
            (&["CANAT"], "CATNAT", 40),
            (&["RVAC", "RVTR"], "RC. Du Transporteur", 150),
        ];
        for (codes, groupe, ordre) in groupes_par_sous_code {
            for c in codes {
                conn.execute(
                    "UPDATE branches SET groupe_bordereau = ?1, ordre_bordereau = ?2 WHERE sous_code = ?3 AND groupe_bordereau = ''",
                    params![groupe, ordre, c],
                )
                .map_err(db_err)?;
            }
        }
        let groupes_par_code: [(&[&str], &str, i32); 10] = [
            (&["8.1.1.0", "8.1.2.0"], "INCENDIE", 50),
            (&["13.1"], "RC. Générale", 60),
            (&["13.2"], "RC. Professionnelle", 70),
            (&["9.5.4.0"], "Engins de Chantiers", 80),
            (&["9.5.1.0"], "Bris de Machine", 90),
            (&["9.4.0.0"], "Tous Risques Chantiers", 100),
            (&["9.5.2.0"], "Tous Risques Informatiques", 110),
            (&["8.1.2.2"], "Multirisques professionnelle", 120),
            (&["8.1.2.1"], "Multirisques habitation", 130),
            (&["8.1.2.3"], "Multirisques immeubles", 140),
        ];
        for (codes, groupe, ordre) in groupes_par_code {
            for c in codes {
                conn.execute(
                    "UPDATE branches SET groupe_bordereau = ?1, ordre_bordereau = ?2 WHERE code = ?3 AND groupe_bordereau = ''",
                    params![groupe, ordre, c],
                )
                .map_err(db_err)?;
            }
        }

        // Alias des codes branche des registres de production Excel -> ligne de paramètre
        // (cible désignée par sous-code "S:" ou par code ministériel "C:", première ligne si plusieurs)
        let alias: [(&str, &str); 17] = [
            ("CN", "S:CANAT"),
            ("IN", "C:8.1.1.0"),
            ("RG", "C:13.1"),
            ("RP", "C:13.2"),
            ("EC", "C:9.5.4.0"),
            ("FM", "C:9.5.1.0"),
            ("TC", "C:9.4.0.0"),
            ("TI", "C:9.5.2.0"),
            ("MP", "C:8.1.2.2"),
            ("MH", "C:8.1.2.1"),
            ("IM", "C:8.1.2.3"),
            ("CR", "C:6.1.1.0"),
            ("FB", "C:7.4"),
            ("FT", "C:7.1"),
            ("RV", "S:RVTR"),
            ("VP,VO,VR", "S:VRC"),
            ("VF", "S:VTR"),
        ];
        let existing: Vec<String> = {
            let mut st = conn.prepare("SELECT alias_import FROM branches").map_err(db_err)?;
            let rows = st.query_map([], |r| r.get::<_, String>(0)).map_err(db_err)?;
            let mut v = Vec::new();
            for r in rows {
                for a in r.map_err(db_err)?.split(',') {
                    let a = a.trim().to_uppercase();
                    if !a.is_empty() {
                        v.push(a);
                    }
                }
            }
            v
        };
        for (aliases, cible) in alias {
            if aliases.split(',').any(|a| existing.contains(&a.to_string())) {
                continue;
            }
            let (col, val) = cible.split_at(2);
            let sql = if col == "S:" {
                "SELECT id FROM branches WHERE sous_code = ?1 AND alias_import = '' ORDER BY id LIMIT 1"
            } else {
                "SELECT id FROM branches WHERE code = ?1 AND alias_import = '' ORDER BY id LIMIT 1"
            };
            let id: Option<i64> = conn.query_row(sql, params![val], |r| r.get(0)).optional().map_err(db_err)?;
            if let Some(id) = id {
                conn.execute("UPDATE branches SET alias_import = ?1 WHERE id = ?2", params![aliases, id])
                    .map_err(db_err)?;
            }
        }
        Ok(())
    }

    fn column_exists(conn: &Connection, table: &str, column: &str) -> DbResult<bool> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table)).map_err(db_err)?;
        let names = stmt.query_map([], |row| row.get::<_, String>(1)).map_err(db_err)?;
        for n in names {
            if n.map_err(db_err)? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn has_any_user_tables(conn: &Connection) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='branches'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .unwrap_or(0)
            > 0
    }

    pub fn create_pre_migration_backup(&self) -> DbResult<PathBuf> {
        if !self.db_path.exists() {
            return Ok(PathBuf::new());
        }
        let now = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_dir = self.db_path.parent().unwrap_or(Path::new(".")).join("backups");
        fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
        let backup_path = backup_dir.join(format!("backup_commissions_{}.db", now));
        // VACUUM INTO produit une copie cohérente même en mode WAL
        let conn = Connection::open(&self.db_path).map_err(db_err)?;
        let target = backup_path.to_string_lossy().replace('\'', "''");
        if conn.execute_batch(&format!("VACUUM INTO '{}'", target)).is_err() {
            fs::copy(&self.db_path, &backup_path).map_err(|e| e.to_string())?;
        }
        Ok(backup_path)
    }

    pub fn get_info(&self) -> DbResult<DbInfo> {
        let conn = self.get_connection()?;
        let count = |sql: &str| conn.query_row(sql, [], |row| row.get::<_, i64>(0)).unwrap_or(0);
        let version = count("SELECT COALESCE(MAX(version), 0) FROM _migrations") as i32;
        let total_branches = count("SELECT COUNT(*) FROM branches");
        let total_intermediaires = count("SELECT COUNT(*) FROM intermediaires");
        let total_bordereaux = count("SELECT COUNT(*) FROM bordereaux");

        let backups_dir = self.db_path.parent().unwrap_or(Path::new(".")).join("backups");
        let mut last_backup = None;
        if let Ok(entries) = fs::read_dir(backups_dir) {
            let mut latest_time = None;
            for entry in entries.flatten() {
                if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
                    if latest_time.map_or(true, |t| modified > t) {
                        latest_time = Some(modified);
                        last_backup = Some(entry.file_name().to_string_lossy().to_string());
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

    // -----------------------------------------------------------------------------------------
    // Paramètres (branches / barèmes)
    // -----------------------------------------------------------------------------------------

    pub fn get_all_branches(&self) -> DbResult<Vec<BrancheParam>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, code, branche, COALESCE(sous_code, ''), COALESCE(sous_libelle, ''), taux_apport_courtier,
                        taux_apport_aga, taux_gestion_aga, alias_import, groupe_bordereau, ordre_bordereau, categorie_id
                 FROM branches ORDER BY id ASC",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(BrancheParam {
                    id: Some(row.get(0)?),
                    code: row.get(1)?,
                    branche: row.get(2)?,
                    sous_code: row.get(3)?,
                    sous_libelle: row.get(4)?,
                    taux_apport_courtier: row.get(5)?,
                    taux_apport_aga: row.get(6)?,
                    taux_gestion_aga: row.get(7)?,
                    alias_import: row.get(8)?,
                    groupe_bordereau: row.get(9)?,
                    ordre_bordereau: row.get(10)?,
                    categorie_id: row.get(11)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    /// Sous-codes paramétrés (servent au découpage des codes garanties des rapports importés)
    pub fn known_sous_codes(&self) -> DbResult<Vec<String>> {
        Ok(self
            .get_all_branches()?
            .into_iter()
            .map(|b| b.sous_code.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect())
    }

    fn validate_branche(&self, b: &BrancheParam) -> DbResult<BrancheParam> {
        let mut clean = b.clone();
        clean.code = b.code.trim().to_string();
        clean.branche = b.branche.trim().to_string();
        clean.sous_code = b.sous_code.trim().to_uppercase();
        clean.sous_libelle = b.sous_libelle.trim().to_string();
        clean.groupe_bordereau = b.groupe_bordereau.trim().to_string();
        let aliases = alias_list(b);
        clean.alias_import = aliases.join(",");

        if clean.code.is_empty() || clean.branche.is_empty() {
            return Err("Le code et le nom de la branche sont obligatoires.".to_string());
        }
        for (nom, t) in [
            ("apport courtier", clean.taux_apport_courtier),
            ("apport AGA", clean.taux_apport_aga),
            ("gestion AGA", clean.taux_gestion_aga),
        ] {
            if !(0.0..=1.0).contains(&t) || t.is_nan() {
                return Err(format!("Taux {} invalide ({}) : il doit être compris entre 0 % et 100 %.", nom, t));
            }
        }
        if let Some(cid) = clean.categorie_id {
            let conn = self.get_connection()?;
            let existe: Option<i64> = conn
                .query_row("SELECT id FROM categories WHERE id = ?1", params![cid], |r| r.get(0))
                .optional()
                .map_err(db_err)?;
            if existe.is_none() {
                return Err(format!("Catégorie #{} introuvable.", cid));
            }
        }
        let others: Vec<BrancheParam> = self
            .get_all_branches()?
            .into_iter()
            .filter(|o| o.id != b.id || b.id.is_none())
            .collect();
        if !clean.sous_code.is_empty() {
            if let Some(o) = others
                .iter()
                .find(|o| o.sous_code.trim().to_uppercase() == clean.sous_code && o.code.trim() == clean.code)
            {
                return Err(format!(
                    "Le sous-code « {} » existe déjà pour la branche {} ({}).",
                    clean.sous_code, clean.code, o.branche
                ));
            }
        }
        for a in &aliases {
            if let Some(o) = others.iter().find(|o| alias_list(o).contains(a)) {
                return Err(format!(
                    "L'alias d'import « {} » est déjà utilisé par {} {} ({}).",
                    a, o.code, o.sous_code, o.sous_libelle
                ));
            }
        }
        Ok(clean)
    }

    pub fn save_branche(&self, b: &BrancheParam) -> DbResult<i64> {
        let b = self.validate_branche(b)?;
        let conn = self.get_connection()?;
        if let Some(id) = b.id.filter(|id| *id > 0) {
            let n = conn
                .execute(
                    "UPDATE branches SET code = ?1, branche = ?2, sous_code = ?3, sous_libelle = ?4,
                        taux_apport_courtier = ?5, taux_apport_aga = ?6, taux_gestion_aga = ?7,
                        alias_import = ?8, groupe_bordereau = ?9, ordre_bordereau = ?10, categorie_id = ?11
                     WHERE id = ?12",
                    params![
                        b.code, b.branche, b.sous_code, b.sous_libelle, b.taux_apport_courtier, b.taux_apport_aga,
                        b.taux_gestion_aga, b.alias_import, b.groupe_bordereau, b.ordre_bordereau, b.categorie_id, id
                    ],
                )
                .map_err(db_err)?;
            if n == 0 {
                return Err(format!("Paramètre #{} introuvable (supprimé entre-temps ?).", id));
            }
            return Ok(id);
        }
        conn.execute(
            "INSERT INTO branches (code, branche, sous_code, sous_libelle, taux_apport_courtier, taux_apport_aga,
                                   taux_gestion_aga, alias_import, groupe_bordereau, ordre_bordereau, categorie_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                b.code, b.branche, b.sous_code, b.sous_libelle, b.taux_apport_courtier, b.taux_apport_aga,
                b.taux_gestion_aga, b.alias_import, b.groupe_bordereau, b.ordre_bordereau, b.categorie_id
            ],
        )
        .map_err(db_err)?;
        Ok(conn.last_insert_rowid())
    }

    /// Suppression d'un paramètre : les bordereaux déjà enregistrés conservent leurs taux (figés dans les lignes)
    pub fn delete_branche(&self, id: i64) -> DbResult<()> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction().map_err(db_err)?;
        tx.execute("UPDATE bordereau_lignes SET branche_id = NULL WHERE branche_id = ?1", params![id])
            .map_err(db_err)?;
        let n = tx.execute("DELETE FROM branches WHERE id = ?1", params![id]).map_err(db_err)?;
        if n == 0 {
            return Err(format!("Paramètre #{} introuvable.", id));
        }
        tx.commit().map_err(db_err)
    }

    pub fn reset_branches_to_default(&self) -> DbResult<()> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction().map_err(db_err)?;
        tx.execute("UPDATE bordereau_lignes SET branche_id = NULL", []).map_err(db_err)?;
        tx.execute("DELETE FROM branches", []).map_err(db_err)?;
        Self::insert_default_rates(&tx)?;
        Self::fix_default_labels(&tx)?;
        Self::apply_default_mappings(&tx)?;
        Self::apply_default_categories(&tx)?;
        tx.commit().map_err(db_err)
    }

    // -----------------------------------------------------------------------------------------
    // Catégories de paramètres
    // -----------------------------------------------------------------------------------------

    pub fn get_categories(&self) -> DbResult<Vec<Categorie>> {
        let conn = self.get_connection()?;
        let mut st = conn
            .prepare(
                "SELECT c.id, c.libelle, c.ordre, (SELECT COUNT(*) FROM branches b WHERE b.categorie_id = c.id)
                 FROM categories c ORDER BY c.ordre ASC, c.libelle ASC",
            )
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| Ok(Categorie { id: Some(r.get(0)?), libelle: r.get(1)?, ordre: r.get(2)?, nb_branches: r.get(3)? }))
            .map_err(db_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    pub fn save_categorie(&self, c: &Categorie) -> DbResult<i64> {
        let libelle = c.libelle.trim();
        if libelle.is_empty() {
            return Err("Le libellé de la catégorie est obligatoire.".to_string());
        }
        let conn = self.get_connection()?;
        let doublon: Option<i64> = conn
            .query_row(
                "SELECT id FROM categories WHERE UPPER(libelle) = UPPER(?1) AND id IS NOT ?2",
                params![libelle, c.id],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_err)?;
        if doublon.is_some() {
            return Err(format!("La catégorie « {} » existe déjà.", libelle));
        }
        match c.id {
            Some(id) => {
                let n = conn
                    .execute("UPDATE categories SET libelle = ?1, ordre = ?2 WHERE id = ?3", params![libelle, c.ordre, id])
                    .map_err(db_err)?;
                if n == 0 {
                    return Err(format!("Catégorie #{} introuvable.", id));
                }
                Ok(id)
            }
            None => {
                conn.execute("INSERT INTO categories (libelle, ordre) VALUES (?1, ?2)", params![libelle, c.ordre])
                    .map_err(db_err)?;
                Ok(conn.last_insert_rowid())
            }
        }
    }

    /// Supprime une catégorie : ses lignes de paramètres deviennent « Sans catégorie » (taux inchangés)
    pub fn delete_categorie(&self, id: i64) -> DbResult<()> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction().map_err(db_err)?;
        tx.execute("UPDATE branches SET categorie_id = NULL WHERE categorie_id = ?1", params![id])
            .map_err(db_err)?;
        let n = tx.execute("DELETE FROM categories WHERE id = ?1", params![id]).map_err(db_err)?;
        if n == 0 {
            return Err(format!("Catégorie #{} introuvable.", id));
        }
        tx.commit().map_err(db_err)
    }

    /// Range plusieurs lignes de paramètres dans une catégorie (aucune = « Sans catégorie »)
    pub fn affecter_categorie(&self, branche_ids: &[i64], categorie_id: Option<i64>) -> DbResult<()> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction().map_err(db_err)?;
        for id in branche_ids {
            tx.execute("UPDATE branches SET categorie_id = ?1 WHERE id = ?2", params![categorie_id, id])
                .map_err(db_err)?;
        }
        tx.commit().map_err(db_err)
    }

    // -----------------------------------------------------------------------------------------
    // Intermédiaires
    // -----------------------------------------------------------------------------------------

    fn map_intermediaire(row: &rusqlite::Row) -> rusqlite::Result<Intermediaire> {
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
    }

    pub fn get_all_intermediaires(&self) -> DbResult<Vec<Intermediaire>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, type_intermediaire, code, nom, succursale, departement, contact, adresse, tva_applicable, actif
                 FROM intermediaires ORDER BY nom ASC",
            )
            .map_err(db_err)?;
        let rows = stmt.query_map([], Self::map_intermediaire).map_err(db_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    pub fn get_intermediaire(&self, id: i64) -> DbResult<Option<Intermediaire>> {
        let conn = self.get_connection()?;
        conn.query_row(
            "SELECT id, type_intermediaire, code, nom, succursale, departement, contact, adresse, tva_applicable, actif
             FROM intermediaires WHERE id = ?1",
            params![id],
            Self::map_intermediaire,
        )
        .optional()
        .map_err(db_err)
    }

    fn count_bordereaux_intermediaire(conn: &Connection, id: i64) -> DbResult<i64> {
        conn.query_row(
            "SELECT COUNT(*) FROM bordereaux WHERE intermediaire_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(db_err)
    }

    pub fn save_intermediaire(&self, item: &Intermediaire) -> DbResult<i64> {
        let code = item.code.trim().to_string();
        let nom = item.nom.trim().to_string();
        if code.is_empty() || nom.is_empty() {
            return Err("Le code et le nom de l'intermédiaire sont obligatoires.".to_string());
        }
        if item.type_intermediaire != "AGA" && item.type_intermediaire != "COURTIER" {
            return Err("Le type d'intermédiaire doit être AGA ou COURTIER.".to_string());
        }
        if !(0.0..=1.0).contains(&item.tva_applicable) {
            return Err("Le taux de TVA doit être compris entre 0 % et 100 %.".to_string());
        }
        let conn = self.get_connection()?;
        let doublon: Option<i64> = conn
            .query_row(
                "SELECT id FROM intermediaires WHERE UPPER(code) = UPPER(?1) AND id IS NOT ?2",
                params![code, item.id],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_err)?;
        if doublon.is_some() {
            return Err(format!("Le code intermédiaire « {} » est déjà utilisé.", code));
        }

        if let Some(id) = item.id {
            let before = self
                .get_intermediaire(id)?
                .ok_or_else(|| format!("Intermédiaire #{} introuvable.", id))?;
            let nb = Self::count_bordereaux_intermediaire(&conn, id)?;
            if nb > 0 && (before.code != code || before.type_intermediaire != item.type_intermediaire) {
                return Err(format!(
                    "Impossible de modifier le code ou le type : {} bordereau(x) sont rattachés à cet intermédiaire.",
                    nb
                ));
            }
            conn.execute(
                "UPDATE intermediaires SET type_intermediaire = ?1, code = ?2, nom = ?3, succursale = ?4, departement = ?5,
                    contact = ?6, adresse = ?7, tva_applicable = ?8, actif = ?9
                 WHERE id = ?10",
                params![
                    item.type_intermediaire, code, nom, item.succursale.trim(), item.departement.trim(), item.contact,
                    item.adresse, item.tva_applicable, if item.actif { 1 } else { 0 }, id
                ],
            )
            .map_err(db_err)?;
            Ok(id)
        } else {
            conn.execute(
                "INSERT INTO intermediaires
                 (type_intermediaire, code, nom, succursale, departement, contact, adresse, tva_applicable, actif)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    item.type_intermediaire, code, nom, item.succursale.trim(), item.departement.trim(), item.contact,
                    item.adresse, item.tva_applicable, if item.actif { 1 } else { 0 }
                ],
            )
            .map_err(db_err)?;
            Ok(conn.last_insert_rowid())
        }
    }

    pub fn delete_intermediaire(&self, id: i64) -> DbResult<()> {
        let conn = self.get_connection()?;
        let nb = Self::count_bordereaux_intermediaire(&conn, id)?;
        if nb > 0 {
            return Err(format!(
                "Suppression impossible : {} bordereau(x) sont rattachés à cet intermédiaire. Désactivez-le plutôt.",
                nb
            ));
        }
        let n = conn.execute("DELETE FROM intermediaires WHERE id = ?1", params![id]).map_err(db_err)?;
        if n == 0 {
            return Err(format!("Intermédiaire #{} introuvable.", id));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------------------------
    // Bordereaux
    // -----------------------------------------------------------------------------------------

    /// Bordereau déjà enregistré qui contient un fichier de même empreinte (autre que `exclude`)
    pub fn find_hash_usage(&self, hash: &str, exclude: Option<i64>) -> DbResult<Option<(i64, String)>> {
        if hash.is_empty() {
            return Ok(None);
        }
        let conn = self.get_connection()?;
        conn.query_row(
            "SELECT b.id, b.numero_bordereau FROM bordereau_fichiers f JOIN bordereaux b ON b.id = f.bordereau_id
             WHERE f.hash = ?1 AND f.type_piece = 'IMPORT' AND b.id IS NOT ?2 LIMIT 1",
            params![hash, exclude],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(db_err)
    }

    /// Polices déjà commissionnées dans les autres bordereaux (lignes actives, polices non exclues) :
    /// N° de police normalisé -> numéro du bordereau
    pub fn polices_commissionnees(&self, exclude: Option<i64>) -> DbResult<HashMap<String, String>> {
        let conn = self.get_connection()?;
        Self::polices_commissionnees_conn(&conn, exclude)
    }

    fn polices_commissionnees_conn(conn: &Connection, exclude: Option<i64>) -> DbResult<HashMap<String, String>> {
        let mut st = conn
            .prepare(
                "SELECT b.numero_bordereau, l.contributions_json, b.facture_json FROM bordereau_lignes l
                 JOIN bordereaux b ON b.id = l.bordereau_id
                 WHERE b.id IS NOT ?1 AND COALESCE(l.desactive, 0) = 0 AND l.contributions_json IS NOT NULL",
            )
            .map_err(db_err)?;
        let rows = st
            .query_map(params![exclude], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?))
            })
            .map_err(db_err)?;
        let mut map = HashMap::new();
        for row in rows {
            let (bordereau, json, facture_json) = row.map_err(db_err)?;
            // Libellé de l'origine : bordereau et, si saisie, facture de l'intermédiaire
            let facture: Option<crate::models::FactureIntermediaire> = facture_json.and_then(|j| serde_json::from_str(&j).ok());
            let numero = match facture.filter(|f| !f.numero.trim().is_empty()) {
                Some(f) if !f.date.trim().is_empty() => format!("bordereau {} — facture N° {} du {}", bordereau, f.numero.trim(), f.date.trim()),
                Some(f) => format!("bordereau {} — facture N° {}", bordereau, f.numero.trim()),
                None => format!("bordereau {}", bordereau),
            };
            let contributions: Vec<crate::models::LigneContribution> = serde_json::from_str(&json).unwrap_or_default();
            for c in contributions.into_iter().filter(|c| !c.exclue) {
                if let Some(p) = c.police {
                    let key = normaliser_police(&p);
                    if !key.is_empty() {
                        map.entry(key).or_insert_with(|| numero.clone());
                    }
                }
            }
        }
        Ok(map)
    }

    /// Enregistre (création ou mise à jour) un bordereau avec ses lignes et, si fournis, ses fichiers d'import.
    /// Contrôles : intermédiaire existant et cohérent, bordereau modifiable (BROUILLON), aucune ligne active
    /// non paramétrée, aucun fichier en anomalie bloquante ni déjà utilisé par un autre bordereau.
    pub fn save_bordereau(&self, b: &Bordereau, imports: Option<&[ImportAEnregistrer]>) -> DbResult<i64> {
        if b.lignes.is_empty() {
            return Err("Le bordereau ne contient aucune ligne.".to_string());
        }
        if !STATUTS_BORDEREAU.contains(&b.statut.as_str()) {
            return Err(format!("Statut de bordereau invalide : {}", b.statut));
        }
        let inter_id = b
            .intermediaire_id
            .ok_or_else(|| "Le bordereau doit être rattaché à un intermédiaire enregistré.".to_string())?;
        let inter = self
            .get_intermediaire(inter_id)?
            .ok_or_else(|| format!("Intermédiaire #{} introuvable.", inter_id))?;
        if inter.type_intermediaire != b.type_intermediaire || inter.code != b.intermediaire_code {
            return Err(format!(
                "Incohérence : le bordereau ({} {}) ne correspond pas à l'intermédiaire sélectionné ({} {}).",
                b.type_intermediaire, b.intermediaire_code, inter.type_intermediaire, inter.code
            ));
        }
        let np: Vec<&BordereauLigne> = b.lignes.iter().filter(|l| l.non_parametre && l.est_active()).collect();
        if !np.is_empty() {
            return Err(format!(
                "Enregistrement impossible : {} ligne(s) sans taux paramétré ({}). Paramétrez-les ou excluez-les du bordereau.",
                np.len(),
                np.iter().map(|l| l.code_garantie.clone()).collect::<Vec<_>>().join(", ")
            ));
        }

        let mut conn = self.get_connection()?;
        let tx = conn.transaction().map_err(db_err)?;

        let existing: Option<(i64, String)> = match b.id {
            Some(id) => tx
                .query_row("SELECT id, statut FROM bordereaux WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()
                .map_err(db_err)?,
            None => None,
        };
        if let Some((_, statut)) = &existing {
            if statut != "BROUILLON" {
                return Err(format!(
                    "Ce bordereau est au statut {} : repassez-le en BROUILLON pour le modifier.",
                    statut
                ));
            }
        }
        let existing_id = existing.map(|(id, _)| id);

        if let Some(imports) = imports {
            for imp in imports {
                let f = imp.fichier;
                if let Some(a) = f.anomalies.iter().chain(f.anomalies_contexte.iter()).find(|a| a.est_bloquante()) {
                    return Err(format!("Le fichier « {} » présente une anomalie bloquante : {}", f.file_name, a.message));
                }
                // Rapport CIRIS : totaux sans N° de police, le contrôle se fait sur le fichier lui-même
                if f.file_type == "CIRIS_PDF" && !f.hash.is_empty() {
                    let used: Option<String> = tx
                        .query_row(
                            "SELECT b.numero_bordereau FROM bordereau_fichiers f JOIN bordereaux b ON b.id = f.bordereau_id
                             WHERE f.hash = ?1 AND f.type_piece = 'IMPORT' AND b.id IS NOT ?2 LIMIT 1",
                            params![f.hash, existing_id],
                            |r| r.get(0),
                        )
                        .optional()
                        .map_err(db_err)?;
                    if let Some(num) = used {
                        return Err(format!(
                            "Le fichier « {} » est déjà rattaché au bordereau {} : il ne peut pas être commissionné deux fois.",
                            f.file_name, num
                        ));
                    }
                }
            }
        }

        // Une police ne peut être commissionnée que dans un seul bordereau
        let deja = Self::polices_commissionnees_conn(&tx, existing_id)?;
        let mut doublons: Vec<String> = Vec::new();
        for l in b.lignes.iter().filter(|l| !l.desactive) {
            for c in l.contributions.iter().filter(|c| !c.exclue) {
                if let Some(num) = c.police.as_deref().and_then(|p| deja.get(&normaliser_police(p))) {
                    // Double facturation : une police déjà facturée ne peut pas être refacturée
                    doublons.push(format!("{} ({})", c.police.clone().unwrap_or_default(), num));
                }
            }
        }
        if !doublons.is_empty() {
            doublons.sort();
            doublons.dedup();
            return Err(format!(
                "Double facturation refusée : {} police(s) déjà facturée(s) dans un autre bordereau : {}{}",
                doublons.len(),
                doublons.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
                if doublons.len() > 5 { "…" } else { "" }
            ));
        }

        // Numéro de bordereau unique : suffixe -2, -3... s'il est déjà pris par un autre bordereau
        let base_numero = if b.numero_bordereau.trim().is_empty() {
            format!("BORD-{}-{}-{}", b.type_intermediaire, b.intermediaire_code, b.exercice)
        } else {
            b.numero_bordereau.trim().to_string()
        };
        let mut numero = base_numero.clone();
        let mut suffix = 2;
        while tx
            .query_row(
                "SELECT id FROM bordereaux WHERE numero_bordereau = ?1 AND id IS NOT ?2",
                params![numero, existing_id],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .map_err(db_err)?
            .is_some()
        {
            numero = format!("{}-{}", base_numero, suffix);
            suffix += 1;
        }

        let anomalies_json = if b.anomalies.is_empty() { None } else { serde_json::to_string(&b.anomalies).ok() };
        let facture_json = b.facture.as_ref().and_then(|f| serde_json::to_string(f).ok());

        let bordereau_id: i64 = if let Some(id) = existing_id {
            tx.execute(
                "UPDATE bordereaux SET intermediaire_id = ?1, intermediaire_nom = ?2, intermediaire_code = ?3,
                    type_intermediaire = ?4, numero_bordereau = ?5, exercice = ?6, mois = ?7, code_client = ?8,
                    succursale = ?9, departement = ?10, tva_taux = ?11, date_generation = ?12, total_prime_nette = ?13,
                    total_commission_apport = ?14, total_commission_gestion = ?15, total_commission_ht = ?16,
                    total_tva = ?17, total_commission_ttc = ?18, statut = ?19, anomalies_json = ?20, facture_json = ?21
                 WHERE id = ?22",
                params![
                    inter_id, b.intermediaire_nom, b.intermediaire_code, b.type_intermediaire, numero, b.exercice, b.mois,
                    b.code_client, b.succursale, b.departement, b.tva_taux, b.date_generation, b.total_prime_nette,
                    b.total_commission_apport, b.total_commission_gestion, b.total_commission_ht, b.total_tva,
                    b.total_commission_ttc, b.statut, anomalies_json, facture_json, id
                ],
            )
            .map_err(db_err)?;
            tx.execute("DELETE FROM bordereau_lignes WHERE bordereau_id = ?1", params![id]).map_err(db_err)?;
            id
        } else {
            tx.execute(
                "INSERT INTO bordereaux
                 (intermediaire_id, intermediaire_nom, intermediaire_code, type_intermediaire, numero_bordereau,
                  exercice, mois, code_client, succursale, departement, tva_taux, date_generation,
                  total_prime_nette, total_commission_apport, total_commission_gestion, total_commission_ht,
                  total_tva, total_commission_ttc, statut, anomalies_json, facture_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
                params![
                    inter_id, b.intermediaire_nom, b.intermediaire_code, b.type_intermediaire, numero, b.exercice, b.mois,
                    b.code_client, b.succursale, b.departement, b.tva_taux, b.date_generation, b.total_prime_nette,
                    b.total_commission_apport, b.total_commission_gestion, b.total_commission_ht, b.total_tva,
                    b.total_commission_ttc, b.statut, anomalies_json, facture_json
                ],
            )
            .map_err(db_err)?;
            tx.last_insert_rowid()
        };

        for l in &b.lignes {
            let contributions_json =
                if l.contributions.is_empty() { None } else { serde_json::to_string(&l.contributions).ok() };
            // La ligne ne référence une branche que si celle-ci existe encore (évite l'échec de clé étrangère)
            let branche_id: Option<i64> = match l.branche_id {
                Some(bid) => tx
                    .query_row("SELECT id FROM branches WHERE id = ?1", params![bid], |r| r.get(0))
                    .optional()
                    .map_err(db_err)?,
                None => None,
            };
            tx.execute(
                "INSERT INTO bordereau_lignes
                 (bordereau_id, branche_id, code_garantie, libelle_garantie, branche_nom, prime_nette,
                  taux_apport, taux_gestion, montant_apport, montant_gestion, montant_ht, montant_tva, montant_ttc, desactive,
                  non_parametre, contributions_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    bordereau_id, branche_id, l.code_garantie, l.libelle_garantie, l.branche_nom, l.prime_nette,
                    l.taux_apport, l.taux_gestion, l.montant_apport, l.montant_gestion, l.montant_ht, l.montant_tva,
                    l.montant_ttc, if l.desactive { 1 } else { 0 }, if l.non_parametre { 1 } else { 0 }, contributions_json
                ],
            )
            .map_err(db_err)?;
        }

        // Fichiers d'import : la liste fournie devient la liste des imports du bordereau
        if let Some(imports) = imports {
            let date = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let mut hashes_gardes: Vec<String> = Vec::new();
            for imp in imports {
                let f = imp.fichier;
                let deja: Option<i64> = tx
                    .query_row(
                        "SELECT id FROM bordereau_fichiers WHERE bordereau_id = ?1 AND hash = ?2 AND type_piece = 'IMPORT'",
                        params![bordereau_id, f.hash],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(db_err)?;
                let toutes: Vec<&Anomalie> = f.anomalies.iter().chain(f.anomalies_contexte.iter()).collect();
                let anomalies_json = if toutes.is_empty() { None } else { serde_json::to_string(&toutes).ok() };
                match (&imp.contenu, deja) {
                    (_, Some(fid)) => {
                        tx.execute(
                            "UPDATE bordereau_fichiers SET nom_fichier = ?1, client_code = ?2, client_nom = ?3, periode = ?4,
                                total_prime_nette = ?5, nb_lignes = ?6, anomalies_json = ?7, selection = ?8 WHERE id = ?9",
                            params![
                                f.file_name, f.code, f.courtier_ou_client, f.periode, f.total_prime_nette,
                                f.items.len() as i64, anomalies_json, f.selection, fid
                            ],
                        )
                        .map_err(db_err)?;
                    }
                    (Some(contenu), None) => {
                        tx.execute(
                            "INSERT OR IGNORE INTO fichiers_contenu (hash, contenu) VALUES (?1, ?2)",
                            params![f.hash, contenu],
                        )
                        .map_err(db_err)?;
                        tx.execute(
                            "INSERT INTO bordereau_fichiers
                             (bordereau_id, type_piece, nom_fichier, type_fichier, hash, taille, date_ajout, client_code,
                              client_nom, periode, total_prime_nette, nb_lignes, anomalies_json, selection)
                             VALUES (?1, 'IMPORT', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                            params![
                                bordereau_id, f.file_name, f.file_type, f.hash, contenu.len() as i64, date, f.code,
                                f.courtier_ou_client, f.periode, f.total_prime_nette, f.items.len() as i64, anomalies_json,
                                f.selection
                            ],
                        )
                        .map_err(db_err)?;
                    }
                    (None, None) => {
                        return Err(format!(
                            "Le contenu du fichier « {} » n'est plus disponible : réimportez-le avant d'enregistrer.",
                            f.file_name
                        ));
                    }
                }
                hashes_gardes.push(f.hash.clone());
            }
            // Fichiers retirés de l'import en cours
            let mut st = tx
                .prepare("SELECT id, hash FROM bordereau_fichiers WHERE bordereau_id = ?1 AND type_piece = 'IMPORT'")
                .map_err(db_err)?;
            let rows: Vec<(i64, String)> = st
                .query_map(params![bordereau_id], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(db_err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(db_err)?;
            drop(st);
            for (fid, h) in rows {
                if !hashes_gardes.contains(&h) {
                    tx.execute("DELETE FROM bordereau_fichiers WHERE id = ?1", params![fid]).map_err(db_err)?;
                }
            }
            Self::purger_contenus_orphelins(&tx)?;
        }

        tx.commit().map_err(db_err)?;
        Ok(bordereau_id)
    }

    fn map_bordereau(row: &rusqlite::Row) -> rusqlite::Result<Bordereau> {
        let anomalies_json: Option<String> = row.get(20)?;
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
            anomalies: anomalies_json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default(),
            fichiers: Vec::new(),
            facture: row.get::<_, Option<String>>(21)?.and_then(|j| serde_json::from_str(&j).ok()),
        })
    }

    const BORDEREAU_COLS: &'static str = "id, intermediaire_id, intermediaire_nom, intermediaire_code, type_intermediaire,
        numero_bordereau, exercice, mois, code_client, succursale, departement, tva_taux, date_generation,
        total_prime_nette, total_commission_apport, total_commission_gestion, total_commission_ht, total_tva,
        total_commission_ttc, statut, anomalies_json, facture_json";

    pub fn get_all_bordereaux(&self) -> DbResult<Vec<Bordereau>> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare(&format!("SELECT {} FROM bordereaux ORDER BY id DESC", Self::BORDEREAU_COLS))
            .map_err(db_err)?;
        let rows = stmt.query_map([], Self::map_bordereau).map_err(db_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    pub fn get_bordereau_by_id(&self, id: i64) -> DbResult<Option<Bordereau>> {
        let conn = self.get_connection()?;
        let mut b = match conn
            .query_row(
                &format!("SELECT {} FROM bordereaux WHERE id = ?1", Self::BORDEREAU_COLS),
                params![id],
                Self::map_bordereau,
            )
            .optional()
            .map_err(db_err)?
        {
            Some(b) => b,
            None => return Ok(None),
        };

        // Taux et montants relus tels qu'enregistrés : le barème actuel n'est jamais consulté
        let mut l_stmt = conn
            .prepare(
                "SELECT id, bordereau_id, branche_id, code_garantie, libelle_garantie, branche_nom,
                        prime_nette, taux_apport, taux_gestion, montant_apport, montant_gestion,
                        montant_ht, montant_tva, montant_ttc, COALESCE(desactive, 0),
                        COALESCE(non_parametre, 0), contributions_json
                 FROM bordereau_lignes WHERE bordereau_id = ?1 ORDER BY id ASC",
            )
            .map_err(db_err)?;
        let l_rows = l_stmt
            .query_map(params![id], |row| {
                let desactive_val: i32 = row.get(14)?;
                let non_param_val: i32 = row.get(15)?;
                let contributions_json: Option<String> = row.get(16)?;
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
                    non_parametre: non_param_val != 0,
                    desactive: desactive_val != 0,
                    contributions: contributions_json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default(),
                    source_code: None,
                    source_code_ministeriel: None,
                    source_type: None,
                })
            })
            .map_err(db_err)?;
        b.lignes = l_rows.collect::<Result<Vec<_>, _>>().map_err(db_err)?;
        drop(l_stmt);
        drop(conn);
        b.fichiers = self.list_fichiers(id)?;
        Ok(Some(b))
    }

    /// Enregistre la facture de l'intermédiaire (confirmation). Possible en brouillon ou validé, pas une fois réglé :
    /// elle ne modifie ni les taux ni les montants calculés du bordereau.
    pub fn enregistrer_facture(&self, id: i64, facture: Option<&crate::models::FactureIntermediaire>) -> DbResult<()> {
        let conn = self.get_connection()?;
        let statut: String = conn
            .query_row("SELECT statut FROM bordereaux WHERE id = ?1", params![id], |r| r.get(0))
            .optional()
            .map_err(db_err)?
            .ok_or_else(|| format!("Bordereau #{} introuvable.", id))?;
        if statut == "REGLE" {
            return Err("Bordereau réglé : la facture ne peut plus être modifiée.".to_string());
        }
        if let Some(f) = facture {
            if let Some((k, v)) = f.montants_ht.iter().find(|(_, v)| !v.is_finite()) {
                return Err(format!("Montant invalide pour « {} » : {}", k, v));
            }
        }
        let json = facture.and_then(|f| serde_json::to_string(f).ok());
        conn.execute("UPDATE bordereaux SET facture_json = ?1 WHERE id = ?2", params![json, id])
            .map_err(db_err)?;
        Ok(())
    }

    pub fn update_bordereau_statut(&self, id: i64, statut: &str) -> DbResult<()> {
        if !STATUTS_BORDEREAU.contains(&statut) {
            return Err(format!("Statut invalide : {}", statut));
        }
        let conn = self.get_connection()?;
        let actuel: String = conn
            .query_row("SELECT statut FROM bordereaux WHERE id = ?1", params![id], |r| r.get(0))
            .optional()
            .map_err(db_err)?
            .ok_or_else(|| format!("Bordereau #{} introuvable.", id))?;
        let autorise = matches!(
            (actuel.as_str(), statut),
            ("BROUILLON", "VALIDE") | ("VALIDE", "BROUILLON") | ("VALIDE", "REGLE") | ("REGLE", "VALIDE")
        ) || actuel == statut;
        if !autorise {
            return Err(format!("Passage du statut {} à {} non autorisé.", actuel, statut));
        }
        conn.execute("UPDATE bordereaux SET statut = ?1 WHERE id = ?2", params![statut, id])
            .map_err(db_err)?;
        Ok(())
    }

    pub fn delete_bordereau(&self, id: i64) -> DbResult<()> {
        let conn = self.get_connection()?;
        let statut: String = conn
            .query_row("SELECT statut FROM bordereaux WHERE id = ?1", params![id], |r| r.get(0))
            .optional()
            .map_err(db_err)?
            .ok_or_else(|| format!("Bordereau #{} introuvable.", id))?;
        if statut == "REGLE" {
            return Err("Un bordereau réglé ne peut pas être supprimé (repassez-le d'abord en VALIDE).".to_string());
        }
        // Lignes et fichiers supprimés en cascade
        conn.execute("DELETE FROM bordereaux WHERE id = ?1", params![id]).map_err(db_err)?;
        Self::purger_contenus_orphelins(&conn)
    }

    /// Supprime les contenus de fichiers qui ne sont plus rattachés à aucun bordereau
    fn purger_contenus_orphelins(conn: &Connection) -> DbResult<()> {
        conn.execute(
            "DELETE FROM fichiers_contenu WHERE hash NOT IN (SELECT hash FROM bordereau_fichiers)",
            [],
        )
        .map_err(db_err)?;
        Ok(())
    }

    // -----------------------------------------------------------------------------------------
    // Fichiers rattachés (imports sources et pièces jointes)
    // -----------------------------------------------------------------------------------------

    pub fn list_fichiers(&self, bordereau_id: i64) -> DbResult<Vec<FichierBordereau>> {
        let conn = self.get_connection()?;
        let mut st = conn
            .prepare(
                "SELECT id, bordereau_id, type_piece, nom_fichier, type_fichier, hash, taille, date_ajout, client_code,
                        client_nom, periode, total_prime_nette, nb_lignes, anomalies_json, selection
                 FROM bordereau_fichiers WHERE bordereau_id = ?1 ORDER BY type_piece ASC, id ASC",
            )
            .map_err(db_err)?;
        let rows = st
            .query_map(params![bordereau_id], |r| {
                let an: Option<String> = r.get(13)?;
                Ok(FichierBordereau {
                    id: Some(r.get(0)?),
                    bordereau_id: r.get(1)?,
                    type_piece: r.get(2)?,
                    nom_fichier: r.get(3)?,
                    type_fichier: r.get(4)?,
                    hash: r.get(5)?,
                    taille: r.get(6)?,
                    date_ajout: r.get(7)?,
                    client_code: r.get(8)?,
                    client_nom: r.get(9)?,
                    periode: r.get(10)?,
                    total_prime_nette: r.get(11)?,
                    nb_lignes: r.get(12)?,
                    anomalies: an.and_then(|j| serde_json::from_str::<Vec<Anomalie>>(&j).ok()).unwrap_or_default(),
                    selection: r.get(14)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    pub fn add_piece_jointe(&self, bordereau_id: i64, nom: &str, hash: &str, contenu: &[u8]) -> DbResult<i64> {
        let nom = nom.trim();
        if nom.is_empty() {
            return Err("Nom de fichier manquant.".to_string());
        }
        if contenu.is_empty() {
            return Err("Le fichier est vide.".to_string());
        }
        let conn = self.get_connection()?;
        let existe: Option<i64> = conn
            .query_row("SELECT id FROM bordereaux WHERE id = ?1", params![bordereau_id], |r| r.get(0))
            .optional()
            .map_err(db_err)?;
        if existe.is_none() {
            return Err(format!("Bordereau #{} introuvable.", bordereau_id));
        }
        let ext = Path::new(nom).extension().and_then(|e| e.to_str()).unwrap_or("").to_uppercase();
        conn.execute("INSERT OR IGNORE INTO fichiers_contenu (hash, contenu) VALUES (?1, ?2)", params![hash, contenu])
            .map_err(db_err)?;
        conn.execute(
            "INSERT INTO bordereau_fichiers (bordereau_id, type_piece, nom_fichier, type_fichier, hash, taille, date_ajout)
             VALUES (?1, 'PIECE_JOINTE', ?2, ?3, ?4, ?5, ?6)",
            params![
                bordereau_id,
                nom,
                if ext.is_empty() { "FICHIER".to_string() } else { ext },
                hash,
                contenu.len() as i64,
                Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
            ],
        )
        .map_err(db_err)?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_fichier_contenu(&self, fichier_id: i64) -> DbResult<(String, i64, Vec<u8>)> {
        let conn = self.get_connection()?;
        conn.query_row(
            "SELECT f.nom_fichier, f.bordereau_id, c.contenu FROM bordereau_fichiers f
             JOIN fichiers_contenu c ON c.hash = f.hash WHERE f.id = ?1",
            params![fichier_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(db_err)?
        .ok_or_else(|| format!("Fichier #{} introuvable.", fichier_id))
    }

    pub fn delete_piece_jointe(&self, fichier_id: i64) -> DbResult<()> {
        let conn = self.get_connection()?;
        let type_piece: String = conn
            .query_row("SELECT type_piece FROM bordereau_fichiers WHERE id = ?1", params![fichier_id], |r| r.get(0))
            .optional()
            .map_err(db_err)?
            .ok_or_else(|| format!("Fichier #{} introuvable.", fichier_id))?;
        if type_piece == "IMPORT" {
            return Err(
                "Un fichier source d'import ne peut pas être supprimé seul : il justifie le calcul du bordereau.".to_string(),
            );
        }
        conn.execute("DELETE FROM bordereau_fichiers WHERE id = ?1", params![fichier_id]).map_err(db_err)?;
        Self::purger_contenus_orphelins(&conn)
    }
}

/// N° de police comparable d'un fichier à l'autre (sans espaces, en majuscules)
pub fn normaliser_police(p: &str) -> String {
    p.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase()
}

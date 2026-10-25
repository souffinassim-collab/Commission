use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrancheParam {
    pub id: Option<i64>,
    pub code: String,
    pub branche: String,
    pub sous_code: String,
    pub sous_libelle: String,
    pub taux_apport_courtier: f64,
    pub taux_apport_aga: f64,
    pub taux_gestion_aga: f64,
    /// Codes utilisés dans les fichiers importés (ex: colonne "Branche" Excel : "VP,VO,VR"), séparés par des virgules
    #[serde(default)]
    pub alias_import: String,
    /// Libellé de la ligne cumulée sur le bordereau courtier (ex: "RC AUTOMOBILE") ; vide = libellé propre
    #[serde(default)]
    pub groupe_bordereau: String,
    /// Ordre d'affichage de la ligne cumulée sur le bordereau
    #[serde(default = "default_ordre")]
    pub ordre_bordereau: i32,
    /// Catégorie d'affichage dans l'écran Paramètres (Automobile, Transport…) ; aucune = « Sans catégorie »
    #[serde(default)]
    pub categorie_id: Option<i64>,
}

/// Catégorie de paramètres (regroupement d'affichage, gérée par l'utilisateur)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Categorie {
    pub id: Option<i64>,
    pub libelle: String,
    #[serde(default)]
    pub ordre: i32,
    /// Nombre de lignes de paramètres rattachées (lecture seule)
    #[serde(default)]
    pub nb_branches: i64,
}

fn default_ordre() -> i32 {
    999
}

/// Anomalie détectée lors de l'import ou du calcul. niveau : "ERREUR" (bloquante), "AVERTISSEMENT", "INFO"
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Anomalie {
    pub niveau: String,
    pub message: String,
    /// Code de la ligne de bordereau concernée : l'anomalie ne bloque plus si cette ligne est exclue
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ligne: Option<String>,
}

impl Anomalie {
    pub fn erreur(m: impl Into<String>) -> Self {
        Self { niveau: "ERREUR".to_string(), message: m.into(), ligne: None }
    }
    pub fn avertissement(m: impl Into<String>) -> Self {
        Self { niveau: "AVERTISSEMENT".to_string(), message: m.into(), ligne: None }
    }
    pub fn info(m: impl Into<String>) -> Self {
        Self { niveau: "INFO".to_string(), message: m.into(), ligne: None }
    }
    pub fn pour_ligne(mut self, code: impl Into<String>) -> Self {
        self.ligne = Some(code.into());
        self
    }
    pub fn est_bloquante(&self) -> bool {
        self.niveau == "ERREUR"
    }
}

/// Fichier (import source ou pièce jointe) rattaché à un bordereau
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FichierBordereau {
    pub id: Option<i64>,
    pub bordereau_id: i64,
    pub type_piece: String, // "IMPORT" ou "PIECE_JOINTE"
    pub nom_fichier: String,
    pub type_fichier: String,
    pub hash: String,
    pub taille: i64,
    pub date_ajout: String,
    pub client_code: Option<String>,
    pub client_nom: Option<String>,
    pub periode: Option<String>,
    pub total_prime_nette: f64,
    pub nb_lignes: i64,
    #[serde(default)]
    pub anomalies: Vec<Anomalie>,
    /// Sélection retenue dans le fichier (ex: point de vente, clients)
    #[serde(default)]
    pub selection: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intermediaire {
    pub id: Option<i64>,
    pub type_intermediaire: String, // "AGA" ou "COURTIER"
    pub code: String,
    pub nom: String,
    pub succursale: String,
    pub departement: String,
    pub contact: Option<String>,
    pub adresse: Option<String>,
    pub tva_applicable: f64,
    pub actif: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LigneContribution {
    pub fichier_source: String,
    pub police: Option<String>,
    pub client: Option<String>,
    pub montant: f64,
    /// Police exclue manuellement par le commercial (ex: prime non encaissée) : non commissionnée
    #[serde(default)]
    pub exclue: bool,
    /// Informations de la police reprises du registre (pour le « Détail de production »)
    #[serde(default)]
    pub client_code: Option<String>,
    #[serde(default)]
    pub branche: Option<String>,
    #[serde(default)]
    pub date_emission: Option<String>,
    #[serde(default)]
    pub date_effet: Option<String>,
    #[serde(default)]
    pub date_echeance: Option<String>,
    #[serde(default)]
    pub cp: Option<f64>,
    #[serde(default)]
    pub prime_ttc: Option<f64>,
    /// Police déjà facturée dans un autre bordereau (« bordereau X — facture N° Y ») : verrouillée, non comptée
    #[serde(default)]
    pub deja_facturee: Option<String>,
    /// Code garantie d'origine (ex: VRC, VASS, VTR pour un PDF de flotte ; code branche pour un registre)
    #[serde(default)]
    pub garantie: Option<String>,
}

/// Facture de l'intermédiaire saisie par le commercial pour confirmation (rapprochement avec le calcul)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FactureIntermediaire {
    #[serde(default)]
    pub numero: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub commentaire: String,
    /// Commission HT déclarée par ligne du bordereau (clé : code|libellé|branche de la ligne)
    #[serde(default)]
    pub montants_ht: std::collections::BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BordereauLigne {
    pub id: Option<i64>,
    pub bordereau_id: Option<i64>,
    pub branche_id: Option<i64>,
    pub code_garantie: String,
    pub libelle_garantie: String,
    pub branche_nom: String,
    pub prime_nette: f64,
    pub taux_apport: f64,
    pub taux_gestion: f64,
    pub montant_apport: f64,
    pub montant_gestion: f64,
    pub montant_ht: f64,
    pub montant_tva: f64,
    pub montant_ttc: f64,
    #[serde(default)]
    pub non_parametre: bool,
    #[serde(default)]
    pub desactive: bool,
    #[serde(default)]
    pub contributions: Vec<LigneContribution>,
    /// Code d'origine (garantie CIRIS/flotte ou branche Excel) — sert au paramétrage d'une ligne non paramétrée
    #[serde(default)]
    pub source_code: Option<String>,
    /// Code ministériel lu dans le fichier (ex: 3.1.0.2)
    #[serde(default)]
    pub source_code_ministeriel: Option<String>,
    /// "GARANTIE" (sous-code) ou "BRANCHE_IMPORT" (alias de branche Excel)
    #[serde(default)]
    pub source_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bordereau {
    pub id: Option<i64>,
    pub intermediaire_id: Option<i64>,
    pub intermediaire_nom: String,
    pub intermediaire_code: String,
    pub type_intermediaire: String,
    pub numero_bordereau: String,
    pub exercice: i32,
    pub mois: Option<i32>,
    pub code_client: Option<String>,
    pub succursale: String,
    pub departement: String,
    pub tva_taux: f64,
    pub date_generation: String,
    pub total_prime_nette: f64,
    pub total_commission_apport: f64,
    pub total_commission_gestion: f64,
    pub total_commission_ht: f64,
    pub total_tva: f64,
    pub total_commission_ttc: f64,
    pub statut: String,
    pub lignes: Vec<BordereauLigne>,
    #[serde(default)]
    pub anomalies: Vec<Anomalie>,
    #[serde(default)]
    pub fichiers: Vec<FichierBordereau>,
    /// Facture de l'intermédiaire saisie pour confirmation
    #[serde(default)]
    pub facture: Option<FactureIntermediaire>,
    /// Client(s) du bordereau au format « code nom » (plusieurs : séparés par « & »)
    #[serde(default)]
    pub client_libelle: Option<String>,
}

/// Libellé client « code nom » à partir des couples (code, nom) ; au-delà de 5 clients : « N clients »
pub fn libelle_clients<'a>(couples: impl Iterator<Item = (Option<&'a str>, Option<&'a str>)>) -> Option<String> {
    let mut vus: Vec<String> = Vec::new();
    for (code, nom) in couples {
        let code = code.map(str::trim).unwrap_or("");
        let nom = nom.map(str::trim).unwrap_or("");
        let lib = format!("{} {}", code, nom).trim().to_string();
        if !lib.is_empty() && !vus.contains(&lib) {
            vus.push(lib);
        }
    }
    match vus.len() {
        0 => None,
        1..=5 => Some(vus.join(" & ")),
        n => Some(format!("{} clients", n)),
    }
}

/// Date au format JJ/MM/AAAA, vérifiée (jour existant) ; vide accepté
pub fn date_valide(s: &str) -> bool {
    let s = s.trim();
    s.is_empty() || (s.len() == 10 && chrono::NaiveDate::parse_from_str(s, "%d/%m/%Y").is_ok())
}

impl BordereauLigne {
    /// Ligne prise en compte : non désactivée et au moins une contribution non exclue
    pub fn est_active(&self) -> bool {
        !self.desactive && (self.contributions.is_empty() || self.contributions.iter().any(|c| !c.exclue))
    }
}

impl Bordereau {
    /// Anomalies qui empêchent l'enregistrement (celles liées à une ligne exclue ne bloquent plus)
    pub fn anomalies_bloquantes(&self) -> Vec<&Anomalie> {
        self.anomalies
            .iter()
            .filter(|a| a.est_bloquante())
            .filter(|a| match &a.ligne {
                Some(code) => self.lignes.iter().any(|l| &l.code_garantie == code && l.est_active()),
                None => true,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedItem {
    pub fichier_source: String,
    pub police: Option<String>,
    pub client_code: Option<String>,
    pub client_nom: Option<String>,
    pub code_garantie: String,
    pub libelle_garantie: String,
    pub branche_code: String,
    pub branche_libelle: String,
    pub prime_nette: f64,
    pub prime_ttc: Option<f64>,
    /// Code ministériel de la branche tel qu'indiqué dans le fichier
    #[serde(default)]
    pub code_ministeriel: Option<String>,
    /// "GARANTIE" (code garantie) ou "BRANCHE_IMPORT" (code branche d'un registre Excel)
    #[serde(default = "default_source_type")]
    pub source_type: String,
    /// Registre Excel : dates (JJ/MM/AAAA) et coût de police
    #[serde(default)]
    pub date_emission: Option<String>,
    #[serde(default)]
    pub date_effet: Option<String>,
    #[serde(default)]
    pub date_echeance: Option<String>,
    #[serde(default)]
    pub cp: Option<f64>,
}

fn default_source_type() -> String {
    "GARANTIE".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadedFileResult {
    pub id: String,
    pub file_name: String,
    pub file_type: String, // "CIRIS_PDF", "FLOTTE_PDF", "EXCEL_PRODUCTION"
    pub courtier_ou_client: Option<String>,
    pub code: Option<String>,
    pub periode: Option<String>,
    pub total_prime_nette: f64,
    pub items: Vec<ExtractedItem>,
    #[serde(default)]
    pub anomalies: Vec<Anomalie>,
    /// Contrôles liés au bordereau en cours (intermédiaire, période, fichier déjà commissionné) :
    /// recalculés dès que l'intermédiaire ou la période est connu(e), et de nouveau à l'enregistrement
    #[serde(default)]
    pub anomalies_contexte: Vec<Anomalie>,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub taille: i64,
    /// Exercice / mois détectés dans le fichier (rapport CIRIS)
    #[serde(default)]
    pub exercice: Option<i32>,
    #[serde(default)]
    pub mois: Option<i32>,
    /// Registre Excel : point de vente retenu (préfixe du N° de police = code agence / AGA)
    #[serde(default)]
    pub point_vente: Option<String>,
    /// Registre Excel : clients retenus (affaires apportées par un courtier)
    #[serde(default)]
    pub clients_selectionnes: Vec<String>,
    /// Description de la sélection faite dans le fichier (enregistrée avec le bordereau)
    #[serde(default)]
    pub selection: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointVenteSummary {
    pub code: String,
    pub nombre_polices: usize,
    pub nombre_clients: usize,
    pub total_prime_nette: f64,
    pub branches: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelClientSummary {
    pub code_client: String,
    pub designation_client: String,
    pub nombre_polices: usize,
    pub total_prime_nette: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelScanResult {
    /// Identifiant du classeur conservé côté application le temps de la sélection
    pub staging_id: String,
    pub file_name: String,
    pub total_lignes: usize,
    pub points_vente: Vec<PointVenteSummary>,
    /// Informations de l'export CIRIS (agence, période d'extraction)
    pub info_export: Vec<String>,
    pub periode_emission: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculateMultiBordereauRequest {
    pub files: Vec<UploadedFileResult>,
    /// Bordereau en cours de modification (ses propres polices ne sont pas des doublons)
    #[serde(default)]
    pub bordereau_id: Option<i64>,
    #[serde(default)]
    pub intermediaire_id: Option<i64>,
    pub type_intermediaire: String, // "COURTIER" ou "AGA"
    pub intermediaire_code: String,
    pub intermediaire_nom: String,
    pub code_client: Option<String>,
    pub succursale: String,
    pub departement: String,
    pub tva_taux: f64,
    pub exercice: i32,
    pub mois: Option<i32>,
    pub cumuler_par_branche: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbInfo {
    pub db_path: String,
    pub version: i32,
    pub total_branches: i64,
    pub total_intermediaires: i64,
    pub total_bordereaux: i64,
    pub last_backup: Option<String>,
}

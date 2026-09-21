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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CirisGarantieItem {
    pub garantie: String,
    pub description: String,
    pub branche_code: String,
    pub branche_libelle: String,
    pub prime_nette: f64,
    pub primes_ttc: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CirisPdfExtractedData {
    pub agence_code: String,
    pub agence_nom: String,
    pub periode_debut: String,
    pub periode_fin: String,
    pub mois: i32,
    pub exercice: i32,
    pub devise: String,
    pub total_pdf_primes_nettes: f64,
    pub items: Vec<CirisGarantieItem>,
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

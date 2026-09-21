use crate::models::{BrancheParam, BordereauLigne, CirisGarantieItem};

pub struct CalculationEngine;

impl CalculationEngine {
    pub fn round_cents(val: f64) -> f64 {
        (val * 100.0).round() / 100.0
    }

    pub fn match_branche<'a>(
        item: &CirisGarantieItem,
        branches: &'a [BrancheParam],
    ) -> Option<&'a BrancheParam> {
        let clean_code = item.garantie.trim().to_uppercase();
        let clean_desc = item.description.trim().to_lowercase();
        let clean_branche = item.branche_libelle.trim().to_lowercase();

        // 1. Recherche par correspondance exacte sous_code
        let candidates: Vec<&'a BrancheParam> = branches
            .iter()
            .filter(|b| b.sous_code.trim().to_uppercase() == clean_code && !b.sous_code.is_empty())
            .collect();

        if candidates.len() == 1 {
            return Some(candidates[0]);
        }

        // Si plusieurs candidats (ex: VDR, VPT, VRC sur différentes branches d'auto)
        if !candidates.is_empty() {
            // Affiner par la branche du rapport (ex: "3.1.0.1", "Automobile", "Entreprise", "Frontière")
            for c in &candidates {
                let b_lower = c.branche.to_lowercase();
                if clean_branche.contains(&b_lower) || b_lower.contains("auto") {
                    return Some(c);
                }
            }
            return Some(candidates[0]);
        }

        // 2. Recherche par libellé de sous-garantie
        for b in branches {
            let b_sub = b.sous_libelle.trim().to_lowercase();
            if !b_sub.is_empty() && (clean_desc.contains(&b_sub) || b_sub.contains(&clean_desc)) {
                return Some(b);
            }
        }

        // 3. Recherche par nom de branche
        for b in branches {
            let b_name = b.branche.trim().to_lowercase();
            if !b_name.is_empty() && (clean_branche.contains(&b_name) || b_name.contains(&clean_branche)) {
                return Some(b);
            }
        }

        None
    }

    pub fn compute_ligne(
        item: &CirisGarantieItem,
        matched_branche: Option<&BrancheParam>,
        type_intermediaire: &str,
        tva_taux: f64,
    ) -> BordereauLigne {
        let (taux_apport, taux_gestion, branche_id, branche_nom, libelle) = match matched_branche {
            Some(b) => {
                let t_app = if type_intermediaire == "COURTIER" {
                    b.taux_apport_courtier
                } else {
                    b.taux_apport_aga
                };
                let t_gest = if type_intermediaire == "COURTIER" {
                    0.0
                } else {
                    b.taux_gestion_aga
                };
                (
                    t_app,
                    t_gest,
                    b.id,
                    b.branche.clone(),
                    if !b.sous_libelle.is_empty() {
                        b.sous_libelle.clone()
                    } else {
                        item.description.clone()
                    },
                )
            }
            None => (0.0, 0.0, None, item.branche_libelle.clone(), item.description.clone()),
        };

        let montant_apport = Self::round_cents(item.prime_nette * taux_apport);
        let montant_gestion = Self::round_cents(item.prime_nette * taux_gestion);
        let montant_ht = Self::round_cents(montant_apport + montant_gestion);
        let montant_tva = Self::round_cents(montant_ht * tva_taux);
        let montant_ttc = Self::round_cents(montant_ht + montant_tva);

        BordereauLigne {
            id: None,
            bordereau_id: None,
            branche_id,
            code_garantie: item.garantie.clone(),
            libelle_garantie: libelle,
            branche_nom,
            prime_nette: item.prime_nette,
            taux_apport,
            taux_gestion,
            montant_apport,
            montant_gestion,
            montant_ht,
            montant_tva,
            montant_ttc,
        }
    }
}

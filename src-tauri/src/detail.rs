//! « Détail de production » à confirmer : détail par police, rapprochement par ligne du bordereau entre la
//! commission calculée et la commission déclarée sur la facture de l'intermédiaire, et calcul des écarts.
//! Une seule construction sert à l'écran, à l'impression et à l'export Excel.

use std::collections::BTreeMap;
use serde::Serialize;
use crate::calculation::CalculationEngine;
use crate::models::{Bordereau, BordereauLigne};

/// Clé stable d'une ligne de bordereau (sert à rattacher les montants de la facture)
pub fn cle_ligne(l: &BordereauLigne) -> String {
    format!("{}|{}|{}", l.code_garantie, l.libelle_garantie, l.branche_nom)
}

fn r2(v: f64) -> f64 {
    CalculationEngine::round_cents(v)
}

#[derive(Debug, Clone, Serialize)]
pub struct PoliceDetail {
    pub exercice: i32,
    pub police: String,
    pub branche: String,
    pub client: Option<String>,
    pub date_emission: Option<String>,
    pub date_effet: Option<String>,
    pub date_echeance: Option<String>,
    pub prime_nette: f64,
    /// Montant par colonne de ventilation (polices réparties sur plusieurs lignes : RC, assistance, dommages…)
    pub ventilation: Vec<Option<f64>>,
    pub cp: Option<f64>,
    pub prime_ttc: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LigneRapprochement {
    pub cle: String,
    pub libelle: String,
    pub prime_nette: f64,
    pub taux: f64,
    pub com_ht_calculee: f64,
    pub com_ht_declaree: Option<f64>,
    pub ecart: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DetailProduction {
    /// « Détail de production courtier » (ou AGA)
    pub titre: String,
    pub succursale: String,
    pub departement: String,
    pub type_intermediaire: String,
    pub intermediaire: String,
    pub clients: Vec<String>,
    pub exercice: i32,
    pub mois: Option<i32>,
    pub numero_bordereau: String,
    pub colonnes_ventilation: Vec<String>,
    pub polices: Vec<PoliceDetail>,
    pub total_prime_nette: f64,
    pub total_ventilation: Vec<f64>,
    pub total_cp: f64,
    pub total_prime_ttc: f64,
    pub lignes: Vec<LigneRapprochement>,
    pub total_prime_lignes: f64,
    /// Total des lignes − total du détail par police : doit être nul
    pub controle: f64,
    pub tva_taux: f64,
    pub ht_calcule: f64,
    pub tva_calculee: f64,
    pub ttc_calcule: f64,
    pub facture_saisie: bool,
    pub facture_numero: String,
    pub facture_date: String,
    pub ht_declare: f64,
    pub tva_declaree: f64,
    pub ttc_declare: f64,
    pub ecart_ht: f64,
    pub ecart_tva: f64,
    pub ecart_ttc: f64,
    pub lignes_non_saisies: usize,
    /// Polices exclues par le commercial (primes non encaissées) : (police, client, montant)
    pub polices_exclues: Vec<(String, String, f64)>,
}

pub fn construire(b: &Bordereau) -> DetailProduction {
    let lignes: Vec<&BordereauLigne> = b.lignes.iter().filter(|l| l.est_active()).collect();

    // Polices : contributions non exclues, regroupées par N° de police (sans police : par fichier et ligne)
    struct Acc {
        branche: String,
        client: Option<String>,
        client_code: Option<String>,
        dates: (Option<String>, Option<String>, Option<String>),
        cp: Option<f64>,
        ttc: Option<f64>,
        par_ligne: BTreeMap<usize, f64>,
        par_garantie: BTreeMap<String, f64>,
    }
    let mut polices: BTreeMap<String, Acc> = BTreeMap::new();
    let mut exclues: BTreeMap<String, (String, f64)> = BTreeMap::new();
    for (li, l) in lignes.iter().enumerate() {
        for c in &l.contributions {
            let police = c.police.clone().unwrap_or_else(|| format!("{} — {}", c.fichier_source, l.libelle_garantie));
            if c.deja_facturee.is_some() {
                continue;
            }
            if c.exclue {
                let e = exclues.entry(police).or_insert((c.client.clone().unwrap_or_default(), 0.0));
                e.1 += c.montant;
                continue;
            }
            let a = polices.entry(police).or_insert_with(|| Acc {
                branche: c.branche.clone().unwrap_or_else(|| l.code_garantie.clone()),
                client: c.client.clone(),
                client_code: c.client_code.clone(),
                dates: (c.date_emission.clone(), c.date_effet.clone(), c.date_echeance.clone()),
                cp: c.cp,
                ttc: c.prime_ttc,
                par_ligne: BTreeMap::new(),
                par_garantie: BTreeMap::new(),
            });
            // Informations de police : première valeur connue (CP / TTC sont propres à la police, non cumulés)
            a.cp = a.cp.or(c.cp);
            a.ttc = a.ttc.or(c.prime_ttc);
            a.dates.0 = a.dates.0.clone().or_else(|| c.date_emission.clone());
            a.dates.1 = a.dates.1.clone().or_else(|| c.date_effet.clone());
            a.dates.2 = a.dates.2.clone().or_else(|| c.date_echeance.clone());
            *a.par_ligne.entry(li).or_insert(0.0) += c.montant;
            *a.par_garantie.entry(c.garantie.clone().unwrap_or_default().to_uppercase()).or_insert(0.0) += c.montant;
        }
    }
    // Ventilation des polices automobile détaillées par garantie (PDF de flotte) — colonnes de l'état :
    // VRC (RC), VRCV (autres RC auto : remorque, étranger), ASSISTANSE, Dommages (reste de la prime nette)
    let colonnes_ventilation: Vec<String> = ["VRC", "VRCV", "ASSISTANSE", "Dommages"].iter().map(|s| s.to_string()).collect();
    const GARANTIES_AUTO: [&str; 13] = ["VRC", "VRCR", "VRCE", "VASS", "VTR", "VDR", "VPT", "VBDG", "VDC", "VINV", "VOLV", "VVA", "VVI"];

    // Clients : un même nom avec plusieurs codes s'écrit « NOM code1 & code2 »
    let mut clients_codes: Vec<(String, Vec<String>)> = Vec::new();
    let mut details: Vec<PoliceDetail> = Vec::new();
    let mut total_vent = vec![0.0; 4];
    let (mut total_pn, mut total_cp, mut total_ttc) = (0.0, 0.0, 0.0);
    for (police, a) in &polices {
        let pn: f64 = a.par_ligne.values().sum();
        let g = |codes: &[&str]| -> f64 { codes.iter().map(|c| a.par_garantie.get(*c).copied().unwrap_or(0.0)).sum() };
        let ventilee = a.par_garantie.keys().any(|k| GARANTIES_AUTO.contains(&k.as_str()));
        let ventilation: Vec<Option<f64>> = if ventilee {
            let (vrc, vrcv, ass) = (g(&["VRC"]), g(&["VRCR", "VRCE", "VRCV"]), g(&["VASS"]));
            [vrc, vrcv, ass, pn - vrc - vrcv - ass]
                .iter()
                .map(|v| if v.abs() >= 0.005 { Some(r2(*v)) } else { None })
                .collect()
        } else {
            vec![None; 4]
        };
        for (k, v) in ventilation.iter().enumerate() {
            total_vent[k] += v.unwrap_or(0.0);
        }
        total_pn += pn;
        total_cp += a.cp.unwrap_or(0.0);
        total_ttc += a.ttc.unwrap_or(0.0);
        if let Some(nom) = a.client.as_ref().map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
            let idx = match clients_codes.iter().position(|(n, _)| *n == nom) {
                Some(i) => i,
                None => {
                    clients_codes.push((nom, Vec::new()));
                    clients_codes.len() - 1
                }
            };
            if let Some(code) = a.client_code.as_ref().filter(|c| !c.is_empty()) {
                if !clients_codes[idx].1.contains(code) {
                    clients_codes[idx].1.push(code.clone());
                }
            }
        }
        details.push(PoliceDetail {
            exercice: b.exercice,
            police: police.clone(),
            branche: a.branche.clone(),
            client: a.client.clone(),
            date_emission: a.dates.0.clone(),
            date_effet: a.dates.1.clone(),
            date_echeance: a.dates.2.clone(),
            prime_nette: r2(pn),
            ventilation,
            cp: a.cp,
            prime_ttc: a.ttc,
        });
    }

    // Ordre de l'état : polices simples par N° de police, puis polices automobile ventilées (RC / assistance / dommages)
    details.sort_by(|a, b| {
        let va = a.ventilation.iter().any(|v| v.is_some());
        let vb = b.ventilation.iter().any(|v| v.is_some());
        va.cmp(&vb).then_with(|| a.police.cmp(&b.police))
    });

    let clients: Vec<String> = clients_codes
        .into_iter()
        .map(|(nom, codes)| if codes.is_empty() { nom } else { format!("{} {}", nom, codes.join(" & ")) })
        .collect();

    // Rapprochement par ligne : commission calculée / déclarée sur la facture
    let facture = b.facture.clone().unwrap_or_default();
    let facture_saisie = b.facture.is_some();
    let mut rappro = Vec::new();
    let (mut tp, mut decl) = (0.0, 0.0);
    let mut non_saisies = 0;
    for l in &lignes {
        let cle = cle_ligne(l);
        let declare = facture.montants_ht.get(&cle).copied();
        if declare.is_none() {
            non_saisies += 1;
        }
        tp += l.prime_nette;
        decl += declare.unwrap_or(0.0);
        rappro.push(LigneRapprochement {
            cle,
            libelle: l.libelle_garantie.clone(),
            prime_nette: l.prime_nette,
            taux: l.taux_apport + l.taux_gestion,
            com_ht_calculee: l.montant_ht,
            com_ht_declaree: declare,
            ecart: declare.map(|d| r2(d - l.montant_ht)),
        });
    }
    let ht_declare = r2(decl);
    let tva_declaree = r2(ht_declare * b.tva_taux);
    let ttc_declare = r2(ht_declare + tva_declaree);

    DetailProduction {
        titre: if b.type_intermediaire == "COURTIER" {
            "Détail de production courtier".to_string()
        } else {
            "Détail de production AGA".to_string()
        },
        succursale: b.succursale.clone(),
        departement: b.departement.clone(),
        type_intermediaire: b.type_intermediaire.clone(),
        intermediaire: format!("{} {}", b.intermediaire_code, b.intermediaire_nom),
        clients,
        exercice: b.exercice,
        mois: b.mois,
        numero_bordereau: b.numero_bordereau.clone(),
        colonnes_ventilation,
        polices: details,
        total_prime_nette: r2(total_pn),
        total_ventilation: total_vent.into_iter().map(r2).collect(),
        total_cp: r2(total_cp),
        total_prime_ttc: r2(total_ttc),
        lignes: rappro,
        total_prime_lignes: r2(tp),
        controle: r2(tp - total_pn),
        tva_taux: b.tva_taux,
        ht_calcule: b.total_commission_ht,
        tva_calculee: b.total_tva,
        ttc_calcule: b.total_commission_ttc,
        facture_saisie,
        facture_numero: facture.numero.clone(),
        facture_date: facture.date.clone(),
        ht_declare,
        tva_declaree,
        ttc_declare,
        ecart_ht: r2(ht_declare - b.total_commission_ht),
        ecart_tva: r2(tva_declaree - b.total_tva),
        ecart_ttc: r2(ttc_declare - b.total_commission_ttc),
        lignes_non_saisies: non_saisies,
        polices_exclues: exclues.into_iter().map(|(p, (c, m))| (p, c, r2(m))).collect(),
    }
}

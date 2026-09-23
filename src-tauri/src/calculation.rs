use std::collections::HashMap;
use crate::models::{
    Anomalie, Bordereau, BordereauLigne, BrancheParam, CalculateMultiBordereauRequest, ExtractedItem, LigneContribution,
};

pub struct CalculationEngine;

/// Résultat de la recherche du barème applicable à une ligne importée
pub enum Resolution<'a> {
    Trouve(&'a BrancheParam),
    Ambigu(Vec<&'a BrancheParam>),
    Absent,
}

pub fn alias_list(b: &BrancheParam) -> Vec<String> {
    b.alias_import
        .split(|c| c == ',' || c == ';')
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn libelle_param(p: &BrancheParam) -> String {
    if p.sous_libelle.trim().is_empty() {
        p.branche.clone()
    } else {
        p.sous_libelle.clone()
    }
}

fn fmt2(v: f64) -> String {
    format!("{:.2}", v)
}

impl CalculationEngine {
    pub fn round_cents(val: f64) -> f64 {
        (val * 100.0).round() / 100.0
    }

    /// Barème applicable, exclusivement à partir des paramètres enregistrés en BDD :
    /// - garantie (CIRIS / flotte) : sous-code exact, départagé par le code ministériel si besoin ;
    /// - branche d'un registre Excel : alias d'import déclaré sur la ligne de paramètre.
    /// Aucun rapprochement approximatif ni taux par défaut.
    pub fn resolve_param<'a>(item: &ExtractedItem, branches: &'a [BrancheParam]) -> Resolution<'a> {
        let code = item.code_garantie.trim().to_uppercase();
        let by_alias = || -> Resolution<'a> {
            let c: Vec<&BrancheParam> = branches.iter().filter(|b| alias_list(b).contains(&code)).collect();
            match c.len() {
                0 => Resolution::Absent,
                1 => Resolution::Trouve(c[0]),
                _ => Resolution::Ambigu(c),
            }
        };

        if item.source_type == "BRANCHE_IMPORT" {
            return by_alias();
        }

        let candidates: Vec<&BrancheParam> = branches
            .iter()
            .filter(|b| !b.sous_code.trim().is_empty() && b.sous_code.trim().to_uppercase() == code)
            .collect();
        match candidates.len() {
            0 => by_alias(),
            1 => Resolution::Trouve(candidates[0]),
            _ => {
                if let Some(cm) = item.code_ministeriel.as_deref() {
                    let m: Vec<&BrancheParam> = candidates.iter().copied().filter(|b| b.code.trim() == cm.trim()).collect();
                    if m.len() == 1 {
                        return Resolution::Trouve(m[0]);
                    }
                }
                Resolution::Ambigu(candidates)
            }
        }
    }

    pub fn compute_multi_bordereau(req: &CalculateMultiBordereauRequest, branches: &[BrancheParam]) -> Bordereau {
        let mut anomalies: Vec<Anomalie> = Vec::new();
        let is_courtier = req.type_intermediaire == "COURTIER";
        // Regroupement "modèle officiel courtier" uniquement si le cumul par branche est demandé
        let use_groups = is_courtier && req.cumuler_par_branche;

        let norm_police = |p: &str| p.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();

        // 1. Chaque fichier est une source indépendante. Contrôle : une même police ne doit figurer que dans
        //    un seul fichier ; si c'est le cas malgré tout, elle n'est comptée qu'une fois (premier fichier importé)
        //    et l'anomalie est signalée.
        let mut premier_fichier: HashMap<String, String> = HashMap::new();
        for file in &req.files {
            for p in file.items.iter().filter_map(|i| i.police.as_deref()).map(norm_police).filter(|p| !p.is_empty()) {
                premier_fichier.entry(p).or_insert_with(|| file.file_name.clone());
            }
        }
        // (police, fichier retenu, fichier écarté) -> (lignes écartées, montant écarté)
        let mut ecartees: std::collections::BTreeMap<(String, String, String), (usize, f64)> = std::collections::BTreeMap::new();

        struct Acc {
            branche_id: Option<i64>,
            code_garantie: String,
            libelle_garantie: String,
            branche_nom: String,
            prime_nette: f64,
            taux_apport: f64,
            taux_gestion: f64,
            non_parametre: bool,
            contributions: Vec<LigneContribution>,
            ordre: (i32, usize),
            source_code: Option<String>,
            source_code_ministeriel: Option<String>,
            source_type: Option<String>,
        }

        let mut groups: HashMap<String, Acc> = HashMap::new();
        let mut seq = 0usize;

        for file in &req.files {
            for item in &file.items {
                if let Some(p) = item.police.as_deref().map(norm_police).filter(|p| !p.is_empty()) {
                    if let Some(retenu) = premier_fichier.get(&p) {
                        if retenu != &file.file_name {
                            let e = ecartees.entry((p, retenu.clone(), file.file_name.clone())).or_insert((0, 0.0));
                            e.0 += 1;
                            e.1 += item.prime_nette;
                            continue;
                        }
                    }
                }

                let code = item.code_garantie.trim().to_uppercase();
                let resolution = Self::resolve_param(item, branches);
                if let Resolution::Ambigu(c) = &resolution {
                    anomalies.push(
                        Anomalie::erreur(format!(
                            "Paramétrage ambigu pour « {} » ({}) : {} lignes de paramètres correspondent ({}). Corrigez les paramètres.",
                            code,
                            item.fichier_source,
                            c.len(),
                            c.iter().map(|b| format!("{} {}", b.code, b.sous_code)).collect::<Vec<_>>().join(", ")
                        ))
                        .pour_ligne(code.clone()),
                    );
                }

                let source_code = Some(code.clone());
                let source_code_ministeriel = item.code_ministeriel.clone();
                let source_type = Some(item.source_type.clone());

                let (key, acc) = match resolution {
                    Resolution::Trouve(p) => {
                        let (ta, tg) = if is_courtier {
                            (p.taux_apport_courtier, 0.0)
                        } else {
                            (p.taux_apport_aga, p.taux_gestion_aga)
                        };
                        let zero = if is_courtier { ta == 0.0 } else { ta == 0.0 && tg == 0.0 };
                        if use_groups {
                            let label = if p.groupe_bordereau.trim().is_empty() {
                                libelle_param(p)
                            } else {
                                p.groupe_bordereau.trim().to_string()
                            };
                            (
                                format!("G|{}|{}|{}", label, ta, tg),
                                Acc {
                                    branche_id: p.id,
                                    code_garantie: label.clone(),
                                    libelle_garantie: label,
                                    branche_nom: p.branche.clone(),
                                    prime_nette: 0.0,
                                    taux_apport: ta,
                                    taux_gestion: tg,
                                    non_parametre: zero,
                                    contributions: Vec::new(),
                                    ordre: (p.ordre_bordereau, seq),
                                    source_code,
                                    source_code_ministeriel,
                                    source_type,
                                },
                            )
                        } else {
                            let branche_nom = if item.branche_libelle.trim().is_empty() {
                                p.branche.clone()
                            } else {
                                item.branche_libelle.clone()
                            };
                            (
                                format!("P|{}|{}", p.id.unwrap_or(0), item.branche_code),
                                Acc {
                                    branche_id: p.id,
                                    code_garantie: if p.sous_code.trim().is_empty() { code.clone() } else { p.sous_code.clone() },
                                    libelle_garantie: libelle_param(p),
                                    branche_nom,
                                    prime_nette: 0.0,
                                    taux_apport: ta,
                                    taux_gestion: tg,
                                    non_parametre: zero,
                                    contributions: Vec::new(),
                                    ordre: (0, seq),
                                    source_code,
                                    source_code_ministeriel,
                                    source_type,
                                },
                            )
                        }
                    }
                    Resolution::Ambigu(_) | Resolution::Absent => (
                        format!(
                            "NP|{}|{}|{}",
                            item.source_type,
                            code,
                            item.code_ministeriel.clone().unwrap_or_else(|| item.branche_code.clone())
                        ),
                        Acc {
                            branche_id: None,
                            code_garantie: code.clone(),
                            libelle_garantie: format!("{} — non paramétré", code),
                            branche_nom: if item.branche_libelle.trim().is_empty() {
                                item.branche_code.clone()
                            } else {
                                item.branche_libelle.clone()
                            },
                            prime_nette: 0.0,
                            taux_apport: 0.0,
                            taux_gestion: 0.0,
                            non_parametre: true,
                            contributions: Vec::new(),
                            ordre: (-1, seq),
                            source_code,
                            source_code_ministeriel,
                            source_type,
                        },
                    ),
                };
                seq += 1;

                let entry = groups.entry(key).or_insert(acc);
                entry.prime_nette += item.prime_nette;
                entry.contributions.push(LigneContribution {
                    fichier_source: item.fichier_source.clone(),
                    police: item.police.clone(),
                    client: item.client_nom.clone(),
                    montant: item.prime_nette,
                    exclue: false,
                });
            }
        }

        for ((police, retenu, ecarte), (n, montant)) in &ecartees {
            anomalies.push(Anomalie::avertissement(format!(
                "Police {} présente dans deux fichiers : comptée une seule fois (« {} ») ; {} ligne(s) de « {} » ({} DZD) non comptée(s).",
                police,
                retenu,
                n,
                ecarte,
                fmt2(Self::round_cents(*montant))
            )));
        }

        // 2. Lignes finales ordonnées
        let mut sorted: Vec<Acc> = groups.into_values().collect();
        sorted.sort_by(|a, b| a.ordre.cmp(&b.ordre));

        if use_groups {
            let mut labels: HashMap<String, usize> = HashMap::new();
            for a in sorted.iter().filter(|a| a.branche_id.is_some()) {
                *labels.entry(a.libelle_garantie.clone()).or_insert(0) += 1;
            }
            let mut dup: Vec<_> = labels.into_iter().filter(|(_, n)| *n > 1).collect();
            dup.sort();
            for (label, n) in dup {
                anomalies.push(Anomalie::avertissement(format!(
                    "Le groupe « {} » contient des garanties à taux différents : {} lignes distinctes sur le bordereau.",
                    label, n
                )));
            }
        }

        for a in sorted.iter().filter(|a| a.non_parametre) {
            let msg = if a.branche_id.is_none() {
                format!(
                    "« {} » ({}) : aucun taux paramétré — prime {} DZD. Ajoutez ce taux dans les paramètres.",
                    a.source_code.clone().unwrap_or_default(),
                    a.branche_nom,
                    fmt2(Self::round_cents(a.prime_nette))
                )
            } else {
                format!(
                    "« {} » : taux à 0 % dans les paramètres pour un {} — prime {} DZD.",
                    a.libelle_garantie,
                    if is_courtier { "courtier" } else { "AGA" },
                    fmt2(Self::round_cents(a.prime_nette))
                )
            };
            anomalies.push(Anomalie::erreur(msg).pour_ligne(a.code_garantie.clone()));
        }

        let lignes: Vec<BordereauLigne> = sorted
            .into_iter()
            .map(|acc| BordereauLigne {
                id: None,
                bordereau_id: None,
                branche_id: acc.branche_id,
                code_garantie: acc.code_garantie,
                libelle_garantie: acc.libelle_garantie,
                branche_nom: acc.branche_nom,
                prime_nette: acc.prime_nette,
                taux_apport: acc.taux_apport,
                taux_gestion: acc.taux_gestion,
                montant_apport: 0.0,
                montant_gestion: 0.0,
                montant_ht: 0.0,
                montant_tva: 0.0,
                montant_ttc: 0.0,
                non_parametre: acc.non_parametre,
                desactive: false,
                contributions: acc.contributions,
                source_code: acc.source_code,
                source_code_ministeriel: acc.source_code_ministeriel,
                source_type: acc.source_type,
            })
            .collect();

        let code_inter: String = req
            .intermediaire_code
            .trim()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
            .collect();
        let num_bordereau = match req.mois {
            Some(m) if m > 0 => format!("BORD-{}-{}-{}-{:02}", req.type_intermediaire, code_inter, req.exercice, m),
            _ => format!("BORD-{}-{}-{}", req.type_intermediaire, code_inter, req.exercice),
        };

        let mut bordereau = Bordereau {
            id: None,
            intermediaire_id: req.intermediaire_id,
            intermediaire_nom: req.intermediaire_nom.clone(),
            intermediaire_code: req.intermediaire_code.clone(),
            type_intermediaire: req.type_intermediaire.clone(),
            numero_bordereau: num_bordereau,
            exercice: req.exercice,
            mois: req.mois,
            code_client: req.code_client.clone(),
            succursale: req.succursale.clone(),
            departement: req.departement.clone(),
            tva_taux: req.tva_taux,
            date_generation: chrono::Local::now().format("%Y-%m-%d").to_string(),
            total_prime_nette: 0.0,
            total_commission_apport: 0.0,
            total_commission_gestion: 0.0,
            total_commission_ht: 0.0,
            total_tva: 0.0,
            total_commission_ttc: 0.0,
            statut: "BROUILLON".to_string(),
            lignes,
            anomalies,
            fichiers: Vec::new(),
        };
        Self::recompute_bordereau(&mut bordereau);
        bordereau
    }

    /// Recalcule les montants de chaque ligne à partir de SES PROPRES taux (figés dans la ligne,
    /// jamais relus depuis le barème) puis les totaux du bordereau en excluant les lignes désactivées.
    /// Formule unique utilisée au calcul initial, à l'enregistrement et côté interface.
    pub fn recompute_bordereau(b: &mut Bordereau) {
        let tva = b.tva_taux;
        let mut total_prime_nette = 0.0;
        let mut total_apport = 0.0;
        let mut total_gestion = 0.0;

        for l in b.lignes.iter_mut() {
            // Prime de la ligne = somme des contributions (polices) non exclues par le commercial
            if !l.contributions.is_empty() {
                l.prime_nette = l.contributions.iter().filter(|c| !c.exclue).map(|c| c.montant).sum();
            }
            l.prime_nette = Self::round_cents(l.prime_nette);
            l.montant_apport = Self::round_cents(l.prime_nette * l.taux_apport);
            l.montant_gestion = Self::round_cents(l.prime_nette * l.taux_gestion);
            l.montant_ht = Self::round_cents(l.montant_apport + l.montant_gestion);
            l.montant_tva = Self::round_cents(l.montant_ht * tva);
            l.montant_ttc = Self::round_cents(l.montant_ht + l.montant_tva);

            if !l.desactive {
                total_prime_nette += l.prime_nette;
                total_apport += l.montant_apport;
                total_gestion += l.montant_gestion;
            }
        }

        b.total_prime_nette = Self::round_cents(total_prime_nette);
        b.total_commission_apport = Self::round_cents(total_apport);
        b.total_commission_gestion = Self::round_cents(total_gestion);
        b.total_commission_ht = Self::round_cents(b.total_commission_apport + b.total_commission_gestion);
        b.total_tva = Self::round_cents(b.total_commission_ht * tva);
        b.total_commission_ttc = Self::round_cents(b.total_commission_ht + b.total_tva);
    }
}

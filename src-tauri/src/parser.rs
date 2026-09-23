//! Lecture des fichiers de production (rapport CIRIS, PDF de flotte, registre Excel).
//!
//! Principes :
//! - aucune table de codes, de libellés ou de taux codée en dur : le découpage des codes garanties
//!   s'appuie uniquement sur les sous-codes enregistrés dans les paramètres (BDD) ;
//! - chaque lecture est contrôlée par recoupement avec les totaux présents dans le document ;
//! - toute incohérence est remontée sous forme d'`Anomalie` (ERREUR = bloquante pour l'enregistrement).

use lopdf::Document;
use crate::models::{Anomalie, ExtractedItem, UploadedFileResult};

const TOLERANCE: f64 = 0.005;

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn fmt_montant(v: f64) -> String {
    format!("{:.2}", v)
}

/// Montant au format anglo-saxon "1,234.56" (rapports CIRIS). Tolère un texte collé après le nombre.
fn parse_en_amount(token: &str) -> Option<f64> {
    if token.contains('/') || token.contains(':') {
        return None;
    }
    let start = token.find(|c: char| c.is_ascii_digit() || c == '-')?;
    let rest = &token[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == ',' || c == '.' || c == '-'))
        .unwrap_or(rest.len());
    let num = rest[..end].trim_end_matches(|c| c == ',' || c == '.');
    let dot = num.rfind('.')?;
    if num.len() - dot - 1 != 2 {
        return None;
    }
    num.replace(',', "").parse::<f64>().ok()
}

fn en_amounts(line: &str) -> Vec<f64> {
    line.split_whitespace().filter_map(parse_en_amount).collect()
}

/// Montants au format français "1.234,56" (PDF de flotte). Tolère un texte collé avant/après.
fn fr_amounts(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    for w in text.split_whitespace() {
        let start = match w.find(|c: char| c.is_ascii_digit() || c == '-') {
            Some(s) => s,
            None => continue,
        };
        let rest = &w[start..];
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || c == ',' || c == '.' || c == '-'))
            .unwrap_or(rest.len());
        let num = &rest[..end];
        if let Some(comma) = num.rfind(',') {
            if num.len() - comma - 1 == 2 && num.matches(',').count() == 1 {
                let s = num[..comma].replace('.', "") + "." + &num[comma + 1..];
                if let Ok(v) = s.parse::<f64>() {
                    out.push(v);
                }
            }
        }
    }
    out
}

/// Dates JJ/MM/AAAA présentes dans un texte, dans l'ordre
fn dates_in(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    if b.len() < 10 {
        return out;
    }
    for i in 0..=b.len() - 10 {
        let w = &b[i..i + 10];
        let ok = w[0].is_ascii_digit() && w[1].is_ascii_digit() && w[2] == b'/'
            && w[3].is_ascii_digit() && w[4].is_ascii_digit() && w[5] == b'/'
            && w[6..].iter().all(|c| c.is_ascii_digit());
        let before_ok = i == 0 || !b[i - 1].is_ascii_digit();
        let after_ok = i + 10 == b.len() || !b[i + 10].is_ascii_digit();
        if ok && before_ok && after_ok {
            out.push(String::from_utf8_lossy(w).to_string());
        }
    }
    out
}

pub fn extract_pages(doc: &Document) -> Vec<String> {
    let n = doc.get_pages().len() as u32;
    (1..=n).map(|p| doc.extract_text(&[p]).unwrap_or_default()).collect()
}

/// Découpe une chaîne concaténée de codes (ex: "CANATRVACVDRVPT") en exactement `k` codes.
/// Les codes connus (sous-codes des paramètres) sont privilégiés ; un code inconnu (2 à 6 lettres)
/// n'est admis que s'il est indispensable pour obtenir exactement `k` codes.
pub fn split_codes(s: &str, k: usize, known: &[String]) -> Option<Vec<String>> {
    split_codes_range(s, k, k, known)
}

/// Découpage en un nombre de codes compris entre `kmin` et `kmax`. Critères, dans l'ordre :
/// 1. le moins de codes inconnus ;
/// 2. à chaque position, le code paramétré le plus long (ex: "VRCRVTR" = VRCR + VTR et non VRC + RVTR) ;
/// 3. le plus petit nombre de codes.
pub fn split_codes_range(s: &str, kmin: usize, kmax: usize, known: &[String]) -> Option<Vec<String>> {
    let s = s.trim();
    let n = s.len();
    let k = kmax;
    if kmin == 0 || kmin > kmax || n == 0 || !s.is_ascii() {
        return None;
    }
    const INF: (u32, u32) = (u32::MAX / 2, u32::MAX / 2);
    // dp[i][j] = (codes inconnus, codes non maximaux) pour découper s[..i] en j codes
    let mut dp = vec![vec![INF; k + 1]; n + 1];
    let mut prev: Vec<Vec<Option<usize>>> = vec![vec![None; k + 1]; n + 1];
    dp[0][0] = (0, 0);
    for i in 0..n {
        let longest = known
            .iter()
            .filter(|c| !c.is_empty() && s[i..].starts_with(c.as_str()))
            .map(|c| c.len())
            .max()
            .unwrap_or(0);
        for j in 0..k {
            let (unk, pen) = dp[i][j];
            if unk >= INF.0 {
                continue;
            }
            for c in known {
                if !c.is_empty() && s[i..].starts_with(c.as_str()) {
                    let ni = i + c.len();
                    let cost = (unk, pen + if c.len() < longest { 1 } else { 0 });
                    if cost < dp[ni][j + 1] {
                        dp[ni][j + 1] = cost;
                        prev[ni][j + 1] = Some(i);
                    }
                }
            }
            for len in 2..=6 {
                let ni = i + len;
                if ni > n {
                    break;
                }
                let cost = (unk + 1, pen);
                if cost < dp[ni][j + 1] {
                    dp[ni][j + 1] = cost;
                    prev[ni][j + 1] = Some(i);
                }
            }
        }
    }
    let best_k = (kmin..=kmax).filter(|j| dp[n][*j].0 < INF.0).min_by_key(|j| (dp[n][*j], *j))?;
    let mut out = Vec::with_capacity(best_k);
    let (mut i, mut j) = (n, best_k);
    while j > 0 {
        let pi = prev[i][j]?;
        out.push(s[pi..i].to_string());
        i = pi;
        j -= 1;
    }
    out.reverse();
    Some(out)
}

/// Code ministériel entre parenthèses en fin de libellé : "AUTOMOBILE ENTREPRISE ( 3.1.0.2)" -> "3.1.0.2"
fn code_ministeriel_from_label(label: &str) -> Option<String> {
    let open = label.rfind('(')?;
    let close = label[open..].find(')')? + open;
    let inner = label[open + 1..close].trim();
    if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit() || c == '.') {
        Some(inner.to_string())
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn finalize_result(
    id_prefix: &str,
    file_name: &str,
    file_type: &str,
    client: Option<String>,
    code: Option<String>,
    periode: Option<String>,
    items: Vec<ExtractedItem>,
    anomalies: Vec<Anomalie>,
) -> UploadedFileResult {
    let total = round2(items.iter().map(|i| i.prime_nette).sum());
    UploadedFileResult {
        id: format!("{}-{}", id_prefix, chrono::Local::now().timestamp_micros()),
        file_name: file_name.to_string(),
        file_type: file_type.to_string(),
        courtier_ou_client: client,
        code,
        periode,
        total_prime_nette: total,
        items,
        anomalies,
        anomalies_contexte: Vec::new(),
        hash: String::new(),
        taille: 0,
        exercice: None,
        mois: None,
    }
}

// ---------------------------------------------------------------------------------------------
// Rapport CIRIS « Statistiques de la Production Par Garantie »
// ---------------------------------------------------------------------------------------------

pub struct CirisPdfParser;

struct CirisBranche {
    code: String,
    libelle: String,
    code_ministeriel: Option<String>,
}

impl CirisPdfParser {
    pub fn is_ciris(pages: &[String]) -> bool {
        pages.iter().any(|p| p.contains("Production Par Garantie") || p.contains("Total par branche"))
    }

    #[allow(dead_code)]
    pub fn parse_from_bytes(bytes: &[u8], file_name: &str, known_codes: &[String]) -> Result<UploadedFileResult, String> {
        let doc = Document::load_mem(bytes).map_err(|e| format!("Erreur lecture PDF: {:?}", e))?;
        let pages = extract_pages(&doc);
        Self::parse_pages(&pages, file_name, known_codes)
    }

    pub fn parse_pages(pages: &[String], file_name: &str, known_codes: &[String]) -> Result<UploadedFileResult, String> {
        let mut anomalies: Vec<Anomalie> = Vec::new();

        // --- Métadonnées ---
        let mut agence_code = String::new();
        let mut agence_nom = String::new();
        let mut periode_debut = String::new();
        let mut periode_fin = String::new();
        let mut totaux_rapport: Vec<(String, f64)> = Vec::new();

        for text in pages {
            for line in text.lines() {
                let l = line.trim();
                if agence_code.is_empty() {
                    if let Some(idx) = l.find("Agence") {
                        let after = l[idx + "Agence".len()..].trim_start().trim_start_matches(':').trim();
                        let cut = ["Dépendances", "Devise", "Total", "Rapport"]
                            .iter()
                            .filter_map(|m| after.find(m))
                            .min()
                            .unwrap_or(after.len());
                        let clean = after[..cut].trim();
                        let mut words = clean.split_whitespace();
                        if let Some(first) = words.next() {
                            let code: String = first.chars().filter(|c| c.is_alphanumeric()).collect();
                            if !code.is_empty() && code.chars().any(|c| c.is_ascii_digit()) {
                                agence_code = code;
                                agence_nom = words
                                    .collect::<Vec<_>>()
                                    .join(" ")
                                    .trim_start_matches('-')
                                    .trim()
                                    .to_string();
                            }
                        }
                    }
                }
                if periode_debut.is_empty() && l.contains("mission") {
                    let d = dates_in(l);
                    if d.len() >= 2 {
                        periode_debut = d[0].clone();
                        periode_fin = d[1].clone();
                    }
                }
                for label in ["Total par agence", "Total en devise locale", "Total par devise"] {
                    if let Some(idx) = l.find(label) {
                        if let Some(v) = en_amounts(&l[idx + label.len()..]).first() {
                            totaux_rapport.push((label.to_string(), *v));
                        }
                    }
                }
            }
        }

        // --- Garanties, page par page ---
        let mut items: Vec<ExtractedItem> = Vec::new();
        let mut total_lu = 0.0;

        for (pidx, text) in pages.iter().enumerate() {
            let page_no = pidx + 1;
            let lines: Vec<&str> = text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();

            // Branches de la page, dans l'ordre
            let mut branches: Vec<CirisBranche> = Vec::new();
            for line in &lines {
                let mut rest: &str = line;
                while let Some(idx) = rest.find("Branche") {
                    let after = rest[idx + "Branche".len()..].trim_start();
                    if !after.starts_with(':') {
                        rest = &rest[idx + "Branche".len()..];
                        continue;
                    }
                    let after = after[1..].trim_start();
                    let end = after.find("Branche").unwrap_or(after.len());
                    let seg = after[..end].trim();
                    let mut w = seg.split_whitespace();
                    if let Some(code) = w.next() {
                        let libelle = w.collect::<Vec<_>>().join(" ");
                        branches.push(CirisBranche {
                            code: code.to_string(),
                            code_ministeriel: code_ministeriel_from_label(&libelle),
                            libelle,
                        });
                    }
                    rest = &after[end..];
                }
            }

            // Totaux par branche (N premiers montants = primes nettes)
            let mut totaux: Vec<f64> = Vec::new();
            for line in &lines {
                if let Some(idx) = line.find("Total par branche") {
                    totaux = en_amounts(&line[idx..]).into_iter().take(branches.len()).collect();
                    break;
                }
            }

            // Couples (ligne de codes concaténés, ligne de montants)
            let mut page_items: Vec<(String, f64, Option<f64>)> = Vec::new();
            for i in 0..lines.len().saturating_sub(1) {
                let codes_line = lines[i];
                let is_codes = codes_line.len() >= 2 && codes_line.chars().all(|c| c.is_ascii_uppercase());
                if !is_codes {
                    continue;
                }
                let nums = en_amounts(lines[i + 1]);
                if nums.len() < 2 {
                    continue;
                }
                // La ligne contient les primes nettes (une par garantie) suivies des primes TTC ;
                // le rapport omet parfois des TTC nulles : le nombre de garanties k vérifie k <= n <= 2k.
                // Les primes nettes sont ensuite validées par les totaux de branche.
                let n = nums.len();
                match split_codes_range(codes_line, n.div_ceil(2), n, known_codes) {
                    Some(codes) => {
                        let k = codes.len();
                        let ttc_complet = n == 2 * k;
                        if !ttc_complet {
                            anomalies.push(Anomalie::info(format!(
                                "Page {} : primes TTC incomplètes dans le rapport ({} montants pour {} garanties) — seules les primes nettes sont utilisées.",
                                page_no, n, k
                            )));
                        }
                        for (j, c) in codes.into_iter().enumerate() {
                            page_items.push((c, nums[j], if ttc_complet { Some(nums[k + j]) } else { None }));
                        }
                    }
                    None => anomalies.push(Anomalie::erreur(format!(
                        "Page {} : impossible de découper « {} » en codes garanties cohérents avec les {} montants lus. Vérifiez les sous-codes dans les paramètres.",
                        page_no, codes_line, n
                    ))),
                }
            }

            if page_items.is_empty() {
                continue;
            }
            if branches.is_empty() {
                anomalies.push(Anomalie::erreur(format!(
                    "Page {} : {} garanties lues mais aucune branche identifiée.",
                    page_no,
                    page_items.len()
                )));
                continue;
            }
            if totaux.len() != branches.len() {
                anomalies.push(Anomalie::erreur(format!(
                    "Page {} : {} branches mais {} totaux par branche lus.",
                    page_no,
                    branches.len(),
                    totaux.len()
                )));
                continue;
            }

            // Répartition séquentielle des garanties dans les branches, contrôlée par les totaux
            let mut sums = vec![0.0; branches.len()];
            let mut assign: Vec<usize> = Vec::with_capacity(page_items.len());
            let mut b = 0usize;
            for (_, net, _) in page_items.iter() {
                // Branche soldée (somme = total) : une garantie non nulle passe à la branche suivante
                while *net != 0.0
                    && b + 1 < branches.len()
                    && (sums[b] - totaux[b]).abs() < TOLERANCE
                    && (sums[b] != 0.0 || totaux[b] == 0.0)
                {
                    b += 1;
                }
                sums[b] += net;
                assign.push(b);
            }
            let mut coherent = true;
            for (bi, br) in branches.iter().enumerate() {
                if (round2(sums[bi]) - totaux[bi]).abs() > TOLERANCE {
                    coherent = false;
                    anomalies.push(Anomalie::erreur(format!(
                        "Page {} : branche {} — somme des garanties {} ≠ total de branche {}.",
                        page_no,
                        br.code,
                        fmt_montant(round2(sums[bi])),
                        fmt_montant(totaux[bi])
                    )));
                }
            }
            if !coherent {
                continue;
            }

            for ((code, net, ttc), bi) in page_items.into_iter().zip(assign.into_iter()) {
                total_lu += net;
                let br = &branches[bi];
                if !known_codes.iter().any(|k| *k == code) {
                    let msg = format!(
                        "Garantie « {} » (branche {} {}) absente des paramètres{}.",
                        code,
                        br.code,
                        br.libelle,
                        if net == 0.0 { " — montant nul, ignorée" } else { " — taux à paramétrer" }
                    );
                    anomalies.push(if net == 0.0 { Anomalie::info(msg) } else { Anomalie::avertissement(msg) });
                }
                if net == 0.0 {
                    continue;
                }
                items.push(ExtractedItem {
                    fichier_source: file_name.to_string(),
                    police: None,
                    client_code: Some(agence_code.clone()),
                    client_nom: Some(agence_nom.clone()),
                    libelle_garantie: code.clone(),
                    code_garantie: code,
                    branche_code: br.code.clone(),
                    branche_libelle: br.libelle.clone(),
                    prime_nette: net,
                    prime_ttc: ttc,
                    code_ministeriel: br.code_ministeriel.clone(),
                    source_type: "GARANTIE".to_string(),
                });
            }
        }

        if items.is_empty() && !anomalies.iter().any(|a| a.est_bloquante()) {
            return Err("Aucune garantie n'a pu être extraite de ce rapport CIRIS (format non reconnu ou PDF scanné sans couche texte).".to_string());
        }

        // --- Contrôles globaux ---
        if totaux_rapport.is_empty() {
            anomalies.push(Anomalie::avertissement(
                "Total agence introuvable dans le rapport : contrôle du total impossible.",
            ));
        }
        for (label, t) in &totaux_rapport {
            if (round2(total_lu) - t).abs() > TOLERANCE {
                anomalies.push(Anomalie::erreur(format!(
                    "Total des garanties lues {} ≠ « {} » du rapport {} (écart {}).",
                    fmt_montant(round2(total_lu)),
                    label,
                    fmt_montant(*t),
                    fmt_montant(round2(t - total_lu))
                )));
            }
        }
        if agence_code.is_empty() {
            anomalies.push(Anomalie::erreur("Code agence introuvable dans le rapport CIRIS."));
        }
        if periode_debut.is_empty() {
            anomalies.push(Anomalie::erreur("Période d'émission introuvable dans le rapport CIRIS."));
        }

        let pd: Vec<&str> = periode_debut.split('/').collect();
        let pf: Vec<&str> = periode_fin.split('/').collect();
        let (mois, exercice) = if pd.len() == 3 {
            (pd[1].parse::<i32>().ok(), pd[2].parse::<i32>().ok())
        } else {
            (None, None)
        };
        if pd.len() == 3 && pf.len() == 3 && (pd[1] != pf[1] || pd[2] != pf[2]) {
            anomalies.push(Anomalie::avertissement(format!(
                "La période du rapport ({} - {}) couvre plusieurs mois.",
                periode_debut, periode_fin
            )));
        }

        let mut res = finalize_result(
            "ciris",
            file_name,
            "CIRIS_PDF",
            if agence_nom.is_empty() { None } else { Some(agence_nom) },
            if agence_code.is_empty() { None } else { Some(agence_code) },
            if periode_debut.is_empty() { None } else { Some(format!("{} - {}", periode_debut, periode_fin)) },
            items,
            anomalies,
        );
        res.mois = mois;
        res.exercice = exercice;
        Ok(res)
    }
}

// ---------------------------------------------------------------------------------------------
// PDF de flotte automobile (détail par véhicule)
// ---------------------------------------------------------------------------------------------

pub struct FlottePdfParser;

impl FlottePdfParser {
    /// Repère les garanties "CODE - Libellé" : le code est la suite de majuscules qui précède " - "
    /// et le libellé commence par une majuscule suivie d'une minuscule (exclut "EPE - SPA ...").
    /// Retourne les codes dans l'ordre et la position qui suit la dernière garantie trouvée.
    fn find_guarantees(text: &str, known: &[String], anomalies: &mut Vec<Anomalie>, page_no: usize) -> (Vec<String>, usize) {
        let bytes = text.as_bytes();
        let mut codes = Vec::new();
        let mut last_end = 0usize;
        let mut search = 0usize;
        while let Some(rel) = text[search..].find(" - ") {
            let pos = search + rel;
            search = pos + 3;
            let after = &text[pos + 3..];
            let mut ch = after.chars();
            let (c1, c2) = (ch.next(), ch.next());
            let label_ok = matches!(c1, Some(c) if c.is_uppercase())
                && matches!(c2, Some(c) if c.is_lowercase() || !c.is_ascii());
            if !label_ok {
                continue;
            }
            let mut start = pos;
            while start > 0 && bytes[start - 1].is_ascii_uppercase() {
                start -= 1;
            }
            let run = &text[start..pos];
            if run.len() < 2 {
                continue;
            }
            let code = if run.len() <= 6 {
                Some(run.to_string())
            } else {
                // Code collé à un mot en majuscules (ex: "GHARDAIAVRC") : plus long sous-code paramétré en suffixe
                known
                    .iter()
                    .filter(|k| !k.is_empty() && run.ends_with(k.as_str()))
                    .max_by_key(|k| k.len())
                    .cloned()
            };
            match code {
                Some(c) => {
                    codes.push(c);
                    last_end = pos + 3;
                }
                None => anomalies.push(Anomalie::erreur(format!(
                    "Page {} : code garantie illisible dans « {} - … » (aucun sous-code paramétré ne correspond).",
                    page_no, run
                ))),
            }
        }
        (codes, last_end)
    }

    #[allow(dead_code)]
    pub fn parse_from_bytes(bytes: &[u8], file_name: &str, known_codes: &[String]) -> Result<UploadedFileResult, String> {
        let doc = Document::load_mem(bytes).map_err(|e| format!("Erreur lecture PDF Flotte: {:?}", e))?;
        let pages = extract_pages(&doc);
        Ok(Self::parse_pages(&pages, file_name, known_codes))
    }

    pub fn parse_pages(pages: &[String], file_name: &str, known_codes: &[String]) -> UploadedFileResult {
        let mut anomalies: Vec<Anomalie> = Vec::new();
        let full = pages.join("\n");

        // --- Métadonnées ---
        let mut police = String::new();
        let mut code_min: Option<String> = None;
        if let Some(idx) = full.find("Police:").or_else(|| full.find("Police :")) {
            let rem = full[idx..].splitn(2, ':').nth(1).unwrap_or("").trim_start();
            let end = rem.find(|c| c == '(' || c == '\n').unwrap_or(rem.len());
            let compact: String = rem[..end].chars().filter(|c| !c.is_whitespace()).collect();
            let b = compact.as_bytes();
            police = if b.len() > 5 && b[..4].iter().all(|c| c.is_ascii_digit()) && b[4] == b'-' {
                compact[5..].to_string()
            } else {
                compact
            };
            if rem[end..].starts_with('(') {
                let close = rem[end..].find(')').map(|p| end + p + 1).unwrap_or(rem.len());
                code_min = code_ministeriel_from_label(&rem[end..close]);
            }
        }
        if code_min.is_none() {
            if let Some(idx) = full.find("Code Minist") {
                let rem = full[idx..].splitn(2, ':').nth(1).unwrap_or("").trim_start();
                let v: String = rem.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                if !v.is_empty() {
                    code_min = Some(v.trim_end_matches('.').to_string());
                }
            }
        }

        let mut client_nom = String::new();
        let mut client_code = String::new();
        if let Some(idx) = full.find("Assur") {
            let rem = full[idx..].splitn(2, ':').nth(1).unwrap_or("").trim_start();
            let end = rem.find("Code").unwrap_or(rem.len()).min(rem.find('\n').unwrap_or(rem.len()));
            let raw = &rem[..end];
            if let Some(open) = raw.find('(') {
                client_nom = raw[..open].trim().to_string();
                if let Some(close) = raw[open..].find(')') {
                    client_code = raw[open + 1..open + close].chars().filter(|c| !c.is_whitespace()).collect();
                }
            } else {
                client_nom = raw.trim().to_string();
            }
        }
        let periode = dates_in(&full).first().map(|d| d[6..].to_string());

        // --- Garanties page par page ---
        let mut items: Vec<ExtractedItem> = Vec::new();
        let mut total_lu = 0.0;
        let mut nb_garanties = 0usize;
        for (pidx, text) in pages.iter().enumerate() {
            let page_no = pidx + 1;
            let (codes, last_end) = Self::find_guarantees(text, known_codes, &mut anomalies, page_no);
            if codes.is_empty() {
                continue;
            }
            nb_garanties += codes.len();
            let amounts = fr_amounts(&text[last_end..]);
            if amounts.len() < codes.len() {
                anomalies.push(Anomalie::erreur(format!(
                    "Page {} : {} garanties mais seulement {} montants lus.",
                    page_no,
                    codes.len(),
                    amounts.len()
                )));
                continue;
            }
            for (code, amt) in codes.into_iter().zip(amounts.into_iter()) {
                total_lu += amt;
                if amt == 0.0 {
                    continue;
                }
                if !known_codes.iter().any(|k| *k == code) {
                    anomalies.push(Anomalie::avertissement(format!(
                        "Page {} : garantie « {} » absente des paramètres — taux à paramétrer.",
                        page_no, code
                    )));
                }
                items.push(ExtractedItem {
                    fichier_source: file_name.to_string(),
                    police: if police.is_empty() { None } else { Some(police.clone()) },
                    client_code: if client_code.is_empty() { None } else { Some(client_code.clone()) },
                    client_nom: if client_nom.is_empty() { None } else { Some(client_nom.clone()) },
                    libelle_garantie: code.clone(),
                    code_garantie: code,
                    branche_code: code_min.clone().unwrap_or_default(),
                    branche_libelle: code_min.clone().unwrap_or_default(),
                    prime_nette: amt,
                    prime_ttc: None,
                    code_ministeriel: code_min.clone(),
                    source_type: "GARANTIE".to_string(),
                });
            }
        }

        if nb_garanties > 0 {
            // Contrôle : le total recalculé doit figurer dans le récapitulatif « Prime nette » du document
            let total = round2(total_lu);
            match pages.iter().rev().find(|p| p.contains("Prime nette")) {
                Some(p) => {
                    let idx = p.rfind("Prime nette").unwrap_or(0);
                    let from = p[..idx].rfind('\n').unwrap_or(0);
                    let to = p[idx..].find('\n').map(|x| idx + x).unwrap_or(p.len());
                    let recap = fr_amounts(&p[from..to]);
                    if !recap.iter().any(|v| (v - total).abs() < TOLERANCE) {
                        anomalies.push(Anomalie::erreur(format!(
                            "Total des garanties lues {} introuvable dans le récapitulatif « Prime nette » du PDF ({}).",
                            fmt_montant(total),
                            recap.iter().map(|v| fmt_montant(*v)).collect::<Vec<_>>().join(" / ")
                        )));
                    }
                }
                None => anomalies.push(Anomalie::avertissement(
                    "Récapitulatif « Prime nette » introuvable : contrôle du total impossible.",
                )),
            }
            if police.is_empty() {
                anomalies.push(Anomalie::erreur("Numéro de police introuvable dans le PDF de flotte."));
            }
            if client_code.is_empty() {
                anomalies.push(Anomalie::avertissement("Code client (assuré) introuvable dans le PDF de flotte."));
            }
            if code_min.is_none() {
                anomalies.push(Anomalie::avertissement("Code ministériel introuvable dans le PDF de flotte."));
            }
        }

        finalize_result(
            "flotte",
            file_name,
            "FLOTTE_PDF",
            if client_nom.is_empty() { None } else { Some(client_nom) },
            if client_code.is_empty() { None } else { Some(client_code) },
            periode,
            items,
            anomalies,
        )
    }
}

// ---------------------------------------------------------------------------------------------
// Registre de production Excel
// ---------------------------------------------------------------------------------------------

pub struct ExcelProductionParser;

struct ExcelColumns {
    police: usize,
    code_client: usize,
    designation: usize,
    branche: usize,
    prime_nette: usize,
    prime_ttc: Option<usize>,
    date_emission: Option<usize>,
}

fn normalize_header(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' => 'a',
            'î' | 'ï' => 'i',
            'ô' => 'o',
            'û' | 'ù' => 'u',
            'ç' => 'c',
            _ => c,
        })
        .collect()
}

fn cell_text(c: &calamine::Data) -> String {
    c.to_string().trim().to_string()
}

fn cell_amount(c: &calamine::Data) -> Option<f64> {
    use calamine::Data;
    match c {
        Data::Float(v) => Some(*v),
        Data::Int(v) => Some(*v as f64),
        Data::String(s) => {
            let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
            if t.is_empty() {
                return None;
            }
            t.replace(',', ".").parse::<f64>().ok()
        }
        _ => None,
    }
}

fn cell_date(c: &calamine::Data) -> Option<chrono::NaiveDate> {
    use calamine::Data;
    let serial = match c {
        Data::DateTime(d) => d.as_f64(),
        Data::Float(v) => *v,
        Data::DateTimeIso(s) | Data::String(s) => {
            let s = s.trim();
            let head = &s[..s.len().min(10)];
            return chrono::NaiveDate::parse_from_str(head, "%Y-%m-%d")
                .or_else(|_| chrono::NaiveDate::parse_from_str(head, "%d/%m/%Y"))
                .ok();
        }
        _ => return None,
    };
    let base = chrono::NaiveDate::from_ymd_opt(1899, 12, 30)?;
    base.checked_add_signed(chrono::Duration::days(serial.floor() as i64))
}

impl ExcelProductionParser {
    fn open_range(bytes: &[u8]) -> Result<calamine::Range<calamine::Data>, String> {
        use calamine::{open_workbook_auto_from_rs, Reader};
        let cursor = std::io::Cursor::new(bytes.to_vec());
        let mut excel = open_workbook_auto_from_rs(cursor).map_err(|e| format!("Erreur ouverture Excel: {:?}", e))?;
        let names = excel.sheet_names().to_vec();
        let sheet = names
            .iter()
            .find(|n| n.as_str() == "RCCPDE_D")
            .or_else(|| names.first())
            .cloned()
            .ok_or_else(|| "Aucune feuille trouvée dans le classeur Excel".to_string())?;
        excel
            .worksheet_range(&sheet)
            .map_err(|e| format!("Erreur lecture feuille {}: {:?}", sheet, e))
    }

    fn detect_columns(range: &calamine::Range<calamine::Data>) -> Result<ExcelColumns, String> {
        let header = range.rows().next().ok_or_else(|| "Le classeur est vide.".to_string())?;
        let (mut police, mut code_client, mut designation, mut branche, mut prime_nette) = (None, None, None, None, None);
        let (mut prime_ttc, mut date_emission) = (None, None);
        for (idx, cell) in header.iter().enumerate() {
            let s = normalize_header(&cell.to_string());
            if s.contains("police") && police.is_none() {
                police = Some(idx);
            } else if s.contains("code") && s.contains("client") {
                code_client = Some(idx);
            } else if s.contains("designation") && designation.is_none() {
                designation = Some(idx);
            } else if s.contains("branche") && branche.is_none() {
                branche = Some(idx);
            } else if s.contains("prime") && s.contains("nette") {
                prime_nette = Some(idx);
            } else if s.contains("prime") && s.contains("ttc") {
                prime_ttc = Some(idx);
            } else if s.contains("emission") && date_emission.is_none() {
                date_emission = Some(idx);
            }
        }
        let mut missing = Vec::new();
        if police.is_none() { missing.push("Police"); }
        if code_client.is_none() { missing.push("Code Client"); }
        if designation.is_none() { missing.push("Designation Client"); }
        if branche.is_none() { missing.push("Branche"); }
        if prime_nette.is_none() { missing.push("Prime Nette"); }
        if !missing.is_empty() {
            return Err(format!(
                "Colonnes obligatoires introuvables dans l'en-tête du registre Excel : {}.",
                missing.join(", ")
            ));
        }
        Ok(ExcelColumns {
            police: police.unwrap(),
            code_client: code_client.unwrap(),
            designation: designation.unwrap(),
            branche: branche.unwrap(),
            prime_nette: prime_nette.unwrap(),
            prime_ttc,
            date_emission,
        })
    }

    pub fn scan_clients_from_bytes(bytes: &[u8], file_name: &str) -> Result<crate::models::ExcelScanResult, String> {
        let range = Self::open_range(bytes)?;
        let cols = Self::detect_columns(&range)?;
        let mut client_map: std::collections::HashMap<String, (String, usize, f64)> = std::collections::HashMap::new();
        let mut total_rows = 0;

        for row in range.rows().skip(1) {
            let code = row.get(cols.code_client).map(cell_text).unwrap_or_default();
            if code.is_empty() || code.to_lowercase().contains("total") {
                continue;
            }
            let desig = row.get(cols.designation).map(cell_text).unwrap_or_default();
            let prime = row.get(cols.prime_nette).and_then(cell_amount).unwrap_or(0.0);
            total_rows += 1;
            let entry = client_map.entry(code).or_insert((desig, 0, 0.0));
            entry.1 += 1;
            entry.2 += prime;
        }

        let mut clients: Vec<crate::models::ExcelClientSummary> = client_map
            .into_iter()
            .map(|(code, (nom, count, sum))| crate::models::ExcelClientSummary {
                code_client: code,
                designation_client: nom,
                nombre_polices: count,
                total_prime_nette: round2(sum),
            })
            .collect();
        clients.sort_by(|a, b| b.total_prime_nette.partial_cmp(&a.total_prime_nette).unwrap_or(std::cmp::Ordering::Equal));
        let multiple_clients = clients.len() > 1;

        Ok(crate::models::ExcelScanResult {
            file_name: file_name.to_string(),
            clients,
            total_lignes: total_rows,
            multiple_clients,
        })
    }

    pub fn parse_from_bytes(bytes: &[u8], file_name: &str, client_filter: Option<&str>) -> Result<UploadedFileResult, String> {
        let range = Self::open_range(bytes)?;
        let cols = Self::detect_columns(&range)?;
        let mut anomalies: Vec<Anomalie> = Vec::new();

        let filter = client_filter.map(|f| f.trim().to_lowercase()).unwrap_or_default();
        // Filtre strict sur le code client s'il correspond exactement à un code du fichier,
        // sinon recherche dans le code ou la désignation
        let exact_code_mode = !filter.is_empty()
            && range
                .rows()
                .skip(1)
                .any(|row| row.get(cols.code_client).map(|c| cell_text(c).to_lowercase() == filter).unwrap_or(false));

        let mut items = Vec::new();
        let mut clients: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
        let mut polices_vues: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        let mut date_min: Option<chrono::NaiveDate> = None;
        let mut date_max: Option<chrono::NaiveDate> = None;

        for (ridx, row) in range.rows().enumerate().skip(1) {
            let ligne_excel = ridx + 1;
            if row.iter().all(|c| cell_text(c).is_empty()) {
                continue;
            }
            let code = row.get(cols.code_client).map(cell_text).unwrap_or_default();
            let desig = row.get(cols.designation).map(cell_text).unwrap_or_default();
            let police = row.get(cols.police).map(cell_text).unwrap_or_default();

            if code.to_lowercase().contains("total") || police.to_lowercase().contains("total") {
                anomalies.push(Anomalie::info(format!("Ligne {} : ligne de total ignorée.", ligne_excel)));
                continue;
            }

            if exact_code_mode {
                if code.to_lowercase() != filter {
                    continue;
                }
            } else if !filter.is_empty()
                && !code.to_lowercase().contains(&filter)
                && !desig.to_lowercase().contains(&filter)
            {
                continue;
            }

            let branche = row.get(cols.branche).map(cell_text).unwrap_or_default().to_uppercase();
            let prime = match row.get(cols.prime_nette).and_then(cell_amount) {
                Some(p) => p,
                None => {
                    anomalies.push(Anomalie::erreur(format!(
                        "Ligne {} (police {}) : prime nette absente ou non numérique.",
                        ligne_excel, police
                    )));
                    continue;
                }
            };
            if branche.is_empty() {
                anomalies.push(Anomalie::erreur(format!(
                    "Ligne {} (police {}) : branche absente, prime {} non affectable.",
                    ligne_excel,
                    police,
                    fmt_montant(prime)
                )));
                continue;
            }
            if code.is_empty() {
                anomalies.push(Anomalie::avertissement(format!(
                    "Ligne {} (police {}) : code client absent.",
                    ligne_excel, police
                )));
            }
            if !police.is_empty() {
                *polices_vues.entry(police.clone()).or_insert(0) += 1;
            }
            if let Some(d) = cols.date_emission.and_then(|i| row.get(i)).and_then(cell_date) {
                date_min = Some(date_min.map_or(d, |m| m.min(d)));
                date_max = Some(date_max.map_or(d, |m| m.max(d)));
            }
            if prime == 0.0 {
                continue;
            }
            clients.entry(code.clone()).or_insert(desig.clone());
            let prime_ttc = cols.prime_ttc.and_then(|i| row.get(i)).and_then(cell_amount);

            items.push(ExtractedItem {
                fichier_source: file_name.to_string(),
                police: if police.is_empty() { None } else { Some(police) },
                client_code: if code.is_empty() { None } else { Some(code) },
                client_nom: if desig.is_empty() { None } else { Some(desig) },
                libelle_garantie: format!("Branche {}", branche),
                code_garantie: branche.clone(),
                branche_libelle: branche.clone(),
                branche_code: branche,
                prime_nette: prime,
                prime_ttc,
                code_ministeriel: None,
                source_type: "BRANCHE_IMPORT".to_string(),
            });
        }

        for (p, n) in polices_vues.iter().filter(|(_, n)| **n > 1) {
            anomalies.push(Anomalie::avertissement(format!("Police {} présente {} fois dans le registre.", p, n)));
        }
        if clients.len() > 1 {
            anomalies.push(Anomalie::avertissement(format!(
                "{} clients différents retenus dans ce fichier{}.",
                clients.len(),
                if filter.is_empty() { " (aucun filtre client appliqué)" } else { "" }
            )));
        }
        if items.is_empty() {
            anomalies.push(Anomalie::erreur(if filter.is_empty() {
                "Aucune ligne de production exploitable dans le registre.".to_string()
            } else {
                format!("Aucune ligne ne correspond au filtre client « {} ».", filter)
            }));
        }

        let valid_codes: Vec<String> = {
            let mut set = std::collections::BTreeSet::new();
            for c in clients.keys() {
                let trimmed = c.trim();
                if !trimmed.is_empty() {
                    set.insert(trimmed.to_string());
                }
            }
            set.into_iter().collect()
        };
        let valid_noms: Vec<String> = {
            let mut set = std::collections::BTreeSet::new();
            for n in clients.values() {
                let trimmed = n.trim();
                if !trimmed.is_empty() {
                    set.insert(trimmed.to_string());
                }
            }
            set.into_iter().collect()
        };

        let client_code = if valid_codes.len() == 1 {
            Some(valid_codes[0].clone())
        } else {
            None
        };

        let client_nom = if valid_noms.len() == 1 {
            Some(valid_noms[0].clone())
        } else {
            None
        };
        let periode = match (date_min, date_max) {
            (Some(a), Some(b)) => Some(format!("{} - {}", a.format("%d/%m/%Y"), b.format("%d/%m/%Y"))),
            _ => None,
        };
        let mut res = finalize_result("excel", file_name, "EXCEL_PRODUCTION", client_nom, client_code, periode, items, anomalies);
        if let (Some(a), Some(b)) = (date_min, date_max) {
            use chrono::Datelike;
            if a.year() == b.year() {
                res.exercice = Some(a.year());
            }
        }
        Ok(res)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Sous-codes des paramètres par défaut (initial_rates.json), tels que chargés depuis la BDD
    pub fn known() -> Vec<String> {
        let json = include_str!("../../initial_rates.json");
        let v: Vec<serde_json::Value> = serde_json::from_str(json).unwrap();
        v.iter()
            .filter_map(|x| x.get("sous_code").and_then(|s| s.as_str()).map(|s| s.trim().to_uppercase()))
            .filter(|s| !s.is_empty())
            .collect()
    }

    #[test]
    fn test_split_codes() {
        let k = known();
        assert_eq!(
            split_codes("CANATRVACVDRVPTVRCVRCE", 6, &k).unwrap(),
            vec!["CANAT", "RVAC", "VDR", "VPT", "VRC", "VRCE"]
        );
        // VVI n'est pas paramétré : accepté comme code inconnu car indispensable au découpage en 14 codes
        let s = split_codes("VASSVBDGVDCVDRVINVVOLVVPTVRCVRCRVTRVVAVVIVDRVRC", 14, &k).unwrap();
        assert_eq!(s.len(), 14);
        assert_eq!(s[11], "VVI");
        assert!(split_codes("VRC", 3, &k).is_none());
        // Ambiguïté VRC+RVTR / VRCR+VTR : le code le plus long l'emporte
        assert_eq!(split_codes("VRCRVTR", 2, &k).unwrap(), vec!["VRCR", "VTR"]);
    }

    #[test]
    fn test_amount_parsers() {
        assert_eq!(parse_en_amount("14,022.17Catastrophe"), Some(14022.17));
        assert_eq!(parse_en_amount("2,272,025.22"), Some(2272025.22));
        assert_eq!(parse_en_amount("01/05/2025"), None);
        assert_eq!(fr_amounts("VPT - Personnes 1.412,06 400,00 48,00"), vec![1412.06, 400.0, 48.0]);
    }

    #[test]
    fn test_parse_real_ciris_pdf() {
        let pdf_path = std::path::Path::new("../Import/05-2025.PDF");
        if !pdf_path.exists() {
            return;
        }
        let bytes = std::fs::read(pdf_path).unwrap();
        let res = CirisPdfParser::parse_from_bytes(&bytes, "05-2025.PDF", &known()).expect("parse");
        for a in &res.anomalies {
            println!("{:?}", a);
        }
        assert!(!res.anomalies.iter().any(|a| a.est_bloquante()), "aucune anomalie bloquante attendue");
        assert_eq!(res.code.as_deref(), Some("492"));
        assert_eq!(res.periode.as_deref(), Some("01/05/2025 - 31/05/2025"));
        assert_eq!(res.mois, Some(5));
        assert_eq!(res.exercice, Some(2025));
        assert_eq!(res.items.len(), 17);
        assert_eq!(res.total_prime_nette, 2272025.22);
        assert!(res.items.iter().any(|i| i.code_garantie == "VRC" && i.branche_code == "VR" && i.prime_nette == 12522.0));
        assert!(res.items.iter().any(|i| i.code_garantie == "VRCR" && i.prime_nette == 3733.59));
        assert!(res.items.iter().any(|i| i.code_garantie == "VTR" && i.prime_nette == 12301.21));
        assert!(!res.items.iter().any(|i| i.code_garantie == "RVTR"));
        assert!(res.items.iter().any(|i| i.branche_code == "VF" && i.code_ministeriel.as_deref() == Some("3.1.0.2")));
    }

    #[test]
    fn test_ciris_total_mismatch_is_reported() {
        let bytes = match std::fs::read("../Import/05-2025.PDF") {
            Ok(b) => b,
            Err(_) => return,
        };
        let doc = Document::load_mem(&bytes).unwrap();
        let mut pages = extract_pages(&doc);
        let altered = pages[2].replace("Total par agence 2,272,025.22", "Total par agence 2,272,125.22");
        assert_ne!(altered, pages[2]);
        pages[2] = altered;
        let res = CirisPdfParser::parse_pages(&pages, "altere.pdf", &known()).unwrap();
        assert!(res.anomalies.iter().any(|a| a.est_bloquante() && a.message.contains("Total par agence")));
    }

    #[test]
    fn test_parse_real_flotte_pdfs() {
        for (f, total) in [("../Import/RCCDVF.pdf", 764317.19), ("../Import/RCCDVF01.pdf", 128648.76)] {
            let path = std::path::Path::new(f);
            if !path.exists() {
                continue;
            }
            let bytes = std::fs::read(path).unwrap();
            let res = FlottePdfParser::parse_from_bytes(&bytes, f, &known()).unwrap();
            for a in &res.anomalies {
                println!("{} {:?}", f, a);
            }
            assert!(!res.anomalies.iter().any(|a| a.est_bloquante()), "{}", f);
            assert_eq!(res.total_prime_nette, total, "{}", f);
            assert!(res.items.iter().all(|i| i.police.is_some() && i.code_ministeriel.as_deref() == Some("3.1.0.2")));
            assert_eq!(res.code.as_deref(), Some("347024203/0"));
        }
    }

    #[test]
    fn test_parse_real_excel() {
        let path = std::path::Path::new("../Import/Divers.xlsx");
        if !path.exists() {
            return;
        }
        let bytes = std::fs::read(path).unwrap();
        let scan = ExcelProductionParser::scan_clients_from_bytes(&bytes, "Divers.xlsx").unwrap();
        assert!(scan.multiple_clients);
        let res = ExcelProductionParser::parse_from_bytes(&bytes, "Divers.xlsx", Some("SPA FSP")).unwrap();
        println!("FILTER SPA FSP -> code: {:?}, courtier_ou_client: {:?}", res.code, res.courtier_ou_client);
        // 12 lignes SPA FSP dont 2 à prime nulle (ignorées)
        assert_eq!(res.items.len(), 10);
        assert!(res.items.iter().all(|i| i.source_type == "BRANCHE_IMPORT"));
        assert_eq!(res.exercice, Some(2025));
        let exact = ExcelProductionParser::parse_from_bytes(&bytes, "Divers.xlsx", Some("347024203/0")).unwrap();
        println!("FILTER 347024203/0 -> code: {:?}, courtier_ou_client: {:?}", exact.code, exact.courtier_ou_client);
        assert!(!exact.items.is_empty());
        assert!(exact.items.iter().all(|i| i.client_code.as_deref() == Some("347024203/0")));
    }
}

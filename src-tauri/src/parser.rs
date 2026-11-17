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

/// Montants au format français "1.234,56" ou "-1.234,56" (PDF de flotte).
/// Gère les montants négatifs (ristournes), les nombres collés (ex: "-645,94-95,34"),
/// les tirets Unicode (U+2212, U+2013), et le texte collé avant/après.
fn fr_amounts(text: &str) -> Vec<f64> {
    let s = text
        .replace('\u{2212}', "-")
        .replace('\u{2013}', "-")
        .replace('\u{2014}', "-")
        .replace('\u{00A0}', " ");
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut out = Vec::new();
    let mut i = 0;

    while i < n {
        if bytes[i] == b',' {
            let c_idx = i;
            // Vérifier qu'il y a exactement 2 chiffres décimaux après la virgule
            if c_idx + 2 < n && bytes[c_idx + 1].is_ascii_digit() && bytes[c_idx + 2].is_ascii_digit() {
                let c3_is_digit = c_idx + 3 < n && bytes[c_idx + 3].is_ascii_digit();
                if !c3_is_digit {
                    // Remonter vers l'arrière pour trouver le début du nombre
                    let mut start = c_idx;
                    while start > 0 {
                        let prev = bytes[start - 1];
                        if prev.is_ascii_digit() {
                            start -= 1;
                        } else if prev == b'.' && start >= 2 && bytes[start - 2].is_ascii_digit() {
                            start -= 1;
                        } else if prev == b'-' {
                            start -= 1;
                            break;
                        } else {
                            break;
                        }
                    }

                    // Nettoyer les points orphelins au début
                    while start < c_idx && bytes[start] == b'.' {
                        start += 1;
                    }
                    if bytes[start] == b'-' && (start + 1 >= c_idx || !bytes[start + 1].is_ascii_digit()) {
                        start += 1;
                    }

                    if start < c_idx && bytes[start..c_idx].iter().any(|b| b.is_ascii_digit()) {
                        let end = c_idx + 3;
                        let slice = &s[start..end];
                        // Remplacer les séparateurs de milliers '.' et mettre le point décimal
                        let integer_part = &slice[..slice.len() - 3].replace('.', "");
                        let decimal_part = &slice[slice.len() - 2..];
                        let norm = format!("{}.{}", integer_part, decimal_part);
                        if let Ok(v) = norm.parse::<f64>() {
                            out.push(v);
                            i = end;
                            continue;
                        }
                    }
                }
            }
        }
        i += 1;
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

/// Multiplie deux matrices affines 2D `[a, b, c, d, e, f]` (convention PDF : vecteur ligne `[x y 1] * M1 * M2`).
fn mul_mat(m1: [f32; 6], m2: [f32; 6]) -> [f32; 6] {
    [
        m1[0] * m2[0] + m1[1] * m2[2],
        m1[0] * m2[1] + m1[1] * m2[3],
        m1[2] * m2[0] + m1[3] * m2[2],
        m1[2] * m2[1] + m1[3] * m2[3],
        m1[4] * m2[0] + m1[5] * m2[2] + m2[4],
        m1[4] * m2[1] + m1[5] * m2[3] + m2[5],
    ]
}

fn mat_from_operands(ops: &[lopdf::Object]) -> Option<[f32; 6]> {
    if ops.len() < 6 {
        return None;
    }
    Some([
        ops[0].as_float().ok()?,
        ops[1].as_float().ok()?,
        ops[2].as_float().ok()?,
        ops[3].as_float().ok()?,
        ops[4].as_float().ok()?,
        ops[5].as_float().ok()?,
    ])
}

/// Extrait les cellules textuelles d'une page PDF avec leurs coordonnées `(x, y)`, regroupées par ligne visuelle.
/// Évite la concaténation de colonnes produite par `Document::extract_text` lorsque les cellules d'un tableau
/// sont positionnées par `Td`/`TD`/`Tm` dans un même bloc `BT ... ET`.
fn extract_page_rows(doc: &Document, page_number: u32) -> Option<Vec<Vec<(f32, String)>>> {
    use lopdf::content::Content;
    use lopdf::{Encoding, Object};
    use std::collections::BTreeMap;

    fn append_text(out: &mut String, encoding: Option<&Encoding>, operands: &[Object]) {
        for op in operands {
            match op {
                Object::String(bytes, _) => {
                    if let Some(enc) = encoding {
                        if let Ok(s) = Document::decode_text(enc, bytes) {
                            out.push_str(&s);
                        }
                    } else {
                        out.push_str(&String::from_utf8_lossy(bytes));
                    }
                }
                Object::Array(arr) => append_text(out, encoding, arr),
                Object::Integer(i) if *i < -100 => out.push(' '),
                Object::Real(r) if *r < -100.0 => out.push(' '),
                _ => {}
            }
        }
    }

    let pages = doc.get_pages();
    let page_id = *pages.get(&page_number)?;
    let fonts = doc.get_page_fonts(page_id).ok()?;
    let encodings: BTreeMap<Vec<u8>, Encoding> = fonts
        .into_iter()
        .filter_map(|(name, font)| font.get_font_encoding(doc).ok().map(|e| (name, e)))
        .collect();
    let content_data = doc.get_page_content(page_id).ok()?;
    let content = Content::decode(&content_data).ok()?;

    let ident = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut ctm = ident;
    let mut ctm_stack: Vec<[f32; 6]> = Vec::new();
    let mut tm = ident;
    let mut tlm = ident;
    let mut tl = 0.0f32;
    let mut current_encoding: Option<&Encoding> = None;

    let mut cur_text = String::new();
    let mut cur_pos: Option<(f32, f32)> = None;
    let mut tokens: Vec<(f32, f32, String)> = Vec::new();

    let flush = |cur_text: &mut String, cur_pos: &mut Option<(f32, f32)>, tokens: &mut Vec<(f32, f32, String)>| {
        if let Some((x, y)) = cur_pos.take() {
            let trimmed = cur_text.trim();
            if !trimmed.is_empty() {
                tokens.push((x, y, trimmed.to_string()));
            }
        }
        cur_text.clear();
    };

    for op in &content.operations {
        match op.operator.as_ref() {
            "q" => ctm_stack.push(ctm),
            "Q" => ctm = ctm_stack.pop().unwrap_or(ident),
            "cm" => {
                if let Some(m) = mat_from_operands(&op.operands) {
                    ctm = mul_mat(m, ctm);
                }
            }
            "BT" => {
                flush(&mut cur_text, &mut cur_pos, &mut tokens);
                tm = ident;
                tlm = ident;
            }
            "ET" => flush(&mut cur_text, &mut cur_pos, &mut tokens),
            "Tf" => {
                if let Some(name) = op.operands.first().and_then(|o| o.as_name().ok()) {
                    current_encoding = encodings.get(name);
                }
            }
            "TL" => {
                if let Some(Ok(v)) = op.operands.first().map(|o| o.as_float()) {
                    tl = v;
                }
            }
            "Tm" => {
                flush(&mut cur_text, &mut cur_pos, &mut tokens);
                if let Some(m) = mat_from_operands(&op.operands) {
                    tm = m;
                    tlm = m;
                }
            }
            "Td" | "TD" => {
                flush(&mut cur_text, &mut cur_pos, &mut tokens);
                if op.operands.len() >= 2 {
                    if let (Ok(tx), Ok(ty)) = (op.operands[0].as_float(), op.operands[1].as_float()) {
                        if op.operator == "TD" {
                            tl = -ty;
                        }
                        tlm = mul_mat([1.0, 0.0, 0.0, 1.0, tx, ty], tlm);
                        tm = tlm;
                    }
                }
            }
            "T*" => {
                flush(&mut cur_text, &mut cur_pos, &mut tokens);
                tlm = mul_mat([1.0, 0.0, 0.0, 1.0, 0.0, -tl], tlm);
                tm = tlm;
            }
            "'" | "\"" => {
                flush(&mut cur_text, &mut cur_pos, &mut tokens);
                tlm = mul_mat([1.0, 0.0, 0.0, 1.0, 0.0, -tl], tlm);
                tm = tlm;
                let trm = mul_mat(tm, ctm);
                cur_pos = Some((trm[4], trm[5]));
                let str_ops = if op.operator == "\"" && op.operands.len() >= 3 {
                    &op.operands[2..]
                } else {
                    &op.operands[..]
                };
                append_text(&mut cur_text, current_encoding, str_ops);
            }
            "Tj" | "TJ" => {
                if cur_pos.is_none() {
                    let trm = mul_mat(tm, ctm);
                    cur_pos = Some((trm[4], trm[5]));
                }
                append_text(&mut cur_text, current_encoding, &op.operands);
            }
            _ => {}
        }
    }
    flush(&mut cur_text, &mut cur_pos, &mut tokens);

    // Tri de haut en bas (y décroissant), puis de gauche à droite (x croissant)
    tokens.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.total_cmp(&b.0)));

    let mut rows: Vec<(f32, Vec<(f32, String)>)> = Vec::new();
    for (x, y, text) in tokens {
        if let Some((ry, cols)) = rows.last_mut() {
            if (*ry - y).abs() <= 2.5 {
                cols.push((x, text));
                continue;
            }
        }
        rows.push((y, vec![(x, text)]));
    }
    for (_, cols) in &mut rows {
        cols.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    Some(rows.into_iter().map(|(_, cols)| cols).collect())
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
        point_vente: None,
        clients_selectionnes: Vec::new(),
        selection: None,
    }
}

// ---------------------------------------------------------------------------------------------
// Rapport CIRIS « Statistiques de la Production Par Garantie »
// ---------------------------------------------------------------------------------------------

pub struct CirisPdfParser;

#[derive(Clone)]
struct CirisBranche {
    code: String,
    libelle: String,
    code_ministeriel: Option<String>,
}

#[derive(Default)]
struct CirisMeta {
    agence_code: String,
    agence_nom: String,
    periode_debut: String,
    periode_fin: String,
    totaux_rapport: Vec<(String, f64)>,
}

impl CirisMeta {
    fn scan_line(&mut self, line: &str) {
        let l = line.trim();
        if self.agence_code.is_empty() {
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
                        self.agence_code = code;
                        self.agence_nom = words
                            .collect::<Vec<_>>()
                            .join(" ")
                            .trim_start_matches('-')
                            .trim()
                            .to_string();
                    }
                }
            }
        }
        if l.contains("mission") {
            let d = dates_in(l);
            if d.len() >= 2 && self.periode_debut.is_empty() {
                self.periode_debut = d[0].clone();
                self.periode_fin = d[1].clone();
            } else if d.len() == 1 {
                if self.periode_debut.is_empty() && l.contains("De la") {
                    self.periode_debut = d[0].clone();
                } else if self.periode_fin.is_empty() && l.contains("A la") {
                    self.periode_fin = d[0].clone();
                }
            }
        }
        for label in ["Total par agence", "Total en devise locale", "Total par devise"] {
            if let Some(idx) = l.find(label) {
                if let Some(v) = en_amounts(&l[idx + label.len()..]).first() {
                    self.totaux_rapport.push((label.to_string(), *v));
                }
            }
        }
    }
}

fn parse_ciris_branche_headers(line: &str) -> Vec<CirisBranche> {
    let mut branches = Vec::new();
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
    branches
}

fn push_ciris_item(
    file_name: &str,
    meta: &CirisMeta,
    br: &CirisBranche,
    code: String,
    net: f64,
    ttc: Option<f64>,
    known_codes: &[String],
    items: &mut Vec<ExtractedItem>,
    anomalies: &mut Vec<Anomalie>,
    total_lu: &mut f64,
) {
    *total_lu += net;
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
        return;
    }
    items.push(ExtractedItem {
        fichier_source: file_name.to_string(),
        police: None,
        client_code: if meta.agence_code.is_empty() { None } else { Some(meta.agence_code.clone()) },
        client_nom: if meta.agence_nom.is_empty() { None } else { Some(meta.agence_nom.clone()) },
        libelle_garantie: code.clone(),
        code_garantie: code,
        branche_code: br.code.clone(),
        branche_libelle: br.libelle.clone(),
        prime_nette: net,
        prime_ttc: ttc,
        code_ministeriel: br.code_ministeriel.clone(),
        source_type: "GARANTIE".to_string(),
        date_emission: None,
        date_effet: None,
        date_echeance: None,
        cp: None,
    });
}

fn finalize_ciris_result(
    file_name: &str,
    meta: CirisMeta,
    mut items: Vec<ExtractedItem>,
    mut anomalies: Vec<Anomalie>,
    total_lu: f64,
) -> Result<UploadedFileResult, String> {
    if items.is_empty() && !anomalies.iter().any(|a| a.est_bloquante()) {
        return Err("Aucune garantie n'a pu être extraite de ce rapport CIRIS (format non reconnu ou PDF scanné sans couche texte).".to_string());
    }

    if !meta.agence_code.is_empty() {
        for it in &mut items {
            if it.client_code.is_none() {
                it.client_code = Some(meta.agence_code.clone());
            }
            if it.client_nom.is_none() && !meta.agence_nom.is_empty() {
                it.client_nom = Some(meta.agence_nom.clone());
            }
        }
    }

    if meta.totaux_rapport.is_empty() {
        anomalies.push(Anomalie::avertissement(
            "Total agence introuvable dans le rapport : contrôle du total impossible.",
        ));
    }
    for (label, t) in &meta.totaux_rapport {
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
    if meta.agence_code.is_empty() {
        anomalies.push(Anomalie::erreur("Code agence introuvable dans le rapport CIRIS."));
    }
    if meta.periode_debut.is_empty() {
        anomalies.push(Anomalie::erreur("Période d'émission introuvable dans le rapport CIRIS."));
    }

    let pd: Vec<&str> = meta.periode_debut.split('/').collect();
    let pf: Vec<&str> = meta.periode_fin.split('/').collect();
    let (mois, exercice) = if pd.len() == 3 {
        (pd[1].parse::<i32>().ok(), pd[2].parse::<i32>().ok())
    } else {
        (None, None)
    };
    if pd.len() == 3 && pf.len() == 3 && (pd[1] != pf[1] || pd[2] != pf[2]) {
        anomalies.push(Anomalie::avertissement(format!(
            "La période du rapport ({} - {}) couvre plusieurs mois.",
            meta.periode_debut, meta.periode_fin
        )));
    }

    let mut res = finalize_result(
        "ciris",
        file_name,
        "CIRIS_PDF",
        if meta.agence_nom.is_empty() { None } else { Some(meta.agence_nom) },
        if meta.agence_code.is_empty() { None } else { Some(meta.agence_code) },
        if meta.periode_debut.is_empty() { None } else { Some(format!("{} - {}", meta.periode_debut, meta.periode_fin)) },
        items,
        anomalies,
    );
    res.mois = mois;
    res.exercice = exercice;
    Ok(res)
}

#[derive(Clone, Copy, Default)]
struct CirisCols {
    x_net: Option<f32>,
    x_ttc: Option<f32>,
    x_cpo: Option<f32>,
}

impl CirisCols {
    fn detect(rows: &[Vec<(f32, String)>]) -> Self {
        let mut cols = Self::default();
        for row in rows {
            for (x, s) in row {
                let t = s.trim();
                if cols.x_net.is_none()
                    && *x > 200.0
                    && t.starts_with("Primes Nettes")
                    && !t.contains('+')
                {
                    cols.x_net = Some(*x);
                }
            }
        }
        if let Some(xn) = cols.x_net {
            for row in rows {
                for (x, s) in row {
                    let t = s.trim();
                    if cols.x_ttc.is_none() && *x > xn && t.contains("TTC") {
                        cols.x_ttc = Some(*x);
                    }
                    if cols.x_cpo.is_none() && *x > xn && (t.contains("CPO") || t.contains("Primes Nettes +")) {
                        cols.x_cpo = Some(*x);
                    }
                }
            }
        }
        cols
    }

    fn amount_in(&self, row: &[(f32, String)], min_x: f32, max_x: f32) -> Option<f64> {
        let joined: String = row
            .iter()
            .filter(|(x, _)| *x >= min_x && *x < max_x)
            .map(|(_, s)| s.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        en_amounts(&joined).into_iter().next()
    }

    fn row_amounts(&self, row: &[(f32, String)]) -> (Option<f64>, Option<f64>) {
        let Some(xn) = self.x_net else {
            return (None, None);
        };
        let ttc_bound = self.x_ttc.map(|xt| xt - 15.0).unwrap_or(f32::INFINITY);
        let net = self.amount_in(row, xn - 60.0, ttc_bound);
        let ttc = self.x_ttc.and_then(|xt| {
            let cpo_bound = self.x_cpo.map(|xc| xc - 15.0).unwrap_or(xt + 75.0);
            self.amount_in(row, xt - 15.0, cpo_bound)
        });
        (net, ttc)
    }
}

fn flush_ciris_branch(
    page_num: u32,
    file_name: &str,
    meta: &CirisMeta,
    current_branche: &mut Option<CirisBranche>,
    branch_items: &mut Vec<(String, f64, Option<f64>)>,
    total_branche: Option<f64>,
    known_codes: &[String],
    items: &mut Vec<ExtractedItem>,
    anomalies: &mut Vec<Anomalie>,
    total_lu: &mut f64,
) {
    let buffered = std::mem::take(branch_items);
    let Some(br) = current_branche.take() else {
        return;
    };
    let sum_net: f64 = buffered.iter().map(|(_, net, _)| *net).sum();
    if let Some(tot) = total_branche {
        if (round2(sum_net) - tot).abs() > TOLERANCE {
            anomalies.push(Anomalie::erreur(format!(
                "Page {} : branche {} — somme des garanties {} ≠ total de branche {}.",
                page_num,
                br.code,
                fmt_montant(round2(sum_net)),
                fmt_montant(tot)
            )));
            return;
        }
    }
    for (code, net, ttc) in buffered {
        push_ciris_item(file_name, meta, &br, code, net, ttc, known_codes, items, anomalies, total_lu);
    }
}

impl CirisPdfParser {
    pub fn is_ciris(pages: &[String]) -> bool {
        pages.iter().any(|p| p.contains("Production Par Garantie") || p.contains("Total par branche"))
    }

    #[allow(dead_code)]
    pub fn parse_from_bytes(bytes: &[u8], file_name: &str, known_codes: &[String]) -> Result<UploadedFileResult, String> {
        let doc = Document::load_mem(bytes).map_err(|e| format!("Erreur lecture PDF: {:?}", e))?;
        Self::parse_document(&doc, file_name, known_codes)
    }

    /// Extrait les tableaux CIRIS via les coordonnées réelles `(x, y)` de chaque cellule,
    /// évitant la concaténation de codes de garanties produite par `extract_text`.
    pub fn parse_document(doc: &Document, file_name: &str, known_codes: &[String]) -> Result<UploadedFileResult, String> {
        let mut anomalies: Vec<Anomalie> = Vec::new();
        let mut meta = CirisMeta::default();
        let mut items: Vec<ExtractedItem> = Vec::new();
        let mut total_lu = 0.0;

        let mut page_nums: Vec<u32> = doc.get_pages().keys().copied().collect();
        page_nums.sort_unstable();

        let mut current_branche: Option<CirisBranche> = None;
        let mut branch_items: Vec<(String, f64, Option<f64>)> = Vec::new();
        let mut found_table = false;

        for page_num in page_nums {
            let Some(rows) = extract_page_rows(doc, page_num) else {
                continue;
            };
            let cols = CirisCols::detect(&rows);

            for row in rows {
                let row_line: String = row.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join(" ");
                meta.scan_line(&row_line);

                let new_branches = parse_ciris_branche_headers(&row_line);
                if !new_branches.is_empty() {
                    if let Some(first) = new_branches.into_iter().last() {
                        let same_open = current_branche.as_ref().map(|b| b.code == first.code).unwrap_or(false);
                        if !same_open {
                            if !branch_items.is_empty() {
                                anomalies.push(Anomalie::avertissement(format!(
                                    "Page {} : changement de branche sans « Total par branche » intermédiaire.",
                                    page_num
                                )));
                                flush_ciris_branch(
                                    page_num,
                                    file_name,
                                    &meta,
                                    &mut current_branche,
                                    &mut branch_items,
                                    None,
                                    known_codes,
                                    &mut items,
                                    &mut anomalies,
                                    &mut total_lu,
                                );
                            }
                            current_branche = Some(first);
                        }
                    }
                    continue;
                }

                if row_line.contains("Total par branche") {
                    found_table = true;
                    let total_branche = cols
                        .row_amounts(&row)
                        .0
                        .or_else(|| {
                            row_line
                                .find("Total par branche")
                                .and_then(|idx| en_amounts(&row_line[idx..]).into_iter().next())
                        });
                    flush_ciris_branch(
                        page_num,
                        file_name,
                        &meta,
                        &mut current_branche,
                        &mut branch_items,
                        total_branche,
                        known_codes,
                        &mut items,
                        &mut anomalies,
                        &mut total_lu,
                    );
                    continue;
                }

                let Some(xn) = cols.x_net else {
                    continue;
                };
                if current_branche.is_none()
                    || row_line.contains("Total par")
                    || row_line.contains("Agence")
                    || row_line.contains("Page ")
                {
                    continue;
                }
                if let Some((x0, s0)) = row.first() {
                    let code = s0.trim();
                    let is_code = *x0 < xn - 150.0
                        && code.len() >= 2
                        && code.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                        && code.chars().any(|c| c.is_ascii_uppercase());
                    if is_code {
                        let (net, ttc) = cols.row_amounts(&row);
                        if let Some(net_val) = net {
                            found_table = true;
                            branch_items.push((code.to_string(), net_val, ttc));
                        }
                    }
                }
            }
        }

        if !branch_items.is_empty() {
            anomalies.push(Anomalie::avertissement(
                "Total par branche manquant en fin de rapport CIRIS.",
            ));
            let last_page = doc.get_pages().keys().copied().max().unwrap_or(1);
            flush_ciris_branch(
                last_page,
                file_name,
                &meta,
                &mut current_branche,
                &mut branch_items,
                None,
                known_codes,
                &mut items,
                &mut anomalies,
                &mut total_lu,
            );
        }

        if !found_table && items.is_empty() && !anomalies.iter().any(|a| a.est_bloquante()) {
            let pages = extract_pages(doc);
            return Self::parse_pages(&pages, file_name, known_codes);
        }

        finalize_ciris_result(file_name, meta, items, anomalies, total_lu)
    }

    pub fn parse_pages(pages: &[String], file_name: &str, known_codes: &[String]) -> Result<UploadedFileResult, String> {
        let mut anomalies: Vec<Anomalie> = Vec::new();
        let mut meta = CirisMeta::default();

        for text in pages {
            for line in text.lines() {
                meta.scan_line(line);
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
                branches.extend(parse_ciris_branche_headers(line));
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
                push_ciris_item(
                    file_name,
                    &meta,
                    &branches[bi],
                    code,
                    net,
                    ttc,
                    known_codes,
                    &mut items,
                    &mut anomalies,
                    &mut total_lu,
                );
            }
        }

        finalize_ciris_result(file_name, meta, items, anomalies, total_lu)
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
                    date_emission: None,
                    date_effet: None,
                    date_echeance: None,
                    cp: None,
                });
            }
        }

        if nb_garanties > 0 {
            // Contrôle : le total recalculé doit figurer dans le récapitulatif « Prime nette » du document
            let total = round2(total_lu);
            match pages.iter().rev().find(|p| p.contains("Prime nette")) {
                Some(p) => {
                    let idx = p.rfind("Prime nette").unwrap_or(0);
                    let window_start = idx.saturating_sub(150);
                    let window_end = (idx + 350).min(p.len());
                    let recap = fr_amounts(&p[window_start..window_end]);
                    if !recap.iter().any(|v| (v - total).abs() < TOLERANCE) {
                        anomalies.push(Anomalie::avertissement(format!(
                            "Écart récapitulatif : total des garanties lues {} ≠ « Prime nette » du récapitulatif PDF ({}).",
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
    date_effet: Option<usize>,
    date_echeance: Option<usize>,
    cp: Option<usize>,
}

/// Feuille de données du registre : colonnes repérées et ligne d'en-tête
/// Feuille de détail du registre (un export volumineux continue sur plusieurs feuilles)
struct FeuilleDonnees {
    nom: String,
    range: calamine::Range<calamine::Data>,
    cols: ExcelColumns,
    /// Première ligne de données (après l'en-tête)
    debut: usize,
}

struct ExcelData {
    feuilles: Vec<FeuilleDonnees>,
    /// Informations de la feuille « Parametres » de l'export CIRIS (agence, période d'extraction)
    info_export: Vec<String>,
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

/// Point de vente (agence) d'une police : premier segment du numéro ("492/VR/187/0/0" -> "492")
pub fn point_de_vente(police: &str) -> String {
    police.split('/').next().unwrap_or("").trim().to_string()
}

/// Sélection faite par l'utilisateur dans un registre de production
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct ExcelSelection {
    /// Point de vente retenu (préfixe du numéro de police) ; aucun = tout le fichier
    #[serde(default)]
    pub point_vente: Option<String>,
    /// Clients retenus (codes exacts) — affaires apportées par un courtier ; vide = tout le point de vente
    #[serde(default)]
    pub clients: Vec<String>,
}

/// Période du bordereau : seules les polices émises dans cette période sont retenues
#[derive(Debug, Clone, Copy)]
pub struct PeriodeBordereau {
    pub exercice: i32,
    pub mois: Option<i32>,
}

impl PeriodeBordereau {
    fn contient(&self, d: chrono::NaiveDate) -> bool {
        use chrono::Datelike;
        d.year() == self.exercice && self.mois.map_or(true, |m| d.month() as i32 == m)
    }
    fn libelle(&self) -> String {
        match self.mois {
            Some(m) => format!("{:02}/{}", m, self.exercice),
            None => format!("exercice {}", self.exercice),
        }
    }
}

impl ExcelProductionParser {
    /// Repère l'en-tête (Police, Code Client, Désignation, Branche, Prime Nette) dans une ligne
    fn columns_from_row(row: &[calamine::Data]) -> Result<ExcelColumns, Vec<&'static str>> {
        let (mut police, mut code_client, mut designation, mut branche, mut prime_nette) = (None, None, None, None, None);
        let (mut prime_ttc, mut date_emission) = (None, None);
        let (mut date_effet, mut date_echeance, mut cp) = (None, None, None);
        for (idx, cell) in row.iter().enumerate() {
            let s = normalize_header(&cell.to_string());
            if s.trim() == "cp" && cp.is_none() {
                cp = Some(idx);
            } else if s.contains("effet") && date_effet.is_none() {
                date_effet = Some(idx);
            } else if s.contains("echean") && date_echeance.is_none() {
                date_echeance = Some(idx);
            } else if s.contains("police") && police.is_none() {
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
            return Err(missing);
        }
        Ok(ExcelColumns {
            police: police.unwrap(),
            code_client: code_client.unwrap(),
            designation: designation.unwrap(),
            branche: branche.unwrap(),
            prime_nette: prime_nette.unwrap(),
            prime_ttc,
            date_emission,
            date_effet,
            date_echeance,
            cp,
        })
    }

    /// Ouvre le classeur (.xls ou .xlsx) et retient TOUTES les feuilles de détail, dans l'ordre du classeur :
    /// l'export CIRIS place des feuilles de paramètres avant le détail (en-tête qui n'est pas en première
    /// ligne), et un export volumineux continue sur les feuilles suivantes (2, 3…), avec ou sans en-tête répété.
    fn open_data(bytes: &[u8]) -> Result<ExcelData, String> {
        use calamine::{open_workbook_auto_from_rs, Reader};
        let cursor = std::io::Cursor::new(bytes.to_vec());
        let mut excel = open_workbook_auto_from_rs(cursor).map_err(|e| format!("Erreur ouverture Excel: {:?}", e))?;
        let names = excel.sheet_names().to_vec();
        if names.is_empty() {
            return Err("Aucune feuille trouvée dans le classeur Excel".to_string());
        }

        let mut info_export = Vec::new();
        let mut feuilles: Vec<FeuilleDonnees> = Vec::new();
        let mut meilleur_manque: Option<Vec<&'static str>> = None;
        for name in &names {
            let range = match excel.worksheet_range(name) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if normalize_header(name).starts_with("parametres") {
                for row in range.rows() {
                    let label = row.first().map(cell_text).unwrap_or_default();
                    let val = row.get(1).map(|c| match c {
                        calamine::Data::Float(_) | calamine::Data::DateTime(_) if normalize_header(&label).contains("date") || label.contains("Export") => {
                            cell_date(c).map(|d| d.format("%d/%m/%Y").to_string()).unwrap_or_else(|| cell_text(c))
                        }
                        _ => cell_text(c),
                    });
                    let l = normalize_header(&label);
                    if let Some(v) = val.filter(|v| !v.is_empty()) {
                        if l.starts_with("agence") || l.contains("de la date") || l.contains("a la date") || l.starts_with("exporte") {
                            info_export.push(format!("{} {}", label.trim_end_matches(':').trim(), v));
                        }
                    }
                }
                continue;
            }

            // 1. Feuille avec son propre en-tête
            let mut trouvee = false;
            for (idx, row) in range.rows().enumerate().take(60) {
                match Self::columns_from_row(row) {
                    Ok(cols) => {
                        feuilles.push(FeuilleDonnees { nom: name.clone(), range: range.clone(), cols, debut: idx + 1 });
                        trouvee = true;
                        break;
                    }
                    Err(missing) => {
                        if missing.len() < 5 && meilleur_manque.as_ref().map_or(true, |m| missing.len() < m.len()) {
                            meilleur_manque = Some(missing);
                        }
                    }
                }
            }
            if trouvee {
                continue;
            }
            // 2. Suite d'une feuille précédente sans en-tête répété : mêmes colonnes, lignes de polices
            if let Some(prec) = feuilles.last() {
                let (ip, ib, inn) = (prec.cols.police, prec.cols.branche, prec.cols.prime_nette);
                let debut = range.rows().position(|row| {
                    let p = row.get(ip).map(cell_text).unwrap_or_default();
                    p.split('/').count() >= 3
                        && !row.get(ib).map(cell_text).unwrap_or_default().is_empty()
                        && row.get(inn).and_then(cell_amount).is_some()
                });
                if let Some(debut) = debut {
                    let cols = ExcelColumns {
                        police: prec.cols.police,
                        code_client: prec.cols.code_client,
                        designation: prec.cols.designation,
                        branche: prec.cols.branche,
                        prime_nette: prec.cols.prime_nette,
                        prime_ttc: prec.cols.prime_ttc,
                        date_emission: prec.cols.date_emission,
                        date_effet: prec.cols.date_effet,
                        date_echeance: prec.cols.date_echeance,
                        cp: prec.cols.cp,
                    };
                    feuilles.push(FeuilleDonnees { nom: name.clone(), range: range.clone(), cols, debut });
                }
            }
        }
        if feuilles.is_empty() {
            return Err(match meilleur_manque {
                Some(m) => format!("Colonnes obligatoires introuvables dans l'en-tête du registre Excel : {}.", m.join(", ")),
                None => "Aucune feuille de production (colonnes Police, Code Client, Branche, Prime Nette) trouvée dans ce classeur.".to_string(),
            });
        }
        Ok(ExcelData { feuilles, info_export })
    }

    /// Lignes de données de toutes les feuilles : (repère « feuille, ligne », colonnes de la feuille, ligne)
    fn data_rows<'a>(d: &'a ExcelData) -> impl Iterator<Item = (String, &'a ExcelColumns, &'a [calamine::Data])> {
        let plusieurs = d.feuilles.len() > 1;
        d.feuilles.iter().flat_map(move |f| {
            f.range.rows().enumerate().skip(f.debut).map(move |(i, r)| {
                let repere = if plusieurs { format!("{} ligne {}", f.nom, i + 1) } else { format!("{}", i + 1) };
                (repere, &f.cols, r)
            })
        })
    }

    /// Analyse rapide : points de vente présents (préfixe du N° de police), sans doublon
    pub fn scan_from_bytes(bytes: &[u8], file_name: &str) -> Result<crate::models::ExcelScanResult, String> {
        use std::collections::{BTreeMap, BTreeSet, HashSet};
        let d = Self::open_data(bytes)?;
        struct Pv {
            polices: usize,
            total: f64,
            clients: HashSet<String>,
            branches: BTreeSet<String>,
        }
        let mut pvs: BTreeMap<String, Pv> = BTreeMap::new();
        let mut total_lignes = 0;
        let mut date_min: Option<chrono::NaiveDate> = None;
        let mut date_max: Option<chrono::NaiveDate> = None;
        for (_, cols, row) in Self::data_rows(&d) {
            let police = row.get(cols.police).map(cell_text).unwrap_or_default();
            if police.is_empty() || police.to_lowercase().contains("total") {
                continue;
            }
            total_lignes += 1;
            let pv = pvs.entry(point_de_vente(&police)).or_insert(Pv {
                polices: 0,
                total: 0.0,
                clients: HashSet::new(),
                branches: BTreeSet::new(),
            });
            pv.polices += 1;
            pv.total += row.get(cols.prime_nette).and_then(cell_amount).unwrap_or(0.0);
            pv.clients.insert(row.get(cols.code_client).map(cell_text).unwrap_or_default());
            let br = row.get(cols.branche).map(cell_text).unwrap_or_default().to_uppercase();
            if !br.is_empty() {
                pv.branches.insert(br);
            }
            if let Some(dt) = cols.date_emission.and_then(|i| row.get(i)).and_then(cell_date) {
                date_min = Some(date_min.map_or(dt, |m| m.min(dt)));
                date_max = Some(date_max.map_or(dt, |m| m.max(dt)));
            }
        }
        let points_vente: Vec<crate::models::PointVenteSummary> = pvs
            .into_iter()
            .map(|(code, p)| crate::models::PointVenteSummary {
                code,
                nombre_polices: p.polices,
                nombre_clients: p.clients.len(),
                total_prime_nette: round2(p.total),
                branches: p.branches.into_iter().collect(),
            })
            .collect();
        Ok(crate::models::ExcelScanResult {
            staging_id: String::new(),
            file_name: file_name.to_string(),
            total_lignes,
            points_vente,
            info_export: d.info_export,
            periode_emission: match (date_min, date_max) {
                (Some(a), Some(b)) => Some(format!("{} - {}", a.format("%d/%m/%Y"), b.format("%d/%m/%Y"))),
                _ => None,
            },
        })
    }

    /// Clients d'un point de vente (un par code, polices regroupées)
    pub fn clients_from_bytes(bytes: &[u8], point_vente: &str) -> Result<Vec<crate::models::ExcelClientSummary>, String> {
        let d = Self::open_data(bytes)?;
        let mut map: std::collections::HashMap<String, (String, usize, f64)> = std::collections::HashMap::new();
        for (_, cols, row) in Self::data_rows(&d) {
            let police = row.get(cols.police).map(cell_text).unwrap_or_default();
            if police.is_empty() || point_de_vente(&police) != point_vente.trim() {
                continue;
            }
            let code = row.get(cols.code_client).map(cell_text).unwrap_or_default();
            let desig = row.get(cols.designation).map(cell_text).unwrap_or_default();
            let e = map.entry(code).or_insert((desig, 0, 0.0));
            e.1 += 1;
            e.2 += row.get(cols.prime_nette).and_then(cell_amount).unwrap_or(0.0);
        }
        let mut clients: Vec<crate::models::ExcelClientSummary> = map
            .into_iter()
            .map(|(code, (nom, n, s))| crate::models::ExcelClientSummary {
                code_client: code,
                designation_client: nom,
                nombre_polices: n,
                total_prime_nette: round2(s),
            })
            .collect();
        clients.sort_by(|a, b| b.total_prime_nette.partial_cmp(&a.total_prime_nette).unwrap_or(std::cmp::Ordering::Equal));
        Ok(clients)
    }

    pub fn parse_from_bytes(
        bytes: &[u8],
        file_name: &str,
        selection: &ExcelSelection,
        periode: Option<PeriodeBordereau>,
    ) -> Result<UploadedFileResult, String> {
        use std::collections::{BTreeMap, BTreeSet, HashSet};
        let d = Self::open_data(bytes)?;
        let mut anomalies: Vec<Anomalie> = Vec::new();

        let pv_filtre = selection.point_vente.as_deref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let clients_filtre: HashSet<String> = selection
            .clients
            .iter()
            .map(|c| c.trim().to_uppercase())
            .filter(|c| !c.is_empty())
            .collect();

        let mut items = Vec::new();
        let mut clients_trouves: BTreeMap<String, String> = BTreeMap::new();
        let mut pvs_trouves: BTreeSet<String> = BTreeSet::new();
        let mut polices_vues: BTreeMap<String, usize> = BTreeMap::new();
        let (mut date_min, mut date_max): (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) = (None, None);
        let (mut hors_periode, mut hors_periode_montant) = (0usize, 0.0);
        let (mut sans_date, mut nulles) = (0usize, 0usize);

        for (ligne_excel, cols, row) in Self::data_rows(&d) {
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
            let pv = point_de_vente(&police);
            if let Some(f) = &pv_filtre {
                if &pv != f {
                    continue;
                }
            }
            if !clients_filtre.is_empty() && !clients_filtre.contains(&code.to_uppercase()) {
                continue;
            }

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
            let date = cols.date_emission.and_then(|i| row.get(i)).and_then(cell_date);
            if let Some(p) = periode {
                match date {
                    Some(dt) if !p.contient(dt) => {
                        hors_periode += 1;
                        hors_periode_montant += prime;
                        continue;
                    }
                    None => sans_date += 1,
                    _ => {}
                }
            }
            let branche = row.get(cols.branche).map(cell_text).unwrap_or_default().to_uppercase();
            if branche.is_empty() {
                anomalies.push(Anomalie::erreur(format!(
                    "Ligne {} (police {}) : branche absente, prime {} non affectable.",
                    ligne_excel,
                    police,
                    fmt_montant(prime)
                )));
                continue;
            }
            if police.is_empty() {
                anomalies.push(Anomalie::avertissement(format!("Ligne {} : numéro de police absent.", ligne_excel)));
            } else {
                *polices_vues.entry(police.clone()).or_insert(0) += 1;
            }
            if let Some(dt) = date {
                date_min = Some(date_min.map_or(dt, |m| m.min(dt)));
                date_max = Some(date_max.map_or(dt, |m| m.max(dt)));
            }
            if prime == 0.0 {
                nulles += 1;
                continue;
            }
            clients_trouves.entry(code.clone()).or_insert(desig.clone());
            pvs_trouves.insert(pv);
            let prime_ttc = cols.prime_ttc.and_then(|i| row.get(i)).and_then(cell_amount);
            let date_txt = |col: Option<usize>| col.and_then(|i| row.get(i)).and_then(cell_date).map(|d| d.format("%d/%m/%Y").to_string());
            let (date_effet, date_echeance) = (date_txt(cols.date_effet), date_txt(cols.date_echeance));
            let cp = cols.cp.and_then(|i| row.get(i)).and_then(cell_amount);
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
                date_emission: date.map(|d| d.format("%d/%m/%Y").to_string()),
                date_effet,
                date_echeance,
                cp,
            });
        }

        for (p, n) in polices_vues.iter().filter(|(_, n)| **n > 1) {
            anomalies.push(Anomalie::avertissement(format!("Police {} présente {} fois dans le registre.", p, n)));
        }
        if let Some(p) = periode {
            if hors_periode > 0 {
                anomalies.push(Anomalie::info(format!(
                    "{} ligne(s) émise(s) hors de la période du bordereau ({}) non retenue(s) — {} DZD.",
                    hors_periode,
                    p.libelle(),
                    fmt_montant(round2(hors_periode_montant))
                )));
            }
            if sans_date > 0 {
                anomalies.push(Anomalie::avertissement(format!(
                    "{} ligne(s) sans date d'émission : retenue(s) sans contrôle de période.",
                    sans_date
                )));
            }
        }
        if nulles > 0 {
            anomalies.push(Anomalie::info(format!("{} ligne(s) à prime nette nulle ignorée(s).", nulles)));
        }
        let manquants: Vec<&String> = selection
            .clients
            .iter()
            .filter(|c| !clients_trouves.keys().any(|k| k.eq_ignore_ascii_case(c.trim())))
            .collect();
        if !manquants.is_empty() && !items.is_empty() {
            anomalies.push(Anomalie::avertissement(format!(
                "Client(s) sélectionné(s) sans production retenue sur la période : {}.",
                manquants.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            )));
        }
        if pv_filtre.is_none() && pvs_trouves.len() > 1 {
            anomalies.push(Anomalie::erreur(format!(
                "Le registre contient {} points de vente ({}) : choisissez celui du bordereau.",
                pvs_trouves.len(),
                pvs_trouves.iter().cloned().collect::<Vec<_>>().join(", ")
            )));
        }
        if items.is_empty() {
            anomalies.push(Anomalie::erreur(format!(
                "Aucune ligne de production retenue{}{}.",
                pv_filtre.as_ref().map(|p| format!(" pour le point de vente {}", p)).unwrap_or_default(),
                periode.map(|p| format!(" sur la période {}", p.libelle())).unwrap_or_default()
            )));
        }
        for info in &d.info_export {
            anomalies.push(Anomalie::info(format!("Export CIRIS — {}", info)));
        }

        // Point de vente = code de l'agence / de l'AGA ; clients = affaires d'un courtier
        let point_vente = pv_filtre.clone().or_else(|| if pvs_trouves.len() == 1 { pvs_trouves.iter().next().cloned() } else { None });
        let (code, nom) = if clients_filtre.is_empty() {
            (point_vente.clone(), None)
        } else if clients_trouves.len() == 1 {
            let (c, n) = clients_trouves.iter().next().unwrap();
            (Some(c.clone()), Some(n.clone()))
        } else {
            (None, None)
        };
        let periode_txt = match (date_min, date_max) {
            (Some(a), Some(b)) => Some(format!("{} - {}", a.format("%d/%m/%Y"), b.format("%d/%m/%Y"))),
            _ => None,
        };
        let mut res = finalize_result("excel", file_name, "EXCEL_PRODUCTION", nom, code, periode_txt, items, anomalies);
        if let (Some(a), Some(b)) = (date_min, date_max) {
            use chrono::Datelike;
            if a.year() == b.year() {
                res.exercice = Some(a.year());
            }
        }
        res.point_vente = point_vente.clone();
        res.clients_selectionnes = clients_trouves.keys().filter(|_| !clients_filtre.is_empty()).cloned().collect();
        res.selection = Some(match (&point_vente, clients_filtre.is_empty()) {
            (Some(pv), true) => format!("Point de vente {} (tout le portefeuille)", pv),
            (Some(pv), false) => format!(
                "Point de vente {} — client(s) : {}",
                pv,
                clients_trouves.iter().map(|(c, n)| format!("{} {}", c, n)).collect::<Vec<_>>().join(", ")
            ),
            (None, _) => "Tout le fichier".to_string(),
        });
        Ok(res)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Fichier réel de test, cherché dans Docs/, docs/, Import/ puis à la racine du projet.
    /// Absent : le test concerné est sauté (avec un message), les fichiers métier n'étant pas versionnés.
    pub fn fichier_test(nom: &str) -> Option<Vec<u8>> {
        for dossier in ["../Docs", "../docs", "../Import", ".."] {
            let p = std::path::Path::new(dossier).join(nom);
            if let Ok(b) = std::fs::read(&p) {
                return Some(b);
            }
        }
        eprintln!("[test sauté] fichier {} introuvable (Docs/, Import/ ou racine)", nom);
        None
    }

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
        // Ristournes négatives et nombres collés sans espace
        assert_eq!(
            fr_amounts("-645,94-95,34-5.720,55 0,00-158,90-63,56-635,62-508,49-762,74"),
            vec![-645.94, -95.34, -5720.55, 0.0, -158.90, -63.56, -635.62, -508.49, -762.74]
        );
        // Tirets Unicode
        assert_eq!(fr_amounts("−8.591,14 –17,88"), vec![-8591.14, -17.88]);
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
    fn test_flotte_negative_ristourne() {
        let text = r#"
COMPAGNIE ALGERIENNE DES ASSURANCES
N° Police: 2025-347 / VF / 316 / 0 / 21(3.1.0.2)
Assuré: SARL TAHRAOUI (2224 / 0)
Code Ministériel: 3.1.0.2
SUCCURSALE GHARDAIA
VRC - Responsabilité Civile
VDR - Défense Et Recours
VTR - Tous Risques
VVA - Vol Autoradio
VBDG - Bris De Glaces
VPT - Personnes Transportées
VASS - Assistance
VINV - Incendie de Véhicule
VOLV - Vol de Véhicule
-645,94-95,34-5.720,55 0,00-158,90-63,56-635,62-508,49-762,74
CPO FGA
Skoda / Octavia 102190-117-16 07/09/2025 31/12/2025 -8.591,14 50 -17,88 0,00
Détail Assurance Automobile Entreprise
Prime nette:
 60,00
 50,00
-8.591,14
-1.622,82
Prime totale: -10.121,84
"#;
        let pages = vec![text.to_string()];
        let known = vec![
            "VRC".to_string(), "VDR".to_string(), "VTR".to_string(), "VVA".to_string(),
            "VBDG".to_string(), "VPT".to_string(), "VASS".to_string(), "VINV".to_string(),
            "VOLV".to_string(),
        ];
        let res = FlottePdfParser::parse_pages(&pages, "347-vf-316-0-21.PDF", &known);
        assert!(!res.anomalies.iter().any(|a| a.est_bloquante()), "Anomalies: {:?}", res.anomalies);
        assert_eq!(res.items.len(), 8); // 8 garanties non nulles (VVA est à 0.00)
        assert_eq!(res.total_prime_nette, -8591.14);
        assert!(res.items.iter().all(|i| i.police.as_deref() == Some("347/VF/316/0/21")));
        assert_eq!(res.code.as_deref(), Some("2224/0"));
        assert_eq!(res.courtier_ou_client.as_deref(), Some("SARL TAHRAOUI"));
    }

    #[test]
    fn test_parse_real_excel() {
        let path = std::path::Path::new("../Import/Divers.xlsx");
        if !path.exists() {
            return;
        }
        let bytes = std::fs::read(path).unwrap();
        let scan = ExcelProductionParser::scan_from_bytes(&bytes, "Divers.xlsx").unwrap();
        assert_eq!(scan.points_vente.len(), 1);
        assert_eq!(scan.points_vente[0].code, "347");
        let clients = ExcelProductionParser::clients_from_bytes(&bytes, "347").unwrap();
        assert!(clients.iter().any(|c| c.code_client == "347024203/0"));
        // Client SPA FSP : 12 lignes dont 2 à prime nulle (ignorées)
        let sel = ExcelSelection { point_vente: Some("347".into()), clients: vec!["347024203/0".into()] };
        let res = ExcelProductionParser::parse_from_bytes(&bytes, "Divers.xlsx", &sel, None).unwrap();
        assert_eq!(res.items.len(), 10);
        assert!(res.items.iter().all(|i| i.source_type == "BRANCHE_IMPORT"));
        assert!(res.items.iter().all(|i| i.client_code.as_deref() == Some("347024203/0")));
        assert_eq!(res.exercice, Some(2025));
        assert_eq!(res.point_vente.as_deref(), Some("347"));
        assert_eq!(res.code.as_deref(), Some("347024203/0"));
    }

    /// Export volumineux : le détail continue sur les feuilles suivantes (avec ou sans en-tête répété)
    #[test]
    fn test_registre_sur_plusieurs_feuilles() {
        use rust_xlsxwriter::Workbook;
        let mut wb = Workbook::new();
        let entete = ["Police", "Code Client", "Designation Client", "Type Client", "Branche", "Date d'émission", "Prime Nette"];
        let p = wb.add_worksheet();
        p.set_name("Parametres").unwrap();
        p.write_string(7, 0, "Agence :").unwrap();
        p.write_string(7, 1, "7 - SUCCURSALE GHARDAIA").unwrap();
        let lignes = |ws: &mut rust_xlsxwriter::Worksheet, debut: u32, data: &[(&str, &str, f64)]| {
            for (i, (police, branche, prime)) in data.iter().enumerate() {
                let r = debut + i as u32;
                ws.write_string(r, 0, *police).unwrap();
                ws.write_string(r, 1, "C1/0").unwrap();
                ws.write_string(r, 2, "CLIENT UN").unwrap();
                ws.write_string(r, 3, "Individuel").unwrap();
                ws.write_string(r, 4, *branche).unwrap();
                ws.write_number(r, 5, 46150.0).unwrap(); // 07/05/2026
                ws.write_number(r, 6, *prime).unwrap();
            }
        };
        // Feuille 1 : en-tête en ligne 7
        let f1 = wb.add_worksheet();
        f1.set_name("RCCPDE_S").unwrap();
        for (c, h) in entete.iter().enumerate() {
            f1.write_string(6, c as u16, *h).unwrap();
        }
        lignes(f1, 7, &[("492/VP/1/0/0", "VP", 100.0), ("492/VP/2/0/0", "VP", 200.0), ("445/VP/1/0/0", "VP", 50.0)]);
        // Feuille 2 : suite sans en-tête
        let f2 = wb.add_worksheet();
        f2.set_name("RCCPDE_S2").unwrap();
        lignes(f2, 0, &[("492/CN/1/0/0", "CN", 300.0), ("492/VR/1/0/0", "VR", 400.0)]);
        // Feuille 3 : suite avec en-tête répété
        let f3 = wb.add_worksheet();
        f3.set_name("RCCPDE_S3").unwrap();
        for (c, h) in entete.iter().enumerate() {
            f3.write_string(0, c as u16, *h).unwrap();
        }
        lignes(f3, 1, &[("492/MH/1/0/0", "MH", 500.0)]);
        let bytes = wb.save_to_buffer().unwrap();

        let scan = ExcelProductionParser::scan_from_bytes(&bytes, "multi.xlsx").unwrap();
        assert_eq!(scan.total_lignes, 6);
        let pv492 = scan.points_vente.iter().find(|p| p.code == "492").unwrap();
        assert_eq!(pv492.nombre_polices, 5);
        assert_eq!(pv492.total_prime_nette, 1500.0);
        assert!(scan.info_export.iter().any(|i| i.contains("GHARDAIA")));

        let sel = ExcelSelection { point_vente: Some("492".into()), clients: vec![] };
        let res = ExcelProductionParser::parse_from_bytes(&bytes, "multi.xlsx", &sel, Some(PeriodeBordereau { exercice: 2026, mois: Some(5) })).unwrap();
        assert_eq!(res.items.len(), 5);
        assert_eq!(res.total_prime_nette, 1500.0);
        assert!(!res.anomalies.iter().any(|a| a.est_bloquante()), "{:?}", res.anomalies);
        let clients = ExcelProductionParser::clients_from_bytes(&bytes, "492").unwrap();
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0].nombre_polices, 5);
    }

    /// Vrai export CIRIS « Detail Production Excel » (.xls) : feuilles de paramètres, en-tête en ligne 7,
    /// 19 points de vente ; le préfixe du N° de police est le code de l'agence / AGA
    #[test]
    fn test_registre_ciris_reel_prod2026() {
        let Some(bytes) = fichier_test("PROD2026.XLS") else { return };
        let scan = ExcelProductionParser::scan_from_bytes(&bytes, "PROD2026.XLS").unwrap();
        assert_eq!(scan.total_lignes, 61900);
        assert_eq!(scan.points_vente.len(), 19);
        assert!(scan.info_export.iter().any(|i| i.contains("SUCCURSALE GHARDAIA")), "{:?}", scan.info_export);
        let pv492 = scan.points_vente.iter().find(|p| p.code == "492").unwrap();
        assert_eq!(pv492.nombre_polices, 3516);
        assert_eq!(pv492.total_prime_nette, 21533258.89);

        // Tout le point de vente 492 sur l'exercice 2026
        let sel = ExcelSelection { point_vente: Some("492".into()), clients: vec![] };
        let annuel = ExcelProductionParser::parse_from_bytes(
            &bytes,
            "PROD2026.XLS",
            &sel,
            Some(PeriodeBordereau { exercice: 2026, mois: None }),
        )
        .unwrap();
        assert!(!annuel.anomalies.iter().any(|a| a.est_bloquante()), "{:?}", annuel.anomalies);
        assert_eq!(annuel.total_prime_nette, 21533258.89);
        assert!(annuel.items.iter().all(|i| i.police.as_deref().unwrap().starts_with("492/")));
        assert_eq!(annuel.code.as_deref(), Some("492"));

        // Bordereau de mai 2026 : seules les polices émises en mai, le reste signalé hors période
        let mai = ExcelProductionParser::parse_from_bytes(
            &bytes,
            "PROD2026.XLS",
            &sel,
            Some(PeriodeBordereau { exercice: 2026, mois: Some(5) }),
        )
        .unwrap();
        assert!(mai.total_prime_nette > 0.0 && mai.total_prime_nette < annuel.total_prime_nette);
        let hors = mai.anomalies.iter().find(|a| a.message.contains("hors de la période")).expect("hors période signalé");
        assert!(hors.message.contains("05/2026"));

        // Sans choix du point de vente : bloquant (19 agences mélangées)
        let tout = ExcelProductionParser::parse_from_bytes(&bytes, "PROD2026.XLS", &ExcelSelection::default(), None).unwrap();
        assert!(tout.anomalies.iter().any(|a| a.est_bloquante() && a.message.contains("points de vente")));
    }

    #[test]
    fn test_ciris_05_2025_sans_sous_codes() {
        let pdf_path = std::path::Path::new("../Import/05-2025.PDF");
        if !pdf_path.exists() {
            return;
        }
        let bytes = std::fs::read(pdf_path).unwrap();
        // Même sans aucun sous_code paramétré, les coordonnées PDF préservent chaque code intact
        let res = CirisPdfParser::parse_from_bytes(&bytes, "05-2025.PDF", &[]).expect("parse");
        assert!(!res.anomalies.iter().any(|a| a.est_bloquante()), "{:?}", res.anomalies);
        assert_eq!(res.items.len(), 17);
        assert_eq!(res.total_prime_nette, 2272025.22);
        assert!(res.items.iter().any(|i| i.code_garantie == "VRC" && i.branche_code == "VR"));
        assert!(res.items.iter().any(|i| i.code_garantie == "VRCR"));
        assert!(res.items.iter().any(|i| i.code_garantie == "VTR"));
        assert!(!res.items.iter().any(|i| i.code_garantie == "RVTR"));
    }

    fn build_synthetic_ciris_pdf(pages_streams: &[Vec<u8>]) -> Vec<u8> {
        use lopdf::{Dictionary, Object, Stream, dictionary};
        let mut doc = Document::with_version("1.4");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        let mut kids = Vec::new();
        for stream_bytes in pages_streams {
            let content_id = doc.add_object(Stream::new(Dictionary::new(), stream_bytes.clone()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 842.into(), 595.into()],
                "Resources" => resources_id,
                "Contents" => content_id,
            });
            kids.push(Object::Reference(page_id));
        }
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => kids,
                "Count" => pages_streams.len() as i64,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    #[test]
    fn test_ciris_pdf_cellules_positionnees_et_conflit_alias() {
        use std::fmt::Write as _;

        // Page 1 : métadonnées avec dates sur deux lignes séparées (comme RCCBPG 1.23.5)
        let p1 = b"BT /F1 10 Tf 50 500 Td (Production Par Garantie) Tj 0 -20 Td (Agence 492 - AGA ABDELLAH) Tj 0 -20 Td (De la date d'\xe9mission 01/10/2026) Tj 0 -20 Td (A la date d'\xe9mission 31/10/2026) Tj ET".to_vec();

        // Page 2 & 3 : un seul bloc BT..ET pour tous les codes de la colonne Garantie (cas qui cassait extract_text)
        let mut p2 = String::new();
        p2.push_str("BT /F1 8 Tf ");
        p2.push_str("1 0 0 1 10 540 Tm (Garantie) Tj ");
        p2.push_str("1 0 0 1 130 540 Tm (Description Garantie) Tj ");
        p2.push_str("1 0 0 1 512 540 Tm (Primes Nettes) Tj ");
        p2.push_str("1 0 0 1 598 540 Tm (Primes TTC Police) Tj ");
        p2.push_str("1 0 0 1 694 540 Tm (CPO) Tj ");

        // En-têtes de branches & totaux sur page 2
        let p2_rows: &[(&str, &str, f64, &str)] = &[
            ("CANAT", "Catastrophes naturelles", 100000.00, "100,000.00"),
            ("MPDE", "Dégâts des eaux", 920.00, "920.00"),
            ("MPINC", "Incendie", 5000.00, "5,000.00"),
            ("MPRP", "Risques divers", 3600.00, "3,600.00"),
            ("MPVOL", "Vol", 12000.00, "12,000.00"),
            ("RDCI", "Contrôle technique", 15000.00, "15,000.00"),
            ("RDRC", "RC Décennale", 35660.58, "35,660.58"),
            ("RVAC", "Accidents", 500.00, "500.00"),
            ("RVTR", "Transport", 850.00, "850.00"),
        ];
        // Positions y des lignes sur page 2
        // Branches : CA(520), MP(490), RD(410), RV(350)
        p2.push_str("1 0 0 1 6 520 Tm (Branche : CA CATASTROPHES NATURELLES) Tj ");
        p2.push_str("1 0 0 1 6 485 Tm (Total par branche) Tj 1 0 0 1 540 485 Tm (100,000.00) Tj ");
        p2.push_str("1 0 0 1 6 470 Tm (Branche : MP MULTIRISQUES PROFESSIONNELLES ( 8.1.2.2 )) Tj ");
        p2.push_str("1 0 0 1 6 405 Tm (Total par branche) Tj 1 0 0 1 540 405 Tm (21,520.00) Tj ");
        p2.push_str("1 0 0 1 6 390 Tm (Branche : RD RC DECENNALE ( 13.5.1.0 )) Tj ");
        p2.push_str("1 0 0 1 6 345 Tm (Total par branche) Tj 1 0 0 1 540 345 Tm (50,660.58) Tj ");
        p2.push_str("1 0 0 1 6 330 Tm (Branche : RV RC VOITURIER (RC DU TRANSPORTEUR) ( 10.2.0.0 )) Tj ");
        p2.push_str("1 0 0 1 6 285 Tm (Total par branche) Tj 1 0 0 1 540 285 Tm (1,350.00) Tj ET ");

        let p2_ys = [505, 455, 442, 429, 416, 375, 362, 315, 302];
        // Tous les codes dans un seul BT..ET avec TD (ce qui collait en "CANATMPDEMPINCMPRPMPVOLRDCIRDRCRVACRVTR")
        p2.push_str("BT /F1 8 Tf 6 505 Td ");
        for (idx, (code, _, _, _)) in p2_rows.iter().enumerate() {
            let dy = if idx == 0 { 0 } else { p2_ys[idx] - p2_ys[idx - 1] };
            let _ = write!(p2, "0 {} Td ({}) Tj ", dy, code);
        }
        p2.push_str("ET BT /F1 8 Tf 545 505 Td ");
        for (idx, (_, _, _, amt)) in p2_rows.iter().enumerate() {
            let dy = if idx == 0 { 0 } else { p2_ys[idx] - p2_ys[idx - 1] };
            let _ = write!(p2, "0 {} Td ({}) Tj ", dy, amt);
        }
        p2.push_str("ET");

        let mut p3 = String::new();
        p3.push_str("BT /F1 8 Tf ");
        p3.push_str("1 0 0 1 10 540 Tm (Garantie) Tj ");
        p3.push_str("1 0 0 1 512 540 Tm (Primes Nettes) Tj ");
        p3.push_str("1 0 0 1 598 540 Tm (Primes TTC Police) Tj ");
        p3.push_str("1 0 0 1 694 540 Tm (CPO) Tj ");
        p3.push_str("1 0 0 1 6 520 Tm (Branche : VO Assurance Extension De Territoire ( 3.1.0.3 )) Tj ");
        p3.push_str("1 0 0 1 6 485 Tm (Total par branche) Tj 1 0 0 1 540 485 Tm (6,907.11) Tj ");
        p3.push_str("1 0 0 1 6 470 Tm (Branche : VP AUTOMOBILE PARTICULIER ( 3.1.0.1 )) Tj ");
        p3.push_str("1 0 0 1 6 240 Tm (Total par branche) Tj 1 0 0 1 540 240 Tm (2,641,348.00) Tj ");
        p3.push_str("1 0 0 1 6 225 Tm (Branche : VR ASSURANCE FRONTIERE ( 3.1.0.1 )) Tj ");
        p3.push_str("1 0 0 1 6 180 Tm (Total par branche) Tj 1 0 0 1 540 180 Tm (25,044.00) Tj ");
        p3.push_str("1 0 0 1 6 160 Tm (Total par agence) Tj 1 0 0 1 540 160 Tm (2,846,829.69) Tj ET ");

        let p3_rows: &[(&str, f64, &str)] = &[
            ("VRCE", 6907.11, "6,907.11"),
            ("VASS", 145119.99, "145,119.99"),
            ("VBD", 20000.00, "20,000.00"),
            ("VGVD", 30000.00, "30,000.00"),
            ("VCVD", 40000.00, "40,000.00"),
            ("VDR", 50000.00, "50,000.00"),
            ("VINV", 60000.00, "60,000.00"),
            ("VVOL", 100000.00, "100,000.00"),
            ("VVPJ", 80000.00, "80,000.00"),
            ("VPT", 200000.00, "200,000.00"),
            ("VRC", 1500000.00, "1,500,000.00"),
            ("VRCR", 150000.00, "150,000.00"),
            ("VTR", 200000.00, "200,000.00"),
            ("VRF", 50000.00, "50,000.00"),
            ("VRV", 16228.01, "16,228.01"),
            ("VVA", 0.00, "0.00"),
            ("VDR", 0.00, "0.00"),
            ("VRC", 25044.00, "25,044.00"),
        ];
        let p3_ys = [
            505, 455, 442, 429, 416, 403, 390, 377, 364, 351, 338, 325, 312, 299, 286, 273, 210, 197,
        ];
        p3.push_str("BT /F1 8 Tf 6 505 Td ");
        for (idx, (code, _, _)) in p3_rows.iter().enumerate() {
            let dy = if idx == 0 { 0 } else { p3_ys[idx] - p3_ys[idx - 1] };
            let _ = write!(p3, "0 {} Td ({}) Tj ", dy, code);
        }
        p3.push_str("ET BT /F1 8 Tf 545 505 Td ");
        for (idx, (_, _, amt)) in p3_rows.iter().enumerate() {
            let dy = if idx == 0 { 0 } else { p3_ys[idx] - p3_ys[idx - 1] };
            let _ = write!(p3, "0 {} Td ({}) Tj ", dy, amt);
        }
        p3.push_str("ET");

        let pdf_bytes = build_synthetic_ciris_pdf(&[p1, p2.into_bytes(), p3.into_bytes()]);
        // Simulation exacte du conflit signalé : seulement CANAT et quelques alias courts (MP, RV, CR)
        let partial_codes: Vec<String> = ["CANAT", "MP", "RV", "CR"].iter().map(|s| s.to_string()).collect();
        let res = CirisPdfParser::parse_from_bytes(&pdf_bytes, "10-492.PDF", &partial_codes).expect("parse synthetic");

        assert!(!res.anomalies.iter().any(|a| a.est_bloquante()), "{:?}", res.anomalies);
        assert_eq!(res.code.as_deref(), Some("492"));
        assert_eq!(res.mois, Some(10));
        assert_eq!(res.exercice, Some(2026));
        assert_eq!(res.total_prime_nette, 2846829.69);

        let codes: Vec<&str> = res.items.iter().map(|i| i.code_garantie.as_str()).collect();
        for expected in [
            "CANAT", "MPDE", "MPINC", "MPRP", "MPVOL", "RDCI", "RDRC", "RVAC", "RVTR",
            "VRCE", "VASS", "VBD", "VGVD", "VCVD", "VDR", "VINV", "VVOL", "VVPJ", "VPT",
            "VRC", "VRCR", "VTR", "VRF", "VRV",
        ] {
            assert!(codes.contains(&expected), "code attendu manquant: {} dans {:?}", expected, codes);
        }
        for scrambled in ["DEMPI", "NCMPRP", "VOLRD", "CIRDRC", "ACRVTR", "CE", "VA", "SSVBD", "GVDCVD", "FVRV", "MP", "RV"] {
            assert!(!codes.contains(&scrambled), "code tronqué/mélangé détecté: {}", scrambled);
        }
    }
}

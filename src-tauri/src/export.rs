use rust_xlsxwriter::{Format, FormatBorder, Workbook, XlsxError};
use std::path::Path;
use crate::models::Bordereau;

pub struct ExcelExporter;

/// Libellé d'une ligne : "CODE - Libellé" (ou le libellé seul pour une ligne cumulée), mention si exclue
fn ligne_label(l: &crate::models::BordereauLigne) -> String {
    let base = if l.code_garantie.trim().is_empty() || l.code_garantie == l.libelle_garantie {
        l.libelle_garantie.clone()
    } else {
        format!("{} - {}", l.code_garantie, l.libelle_garantie)
    };
    if l.desactive {
        format!("{} (DÉSACTIVÉ / EXCLU)", base)
    } else {
        base
    }
}

impl ExcelExporter {
    pub fn export_bordereau_to_file<P: AsRef<Path>>(bordereau: &Bordereau, file_path: P) -> Result<(), XlsxError> {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        // Formats
        let title_format = Format::new()
            .set_bold()
            .set_font_size(14)
            .set_font_color("#1e293b");

        let table_header_format = Format::new()
            .set_bold()
            .set_background_color("#f1f5f9")
            .set_font_color("#0f172a")
            .set_border(FormatBorder::Thin);

        let currency_format = Format::new()
            .set_num_format("#,##0.00")
            .set_border(FormatBorder::Thin);

        let percent_format = Format::new()
            .set_num_format("0.0%")
            .set_border(FormatBorder::Thin);

        let text_cell_format = Format::new()
            .set_border(FormatBorder::Thin);

        let total_format = Format::new()
            .set_bold()
            .set_background_color("#e2e8f0")
            .set_num_format("#,##0.00")
            .set_border(FormatBorder::Thin);

        let total_label_format = Format::new()
            .set_bold()
            .set_background_color("#e2e8f0")
            .set_border(FormatBorder::Thin);

        // Largeurs de colonnes
        worksheet.set_column_width(0, 32)?; // Branche / Libellé
        worksheet.set_column_width(1, 16)?; // Prime nette
        worksheet.set_column_width(2, 14)?; // Taux apport / Apport
        worksheet.set_column_width(3, 14)?; // Gestion / Taux gestion
        worksheet.set_column_width(4, 18)?; // Commission HT
        worksheet.set_column_width(5, 16)?; // TVA
        worksheet.set_column_width(6, 18)?; // Commission TTC

        // Entête
        worksheet.write_string_with_format(0, 0, &bordereau.succursale, &title_format)?;
        worksheet.write_string(1, 0, &bordereau.departement)?;

        let is_courtier = bordereau.type_intermediaire == "COURTIER";
        let type_title = if is_courtier { "Commission COURTIER" } else { "Commission AGA" };
        worksheet.write_string_with_format(2, 2, type_title, &title_format)?;

        if is_courtier {
            worksheet.write_string(3, 0, format!("COURTIER : {}", bordereau.intermediaire_nom))?;
            worksheet.write_string(4, 0, format!("Code : {}", bordereau.intermediaire_code))?;
            worksheet.write_string(5, 0, format!("N° FACTURE : {}", bordereau.numero_bordereau))?;
            let client = bordereau.client_libelle.clone().or_else(|| bordereau.code_client.clone()).unwrap_or_else(|| "N/A".to_string());
            worksheet.write_string(6, 0, format!("CLIENT : {}", client))?;
            worksheet.write_string(7, 0, format!("EXERCICE : {}", bordereau.exercice))?;
        } else {
            worksheet.write_string(3, 0, format!("AGA : {}", bordereau.intermediaire_nom))?;
            worksheet.write_string(4, 0, format!("Code : {}", bordereau.intermediaire_code))?;
            let mois_str = bordereau.mois.map_or("".to_string(), |m| format!("{:02}", m));
            worksheet.write_string(5, 0, format!("Mois : {}", mois_str))?;
            worksheet.write_string(6, 0, format!("EXERCICE : {}", bordereau.exercice))?;
            worksheet.write_string(7, 0, format!("N° Bordereau : {}", bordereau.numero_bordereau))?;
        }

        let deactivated_format = Format::new()
            .set_font_color("#94a3b8")
            .set_italic()
            .set_border(FormatBorder::Thin);

        let deactivated_currency_format = Format::new()
            .set_font_color("#94a3b8")
            .set_italic()
            .set_num_format("#,##0.00")
            .set_border(FormatBorder::Thin);

        let deactivated_percent_format = Format::new()
            .set_font_color("#94a3b8")
            .set_italic()
            .set_num_format("0.0%")
            .set_border(FormatBorder::Thin);

        let mut row = 9;

        if is_courtier {
            worksheet.write_string_with_format(row, 0, "Branche / Produit", &table_header_format)?;
            worksheet.write_string_with_format(row, 1, "Prime net", &table_header_format)?;
            worksheet.write_string_with_format(row, 2, "Taux d'apport", &table_header_format)?;
            worksheet.write_string_with_format(row, 3, "Commission H.T", &table_header_format)?;
            worksheet.write_string_with_format(row, 4, &format!("TVA ({:.0}%)", bordereau.tva_taux * 100.0), &table_header_format)?;
            worksheet.write_string_with_format(row, 5, "Commission T.T.C", &table_header_format)?;
            row += 1;

            for l in &bordereau.lignes {
                let fmt = if l.desactive { &deactivated_format } else { &text_cell_format };
                let c_fmt = if l.desactive { &deactivated_currency_format } else { &currency_format };
                let p_fmt = if l.desactive { &deactivated_percent_format } else { &percent_format };

                let label = ligne_label(l);

                worksheet.write_string_with_format(row, 0, label, fmt)?;
                worksheet.write_number_with_format(row, 1, l.prime_nette, c_fmt)?;
                worksheet.write_number_with_format(row, 2, l.taux_apport, p_fmt)?;
                worksheet.write_number_with_format(row, 3, l.montant_ht, c_fmt)?;
                worksheet.write_number_with_format(row, 4, l.montant_tva, c_fmt)?;
                worksheet.write_number_with_format(row, 5, l.montant_ttc, c_fmt)?;
                row += 1;
            }

            // Totaux
            worksheet.write_string_with_format(row, 2, "Total HT", &total_label_format)?;
            worksheet.write_number_with_format(row, 3, bordereau.total_commission_ht, &total_format)?;
            row += 1;

            worksheet.write_string_with_format(row, 2, "Total TVA", &total_label_format)?;
            worksheet.write_number_with_format(row, 3, bordereau.total_tva, &total_format)?;
            row += 1;

            worksheet.write_string_with_format(row, 2, "Total TTC", &total_label_format)?;
            worksheet.write_number_with_format(row, 3, bordereau.total_commission_ttc, &total_format)?;
        } else {
            worksheet.write_string_with_format(row, 0, "Branche / Garantie", &table_header_format)?;
            worksheet.write_string_with_format(row, 1, "Prime net", &table_header_format)?;
            worksheet.write_string_with_format(row, 2, "Apport", &table_header_format)?;
            worksheet.write_string_with_format(row, 3, "Gestion", &table_header_format)?;
            worksheet.write_string_with_format(row, 4, "Commission H.T.T", &table_header_format)?;
            worksheet.write_string_with_format(row, 5, "TVA", &table_header_format)?;
            worksheet.write_string_with_format(row, 6, "Commission T.T.C", &table_header_format)?;
            row += 1;

            for l in &bordereau.lignes {
                let fmt = if l.desactive { &deactivated_format } else { &text_cell_format };
                let c_fmt = if l.desactive { &deactivated_currency_format } else { &currency_format };

                let label = ligne_label(l);

                worksheet.write_string_with_format(row, 0, label, fmt)?;
                worksheet.write_number_with_format(row, 1, l.prime_nette, c_fmt)?;
                worksheet.write_number_with_format(row, 2, l.montant_apport, c_fmt)?;
                worksheet.write_number_with_format(row, 3, l.montant_gestion, c_fmt)?;
                worksheet.write_number_with_format(row, 4, l.montant_ht, c_fmt)?;
                worksheet.write_number_with_format(row, 5, l.montant_tva, c_fmt)?;
                worksheet.write_number_with_format(row, 6, l.montant_ttc, c_fmt)?;
                row += 1;
            }

            // Totaux
            worksheet.write_string_with_format(row, 0, "Total Général", &total_label_format)?;
            worksheet.write_number_with_format(row, 1, bordereau.total_prime_nette, &total_format)?;
            worksheet.write_number_with_format(row, 2, bordereau.total_commission_apport, &total_format)?;
            worksheet.write_number_with_format(row, 3, bordereau.total_commission_gestion, &total_format)?;
            worksheet.write_number_with_format(row, 4, bordereau.total_commission_ht, &total_format)?;
            worksheet.write_number_with_format(row, 5, bordereau.total_tva, &total_format)?;
            worksheet.write_number_with_format(row, 6, bordereau.total_commission_ttc, &total_format)?;
        }

        // Traçabilité : polices exclues par le commercial (primes non encaissées), non commissionnées
        let exclues: Vec<(&str, &crate::models::LigneContribution)> = bordereau
            .lignes
            .iter()
            .flat_map(|l| l.contributions.iter().filter(|c| c.exclue).map(move |c| (l.libelle_garantie.as_str(), c)))
            .collect();
        if !exclues.is_empty() {
            let ws = workbook.add_worksheet();
            ws.set_name("Polices exclues")?;
            ws.set_column_width(0, 22)?;
            ws.set_column_width(1, 30)?;
            ws.set_column_width(2, 26)?;
            ws.set_column_width(3, 24)?;
            ws.set_column_width(4, 16)?;
            ws.write_string_with_format(0, 0, "Polices exclues du calcul (primes non encaissées)", &title_format)?;
            for (col, h) in ["N° Police", "Client", "Ligne du bordereau", "Fichier source", "Prime nette"].iter().enumerate() {
                ws.write_string_with_format(2, col as u16, *h, &table_header_format)?;
            }
            let mut r = 3;
            let mut total = 0.0;
            for (ligne, c) in &exclues {
                ws.write_string_with_format(r, 0, c.police.as_deref().unwrap_or("-"), &text_cell_format)?;
                ws.write_string_with_format(r, 1, c.client.as_deref().unwrap_or(""), &text_cell_format)?;
                ws.write_string_with_format(r, 2, *ligne, &text_cell_format)?;
                ws.write_string_with_format(r, 3, &c.fichier_source, &text_cell_format)?;
                ws.write_number_with_format(r, 4, c.montant, &currency_format)?;
                total += c.montant;
                r += 1;
            }
            ws.write_string_with_format(r, 3, "Total exclu", &total_label_format)?;
            ws.write_number_with_format(r, 4, (total * 100.0).round() / 100.0, &total_format)?;
        }

        workbook.save(file_path)?;
        Ok(())
    }
}

impl ExcelExporter {
    /// État « Détail de production courtier » : même présentation que le modèle du service commercial
    /// (logo, en-tête, détail par police, rapprochement commission calculée / facture, écarts, visa).
    pub fn export_detail_production<P: AsRef<Path>>(d: &crate::detail::DetailProduction, file_path: P) -> Result<(), XlsxError> {
        use rust_xlsxwriter::{ExcelDateTime, FormatAlign, Formula, Image};
        let mut workbook = Workbook::new();
        let comptable = "_-* #,##0.00_-;\\-* #,##0.00_-;_-* \"-\"??_-;_-@_-";
        let bleu = "#DCE6F1";
        let entete_doc = Format::new().set_bold().set_font_name("Times New Roman").set_font_size(14);
        let titre = Format::new().set_bold().set_font_size(22).set_align(FormatAlign::Center).set_align(FormatAlign::VerticalCenter);
        let info = Format::new().set_bold().set_font_size(12);
        let info_gauche = info.clone().set_align(FormatAlign::Left);
        let entete = Format::new()
            .set_border(FormatBorder::Thin)
            .set_background_color(bleu)
            .set_align(FormatAlign::Center)
            .set_align(FormatAlign::VerticalCenter)
            .set_text_wrap();
        let entete_gras = entete.clone().set_bold();
        let texte = Format::new().set_border(FormatBorder::Thin);
        let centre = texte.clone().set_align(FormatAlign::Center);
        let date = texte.clone().set_num_format("dd/mm/yyyy").set_align(FormatAlign::Right);
        let montant = Format::new().set_border(FormatBorder::Thin).set_num_format(comptable);
        let montant_gras = montant.clone().set_bold();
        let total_label = Format::new().set_bold().set_border(FormatBorder::Thin);
        let pct_entier = Format::new().set_border(FormatBorder::Thin).set_num_format("0%");
        let pct_decimal = Format::new().set_border(FormatBorder::Thin).set_num_format("0.00%");
        let res = |v: f64| format!("{:.2}", v);
        let date_excel = |s: &Option<String>| -> Option<ExcelDateTime> {
            let s = s.as_deref()?;
            let p: Vec<&str> = s.split('/').collect();
            if p.len() != 3 {
                return None;
            }
            ExcelDateTime::from_ymd(p[2].parse().ok()?, p[1].parse().ok()?, p[0].parse().ok()?).ok()
        };

        {
            let ws = workbook.add_worksheet();
            ws.set_name("Détail de production")?;
            ws.set_landscape();
            ws.set_paper_size(9);
            ws.set_print_fit_to_pages(1, 0);
            ws.set_print_center_horizontally(true);
            ws.set_margins(0.3, 0.3, 0.4, 0.4, 0.2, 0.2);
            for (c, w) in [
                (0u16, 11.0),
                (1, 16.86),
                (2, 16.0),
                (3, 15.71),
                (4, 15.14),
                (5, 16.14),
                (6, 15.71),
                (7, 12.0),
                (8, 6.0),
                (9, 12.57),
                (10, 13.86),
                (11, 9.29),
                (12, 17.14),
            ] {
                ws.set_column_width(c, w)?;
            }
            for (r, h) in [(0u32, 18.75), (1, 18.75), (2, 28.5), (3, 23.25), (4, 15.75), (5, 15.75), (6, 15.75), (7, 30.0)] {
                ws.set_row_height(r, h)?;
            }

            // En-tête : succursale, département, titre, logo
            ws.write_string_with_format(0, 0, &d.succursale, &entete_doc)?;
            ws.write_string_with_format(1, 0, &d.departement, &entete_doc)?;
            ws.merge_range(2, 0, 2, 11, &d.titre, &titre)?;
            let logo = Image::new_from_buffer(include_bytes!("../../src/assets/logo_caat.png"))?.set_scale_width(0.78).set_scale_height(0.78);
            ws.insert_image(0, 12, &logo)?;
            ws.write_string_with_format(4, 0, "Client : ", &info)?;
            ws.write_string_with_format(4, 1, &d.clients.join(" & "), &info)?;
            ws.write_string_with_format(5, 0, if d.type_intermediaire == "COURTIER" { "Courtier : " } else { "AGA : " }, &info)?;
            ws.write_string_with_format(5, 1, &d.intermediaire, &info)?;
            ws.write_string_with_format(6, 0, "EXERCICE ", &info)?;
            match d.mois {
                Some(m) => ws.write_string_with_format(6, 1, &format!("{} (mois {:02})", d.exercice, m), &info_gauche)?,
                None => ws.write_number_with_format(6, 1, d.exercice as f64, &info_gauche)?,
            };

            // Détail par police (A..M)
            let entetes = [
                "Exercice", "Police", "Branche", "Date d'émission", "Date d'éffet", "Date d'échéanche", "Prime Nette", "VRC", "VRCV",
                "ASSISTANSE", "Dommages", "CP", "Prime ttc",
            ];
            for (c, h) in entetes.iter().enumerate() {
                ws.write_string_with_format(7, c as u16, *h, &entete)?;
            }
            let premiere: u32 = 8;
            let mut r = premiere;
            let nombre = |ws: &mut rust_xlsxwriter::Worksheet, r: u32, c: u16, v: Option<f64>| -> Result<(), XlsxError> {
                match v {
                    Some(x) => ws.write_number_with_format(r, c, x, &montant)?,
                    None => ws.write_blank(r, c, &montant)?,
                };
                Ok(())
            };
            for p in &d.polices {
                ws.write_number_with_format(r, 0, p.exercice as f64, &centre)?;
                ws.write_string_with_format(r, 1, &p.police, &texte)?;
                ws.write_string_with_format(r, 2, &p.branche, &centre)?;
                for (c, v) in [(3u16, &p.date_emission), (4, &p.date_effet), (5, &p.date_echeance)] {
                    match date_excel(v) {
                        Some(dt) => ws.write_datetime_with_format(r, c, &dt, &date)?,
                        None => ws.write_string_with_format(r, c, v.as_deref().unwrap_or(""), &date)?,
                    };
                }
                nombre(ws, r, 6, Some(p.prime_nette))?;
                for k in 0..4 {
                    nombre(ws, r, 7 + k as u16, p.ventilation.get(k).copied().flatten())?;
                }
                nombre(ws, r, 11, p.cp)?;
                nombre(ws, r, 12, p.prime_ttc)?;
                r += 1;
            }
            let derniere = r.saturating_sub(1).max(premiere);
            ws.write_string_with_format(r, 5, "TOTAL ", &total_label)?;
            let lettres = ["G", "H", "I", "J", "K", "L", "M"];
            let valeurs = [
                d.total_prime_nette,
                d.total_ventilation.first().copied().unwrap_or(0.0),
                d.total_ventilation.get(1).copied().unwrap_or(0.0),
                d.total_ventilation.get(2).copied().unwrap_or(0.0),
                d.total_ventilation.get(3).copied().unwrap_or(0.0),
                d.total_cp,
                d.total_prime_ttc,
            ];
            for (k, (l, v)) in lettres.iter().zip(valeurs.iter()).enumerate() {
                ws.write_formula_with_format(
                    r,
                    6 + k as u16,
                    Formula::new(format!("=SUM({l}{a}:{l}{b})", a = premiere + 1, b = derniere + 1)).set_result(res(*v)),
                    &montant,
                )?;
            }
            let ligne_total_detail = r + 1;
            r += 3;

            // Rapprochement par branche (B..G)
            for (c, h) in ["Branche", "P-NET", "TAUX", "COM-HT", "VOTR-COM-HT", "Ecart"].iter().enumerate() {
                ws.write_string_with_format(r, 1 + c as u16, *h, &entete_gras)?;
            }
            r += 1;
            let premiere_l = r;
            for l in &d.lignes {
                ws.write_string_with_format(r, 1, &l.libelle, &texte)?;
                ws.write_number_with_format(r, 2, l.prime_nette, &montant)?;
                let entier = ((l.taux * 100.0) - (l.taux * 100.0).round()).abs() < 1e-9;
                ws.write_number_with_format(r, 3, l.taux, if entier { &pct_entier } else { &pct_decimal })?;
                ws.write_number_with_format(r, 4, l.com_ht_calculee, &montant)?;
                nombre(ws, r, 5, l.com_ht_declaree)?;
                let e = l.ecart.unwrap_or(-l.com_ht_calculee);
                ws.write_formula_with_format(r, 6, Formula::new(format!("=F{n}-E{n}", n = r + 1)).set_result(res(e)), &montant)?;
                r += 1;
            }
            let derniere_l = r.saturating_sub(1).max(premiere_l);
            ws.write_string_with_format(r, 1, "TOTAL ", &texte)?;
            ws.write_formula_with_format(
                r,
                2,
                Formula::new(format!("=SUM(C{}:C{})", premiere_l + 1, derniere_l + 1)).set_result(res(d.total_prime_lignes)),
                &montant,
            )?;
            for c in 3..6u16 {
                ws.write_blank(r, c, &texte)?;
            }
            ws.write_formula_with_format(r, 6, Formula::new(format!("=F{n}-E{n}", n = r + 1)).set_result("0"), &montant)?;
            let ligne_total_branches = r + 1;
            r += 1;
            // Contrôle (total des branches − total du détail) puis totaux HT / TVA / TTC
            ws.write_formula_with_format(
                r,
                2,
                Formula::new(format!("=C{}-G{}", ligne_total_branches, ligne_total_detail)).set_result(res(d.controle)),
                &Format::new().set_num_format(comptable),
            )?;
            let l_ht = r + 1;
            let taux_tva = format!("{}", (d.tva_taux * 10000.0).round() / 100.0).replace('.', ",");
            let lignes_totaux: [(&str, String, String, String, f64, f64, f64); 3] = [
                ("TOTAL HT", format!("=SUM(E{}:E{})", premiere_l + 1, derniere_l + 1), format!("=SUM(F{}:F{})", premiere_l + 1, derniere_l + 1), String::new(), d.ht_calcule, d.ht_declare, d.ecart_ht),
                (
                    "",
                    format!("=ROUND(E{}*{},2)", l_ht, d.tva_taux),
                    format!("=ROUND(F{}*{},2)", l_ht, d.tva_taux),
                    String::new(),
                    d.tva_calculee,
                    d.tva_declaree,
                    d.ecart_tva,
                ),
                ("TOTAL T.T.C", format!("=E{}+E{}", l_ht, l_ht + 1), format!("=F{}+F{}", l_ht, l_ht + 1), String::new(), d.ttc_calcule, d.ttc_declare, d.ecart_ttc),
            ];
            for (i, (lib, fe, ff, _, ve, vf, vg)) in lignes_totaux.iter().enumerate() {
                let label = if i == 1 { format!("TVA {}%", taux_tva) } else { lib.to_string() };
                ws.write_string_with_format(r, 3, &label, &total_label)?;
                ws.write_formula_with_format(r, 4, Formula::new(fe.clone()).set_result(res(*ve)), &montant_gras)?;
                ws.write_formula_with_format(r, 5, Formula::new(ff.clone()).set_result(res(*vf)), &montant_gras)?;
                ws.write_formula_with_format(r, 6, Formula::new(format!("=F{n}-E{n}", n = r + 1)).set_result(res(*vg)), &montant_gras)?;
                r += 1;
            }
            r += 1;
            let pied = Format::new().set_bold().set_font_size(12).set_align(FormatAlign::Center);
            ws.merge_range(r, 2, r, 11, "Vérification et Confirmation d'encaissement par le directeur d'Agence", &pied)?;
        }

        if !d.polices_exclues.is_empty() {
            let ws2 = workbook.add_worksheet();
            ws2.set_name("Polices exclues")?;
            ws2.set_column_width(0, 22)?;
            ws2.set_column_width(1, 34)?;
            ws2.set_column_width(2, 16)?;
            for (c, h) in ["N° Police", "Client", "Prime nette"].iter().enumerate() {
                ws2.write_string_with_format(0, c as u16, *h, &entete_gras)?;
            }
            for (i, (p, c, m)) in d.polices_exclues.iter().enumerate() {
                let row = i as u32 + 1;
                ws2.write_string_with_format(row, 0, p, &texte)?;
                ws2.write_string_with_format(row, 1, c, &texte)?;
                ws2.write_number_with_format(row, 2, *m, &montant)?;
            }
        }

        workbook.save(file_path)?;
        Ok(())
    }
}

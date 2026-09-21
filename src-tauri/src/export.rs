use rust_xlsxwriter::{Format, FormatBorder, Workbook, XlsxError};
use std::path::Path;
use crate::models::Bordereau;

pub struct ExcelExporter;

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
            worksheet.write_string(6, 0, format!("CODE CLIENT : {}", bordereau.code_client.as_deref().unwrap_or("N/A")))?;
            worksheet.write_string(7, 0, format!("EXERCICE : {}", bordereau.exercice))?;
        } else {
            worksheet.write_string(3, 0, format!("AGA : {}", bordereau.intermediaire_nom))?;
            worksheet.write_string(4, 0, format!("Code : {}", bordereau.intermediaire_code))?;
            let mois_str = bordereau.mois.map_or("".to_string(), |m| format!("{:02}", m));
            worksheet.write_string(5, 0, format!("Mois : {}", mois_str))?;
            worksheet.write_string(6, 0, format!("EXERCICE : {}", bordereau.exercice))?;
            worksheet.write_string(7, 0, format!("N° Bordereau : {}", bordereau.numero_bordereau))?;
        }

        let mut row = 9;

        if is_courtier {
            worksheet.write_string_with_format(row, 0, "Branche / Produit", &table_header_format)?;
            worksheet.write_string_with_format(row, 1, "Prime net", &table_header_format)?;
            worksheet.write_string_with_format(row, 2, "Taux d'apport", &table_header_format)?;
            worksheet.write_string_with_format(row, 3, "Commission H.T", &table_header_format)?;
            worksheet.write_string_with_format(row, 4, "TVA (19%)", &table_header_format)?;
            worksheet.write_string_with_format(row, 5, "Commission T.T.C", &table_header_format)?;
            row += 1;

            for l in &bordereau.lignes {
                worksheet.write_string_with_format(row, 0, format!("{} - {}", l.code_garantie, l.libelle_garantie), &text_cell_format)?;
                worksheet.write_number_with_format(row, 1, l.prime_nette, &currency_format)?;
                worksheet.write_number_with_format(row, 2, l.taux_apport, &percent_format)?;
                worksheet.write_number_with_format(row, 3, l.montant_ht, &currency_format)?;
                worksheet.write_number_with_format(row, 4, l.montant_tva, &currency_format)?;
                worksheet.write_number_with_format(row, 5, l.montant_ttc, &currency_format)?;
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
                worksheet.write_string_with_format(row, 0, format!("{} - {}", l.code_garantie, l.libelle_garantie), &text_cell_format)?;
                worksheet.write_number_with_format(row, 1, l.prime_nette, &currency_format)?;
                worksheet.write_number_with_format(row, 2, l.montant_apport, &currency_format)?;
                worksheet.write_number_with_format(row, 3, l.montant_gestion, &currency_format)?;
                worksheet.write_number_with_format(row, 4, l.montant_ht, &currency_format)?;
                worksheet.write_number_with_format(row, 5, l.montant_tva, &currency_format)?;
                worksheet.write_number_with_format(row, 6, l.montant_ttc, &currency_format)?;
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

        workbook.save(file_path)?;
        Ok(())
    }
}

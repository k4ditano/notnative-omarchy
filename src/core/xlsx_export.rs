//! Exportación de tablas a formato XLSX con fórmulas nativas de Excel
//!
//! Este módulo permite exportar las tablas de Base a archivos Excel (.xlsx)
//! preservando las fórmulas para que funcionen directamente en Excel.

use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Formula, Workbook, Worksheet, XlsxError};
use std::path::Path;

use super::base::{CellFormat, SpecialCellContent, SpecialRow, ColumnConfig};
use super::base_query::NoteWithProperties;
use super::formula::col_to_letters;

/// Exportar tabla a XLSX
pub fn export_to_xlsx(
    path: &Path,
    notes: &[NoteWithProperties],
    columns: &[ColumnConfig],
    special_rows: &[SpecialRow],
    sheet_name: &str,
) -> Result<(), XlsxError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name(sheet_name)?;

    // Formatos
    let header_format = Format::new()
        .set_bold()
        .set_align(FormatAlign::Center)
        .set_background_color(Color::RGB(0x313244))
        .set_font_color(Color::RGB(0xCDD6F4))
        .set_border(FormatBorder::Thin);

    let cell_format = Format::new()
        .set_border(FormatBorder::Thin);

    let special_row_format = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x45475A))
        .set_font_color(Color::RGB(0xCDD6F4))
        .set_border(FormatBorder::Thin);

    // Columnas visibles
    let visible_columns: Vec<_> = columns.iter().filter(|c| c.visible).collect();

    // Escribir headers
    for (col_idx, col) in visible_columns.iter().enumerate() {
        let header = col.title.as_ref().unwrap_or(&col.property);
        worksheet.write_with_format(0, col_idx as u16, header, &header_format)?;
        
        // Ajustar ancho de columna
        let width = col.width.unwrap_or(120) as f64 / 8.0;
        worksheet.set_column_width(col_idx as u16, width)?;
    }

    // Escribir datos
    for (row_idx, note) in notes.iter().enumerate() {
        let excel_row = (row_idx + 1) as u32; // +1 por header

        for (col_idx, col) in visible_columns.iter().enumerate() {
            let value = get_property_value(note, &col.property);
            let excel_col = col_idx as u16;

            // Intentar escribir como número si es posible
            if let Ok(num) = value.parse::<f64>() {
                worksheet.write_number_with_format(excel_row, excel_col, num, &cell_format)?;
            } else {
                worksheet.write_string_with_format(excel_row, excel_col, &value, &cell_format)?;
            }
        }
    }

    // Escribir filas especiales al final
    let special_start_row = (notes.len() + 1) as u32;

    for (special_idx, special_row) in special_rows.iter().enumerate() {
        let excel_row = special_start_row + special_idx as u32;

        // Primera columna: label
        worksheet.write_string_with_format(excel_row, 0, &special_row.label, &special_row_format)?;

        // Resto de columnas
        for (col_idx, col) in visible_columns.iter().enumerate().skip(1) {
            let excel_col = col_idx as u16;

            if let Some(cell_content) = special_row.cells.get(&col.property) {
                let format = create_cell_format(&cell_content.format, &special_row_format);

                if cell_content.is_formula() {
                    // Convertir fórmula a formato Excel
                    let excel_formula = convert_formula_for_excel(&cell_content.content, notes.len());
                    worksheet.write_formula_with_format(excel_row, excel_col, Formula::new(&excel_formula), &format)?;
                } else {
                    worksheet.write_string_with_format(excel_row, excel_col, &cell_content.content, &format)?;
                }
            }
        }
    }

    // Guardar
    workbook.save(path)?;
    Ok(())
}

/// Obtener valor de una propiedad de la nota
fn get_property_value(note: &NoteWithProperties, property: &str) -> String {
    match property {
        "title" => note.metadata.name.clone(),
        "created" => note.metadata.created_at.format("%Y-%m-%d %H:%M").to_string(),
        "modified" => note.metadata.updated_at.format("%Y-%m-%d %H:%M").to_string(),
        other => note
            .properties
            .get(other)
            .map(|v| v.to_display_string())
            .unwrap_or_default(),
    }
}

/// Convertir fórmula interna a formato Excel
/// Nuestras fórmulas usan 1-indexed rows, Excel también
fn convert_formula_for_excel(formula: &str, data_rows: usize) -> String {
    let mut excel_formula = formula.to_string();

    // Reemplazar rangos de columna entera (B:B) por rangos específicos
    // B:B -> B2:B{last_row} (excluyendo header)
    let last_row = data_rows + 1; // +1 por header

    // Buscar patrones como "B:B" y convertir a "B2:B{last_row}"
    let col_range_pattern = regex::Regex::new(r"([A-Z]+):([A-Z]+)").unwrap();
    excel_formula = col_range_pattern
        .replace_all(&excel_formula, |caps: &regex::Captures| {
            let col = &caps[1];
            format!("{}2:{}{}", col, col, last_row)
        })
        .to_string();

    excel_formula
}

/// Crear formato de celda para Excel basado en CellFormat
fn create_cell_format(cell_format: &CellFormat, base_format: &Format) -> Format {
    let mut format = base_format.clone();

    if cell_format.bold {
        format = format.set_bold();
    }

    if let Some(ref color) = cell_format.color {
        // Convertir color CSS a Color para Excel
        if let Some(c) = css_color_to_rgb(color) {
            format = format.set_font_color(c);
        }
    }

    if let Some(ref bg) = cell_format.background {
        if let Some(c) = css_color_to_rgb(bg) {
            format = format.set_background_color(c);
        }
    }

    // Formatear números según decimales
    if let Some(decimals) = cell_format.decimals {
        let num_format = match decimals {
            0 => "#,##0",
            1 => "#,##0.0",
            2 => "#,##0.00",
            _ => "#,##0.00",
        };
        
        // Añadir prefijo/sufijo si existen
        let mut full_format = String::new();
        if let Some(ref prefix) = cell_format.prefix {
            full_format.push_str(&format!("\"{}\"", prefix));
        }
        full_format.push_str(num_format);
        if let Some(ref suffix) = cell_format.suffix {
            full_format.push_str(&format!("\"{}\"", suffix));
        }
        
        format = format.set_num_format(&full_format);
    }

    format
}

/// Convertir color CSS a Color para Excel
fn css_color_to_rgb(color: &str) -> Option<Color> {
    // Colores con nombre común
    match color.to_lowercase().as_str() {
        "red" => return Some(Color::Red),
        "green" => return Some(Color::Green),
        "blue" => return Some(Color::Blue),
        "white" => return Some(Color::White),
        "black" => return Some(Color::Black),
        "yellow" => return Some(Color::Yellow),
        "orange" => return Some(Color::Orange),
        "purple" => return Some(Color::Purple),
        "gray" | "grey" => return Some(Color::Gray),
        _ => {}
    }

    // Si es hex (#RRGGBB)
    if color.starts_with('#') && color.len() == 7 {
        if let Ok(r) = u8::from_str_radix(&color[1..3], 16) {
            if let Ok(g) = u8::from_str_radix(&color[3..5], 16) {
                if let Ok(b) = u8::from_str_radix(&color[5..7], 16) {
                    let rgb = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                    return Some(Color::RGB(rgb));
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_formula() {
        assert_eq!(
            convert_formula_for_excel("=SUM(B:B)", 10),
            "=SUM(B2:B11)"
        );
        assert_eq!(
            convert_formula_for_excel("=AVG(C1:C10)", 10),
            "=AVG(C1:C10)" // No cambia rangos explícitos
        );
    }

    #[test]
    fn test_css_color_to_rgb() {
        assert!(matches!(css_color_to_rgb("red"), Some(Color::Red)));
        assert!(matches!(css_color_to_rgb("#ff5500"), Some(Color::RGB(_))));
    }
}

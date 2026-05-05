//! Import/export boundary for cut-list data.
//!
//! CSV is the first import format. Its schema is documented in
//! `docs/csv-import-schema.md`.

use std::{collections::HashMap, fs, io, path::Path};

use crate::domain::{CutPiece, PatternDirection, PieceId, StockPiece};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CsvImportResult {
    pub stock_pieces: Vec<StockPiece>,
    pub cut_pieces: Vec<CutPiece>,
    pub errors: Vec<CsvImportError>,
}

impl CsvImportResult {
    #[must_use]
    pub fn imported_count(&self) -> usize {
        self.stock_pieces.len() + self.cut_pieces.len()
    }

    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvImportError {
    pub line: usize,
    pub message: String,
}

#[allow(clippy::missing_errors_doc)]
pub fn import_project_csv_file(
    path: impl AsRef<Path>,
    first_piece_id: u64,
) -> io::Result<CsvImportResult> {
    let source = fs::read_to_string(path)?;
    Ok(import_project_csv(&source, first_piece_id))
}

#[must_use]
pub fn import_project_csv(source: &str, first_piece_id: u64) -> CsvImportResult {
    let mut result = CsvImportResult::default();
    let mut next_id = first_piece_id;
    let Some((header_line, header)) = first_record(source, &mut result.errors) else {
        if result.errors.is_empty() {
            result.errors.push(CsvImportError {
                line: 1,
                message: "CSV enthält keine Header-Zeile".to_string(),
            });
        }
        return result;
    };

    let Some(header) = CsvHeader::from_record(header_line, &header, &mut result.errors) else {
        return result;
    };

    for (line, record) in records_after_header(source, header_line, &mut result.errors) {
        if record.len() != header.column_count {
            result.errors.push(CsvImportError {
                line,
                message: format!(
                    "{} Spalte(n) gefunden, erwartet {}",
                    record.len(),
                    header.column_count
                ),
            });
            continue;
        }

        match csv_piece_from_record(line, &record, &header, next_id) {
            Ok(CsvPiece::Cut(cut)) => {
                next_id += 1;
                result.cut_pieces.push(cut);
            }
            Ok(CsvPiece::Stock(stock)) => {
                next_id += 1;
                result.stock_pieces.push(stock);
            }
            Err(error) => result.errors.push(error),
        }
    }

    result
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CsvHeader {
    column_count: usize,
    label: usize,
    width: usize,
    length: usize,
    quantity: usize,
    pattern: Option<usize>,
    rotation: Option<usize>,
    piece_type: Option<usize>,
}

impl CsvHeader {
    fn from_record(
        line: usize,
        record: &[String],
        errors: &mut Vec<CsvImportError>,
    ) -> Option<Self> {
        let mut columns = HashMap::new();

        for (index, name) in record.iter().enumerate() {
            let Some(column) = csv_column(name) else {
                continue;
            };

            if columns.insert(column, index).is_some() {
                errors.push(CsvImportError {
                    line,
                    message: format!("Spalte `{}` ist doppelt vorhanden", column.name()),
                });
                return None;
            }
        }

        let label = required_column(line, &columns, CsvColumn::Label, errors)?;
        let width = required_column(line, &columns, CsvColumn::Width, errors)?;
        let length = required_column(line, &columns, CsvColumn::Length, errors)?;
        let quantity = required_column(line, &columns, CsvColumn::Quantity, errors)?;

        Some(Self {
            column_count: record.len(),
            label,
            width,
            length,
            quantity,
            pattern: columns.get(&CsvColumn::Pattern).copied(),
            rotation: columns.get(&CsvColumn::Rotation).copied(),
            piece_type: columns.get(&CsvColumn::PieceType).copied(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CsvColumn {
    Label,
    Width,
    Length,
    Quantity,
    Pattern,
    Rotation,
    PieceType,
}

impl CsvColumn {
    fn name(self) -> &'static str {
        match self {
            Self::Label => "label",
            Self::Width => "width",
            Self::Length => "length",
            Self::Quantity => "quantity",
            Self::Pattern => "pattern",
            Self::Rotation => "rotation",
            Self::PieceType => "piece_type",
        }
    }
}

fn required_column(
    line: usize,
    columns: &HashMap<CsvColumn, usize>,
    column: CsvColumn,
    errors: &mut Vec<CsvImportError>,
) -> Option<usize> {
    if let Some(index) = columns.get(&column).copied() {
        Some(index)
    } else {
        errors.push(CsvImportError {
            line,
            message: format!("Pflichtspalte `{}` fehlt", column.name()),
        });
        None
    }
}

enum CsvPiece {
    Cut(CutPiece),
    Stock(StockPiece),
}

fn csv_piece_from_record(
    line: usize,
    record: &[String],
    header: &CsvHeader,
    id: u64,
) -> Result<CsvPiece, CsvImportError> {
    let piece_type = parse_piece_type(optional_cell(record, header.piece_type).unwrap_or_default())
        .map_err(|message| CsvImportError { line, message })?;
    let width = parse_positive_u32(cell(record, header.width, line)?, "width")
        .map_err(|message| CsvImportError { line, message })?;
    let length = parse_positive_u32(cell(record, header.length, line)?, "length")
        .map_err(|message| CsvImportError { line, message })?;
    let quantity = parse_positive_u32(cell(record, header.quantity, line)?, "quantity")
        .map_err(|message| CsvImportError { line, message })?;
    let pattern = parse_pattern(optional_cell(record, header.pattern).unwrap_or_default())
        .map_err(|message| CsvImportError { line, message })?;

    match piece_type {
        CsvPieceType::Cut => {
            let label = cell(record, header.label, line)?.trim();
            if label.is_empty() {
                return Err(CsvImportError {
                    line,
                    message: "label darf für Cutpieces nicht leer sein".to_string(),
                });
            }

            let can_rotate =
                parse_rotation(optional_cell(record, header.rotation).unwrap_or_default())
                    .map_err(|message| CsvImportError { line, message })?;

            Ok(CsvPiece::Cut(CutPiece {
                id: PieceId(id),
                label: label.to_string(),
                width,
                length,
                quantity,
                pattern,
                can_rotate,
            }))
        }
        CsvPieceType::Stock => Ok(CsvPiece::Stock(StockPiece {
            id: PieceId(id),
            width,
            length,
            quantity: Some(quantity),
            pattern,
        })),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CsvPieceType {
    Cut,
    Stock,
}

fn cell(record: &[String], index: usize, line: usize) -> Result<&str, CsvImportError> {
    record
        .get(index)
        .map(String::as_str)
        .ok_or_else(|| CsvImportError {
            line,
            message: "CSV-Zeile ist kürzer als die Header-Definition".to_string(),
        })
}

fn optional_cell(record: &[String], index: Option<usize>) -> Option<&str> {
    index
        .and_then(|index| record.get(index))
        .map(|cell| cell.trim())
}

fn parse_positive_u32(value: &str, field: &str) -> Result<u32, String> {
    let value = value.trim();
    let parsed = value
        .parse::<u32>()
        .map_err(|_parse_error| format!("{field} muss eine positive ganze Zahl sein"))?;

    if parsed == 0 {
        return Err(format!("{field} muss größer als 0 sein"));
    }

    Ok(parsed)
}

fn parse_piece_type(value: &str) -> Result<CsvPieceType, String> {
    match normalize_value(value).as_str() {
        "" | "cut" | "cutpiece" | "cut_piece" => Ok(CsvPieceType::Cut),
        "stock" | "stockpiece" | "stock_piece" => Ok(CsvPieceType::Stock),
        _ => Err(format!("unbekannter piece_type `{}`", value.trim())),
    }
}

fn parse_pattern(value: &str) -> Result<PatternDirection, String> {
    match normalize_value(value).as_str() {
        "" | "none" | "no" | "n" => Ok(PatternDirection::None),
        "width" | "parallelwidth" | "parallel_width" | "parallel_to_width" | "paralleltowidth" => {
            Ok(PatternDirection::ParallelToWidth)
        }
        "length" | "parallel_length" | "parallellength" | "parallel_to_length"
        | "paralleltolength" => Ok(PatternDirection::ParallelToLength),
        _ => Err(format!("unbekanntes pattern `{}`", value.trim())),
    }
}

fn parse_rotation(value: &str) -> Result<bool, String> {
    match normalize_value(value).as_str() {
        "" | "true" | "yes" | "y" | "1" => Ok(true),
        "false" | "no" | "n" | "0" => Ok(false),
        _ => Err(format!("unbekannter rotation-Wert `{}`", value.trim())),
    }
}

fn csv_column(name: &str) -> Option<CsvColumn> {
    match normalize_header(name).as_str() {
        "label" | "name" => Some(CsvColumn::Label),
        "width" => Some(CsvColumn::Width),
        "length" => Some(CsvColumn::Length),
        "quantity" | "amount" => Some(CsvColumn::Quantity),
        "pattern" => Some(CsvColumn::Pattern),
        "rotation" | "canrotate" | "can_rotate" => Some(CsvColumn::Rotation),
        "piecetype" | "piece_type" | "type" | "kind" => Some(CsvColumn::PieceType),
        _ => None,
    }
}

fn normalize_header(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn normalize_value(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn first_record(source: &str, errors: &mut Vec<CsvImportError>) -> Option<(usize, Vec<String>)> {
    for (line, raw_line) in source.lines().enumerate() {
        if raw_line.trim().is_empty() {
            continue;
        }

        return parse_csv_line(line + 1, raw_line, errors).map(|record| (line + 1, record));
    }

    None
}

fn records_after_header(
    source: &str,
    header_line: usize,
    errors: &mut Vec<CsvImportError>,
) -> Vec<(usize, Vec<String>)> {
    let mut records = Vec::new();

    for (line_index, raw_line) in source.lines().enumerate().skip(header_line) {
        if raw_line.trim().is_empty() {
            continue;
        }

        if let Some(record) = parse_csv_line(line_index + 1, raw_line, errors) {
            records.push((line_index + 1, record));
        }
    }

    records
}

fn parse_csv_line(
    line: usize,
    raw_line: &str,
    errors: &mut Vec<CsvImportError>,
) -> Option<Vec<String>> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = raw_line.chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => {
                in_quotes = !in_quotes;
            }
            ',' if !in_quotes => {
                fields.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if in_quotes {
        errors.push(CsvImportError {
            line,
            message: "nicht geschlossenes Anführungszeichen".to_string(),
        });
        return None;
    }

    fields.push(current.trim().to_string());
    Some(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_cut_rows_with_required_and_optional_fields() {
        let source = "label,width,length,quantity,pattern,rotation\n\
side,700,500,2,width,false\n\
shelf,600,300,4,none,true\n";

        let result = import_project_csv(source, 10);

        assert_eq!(result.errors, Vec::new());
        assert!(result.stock_pieces.is_empty());
        assert_eq!(result.cut_pieces.len(), 2);
        assert_eq!(result.cut_pieces[0].id, PieceId(10));
        assert_eq!(result.cut_pieces[0].label, "side");
        assert_eq!(result.cut_pieces[0].width, 700);
        assert_eq!(result.cut_pieces[0].length, 500);
        assert_eq!(result.cut_pieces[0].quantity, 2);
        assert_eq!(
            result.cut_pieces[0].pattern,
            PatternDirection::ParallelToWidth
        );
        assert!(!result.cut_pieces[0].can_rotate);
        assert_eq!(result.cut_pieces[1].id, PieceId(11));
        assert!(result.cut_pieces[1].can_rotate);
    }

    #[test]
    fn imports_stock_rows_when_piece_type_is_stock() {
        let source = "name,width,length,amount,pattern,piece_type\n\
birch,2440,1220,3,length,stock\n";

        let result = import_project_csv(source, 1);

        assert_eq!(result.errors, Vec::new());
        assert!(result.cut_pieces.is_empty());
        assert_eq!(result.stock_pieces.len(), 1);
        assert_eq!(result.stock_pieces[0].quantity, Some(3));
        assert_eq!(
            result.stock_pieces[0].pattern,
            PatternDirection::ParallelToLength
        );
    }

    #[test]
    fn reports_row_errors_without_dropping_valid_rows() {
        let source = "label,width,length,quantity\n\
valid,10,20,1\n\
broken,0,20,1\n\
valid2,30,40,2\n";

        let result = import_project_csv(source, 5);

        assert_eq!(result.cut_pieces.len(), 2);
        assert_eq!(result.cut_pieces[0].id, PieceId(5));
        assert_eq!(result.cut_pieces[1].id, PieceId(6));
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line, 3);
        assert_eq!(result.errors[0].message, "width muss größer als 0 sein");
    }

    #[test]
    fn reports_missing_required_header() {
        let source = "label,width,quantity\npart,10,1\n";

        let result = import_project_csv(source, 1);

        assert_eq!(result.imported_count(), 0);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line, 1);
        assert_eq!(result.errors[0].message, "Pflichtspalte `length` fehlt");
    }

    #[test]
    fn parses_quoted_commas_and_escaped_quotes() {
        let source = "label,width,length,quantity\n\"left, \"\"pretty\"\" side\",10,20,1\n";

        let result = import_project_csv(source, 1);

        assert_eq!(result.errors, Vec::new());
        assert_eq!(result.cut_pieces[0].label, "left, \"pretty\" side");
    }
}

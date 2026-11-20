//! Export boundary for `render::Solution`.
//!
//! The first export target is a small dependency-free PDF writer. It consumes only
//! the shared render model so GUI preview and file export stay aligned.

use std::{collections::HashMap, fmt, fmt::Write as _, fs, io, path::Path};

use crate::domain::{CutPiece, LayoutKind, PieceId, Unit};
use crate::measurement::format_length;
use crate::render::{
    solution_sheet_kerf_geometries, CutKerfGeometry, CutKerfLine, PlacedPiece, Rect, Solution,
    SolutionSheet,
};

const PAGE_WIDTH: f64 = 842.0;
const PAGE_HEIGHT: f64 = 595.0;
const MARGIN: f64 = 36.0;
const TABLE_MARGIN: f64 = 18.0;
const LAYOUT_DRAWING_AREA: PdfRect = PdfRect {
    x: MARGIN,
    y: 60.0,
    width: PAGE_WIDTH - MARGIN * 2.0,
    height: 420.0,
};
const TABLE_X: f64 = TABLE_MARGIN;
const TABLE_TOP_Y: f64 = 490.0;
const TABLE_WIDTH: f64 = PAGE_WIDTH - TABLE_MARGIN * 2.0;
const TABLE_ROW_HEIGHT: f64 = 16.0;
const TABLE_CELL_HORIZONTAL_PADDING: f64 = 3.0;
const CUT_LIST_ROWS_PER_PAGE: usize = 26;
const PDF_KERF_COLOR: (f64, f64, f64) = (1.0, 0.0, 0.0);
const PDF_ZERO_KERF_LINE_WIDTH: f64 = 1.0;
const PDF_MIN_VISIBLE_KERF_LINE_WIDTH: f64 = 1.0;

#[derive(Debug)]
pub enum PdfExportError {
    Io(io::Error),
}

impl fmt::Display for PdfExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "PDF-Dateifehler: {error}"),
        }
    }
}

impl std::error::Error for PdfExportError {}

impl From<io::Error> for PdfExportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[allow(clippy::missing_errors_doc)]
pub fn export_solution_pdf_file(
    path: impl AsRef<Path>,
    solution: &Solution,
    unit: Unit,
) -> Result<(), PdfExportError> {
    fs::write(path, export_solution_pdf_bytes(solution, unit))?;
    Ok(())
}

pub(crate) fn export_solution_pdf_file_with_labels(
    path: impl AsRef<Path>,
    solution: &Solution,
    unit: Unit,
    cut_pieces: &[CutPiece],
) -> Result<(), PdfExportError> {
    fs::write(
        path,
        export_solution_pdf_bytes_with_labels(solution, unit, cut_pieces),
    )?;
    Ok(())
}

#[must_use]
pub fn export_solution_pdf_bytes(solution: &Solution, unit: Unit) -> Vec<u8> {
    export_solution_pdf_bytes_with_labels(solution, unit, &[])
}

fn export_solution_pdf_bytes_with_labels(
    solution: &Solution,
    unit: Unit,
    cut_pieces: &[CutPiece],
) -> Vec<u8> {
    let cut_labels = cut_pieces
        .iter()
        .map(|piece| (piece.id, piece.label.as_str()))
        .collect::<HashMap<PieceId, _>>();
    let page_contents = if solution.sheets.is_empty() {
        vec![empty_solution_page_content()]
    } else {
        solution
            .sheets
            .iter()
            .enumerate()
            .flat_map(|(index, sheet)| {
                sheet_page_contents(
                    index,
                    sheet,
                    solution.layout,
                    solution.fitness,
                    unit,
                    &cut_labels,
                )
            })
            .collect()
    };

    pdf_document(page_contents)
}

fn pdf_document(page_contents: Vec<String>) -> Vec<u8> {
    let page_count = page_contents.len();
    let mut objects = Vec::with_capacity(3 + page_count * 2);
    let kids = (0..page_count)
        .map(|index| format!("{} 0 R", page_object_id(index)))
        .collect::<Vec<_>>()
        .join(" ");

    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_string());
    objects.push(format!(
        "<< /Type /Pages /Kids [{kids}] /Count {page_count} >>"
    ));
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());

    for (index, content) in page_contents.into_iter().enumerate() {
        let content_id = content_object_id(index);
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_WIDTH:.0} {PAGE_HEIGHT:.0}] /Resources << /Font << /F1 3 0 R >> >> /Contents {content_id} 0 R >>"
        ));
        objects.push(format!(
            "<< /Length {} >>\nstream\n{}endstream",
            content.len(),
            content
        ));
    }

    let mut pdf = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());

    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }

    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");

    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }

    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );

    pdf
}

fn page_object_id(index: usize) -> usize {
    4 + index * 2
}

fn content_object_id(index: usize) -> usize {
    page_object_id(index) + 1
}

fn empty_solution_page_content() -> String {
    let mut content = String::new();
    add_text(
        &mut content,
        24.0,
        MARGIN,
        PAGE_HEIGHT - MARGIN,
        "Freecut solution",
        false,
    );
    add_text(
        &mut content,
        12.0,
        MARGIN,
        PAGE_HEIGHT - MARGIN - 30.0,
        "No sheets in solution.",
        false,
    );
    content
}

fn sheet_page_contents(
    index: usize,
    sheet: &SolutionSheet,
    layout: LayoutKind,
    fitness: Option<f64>,
    unit: Unit,
    cut_labels: &HashMap<PieceId, &str>,
) -> Vec<String> {
    let mut pages = vec![sheet_layout_page_content(
        index, sheet, layout, fitness, unit, cut_labels,
    )];

    let piece_refs = sheet.placed_pieces.iter().collect::<Vec<_>>();
    if piece_refs.is_empty() {
        pages.push(cut_list_page_content(
            index,
            0,
            sheet,
            &[],
            unit,
            cut_labels,
        ));
    } else {
        for (page_index, pieces) in piece_refs.chunks(CUT_LIST_ROWS_PER_PAGE).enumerate() {
            pages.push(cut_list_page_content(
                index, page_index, sheet, pieces, unit, cut_labels,
            ));
        }
    }

    pages
}

fn sheet_layout_page_content(
    index: usize,
    sheet: &SolutionSheet,
    layout: LayoutKind,
    fitness: Option<f64>,
    unit: Unit,
    cut_labels: &HashMap<PieceId, &str>,
) -> String {
    let mut content = String::new();
    let title = format!(
        "Freecut solution - Sheet {} - Stock #{}",
        index + 1,
        sheet.stock_id.0
    );
    add_text(
        &mut content,
        18.0,
        MARGIN,
        PAGE_HEIGHT - MARGIN,
        &title,
        false,
    );

    let subtitle = match fitness {
        Some(fitness) => format!(
            "Size {} x {} {} - {} cut(s) - fitness {:.3}",
            format_length(sheet.width, unit),
            format_length(sheet.length, unit),
            unit_label(unit),
            sheet.placed_pieces.len(),
            fitness
        ),
        None => format!(
            "Size {} x {} {} - {} cut(s)",
            format_length(sheet.width, unit),
            format_length(sheet.length, unit),
            unit_label(unit),
            sheet.placed_pieces.len()
        ),
    };
    add_text(
        &mut content,
        10.0,
        MARGIN,
        PAGE_HEIGHT - MARGIN - 22.0,
        &subtitle,
        false,
    );

    if let Some(transform) = SheetPdfTransform::new(sheet, LAYOUT_DRAWING_AREA) {
        add_filled_rect(
            &mut content,
            transform.sheet_pdf_rect(),
            sheet_base_fill(sheet, layout),
            Some((0.0, 0.0, 0.0)),
            1.0,
        );

        for waste in &sheet.waste {
            add_filled_rect(
                &mut content,
                transform.rect_to_pdf(*waste),
                (0.92, 0.92, 0.92),
                None,
                0.0,
            );
        }

        for piece in &sheet.placed_pieces {
            add_piece(&mut content, &transform, piece, cut_labels);
        }

        add_kerf_geometries(&mut content, &transform, sheet, layout);
    } else {
        add_text(
            &mut content,
            12.0,
            LAYOUT_DRAWING_AREA.x,
            LAYOUT_DRAWING_AREA.y + LAYOUT_DRAWING_AREA.height - 20.0,
            "Sheet has invalid dimensions.",
            false,
        );
    }

    add_text(
        &mut content,
        9.0,
        MARGIN,
        38.0,
        &format!(
            "Cut rectangles use render coordinates in {}; the following page contains the sheet cut list.",
            unit_label(unit)
        ),
        false,
    );
    content
}

fn add_piece(
    content: &mut String,
    transform: &SheetPdfTransform,
    piece: &PlacedPiece,
    cut_labels: &HashMap<PieceId, &str>,
) {
    let rect = transform.rect_to_pdf(piece.rect);
    let fill = if piece.rotated {
        (0.70, 0.86, 1.0)
    } else {
        (0.74, 0.92, 0.78)
    };
    add_filled_rect(content, rect, fill, None, 0.0);

    add_piece_labels(content, rect, piece, cut_labels);
}

fn add_piece_labels(
    content: &mut String,
    rect: PdfRect,
    piece: &PlacedPiece,
    cut_labels: &HashMap<PieceId, &str>,
) {
    let inset_x = (rect.width * 0.1).min(2.0);
    let inset_y = (rect.height * 0.1).min(2.0);
    let available_width = rect.width - inset_x * 2.0;
    let available_height = rect.height - inset_y * 2.0;
    if available_width <= 0.0 || available_height <= 0.0 {
        return;
    }

    let id = format!("Cut {}", cut_id_label(piece));
    let id_font_size = 7.0_f64
        .min(available_height * 0.75)
        .min(available_width / estimated_pdf_text_width(&id, 1.0));
    let id_cell_height = id_font_size + 1.0;
    let id_cell_y = rect.y + rect.height - inset_y - id_cell_height;
    let _ = writeln!(
        content,
        "q {:.2} {:.2} {:.2} {:.2} re W n",
        rect.x + inset_x,
        id_cell_y,
        available_width,
        id_cell_height
    );
    add_text(
        content,
        id_font_size,
        rect.x + inset_x,
        id_cell_y + id_font_size * 0.25,
        &id,
        false,
    );
    content.push_str("Q\n");

    let Some(name) = cut_labels
        .get(&piece.cut_id)
        .filter(|label| !label.trim().is_empty())
    else {
        return;
    };

    let name_y = rect.y + inset_y;
    let name_height = (id_cell_y - name_y - 1.0).max(0.0);
    let name_font_size = 7.0;
    let name_lines = wrap_graphic_label(name, available_width, name_height, name_font_size);
    if name_lines.is_empty() {
        return;
    }

    let _ = writeln!(
        content,
        "q {:.2} {:.2} {:.2} {:.2} re W n",
        rect.x + inset_x,
        name_y,
        available_width,
        name_height
    );
    let line_height = name_font_size + 1.0;
    let mut line_span = 0.0;
    for _ in name_lines.iter().skip(1) {
        line_span += line_height;
    }
    let mut baseline = name_y + name_height / 2.0 - name_font_size * 0.36 + line_span / 2.0;
    for line in name_lines {
        add_text(
            content,
            name_font_size,
            rect.x + inset_x,
            baseline,
            &line,
            false,
        );
        baseline -= line_height;
    }
    content.push_str("Q\n");
}

fn wrap_graphic_label(
    label: &str,
    available_width: f64,
    available_height: f64,
    font_size: f64,
) -> Vec<String> {
    let character_width = font_size * 0.6;
    let mut max_characters_per_line = 0;
    let mut line_width = 0.0;
    while line_width + character_width <= available_width {
        max_characters_per_line += 1;
        line_width += character_width;
    }

    let line_height = font_size + 1.0;
    let mut max_lines = 0;
    let mut text_height = 0.0;
    while text_height + line_height <= available_height {
        max_lines += 1;
        text_height += line_height;
    }
    if max_characters_per_line == 0 || max_lines == 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    for word in label.split_whitespace() {
        let word_length = word.chars().count();
        let current_length = current_line.chars().count();
        if !current_line.is_empty() && current_length + 1 + word_length <= max_characters_per_line {
            current_line.push(' ');
            current_line.push_str(word);
            continue;
        }

        if !current_line.is_empty() {
            lines.push(std::mem::take(&mut current_line));
        }

        let word_characters = word.chars().collect::<Vec<_>>();
        let mut chunk = String::new();
        let mut word_offset = 0;
        while word_offset < word_characters.len() {
            let remaining = word_characters.len() - word_offset;
            let mut chunk_length = remaining.min(max_characters_per_line);
            if max_characters_per_line > 1
                && remaining > max_characters_per_line
                && remaining - max_characters_per_line == 1
            {
                chunk_length -= 1;
            }
            for character in &word_characters[word_offset..word_offset + chunk_length] {
                chunk.push(*character);
            }
            word_offset += chunk_length;
            if word_offset < word_characters.len() {
                lines.push(std::mem::take(&mut chunk));
            }
        }
        current_line = chunk;
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let ellipsis = ".".repeat((max_characters_per_line / 3).clamp(1, 3));
        let last_line = lines.last_mut().expect("truncated lines are non-empty");
        while last_line.chars().count() + ellipsis.len() > max_characters_per_line {
            last_line.pop();
        }
        last_line.push_str(&ellipsis);
    }

    lines
}

fn sheet_base_fill(sheet: &SolutionSheet, layout: LayoutKind) -> (f64, f64, f64) {
    if layout == LayoutKind::Nested
        || (layout == LayoutKind::Guillotine && sheet.cutting_guide.is_some())
    {
        PDF_KERF_COLOR
    } else {
        (0.98, 0.98, 0.98)
    }
}

fn add_kerf_geometries(
    content: &mut String,
    transform: &SheetPdfTransform,
    sheet: &SolutionSheet,
    layout: LayoutKind,
) {
    for geometry in solution_sheet_kerf_geometries(sheet, layout) {
        match geometry {
            CutKerfGeometry::KerfRect(rect) => {
                let pdf_rect = transform.rect_to_pdf(rect);
                add_filled_rect(content, pdf_rect, PDF_KERF_COLOR, None, 0.0);
                if let Some([start, end]) = thin_pdf_kerf_rect_center_line(pdf_rect) {
                    add_colored_line(
                        content,
                        start,
                        end,
                        PDF_KERF_COLOR,
                        PDF_MIN_VISIBLE_KERF_LINE_WIDTH,
                    );
                }
            }
            CutKerfGeometry::ZeroKerfLine(line) => {
                let [start, end] = transform.kerf_line_to_pdf_points(line);
                add_colored_line(
                    content,
                    start,
                    end,
                    PDF_KERF_COLOR,
                    PDF_ZERO_KERF_LINE_WIDTH,
                );
            }
        }
    }
}

fn cut_list_page_content(
    sheet_index: usize,
    page_index: usize,
    sheet: &SolutionSheet,
    pieces: &[&PlacedPiece],
    unit: Unit,
    cut_labels: &HashMap<PieceId, &str>,
) -> String {
    let mut content = String::new();
    let page_count = sheet
        .placed_pieces
        .len()
        .max(1)
        .div_ceil(CUT_LIST_ROWS_PER_PAGE);
    let title = format!(
        "Cut list - Sheet {} - Stock #{}",
        sheet_index + 1,
        sheet.stock_id.0
    );
    add_text(
        &mut content,
        18.0,
        MARGIN,
        PAGE_HEIGHT - MARGIN,
        &title,
        false,
    );
    add_text(
        &mut content,
        10.0,
        MARGIN,
        PAGE_HEIGHT - MARGIN - 22.0,
        &format!(
            "{} total cut(s) - table page {} of {}",
            sheet.placed_pieces.len(),
            page_index + 1,
            page_count
        ),
        false,
    );

    let table_layout = cut_list_table_layout(pieces, unit, cut_labels);
    add_cut_list_table_header(&mut content, &table_layout);

    if pieces.is_empty() {
        add_text(
            &mut content,
            9.0,
            TABLE_X + 8.0,
            TABLE_TOP_Y - TABLE_ROW_HEIGHT - 11.0,
            "No cuts on this sheet.",
            false,
        );
        return content;
    }

    for (index, piece) in pieces.iter().enumerate() {
        let row_index = f64::from(u16::try_from(index).expect("cut list row index fits in u16"));
        let row_top_y = TABLE_TOP_Y - TABLE_ROW_HEIGHT * (row_index + 1.0);
        add_cut_list_table_row(
            &mut content,
            row_top_y,
            piece,
            index % 2 == 1,
            unit,
            cut_labels,
            &table_layout,
        );
    }

    content
}

fn add_cut_list_table_header(content: &mut String, table_layout: &CutListTableLayout) {
    let header = PdfRect {
        x: TABLE_X,
        y: TABLE_TOP_Y - TABLE_ROW_HEIGHT,
        width: table_layout.width,
        height: TABLE_ROW_HEIGHT,
    };
    add_filled_rect(
        content,
        header,
        (0.86, 0.89, 0.92),
        Some((0.62, 0.67, 0.72)),
        0.6,
    );

    for column in &table_layout.columns {
        add_cut_list_cell_text(
            content,
            8.0,
            PdfRect {
                x: column.x,
                y: TABLE_TOP_Y - TABLE_ROW_HEIGHT,
                width: column.width,
                height: TABLE_ROW_HEIGHT,
            },
            &column.label,
            true,
        );
        add_vertical_line(
            content,
            column.x,
            TABLE_TOP_Y,
            TABLE_TOP_Y - TABLE_ROW_HEIGHT,
        );
    }
    add_vertical_line(
        content,
        TABLE_X + table_layout.width,
        TABLE_TOP_Y,
        TABLE_TOP_Y - TABLE_ROW_HEIGHT,
    );
}

fn add_cut_list_table_row(
    content: &mut String,
    row_top_y: f64,
    piece: &PlacedPiece,
    shaded: bool,
    unit: Unit,
    cut_labels: &HashMap<PieceId, &str>,
    table_layout: &CutListTableLayout,
) {
    let fill = if shaded {
        (0.96, 0.97, 0.98)
    } else {
        (1.0, 1.0, 1.0)
    };
    add_filled_rect(
        content,
        PdfRect {
            x: TABLE_X,
            y: row_top_y - TABLE_ROW_HEIGHT,
            width: table_layout.width,
            height: TABLE_ROW_HEIGHT,
        },
        fill,
        Some((0.78, 0.82, 0.86)),
        0.35,
    );

    let values = cut_list_row_values(piece, unit, cut_labels);

    for (column, value) in table_layout.columns.iter().zip(values) {
        add_cut_list_cell_text(
            content,
            8.0,
            PdfRect {
                x: column.x,
                y: row_top_y - TABLE_ROW_HEIGHT,
                width: column.width,
                height: TABLE_ROW_HEIGHT,
            },
            &value,
            false,
        );
        add_vertical_line(content, column.x, row_top_y, row_top_y - TABLE_ROW_HEIGHT);
    }
    add_vertical_line(
        content,
        TABLE_X + table_layout.width,
        row_top_y,
        row_top_y - TABLE_ROW_HEIGHT,
    );
}

fn add_cut_list_cell_text(
    content: &mut String,
    font_size: f64,
    cell: PdfRect,
    text: &str,
    bold_hint: bool,
) {
    let horizontal_padding = TABLE_CELL_HORIZONTAL_PADDING;
    let text_width = (cell.width - horizontal_padding * 2.0).max(0.0);
    let fitted_text = fit_cut_list_text(text, text_width, font_size);
    let _ = writeln!(
        content,
        "q {:.2} {:.2} {:.2} {:.2} re W n",
        cell.x + horizontal_padding,
        cell.y,
        text_width,
        cell.height
    );
    add_text(
        content,
        font_size,
        cell.x + horizontal_padding,
        cell.y + 5.0,
        &fitted_text,
        bold_hint,
    );
    content.push_str("Q\n");
}

fn fit_cut_list_text(text: &str, available_width: f64, font_size: f64) -> String {
    let character_width = font_size * 0.6;
    let mut max_characters = 0;
    let mut text_width = 0.0;
    while text_width + character_width <= available_width + 0.01 {
        max_characters += 1;
        text_width += character_width;
    }

    let characters = text.chars().collect::<Vec<_>>();
    if characters.len() <= max_characters {
        return text.to_string();
    }

    if max_characters <= 3 {
        return characters.into_iter().take(max_characters).collect();
    }

    let mut fitted = characters
        .into_iter()
        .take(max_characters - 3)
        .collect::<String>();
    fitted.push_str("...");
    fitted
}

fn cut_list_row_values(
    piece: &PlacedPiece,
    unit: Unit,
    cut_labels: &HashMap<PieceId, &str>,
) -> [String; 8] {
    [
        part_name(piece, cut_labels),
        cut_id_label(piece),
        format_length(piece.rect.x, unit),
        format_length(piece.rect.y, unit),
        format_length(piece.rect.width, unit),
        format_length(piece.rect.length, unit),
        if piece.rotated { "rotated" } else { "fixed" }.to_string(),
        pattern_label(piece.pattern).to_string(),
    ]
}

#[derive(Debug, Clone, Copy)]
struct CutListColumnDefinition {
    label: &'static str,
    minimum_width: f64,
}

impl CutListColumnDefinition {
    fn label(self, unit: Unit) -> String {
        if matches!(self.label, "X" | "Y" | "Width" | "Length") {
            format!("{} ({})", self.label, unit_label(unit))
        } else {
            self.label.to_string()
        }
    }
}

const CUT_LIST_COLUMN_DEFINITIONS: [CutListColumnDefinition; 8] = [
    CutListColumnDefinition {
        label: "Part",
        minimum_width: 52.0,
    },
    CutListColumnDefinition {
        label: "Cut ID",
        minimum_width: 48.0,
    },
    CutListColumnDefinition {
        label: "X",
        minimum_width: 34.0,
    },
    CutListColumnDefinition {
        label: "Y",
        minimum_width: 34.0,
    },
    CutListColumnDefinition {
        label: "Width",
        minimum_width: 44.0,
    },
    CutListColumnDefinition {
        label: "Length",
        minimum_width: 44.0,
    },
    CutListColumnDefinition {
        label: "Rotation",
        minimum_width: 48.0,
    },
    CutListColumnDefinition {
        label: "Pattern",
        minimum_width: 58.0,
    },
];

#[derive(Debug, Clone, PartialEq)]
struct CutListColumn {
    label: String,
    x: f64,
    width: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct CutListTableLayout {
    columns: Vec<CutListColumn>,
    width: f64,
}

fn cut_list_table_layout(
    pieces: &[&PlacedPiece],
    unit: Unit,
    cut_labels: &HashMap<PieceId, &str>,
) -> CutListTableLayout {
    let mut desired_widths = CUT_LIST_COLUMN_DEFINITIONS
        .iter()
        .map(|column| {
            column.minimum_width.max(
                estimated_pdf_text_width(&column.label(unit), 8.0)
                    + TABLE_CELL_HORIZONTAL_PADDING * 2.0,
            )
        })
        .collect::<Vec<_>>();

    for piece in pieces {
        for (width, value) in desired_widths
            .iter_mut()
            .zip(cut_list_row_values(piece, unit, cut_labels))
        {
            *width = width
                .max(estimated_pdf_text_width(&value, 8.0) + TABLE_CELL_HORIZONTAL_PADDING * 2.0);
        }
    }

    let minimum_total = CUT_LIST_COLUMN_DEFINITIONS
        .iter()
        .map(|column| column.minimum_width)
        .sum::<f64>();
    let desired_growth = desired_widths
        .iter()
        .zip(CUT_LIST_COLUMN_DEFINITIONS)
        .map(|(width, column)| width - column.minimum_width)
        .sum::<f64>();
    let available_growth = TABLE_WIDTH - minimum_total;
    let growth_scale = if desired_growth > available_growth {
        available_growth / desired_growth
    } else {
        1.0
    };

    let mut x = TABLE_X;
    let columns = CUT_LIST_COLUMN_DEFINITIONS
        .iter()
        .zip(desired_widths)
        .map(|(definition, desired_width)| {
            let width = definition.minimum_width
                + (desired_width - definition.minimum_width) * growth_scale;
            let column = CutListColumn {
                label: definition.label(unit),
                x,
                width,
            };
            x += width;
            column
        })
        .collect();

    CutListTableLayout {
        columns,
        width: x - TABLE_X,
    }
}

fn estimated_pdf_text_width(text: &str, font_size: f64) -> f64 {
    text.chars().fold(0.0, |width, _| width + font_size * 0.6)
}

fn pattern_label(pattern: crate::domain::PatternDirection) -> &'static str {
    match pattern {
        crate::domain::PatternDirection::None => "none",
        crate::domain::PatternDirection::ParallelToWidth => "parallel width",
        crate::domain::PatternDirection::ParallelToLength => "parallel length",
    }
}

fn unit_label(unit: Unit) -> &'static str {
    match unit {
        Unit::Millimeter => "mm",
        Unit::Inch => "inch",
        Unit::Foot => "foot",
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SheetPdfTransform {
    origin_x: f64,
    origin_y: f64,
    sheet_width: f64,
    sheet_length: f64,
    scale: f64,
}

impl SheetPdfTransform {
    fn new(sheet: &SolutionSheet, area: PdfRect) -> Option<Self> {
        if sheet.width == 0 || sheet.length == 0 {
            return None;
        }

        let sheet_width = f64::from(sheet.width);
        let sheet_length = f64::from(sheet.length);
        let scale = (area.width / sheet_width).min(area.height / sheet_length);
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }

        let drawn_width = sheet_width * scale;
        let drawn_height = sheet_length * scale;

        Some(Self {
            origin_x: area.x + (area.width - drawn_width) / 2.0,
            origin_y: area.y + (area.height - drawn_height) / 2.0,
            sheet_width,
            sheet_length,
            scale,
        })
    }

    fn sheet_pdf_rect(self) -> PdfRect {
        PdfRect {
            x: self.origin_x,
            y: self.origin_y,
            width: self.sheet_width * self.scale,
            height: self.sheet_length * self.scale,
        }
    }

    fn rect_to_pdf(self, rect: Rect) -> PdfRect {
        PdfRect {
            x: self.origin_x + f64::from(rect.x) * self.scale,
            y: self.origin_y
                + (self.sheet_length - f64::from(rect.y) - f64::from(rect.length)) * self.scale,
            width: f64::from(rect.width) * self.scale,
            height: f64::from(rect.length) * self.scale,
        }
    }

    fn point_to_pdf(self, x: u32, y: u32) -> PdfPoint {
        PdfPoint {
            x: self.origin_x + f64::from(x) * self.scale,
            y: self.origin_y + (self.sheet_length - f64::from(y)) * self.scale,
        }
    }

    fn kerf_line_to_pdf_points(self, line: CutKerfLine) -> [PdfPoint; 2] {
        [
            self.point_to_pdf(line.start_x, line.start_y),
            self.point_to_pdf(line.end_x, line.end_y),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PdfRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PdfPoint {
    x: f64,
    y: f64,
}

fn add_filled_rect(
    content: &mut String,
    rect: PdfRect,
    fill: (f64, f64, f64),
    stroke: Option<(f64, f64, f64)>,
    stroke_width: f64,
) {
    let operator = if stroke.is_some() { "B" } else { "f" };
    let stroke_color = stroke.unwrap_or(fill);
    let _ = writeln!(
        content,
        "q {:.3} {:.3} {:.3} rg {:.3} {:.3} {:.3} RG {:.2} w {:.2} {:.2} {:.2} {:.2} re {operator} Q",
        fill.0,
        fill.1,
        fill.2,
        stroke_color.0,
        stroke_color.1,
        stroke_color.2,
        stroke_width,
        rect.x,
        rect.y,
        rect.width,
        rect.height
    );
}

fn add_vertical_line(content: &mut String, x: f64, top_y: f64, bottom_y: f64) {
    add_colored_line(
        content,
        PdfPoint { x, y: bottom_y },
        PdfPoint { x, y: top_y },
        (0.70, 0.74, 0.78),
        0.30,
    );
}

fn add_colored_line(
    content: &mut String,
    start: PdfPoint,
    end: PdfPoint,
    color: (f64, f64, f64),
    width: f64,
) {
    let _ = writeln!(
        content,
        "q {:.3} {:.3} {:.3} RG {:.2} w {:.2} {:.2} m {:.2} {:.2} l S Q",
        color.0, color.1, color.2, width, start.x, start.y, end.x, end.y
    );
}

fn thin_pdf_kerf_rect_center_line(rect: PdfRect) -> Option<[PdfPoint; 2]> {
    if rect.width >= PDF_MIN_VISIBLE_KERF_LINE_WIDTH
        && rect.height >= PDF_MIN_VISIBLE_KERF_LINE_WIDTH
    {
        return None;
    }

    let center_x = rect.x + rect.width / 2.0;
    let center_y = rect.y + rect.height / 2.0;
    if rect.width <= rect.height {
        Some([
            PdfPoint {
                x: center_x,
                y: rect.y,
            },
            PdfPoint {
                x: center_x,
                y: rect.y + rect.height,
            },
        ])
    } else {
        Some([
            PdfPoint {
                x: rect.x,
                y: center_y,
            },
            PdfPoint {
                x: rect.x + rect.width,
                y: center_y,
            },
        ])
    }
}

fn add_text(content: &mut String, size: f64, x: f64, y: f64, text: &str, bold_hint: bool) {
    let rendering_mode = if bold_hint { "2 Tr 0.45 w" } else { "0 Tr" };
    let _ = writeln!(
        content,
        "BT /F1 {:.1} Tf {rendering_mode} {:.2} {:.2} Td ({}) Tj ET",
        size,
        x,
        y,
        escape_pdf_text(text)
    );
}

fn part_name(piece: &PlacedPiece, cut_labels: &HashMap<PieceId, &str>) -> String {
    cut_labels
        .get(&piece.cut_id)
        .filter(|label| !label.trim().is_empty())
        .map_or_else(String::new, |label| (*label).to_string())
}

fn cut_id_label(piece: &PlacedPiece) -> String {
    format!("#{}-{}", piece.cut_id.0, piece.instance + 1)
}

fn escape_pdf_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());

    for character in text.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '(' => escaped.push_str("\\("),
            ')' => escaped.push_str("\\)"),
            '\n' | '\r' | '\t' => escaped.push(' '),
            character if character.is_ascii() && !character.is_control() => escaped.push(character),
            _ => escaped.push('?'),
        }
    }

    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{CutPiece, LayoutKind, PatternDirection, PieceId},
        render::{Cut, CutOrientation, LeafKind, SliceNode},
    };

    #[test]
    fn pdf_export_contains_catalog_page_and_cut_list() {
        let solution = sample_solution();

        let pdf = export_solution_pdf_bytes(&solution, Unit::Millimeter);
        let text = String::from_utf8_lossy(&pdf);

        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(text.contains("/Type /Catalog"));
        assert!(text.contains("/Type /Page"));
        assert!(text.contains("Freecut solution - Sheet 1 - Stock #1"));
        assert!(text.contains("Cut #7-1"));
        assert!(text.contains("xref"));
        assert!(text.contains("%%EOF"));
    }

    #[test]
    fn pdf_export_includes_part_labels_in_the_graphic_and_cut_list() {
        let solution = sample_solution();
        let cut_pieces = [CutPiece {
            id: PieceId(7),
            label: "Side panel".to_string(),
            width: 30,
            length: 40,
            quantity: 1,
            pattern: PatternDirection::None,
            can_rotate: false,
        }];

        let pdf = export_solution_pdf_bytes_with_labels(&solution, Unit::Millimeter, &cut_pieces);
        let text = String::from_utf8_lossy(&pdf);

        assert_eq!(text.matches("Side panel").count(), 2);
        assert!(text.contains("Cut ID"));
        assert!(text.contains("#7-1"));
        assert!(!text.contains(r"Side panel \(Cut #7-1\)"));
        assert!(text.contains("re W n"));
    }

    #[test]
    fn graphic_part_labels_wrap_and_truncate_to_fit_piece_bounds() {
        let wrapped = wrap_graphic_label("Side panel", 18.0, 26.0, 7.0);
        assert_eq!(wrapped, ["Side", "pan", "el"]);

        let truncated = wrap_graphic_label("Very long part label", 18.0, 8.0, 7.0);
        assert_eq!(truncated, ["Ver."]);
        assert!(truncated.iter().all(|line| line.chars().count() <= 4));
    }

    #[test]
    fn graphic_shows_cut_id_at_top_left_and_part_name_when_room_allows() {
        let piece = sample_solution().sheets[0].placed_pieces[0].clone();
        let cut_labels = HashMap::from([(PieceId(7), "Side panel")]);
        let mut content = String::new();

        add_piece_labels(
            &mut content,
            PdfRect {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 50.0,
            },
            &piece,
            &cut_labels,
        );

        assert!(content.contains("12.00 61.75 Td (Cut #7-1) Tj ET"));
        assert!(content.contains("(Side panel) Tj"));
    }

    #[test]
    fn graphic_keeps_cut_id_visible_when_part_name_does_not_fit() {
        let mut piece = sample_solution().sheets[0].placed_pieces[0].clone();
        piece.instance = 2;
        let cut_labels = HashMap::from([(PieceId(7), "Side panel")]);
        let mut content = String::new();

        add_piece_labels(
            &mut content,
            PdfRect {
                x: 0.0,
                y: 0.0,
                width: 5.0,
                height: 5.0,
            },
            &piece,
            &cut_labels,
        );

        assert!(content.contains("(Cut #7-3) Tj"));
        assert!(!content.contains("Side panel"));
    }

    #[test]
    fn cut_list_column_widths_follow_content_and_fit_the_page() {
        let solution = sample_solution();
        let piece = &solution.sheets[0].placed_pieces[0];
        let short_labels = HashMap::from([(PieceId(7), "Door")]);
        let short_layout = cut_list_table_layout(&[piece], Unit::Millimeter, &short_labels);
        let long_label = "A very long panel name ".repeat(20);
        let long_labels = HashMap::from([(PieceId(7), long_label.as_str())]);
        let long_layout = cut_list_table_layout(&[piece], Unit::Millimeter, &long_labels);

        assert!(short_layout.width < TABLE_WIDTH);
        assert!(long_layout.columns[0].width > short_layout.columns[0].width);
        assert!(long_layout.width <= TABLE_WIDTH);

        for layout in [&short_layout, &long_layout] {
            let mut next_x = TABLE_X;
            for column in &layout.columns {
                assert_eq!(column.x, next_x);
                next_x += column.width;
            }
            assert_eq!(layout.width, next_x - TABLE_X);
        }
    }

    #[test]
    fn cut_list_text_is_shortened_to_fit_its_cell() {
        let fitted = fit_cut_list_text("a very long part label", 60.0, 8.0);

        assert!(fitted.ends_with("..."));
        assert!(fitted.chars().count() <= 12);
        assert_eq!(fit_cut_list_text("rotated", 60.0, 8.0), "rotated");
    }

    #[test]
    fn pdf_export_labels_dimensions_with_selected_unit() {
        let mut solution = sample_solution();
        solution.sheets[0].width =
            crate::measurement::length_from_whole_units(100, Unit::Inch).expect("fixture fits");
        solution.sheets[0].length =
            crate::measurement::length_from_whole_units(80, Unit::Inch).expect("fixture fits");

        let pdf = export_solution_pdf_bytes(&solution, Unit::Inch);
        let text = String::from_utf8_lossy(&pdf);

        assert!(text.contains("Size 100 x 80 inch"));
        assert!(text.contains(r"X \(inch\)"));
        assert!(text.contains(r"Y \(inch\)"));
        assert!(text.contains(r"Width \(inch\)"));
        assert!(text.contains(r"Length \(inch\)"));
        assert!(text.contains("render coordinates in inch"));
    }

    #[test]
    fn pdf_export_creates_one_page_per_sheet_without_list_overflow() {
        let mut solution = sample_solution();
        solution.sheets.push(SolutionSheet {
            stock_id: PieceId(2),
            width: 50,
            length: 50,
            placed_pieces: Vec::new(),
            waste: Vec::new(),
            cutting_guide: None,
        });

        let pdf = export_solution_pdf_bytes(&solution, Unit::Millimeter);
        let text = String::from_utf8_lossy(&pdf);

        assert_eq!(text.matches("/Type /Page /Parent").count(), 4);
        assert!(text.contains("Freecut solution - Sheet 2 - Stock #2"));
        assert!(text.contains("Cut list - Sheet 2 - Stock #2"));
        assert!(text.contains("No cuts on this sheet."));
    }

    #[test]
    fn pdf_export_uses_dedicated_table_pages_for_cut_lists() {
        let solution = sample_solution_with_piece_count(113);

        let pdf = export_solution_pdf_bytes(&solution, Unit::Millimeter);
        let text = String::from_utf8_lossy(&pdf);

        assert_eq!(text.matches("/Type /Page /Parent").count(), 6);
        assert!(text.contains(r"113 total cut\(s\) - table page 1 of 5"));
        assert!(text.contains(r"113 total cut\(s\) - table page 5 of 5"));
        assert!(text.contains("Width"));
        assert!(text.contains("Length"));
        assert!(text.contains("Rotation"));
        assert!(text.contains("Cut #7-113"));
        assert!(!text.contains("... 85 more"));
        assert!(!text.contains("continued on following page"));
    }

    #[test]
    fn pdf_export_does_not_draw_red_kerf_without_guillotine_guide() {
        let pdf = export_solution_pdf_bytes(&sample_solution(), Unit::Millimeter);
        let text = String::from_utf8_lossy(&pdf);

        assert!(!text.contains("1.000 0.000 0.000 rg"));
        assert!(!text.contains("1.000 0.000 0.000 RG"));
    }

    #[test]
    fn pdf_export_draws_guillotine_guide_kerf_in_red() {
        let pdf =
            export_solution_pdf_bytes(&guillotine_solution_with_cutting_guide(), Unit::Millimeter);
        let text = String::from_utf8_lossy(&pdf);

        assert!(text.contains("1.000 0.000 0.000 rg"));
    }

    #[test]
    fn seam_pdf_pages_use_red_base_for_uncovered_kerf_gaps() {
        let plain = sample_solution();
        let guided = guillotine_solution_with_cutting_guide();
        let nested = nested_solution_with_gap();

        assert_eq!(
            sheet_base_fill(&plain.sheets[0], plain.layout),
            (0.98, 0.98, 0.98)
        );
        assert_eq!(
            sheet_base_fill(&guided.sheets[0], guided.layout),
            PDF_KERF_COLOR
        );
        assert_eq!(
            sheet_base_fill(&nested.sheets[0], nested.layout),
            PDF_KERF_COLOR
        );
    }

    #[test]
    fn pdf_export_draws_nested_uncovered_kerf_gaps_in_red() {
        let pdf = export_solution_pdf_bytes(&nested_solution_with_gap(), Unit::Millimeter);
        let text = String::from_utf8_lossy(&pdf);

        assert!(text.contains("1.000 0.000 0.000 rg"));
    }

    #[test]
    fn pdf_export_draws_zero_kerf_guide_lines_in_red() {
        let pdf = export_solution_pdf_bytes(
            &guillotine_solution_with_zero_kerf_guide(),
            Unit::Millimeter,
        );
        let text = String::from_utf8_lossy(&pdf);

        assert!(text.contains("1.000 0.000 0.000 RG"));
    }

    #[test]
    fn thin_pdf_kerf_rects_get_visible_center_lines() {
        let vertical = thin_pdf_kerf_rect_center_line(PdfRect {
            x: 10.0,
            y: 20.0,
            width: 0.5,
            height: 30.0,
        })
        .expect("thin vertical kerf gets a line");
        assert_eq!(vertical[0].x, vertical[1].x);
        assert_eq!(vertical[0].y, 20.0);
        assert_eq!(vertical[1].y, 50.0);

        let horizontal = thin_pdf_kerf_rect_center_line(PdfRect {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 0.5,
        })
        .expect("thin horizontal kerf gets a line");
        assert_eq!(horizontal[0].y, horizontal[1].y);
        assert_eq!(horizontal[0].x, 10.0);
        assert_eq!(horizontal[1].x, 40.0);

        assert!(thin_pdf_kerf_rect_center_line(PdfRect {
            x: 10.0,
            y: 20.0,
            width: PDF_MIN_VISIBLE_KERF_LINE_WIDTH,
            height: PDF_MIN_VISIBLE_KERF_LINE_WIDTH,
        })
        .is_none());
    }

    #[test]
    fn pdf_transform_maps_render_top_left_to_pdf_bottom_left() {
        let mut solution = sample_solution();
        let sheet = solution.sheets.remove(0);
        let transform =
            SheetPdfTransform::new(&sheet, LAYOUT_DRAWING_AREA).expect("valid transform");

        let pdf_rect = transform.rect_to_pdf(Rect {
            x: 10,
            y: 20,
            width: 30,
            length: 40,
        });

        assert_eq!(pdf_rect.x, transform.origin_x + 10.0 * transform.scale);
        assert_eq!(pdf_rect.width, 30.0 * transform.scale);
        assert_eq!(pdf_rect.height, 40.0 * transform.scale);
        assert_eq!(
            pdf_rect.y,
            transform.origin_y + (f64::from(sheet.length) - 20.0 - 40.0) * transform.scale
        );
    }

    #[test]
    fn pdf_text_escaping_masks_pdf_control_characters() {
        assert_eq!(escape_pdf_text(r#"a(b)c\d"#), r#"a\(b\)c\\d"#);
        assert_eq!(escape_pdf_text("ä\n"), "? ");
    }

    #[test]
    fn pdf_export_writes_file() {
        let path = std::env::temp_dir().join(format!(
            "freecut-export-{}-{PAGE_WIDTH:.0}.pdf",
            std::process::id(),
        ));

        export_solution_pdf_file(&path, &sample_solution(), Unit::Millimeter).expect("write pdf");
        let bytes = std::fs::read(&path).expect("read pdf");
        std::fs::remove_file(path).expect("remove pdf fixture");

        assert!(bytes.starts_with(b"%PDF-1.4"));
    }

    fn sample_solution() -> Solution {
        sample_solution_with_piece_count(1)
    }

    fn sample_solution_with_piece_count(piece_count: usize) -> Solution {
        Solution {
            layout: LayoutKind::Guillotine,
            sheets: vec![SolutionSheet {
                stock_id: PieceId(1),
                width: 100,
                length: 80,
                placed_pieces: (0..piece_count)
                    .map(|index| PlacedPiece {
                        cut_id: PieceId(7),
                        instance: index as u32,
                        rect: Rect {
                            x: 10,
                            y: 20,
                            width: 30,
                            length: 40,
                        },
                        pattern: PatternDirection::None,
                        rotated: false,
                    })
                    .collect(),
                waste: vec![Rect {
                    x: 40,
                    y: 0,
                    width: 60,
                    length: 80,
                }],
                cutting_guide: None,
            }],
            fitness: Some(0.8),
        }
    }

    fn guillotine_solution_with_cutting_guide() -> Solution {
        let cut = Cut::new(
            Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 80,
            },
            CutOrientation::Vertical,
            40,
            2,
        )
        .expect("valid cut");

        Solution {
            layout: LayoutKind::Guillotine,
            sheets: vec![SolutionSheet {
                stock_id: PieceId(1),
                width: 100,
                length: 80,
                placed_pieces: vec![placed_piece(PieceId(7), 0, 0, 0, 40, 80)],
                waste: vec![Rect {
                    x: 42,
                    y: 0,
                    width: 58,
                    length: 80,
                }],
                cutting_guide: Some(SliceNode::cut(
                    cut,
                    SliceNode::leaf(
                        Rect {
                            x: 0,
                            y: 0,
                            width: 40,
                            length: 80,
                        },
                        LeafKind::CutPiece {
                            cut_id: PieceId(7),
                            instance: 0,
                        },
                    ),
                    SliceNode::leaf(
                        Rect {
                            x: 42,
                            y: 0,
                            width: 58,
                            length: 80,
                        },
                        LeafKind::Waste,
                    ),
                )),
            }],
            fitness: Some(0.8),
        }
    }

    fn guillotine_solution_with_zero_kerf_guide() -> Solution {
        let cut = Cut::new(
            Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 80,
            },
            CutOrientation::Horizontal,
            40,
            0,
        )
        .expect("valid zero-kerf cut");

        Solution {
            layout: LayoutKind::Guillotine,
            sheets: vec![SolutionSheet {
                stock_id: PieceId(1),
                width: 100,
                length: 80,
                placed_pieces: vec![placed_piece(PieceId(7), 0, 0, 0, 100, 40)],
                waste: vec![Rect {
                    x: 0,
                    y: 40,
                    width: 100,
                    length: 40,
                }],
                cutting_guide: Some(SliceNode::cut(
                    cut,
                    SliceNode::leaf(
                        Rect {
                            x: 0,
                            y: 0,
                            width: 100,
                            length: 40,
                        },
                        LeafKind::CutPiece {
                            cut_id: PieceId(7),
                            instance: 0,
                        },
                    ),
                    SliceNode::leaf(
                        Rect {
                            x: 0,
                            y: 40,
                            width: 100,
                            length: 40,
                        },
                        LeafKind::Waste,
                    ),
                )),
            }],
            fitness: Some(0.8),
        }
    }

    fn nested_solution_with_gap() -> Solution {
        Solution {
            layout: LayoutKind::Nested,
            sheets: vec![SolutionSheet {
                stock_id: PieceId(1),
                width: 10,
                length: 6,
                placed_pieces: vec![
                    placed_piece(PieceId(7), 0, 0, 0, 4, 6),
                    placed_piece(PieceId(8), 0, 6, 0, 4, 6),
                ],
                waste: Vec::new(),
                cutting_guide: None,
            }],
            fitness: Some(0.8),
        }
    }

    fn placed_piece(
        cut_id: PieceId,
        instance: u32,
        x: u32,
        y: u32,
        width: u32,
        length: u32,
    ) -> PlacedPiece {
        PlacedPiece {
            cut_id,
            instance,
            rect: Rect {
                x,
                y,
                width,
                length,
            },
            pattern: PatternDirection::None,
            rotated: false,
        }
    }
}

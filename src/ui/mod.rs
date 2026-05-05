//! egui/eframe user-interface boundary.
//!
//! UI code edits `domain::Project` and displays `render::Solution`, but does not own
//! optimizer-specific data structures.

use std::fmt::Write as _;

use eframe::egui;

use crate::{
    domain::{
        CutPiece, CutSettings, LayoutKind, PatternDirection, PieceId, Project, StockPiece, Unit,
    },
    export::export_solution_pdf_file,
    import::{import_project_csv_file, CsvImportResult},
    optimizer::{BaselineOptimizer, OptimizeError, OptimizerConfig, OptimizerEffort},
    project_io::{load_project_file, save_project_file, ProjectDocument, PROJECT_FILE_EXTENSION},
    render::{
        solution_sheet_kerf_geometries, CutKerfGeometry as CutPreviewGeometry,
        CutKerfLine as CutPreviewLine, PlacedPiece, Rect as SolutionRect, Solution, SolutionSheet,
    },
};

const APP_TITLE: &str = "Freecut";
const MAX_DIMENSION: u32 = 100_000;
const MAX_QUANTITY: u32 = 100_000;
const VALIDATION_ERROR_PREFIX_EN: &str = "Input error:";
const VALIDATION_ERROR_PREFIX_DE: &str = "Eingabefehler:";
const SHEET_VIEW_PADDING: f32 = 16.0;
const SHEET_VIEW_DEFAULT_MAX_HEIGHT: f32 = 420.0;
const SHEET_VIEW_BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_rgb(18, 22, 28);
const SHEET_SURFACE_COLOR: egui::Color32 = egui::Color32::from_rgb(31, 36, 44);
const SHEET_WASTE_COLOR: egui::Color32 = egui::Color32::from_rgb(46, 52, 62);
const SHEET_WASTE_STROKE_COLOR: egui::Color32 = egui::Color32::from_rgb(116, 124, 136);
const PIECE_NORMAL_FILL_COLOR: egui::Color32 = egui::Color32::from_rgb(52, 103, 75);
const PIECE_ROTATED_FILL_COLOR: egui::Color32 = egui::Color32::from_rgb(46, 83, 128);
const PIECE_HIGHLIGHT_FILL_COLOR: egui::Color32 = egui::Color32::from_rgb(127, 91, 31);
const PIECE_BOUNDARY_STROKE_COLOR: egui::Color32 = egui::Color32::from_rgb(34, 72, 104);
const SOLUTION_BOUNDARY_STROKE_WIDTH: f32 = 1.25;
const CUT_KERF_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 0, 0);
const CUT_KERF_STROKE_WIDTH: f32 = 2.0;
const PIECE_HIGHLIGHT_STROKE_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 183, 90);
const PIECE_LABEL_COLOR: egui::Color32 = egui::Color32::from_rgb(242, 245, 248);

#[allow(clippy::missing_errors_doc)]
pub fn run_native() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();

    eframe::run_native(
        APP_TITLE,
        native_options,
        Box::new(|_creation_context| Ok(Box::<FreecutApp>::default())),
    )
}

#[derive(Debug, Clone, PartialEq)]
struct FreecutAppState {
    project: Project,
    solution: Option<Solution>,
    selection: UiSelection,
    optimizer_effort: OptimizerEffort,
    font_size: UiFontSize,
    language: UiLanguage,
    project_path: String,
    project_io_report: Option<String>,
    csv_import_path: String,
    csv_import_report: Option<String>,
    export_path: String,
    export_report: Option<String>,
    error_message: Option<String>,
    dirty: bool,
}

impl Default for FreecutAppState {
    fn default() -> Self {
        Self {
            project: empty_project(),
            solution: None,
            selection: UiSelection::default(),
            optimizer_effort: OptimizerEffort::default(),
            font_size: UiFontSize::default(),
            language: UiLanguage::default(),
            project_path: String::new(),
            project_io_report: None,
            csv_import_path: String::new(),
            csv_import_report: None,
            export_path: String::new(),
            export_report: None,
            error_message: None,
            dirty: false,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
struct UiSelection {
    stock_index: Option<usize>,
    cut_index: Option<usize>,
    sheet_index: Option<usize>,
    placed_piece_index: Option<usize>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum UiFontSize {
    Compact,
    #[default]
    Normal,
    Large,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum UiLanguage {
    #[default]
    English,
    German,
}

#[derive(Debug, Clone, Copy)]
struct UiTexts {
    language: UiLanguage,
}

#[allow(clippy::match_same_arms)]
impl UiTexts {
    fn new(language: UiLanguage) -> Self {
        Self { language }
    }

    fn project_panel_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Project / Input",
            UiLanguage::German => "Projekt / Eingabe",
        }
    }

    fn project_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Project",
            UiLanguage::German => "Projekt",
        }
    }

    fn project_settings_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Project settings",
            UiLanguage::German => "Projektparameter",
        }
    }

    fn language_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Language",
            UiLanguage::German => "Sprache",
        }
    }

    fn unit_setting_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Unit",
            UiLanguage::German => "Einheit",
        }
    }

    fn kerf_width_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Kerf width",
            UiLanguage::German => "Schnittfuge / Kerf",
        }
    }

    fn layout_setting_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Layout",
            UiLanguage::German => "Layout",
        }
    }

    fn optimizer_effort_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Optimizer effort",
            UiLanguage::German => "Optimizer-Effort",
        }
    }

    fn font_size_setting_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Font size",
            UiLanguage::German => "Schriftgröße",
        }
    }

    fn layout_help(self) -> &'static str {
        match self.language {
            UiLanguage::English => {
                "Guillotine and Nested are available; Nested creates non-guillotine rectangular layouts."
            }
            UiLanguage::German => {
                "Guillotine und Nested sind verfügbar; Nested erzeugt nicht-guillotinierende Rechtecklayouts."
            }
        }
    }

    fn optimize_button(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Optimize",
            UiLanguage::German => "Optimieren",
        }
    }

    fn effort_status_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Effort",
            UiLanguage::German => "Effort",
        }
    }

    fn deterministic_hint(self) -> &'static str {
        match self.language {
            UiLanguage::English => {
                "Deterministic: same input and same effort produce reproducible runs."
            }
            UiLanguage::German => {
                "Deterministisch: gleiche Eingabe und gleicher Effort liefern reproduzierbare Läufe."
            }
        }
    }

    fn ready_status(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Ready",
            UiLanguage::German => "Bereit",
        }
    }

    fn unsaved_project_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Unsaved",
            UiLanguage::German => "Nicht gespeichert",
        }
    }

    fn saved_project_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Saved",
            UiLanguage::German => "Gespeichert",
        }
    }

    fn changed_project_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Changed",
            UiLanguage::German => "Geändert",
        }
    }

    fn no_selection_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "No selection",
            UiLanguage::German => "Keine Auswahl",
        }
    }

    fn selection_prefix(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Selection",
            UiLanguage::German => "Auswahl",
        }
    }

    fn solution_piece_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Solution piece",
            UiLanguage::German => "Lösungsteil",
        }
    }

    fn unit_label(self, unit: Unit) -> &'static str {
        match (self.language, unit) {
            (_, Unit::Millimeter) => "mm",
            (UiLanguage::English, Unit::Inch) => "inch",
            (UiLanguage::English, Unit::Foot) => "foot",
            (UiLanguage::German, Unit::Inch) => "Zoll",
            (UiLanguage::German, Unit::Foot) => "Fuß",
        }
    }

    fn layout_label(layout: LayoutKind) -> &'static str {
        match layout {
            LayoutKind::Guillotine => "Guillotine",
            LayoutKind::Nested => "Nested",
        }
    }

    fn effort_label(self, effort: OptimizerEffort) -> &'static str {
        match (self.language, effort) {
            (UiLanguage::English, OptimizerEffort::Fast) => "Fast",
            (UiLanguage::English, OptimizerEffort::Balanced) => "Balanced",
            (UiLanguage::English, OptimizerEffort::Thorough) => "Thorough",
            (UiLanguage::German, OptimizerEffort::Fast) => "Schnell",
            (UiLanguage::German, OptimizerEffort::Balanced) => "Ausgewogen",
            (UiLanguage::German, OptimizerEffort::Thorough) => "Gründlich",
        }
    }

    fn font_size_label(self, font_size: UiFontSize) -> &'static str {
        match (self.language, font_size) {
            (UiLanguage::English, UiFontSize::Compact) => "Compact",
            (_, UiFontSize::Normal) => "Normal",
            (UiLanguage::English, UiFontSize::Large) => "Large",
            (UiLanguage::German, UiFontSize::Compact) => "Kompakt",
            (UiLanguage::German, UiFontSize::Large) => "Groß",
        }
    }

    fn empty_optimize_input_message(self) -> &'static str {
        match self.language {
            UiLanguage::English => {
                "Add at least one stock piece and one cut piece before optimizing"
            }
            UiLanguage::German => {
                "Zum Optimieren mindestens ein Rohteil und einen Zuschnitt anlegen"
            }
        }
    }

    fn optimize_error_message(self, error: OptimizeError) -> String {
        match error {
            OptimizeError::EmptyInput => match self.language {
                UiLanguage::English => "No optimizable input".to_string(),
                UiLanguage::German => "Keine optimierbaren Eingaben".to_string(),
            },
            OptimizeError::NoSolution => match self.language {
                UiLanguage::English => "No solution found for the current input".to_string(),
                UiLanguage::German => {
                    "Keine Lösung für die aktuellen Eingaben gefunden".to_string()
                }
            },
            OptimizeError::InvalidProject(message) => match self.language {
                UiLanguage::English => format!("Invalid project: {message}"),
                UiLanguage::German => format!("Ungültiges Projekt: {message}"),
            },
        }
    }

    fn project_file_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Project file",
            UiLanguage::German => "Projektdatei",
        }
    }

    fn path_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Path",
            UiLanguage::German => "Pfad",
        }
    }

    fn save_project_button(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Save project",
            UiLanguage::German => "Projekt speichern",
        }
    }

    fn load_project_button(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Load project",
            UiLanguage::German => "Projekt laden",
        }
    }

    fn native_project_file_hint(self) -> String {
        match self.language {
            UiLanguage::English => format!(
                "Native project file: JSON (*.{PROJECT_FILE_EXTENSION}); no hidden autosave path."
            ),
            UiLanguage::German => format!(
                "Native Projektdatei: JSON (*.{PROJECT_FILE_EXTENSION}); kein versteckter Auto-Speicherpfad."
            ),
        }
    }

    fn project_file_empty_save_path_message(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Project file: enter a save path",
            UiLanguage::German => "Projektdatei: Bitte einen Speicherpfad angeben",
        }
    }

    fn project_file_saved_message(self, path: &str) -> String {
        match self.language {
            UiLanguage::English => format!("Project file saved: {path}"),
            UiLanguage::German => format!("Projektdatei gespeichert: {path}"),
        }
    }

    fn project_file_save_failed_message(self, error: impl std::fmt::Display) -> String {
        match self.language {
            UiLanguage::English => format!("Project file: save failed: {error}"),
            UiLanguage::German => format!("Projektdatei: Speichern fehlgeschlagen: {error}"),
        }
    }

    fn project_file_empty_load_path_message(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Project file: enter a load path",
            UiLanguage::German => "Projektdatei: Bitte einen Ladepfad angeben",
        }
    }

    fn project_file_loaded_message(self, path: &str) -> String {
        match self.language {
            UiLanguage::English => format!("Project file loaded: {path}"),
            UiLanguage::German => format!("Projektdatei geladen: {path}"),
        }
    }

    fn project_file_load_failed_message(self, error: impl std::fmt::Display) -> String {
        match self.language {
            UiLanguage::English => format!("Project file: load failed: {error}"),
            UiLanguage::German => format!("Projektdatei: Laden fehlgeschlagen: {error}"),
        }
    }

    fn csv_import_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "CSV import",
            UiLanguage::German => "CSV-Import",
        }
    }

    fn csv_import_button(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Import CSV",
            UiLanguage::German => "CSV importieren",
        }
    }

    fn csv_import_hint(self) -> &'static str {
        match self.language {
            UiLanguage::English => {
                "Schema: docs/csv-import-schema.md · valid rows are imported even when other rows have errors."
            }
            UiLanguage::German => {
                "Schema: docs/csv-import-schema.md · gültige Zeilen werden trotz Zeilenfehlern übernommen."
            }
        }
    }

    fn csv_import_empty_path_message(self) -> &'static str {
        match self.language {
            UiLanguage::English => "CSV import: enter a file path",
            UiLanguage::German => "CSV-Import: Bitte einen Dateipfad angeben",
        }
    }

    fn csv_import_read_failed_message(self, error: impl std::fmt::Display) -> String {
        match self.language {
            UiLanguage::English => format!("CSV import: file could not be read: {error}"),
            UiLanguage::German => format!("CSV-Import: Datei konnte nicht gelesen werden: {error}"),
        }
    }

    fn csv_import_summary(self, result: &CsvImportResult) -> String {
        let mut summary = match self.language {
            UiLanguage::English => format!(
                "CSV import: {} cut piece(s), {} stock piece(s), {} error(s)",
                result.cut_pieces.len(),
                result.stock_pieces.len(),
                result.errors.len()
            ),
            UiLanguage::German => format!(
                "CSV-Import: {} Zuschnitt(e), {} Rohteil(e), {} Fehler",
                result.cut_pieces.len(),
                result.stock_pieces.len(),
                result.errors.len()
            ),
        };

        for error in result.errors.iter().take(3) {
            match self.language {
                UiLanguage::English => {
                    let _ = write!(summary, "; line {}: {}", error.line, error.message);
                }
                UiLanguage::German => {
                    let _ = write!(summary, "; Zeile {}: {}", error.line, error.message);
                }
            }
        }

        if result.errors.len() > 3 {
            match self.language {
                UiLanguage::English => {
                    let _ = write!(summary, "; … {} more", result.errors.len() - 3);
                }
                UiLanguage::German => {
                    let _ = write!(summary, "; … {} weitere", result.errors.len() - 3);
                }
            }
        }

        summary
    }

    fn stock_editor_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Stock pieces",
            UiLanguage::German => "Rohteile",
        }
    }

    fn add_stock_button(self) -> &'static str {
        match self.language {
            UiLanguage::English => "+ Stock",
            UiLanguage::German => "+ Rohteil",
        }
    }

    fn no_stock_pieces_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "No stock pieces yet.",
            UiLanguage::German => "Noch keine Rohteile.",
        }
    }

    fn cut_editor_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Cut pieces",
            UiLanguage::German => "Zuschnitte",
        }
    }

    fn cut_piece_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Cut piece",
            UiLanguage::German => "Zuschnitt",
        }
    }

    fn add_cut_button(self) -> &'static str {
        match self.language {
            UiLanguage::English => "+ Cut",
            UiLanguage::German => "+ Zuschnitt",
        }
    }

    fn no_cut_pieces_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "No cut pieces yet.",
            UiLanguage::German => "Noch keine Zuschnitte.",
        }
    }

    fn width_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Width",
            UiLanguage::German => "Breite",
        }
    }

    fn length_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Length",
            UiLanguage::German => "Länge",
        }
    }

    fn quantity_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Quantity",
            UiLanguage::German => "Menge",
        }
    }

    fn pattern_setting_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Pattern",
            UiLanguage::German => "Maserung",
        }
    }

    fn status_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Status",
            UiLanguage::German => "Status",
        }
    }

    fn label_column_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Label",
            UiLanguage::German => "Label",
        }
    }

    fn rotation_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Rotation",
            UiLanguage::German => "Rotation",
        }
    }

    fn rotatable_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "rotatable",
            UiLanguage::German => "drehbar",
        }
    }

    fn delete_button(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Delete",
            UiLanguage::German => "Löschen",
        }
    }

    fn width_prefix(self) -> &'static str {
        match self.language {
            UiLanguage::English => "w ",
            UiLanguage::German => "b ",
        }
    }

    fn length_prefix(self) -> &'static str {
        match self.language {
            UiLanguage::English => "l ",
            UiLanguage::German => "l ",
        }
    }

    fn quantity_prefix(self) -> &'static str {
        match self.language {
            UiLanguage::English => "q ",
            UiLanguage::German => "m ",
        }
    }

    fn pattern_label(self, pattern: PatternDirection) -> &'static str {
        match (self.language, pattern) {
            (UiLanguage::English, PatternDirection::None) => "none",
            (UiLanguage::English, PatternDirection::ParallelToWidth) => "parallel width",
            (UiLanguage::English, PatternDirection::ParallelToLength) => "parallel length",
            (UiLanguage::German, PatternDirection::None) => "keine",
            (UiLanguage::German, PatternDirection::ParallelToWidth) => "parallel Breite",
            (UiLanguage::German, PatternDirection::ParallelToLength) => "parallel Länge",
        }
    }

    fn validation_ok_label() -> &'static str {
        "ok"
    }

    fn dimension_validation_message(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Dimensions > 0",
            UiLanguage::German => "Maße > 0",
        }
    }

    fn quantity_validation_message(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Quantity > 0",
            UiLanguage::German => "Menge > 0",
        }
    }

    fn missing_label_validation_message(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Label missing",
            UiLanguage::German => "Label fehlt",
        }
    }

    fn validation_error_prefix(self) -> &'static str {
        match self.language {
            UiLanguage::English => VALIDATION_ERROR_PREFIX_EN,
            UiLanguage::German => VALIDATION_ERROR_PREFIX_DE,
        }
    }

    fn pdf_export_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "PDF export",
            UiLanguage::German => "PDF-Export",
        }
    }

    fn pdf_export_button(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Export PDF",
            UiLanguage::German => "PDF exportieren",
        }
    }

    fn pdf_export_hint(self) -> &'static str {
        match self.language {
            UiLanguage::English => {
                "Exports the currently displayed solution as a PDF; file dialog may follow in UX polish."
            }
            UiLanguage::German => {
                "Exportiert die aktuell angezeigte Lösung als PDF; File-Dialog folgt frühestens in der UX-Politur."
            }
        }
    }

    fn pdf_export_empty_path_message(self) -> &'static str {
        match self.language {
            UiLanguage::English => "PDF export: enter an export path",
            UiLanguage::German => "PDF-Export: Bitte einen Exportpfad angeben",
        }
    }

    fn pdf_export_missing_solution_message(self) -> &'static str {
        match self.language {
            UiLanguage::English => "PDF export: no solution available to export",
            UiLanguage::German => "PDF-Export: Keine Lösung zum Exportieren vorhanden",
        }
    }

    fn pdf_exported_message(self, path: &str) -> String {
        match self.language {
            UiLanguage::English => format!("PDF exported: {path}"),
            UiLanguage::German => format!("PDF exportiert: {path}"),
        }
    }

    fn pdf_export_failed_message(self, error: impl std::fmt::Display) -> String {
        match self.language {
            UiLanguage::English => format!("PDF export failed: {error}"),
            UiLanguage::German => format!("PDF-Export fehlgeschlagen: {error}"),
        }
    }

    fn solution_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Solution",
            UiLanguage::German => "Lösung",
        }
    }

    fn sheets_count_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Sheets",
            UiLanguage::German => "Sheets",
        }
    }

    fn fitness_label() -> &'static str {
        "Fitness"
    }

    fn not_available_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "n/a",
            UiLanguage::German => "n/a",
        }
    }

    fn no_solution_sheets_message(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Solution contains no sheets.",
            UiLanguage::German => "Lösung enthält keine Sheets.",
        }
    }

    fn no_solution_yet_message(self) -> &'static str {
        match self.language {
            UiLanguage::English => "No solution calculated yet.",
            UiLanguage::German => "Noch keine Lösung berechnet.",
        }
    }

    fn sheet_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Sheet",
            UiLanguage::German => "Sheet",
        }
    }

    fn stock_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Stock",
            UiLanguage::German => "Rohteil",
        }
    }

    fn cuts_count_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "cut(s)",
            UiLanguage::German => "Zuschnitt(e)",
        }
    }

    fn waste_count_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "waste rect(s)",
            UiLanguage::German => "Restfläche(n)",
        }
    }

    fn invalid_sheet_dimensions_message(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Sheet has invalid dimensions.",
            UiLanguage::German => "Sheet hat ungültige Maße.",
        }
    }

    fn selected_input_cut_hint(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Input cut selected; matching solution pieces are highlighted.",
            UiLanguage::German => {
                "Eingabe-Zuschnitt ausgewählt; passende Lösungsteile sind hervorgehoben."
            }
        }
    }

    fn click_solution_piece_hint(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Click a cut rectangle in the solution to see details.",
            UiLanguage::German => {
                "Klicke ein Zuschnitt-Rechteck in der Lösung, um Details zu sehen."
            }
        }
    }

    fn selected_cut_piece_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Selected cut piece",
            UiLanguage::German => "Ausgewählter Zuschnitt",
        }
    }

    fn input_id_label_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Input ID / label",
            UiLanguage::German => "Eingabe-ID / Label",
        }
    }

    fn input_dimensions_quantity_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Input dimensions / quantity",
            UiLanguage::German => "Eingabe-Maße / Menge",
        }
    }

    fn quantity_value_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "quantity",
            UiLanguage::German => "Menge",
        }
    }

    fn pattern_rotation_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Pattern / rotation",
            UiLanguage::German => "Maserung / Rotation",
        }
    }

    fn fixed_rotation_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "fixed",
            UiLanguage::German => "fix",
        }
    }

    fn missing_input_cut_piece_message(self, id: PieceId) -> String {
        match self.language {
            UiLanguage::English => format!("No input cut piece with ID #{} found.", id.0),
            UiLanguage::German => format!("Kein Eingabe-Zuschnitt mit ID #{} gefunden.", id.0),
        }
    }

    fn position_dimensions_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Position / dimensions",
            UiLanguage::German => "Position / Maße",
        }
    }

    fn placed_heading(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Placed",
            UiLanguage::German => "Platziert",
        }
    }

    fn pattern_value_prefix(self) -> &'static str {
        match self.language {
            UiLanguage::English => "Pattern",
            UiLanguage::German => "Maserung",
        }
    }

    fn rotated_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "rotated",
            UiLanguage::German => "rotiert",
        }
    }

    fn not_rotated_label(self) -> &'static str {
        match self.language {
            UiLanguage::English => "not rotated",
            UiLanguage::German => "nicht rotiert",
        }
    }
}

#[derive(Debug, Default)]
struct FreecutApp {
    state: FreecutAppState,
}

impl eframe::App for FreecutApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        apply_ui_font_size(context, self.state.font_size);

        egui::TopBottomPanel::bottom("status_panel").show(context, |ui| {
            self.status_panel(ui);
        });

        egui::SidePanel::left("project_panel")
            .resizable(true)
            .default_width(520.0)
            .show(context, |ui| {
                self.project_panel(ui);
            });

        egui::CentralPanel::default().show(context, |ui| {
            self.solution_panel(ui);
        });
    }
}

impl FreecutApp {
    fn project_panel(&mut self, ui: &mut egui::Ui) {
        let texts = UiTexts::new(self.state.language);

        ui.heading(texts.project_panel_heading());
        ui.separator();
        ui.label(format!(
            "{}: {}",
            texts.project_label(),
            self.state.project.name
        ));

        egui::ScrollArea::vertical().show(ui, |ui| {
            self.project_settings_editor(ui);
            ui.separator();
            self.project_file_panel(ui);
            ui.separator();
            self.csv_import_panel(ui);
            ui.separator();
            self.stock_editor(ui);
            ui.separator();
            self.cut_editor(ui);
        });

        sync_validation_error(&mut self.state);
    }

    fn project_settings_editor(&mut self, ui: &mut egui::Ui) {
        let texts = UiTexts::new(self.state.language);

        ui.heading(texts.project_settings_heading());

        let mut changed = false;

        egui::Grid::new("project_settings_editor")
            .num_columns(2)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                ui.label(texts.unit_setting_label());
                changed |= unit_combo(
                    ui,
                    "project_unit",
                    &mut self.state.project.settings.unit,
                    texts,
                );
                ui.end_row();

                ui.label(texts.kerf_width_label());
                changed |= ui
                    .add(dimension_drag_value(
                        &mut self.state.project.settings.kerf_width,
                    ))
                    .changed();
                ui.end_row();

                ui.label(texts.layout_setting_label());
                changed |= layout_combo(
                    ui,
                    "project_layout",
                    &mut self.state.project.settings.layout,
                );
                ui.end_row();

                ui.label(texts.optimizer_effort_label());
                changed |= effort_combo(
                    ui,
                    "optimizer_effort",
                    &mut self.state.optimizer_effort,
                    texts,
                );
                ui.end_row();

                ui.label(texts.language_label());
                language_combo(ui, "ui_language", &mut self.state.language);
                ui.end_row();

                ui.label(texts.font_size_setting_label());
                if font_size_combo(ui, "ui_font_size", &mut self.state.font_size, texts) {
                    apply_ui_font_size(ui.ctx(), self.state.font_size);
                }
                ui.end_row();
            });

        ui.small(texts.layout_help());

        if changed {
            mark_dirty(&mut self.state);
        }
    }

    fn project_file_panel(&mut self, ui: &mut egui::Ui) {
        let texts = UiTexts::new(self.state.language);

        ui.heading(texts.project_file_heading());
        ui.horizontal(|ui| {
            ui.label(texts.path_label());
            ui.text_edit_singleline(&mut self.state.project_path);
            if ui.button(texts.save_project_button()).clicked() {
                save_project_from_state_path(&mut self.state);
            }
            if ui.button(texts.load_project_button()).clicked() {
                load_project_from_state_path(&mut self.state);
            }
        });
        ui.small(texts.native_project_file_hint());

        if let Some(report) = &self.state.project_io_report {
            ui.label(report);
        }
    }

    fn csv_import_panel(&mut self, ui: &mut egui::Ui) {
        let texts = UiTexts::new(self.state.language);

        ui.heading(texts.csv_import_heading());
        ui.horizontal(|ui| {
            ui.label(texts.path_label());
            ui.text_edit_singleline(&mut self.state.csv_import_path);
            if ui.button(texts.csv_import_button()).clicked() {
                import_csv_from_state_path(&mut self.state);
            }
        });
        ui.small(texts.csv_import_hint());

        if let Some(report) = &self.state.csv_import_report {
            ui.label(report);
        }
    }

    fn stock_editor(&mut self, ui: &mut egui::Ui) {
        let texts = UiTexts::new(self.state.language);

        ui.horizontal(|ui| {
            ui.heading(texts.stock_editor_heading());
            if ui.button(texts.add_stock_button()).clicked() {
                add_default_stock_piece(&mut self.state);
            }
        });

        if self.state.project.stock_pieces.is_empty() {
            ui.label(texts.no_stock_pieces_label());
            return;
        }

        let mut remove_index = None;
        let mut any_changed = false;

        egui::Grid::new("stock_piece_editor")
            .striped(true)
            .num_columns(7)
            .show(ui, |ui| {
                ui.strong("ID");
                ui.strong(texts.width_label());
                ui.strong(texts.length_label());
                ui.strong(texts.quantity_label());
                ui.strong(texts.pattern_setting_label());
                ui.strong(texts.status_label());
                ui.strong("");
                ui.end_row();

                for (index, stock) in self.state.project.stock_pieces.iter_mut().enumerate() {
                    if ui
                        .selectable_value(
                            &mut self.state.selection.stock_index,
                            Some(index),
                            stock.id.0.to_string(),
                        )
                        .clicked()
                    {
                        self.state.selection.cut_index = None;
                        self.state.selection.placed_piece_index = None;
                    }

                    let mut changed = false;
                    changed |= ui
                        .add(dimension_drag_value(&mut stock.width).prefix(texts.width_prefix()))
                        .changed();
                    changed |= ui
                        .add(dimension_drag_value(&mut stock.length).prefix(texts.length_prefix()))
                        .changed();

                    let mut quantity = stock.quantity.unwrap_or(0);
                    if ui
                        .add(quantity_drag_value(&mut quantity).prefix(texts.quantity_prefix()))
                        .changed()
                    {
                        stock.quantity = Some(quantity);
                        changed = true;
                    }
                    changed |=
                        pattern_combo(ui, ("stock_pattern", index), &mut stock.pattern, texts);

                    validation_label(ui, stock_validation_message(stock, texts));

                    if changed {
                        self.state.selection.stock_index = Some(index);
                        self.state.selection.cut_index = None;
                        self.state.selection.placed_piece_index = None;
                    }

                    if ui.button(texts.delete_button()).clicked() {
                        remove_index = Some(index);
                    }

                    any_changed |= changed;
                    ui.end_row();
                }
            });

        if any_changed {
            mark_dirty(&mut self.state);
        }

        if let Some(index) = remove_index {
            remove_stock_piece(&mut self.state, index);
        }
    }

    fn cut_editor(&mut self, ui: &mut egui::Ui) {
        let texts = UiTexts::new(self.state.language);

        ui.horizontal(|ui| {
            ui.heading(texts.cut_editor_heading());
            if ui.button(texts.add_cut_button()).clicked() {
                add_default_cut_piece(&mut self.state);
            }
        });

        if self.state.project.cut_pieces.is_empty() {
            ui.label(texts.no_cut_pieces_label());
            return;
        }

        let mut remove_index = None;
        let mut any_changed = false;

        egui::Grid::new("cut_piece_editor")
            .striped(true)
            .num_columns(9)
            .show(ui, |ui| {
                ui.strong("ID");
                ui.strong(texts.label_column_label());
                ui.strong(texts.width_label());
                ui.strong(texts.length_label());
                ui.strong(texts.quantity_label());
                ui.strong(texts.pattern_setting_label());
                ui.strong(texts.rotation_label());
                ui.strong(texts.status_label());
                ui.strong("");
                ui.end_row();

                for (index, cut) in self.state.project.cut_pieces.iter_mut().enumerate() {
                    if ui
                        .selectable_value(
                            &mut self.state.selection.cut_index,
                            Some(index),
                            cut.id.0.to_string(),
                        )
                        .clicked()
                    {
                        self.state.selection.stock_index = None;
                        self.state.selection.placed_piece_index = None;
                    }

                    let mut changed = false;
                    changed |= ui.text_edit_singleline(&mut cut.label).changed();
                    changed |= ui
                        .add(dimension_drag_value(&mut cut.width).prefix(texts.width_prefix()))
                        .changed();
                    changed |= ui
                        .add(dimension_drag_value(&mut cut.length).prefix(texts.length_prefix()))
                        .changed();
                    changed |= ui
                        .add(quantity_drag_value(&mut cut.quantity).prefix(texts.quantity_prefix()))
                        .changed();
                    changed |= pattern_combo(ui, ("cut_pattern", index), &mut cut.pattern, texts);
                    changed |= ui
                        .checkbox(&mut cut.can_rotate, texts.rotatable_label())
                        .changed();

                    validation_label(ui, cut_validation_message(cut, texts));

                    if changed {
                        self.state.selection.cut_index = Some(index);
                        self.state.selection.stock_index = None;
                        self.state.selection.placed_piece_index = None;
                    }

                    if ui.button(texts.delete_button()).clicked() {
                        remove_index = Some(index);
                    }

                    any_changed |= changed;
                    ui.end_row();
                }
            });

        if any_changed {
            mark_dirty(&mut self.state);
        }

        if let Some(index) = remove_index {
            remove_cut_piece(&mut self.state, index);
        }
    }

    fn solution_panel(&mut self, ui: &mut egui::Ui) {
        let texts = UiTexts::new(self.state.language);

        ui.heading(texts.solution_heading());
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button(texts.optimize_button()).clicked() {
                optimize_current_project(&mut self.state);
            }
            ui.label(format!(
                "{}: {}",
                texts.effort_status_label(),
                texts.effort_label(self.state.optimizer_effort)
            ));
            ui.small(texts.deterministic_hint());
        });
        ui.separator();
        self.solution_export_panel(ui);
        ui.separator();

        match &self.state.solution {
            Some(solution) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "{}: {}",
                        texts.sheets_count_label(),
                        solution.sheets.len()
                    ));
                    ui.separator();
                    ui.label(format!(
                        "{}: {}",
                        texts.layout_setting_label(),
                        UiTexts::layout_label(solution.layout)
                    ));
                    ui.separator();
                    match solution.fitness {
                        Some(fitness) => {
                            ui.label(format!("{}: {fitness:.3}", UiTexts::fitness_label()))
                        }
                        None => ui.label(format!(
                            "{}: {}",
                            UiTexts::fitness_label(),
                            texts.not_available_label()
                        )),
                    };
                });

                if solution.sheets.is_empty() {
                    ui.label(texts.no_solution_sheets_message());
                    return;
                }

                let highlighted_cut_id = selected_cut_id(&self.state);
                selected_piece_details(ui, &self.state, texts);
                let mut pending_graphic_selection = None;
                let sheet_view_max_height = dynamic_sheet_view_max_height(ui.available_height());

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (index, sheet) in solution.sheets.iter().enumerate() {
                        let selected = self.state.selection.sheet_index == Some(index);
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    selected,
                                    format!("{} {}", texts.sheet_label(), index + 1),
                                )
                                .clicked()
                            {
                                self.state.selection.sheet_index = Some(index);
                                self.state.selection.placed_piece_index = None;
                            }
                            ui.label(format!(
                                "{} #{} · {} x {} · {} {} · {} {}",
                                texts.stock_label(),
                                sheet.stock_id.0,
                                sheet.width,
                                sheet.length,
                                sheet.placed_pieces.len(),
                                texts.cuts_count_label(),
                                sheet.waste.len(),
                                texts.waste_count_label()
                            ));
                        });

                        if let Some(piece_index) = draw_solution_sheet(
                            ui,
                            sheet,
                            selected,
                            highlighted_cut_id,
                            sheet_view_max_height,
                            texts,
                            solution.layout,
                        ) {
                            pending_graphic_selection = Some((index, piece_index));
                        }
                        ui.add_space(12.0);
                    }
                });

                if let Some((sheet_index, piece_index)) = pending_graphic_selection {
                    select_placed_piece(&mut self.state, sheet_index, piece_index);
                }
            }
            None => {
                ui.label(texts.no_solution_yet_message());
            }
        }
    }

    fn solution_export_panel(&mut self, ui: &mut egui::Ui) {
        let texts = UiTexts::new(self.state.language);

        ui.heading(texts.pdf_export_heading());
        ui.horizontal(|ui| {
            ui.label(texts.path_label());
            ui.text_edit_singleline(&mut self.state.export_path);
            if ui.button(texts.pdf_export_button()).clicked() {
                export_solution_from_state_path(&mut self.state);
            }
        });
        ui.small(texts.pdf_export_hint());

        if let Some(report) = &self.state.export_report {
            ui.label(report);
        }
    }

    fn status_panel(&mut self, ui: &mut egui::Ui) {
        let texts = UiTexts::new(self.state.language);

        ui.horizontal_wrapped(|ui| {
            ui.label(project_save_state_label(&self.state, texts));
            ui.separator();

            let selection_summary = selection_summary(self.state.selection, texts);
            ui.label(selection_summary);
            ui.separator();

            match &self.state.error_message {
                Some(message) => {
                    ui.colored_label(ui.visuals().error_fg_color, message);
                }
                None => {
                    ui.label(texts.ready_status());
                }
            }
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SheetViewBounds {
    view_size: egui::Vec2,
    sheet_size: egui::Vec2,
    scale: f32,
}

fn draw_solution_sheet(
    ui: &mut egui::Ui,
    sheet: &SolutionSheet,
    selected: bool,
    highlighted_cut_id: Option<PieceId>,
    max_height: f32,
    texts: UiTexts,
    layout: LayoutKind,
) -> Option<usize> {
    let Some(bounds) = sheet_view_bounds(sheet, ui.available_width(), max_height) else {
        ui.colored_label(
            ui.visuals().error_fg_color,
            texts.invalid_sheet_dimensions_message(),
        );
        return None;
    };

    let (response, painter) = ui.allocate_painter(bounds.view_size, egui::Sense::click());
    let sheet_rect = egui::Rect::from_min_size(
        response.rect.min + egui::vec2(SHEET_VIEW_PADDING, SHEET_VIEW_PADDING),
        bounds.sheet_size,
    );

    painter.rect_filled(response.rect, 0.0, SHEET_VIEW_BACKGROUND_COLOR);
    painter.rect_filled(sheet_rect, 0.0, sheet_surface_color(sheet, layout));

    let draw_neutral_waste_boundaries = should_draw_neutral_waste_boundaries(sheet, layout);
    let draw_piece_boundaries = should_draw_piece_boundaries(sheet, layout);
    for waste in &sheet.waste {
        let waste_rect = solution_rect_to_screen_rect(*waste, sheet_rect, bounds.scale);
        painter.rect_filled(waste_rect, 0.0, SHEET_WASTE_COLOR);
        if draw_neutral_waste_boundaries {
            painter.rect_stroke(
                waste_rect,
                0.0,
                waste_boundary_stroke(),
                egui::StrokeKind::Inside,
            );
        }
    }

    for piece in &sheet.placed_pieces {
        draw_placed_piece(
            &painter,
            piece,
            sheet_rect,
            bounds.scale,
            highlighted_cut_id == Some(piece.cut_id),
            draw_piece_boundaries,
        );
    }

    draw_cut_kerf_preview(&painter, sheet, sheet_rect, bounds.scale, layout);

    let border_color = if selected {
        egui::Color32::from_rgb(80, 130, 240)
    } else {
        ui.visuals().widgets.noninteractive.fg_stroke.color
    };
    let border_width = if selected { 3.0 } else { 1.5 };
    painter.rect_stroke(
        sheet_rect,
        0.0,
        egui::Stroke::new(border_width, border_color),
        egui::StrokeKind::Inside,
    );

    if !response.clicked() {
        return None;
    }

    let position = response.interact_pointer_pos()?;

    hit_test_placed_piece(sheet, sheet_rect, bounds.scale, position)
}

fn should_draw_neutral_waste_boundaries(sheet: &SolutionSheet, layout: LayoutKind) -> bool {
    layout == LayoutKind::Guillotine && sheet.cutting_guide.is_none()
}

fn should_draw_piece_boundaries(sheet: &SolutionSheet, layout: LayoutKind) -> bool {
    layout == LayoutKind::Guillotine && sheet.cutting_guide.is_none()
}

fn sheet_surface_color(sheet: &SolutionSheet, layout: LayoutKind) -> egui::Color32 {
    if should_paint_uncovered_kerf_base(sheet, layout) {
        CUT_KERF_COLOR
    } else {
        SHEET_SURFACE_COLOR
    }
}

fn should_paint_uncovered_kerf_base(sheet: &SolutionSheet, layout: LayoutKind) -> bool {
    layout == LayoutKind::Nested
        || (layout == LayoutKind::Guillotine && sheet.cutting_guide.is_some())
}

fn draw_placed_piece(
    painter: &egui::Painter,
    piece: &PlacedPiece,
    sheet_rect: egui::Rect,
    scale: f32,
    highlighted: bool,
    draw_boundary: bool,
) {
    let piece_rect = solution_rect_to_screen_rect(piece.rect, sheet_rect, scale);
    painter.rect_filled(piece_rect, 0.0, piece_fill_color(piece, highlighted));
    if draw_boundary {
        painter.rect_stroke(
            piece_rect,
            0.0,
            if highlighted {
                piece_highlight_stroke()
            } else {
                piece_boundary_stroke()
            },
            egui::StrokeKind::Inside,
        );
    }
    painter.text(
        piece_rect.center(),
        egui::Align2::CENTER_CENTER,
        piece_label(piece),
        egui::FontId::monospace(11.0),
        PIECE_LABEL_COLOR,
    );
}

fn draw_cut_kerf_preview(
    painter: &egui::Painter,
    sheet: &SolutionSheet,
    sheet_rect: egui::Rect,
    scale: f32,
    layout: LayoutKind,
) {
    let stroke = cut_kerf_stroke();

    for geometry in sheet_cut_preview_geometries(sheet, layout) {
        match geometry {
            CutPreviewGeometry::KerfRect(rect) => {
                let screen_rect = solution_rect_to_screen_rect(rect, sheet_rect, scale);
                painter.rect_filled(screen_rect, 0.0, cut_kerf_color());
                if let Some(line) = thin_kerf_rect_center_line(screen_rect) {
                    painter.line_segment(line, stroke);
                }
            }
            CutPreviewGeometry::ZeroKerfLine(line) => {
                painter.line_segment(
                    cut_preview_line_to_screen_points(line, sheet_rect, scale),
                    stroke,
                );
            }
        }
    }
}

fn dynamic_sheet_view_max_height(available_height: f32) -> f32 {
    if available_height.is_finite() && available_height > SHEET_VIEW_DEFAULT_MAX_HEIGHT {
        available_height
    } else {
        SHEET_VIEW_DEFAULT_MAX_HEIGHT
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "egui uses f32 screen coordinates; solution dimensions are display-only u32 values here"
)]
fn screen_units(value: u32) -> f32 {
    value as f32
}

fn sheet_view_bounds(
    sheet: &SolutionSheet,
    available_width: f32,
    max_height: f32,
) -> Option<SheetViewBounds> {
    if sheet.width == 0 || sheet.length == 0 {
        return None;
    }

    let drawable_width = (available_width - 2.0 * SHEET_VIEW_PADDING).max(1.0);
    let drawable_height = (max_height - 2.0 * SHEET_VIEW_PADDING).max(1.0);
    let sheet_width = screen_units(sheet.width);
    let sheet_length = screen_units(sheet.length);
    let scale = (drawable_width / sheet_width).min(drawable_height / sheet_length);

    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }

    let sheet_size = egui::vec2(sheet_width * scale, sheet_length * scale);
    let view_size = sheet_size + egui::vec2(2.0 * SHEET_VIEW_PADDING, 2.0 * SHEET_VIEW_PADDING);

    Some(SheetViewBounds {
        view_size,
        sheet_size,
        scale,
    })
}

fn solution_rect_to_screen_rect(
    rect: SolutionRect,
    sheet_rect: egui::Rect,
    scale: f32,
) -> egui::Rect {
    egui::Rect::from_min_size(
        sheet_rect.min + egui::vec2(screen_units(rect.x) * scale, screen_units(rect.y) * scale),
        egui::vec2(
            screen_units(rect.width) * scale,
            screen_units(rect.length) * scale,
        ),
    )
}

fn sheet_cut_preview_geometries(
    sheet: &SolutionSheet,
    layout: LayoutKind,
) -> Vec<CutPreviewGeometry> {
    solution_sheet_kerf_geometries(sheet, layout)
}

fn cut_preview_line_to_screen_points(
    line: CutPreviewLine,
    sheet_rect: egui::Rect,
    scale: f32,
) -> [egui::Pos2; 2] {
    [
        sheet_rect.min
            + egui::vec2(
                screen_units(line.start_x) * scale,
                screen_units(line.start_y) * scale,
            ),
        sheet_rect.min
            + egui::vec2(
                screen_units(line.end_x) * scale,
                screen_units(line.end_y) * scale,
            ),
    ]
}

fn thin_kerf_rect_center_line(screen_rect: egui::Rect) -> Option<[egui::Pos2; 2]> {
    if screen_rect.width() >= CUT_KERF_STROKE_WIDTH && screen_rect.height() >= CUT_KERF_STROKE_WIDTH
    {
        return None;
    }

    let center = screen_rect.center();
    if screen_rect.width() <= screen_rect.height() {
        Some([
            egui::pos2(center.x, screen_rect.top()),
            egui::pos2(center.x, screen_rect.bottom()),
        ])
    } else {
        Some([
            egui::pos2(screen_rect.left(), center.y),
            egui::pos2(screen_rect.right(), center.y),
        ])
    }
}

fn cut_kerf_stroke() -> egui::Stroke {
    egui::Stroke::new(CUT_KERF_STROKE_WIDTH, CUT_KERF_COLOR)
}

fn cut_kerf_color() -> egui::Color32 {
    CUT_KERF_COLOR
}

fn hit_test_placed_piece(
    sheet: &SolutionSheet,
    sheet_rect: egui::Rect,
    scale: f32,
    position: egui::Pos2,
) -> Option<usize> {
    sheet
        .placed_pieces
        .iter()
        .enumerate()
        .rev()
        .find(|(_, piece)| {
            solution_rect_to_screen_rect(piece.rect, sheet_rect, scale).contains(position)
        })
        .map(|(index, _)| index)
}

fn piece_label(piece: &PlacedPiece) -> String {
    format!("#{}-{}", piece.cut_id.0, piece.instance + 1)
}

fn piece_fill_color(piece: &PlacedPiece, highlighted: bool) -> egui::Color32 {
    if highlighted {
        PIECE_HIGHLIGHT_FILL_COLOR
    } else if piece.rotated {
        PIECE_ROTATED_FILL_COLOR
    } else {
        PIECE_NORMAL_FILL_COLOR
    }
}

fn piece_highlight_stroke() -> egui::Stroke {
    egui::Stroke::new(2.5, PIECE_HIGHLIGHT_STROKE_COLOR)
}

fn piece_boundary_stroke() -> egui::Stroke {
    egui::Stroke::new(SOLUTION_BOUNDARY_STROKE_WIDTH, PIECE_BOUNDARY_STROKE_COLOR)
}

fn waste_boundary_stroke() -> egui::Stroke {
    egui::Stroke::new(SOLUTION_BOUNDARY_STROKE_WIDTH, SHEET_WASTE_STROKE_COLOR)
}

#[derive(Debug, Clone, Copy)]
struct SelectedPieceDetails<'a> {
    sheet_index: usize,
    piece_index: usize,
    placed_piece: &'a PlacedPiece,
    cut_piece: Option<&'a CutPiece>,
}

fn selected_piece_details(ui: &mut egui::Ui, state: &FreecutAppState, texts: UiTexts) {
    let Some(details) = selected_placed_piece_details(state) else {
        if selected_cut_id(state).is_some() {
            ui.label(texts.selected_input_cut_hint());
        } else {
            ui.label(texts.click_solution_piece_hint());
        }
        ui.separator();
        return;
    };

    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.strong(texts.selected_cut_piece_heading());
        ui.separator();

        if let Some(cut) = details.cut_piece {
            egui::Grid::new("selected_piece_input_details")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label(texts.input_id_label_heading());
                    ui.label(format!("#{} · {}", cut.id.0, cut.label));
                    ui.end_row();

                    ui.label(texts.input_dimensions_quantity_heading());
                    ui.label(format!(
                        "{} x {} · {} {}",
                        cut.width,
                        cut.length,
                        texts.quantity_value_label(),
                        cut.quantity
                    ));
                    ui.end_row();

                    ui.label(texts.pattern_rotation_heading());
                    ui.label(format!(
                        "{} · {}",
                        texts.pattern_label(cut.pattern),
                        if cut.can_rotate {
                            texts.rotatable_label()
                        } else {
                            texts.fixed_rotation_label()
                        }
                    ));
                    ui.end_row();
                });
        } else {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                texts.missing_input_cut_piece_message(details.placed_piece.cut_id),
            );
        }

        egui::Grid::new("selected_piece_solution_details")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.label(texts.solution_heading());
                ui.label(format!(
                    "{} {} · {} {} · {}",
                    texts.sheet_label(),
                    details.sheet_index + 1,
                    texts.solution_piece_label(),
                    details.piece_index + 1,
                    piece_label(details.placed_piece)
                ));
                ui.end_row();

                ui.label(texts.position_dimensions_heading());
                ui.label(format!(
                    "x {}, y {}, {} x {}",
                    details.placed_piece.rect.x,
                    details.placed_piece.rect.y,
                    details.placed_piece.rect.width,
                    details.placed_piece.rect.length
                ));
                ui.end_row();

                ui.label(texts.placed_heading());
                ui.label(format!(
                    "{} {} · {}",
                    texts.pattern_value_prefix(),
                    texts.pattern_label(details.placed_piece.pattern),
                    if details.placed_piece.rotated {
                        texts.rotated_label()
                    } else {
                        texts.not_rotated_label()
                    }
                ));
                ui.end_row();
            });
    });
    ui.separator();
}

fn selected_placed_piece_details(state: &FreecutAppState) -> Option<SelectedPieceDetails<'_>> {
    let solution = state.solution.as_ref()?;
    let sheet_index = state.selection.sheet_index?;
    let piece_index = state.selection.placed_piece_index?;
    let placed_piece = solution
        .sheets
        .get(sheet_index)?
        .placed_pieces
        .get(piece_index)?;
    let cut_piece = state
        .project
        .cut_pieces
        .iter()
        .find(|cut| cut.id == placed_piece.cut_id);

    Some(SelectedPieceDetails {
        sheet_index,
        piece_index,
        placed_piece,
        cut_piece,
    })
}

fn select_placed_piece(state: &mut FreecutAppState, sheet_index: usize, piece_index: usize) {
    let Some(cut_id) = state
        .solution
        .as_ref()
        .and_then(|solution| solution.sheets.get(sheet_index))
        .and_then(|sheet| sheet.placed_pieces.get(piece_index))
        .map(|piece| piece.cut_id)
    else {
        return;
    };

    state.selection.sheet_index = Some(sheet_index);
    state.selection.placed_piece_index = Some(piece_index);
    state.selection.stock_index = None;
    state.selection.cut_index = state
        .project
        .cut_pieces
        .iter()
        .position(|cut| cut.id == cut_id);
}

fn selected_cut_id(state: &FreecutAppState) -> Option<PieceId> {
    state
        .selection
        .cut_index
        .and_then(|index| state.project.cut_pieces.get(index))
        .map(|cut| cut.id)
}

fn select_stock_input_row(state: &mut FreecutAppState, index: usize) {
    if index < state.project.stock_pieces.len() {
        state.selection.stock_index = Some(index);
        state.selection.cut_index = None;
        state.selection.placed_piece_index = None;
    }
}

fn select_cut_input_row(state: &mut FreecutAppState, index: usize) {
    if index < state.project.cut_pieces.len() {
        state.selection.cut_index = Some(index);
        state.selection.stock_index = None;
        state.selection.placed_piece_index = None;
    }
}

fn save_project_from_state_path(state: &mut FreecutAppState) {
    let texts = UiTexts::new(state.language);
    let path = state.project_path.trim();
    if path.is_empty() {
        let message = texts.project_file_empty_save_path_message().to_string();
        state.project_io_report = Some(message.clone());
        state.error_message = Some(message);
        return;
    }

    let document = ProjectDocument::new(state.project.clone(), state.optimizer_effort);
    match save_project_file(path, &document) {
        Ok(()) => {
            let message = texts.project_file_saved_message(path);
            state.project_io_report = Some(message);
            state.error_message = None;
            state.dirty = false;
        }
        Err(error) => {
            let message = texts.project_file_save_failed_message(error);
            state.project_io_report = Some(message.clone());
            state.error_message = Some(message);
        }
    }
}

fn load_project_from_state_path(state: &mut FreecutAppState) {
    let texts = UiTexts::new(state.language);
    let path = state.project_path.trim();
    if path.is_empty() {
        let message = texts.project_file_empty_load_path_message().to_string();
        state.project_io_report = Some(message.clone());
        state.error_message = Some(message);
        return;
    }

    match load_project_file(path) {
        Ok(document) => {
            state.project = document.project;
            state.optimizer_effort = document.optimizer_effort;
            state.solution = None;
            state.selection = UiSelection::default();
            state.error_message = None;
            state.dirty = false;
            state.project_io_report = Some(texts.project_file_loaded_message(path));
            state.csv_import_report = None;
        }
        Err(error) => {
            let message = texts.project_file_load_failed_message(error);
            state.project_io_report = Some(message.clone());
            state.error_message = Some(message);
        }
    }
}

fn export_solution_from_state_path(state: &mut FreecutAppState) {
    let texts = UiTexts::new(state.language);
    let path = state.export_path.trim();
    if path.is_empty() {
        let message = texts.pdf_export_empty_path_message().to_string();
        state.export_report = Some(message.clone());
        state.error_message = Some(message);
        return;
    }

    let Some(solution) = state.solution.as_ref() else {
        let message = texts.pdf_export_missing_solution_message().to_string();
        state.export_report = Some(message.clone());
        state.error_message = Some(message);
        return;
    };

    match export_solution_pdf_file(path, solution, state.project.settings.unit) {
        Ok(()) => {
            let message = texts.pdf_exported_message(path);
            state.export_report = Some(message);
            state.error_message = None;
        }
        Err(error) => {
            let message = texts.pdf_export_failed_message(error);
            state.export_report = Some(message.clone());
            state.error_message = Some(message);
        }
    }
}

fn import_csv_from_state_path(state: &mut FreecutAppState) {
    let texts = UiTexts::new(state.language);
    let path = state.csv_import_path.trim();
    if path.is_empty() {
        let message = texts.csv_import_empty_path_message().to_string();
        state.csv_import_report = Some(message.clone());
        state.error_message = Some(message);
        return;
    }

    match import_project_csv_file(path, next_piece_id(&state.project).0) {
        Ok(result) => apply_csv_import_result(state, result),
        Err(error) => {
            let message = texts.csv_import_read_failed_message(error);
            state.csv_import_report = Some(message.clone());
            state.error_message = Some(message);
        }
    }
}

fn apply_csv_import_result(state: &mut FreecutAppState, result: CsvImportResult) {
    let imported_count = result.imported_count();
    let has_errors = result.has_errors();
    let summary = UiTexts::new(state.language).csv_import_summary(&result);

    if imported_count > 0 {
        state.project.stock_pieces.extend(result.stock_pieces);
        state.project.cut_pieces.extend(result.cut_pieces);
        mark_dirty(state);
    }

    state.csv_import_report = Some(summary.clone());
    state.error_message = has_errors.then_some(summary);
}

fn sync_validation_error(state: &mut FreecutAppState) {
    let texts = UiTexts::new(state.language);

    match validation_report(&state.project).summary(texts) {
        Some(summary) => state.error_message = Some(summary),
        None if state
            .error_message
            .as_deref()
            .is_some_and(is_validation_error_message) =>
        {
            state.error_message = None;
        }
        None => {}
    }
}

fn optimize_current_project(state: &mut FreecutAppState) {
    let texts = UiTexts::new(state.language);
    let report = validation_report(&state.project);
    if !report.is_valid() {
        state.solution = None;
        state.error_message = report.summary(texts);
        return;
    }

    if state.project.stock_pieces.is_empty() || state.project.cut_pieces.is_empty() {
        state.solution = None;
        state.error_message = Some(texts.empty_optimize_input_message().to_string());
        return;
    }

    match BaselineOptimizer
        .optimize_with_config(&state.project, OptimizerConfig::new(state.optimizer_effort))
    {
        Ok(solution) => {
            state.solution = Some(solution);
            state.error_message = None;
            state.selection.sheet_index = Some(0);
        }
        Err(error) => {
            state.solution = None;
            state.error_message = Some(texts.optimize_error_message(error));
        }
    }
}

fn add_default_stock_piece(state: &mut FreecutAppState) {
    let id = next_piece_id(&state.project);
    state.project.stock_pieces.push(StockPiece {
        id,
        width: 2440,
        length: 1220,
        quantity: Some(1),
        pattern: PatternDirection::None,
    });
    select_stock_input_row(state, state.project.stock_pieces.len() - 1);
    mark_dirty(state);
}

fn add_default_cut_piece(state: &mut FreecutAppState) {
    let id = next_piece_id(&state.project);
    state.project.cut_pieces.push(CutPiece {
        id,
        label: format!("cut-{}", id.0),
        width: 100,
        length: 100,
        quantity: 1,
        pattern: PatternDirection::None,
        can_rotate: true,
    });
    select_cut_input_row(state, state.project.cut_pieces.len() - 1);
    mark_dirty(state);
}

fn remove_stock_piece(state: &mut FreecutAppState, index: usize) {
    if index < state.project.stock_pieces.len() {
        state.project.stock_pieces.remove(index);
        state.selection.stock_index = previous_valid_index(state.project.stock_pieces.len(), index);
        state.selection.placed_piece_index = None;
        mark_dirty(state);
    }
}

fn remove_cut_piece(state: &mut FreecutAppState, index: usize) {
    if index < state.project.cut_pieces.len() {
        state.project.cut_pieces.remove(index);
        state.selection.cut_index = previous_valid_index(state.project.cut_pieces.len(), index);
        state.selection.placed_piece_index = None;
        mark_dirty(state);
    }
}

fn previous_valid_index(len: usize, removed_index: usize) -> Option<usize> {
    if len == 0 {
        None
    } else {
        Some(removed_index.min(len - 1))
    }
}

fn mark_dirty(state: &mut FreecutAppState) {
    state.dirty = true;
    state.solution = None;
    state.export_report = None;
    state.selection.sheet_index = None;
    state.selection.placed_piece_index = None;
    if state
        .error_message
        .as_deref()
        .is_some_and(|message| !is_validation_error_message(message))
    {
        state.error_message = None;
    }
}

fn is_validation_error_message(message: &str) -> bool {
    message.starts_with(VALIDATION_ERROR_PREFIX_EN)
        || message.starts_with(VALIDATION_ERROR_PREFIX_DE)
}

fn next_piece_id(project: &Project) -> PieceId {
    let max_stock_id = project
        .stock_pieces
        .iter()
        .map(|piece| piece.id.0)
        .max()
        .unwrap_or(0);
    let max_cut_id = project
        .cut_pieces
        .iter()
        .map(|piece| piece.id.0)
        .max()
        .unwrap_or(0);

    PieceId(max_stock_id.max(max_cut_id) + 1)
}

fn dimension_drag_value(value: &mut u32) -> egui::DragValue<'_> {
    egui::DragValue::new(value)
        .speed(1.0)
        .range(0..=MAX_DIMENSION)
}

fn quantity_drag_value(value: &mut u32) -> egui::DragValue<'_> {
    egui::DragValue::new(value)
        .speed(1.0)
        .range(0..=MAX_QUANTITY)
}

fn unit_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    unit: &mut Unit,
    texts: UiTexts,
) -> bool {
    let before = *unit;
    egui::ComboBox::from_id_salt(id)
        .selected_text(texts.unit_label(*unit))
        .show_ui(ui, |ui| {
            ui.selectable_value(unit, Unit::Millimeter, texts.unit_label(Unit::Millimeter));
            ui.selectable_value(unit, Unit::Inch, texts.unit_label(Unit::Inch));
            ui.selectable_value(unit, Unit::Foot, texts.unit_label(Unit::Foot));
        });

    *unit != before
}

fn pattern_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    pattern: &mut PatternDirection,
    texts: UiTexts,
) -> bool {
    let before = *pattern;
    egui::ComboBox::from_id_salt(id)
        .selected_text(texts.pattern_label(*pattern))
        .show_ui(ui, |ui| {
            ui.selectable_value(
                pattern,
                PatternDirection::None,
                texts.pattern_label(PatternDirection::None),
            );
            ui.selectable_value(
                pattern,
                PatternDirection::ParallelToWidth,
                texts.pattern_label(PatternDirection::ParallelToWidth),
            );
            ui.selectable_value(
                pattern,
                PatternDirection::ParallelToLength,
                texts.pattern_label(PatternDirection::ParallelToLength),
            );
        });

    *pattern != before
}

fn layout_combo(ui: &mut egui::Ui, id: impl std::hash::Hash, layout: &mut LayoutKind) -> bool {
    let before = *layout;
    egui::ComboBox::from_id_salt(id)
        .selected_text(UiTexts::layout_label(*layout))
        .show_ui(ui, |ui| {
            for option in available_layouts() {
                ui.selectable_value(layout, option, UiTexts::layout_label(option));
            }
        });

    *layout != before
}

fn available_layouts() -> [LayoutKind; 2] {
    [LayoutKind::Guillotine, LayoutKind::Nested]
}

fn effort_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    effort: &mut OptimizerEffort,
    texts: UiTexts,
) -> bool {
    let before = *effort;
    egui::ComboBox::from_id_salt(id)
        .selected_text(texts.effort_label(*effort))
        .show_ui(ui, |ui| {
            ui.selectable_value(
                effort,
                OptimizerEffort::Fast,
                texts.effort_label(OptimizerEffort::Fast),
            );
            ui.selectable_value(
                effort,
                OptimizerEffort::Balanced,
                texts.effort_label(OptimizerEffort::Balanced),
            );
            ui.selectable_value(
                effort,
                OptimizerEffort::Thorough,
                texts.effort_label(OptimizerEffort::Thorough),
            );
        });

    *effort != before
}

fn font_size_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    font_size: &mut UiFontSize,
    texts: UiTexts,
) -> bool {
    let before = *font_size;
    egui::ComboBox::from_id_salt(id)
        .selected_text(texts.font_size_label(*font_size))
        .show_ui(ui, |ui| {
            for option in available_font_sizes() {
                ui.selectable_value(font_size, option, texts.font_size_label(option));
            }
        });

    *font_size != before
}

fn available_font_sizes() -> [UiFontSize; 3] {
    [UiFontSize::Compact, UiFontSize::Normal, UiFontSize::Large]
}

fn language_combo(ui: &mut egui::Ui, id: impl std::hash::Hash, language: &mut UiLanguage) -> bool {
    let before = *language;
    egui::ComboBox::from_id_salt(id)
        .selected_text(language_label(*language))
        .show_ui(ui, |ui| {
            for option in available_languages() {
                ui.selectable_value(language, option, language_label(option));
            }
        });

    *language != before
}

fn available_languages() -> [UiLanguage; 2] {
    [UiLanguage::English, UiLanguage::German]
}

fn apply_ui_font_size(context: &egui::Context, font_size: UiFontSize) {
    let mut style = (*context.style()).clone();
    set_text_style_size(
        &mut style,
        egui::TextStyle::Heading,
        font_size.heading_points(),
    );
    set_text_style_size(&mut style, egui::TextStyle::Body, font_size.body_points());
    set_text_style_size(&mut style, egui::TextStyle::Button, font_size.body_points());
    set_text_style_size(
        &mut style,
        egui::TextStyle::Monospace,
        font_size.monospace_points(),
    );
    set_text_style_size(&mut style, egui::TextStyle::Small, font_size.small_points());
    context.set_style(style);
}

fn set_text_style_size(style: &mut egui::Style, text_style: egui::TextStyle, size: f32) {
    if let Some(font_id) = style.text_styles.get_mut(&text_style) {
        font_id.size = size;
    } else {
        style
            .text_styles
            .insert(text_style, egui::FontId::proportional(size));
    }
}

fn validation_label(ui: &mut egui::Ui, message: Option<&'static str>) {
    match message {
        Some(message) => {
            ui.colored_label(ui.visuals().error_fg_color, message);
        }
        None => {
            ui.label(UiTexts::validation_ok_label());
        }
    }
}

fn stock_validation_message(stock: &StockPiece, texts: UiTexts) -> Option<&'static str> {
    if stock_has_invalid_dimensions(stock) {
        Some(texts.dimension_validation_message())
    } else if stock_has_invalid_quantity(stock) {
        Some(texts.quantity_validation_message())
    } else {
        None
    }
}

fn cut_validation_message(cut: &CutPiece, texts: UiTexts) -> Option<&'static str> {
    if cut.label.trim().is_empty() {
        Some(texts.missing_label_validation_message())
    } else if cut_has_invalid_dimensions(cut) {
        Some(texts.dimension_validation_message())
    } else if cut_has_invalid_quantity(cut) {
        Some(texts.quantity_validation_message())
    } else {
        None
    }
}

fn stock_has_invalid_dimensions(stock: &StockPiece) -> bool {
    stock.width == 0 || stock.length == 0
}

fn stock_has_invalid_quantity(stock: &StockPiece) -> bool {
    stock.quantity.unwrap_or(0) == 0
}

fn cut_has_invalid_dimensions(cut: &CutPiece) -> bool {
    cut.width == 0 || cut.length == 0
}

fn cut_has_invalid_quantity(cut: &CutPiece) -> bool {
    cut.quantity == 0
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ProjectValidationReport {
    project_messages: Vec<&'static str>,
    stock_errors: usize,
    cut_errors: usize,
}

impl ProjectValidationReport {
    fn is_valid(&self) -> bool {
        self.project_messages.is_empty() && self.stock_errors == 0 && self.cut_errors == 0
    }

    fn summary(&self, texts: UiTexts) -> Option<String> {
        if self.is_valid() {
            return None;
        }

        Some(match texts.language {
            UiLanguage::English => format!(
                "{} {} project, {} stock piece(s), {} cut piece(s)",
                texts.validation_error_prefix(),
                self.project_messages.len(),
                self.stock_errors,
                self.cut_errors
            ),
            UiLanguage::German => format!(
                "{} {} Projekt, {} Rohteil(e), {} Zuschnitt(e)",
                texts.validation_error_prefix(),
                self.project_messages.len(),
                self.stock_errors,
                self.cut_errors
            ),
        })
    }
}

fn validation_report(project: &Project) -> ProjectValidationReport {
    ProjectValidationReport {
        project_messages: project_validation_messages(project),
        stock_errors: project
            .stock_pieces
            .iter()
            .filter(|stock| {
                stock_has_invalid_dimensions(stock) || stock_has_invalid_quantity(stock)
            })
            .count(),
        cut_errors: project
            .cut_pieces
            .iter()
            .filter(|cut| {
                cut.label.trim().is_empty()
                    || cut_has_invalid_dimensions(cut)
                    || cut_has_invalid_quantity(cut)
            })
            .count(),
    }
}

fn project_validation_messages(_project: &Project) -> Vec<&'static str> {
    Vec::new()
}

fn empty_project() -> Project {
    Project {
        name: "Untitled Freecut Project".to_string(),
        stock_pieces: Vec::new(),
        cut_pieces: Vec::new(),
        settings: CutSettings {
            unit: Unit::Millimeter,
            kerf_width: 3,
            layout: LayoutKind::Guillotine,
        },
    }
}

fn language_label(language: UiLanguage) -> &'static str {
    match language {
        UiLanguage::English => "English",
        UiLanguage::German => "Deutsch",
    }
}

impl UiFontSize {
    fn heading_points(self) -> f32 {
        match self {
            Self::Compact => 17.0,
            Self::Normal => 20.0,
            Self::Large => 24.0,
        }
    }

    fn body_points(self) -> f32 {
        match self {
            Self::Compact => 12.0,
            Self::Normal => 14.0,
            Self::Large => 17.0,
        }
    }

    fn monospace_points(self) -> f32 {
        match self {
            Self::Compact => 11.0,
            Self::Normal => 13.0,
            Self::Large => 16.0,
        }
    }

    fn small_points(self) -> f32 {
        match self {
            Self::Compact => 10.0,
            Self::Normal => 11.0,
            Self::Large => 14.0,
        }
    }
}

fn project_save_state_label(state: &FreecutAppState, texts: UiTexts) -> &'static str {
    if state.dirty {
        texts.changed_project_label()
    } else if state.project_path.trim().is_empty() {
        texts.unsaved_project_label()
    } else {
        texts.saved_project_label()
    }
}

fn selection_summary(selection: UiSelection, texts: UiTexts) -> String {
    let mut parts = Vec::new();

    if let Some(index) = selection.stock_index {
        parts.push(format!("{} {}", texts.stock_label(), index + 1));
    }
    if let Some(index) = selection.cut_index {
        parts.push(format!("{} {}", texts.cut_piece_label(), index + 1));
    }
    if let Some(index) = selection.sheet_index {
        parts.push(format!("{} {}", texts.sheet_label(), index + 1));
    }
    if let Some(index) = selection.placed_piece_index {
        parts.push(format!("{} {}", texts.solution_piece_label(), index + 1));
    }

    if parts.is_empty() {
        texts.no_selection_label().to_string()
    } else {
        format!("{}: {}", texts.selection_prefix(), parts.join(" · "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{
        cut_kerf_geometry as cut_preview_geometry, Cut as SolutionCut, CutOrientation,
        SliceNode as SolutionSliceNode,
    };

    #[test]
    fn app_state_starts_without_solution_error_or_dirty_flag() {
        let state = FreecutAppState::default();

        assert_eq!(state.project.name, "Untitled Freecut Project");
        assert!(state.project.stock_pieces.is_empty());
        assert!(state.project.cut_pieces.is_empty());
        assert_eq!(state.project.settings.layout, LayoutKind::Guillotine);
        assert_eq!(state.project.settings.kerf_width, 3);
        assert_eq!(state.optimizer_effort, OptimizerEffort::Fast);
        assert_eq!(state.font_size, UiFontSize::Normal);
        assert_eq!(state.language, UiLanguage::English);
        assert!(state.project_path.is_empty());
        assert!(state.project_io_report.is_none());
        assert!(state.export_path.is_empty());
        assert!(state.export_report.is_none());
        assert!(state.solution.is_none());
        assert!(state.error_message.is_none());
        assert!(!state.dirty);
    }

    #[test]
    fn default_selection_summary_is_explicit() {
        assert_eq!(
            selection_summary(UiSelection::default(), UiTexts::new(UiLanguage::English)),
            "No selection"
        );
        assert_eq!(
            selection_summary(UiSelection::default(), UiTexts::new(UiLanguage::German)),
            "Keine Auswahl"
        );
    }

    #[test]
    fn populated_selection_summary_is_user_readable_and_one_based() {
        assert_eq!(
            selection_summary(
                UiSelection {
                    stock_index: None,
                    cut_index: Some(1),
                    sheet_index: Some(0),
                    placed_piece_index: Some(2),
                },
                UiTexts::new(UiLanguage::English),
            ),
            "Selection: Cut piece 2 · Sheet 1 · Solution piece 3"
        );
        assert_eq!(
            selection_summary(
                UiSelection {
                    stock_index: None,
                    cut_index: Some(1),
                    sheet_index: Some(0),
                    placed_piece_index: Some(2),
                },
                UiTexts::new(UiLanguage::German),
            ),
            "Auswahl: Zuschnitt 2 · Sheet 1 · Lösungsteil 3"
        );
        assert_eq!(
            selection_summary(
                UiSelection {
                    stock_index: Some(0),
                    cut_index: None,
                    sheet_index: None,
                    placed_piece_index: None,
                },
                UiTexts::new(UiLanguage::German),
            ),
            "Auswahl: Rohteil 1"
        );
    }

    #[test]
    fn save_state_label_distinguishes_unsaved_clean_projects() {
        let mut state = FreecutAppState::default();
        let english = UiTexts::new(UiLanguage::English);
        let german = UiTexts::new(UiLanguage::German);

        assert_eq!(project_save_state_label(&state, english), "Unsaved");
        assert_eq!(
            project_save_state_label(&state, german),
            "Nicht gespeichert"
        );

        state.project_path = "demo.freecut.json".to_string();
        assert_eq!(project_save_state_label(&state, english), "Saved");
        assert_eq!(project_save_state_label(&state, german), "Gespeichert");

        state.dirty = true;
        assert_eq!(project_save_state_label(&state, english), "Changed");
        assert_eq!(project_save_state_label(&state, german), "Geändert");
    }

    #[test]
    fn adding_rows_assigns_unique_ids_and_marks_state_dirty() {
        let mut state = FreecutAppState::default();

        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);

        assert_eq!(state.project.stock_pieces[0].id, PieceId(1));
        assert_eq!(state.project.stock_pieces[0].quantity, Some(1));
        assert_eq!(state.project.cut_pieces[0].id, PieceId(2));
        assert_eq!(state.project.cut_pieces[0].label, "cut-2");
        assert!(state.project.cut_pieces[0].can_rotate);
        assert!(state.dirty);
        assert!(state.solution.is_none());
    }

    #[test]
    fn input_row_selection_is_mutually_exclusive_and_clears_stale_piece_selection() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        state.selection.placed_piece_index = Some(0);

        select_stock_input_row(&mut state, 0);

        assert_eq!(state.selection.stock_index, Some(0));
        assert_eq!(state.selection.cut_index, None);
        assert_eq!(state.selection.placed_piece_index, None);

        state.selection.placed_piece_index = Some(0);
        select_cut_input_row(&mut state, 0);

        assert_eq!(state.selection.stock_index, None);
        assert_eq!(state.selection.cut_index, Some(0));
        assert_eq!(state.selection.placed_piece_index, None);
    }

    #[test]
    fn dirtying_project_clears_stale_solution_and_export_report() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        optimize_current_project(&mut state);
        state.export_report = Some("PDF exportiert: old.pdf".to_string());

        mark_dirty(&mut state);

        assert!(state.solution.is_none());
        assert!(state.export_report.is_none());
        assert_eq!(state.selection.sheet_index, None);
        assert_eq!(state.selection.placed_piece_index, None);
    }

    #[test]
    fn removing_rows_keeps_selection_valid_for_empty_lists() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);

        remove_stock_piece(&mut state, 0);
        remove_cut_piece(&mut state, 0);

        assert!(state.project.stock_pieces.is_empty());
        assert!(state.project.cut_pieces.is_empty());
        assert_eq!(state.selection.stock_index, None);
        assert_eq!(state.selection.cut_index, None);
    }

    #[test]
    fn cut_piece_dimensions_can_be_modified_after_creation() {
        let mut state = FreecutAppState::default();
        add_default_cut_piece(&mut state);

        let cut = &mut state.project.cut_pieces[0];
        cut.width = 321;
        cut.length = 654;
        cut.quantity = 3;
        cut.label = "side panel".to_string();
        cut.can_rotate = false;

        assert_eq!(state.project.cut_pieces[0].width, 321);
        assert_eq!(state.project.cut_pieces[0].length, 654);
        assert_eq!(state.project.cut_pieces[0].quantity, 3);
        assert_eq!(state.project.cut_pieces[0].label, "side panel");
        assert!(!state.project.cut_pieces[0].can_rotate);
        assert!(validation_report(&state.project).is_valid());
    }

    #[test]
    fn project_parameters_are_domain_values_and_effort_stays_ui_state() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);

        state.project.settings.unit = Unit::Inch;
        state.project.settings.kerf_width = 3;
        state.project.stock_pieces[0].pattern = PatternDirection::ParallelToWidth;
        state.project.cut_pieces[0].pattern = PatternDirection::ParallelToLength;
        state.optimizer_effort = OptimizerEffort::Thorough;
        state.font_size = UiFontSize::Large;

        assert_eq!(state.project.settings.unit, Unit::Inch);
        assert_eq!(state.project.settings.kerf_width, 3);
        assert_eq!(state.project.settings.layout, LayoutKind::Guillotine);
        assert_eq!(
            state.project.stock_pieces[0].pattern,
            PatternDirection::ParallelToWidth
        );
        assert_eq!(
            state.project.cut_pieces[0].pattern,
            PatternDirection::ParallelToLength
        );
        assert_eq!(state.optimizer_effort, OptimizerEffort::Thorough);
        assert_eq!(state.font_size, UiFontSize::Large);
        assert!(validation_report(&state.project).is_valid());
    }

    #[test]
    fn ui_font_size_offers_three_non_project_settings() {
        let mut state = FreecutAppState::default();
        let english = UiTexts::new(UiLanguage::English);
        let german = UiTexts::new(UiLanguage::German);

        state.font_size = UiFontSize::Compact;

        assert_eq!(
            available_font_sizes(),
            [UiFontSize::Compact, UiFontSize::Normal, UiFontSize::Large]
        );
        assert_eq!(english.font_size_label(UiFontSize::Compact), "Compact");
        assert_eq!(english.font_size_label(UiFontSize::Normal), "Normal");
        assert_eq!(english.font_size_label(UiFontSize::Large), "Large");
        assert_eq!(german.font_size_label(UiFontSize::Compact), "Kompakt");
        assert_eq!(german.font_size_label(UiFontSize::Normal), "Normal");
        assert_eq!(german.font_size_label(UiFontSize::Large), "Groß");
        assert_eq!(state.project.settings.layout, LayoutKind::Guillotine);
        assert!(!state.dirty);
    }

    #[test]
    fn ui_font_size_updates_egui_text_style_sizes() {
        let context = egui::Context::default();

        apply_ui_font_size(&context, UiFontSize::Large);
        let style = context.style();

        assert_eq!(
            style
                .text_styles
                .get(&egui::TextStyle::Heading)
                .unwrap()
                .size,
            UiFontSize::Large.heading_points()
        );
        assert_eq!(
            style.text_styles.get(&egui::TextStyle::Body).unwrap().size,
            UiFontSize::Large.body_points()
        );
        assert_eq!(
            style.text_styles.get(&egui::TextStyle::Small).unwrap().size,
            UiFontSize::Large.small_points()
        );
    }

    #[test]
    fn ui_language_defaults_to_english_and_offers_german() {
        let state = FreecutAppState::default();

        assert_eq!(state.language, UiLanguage::English);
        assert_eq!(
            available_languages(),
            [UiLanguage::English, UiLanguage::German]
        );
        assert_eq!(language_label(UiLanguage::English), "English");
        assert_eq!(language_label(UiLanguage::German), "Deutsch");
    }

    #[test]
    fn ui_language_is_session_state_and_not_project_dirty_state() {
        let mut state = FreecutAppState::default();

        state.language = UiLanguage::German;

        assert_eq!(state.language, UiLanguage::German);
        assert_eq!(state.project.name, "Untitled Freecut Project");
        assert!(!state.dirty);
    }

    #[test]
    fn ui_texts_return_language_specific_values() {
        let english = UiTexts::new(UiLanguage::English);
        let german = UiTexts::new(UiLanguage::German);

        assert_eq!(english.project_panel_heading(), "Project / Input");
        assert_eq!(german.project_panel_heading(), "Projekt / Eingabe");
        assert_ne!(
            english.project_settings_heading(),
            german.project_settings_heading()
        );
        assert_ne!(english.language_label(), german.language_label());
        assert_eq!(english.unit_setting_label(), "Unit");
        assert_eq!(german.unit_setting_label(), "Einheit");
        assert_eq!(english.kerf_width_label(), "Kerf width");
        assert_eq!(german.kerf_width_label(), "Schnittfuge / Kerf");
        assert_eq!(english.optimize_button(), "Optimize");
        assert_eq!(german.optimize_button(), "Optimieren");
        assert_eq!(english.ready_status(), "Ready");
        assert_eq!(german.ready_status(), "Bereit");
        assert_eq!(english.effort_label(OptimizerEffort::Thorough), "Thorough");
        assert_eq!(german.effort_label(OptimizerEffort::Thorough), "Gründlich");
        assert_eq!(english.project_file_heading(), "Project file");
        assert_eq!(german.project_file_heading(), "Projektdatei");
        assert_eq!(english.csv_import_heading(), "CSV import");
        assert_eq!(german.csv_import_heading(), "CSV-Import");
        assert_eq!(english.stock_editor_heading(), "Stock pieces");
        assert_eq!(german.stock_editor_heading(), "Rohteile");
        assert_eq!(english.cut_editor_heading(), "Cut pieces");
        assert_eq!(german.cut_editor_heading(), "Zuschnitte");
        assert_eq!(english.pdf_export_heading(), "PDF export");
        assert_eq!(german.pdf_export_heading(), "PDF-Export");
        assert_eq!(english.solution_heading(), "Solution");
        assert_eq!(german.solution_heading(), "Lösung");
        assert_eq!(
            english.no_solution_yet_message(),
            "No solution calculated yet."
        );
        assert_eq!(
            german.no_solution_yet_message(),
            "Noch keine Lösung berechnet."
        );
        assert_eq!(english.stock_label(), "Stock");
        assert_eq!(german.stock_label(), "Rohteil");
        assert_eq!(english.cuts_count_label(), "cut(s)");
        assert_eq!(german.cuts_count_label(), "Zuschnitt(e)");
        assert_eq!(english.pattern_label(PatternDirection::None), "none");
        assert_eq!(german.pattern_label(PatternDirection::None), "keine");
        assert_eq!(english.rotated_label(), "rotated");
        assert_eq!(german.rotated_label(), "rotiert");
    }

    #[test]
    fn nested_layout_is_offered_when_backend_is_available() {
        let mut state = FreecutAppState::default();
        state.project.settings.layout = LayoutKind::Nested;

        let report = validation_report(&state.project);

        assert_eq!(
            available_layouts(),
            [LayoutKind::Guillotine, LayoutKind::Nested]
        );
        assert_eq!(UiTexts::layout_label(LayoutKind::Guillotine), "Guillotine");
        assert_eq!(UiTexts::layout_label(LayoutKind::Nested), "Nested");
        assert!(report.project_messages.is_empty());
        assert!(report.summary(UiTexts::new(UiLanguage::English)).is_none());
    }

    #[test]
    fn optimize_action_stores_solution_for_valid_project_with_selected_effort() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        state.optimizer_effort = OptimizerEffort::Balanced;

        optimize_current_project(&mut state);

        let solution = state
            .solution
            .expect("valid project should produce a solution");
        assert_eq!(solution.layout, LayoutKind::Guillotine);
        assert_eq!(solution.sheets.len(), 1);
        assert_eq!(solution.sheets[0].placed_pieces.len(), 1);
        assert!(solution.sheets[0].cutting_guide.is_some());
        assert!(state.error_message.is_none());
        assert_eq!(state.selection.sheet_index, Some(0));
    }

    #[test]
    fn optimize_action_stores_solution_for_nested_layout() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        state.project.settings.layout = LayoutKind::Nested;
        state.optimizer_effort = OptimizerEffort::Thorough;

        optimize_current_project(&mut state);

        let solution = state
            .solution
            .expect("valid nested project should produce a solution");
        assert_eq!(solution.layout, LayoutKind::Nested);
        assert_eq!(solution.sheets.len(), 1);
        assert_eq!(solution.sheets[0].placed_pieces.len(), 1);
        assert_eq!(solution.sheets[0].cutting_guide, None);
        assert!(state.error_message.is_none());
        assert_eq!(state.selection.sheet_index, Some(0));
    }

    #[test]
    fn changing_language_does_not_dirty_or_change_solution_data() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        optimize_current_project(&mut state);
        state.dirty = false;
        let solution_before = state.solution.clone();

        state.language = UiLanguage::German;

        assert!(!state.dirty);
        assert_eq!(state.solution, solution_before);
        assert_eq!(state.project.settings.layout, LayoutKind::Guillotine);
    }

    #[test]
    fn optimize_action_reports_validation_before_calling_optimizer() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        state.project.cut_pieces[0].quantity = 0;

        optimize_current_project(&mut state);

        assert!(state.solution.is_none());
        assert_eq!(
            state.error_message,
            Some("Input error: 0 project, 0 stock piece(s), 1 cut piece(s)".to_string())
        );
    }

    #[test]
    fn optimize_action_reports_no_solution_without_losing_message_on_validation_sync() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        state.project.stock_pieces[0].width = 50;
        state.project.stock_pieces[0].length = 50;
        state.project.cut_pieces[0].width = 100;
        state.project.cut_pieces[0].length = 100;

        optimize_current_project(&mut state);
        sync_validation_error(&mut state);

        assert!(state.solution.is_none());
        assert_eq!(
            state.error_message,
            Some("No solution found for the current input".to_string())
        );
    }

    #[test]
    fn optimize_action_uses_german_phase_two_status_text_when_selected() {
        let mut state = FreecutAppState {
            language: UiLanguage::German,
            ..FreecutAppState::default()
        };
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        state.project.stock_pieces[0].width = 50;
        state.project.stock_pieces[0].length = 50;
        state.project.cut_pieces[0].width = 100;
        state.project.cut_pieces[0].length = 100;

        optimize_current_project(&mut state);

        assert!(state.solution.is_none());
        assert_eq!(
            state.error_message,
            Some("Keine Lösung für die aktuellen Eingaben gefunden".to_string())
        );
    }

    #[test]
    fn csv_import_adds_valid_rows_and_reports_line_errors() {
        let mut state = FreecutAppState::default();
        let csv = "label,width,length,quantity\n\
valid,10,20,1\n\
broken,0,20,1\n\
valid2,30,40,2\n";
        let result = crate::import::import_project_csv(csv, next_piece_id(&state.project).0);

        apply_csv_import_result(&mut state, result);

        assert_eq!(state.project.cut_pieces.len(), 2);
        assert_eq!(state.project.cut_pieces[0].id, PieceId(1));
        assert_eq!(state.project.cut_pieces[1].id, PieceId(2));
        assert!(state.dirty);
        assert!(state.solution.is_none());
        assert_eq!(
            state.error_message,
            Some(
                "CSV import: 2 cut piece(s), 0 stock piece(s), 1 error(s); line 3: width muss größer als 0 sein"
                    .to_string()
            )
        );
    }

    #[test]
    fn csv_import_from_state_path_reads_file_and_uses_next_piece_id() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        state.dirty = false;
        let path = std::env::temp_dir().join(format!(
            "freecut-ui-import-{}-{}.csv",
            std::process::id(),
            state.project.stock_pieces.len()
        ));
        std::fs::write(&path, "label,width,length,quantity\npart,10,20,1\n")
            .expect("write csv fixture");
        state.csv_import_path = path.to_string_lossy().to_string();

        import_csv_from_state_path(&mut state);

        std::fs::remove_file(path).expect("remove csv fixture");
        assert_eq!(state.project.stock_pieces.len(), 1);
        assert_eq!(state.project.cut_pieces.len(), 1);
        assert_eq!(state.project.cut_pieces[0].id, PieceId(2));
        assert_eq!(
            state.csv_import_report.as_deref(),
            Some("CSV import: 1 cut piece(s), 0 stock piece(s), 0 error(s)")
        );
        assert!(state.error_message.is_none());
        assert!(state.dirty);
    }

    #[test]
    fn csv_import_summary_uses_german_when_selected() {
        let mut state = FreecutAppState {
            language: UiLanguage::German,
            ..FreecutAppState::default()
        };
        let csv = "label,width,length,quantity\n\
valid,10,20,1\n\
broken,0,20,1\n";
        let result = crate::import::import_project_csv(csv, next_piece_id(&state.project).0);

        apply_csv_import_result(&mut state, result);

        assert_eq!(
            state.csv_import_report.as_deref(),
            Some(
                "CSV-Import: 1 Zuschnitt(e), 0 Rohteil(e), 1 Fehler; Zeile 3: width muss größer als 0 sein"
            )
        );
        assert_eq!(state.csv_import_report, state.error_message);
    }

    #[test]
    fn project_save_and_load_roundtrip_uses_explicit_path_and_dirty_flag() {
        let path = std::env::temp_dir().join(format!(
            "freecut-ui-project-{}-{}.{}",
            std::process::id(),
            1,
            PROJECT_FILE_EXTENSION
        ));

        let mut saved_state = FreecutAppState::default();
        add_default_stock_piece(&mut saved_state);
        add_default_cut_piece(&mut saved_state);
        saved_state.project.name = "saved project".to_string();
        saved_state.optimizer_effort = OptimizerEffort::Thorough;
        saved_state.project_path = path.to_string_lossy().to_string();

        save_project_from_state_path(&mut saved_state);

        assert!(!saved_state.dirty);
        assert!(saved_state.error_message.is_none());
        assert!(saved_state
            .project_io_report
            .as_deref()
            .is_some_and(|message| message.starts_with("Project file saved:")));

        let mut loaded_state = FreecutAppState::default();
        loaded_state.project_path = saved_state.project_path.clone();
        load_project_from_state_path(&mut loaded_state);

        std::fs::remove_file(path).expect("remove project fixture");
        assert_eq!(loaded_state.project, saved_state.project);
        assert_eq!(loaded_state.optimizer_effort, OptimizerEffort::Thorough);
        assert!(loaded_state.solution.is_none());
        assert_eq!(loaded_state.selection, UiSelection::default());
        assert!(!loaded_state.dirty);
        assert!(loaded_state.error_message.is_none());
    }

    #[test]
    fn project_load_reports_file_errors_without_replacing_current_project() {
        let mut state = FreecutAppState::default();
        add_default_cut_piece(&mut state);
        let before = state.project.clone();
        state.project_path = "/definitely/missing/freecut-project.json".to_string();

        load_project_from_state_path(&mut state);

        assert_eq!(state.project, before);
        assert!(state.solution.is_none());
        assert!(state
            .error_message
            .as_deref()
            .is_some_and(|message| message.starts_with("Project file: load failed:")));
    }

    #[test]
    fn project_io_empty_paths_use_selected_language_without_dirtying() {
        let mut state = FreecutAppState {
            language: UiLanguage::German,
            ..FreecutAppState::default()
        };

        save_project_from_state_path(&mut state);
        assert_eq!(
            state.error_message,
            Some("Projektdatei: Bitte einen Speicherpfad angeben".to_string())
        );
        assert!(!state.dirty);

        state.error_message = None;
        state.project_io_report = None;

        load_project_from_state_path(&mut state);
        assert_eq!(
            state.error_message,
            Some("Projektdatei: Bitte einen Ladepfad angeben".to_string())
        );
        assert!(!state.dirty);
    }

    #[test]
    fn pdf_export_from_state_path_writes_current_solution() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        state.project.settings.unit = Unit::Foot;
        optimize_current_project(&mut state);
        let path = std::env::temp_dir().join(format!(
            "freecut-ui-export-{}-{}.pdf",
            std::process::id(),
            state.project.cut_pieces.len()
        ));
        state.export_path = path.to_string_lossy().to_string();

        export_solution_from_state_path(&mut state);

        let bytes = std::fs::read(&path).expect("read exported pdf");
        std::fs::remove_file(path).expect("remove exported pdf fixture");
        let text = String::from_utf8_lossy(&bytes);
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(text.contains("Size 2440 x 1220 foot"));
        assert!(text.contains(r"Width \(foot\)"));
        assert!(state.error_message.is_none());
        assert!(state
            .export_report
            .as_deref()
            .is_some_and(|message| message.starts_with("PDF exported:")));
    }

    #[test]
    fn pdf_export_reports_missing_solution_without_writing() {
        let mut state = FreecutAppState::default();
        state.export_path = "target/freecut-missing-solution.pdf".to_string();

        export_solution_from_state_path(&mut state);

        assert_eq!(
            state.error_message,
            Some("PDF export: no solution available to export".to_string())
        );
    }

    #[test]
    fn pdf_export_reports_empty_export_path() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        optimize_current_project(&mut state);

        export_solution_from_state_path(&mut state);

        assert_eq!(
            state.error_message,
            Some("PDF export: enter an export path".to_string())
        );
    }

    #[test]
    fn pdf_export_status_uses_german_when_selected() {
        let mut state = FreecutAppState {
            language: UiLanguage::German,
            export_path: "target/freecut-missing-solution-de.pdf".to_string(),
            ..FreecutAppState::default()
        };

        export_solution_from_state_path(&mut state);

        assert_eq!(
            state.error_message,
            Some("PDF-Export: Keine Lösung zum Exportieren vorhanden".to_string())
        );
        assert!(!state.dirty);
    }

    #[test]
    fn dynamic_sheet_view_max_height_uses_available_panel_height() {
        assert_eq!(
            dynamic_sheet_view_max_height(SHEET_VIEW_DEFAULT_MAX_HEIGHT + 180.0),
            SHEET_VIEW_DEFAULT_MAX_HEIGHT + 180.0
        );
        assert_eq!(
            dynamic_sheet_view_max_height(SHEET_VIEW_DEFAULT_MAX_HEIGHT - 80.0),
            SHEET_VIEW_DEFAULT_MAX_HEIGHT
        );
        assert_eq!(
            dynamic_sheet_view_max_height(f32::INFINITY),
            SHEET_VIEW_DEFAULT_MAX_HEIGHT
        );
    }

    #[test]
    fn sheet_view_bounds_scales_sheet_to_available_area() {
        let sheet = solution_sheet_with_size(200, 100);

        let bounds = sheet_view_bounds(&sheet, 232.0, 132.0).expect("valid sheet bounds");

        assert_eq!(bounds.scale, 1.0);
        assert_eq!(bounds.sheet_size, egui::vec2(200.0, 100.0));
        assert_eq!(bounds.view_size, egui::vec2(232.0, 132.0));
    }

    #[test]
    fn sheet_view_bounds_limits_by_height_when_sheet_is_tall() {
        let sheet = solution_sheet_with_size(200, 400);

        let bounds = sheet_view_bounds(&sheet, 432.0, 232.0).expect("valid sheet bounds");

        assert_eq!(bounds.scale, 0.5);
        assert_eq!(bounds.sheet_size, egui::vec2(100.0, 200.0));
        assert_eq!(bounds.view_size, egui::vec2(132.0, 232.0));
    }

    #[test]
    fn sheet_view_bounds_rejects_zero_sized_sheet() {
        assert!(sheet_view_bounds(&solution_sheet_with_size(0, 100), 232.0, 132.0).is_none());
        assert!(sheet_view_bounds(&solution_sheet_with_size(100, 0), 232.0, 132.0).is_none());
    }

    #[test]
    fn solution_rect_to_screen_rect_preserves_render_coordinates() {
        let sheet_rect =
            egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(200.0, 100.0));
        let render_rect = SolutionRect {
            x: 5,
            y: 10,
            width: 20,
            length: 30,
        };

        let screen_rect = solution_rect_to_screen_rect(render_rect, sheet_rect, 2.0);

        assert_eq!(screen_rect.min, egui::pos2(20.0, 40.0));
        assert_eq!(screen_rect.size(), egui::vec2(40.0, 60.0));
    }

    #[test]
    fn cut_preview_geometry_uses_horizontal_kerf_area_on_work_rect() {
        let cut = SolutionCut::new(
            SolutionRect {
                x: 10,
                y: 20,
                width: 100,
                length: 80,
            },
            CutOrientation::Horizontal,
            30,
            3,
        )
        .expect("valid cut");

        assert_eq!(
            cut_preview_geometry(&cut),
            CutPreviewGeometry::KerfRect(SolutionRect {
                x: 10,
                y: 50,
                width: 100,
                length: 3,
            })
        );
    }

    #[test]
    fn cut_preview_geometry_uses_vertical_kerf_area_on_work_rect() {
        let cut = SolutionCut::new(
            SolutionRect {
                x: 10,
                y: 20,
                width: 100,
                length: 80,
            },
            CutOrientation::Vertical,
            40,
            2,
        )
        .expect("valid cut");

        assert_eq!(
            cut_preview_geometry(&cut),
            CutPreviewGeometry::KerfRect(SolutionRect {
                x: 50,
                y: 20,
                width: 2,
                length: 80,
            })
        );
    }

    #[test]
    fn cut_preview_geometry_uses_lines_for_zero_kerf_cuts() {
        let horizontal = SolutionCut::new(
            SolutionRect {
                x: 10,
                y: 20,
                width: 100,
                length: 80,
            },
            CutOrientation::Horizontal,
            30,
            0,
        )
        .expect("valid zero-kerf horizontal cut");
        let vertical = SolutionCut::new(
            SolutionRect {
                x: 10,
                y: 20,
                width: 100,
                length: 80,
            },
            CutOrientation::Vertical,
            40,
            0,
        )
        .expect("valid zero-kerf vertical cut");

        assert_eq!(
            cut_preview_geometry(&horizontal),
            CutPreviewGeometry::ZeroKerfLine(CutPreviewLine {
                start_x: 10,
                start_y: 50,
                end_x: 110,
                end_y: 50,
            })
        );
        assert_eq!(
            cut_preview_geometry(&vertical),
            CutPreviewGeometry::ZeroKerfLine(CutPreviewLine {
                start_x: 50,
                start_y: 20,
                end_x: 50,
                end_y: 100,
            })
        );
    }

    #[test]
    fn cut_preview_line_to_screen_points_preserves_render_coordinates() {
        let sheet_rect =
            egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(200.0, 100.0));
        let line = CutPreviewLine {
            start_x: 5,
            start_y: 10,
            end_x: 25,
            end_y: 10,
        };

        assert_eq!(
            cut_preview_line_to_screen_points(line, sheet_rect, 2.0),
            [egui::pos2(20.0, 40.0), egui::pos2(60.0, 40.0)]
        );
    }

    #[test]
    fn sheet_cut_preview_geometries_follow_preorder_cuts() {
        let root_cut = SolutionCut::new(
            SolutionRect {
                x: 0,
                y: 0,
                width: 100,
                length: 80,
            },
            CutOrientation::Horizontal,
            50,
            3,
        )
        .expect("valid root cut");
        let child_cut = SolutionCut::new(
            SolutionRect {
                x: 0,
                y: 0,
                width: 100,
                length: 50,
            },
            CutOrientation::Vertical,
            60,
            3,
        )
        .expect("valid child cut");
        let mut sheet = solution_sheet_with_size(100, 80);
        sheet.cutting_guide = Some(SolutionSliceNode::cut(
            root_cut,
            SolutionSliceNode::cut(
                child_cut,
                SolutionSliceNode::leaf(
                    SolutionRect {
                        x: 0,
                        y: 0,
                        width: 60,
                        length: 50,
                    },
                    crate::render::LeafKind::CutPiece {
                        cut_id: PieceId(1),
                        instance: 0,
                    },
                ),
                SolutionSliceNode::leaf(
                    SolutionRect {
                        x: 63,
                        y: 0,
                        width: 37,
                        length: 50,
                    },
                    crate::render::LeafKind::Waste,
                ),
            ),
            SolutionSliceNode::leaf(
                SolutionRect {
                    x: 0,
                    y: 53,
                    width: 100,
                    length: 27,
                },
                crate::render::LeafKind::Waste,
            ),
        ));

        assert_eq!(
            sheet_cut_preview_geometries(&sheet, LayoutKind::Guillotine),
            vec![
                CutPreviewGeometry::KerfRect(SolutionRect {
                    x: 0,
                    y: 50,
                    width: 100,
                    length: 3,
                }),
                CutPreviewGeometry::KerfRect(SolutionRect {
                    x: 60,
                    y: 0,
                    width: 3,
                    length: 50,
                }),
            ]
        );
    }

    #[test]
    fn sheet_cut_preview_geometries_are_empty_without_cutting_guide() {
        let sheet = solution_sheet_with_size(100, 80);

        assert_eq!(
            sheet_cut_preview_geometries(&sheet, LayoutKind::Guillotine),
            Vec::new()
        );
    }

    #[test]
    fn nested_cut_preview_geometries_use_actual_uncovered_kerf_gaps() {
        let mut sheet = solution_sheet_with_size(10, 6);
        sheet.placed_pieces = vec![
            placed_piece(PieceId(1), 0, 0, 0, 4, 6),
            placed_piece(PieceId(2), 0, 6, 0, 4, 6),
        ];

        assert_eq!(
            sheet_cut_preview_geometries(&sheet, LayoutKind::Nested),
            vec![CutPreviewGeometry::KerfRect(SolutionRect {
                x: 4,
                y: 0,
                width: 2,
                length: 6,
            })]
        );
    }

    #[test]
    fn nested_cut_preview_geometries_subtract_waste_from_uncovered_area() {
        let mut sheet = solution_sheet_with_size(10, 6);
        sheet.placed_pieces = vec![placed_piece(PieceId(1), 0, 0, 0, 4, 6)];
        sheet.waste = vec![SolutionRect {
            x: 6,
            y: 0,
            width: 4,
            length: 6,
        }];

        assert_eq!(
            sheet_cut_preview_geometries(&sheet, LayoutKind::Nested),
            vec![CutPreviewGeometry::KerfRect(SolutionRect {
                x: 4,
                y: 0,
                width: 2,
                length: 6,
            })]
        );
    }

    #[test]
    fn piece_label_uses_cut_id_and_one_based_instance() {
        let piece = PlacedPiece {
            cut_id: PieceId(7),
            instance: 2,
            rect: SolutionRect {
                x: 0,
                y: 0,
                width: 10,
                length: 10,
            },
            pattern: PatternDirection::None,
            rotated: false,
        };

        assert_eq!(piece_label(&piece), "#7-3");
    }

    #[test]
    fn selected_cut_id_prepares_solution_highlight_from_input_selection() {
        let mut state = FreecutAppState::default();
        add_default_cut_piece(&mut state);
        add_default_cut_piece(&mut state);
        state.selection.cut_index = Some(0);

        assert_eq!(selected_cut_id(&state), Some(PieceId(1)));

        state.selection.cut_index = Some(99);
        assert_eq!(selected_cut_id(&state), None);
    }

    #[test]
    fn highlighted_piece_gets_distinct_style() {
        let piece = PlacedPiece {
            cut_id: PieceId(7),
            instance: 0,
            rect: SolutionRect {
                x: 0,
                y: 0,
                width: 10,
                length: 10,
            },
            pattern: PatternDirection::None,
            rotated: false,
        };

        assert_ne!(
            piece_fill_color(&piece, false),
            piece_fill_color(&piece, true)
        );
        assert!(piece_highlight_stroke().width > SOLUTION_BOUNDARY_STROKE_WIDTH);
    }

    #[test]
    fn sheet_view_palette_uses_dark_preview_colors() {
        assert!(
            color_luminance(SHEET_VIEW_BACKGROUND_COLOR) < color_luminance(SHEET_SURFACE_COLOR)
        );
        assert!(color_luminance(SHEET_SURFACE_COLOR) < 80);
        assert!(color_luminance(SHEET_WASTE_COLOR) < 90);
        assert!(color_luminance(PIECE_LABEL_COLOR) > 220);
    }

    #[test]
    fn piece_colors_keep_rotation_and_highlight_distinct_on_dark_preview() {
        let normal_piece = PlacedPiece {
            cut_id: PieceId(7),
            instance: 0,
            rect: SolutionRect {
                x: 0,
                y: 0,
                width: 10,
                length: 10,
            },
            pattern: PatternDirection::None,
            rotated: false,
        };
        let rotated_piece = PlacedPiece {
            rotated: true,
            ..normal_piece.clone()
        };

        assert_ne!(
            piece_fill_color(&normal_piece, false),
            piece_fill_color(&rotated_piece, false)
        );
        assert_ne!(
            piece_fill_color(&normal_piece, false),
            piece_fill_color(&normal_piece, true)
        );
        assert_ne!(
            piece_boundary_stroke().color,
            piece_highlight_stroke().color
        );
    }

    #[test]
    fn neutral_boundaries_are_hidden_for_nested_and_cutting_guides() {
        let mut sheet = solution_sheet_with_size(100, 80);
        assert!(should_draw_neutral_waste_boundaries(
            &sheet,
            LayoutKind::Guillotine
        ));
        assert!(should_draw_piece_boundaries(&sheet, LayoutKind::Guillotine));
        assert!(!should_draw_neutral_waste_boundaries(
            &sheet,
            LayoutKind::Nested
        ));
        assert!(!should_draw_piece_boundaries(&sheet, LayoutKind::Nested));

        let cut = SolutionCut::new(
            SolutionRect {
                x: 0,
                y: 0,
                width: 100,
                length: 80,
            },
            CutOrientation::Horizontal,
            50,
            3,
        )
        .expect("valid cut");
        sheet.cutting_guide = Some(SolutionSliceNode::cut(
            cut,
            SolutionSliceNode::leaf(
                SolutionRect {
                    x: 0,
                    y: 0,
                    width: 100,
                    length: 50,
                },
                crate::render::LeafKind::Waste,
            ),
            SolutionSliceNode::leaf(
                SolutionRect {
                    x: 0,
                    y: 53,
                    width: 100,
                    length: 27,
                },
                crate::render::LeafKind::Waste,
            ),
        ));

        assert!(!should_draw_neutral_waste_boundaries(
            &sheet,
            LayoutKind::Guillotine
        ));
        assert!(!should_draw_piece_boundaries(
            &sheet,
            LayoutKind::Guillotine
        ));
    }

    #[test]
    fn solution_boundary_strokes_are_visible_but_not_highlight_style() {
        assert!(SOLUTION_BOUNDARY_STROKE_WIDTH >= 1.0);
        assert_ne!(
            piece_boundary_stroke().color,
            piece_fill_color(&placed_piece(PieceId(1), 0, 0, 0, 10, 10), false)
        );
        assert_ne!(waste_boundary_stroke().color, SHEET_WASTE_COLOR);
        assert_ne!(
            piece_boundary_stroke().color,
            piece_highlight_stroke().color
        );
    }

    #[test]
    fn cut_kerf_style_is_constant_red_and_distinct() {
        assert_eq!(CUT_KERF_COLOR, egui::Color32::from_rgb(255, 0, 0));
        assert_eq!(cut_kerf_color(), CUT_KERF_COLOR);
        assert_eq!(cut_kerf_stroke().color, CUT_KERF_COLOR);
        assert!(cut_kerf_stroke().width > SOLUTION_BOUNDARY_STROKE_WIDTH);
        assert_ne!(CUT_KERF_COLOR, PIECE_HIGHLIGHT_STROKE_COLOR);
        assert_ne!(CUT_KERF_COLOR, PIECE_BOUNDARY_STROKE_COLOR);
        assert_ne!(cut_kerf_color(), SHEET_WASTE_COLOR);
    }

    #[test]
    fn seam_modes_use_red_sheet_base_for_uncovered_kerf_gaps() {
        let mut sheet = solution_sheet_with_size(100, 80);
        assert_eq!(
            sheet_surface_color(&sheet, LayoutKind::Guillotine),
            SHEET_SURFACE_COLOR
        );

        assert_eq!(
            sheet_surface_color(&sheet, LayoutKind::Nested),
            CUT_KERF_COLOR
        );

        sheet.cutting_guide = Some(crate::render::SliceNode::leaf(
            SolutionRect {
                x: 0,
                y: 0,
                width: 100,
                length: 80,
            },
            crate::render::LeafKind::Waste,
        ));
        assert_eq!(
            sheet_surface_color(&sheet, LayoutKind::Guillotine),
            CUT_KERF_COLOR
        );
    }

    #[test]
    fn painter_records_red_kerf_shapes() {
        let context = egui::Context::default();
        context.begin_pass(egui::RawInput::default());
        let painter = egui::Painter::new(
            context.clone(),
            egui::LayerId::new(egui::Order::Foreground, egui::Id::new("kerf_test")),
            egui::Rect::EVERYTHING,
        );
        let mut sheet = solution_sheet_with_size(100, 80);
        sheet.cutting_guide = Some(crate::render::SliceNode::cut(
            SolutionCut::new(
                SolutionRect {
                    x: 0,
                    y: 0,
                    width: 100,
                    length: 80,
                },
                CutOrientation::Vertical,
                40,
                2,
            )
            .expect("valid cut"),
            crate::render::SliceNode::leaf(
                SolutionRect {
                    x: 0,
                    y: 0,
                    width: 40,
                    length: 80,
                },
                crate::render::LeafKind::CutPiece {
                    cut_id: PieceId(1),
                    instance: 0,
                },
            ),
            crate::render::SliceNode::leaf(
                SolutionRect {
                    x: 42,
                    y: 0,
                    width: 58,
                    length: 80,
                },
                crate::render::LeafKind::Waste,
            ),
        ));

        draw_cut_kerf_preview(
            &painter,
            &sheet,
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 80.0)),
            1.0,
            LayoutKind::Guillotine,
        );
        let output = context.end_pass();

        assert!(output.shapes.iter().any(|shape| match &shape.shape {
            egui::epaint::Shape::Rect(rect) => rect.fill == CUT_KERF_COLOR,
            egui::epaint::Shape::LineSegment { stroke, .. } => stroke.color == CUT_KERF_COLOR,
            _ => false,
        }));
    }

    #[test]
    fn thin_kerf_rects_get_visible_center_lines() {
        let vertical = thin_kerf_rect_center_line(egui::Rect::from_min_size(
            egui::pos2(10.0, 20.0),
            egui::vec2(0.5, 30.0),
        ))
        .expect("thin vertical kerf gets a line");
        assert_eq!(vertical[0].x, vertical[1].x);
        assert_eq!(vertical[0].y, 20.0);
        assert_eq!(vertical[1].y, 50.0);

        let horizontal = thin_kerf_rect_center_line(egui::Rect::from_min_size(
            egui::pos2(10.0, 20.0),
            egui::vec2(30.0, 0.5),
        ))
        .expect("thin horizontal kerf gets a line");
        assert_eq!(horizontal[0].y, horizontal[1].y);
        assert_eq!(horizontal[0].x, 10.0);
        assert_eq!(horizontal[1].x, 40.0);

        assert!(thin_kerf_rect_center_line(egui::Rect::from_min_size(
            egui::pos2(10.0, 20.0),
            egui::vec2(CUT_KERF_STROKE_WIDTH, CUT_KERF_STROKE_WIDTH),
        ))
        .is_none());
    }

    #[test]
    fn hit_test_placed_piece_selects_topmost_containing_piece() {
        let mut sheet = solution_sheet_with_size(100, 100);
        sheet.placed_pieces = vec![
            placed_piece(PieceId(1), 0, 0, 0, 80, 80),
            placed_piece(PieceId(2), 0, 20, 20, 40, 40),
        ];
        let sheet_rect =
            egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(100.0, 100.0));

        assert_eq!(
            hit_test_placed_piece(&sheet, sheet_rect, 1.0, egui::pos2(35.0, 35.0)),
            Some(1)
        );
        assert_eq!(
            hit_test_placed_piece(&sheet, sheet_rect, 1.0, egui::pos2(15.0, 15.0)),
            Some(0)
        );
        assert_eq!(
            hit_test_placed_piece(&sheet, sheet_rect, 1.0, egui::pos2(95.0, 95.0)),
            None
        );
    }

    #[test]
    fn selecting_placed_piece_sets_sheet_piece_and_input_cut_selection() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        add_default_cut_piece(&mut state);
        state.selection.stock_index = Some(0);
        state.solution = Some(Solution {
            layout: LayoutKind::Guillotine,
            sheets: vec![SolutionSheet {
                stock_id: PieceId(1),
                width: 200,
                length: 100,
                placed_pieces: vec![
                    placed_piece(PieceId(2), 0, 0, 0, 50, 50),
                    placed_piece(PieceId(3), 0, 50, 0, 50, 50),
                ],
                waste: Vec::new(),
                cutting_guide: None,
            }],
            fitness: None,
        });

        select_placed_piece(&mut state, 0, 1);

        assert_eq!(state.selection.sheet_index, Some(0));
        assert_eq!(state.selection.placed_piece_index, Some(1));
        assert_eq!(state.selection.stock_index, None);
        assert_eq!(state.selection.cut_index, Some(1));
    }

    #[test]
    fn selected_placed_piece_details_connect_solution_piece_to_input_row() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        state.project.cut_pieces[0].label = "door".to_string();
        state.project.cut_pieces[0].width = 70;
        state.project.cut_pieces[0].length = 30;
        state.solution = Some(Solution {
            layout: LayoutKind::Guillotine,
            sheets: vec![SolutionSheet {
                stock_id: PieceId(1),
                width: 200,
                length: 100,
                placed_pieces: vec![placed_piece(PieceId(2), 3, 10, 20, 70, 30)],
                waste: Vec::new(),
                cutting_guide: None,
            }],
            fitness: None,
        });
        state.selection.sheet_index = Some(0);
        state.selection.placed_piece_index = Some(0);

        let details = selected_placed_piece_details(&state).expect("selected piece details");

        assert_eq!(details.sheet_index, 0);
        assert_eq!(details.piece_index, 0);
        assert_eq!(details.placed_piece.rect.x, 10);
        assert_eq!(details.cut_piece.expect("input cut").label, "door");
    }

    #[test]
    fn stale_placed_piece_selection_is_cleared_when_project_changes() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        optimize_current_project(&mut state);
        state.selection.placed_piece_index = Some(0);

        add_default_cut_piece(&mut state);

        assert!(state.solution.is_none());
        assert_eq!(state.selection.sheet_index, None);
        assert_eq!(state.selection.placed_piece_index, None);
    }

    #[test]
    fn optimized_solution_has_drawable_sheet_bounds() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);

        optimize_current_project(&mut state);

        let solution = state
            .solution
            .expect("valid project should produce solution");
        let bounds = sheet_view_bounds(
            &solution.sheets[0],
            480.0,
            dynamic_sheet_view_max_height(720.0),
        );
        assert!(bounds.is_some());
    }

    #[test]
    fn validation_summary_reports_invalid_rows_inline_source() {
        let mut state = FreecutAppState::default();
        add_default_stock_piece(&mut state);
        add_default_cut_piece(&mut state);
        state.project.stock_pieces[0].width = 0;
        state.project.cut_pieces[0].label.clear();
        let english = UiTexts::new(UiLanguage::English);
        let german = UiTexts::new(UiLanguage::German);

        assert_eq!(
            stock_validation_message(&state.project.stock_pieces[0], german),
            Some("Maße > 0")
        );
        assert_eq!(
            stock_validation_message(&state.project.stock_pieces[0], english),
            Some("Dimensions > 0")
        );
        assert_eq!(
            cut_validation_message(&state.project.cut_pieces[0], german),
            Some("Label fehlt")
        );
        assert_eq!(
            cut_validation_message(&state.project.cut_pieces[0], english),
            Some("Label missing")
        );
        assert_eq!(
            validation_report(&state.project).summary(english),
            Some("Input error: 0 project, 1 stock piece(s), 1 cut piece(s)".to_string())
        );
        assert_eq!(
            validation_report(&state.project).summary(german),
            Some("Eingabefehler: 0 Projekt, 1 Rohteil(e), 1 Zuschnitt(e)".to_string())
        );
    }

    fn solution_sheet_with_size(width: u32, length: u32) -> SolutionSheet {
        SolutionSheet {
            stock_id: PieceId(1),
            width,
            length,
            placed_pieces: Vec::new(),
            waste: Vec::new(),
            cutting_guide: None,
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
            rect: SolutionRect {
                x,
                y,
                width,
                length,
            },
            pattern: PatternDirection::None,
            rotated: false,
        }
    }

    fn color_luminance(color: egui::Color32) -> u8 {
        let [red, green, blue, _alpha] = color.to_array();
        ((u16::from(red) + u16::from(green) + u16::from(blue)) / 3) as u8
    }
}

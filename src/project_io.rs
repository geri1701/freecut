//! Native Freecut project file persistence.
//!
//! Project files are explicit JSON documents. They are intentionally separate from the old
//! hidden `$HOME/.config/FreeCut` auto-save file.

use std::{fmt, fs, io, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    domain::{default_cut_pattern, Project},
    measurement::length_from_whole_units,
    optimizer::OptimizerEffort,
};

pub const PROJECT_FILE_VERSION: u32 = 3;
pub const PROJECT_FILE_EXTENSION: &str = "freecut.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectDocument {
    pub version: u32,
    pub project: Project,
    pub optimizer_effort: OptimizerEffort,
}

impl ProjectDocument {
    #[must_use]
    pub fn new(project: Project, optimizer_effort: OptimizerEffort) -> Self {
        Self {
            version: PROJECT_FILE_VERSION,
            project,
            optimizer_effort,
        }
    }
}

#[derive(Debug)]
pub enum ProjectIoError {
    Io(io::Error),
    Json(serde_json::Error),
    UnsupportedVersion(u32),
    DimensionOverflow,
}

impl fmt::Display for ProjectIoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "Dateifehler: {error}"),
            Self::Json(error) => write!(formatter, "Projektdatei ist kein gültiges JSON: {error}"),
            Self::UnsupportedVersion(version) => {
                write!(
                    formatter,
                    "Projektdatei-Version {version} wird nicht unterstützt"
                )
            }
            Self::DimensionOverflow => {
                formatter.write_str("Projekt enthält ein Maß außerhalb des unterstützten Bereichs")
            }
        }
    }
}

impl std::error::Error for ProjectIoError {}

impl From<io::Error> for ProjectIoError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ProjectIoError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[allow(clippy::missing_errors_doc)]
pub fn save_project_file(
    path: impl AsRef<Path>,
    document: &ProjectDocument,
) -> Result<(), ProjectIoError> {
    let mut serialized = serde_json::to_string_pretty(document)?;
    serialized.push('\n');
    fs::write(path, serialized)?;
    Ok(())
}

#[allow(clippy::missing_errors_doc)]
pub fn load_project_file(path: impl AsRef<Path>) -> Result<ProjectDocument, ProjectIoError> {
    let source = fs::read_to_string(path)?;
    load_project_document_from_str(&source)
}

#[allow(clippy::missing_errors_doc)]
pub fn load_project_document_from_str(source: &str) -> Result<ProjectDocument, ProjectIoError> {
    let mut document = serde_json::from_str::<ProjectDocument>(source)?;

    match document.version {
        1 => {
            migrate_version_1_document(&mut document)?;
            migrate_cut_patterns(&mut document);
            document.version = PROJECT_FILE_VERSION;
        }
        2 => {
            migrate_cut_patterns(&mut document);
            document.version = PROJECT_FILE_VERSION;
        }
        PROJECT_FILE_VERSION => {}
        version => return Err(ProjectIoError::UnsupportedVersion(version)),
    }

    Ok(document)
}

fn migrate_version_1_document(document: &mut ProjectDocument) -> Result<(), ProjectIoError> {
    let unit = document.project.settings.unit;
    document.project.settings.kerf_width =
        length_from_whole_units(document.project.settings.kerf_width, unit)
            .ok_or(ProjectIoError::DimensionOverflow)?;

    for stock in &mut document.project.stock_pieces {
        stock.width =
            length_from_whole_units(stock.width, unit).ok_or(ProjectIoError::DimensionOverflow)?;
        stock.length =
            length_from_whole_units(stock.length, unit).ok_or(ProjectIoError::DimensionOverflow)?;
    }
    for cut in &mut document.project.cut_pieces {
        cut.width =
            length_from_whole_units(cut.width, unit).ok_or(ProjectIoError::DimensionOverflow)?;
        cut.length =
            length_from_whole_units(cut.length, unit).ok_or(ProjectIoError::DimensionOverflow)?;
    }

    Ok(())
}

fn migrate_cut_patterns(document: &mut ProjectDocument) {
    for cut in &mut document.project.cut_pieces {
        cut.pattern = default_cut_pattern(cut.can_rotate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CutSettings, LayoutKind, PatternDirection, PieceId, RotationMode, StockPiece, Unit,
    };
    use crate::measurement::{length_from_whole_units, parse_length};

    #[test]
    fn version_3_document_roundtrip_preserves_settings_and_effort() {
        let project = Project {
            name: "roundtrip".to_string(),
            stock_pieces: vec![StockPiece {
                id: PieceId(1),
                width: parse_length("48 1/8", Unit::Inch).expect("fixture parses"),
                length: parse_length("24.5", Unit::Inch).expect("fixture parses"),
                quantity: Some(2),
                pattern: PatternDirection::ParallelToLength,
            }],
            cut_pieces: Vec::new(),
            settings: CutSettings {
                unit: Unit::Inch,
                kerf_width: parse_length("1/8", Unit::Inch).expect("fixture parses"),
                layout: LayoutKind::Guillotine,
                rotation_mode: RotationMode::All,
            },
        };
        let document = ProjectDocument::new(project, OptimizerEffort::Thorough);
        let serialized = serde_json::to_string_pretty(&document).expect("serialize project");

        let loaded = load_project_document_from_str(&serialized).expect("load project");

        assert_eq!(loaded.version, PROJECT_FILE_VERSION);
        assert_eq!(loaded, document);
    }

    #[test]
    fn deserializes_legacy_version_3_rotation_setting() {
        let source = r#"{
  "version": 3,
  "project": {
    "name": "legacy rotation",
    "stock_pieces": [],
    "cut_pieces": [{
      "id": 1,
      "label": "fixed part",
      "width": 10,
      "length": 20,
      "quantity": 1,
      "pattern": "ParallelToWidth",
      "can_rotate": false
    }],
    "settings": {
      "unit": "Millimeter",
      "kerf_width": 0,
      "layout": "Guillotine",
      "all_parts_rotatable": true
    }
  },
  "optimizer_effort": "Fast"
}"#;

        let document = load_project_document_from_str(source).expect("load all-rotatable mode");

        assert_eq!(document.version, PROJECT_FILE_VERSION);
        assert_eq!(document.project.settings.rotation_mode, RotationMode::All);
        assert!(!document.project.cut_pieces[0].can_rotate);
        assert_eq!(
            document.project.cut_pieces[0].pattern,
            PatternDirection::ParallelToWidth
        );

        let source = source.replace("true", "false");
        let document = load_project_document_from_str(&source).expect("migrate per-part mode");
        assert_eq!(
            document.project.settings.rotation_mode,
            RotationMode::PerPart
        );
    }

    #[test]
    fn migrates_version_2_cut_patterns_from_rotation_defaults() {
        let source = r#"{
  "version": 2,
  "project": {
    "name": "legacy patterns",
    "stock_pieces": [],
    "cut_pieces": [
      {
        "id": 1,
        "label": "fixed part",
        "width": 10,
        "length": 20,
        "quantity": 1,
        "pattern": "ParallelToWidth",
        "can_rotate": false
      },
      {
        "id": 2,
        "label": "rotatable part",
        "width": 10,
        "length": 20,
        "quantity": 1,
        "pattern": "ParallelToLength",
        "can_rotate": true
      }
    ],
    "settings": { "unit": "Millimeter", "kerf_width": 0, "layout": "Guillotine" }
  },
  "optimizer_effort": "Fast"
}"#;

        let document = load_project_document_from_str(source).expect("migrate project");

        assert_eq!(document.version, PROJECT_FILE_VERSION);
        assert_eq!(
            document.project.settings.rotation_mode,
            RotationMode::PerPart
        );
        assert_eq!(
            document.project.cut_pieces[0].pattern,
            PatternDirection::ParallelToLength
        );
        assert_eq!(
            document.project.cut_pieces[1].pattern,
            PatternDirection::None
        );
    }

    #[test]
    fn rejects_unsupported_project_file_version() {
        let source = r#"{
  "version": 999,
  "project": {
    "name": "too new",
    "stock_pieces": [],
    "cut_pieces": [],
    "settings": { "unit": "Millimeter", "kerf_width": 0, "layout": "Guillotine" }
  },
  "optimizer_effort": "Fast"
}"#;

        let error = load_project_document_from_str(source).expect_err("version should fail");

        assert_eq!(
            error.to_string(),
            "Projektdatei-Version 999 wird nicht unterstützt"
        );
    }

    #[test]
    fn migrates_version_1_dimensions_from_the_declared_project_unit() {
        let source = r#"{
  "version": 1,
  "project": {
    "name": "legacy inch project",
    "stock_pieces": [{
      "id": 1,
      "width": 48,
      "length": 24,
      "quantity": 1,
      "pattern": "None"
    }],
    "cut_pieces": [],
    "settings": { "unit": "Inch", "kerf_width": 1, "layout": "Guillotine" }
  },
  "optimizer_effort": "Fast"
}"#;

        let document = load_project_document_from_str(source).expect("migrate project");

        assert_eq!(document.version, PROJECT_FILE_VERSION);
        assert_eq!(
            document.project.stock_pieces[0].width,
            length_from_whole_units(48, Unit::Inch).expect("fixture fits")
        );
        assert_eq!(
            document.project.settings.kerf_width,
            length_from_whole_units(1, Unit::Inch).expect("fixture fits")
        );
    }

    #[test]
    fn rejects_version_1_dimensions_that_do_not_fit_geometry_coordinates() {
        let source = r#"{
  "version": 1,
  "project": {
    "name": "oversized legacy project",
    "stock_pieces": [],
    "cut_pieces": [],
    "settings": { "unit": "Foot", "kerf_width": 1000, "layout": "Guillotine" }
  },
  "optimizer_effort": "Fast"
}"#;

        let error = load_project_document_from_str(source).expect_err("migration must fail");

        assert!(matches!(error, ProjectIoError::DimensionOverflow));
    }

    #[test]
    fn save_and_load_project_file_roundtrip() {
        let path = std::env::temp_dir().join(format!(
            "freecut-project-{}-{}.{}",
            std::process::id(),
            PROJECT_FILE_VERSION,
            PROJECT_FILE_EXTENSION
        ));
        let document = ProjectDocument::new(
            Project {
                name: "file roundtrip".to_string(),
                stock_pieces: Vec::new(),
                cut_pieces: Vec::new(),
                settings: CutSettings {
                    unit: Unit::Foot,
                    kerf_width: parse_length("1/96", Unit::Foot).expect("fixture parses"),
                    layout: LayoutKind::Guillotine,
                    rotation_mode: RotationMode::PerPart,
                },
            },
            OptimizerEffort::Balanced,
        );

        save_project_file(&path, &document).expect("save project");
        let loaded = load_project_file(&path).expect("load project");
        std::fs::remove_file(path).expect("remove project fixture");

        assert_eq!(loaded, document);
    }
}

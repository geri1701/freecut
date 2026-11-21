//! Core project model for Freecut.
//!
//! This layer should stay independent from GUI toolkit, PDF backend, and concrete optimizer.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub stock_pieces: Vec<StockPiece>,
    pub cut_pieces: Vec<CutPiece>,
    pub settings: CutSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    Millimeter,
    Inch,
    Foot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternDirection {
    None,
    ParallelToWidth,
    ParallelToLength,
}

#[must_use]
pub const fn default_cut_pattern(can_rotate: bool) -> PatternDirection {
    if can_rotate {
        PatternDirection::None
    } else {
        PatternDirection::ParallelToLength
    }
}

#[must_use]
pub fn effective_cut_pattern(
    pattern: PatternDirection,
    can_rotate: bool,
    rotation_mode: RotationMode,
) -> PatternDirection {
    if pattern == default_cut_pattern(can_rotate) {
        default_cut_pattern(rotation_mode.allows_rotation(can_rotate))
    } else {
        pattern
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockPiece {
    pub id: PieceId,
    /// Width in `measurement` geometry ticks.
    pub width: u32,
    /// Length in `measurement` geometry ticks.
    pub length: u32,
    pub quantity: Option<u32>,
    pub pattern: PatternDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutPiece {
    pub id: PieceId,
    pub label: String,
    /// Width in `measurement` geometry ticks.
    pub width: u32,
    /// Length in `measurement` geometry ticks.
    pub length: u32,
    pub quantity: u32,
    pub pattern: PatternDirection,
    pub can_rotate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PieceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutSettings {
    /// Unit used to parse and display project measurements.
    pub unit: Unit,
    /// Kerf width in `measurement` geometry ticks.
    pub kerf_width: u32,
    pub layout: LayoutKind,
    #[serde(
        default,
        rename = "rotation_mode",
        alias = "all_parts_rotatable",
        deserialize_with = "deserialize_rotation_mode"
    )]
    pub rotation_mode: RotationMode,
    #[serde(default)]
    pub open_pdf_after_export: bool,
}

fn deserialize_rotation_mode<'de, D>(deserializer: D) -> Result<RotationMode, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum RotationModeOrLegacy {
        Mode(RotationMode),
        LegacyAllPartsRotatable(bool),
    }

    match RotationModeOrLegacy::deserialize(deserializer)? {
        RotationModeOrLegacy::Mode(mode) => Ok(mode),
        RotationModeOrLegacy::LegacyAllPartsRotatable(true) => Ok(RotationMode::All),
        RotationModeOrLegacy::LegacyAllPartsRotatable(false) => Ok(RotationMode::PerPart),
    }
}

#[cfg(test)]
mod tests {
    use super::{effective_cut_pattern, PatternDirection, RotationMode};

    #[test]
    fn rotation_mode_cycles_none_per_part_and_all() {
        assert_eq!(RotationMode::None.next(), RotationMode::PerPart);
        assert_eq!(RotationMode::PerPart.next(), RotationMode::All);
        assert_eq!(RotationMode::All.next(), RotationMode::None);
    }

    #[test]
    fn rotation_mode_applies_override_or_per_part_preference() {
        assert!(!RotationMode::None.allows_rotation(true));
        assert!(!RotationMode::PerPart.allows_rotation(false));
        assert!(RotationMode::PerPart.allows_rotation(true));
        assert!(RotationMode::All.allows_rotation(false));
    }

    #[test]
    fn global_rotation_modes_update_only_default_cut_patterns() {
        assert_eq!(
            effective_cut_pattern(PatternDirection::ParallelToLength, false, RotationMode::All),
            PatternDirection::None
        );
        assert_eq!(
            effective_cut_pattern(
                PatternDirection::ParallelToLength,
                false,
                RotationMode::None
            ),
            PatternDirection::ParallelToLength
        );
        assert_eq!(
            effective_cut_pattern(PatternDirection::ParallelToWidth, false, RotationMode::All),
            PatternDirection::ParallelToWidth
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutKind {
    Guillotine,
    Nested,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationMode {
    None,
    #[default]
    PerPart,
    All,
}

impl RotationMode {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::None => Self::PerPart,
            Self::PerPart => Self::All,
            Self::All => Self::None,
        }
    }

    #[must_use]
    pub const fn allows_rotation(self, per_part_can_rotate: bool) -> bool {
        match self {
            Self::None => false,
            Self::PerPart => per_part_can_rotate,
            Self::All => true,
        }
    }
}

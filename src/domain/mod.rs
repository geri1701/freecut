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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutKind {
    Guillotine,
    Nested,
}

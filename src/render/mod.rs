//! Shared solution/render model.
//!
//! GUI preview, PDF export, and later SVG/PNG export should consume this representation.

use crate::domain::{LayoutKind, PatternDirection, PieceId};

#[derive(Debug, Clone, PartialEq)]
pub struct Solution {
    pub layout: LayoutKind,
    pub sheets: Vec<SolutionSheet>,
    pub fitness: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolutionSheet {
    pub stock_id: PieceId,
    pub width: u32,
    pub length: u32,
    pub placed_pieces: Vec<PlacedPiece>,
    pub waste: Vec<Rect>,
    pub cutting_guide: Option<SliceNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedPiece {
    pub cut_id: PieceId,
    pub instance: u32,
    pub rect: Rect,
    pub pattern: PatternDirection,
    pub rotated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub length: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutOrientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cut {
    work_rect: Rect,
    orientation: CutOrientation,
    offset: u32,
    kerf_width: u32,
}

impl Cut {
    #[must_use]
    pub fn new(
        work_rect: Rect,
        orientation: CutOrientation,
        offset: u32,
        kerf_width: u32,
    ) -> Option<Self> {
        let span = match orientation {
            CutOrientation::Horizontal => work_rect.length,
            CutOrientation::Vertical => work_rect.width,
        };
        let kerf_end = offset.checked_add(kerf_width)?;

        if work_rect.width == 0 || work_rect.length == 0 || offset == 0 || kerf_end >= span {
            return None;
        }

        Some(Self {
            work_rect,
            orientation,
            offset,
            kerf_width,
        })
    }

    #[must_use]
    pub fn work_rect(&self) -> Rect {
        self.work_rect
    }

    #[must_use]
    pub fn orientation(&self) -> CutOrientation {
        self.orientation
    }

    #[must_use]
    pub fn offset(&self) -> u32 {
        self.offset
    }

    #[must_use]
    pub fn kerf_width(&self) -> u32 {
        self.kerf_width
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SliceNode {
    Cut {
        cut: Cut,
        first: Box<SliceNode>,
        second: Box<SliceNode>,
    },
    Leaf {
        rect: Rect,
        kind: LeafKind,
    },
}

impl SliceNode {
    #[must_use]
    pub fn cut(cut: Cut, first: SliceNode, second: SliceNode) -> Self {
        Self::Cut {
            cut,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    #[must_use]
    pub fn leaf(rect: Rect, kind: LeafKind) -> Self {
        Self::Leaf { rect, kind }
    }

    #[must_use]
    pub fn preorder_cuts(&self) -> PreorderCuts<'_> {
        PreorderCuts { stack: vec![self] }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafKind {
    CutPiece { cut_id: PieceId, instance: u32 },
    Waste,
}

pub struct PreorderCuts<'a> {
    stack: Vec<&'a SliceNode>,
}

impl<'a> Iterator for PreorderCuts<'a> {
    type Item = &'a Cut;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.stack.pop() {
            match node {
                SliceNode::Cut { cut, first, second } => {
                    self.stack.push(second);
                    self.stack.push(first);
                    return Some(cut);
                }
                SliceNode::Leaf { .. } => {}
            }
        }

        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutKerfGeometry {
    KerfRect(Rect),
    ZeroKerfLine(CutKerfLine),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutKerfLine {
    pub start_x: u32,
    pub start_y: u32,
    pub end_x: u32,
    pub end_y: u32,
}

#[must_use]
pub fn solution_sheet_kerf_geometries(
    sheet: &SolutionSheet,
    layout: LayoutKind,
) -> Vec<CutKerfGeometry> {
    match layout {
        LayoutKind::Guillotine => sheet
            .cutting_guide
            .as_ref()
            .map(|guide| guide.preorder_cuts().map(cut_kerf_geometry).collect())
            .unwrap_or_default(),
        LayoutKind::Nested => nested_kerf_geometries(sheet),
    }
}

#[must_use]
pub fn cut_kerf_geometry(cut: &Cut) -> CutKerfGeometry {
    let work_rect = cut.work_rect();
    let kerf_width = cut.kerf_width();

    match (cut.orientation(), kerf_width) {
        (CutOrientation::Horizontal, 0) => {
            let y = work_rect.y + cut.offset();
            CutKerfGeometry::ZeroKerfLine(CutKerfLine {
                start_x: work_rect.x,
                start_y: y,
                end_x: work_rect.x + work_rect.width,
                end_y: y,
            })
        }
        (CutOrientation::Vertical, 0) => {
            let x = work_rect.x + cut.offset();
            CutKerfGeometry::ZeroKerfLine(CutKerfLine {
                start_x: x,
                start_y: work_rect.y,
                end_x: x,
                end_y: work_rect.y + work_rect.length,
            })
        }
        (CutOrientation::Horizontal, kerf_width) => CutKerfGeometry::KerfRect(Rect {
            x: work_rect.x,
            y: work_rect.y + cut.offset(),
            width: work_rect.width,
            length: kerf_width,
        }),
        (CutOrientation::Vertical, kerf_width) => CutKerfGeometry::KerfRect(Rect {
            x: work_rect.x + cut.offset(),
            y: work_rect.y,
            width: kerf_width,
            length: work_rect.length,
        }),
    }
}

fn nested_kerf_geometries(sheet: &SolutionSheet) -> Vec<CutKerfGeometry> {
    sheet_uncovered_rects(sheet)
        .into_iter()
        .map(CutKerfGeometry::KerfRect)
        .collect()
}

fn sheet_uncovered_rects(sheet: &SolutionSheet) -> Vec<Rect> {
    let mut remaining = vec![Rect {
        x: 0,
        y: 0,
        width: sheet.width,
        length: sheet.length,
    }];

    for occupied in sheet
        .placed_pieces
        .iter()
        .map(|piece| piece.rect)
        .chain(sheet.waste.iter().copied())
    {
        remaining = remaining
            .into_iter()
            .flat_map(|rect| subtract_rect(rect, occupied))
            .collect();
    }

    remaining
}

fn subtract_rect(rect: Rect, occupied: Rect) -> Vec<Rect> {
    if !rects_intersect(rect, occupied) {
        return vec![rect];
    }

    let rect_right = rect.x + rect.width;
    let rect_bottom = rect.y + rect.length;
    let occupied_right = occupied.x + occupied.width;
    let occupied_bottom = occupied.y + occupied.length;
    let intersection_left = rect.x.max(occupied.x);
    let intersection_top = rect.y.max(occupied.y);
    let intersection_right = rect_right.min(occupied_right);
    let intersection_bottom = rect_bottom.min(occupied_bottom);

    let mut remaining = Vec::new();

    if intersection_top > rect.y {
        remaining.push(Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            length: intersection_top - rect.y,
        });
    }

    if intersection_bottom < rect_bottom {
        remaining.push(Rect {
            x: rect.x,
            y: intersection_bottom,
            width: rect.width,
            length: rect_bottom - intersection_bottom,
        });
    }

    if intersection_left > rect.x {
        remaining.push(Rect {
            x: rect.x,
            y: intersection_top,
            width: intersection_left - rect.x,
            length: intersection_bottom - intersection_top,
        });
    }

    if intersection_right < rect_right {
        remaining.push(Rect {
            x: intersection_right,
            y: intersection_top,
            width: rect_right - intersection_right,
            length: intersection_bottom - intersection_top,
        });
    }

    remaining
}

fn rects_intersect(left: Rect, right: Rect) -> bool {
    left.x < right.x + right.width
        && left.x + left.width > right.x
        && left.y < right.y + right.length
        && left.y + left.length > right.y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(width: u32, length: u32) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width,
            length,
        }
    }

    #[test]
    fn cut_requires_a_real_split_inside_the_work_rect() {
        let work_rect = rect(100, 50);

        assert!(Cut::new(work_rect, CutOrientation::Vertical, 40, 3).is_some());
        assert!(Cut::new(work_rect, CutOrientation::Horizontal, 20, 0).is_some());

        assert_eq!(Cut::new(work_rect, CutOrientation::Vertical, 0, 3), None);
        assert_eq!(Cut::new(work_rect, CutOrientation::Vertical, 97, 3), None);
        assert_eq!(Cut::new(work_rect, CutOrientation::Horizontal, 50, 0), None);
        assert_eq!(Cut::new(rect(0, 50), CutOrientation::Vertical, 10, 0), None);
    }

    #[test]
    fn preorder_cuts_visits_parent_before_child_subtrees() {
        let root_cut = Cut::new(rect(100, 80), CutOrientation::Horizontal, 50, 3).unwrap();
        let first_child_cut = Cut::new(rect(100, 50), CutOrientation::Vertical, 60, 3).unwrap();
        let second_child_cut = Cut::new(rect(100, 27), CutOrientation::Vertical, 30, 0).unwrap();

        let tree = SliceNode::cut(
            root_cut,
            SliceNode::cut(
                first_child_cut,
                SliceNode::leaf(
                    rect(60, 50),
                    LeafKind::CutPiece {
                        cut_id: PieceId(1),
                        instance: 0,
                    },
                ),
                SliceNode::leaf(rect(37, 50), LeafKind::Waste),
            ),
            SliceNode::cut(
                second_child_cut,
                SliceNode::leaf(rect(30, 27), LeafKind::Waste),
                SliceNode::leaf(rect(70, 27), LeafKind::Waste),
            ),
        );

        let visited = tree
            .preorder_cuts()
            .map(|cut| (cut.orientation(), cut.offset(), cut.kerf_width()))
            .collect::<Vec<_>>();

        assert_eq!(
            visited,
            vec![
                (CutOrientation::Horizontal, 50, 3),
                (CutOrientation::Vertical, 60, 3),
                (CutOrientation::Vertical, 30, 0),
            ]
        );
    }

    #[test]
    fn preorder_cuts_skips_leaf_nodes() {
        let tree = SliceNode::leaf(rect(100, 50), LeafKind::Waste);

        assert_eq!(tree.preorder_cuts().next(), None);
    }

    fn solution_sheet(width: u32, length: u32) -> SolutionSheet {
        SolutionSheet {
            stock_id: PieceId(1),
            width,
            length,
            placed_pieces: Vec::new(),
            waste: Vec::new(),
            cutting_guide: None,
        }
    }

    fn placed_piece(x: u32, y: u32, width: u32, length: u32) -> PlacedPiece {
        PlacedPiece {
            cut_id: PieceId(10),
            instance: 0,
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

    #[test]
    fn solution_sheet_kerf_geometries_use_guillotine_guide_preorder() {
        let root_cut = Cut::new(rect(100, 80), CutOrientation::Horizontal, 50, 3).unwrap();
        let child_cut = Cut::new(rect(100, 50), CutOrientation::Vertical, 60, 3).unwrap();
        let mut sheet = solution_sheet(100, 80);
        sheet.cutting_guide = Some(SliceNode::cut(
            root_cut,
            SliceNode::cut(
                child_cut,
                SliceNode::leaf(
                    rect(60, 50),
                    LeafKind::CutPiece {
                        cut_id: PieceId(1),
                        instance: 0,
                    },
                ),
                SliceNode::leaf(rect(37, 50), LeafKind::Waste),
            ),
            SliceNode::leaf(rect(100, 27), LeafKind::Waste),
        ));

        assert_eq!(
            solution_sheet_kerf_geometries(&sheet, LayoutKind::Guillotine),
            vec![
                CutKerfGeometry::KerfRect(Rect {
                    x: 0,
                    y: 50,
                    width: 100,
                    length: 3,
                }),
                CutKerfGeometry::KerfRect(Rect {
                    x: 60,
                    y: 0,
                    width: 3,
                    length: 50,
                }),
            ]
        );
    }

    #[test]
    fn solution_sheet_kerf_geometries_leave_guillotine_without_guide_empty() {
        let sheet = solution_sheet(100, 80);

        assert_eq!(
            solution_sheet_kerf_geometries(&sheet, LayoutKind::Guillotine),
            Vec::new()
        );
    }

    #[test]
    fn solution_sheet_kerf_geometries_use_nested_uncovered_gaps() {
        let mut sheet = solution_sheet(103, 50);
        sheet.placed_pieces = vec![placed_piece(0, 0, 50, 50), placed_piece(53, 0, 50, 50)];

        assert_eq!(
            solution_sheet_kerf_geometries(&sheet, LayoutKind::Nested),
            vec![CutKerfGeometry::KerfRect(Rect {
                x: 50,
                y: 0,
                width: 3,
                length: 50,
            })]
        );
    }
}

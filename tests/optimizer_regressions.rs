use freecut::{
    domain::{
        CutPiece, CutSettings, LayoutKind, PatternDirection, PieceId, Project, StockPiece, Unit,
    },
    optimizer::{BaselineOptimizer, OptimizeError, OptimizerConfig, OptimizerEffort},
    render::Rect,
};

#[test]
fn balanced_optimizer_places_gui_case_after_adding_one_more_small_cut() {
    let project = Project {
        name: "repro".to_string(),
        stock_pieces: vec![StockPiece {
            id: PieceId(1),
            width: 2440,
            length: 1220,
            quantity: Some(1),
            pattern: PatternDirection::None,
        }],
        cut_pieces: vec![
            CutPiece {
                id: PieceId(2),
                label: "cut-2".to_string(),
                width: 800,
                length: 100,
                quantity: 15,
                pattern: PatternDirection::None,
                can_rotate: true,
            },
            CutPiece {
                id: PieceId(3),
                label: "cut-3".to_string(),
                width: 100,
                length: 78,
                quantity: 44,
                pattern: PatternDirection::None,
                can_rotate: true,
            },
            CutPiece {
                id: PieceId(4),
                label: "cut-4".to_string(),
                width: 200,
                length: 60,
                quantity: 10,
                pattern: PatternDirection::None,
                can_rotate: true,
            },
            CutPiece {
                id: PieceId(5),
                label: "cut-5".to_string(),
                width: 150,
                length: 150,
                quantity: 35,
                pattern: PatternDirection::None,
                can_rotate: true,
            },
        ],
        settings: CutSettings {
            unit: Unit::Millimeter,
            kerf_width: 0,
            layout: LayoutKind::Guillotine,
        },
    };

    let solution = BaselineOptimizer
        .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Balanced))
        .expect("should fit");

    assert_eq!(solution.sheets.len(), 1);
    assert_eq!(solution.sheets[0].placed_pieces.len(), 104);
}

#[test]
fn thorough_guillotine_places_q71_side_strip_case() {
    let project = Project {
        name: "side-strip-repro".to_string(),
        stock_pieces: vec![StockPiece {
            id: PieceId(2),
            width: 2440,
            length: 1220,
            quantity: Some(1),
            pattern: PatternDirection::None,
        }],
        cut_pieces: vec![
            CutPiece {
                id: PieceId(1),
                label: "cut-1".to_string(),
                width: 100,
                length: 100,
                quantity: 85,
                pattern: PatternDirection::None,
                can_rotate: true,
            },
            CutPiece {
                id: PieceId(3),
                label: "cut-3".to_string(),
                width: 234,
                length: 344,
                quantity: 21,
                pattern: PatternDirection::None,
                can_rotate: true,
            },
            CutPiece {
                id: PieceId(4),
                label: "cut-4".to_string(),
                width: 40,
                length: 120,
                quantity: 71,
                pattern: PatternDirection::None,
                can_rotate: true,
            },
        ],
        settings: CutSettings {
            unit: Unit::Millimeter,
            kerf_width: 1,
            layout: LayoutKind::Guillotine,
        },
    };

    let solution = BaselineOptimizer
        .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Thorough))
        .expect("q71 side-strip case should fit on one sheet");

    assert_eq!(solution.sheets.len(), 1);
    assert_eq!(solution.sheets[0].placed_pieces.len(), 177);
    assert_eq!(
        solution.sheets[0]
            .placed_pieces
            .iter()
            .filter(|piece| piece.cut_id == PieceId(1))
            .count(),
        85
    );
    assert_eq!(
        solution.sheets[0]
            .placed_pieces
            .iter()
            .filter(|piece| piece.cut_id == PieceId(3))
            .count(),
        21
    );
    assert_eq!(
        solution.sheets[0]
            .placed_pieces
            .iter()
            .filter(|piece| piece.cut_id == PieceId(4))
            .count(),
        71
    );
}

#[test]
fn guillotine_thorough_is_independent_of_stock_input_order_for_issue_35() {
    let width_descending_stock = issue_35_project(vec![
        stock(101, 1500, 4000, 1),
        stock(102, 1500, 3500, 1),
        stock(103, 1250, 5000, 2),
    ]);
    let reported_failing_stock_order = issue_35_project(vec![
        stock(101, 1500, 3500, 1),
        stock(102, 1250, 5000, 2),
        stock(103, 1500, 4000, 1),
    ]);

    for project in [width_descending_stock, reported_failing_stock_order] {
        let solution = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Thorough))
            .expect("issue #35 stock ordering should not decide whether a solution exists");

        assert_eq!(solution.sheets.len(), 4);
        assert_eq!(
            solution
                .sheets
                .iter()
                .map(|sheet| sheet.placed_pieces.len())
                .sum::<usize>(),
            21
        );
        assert_solution_within_bounds_and_non_overlapping(&solution);
    }
}

#[test]
fn nested_places_small_non_guillotine_advantage_case() {
    let nested = nested_project(
        10,
        10,
        vec![
            cut(10, 2, 2, 1, false),
            cut(20, 2, 3, 1, false),
            cut(30, 2, 6, 1, false),
            cut(40, 7, 8, 1, false),
        ],
    );
    let mut guillotine = nested.clone();
    guillotine.settings.layout = LayoutKind::Guillotine;

    let nested_solution = BaselineOptimizer
        .optimize_with_config(&nested, OptimizerConfig::new(OptimizerEffort::Thorough))
        .expect("nested should place this small non-guillotine layout");
    let guillotine_error = BaselineOptimizer
        .optimize_with_config(&guillotine, OptimizerConfig::new(OptimizerEffort::Thorough))
        .expect_err("guillotine should not place this non-sliceable layout with one sheet");

    assert_eq!(guillotine_error, OptimizeError::NoSolution);
    assert_eq!(nested_solution.sheets.len(), 1);
    assert_eq!(nested_solution.sheets[0].placed_pieces.len(), 4);
    assert_solution_within_bounds_and_non_overlapping(&nested_solution);
}

#[test]
fn nested_rotates_narrow_piece_into_remaining_strip() {
    let project = nested_project(
        100,
        60,
        vec![cut(10, 60, 60, 1, false), cut(20, 50, 40, 1, true)],
    );

    let solution = BaselineOptimizer
        .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Thorough))
        .expect("rotated narrow piece should fit in the side strip");

    assert_eq!(solution.sheets.len(), 1);
    assert_eq!(solution.sheets[0].placed_pieces.len(), 2);
    assert!(solution.sheets[0]
        .placed_pieces
        .iter()
        .any(|piece| piece.cut_id == PieceId(20) && piece.rotated));
    assert_solution_within_bounds_and_non_overlapping(&solution);
}

#[test]
fn nested_uses_pattern_wildcard_like_guillotine() {
    let project = Project {
        name: "nested-pattern-wildcard".to_string(),
        stock_pieces: vec![StockPiece {
            id: PieceId(1),
            width: 100,
            length: 100,
            quantity: Some(1),
            pattern: PatternDirection::ParallelToWidth,
        }],
        cut_pieces: vec![cut(10, 50, 50, 1, false)],
        settings: CutSettings {
            unit: Unit::Millimeter,
            kerf_width: 0,
            layout: LayoutKind::Nested,
        },
    };

    let solution = BaselineOptimizer
        .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Fast))
        .expect("patternless cut should fit patterned stock");

    assert_eq!(solution.sheets.len(), 1);
    assert_eq!(solution.sheets[0].placed_pieces.len(), 1);
    assert_eq!(
        solution.sheets[0].placed_pieces[0].pattern,
        PatternDirection::None
    );
}

#[test]
fn nested_respects_finite_stock_quantity() {
    let project = nested_project(100, 100, vec![cut(10, 60, 100, 2, false)]);

    let error = BaselineOptimizer
        .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Thorough))
        .expect_err("one finite stock sheet cannot hold both cuts");

    assert_eq!(error, OptimizeError::NoSolution);
}

#[test]
fn nested_rejects_when_total_cut_area_exceeds_stock_area() {
    let project = nested_project(100, 100, vec![cut(10, 80, 80, 2, false)]);

    let error = BaselineOptimizer
        .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Fast))
        .expect_err("total cut area exceeds available stock area");

    assert_eq!(error, OptimizeError::NoSolution);
}

#[test]
fn nested_rejects_individually_unpassable_cut() {
    let project = nested_project(100, 100, vec![cut(10, 101, 50, 1, false)]);

    let error = BaselineOptimizer
        .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Fast))
        .expect_err("cut is wider than any stock piece");

    assert_eq!(error, OptimizeError::NoSolution);
}

#[test]
fn nested_repeats_deterministically_for_all_effort_levels() {
    let project = nested_project(
        180,
        120,
        vec![
            cut(10, 70, 40, 1, true),
            cut(20, 60, 50, 1, true),
            cut(30, 30, 80, 1, true),
            cut(40, 25, 25, 3, true),
        ],
    );

    for effort in [
        OptimizerEffort::Fast,
        OptimizerEffort::Balanced,
        OptimizerEffort::Thorough,
    ] {
        let first = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(effort))
            .expect("first nested run should produce a solution");
        let second = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(effort))
            .expect("second nested run should produce a solution");

        assert_eq!(first, second);
    }
}

#[test]
fn guillotine_balanced_places_rotation_disabled_small_cut_cases() {
    for disabled_cut_id in [4, 5] {
        let project = rotation_disabled_regression_project(LayoutKind::Guillotine, disabled_cut_id);

        let fast_error = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Fast))
            .expect_err("fast may remain a narrow greedy guillotine pass");
        let balanced_solution = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Balanced))
            .expect("balanced should include enough guillotine variants for this practical case");

        assert_eq!(fast_error, OptimizeError::NoSolution);
        assert_eq!(balanced_solution.sheets.len(), 1);
        assert_eq!(balanced_solution.sheets[0].placed_pieces.len(), 47);
        assert_solution_within_bounds_and_non_overlapping(&balanced_solution);
    }
}

#[test]
fn nested_balanced_and_thorough_place_wide_panel_rotation_disabled_case() {
    let project = rotation_disabled_regression_project(LayoutKind::Nested, 2);

    for effort in [OptimizerEffort::Balanced, OptimizerEffort::Thorough] {
        let solution = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(effort))
            .expect("nested should handle the wide-panel case without requiring rotation");

        assert_eq!(solution.sheets.len(), 1);
        assert_eq!(solution.sheets[0].placed_pieces.len(), 47);
        assert_solution_within_bounds_and_non_overlapping(&solution);
    }
}

fn nested_project(stock_width: u32, stock_length: u32, cut_pieces: Vec<CutPiece>) -> Project {
    Project {
        name: "nested-regression".to_string(),
        stock_pieces: vec![StockPiece {
            id: PieceId(1),
            width: stock_width,
            length: stock_length,
            quantity: Some(1),
            pattern: PatternDirection::None,
        }],
        cut_pieces,
        settings: CutSettings {
            unit: Unit::Millimeter,
            kerf_width: 0,
            layout: LayoutKind::Nested,
        },
    }
}

fn rotation_disabled_regression_project(layout: LayoutKind, disabled_cut_id: u64) -> Project {
    Project {
        name: "rotation-disabled-regression".to_string(),
        stock_pieces: vec![StockPiece {
            id: PieceId(1),
            width: 2440,
            length: 1220,
            quantity: Some(1),
            pattern: PatternDirection::None,
        }],
        cut_pieces: vec![
            cut(2, 500, 620, 4, disabled_cut_id != 2),
            cut(3, 1223, 220, 3, disabled_cut_id != 3),
            cut(4, 110, 100, 30, disabled_cut_id != 4),
            cut(5, 100, 200, 10, disabled_cut_id != 5),
        ],
        settings: CutSettings {
            unit: Unit::Millimeter,
            kerf_width: 2,
            layout,
        },
    }
}

fn issue_35_project(stock_pieces: Vec<StockPiece>) -> Project {
    Project {
        name: "issue-35-stock-order".to_string(),
        stock_pieces,
        cut_pieces: vec![
            cut(1, 551, 2210, 3, true),
            cut(2, 500, 993, 2, true),
            cut(3, 700, 1003, 2, true),
            cut(4, 750, 2026, 1, true),
            cut(5, 500, 863, 2, true),
            cut(6, 700, 942, 2, true),
            cut(7, 551, 2089, 1, true),
            cut(8, 751, 1662, 1, true),
            cut(9, 751, 1781, 1, true),
            cut(10, 551, 1321, 1, true),
            cut(11, 551, 1262, 1, true),
            cut(12, 551, 2147, 1, true),
            cut(13, 750, 1901, 1, true),
            cut(14, 750, 2026, 1, true),
            cut(15, 550, 1884, 1, true),
        ],
        settings: CutSettings {
            unit: Unit::Millimeter,
            kerf_width: 0,
            layout: LayoutKind::Guillotine,
        },
    }
}

fn stock(id: u64, width: u32, length: u32, quantity: u32) -> StockPiece {
    StockPiece {
        id: PieceId(id),
        width,
        length,
        quantity: Some(quantity),
        pattern: PatternDirection::None,
    }
}

fn cut(id: u64, width: u32, length: u32, quantity: u32, can_rotate: bool) -> CutPiece {
    CutPiece {
        id: PieceId(id),
        label: format!("cut-{id}"),
        width,
        length,
        quantity,
        pattern: PatternDirection::None,
        can_rotate,
    }
}

fn assert_solution_within_bounds_and_non_overlapping(solution: &freecut::render::Solution) {
    for sheet in &solution.sheets {
        for (index, placed) in sheet.placed_pieces.iter().enumerate() {
            assert!(
                placed.rect.x + placed.rect.width <= sheet.width,
                "placed piece {index} exceeds sheet width"
            );
            assert!(
                placed.rect.y + placed.rect.length <= sheet.length,
                "placed piece {index} exceeds sheet length"
            );

            for other in sheet.placed_pieces.iter().skip(index + 1) {
                assert!(
                    !rects_overlap(placed.rect, other.rect),
                    "placed pieces overlap: {:?} and {:?}",
                    placed.rect,
                    other.rect
                );
            }
        }
    }
}

fn rects_overlap(left: Rect, right: Rect) -> bool {
    left.x < right.x + right.width
        && left.x + left.width > right.x
        && left.y < right.y + right.length
        && left.y + left.length > right.y
}

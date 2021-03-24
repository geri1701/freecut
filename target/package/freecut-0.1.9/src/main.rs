#![windows_subsystem = "windows"]
use crate::co::Optimizer;
use crate::co::{CutPiece, Solution, StockPiece};
use comfy_table::Table;
use fltk::{app::*, browser::*, button::*, group::*, input::*, misc::*, text::*, window::*};
use rand::prelude::*;
mod func;
use cut_optimizer_2d as co;
use func::{create_solution_pdf, cutpiece_exists, read_user_input, stockpiece_exists};
fn main() {
    let mut used_id: Vec<usize> = Vec::new();
    let mut optimizer = Optimizer::new();
    let app = App::default().with_scheme(AppScheme::Gleam);
    let mut wind = Window::new(0, 0, 600, 380, "freecut");
    fltk::app::set_font(fltk::enums::Font::Courier);
    let mut tabs = Tabs::new(0, 0, 600, 380, "");
    tabs.end();
    let mut opt_tab = Group::new(0, 27, 580, 340, "Optimizer");
    opt_tab.end();
    opt_tab.show();
    tabs.add(&opt_tab);
    let mut man_tab = Group::new(0, 27, 580, 340, "Manual");
    man_tab.end();
    man_tab.hide();
    tabs.add(&man_tab);
    let mut manual_display = TextDisplay::new(5, 27, 580, 340, "");
    man_tab.add(&manual_display);
    let manual = include_str!("manual.md").to_owned();
    let manual_buffer = TextBuffer::default();
    manual_display.set_buffer(Some(manual_buffer));
    manual_display.wrap_mode(WrapMode::AtColumn, 55);
    manual_display.set_scrollbar_size(15);
    manual_display.insert(&manual);
    let mut license_tab = Group::new(0, 27, 580, 340, "License");
    license_tab.end();
    license_tab.hide();
    tabs.add(&license_tab);
    let mut license_display = TextDisplay::new(5, 27, 580, 340, "");
    license_tab.add(&license_display);
    let license = include_str!("license.md").to_owned();
    let license_buffer = TextBuffer::default();
    license_display.set_buffer(Some(license_buffer));
    license_display.wrap_mode(WrapMode::AtColumn, 80);
    license_display.set_scrollbar_size(15);
    license_display.insert(&license);
    let input_width = FloatInput::new(200, 45, 65, 25, "width (mm):");
    opt_tab.add(&input_width);
    let input_length = FloatInput::new(365, 45, 65, 25, "length (mm):");
    opt_tab.add(&input_length);
    let amount = FloatInput::new(495, 45, 65, 24, "amount:");
    opt_tab.add(&amount);
    let mut pattern = InputChoice::new(200, 75, 75, 25, "pattern (parallel to):");
    opt_tab.add(&pattern);
    pattern.add("none");
    pattern.add("width");
    pattern.add("length");
    let mut piece_choice = InputChoice::new(365, 75, 105, 25, "piece type");
    opt_tab.add(&piece_choice);
    piece_choice.add("cutpiece");
    piece_choice.add("stockpiece");
    let mut add = Button::new(475, 75, 85, 25, "add piece");
    opt_tab.add(&add);
    let mut output = Browser::new(50, 105, 500, 200, "");
    output.set_label_font(fltk::enums::Font::Courier);
    output.set_scrollbar_size(15);
    opt_tab.add(&output);
    let cut_width = FloatInput::new(230, 315, 65, 25, "cut_width (mm):");
    opt_tab.add(&cut_width);
    let mut optimizer_layout = InputChoice::new(365, 315, 95, 25, "layout:");
    opt_tab.add(&optimizer_layout);
    optimizer_layout.add("guillotine");
    optimizer_layout.add("nested");
    let mut opt = Button::new(475, 315, 85, 25, "optimize");
    opt_tab.add(&opt);
    let mut res = Button::new(10, 315, 80, 25, "reset");
    opt_tab.add(&res);
    opt_tab.end();
    wind.end();
    wind.show();
    let (sa, ra) = fltk::app::channel();
    add.emit(sa, "add");
    res.emit(sa, "reset");
    opt.emit(sa, "opt");
    let allowed_pattern = [
        Some("none".to_string()),
        Some("width".to_string()),
        Some("length".to_string()),
    ];
    let allowed_range = 1.0..100000.1;
    let allowed_cutw_range = 0.0..15.0;
    let mut pieces_vec: Vec<Vec<String>> = Vec::new();
    let mut table = Table::new();
    table.set_header(vec!["Type", "width", "length", "pattern", "amount"]);
    let mut add_button_pressed = false;
    let mut opt_button_pressed = false;
    let mut res_button_pressed = false;
    let mut output_altered = false;
    let error_output =
        "Add at least one stockpiece to the draft list!\nAdd at least one cutpiece to the draft list!\n";
    let err_stockpiece = "Add at least one stockpiece to the draft list!";
    let err_cutpiece = "Add at least one cutpiece to the draft list!";
    let err_amount = "Amount is out of range!";
    let err_width = "Width is out of range!";
    let err_lenght = "Lenght is out of range!";
    let err_pattern = "Add none or a direction in pattern field!";
    let mut output_string = format!("{}\n{}", error_output, table);
    let mut cutvec: Vec<CutPiece> = Vec::new();
    let mut stockvec: Vec<StockPiece> = Vec::new();
    while app.wait() {
        if let Some(msg) = ra.recv() {
            match msg {
                "add" => {
                    add_button_pressed = true;
                }
                "reset" => {
                    res_button_pressed = true;
                }
                "opt" => {
                    opt_button_pressed = true;
                }
                _ => {
                    add_button_pressed = false;
                }
            }
        };
        if res_button_pressed {
            res_button_pressed = false;
            pieces_vec.clear();
            optimizer = Optimizer::new();
            cutvec.clear();
            stockvec.clear();
            output_string = "".to_string();
            let mut table = Table::new();
            table.set_header(vec!["Type", "width", "length", "pattern", "amount"]);
            output_altered = true;
        }
        if output_altered {
            output.clear();
            for (idx, line) in output_string.lines().enumerate() {
                output.insert(idx as u32 + 1, line);
            }
            output_altered = false;
        }
        if add_button_pressed {
            output_altered = true;
            add_button_pressed = false;
            if input_width.value().is_empty()
                || input_length.value().is_empty()
                || amount.value().is_empty()
                || piece_choice.value().is_none()
            {
                output_string = format!("All fields must have a value!\n{}", table);
            } else {
                match (
                    stockpiece_exists(piece_choice.value(), pieces_vec.clone()),
                    cutpiece_exists(piece_choice.value(), pieces_vec.clone()),
                    allowed_range.contains(&input_width.value().parse::<f32>().unwrap()),
                    allowed_range.contains(&input_length.value().parse::<f32>().unwrap()),
                    allowed_range.contains(&amount.value().parse::<f32>().unwrap()),
                    allowed_pattern.contains(&pattern.value()),
                ) {
                    (false, false, false, false, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n{}\n{}\n",
                                err_stockpiece,
                                err_cutpiece,
                                err_width,
                                err_lenght,
                                err_amount,
                                err_pattern
                            ),
                            table
                        )
                    }
                    (false, false, false, false, false, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_cutpiece, err_width, err_lenght, err_amount
                            ),
                            table
                        )
                    }
                    (true, false, false, false, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n{}\n",
                                err_cutpiece, err_width, err_lenght, err_amount, err_pattern
                            ),
                            table
                        )
                    }
                    (true, false, false, false, false, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_cutpiece, err_width, err_lenght, err_amount
                            ),
                            table
                        )
                    }
                    (false, false, true, false, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_cutpiece, err_lenght, err_amount, err_pattern
                            ),
                            table
                        )
                    }
                    (false, false, true, false, false, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_cutpiece, err_lenght, err_amount
                            ),
                            table
                        )
                    }
                    (false, false, false, true, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_cutpiece, err_width, err_amount, err_pattern
                            ),
                            table
                        )
                    }
                    (false, false, false, true, false, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_cutpiece, err_width, err_amount
                            ),
                            table
                        )
                    }
                    (false, true, false, false, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_width, err_lenght, err_pattern
                            ),
                            table
                        )
                    }

                    (false, false, false, false, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_cutpiece, err_width, err_lenght, err_pattern
                            ),
                            table
                        )
                    }
                    (false, false, false, false, true, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_cutpiece, err_width, err_lenght
                            ),
                            table
                        )
                    }
                    (false, false, true, false, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_cutpiece, err_lenght, err_pattern
                            ),
                            table
                        )
                    }
                    (false, false, true, false, true, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_stockpiece, err_cutpiece, err_lenght),
                            table
                        )
                    }
                    (true, false, false, true, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_cutpiece, err_width, err_amount, err_pattern
                            ),
                            table
                        )
                    }
                    (true, false, false, true, false, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_cutpiece, err_width, err_amount),
                            table
                        )
                    }
                    (true, false, false, false, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_cutpiece, err_width, err_lenght, err_pattern
                            ),
                            table
                        )
                    }
                    (true, false, false, false, true, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_cutpiece, err_width, err_lenght),
                            table
                        )
                    }
                    (false, false, true, true, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_cutpiece, err_amount, err_pattern
                            ),
                            table
                        )
                    }
                    (false, false, true, true, false, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_stockpiece, err_cutpiece, err_amount),
                            table
                        )
                    }
                    (false, false, false, true, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_cutpiece, err_width, err_pattern
                            ),
                            table
                        )
                    }
                    (false, false, false, true, true, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_stockpiece, err_cutpiece, err_width),
                            table
                        )
                    }
                    (true, false, true, true, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_cutpiece, err_amount, err_pattern),
                            table
                        )
                    }
                    (true, false, true, true, false, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n", err_cutpiece, err_amount),
                            table
                        )
                    }
                    (true, false, true, false, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_cutpiece, err_lenght, err_pattern),
                            table
                        )
                    }
                    (true, false, true, false, true, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n", err_cutpiece, err_lenght),
                            table
                        )
                    }
                    (true, false, false, true, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_cutpiece, err_width, err_pattern),
                            table
                        )
                    }
                    (true, false, false, true, true, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n", err_cutpiece, err_width),
                            table
                        )
                    }
                    (false, true, false, true, true, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n", err_stockpiece, err_width),
                            table
                        )
                    }
                    (false, false, true, true, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_stockpiece, err_cutpiece, err_pattern),
                            table
                        )
                    }
                    (false, false, true, true, true, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n", err_stockpiece, err_cutpiece),
                            table
                        )
                    }
                    (true, false, true, false, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_cutpiece, err_lenght, err_amount, err_pattern
                            ),
                            table
                        )
                    }
                    (true, false, true, false, false, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_cutpiece, err_lenght, err_amount),
                            table
                        )
                    }
                    (true, false, true, true, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n", err_cutpiece, err_pattern),
                            table
                        )
                    }
                    (true, false, true, true, true, true) => {
                        output_string = format!("{}\n{}", format!("{}\n", err_cutpiece), table)
                    }
                    (false, true, true, true, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n", err_stockpiece, err_pattern),
                            table
                        )
                    }
                    (false, true, true, true, true, true) => {
                        output_string = format!("{}\n{}", format!("{}\n", err_stockpiece), table)
                    }
                    (false, true, false, true, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_stockpiece, err_width, err_pattern),
                            table
                        )
                    }
                    (false, true, false, false, true, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}", err_stockpiece, err_width, err_lenght),
                            table
                        )
                    }
                    (false, true, false, false, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_width, err_lenght, err_amount, err_pattern
                            ),
                            table
                        )
                    }
                    (false, true, false, false, false, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_width, err_lenght, err_amount
                            ),
                            table
                        )
                    }
                    (false, true, false, true, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_width, err_amount, err_pattern
                            ),
                            table
                        )
                    }
                    (false, true, false, true, false, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_stockpiece, err_width, err_amount),
                            table
                        )
                    }
                    (false, true, true, false, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_stockpiece, err_lenght, err_pattern),
                            table
                        )
                    }
                    (false, true, true, false, true, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n", err_stockpiece, err_lenght),
                            table
                        )
                    }
                    (false, true, true, false, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_stockpiece, err_lenght, err_amount, err_pattern
                            ),
                            table
                        )
                    }
                    (false, true, true, false, false, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_stockpiece, err_lenght, err_amount),
                            table
                        )
                    }
                    (false, true, true, true, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_stockpiece, err_amount, err_pattern),
                            table
                        )
                    }
                    (false, true, true, true, false, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n", err_stockpiece, err_amount),
                            table
                        )
                    }
                    (true, true, false, false, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_width, err_lenght, err_pattern),
                            table
                        )
                    }
                    (true, true, false, false, true, true) => {
                        output_string =
                            format!("{}\n{}", format!("{}\n{}\n", err_width, err_lenght), table)
                    }
                    (true, true, true, false, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_lenght, err_amount, err_pattern),
                            table
                        )
                    }
                    (true, true, true, false, false, true) => {
                        output_string =
                            format!("{}\n{}", format!("{}\n{}\n", err_lenght, err_amount), table)
                    }
                    (true, true, true, false, true, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n", err_lenght, err_pattern),
                            table
                        )
                    }
                    (true, true, true, false, true, true) => {
                        output_string = format!("{}\n{}", format!("{}\n", err_lenght), table)
                    }
                    (true, true, true, true, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n", err_amount, err_pattern),
                            table
                        )
                    }
                    (true, true, true, true, false, true) => {
                        output_string = format!("{}\n{}", format!("{}\n", err_amount), table)
                    }
                    (true, true, false, true, true, false) => {
                        output_string =
                            format!("{}\n{}", format!("{}\n{}\n", err_width, err_pattern), table)
                    }
                    (true, true, false, true, true, true) => {
                        output_string = format!("{}\n{}", format!("{}\n", err_width), table)
                    }
                    (true, true, false, false, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!(
                                "{}\n{}\n{}\n{}\n",
                                err_width, err_lenght, err_amount, err_pattern
                            ),
                            table
                        )
                    }
                    (true, true, false, false, false, true) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}", err_width, err_lenght, err_amount),
                            table
                        )
                    }
                    (true, true, false, true, false, false) => {
                        output_string = format!(
                            "{}\n{}",
                            format!("{}\n{}\n{}\n", err_width, err_amount, err_pattern),
                            table
                        )
                    }
                    (true, true, false, true, false, true) => {
                        output_string =
                            format!("{}\n{}", format!("{}\n{}\n", err_width, err_amount), table)
                    }
                    (true, true, true, true, true, false) => {
                        output_string = format!("{}\n{}", err_pattern, table)
                    }
                    (true, true, true, true, true, true) => {
                        pieces_vec.push(vec![
                            piece_choice.value().unwrap().to_string(),
                            input_width.value().to_string(),
                            input_length.value(),
                            pattern.value().unwrap(),
                            amount.value(),
                        ]);
                        table = Table::new();
                        table.set_header(vec!["Type", "width", "length", "pattern", "amount"]);
                        for piece in &pieces_vec {
                            table.add_row(vec![
                                piece[0].clone(),
                                piece[1].clone(),
                                piece[2].clone(),
                                piece[3].clone(),
                                piece[4].clone(),
                            ]);
                        }
                        output_string = format!("{}", table);
                    }
                }
            }
        }
        if opt_button_pressed {
            output_altered = true;
            opt_button_pressed = false;
            if cut_width.value().is_empty()
                || optimizer_layout.value().is_none()
                || input_width.value().is_empty()
                || input_length.value().is_empty()
                || amount.value().is_empty()
                || piece_choice.value().is_none()
                || !allowed_cutw_range.contains(&cut_width.value().parse::<f32>().unwrap())
            {
                output_string = format!(
                    "{}\n{}",
                    "All fields must have a value,", "allowed cutwidth is between 0 and 15mm"
                );
                output.clear();
                output.insert(1, &output_string);
            } else {
                let cutwidth = cut_width.value().parse().unwrap();
                let (stockvec, cutvec) = read_user_input(&mut used_id, pieces_vec.clone());
                Optimizer::add_stock_pieces(&mut optimizer, stockvec);
                Optimizer::add_cut_pieces(&mut optimizer, cutvec);
                let rand_num = random::<u64>();
                Optimizer::set_random_seed(&mut optimizer, rand_num);
                Optimizer::set_cut_width(&mut optimizer, cutwidth);
                let callback = |_| ();
                let result = if optimizer_layout.value() == Some("guillotine".to_string()) {
                    Optimizer::optimize_guillotine(&optimizer, callback)
                } else {
                    Optimizer::optimize_nested(&optimizer, callback)
                };
                let solution: Solution;
                match result {
                    Ok(s) => {
                        solution = s;
                        create_solution_pdf(&rand_num.to_string(), solution);
                        output_string =
                            format!("{} solution_{}.pdf saved to disk!", "Outputfile", rand_num);
                        output.clear();
                        output.insert(1, &output_string);
                    }
                    Err(_) => {
                        output_string = format!(
                            "{}\n{}\n{}",
                            "No solution, invalid input!",
                            "If a pattern is selected for a stockpiece,",
                            "you have to choose a possible pattern for all of the cutpieces!"
                        );
                        output.clear();
                        output.insert(1, &output_string);
                    }
                }
            }
        }
    }
}

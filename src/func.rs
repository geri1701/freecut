use cut_optimizer_2d::PatternDirection::{None, ParallelToLength, ParallelToWidth};
use cut_optimizer_2d::{CutPiece, Solution, StockPiece};
use pdf_canvas::graphicsstate::Color;
use pdf_canvas::{BuiltinFont, Pdf};
pub fn create_solution_pdf(name: &str, solution: Solution) {
    let filename = format!("{}_{}.{}", "solution", name, "pdf");
    let mut document = Pdf::create(&filename).expect("Create pdf file");
    let font = BuiltinFont::Courier;
    let pt = 2.834;
    let mut text_output = Vec::new();
    let (mut doc_width, mut doc_lenght, mut f, mut x_os, mut y_os);
    doc_width = 595.0;
    doc_lenght = 842.0;
    let mut stp_n = 1;
    for stp in solution.stock_pieces {
        text_output.push(format!("Stockpiece {}:", stp_n));
        text_output.push("----------------".to_string());
        if stp.width > stp.length {
            doc_lenght = 595.0;
            doc_width = 842.0;
        }
        f = scale_fac(
            doc_width,
            doc_lenght,
            stp.width as f32 * pt,
            stp.length as f32 * pt,
        );
        y_os = (doc_lenght - ((stp.length as f32 * pt) / f)) / 2.0;
        x_os = (doc_width - ((stp.width as f32 * pt) / f)) / 2.0;
        document
            .render_page(doc_width, doc_lenght, |canvas| {
                canvas.set_stroke_color(Color::rgb(0, 0, 248))?;
                canvas.set_fill_color(Color::gray(128))?;
                canvas.rectangle(
                    x_os,
                    y_os,
                    (stp.width as f32 * pt) / f,
                    (stp.length as f32 * pt) / f,
                )?;
                canvas.fill()?;
                canvas.stroke()?;
                canvas.rectangle(
                    x_os,
                    y_os,
                    (stp.width as f32 * pt) / f,
                    (stp.length as f32 * pt) / f,
                )?;
                canvas.close_and_stroke()?;
                stp_n += 1;
                for cutp in stp.cut_pieces {
                    text_output.push(format!(
                        "Id{}: {} x {}mm",
                        cutp.external_id, cutp.width, cutp.length
                    ));
                    canvas.set_stroke_color(Color::rgb(0, 248, 0))?;
                    canvas.rectangle(
                        ((cutp.x as f32 * pt) / f) + x_os,
                        ((cutp.y as f32 * pt) / f) + y_os,
                        (cutp.width as f32 * pt) / f,
                        (cutp.length as f32 * pt) / f,
                    )?;
                    canvas.set_fill_color(Color::gray(255))?;
                    canvas.fill()?;
                    canvas.set_stroke_color(Color::rgb(0, 248, 0))?;
                    canvas.rectangle(
                        ((cutp.x as f32 * pt) / f) + x_os,
                        ((cutp.y as f32 * pt) / f) + y_os,
                        (cutp.width as f32 * pt) / f,
                        (cutp.length as f32 * pt) / f,
                    )?;
                    canvas.stroke()?;
                    canvas.set_fill_color(Color::gray(128))?;
                    canvas.left_text(
                        ((cutp.x as f32 * pt) / f) + x_os + ((cutp.width as f32 * pt / f) / 24_f32),
                        ((cutp.y as f32 * pt) / f)
                            + y_os
                            + ((cutp.length as f32 * pt / f) / 24_f32),
                        font,
                        8.0,
                        &cutp.external_id.to_string(),
                    )?;
                    canvas.fill()?;
                }
                Ok(())
            })
            .expect("Write page");
    }
    let mut linecount = 0;
    let mut new_page = true;
    let x = if doc_width > doc_lenght { 728 } else { 620 };
    while new_page && !text_output.is_empty() {
        document
            .render_page(doc_width, doc_lenght, |canvas| {
                canvas.set_fill_color(Color::gray(1))?;
                let mut h = 0.0;
                let mut w = 0.0;
                linecount = 0;
                for _ in 0..x {
                    if doc_width > doc_lenght {
                        if h > doc_width - 300.0 {
                            h = 0.0;
                            w += 100.0;
                        }
                    } else if h > doc_lenght - 100.0 {
                        h = 0.0;
                        w += 100.0;
                    }
                    if linecount % 2 == 0 {
                        canvas.set_fill_color(Color::gray(128))?;
                    } else {
                        canvas.set_fill_color(Color::gray(1))?;
                    }
                    canvas.left_text(
                        25.0 + w,
                        (doc_lenght - 25.0) - h,
                        font,
                        8.0,
                        &text_output.remove(0),
                    )?;
                    h += 6.0;
                    linecount += 1;
                    if text_output.is_empty() {
                        new_page = false;
                        break;
                    }
                }
                Ok(())
            })
            .expect("Write page");
    }
    document.finish().expect("Finish pdf document");
}
pub fn scale_fac(doc_width: f32, doc_lenght: f32, width: f32, length: f32) -> f32 {
    let fac;
    if doc_width > doc_lenght && width > length {
        fac = width / (doc_width - 50.0);
    } else if doc_width > doc_lenght && length > width {
        fac = length / (doc_width - 50.0);
    } else if doc_lenght > doc_width && width > length {
        fac = width / (doc_lenght - 50.0);
    } else if doc_lenght > doc_width && length > width {
        fac = length / (doc_lenght - 50.0);
    } else {
        fac = length / (doc_width - 50.0);
    }
    fac
}

pub fn generate_uid(used_id: &mut Vec<usize>) -> usize {
    let mut result = 0;
    for id in 1.. {
        if !used_id.contains(&id) {
            used_id.push(id);
            result = id;
            break;
        }
    }
    result
}

pub fn read_user_input(
    used_id: &mut Vec<usize>,
    pieces_vec: Vec<Vec<String>>,
) -> (Vec<StockPiece>, Vec<CutPiece>) {
    let mut result_stock = Vec::new();
    let mut result_cut = Vec::new();
    for piece in pieces_vec {
        if piece[0] == "stockpiece" {
            let width = piece[1].parse::<usize>().unwrap();
            let length = piece[2].parse::<usize>().unwrap();
            let pattern_direction = match &piece[3] as &str {
                "width" => ParallelToWidth,
                "length" => ParallelToLength,
                _ => None,
            };
            let amount = piece[4].parse::<usize>().unwrap();
            for _ in 0..amount {
                let stockpiece: StockPiece = StockPiece {
                    width,
                    length,
                    pattern_direction,
                };
                result_stock.push(stockpiece);
            }
        }
        if piece[0] == "cutpiece" {
            let width = piece[1].parse::<usize>().unwrap();
            let length = piece[2].parse::<usize>().unwrap();
            let pattern_direction = match &piece[3] as &str {
                "width" => ParallelToWidth,
                "length" => ParallelToLength,
                _ => None,
            };
            let amount = piece[4].parse::<usize>().unwrap();
            for _ in 0..amount {
                let cutpiece: CutPiece = CutPiece {
                    external_id: generate_uid(used_id),
                    width,
                    length,
                    can_rotate: true,
                    pattern_direction,
                };
                result_cut.push(cutpiece);
            }
        }
    }

    (result_stock, result_cut)
}
pub fn stockpiece_exists(choice_val: Option<String>, pieces_vec: Vec<Vec<String>>) -> bool {
    let mut exists = false;
    if choice_val == Some("stockpiece".to_string()) || pieces_vec.is_empty() {
        exists = true;
    } else {
        for piece in pieces_vec {
            if piece[0] == "stockpiece" {
                exists = true;
            }
        }
    }
    exists
}
pub fn cutpiece_exists(choice_val: Option<String>, pieces_vec: Vec<Vec<String>>) -> bool {
    let mut exists = false;
    if choice_val == Some("cutpiece".to_string()) || pieces_vec.is_empty() {
        exists = true;
    } else {
        for piece in pieces_vec {
            if piece[0] == "cutpiece" {
                exists = true;
            }
        }
    }
    exists
}

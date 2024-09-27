use {
    cascade::cascade,
    cut_optimizer_2d::{CutPiece, Optimizer, PatternDirection, Solution, StockPiece},
    pdf_canvas::{graphicsstate::Color, BuiltinFont, Pdf},
    serde::{Deserialize, Serialize},
    std::{env, fs},
    uom::si::{
        f32::Length,
        length::{foot, inch, millimeter, point_computer},
    },
};

pub fn create_solution_pdf(random_seed: u64, solution: Solution, unit: i32) {
    let mut document =
        Pdf::create(&format!("solution_{random_seed}.pdf")).expect("Create pdf file");
    let pt = match unit {
        1 => Length::new::<inch>(1.0).get::<point_computer>(),
        2 => Length::new::<foot>(1.0).get::<point_computer>(),
        _ => Length::new::<millimeter>(1.0).get::<point_computer>(),
    };
    dbg!(pt);
    dbg!(unit);
    let mut text_output = Vec::new();
    let (mut doc_width, mut doc_lenght, mut x_os, mut y_os);
    doc_width = 595.0;
    doc_lenght = 842.0;
    let mut stp_n = 1;
    for stp in solution.stock_pieces {
        text_output.push(format!("Stockpiece {stp_n}:"));
        text_output.push("----------------".to_string());
        if stp.width > stp.length {
            doc_lenght = 595.0;
            doc_width = 842.0;
        }
        let f = scale_fac(
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
                let short_unit = ["mm", "in", "ft"];
                for cutp in stp.cut_pieces {
                    let output_width = match unit {
                        1 => Length::new::<millimeter>(cutp.width as f32).get::<inch>(),
                        2 => Length::new::<millimeter>(cutp.width as f32).get::<foot>(),
                        _ => cutp.width as f32,
                    };
                    let output_length = match unit {
                        1 => Length::new::<millimeter>(cutp.length as f32).get::<inch>(),
                        2 => Length::new::<millimeter>(cutp.length as f32).get::<foot>(),
                        _ => cutp.length as f32,
                    };
                    text_output.push(format!(
                        "Id{}: {} x {}{}",
                        cutp.external_id.unwrap(),
                        output_width.round(),
                        output_length.round(),
                        short_unit[unit as usize]
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
                        BuiltinFont::Courier,
                        8.0,
                        &cutp.external_id.unwrap().to_string(),
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
                        BuiltinFont::Courier,
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
    if doc_width > doc_lenght && width > length {
        width / (doc_width - 50.0)
    } else if doc_width > doc_lenght && length > width {
        length / (doc_width - 50.0)
    } else if doc_lenght > doc_width && width > length {
        width / (doc_lenght - 50.0)
    } else if doc_lenght > doc_width && length > width {
        length / (doc_lenght - 50.0)
    } else {
        length / (doc_width - 50.0)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Piece {
    width: f32,
    length: f32,
    amount: usize,
    pattern: i32,
    kind: i32,
}

impl Piece {
    pub fn default() -> Self {
        Self {
            width: 1f32,
            length: 1f32,
            amount: 1,
            pattern: 0,
            kind: 0,
        }
    }
    pub fn width(&self) -> String {
        self.width.to_string()
    }
    pub fn length(&self) -> String {
        self.length.to_string()
    }
    pub fn amount(&self) -> String {
        self.amount.to_string()
    }
    pub fn pattern(&self) -> i32 {
        self.pattern
    }
    pub fn kind(&self) -> i32 {
        self.kind
    }
    pub fn set_width(&mut self, value: f32) {
        self.width = value;
    }
    pub fn set_length(&mut self, value: f32) {
        self.length = value;
    }
    pub fn set_amount(&mut self, value: usize) {
        self.amount = value;
    }
    pub fn set_pattern(&mut self, value: i32) {
        self.pattern = value;
    }
    pub fn set_kind(&mut self, value: i32) {
        self.kind = value;
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Model {
    unit: i32,
    layout: i32,
    width: f32,
    pieces: Vec<Piece>,
}

impl Model {
    pub fn default() -> Self {
        if let Ok(value) = fs::read(file()) {
            if let Ok(value) = rmp_serde::from_slice::<Self>(&value) {
                return value;
            }
        };
        Self {
            unit: 0,
            layout: 0,
            width: 3f32,
            pieces: Vec::from([Piece::default()]),
        }
    }
    pub fn allowed_range(&self, value: &f32) -> bool {
        const MIN: f32 = 1f32;
        const MAX: f32 = 100000f32;
        match self.unit {
            1 => (Length::new::<millimeter>(MIN).get::<inch>()
                ..=Length::new::<millimeter>(MAX).get::<inch>())
                .contains(value),
            2 => (Length::new::<millimeter>(MIN).get::<foot>()
                ..=Length::new::<millimeter>(MAX).get::<foot>())
                .contains(value),
            _ => (MIN..=100000f32).contains(value),
        }
    }
    pub fn save(&self) {
        fs::write(file(), rmp_serde::to_vec(&self).unwrap()).unwrap();
    }
    pub fn pieces(&self) -> &Vec<Piece> {
        &self.pieces
    }
    pub fn piece(&self) -> &Piece {
        &self.pieces[self.pieces.len() - 1]
    }
    pub fn width(&self) -> String {
        self.width.to_string()
    }
    pub fn unit(&self) -> i32 {
        self.unit
    }
    pub fn layout(&self) -> i32 {
        self.layout
    }
    pub fn clean(&mut self) {
        self.pieces = Vec::from([Piece::default()]);
    }
    pub fn set_width(&mut self, value: f32) {
        self.width = value;
    }
    pub fn set_unit(&mut self, value: i32) {
        self.unit = value;
    }
    pub fn set_layout(&mut self, value: i32) {
        self.layout = value;
    }
    fn last(&self) -> usize {
        self.pieces.len() - 1
    }
    fn uom(&self, value: f32) -> usize {
        (match self.unit {
            1 => Length::new::<inch>(value).get::<millimeter>(),
            2 => Length::new::<foot>(value).get::<millimeter>(),
            _ => value,
        }) as usize
    }
    pub fn piece_width(&mut self, value: f32) {
        let last = self.last();
        self.pieces[last].set_width(value);
    }
    pub fn piece_length(&mut self, value: f32) {
        let last = self.last();
        self.pieces[last].set_length(value);
    }
    pub fn piece_amount(&mut self, value: usize) {
        let last = self.last();
        self.pieces[last].set_amount(value);
    }
    pub fn piece_pattern(&mut self, value: i32) {
        let last = self.last();
        self.pieces[last].set_pattern(value);
    }
    pub fn piece_kind(&mut self, value: i32) {
        let last = self.last();
        self.pieces[last].set_kind(value);
    }
    pub fn add(&mut self) {
        self.pieces.push(Piece::default());
    }
    fn unzip(&mut self) -> (Vec<StockPiece>, Vec<CutPiece>) {
        let mut seq: usize = 0;
        let mut stock_pieces = Vec::new();
        let mut cut_pieces = Vec::new();
        for piece in &self.pieces {
            let width = self.uom(piece.width);
            let length = self.uom(piece.length);
            let amount = piece.amount;
            let pattern_direction = match piece.pattern {
                1 => PatternDirection::ParallelToWidth,
                2 => PatternDirection::ParallelToLength,
                _ => PatternDirection::None,
            };
            if piece.kind == 0 {
                for _ in 0..amount {
                    seq += 1;
                    cut_pieces.push(CutPiece {
                        width,
                        length,
                        quantity: 1,
                        pattern_direction,
                        external_id: Some(seq),
                        can_rotate: true,
                    });
                }
            } else {
                stock_pieces.append(&mut vec![
                    StockPiece {
                        width,
                        length,
                        quantity: Some(1),
                        pattern_direction,
                        price: 0,
                    };
                    amount
                ]);
            }
        }
        (stock_pieces, cut_pieces)
    }
    pub fn optimize(&mut self) -> String {
        let (stock_pieces, cut_pieces) = self.unzip();
        let random_seed = rand::random::<u64>();
        let optimizer = cascade!(
            Optimizer::default();
            ..set_random_seed(random_seed);
            ..set_cut_width(self.uom(self.width));
            ..add_stock_pieces(stock_pieces);
            ..add_cut_pieces(cut_pieces);
        );
        if let Ok(solution) = match self.layout {
            0 => optimizer.optimize_guillotine(|_| ()),
            _ => optimizer.optimize_nested(|_| ()),
        } {
            create_solution_pdf(random_seed, solution, self.unit);
            return format!("Outputfile solution_{random_seed}.pdf saved to disk!");
        }
        "No solution, invalid input!\nIf a pattern is selected for a stockpiece,\nyou have to choose a possible pattern for all of the cutpieces!".to_string()
    }
}

fn file() -> String {
    env::var("HOME").unwrap() + "/.config/" + crate::NAME
}


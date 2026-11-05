//! Motor minimal de paginare PDF (A4, puncte tipografice, origine sus-stânga) peste `krilla`:
//! titluri, paragrafe cu încadrare, blocuri de calcul, tabele cu antet repetat la
//! trecerea pe pagina următoare, numerotarea paginilor și desen vectorial.
//!
//! Fonturile (DejaVu) sunt încorporate, deci diacriticele românești apar corect.

use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point};
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, Stroke};
use krilla::text::{Font, TextDirection};
use krilla::Document;
use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};

pub const PAGE_W: f32 = 595.28;
pub const PAGE_H: f32 = 841.89;
pub const MARGIN: f32 = 45.0;
pub const CONTENT_W: f32 = PAGE_W - 2.0 * MARGIN;
const BOTTOM: f32 = PAGE_H - 45.0;
/// mm → pt
pub const MM: f32 = 72.0 / 25.4;

static REGULAR: &[u8] = include_bytes!("../../../../assets/fonts/DejaVuSans.ttf");
static BOLD: &[u8] = include_bytes!("../../../../assets/fonts/DejaVuSans-Bold.ttf");
static MONO: &[u8] = include_bytes!("../../../../assets/fonts/DejaVuSansMono.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Regular,
    Bold,
    Mono,
}

impl Face {
    fn bytes(self) -> &'static [u8] {
        match self {
            Face::Regular => REGULAR,
            Face::Bold => BOLD,
            Face::Mono => MONO,
        }
    }
}

pub type Rgb = (u8, u8, u8);
pub const BLACK: Rgb = (0, 0, 0);
pub const HEAD: Rgb = (14, 116, 144);
pub const GREY: Rgb = (110, 110, 110);
pub const LIGHT: Rgb = (243, 246, 249);
pub const STRIPE: Rgb = (247, 249, 251);
pub const BORDER: Rgb = (205, 212, 220);

/// Lățimea textului [pt] la mărimea `size`.
pub fn text_width(face: Face, size: f32, text: &str) -> f32 {
    let Ok(font) = FontRef::new(face.bytes()) else { return 0.0 };
    let cmap = font.charmap();
    let m = font.glyph_metrics(Size::new(size), LocationRef::default());
    text.chars().map(|c| cmap.map(c).and_then(|g| m.advance_width(g)).unwrap_or(size * 0.5)).sum()
}

/// Împarte textul în rânduri care încap în `width`.
pub fn wrap(face: Face, size: f32, text: &str, width: f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let cand = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if text_width(face, size, &cand) <= width || line.is_empty() {
                line = cand;
            } else {
                out.push(std::mem::take(&mut line));
                line = word.to_string();
            }
        }
        out.push(line);
    }
    out
}

#[derive(Debug, Clone)]
pub enum Op {
    Text { x: f32, y: f32, size: f32, face: Face, color: Rgb, text: String },
    Rect { x: f32, y: f32, w: f32, h: f32, fill: Option<Rgb>, stroke: Option<(Rgb, f32)> },
    Line { x1: f32, y1: f32, x2: f32, y2: f32, color: Rgb, width: f32 },
    Polygon { pts: Vec<(f32, f32)>, fill: Rgb },
    Circle { cx: f32, cy: f32, r: f32, fill: Option<Rgb>, stroke: Option<(Rgb, f32)> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

pub struct Writer {
    pages: Vec<Vec<Op>>,
    pub y: f32,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl Writer {
    pub fn new() -> Self {
        Self { pages: vec![Vec::new()], y: MARGIN }
    }

    pub fn push(&mut self, op: Op) {
        self.pages.last_mut().expect("cel puțin o pagină").push(op);
    }

    pub fn new_page(&mut self) {
        self.pages.push(Vec::new());
        self.y = MARGIN;
    }

    /// Pagină nouă dacă nu mai încap `space` puncte.
    pub fn ensure(&mut self, space: f32) {
        if self.y + space > BOTTOM {
            self.new_page();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn text_at(&mut self, x: f32, y: f32, size: f32, face: Face, color: Rgb, text: &str, align: Align) {
        let w = text_width(face, size, text);
        let x = match align {
            Align::Left => x,
            Align::Center => x - w / 2.0,
            Align::Right => x - w,
        };
        self.push(Op::Text { x, y, size, face, color, text: text.into() });
    }

    pub fn title(&mut self, text: &str) {
        self.ensure(40.0);
        self.y += 8.0;
        self.text_at(MARGIN, self.y + 12.0, 13.0, Face::Bold, HEAD, text, Align::Left);
        self.y += 16.0;
        self.push(Op::Line { x1: MARGIN, y1: self.y, x2: PAGE_W - MARGIN, y2: self.y, color: HEAD, width: 0.8 });
        self.y += 10.0;
    }

    pub fn subtitle(&mut self, text: &str) {
        self.ensure(30.0);
        self.y += 4.0;
        self.text_at(MARGIN, self.y + 10.0, 11.0, Face::Bold, BLACK, text, Align::Left);
        self.y += 16.0;
    }

    pub fn para(&mut self, text: &str, size: f32, face: Face) {
        let lh = size * 1.4;
        for l in wrap(face, size, text, CONTENT_W) {
            self.ensure(lh);
            self.text_at(MARGIN, self.y + size, size, face, BLACK, &l, Align::Left);
            self.y += lh;
        }
        self.y += 2.0;
    }

    pub fn gap(&mut self, h: f32) {
        self.y += h;
    }

    /// Bloc de calcul: denumire (bold), apoi formula, înlocuirea și rezultatul (monospațiat, pe fundal).
    pub fn calc(&mut self, label: &str, formula: &str, substitution: Option<&str>, result: &str) {
        let size = 8.5;
        let lh = size * 1.35;
        let mut lines: Vec<String> = Vec::new();
        let mut add = |s: &str| lines.extend(wrap(Face::Mono, size, s, CONTENT_W - 16.0));
        add(formula);
        if let Some(s) = substitution {
            add(&format!("  = {s}"));
        }
        add(&format!("  = {result}"));
        let block = lines.len() as f32 * lh + 8.0;
        self.ensure(block + 18.0);
        self.text_at(MARGIN, self.y + 9.0, 9.0, Face::Bold, BLACK, label, Align::Left);
        self.y += 14.0;
        self.push(Op::Rect { x: MARGIN, y: self.y, w: CONTENT_W, h: block, fill: Some(LIGHT), stroke: None });
        let mut yy = self.y + 4.0;
        for l in &lines {
            self.text_at(MARGIN + 8.0, yy + size, size, Face::Mono, BLACK, l, Align::Left);
            yy += lh;
        }
        self.y += block + 8.0;
    }

    /// Tabel cu antet colorat; coloanele se dimensionează după conținut și se scalează la lățimea paginii.
    pub fn table(&mut self, head: &[String], rows: &[Vec<String>], size: f32) {
        let pad = 3.5;
        let n = head.len();
        let mut widths: Vec<f32> = (0..n)
            .map(|c| {
                let hw = text_width(Face::Bold, size, &head[c]);
                rows.iter().map(|r| text_width(Face::Regular, size, r.get(c).map_or("", String::as_str))).fold(hw, f32::max) + 2.0 * pad
            })
            .collect();
        let total: f32 = widths.iter().sum();
        let scale = CONTENT_W / total;
        if scale < 1.0 || total > CONTENT_W * 0.6 {
            widths.iter_mut().for_each(|w| *w *= scale);
        }
        let row_h = |cells: &[String], face: Face, widths: &[f32]| {
            cells.iter().zip(widths).map(|(c, w)| wrap(face, size, c, w - 2.0 * pad).len()).max().unwrap_or(1) as f32 * size * 1.3 + 2.0 * pad
        };
        let draw_row = |wr: &mut Writer, cells: &[String], face: Face, fill: Option<Rgb>, color: Rgb, widths: &[f32]| {
            let h = row_h(cells, face, widths);
            let mut x = MARGIN;
            let tw: f32 = widths.iter().sum();
            if let Some(f) = fill {
                wr.push(Op::Rect { x, y: wr.y, w: tw, h, fill: Some(f), stroke: None });
            }
            for (c, w) in cells.iter().zip(widths) {
                let mut yy = wr.y + pad;
                for l in wrap(face, size, c, w - 2.0 * pad) {
                    wr.text_at(x + pad, yy + size, size, face, color, &l, Align::Left);
                    yy += size * 1.3;
                }
                x += w;
            }
            wr.push(Op::Line { x1: MARGIN, y1: wr.y + h, x2: MARGIN + tw, y2: wr.y + h, color: BORDER, width: 0.4 });
            wr.y += h;
        };
        let head_h = row_h(head, Face::Bold, &widths);
        self.ensure(head_h * 2.0);
        draw_row(self, head, Face::Bold, Some(HEAD), (255, 255, 255), &widths);
        for (i, r) in rows.iter().enumerate() {
            let h = row_h(r, Face::Regular, &widths);
            if self.y + h > BOTTOM {
                self.new_page();
                draw_row(self, head, Face::Bold, Some(HEAD), (255, 255, 255), &widths);
            }
            draw_row(self, r, Face::Regular, (i % 2 == 1).then_some(STRIPE), BLACK, &widths);
        }
        self.y += 10.0;
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Subsol pe fiecare pagină începând cu `from` (0 = prima): text stânga + „i / n”.
    pub fn footer(&mut self, from: usize, left: &str) {
        let n = self.pages.len().saturating_sub(from);
        for (i, page) in self.pages.iter_mut().enumerate().skip(from) {
            let y = PAGE_H - 22.0;
            page.push(Op::Text { x: MARGIN, y, size: 7.5, face: Face::Regular, color: (120, 120, 120), text: left.into() });
            let num = format!("{} / {}", i - from + 1, n);
            let w = text_width(Face::Regular, 7.5, &num);
            page.push(Op::Text { x: PAGE_W - MARGIN - w, y, size: 7.5, face: Face::Regular, color: (120, 120, 120), text: num });
        }
    }

    /// Documentul PDF final.
    pub fn finish(self) -> Result<Vec<u8>, String> {
        let fonts = [Face::Regular, Face::Bold, Face::Mono].map(|f| Font::new(f.bytes().to_vec().into(), 0).expect("font valid"));
        let font = |f: Face| fonts[f as usize].clone();
        let color = |c: Rgb| rgb::Color::new(c.0, c.1, c.2);
        let mut doc = Document::new();
        for ops in self.pages {
            let mut page = doc.start_page_with(PageSettings::from_wh(PAGE_W, PAGE_H).ok_or("dimensiune pagină")?);
            let mut s = page.surface();
            for op in ops {
                match op {
                    Op::Text { x, y, size, face, color: c, text } => {
                        s.set_stroke(None);
                        s.set_fill(Some(Fill { paint: color(c).into(), ..Default::default() }));
                        s.draw_text(Point::from_xy(x, y), font(face), size, &text, false, TextDirection::Auto);
                    }
                    Op::Rect { x, y, w, h, fill, stroke } => {
                        let mut pb = PathBuilder::new();
                        if let Some(r) = krilla::geom::Rect::from_xywh(x, y, w, h) {
                            pb.push_rect(r);
                        }
                        if let Some(path) = pb.finish() {
                            s.set_fill(fill.map(|f| Fill { paint: color(f).into(), ..Default::default() }));
                            s.set_stroke(stroke.map(|(c, w)| Stroke { paint: color(c).into(), width: w, ..Default::default() }));
                            s.draw_path(&path);
                        }
                    }
                    Op::Line { x1, y1, x2, y2, color: c, width } => {
                        let mut pb = PathBuilder::new();
                        pb.move_to(x1, y1);
                        pb.line_to(x2, y2);
                        if let Some(path) = pb.finish() {
                            s.set_fill(None);
                            s.set_stroke(Some(Stroke { paint: color(c).into(), width, line_cap: krilla::paint::LineCap::Round, ..Default::default() }));
                            s.draw_path(&path);
                        }
                    }
                    Op::Polygon { pts, fill } => {
                        let mut pb = PathBuilder::new();
                        for (i, (x, y)) in pts.iter().enumerate() {
                            if i == 0 { pb.move_to(*x, *y) } else { pb.line_to(*x, *y) }
                        }
                        pb.close();
                        if let Some(path) = pb.finish() {
                            s.set_stroke(None);
                            s.set_fill(Some(Fill { paint: color(fill).into(), ..Default::default() }));
                            s.draw_path(&path);
                        }
                    }
                    Op::Circle { cx, cy, r, fill, stroke } => {
                        // Cerc din 4 arce Bézier.
                        let k = 0.552_284_8 * r;
                        let mut pb = PathBuilder::new();
                        pb.move_to(cx + r, cy);
                        pb.cubic_to(cx + r, cy + k, cx + k, cy + r, cx, cy + r);
                        pb.cubic_to(cx - k, cy + r, cx - r, cy + k, cx - r, cy);
                        pb.cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r);
                        pb.cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy);
                        pb.close();
                        if let Some(path) = pb.finish() {
                            s.set_fill(fill.map(|f| Fill { paint: color(f).into(), ..Default::default() }));
                            s.set_stroke(stroke.map(|(c, w)| Stroke { paint: color(c).into(), width: w, ..Default::default() }));
                            s.draw_path(&path);
                        }
                    }
                }
            }
            s.finish();
            page.finish();
        }
        let _ = NormalizedF32::ONE;
        doc.finish().map_err(|e| format!("{e:?}"))
    }
}

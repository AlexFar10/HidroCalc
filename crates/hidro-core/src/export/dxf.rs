//! Export DXF R12 (AC1009) al schemei, portat din `exportDXF.ts`.
//! X_dxf = x_px × mpp, Y_dxf = −y_px × mpp (axa Y inversată), unități: metri.

use crate::model::{SchemaElement, SchemaKind};
use crate::schema::meters_per_px;
use std::fmt::Write;

const BOX_W: f64 = 52.0;
const BOX_H: f64 = 52.0;
const CIRCLE_R: f64 = 10.0;
const SUB_W: f64 = 180.0;
const SUB_H: f64 = 100.0;

const LAYERS: &[(&str, i32)] = &[
    ("TRONSOANE", 5),
    ("REZERVOARE", 3),
    ("SURSE", 3),
    ("HIDRANTI", 1),
    ("CAMINE", 4),
    ("TEXTE", 7),
    ("SUBSISTEME", 2),
];

struct Dxf(String);

impl Dxf {
    fn g(&mut self, code: i32, v: impl std::fmt::Display) {
        let _ = write!(self.0, "{code:>3}\n{v}\n");
    }
    fn f(&mut self, code: i32, v: f64) {
        self.g(code, format!("{v:.6}"));
    }
    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, layer: &str) {
        self.g(0, "LINE");
        self.g(8, layer);
        self.f(10, x1);
        self.f(20, y1);
        self.g(30, "0.0");
        self.f(11, x2);
        self.f(21, y2);
        self.g(31, "0.0");
    }
    fn circle(&mut self, cx: f64, cy: f64, r: f64, layer: &str) {
        self.g(0, "CIRCLE");
        self.g(8, layer);
        self.f(10, cx);
        self.f(20, cy);
        self.g(30, "0.0");
        self.f(40, r);
    }
    fn text(&mut self, x: f64, y: f64, text: &str, h: f64, layer: &str) {
        self.g(0, "TEXT");
        self.g(8, layer);
        self.f(10, x);
        self.f(20, y);
        self.g(30, "0.0");
        self.f(40, h);
        self.g(1, text);
        self.g(72, 1);
        self.f(11, x);
        self.f(21, y);
        self.g(31, "0.0");
    }
    fn rect(&mut self, cx: f64, cy: f64, w: f64, h: f64, layer: &str) {
        let (x1, x2, y1, y2) = (cx - w / 2.0, cx + w / 2.0, cy - h / 2.0, cy + h / 2.0);
        self.line(x1, y1, x2, y1, layer);
        self.line(x2, y1, x2, y2, layer);
        self.line(x2, y2, x1, y2, layer);
        self.line(x1, y2, x1, y1, layer);
    }
}

fn entities(out: &mut Dxf, els: &[SchemaElement], m: f64) {
    let text_h = 12.0 * m;
    let (tx, ty) = (|x: f64| x * m, |y: f64| -y * m);
    for el in els {
        match el.kind {
            SchemaKind::Tronson => {
                let (x1, y1) = (tx(el.x), ty(el.y));
                let (x2, y2) = (tx(el.x2.unwrap_or(el.x)), ty(el.y2.unwrap_or(el.y)));
                out.line(x1, y1, x2, y2, "TRONSOANE");
                if let Some(l) = &el.label {
                    out.text((x1 + x2) / 2.0, (y1 + y2) / 2.0 + text_h, l, text_h, "TEXTE");
                }
            }
            SchemaKind::Rezervor | SchemaKind::Sursa => {
                let (w_px, h_px) = (el.w.unwrap_or(BOX_W), el.h.unwrap_or(BOX_H));
                let (w, h) = (w_px * m, h_px * m);
                let (cx, cy) = (tx(el.x), ty(el.y - h_px / 2.0));
                out.rect(cx, cy, w, h, if el.kind == SchemaKind::Rezervor { "REZERVOARE" } else { "SURSE" });
                if let Some(l) = &el.label {
                    out.text(cx, cy - h / 2.0 - text_h * 1.2, l, text_h, "TEXTE");
                }
            }
            SchemaKind::Hidrant | SchemaKind::CaminGrav | SchemaKind::CaminAer => {
                let r = el.r.unwrap_or(CIRCLE_R) * m;
                let (cx, cy) = (tx(el.x), ty(el.y));
                out.circle(cx, cy, r, if el.kind == SchemaKind::Hidrant { "HIDRANTI" } else { "CAMINE" });
                if let Some(l) = &el.label {
                    out.text(cx, cy - r - text_h * 1.2, l, text_h, "TEXTE");
                }
            }
            SchemaKind::Text => {
                let fs = el.font_size.unwrap_or(12.0) * m;
                out.text(tx(el.x), ty(el.y), el.text.as_deref().unwrap_or(""), fs, "TEXTE");
            }
            SchemaKind::Subsistem => {
                let (w, h) = (el.w.unwrap_or(SUB_W) * m, el.h.unwrap_or(SUB_H) * m);
                let (cx, cy) = (tx(el.x), ty(el.y));
                out.rect(cx, cy, w, h, "SUBSISTEME");
                if let Some(l) = &el.label {
                    out.text(cx, cy + h / 2.0 + text_h * 1.2, l, text_h, "TEXTE");
                }
                if let Some(ch) = &el.children {
                    entities(out, ch, m);
                }
            }
        }
    }
}

/// Conținutul fișierului DXF. `None` dacă schema e goală.
pub fn export(schema: &[SchemaElement], scale_ratio: f64) -> Option<String> {
    if schema.is_empty() {
        return None;
    }
    let mut d = Dxf(String::new());
    d.g(0, "SECTION");
    d.g(2, "HEADER");
    d.g(9, "$ACADVER");
    d.g(1, "AC1009");
    d.g(9, "$INSUNITS");
    d.g(70, 6);
    d.g(9, "$MEASUREMENT");
    d.g(70, 1);
    d.g(0, "ENDSEC");
    d.g(0, "SECTION");
    d.g(2, "TABLES");
    d.g(0, "TABLE");
    d.g(2, "LTYPE");
    d.g(70, 1);
    d.g(0, "LTYPE");
    d.g(2, "CONTINUOUS");
    d.g(70, 0);
    d.g(3, "Solid line");
    d.g(72, 65);
    d.g(73, 0);
    d.g(40, "0.0");
    d.g(0, "ENDTAB");
    d.g(0, "TABLE");
    d.g(2, "LAYER");
    d.g(70, LAYERS.len());
    for (name, color) in LAYERS {
        d.g(0, "LAYER");
        d.g(2, name);
        d.g(70, 0);
        d.g(62, color);
        d.g(6, "CONTINUOUS");
    }
    d.g(0, "ENDTAB");
    d.g(0, "ENDSEC");
    d.g(0, "SECTION");
    d.g(2, "ENTITIES");
    entities(&mut d, schema, meters_per_px(scale_ratio));
    d.g(0, "ENDSEC");
    d.g(0, "EOF");
    Some(d.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_in_metres_with_flipped_y() {
        let s = vec![SchemaElement {
            kind: SchemaKind::Tronson,
            x: 0.0,
            y: 100.0,
            x2: Some(1000.0),
            y2: Some(100.0),
            label: Some("Tr 1".into()),
            ..Default::default()
        }];
        let d = export(&s, 1000.0).unwrap();
        assert!(d.starts_with("  0\nSECTION\n"));
        assert!(d.contains(" 11\n264.580000\n")); // 1000 px × 0.26458 m
        assert!(d.contains(" 20\n-26.458000\n")); // y inversat
        assert!(d.contains("  1\nTr 1\n"));
        assert!(d.trim_end().ends_with("EOF"));
        assert!(export(&[], 1000.0).is_none());
    }
}

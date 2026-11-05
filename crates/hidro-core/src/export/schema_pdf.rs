//! Schema rețelei desenată vectorial în PDF (încadrată într-un dreptunghi dat).

use super::pdf::{Align, Face, Op, Rgb, Writer};
use crate::calculations::network::NetworkSection;
use crate::model::{SchemaElement, SchemaKind};
use crate::schema::meters_per_px;

const RED: Rgb = (220, 38, 38);
const DARK: Rgb = (26, 26, 26);
const BLUE: Rgb = (29, 78, 216);
const BLUE_L: Rgb = (59, 130, 246);
const TEAL: Rgb = (13, 148, 136);
const Q: Rgb = (37, 99, 235);

fn parse_color(s: Option<&str>) -> Option<Rgb> {
    let h = s?.strip_prefix('#')?;
    let v = u32::from_str_radix(h, 16).ok().filter(|_| h.len() == 6)?;
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

fn bounds(els: &[SchemaElement]) -> Option<(f64, f64, f64, f64)> {
    let mut pts = Vec::new();
    for e in els {
        pts.push((e.x, e.y));
        match e.kind {
            SchemaKind::Tronson => pts.push((e.x2.unwrap_or(e.x), e.y2.unwrap_or(e.y))),
            SchemaKind::Rezervor | SchemaKind::Sursa => {
                let (w, h) = (e.w.unwrap_or(52.0), e.h.unwrap_or(52.0));
                pts.push((e.x - w / 2.0, e.y - h));
                pts.push((e.x + w / 2.0, e.y));
            }
            SchemaKind::Subsistem => {
                let (w, h) = (e.w.unwrap_or(180.0), e.h.unwrap_or(100.0));
                pts.push((e.x - w / 2.0, e.y - h / 2.0));
                pts.push((e.x + w / 2.0, e.y + h / 2.0));
            }
            _ => {}
        }
    }
    let first = *pts.first()?;
    Some(pts.iter().fold((first.0, first.1, first.0, first.1), |(a, b, c, d), p| (a.min(p.0), b.min(p.1), c.max(p.0), d.max(p.1))))
}

/// Desenează schema principală în dreptunghiul (x, y, w, h) [pt].
pub fn draw(wr: &mut Writer, schema: &[SchemaElement], sections: &[NetworkSection], scale_ratio: f64, area: (f32, f32, f32, f32)) {
    let (ax, ay, aw, ah) = area;
    wr.push(Op::Rect { x: ax, y: ay, w: aw, h: ah, fill: None, stroke: Some(((205, 212, 220), 0.6)) });
    let Some((x0, y0, x1, y1)) = bounds(schema) else { return };
    let pad = 30.0;
    let (bw, bh) = ((x1 - x0 + 2.0 * pad).max(1.0), (y1 - y0 + 2.0 * pad).max(1.0));
    let k = (aw as f64 / bw).min(ah as f64 / bh);
    let ox = ax as f64 + (aw as f64 - bw * k) / 2.0 - (x0 - pad) * k;
    let oy = ay as f64 + (ah as f64 - bh * k) / 2.0 - (y0 - pad) * k;
    let p = |x: f64, y: f64| ((ox + x * k) as f32, (oy + y * k) as f32);
    let mpp = meters_per_px(scale_ratio);
    let q: std::collections::HashMap<&str, f64> = sections.iter().filter(|s| s.group.is_none()).map(|s| (s.name.as_str(), s.q_c)).collect();
    let fs = 7.0;
    for e in schema {
        match e.kind {
            SchemaKind::Tronson => {
                let c = parse_color(e.color.as_deref()).unwrap_or(DARK);
                let (a, b) = (p(e.x, e.y), p(e.x2.unwrap_or(e.x), e.y2.unwrap_or(e.y)));
                wr.push(Op::Line { x1: a.0, y1: a.1, x2: b.0, y2: b.1, color: c, width: 1.2 });
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let len = dx.hypot(dy);
                if len < 2.0 {
                    continue;
                }
                let (ux, uy) = (dx / len, dy / len);
                let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                let s = 6.0;
                wr.push(Op::Polygon {
                    pts: vec![(m.0 + ux * s / 2.0, m.1 + uy * s / 2.0), (m.0 - ux * s / 2.0 - uy * s / 2.5, m.1 - uy * s / 2.0 + ux * s / 2.5), (m.0 - ux * s / 2.0 + uy * s / 2.5, m.1 - uy * s / 2.0 - ux * s / 2.5)],
                    fill: c,
                });
                for pt in [a, b] {
                    wr.push(Op::Circle { cx: pt.0, cy: pt.1, r: 2.0, fill: Some(c), stroke: None });
                }
                if let Some(l) = &e.label {
                    let (px, py) = (-uy, ux);
                    let vertical = px.abs() > 0.7;
                    let al = |v: f32| if vertical { if v < 0.0 { Align::Right } else { Align::Left } } else { Align::Center };
                    let off = 9.0;
                    wr.text_at(m.0 + px * off, m.1 + py * off + fs / 3.0, fs, Face::Bold, parse_color(e.color.as_deref()).unwrap_or(RED), l, al(px));
                    let shown = e.override_length.unwrap_or(((e.x2.unwrap_or(e.x) - e.x).hypot(e.y2.unwrap_or(e.y) - e.y)) * mpp);
                    wr.text_at(m.0 - px * off, m.1 - py * off + fs / 3.0, fs, Face::Regular, DARK, &format!("{shown:.1} m"), al(-px));
                    if let Some(qv) = q.get(l.as_str()).filter(|v| **v > 0.0) {
                        let (qx, qy) = if vertical { (m.0 + px * off, m.1 + py * off + fs / 3.0 + 8.0) } else { (m.0 + px * off * 2.0, m.1 + py * off * 2.0 + fs / 3.0) };
                        wr.text_at(qx, qy, 6.0, Face::Regular, Q, &format!("Q={qv:.2} l/s"), al(px));
                    }
                }
            }
            SchemaKind::Rezervor | SchemaKind::Sursa => {
                let (w, h) = (e.w.unwrap_or(52.0), e.h.unwrap_or(52.0));
                let (tl, br) = (p(e.x - w / 2.0, e.y - h), p(e.x + w / 2.0, e.y));
                let c = if e.kind == SchemaKind::Rezervor { RED } else { DARK };
                wr.push(Op::Rect { x: tl.0, y: tl.1, w: br.0 - tl.0, h: br.1 - tl.1, fill: Some((255, 255, 255)), stroke: Some((c, 1.2)) });
                let hh = br.1 - tl.1;
                let letter = if e.kind == SchemaKind::Rezervor { "R" } else { "S" };
                wr.text_at((tl.0 + br.0) / 2.0, tl.1 + hh * 0.7, (hh * 0.55).max(5.0), Face::Bold, DARK, letter, Align::Center);
                if let Some(l) = &e.label {
                    wr.text_at((tl.0 + br.0) / 2.0, br.1 + fs + 2.0, fs, Face::Regular, c, l, Align::Center);
                }
            }
            SchemaKind::Hidrant | SchemaKind::CaminGrav | SchemaKind::CaminAer => {
                let c = match e.kind {
                    SchemaKind::Hidrant => RED,
                    SchemaKind::CaminGrav => BLUE,
                    _ => BLUE_L,
                };
                let ctr = p(e.x, e.y);
                let r = (e.r.unwrap_or(10.0) * k) as f32;
                wr.push(Op::Circle { cx: ctr.0, cy: ctr.1, r: r.max(2.5), fill: Some((255, 255, 255)), stroke: Some((c, 1.0)) });
                if let Some(l) = &e.label {
                    wr.text_at(ctr.0, ctr.1 + r.max(2.5) + fs + 1.0, fs, Face::Regular, c, l, Align::Center);
                }
            }
            SchemaKind::Subsistem => {
                let (w, h) = (e.w.unwrap_or(180.0), e.h.unwrap_or(100.0));
                let (tl, br) = (p(e.x - w / 2.0, e.y - h / 2.0), p(e.x + w / 2.0, e.y + h / 2.0));
                wr.push(Op::Rect { x: tl.0, y: tl.1, w: br.0 - tl.0, h: br.1 - tl.1, fill: Some((240, 253, 250)), stroke: Some((TEAL, 1.0)) });
                if let Some(l) = &e.label {
                    wr.text_at((tl.0 + br.0) / 2.0, tl.1 + fs + 3.0, fs, Face::Bold, TEAL, l, Align::Center);
                }
            }
            SchemaKind::Text => {
                if let Some(t) = &e.text {
                    let c = p(e.x, e.y);
                    let size = ((e.font_size.unwrap_or(12.0) * k) as f32).clamp(5.0, 14.0);
                    wr.text_at(c.0, c.1 + size, size, Face::Regular, parse_color(e.color.as_deref()).unwrap_or(DARK), t, Align::Left);
                }
            }
        }
    }
}

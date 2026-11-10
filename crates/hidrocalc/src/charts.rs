//! Grafice simple desenate direct cu egui (bare, bare stivuite, puncte),
//! echivalentele graficelor Recharts din pasul 9.

use crate::widgets::{fmt, pal};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

pub const CYAN: Color32 = Color32::from_rgb(0x5B, 0x8D, 0xB8);
pub const AMBER: Color32 = Color32::from_rgb(0xC9, 0xA4, 0x67);
pub const GREEN: Color32 = Color32::from_rgb(0x6F, 0xA5, 0x8A);
pub const RED: Color32 = Color32::from_rgb(0xC7, 0x7A, 0x76);

/// Valoare „rotundă” ≥ v pentru capătul axei Y.
fn nice_max(v: f64) -> f64 {
    if v <= 0.0 || !v.is_finite() {
        return 1.0;
    }
    let p = 10f64.powf(v.log10().floor());
    for m in [1.0, 2.0, 2.5, 5.0, 10.0] {
        if v <= m * p {
            return m * p;
        }
    }
    10.0 * p
}

struct Frame {
    plot: Rect,
    y_max: f64,
}

fn axes(ui: &mut Ui, height: f32, y_max: f64, decimals: usize) -> (egui::Response, egui::Painter, Frame) {
    let p = pal(ui);
    let (resp, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), height), Sense::hover());
    let r = resp.rect;
    let plot = Rect::from_min_max(Pos2::new(r.left() + 48.0, r.top() + 6.0), Pos2::new(r.right() - 8.0, r.bottom() - 22.0));
    let y_max = nice_max(y_max);
    let grid = Stroke::new(1.0, p.border);
    for i in 0..=4 {
        let f = i as f32 / 4.0;
        let y = plot.bottom() - f * plot.height();
        painter.line_segment([Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)], grid);
        painter.text(
            Pos2::new(plot.left() - 6.0, y),
            Align2::RIGHT_CENTER,
            fmt(y_max * f as f64, decimals),
            FontId::proportional(10.5),
            p.muted,
        );
    }
    (resp, painter, Frame { plot, y_max })
}

/// Bare (eventual stivuite): `series` = (valori, culoare, nume), câte o valoare pe etichetă.
pub fn bars(ui: &mut Ui, labels: &[String], series: &[(Vec<f64>, Color32, String)], unit: &str, height: f32) {
    let p = pal(ui);
    let totals: Vec<f64> = (0..labels.len()).map(|i| series.iter().map(|s| s.0[i].max(0.0)).sum()).collect();
    let max = totals.iter().cloned().fold(0.0, f64::max);
    let (resp, painter, f) = axes(ui, height, max, if max < 1.0 { 3 } else { 1 });
    let n = labels.len().max(1);
    let slot = f.plot.width() / n as f32;
    let bar_w = (slot * 0.6).min(42.0);
    let mut hovered = None;
    for (i, label) in labels.iter().enumerate() {
        let cx = f.plot.left() + slot * (i as f32 + 0.5);
        let mut base = 0.0;
        for (vals, color, _) in series {
            let v = vals[i].max(0.0);
            let y0 = f.plot.bottom() - (base / f.y_max) as f32 * f.plot.height();
            let y1 = f.plot.bottom() - ((base + v) / f.y_max) as f32 * f.plot.height();
            let rect = Rect::from_min_max(Pos2::new(cx - bar_w / 2.0, y1), Pos2::new(cx + bar_w / 2.0, y0));
            painter.rect_filled(rect, 2.0, *color);
            base += v;
        }
        if n <= 30 || i % (n / 30 + 1) == 0 {
            painter.text(Pos2::new(cx, f.plot.bottom() + 4.0), Align2::CENTER_TOP, label, FontId::proportional(10.5), p.muted);
        }
        let col = Rect::from_min_max(Pos2::new(cx - slot / 2.0, f.plot.top()), Pos2::new(cx + slot / 2.0, f.plot.bottom()));
        if resp.hover_pos().is_some_and(|h| col.contains(h)) {
            hovered = Some(i);
        }
    }
    if let Some(i) = hovered {
        resp.on_hover_ui_at_pointer(|ui| {
            ui.label(egui::RichText::new(&labels[i]).strong());
            for (vals, color, name) in series {
                ui.label(egui::RichText::new(format!("{name}: {} {unit}", fmt(vals[i], 4))).color(*color));
            }
        });
    }
}

/// Viteza reală vs. diametrul interior, cu benzile 0–0.5 / 0.5–2.5 / > 2.5 m/s.
pub fn velocity_scatter(ui: &mut Ui, points: &[(String, f64, f64)], height: f32) {
    let p = pal(ui);
    let v_max = points.iter().map(|x| x.2).fold(3.0, f64::max);
    let d_max = nice_max(points.iter().map(|x| x.1).fold(0.0, f64::max) * 1.05);
    let (resp, painter, f) = axes(ui, height, v_max, 2);
    let y_of = |v: f64| f.plot.bottom() - (v / f.y_max) as f32 * f.plot.height();
    for (lo, hi, c) in [(0.0, 0.5, AMBER), (0.5, 2.5, GREEN), (2.5, f.y_max, RED)] {
        let rect = Rect::from_min_max(Pos2::new(f.plot.left(), y_of(hi.min(f.y_max))), Pos2::new(f.plot.right(), y_of(lo)));
        painter.rect_filled(rect, 0.0, c.gamma_multiply(0.10));
    }
    for i in 0..=4 {
        let d = d_max * i as f64 / 4.0;
        let x = f.plot.left() + (d / d_max) as f32 * f.plot.width();
        painter.text(Pos2::new(x, f.plot.bottom() + 4.0), Align2::CENTER_TOP, format!("{d:.0} mm"), FontId::proportional(10.5), p.muted);
    }
    let mut hovered = None;
    for (i, (_, d, v)) in points.iter().enumerate() {
        let pos = Pos2::new(f.plot.left() + (*d / d_max) as f32 * f.plot.width(), y_of(*v));
        painter.circle_filled(pos, 4.5, CYAN);
        if resp.hover_pos().is_some_and(|h| h.distance(pos) < 7.0) {
            hovered = Some(i);
        }
    }
    if let Some(i) = hovered {
        let (name, d, v) = &points[i];
        resp.on_hover_ui_at_pointer(|ui| {
            ui.label(egui::RichText::new(name).strong());
            ui.label(format!("Di = {d:.1} mm"));
            ui.label(format!("v = {} m/s", fmt(*v, 3)));
        });
    }
}

//! Componente de interfață reutilizabile (echivalentele FormField, ResultCard,
//! StepNavigation etc. din aplicația React).

use crate::theme::{palette, Palette};
use eframe::egui::{self, Align, Color32, CornerRadius, Layout, Margin, RichText, Stroke, TextEdit, Ui};

pub fn pal(ui: &Ui) -> &'static Palette {
    palette(ui.ctx())
}

/// Formatare numerică cu `d` zecimale; `-` pentru NaN/infinit.
pub fn fmt(v: f64, d: usize) -> String {
    if v.is_finite() {
        format!("{v:.d$}")
    } else {
        "-".into()
    }
}

/// Afișarea „naturală” a unei valori introduse (fără zerouri inutile).
fn display_num(v: f64) -> String {
    if v == 0.0 {
        "0".into()
    } else {
        let s = format!("{v}");
        if s.len() > 14 { format!("{v:.6}").trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
    }
}

/// Acceptă atât `,` cât și `.` ca separator zecimal (uzanță românească).
pub fn parse_decimal(s: &str) -> Option<f64> {
    let t = s.trim().replace(',', ".");
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = pal(ui);
    egui::Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn soft_box<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = pal(ui);
    egui::Frame::new()
        .fill(p.panel2)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Rânduri de formule, monospațiate.
pub fn formula(ui: &mut Ui, lines: &[String]) {
    let p = pal(ui);
    soft_box(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        for l in lines {
            ui.add(egui::Label::new(RichText::new(l).monospace().size(12.0).color(p.ink2)).wrap());
        }
    });
}

pub fn alert(ui: &mut Ui, text: &str, danger: bool) {
    let p = pal(ui);
    let (fg, bg) = if danger { (p.danger, p.danger_soft) } else { (p.warning, p.warning_soft) };
    egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, fg.gamma_multiply(0.6)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(RichText::new(text).color(fg).size(12.5)).wrap());
        });
}

pub fn step_header(ui: &mut Ui, title: &str, subtitle: &str) {
    let p = pal(ui);
    ui.label(RichText::new(title).size(21.0).strong().color(p.ink));
    ui.label(RichText::new(subtitle).size(13.0).color(p.muted));
    ui.add_space(10.0);
}

pub fn section_title(ui: &mut Ui, text: &str) {
    let p = pal(ui);
    ui.label(RichText::new(text.to_uppercase()).size(12.0).strong().color(p.accent));
    ui.add_space(4.0);
}

/// Eticheta unui câmp; întoarce id-ul ei, pentru a lega câmpul de etichetă (accesibilitate).
fn field_label(ui: &mut Ui, label: &str, tooltip: Option<&str>, computed: bool) -> egui::Id {
    let p = pal(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let id = ui.label(RichText::new(label).size(12.5).color(p.ink2).strong()).id;
        if computed {
            ui.label(RichText::new("calculat").size(10.0).color(p.accent).italics());
        }
        if let Some(tip) = tooltip {
            ui.label(RichText::new("ℹ").size(12.0).color(p.muted)).on_hover_text(tip);
        }
        id
    })
    .inner
}

fn unit_suffix(ui: &mut Ui, unit: Option<&str>) {
    if let Some(u) = unit {
        ui.label(RichText::new(u).monospace().size(11.5).color(pal(ui).muted));
    }
}

/// Câmp numeric etichetat. Textul tastat e păstrat cât timp câmpul are focus,
/// deci se pot introduce valori intermediare ca „0,” sau „-”. Întoarce `true`
/// când valoarea s-a schimbat.
pub fn num_field(ui: &mut Ui, label: &str, value: &mut f64, unit: Option<&str>, tooltip: Option<&str>) -> bool {
    let mut changed = false;
    ui.vertical(|ui| {
        let label_id = field_label(ui, label, tooltip, false);
        ui.horizontal(|ui| {
            changed = num_input_labelled(ui, egui::Id::unique(("num", label)).with(ui.scope_id()), value, unit.map_or(0.0, |_| 52.0), Some(label_id));
            unit_suffix(ui, unit);
        });
    });
    changed
}

/// Doar căsuța numerică (fără etichetă), pentru tabele.
pub fn num_input(ui: &mut Ui, id: egui::Id, value: &mut f64, reserve_right: f32) -> bool {
    num_input_labelled(ui, id, value, reserve_right, None)
}

fn num_input_labelled(ui: &mut Ui, id: egui::Id, value: &mut f64, reserve_right: f32, label: Option<egui::Id>) -> bool {
    let buf_id = id.with("buf");
    let edit_id = id.with("edit");
    let focused = ui.memory(|m| m.has_focus(edit_id));
    let mut buf = if focused {
        ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or_else(|| display_num(*value))
    } else {
        display_num(*value)
    };
    let width = (ui.available_width() - reserve_right).max(50.0);
    let mut resp = ui.add(TextEdit::singleline(&mut buf).id(edit_id).desired_width(width).font(egui::TextStyle::Monospace));
    if let Some(l) = label {
        resp = resp.labelled_by(l);
    }
    let mut changed = false;
    if resp.changed() {
        if let Some(v) = parse_decimal(&buf) {
            if v != *value {
                *value = v;
                changed = true;
            }
        }
    }
    if resp.lost_focus() && parse_decimal(&buf).is_none() && *value != 0.0 {
        *value = 0.0;
        changed = true;
    }
    if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(buf_id, buf));
    } else {
        ui.data_mut(|d| d.remove::<String>(buf_id));
    }
    changed
}

pub fn text_field(ui: &mut Ui, label: &str, value: &mut String, hint: &str) -> bool {
    let mut changed = false;
    ui.vertical(|ui| {
        let label_id = field_label(ui, label, None, false);
        changed = ui
            .add(TextEdit::singleline(value).hint_text(hint).desired_width(ui.available_width()))
            .labelled_by(label_id)
            .changed();
    });
    changed
}

/// Listă derulantă: `options` = (valoare stocată, text afișat).
pub fn combo(ui: &mut Ui, label: &str, value: &mut String, options: &[(String, String)]) -> bool {
    let mut changed = false;
    ui.vertical(|ui| {
        let label_id = field_label(ui, label, None, false);
        changed = combo_labelled(ui, egui::Id::unique(("combo", label)).with(ui.scope_id()), value, options, ui.available_width(), Some(label_id));
    });
    changed
}

pub fn combo_input(ui: &mut Ui, id: egui::Id, value: &mut String, options: &[(String, String)], width: f32) -> bool {
    combo_labelled(ui, id, value, options, width, None)
}

/// Listă derulantă legată de eticheta ei (`label`), pentru cititoarele de ecran.
pub fn combo_labelled(
    ui: &mut Ui,
    id: egui::Id,
    value: &mut String,
    options: &[(String, String)],
    width: f32,
    label: Option<egui::Id>,
) -> bool {
    let shown = options.iter().find(|(v, _)| v == value).map(|(_, d)| d.clone()).unwrap_or_else(|| value.clone());
    let mut changed = false;
    let resp = egui::ComboBox::from_id_salt(id).selected_text(shown).width(width).show_ui(ui, |ui| {
        for (v, d) in options {
            if ui.selectable_label(value == v, d).clicked() && value != v {
                *value = v.clone();
                changed = true;
            }
        }
    });
    if let Some(l) = label {
        let _ = resp.response.labelled_by(l);
    }
    changed
}

/// Câmp calculat, doar pentru citire.
pub fn computed_field(ui: &mut Ui, label: &str, text: &str, unit: Option<&str>, tooltip: Option<&str>, danger: bool) {
    let p = pal(ui);
    ui.vertical(|ui| {
        field_label(ui, label, tooltip, true);
        let (fg, bg, stroke) =
            if danger { (p.danger, p.danger_soft, p.danger) } else { (p.accent, p.accent_soft, p.accent_ring) };
        egui::Frame::new()
            .fill(bg)
            .stroke(Stroke::new(1.0, stroke))
            .corner_radius(CornerRadius::same(7))
            .inner_margin(Margin::symmetric(10, 5))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(text).monospace().color(fg));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| unit_suffix(ui, unit));
                });
            });
    });
}

#[derive(Clone, Copy, PartialEq)]
pub enum CardSize {
    Normal,
    Large,
}

pub fn result_card(ui: &mut Ui, label: &str, value: f64, unit: &str, primary: bool, size: CardSize) {
    result_card_dec(ui, label, value, unit, primary, size, 3);
}

pub fn result_card_dec(ui: &mut Ui, label: &str, value: f64, unit: &str, primary: bool, size: CardSize, decimals: usize) {
    let p = pal(ui);
    let (fill, stroke, value_color) =
        if primary { (p.accent_soft, p.accent_ring, p.accent) } else { (p.panel, p.border, p.ink) };
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(label).size(11.5).color(p.muted));
            ui.horizontal(|ui| {
                let sz = if size == CardSize::Large { 26.0 } else { 19.0 };
                ui.label(RichText::new(fmt(value, decimals)).monospace().size(sz).strong().color(value_color));
                ui.label(RichText::new(unit).size(11.5).color(p.muted));
            });
        });
}

pub fn primary_button(ui: &mut Ui, text: &str) -> egui::Response {
    let p = pal(ui);
    ui.add(
        egui::Button::new(RichText::new(text).color(p.accent_fg).strong())
            .fill(p.accent)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(8)),
    )
}

pub fn secondary_button(ui: &mut Ui, text: &str) -> egui::Response {
    let p = pal(ui);
    ui.add(
        egui::Button::new(RichText::new(text).color(p.ink2))
            .fill(p.panel)
            .stroke(Stroke::new(1.0, p.border_strong))
            .corner_radius(CornerRadius::same(8)),
    )
}

pub fn danger_button(ui: &mut Ui, text: &str, armed: bool) -> egui::Response {
    let p = pal(ui);
    let (fg, bg) = if armed { (Color32::WHITE, p.danger) } else { (p.danger, p.panel2) };
    ui.add(egui::Button::new(RichText::new(text).color(fg).size(12.5)).fill(bg).stroke(Stroke::NONE))
}

/// Butoanele Înapoi / Următorul din josul fiecărui pas. Întoarce pasul ales.
pub fn step_nav(ui: &mut Ui, t: &crate::i18n::I18n, back: Option<u32>, next: Option<u32>) -> Option<u32> {
    let mut go = None;
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if let Some(b) = back {
            if secondary_button(ui, &t.t("common.back")).clicked() {
                go = Some(b);
            }
        }
        if let Some(n) = next {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary_button(ui, &t.t("common.next")).clicked() {
                    go = Some(n);
                }
            });
        }
    });
    go
}

/// Conținut centrat cu lățime maximă (echivalentul `max-w-* mx-auto`).
pub fn centered<R>(ui: &mut Ui, max_width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let avail = ui.available_width();
    let w = avail.min(max_width);
    let pad = ((avail - w) / 2.0).max(0.0);
    ui.horizontal_top(|ui| {
        ui.add_space(pad);
        ui.vertical(|ui| {
            ui.set_width(w);
            add(ui)
        })
        .inner
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_parsing_accepts_comma() {
        assert_eq!(parse_decimal("1,5"), Some(1.5));
        assert_eq!(parse_decimal(" 0.15 "), Some(0.15));
        assert_eq!(parse_decimal("0,"), Some(0.0));
        assert_eq!(parse_decimal(""), None);
        assert_eq!(parse_decimal("abc"), None);
        assert_eq!(display_num(120.0), "120");
        assert_eq!(display_num(0.00000131), "0.00000131");
    }
}

#[cfg(test)]
mod glyph_tests {
    use eframe::egui;

    /// Toate caracterele non-ASCII din sursele UI și din traduceri trebuie să
    /// existe în fonturile încărcate (altfel apar pătrățele „tofu”).
    #[test]
    fn all_ui_characters_have_glyphs() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut out = ctx.run_ui(Default::default(), |_| {});
        out.textures_delta.clear();
        let sources = [
            include_str!("app.rs"),
            include_str!("steps.rs"),
            include_str!("canvas.rs"),
            include_str!("network_ui.rs"),
            include_str!("charts.rs"),
            include_str!("map_ui.rs"),
            include_str!("view3d.rs"),
            include_str!("widgets.rs"),
            include_str!("../../../assets/locales/ro.json"),
            include_str!("../../../assets/locales/en.json"),
        ];
        let mut chars: Vec<char> = sources.iter().flat_map(|s| s.chars()).filter(|c| !c.is_ascii()).collect();
        chars.sort();
        chars.dedup();
        let missing: String = ctx.fonts_mut(|f| {
            let prop = egui::FontId::proportional(14.0);
            let mono = egui::FontId::monospace(14.0);
            chars.iter().filter(|c| !f.has_glyph(&prop, **c) || !f.has_glyph(&mono, **c)).collect()
        });
        assert!(missing.is_empty(), "lipsesc glife pentru: {missing}");
    }
}

#[cfg(test)]
mod interaction_tests {
    use egui_kittest::{kittest::Queryable, Harness};

    #[test]
    fn typing_comma_decimal_updates_value() {
        let mut h = Harness::new_ui_state(
            |ui, v: &mut f64| {
                super::num_field(ui, "Debit", v, Some("l/s"), None);
            },
            0.0_f64,
        );
        let input = h.get_by_role(egui::accesskit::Role::TextInput);
        input.focus();
        input.type_text("1,5");
        h.run();
        assert_eq!(*h.state(), 1.5, "0 + „1,5” tastat → 01,5 = 1.5");
    }
}

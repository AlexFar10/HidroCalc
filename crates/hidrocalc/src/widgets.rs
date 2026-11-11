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
        let s = format!("{v:.d$}");
        // „-0.000” (zero negativ sau rotunjit la zero) se afișează fără minus.
        match s.strip_prefix('-') {
            Some(rest) if rest.chars().all(|c| c == '0' || c == '.') => rest.to_string(),
            _ => s,
        }
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
        if s.len() > 14 {
            format!("{v:.6}").trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
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

/// Conturul de focus se desenează global, pentru orice widget (vezi `app::paint_focus_ring`);
/// păstrat ca punct unic de apel pentru widgeturile desenate manual.
pub fn focus_ring(_ui: &Ui, _resp: &egui::Response, _radius: u8) {}

/// Etichetă pentru cititoarele de ecran la un widget desenat manual.
pub fn describe(resp: &egui::Response, kind: egui::Role, label: &str) {
    let enabled = resp.enabled();
    resp.widget_info(|| egui::WidgetInfo::labeled(kind, enabled, label));
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

/// Id stabil pentru starea „deschis/închis” a formulelor: ordinea în cadru + pasul curent,
/// deci nu se schimbă când se deschide un conținut (spre deosebire de `next_auto_id`).
fn formula_id(ui: &Ui) -> egui::Id {
    let base = egui::Id::NULL.with("formula_seq");
    let n = ui.ctx().data_mut(|d| {
        let n = d.get_temp::<u32>(base).unwrap_or(0);
        d.insert_temp(base, n + 1);
        n
    });
    let step = ui.ctx().data(|d| d.get_temp::<u32>(egui::Id::NULL.with("formula_step"))).unwrap_or(0);
    egui::Id::NULL.with(("formula", step, n))
}

/// Formulele, ascunse implicit sub o săgeată de expandare.
pub fn formula(ui: &mut Ui, lines: &[String]) {
    let p = pal(ui);
    let label = ui.ctx().data(|d| d.get_temp::<String>(egui::Id::NULL.with("formulas_label"))).unwrap_or_else(|| "Formule".into());
    let id = formula_id(ui);
    egui::CollapsingHeader::new(RichText::new(label).size(12.5).color(p.muted)).id_salt(id).default_open(false).show(ui, |ui| {
        soft_box(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            for l in lines {
                ui.add(egui::Label::new(RichText::new(l).monospace().size(12.0).color(p.ink2)).wrap());
            }
        });
    });
}

/// Celulă centrată: titlu, valoare cu unitate, apoi butonul de formule; formula
/// (după deschidere) apare dedesubt.
pub fn centered_result(ui: &mut Ui, title: &str, value: &str, unit: &str, lines: &[String]) {
    use egui::text::{LayoutJob, TextFormat};
    let p = pal(ui);
    let medium = egui::FontId::new(15.0, egui::FontFamily::Name("Medium".into()));
    let mut val = LayoutJob::default();
    val.append(value, 0.0, TextFormat::simple(egui::FontId::monospace(26.0), p.ink));
    val.append(unit, 6.0, TextFormat::simple(egui::FontId::proportional(12.5), p.muted));
    let id = formula_id(ui).with("open");
    let open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let label = ui.ctx().data(|d| d.get_temp::<String>(egui::Id::NULL.with("formulas_label"))).unwrap_or_else(|| "Formule".into());
    let mut clicked = false;
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(RichText::new(title).font(medium).color(p.ink));
        ui.add_space(4.0);
        ui.label(val);
        if !lines.is_empty() {
            let r = ui.add(egui::Label::new(RichText::new(format!("{} {label}", if open { "▾" } else { "▸" })).size(12.5).color(p.muted)).sense(egui::Sense::click()));
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            clicked = r.clicked();
        }
    });
    if clicked {
        ui.data_mut(|d| d.insert_temp(id, !open));
    }
    if open {
        ui.add_space(6.0);
        soft_box(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            for l in lines {
                ui.add(egui::Label::new(RichText::new(l).monospace().size(12.0).color(p.ink2)).wrap());
            }
        });
    }
}

pub fn alert(ui: &mut Ui, text: &str, danger: bool) {
    let p = pal(ui);
    let (bar, bg) = if danger { (p.danger, p.danger_soft) } else { (p.warning, p.warning_soft) };
    let resp = egui::Frame::new().fill(bg).corner_radius(CornerRadius::same(8)).inner_margin(Margin { left: 16, right: 14, top: 10, bottom: 10 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(RichText::new(format!("⚠  {text}")).color(p.ink).size(13.0)).wrap());
    });
    let r = resp.response.rect;
    ui.painter().rect_filled(egui::Rect::from_min_size(r.min, egui::vec2(3.0, r.height())), CornerRadius { nw: 8, sw: 8, ne: 0, se: 0 }, bar);
}

/// Titlul „HidroCalc” în două tonuri (Hidro: text, Calc: accent), font Medium.
pub fn wordmark(ui: &mut Ui, size: f32) {
    let p = pal(ui);
    let font = egui::FontId::new(size, egui::FontFamily::Name("Medium".into()));
    let mut job = egui::text::LayoutJob::default();
    job.append("Hidro", 0.0, egui::text::TextFormat::simple(font.clone(), p.ink));
    job.append("Calc", 0.0, egui::text::TextFormat::simple(font, p.accent));
    ui.label(job);
}

/// Comutator segmentat (ex. RO | EN). Întoarce indexul ales dacă s-a schimbat.
pub fn segmented(ui: &mut Ui, items: &[&str], sel: usize) -> Option<usize> {
    let p = pal(ui);
    let mut out = None;
    egui::Frame::new().fill(p.panel2).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(2)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.spacing_mut().button_padding = egui::vec2(10.0, 3.0);
        ui.spacing_mut().interact_size.y = 24.0;
        ui.horizontal(|ui| {
            for (i, it) in items.iter().enumerate() {
                let on = i == sel;
                let txt = RichText::new(*it).size(12.5).color(if on { p.ink } else { p.muted });
                let b = egui::Button::new(txt).fill(if on { p.panel } else { Color32::TRANSPARENT }).stroke(Stroke::NONE).corner_radius(CornerRadius::same(6));
                if ui.add(b).clicked() && !on {
                    out = Some(i);
                }
            }
        });
    });
    out
}

pub fn step_header(ui: &mut Ui, title: &str, subtitle: &str) {
    let p = pal(ui);
    ui.label(RichText::new(title).size(21.0).strong().color(p.ink));
    ui.label(RichText::new(subtitle).size(13.0).color(p.muted));
    ui.add_space(10.0);
}

pub fn section_title(ui: &mut Ui, text: &str) {
    let p = pal(ui);
    ui.label(RichText::new(text.to_uppercase()).size(11.5).strong().color(p.muted));
    ui.add_space(4.0);
}

/// Eticheta unui câmp; întoarce id-ul ei, pentru a lega câmpul de etichetă (accesibilitate).
fn field_label(ui: &mut Ui, label: &str, tooltip: Option<&str>, computed: bool) -> egui::Id {
    let p = pal(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let id = ui.label(RichText::new(label).size(12.5).color(p.ink2).strong()).id;
        if computed {
            ui.label(RichText::new("calculat").size(10.5).color(p.muted).italics());
        }
        if let Some(tip) = tooltip {
            help_icon(ui, tip);
        }
        id
    })
    .inner
}

/// Bulină cu „?” (ajutor): la hover apare imediat explicația.
fn help_icon(ui: &mut Ui, tip: &str) {
    let p = pal(ui);
    // `click` o face focusabilă cu Tab; explicația apare și când are focus, nu doar la hover.
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
    let hot = resp.hovered() || resp.has_focus();
    let c = rect.center();
    let (line, fill) = if hot { (p.accent, p.accent_soft) } else { (p.muted, Color32::TRANSPARENT) };
    ui.painter().circle(c, 8.5, fill, Stroke::new(1.2, line));
    ui.painter().text(c, egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(12.0), line);
    focus_ring(ui, &resp, 8);
    describe(&resp, egui::Role::Button, tip);
    let resp = resp.on_hover_cursor(egui::CursorIcon::Help);
    let show = |ui: &mut Ui| {
        ui.set_max_width(320.0);
        ui.add(egui::Label::new(RichText::new(tip).size(12.5)).wrap());
    };
    if resp.has_focus() {
        egui::Tooltip::for_widget(&resp).show(show);
    } else {
        resp.on_hover_ui(show);
    }
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
        changed = num_input_labelled(ui, egui::Id::unique(("num", label)).with(ui.scope_id()), value, unit, Some(label_id));
    });
    changed
}

/// Păstrat pentru compatibilitate: unitatea e acum mereu în interiorul câmpului,
/// deci toate câmpurile au aceeași lățime.
pub fn num_field_reserved(ui: &mut Ui, label: &str, value: &mut f64, unit: Option<&str>, tooltip: Option<&str>, _reserve: f32) -> bool {
    num_field(ui, label, value, unit, tooltip)
}

/// Doar căsuța numerică (fără etichetă), pentru tabele.
pub fn num_input(ui: &mut Ui, id: egui::Id, value: &mut f64, _reserve_right: f32) -> bool {
    num_input_labelled(ui, id, value, None, None)
}

fn num_input_labelled(ui: &mut Ui, id: egui::Id, value: &mut f64, unit: Option<&str>, label: Option<egui::Id>) -> bool {
    let buf_id = id.with("buf");
    let edit_id = id.with("edit");
    let focused = ui.memory(|m| m.has_focus(edit_id));
    let mut buf = if focused { ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or_else(|| display_num(*value)) } else { display_num(*value) };
    let width = ui.available_width().max(50.0);
    let right = unit.map_or(8, |u| (u.chars().count() as f32 * 7.5 + 18.0) as i8);
    let mut resp = ui.add(TextEdit::singleline(&mut buf).id(edit_id).desired_width(width).margin(Margin { left: 8, right, top: 4, bottom: 4 }).font(egui::TextStyle::Monospace));
    if let Some(u) = unit {
        ui.painter().text(resp.rect.right_center() - egui::vec2(10.0, 0.0), egui::Align2::RIGHT_CENTER, u, egui::FontId::monospace(11.5), pal(ui).muted);
    }
    if let Some(l) = label {
        resp = resp.labelled_by(l);
    }
    // Text care nu e (încă) un număr: contur roșu + mesaj, nu doar resetare tăcută la 0.
    let t_trim = buf.trim();
    let intermediate = matches!(t_trim, "-" | "+" | "." | "," | "-." | "-,");
    let invalid = !t_trim.is_empty() && !intermediate && parse_decimal(&buf).is_none();
    if invalid {
        let p = pal(ui);
        ui.painter().rect_stroke(resp.rect, CornerRadius::same(7), Stroke::new(1.5, p.danger), egui::StrokeKind::Inside);
        if label.is_some() {
            let msg = ui.ctx().data(|d| d.get_temp::<String>(egui::Id::NULL.with("invalid_number_msg"))).unwrap_or_else(|| "Introdu un număr".into());
            ui.label(RichText::new(format!("⚠ {msg}")).size(11.5).color(p.danger));
        }
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
        changed = ui.add(TextEdit::singleline(value).hint_text(hint).desired_width(ui.available_width())).labelled_by(label_id).changed();
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
pub fn combo_labelled(ui: &mut Ui, id: egui::Id, value: &mut String, options: &[(String, String)], width: f32, label: Option<egui::Id>) -> bool {
    let shown = options.iter().find(|(v, _)| v == value).map(|(_, d)| d.clone()).unwrap_or_else(|| value.clone());
    let mut changed = false;
    let resp = egui::ComboBox::from_id_salt(id).selected_text(shown).width(width).wrap_mode(egui::TextWrapMode::Truncate).show_ui(ui, |ui| {
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
        egui::Frame::new().fill(p.panel).stroke(Stroke::new(1.0, p.border)).corner_radius(CornerRadius::same(7)).inner_margin(Margin::symmetric(8, 0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().interact_size.y = 26.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new(if danger { format!("⚠ {text}") } else { text.to_string() }).monospace().color(if danger { p.danger } else { p.ink2 }));
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
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(RichText::new(label).size(12.0).color(p.muted));
        ui.horizontal(|ui| {
            let base = if size == CardSize::Large { 28.0 } else { 19.0 };
            let sz = if primary { base + 3.0 } else { base };
            ui.label(RichText::new(fmt(value, decimals)).monospace().size(sz).color(if primary { p.accent } else { p.ink }));
            ui.label(RichText::new(unit).size(12.0).color(p.muted));
        });
    });
}

/// Valoare centrată (etichetă deasupra, număr dedesubt), pentru tablouri de sinteză.
pub fn metric_centered(ui: &mut Ui, label: &str, value: f64, unit: &str, decimals: usize) {
    let p = pal(ui);
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.label(RichText::new(label).size(12.0).color(p.muted));
        let mut job = egui::text::LayoutJob::default();
        job.append(&fmt(value, decimals), 0.0, egui::text::TextFormat::simple(egui::FontId::monospace(22.0), p.ink));
        if !unit.is_empty() {
            job.append(unit, 5.0, egui::text::TextFormat::simple(egui::FontId::proportional(12.0), p.muted));
        }
        ui.label(job);
    });
}

/// Buton plat de bară de instrumente; cel selectat are fundal discret.
pub fn tool_button(ui: &mut Ui, text: &str, selected: bool) -> egui::Response {
    let p = pal(ui);
    let txt = RichText::new(text).size(12.5).color(if selected { p.ink } else { p.ink2 });
    let b = egui::Button::new(txt).corner_radius(CornerRadius::same(6));
    if selected {
        ui.add(b.fill(p.accent_soft).stroke(Stroke::NONE))
    } else {
        ui.add(b.frame_when_inactive(false))
    }
}

/// Ca `tool_button`, dar pe toată lățimea disponibilă (paleta laterală).
pub fn tool_button_w(ui: &mut Ui, text: &str, selected: bool) -> egui::Response {
    let p = pal(ui);
    let txt = RichText::new(text).size(12.5).color(if selected { p.ink } else { p.ink2 });
    let b = egui::Button::new(txt).corner_radius(CornerRadius::same(6));
    let size = egui::vec2(ui.available_width(), 26.0);
    if selected {
        ui.add_sized(size, b.fill(p.accent_soft).stroke(Stroke::NONE))
    } else {
        ui.add_sized(size, b.fill(p.panel2).stroke(Stroke::NONE))
    }
}

/// Buton de paletă pe toată lățimea, cu pictogramă (desenată de `icon`) și text.
pub fn tool_button_icon(ui: &mut Ui, text: &str, selected: bool, icon: impl FnOnce(&egui::Painter, egui::Pos2, Color32)) -> egui::Response {
    let p = pal(ui);
    let size = egui::vec2(ui.available_width(), 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let hot = resp.hovered();
    let fill = if selected {
        p.accent_soft
    } else if hot {
        p.border
    } else {
        p.panel2
    };
    ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
    let fg = if selected { p.accent } else { p.ink2 };
    icon(ui.painter(), egui::pos2(rect.left() + 15.0, rect.center().y), fg);
    ui.painter().text(egui::pos2(rect.left() + 30.0, rect.center().y), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(12.5), if selected { p.ink } else { p.ink2 });
    focus_ring(ui, &resp, 6);
    describe(&resp, egui::Role::Button, text);
    resp
}

/// Buton pătrat cu pictogramă desenată de `icon` (painter, dreptunghi, culoare, fundal), cu fundal discret.
pub fn icon_button(ui: &mut Ui, tip: &str, icon: impl FnOnce(&egui::Painter, egui::Rect, Color32, Color32)) -> egui::Response {
    let p = pal(ui);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(34.0, 30.0), egui::Sense::click());
    let fill = if resp.hovered() { p.accent_soft } else { p.panel2 };
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    icon(ui.painter(), rect, if resp.hovered() { p.accent } else { p.ink2 }, fill);
    focus_ring(ui, &resp, 8);
    describe(&resp, egui::Role::Button, tip);
    resp.on_hover_text(tip)
}

/// Pictograma temei: soare (în dark, ca să treci la light) sau lună (în light), desenate cu
/// culorile temei curente.
pub fn paint_theme_icon(p: &egui::Painter, r: egui::Rect, col: Color32, bg: Color32, dark: bool) {
    let c = r.center();
    if dark {
        p.circle_filled(c, 3.6, col);
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            let (s, co) = a.sin_cos();
            p.line_segment([c + egui::vec2(co, s) * 6.2, c + egui::vec2(co, s) * 8.2], Stroke::new(1.4, col));
        }
    } else {
        p.circle_filled(c, 6.2, col);
        p.circle_filled(c + egui::vec2(3.2, -2.6), 5.4, bg);
    }
}

/// „✕” care cere confirmare: primul click îl face „Confirmă ștergerea” (3 secunde), al doilea șterge.
pub fn confirm_delete(ui: &mut Ui, id: egui::Id, tip: &str, confirm_text: &str) -> bool {
    let p = pal(ui);
    let now = ui.input(|i| i.time);
    let armed_at = ui.data(|d| d.get_temp::<f64>(id));
    let armed = armed_at.is_some_and(|t| now - t < 3.0);
    if armed_at.is_some() && !armed {
        ui.data_mut(|d| d.remove::<f64>(id));
    }
    let text = if armed { RichText::new(confirm_text).size(12.0).color(Color32::WHITE) } else { RichText::new("✕").color(p.muted) };
    let mut b = egui::Button::new(text);
    b = if armed { b.fill(p.danger).stroke(Stroke::NONE) } else { b.frame(false) };
    let r = ui.add(b);
    describe(&r, egui::Role::Button, if armed { confirm_text } else { tip });
    let r = r.on_hover_text(tip);
    if armed {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    }
    if r.clicked() {
        if armed {
            ui.data_mut(|d| d.remove::<f64>(id));
            return true;
        }
        ui.data_mut(|d| d.insert_temp(id, now));
    }
    false
}

pub fn primary_button(ui: &mut Ui, text: &str) -> egui::Response {
    let p = pal(ui);
    ui.add(egui::Button::new(RichText::new(text).color(p.accent_fg).strong()).fill(p.accent).stroke(Stroke::NONE).corner_radius(CornerRadius::same(8)))
}

pub fn secondary_button(ui: &mut Ui, text: &str) -> egui::Response {
    let p = pal(ui);
    ui.add(egui::Button::new(RichText::new(text).color(p.ink2)).fill(p.panel).stroke(Stroke::new(1.0, p.border)).corner_radius(CornerRadius::same(8)))
}

pub fn danger_button(ui: &mut Ui, text: &str, armed: bool) -> egui::Response {
    let p = pal(ui);
    let (fg, bg) = if armed { (Color32::WHITE, p.danger) } else { (p.ink2, p.panel2) };
    ui.add(egui::Button::new(RichText::new(text).color(fg).size(12.5)).fill(bg).stroke(Stroke::NONE))
}

/// Navigarea Înapoi / Următorul e acum în bara de sus a aplicației; păstrată ca no-op
/// pentru ca pașii să nu mai deseneze butoane proprii.
pub fn step_nav(_ui: &mut Ui, _t: &crate::i18n::I18n, _back: Option<u32>, _next: Option<u32>) -> Option<u32> {
    None
}

/// Conținut centrat cu lățime maximă (echivalentul `max-w-* mx-auto`). Lățimea e „fluidă”:
/// maximul primit se mărește cu 30% (până la 1320), ca să nu sară conținutul când bara laterală
/// se ascunde sau se arată, ci să umple spațiul.
pub fn centered<R>(ui: &mut Ui, max_width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let avail = ui.available_width();
    let w = avail.min((max_width * 1.3).min(1320.0));
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

#[cfg(test)]
mod a11y_tests {
    use egui_kittest::{kittest::Queryable, Harness};

    #[test]
    fn invalid_number_shows_an_inline_message() {
        let mut h = Harness::new_ui_state(
            |ui, v: &mut f64| {
                super::num_field(ui, "Debit", v, Some("l/s"), None);
            },
            0.0_f64,
        );
        let input = h.get_by_role(egui::accesskit::Role::TextInput);
        input.focus();
        input.type_text("abc");
        h.run();
        assert!(h.query_by_label_contains("Introdu un număr").is_some(), "lipsește mesajul de eroare lângă câmp");
    }
}

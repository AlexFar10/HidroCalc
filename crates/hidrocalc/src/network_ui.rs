//! Pasul 8 (tronsoane, constante, diametre PEHD, inspector) și pasul 9 (rezultate).

use crate::charts;
use crate::i18n::I18n;
use crate::widgets::{self as w, fmt, pal, CardSize};
use eframe::egui::{self, RichText, Stroke, Ui};
use hidro_core::calculations::network::{NetworkSection, PEHD_DIAMETERS};
use hidro_core::model::ProjectData;
use hidro_core::pipeline::{NetworkError, ProjectResults};
use hidro_core::schema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Sections,
    Constants,
    Diameters,
}

/// Starea de interfață a pasului 8 (nu se salvează în proiect).
#[derive(Default)]
pub struct NetworkUi {
    pub tab: Tab,
    pub selected: Option<String>,
    pub error: Option<NetworkError>,
    pub notice: Option<(String, std::time::Instant)>,
    pub canvas: crate::canvas::CanvasUi,
    pub view3d: crate::view3d::View3D,
    pub show_3d: bool,
    /// Ecran complet: zona de desen ocupă toată fereastra (fără bara laterală și pași).
    pub expanded: bool,
    /// Export cerut din interfață; îl execută aplicația după desenarea cadrului.
    pub export: Option<ExportKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Dxf,
    Xlsx,
    Pdf,
    Breviar,
    Memoriu,
    /// Alegerea unui alt șablon (true = Word, false = Excel), apoi exportul.
    PickTemplate(bool),
}

const AREA_ID: &str = "hidrocalc-area-height";
pub const AREA_MIN: f32 = 300.0;
const AREA_DEFAULT: f32 = 520.0;

/// Înălțimea zonei de desen (2D și 3D): reglată de utilizator și păstrată între sesiuni;
/// în ecran complet, tot spațiul vertical disponibil.
pub fn area_height(ui: &Ui, expanded: bool) -> f32 {
    if expanded {
        return (ui.ctx().content_rect().height() - 250.0).max(AREA_MIN);
    }
    ui.ctx().data_mut(|d| d.get_persisted::<f32>(egui::Id::unique(AREA_ID))).unwrap_or(AREA_DEFAULT)
}

/// Mânerul de sub zona de desen: tras în sus/jos o micșorează/mărește.
fn resize_grip(ui: &mut Ui, t: &I18n) {
    let pl = pal(ui);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 12.0), egui::Sense::drag());
    let resp = resp.on_hover_cursor(egui::CursorIcon::ResizeVertical).on_hover_text(t.t("native.resizeArea"));
    let c = if resp.hovered() || resp.dragged() { pl.accent } else { pl.border_strong };
    let mid = rect.center();
    for dy in [-2.0, 2.0] {
        ui.painter().line_segment([mid + egui::vec2(-24.0, dy), mid + egui::vec2(24.0, dy)], Stroke::new(1.5, c));
    }
    if resp.dragged() {
        let id = egui::Id::unique(AREA_ID);
        let cur = ui.ctx().data_mut(|d| d.get_persisted::<f32>(id)).unwrap_or(AREA_DEFAULT);
        let h = (cur + resp.drag_delta().y).clamp(AREA_MIN, 3000.0);
        ui.ctx().data_mut(|d| d.insert_persisted(id, h));
    }
    if resp.double_clicked() {
        ui.ctx().data_mut(|d| d.insert_persisted(egui::Id::unique(AREA_ID), AREA_DEFAULT));
    }
}

pub fn error_text(t: &I18n, e: &NetworkError) -> String {
    match e {
        NetworkError::Cycle => t.t("errors.ERR_CYCLE_DETECTED"),
        NetworkError::InvalidConstants(c) => format!("{} {}", t.t("native.invalidConstants"), c.join(", ")),
    }
}

fn regime(t: &I18n, r: &str) -> String {
    if r.is_empty() {
        "-".into()
    } else {
        let k = format!("network.regime.{r}");
        let s = t.t(&k);
        if s == k { r.into() } else { s }
    }
}

/// Celulă cu override manual: gol = valoarea calculată (afișată ca indiciu).
fn override_cell(ui: &mut Ui, id: egui::Id, ov: &mut Option<f64>, computed: f64, dec: usize) {
    let p = pal(ui);
    let edit_id = id.with("e");
    let focused = ui.memory(|m| m.has_focus(edit_id));
    let mut buf = if focused {
        ui.data(|d| d.get_temp::<String>(id)).unwrap_or_default()
    } else {
        ov.map(|v| format!("{v}")).unwrap_or_default()
    };
    let color = if ov.is_some() { p.accent } else { p.muted };
    let resp = ui.add_sized(
        [74.0, 24.0],
        egui::TextEdit::singleline(&mut buf)
            .id(edit_id)
            .font(egui::TextStyle::Monospace)
            .text_color(color)
            .hint_text(fmt(computed, dec)),
    );
    if resp.changed() {
        if buf.trim().is_empty() {
            *ov = None;
        } else if let Some(v) = w::parse_decimal(&buf) {
            *ov = Some(v);
        }
    }
    if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, buf));
    }
}

/// Câmp numeric opțional: gol = fără valoare.
fn opt_num(ui: &mut Ui, id: egui::Id, v: &mut Option<f64>) -> bool {
    let edit_id = id.with("e");
    let focused = ui.memory(|m| m.has_focus(edit_id));
    let mut buf = if focused { ui.data(|d| d.get_temp::<String>(id)).unwrap_or_default() } else { v.map(|x| format!("{x}")).unwrap_or_default() };
    let resp = ui.add(egui::TextEdit::singleline(&mut buf).id(edit_id).desired_width(ui.available_width() - 20.0).font(egui::TextStyle::Monospace).hint_text("—"));
    let mut changed = false;
    if resp.changed() {
        let nv = if buf.trim().is_empty() { None } else { w::parse_decimal(&buf).map(|x| (x * 100.0).round() / 100.0) };
        if buf.trim().is_empty() || nv.is_some() {
            changed = nv != *v;
            *v = nv;
        }
    }
    if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, buf));
    }
    changed
}

fn header(ui: &mut Ui, text: &str, tip: Option<String>) {
    let p = pal(ui);
    let r = ui.label(RichText::new(text).size(11.0).strong().color(p.muted));
    if let Some(tip) = tip {
        r.on_hover_text(tip);
    }
}

/// Tabelul de rezultate hidraulice. Editabil la pasul 8, doar citire la pasul 9.
fn results_table(ui: &mut Ui, t: &I18n, p: &mut ProjectData, st: Option<&mut NetworkUi>) {
    let pl = pal(ui);
    let editable = st.is_some();
    let mut selected = st.as_ref().and_then(|s| s.selected.clone());
    let mut remove = None;
    let mut swap: Option<(usize, usize)> = None;
    let mut length_changes: Vec<(String, f64)> = Vec::new();
    let cols = [
        ("colNrCrt", false),
        ("colSection", false),
        ("colQi", true),
        ("colQf", true),
        ("colQie", true),
        ("colQc", true),
        ("colL", true),
        ("colDp", true),
        ("colDiStas", true),
        ("colDeStas", true),
        ("colVef", true),
        ("colRe", true),
        ("colDOverK", true),
        ("colLambda", true),
        ("colRegim", true),
    ];
    egui::ScrollArea::horizontal().id_salt(("net_table", editable)).show(ui, |ui| {
        egui::Grid::new(("net_grid", editable)).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
            if editable {
                ui.label("");
            }
            for (k, tip) in cols {
                let tip = tip.then(|| t.t(&format!("step8.{k}Tip"))).filter(|s| !s.starts_with("step8."));
                header(ui, &t.t(&format!("step8.{k}")), tip);
            }
            ui.end_row();

            let n = p.network_sections.len();
            let groups: Vec<String> = p.network_sections.iter().map(|s| s.group.clone().unwrap_or_default()).collect();
            for i in 0..n {
                let show_group = if i == 0 { !groups[0].is_empty() } else { groups[i] != groups[i - 1] };
                if show_group {
                    if editable {
                        ui.label("");
                    }
                    ui.label("");
                    let g = if groups[i].is_empty() { t.t("step8.mainSchema") } else { groups[i].clone() };
                    ui.label(RichText::new(g.to_uppercase()).size(10.5).strong().color(pl.muted));
                    ui.end_row();
                }
                let s = &mut p.network_sections[i];
                let id = egui::Id::unique(("sec", &s.id));
                let is_sel = selected.as_deref() == Some(s.id.as_str());
                if editable {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let up = i > 0 && groups[i - 1] == groups[i];
                        let down = i + 1 < n && groups[i + 1] == groups[i];
                        if ui.add_enabled(up, egui::Button::new("▲").small().frame(false)).clicked() {
                            swap = Some((i, i - 1));
                        }
                        if ui.add_enabled(down, egui::Button::new("▼").small().frame(false)).clicked() {
                            swap = Some((i, i + 1));
                        }
                    });
                }
                let nr = ui.selectable_label(is_sel, RichText::new(format!("{}", i + 1)).monospace());
                if nr.clicked() {
                    selected = if is_sel { None } else { Some(s.id.clone()) };
                }
                if editable {
                    let r = ui.add_sized([84.0, 24.0], egui::TextEdit::singleline(&mut s.name));
                    if r.gained_focus() {
                        selected = Some(s.id.clone());
                    }
                    override_cell(ui, id.with("qi"), &mut s.q_i_override, s.q_i, 3);
                    override_cell(ui, id.with("qf"), &mut s.q_f_override, s.q_f, 3);
                    override_cell(ui, id.with("qie"), &mut s.q_ie_override, s.q_ie, 3);
                    override_cell(ui, id.with("qc"), &mut s.q_c_override, s.q_c, 3);
                    let before = s.length;
                    ui.scope(|ui| {
                        ui.set_min_width(74.0);
                        ui.set_max_width(74.0);
                        w::num_input(ui, id.with("l"), &mut s.length, 0.0);
                    });
                    if s.length != before {
                        length_changes.push((s.name.clone(), s.length));
                    }
                    override_cell(ui, id.with("dp"), &mut s.d_calc_override, s.d_calc, 2);
                    override_cell(ui, id.with("di"), &mut s.d_std_int_override, s.d_std_int, 1);
                } else {
                    ui.label(RichText::new(&s.name).strong());
                    for (v, d) in [(s.q_i, 3), (s.q_f, 3)] {
                        ui.label(RichText::new(fmt(v, d)).monospace().color(pl.muted));
                    }
                    ui.label(RichText::new(if s.q_ie > 0.0 { fmt(s.q_ie, 3) } else { "-".into() }).monospace().color(pl.muted));
                    ui.label(RichText::new(fmt(s.q_c, 3)).monospace().color(pl.accent));
                    for (v, d) in [(s.length, 1), (s.d_calc, 2), (s.d_std_int, 1)] {
                        ui.label(RichText::new(fmt(v, d)).monospace().color(pl.muted));
                    }
                }
                let v_warn = s.v_real > 0.0 && !(0.5..=2.5).contains(&s.v_real);
                ui.label(RichText::new(fmt(s.d_std_ext, 1)).monospace().color(pl.muted));
                ui.label(RichText::new(fmt(s.v_real, 3)).monospace().color(if v_warn { pl.warning } else { pl.muted }));
                ui.label(RichText::new(fmt(s.re, 0)).monospace().color(pl.muted));
                ui.label(RichText::new(if s.d_over_k > 0.0 { fmt(s.d_over_k, 0) } else { "-".into() }).monospace().color(pl.muted));
                ui.label(RichText::new(fmt(s.lambda, 4)).monospace().color(pl.muted));
                ui.label(RichText::new(regime(t, &s.regime)).color(pl.muted));
                if editable && ui.add(egui::Button::new(RichText::new("✕").color(pl.muted)).frame(false)).on_hover_text(t.t("common.delete")).clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    });
    for (name, len) in length_changes {
        schema::set_tronson_override_length(&mut p.network_schema, &name, (len > 0.0).then_some(len));
    }
    if let Some((a, b)) = swap {
        p.network_sections.swap(a, b);
    }
    if let Some(i) = remove {
        let id = p.network_sections.remove(i).id;
        for s in &mut p.network_sections {
            if s.parent_section_id.as_deref() == Some(id.as_str()) {
                s.parent_section_id = None;
            }
        }
        if selected.as_deref() == Some(id.as_str()) {
            selected = None;
        }
    }
    if let Some(st) = st {
        st.selected = selected;
    }
}

fn inspector(ui: &mut Ui, t: &I18n, p: &mut ProjectData, st: &mut NetworkUi) {
    let pl = pal(ui);
    let Some(sel) = st.selected.clone() else { return };
    let Some(idx) = p.network_sections.iter().position(|s| s.id == sel) else {
        st.selected = None;
        return;
    };
    let others: Vec<(String, String)> = std::iter::once((String::new(), t.t("step8.sourceReservoir")))
        .chain(p.network_sections.iter().filter(|s| s.id != sel).map(|s| (s.id.clone(), s.name.clone())))
        .collect();
    let s = &mut p.network_sections[idx];
    w::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(t.t("step8.selected")).size(10.5).strong().color(pl.muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("✕").frame(false)).on_hover_text(t.t("common.close")).clicked() {
                    st.selected = None;
                }
            });
        });
        ui.label(RichText::new(&s.name).size(15.0).strong());
        ui.add_space(4.0);
        let kpi = |ui: &mut Ui, label: &str, val: String, warn: bool| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).size(12.0).color(pl.muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(val).monospace().size(12.0).color(if warn { pl.warning } else { pl.ink2 }));
                });
            });
        };
        let v_warn = s.v_real > 0.0 && !(0.5..=2.5).contains(&s.v_real);
        kpi(ui, "Qc", format!("{} l/s", fmt(s.q_c, 4)), false);
        kpi(ui, "D ext", format!("{} mm", fmt(s.d_std_ext, 0)), false);
        kpi(ui, "v", format!("{} m/s", fmt(s.v_real, 3)), v_warn);
        kpi(ui, "Re", fmt(s.re, 0), false);
        kpi(ui, "h_tot", format!("{} m", fmt(s.h_tot, 4)), false);
        if v_warn {
            let msg = if s.v_real < 0.5 {
                format!("⚠ v = {} m/s < 0.5 m/s — risc de depuneri.", fmt(s.v_real, 3))
            } else {
                format!("⚠ v = {} m/s > 2.5 m/s — risc de eroziune.", fmt(s.v_real, 3))
            };
            w::alert(ui, &msg, false);
        }
        ui.separator();
        let id = egui::Id::unique(("insp", &sel));
        let mut parent = s.parent_section_id.clone().unwrap_or_default();
        if w::combo(ui, &t.t("native.parent"), &mut parent, &others) {
            s.parent_section_id = (!parent.is_empty()).then_some(parent);
        }
        w::num_field(ui, &t.t("native.qExtern"), &mut s.q_extern, Some("l/s"), None);
        w::num_field(ui, &t.t("native.qLinear"), &mut s.q_linear, Some("l/s·m"), None);
        w::num_field(ui, &t.t("native.zeta"), &mut s.zeta, None, None);
        ui.checkbox(&mut s.has_hydrant, t.t("native.hasHydrant"));
        ui.separator();
        for (label, val, src) in [
            (t.t("terrain.cotaUp"), &mut s.cota_up, &mut s.cota_up_source),
            (t.t("terrain.cotaDown"), &mut s.cota_down, &mut s.cota_down_source),
        ] {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&label).size(12.0).color(pl.ink2).strong());
                let badge = match (src.as_deref(), val.is_some()) {
                    (Some("dem"), true) => Some((t.t("terrain.demBadge"), pl.warning)),
                    (Some("topo"), true) => Some((t.t("terrain.topoBadge"), charts::GREEN)),
                    (_, true) => Some((t.t("terrain.manualBadge"), pl.muted)),
                    _ => None,
                };
                if let Some((b, c)) = badge {
                    ui.label(RichText::new(b).size(10.0).color(c));
                }
            });
            ui.horizontal(|ui| {
                if opt_num(ui, id.with(&label), val) {
                    *src = val.map(|_| "manual".to_string());
                }
                ui.label(RichText::new("m").size(11.0).color(pl.muted));
            });
        }
    });
}

pub fn step8(ui: &mut Ui, t: &I18n, p: &mut ProjectData, st: &mut NetworkUi) -> Option<u32> {
    let pl = pal(ui);
    let mut go = None;
    let title = t.list("steps.names").get(7).cloned().unwrap_or_default();
    w::step_header(ui, &title, &t.t("native.step8Subtitle"));

    ui.horizontal(|ui| {
        for (three, key) in [(false, "view3d.mode2D"), (true, "view3d.mode3D")] {
            let sel = st.show_3d == three;
            let txt = RichText::new(t.t(key)).strong().color(if sel { pl.accent_fg } else { pl.muted });
            if ui.add(egui::Button::new(txt).fill(if sel { pl.accent } else { pl.panel })).clicked() {
                st.show_3d = three;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let key = if st.expanded { "native.exitFullscreen" } else { "native.fullscreen" };
            if ui.add(egui::Button::new(format!("⛶ {}", t.t(key))).selected(st.expanded)).on_hover_text(t.t("native.fullscreenTip")).clicked() {
                st.expanded = !st.expanded;
            }
        });
    });
    if st.expanded && ui.input(|i| i.key_pressed(egui::Key::Escape)) && st.canvas.selected.is_none() {
        st.expanded = false;
    }
    ui.add_space(4.0);
    let area_h = area_height(ui, st.expanded);
    w::card(ui, |ui| {
        if st.show_3d {
            st.view3d.ui(ui, t, p, area_h);
            if !st.expanded {
                resize_grip(ui, t);
            }
        } else {
            st.canvas.ui(ui, t, p, area_h);
            if !st.expanded {
                resize_grip(ui, t);
            }
            if std::mem::take(&mut st.canvas.export_dxf) {
                st.export = Some(ExportKind::Dxf);
            }
            ui.label(RichText::new(t.t("native.canvasHint")).size(11.0).color(pl.muted));
        }
    });
    let n_schema = schema::count_tronsoane(&p.network_schema);
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        for (tab, key) in [(Tab::Sections, "step8.tabSections"), (Tab::Constants, "step8.tabConstants"), (Tab::Diameters, "step8.tabDiameters")] {
            let sel = st.tab == tab;
            let txt = RichText::new(t.t(key)).strong().color(if sel { pl.accent } else { pl.muted });
            if ui.add(egui::Button::new(txt).fill(if sel { pl.accent_soft } else { egui::Color32::TRANSPARENT }).stroke(Stroke::NONE)).clicked() {
                st.tab = tab;
            }
        }
    });
    ui.add_space(6.0);

    match st.tab {
        Tab::Constants => {
            let c = &mut p.network_constants;
            w::card(ui, |ui| {
                w::section_title(ui, &t.t("step8.tabConstants"));
                ui.columns(3, |col| {
                    w::num_field(&mut col[0], &t.t("step8.roughness"), &mut c.roughness, Some("m"), Some("Rugozitatea absolută a peretelui conductei (k), folosită în Darcy-Weisbach. Pentru PEHD: ~0.0001 m."));
                    w::num_field(&mut col[1], &t.t("step8.viscosity"), &mut c.viscosity, Some("m²/s"), Some("Vâscozitatea cinematică a apei, folosită la numărul Reynolds. La 10°C: 1.31×10⁻⁶ m²/s."));
                    w::num_field(&mut col[2], &t.t("step8.economicVelocity"), &mut c.economic_velocity, Some("m/s"), Some("Viteza economică folosită pentru predimensionarea diametrului D_calc. Sursă: NP 133 (uzual 0.7–1.5 m/s)."));
                });
            });
        }
        Tab::Diameters => {
            w::card(ui, |ui| {
                w::section_title(ui, &t.t("step8.pehdTitle"));
                egui::Grid::new("pehd").striped(true).spacing([40.0, 4.0]).show(ui, |ui| {
                    for h in ["D_ext [mm]", "Grosime [mm]", "D_int [mm]"] {
                        header(ui, h, None);
                    }
                    ui.end_row();
                    for d in PEHD_DIAMETERS {
                        ui.label(RichText::new(fmt(d.d_ext, 0)).monospace());
                        ui.label(RichText::new(fmt(d.thickness(), 1)).monospace().color(pl.muted));
                        ui.label(RichText::new(fmt(d.d_int, 1)).monospace().color(pl.accent));
                        ui.end_row();
                    }
                });
            });
        }
        Tab::Sections => {
            if let Some(e) = &st.error {
                w::alert(ui, &error_text(t, e), true);
                ui.add_space(6.0);
            }
            let has_sel = st.selected.is_some();
            ui.horizontal_top(|ui| {
                let table_w = if has_sel { ui.available_width() - 280.0 } else { ui.available_width() };
                ui.vertical(|ui| {
                    ui.set_width(table_w);
                    w::card(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(t.t("step8.hydraulicResults")).strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if w::primary_button(ui, &t.t("step8.addSection")).clicked() {
                                    let name = format!("Tr{}", p.network_sections.len() + 1);
                                    let s = NetworkSection::new(uuid::Uuid::new_v4().to_string(), name);
                                    st.selected = Some(s.id.clone());
                                    p.network_sections.push(s);
                                }
                                if w::secondary_button(ui, &t.t("step8.importFromCanvas")).on_hover_text(t.t("step8.importFromCanvasTip")).clicked() {
                                    if n_schema == 0 {
                                        st.notice = Some((t.t("step8.noCanvasSections"), std::time::Instant::now()));
                                    } else {
                                        p.network_sections = schema::build_sections_from_schema(
                                            &p.network_schema,
                                            &p.network_sections,
                                            scale_ratio(p),
                                            || uuid::Uuid::new_v4().to_string(),
                                        );
                                    }
                                }
                            });
                        });
                        if let Some((msg, at)) = &st.notice {
                            if at.elapsed().as_secs() < 4 {
                                ui.label(RichText::new(msg).size(12.0).color(pl.danger));
                                ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
                            } else {
                                st.notice = None;
                            }
                        }
                        ui.label(RichText::new(t.t("native.canvasPending")).size(11.0).color(pl.muted));
                        if p.network_sections.is_empty() {
                            ui.add_space(16.0);
                            ui.vertical_centered(|ui| ui.label(RichText::new(t.t("step8.noSections")).color(pl.muted)));
                            ui.add_space(16.0);
                        } else {
                            ui.label(RichText::new(t.t("step8.overrideHint")).size(11.0).color(pl.muted));
                            ui.add_space(4.0);
                            results_table(ui, t, p, Some(st));
                        }
                    });
                });
                if has_sel {
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.set_width(268.0);
                        inspector(ui, t, p, st);
                    });
                }
            });
        }
    }
    if let Some(g) = w::step_nav(ui, t, Some(7), Some(9)) {
        go = Some(g);
    }
    go
}

fn scale_ratio(p: &ProjectData) -> f64 {
    crate::canvas::scale_ratio(p)
}

pub fn step9(ui: &mut Ui, t: &I18n, p: &mut ProjectData, r: &ProjectResults, net: &mut NetworkUi) -> Option<u32> {
    let pl = pal(ui);
    w::centered(ui, 1100.0, |ui| {
        w::step_header(ui, &t.t("step9.title"), &t.t("step9.subtitle"));
        let secs = &p.network_sections;
        let total_len: f64 = secs.iter().map(|s| s.length).sum();
        let total_h: f64 = secs.iter().map(|s| s.h_tot).sum();
        ui.columns(4, |c| {
            w::result_card(&mut c[0], &t.t("step9.maxHourlyFlow"), r.water_demand.q_s_or_max, "m³/h", true, CardSize::Large);
            w::result_card(&mut c[1], &t.t("step9.reservoirVolume"), r.reservoir.v_total, "m³", true, CardSize::Large);
            w::result_card_dec(&mut c[2], &t.t("step9.totalNetworkLength"), total_len, "m", false, CardSize::Large, 1);
            w::result_card_dec(&mut c[3], &t.t("step9.sectionsCount"), secs.len() as f64, "", false, CardSize::Large, 0);
        });
        ui.add_space(8.0);
        ui.columns(3, |c| {
            w::result_card_dec(&mut c[0], &t.t("step9.totalNetworkLoss"), total_h, "m", false, CardSize::Normal, 4);
            w::result_card(&mut c[1], &t.t("step9.qZiMax"), r.water_demand.q_s_zi_max, "m³/zi", false, CardSize::Normal);
            w::result_card(&mut c[2], &t.t("step9.qOrMaxLs"), r.water_demand.q_s_or_max_ls, "l/s", false, CardSize::Normal);
        });
        ui.add_space(12.0);

        if secs.is_empty() {
            let mut go = None;
            w::card(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label(RichText::new(t.t("native.noSections")).strong());
                    ui.label(RichText::new(t.t("native.noSectionsHint")).color(pl.muted));
                    ui.add_space(6.0);
                    if w::primary_button(ui, &t.t("native.goToStep8")).clicked() {
                        go = Some(8);
                    }
                    ui.add_space(10.0);
                });
            });
            return go.or_else(|| w::step_nav(ui, t, Some(8), None));
        }

        let labels: Vec<String> = secs.iter().map(|s| s.name.clone()).collect();
        ui.columns(2, |c| {
            w::card(&mut c[0], |ui| {
                ui.label(RichText::new(t.t("step9.chartFlows")).strong());
                charts::bars(ui, &labels, &[(secs.iter().map(|s| s.q_c).collect(), charts::CYAN, "Qc".into())], "l/s", 230.0);
            });
            w::card(&mut c[1], |ui| {
                ui.label(RichText::new(t.t("step9.chartHeadloss")).strong());
                charts::bars(
                    ui,
                    &labels,
                    &[
                        (secs.iter().map(|s| s.h_lin).collect(), charts::CYAN, t.t("charts.linearLosses")),
                        (secs.iter().map(|s| s.h_loc).collect(), charts::AMBER, t.t("charts.localLosses")),
                    ],
                    "m",
                    230.0,
                );
            });
        });
        ui.add_space(10.0);
        w::card(ui, |ui| {
            ui.label(RichText::new(t.t("step9.chartVelocity")).strong());
            ui.label(RichText::new(t.t("step9.velocityLegend")).size(11.0).color(pl.muted));
            let pts: Vec<(String, f64, f64)> =
                secs.iter().filter(|s| s.d_std_int > 0.0).map(|s| (s.name.clone(), s.d_std_int, s.v_real)).collect();
            charts::velocity_scatter(ui, &pts, 240.0);
        });
        ui.add_space(10.0);
        w::card(ui, |ui| {
            ui.label(RichText::new(t.t("step8.hydraulicResults")).strong());
            ui.add_space(4.0);
            results_table(ui, t, p, None);
        });
        ui.add_space(10.0);
        documentation(ui, t, net);
        w::step_nav(ui, t, Some(8), None)
    })
}

/// Panoul „Documentație” (echivalentul `DocumentationPanel.tsx`).
fn documentation(ui: &mut Ui, t: &I18n, net: &mut NetworkUi) {
    let pl = pal(ui);
    w::card(ui, |ui| {
        ui.label(RichText::new(t.t("docs.title")).size(15.0).strong());
        ui.label(RichText::new(t.t("docs.subtitle")).size(12.0).color(pl.muted));
        ui.add_space(8.0);
        let mut pick: Option<bool> = None;
        let mut row = |ui: &mut Ui, title: String, desc: String, ready: bool, template: Option<bool>| -> bool {
            let mut clicked = false;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - 160.0).max(200.0));
                    ui.label(RichText::new(title).strong());
                    ui.add(egui::Label::new(RichText::new(desc).size(12.0).color(pl.muted)).wrap());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ready {
                        clicked = w::primary_button(ui, &t.t("docs.generate")).clicked();
                        if let Some(word) = template {
                            if w::secondary_button(ui, &t.t("docs.pickTemplate")).on_hover_text(t.t("docs.pickTemplateTip")).clicked() {
                                pick = Some(word);
                            }
                        }
                    } else {
                        ui.label(RichText::new(t.t("native.notMigratedTitle")).size(11.5).italics().color(pl.muted));
                    }
                });
            });
            ui.separator();
            clicked
        };
        if row(ui, t.t("docs.memoriu.title"), t.t("native.memoriuDesc"), true, Some(true)) {
            net.export = Some(ExportKind::Memoriu);
        }
        if row(ui, t.t("docs.cantitati.title"), t.t("docs.cantitati.desc"), true, Some(false)) {
            net.export = Some(ExportKind::Xlsx);
        }
        if row(ui, t.t("docs.breviar.title"), t.t("docs.breviar.desc"), true, None) {
            net.export = Some(ExportKind::Breviar);
        }
        if row(ui, t.t("nav.exportPdf"), t.t("native.pdfDesc"), true, None) {
            net.export = Some(ExportKind::Pdf);
        }
        if row(ui, t.t("native.dxfTitle"), t.t("native.dxfDesc"), true, None) {
            net.export = Some(ExportKind::Dxf);
        }
        if let Some(word) = pick {
            net.export = Some(ExportKind::PickTemplate(word));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;
    use egui_kittest::Harness;

    #[test]
    fn dragging_the_grip_resizes_the_area() {
        let t = I18n::new(Lang::Ro);
        let mut h = Harness::new_ui_state(
            |ui, height: &mut f32| {
                ui.add_space(100.0);
                resize_grip(ui, &t);
                *height = area_height(ui, false);
            },
            0.0_f32,
        );
        h.run();
        assert_eq!(*h.state(), AREA_DEFAULT);
        // Mânerul are 12 px înălțime, imediat sub spațiul de 100 px.
        let grip = egui::pos2(200.0, 100.0 + 8.0 + 6.0);
        h.hover_at(grip);
        h.run();
        h.drag_at(grip);
        h.run();
        for i in 1..=4 {
            h.hover_at(grip + egui::vec2(0.0, 30.0 * i as f32));
            h.run();
        }
        h.drop_at(grip + egui::vec2(0.0, 120.0));
        h.run();
        assert!((*h.state() - (AREA_DEFAULT + 120.0)).abs() < 2.0, "înălțime: {}", h.state());
        // Nu sub minim (mânerul rămâne la aceeași poziție în acest test)
        h.hover_at(grip);
        h.run();
        h.drag_at(grip);
        h.run();
        for i in 1..=4 {
            h.hover_at(grip - egui::vec2(0.0, 300.0 * i as f32));
            h.run();
        }
        h.drop_at(grip + egui::vec2(0.0, -1100.0));
        h.run();
        assert_eq!(*h.state(), AREA_MIN);
        let _ = Stroke::NONE;
    }
}

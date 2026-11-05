//! Starea aplicației, ecranul de start și cadrul unui proiect deschis
//! (bara laterală cu pașii, antetul cu pașii și conținutul pasului curent).

use crate::db::{Db, IndexEntry};
use crate::i18n::{I18n, Lang};
use crate::network_ui::NetworkUi;
use crate::steps;
use crate::theme;
use crate::widgets::{self, pal};
use eframe::egui::{self, Align, CornerRadius, Layout, Margin, RichText, Stroke};
use hidro_core::model::ProjectData;
use hidro_core::pipeline::{self, ProjectResults};
use std::time::{Duration, Instant};

const SAVE_DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub lang: Lang,
    pub dark: bool,
    /// Ultimul șablon Word ales pentru Memoriul tehnic.
    pub docx_template: Option<String>,
    /// Ultimul șablon Excel ales pentru listele de cantități (fără el: registrul implicit).
    pub xlsx_template: Option<String>,
}

struct Toast {
    text: String,
    error: bool,
    since: Instant,
}

pub struct HidroApp {
    pub t: I18n,
    settings: Settings,
    db: Option<Db>,
    db_error: Option<String>,
    projects: Vec<IndexEntry>,
    pub project: Option<ProjectData>,
    /// Ultima variantă salvată, pentru a detecta modificările.
    saved: Option<ProjectData>,
    dirty_since: Option<Instant>,
    pub results: Option<ProjectResults>,
    delete_armed: Option<(String, Instant)>,
    toasts: Vec<Toast>,
    net_ui: NetworkUi,
    terrain: crate::terrain_sync::TerrainSync,
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn format_date(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|d| d.with_timezone(&chrono::Local).format("%d.%m.%Y, %H:%M").to_string())
        .unwrap_or_else(|_| iso.to_string())
}

impl HidroApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings: Settings = cc.storage.and_then(|s| eframe::get_value(s, "settings")).unwrap_or_default();
        Self::with_db(cc, settings, Db::open_default())
    }

    pub fn with_db(cc: &eframe::CreationContext<'_>, settings: Settings, db: Result<Db, String>) -> Self {
        theme::install(&cc.egui_ctx);
        cc.egui_ctx.set_theme(if settings.dark { egui::Theme::Dark } else { egui::Theme::Light });
        let (db, db_error) = match db {
            Ok(db) => (Some(db), None),
            Err(e) => (None, Some(e)),
        };
        let mut app = Self {
            t: I18n::new(settings.lang),
            settings,
            db,
            db_error,
            projects: Vec::new(),
            project: None,
            saved: None,
            dirty_since: None,
            results: None,
            delete_armed: None,
            toasts: Vec::new(),
            net_ui: NetworkUi::default(),
            terrain: Default::default(),
        };
        app.reload_list();
        app
    }

    fn toast(&mut self, text: impl Into<String>, error: bool) {
        self.toasts.push(Toast { text: text.into(), error, since: Instant::now() });
    }

    fn reload_list(&mut self) {
        if let Some(db) = &self.db {
            match db.list() {
                Ok(l) => self.projects = l,
                Err(e) => self.toast(e, true),
            }
        }
    }

    fn open(&mut self, p: ProjectData) {
        self.net_ui = NetworkUi::default();
        self.terrain = Default::default();
        self.saved = Some(p.clone());
        self.project = Some(p);
        self.dirty_since = None;
    }

    pub fn create_project(&mut self) {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let p = ProjectData::new(uuid::Uuid::new_v4().to_string(), now_iso(), today);
        if let Some(db) = &self.db {
            if let Err(e) = db.save(&p) {
                self.toast(e, true);
                return;
            }
        }
        self.open(p);
    }

    fn open_by_id(&mut self, id: &str) {
        let Some(db) = &self.db else { return };
        match db.load(id) {
            Ok(Some(p)) => self.open(p),
            Ok(None) => self.reload_list(),
            Err(e) => self.toast(e, true),
        }
    }

    fn delete(&mut self, id: &str) {
        if let Some(db) = &self.db {
            if let Err(e) = db.delete(id) {
                self.toast(e, true);
            }
        }
        self.reload_list();
    }

    /// Salvează imediat modificările în așteptare.
    pub fn flush(&mut self) {
        let (Some(db), Some(p)) = (&self.db, &mut self.project) else { return };
        if self.saved.as_ref() == Some(p) {
            self.dirty_since = None;
            return;
        }
        p.updated_at = now_iso();
        match db.save(p) {
            Ok(()) => {
                self.saved = Some(p.clone());
                self.dirty_since = None;
            }
            Err(e) => {
                self.dirty_since = None;
                self.toast(e, true);
            }
        }
    }

    #[cfg(test)]
    pub fn close_project_for_test(&mut self) {
        self.close_project();
    }

    fn close_project(&mut self) {
        self.flush();
        self.project = None;
        self.saved = None;
        self.results = None;
        self.reload_list();
    }

    fn import_json(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() else { return };
        let parsed = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).map_err(|e| e.to_string()));
        let value = match parsed {
            Ok(v) => v,
            Err(_) => return self.toast(self.t.t("home.invalidJson"), true),
        };
        if !ProjectData::looks_valid(&value) {
            return self.toast(self.t.t("home.invalidFile"), true);
        }
        let mut p: ProjectData = match serde_json::from_value(value) {
            Ok(p) => p,
            Err(_) => return self.toast(self.t.t("home.invalidFile"), true),
        };
        p.id = uuid::Uuid::new_v4().to_string();
        p.updated_at = now_iso();
        if let Some(db) = &self.db {
            if let Err(e) = db.save(&p) {
                return self.toast(e, true);
            }
        }
        self.open(p);
    }

    fn export_json(&mut self) {
        self.flush();
        let Some(p) = &self.project else { return };
        let name = if p.general.name.is_empty() { "proiect".to_string() } else { p.general.name.clone() };
        let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).set_file_name(format!("{name}.json")).save_file()
        else {
            return;
        };
        let res = serde_json::to_string_pretty(p).map_err(|e| e.to_string()).and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string()));
        match res {
            Ok(()) => self.toast(format!("{} ✓", path.display()), false),
            Err(e) => self.toast(e, true),
        }
    }

    fn set_settings(&mut self, ctx: &egui::Context, s: Settings) {
        self.t.lang = s.lang;
        self.settings = s;
        ctx.set_theme(if self.settings.dark { egui::Theme::Dark } else { egui::Theme::Light });
    }

    // ───────────────────────────── UI ─────────────────────────────

    fn lang_theme_switch(&mut self, ui: &mut egui::Ui) {
        let mut s = self.settings.clone();
        let p = pal(ui);
        for l in [Lang::Ro, Lang::En] {
            let sel = s.lang == l;
            let txt = RichText::new(l.code()).size(12.0).color(if sel { p.accent_fg } else { p.muted });
            if ui.add(egui::Button::new(txt).fill(if sel { p.accent } else { p.panel })).clicked() {
                s.lang = l;
            }
        }
        ui.add_space(6.0);
        let icon = if s.dark { "☀" } else { "🌙" };
        let tip = if s.dark { self.t.t("nav.switchToLight") } else { self.t.t("nav.switchToDark") };
        if ui.add(egui::Button::new(icon).fill(p.panel)).on_hover_text(tip).clicked() {
            s.dark = !s.dark;
        }
        if s != self.settings {
            self.set_settings(ui.ctx(), s);
        }
    }

    fn home(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        egui::Panel::top("home_hero")
            .frame(egui::Frame::new().fill(p.panel).inner_margin(Margin::symmetric(16, 28)).stroke(Stroke::new(1.0, p.border)))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("HidroCalc").size(30.0).strong().color(p.ink));
                    ui.label(RichText::new(self.t.t("home.subtitle")).color(p.muted));
                    ui.label(RichText::new(self.t.t("home.subtitle2")).size(12.5).color(p.muted));
                });
            });

        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.bg).inner_margin(Margin::symmetric(16, 24))).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                widgets::centered(ui, 760.0, |ui| {
                    if let Some(e) = &self.db_error {
                        widgets::alert(ui, &format!("{}: {e}", self.t.t("app.dbError")), true);
                        ui.add_space(8.0);
                    }
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(self.t.t("home.myProjects")).size(17.0).strong());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::primary_button(ui, &self.t.t("home.newProject")).clicked() {
                                self.create_project();
                            }
                            if widgets::secondary_button(ui, &self.t.t("home.importProject")).clicked() {
                                self.import_json();
                            }
                            ui.add_space(8.0);
                            self.lang_theme_switch(ui);
                        });
                    });
                    ui.add_space(12.0);

                    if self.projects.is_empty() {
                        widgets::card(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(18.0);
                                ui.label(RichText::new(self.t.t("home.noProjects")).color(p.muted));
                                ui.label(RichText::new(self.t.t("home.noProjectsHint")).size(12.5).color(p.muted));
                                ui.add_space(18.0);
                            });
                        });
                        return;
                    }

                    if self.delete_armed.as_ref().is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(3)) {
                        self.delete_armed = None;
                    }
                    let mut open = None;
                    let mut delete = None;
                    for e in &self.projects {
                        let resp = egui::Frame::new()
                            .fill(p.panel)
                            .stroke(Stroke::new(1.0, p.border))
                            .corner_radius(CornerRadius::same(12))
                            .inner_margin(Margin::symmetric(16, 12))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(&e.name).strong().color(p.ink));
                                        ui.label(
                                            RichText::new(format!("{} · {} {}", e.locality, self.t.t("home.updated"), format_date(&e.updated_at)))
                                                .size(12.0)
                                                .color(p.muted),
                                        );
                                    });
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let armed = self.delete_armed.as_ref().is_some_and(|(id, _)| id == &e.id);
                                        let label = if armed { self.t.t("home.deleteConfirm") } else { self.t.t("home.delete") };
                                        if widgets::danger_button(ui, &label, armed).clicked() {
                                            delete = Some((e.id.clone(), armed));
                                        }
                                    });
                                });
                            })
                            .response
                            .interact(egui::Sense::click());
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp.clicked() && delete.is_none() {
                            open = Some(e.id.clone());
                        }
                        ui.add_space(6.0);
                    }
                    if let Some((id, armed)) = delete {
                        if armed {
                            self.delete_armed = None;
                            self.delete(&id);
                        } else {
                            self.delete_armed = Some((id, Instant::now()));
                        }
                    } else if let Some(id) = open {
                        self.open_by_id(&id);
                    }
                });
            });
        });
        if self.delete_armed.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(500));
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let names = self.t.list("steps.names");
        let Some(project) = &mut self.project else { return };
        let done = project.step_completion();
        let mut close = false;
        let mut export = false;

        ui.add_space(10.0);
        if ui.add(egui::Button::new(RichText::new(format!("← {}", self.t.t("nav.projects"))).color(p.muted)).frame(false)).clicked() {
            close = true;
        }
        ui.add_space(4.0);
        let title = if project.general.name.is_empty() { self.t.t("common.newProject") } else { project.general.name.clone() };
        ui.add(egui::Label::new(RichText::new(title).size(16.0).strong().color(p.ink)).wrap());
        if !project.general.locality.is_empty() {
            ui.label(RichText::new(&project.general.locality).size(12.0).color(p.muted));
        }
        ui.add_space(6.0);
        if widgets::secondary_button(ui, &self.t.t("nav.exportJson")).clicked() {
            export = true;
        }
        ui.add_space(8.0);
        ui.separator();

        for (i, name) in names.iter().enumerate() {
            let step = i as u32 + 1;
            let active = project.current_step == step;
            let resp = egui::Frame::new()
                .fill(if active { p.accent_soft } else { egui::Color32::TRANSPARENT })
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (fg, bg) = if active || done[i] { (p.accent_fg, p.accent) } else { (p.ink2, p.panel2) };
                        egui::Frame::new().fill(bg).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(6, 1)).show(ui, |ui| {
                            ui.label(RichText::new(step.to_string()).size(11.0).strong().color(fg));
                        });
                        ui.label(RichText::new(name).size(13.0).color(if active { p.accent } else { p.ink2 }).strong());
                        if done[i] && !active {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new("✓").color(p.accent));
                            });
                        }
                    });
                })
                .response
                .interact(egui::Sense::click());
            if resp.clicked() {
                project.current_step = step;
            }
        }

        if let Some(r) = &self.results {
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(4.0);
            let row = |ui: &mut egui::Ui, label: &str, v: f64, unit: &str, dec: usize| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).size(12.0).color(p.ink2));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(unit).size(11.0).color(p.muted));
                        ui.label(RichText::new(widgets::fmt(v, dec)).monospace().size(12.0));
                    });
                });
            };
            row(ui, &self.t.t("sidebar.perspectiveInhabitants"), project.consumers.perspective_inhabitants, "loc.", 0);
            row(ui, "Q_s or max", r.water_demand.q_s_or_max_ls, "l/s", 3);
            row(ui, &self.t.t("sidebar.fireReserveV"), r.hydrants.v_ri, "m³", 3);
            row(ui, &self.t.t("sidebar.vTotal"), r.reservoir.v_total, "m³", 3);
            row(ui, "Q_dim", r.design_flows.q_dim_network, "m³/h", 3);
        }

        if export {
            self.export_json();
        }
        if close {
            self.close_project();
        }
    }

    fn stepper(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let names = self.t.list("steps.names");
        let Some(project) = &mut self.project else { return };
        let done = project.step_completion();
        egui::ScrollArea::horizontal().id_salt("stepper").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.spacing_mut().button_padding = egui::vec2(8.0, 5.0);
                for (i, name) in names.iter().enumerate() {
                    let step = i as u32 + 1;
                    let active = project.current_step == step;
                    let (fg, fill, stroke) = if active {
                        (p.accent_fg, p.accent, p.accent)
                    } else if done[i] {
                        (p.accent, p.accent_soft, p.accent_ring)
                    } else {
                        (p.muted, egui::Color32::TRANSPARENT, egui::Color32::TRANSPARENT)
                    };
                    let txt = RichText::new(format!("{step} {name}")).size(12.0).color(fg).strong();
                    if ui.add(egui::Button::new(txt).fill(fill).stroke(Stroke::new(1.0, stroke))).clicked() {
                        project.current_step = step;
                    }
                }
            });
        });
    }

    fn project_view(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        // Calcule: populația de perspectivă + lanțul complet (microsecunde, deci la fiecare cadru).
        if let Some(project) = &mut self.project {
            let n = pipeline::perspective_inhabitants(project);
            if n.is_finite() && n != project.consumers.perspective_inhabitants {
                project.consumers.perspective_inhabitants = n;
            }
            self.results = Some(pipeline::compute(project));
            // Calculul hidraulic al rețelei, live (idempotent, deci se scrie doar la schimbare).
            match pipeline::compute_network(project) {
                Ok(secs) => {
                    if secs != project.network_sections {
                        project.network_sections = secs;
                    }
                    self.net_ui.error = None;
                }
                Err(e) => self.net_ui.error = Some(e),
            }
        }

        // Ecran complet (pasul 8): fără bara laterală și fără pași, zona de desen ia tot spațiul.
        let expanded = self.net_ui.expanded && self.project.as_ref().is_some_and(|p| p.current_step == 8);
        if !expanded {
        egui::Panel::left("sidebar")
            .exact_size(270.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(p.panel).inner_margin(Margin::symmetric(12, 4)).stroke(Stroke::new(1.0, p.border)))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| self.sidebar(ui));
            });
        if self.project.is_none() {
            return;
        }
        egui::Panel::top("stepper")
            .frame(egui::Frame::new().fill(p.panel).inner_margin(Margin::symmetric(12, 8)).stroke(Stroke::new(1.0, p.border)))
            .show(ui, |ui| self.stepper(ui));
        }

        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.bg).inner_margin(Margin::symmetric(20, 18))).show(ui, |ui| {
            let step = self.project.as_ref().map_or(1, |p| p.current_step);
            egui::ScrollArea::vertical().id_salt(("step", step)).auto_shrink(false).show(ui, |ui| {
                let (Some(project), Some(results)) = (&mut self.project, &self.results) else { return };
                if let Some(go) = steps::show(ui, &self.t, project, results, &mut self.net_ui) {
                    project.current_step = go;
                }
            });
        });
    }

    /// Exporturile cerute din pași (DXF, XLSX): fișierul se alege cu dialogul nativ.
    /// Alege un șablon (.docx / .xlsx) și îl ține minte; `None` dacă s-a anulat.
    fn pick_template(&mut self, word: bool) -> Option<String> {
        let (name, ext, title) = if word { ("Word", "docx", "export.doc.pickDocx") } else { ("Excel", "xlsx", "export.doc.pickXlsx") };
        let path = rfd::FileDialog::new().set_title(self.t.t(title)).add_filter(name, &[ext]).pick_file()?;
        let p = path.to_string_lossy().into_owned();
        if word {
            self.settings.docx_template = Some(p.clone());
        } else {
            self.settings.xlsx_template = Some(p.clone());
        }
        Some(p)
    }

    fn template_bytes(&mut self, word: bool) -> Option<Result<Vec<u8>, String>> {
        let path = if word { self.settings.docx_template.clone() } else { self.settings.xlsx_template.clone() };
        let path = match path.filter(|p| std::path::Path::new(p).exists()) {
            Some(p) => p,
            None if word => self.pick_template(true)?,
            None => return None,
        };
        Some(std::fs::read(&path).map_err(|e| format!("{path}: {e}")))
    }

    fn run_export(&mut self, kind: crate::network_ui::ExportKind) {
        use crate::network_ui::ExportKind;
        if let ExportKind::PickTemplate(word) = kind {
            if self.pick_template(word).is_some() {
                self.run_export(if word { ExportKind::Memoriu } else { ExportKind::Xlsx });
            }
            return;
        }
        // Șablonul se citește înainte de a împrumuta proiectul.
        let template = match kind {
            ExportKind::Memoriu => match self.template_bytes(true) {
                Some(Ok(b)) => Some(b),
                Some(Err(e)) => return self.toast(format!("{} {e}", self.t.t("docs.exportFailed")), true),
                None => return,
            },
            ExportKind::Xlsx => match self.template_bytes(false) {
                Some(Ok(b)) => Some(b),
                Some(Err(e)) => return self.toast(format!("{} {e}", self.t.t("docs.exportFailed")), true),
                None => None,
            },
            _ => None,
        };
        let mut missing_vars: std::collections::BTreeSet<String> = Default::default();
        use hidro_core::export::{self, breviar, document, docx, dxf, report, xlsx, xlsx_template};
        let Some(p) = &self.project else { return };
        let base = export::safe_file_name(&p.general.name, "proiect");
        let t = &self.t;
        let tr = |k: &str, args: &[(&str, &str)]| t.tf(k, args);
        type Prepared<'a> = (Result<Vec<u8>, String>, String, (&'a str, &'a str), String);
        let (bytes, file, filter, label): Prepared = match kind {
            ExportKind::Dxf => match dxf::export(&p.network_schema, crate::canvas::scale_ratio(p)) {
                Some(s) => (Ok(s.into_bytes()), format!("{base}.dxf"), ("DXF", "dxf"), "DXF".into()),
                None => return self.toast(self.t.t("canvas.exportEmptySchema"), true),
            },
            ExportKind::Pdf | ExportKind::Breviar => {
                let Some(res) = &self.results else { return };
                if kind == ExportKind::Breviar {
                    (breviar::build(p, res, &tr), format!("{base} - Breviar de calcul.pdf"), ("PDF", "pdf"), self.t.t("export.doc.breviar"))
                } else {
                    let map_line = crate::map_ui::anchor(p).map(|a| {
                        let mut parts = vec!["Hartă OSM".to_string()];
                        if let Some(info) = p.network_canvas_settings.get("mapLocationInfo") {
                            for (k, pre) in [("locality", ""), ("county", "Jud. "), ("country", "")] {
                                if let Some(v) = info.get(k).and_then(serde_json::Value::as_str).filter(|s| !s.is_empty()) {
                                    parts.push(format!("{pre}{v}"));
                                }
                            }
                        }
                        parts.push(format!("{:.6}°N  {:.6}°E", a.lat, a.lng));
                        parts.push(format!("Scară 1:{}", crate::canvas::scale_ratio(p)));
                        parts.join("  ·  ")
                    });
                    (report::build(p, res, &tr, map_line, crate::canvas::scale_ratio(p)), format!("{base}.pdf"), ("PDF", "pdf"), "PDF".into())
                }
            }
            ExportKind::Xlsx | ExportKind::Memoriu => {
                let Some(res) = &self.results else { return };
                let today = chrono::Local::now().format("%d.%m.%Y").to_string();
                let data = document::build(p, res, &p.network_sections, &today, &tr);
                let render = |r: Result<(Vec<u8>, std::collections::BTreeSet<String>), String>, missing: &mut std::collections::BTreeSet<String>| {
                    r.map(|(b, m)| {
                        *missing = m;
                        b
                    })
                };
                if kind == ExportKind::Memoriu {
                    let bytes = render(docx::render(template.as_deref().unwrap_or_default(), &data), &mut missing_vars)
                        .map_err(|e| format!("{} {e}", self.t.t("export.doc.templateError")));
                    (bytes, format!("{base} - Memoriu tehnic.docx"), ("Word", "docx"), self.t.t("export.doc.memoriu"))
                } else {
                    let bytes = match &template {
                        Some(tpl) => render(xlsx_template::render(tpl, &data), &mut missing_vars),
                        None => xlsx::default_workbook(&data, &tr).map_err(|e| e.to_string()),
                    };
                    (bytes, format!("{base} - Liste cantitati.xlsx"), ("Excel", "xlsx"), self.t.t("export.doc.cantitati"))
                }
            }
            ExportKind::PickTemplate(_) => return,
        };
        let bytes = match bytes {
            Ok(b) => b,
            Err(e) => return self.toast(format!("{} {e}", self.t.t("docs.exportFailed")), true),
        };
        if !missing_vars.is_empty() {
            let vars: Vec<String> = missing_vars.iter().take(8).cloned().collect();
            self.toast(self.t.tf("export.doc.missingVars", &[("vars", &vars.join(", "))]), false);
        }
        let Some(path) = rfd::FileDialog::new().add_filter(filter.0, &[filter.1]).set_file_name(&file).save_file() else { return };
        match std::fs::write(&path, bytes) {
            Ok(()) => self.toast(format!("{label} ✓  {}", path.display()), false),
            Err(e) => self.toast(format!("{} {e}", self.t.t("docs.exportFailed")), true),
        }
    }

    fn toasts_ui(&mut self, ctx: &egui::Context) {
        self.toasts.retain(|t| t.since.elapsed() < Duration::from_secs(5));
        if self.toasts.is_empty() {
            return;
        }
        let p = theme::palette(ctx);
        egui::Area::new(egui::Id::unique("toasts")).anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0)).show(ctx, |ui| {
            for t in &self.toasts {
                let fg = if t.error { p.danger } else { p.ink };
                egui::Frame::new()
                    .fill(p.panel)
                    .stroke(Stroke::new(1.0, if t.error { p.danger } else { p.border_strong }))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_max_width(380.0);
                        ui.add(egui::Label::new(RichText::new(&t.text).color(fg)).wrap());
                    });
                ui.add_space(6.0);
            }
        });
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

impl eframe::App for HidroApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.project.is_some() {
            self.project_view(ui);
        } else {
            self.home(ui);
        }
        if let Some(project) = &mut self.project {
            if let Some(err) = self.terrain.tick(ui.ctx(), project) {
                self.toast(format!("{}: {err}", self.t.t("terrain.fetchFailed")), true);
            }
        }
        if let Some((msg, err)) = self.net_ui.canvas.notice.take() {
            self.toast(msg, err);
        }
        if let Some(kind) = self.net_ui.export.take() {
            self.run_export(kind);
        }
        let ctx = ui.ctx().clone();
        self.toasts_ui(&ctx);

        // Salvare cu debounce (ca `saveProject()` din store-ul React).
        if self.project.is_some() && self.project != self.saved {
            let since = *self.dirty_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= SAVE_DEBOUNCE {
                self.flush();
            } else {
                ctx.request_repaint_after(SAVE_DEBOUNCE);
            }
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "settings", &self.settings);
    }

    fn on_exit(&mut self) {
        self.flush();
    }
}

/// Randări fără fereastră ale ecranelor principale (verificare vizuală).
/// `cargo test -p hidrocalc snapshots -- --nocapture` → PNG-uri în `target/ui-snapshots/`.
#[cfg(test)]
mod snapshots {
    use super::*;
    use hidro_core::model::PublicConsumer;

    pub(super) fn sample(app: &mut HidroApp) {
        app.create_project();
        let p = app.project.as_mut().unwrap();
        p.general.name = "Alimentare cu apă sat Ștefănești".into();
        p.general.locality = "Ștefănești".into();
        p.general.county = "Timiș".into();
        p.consumers.inhabitants = 1850.0;
        p.consumers.public_consumers.push(PublicConsumer {
            id: "pc1".into(),
            kind: "Școală gimnazială".into(),
            quantity: 1.0,
            num_consumers: 240.0,
            unit_type: "elevi".into(),
            q_specific: 50.0,
            ..Default::default()
        });
        p.reservoir.cota_teren = 102.5;
        p.reservoir.hb = 14.0;
        p.reservoir.i_med = 0.003;
        p.reservoir.l = 4200.0;
        p.reservoir.coef_debit_min = 0.3;
        p.reservoir.t_av = 12.0;
        p.reservoir.ti = 2.0;
        // Rețea: R → Tr1 → (hidrant) → Tr2 ; Tr1 → Tr3 ; Tr3 → Tr4
        use hidro_core::model::{SchemaElement, SchemaKind};
        let tr = |id: &str, label: &str, a: (f64, f64), b: (f64, f64)| SchemaElement {
            id: id.into(),
            kind: SchemaKind::Tronson,
            x: a.0,
            y: a.1,
            x2: Some(b.0),
            y2: Some(b.1),
            label: Some(label.into()),
            ..Default::default()
        };
        p.network_schema = vec![
            SchemaElement { id: "rez".into(), kind: SchemaKind::Rezervor, x: -44.0, y: 0.0, ..Default::default() },
            tr("a", "Tr1", (0.0, 0.0), (1200.0, 0.0)),
            SchemaElement { id: "h".into(), kind: SchemaKind::Hidrant, x: 1207.0, y: 0.0, ..Default::default() },
            tr("b", "Tr2", (1214.0, 0.0), (2000.0, 0.0)),
            tr("c", "Tr3", (1203.0, 2.0), (1203.0, 900.0)),
            tr("d", "Tr4", (1203.0, 900.0), (1900.0, 900.0)),
        ];
        let mut n = 0;
        p.network_sections = hidro_core::schema::build_sections_from_schema(&p.network_schema, &[], 1000.0, || {
            n += 1;
            format!("s{n}")
        });
        for (i, s) in p.network_sections.iter_mut().enumerate() {
            s.q_linear = 0.004 + 0.002 * i as f64;
            s.zeta = 2.0;
        }
        p.network_sections[1].q_extern = 1.2;
        for (s, (up, dn)) in p.network_sections.iter_mut().zip([(102.5, 99.8), (99.8, 97.2), (99.8, 98.9), (98.9, 98.4)]) {
            s.cota_up = Some(up);
            s.cota_down = Some(dn);
            s.cota_up_source = Some("manual".into());
            s.cota_down_source = Some("manual".into());
        }
    }

    #[test]
    fn render_main_screens() {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
        std::fs::create_dir_all(&out).unwrap();
        let dir = std::env::temp_dir().join(format!("hidrocalc-snap-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        for dark in [false, true] {
            let db_path = dir.join(format!("s{dark}.db"));
            let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1360.0, 880.0)).with_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light }).wgpu().build_eframe(|cc| {
                HidroApp::with_db(cc, Settings { lang: Lang::Ro, dark, ..Default::default() }, Db::open(&db_path))
            });
            let suffix = if dark { "dark" } else { "light" };
            h.run_steps(3);
            h.render().unwrap().save(out.join(format!("home-empty-{suffix}.png"))).unwrap();
            sample(h.state_mut());
            for step in 1..=9 {
                h.state_mut().project.as_mut().unwrap().current_step = step;
                if step == 8 {
                    h.state_mut().net_ui.selected = Some("s3".into());
                }
                h.run_steps(3);
                h.render().unwrap().save(out.join(format!("step{step}-{suffix}.png"))).unwrap();
            }
            // Vederea 3D (fără teren: proiectul de test nu are hartă ancorată)
            {
                let st = h.state_mut();
                st.project.as_mut().unwrap().current_step = 8;
                st.net_ui.show_3d = true;
            }
            h.run_steps(3);
            h.render().unwrap().save(out.join(format!("step8-3d-{suffix}.png"))).unwrap();
            h.state_mut().net_ui.show_3d = false;
            // Ecran complet: fără bara laterală și pași
            h.state_mut().net_ui.expanded = true;
            h.run_steps(3);
            h.render().unwrap().save(out.join(format!("step8-fullscreen-{suffix}.png"))).unwrap();
            h.state_mut().net_ui.expanded = false;
            h.state_mut().close_project_for_test();
            h.run_steps(3);
            h.render().unwrap().save(out.join(format!("home-list-{suffix}.png"))).unwrap();
        }
        println!("snapshots: {}", out.canonicalize().unwrap().display());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// PDF-urile cu traducerile reale, pentru verificare vizuală:
    /// `HIDRO_PDF_OUT=<dir> cargo test -p hidrocalc pdf_exports`.
    #[test]
    fn pdf_exports() {
        let Ok(out) = std::env::var("HIDRO_PDF_OUT") else { return };
        let dir = std::env::temp_dir().join(format!("hidrocalc-pdf-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("p.db");
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1360.0, 880.0)).build_eframe(|cc| {
            HidroApp::with_db(cc, Settings::default(), Db::open(&db_path))
        });
        sample(h.state_mut());
        h.state_mut().project.as_mut().unwrap().current_step = 9;
        h.run_steps(2);
        let app = h.state();
        let p = app.project.as_ref().unwrap();
        let r = app.results.as_ref().unwrap();
        let tr = |k: &str, a: &[(&str, &str)]| app.t.tf(k, a);
        let rep = hidro_core::export::report::build(p, r, &tr, Some("Hartă OSM · Ștefănești".into()), 1000.0).unwrap();
        let brev = hidro_core::export::breviar::build(p, r, &tr).unwrap();
        std::fs::write(format!("{out}/raport.pdf"), rep).unwrap();
        std::fs::write(format!("{out}/breviar.pdf"), brev).unwrap();
        let data = hidro_core::export::document::build(p, r, &p.network_sections, "05.11.2026", &tr);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sabloane");
        let (docx, miss_d) = hidro_core::export::docx::render(&std::fs::read(root.join("exemplu-memoriu-tehnic.docx")).unwrap(), &data).unwrap();
        let (xlsx, miss_x) = hidro_core::export::xlsx_template::render(&std::fs::read(root.join("exemplu-liste-cantitati.xlsx")).unwrap(), &data).unwrap();
        assert!(miss_d.is_empty() && miss_x.is_empty(), "{miss_d:?} {miss_x:?}");
        std::fs::write(format!("{out}/memoriu.docx"), docx).unwrap();
        std::fs::write(format!("{out}/cantitati.xlsx"), xlsx).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Necesită internet (dale OSM reale): `cargo test -p hidrocalc map_snapshot -- --ignored`.
    #[test]
    #[ignore]
    fn map_snapshot() {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
        std::fs::create_dir_all(&out).unwrap();
        let dir = std::env::temp_dir().join(format!("hidrocalc-map-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("m.db");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 880.0))
            .with_theme(egui::Theme::Light)
            .wgpu()
            .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
        sample(h.state_mut());
        {
            let p = h.state_mut().project.as_mut().unwrap();
            // Ancoră în Timișoara (Piața Victoriei), la capătul amonte al lui Tr1.
            let a = hidro_core::geo::MapAnchor {
                lat: 45.7537,
                lng: 21.2257,
                world_x: 0.0,
                world_y: 0.0,
                tile_zoom: hidro_core::geo::compute_tile_zoom(45.7537, 1000.0),
            };
            let o = p.network_canvas_settings.as_object_mut().unwrap();
            o.insert("mapAnchor".into(), serde_json::to_value(a).unwrap());
            o.insert("autoTerrainCota".into(), serde_json::json!(false));
            p.current_step = 8;
        }
        for _ in 0..80 {
            h.run_steps(2);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        h.render().unwrap().save(out.join("step8-map.png")).unwrap();
        // 3D cu terenul DEM real
        h.state_mut().net_ui.show_3d = true;
        for _ in 0..100 {
            h.run_steps(2);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        h.render().unwrap().save(out.join("step8-3d-terrain.png")).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }
}

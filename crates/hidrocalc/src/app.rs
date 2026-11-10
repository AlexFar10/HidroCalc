//! Starea aplicației, ecranul de start și cadrul unui proiect deschis
//! (bara laterală cu pașii, antetul cu pașii și conținutul pasului curent).

use crate::db::{Db, IndexEntry};
use crate::i18n::{I18n, Lang};
use crate::network_ui::{ExportId, NetworkUi};
use crate::steps;
use crate::theme;
use crate::widgets::{self, pal};
use eframe::egui::{self, Align, CornerRadius, Layout, Margin, RichText, Stroke};
use hidro_core::model::ProjectData;
use hidro_core::pipeline::{self, ProjectResults};
use std::time::{Duration, Instant};

/// Scurtături globale de tastatură (⌘ pe macOS, Ctrl în rest).
const SC_SIDEBAR: egui::KeyboardShortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::B);
const SC_PREV: egui::KeyboardShortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::OpenBracket);
const SC_NEXT: egui::KeyboardShortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::CloseBracket);
const SC_HELP: egui::KeyboardShortcut = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F1);
const NUM_KEYS: [egui::Key; 9] = [
    egui::Key::Num1,
    egui::Key::Num2,
    egui::Key::Num3,
    egui::Key::Num4,
    egui::Key::Num5,
    egui::Key::Num6,
    egui::Key::Num7,
    egui::Key::Num8,
    egui::Key::Num9,
];

/// Textul unei scurtături pentru afișare („Ctrl+]”, „⌘B”, „F1”), cu parantezele drepte scrise ca atare.
fn shortcut_text(ctx: &egui::Context, sc: &egui::KeyboardShortcut) -> String {
    let key = match sc.logical_key {
        egui::Key::OpenBracket => "[".to_string(),
        egui::Key::CloseBracket => "]".to_string(),
        k => k.name().to_string(),
    };
    if sc.modifiers.is_none() {
        key
    } else {
        format!("{}+{key}", ctx.format_modifiers(sc.modifiers))
    }
}

/// Comutatorul de limbă, ca cel de temă: un buton care arată limba la care treci (EN când ești pe RO).
fn lang_switch(ui: &mut egui::Ui, s: &mut Settings) {
    let (target, tip) = if s.lang == Lang::Ro { ("EN", "Switch to English") } else { ("RO", "Treci la română") };
    if widgets::icon_button(ui, tip, |p, r, col, _bg| {
        p.text(r.center(), egui::Align2::CENTER_CENTER, target, egui::FontId::proportional(13.0), col);
    })
    .clicked()
    {
        s.lang = if s.lang == Lang::Ro { Lang::En } else { Lang::Ro };
    }
}

/// Acțiuni din panoul de proiect al barei laterale, executate după desenare.
enum Act {
    Home,
    New,
    Import,
    Open(String),
}

/// Înălțimea benzii de sus (logo + bara de meniuri); aceeași în bara laterală și în cea de sus.
const BAR_H: f32 = 56.0;
/// Lățimea barei laterale.
const SIDEBAR_W: f32 = 270.0;
const SAVE_DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub lang: Lang,
    pub dark: bool,
    /// Bara laterală e ascunsă.
    pub sidebar_closed: bool,
    /// Contrast ridicat (paleta AAA).
    pub high_contrast: bool,
    /// Fără animații.
    pub reduce_motion: bool,
    /// Dimensiunea interfeței în procente (0 = implicit 100%).
    pub ui_scale_pct: u16,
}

/// O captură de imagine în curs (planul 2D sau vederea 3D).
struct ExportCap {
    id: ExportId,
    frames: u32,
    requested: bool,
    started: Instant,
}

/// Un export în desfășurare: coada, rezultatele și locul de revenire (pas, mod 2D/3D).
struct ExportJob {
    /// Folderul ales (export multiplu); `None` = dialog „Salvează ca” pentru fiecare fișier.
    folder: Option<std::path::PathBuf>,
    queue: std::collections::VecDeque<ExportId>,
    ok: Vec<std::path::PathBuf>,
    errs: Vec<String>,
    cap: Option<ExportCap>,
    restore: (u32, bool),
}

struct Toast {
    text: String,
    error: bool,
    since: Instant,
    closed: bool,
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
    export_job: Option<ExportJob>,
    show_help: bool,
    /// Cerere de a muta focusul de la tastatură pe conținut („Sari la conținut”).
    focus_content: bool,
    logo: Option<egui::TextureHandle>,
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
        theme::set_high_contrast(&cc.egui_ctx, settings.high_contrast);
        theme::set_reduce_motion(&cc.egui_ctx, settings.reduce_motion);
        if settings.ui_scale_pct > 0 {
            cc.egui_ctx.set_zoom_factor(settings.ui_scale_pct as f32 / 100.0);
        }
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
            export_job: None,
            show_help: false,
            focus_content: false,
            logo: None,
        };
        app.reload_list();
        app
    }

    fn toast(&mut self, text: impl Into<String>, error: bool) {
        self.toasts.push(Toast { text: text.into(), error, since: Instant::now(), closed: false });
    }

    fn reload_list(&mut self) {
        if let Some(db) = &self.db {
            match db.list() {
                Ok(l) => self.projects = l,
                Err(e) => self.toast(e, true),
            }
        }
    }

    fn open(&mut self, mut p: ProjectData) {
        p.current_step = 1;
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
        let mut reload = false;
        match db.save(p) {
            Ok(()) => {
                self.saved = Some(p.clone());
                self.dirty_since = None;
                reload = true;
            }
            Err(e) => {
                self.dirty_since = None;
                self.toast(e, true);
            }
        }
        if reload {
            self.reload_list();
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
        let mut p = match ProjectData::from_export_value(value) {
            Ok(p) => p,
            Err(hidro_core::model::ExportImportError::NewerFormat(_)) => return self.toast(self.t.t("home.newerFormat"), true),
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

    /// Exportul proiectului ca JSON (schema desenată, setările hărții, punctele topo), cu
    /// metadate de format. Fără buton deocamdată: urmează să fie legat de interfață.
    #[allow(dead_code)]
    fn export_json(&mut self) {
        self.flush();
        let Some(p) = &self.project else { return };
        let name = if p.general.name.is_empty() { "proiect".to_string() } else { p.general.name.clone() };
        let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).set_file_name(format!("{name}.json")).save_file()
        else {
            return;
        };
        let doc = p.to_export_json(env!("CARGO_PKG_VERSION"), &now_iso());
        let res = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string()).and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string()));
        match res {
            Ok(()) => self.toast(format!("{} ✓", path.display()), false),
            Err(e) => self.toast(e, true),
        }
    }

    fn set_settings(&mut self, ctx: &egui::Context, s: Settings) {
        self.t.lang = s.lang;
        let old = std::mem::replace(&mut self.settings, s);
        if old.high_contrast != self.settings.high_contrast {
            theme::set_high_contrast(ctx, self.settings.high_contrast);
            ctx.request_discard("contrast schimbat");
        }
        if old.reduce_motion != self.settings.reduce_motion {
            theme::set_reduce_motion(ctx, self.settings.reduce_motion);
        }
        if old.ui_scale_pct != self.settings.ui_scale_pct {
            ctx.set_zoom_factor(if self.settings.ui_scale_pct == 0 { 1.0 } else { self.settings.ui_scale_pct as f32 / 100.0 });
        }
        let theme = if self.settings.dark { egui::Theme::Dark } else { egui::Theme::Light };
        if ctx.theme() != theme {
            ctx.set_theme(theme);
            // Refă cadrul cu tema nouă, ca nimic din el să nu rămână în culorile vechi.
            ctx.request_discard("tema schimbată");
        }
    }

    // ───────────────────────────── UI ─────────────────────────────

    /// Picătura (iconița aplicației) ca textură, încărcată o singură dată.
    fn logo_id(&mut self, ctx: &egui::Context) -> egui::TextureId {
        if self.logo.is_none() {
            let img = image::load_from_memory_with_format(include_bytes!("../../../assets/icons/icon.png"), image::ImageFormat::Png)
                .map(|i| i.to_rgba8())
                .unwrap_or_else(|_| image::RgbaImage::new(1, 1));
            let size = [img.width() as usize, img.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
            self.logo = Some(ctx.load_texture("logo", color, egui::TextureOptions::LINEAR));
        }
        self.logo.as_ref().map(|t| t.id()).unwrap_or_default()
    }

    /// Bara de sus: meniul Proiect, pașii și navigarea Înapoi / Următorul; pe ecranul de
    /// start, în loc de pași, limba, tema și proiect nou/import. Întoarce dreptunghiul ocupat.
    fn app_bar(&mut self, ui: &mut egui::Ui, with_logo: bool) -> egui::Rect {
        let p = pal(ui);
        let logo = self.logo_id(ui.ctx());
        let mut s = self.settings.clone();
        let (mut new, mut import) = (false, false);
        let mut go: Option<u32> = None;
        let mut toggle_sidebar = false;
        let mut go_home = false;
        let mut skip = false;
        let closed = !self.sidebar_visible(ui.ctx());
        let has_project = self.project.is_some();
        let step = self.project.as_ref().map_or(0, |p| p.current_step);
        let names = self.t.list("steps.names");
        let resp = egui::Panel::top("appbar").frame(egui::Frame::new().fill(p.panel).inner_margin(Margin::symmetric(16, 0))).show(ui, |ui| {
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), BAR_H), Layout::left_to_right(Align::Center), |ui| {
                ui.set_min_height(BAR_H);
                ui.spacing_mut().button_padding = egui::vec2(10.0, 5.0);
                ui.spacing_mut().interact_size.y = 28.0;
                if has_project {
                    // „Sari la conținut”: primul element la Tab; devine vizibil doar când are focus.
                    let r = ui.interact(egui::Rect::from_min_size(ui.cursor().min, egui::vec2(1.0, 1.0)), egui::Id::NULL.with("skip_link"), egui::Sense::click());
                    widgets::describe(&r, egui::Role::Button, &self.t.t("a11y.skip"));
                    if r.has_focus() {
                        egui::Area::new(egui::Id::NULL.with("skip_link_area")).order(egui::Order::Foreground).fixed_pos(egui::pos2(12.0, 14.0)).show(ui.ctx(), |ui| {
                            egui::Frame::new().fill(p.accent).corner_radius(CornerRadius::same(8)).inner_margin(Margin::symmetric(12, 6)).show(ui, |ui| {
                                ui.label(RichText::new(self.t.t("a11y.skip")).color(p.accent_fg));
                            });
                        });
                    }
                    if r.clicked() {
                        skip = true;
                    }
                }
                if has_project && !closed {
                    // Logo + titlu în colțul stânga-sus, deasupra barei laterale; click = pagina de start.
                    let head = ui.allocate_ui_with_layout(egui::vec2(SIDEBAR_W - 16.0 - 8.0, BAR_H), Layout::left_to_right(Align::Center), |ui| {
                        ui.set_min_size(egui::vec2(SIDEBAR_W - 16.0 - 8.0, BAR_H));
                        ui.add(egui::Image::new((logo, egui::vec2(24.0, 24.0))));
                        widgets::wordmark(ui, 18.0);
                    });
                    let click = ui.interact(head.response.rect, egui::Id::NULL.with("logo_home"), egui::Sense::click());
                    widgets::describe(&click, egui::Role::Button, &self.t.t("nav.home"));
                    if click.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if click.clicked() {
                        go_home = true;
                    }
                    ui.add_space(8.0);
                }
                if has_project {
                    let tip = format!("{} ({})", self.t.t(if closed { "nav.showSidebar" } else { "nav.hideSidebar" }), shortcut_text(ui.ctx(), &SC_SIDEBAR));
                    if widgets::icon_button(ui, &tip, |p, r, col, _bg| {
                        let body = r.shrink2(egui::vec2(9.0, 8.0));
                        p.rect_stroke(body, 2.0, Stroke::new(1.4, col), egui::StrokeKind::Middle);
                        let split = body.left() + body.width() * 0.38;
                        p.line_segment([egui::pos2(split, body.top()), egui::pos2(split, body.bottom())], Stroke::new(1.4, col));
                        if !closed {
                            p.rect_filled(egui::Rect::from_min_max(body.min + egui::vec2(1.0, 1.0), egui::pos2(split, body.max.y - 1.0)), 0.0, col.gamma_multiply(0.5));
                        }
                    })
                    .clicked()
                    {
                        toggle_sidebar = true;
                    }
                    ui.add_space(8.0);
                }
                if with_logo {
                    ui.add(egui::Image::new((logo, egui::vec2(24.0, 24.0))));
                    widgets::wordmark(ui, 18.0);
                    ui.add_space(14.0);
                }
                if has_project {
                    // Înapoi · pașii · Următorul: săgețile încadrează lista.
                    let arrow = |ui: &mut egui::Ui, text: &str, tip: String, enabled: bool| -> bool {
                        ui.add_enabled_ui(enabled, |ui| {
                            ui.add_sized(
                                [30.0, 28.0],
                                egui::Button::new(RichText::new(text).size(14.0).color(p.ink)).fill(p.panel2).stroke(Stroke::new(1.0, p.border)).corner_radius(egui::CornerRadius::same(8)),
                            )
                            .on_hover_text(tip)
                            .clicked()
                        })
                        .inner
                    };
                    if arrow(ui, "←", format!("{} ({})", self.t.t("common.back").trim_start_matches("← "), shortcut_text(ui.ctx(), &SC_PREV)), step > 1) {
                        go = Some(step - 1);
                    }
                    ui.add_space(6.0);
                    let steps_w = (ui.available_width() - 50.0).max(120.0);
                    egui::ScrollArea::horizontal()
                        .id_salt("steps_bar")
                        .max_width(steps_w)
                        .auto_shrink([true, true])
                        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                ui.spacing_mut().button_padding = egui::vec2(9.0, 5.0);
                                for (i, name) in names.iter().enumerate() {
                                    let n = i as u32 + 1;
                                    if widgets::tool_button(ui, name, n == step).clicked() {
                                        go = Some(n);
                                    }
                                }
                            });
                        });
                    ui.add_space(6.0);
                    if arrow(ui, "→", format!("{} ({})", self.t.t("nav.stepNext").trim_end_matches(" →"), shortcut_text(ui.ctx(), &SC_NEXT)), step < 9) {
                        go = Some(step + 1);
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !has_project {
                        let tip = if s.dark { self.t.t("nav.switchToLight") } else { self.t.t("nav.switchToDark") };
                        let dark = s.dark;
                        if widgets::icon_button(ui, &tip, |pt, r, col, bg| widgets::paint_theme_icon(pt, r, col, bg, dark)).clicked() {
                            s.dark = !s.dark;
                        }
                        lang_switch(ui, &mut s);
                        ui.add_space(8.0);
                        if widgets::primary_button(ui, &self.t.t("home.newProject")).clicked() {
                            new = true;
                        }
                        if widgets::secondary_button(ui, &self.t.t("home.importProject")).clicked() {
                            import = true;
                        }
                    }
                });
            });
        });
        let r = resp.response.rect;
        if let (Some(g), Some(project)) = (go, &mut self.project) {
            project.current_step = g;
        }
        if toggle_sidebar {
            self.toggle_sidebar(ui.ctx());
        }
        if s != self.settings {
            self.set_settings(ui.ctx(), s);
        }
        if new {
            self.flush();
            self.create_project();
        } else if import {
            self.flush();
            self.import_json();
        }
        r
    }

    /// Bara laterală e vizibilă: pe ferestre înguste doar dacă a fost deschisă manual,
    /// altfel după preferința din setări.
    fn sidebar_visible(&self, _ctx: &egui::Context) -> bool {
        !self.settings.sidebar_closed
    }

    fn toggle_sidebar(&mut self, ctx: &egui::Context) {
        let mut s = self.settings.clone();
        s.sidebar_closed = !s.sidebar_closed;
        self.set_settings(ctx, s);
    }

    /// Scurtături globale: pași, bara laterală, ajutor. Se consumă înainte de desenare.
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_shortcut(&SC_HELP)) {
            self.show_help = !self.show_help;
        }
        if self.project.is_none() {
            return;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_SIDEBAR)) {
            self.toggle_sidebar(ctx);
        }
        let mut go: Option<u32> = None;
        let step = self.project.as_ref().map_or(1, |p| p.current_step);
        if ctx.input_mut(|i| i.consume_shortcut(&SC_PREV)) && step > 1 {
            go = Some(step - 1);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_NEXT)) && step < 9 {
            go = Some(step + 1);
        }
        for (n, key) in NUM_KEYS.iter().enumerate() {
            if ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, *key))) {
                go = Some(n as u32 + 1);
            }
        }
        if let (Some(g), Some(p)) = (go, &mut self.project) {
            p.current_step = g;
        }
    }

    /// Contur de focus pentru elementul activ de la tastatură (butoane, bife, liste, desenul).
    /// Se desenează deasupra, deci apare și la widgeturile cu stil propriu.
    fn paint_focus_ring(&self, ctx: &egui::Context) {
        let Some(id) = ctx.memory(|m| m.focused()) else { return };
        let Some(r) = ctx.read_response(id) else { return };
        if !r.has_focus() || !r.rect.is_positive() {
            return;
        }
        let p = theme::palette(ctx);
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::NULL.with("focus_ring")))
            .rect_stroke(r.rect.expand(2.0), CornerRadius::same(8), Stroke::new(2.0, p.accent), egui::StrokeKind::Outside);
    }

    /// Fereastra „Ajutor și accesibilitate” (F1 sau butonul „?” din bara laterală).
    fn help_window(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }
        let p = theme::palette(ctx);
        let t = &self.t;
        let sc = |s: &egui::KeyboardShortcut| shortcut_text(ctx, s);
        let cmd = |k: &str| format!("{}+{k}", ctx.format_modifiers(egui::Modifiers::COMMAND));
        let v = |items: &[&str]| -> Vec<String> { items.iter().map(|s| s.to_string()).collect() };
        // (titlu, [(taste, descriere)])
        let sections: Vec<(&str, Vec<(Vec<String>, String)>)> = vec![
            (
                "help.general",
                vec![
                    (v(&["Tab", "Shift+Tab"]), t.t("help.tab")),
                    (v(&["Enter", "Space"]), t.t("help.activate")),
                    (vec![sc(&SC_PREV), sc(&SC_NEXT)], format!("{} / {}", t.t("help.prev"), t.t("help.next"))),
                    (vec![cmd("1…9")], t.t("help.jump")),
                    (vec![sc(&SC_SIDEBAR)], t.t("help.sidebar")),
                    (vec![sc(&SC_HELP)], t.t("help.help")),
                ],
            ),
            (
                "help.plan",
                vec![
                    (v(&["←", "↑", "→", "↓"]), t.t("help.planMove")),
                    (v(&["PageDown", "PageUp"]), t.t("help.planCycle")),
                    (v(&["V", "L", "R", "S", "H", "G", "A", "U", "T", "E"]), t.t("help.planTools")),
                    (v(&["Enter"]), t.t("help.planEnter")),
                    (v(&["+", "−"]), t.t("help.planZoom")),
                    (v(&["Delete"]), t.t("help.planDel")),
                    (vec![cmd("Z")], t.t("help.planUndo")),
                    (v(&["Esc"]), t.t("help.planEsc")),
                ],
            ),
            (
                "help.view3d",
                vec![
                    (v(&["←", "↑", "→", "↓"]), t.t("help.rotate")),
                    (v(&["Shift", "←", "↑", "→", "↓"]), t.t("help.pan3d")),
                    (v(&["+", "−"]), t.t("help.zoom3d")),
                    (v(&["Home"]), t.t("help.reset3d")),
                ],
            ),
        ];
        let key_cap = |ui: &mut egui::Ui, k: &str| {
            egui::Frame::new()
                .fill(p.panel2)
                .stroke(Stroke::new(1.0, p.border))
                .corner_radius(CornerRadius::same(5))
                .inner_margin(Margin::symmetric(6, 2))
                .show(ui, |ui| {
                    ui.label(RichText::new(k).monospace().size(12.0).color(p.ink));
                });
        };
        let mut open = true;
        let mut s = self.settings.clone();
        egui::Window::new(t.t("help.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .fixed_size(egui::vec2(560.0, (ctx.content_rect().height() - 100.0).clamp(240.0, 720.0)))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    // Accesibilitate: setările, primele.
                    widgets::section_title(ui, &t.t("help.a11yTitle"));
                    widgets::card(ui, |ui| {
                        let pct = if s.ui_scale_pct == 0 { 100 } else { s.ui_scale_pct };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(t.t("help.uiSize")).color(p.ink));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button(t.t("help.reset")).clicked() {
                                    s.ui_scale_pct = 100;
                                }
                                if ui.add_enabled(pct < 200, egui::Button::new("+")).clicked() {
                                    s.ui_scale_pct = (pct + 10).min(200);
                                }
                                ui.label(RichText::new(format!("{pct}%")).monospace().color(p.ink));
                                if ui.add_enabled(pct > 70, egui::Button::new("−")).clicked() {
                                    s.ui_scale_pct = pct.saturating_sub(10).max(70);
                                }
                            });
                        });
                        ui.label(RichText::new(t.t("help.zoomHint")).size(12.0).color(p.muted));
                        ui.add_space(6.0);
                        ui.checkbox(&mut s.high_contrast, t.t("help.contrast"));
                        ui.checkbox(&mut s.reduce_motion, t.t("help.motion"));
                    });
                    for (title, rows) in &sections {
                        ui.add_space(8.0);
                        widgets::section_title(ui, &t.t(title));
                        widgets::card(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 8.0;
                            for (keys, desc) in rows {
                                ui.horizontal_top(|ui| {
                                    ui.allocate_ui_with_layout(egui::vec2(190.0, 0.0), Layout::left_to_right(Align::Min).with_main_wrap(true), |ui| {
                                        ui.set_width(190.0);
                                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                        for k in keys {
                                            key_cap(ui, k);
                                        }
                                    });
                                    ui.add(egui::Label::new(RichText::new(desc).size(13.0).color(p.ink2)).wrap());
                                });
                            }
                        });
                    }
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(RichText::new(t.t("help.focusHint")).size(12.0).color(p.muted)).wrap());
                });
            });
        if s != self.settings {
            self.set_settings(ctx, s);
        }
        self.show_help = open;
    }

    /// Limba și tema, fixate jos în bara laterală.
    fn sidebar_settings(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let mut s = self.settings.clone();
        egui::Panel::bottom("sidebar_settings").frame(egui::Frame::new().fill(p.panel).inner_margin(Margin::symmetric(0, 12))).show(ui, |ui| {
            ui.horizontal(|ui| {
                lang_switch(ui, &mut s);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let tip = if s.dark { self.t.t("nav.switchToLight") } else { self.t.t("nav.switchToDark") };
                    let help_tip = format!("{} ({})", self.t.t("help.title"), shortcut_text(ui.ctx(), &SC_HELP));
                    if widgets::icon_button(ui, &help_tip, |p, r, col, _bg| {
                        p.text(r.center(), egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(15.0), col);
                    })
                    .clicked()
                    {
                        self.show_help = !self.show_help;
                    }
                    let dark = s.dark;
                    if widgets::icon_button(ui, &tip, |pt, r, col, bg| widgets::paint_theme_icon(pt, r, col, bg, dark)).clicked() {
                        s.dark = !s.dark;
                    }
                });
            });
        });
        if s != self.settings {
            self.set_settings(ui.ctx(), s);
        }
    }

    fn home(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let logo = self.logo_id(ui.ctx());
        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.bg).inner_margin(Margin::symmetric(16, 0))).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                widgets::centered(ui, 720.0, |ui| {
                    ui.add_space(48.0);
                    ui.vertical_centered(|ui| {
                        ui.add(egui::Image::new((logo, egui::vec2(84.0, 84.0))));
                        ui.add_space(10.0);
                        widgets::wordmark(ui, 38.0);
                        ui.add_space(2.0);
                        ui.label(RichText::new(self.t.t("home.subtitle")).size(15.0).color(p.ink2));
                        ui.label(RichText::new(self.t.t("home.subtitle2")).size(13.0).color(p.muted));
                    });
                    ui.add_space(44.0);
                    if let Some(e) = &self.db_error {
                        widgets::alert(ui, &format!("{}: {e}", self.t.t("app.dbError")), true);
                        ui.add_space(8.0);
                    }
                    ui.label(RichText::new(self.t.t("home.myProjects").to_uppercase()).size(11.5).strong().color(p.muted));
                    ui.add_space(8.0);

                    if self.projects.is_empty() {
                        widgets::card(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(14.0);
                                ui.label(RichText::new(self.t.t("home.noProjects")).color(p.ink2));
                                ui.label(RichText::new(self.t.t("home.noProjectsHint")).size(12.5).color(p.muted));
                                ui.add_space(14.0);
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
                        let updated = format!("{} {}", self.t.t("home.updated"), format_date(&e.updated_at));
                        let meta = if e.locality.is_empty() { updated } else { format!("{} · {updated}", e.locality) };
                        let resp = egui::Frame::new()
                            .fill(p.panel)
                            .stroke(Stroke::new(1.0, p.border))
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(Margin::symmetric(16, 12))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(&e.name).size(15.0).color(p.ink));
                                        ui.label(RichText::new(meta).size(12.0).color(p.muted));
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
                            ui.painter().rect_stroke(resp.rect, CornerRadius::same(10), Stroke::new(1.0, p.accent_ring), egui::StrokeKind::Inside);
                        }
                        if resp.clicked() && delete.is_none() {
                            open = Some(e.id.clone());
                        }
                        ui.add_space(6.0);
                    }
                    ui.add_space(24.0);
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
        let Some(project) = &mut self.project else { return };
        let mut act: Option<Act> = None;
        ui.add_space(8.0);
        ui.add_space(14.0);
        ui.label(RichText::new(self.t.t("nav.menuProject").to_uppercase()).size(11.5).strong().color(p.muted));
        ui.add_space(2.0);
        let title = if project.general.name.is_empty() { self.t.t("common.newProject") } else { project.general.name.clone() };
        ui.add(egui::Label::new(RichText::new(title).size(15.0).color(p.ink)).wrap());
        if !project.general.locality.is_empty() {
            ui.label(RichText::new(&project.general.locality).size(12.0).color(p.muted));
        }
        ui.add_space(10.0);
        ui.spacing_mut().button_padding = egui::vec2(10.0, 5.0);
        let styled = |text: String| {
            egui::Button::new(RichText::new(text).size(13.0).color(p.ink))
                .fill(p.panel2)
                .stroke(Stroke::new(1.0, p.border_strong))
                .corner_radius(CornerRadius::same(8))
        };
        // Acasă + selectarea altui proiect.
        let half = egui::vec2((ui.available_width() - 4.0) / 2.0, 28.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if ui.add(styled(format!("⌂  {}", self.t.t("nav.home"))).min_size(half)).clicked() {
                act = Some(Act::Home);
            }
            let cur_id = project.id.clone();
            egui::containers::menu::MenuButton::from_button(styled(format!("{}  ▾", self.t.t("nav.projects"))).min_size(half)).ui(ui, |ui| {
                ui.set_min_width(220.0);
                if ui.button(self.t.t("home.newProject")).clicked() {
                    act = Some(Act::New);
                    ui.close();
                }
                if ui.button(self.t.t("home.importProject")).clicked() {
                    act = Some(Act::Import);
                    ui.close();
                }
                if !self.projects.is_empty() {
                    ui.separator();
                }
                for e in &self.projects {
                    let name = if e.id == cur_id { project.general.name.clone() } else { e.name.clone() };
                    let name = if name.is_empty() { self.t.t("common.newProject") } else { name };
                    let txt = if e.id == cur_id { RichText::new(name).color(p.accent) } else { RichText::new(name) };
                    if ui.add(egui::Button::new(txt).frame(false)).clicked() && e.id != cur_id {
                        act = Some(Act::Open(e.id.clone()));
                        ui.close();
                    }
                }
            });
        });

        if let Some(r) = &self.results {
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(4.0);
            let row = |ui: &mut egui::Ui, label: &str, v: f64, unit: &str, dec: usize| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).size(12.5).color(p.ink2));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(unit).size(11.5).color(p.muted));
                        ui.label(RichText::new(widgets::fmt(v, dec)).monospace().size(12.5));
                    });
                });
            };
            row(ui, &self.t.t("sidebar.perspectiveInhabitants"), project.consumers.perspective_inhabitants, "loc.", 0);
            row(ui, "Q_s or max", r.water_demand.q_s_or_max_ls, "l/s", 3);
            row(ui, &self.t.t("sidebar.fireReserveV"), r.hydrants.v_ri, "m³", 3);
            row(ui, &self.t.t("sidebar.vTotal"), r.reservoir.v_total, "m³", 3);
            row(ui, "Q_dim", r.design_flows.q_dim_network, "m³/h", 3);
        }

        if project.current_step == 8 && !self.net_ui.show_3d {
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);
            self.net_ui.canvas.palette(ui, &self.t, project);
        }

        match act {
            Some(Act::Home) => self.close_project(),
            Some(Act::New) => {
                self.flush();
                self.create_project();
            }
            Some(Act::Import) => {
                self.flush();
                self.import_json();
            }
            Some(Act::Open(id)) => {
                self.flush();
                self.open_by_id(&id);
            }
            None => {}
        }
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
        // Ordinea de creare = ordinea Tab: întâi bara de sus (logo, pași), apoi bara laterală, apoi conținutul.
        let sidebar_visible = self.sidebar_visible(ui.ctx());
        let bar = self.app_bar(ui, !sidebar_visible);
        if sidebar_visible {
            let side = egui::Panel::left("sidebar")
                .exact_size(SIDEBAR_W)
                .resizable(false)
                .frame(egui::Frame::new().fill(p.panel).inner_margin(Margin::symmetric(12, 0)))
                .show(ui, |ui| {
                    self.sidebar_settings(ui);
                    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| self.sidebar(ui));
                });
            let sr = side.response.rect;
            ui.painter().vline(sr.right(), bar.bottom()..=sr.bottom(), Stroke::new(1.0, p.border));
        }

        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.bg).inner_margin(Margin::symmetric(20, 18))).show(ui, |ui| {
            let step = self.project.as_ref().map_or(1, |p| p.current_step);
            // Ancoră invizibilă la începutul conținutului: țintă pentru „Sari la conținut”.
            let anchor = ui.interact(egui::Rect::from_min_size(ui.cursor().min, egui::vec2(1.0, 1.0)), egui::Id::NULL.with("content_anchor"), egui::Sense::focusable_noninteractive());
            if std::mem::take(&mut self.focus_content) {
                ui.memory_mut(|m| m.request_focus(anchor.id));
            }
            // Dacă un ecran e mai lat decât fereastra, conținutul se derulează orizontal (bară vizibilă),
            // în loc să iasă din fereastră.
            let viewport_w = ui.available_width();
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            egui::ScrollArea::both().id_salt(("step", step)).auto_shrink(false).show(ui, |ui| {
                ui.set_width(viewport_w);
                let (Some(project), Some(results)) = (&mut self.project, &self.results) else { return };
                if let Some(go) = steps::show(ui, &self.t, project, results, &mut self.net_ui) {
                    project.current_step = go;
                }
            });
        });
    }

    // ─────────────────────────── Exporturi (pasul 9) ───────────────────────────

    /// Pornește exportul: un singur element → dialog „Salvează ca”; mai multe → alegi un folder.
    fn start_export(&mut self, ids: Vec<ExportId>) {
        if ids.is_empty() || self.export_job.is_some() || self.project.is_none() {
            return;
        }
        self.flush();
        let folder = if ids.len() > 1 {
            match rfd::FileDialog::new().set_title(self.t.t("exp.title")).pick_folder() {
                Some(f) => Some(f),
                None => return self.toast(self.t.t("exp.cancelled"), false),
            }
        } else {
            None
        };
        let restore = (self.project.as_ref().map_or(1, |p| p.current_step), self.net_ui.show_3d);
        self.export_job = Some(ExportJob { folder, queue: ids.into(), ok: Vec::new(), errs: Vec::new(), cap: None, restore });
    }

    /// Bytes pentru exporturile care nu cer captură; `Err` cu un mesaj scurt.
    fn export_bytes(&self, id: ExportId) -> Result<Vec<u8>, String> {
        use crate::export3d::ModelFormat;
        let p = self.project.as_ref().ok_or("fără proiect")?;
        match id {
            ExportId::Json => serde_json::to_vec_pretty(&p.to_export_json(env!("CARGO_PKG_VERSION"), &now_iso())).map_err(|e| e.to_string()),
            ExportId::Dxf2d => hidro_core::export::dxf::export(&p.network_schema, crate::canvas::scale_ratio(p)).map(String::into_bytes).ok_or_else(|| self.t.t("canvas.exportEmptySchema")),
            ExportId::Dxf3d => crate::view3d::View3D::model_bytes(p, ModelFormat::Dxf),
            ExportId::Glb => crate::view3d::View3D::model_bytes(p, ModelFormat::Glb),
            ExportId::Stl => crate::view3d::View3D::model_bytes(p, ModelFormat::Stl),
            _ => Err("indisponibil".into()),
        }
    }

    /// Scrie un export: în folderul ales (multiplu) sau după un dialog „Salvează ca” (singular).
    fn write_export(&mut self, job: &mut ExportJob, id: ExportId, bytes: Result<Vec<u8>, String>) {
        let base = self.project.as_ref().map_or("proiect".to_string(), |p| hidro_core::export::safe_file_name(&p.general.name, "proiect"));
        let name = format!("{base}{}.{}", id.file_suffix(), id.ext());
        let bytes = match bytes {
            Ok(b) => b,
            Err(e) => return job.errs.push(format!("{name}: {e}")),
        };
        let path = match &job.folder {
            Some(f) => f.join(&name),
            None => match rfd::FileDialog::new().add_filter(id.ext().to_uppercase(), &[id.ext()]).set_file_name(&name).save_file() {
                Some(p) => p,
                None => return,
            },
        };
        match std::fs::write(&path, bytes) {
            Ok(()) => job.ok.push(path),
            Err(e) => job.errs.push(format!("{name}: {e}")),
        }
    }

    /// Rulează coada de exporturi, câte puțin la fiecare cadru. Imaginile (PNG) se iau din
    /// vederile pasului 8: aplicația trece scurt acolo, face captura și revine.
    fn drive_export(&mut self, ctx: &egui::Context) {
        let Some(mut job) = self.export_job.take() else { return };
        loop {
            if let Some(cap) = &mut job.cap {
                cap.frames += 1;
                let id = cap.id;
                let waited = cap.started.elapsed();
                if !cap.requested {
                    let loading = id == ExportId::Png3d && self.net_ui.view3d.terrain_loading() && waited < Duration::from_secs(20);
                    if cap.frames >= 4 && !loading {
                        cap.requested = true;
                        if id == ExportId::Png2d {
                            self.net_ui.canvas.request_png(ctx);
                        } else {
                            self.net_ui.view3d.request_png(ctx);
                        }
                    }
                    ctx.request_repaint();
                    break;
                }
                let ready = if id == ExportId::Png2d { self.net_ui.canvas.png_ready.take() } else { self.net_ui.view3d.png_ready.take() };
                match ready {
                    Some(res) => {
                        job.cap = None;
                        self.write_export(&mut job, id, res);
                    }
                    None if waited > Duration::from_secs(30) => {
                        job.cap = None;
                        job.errs.push(format!("{}: timeout", id.ext()));
                    }
                    None => {
                        ctx.request_repaint();
                        break;
                    }
                }
                continue;
            }
            let Some(id) = job.queue.pop_front() else {
                // Gata: revenim la pasul de unde s-a pornit și anunțăm rezultatul.
                if let Some(p) = &mut self.project {
                    p.current_step = job.restore.0;
                }
                self.net_ui.show_3d = job.restore.1;
                self.finish_export_report(&job);
                return;
            };
            if id.needs_capture() {
                if let Some(p) = &mut self.project {
                    p.current_step = 8;
                }
                self.net_ui.show_3d = id == ExportId::Png3d;
                self.net_ui.expanded = false;
                job.cap = Some(ExportCap { id, frames: 0, requested: false, started: Instant::now() });
                ctx.request_repaint();
                break;
            }
            let bytes = self.export_bytes(id);
            self.write_export(&mut job, id, bytes);
        }
        self.export_job = Some(job);
    }

    fn finish_export_report(&mut self, job: &ExportJob) {
        match (&job.folder, job.ok.as_slice()) {
            (None, [path]) => self.toast(format!("{} ✓  {}", path.extension().map_or(String::new(), |e| e.to_string_lossy().to_uppercase()), path.display()), false),
            (Some(dir), ok) if !ok.is_empty() => {
                self.toast(self.t.tf("exp.done", &[("n", &ok.len().to_string()), ("dir", &dir.display().to_string())]), false)
            }
            _ => {}
        }
        if !job.errs.is_empty() {
            let first = job.errs.first().cloned().unwrap_or_default();
            self.toast(format!("{}: {first}", self.t.tf("exp.failed", &[("n", &job.errs.len().to_string())])), true);
        }
    }

    fn toasts_ui(&mut self, ctx: &egui::Context) {
        // Erorile rămân până sunt închise; celelalte mesaje dispar după 8 secunde.
        self.toasts.retain(|t| !t.closed && (t.error || t.since.elapsed() < Duration::from_secs(8)));
        if self.toasts.is_empty() {
            return;
        }
        let p = theme::palette(ctx);
        egui::Area::new(egui::Id::unique("toasts")).anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0)).show(ctx, |ui| {
            let close_tip = self.t.t("common.close");
            for t in &mut self.toasts {
                let fg = if t.error { p.danger } else { p.ink };
                egui::Frame::new()
                    .fill(p.panel)
                    .stroke(Stroke::new(1.0, if t.error { p.danger } else { p.border_strong }))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_max_width(400.0);
                        ui.horizontal(|ui| {
                            let text = if t.error { format!("⚠  {}", t.text) } else { t.text.clone() };
                            ui.add(egui::Label::new(RichText::new(text).color(fg)).wrap());
                            let r = ui.add(egui::Button::new(RichText::new("✕").color(p.muted)).frame(false));
                            widgets::describe(&r, egui::Role::Button, &close_tip);
                            if r.on_hover_text(&close_tip).clicked() {
                                t.closed = true;
                            }
                        });
                    });
                ui.add_space(6.0);
            }
        });
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

impl eframe::App for HidroApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx0 = ui.ctx().clone();
        // Ctrl/⌘ +/− schimbă zoom-ul din egui: îl păstrăm în setări.
        let pct = (ctx0.zoom_factor() * 100.0).round() as u16;
        if pct != self.settings.ui_scale_pct && !(pct == 100 && self.settings.ui_scale_pct == 0) {
            self.settings.ui_scale_pct = pct;
        }
        self.handle_shortcuts(&ctx0);
        let label = self.t.t("common.formulas");
        let invalid_msg = self.t.t("a11y.invalidNumber");
        ui.ctx().data_mut(|d| {
            d.insert_temp(egui::Id::NULL.with("formulas_label"), label);
            d.insert_temp(egui::Id::NULL.with("invalid_number_msg"), invalid_msg);
            d.insert_temp(egui::Id::NULL.with("formula_seq"), 0u32);
            d.insert_temp(egui::Id::NULL.with("formula_step"), self.project.as_ref().map_or(0, |p| p.current_step));
        });
        if self.project.is_some() {
            self.project_view(ui);
        } else {
            self.app_bar(ui, true);
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
        if let Some(ids) = self.net_ui.export_req.take() {
            self.start_export(ids);
        }
        if std::mem::take(&mut self.net_ui.save_now) {
            self.flush();
            self.toast(self.t.t("exp.saveBtn") + " ✓", false);
        }
        self.drive_export(ui.ctx());
        self.net_ui.export_busy = self.export_job.is_some();
        let ctx = ui.ctx().clone();
        self.help_window(&ctx);
        self.paint_focus_ring(&ctx);
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

    /// Exportul JSON păstrează tot proiectul: schema desenată, setările canvasului, tronsoanele.
    #[test]
    fn json_export_roundtrip_keeps_schema() {
        let dir = std::env::temp_dir().join(format!("hidrocalc-json-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("j.db");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 880.0))
            .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
        sample(h.state_mut());
        let p = h.state().project.clone().unwrap();
        assert!(!p.network_schema.is_empty(), "proiectul de test trebuie să aibă schemă");
        let text = serde_json::to_string_pretty(&p.to_export_json("0.1.0", "2026-11-10T10:00:00Z")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let back = ProjectData::from_export_value(value).unwrap();
        assert_eq!(back, p, "exportul JSON a pierdut date");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Modelul 3D al proiectului de test se exportă în toate cele trei formate.
    #[test]
    fn model_3d_exports_for_sample_project() {
        use crate::export3d::ModelFormat;
        let dir = std::env::temp_dir().join(format!("hidrocalc-3d-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("m.db");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 880.0))
            .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
        sample(h.state_mut());
        h.state_mut().project.as_mut().unwrap().current_step = 8;
        h.run_steps(2);
        let p = h.state().project.as_ref().unwrap();
        let glb = crate::view3d::View3D::model_bytes(p, ModelFormat::Glb).unwrap();
        let stl = crate::view3d::View3D::model_bytes(p, ModelFormat::Stl).unwrap();
        let dxf = String::from_utf8(crate::view3d::View3D::model_bytes(p, ModelFormat::Dxf).unwrap()).unwrap();
        assert_eq!(&glb[0..4], b"glTF");
        let n = u32::from_le_bytes(stl[80..84].try_into().unwrap()) as usize;
        assert!(n > 100, "prea puține triunghiuri: {n}");
        assert_eq!(stl.len(), 84 + 50 * n);
        assert_eq!(dxf.matches("3DFACE").count(), n, "câte o față DXF pe triunghi");
        if let Ok(out) = std::env::var("HIDRO_3D_OUT") {
            std::fs::write(format!("{out}/model.glb"), &glb).unwrap();
            std::fs::write(format!("{out}/model.stl"), &stl).unwrap();
            std::fs::write(format!("{out}/model.dxf"), dxf).unwrap();
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Panoul de export de la pasul 9, într-o fereastră înaltă (verificare vizuală).
    #[test]
    fn export_panel_snapshot() {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
        std::fs::create_dir_all(&out).unwrap();
        for dark in [false, true] {
            let dir = std::env::temp_dir().join(format!("hidrocalc-expp-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let db_path = dir.join("x.db");
            let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
            let mut h = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1360.0, 1900.0))
                .with_theme(theme)
                .wgpu()
                .build_eframe(|cc| HidroApp::with_db(cc, Settings { dark, ..Default::default() }, Db::open(&db_path)));
            sample(h.state_mut());
            h.state_mut().project.as_mut().unwrap().current_step = 9;
            h.state_mut().net_ui.export_sel.insert(ExportId::Json);
            h.state_mut().net_ui.export_sel.insert(ExportId::Glb);
            h.run_steps(4);
            h.render().unwrap().save(out.join(format!("export-panel-{}.png", if dark { "dark" } else { "light" }))).unwrap();
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// Bara laterală cu paleta de desen și panoul hărții deschis (verificare vizuală, fără internet).
    #[test]
    fn step8_sidebar_with_map_snapshot() {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
        std::fs::create_dir_all(&out).unwrap();
        for dark in [false, true] {
            let dir = std::env::temp_dir().join(format!("hidrocalc-sb-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let db_path = dir.join("s.db");
            let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
            let mut h = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1360.0, 1800.0))
                .with_theme(theme)
                .wgpu()
                .build_eframe(|cc| HidroApp::with_db(cc, Settings { dark, ..Default::default() }, Db::open(&db_path)));
            sample(h.state_mut());
            {
                let p = h.state_mut().project.as_mut().unwrap();
                let a = hidro_core::geo::MapAnchor { lat: 45.7537, lng: 21.2257, world_x: 0.0, world_y: 0.0, tile_zoom: hidro_core::geo::compute_tile_zoom(45.7537, 1000.0) };
                p.network_canvas_settings.as_object_mut().unwrap().insert("mapAnchor".into(), serde_json::to_value(a).unwrap());
                p.network_canvas_settings.as_object_mut().unwrap().insert("autoTerrainCota".into(), serde_json::json!(false));
                p.current_step = 8;
            }
            h.run_steps(3);
            h.state_mut().net_ui.canvas.map.open = true;
            h.run_steps(4);
            h.render().unwrap().save(out.join(format!("step8-sidebar-map-{}.png", if dark { "dark" } else { "light" }))).unwrap();
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// Pasul 9 fără tronsoane (ce se poate face aici, blocat) și bara laterală ascunsă.
    #[test]
    fn step9_empty_and_collapsed_sidebar_snapshots() {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
        std::fs::create_dir_all(&out).unwrap();
        let dir = std::env::temp_dir().join(format!("hidrocalc-e9-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("e.db");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 1500.0))
            .with_theme(egui::Theme::Light)
            .wgpu()
            .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
        h.state_mut().create_project();
        h.state_mut().project.as_mut().unwrap().current_step = 9;
        h.run_steps(3);
        h.render().unwrap().save(out.join("step9-empty-light.png")).unwrap();
        h.state_mut().settings.sidebar_closed = true;
        h.run_steps(3);
        h.render().unwrap().save(out.join("step9-empty-collapsed.png")).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Scurtături și navigare cu tastatura: pași, bara laterală, ajutor, butoane desenate manual, vederea 3D.
    #[test]
    fn keyboard_navigation_works() {
        use egui_kittest::kittest::Queryable;
        let dir = std::env::temp_dir().join(format!("hidrocalc-kb-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("k.db");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
        sample(h.state_mut());
        h.state_mut().project.as_mut().unwrap().current_step = 3;
        h.run_steps(2);
        let step = |h: &egui_kittest::Harness<'_, HidroApp>| h.state().project.as_ref().unwrap().current_step;

        // Pași: Cmd+] / Cmd+[ și Cmd+număr
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::CloseBracket);
        h.run_steps(2);
        assert_eq!(step(&h), 4);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::OpenBracket);
        h.run_steps(2);
        assert_eq!(step(&h), 3);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num8);
        h.run_steps(2);
        assert_eq!(step(&h), 8);

        // Bara laterală și ajutor
        assert!(!h.state().settings.sidebar_closed);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
        h.run_steps(2);
        assert!(h.state().settings.sidebar_closed);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
        h.run_steps(2);
        assert!(!h.state().settings.sidebar_closed);
        h.key_press(egui::Key::F1);
        h.run_steps(2);
        assert!(h.state().show_help);

        // Buton desenat manual (paleta): focus + Enter alege instrumentul
        h.key_press(egui::Key::F1);
        h.run_steps(2);
        assert!(!h.state().show_help);
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Tronson").focus();
        h.run_steps(1);
        h.key_press(egui::Key::Enter);
        h.run_steps(2);
        assert_eq!(h.state().net_ui.canvas.tool, crate::canvas::Tool::Tronson);

        // Vedere 3D: săgeată dreapta rotește
        h.state_mut().net_ui.show_3d = true;
        h.run_steps(3);
        let before = h.state().net_ui.view3d.yaw_for_test();
        let canvas_label = h.state().t.t("view3d.a11yLabel");
        h.get_by_role_and_label(egui::accesskit::Role::Canvas, &canvas_label).focus();
        h.run_steps(1);
        h.key_press(egui::Key::ArrowRight);
        h.run_steps(2);
        assert_ne!(h.state().net_ui.view3d.yaw_for_test(), before, "săgeata trebuie să rotească vederea");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Erorile rămân până sunt închise; mesajele obișnuite dispar; contrastul ridicat schimbă paleta.
    #[test]
    fn toasts_persist_and_high_contrast_applies() {
        let dir = std::env::temp_dir().join(format!("hidrocalc-ac-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("a.db");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
        sample(h.state_mut());
        h.state_mut().toast("eroare de test", true);
        h.state_mut().toast("salvat", false);
        for t in &mut h.state_mut().toasts {
            t.since = Instant::now() - Duration::from_secs(60);
        }
        h.run_steps(2);
        let left: Vec<String> = h.state().toasts.iter().map(|t| t.text.clone()).collect();
        assert_eq!(left, ["eroare de test"], "doar eroarea trebuie să rămână");

        let ctx = h.ctx.clone();
        let before = theme::palette(&ctx).ink;
        let mut s = h.state().settings.clone();
        s.high_contrast = true;
        h.state_mut().set_settings(&ctx, s);
        h.run_steps(2);
        let after = theme::palette(&ctx).ink;
        assert_ne!(before, after, "paleta cu contrast ridicat trebuie să difere");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Ștergerea unui tronson din tabel cere confirmare (al doilea click).
    #[test]
    fn deleting_a_section_needs_confirmation() {
        use egui_kittest::kittest::Queryable;
        let dir = std::env::temp_dir().join(format!("hidrocalc-dc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("d.db");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(2400.0, 1300.0)) // tabelul e lat: butonul de ștergere trebuie să încapă în fereastră
            .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
        sample(h.state_mut());
        h.state_mut().project.as_mut().unwrap().current_step = 8;
        h.run_steps(3);
        let n = |h: &egui_kittest::Harness<'_, HidroApp>| h.state().project.as_ref().unwrap().network_sections.len();
        let before = n(&h);
        assert!(before >= 2);
        let tip = h.state().t.t("common.delete");
        h.get_all_by_label(&tip).next().expect("buton de ștergere").click();
        h.run_steps(2);
        assert_eq!(n(&h), before, "primul click doar cere confirmare");
        let confirm = h.state().t.t("home.deleteConfirm");
        h.get_all_by_label(&confirm).next().expect("buton de confirmare").click();
        h.run_steps(2);
        assert_eq!(n(&h), before - 1, "al doilea click șterge tronsonul");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Fereastra de ajutor/accesibilitate și contrastul ridicat (verificare vizuală).
    #[test]
    fn help_window_and_high_contrast_snapshot() {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
        std::fs::create_dir_all(&out).unwrap();
        for (dark, hc) in [(false, false), (true, true), (false, true)] {
            let dir = std::env::temp_dir().join(format!("hidrocalc-hw-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let db_path = dir.join("h.db");
            let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
            let mut h = egui_kittest::Harness::builder()
                .with_size(egui::vec2(1360.0, 900.0))
                .with_theme(theme)
                .wgpu()
                .build_eframe(|cc| HidroApp::with_db(cc, Settings { dark, high_contrast: hc, ..Default::default() }, Db::open(&db_path)));
            sample(h.state_mut());
            h.state_mut().project.as_mut().unwrap().current_step = 4;
            h.state_mut().show_help = true;
            h.run_steps(4);
            h.render().unwrap().save(out.join(format!("help-{}{}.png", if dark { "dark" } else { "light" }, if hc { "-hc" } else { "" }))).unwrap();
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// Responsive minimal: conținutul umple spațiul (nu sare), iar sub 1100 px bara laterală se ascunde singură.
    #[test]
    fn layout_adapts_to_width_and_sidebar() {
        use egui_kittest::kittest::Queryable;
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
        std::fs::create_dir_all(&out).unwrap();
        let title_x = |h: &egui_kittest::Harness<'_, HidroApp>| h.get_by_label_contains("Debite ale necesarului").rect().min.x;
        let mk = |w: f32| {
            let dir = std::env::temp_dir().join(format!("hidrocalc-rw-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let db_path = dir.join("r.db");
            let mut h = egui_kittest::Harness::builder()
                .with_size(egui::vec2(w, 800.0))
                .with_theme(egui::Theme::Light)
                .wgpu()
                .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
            sample(h.state_mut());
            h.state_mut().project.as_mut().unwrap().current_step = 3;
            h.run_steps(3);
            (h, dir)
        };
        // 1360: bara deschisă, apoi ascunsă — conținutul pornește de la marginea disponibilă, fără salt în gol
        let (mut h, d1) = mk(1360.0);
        let open_x = title_x(&h);
        assert!(open_x >= 270.0 && open_x < 330.0, "cu bara deschisă conținutul începe lângă ea: {open_x}");
        h.render().unwrap().save(out.join("responsive-1360-open.png")).unwrap();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
        h.run_steps(3);
        let closed_x = title_x(&h);
        assert!(closed_x < 60.0, "cu bara ascunsă conținutul umple lățimea: {closed_x}");
        h.render().unwrap().save(out.join("responsive-1360-closed.png")).unwrap();
        // 1000 px: bara nu se mai ascunde singură; butonul/Cmd+B o ascund și o arată, la orice mărime
        let (mut h2, d2) = mk(1000.0);
        assert!(h2.state().sidebar_visible(&h2.ctx.clone()), "bara laterală nu se ascunde automat");
        h2.render().unwrap().save(out.join("responsive-1000.png")).unwrap();
        h2.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
        h2.run_steps(3);
        assert!(!h2.state().sidebar_visible(&h2.ctx.clone()));
        h2.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
        h2.run_steps(3);
        assert!(h2.state().sidebar_visible(&h2.ctx.clone()));
        h2.state_mut().project.as_mut().unwrap().current_step = 2;
        h2.run_steps(3);
        h2.render().unwrap().save(out.join("responsive-step2-1000.png")).unwrap();
        // Interfață mărită (zoom 1.6 => ~625 pt lățime): butonul de ascundere merge în continuare
        let ctx = h2.ctx.clone();
        let mut s = h2.state().settings.clone();
        s.ui_scale_pct = 160;
        h2.state_mut().set_settings(&ctx, s);
        h2.run_steps(3);
        h2.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
        h2.run_steps(3);
        assert!(!h2.state().sidebar_visible(&ctx), "comutarea barei merge și când interfața e mare");
        h2.render().unwrap().save(out.join("responsive-1000-zoom160.png")).unwrap();
        for d in [d1, d2] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    /// Comutatorul de limbă (buton ca cel de temă) trece RO ↔ EN.
    #[test]
    fn language_switch_toggles_between_ro_and_en() {
        use egui_kittest::kittest::Queryable;
        let dir = std::env::temp_dir().join(format!("hidrocalc-ls-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("l.db");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
        sample(h.state_mut());
        h.run_steps(2);
        assert_eq!(h.state().settings.lang, Lang::Ro);
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Switch to English").click();
        h.run_steps(2);
        assert_eq!(h.state().settings.lang, Lang::En);
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Treci la română").click();
        h.run_steps(2);
        assert_eq!(h.state().settings.lang, Lang::Ro);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Desen și editare fără mouse în planul 2D: tronson din tastatură, selectare și mutare.
    #[test]
    fn canvas_keyboard_draw_select_and_move() {
        use egui_kittest::kittest::Queryable;
        let dir = std::env::temp_dir().join(format!("hidrocalc-kc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("c.db");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
        sample(h.state_mut());
        h.state_mut().project.as_mut().unwrap().current_step = 8;
        h.run_steps(3);
        let count = |h: &egui_kittest::Harness<'_, HidroApp>| h.state().project.as_ref().unwrap().network_schema.len();
        let before = count(&h);
        let label = h.state().t.t("canvas.a11yLabel");
        h.get_by_role_and_label(egui::accesskit::Role::Canvas, &label).focus();
        h.run_steps(1);

        // L = tronson; Enter = început; săgeți; Enter = sfârșit
        h.key_press(egui::Key::L);
        h.run_steps(1);
        assert_eq!(h.state().net_ui.canvas.tool, crate::canvas::Tool::Tronson);
        h.key_press(egui::Key::Enter);
        h.run_steps(1);
        for _ in 0..8 {
            h.key_press(egui::Key::ArrowRight);
            h.run_steps(1);
        }
        h.key_press(egui::Key::Enter);
        h.run_steps(2);
        assert_eq!(count(&h), before + 1, "tronsonul făcut din tastatură trebuie adăugat");

        // V = Select; PageDown alege un element; săgeata dreapta îl mută
        h.key_press(egui::Key::V);
        h.run_steps(1);
        h.key_press(egui::Key::PageDown);
        h.run_steps(1);
        let sel = h.state().net_ui.canvas.selected.clone().expect("element ales");
        let x_of = |h: &egui_kittest::Harness<'_, HidroApp>| h.state().project.as_ref().unwrap().network_schema.iter().find(|e| e.id == sel).map(|e| e.x).unwrap();
        let x0 = x_of(&h);
        h.key_press(egui::Key::ArrowRight);
        h.run_steps(2);
        assert!((x_of(&h) - x0 - 10.0).abs() < 1e-6, "elementul trebuie să se mute cu 10");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Cum arată focusul de la tastatură pe butoane (verificare vizuală).
    #[test]
    fn focus_ring_snapshot() {
        use egui_kittest::kittest::Queryable;
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
        std::fs::create_dir_all(&out).unwrap();
        let dir = std::env::temp_dir().join(format!("hidrocalc-fr-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("f.db");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .with_theme(egui::Theme::Light)
            .wgpu()
            .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
        sample(h.state_mut());
        h.state_mut().project.as_mut().unwrap().current_step = 8;
        h.run_steps(3);
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Hidrant").focus();
        h.run_steps(2);
        h.render().unwrap().save(out.join("focus-tool-button.png")).unwrap();
        h.get_by_role_and_label(egui::accesskit::Role::Button, "⌂  Acasă").focus();
        h.run_steps(2);
        h.render().unwrap().save(out.join("focus-std-button.png")).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Comutatoarele de temă/limbă din bara laterală, în ambele teme și după comutare (verificare vizuală).
    #[test]
    fn theme_toggle_snapshot() {
        use egui_kittest::kittest::Queryable;
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
        std::fs::create_dir_all(&out).unwrap();
        let dir = std::env::temp_dir().join(format!("hidrocalc-tt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("t.db");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .with_theme(egui::Theme::Light)
            .wgpu()
            .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
        sample(h.state_mut());
        h.state_mut().project.as_mut().unwrap().current_step = 3;
        h.run_steps(3);
        h.render().unwrap().save(out.join("toggle-1-light.png")).unwrap();
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Comută la dark mode").click();
        h.run_steps(1);
        h.render().unwrap().save(out.join("toggle-2-after-click.png")).unwrap();
        h.run_steps(4);
        h.render().unwrap().save(out.join("toggle-3-dark.png")).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Coada de exporturi (fără dialoguri): fișierele ajung în folder, PNG-urile din captura pasului 8.
    #[test]
    fn export_job_writes_all_files_and_returns_to_step_9() {
        let dir = std::env::temp_dir().join(format!("hidrocalc-exp-{}", uuid::Uuid::new_v4()));
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let db_path = dir.join("e.db");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 880.0))
            .wgpu()
            .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
        sample(h.state_mut());
        h.state_mut().project.as_mut().unwrap().current_step = 9;
        h.run_steps(2);
        let ids = [ExportId::Json, ExportId::Dxf2d, ExportId::Dxf3d, ExportId::Glb, ExportId::Stl, ExportId::Png2d, ExportId::Png3d];
        h.state_mut().export_job = Some(ExportJob {
            folder: Some(out.clone()),
            queue: ids.into_iter().collect(),
            ok: Vec::new(),
            errs: Vec::new(),
            cap: None,
            restore: (9, false),
        });
        for _ in 0..400 {
            h.run_steps(1);
            std::thread::sleep(std::time::Duration::from_millis(15));
            if h.state().export_job.is_none() {
                break;
            }
        }
        assert!(h.state().export_job.is_none(), "exportul nu s-a terminat");
        assert_eq!(h.state().project.as_ref().unwrap().current_step, 9, "trebuie să revină la pasul 9");
        let mut names: Vec<String> = std::fs::read_dir(&out).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names.len(), 7, "{names:?}");
        for n in &names {
            let len = std::fs::metadata(out.join(n)).unwrap().len();
            assert!(len > 100, "{n} e prea mic ({len} B)");
        }
        let png = std::fs::read(out.join(names.iter().find(|n| n.contains("plan 2D") && n.ends_with(".png")).unwrap())).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        if let Ok(dest) = std::env::var("HIDRO_EXP_OUT") {
            for n in &names {
                std::fs::copy(out.join(n), std::path::Path::new(&dest).join(n)).unwrap();
            }
        }
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
        h.state_mut().net_ui.canvas.map.open = true;
        h.run_steps(4);
        h.render().unwrap().save(out.join("step8-map-panel.png")).unwrap();
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

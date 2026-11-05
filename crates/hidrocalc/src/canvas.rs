//! Editorul grafic al schemei rețelei (echivalentul `NetworkCanvas.tsx`).
//!
//! Coordonatele elementelor sunt în „pixeli lume”; ecran = origine + pan + lume × zoom.
//! Dimensiunile vizuale (grosimi, fonturi, simboluri) urmează regula din React:
//! `vs = clamp(1/zoom, 1, 8)`, deci la zoom mic simbolurile nu dispar.

use crate::i18n::I18n;
use crate::widgets::{self as w, pal};
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, Ui, Vec2};
use hidro_core::calculations::network::NetworkSection;
use hidro_core::model::{ProjectData, SchemaElement, SchemaKind};
use hidro_core::schema::meters_per_px;
use serde_json::{json, Value};

const CANVAS_H: f32 = 520.0;
const ZOOM_MIN: f32 = 0.05;
const ZOOM_MAX: f32 = 40.0;
const SNAP_PX: f32 = 14.0;
const HIT_PX: f32 = 10.0;
const DOT_R: f32 = 7.0;
const BOX_W: f32 = 52.0;
const BOX_H: f32 = 52.0;
const CIRCLE_R: f32 = 10.0;
const ARROW_SZ: f32 = 14.0;
const SUB_W: f32 = 180.0;
const SUB_H: f32 = 100.0;
const UNDO_MAX: usize = 60;

const fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}
const C_TRONSON: Color32 = rgb(0x1A1A1A);
const C_TR_LABEL: Color32 = rgb(0xDC2626);
const C_RED: Color32 = rgb(0xDC2626);
const C_CAMIN_G: Color32 = rgb(0x1D4ED8);
const C_CAMIN_A: Color32 = rgb(0x3B82F6);
const C_SELECTED: Color32 = rgb(0xF59E0B);
const C_GRID: Color32 = rgb(0xE5E7EB);
const C_SNAP: Color32 = rgb(0x3B82F6);
const C_SUB_FILL: Color32 = rgb(0xF0FDFA);
const C_SUB_BRD: Color32 = rgb(0x0D9488);
const C_SUB_TEXT: Color32 = rgb(0x0F766E);
const C_Q: Color32 = rgb(0x2563EB);

/// Culorile predefinite pentru tronsoane (cheile de traducere din `step8.*`).
pub const COLOR_PRESETS: &[(&str, &str)] = &[
    ("#1a1a1a", "colorDefault"),
    ("#1d4ed8", "colorPrimary"),
    ("#16a34a", "colorSecondary"),
    ("#ea580c", "colorTertiary"),
    ("#7c3aed", "colorType4"),
    ("#db2777", "colorType5"),
    ("#0891b2", "colorType6"),
    ("#ca8a04", "colorType7"),
];

pub const SCALE_PRESETS: &[f64] = &[50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0];
pub const GRID_SPACINGS: &[f64] = &[1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Tronson,
    Rezervor,
    Sursa,
    Hidrant,
    CaminGrav,
    CaminAer,
    Subsistem,
    Text,
    Eraser,
}

impl Tool {
    const ALL: [(Tool, &'static str); 10] = [
        (Tool::Select, "select"),
        (Tool::Tronson, "tronson"),
        (Tool::Rezervor, "rezervor"),
        (Tool::Sursa, "sursa"),
        (Tool::Hidrant, "hidrant"),
        (Tool::CaminGrav, "caminGrav"),
        (Tool::CaminAer, "caminAer"),
        (Tool::Subsistem, "subsistem"),
        (Tool::Text, "text"),
        (Tool::Eraser, "eraser"),
    ];

    fn kind(self) -> Option<SchemaKind> {
        Some(match self {
            Tool::Rezervor => SchemaKind::Rezervor,
            Tool::Sursa => SchemaKind::Sursa,
            Tool::Hidrant => SchemaKind::Hidrant,
            Tool::CaminGrav => SchemaKind::CaminGrav,
            Tool::CaminAer => SchemaKind::CaminAer,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Hit {
    Start,
    End,
    Body,
}

#[derive(Debug, Clone)]
enum Drag {
    None,
    Pan,
    Move { id: String, hit: Hit, last: Pos2 },
    Label { id: String, last: Pos2 },
    Draw { start: Pos2, end: Pos2 },
    Resize { id: String },
}

pub struct CanvasUi {
    zoom: f32,
    pan: Vec2,
    pub tool: Tool,
    pub selected: Option<String>,
    drag: Drag,
    undo: Vec<Vec<SchemaElement>>,
    /// Subsistemul în care se editează (id-ul elementului din schema principală).
    active_sub: Option<String>,
    /// Snapshot luat la începutul unei tragere, împins în istoric doar dacă s-a schimbat ceva.
    pending_undo: Option<Vec<SchemaElement>>,
    fitted: bool,
    /// Cerere de export DXF din bara de unelte (preluată de pasul 8).
    pub export_dxf: bool,
    tiles: Option<crate::tiles::TileCache>,
    pub map: crate::map_ui::MapWindow,
    last_rect: Rect,
    /// Captura PNG în curs: ascunde indicatorii de interfață și așteaptă imaginea ecranului.
    capturing: bool,
    png_rx: Option<std::sync::mpsc::Receiver<std::sync::Arc<egui::ColorImage>>>,
    /// Mesaj pentru utilizator (text, eroare), preluat de aplicație ca notificare.
    pub notice: Option<(String, bool)>,
}

impl Default for CanvasUi {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::new(60.0, 220.0),
            tool: Tool::Select,
            selected: None,
            drag: Drag::None,
            undo: Vec::new(),
            active_sub: None,
            pending_undo: None,
            fitted: false,
            export_dxf: false,
            tiles: None,
            map: Default::default(),
            last_rect: Rect::NOTHING,
            capturing: false,
            png_rx: None,
            notice: None,
        }
    }
}

// ─── Setări (în `networkCanvasSettings`, păstrat ca JSON) ─────────────────────

fn setting_f64(p: &ProjectData, key: &str, default: f64) -> f64 {
    p.network_canvas_settings.get(key).and_then(Value::as_f64).unwrap_or(default)
}
fn setting_bool(p: &ProjectData, key: &str, default: bool) -> bool {
    p.network_canvas_settings.get(key).and_then(Value::as_bool).unwrap_or(default)
}
fn set_setting(p: &mut ProjectData, key: &str, v: Value) {
    if !p.network_canvas_settings.is_object() {
        p.network_canvas_settings = hidro_core::model::default_canvas_settings();
    }
    if let Some(o) = p.network_canvas_settings.as_object_mut() {
        o.insert(key.into(), v);
    }
}

pub fn scale_ratio(p: &ProjectData) -> f64 {
    setting_f64(p, "scaleRatio", 1000.0)
}

// ─── Geometrie ────────────────────────────────────────────────────────────────

fn parse_color(s: Option<&str>) -> Option<Color32> {
    let h = s?.strip_prefix('#')?;
    (h.len() == 6).then(|| u32::from_str_radix(h, 16).ok().map(rgb)).flatten()
}

fn end_of(el: &SchemaElement) -> (f32, f32) {
    (el.x2.unwrap_or(el.x) as f32, el.y2.unwrap_or(el.y) as f32)
}

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 == 0.0 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn sub_ports(el: &SchemaElement) -> Vec<(Pos2, bool)> {
    let w = el.w.map_or(SUB_W, |v| v as f32);
    let h = el.h.map_or(SUB_H, |v| v as f32);
    let (bx, by) = (el.x as f32 - w / 2.0, el.y as f32 - h / 2.0);
    let sides: Vec<bool> = match &el.ports {
        Some(ports) => ports.iter().map(|p| p.get("side").and_then(Value::as_str) == Some("L")).collect(),
        None => vec![true, false],
    };
    let nl = sides.iter().filter(|l| **l).count();
    let nr = sides.len() - nl;
    let (mut il, mut ir) = (0, 0);
    sides
        .into_iter()
        .map(|left| {
            if left {
                il += 1;
                (Pos2::new(bx, by + h / (nl + 1) as f32 * il as f32), true)
            } else {
                ir += 1;
                (Pos2::new(bx + w, by + h / (nr + 1) as f32 * ir as f32), false)
            }
        })
        .collect()
}

/// Punctele la care se pot lega capetele tronsoanelor.
fn connection_points(el: &SchemaElement) -> Vec<Pos2> {
    let (x, y) = (el.x as f32, el.y as f32);
    match el.kind {
        SchemaKind::Tronson => {
            let (ex, ey) = end_of(el);
            vec![Pos2::new(x, y), Pos2::new(ex, ey)]
        }
        SchemaKind::Text => vec![],
        SchemaKind::Subsistem => sub_ports(el).into_iter().map(|p| p.0).collect(),
        SchemaKind::Rezervor | SchemaKind::Sursa => vec![Pos2::new(x + el.w.map_or(BOX_W, |v| v as f32) / 2.0, y)],
        SchemaKind::Hidrant | SchemaKind::CaminGrav | SchemaKind::CaminAer => {
            let r = el.r.map_or(CIRCLE_R, |v| v as f32);
            vec![Pos2::new(x - r, y), Pos2::new(x + r, y), Pos2::new(x, y - r), Pos2::new(x, y + r)]
        }
    }
}

fn hit_test(el: &SchemaElement, p: Pos2, t: f32) -> Option<Hit> {
    let (x, y) = (el.x as f32, el.y as f32);
    match el.kind {
        SchemaKind::Tronson => {
            let (ex, ey) = end_of(el);
            if p.distance(Pos2::new(x, y)) < t * 1.8 {
                Some(Hit::Start)
            } else if p.distance(Pos2::new(ex, ey)) < t * 1.8 {
                Some(Hit::End)
            } else if dist_to_segment(p, Pos2::new(x, y), Pos2::new(ex, ey)) < t {
                Some(Hit::Body)
            } else {
                None
            }
        }
        SchemaKind::Rezervor | SchemaKind::Sursa => {
            let (w, h) = (el.w.map_or(BOX_W, |v| v as f32), el.h.map_or(BOX_H, |v| v as f32));
            let r = Rect::from_min_size(Pos2::new(x - w / 2.0, y - h), Vec2::new(w, h));
            (r.contains(p) || p.distance(Pos2::new(x, y)) < t).then_some(Hit::Body)
        }
        SchemaKind::Hidrant | SchemaKind::CaminGrav | SchemaKind::CaminAer => {
            (p.distance(Pos2::new(x, y)) < el.r.map_or(CIRCLE_R, |v| v as f32) + t).then_some(Hit::Body)
        }
        SchemaKind::Text => {
            let fs = el.font_size.unwrap_or(12.0) as f32;
            ((p.x - x).abs() < 60.0 && p.y >= y - 4.0 && p.y <= y + fs + 4.0).then_some(Hit::Body)
        }
        SchemaKind::Subsistem => {
            let (w, h) = (el.w.map_or(SUB_W, |v| v as f32), el.h.map_or(SUB_H, |v| v as f32));
            Rect::from_center_size(Pos2::new(x, y), Vec2::new(w, h)).contains(p).then_some(Hit::Body)
        }
    }
}

/// Poziția etichetei (în coordonate lume), pentru tragerea ei.
fn label_pos(el: &SchemaElement, vs: f32) -> Option<Pos2> {
    let (x, y) = (el.x as f32, el.y as f32);
    let off = Vec2::new(el.label_offset_x.unwrap_or(0.0) as f32, el.label_offset_y.unwrap_or(0.0) as f32);
    el.label.as_ref()?;
    Some(match el.kind {
        SchemaKind::Tronson => {
            let (ex, ey) = end_of(el);
            let d = Vec2::new(ex - x, ey - y);
            let len = d.length();
            if len < 2.0 {
                return None;
            }
            let perp = Vec2::new(-d.y / len, d.x / len);
            Pos2::new((x + ex) / 2.0, (y + ey) / 2.0) + perp * 15.0 * vs + off
        }
        SchemaKind::Rezervor | SchemaKind::Sursa => Pos2::new(x, y + DOT_R * vs + 3.0 + 6.0 * vs) + off,
        SchemaKind::Hidrant | SchemaKind::CaminGrav | SchemaKind::CaminAer => {
            Pos2::new(x, y + el.r.map_or(CIRCLE_R, |v| v as f32) * vs + 3.0 + 6.0 * vs) + off
        }
        SchemaKind::Subsistem => Pos2::new(x, y - el.h.map_or(SUB_H, |v| v as f32) / 2.0 + 14.0) + off,
        SchemaKind::Text => return None,
    })
}

/// Mânerul de redimensionare (coordonate lume): colțul dreapta-sus al cutiilor,
/// vârful cercurilor. `vs` = factorul de vizibilitate al simbolurilor.
fn resize_handle(el: &SchemaElement, vs: f32) -> Option<Pos2> {
    let (x, y) = (el.x as f32, el.y as f32);
    match el.kind {
        SchemaKind::Rezervor | SchemaKind::Sursa => {
            let (w, h) = (el.w.map_or(BOX_W, |v| v as f32) * vs, el.h.map_or(BOX_H, |v| v as f32) * vs);
            Some(Pos2::new(x + w / 2.0, y - h))
        }
        SchemaKind::Hidrant | SchemaKind::CaminGrav | SchemaKind::CaminAer => Some(Pos2::new(x, y - el.r.map_or(CIRCLE_R, |v| v as f32) * vs)),
        _ => None,
    }
}

/// Aplică poziția mânerului tras: dimensiunile noi ale simbolului (minim 20 / 5 px).
fn apply_resize(el: &mut SchemaElement, to: Pos2, vs: f32) {
    let (x, y) = (el.x as f32, el.y as f32);
    match el.kind {
        SchemaKind::Rezervor | SchemaKind::Sursa => {
            el.w = Some(((to.x - x) * 2.0 / vs).max(20.0) as f64);
            el.h = Some(((y - to.y) / vs).max(20.0) as f64);
        }
        SchemaKind::Hidrant | SchemaKind::CaminGrav | SchemaKind::CaminAer => el.r = Some(((y - to.y) / vs).max(5.0) as f64),
        _ => {}
    }
}

fn prefix(kind: SchemaKind) -> &'static str {
    match kind {
        SchemaKind::Tronson => "Tr",
        SchemaKind::Rezervor => "R",
        SchemaKind::Sursa => "S",
        SchemaKind::Hidrant => "He",
        SchemaKind::CaminGrav => "Cvg",
        SchemaKind::CaminAer => "Cae",
        SchemaKind::Subsistem => "Loc",
        SchemaKind::Text => "",
    }
}

fn next_label(kind: SchemaKind, level: &[SchemaElement]) -> Option<String> {
    let p = prefix(kind);
    (!p.is_empty()).then(|| format!("{p} {}", level.iter().filter(|e| e.kind == kind).count() + 1))
}

fn format_dist(m: f64) -> String {
    if m >= 1000.0 {
        format!("{:.2} km", m / 1000.0)
    } else if m >= 1.0 {
        format!("{m:.1} m")
    } else {
        format!("{:.0} cm", m * 100.0)
    }
}

// ─── Editorul ─────────────────────────────────────────────────────────────────

impl CanvasUi {
    fn level<'a>(&self, p: &'a mut ProjectData) -> &'a mut Vec<SchemaElement> {
        if let Some(id) = &self.active_sub {
            if let Some(i) = p.network_schema.iter().position(|e| &e.id == id && e.kind == SchemaKind::Subsistem) {
                return p.network_schema[i].children.get_or_insert_with(Vec::new);
            }
        }
        &mut p.network_schema
    }

    fn checkpoint(&mut self, p: &ProjectData) {
        self.undo.push(p.network_schema.clone());
        if self.undo.len() > UNDO_MAX {
            self.undo.remove(0);
        }
    }

    fn vs(&self) -> f32 {
        (1.0 / self.zoom).clamp(1.0, 8.0)
    }

    fn to_screen(&self, origin: Pos2, p: Pos2) -> Pos2 {
        origin + self.pan + p.to_vec2() * self.zoom
    }

    fn to_world(&self, origin: Pos2, s: Pos2) -> Pos2 {
        ((s - origin - self.pan) / self.zoom).to_pos2()
    }

    /// Încadrează schema în zona vizibilă (la prima afișare și la „Încadrează”).
    fn fit(&mut self, level: &[SchemaElement], rect: Rect) {
        let mut pts: Vec<Pos2> = Vec::new();
        for e in level {
            let (x, y) = (e.x as f32, e.y as f32);
            pts.push(Pos2::new(x, y));
            match e.kind {
                SchemaKind::Tronson => {
                    let (ex, ey) = end_of(e);
                    pts.push(Pos2::new(ex, ey));
                }
                SchemaKind::Rezervor | SchemaKind::Sursa => {
                    let (w, h) = (e.w.map_or(BOX_W, |v| v as f32), e.h.map_or(BOX_H, |v| v as f32));
                    pts.push(Pos2::new(x - w / 2.0, y - h));
                    pts.push(Pos2::new(x + w / 2.0, y + 20.0));
                }
                SchemaKind::Subsistem => {
                    let (w, h) = (e.w.map_or(SUB_W, |v| v as f32), e.h.map_or(SUB_H, |v| v as f32));
                    pts.push(Pos2::new(x - w / 2.0, y - h / 2.0));
                    pts.push(Pos2::new(x + w / 2.0, y + h / 2.0));
                }
                _ => {}
            }
        }
        if pts.is_empty() {
            self.zoom = 1.0;
            self.pan = Vec2::new(60.0, rect.height() / 2.0);
            return;
        }
        // Marginea acoperă simbolurile și etichetele, care au dimensiune fixă pe ecran.
        let bb = Rect::from_points(&pts).expand(90.0);
        self.zoom = (rect.width() / bb.width()).min(rect.height() / bb.height()).clamp(ZOOM_MIN, 4.0);
        self.pan = rect.size() / 2.0 - bb.center().to_vec2() * self.zoom;
    }

    fn snap(&self, p: &ProjectData, level: &[SchemaElement], world: Pos2, exclude: Option<&str>) -> (Pos2, bool) {
        if setting_bool(p, "snapToEndpoints", true) {
            let r = SNAP_PX / self.zoom;
            let mut best: Option<(f32, Pos2)> = None;
            for el in level.iter().filter(|e| Some(e.id.as_str()) != exclude) {
                for c in connection_points(el) {
                    let d = c.distance(world);
                    if d < r && best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, c));
                    }
                }
            }
            if let Some((_, c)) = best {
                return (c, true);
            }
        }
        if setting_bool(p, "snapToGrid", false) {
            let step = (setting_f64(p, "gridSpacing", 10.0) / meters_per_px(scale_ratio(p))) as f32;
            if step > 0.0 {
                return (Pos2::new((world.x / step).round() * step, (world.y / step).round() * step), true);
            }
        }
        (world, false)
    }

    /// Decupează zona canvas-ului din captura ecranului și o salvează ca PNG.
    fn finish_png(&mut self, ctx: &egui::Context, t: &I18n, p: &ProjectData) {
        let Some(img) = self.png_rx.as_ref().and_then(|rx| rx.try_recv().ok()) else {
            if self.png_rx.is_some() {
                ctx.request_repaint();
            }
            return;
        };
        self.png_rx = None;
        self.capturing = false;
        let (rgba, w, h) = crop_rgba(&img, self.last_rect, ctx.pixels_per_point());
        let mut png = Vec::new();
        let enc = image::codecs::png::PngEncoder::new(&mut png);
        if let Err(e) = image::ImageEncoder::write_image(enc, &rgba, w as u32, h as u32, image::ExtendedColorType::Rgba8) {
            self.notice = Some((format!("{} {e}", t.t("docs.exportFailed")), true));
            return;
        }
        let name = hidro_core::export::safe_file_name(&p.general.name, "schema-retea");
        let Some(path) = rfd::FileDialog::new().add_filter("PNG", &["png"]).set_file_name(format!("{name}.png")).save_file() else { return };
        self.notice = Some(match std::fs::write(&path, png) {
            Ok(()) => (format!("PNG ✓  {}", path.display()), false),
            Err(e) => (format!("{} {e}", t.t("docs.exportFailed")), true),
        });
    }

    pub fn ui(&mut self, ui: &mut Ui, t: &I18n, p: &mut ProjectData) {
        let ctx = ui.ctx().clone();
        self.finish_png(&ctx, t, p);
        self.map.poll(p);
        if crate::map_ui::anchor(p).is_some() {
            let ctx = ui.ctx().clone();
            self.tiles.get_or_insert_with(|| crate::tiles::TileCache::new(&ctx)).poll(&ctx);
        }
        self.toolbar(ui, t, p);
        ui.add_space(6.0);
        self.breadcrumb(ui, t, p);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), CANVAS_H), Sense::click_and_drag());
        if !self.fitted {
            let lvl = self.level(p).clone();
            self.fit(&lvl, rect);
            self.fitted = true;
        }
        self.last_rect = rect;
        self.interact(ui, &resp, rect, p);
        self.paint(ui, rect, p, &resp);
        crate::map_ui::info_bar(ui, t, p);
        self.properties(ui, t, p);
        // Ancora nouă se pune în punctul din centrul vederii curente.
        let center = self.to_world(rect.min, rect.center());
        let live = crate::map_ui::View { origin: rect.min, pan: self.pan, zoom: self.zoom };
        self.map.ui(ui.ctx(), t, p, (center.x as f64, center.y as f64), live);
    }

    fn toolbar(&mut self, ui: &mut Ui, t: &I18n, p: &mut ProjectData) {
        let pl = pal(ui);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for (tool, key) in Tool::ALL {
                let sel = self.tool == tool;
                let short = if matches!(tool, Tool::Rezervor | Tool::Sursa) { key.to_string() } else { format!("{key}Short") };
                let txt = RichText::new(t.t(&format!("canvas.tools.{short}"))).size(12.0).color(if sel { pl.accent_fg } else { pl.ink2 });
                if ui.add(egui::Button::new(txt).fill(if sel { pl.accent } else { pl.panel })).on_hover_text(t.t(&format!("canvas.tools.{key}"))).clicked() {
                    self.tool = tool;
                }
            }
            ui.separator();
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("↶")).on_hover_text(t.t("canvas.undoTitle")).clicked() {
                self.undo_once(p);
            }
            if ui.button("−").on_hover_text(t.t("canvas.zoomOut")).clicked() {
                self.zoom = (self.zoom / 1.25).max(ZOOM_MIN);
            }
            ui.label(RichText::new(format!("{:.0}%", self.zoom * 100.0)).monospace().size(11.5).color(pl.muted));
            if ui.button("+").on_hover_text(t.t("canvas.zoomIn")).clicked() {
                self.zoom = (self.zoom * 1.25).min(ZOOM_MAX);
            }
            if ui.button(t.t("native.fit")).clicked() {
                self.fitted = false;
            }
            ui.separator();
            let has_map = crate::map_ui::anchor(p).is_some();
            if ui.add(egui::Button::new(format!("🗺 {}", t.t("native.mapButton"))).selected(has_map)).clicked() {
                self.map.open_for(p);
            }
            if ui.button("DXF").on_hover_text(t.t("native.dxfDesc")).clicked() {
                self.export_dxf = true;
            }
            if ui.add_enabled(self.png_rx.is_none(), egui::Button::new("PNG")).on_hover_text(t.t("native.pngDesc")).clicked() {
                // Fără selecție și indicatori pe imagine; captura vine la cadrul următor.
                self.selected = None;
                self.capturing = true;
                let (tx, rx) = std::sync::mpsc::channel();
                ui.ctx().request_screenshot(move |img| {
                    let _ = tx.send(img);
                });
                self.png_rx = Some(rx);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let sr = scale_ratio(p);
            let opts: Vec<(String, String)> = SCALE_PRESETS.iter().map(|r| (format!("{r}"), format!("1:{r}"))).collect();
            let mut cur = format!("{sr}");
            let lbl = ui.label(RichText::new(t.t("canvas.scale")).size(12.0).color(pl.muted)).id;
            if w::combo_labelled(ui, egui::Id::unique("scale"), &mut cur, &opts, 90.0, Some(lbl)) {
                if let Ok(v) = cur.parse::<f64>() {
                    set_setting(p, "scaleRatio", json!(v));
                }
            }
            for (key, label) in [("gridVisible", "canvas.grid"), ("snapToGrid", "native.snapGrid"), ("snapToEndpoints", "canvas.snap")] {
                let mut v = setting_bool(p, key, key == "snapToEndpoints");
                if ui.checkbox(&mut v, t.t(label)).changed() {
                    set_setting(p, key, json!(v));
                }
            }
            if setting_bool(p, "gridVisible", false) || setting_bool(p, "snapToGrid", false) {
                let gs = setting_f64(p, "gridSpacing", 10.0);
                let opts: Vec<(String, String)> = GRID_SPACINGS.iter().map(|g| (format!("{g}"), format!("{g} m"))).collect();
                let mut cur = format!("{gs}");
                let lbl = ui.label(RichText::new(t.t("canvas.gridStep")).size(12.0).color(pl.muted)).id;
                if w::combo_labelled(ui, egui::Id::unique("grid"), &mut cur, &opts, 70.0, Some(lbl)) {
                    if let Ok(v) = cur.parse::<f64>() {
                        set_setting(p, "gridSpacing", json!(v));
                    }
                }
            }
        });
    }

    fn breadcrumb(&mut self, ui: &mut Ui, t: &I18n, p: &ProjectData) {
        let Some(id) = &self.active_sub else { return };
        let label = p.network_schema.iter().find(|e| &e.id == id).and_then(|e| e.label.clone()).unwrap_or_default();
        let pl = pal(ui);
        ui.horizontal(|ui| {
            if ui.link(t.t("step8.mainSchema")).clicked() {
                self.leave_sub();
            }
            ui.label(RichText::new(format!("›  {label}")).strong().color(C_SUB_TEXT));
            ui.label(RichText::new(t.t("canvas.subsistemHint")).size(11.0).color(pl.muted));
        });
    }

    fn leave_sub(&mut self) {
        self.active_sub = None;
        self.selected = None;
        self.fitted = false;
    }

    fn undo_once(&mut self, p: &mut ProjectData) {
        if let Some(prev) = self.undo.pop() {
            p.network_schema = prev;
            self.selected = None;
            if let Some(id) = &self.active_sub {
                if !p.network_schema.iter().any(|e| &e.id == id) {
                    self.leave_sub();
                }
            }
        }
    }

    fn delete_selected(&mut self, p: &mut ProjectData) {
        let Some(id) = self.selected.take() else { return };
        self.checkpoint(p);
        self.level(p).retain(|e| e.id != id);
    }

    fn interact(&mut self, ui: &mut Ui, resp: &egui::Response, rect: Rect, p: &mut ProjectData) {
        let origin = rect.min;
        // Zoom cu rotița / pinch, centrat pe cursor.
        if let Some(hover) = resp.hover_pos() {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = pinch * (scroll / 400.0).exp();
            if (factor - 1.0).abs() > 1e-4 {
                let before = self.to_world(origin, hover);
                self.zoom = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
                self.pan = hover - origin - before.to_vec2() * self.zoom;
            }
        }
        // Tastatură (doar când canvas-ul e sub cursor sau are focus).
        if resp.hovered() || resp.has_focus() {
            let (del, esc, undo) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                    i.key_pressed(egui::Key::Escape),
                    i.modifiers.command && i.key_pressed(egui::Key::Z),
                )
            });
            let typing = ui.memory(|m| m.focused().is_some()) && !resp.has_focus();
            if del && !typing {
                self.delete_selected(p);
            }
            if esc {
                if self.selected.is_some() {
                    self.selected = None;
                } else if self.active_sub.is_some() {
                    self.leave_sub();
                }
                self.tool = Tool::Select;
            }
            if undo && !typing {
                self.undo_once(p);
            }
        }

        let thr = HIT_PX / self.zoom;
        let vs = self.vs();

        if resp.double_clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let wp = self.to_world(origin, pos);
                if self.active_sub.is_none() {
                    if let Some(sub) = p.network_schema.iter().rev().find(|e| e.kind == SchemaKind::Subsistem && hit_test(e, wp, thr).is_some()) {
                        self.active_sub = Some(sub.id.clone());
                        self.selected = None;
                        self.fitted = false;
                        return;
                    }
                }
            }
        }

        if resp.clicked() {
            let Some(pos) = resp.interact_pointer_pos() else { return };
            let wp = self.to_world(origin, pos);
            let level = self.level(p).clone();
            match self.tool {
                Tool::Select => {
                    self.selected = level.iter().rev().find(|e| hit_test(e, wp, thr).is_some()).map(|e| e.id.clone());
                }
                Tool::Eraser => {
                    if let Some(id) = level.iter().rev().find(|e| hit_test(e, wp, thr).is_some()).map(|e| e.id.clone()) {
                        self.checkpoint(p);
                        self.level(p).retain(|e| e.id != id);
                    }
                }
                Tool::Text => {
                    self.checkpoint(p);
                    let id = uuid::Uuid::new_v4().to_string();
                    self.level(p).push(SchemaElement {
                        id: id.clone(),
                        kind: SchemaKind::Text,
                        x: wp.x as f64,
                        y: wp.y as f64,
                        text: Some("Text".into()),
                        font_size: Some(14.0),
                        ..Default::default()
                    });
                    self.selected = Some(id);
                    self.tool = Tool::Select;
                }
                Tool::Subsistem => {
                    if self.active_sub.is_some() {
                        self.tool = Tool::Select;
                        return;
                    }
                    self.checkpoint(p);
                    let id = uuid::Uuid::new_v4().to_string();
                    let label = next_label(SchemaKind::Subsistem, &p.network_schema);
                    let ports = vec![
                        json!({"id": uuid::Uuid::new_v4().to_string(), "side": "L"}),
                        json!({"id": uuid::Uuid::new_v4().to_string(), "side": "R"}),
                    ];
                    p.network_schema.push(SchemaElement {
                        id: id.clone(),
                        kind: SchemaKind::Subsistem,
                        x: wp.x as f64,
                        y: wp.y as f64,
                        label,
                        children: Some(Vec::new()),
                        ports: Some(ports),
                        ..Default::default()
                    });
                    self.selected = Some(id);
                    self.tool = Tool::Select;
                }
                tool => {
                    if let Some(kind) = tool.kind() {
                        self.checkpoint(p);
                        let (sp, _) = self.snap(p, &level, wp, None);
                        let id = uuid::Uuid::new_v4().to_string();
                        let label = next_label(kind, &level);
                        self.level(p).push(SchemaElement { id: id.clone(), kind, x: sp.x as f64, y: sp.y as f64, label, ..Default::default() });
                        self.selected = Some(id);
                        self.tool = Tool::Select;
                    }
                }
            }
            return;
        }

        if resp.drag_started() {
            let Some(start) = ui.input(|i| i.pointer.press_origin()) else { return };
            let wp = self.to_world(origin, start);
            let level = self.level(p).clone();
            let middle = ui.input(|i| i.pointer.middle_down() || i.pointer.secondary_down());
            self.drag = if middle {
                Drag::Pan
            } else {
                match self.tool {
                    Tool::Tronson => {
                        let (sp, _) = self.snap(p, &level, wp, None);
                        Drag::Draw { start: sp, end: sp }
                    }
                    Tool::Select if self
                        .selected
                        .as_ref()
                        .and_then(|id| level.iter().find(|e| &e.id == id))
                        .and_then(|e| resize_handle(e, vs))
                        .is_some_and(|h| h.distance(wp) < thr * 1.5) =>
                    {
                        self.pending_undo = Some(p.network_schema.clone());
                        Drag::Resize { id: self.selected.clone().unwrap_or_default() }
                    }
                    Tool::Select => {
                        let label_hit = level
                            .iter()
                            .rev()
                            .find(|e| hit_test(e, wp, thr).is_none() && label_pos(e, vs).is_some_and(|lp| lp.distance(wp) < thr * 2.5));
                        if let Some(e) = label_hit {
                            self.selected = Some(e.id.clone());
                            self.pending_undo = Some(p.network_schema.clone());
                            Drag::Label { id: e.id.clone(), last: wp }
                        } else if let Some((e, hit)) = level.iter().rev().find_map(|e| hit_test(e, wp, thr).map(|h| (e, h))) {
                            self.selected = Some(e.id.clone());
                            self.pending_undo = Some(p.network_schema.clone());
                            Drag::Move { id: e.id.clone(), hit, last: wp }
                        } else {
                            self.selected = None;
                            Drag::Pan
                        }
                    }
                    _ => Drag::Pan,
                }
            };
        }

        if resp.dragged() {
            let cur = resp.interact_pointer_pos().map(|s| self.to_world(origin, s));
            match self.drag.clone() {
                Drag::Pan => self.pan += resp.drag_delta(),
                Drag::Draw { start, .. } => {
                    if let Some(c) = cur {
                        let level = self.level(p).clone();
                        let (sp, _) = self.snap(p, &level, c, None);
                        self.drag = Drag::Draw { start, end: sp };
                    }
                }
                Drag::Label { id, last } => {
                    if let Some(c) = cur {
                        if let Some(e) = self.level(p).iter_mut().find(|e| e.id == id) {
                            e.label_offset_x = Some(e.label_offset_x.unwrap_or(0.0) + (c.x - last.x) as f64);
                            e.label_offset_y = Some(e.label_offset_y.unwrap_or(0.0) + (c.y - last.y) as f64);
                        }
                        self.drag = Drag::Label { id, last: c };
                    }
                }
                Drag::Move { id, hit, last } => {
                    if let Some(c) = cur {
                        let level = self.level(p).clone();
                        if hit == Hit::Body {
                            let d = c - last;
                            if let Some(e) = self.level(p).iter_mut().find(|e| e.id == id) {
                                e.x += d.x as f64;
                                e.y += d.y as f64;
                                if let (Some(x2), Some(y2)) = (e.x2, e.y2) {
                                    e.x2 = Some(x2 + d.x as f64);
                                    e.y2 = Some(y2 + d.y as f64);
                                }
                            }
                        } else {
                            let (sp, _) = self.snap(p, &level, c, Some(&id));
                            if let Some(e) = self.level(p).iter_mut().find(|e| e.id == id) {
                                if hit == Hit::Start {
                                    e.x = sp.x as f64;
                                    e.y = sp.y as f64;
                                } else {
                                    e.x2 = Some(sp.x as f64);
                                    e.y2 = Some(sp.y as f64);
                                }
                            }
                        }
                        self.drag = Drag::Move { id, hit, last: c };
                    }
                }
                Drag::Resize { id } => {
                    if let Some(c) = cur {
                        if let Some(e) = self.level(p).iter_mut().find(|e| e.id == id) {
                            apply_resize(e, c, vs);
                        }
                    }
                }
                Drag::None => {}
            }
        }

        if resp.drag_stopped() {
            if let Drag::Draw { start, end } = std::mem::replace(&mut self.drag, Drag::None) {
                if start.distance(end) * self.zoom > 5.0 {
                    self.checkpoint(p);
                    let level = self.level(p).clone();
                    let id = uuid::Uuid::new_v4().to_string();
                    self.level(p).push(SchemaElement {
                        id: id.clone(),
                        kind: SchemaKind::Tronson,
                        x: start.x as f64,
                        y: start.y as f64,
                        x2: Some(end.x as f64),
                        y2: Some(end.y as f64),
                        label: next_label(SchemaKind::Tronson, &level),
                        ..Default::default()
                    });
                    self.selected = Some(id);
                }
            }
            if let Some(before) = self.pending_undo.take() {
                if before != p.network_schema {
                    self.undo.push(before);
                }
            }
            self.drag = Drag::None;
        }
    }

    fn paint(&mut self, ui: &mut Ui, rect: Rect, p: &mut ProjectData, resp: &egui::Response) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::same(10), Color32::WHITE);
        let origin = rect.min;
        let view = crate::map_ui::View { origin, pan: self.pan, zoom: self.zoom };
        if let (Some(a), Some(tiles)) = (crate::map_ui::anchor(p), self.tiles.as_mut()) {
            let mv = crate::map_ui::map_view(p, view);
            crate::map_ui::draw_tiles(&painter, rect, mv, &a, crate::map_ui::rotation(p), crate::map_ui::opacity(p), tiles);
        }
        painter.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, pal(ui).border), egui::StrokeKind::Inside);
        let mpp = meters_per_px(scale_ratio(p));
        let z = self.zoom;
        let vs = self.vs();
        let sc = vs * z; // dimensiune lume → ecran pentru simboluri

        // Grilă
        if setting_bool(p, "gridVisible", false) {
            let step = (setting_f64(p, "gridSpacing", 10.0) / mpp) as f32 * z;
            if step >= 6.0 {
                let o = origin + self.pan;
                let mut x = rect.left() + (o.x - rect.left()).rem_euclid(step);
                while x < rect.right() {
                    painter.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], Stroke::new(1.0, C_GRID));
                    x += step;
                }
                let mut y = rect.top() + (o.y - rect.top()).rem_euclid(step);
                while y < rect.bottom() {
                    painter.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], Stroke::new(1.0, C_GRID));
                    y += step;
                }
            }
        }

        if self.active_sub.is_none() {
            if let Some(a) = crate::map_ui::anchor(p) {
                crate::map_ui::draw_topo(&painter, view, &a, crate::map_ui::rotation(p), &crate::map_ui::topo_points(p));
            }
        }
        let q_by_name: std::collections::HashMap<&str, f64> =
            p.network_sections.iter().filter(|s| s.group.is_none()).map(|s: &NetworkSection| (s.name.as_str(), s.q_c)).collect();
        let level: Vec<SchemaElement> = match &self.active_sub {
            Some(id) => p.network_schema.iter().find(|e| &e.id == id).and_then(|e| e.children.clone()).unwrap_or_default(),
            None => p.network_schema.clone(),
        };
        let sp = |wp: Pos2| self.to_screen(origin, wp);

        for el in &level {
            let sel = self.selected.as_deref() == Some(el.id.as_str());
            let (x, y) = (el.x as f32, el.y as f32);
            let off = Vec2::new(el.label_offset_x.unwrap_or(0.0) as f32, el.label_offset_y.unwrap_or(0.0) as f32);
            match el.kind {
                SchemaKind::Tronson => {
                    let base = parse_color(el.color.as_deref()).unwrap_or(C_TRONSON);
                    let q = if self.active_sub.is_none() { el.label.as_deref().and_then(|l| q_by_name.get(l).copied()) } else { None };
                    draw_tronson(&painter, el, sp, sc, vs, z, mpp, if sel { C_SELECTED } else { base }, sel, q, off);
                }
                SchemaKind::Rezervor | SchemaKind::Sursa => {
                    let color = if sel { C_SELECTED } else if el.kind == SchemaKind::Rezervor { C_RED } else { C_TRONSON };
                    let (w, h) = (el.w.map_or(BOX_W, |v| v as f32) * vs, el.h.map_or(BOX_H, |v| v as f32) * vs);
                    let r = Rect::from_min_max(sp(Pos2::new(x - w / 2.0, y - h)), sp(Pos2::new(x + w / 2.0, y)));
                    painter.rect_filled(r, 0.0, Color32::WHITE);
                    painter.rect_stroke(r, 0.0, Stroke::new(2.0 * sc, color), egui::StrokeKind::Middle);
                    let letter = if el.kind == SchemaKind::Rezervor { "R" } else { "S" };
                    painter.text(r.center(), Align2::CENTER_CENTER, letter, FontId::proportional((h * 0.55 * z).max(4.0)), C_TRONSON);
                    painter.circle_filled(sp(Pos2::new(x + w / 2.0, y)), DOT_R * sc, color);
                    if let Some(l) = &el.label {
                        painter.text(sp(Pos2::new(x, y + DOT_R * vs + 3.0) + off), Align2::CENTER_TOP, l, FontId::proportional(13.0 * sc), color);
                    }
                }
                SchemaKind::Hidrant | SchemaKind::CaminGrav | SchemaKind::CaminAer => {
                    let base = match el.kind {
                        SchemaKind::Hidrant => C_RED,
                        SchemaKind::CaminGrav => C_CAMIN_G,
                        _ => C_CAMIN_A,
                    };
                    let c = if sel { C_SELECTED } else { base };
                    let r = el.r.map_or(CIRCLE_R, |v| v as f32) * vs;
                    let center = sp(Pos2::new(x, y));
                    painter.circle(center, r * z, Color32::WHITE, Stroke::new(2.0 * sc, c));
                    for d in [Vec2::new(-r, 0.0), Vec2::new(r, 0.0), Vec2::new(0.0, -r), Vec2::new(0.0, r)] {
                        painter.circle_filled(center + d * z, 2.5 * sc, c);
                    }
                    if let Some(l) = &el.label {
                        painter.text(sp(Pos2::new(x, y + r + 3.0) + off), Align2::CENTER_TOP, l, FontId::proportional(11.0 * sc), c);
                    }
                }
                SchemaKind::Subsistem => {
                    let (w, h) = (el.w.map_or(SUB_W, |v| v as f32), el.h.map_or(SUB_H, |v| v as f32));
                    let r = Rect::from_min_max(sp(Pos2::new(x - w / 2.0, y - h / 2.0)), sp(Pos2::new(x + w / 2.0, y + h / 2.0)));
                    let brd = if sel { C_SELECTED } else { C_SUB_BRD };
                    painter.rect(r, CornerRadius::same((8.0 * z).min(40.0) as u8), C_SUB_FILL, Stroke::new(2.0 * sc, brd), egui::StrokeKind::Middle);
                    if let Some(l) = &el.label {
                        painter.text(sp(Pos2::new(x, y - h / 2.0 + 8.0) + off), Align2::CENTER_TOP, l, FontId::proportional(11.0 * sc), brd);
                    }
                    for (pp, left) in sub_ports(el) {
                        let s = sp(pp);
                        painter.rect_filled(Rect::from_center_size(s, Vec2::splat(5.0 * sc)), 0.0, brd);
                        let dx = if left { -8.0 } else { 8.0 } * sc;
                        painter.line_segment([s, s + Vec2::new(dx, 0.0)], Stroke::new(1.5 * sc, brd));
                    }
                    let n = el.children.as_ref().map_or(0, |c| c.iter().filter(|e| e.kind == SchemaKind::Tronson).count());
                    let msg = if n > 0 { format!("{n} tronson{}", if n > 1 { "e" } else { "" }) } else { "dublu-click pt. a edita".into() };
                    painter.text(sp(Pos2::new(x, y + 6.0)), Align2::CENTER_CENTER, msg, FontId::proportional(9.0 * sc), C_SUB_TEXT);
                }
                SchemaKind::Text => {
                    if let Some(txt) = &el.text {
                        let fs = el.font_size.unwrap_or(12.0) as f32 * sc;
                        let c = if sel { C_SELECTED } else { parse_color(el.color.as_deref()).unwrap_or(C_TRONSON) };
                        let r = painter.text(sp(Pos2::new(x, y) + off), Align2::LEFT_TOP, txt, FontId::proportional(fs), c);
                        if sel {
                            painter.rect_stroke(r.expand(2.0), 0.0, Stroke::new(1.0, C_SELECTED), egui::StrokeKind::Outside);
                        }
                    }
                }
            }
        }

        // Mânerul de redimensionare al elementului selectat
        if let Some(h) = self.selected.as_ref().and_then(|id| level.iter().find(|e| &e.id == id)).and_then(|e| resize_handle(e, vs)) {
            if !self.capturing {
                let r = Rect::from_center_size(sp(h), Vec2::splat(9.0));
                painter.rect(r, 0.0, Color32::WHITE, Stroke::new(1.5, C_SELECTED), egui::StrokeKind::Middle);
            }
        }

        // Previzualizarea tronsonului în desenare
        if let Drag::Draw { start, end } = &self.drag {
            let preview = SchemaElement {
                kind: SchemaKind::Tronson,
                x: start.x as f64,
                y: start.y as f64,
                x2: Some(end.x as f64),
                y2: Some(end.y as f64),
                ..Default::default()
            };
            draw_tronson(&painter, &preview, sp, sc, vs, z, mpp, rgb(0x94A3B8), false, None, Vec2::ZERO);
            let len = start.distance(*end) as f64 * mpp;
            painter.text(sp(*end) + Vec2::new(10.0, -10.0), Align2::LEFT_BOTTOM, format_dist(len), FontId::proportional(12.0), C_SNAP);
        }

        // Indicator de snap + coordonate cursor (nu și pe captura PNG)
        if let Some(h) = resp.hover_pos().filter(|_| !self.capturing) {
            let wp = self.to_world(origin, h);
            if matches!(self.tool, Tool::Tronson) || matches!(self.drag, Drag::Move { hit: Hit::Start | Hit::End, .. }) {
                let exclude = if let Drag::Move { id, .. } = &self.drag { Some(id.as_str()) } else { None };
                let (s, snapped) = self.snap(p, &level, wp, exclude);
                if snapped {
                    painter.circle_stroke(sp(s), 7.0, Stroke::new(2.0, C_SNAP));
                }
            }
            let txt = format!("x {}   y {}", format_dist(wp.x as f64 * mpp), format_dist(wp.y as f64 * mpp));
            painter.text(rect.left_bottom() + Vec2::new(10.0, -8.0), Align2::LEFT_BOTTOM, txt, FontId::monospace(11.0), rgb(0x64748B));
        }
        painter.text(rect.right_bottom() + Vec2::new(-10.0, -8.0), Align2::RIGHT_BOTTOM, format!("1:{}", scale_ratio(p)), FontId::monospace(11.0), rgb(0x64748B));
    }

    fn properties(&mut self, ui: &mut Ui, t: &I18n, p: &mut ProjectData) {
        let Some(id) = self.selected.clone() else { return };
        let mpp = meters_per_px(scale_ratio(p));
        // Porturile implicite ale unui subsistem se completează fără intrare în istoric.
        if let Some(el) = self.level(p).iter_mut().find(|e| e.id == id && e.kind == SchemaKind::Subsistem && e.ports.is_none()) {
            el.ports = Some(vec![
                serde_json::json!({"id": format!("{}-L", el.id), "side": "L"}),
                serde_json::json!({"id": format!("{}-R", el.id), "side": "R"}),
            ]);
        }
        let before = p.network_schema.clone();
        let mut delete = false;
        let pl = pal(ui);
        {
            let level = self.level(p);
            let Some(el) = level.iter_mut().find(|e| e.id == id) else {
                self.selected = None;
                return;
            };
            ui.add_space(6.0);
            w::soft_box(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(t.t("step8.selected")).size(11.0).strong().color(pl.muted));
                    if el.kind == SchemaKind::Text {
                        let lbl = ui.label(RichText::new("Text").size(12.0).color(pl.muted)).id;
                        let txt = el.text.get_or_insert_with(String::new);
                        ui.add(egui::TextEdit::singleline(txt).desired_width(220.0)).labelled_by(lbl);
                        let mut fs = el.font_size.unwrap_or(12.0);
                        let lbl = ui.label(RichText::new(t.t("canvas.fontSize")).size(12.0).color(pl.muted)).id;
                        if ui.add(egui::DragValue::new(&mut fs).range(6.0..=96.0)).labelled_by(lbl).changed() {
                            el.font_size = Some(fs);
                        }
                    } else {
                        let lbl = ui.label(RichText::new(t.t("native.labelField")).size(12.0).color(pl.muted)).id;
                        let mut label = el.label.clone().unwrap_or_default();
                        if ui.add(egui::TextEdit::singleline(&mut label).desired_width(110.0)).labelled_by(lbl).changed() {
                            el.label = Some(label);
                        }
                    }
                    if el.kind == SchemaKind::Tronson {
                        let (ex, ey) = end_of(el);
                        let drawn = ((ex as f64 - el.x).hypot(ey as f64 - el.y)) * mpp;
                        ui.label(RichText::new(format!("{} {}", t.t("native.drawnLength"), format_dist(drawn))).size(12.0).color(pl.muted));
                        let lbl = ui.label(RichText::new(t.t("native.overrideLength")).size(12.0).color(pl.muted)).id;
                        let mut ov = el.override_length.unwrap_or(0.0);
                        if ui.add(egui::DragValue::new(&mut ov).range(0.0..=1.0e6).speed(0.5).suffix(" m")).labelled_by(lbl).changed() {
                            el.override_length = (ov > 0.0).then_some(ov);
                        }
                    }
                    if matches!(el.kind, SchemaKind::Tronson | SchemaKind::Text) {
                        for (hex, key) in COLOR_PRESETS {
                            let c = parse_color(Some(hex)).unwrap_or(C_TRONSON);
                            let cur = el.color.as_deref().unwrap_or("#1a1a1a") == *hex;
                            let (r, resp) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                            ui.painter().circle(r.center(), 7.0, c, Stroke::new(if cur { 2.5 } else { 1.0 }, if cur { C_SELECTED } else { Color32::WHITE }));
                            if resp.on_hover_text(t.t(&format!("step8.{key}"))).clicked() {
                                el.color = (*hex != "#1a1a1a").then(|| hex.to_string());
                            }
                        }
                    }
                    if el.kind == SchemaKind::Subsistem {
                        ports_editor(ui, t, el);
                    }
                    if el.label_offset_x.is_some() && ui.small_button(t.t("native.resetLabel")).clicked() {
                        el.label_offset_x = None;
                        el.label_offset_y = None;
                    }
                    if w::danger_button(ui, &t.t("common.delete"), false).clicked() {
                        delete = true;
                    }
                });
            });
        }
        if delete {
            self.delete_selected(p);
        } else if before != p.network_schema && !ui.ctx().egui_is_using_pointer() {
            // Modificările din panoul de proprietăți intră în istoric (o intrare pe modificare).
            self.undo.push(before);
            if self.undo.len() > UNDO_MAX {
                self.undo.remove(0);
            }
        }
    }
}

/// Porturile unui subsistem: intrări (stânga) și ieșiri (dreapta), cu etichete.
fn ports_editor(ui: &mut Ui, t: &I18n, el: &mut SchemaElement) {
    let ports = el.ports.get_or_insert_with(|| {
        vec![
            serde_json::json!({"id": format!("{}-L", el.id), "side": "L"}),
            serde_json::json!({"id": format!("{}-R", el.id), "side": "R"}),
        ]
    });
    ui.end_row();
    ui.vertical(|ui| {
        ui.label(RichText::new(t.t("canvas.ports")).strong().size(12.0));
        let mut remove = None;
        for (side, title, empty) in [("L", "canvas.portsLeft", "canvas.noInputs"), ("R", "canvas.portsRight", "canvas.noOutputs")] {
            let title_id = ui
                .horizontal(|ui| {
                    let id = ui.label(RichText::new(t.t(title)).size(12.0)).id;
                    if ui.small_button(format!("+ {}", t.t("canvas.addPort"))).clicked() {
                        ports.push(serde_json::json!({"id": uuid::Uuid::new_v4().to_string(), "side": side}));
                    }
                    id
                })
                .inner;
            let mut any = false;
            for (i, port) in ports.iter_mut().enumerate().filter(|(_, p)| p.get("side").and_then(Value::as_str) == Some(side)) {
                any = true;
                ui.horizontal(|ui| {
                    let mut label = port.get("label").and_then(Value::as_str).unwrap_or("").to_string();
                    let resp = ui.add(egui::TextEdit::singleline(&mut label).hint_text(t.t("canvas.portLabel")).desired_width(140.0));
                    let resp = resp.labelled_by(title_id);
                    if resp.changed() {
                        if let Some(o) = port.as_object_mut() {
                            if label.is_empty() {
                                o.remove("label");
                            } else {
                                o.insert("label".into(), Value::String(label));
                            }
                        }
                    }
                    if ui.small_button("✕").on_hover_text(t.t("common.delete")).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if !any {
                ui.label(RichText::new(t.t(empty)).size(11.0).italics());
            }
        }
        if let Some(i) = remove {
            ports.remove(i);
        }
    });
}

/// Zona `r` (în puncte) din captura ecranului, ca RGBA; întoarce și dimensiunile.
fn crop_rgba(img: &egui::ColorImage, r: Rect, ppp: f32) -> (Vec<u8>, usize, usize) {
    let (x0, y0) = ((r.min.x * ppp).round().max(0.0) as usize, (r.min.y * ppp).round().max(0.0) as usize);
    let w = ((r.width() * ppp).round() as usize).min(img.size[0].saturating_sub(x0));
    let h = ((r.height() * ppp).round() as usize).min(img.size[1].saturating_sub(y0));
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            rgba.extend_from_slice(&img.pixels[y * img.size[0] + x].to_array());
        }
    }
    (rgba, w, h)
}

#[allow(clippy::too_many_arguments)]
fn draw_tronson(
    painter: &egui::Painter,
    el: &SchemaElement,
    sp: impl Fn(Pos2) -> Pos2,
    sc: f32,
    vs: f32,
    zoom: f32,
    mpp: f64,
    color: Color32,
    selected: bool,
    q: Option<f64>,
    off: Vec2,
) {
    let a = Pos2::new(el.x as f32, el.y as f32);
    let (ex, ey) = end_of(el);
    let b = Pos2::new(ex, ey);
    let d = b - a;
    let len = d.length();
    if len < 2.0 {
        return;
    }
    let (sa, sb) = (sp(a), sp(b));
    painter.line_segment([sa, sb], Stroke::new(2.0 * sc, color));
    // Săgeata de sens, la mijloc
    let mid = sa + (sb - sa) / 2.0;
    let ang = d.y.atan2(d.x);
    let s = ARROW_SZ * sc;
    let sp6 = std::f32::consts::PI / 6.0;
    let p1 = mid - Vec2::new((ang - sp6).cos(), (ang - sp6).sin()) * s;
    let p2 = mid - Vec2::new((ang + sp6).cos(), (ang + sp6).sin()) * s;
    painter.add(Shape::convex_polygon(vec![mid, p1, p2], color, Stroke::NONE));
    painter.circle_filled(sa, DOT_R * sc, color);
    painter.circle_filled(sb, DOT_R * sc, color);

    if let Some(label) = &el.label {
        let perp = Vec2::new(-d.y / len, d.x / len);
        let step = 15.0 * vs;
        let wmid = a + d / 2.0;
        let name_pos = sp(wmid + perp * step + off);
        let dim_pos = sp(wmid - perp * step + off);
        let shown = el.override_length.unwrap_or(len as f64 * mpp);
        let name_color = if selected { C_SELECTED } else { parse_color(el.color.as_deref()).unwrap_or(C_TR_LABEL) };
        // Textul e orizontal: pe tronsoane verticale îl aliniem dinspre linie spre exterior.
        let side = |v: f32| if perp.x.abs() > 0.7 { if v < 0.0 { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER } } else { Align2::CENTER_CENTER };
        painter.text(name_pos, side(perp.x), label, FontId::proportional(13.0 * sc), name_color);
        painter.text(dim_pos, side(-perp.x), format_dist(shown), FontId::proportional(12.0 * sc), if selected { C_SELECTED } else { C_TRONSON });
        if let Some(q) = q.filter(|q| *q > 0.0) {
            // Pe tronsoanele aproape verticale, Q stă sub nume (textul e orizontal).
            let q_pos = if perp.x.abs() > 0.7 { name_pos + Vec2::new(0.0, 14.0 * sc) } else { sp(wmid + perp * step * 2.0 + off) };
            let align = if perp.x.abs() > 0.7 { if perp.x < 0.0 { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER } } else { Align2::CENTER_CENTER };
            painter.text(q_pos, align, format!("Q={q:.2} l/s"), FontId::proportional(11.0 * sc), C_Q);
        }
    }
    let _ = zoom;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;
    use egui_kittest::Harness;

    struct St {
        c: CanvasUi,
        p: ProjectData,
        t: I18n,
        themed: bool,
    }

    fn harness() -> Harness<'static, St> {
        // zoom 1, pan fix → coordonate previzibile
        let c = CanvasUi { fitted: true, pan: Vec2::ZERO, ..Default::default() };
        Harness::builder().with_size(Vec2::new(1000.0, 800.0)).build_ui_state(
            |ui, s: &mut St| {
                if !s.themed {
                    crate::theme::install(ui.ctx());
                    s.themed = true;
                }
                s.c.ui(ui, &s.t, &mut s.p)
            },
            St { c, p: ProjectData::default(), t: I18n::new(Lang::Ro), themed: false },
        )
    }

    /// Originea canvas-ului pe ecran = colțul stânga-sus al zonei de desen.
    fn origin(h: &mut Harness<'static, St>) -> Pos2 {
        // Canvas-ul începe sub cele două rânduri de unelte; îl găsim după primul element.
        h.run();
        let s = h.state();
        let _ = s;
        Pos2::new(8.0, 0.0)
    }

    fn drag(h: &mut Harness<'static, St>, a: Pos2, b: Pos2) {
        h.hover_at(a);
        h.run();
        h.drag_at(a);
        h.run();
        for i in 1..=5 {
            h.hover_at(a + (b - a) * (i as f32 / 5.0));
            h.run();
        }
        h.drop_at(b);
        h.run();
    }

    #[test]
    fn crop_takes_canvas_area_in_physical_pixels() {
        let mut img = egui::ColorImage::filled([20, 10], Color32::BLACK);
        img.pixels[4 * 20 + 6] = Color32::RED; // (6, 4) în pixeli fizici
        let (rgba, w, h) = crop_rgba(&img, Rect::from_min_size(Pos2::new(3.0, 2.0), Vec2::new(4.0, 3.0)), 2.0);
        assert_eq!((w, h), (8, 6));
        assert_eq!(&rgba[..4], &Color32::RED.to_array());
        // dreptunghi parțial în afara imaginii → tăiat la margine
        let (_, w, h) = crop_rgba(&img, Rect::from_min_size(Pos2::new(8.0, 4.0), Vec2::new(10.0, 10.0)), 2.0);
        assert_eq!((w, h), (4, 2));
    }

    #[test]
    fn draw_tronson_place_hydrant_snap_and_undo() {
        let mut h = harness();
        let _ = origin(&mut h);
        let top = 140.0; // sub bara de unelte
        h.state_mut().c.tool = Tool::Tronson;
        drag(&mut h, Pos2::new(100.0, top + 100.0), Pos2::new(400.0, top + 100.0));
        {
            let s = h.state();
            assert_eq!(s.p.network_schema.len(), 1, "tronsonul trebuie creat prin tragere");
            let tr = &s.p.network_schema[0];
            assert_eq!(tr.kind, SchemaKind::Tronson);
            assert_eq!(tr.label.as_deref(), Some("Tr 1"));
            assert!((tr.x2.unwrap() - tr.x - 300.0).abs() < 1.0);
        }
        // Al doilea tronson pornește lângă capătul primului → prins exact în capăt.
        let (ex, ey) = {
            let tr = &h.state().p.network_schema[0];
            (tr.x2.unwrap() as f32, tr.y2.unwrap() as f32)
        };
        let o = {
            let tr = &h.state().p.network_schema[0];
            Pos2::new(400.0 - tr.x2.unwrap() as f32, top + 100.0 - tr.y2.unwrap() as f32)
        };
        set_setting(&mut h.state_mut().p, "snapToEndpoints", json!(true));
        h.state_mut().c.tool = Tool::Tronson;
        drag(&mut h, o + Vec2::new(ex + 5.0, ey + 4.0), o + Vec2::new(ex + 200.0, ey + 150.0));
        {
            let s = h.state();
            assert_eq!(s.p.network_schema.len(), 2);
            let t2 = &s.p.network_schema[1];
            assert_eq!((t2.x as f32, t2.y as f32), (ex, ey), "snap la capătul tronsonului anterior");
            assert_eq!(t2.label.as_deref(), Some("Tr 2"));
        }
        // Hidrant plasat cu click.
        h.state_mut().c.tool = Tool::Hidrant;
        let at = o + Vec2::new(200.0, 300.0);
        h.hover_at(at);
        h.run();
        h.drag_at(at);
        h.run();
        h.drop_at(at);
        h.run();
        assert_eq!(h.state().p.network_schema.len(), 3);
        assert_eq!(h.state().p.network_schema[2].kind, SchemaKind::Hidrant);
        assert_eq!(h.state().c.tool, Tool::Select);
        // Redimensionare: hidrantul e selectat; tragem mânerul (vârful cercului) în sus cu 15 px.
        let (hx, hy) = {
            let e = &h.state().p.network_schema[2];
            (e.x as f32, e.y as f32)
        };
        let handle = o + Vec2::new(hx, hy - CIRCLE_R);
        drag(&mut h, handle, handle - Vec2::new(0.0, 15.0));
        let r = h.state().p.network_schema[2].r.unwrap_or(0.0);
        assert!((r - 25.0).abs() < 1.0, "raza după redimensionare: {r}");
        {
            let s = h.state_mut();
            s.c.undo_once(&mut s.p);
        }
        assert_eq!(h.state().p.network_schema[2].r, None, "redimensionarea se poate anula");
        // Subsistem selectat → editorul de porturi se afișează (și trece verificarea de accesibilitate).
        {
            let s = h.state_mut();
            s.p.network_schema.push(SchemaElement { id: "sub".into(), kind: SchemaKind::Subsistem, x: 600.0, y: 300.0, label: Some("Loc 1".into()), ..Default::default() });
            s.c.selected = Some("sub".into());
        }
        h.run();
        assert_eq!(h.state().p.network_schema[3].ports.as_ref().map(Vec::len), Some(2), "porturile implicite L/R");
        h.state_mut().p.network_schema.pop();
        h.state_mut().c.selected = None;
        // Anulare: dispare hidrantul.
        {
            let s = h.state_mut();
            s.c.undo_once(&mut s.p);
        }
        assert_eq!(h.state().p.network_schema.len(), 2);
    }
}

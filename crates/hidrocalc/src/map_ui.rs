//! Harta OSM de fundal, punctele topo pe plan și fereastra de setări a hărții
//! (ancorare cu căutare Nominatim, opacitate, rotire, cote teren, import topo).

use crate::i18n::I18n;
use crate::tiles::{TileCache, USER_AGENT};
use crate::widgets::{self as w, pal};
use eframe::egui::{self, Align2, Color32, FontId, Mesh, Pos2, Rect, RichText, Stroke, Ui, Vec2};
use hidro_core::geo::{self, compute_tile_zoom, lat_to_osm_py, latlng_to_world, lng_to_osm_px, MapAnchor, TILE_SIZE};
use hidro_core::model::ProjectData;
use hidro_core::topo::{self, Crs, TopoPoint};
use serde_json::{json, Value};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

const TOPO_COLOR: Color32 = Color32::from_rgb(0x4E, 0x8F, 0x6E);

// ─── Setări din `networkCanvasSettings` ──────────────────────────────────────

pub fn anchor(p: &ProjectData) -> Option<MapAnchor> {
    p.network_canvas_settings.get("mapAnchor").and_then(|v| serde_json::from_value(v.clone()).ok())
}
pub fn rotation(p: &ProjectData) -> f64 {
    p.network_canvas_settings.get("mapRotation").and_then(Value::as_f64).unwrap_or(0.0)
}
pub fn opacity(p: &ProjectData) -> f64 {
    p.network_canvas_settings.get("mapOpacity").and_then(Value::as_f64).unwrap_or(0.7)
}
fn flag(p: &ProjectData, key: &str) -> bool {
    p.network_canvas_settings.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Vederea folosită pentru hartă: cu blocare, zoom-ul/pan-ul salvate la blocare
/// înlocuiesc pe cele curente (harta stă pe loc, schema se mișcă peste ea).
pub fn map_view(p: &ProjectData, live: View) -> View {
    let locked = p.network_canvas_settings.get("mapLockedState");
    let mut v = live;
    if let Some(ls) = locked {
        if flag(p, "mapLockZoom") {
            if let Some(z) = ls.get("zoom").and_then(Value::as_f64) {
                v.zoom = z as f32;
            }
        }
        if flag(p, "mapLockPan") {
            if let (Some(x), Some(y)) = (ls["pan"].get("x").and_then(Value::as_f64), ls["pan"].get("y").and_then(Value::as_f64)) {
                v.pan = Vec2::new(x as f32, y as f32);
            }
        }
    }
    v
}

pub fn auto_terrain(p: &ProjectData) -> bool {
    p.network_canvas_settings.get("autoTerrainCota").and_then(Value::as_bool) != Some(false)
}
pub fn topo_points(p: &ProjectData) -> Vec<TopoPoint> {
    p.network_canvas_settings.get("topoPoints").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
}
pub fn topo_radius(p: &ProjectData) -> f64 {
    p.network_canvas_settings.get("topoRadius").and_then(Value::as_f64).unwrap_or(hidro_core::terrain::DEFAULT_TOPO_RADIUS_M)
}
fn set(p: &mut ProjectData, key: &str, v: Value) {
    if !p.network_canvas_settings.is_object() {
        p.network_canvas_settings = hidro_core::model::default_canvas_settings();
    }
    if let Some(o) = p.network_canvas_settings.as_object_mut() {
        if v.is_null() {
            o.remove(key);
        } else {
            o.insert(key.into(), v);
        }
    }
}

// ─── Desenare ────────────────────────────────────────────────────────────────

/// Transformarea vizualizării: ecran = origine + pan + lume × zoom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub origin: Pos2,
    pub pan: Vec2,
    pub zoom: f32,
}

impl View {
    fn to_screen(self, x: f64, y: f64) -> Pos2 {
        self.origin + self.pan + Vec2::new(x as f32, y as f32) * self.zoom
    }
    fn to_world(self, s: Pos2) -> (f64, f64) {
        let v = (s - self.origin - self.pan) / self.zoom;
        (v.x as f64, v.y as f64)
    }
}

pub fn draw_tiles(painter: &egui::Painter, rect: Rect, view: View, a: &MapAnchor, rot_deg: f64, alpha: f64, cache: &mut TileCache) {
    let eff_z = (a.tile_zoom + (view.zoom as f64).log2()).floor().clamp(0.0, 19.0);
    let ax = lng_to_osm_px(a.lng, eff_z);
    let ay = lat_to_osm_py(a.lat, eff_z);
    let osm_per_world = 2f64.powf(eff_z - a.tile_zoom);
    let (s, c) = rot_deg.to_radians().sin_cos();
    let rotate = |wx: f64, wy: f64| {
        let (dx, dy) = (wx - a.world_x, wy - a.world_y);
        (a.world_x + dx * c - dy * s, a.world_y + dx * s + dy * c)
    };
    let unrotate = |wx: f64, wy: f64| {
        let (dx, dy) = (wx - a.world_x, wy - a.world_y);
        (a.world_x + dx * c + dy * s, a.world_y - dx * s + dy * c)
    };
    let corners: Vec<(f64, f64)> =
        [rect.left_top(), rect.right_top(), rect.left_bottom(), rect.right_bottom()].iter().map(|p| { let (x, y) = view.to_world(*p); unrotate(x, y) }).collect();
    let (wl, wr) = corners.iter().fold((f64::MAX, f64::MIN), |(l, r), p| (l.min(p.0), r.max(p.0)));
    let (wt, wb) = corners.iter().fold((f64::MAX, f64::MIN), |(t, b), p| (t.min(p.1), b.max(p.1)));
    let to_tile = |w: f64, anchor_w: f64, anchor_osm: f64| ((anchor_osm + (w - anchor_w) * osm_per_world) / TILE_SIZE).floor() as i64;
    let (tl, tr) = (to_tile(wl, a.world_x, ax) - 1, to_tile(wr, a.world_x, ax) + 1);
    let (tt, tb) = (to_tile(wt, a.world_y, ay) - 1, to_tile(wb, a.world_y, ay) + 1);
    let max = 2i64.pow(eff_z as u32);
    if (tr - tl) * (tb - tt) > 600 {
        return; // zoom prea mic pentru nivelul de dale: evităm mii de cereri
    }
    let tint = Color32::from_white_alpha((alpha.clamp(0.0, 1.0) * 255.0) as u8);
    let z = eff_z as u8;
    for tx in tl..=tr {
        for ty in tt..=tb {
            if tx < 0 || ty < 0 || tx >= max || ty >= max {
                continue;
            }
            let wx0 = a.world_x + (tx as f64 * TILE_SIZE - ax) / osm_per_world;
            let wy0 = a.world_y + (ty as f64 * TILE_SIZE - ay) / osm_per_world;
            let size = TILE_SIZE / osm_per_world;
            let quad = [(wx0, wy0), (wx0 + size, wy0), (wx0 + size, wy0 + size), (wx0, wy0 + size)].map(|(x, y)| {
                let (rx, ry) = rotate(x, y);
                view.to_screen(rx, ry)
            });
            let key = (z, tx as u32, ty as u32);
            let (tex, uv) = match cache.get(key) {
                Some(t) => (Some(t), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))),
                None if z > 0 => {
                    let parent = cache.peek((z - 1, (tx / 2) as u32, (ty / 2) as u32));
                    let (ox, oy) = ((tx % 2) as f32 * 0.5, (ty % 2) as f32 * 0.5);
                    (parent, Rect::from_min_size(Pos2::new(ox, oy), Vec2::splat(0.5)))
                }
                None => (None, Rect::NOTHING),
            };
            if let Some(tex) = tex {
                let mut mesh = Mesh::with_texture(tex);
                let uvs = [uv.left_top(), uv.right_top(), uv.right_bottom(), uv.left_bottom()];
                for (p, u) in quad.iter().zip(uvs) {
                    mesh.vertices.push(egui::epaint::Vertex { pos: *p, uv: u, color: tint });
                }
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                painter.add(egui::Shape::mesh(mesh));
            }
        }
    }
    let attrib = "© OpenStreetMap contributors";
    let pos = rect.right_top() + Vec2::new(-8.0, 8.0);
    let galley = painter.layout_no_wrap(attrib.into(), FontId::proportional(10.0), Color32::from_gray(50));
    let bg = Rect::from_min_size(pos - Vec2::new(galley.size().x, 0.0), galley.size()).expand(3.0);
    painter.rect_filled(bg, 3.0, Color32::from_white_alpha(210));
    painter.galley(bg.min + Vec2::splat(3.0), galley, Color32::from_gray(50));
}

pub fn draw_topo(painter: &egui::Painter, view: View, a: &MapAnchor, rot: f64, pts: &[TopoPoint]) {
    for p in pts {
        let (x, y) = latlng_to_world(p.lat, p.lng, a, rot);
        let s = view.to_screen(x, y);
        let r = 4.0;
        let st = Stroke::new(1.5, TOPO_COLOR);
        painter.line_segment([s + Vec2::new(-r, -r), s + Vec2::new(r, r)], st);
        painter.line_segment([s + Vec2::new(-r, r), s + Vec2::new(r, -r)], st);
        if view.zoom > 0.4 {
            let label = format!("{:.2}", p.z);
            let pos = s + Vec2::new(r * 1.3, -r * 0.3);
            for d in [Vec2::new(1.0, 0.0), Vec2::new(-1.0, 0.0), Vec2::new(0.0, 1.0), Vec2::new(0.0, -1.0)] {
                painter.text(pos + d, Align2::LEFT_BOTTOM, &label, FontId::proportional(10.0), Color32::WHITE);
            }
            painter.text(pos, Align2::LEFT_BOTTOM, &label, FontId::proportional(10.0), TOPO_COLOR);
        }
    }
}

// ─── Cereri HTTP (Nominatim) ─────────────────────────────────────────────────

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn get_json(url: &str) -> Result<Value, String> {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .user_agent(USER_AGENT)
        .build()
        .get(url)
        .set("Accept-Language", "ro,en")
        .call()
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|e| e.to_string())
        .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
}

fn spawn<T: Send + 'static>(ctx: &egui::Context, f: impl FnOnce() -> T + Send + 'static) -> Receiver<T> {
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(f());
        ctx.request_repaint();
    });
    rx
}

/// Informațiile de localitate pentru o coordonată (căutare inversă Nominatim).
fn reverse_geocode(lat: f64, lng: f64) -> Option<Value> {
    let d = get_json(&format!("https://nominatim.openstreetmap.org/reverse?lat={lat}&lon={lng}&format=json&addressdetails=1")).ok()?;
    let a = d.get("address")?;
    let pick = |keys: &[&str]| keys.iter().find_map(|k| a.get(*k).and_then(Value::as_str)).unwrap_or("").to_string();
    let locality = pick(&["village", "town", "city", "municipality"]);
    let county = pick(&["county", "state"]).trim_start_matches("Județul ").trim_start_matches("Judetul ").to_string();
    Some(json!({ "locality": locality, "county": county, "country": pick(&["country"]) }))
}

// ─── Fereastra hărții ────────────────────────────────────────────────────────

/// Rezultatele căutării: (denumire, lat, lng).
type SearchHits = Vec<(String, f64, f64)>;

#[derive(Default)]
pub struct MapWindow {
    pub open: bool,
    query: String,
    lat: String,
    lng: String,
    error: Option<String>,
    results: SearchHits,
    search: Option<Receiver<Result<SearchHits, String>>>,
    reverse: Option<Receiver<Option<Value>>>,
    crs: Option<Crs>,
    pub notice: Option<String>,
    /// Centrul și transformarea vederii canvasului, actualizate la fiecare cadru de canvas.
    pub view: Option<((f64, f64), View)>,
}

impl MapWindow {
    pub fn open_for(&mut self, p: &ProjectData) {
        self.open = true;
        if let Some(a) = anchor(p) {
            self.lat = format!("{:.6}", a.lat);
            self.lng = format!("{:.6}", a.lng);
        }
    }

    /// Rezultatele cererilor în fundal (căutare, localitate).
    pub fn poll(&mut self, p: &mut ProjectData) {
        if let Some(rx) = &self.search {
            if let Ok(res) = rx.try_recv() {
                self.search = None;
                match res {
                    Ok(r) => self.results = r,
                    Err(e) => self.error = Some(e),
                }
            }
        }
        if let Some(rx) = &self.reverse {
            if let Ok(info) = rx.try_recv() {
                self.reverse = None;
                if let Some(info) = info {
                    set(p, "mapLocationInfo", info);
                }
            }
        }
    }

    /// Panoul hărții, afișat în bara laterală (în locul ferestrei plutitoare).
    pub fn panel(&mut self, ui: &mut Ui, t: &I18n, p: &mut ProjectData) {
        if !self.open {
            return;
        }
        let Some((center, live)) = self.view else { return };
        ui.add_space(6.0);
        self.anchor_section(ui, t, p, center);
        if anchor(p).is_some() {
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(4.0);
            self.display_section(ui, t, p, live);
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(4.0);
            self.topo_section(ui, t, p);
        }
        ui.add_space(8.0);
    }

    fn anchor_section(&mut self, ui: &mut Ui, t: &I18n, p: &mut ProjectData, center: (f64, f64)) {
        let pl = pal(ui);
        ui.label(RichText::new(t.t("native.mapSearch")).size(12.5).color(pl.ink2));
        let mut go = false;
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.query).hint_text("ex: Timișoara…").desired_width(ui.available_width() - 78.0));
            go = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.add_enabled(self.search.is_none() && self.query.trim().len() >= 2, egui::Button::new(t.t("native.search"))).clicked() {
                go = true;
            }
        });
        if go && self.query.trim().len() >= 2 {
            let q = url_encode(self.query.trim());
            self.results.clear();
            self.error = None;
            self.search = Some(spawn(ui.ctx(), move || {
                let d = get_json(&format!("https://nominatim.openstreetmap.org/search?q={q}&format=json&limit=5&addressdetails=0"))?;
                Ok(d.as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|r| {
                                let lat = r.get("lat")?.as_str()?.parse().ok()?;
                                let lon = r.get("lon")?.as_str()?.parse().ok()?;
                                Some((r.get("display_name")?.as_str()?.to_string(), lat, lon))
                            })
                            .collect()
                    })
                    .unwrap_or_default())
            }));
        }
        if self.search.is_some() {
            ui.add_space(4.0);
            ui.spinner();
        }
        // Sugestiile: rânduri pe toată lățimea, cu același aspect ca un câmp.
        let mut picked = None;
        for (name, lat, lng) in &self.results {
            ui.add_space(3.0);
            let b = egui::Button::new(RichText::new(name).size(12.0).color(pl.ink)).wrap().min_size(egui::vec2(ui.available_width(), 0.0));
            if ui.add(b).clicked() {
                picked = Some((name.clone(), *lat, *lng));
            }
        }
        if let Some((name, lat, lng)) = picked {
            self.lat = format!("{lat:.6}");
            self.lng = format!("{lng:.6}");
            self.query = name.split(',').take(2).collect::<Vec<_>>().join(",");
            self.results.clear();
        }
        ui.add_space(8.0);
        ui.columns(2, |c| {
            w::text_field(&mut c[0], &t.t("native.latitude"), &mut self.lat, "45.748900");
            w::text_field(&mut c[1], &t.t("native.longitude"), &mut self.lng, "21.208700");
        });
        if let Some(e) = &self.error {
            ui.label(RichText::new(e).size(12.0).color(pl.danger));
        }
        ui.add_space(8.0);
        let full = egui::vec2(ui.available_width(), 28.0);
        if ui.add_sized(full, egui::Button::new(RichText::new(t.t("native.mapConfirm")).color(pl.accent_fg)).fill(pl.accent).stroke(egui::Stroke::NONE)).clicked() {
            match (w::parse_decimal(&self.lat), w::parse_decimal(&self.lng)) {
                (Some(lat), Some(lng)) if lat.abs() <= 90.0 && lng.abs() <= 180.0 => {
                    let sr = crate::canvas::scale_ratio(p);
                    let a = MapAnchor { lat, lng, world_x: center.0, world_y: center.1, tile_zoom: compute_tile_zoom(lat, sr) };
                    set(p, "mapAnchor", serde_json::to_value(a).unwrap_or(Value::Null));
                    set(p, "mapLocationInfo", Value::Null);
                    self.error = None;
                    self.reverse = Some(spawn(ui.ctx(), move || reverse_geocode(lat, lng)));
                }
                _ => self.error = Some(t.t("native.invalidCoords")),
            }
        }
        if anchor(p).is_some() {
            ui.add_space(4.0);
            if ui.add_sized(full, egui::Button::new(RichText::new(t.t("native.mapDisable")).color(pl.ink2))).clicked() {
                set(p, "mapAnchor", Value::Null);
                set(p, "mapLocationInfo", Value::Null);
            }
        }
    }

    fn display_section(&mut self, ui: &mut Ui, t: &I18n, p: &mut ProjectData, live: View) {
        let pl = pal(ui);
        ui.label(RichText::new(t.t("native.mapLock")).size(12.5).color(pl.ink2));
        ui.add_space(2.0);
        ui.columns(3, |c| {
            for (i, (key, label)) in [("mapLockPan", "native.lockPan"), ("mapLockZoom", "native.lockZoom"), ("mapLockRotation", "native.lockRotation")].into_iter().enumerate() {
                let on = flag(p, key);
                if w::tool_button_w(&mut c[i], &t.t(label), on).clicked() {
                    let on = !on;
                    set(p, key, json!(on));
                    let pan_or_zoom = flag(p, "mapLockPan") || flag(p, "mapLockZoom");
                    if on && key != "mapLockRotation" {
                        // starea vederii din momentul blocării
                        set(p, "mapLockedState", json!({ "zoom": live.zoom, "pan": { "x": live.pan.x, "y": live.pan.y } }));
                    } else if !pan_or_zoom {
                        set(p, "mapLockedState", Value::Null);
                    }
                }
            }
        });
        ui.add_space(10.0);
        // Glisoarele ocupă toată lățimea; valoarea apare în dreapta lor.
        ui.spacing_mut().slider_width = (ui.available_width() - 64.0).max(60.0);
        let mut op = opacity(p);
        ui.label(RichText::new(t.t("native.mapOpacity")).size(12.5).color(pl.ink2));
        if ui.add(egui::Slider::new(&mut op, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0}%", v * 100.0))).changed() {
            set(p, "mapOpacity", json!(op));
        }
        ui.add_space(8.0);
        let mut rot = rotation(p);
        let unlocked = !flag(p, "mapLockRotation");
        ui.horizontal(|ui| {
            ui.label(RichText::new(t.t("native.mapRotation")).size(12.5).color(pl.ink2));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(unlocked, egui::Button::new(RichText::new(t.t("canvas.resetRotation")).size(12.0)).small()).clicked() {
                    set(p, "mapRotation", json!(0.0));
                }
            });
        });
        if ui.add_enabled(unlocked, egui::Slider::new(&mut rot, -180.0..=180.0).suffix("°").step_by(0.5)).changed() {
            set(p, "mapRotation", json!(rot));
        }
        ui.add_space(10.0);
        let mut auto = auto_terrain(p);
        if ui.checkbox(&mut auto, RichText::new(t.t("terrain.autoToggle")).size(12.5)).changed() {
            set(p, "autoTerrainCota", json!(auto));
        }
        ui.add(egui::Label::new(RichText::new(t.t("terrain.attribution")).size(11.0).color(pl.muted)).wrap());
        ui.add_space(8.0);
        let mut depth = hidro_core::export::pipe_depth(p);
        if w::num_field(ui, &t.t("native.pipeDepth"), &mut depth, Some("m"), None) {
            set(p, "pipeDepth", json!(depth.clamp(0.3, 5.0)));
        }
    }

    fn topo_section(&mut self, ui: &mut Ui, t: &I18n, p: &mut ProjectData) {
        let pl = pal(ui);
        let pts = topo_points(p);
        w::section_title(ui, &t.t("topo.title"));
        ui.add(egui::Label::new(RichText::new(t.t("topo.formats")).size(11.0).color(pl.muted)).wrap());
        ui.label(RichText::new(t.tf("native.topoCount", &[("n", &pts.len().to_string())])).size(12.0).color(pl.muted));
        ui.add_space(6.0);
        let crs_opts = [
            (Crs::Auto, "topo.crsAuto"),
            (Crs::Stereo70, "topo.crsStereo70"),
            (Crs::Stereo70En, "topo.crsStereo70en"),
            (Crs::Wgs84, "topo.crsWgs84"),
        ];
        let cur = self.crs.unwrap_or(Crs::Auto);
        let options: Vec<(String, String)> = crs_opts.iter().map(|(c, k)| (format!("{}", *c as u8), t.t(k))).collect();
        let mut sel = format!("{}", cur as u8);
        if w::combo(ui, &t.t("topo.crs"), &mut sel, &options) {
            if let Some((c, _)) = crs_opts.iter().find(|(c, _)| format!("{}", *c as u8) == sel) {
                self.crs = Some(*c);
            }
        }
        ui.add_space(6.0);
        let mut radius = topo_radius(p);
        if w::num_field(ui, &t.t("topo.radius"), &mut radius, Some("m"), None) {
            set(p, "topoRadius", json!(radius.clamp(0.5, 100.0)));
        }
        ui.add_space(8.0);
        let full = egui::vec2(ui.available_width(), 28.0);
        if ui.add_sized(full, egui::Button::new(RichText::new(t.t("topo.chooseFile")).color(pl.ink2))).clicked() {
            self.import_topo(t, p);
        }
        if !pts.is_empty() {
            ui.add_space(4.0);
            if ui.add_sized(full, egui::Button::new(RichText::new(t.t("topo.clear")).color(pl.ink2))).clicked() {
                set(p, "topoPoints", Value::Null);
                // Cotele preluate din topo devin din nou eligibile pentru DEM.
                for s in &mut p.network_sections {
                    if s.cota_up_source.as_deref() == Some("topo") {
                        s.cota_up = None;
                        s.cota_up_source = None;
                    }
                    if s.cota_down_source.as_deref() == Some("topo") {
                        s.cota_down = None;
                        s.cota_down_source = None;
                    }
                }
            }
        }
        if let Some(n) = &self.notice {
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(n).size(12.0).color(pl.ink2)).wrap());
        }
    }

    fn import_topo(&mut self, t: &I18n, p: &mut ProjectData) {
        let Some(path) = rfd::FileDialog::new().add_filter("Topo", &["csv", "txt", "gpx", "kml"]).pick_file() else { return };
        let text = match std::fs::read(&path) {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) => {
                self.notice = Some(e.to_string());
                return;
            }
        };
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let res = topo::parse_file(&name, &text, self.crs.unwrap_or(Crs::Auto));
        let mut pts = topo_points(p);
        for (name, ll, z) in &res.points {
            pts.push(TopoPoint { id: uuid::Uuid::new_v4().to_string(), lat: ll.lat, lng: ll.lng, z: *z, name: name.clone() });
        }
        set(p, "topoPoints", serde_json::to_value(&pts).unwrap_or(Value::Null));
        self.notice = Some(t.tf(
            "native.topoImported",
            &[
                ("n", &res.points.len().to_string()),
                ("noz", &res.skipped_no_z.to_string()),
                ("bad", &res.skipped_invalid.to_string()),
            ],
        ));
    }
}

/// Bara de informații de sub canvas (localitate, coordonate, scară).
pub fn info_bar(ui: &mut Ui, t: &I18n, p: &ProjectData) {
    let Some(a) = anchor(p) else { return };
    let pl = pal(ui);
    ui.horizontal_wrapped(|ui| {
        if let Some(info) = p.network_canvas_settings.get("mapLocationInfo") {
            for (k, key) in [("locality", "step8.localityLabel"), ("county", "step8.countyLabel")] {
                if let Some(v) = info.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()) {
                    ui.label(RichText::new(format!("{}:", t.t(key))).size(11.5).color(pl.muted));
                    ui.label(RichText::new(v).size(11.5).strong());
                }
            }
            if let Some(c) = info.get("country").and_then(Value::as_str) {
                ui.label(RichText::new(c).size(11.5).color(pl.muted));
            }
        } else {
            ui.label(RichText::new(t.t("step8.loadingLocation")).size(11.5).italics().color(pl.muted));
        }
        ui.label(RichText::new(format!("{:.6}°N  {:.6}°E", a.lat, a.lng)).monospace().size(11.0).color(pl.muted));
        let _ = geo::world_to_latlng;
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn url_encoding() {
        assert_eq!(super::url_encode("Cluj-Napoca, România"), "Cluj-Napoca%2C%20Rom%C3%A2nia");
    }
}

#[cfg(test)]
mod lock_tests {
    use super::*;

    #[test]
    fn locked_view_overrides_live_zoom_and_pan() {
        let mut p = ProjectData::default();
        let live = View { origin: Pos2::ZERO, pan: Vec2::new(10.0, 20.0), zoom: 2.0 };
        assert_eq!(map_view(&p, live), live);
        set(&mut p, "mapLockedState", json!({"zoom": 0.5, "pan": {"x": 1.0, "y": 2.0}}));
        set(&mut p, "mapLockZoom", json!(true));
        let v = map_view(&p, live);
        assert_eq!((v.zoom, v.pan), (0.5, Vec2::new(10.0, 20.0)));
        set(&mut p, "mapLockPan", json!(true));
        assert_eq!(map_view(&p, live).pan, Vec2::new(1.0, 2.0));
    }
}

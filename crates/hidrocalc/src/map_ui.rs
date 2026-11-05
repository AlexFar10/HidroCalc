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

const TOPO_COLOR: Color32 = Color32::from_rgb(0x15, 0x80, 0x3D);

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

    /// `view_center` = centrul vizualizării în coordonatele schemei (punctul ancorat).
    pub fn ui(&mut self, ctx: &egui::Context, t: &I18n, p: &mut ProjectData, view_center: (f64, f64), live: View) {
        if !self.open {
            return;
        }
        let mut open = true;
        egui::Window::new(t.t("native.mapTitle")).open(&mut open).resizable(false).default_width(360.0).show(ctx, |ui| {
            self.anchor_section(ui, t, p, view_center);
            if anchor(p).is_some() {
                ui.separator();
                self.display_section(ui, t, p, live);
                ui.separator();
                self.topo_section(ui, t, p);
            }
        });
        self.open &= open;
    }

    fn anchor_section(&mut self, ui: &mut Ui, t: &I18n, p: &mut ProjectData, center: (f64, f64)) {
        let pl = pal(ui);
        ui.label(RichText::new(t.t("native.mapSearch")).size(12.0).color(pl.muted));
        ui.horizontal(|ui| {
            let lbl = ui.label("🔎").id;
            let r = ui.add(egui::TextEdit::singleline(&mut self.query).hint_text("ex: Timișoara, Cluj-Napoca…").desired_width(240.0)).labelled_by(lbl);
            let go = ui.add_enabled(self.search.is_none() && self.query.trim().len() >= 2, egui::Button::new(t.t("native.search"))).clicked()
                || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
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
                ui.spinner();
            }
        });
        let mut picked = None;
        for (name, lat, lng) in &self.results {
            if ui.add(egui::Button::new(RichText::new(name).size(12.0)).frame(false).wrap()).clicked() {
                picked = Some((name.clone(), *lat, *lng));
            }
        }
        if let Some((name, lat, lng)) = picked {
            self.lat = format!("{lat:.6}");
            self.lng = format!("{lng:.6}");
            self.query = name.split(',').take(2).collect::<Vec<_>>().join(",");
            self.results.clear();
        }
        ui.add_space(4.0);
        ui.columns(2, |c| {
            w::text_field(&mut c[0], &t.t("native.latitude"), &mut self.lat, "45.748900");
            w::text_field(&mut c[1], &t.t("native.longitude"), &mut self.lng, "21.208700");
        });
        if let Some(e) = &self.error {
            ui.label(RichText::new(e).size(12.0).color(pl.danger));
        }
        ui.horizontal(|ui| {
            if w::primary_button(ui, &t.t("native.mapConfirm")).clicked() {
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
            if anchor(p).is_some() && w::danger_button(ui, &t.t("native.mapDisable"), false).clicked() {
                set(p, "mapAnchor", Value::Null);
                set(p, "mapLocationInfo", Value::Null);
            }
        });
    }

    fn display_section(&mut self, ui: &mut Ui, t: &I18n, p: &mut ProjectData, live: View) {
        ui.horizontal(|ui| {
            ui.label(t.t("native.mapLock"));
            for (key, label) in [("mapLockPan", "native.lockPan"), ("mapLockZoom", "native.lockZoom"), ("mapLockRotation", "native.lockRotation")] {
                let mut on = flag(p, key);
                if ui.toggle_value(&mut on, t.t(label)).changed() {
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
        let mut op = opacity(p);
        ui.horizontal(|ui| {
            let l = ui.label(t.t("native.mapOpacity")).id;
            if ui.add(egui::Slider::new(&mut op, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0}%", v * 100.0))).labelled_by(l).changed() {
                set(p, "mapOpacity", json!(op));
            }
        });
        let mut rot = rotation(p);
        ui.horizontal(|ui| {
            let l = ui.label(t.t("native.mapRotation")).id;
            let unlocked = !flag(p, "mapLockRotation");
            if ui.add_enabled(unlocked, egui::Slider::new(&mut rot, -180.0..=180.0).suffix("°").step_by(0.5)).labelled_by(l).changed() {
                set(p, "mapRotation", json!(rot));
            }
            if ui.add_enabled(unlocked, egui::Button::new(t.t("canvas.resetRotation")).small()).clicked() {
                set(p, "mapRotation", json!(0.0));
            }
        });
        let mut auto = auto_terrain(p);
        if ui.checkbox(&mut auto, t.t("terrain.autoToggle")).changed() {
            set(p, "autoTerrainCota", json!(auto));
        }
        ui.label(RichText::new(t.t("terrain.attribution")).size(10.5).color(pal(ui).muted));
        let mut depth = hidro_core::export::quantities::pipe_depth(p);
        ui.horizontal(|ui| {
            let l = ui.label(t.t("native.pipeDepth")).id;
            if ui.add(egui::DragValue::new(&mut depth).range(0.3..=5.0).speed(0.05).suffix(" m")).labelled_by(l).changed() {
                set(p, "pipeDepth", json!(depth));
            }
        });
    }

    fn topo_section(&mut self, ui: &mut Ui, t: &I18n, p: &mut ProjectData) {
        let pl = pal(ui);
        let pts = topo_points(p);
        ui.label(RichText::new(t.t("topo.title")).strong());
        ui.add(egui::Label::new(RichText::new(t.t("topo.formats")).size(11.0).color(pl.muted)).wrap());
        ui.label(RichText::new(t.tf("native.topoCount", &[("n", &pts.len().to_string())])).size(12.0).color(pl.muted));
        let crs_opts = [
            (Crs::Auto, "topo.crsAuto"),
            (Crs::Stereo70, "topo.crsStereo70"),
            (Crs::Stereo70En, "topo.crsStereo70en"),
            (Crs::Wgs84, "topo.crsWgs84"),
        ];
        let cur = self.crs.unwrap_or(Crs::Auto);
        ui.horizontal(|ui| {
            let l = ui.label(t.t("topo.crs")).id;
            let shown = crs_opts.iter().find(|(c, _)| *c == cur).map(|(_, k)| t.t(k)).unwrap_or_default();
            let r = egui::ComboBox::from_id_salt("crs").selected_text(shown).show_ui(ui, |ui| {
                for (c, k) in crs_opts {
                    if ui.selectable_label(cur == c, t.t(k)).clicked() {
                        self.crs = Some(c);
                    }
                }
            });
            let _ = r.response.labelled_by(l);
        });
        let mut radius = topo_radius(p);
        ui.horizontal(|ui| {
            let l = ui.label(t.t("topo.radius")).id;
            if ui.add(egui::DragValue::new(&mut radius).range(0.5..=100.0).speed(0.5).suffix(" m")).labelled_by(l).changed() {
                set(p, "topoRadius", json!(radius));
            }
        });
        ui.horizontal(|ui| {
            if w::secondary_button(ui, &t.t("topo.chooseFile")).clicked() {
                self.import_topo(t, p);
            }
            if !pts.is_empty() && w::danger_button(ui, &t.t("topo.clear"), false).clicked() {
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
        });
        if let Some(n) = &self.notice {
            ui.label(RichText::new(n).size(12.0).color(pl.ink2));
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

//! Vizualizarea 3D a rețelei, desenată direct cu egui (proiecție în perspectivă,
//! triunghiuri sortate după adâncime). Echivalentul `NetworkScene3D.tsx`.
//!
//! Comenzi: tragere = rotire în jurul scenei, Shift/clic dreapta + tragere = deplasare,
//! rotiță = zoom.

use crate::i18n::I18n;
use crate::map_ui;
use crate::widgets::{self as w, pal};
use eframe::egui::{self, Align2, Color32, FontId, Mesh, Pos2, Rect, RichText, Sense, Shape, Stroke, Ui, Vec2};
use hidro_core::defaults::DEFAULT_PIPE_DEPTH;
use hidro_core::elevation::{self, LatLng as ElevLatLng};
use hidro_core::geo::{latlng_to_world, world_to_latlng};
use hidro_core::model::{ProjectData, SchemaKind};
use hidro_core::scene3d::{self, Scene, TerrainGrid, Vec3};
use serde_json::{json, Value};
use std::sync::mpsc::{channel, Receiver};


pub struct View3D {
    yaw: f32,
    pitch: f32,
    zoom: f32,
    target: [f32; 3],
    pub labels: bool,
    grid_rx: Option<Receiver<Result<TerrainGrid, String>>>,
    grid_error: Option<String>,
    /// Zona de desen din cadrul curent (decupajul exportului PNG).
    last_rect: Rect,
    png_rx: Option<Receiver<std::sync::Arc<egui::ColorImage>>>,
    /// PNG-ul gata (sau eroarea), preluat de aplicație pentru salvare.
    pub png_ready: Option<Result<Vec<u8>, String>>,
}

impl Default for View3D {
    fn default() -> Self {
        Self {
            yaw: -0.6,
            pitch: 0.55,
            zoom: 1.0,
            target: [0.0; 3],
            labels: true,
            grid_rx: None,
            grid_error: None,
            last_rect: Rect::NOTHING,
            png_rx: None,
            png_ready: None,
        }
    }
}

fn setting_f64(p: &ProjectData, k: &str, d: f64) -> f64 {
    p.network_canvas_settings.get(k).and_then(Value::as_f64).unwrap_or(d)
}
fn set(p: &mut ProjectData, k: &str, v: Value) {
    if let Some(o) = p.network_canvas_settings.as_object_mut() {
        o.insert(k.into(), v);
    }
}

/// Camera: punctul privit, poziția ochiului și baza ortonormată.
struct Camera {
    eye: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    fwd: [f32; 3],
    focal: f32,
    center: Pos2,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}
fn f3(v: Vec3) -> [f32; 3] {
    [v[0] as f32, v[1] as f32, v[2] as f32]
}

/// Geometria modelului 3D, grupată pe obiecte (teren, fiecare conductă, fiecare nod, puncte topo).
/// O folosesc și desenarea vederii, și exporturile 3D, deci ce vezi e ce exporți.
pub(crate) fn model_groups(scene: &Scene) -> Vec<(String, crate::mesh3d::Scene3)> {
    use crate::mesh3d::{self as m3, lerp, rgb, Scene3, Vert};
    let span = scene.span as f32;
    let h = span * 0.035;
    let pipe_r = |de: f64| span * 0.0025 * ((de.max(63.0) / 110.0) as f32).sqrt();
    let mut groups: Vec<(String, Scene3)> = Vec::new();

    // Teren: culori hipsometrice (verde → galben → maro), normale netede, semitransparent
    if let Some(tr) = &scene.terrain {
        let mut s3 = Scene3::default();
        let (low, mid, high) = (rgb(0x4D7C0F), rgb(0xCA8A04), rgb(0x7C2D12));
        let range = (tr.max_h - tr.min_h).max(0.01) as f32;
        let pos: Vec<[f32; 3]> = tr.positions.iter().map(|v| f3(*v)).collect();
        let idx = |i: usize, j: usize| j * tr.nx + i;
        let mut normals = vec![[0.0f32; 3]; pos.len()];
        let mut tris = Vec::new();
        for j in 0..tr.ny - 1 {
            for i in 0..tr.nx - 1 {
                let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i, j + 1), idx(i + 1, j + 1));
                for t in [[a, c, b], [b, c, d]] {
                    let n = m3::cross(m3::sub(pos[t[1]], pos[t[0]]), m3::sub(pos[t[2]], pos[t[0]]));
                    for k in t {
                        normals[k] = m3::add(normals[k], n);
                    }
                    tris.push(t);
                }
            }
        }
        let color = |k: usize| {
            let f = ((tr.heights[k] - tr.min_h) as f32 / range).clamp(0.0, 1.0);
            if f < 0.5 { lerp(low, mid, f * 2.0) } else { lerp(mid, high, (f - 0.5) * 2.0) }
        };
        for t in tris {
            let v = t.map(|k| Vert { p: pos[k], n: m3::norm(normals[k]), c: color(k) });
            s3.tris.push(m3::Tri { v, alpha: 0.55, layer: 1, double_sided: true });
        }
        groups.push(("Teren".into(), s3));
    }

    // Conducte: sub pământ (stratul 0), tuburi împărțite în bucăți pentru sortare
    for p in &scene.pipes {
        let mut s3 = Scene3::default();
        s3.layer = 0;
        let (a, b) = (f3(p.start), f3(p.end));
        let col = p.color.as_deref().and_then(|c| u32::from_str_radix(c.trim_start_matches('#'), 16).ok()).filter(|c| *c != 0x1A1A1A).map(rgb).unwrap_or(rgb(0x2563EB));
        let len = m3::dot(m3::sub(b, a), m3::sub(b, a)).sqrt();
        let n = ((len / (span / 40.0)).ceil() as usize).clamp(1, 40);
        for k in 0..n {
            let (t0, t1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
            s3.cylinder(lerp(a, b, t0), lerp(a, b, t1), pipe_r(p.d_ext), pipe_r(p.d_ext), 10, col, 1.0, k == 0 || k == n - 1);
        }
        // verticalele de la capete (sub pământ) și urma pe sol (deasupra)
        s3.line(f3(p.ground_start), a, Color32::from_rgb(0x78, 0x71, 0x6C), 1.0, false);
        s3.line(f3(p.ground_end), b, Color32::from_rgb(0x78, 0x71, 0x6C), 1.0, false);
        s3.layer = 2;
        s3.line(f3(p.ground_start), f3(p.ground_end), Color32::from_rgb(0xA1, 0x62, 0x07), 1.5, true);
        groups.push((format!("Conducta {}", p.label), s3));
    }

    // Noduri (modelele procedurale din aplicația React)
    for n in &scene.nodes {
        let mut s3 = Scene3::default();
        let g = f3(n.ground);
        let up = |dy: f32| [g[0], g[1] + dy, g[2]];
        // căminele și racordul la conductă sunt sub pământ; restul deasupra
        s3.layer = 0;
        s3.line(f3(n.invert), g, Color32::from_rgb(0x78, 0x71, 0x6C), 1.0, false);
        s3.layer = if matches!(n.kind, SchemaKind::CaminGrav | SchemaKind::CaminAer) { 0 } else { 2 };
        let name = match n.kind {
            SchemaKind::Hidrant => "Hidrant",
            SchemaKind::Rezervor => "Rezervor",
            SchemaKind::Sursa => "Sursa",
            SchemaKind::CaminGrav => "Camin gravitational",
            SchemaKind::CaminAer => "Camin aerian",
            SchemaKind::Subsistem => "Subsistem",
            _ => "Nod",
        };
        match n.kind {
            SchemaKind::Hidrant => {
                s3.cylinder(g, up(h * 0.8), h * 0.15, h * 0.12, 10, rgb(0xDC2626), 1.0, true);
                s3.sphere(up(h * 0.82), h * 0.14, rgb(0xB91C1C));
                s3.cylinder([g[0] - h * 0.21, g[1] + h * 0.55, g[2]], [g[0] + h * 0.21, g[1] + h * 0.55, g[2]], h * 0.05, h * 0.05, 6, rgb(0x991B1B), 1.0, true);
            }
            SchemaKind::Rezervor => {
                s3.cylinder(g, up(h * 0.8), h * 0.7, h * 0.7, 16, rgb(0x94A3B8), 1.0, true);
                s3.cylinder(up(h * 0.8), up(h), h * 0.74, 0.0, 16, rgb(0x64748B), 1.0, false);
            }
            SchemaKind::Sursa => s3.bx(g, h * 0.9, h * 0.7, h * 0.9, rgb(0x0D9488), 1.0),
            SchemaKind::CaminGrav | SchemaKind::CaminAer => {
                let c = if n.kind == SchemaKind::CaminGrav { rgb(0x1D4ED8) } else { rgb(0x3B82F6) };
                s3.cylinder(up(-h * 0.5), g, h * 0.3, h * 0.3, 10, c, 1.0, true);
            }
            SchemaKind::Subsistem => s3.bx(g, h * 1.6, h * 0.3, h, rgb(0x14B8A6), 0.6),
            _ => {}
        }
        let label = if n.label.is_empty() { name.to_string() } else { format!("{name} {}", n.label) };
        groups.push((label, s3));
    }
    if !scene.topo.is_empty() {
        let mut s3 = Scene3::default();
        s3.layer = 2;
        for tp in &scene.topo {
            s3.sphere(f3(*tp), h * 0.12, rgb(0x16A34A));
        }
        groups.push(("Puncte topo".into(), s3));
    }
    groups
}

impl Camera {
    /// Punct 3D → (poziție pe ecran, adâncime); `None` dacă e în spatele camerei.
    fn project(&self, p: [f32; 3]) -> Option<(Pos2, f32)> {
        let d = sub(p, self.eye);
        let z = dot(d, self.fwd);
        if z < 0.5 {
            return None;
        }
        let x = dot(d, self.right) / z * self.focal;
        let y = dot(d, self.up) / z * self.focal;
        Some((self.center + Vec2::new(x, -y), z))
    }
}

impl View3D {
    fn terrain_grid(p: &ProjectData) -> Option<TerrainGrid> {
        p.network_canvas_settings.get("terrainGrid").and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Grila salvată, doar dacă aparține ancorei curente (altfel e de la altă poziție a hărții).
    fn current_grid(p: &ProjectData) -> Option<TerrainGrid> {
        let a = map_ui::anchor(p)?;
        let key = scene3d::terrain_key(&a, map_ui::rotation(p), crate::canvas::scale_ratio(p));
        Self::terrain_grid(p).filter(|g| g.key == key && g.is_valid())
    }

    /// Descarcă grila de teren DEM dacă lipsește sau nu mai acoperă schema.
    fn sync_grid(&mut self, ctx: &egui::Context, p: &mut ProjectData) {
        if let Some(rx) = &self.grid_rx {
            if let Ok(res) = rx.try_recv() {
                self.grid_rx = None;
                match res {
                    Ok(g) => {
                        set(p, "terrainGrid", serde_json::to_value(g).unwrap_or(Value::Null));
                        self.grid_error = None;
                    }
                    Err(e) => self.grid_error = Some(e),
                }
            }
            return;
        }
        if self.grid_error.is_some() || p.network_canvas_settings.get("showTerrain3D").and_then(Value::as_bool) == Some(false) {
            return;
        }
        let (Some(a), Some(b)) = (map_ui::anchor(p), scene3d::schema_bounds(&p.network_schema)) else { return };
        let rot = map_ui::rotation(p);
        let sr = crate::canvas::scale_ratio(p);
        let key = scene3d::terrain_key(&a, rot, sr);
        if scene3d::grid_covers(Self::terrain_grid(p).as_ref(), &b, &key) {
            return;
        }
        let (gb, nx, ny, pts) = scene3d::grid_nodes(&b, sr);
        let ll: Vec<ElevLatLng> = pts
            .iter()
            .map(|&(x, y)| {
                let l = world_to_latlng(x, y, &a, rot);
                ElevLatLng { lat: l.lat, lng: l.lng }
            })
            .collect();
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let dir = directories::ProjectDirs::from("ro", "hidro", "HidroCalc").map(|d| d.cache_dir().join("terrarium").join(elevation::ZOOM.to_string()));
            let res = dir.ok_or_else(|| "cache".to_string()).and_then(|d| elevation::fetch_elevations(&ll, d)).map(|heights| TerrainGrid {
                min_x: gb.min_x,
                min_y: gb.min_y,
                max_x: gb.max_x,
                max_y: gb.max_y,
                nx,
                ny,
                heights,
                key,
            });
            let _ = tx.send(res);
            ctx.request_repaint();
        });
        self.grid_rx = Some(rx);
    }

    fn scene(p: &ProjectData) -> Scene {
        Self::scene_with(p, setting_f64(p, "view3DExaggeration", 5.0))
    }

    fn scene_with(p: &ProjectData, exaggeration: f64) -> Scene {
        let topo: Vec<(f64, f64, f64)> = match map_ui::anchor(p) {
            Some(a) => map_ui::topo_points(p)
                .iter()
                .map(|t| {
                    let (x, y) = latlng_to_world(t.lat, t.lng, &a, map_ui::rotation(p));
                    (x, y, t.z)
                })
                .collect(),
            None => Vec::new(),
        };
        let show_terrain = p.network_canvas_settings.get("showTerrain3D").and_then(Value::as_bool) != Some(false);
        let grid = if show_terrain { Self::current_grid(p) } else { None };
        scene3d::build(&scene3d::Input {
            schema: &p.network_schema,
            sections: &p.network_sections,
            scale_ratio: crate::canvas::scale_ratio(p),
            pipe_depth: setting_f64(p, "pipeDepth", DEFAULT_PIPE_DEPTH),
            exaggeration,
            reservoir_ground: p.reservoir.cota_teren,
            terrain: grid.as_ref(),
            topo: &topo,
            interpolate_ground: show_terrain,
        })
    }

    /// Imaginea cerută cu `request_png`: captura ecranului, decupată pe zona de desen.
    fn finish_png(&mut self, ctx: &egui::Context) {
        let Some(img) = self.png_rx.as_ref().and_then(|rx| rx.try_recv().ok()) else {
            if self.png_rx.is_some() {
                ctx.request_repaint();
            }
            return;
        };
        self.png_rx = None;
        let (rgba, w, h) = crate::canvas::crop_rgba(&img, self.last_rect, ctx.pixels_per_point());
        let mut png = Vec::new();
        let enc = image::codecs::png::PngEncoder::new(&mut png);
        self.png_ready = Some(match image::ImageEncoder::write_image(enc, &rgba, w as u32, h as u32, image::ExtendedColorType::Rgba8) {
            Ok(()) => Ok(png),
            Err(e) => Err(e.to_string()),
        });
    }

    /// Exagerarea verticală „placută”: reliefului i se dă ~10% din întinderea scenei ca înălțime
    /// vizuală (între 1× și 12×); fără teren sau relief, 5×.
    fn pleasant_exaggeration(p: &ProjectData) -> f64 {
        let flat = Self::scene_with(p, 1.0);
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        let mut see = |y: f64| {
            lo = lo.min(y);
            hi = hi.max(y);
        };
        if let Some(t) = &flat.terrain {
            see(t.min_h);
            see(t.max_h);
        }
        for pipe in &flat.pipes {
            see(pipe.ground_start[1]);
            see(pipe.ground_end[1]);
        }
        let relief = hi - lo;
        if !relief.is_finite() || relief < 0.05 || flat.span <= 0.0 {
            return 5.0;
        }
        ((0.10 * flat.span / relief).clamp(1.0, 12.0) * 2.0).round() / 2.0
    }

    /// Obiectul de sub cursor (nod sau conductă), în coordonate de scenă: nodul cel mai apropiat
    /// pe ecran sau cel mai apropiat punct al unei conducte, dacă e la cel mult 40 px.
    fn pick(&self, scene: &Scene, rect: Rect, cursor: Pos2) -> Option<Vec3> {
        const MAX_PX: f32 = 40.0;
        let cam = self.camera(scene, rect);
        let mut best: Option<(f32, Vec3)> = None;
        let mut offer = |d: f32, at: Vec3| {
            if d <= MAX_PX && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                best = Some((d, at));
            }
        };
        for n in &scene.nodes {
            if let Some((s, _)) = cam.project(f3(n.ground)) {
                // nodurile contează puțin mai mult decât conductele de lângă ele
                offer(s.distance(cursor) - 6.0, n.ground);
            }
        }
        for p in &scene.pipes {
            let (Some((a, _)), Some((b, _))) = (cam.project(f3(p.ground_start)), cam.project(f3(p.ground_end))) else { continue };
            let ab = b - a;
            let len2 = ab.length_sq();
            let t = if len2 < 1e-6 { 0.0 } else { ((cursor - a).dot(ab) / len2).clamp(0.0, 1.0) };
            let d = (a + ab * t).distance(cursor);
            let at = [
                p.ground_start[0] + (p.ground_end[0] - p.ground_start[0]) * t as f64,
                p.ground_start[1] + (p.ground_end[1] - p.ground_start[1]) * t as f64,
                p.ground_start[2] + (p.ground_end[2] - p.ground_start[2]) * t as f64,
            ];
            offer(d, at);
        }
        best.map(|(_, at)| at)
    }

    /// Modelul 3D curent (cu exagerarea verticală și adâncimea setate în vedere) în formatul cerut.
    pub fn model_bytes(p: &ProjectData, format: crate::export3d::ModelFormat) -> Result<Vec<u8>, String> {
        let scene = Self::scene(p);
        if p.network_schema.is_empty() {
            return Err("schema e goală".into());
        }
        crate::export3d::export(&model_groups(&scene), format)
    }

    /// Cere exportul PNG al vederii 3D (butonul „PNG 3D” din bara laterală); captura vine la cadrul următor.
    pub fn request_png(&mut self, ctx: &egui::Context) {
        if self.png_rx.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        ctx.request_screenshot(move |img| {
            let _ = tx.send(img);
        });
        self.png_rx = Some(rx);
        ctx.request_repaint();
    }

    #[cfg(test)]
    pub fn yaw_for_test(&self) -> f32 {
        self.yaw
    }

    /// Terenul încă se descarcă (captura trebuie să aștepte).
    pub fn terrain_loading(&self) -> bool {
        self.grid_rx.is_some()
    }

    pub fn ui(&mut self, ui: &mut Ui, t: &I18n, p: &mut ProjectData, height: f32) {
        let pl = pal(ui);
        let ctx = ui.ctx().clone();
        self.sync_grid(&ctx, p);
        self.finish_png(&ctx);

        ui.horizontal(|ui| {
            if w::secondary_button(ui, &t.t("view3d.resetRelief")).on_hover_text(t.t("view3d.resetReliefTip")).clicked() {
                set(p, "view3DExaggeration", json!(Self::pleasant_exaggeration(p)));
            }
        });
        ui.horizontal_wrapped(|ui| {
            let mut ex = setting_f64(p, "view3DExaggeration", 5.0);
            let l = ui.label(t.tf("view3d.exaggeration", &[("val", &format!("{ex}"))])).id;
            if ui.add(egui::Slider::new(&mut ex, 1.0..=20.0).step_by(0.5).show_value(false)).labelled_by(l).changed() {
                set(p, "view3DExaggeration", json!(ex));
            }
            let mut depth = setting_f64(p, "pipeDepth", DEFAULT_PIPE_DEPTH);
            let l = ui.label(t.t("view3d.pipeDepth")).id;
            if ui.add(egui::DragValue::new(&mut depth).range(0.3..=5.0).speed(0.05)).labelled_by(l).changed() {
                set(p, "pipeDepth", json!(depth));
            }
            let mut show = p.network_canvas_settings.get("showTerrain3D").and_then(Value::as_bool) != Some(false);
            if ui.checkbox(&mut show, t.t("view3d.terrain")).changed() {
                set(p, "showTerrain3D", json!(show));
            }
            ui.checkbox(&mut self.labels, t.t("view3d.labels"));
            if ui.button(t.t("native.fit")).clicked() {
                *self = Self { labels: self.labels, grid_rx: self.grid_rx.take(), ..Default::default() };
            }
        });

        let scene = Self::scene(p);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click_and_drag());
        self.last_rect = rect;
        self.interact(ui, &resp, &scene);
        w::focus_ring(ui, &resp, 10);
        w::describe(&resp, egui::Role::Canvas, &t.t("view3d.a11yLabel"));
        let painter = ui.painter_at(rect);
        if p.network_schema.is_empty() {
            painter.rect_filled(rect, 10.0, Color32::from_rgb(0xEE, 0xF2, 0xF6));
            painter.text(rect.center(), Align2::CENTER_CENTER, t.t("view3d.empty"), FontId::proportional(14.0), pl.muted);
            return;
        }
        let cam = self.camera(&scene, rect);
        self.draw(&painter, &cam, &scene, rect, ui.visuals().dark_mode);
        // Obiectul spre care va merge zoom-ul: inel în jurul lui, ca să se vadă ce e „ales”.
        if resp.hovered() && !resp.dragged() {
            if let Some(obj) = resp.hover_pos().and_then(|c| self.pick(&scene, rect, c)) {
                if let Some((s, _)) = cam.project(f3(obj)) {
                    painter.circle_stroke(s, 12.0, Stroke::new(1.5, pl.accent));
                }
            }
        }

        // Starea terenului
        ui.horizontal_wrapped(|ui| {
            let status = if scene.terrain_interpolated {
                format!("{} · {}", t.t("native.groundInterpolated"), t.t("view3d.terrainNeedsAnchor"))
            } else if map_ui::anchor(p).is_none() {
                t.t("view3d.terrainNeedsAnchor")
            } else if self.grid_rx.is_some() {
                t.t("view3d.terrainLoading")
            } else if let Some(e) = &self.grid_error {
                format!("{} {e}", t.t("view3d.terrainError"))
            } else if let Some(tr) = &scene.terrain {
                t.tf("view3d.terrainInfo", &[("nx", &tr.nx.to_string()), ("ny", &tr.ny.to_string()), ("min", &w::fmt(tr.min_h, 1)), ("max", &w::fmt(tr.max_h, 1))])
            } else {
                String::new()
            };
            ui.label(RichText::new(status).size(11.5).color(pl.muted));
            if self.grid_error.is_some() && ui.small_button(t.t("view3d.terrainRetry")).clicked() {
                self.grid_error = None;
            }
            if !scene.topo.is_empty() {
                ui.label(RichText::new(t.tf("view3d.topoCount", &[("n", &scene.topo.len().to_string())])).size(11.5).color(pl.muted));
            }
        });
        ui.label(RichText::new(t.tf("view3d.hint", &[("base", &w::fmt(scene.base_elevation, 2))])).size(11.0).color(pl.muted));
    }

    fn interact(&mut self, ui: &Ui, resp: &egui::Response, scene: &Scene) {
        // Tastatura, cu focus pe vedere: săgeți = rotire (cu Shift: deplasare), +/− = zoom, Home = reset.
        if resp.has_focus() {
            let (dx, dy, shift, zin, zout, home) = ui.input(|i| {
                let k = |key| i.key_pressed(key) as i32 as f32;
                (
                    k(egui::Key::ArrowRight) - k(egui::Key::ArrowLeft),
                    k(egui::Key::ArrowDown) - k(egui::Key::ArrowUp),
                    i.modifiers.shift,
                    i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals),
                    i.key_pressed(egui::Key::Minus),
                    i.key_pressed(egui::Key::Home),
                )
            });
            if shift {
                let k = scene.span as f32 * self.zoom / 600.0 * 40.0;
                let (s, c) = self.yaw.sin_cos();
                // aceleași axe ca la tragerea cu Shift: dreapta/jos mută vederea
                self.target[0] += (c * dx - s * dy) * k;
                self.target[2] += (-s * dx - c * dy) * k;
            } else {
                self.yaw -= dx * 0.1;
                self.pitch = (self.pitch - dy * 0.08).clamp(0.05, 1.5);
            }
            if zin {
                self.zoom = (self.zoom / 1.15).clamp(0.05, 8.0);
            }
            if zout {
                self.zoom = (self.zoom * 1.15).clamp(0.05, 8.0);
            }
            if home {
                *self = Self { labels: self.labels, grid_rx: self.grid_rx.take(), ..Default::default() };
            }
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            let pan = ui.input(|i| i.modifiers.shift || i.pointer.secondary_down() || i.pointer.middle_down());
            if pan {
                // deplasare în planul orizontal, relativă la orientarea camerei
                let k = scene.span as f32 * self.zoom / 600.0;
                let (s, c) = self.yaw.sin_cos();
                self.target[0] -= (c * d.x - s * d.y) * k;
                self.target[2] -= (-s * d.x - c * d.y) * k;
            } else {
                self.yaw -= d.x * 0.008;
                self.pitch = (self.pitch + d.y * 0.006).clamp(0.05, 1.5);
            }
        }
        if resp.hovered() {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let old = self.zoom;
            let new = (old / pinch * (-scroll / 400.0).exp()).clamp(0.05, 8.0);
            if (new - old).abs() > 1e-6 {
                // Zoom spre obiectul de sub cursor (chiar dacă e într-o parte a ecranului):
                // ținta camerei se apropie de obiect pe măsură ce te apropii.
                if let Some(obj) = resp.hover_pos().and_then(|c| self.pick(scene, resp.rect, c)) {
                    let c = scene.center;
                    let ratio = (new / old) as f64;
                    for k in 0..3 {
                        let t = c[k] + self.target[k] as f64;
                        self.target[k] = (obj[k] + (t - obj[k]) * ratio - c[k]) as f32;
                    }
                }
                self.zoom = new;
            }
        }
    }

    fn camera(&self, scene: &Scene, rect: Rect) -> Camera {
        let c = f3(scene.center);
        let target = [c[0] + self.target[0], c[1] + self.target[1], c[2] + self.target[2]];
        let dist = scene.span as f32 * 0.8 * self.zoom;
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let eye = [target[0] + dist * cp * sy, target[1] + dist * sp, target[2] + dist * cp * cy];
        let fwd = norm(sub(target, eye));
        let right = norm(cross(fwd, [0.0, 1.0, 0.0]));
        let up = cross(right, fwd);
        let focal = rect.height() / 2.0 / (22.5f32.to_radians()).tan();
        Camera { eye, right, up, fwd, focal, center: rect.center() }
    }

    fn draw(&self, painter: &egui::Painter, cam: &Camera, scene: &Scene, rect: Rect, dark: bool) {
        use crate::mesh3d::{self as m3, Scene3};
        // Fundal în degrade (cer)
        let (top, bottom) = if dark { (Color32::from_rgb(0x0B, 0x12, 0x20), Color32::from_rgb(0x1A, 0x24, 0x36)) } else { (Color32::from_rgb(0xDC, 0xE6, 0xF2), Color32::from_rgb(0xF4, 0xF6, 0xFA)) };
        let mut bg = Mesh::default();
        for (p, c) in [(rect.left_top(), top), (rect.right_top(), top), (rect.right_bottom(), bottom), (rect.left_bottom(), bottom)] {
            bg.colored_vertex(p, c);
        }
        bg.add_triangle(0, 1, 2);
        bg.add_triangle(0, 2, 3);
        painter.add(Shape::mesh(bg));

        let span = scene.span as f32;
        let h = span * 0.035;
        let mut s3 = Scene3::default();

        // Grila de bază (gridHelper)
        let gy = 0.0;
        let half = span * 0.8;
        let (gc, gc2) = if dark { (Color32::from_rgb(0x33, 0x41, 0x55), Color32::from_rgb(0x1E, 0x29, 0x3B)) } else { (Color32::from_rgb(0x94, 0xA3, 0xB8), Color32::from_rgb(0xCB, 0xD5, 0xE1)) };
        for i in 0..=32 {
            let t = -half + span * 1.6 * i as f32 / 32.0;
            let c = if i == 16 { gc } else { gc2 };
            for (a, b) in [([t, gy, -half], [t, gy, half]), ([-half, gy, t], [half, gy, t])] {
                s3.underlay.push(m3::Line { layer: 0, a, b, color: c, width: 1.0, dashed: false });
            }
        }

        // Geometria modelului (aceeași ca la exportul 3D).
        for (_, g) in model_groups(scene) {
            s3.tris.extend(g.tris);
            s3.lines.extend(g.lines);
        }

        let mcam = m3::Cam { eye: cam.eye, right: cam.right, up: cam.up, fwd: cam.fwd, focal: cam.focal, center: cam.center, near: span / 2000.0 };
        m3::render(painter, &mcam, &s3, &m3::Light::standard());

        // Etichete (deasupra scenei, ca elementele Html din React)
        if self.labels {
            for p in &scene.pipes {
                let m = [(p.ground_start[0] + p.ground_end[0]) / 2.0, (p.ground_start[1] + p.ground_end[1]) / 2.0, (p.ground_start[2] + p.ground_end[2]) / 2.0];
                if let Some((s, _)) = cam.project(f3(m)) {
                    let txt = if p.d_ext > 0.0 { format!("{} · De {} · {:.1} m", p.label, p.d_ext, p.length_m) } else { format!("{} · {:.1} m", p.label, p.length_m) };
                    label(painter, s, &txt, Color32::from_black_alpha(178), Color32::WHITE);
                }
            }
            for n in scene.nodes.iter().filter(|n| !n.label.is_empty()) {
                let g = f3(n.ground);
                if let Some((s, _)) = cam.project([g[0], g[1] + h * 1.25, g[2]]) {
                    label(painter, s, &n.label, Color32::from_white_alpha(217), Color32::from_rgb(0x1E, 0x29, 0x3B));
                }
            }
        }

        // Gizmo (axele X/Y/Z, ca GizmoViewport)
        let o = rect.right_bottom() + Vec2::new(-48.0, -48.0);
        painter.circle_filled(o, 36.0, if dark { Color32::from_black_alpha(90) } else { Color32::from_white_alpha(150) });
        let dir = |v: [f32; 3]| Vec2::new(dot(v, cam.right), -dot(v, cam.up)) * 26.0;
        let mut axes = [([1.0, 0.0, 0.0], Color32::from_rgb(0xEF, 0x44, 0x44), "E"), ([0.0, 1.0, 0.0], Color32::from_rgb(0x22, 0xC5, 0x5E), "↑"), ([0.0, 0.0, -1.0], Color32::from_rgb(0x3B, 0x82, 0xF6), "N")];
        axes.sort_by(|a, b| dot(a.0, cam.fwd).total_cmp(&dot(b.0, cam.fwd)).reverse());
        for (v, c, txt) in axes {
            let e = o + dir(v);
            painter.line_segment([o, e], Stroke::new(2.5, c));
            painter.circle_filled(e, 8.0, c);
            painter.text(e, Align2::CENTER_CENTER, txt, FontId::proportional(10.0), Color32::WHITE);
        }
    }
}

fn label(painter: &egui::Painter, pos: Pos2, text: &str, bg: Color32, fg: Color32) {
    let g = painter.layout_no_wrap(text.into(), FontId::proportional(10.5), fg);
    let r = Rect::from_center_size(pos, g.size()).expand2(Vec2::new(5.0, 2.0));
    painter.rect_filled(r, 3.0, bg);
    painter.galley(r.min + Vec2::new(5.0, 2.0), g, fg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use hidro_core::geo::{compute_tile_zoom, MapAnchor};

    #[test]
    fn stale_terrain_grid_is_ignored() {
        let mut p = ProjectData::default();
        let g = TerrainGrid { min_x: 0.0, min_y: 0.0, max_x: 10.0, max_y: 10.0, nx: 2, ny: 2, heights: vec![Some(1.0); 4], key: "veche".into() };
        set(&mut p, "terrainGrid", serde_json::to_value(&g).unwrap());
        assert!(View3D::current_grid(&p).is_none(), "fără ancoră, grila salvată nu se folosește");
        let a = MapAnchor { lat: 45.75, lng: 21.23, world_x: 0.0, world_y: 0.0, tile_zoom: compute_tile_zoom(45.75, 1000.0) };
        set(&mut p, "mapAnchor", serde_json::to_value(a).unwrap());
        assert!(View3D::current_grid(&p).is_none(), "grila altei ancore nu se folosește");
        let key = scene3d::terrain_key(&a, 0.0, 1000.0);
        set(&mut p, "terrainGrid", serde_json::to_value(TerrainGrid { key, ..g }).unwrap());
        assert!(View3D::current_grid(&p).is_some());
    }

    use hidro_core::model::SchemaElement;

    fn project() -> ProjectData {
        let mut p = ProjectData::new("t".into(), "2026-01-01T00:00:00.000Z".into(), "2026-01-01".into());
        p.network_schema = vec![
            SchemaElement { kind: SchemaKind::Tronson, x: 0.0, y: 0.0, x2: Some(1000.0), y2: Some(0.0), label: Some("Tr 1".into()), ..Default::default() },
            SchemaElement { kind: SchemaKind::Hidrant, x: 1000.0, y: 0.0, ..Default::default() },
        ];
        p
    }

    #[test]
    fn pick_finds_the_object_near_the_cursor() {
        let p = project();
        let scene = View3D::scene(&p);
        let v = View3D::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 600.0));
        let cam = v.camera(&scene, rect);
        let hydrant = scene.nodes.iter().find(|n| n.kind == SchemaKind::Hidrant).expect("hidrant în scenă");
        let (s, _) = cam.project(f3(hydrant.ground)).expect("vizibil");
        // cursorul puțin lateral față de hidrant: alege hidrantul
        let got = v.pick(&scene, rect, s + Vec2::new(12.0, -8.0)).expect("obiect ales");
        assert_eq!(got, hydrant.ground);
        // departe de orice obiect: nimic
        assert!(v.pick(&scene, rect, Pos2::new(5.0, 590.0)).is_none());
    }

    #[test]
    fn pleasant_exaggeration_is_within_the_slider_range() {
        let p = project();
        let e = View3D::pleasant_exaggeration(&p);
        assert!((1.0..=12.0).contains(&e), "{e}");
    }
}

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

const VIEW_H: f32 = 540.0;

pub struct View3D {
    yaw: f32,
    pitch: f32,
    zoom: f32,
    target: [f32; 3],
    pub labels: bool,
    grid_rx: Option<Receiver<Result<TerrainGrid, String>>>,
    grid_error: Option<String>,
}

impl Default for View3D {
    fn default() -> Self {
        Self { yaw: -0.6, pitch: 0.55, zoom: 1.0, target: [0.0; 3], labels: true, grid_rx: None, grid_error: None }
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

fn parse_color(s: Option<&str>) -> Option<Color32> {
    let h = s?.strip_prefix('#')?;
    let v = u32::from_str_radix(h, 16).ok().filter(|_| h.len() == 6)?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// Culoarea terenului după cotă (verde jos → maro sus).
fn terrain_color(t: f32, shade: f32) -> Color32 {
    let lo = [120.0, 170.0, 100.0];
    let hi = [176.0, 150.0, 110.0];
    let c: Vec<u8> = (0..3).map(|i| ((lo[i] + (hi[i] - lo[i]) * t) * shade).clamp(0.0, 255.0) as u8).collect();
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 150)
}

impl View3D {
    fn terrain_grid(p: &ProjectData) -> Option<TerrainGrid> {
        p.network_canvas_settings.get("terrainGrid").and_then(|v| serde_json::from_value(v.clone()).ok())
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
        let grid = if show_terrain { Self::terrain_grid(p) } else { None };
        scene3d::build(&scene3d::Input {
            schema: &p.network_schema,
            sections: &p.network_sections,
            scale_ratio: crate::canvas::scale_ratio(p),
            pipe_depth: setting_f64(p, "pipeDepth", DEFAULT_PIPE_DEPTH),
            exaggeration: setting_f64(p, "view3DExaggeration", 5.0),
            reservoir_ground: p.reservoir.cota_teren,
            terrain: grid.as_ref(),
            topo: &topo,
        })
    }

    pub fn ui(&mut self, ui: &mut Ui, t: &I18n, p: &mut ProjectData) {
        let pl = pal(ui);
        let ctx = ui.ctx().clone();
        self.sync_grid(&ctx, p);

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
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), VIEW_H), Sense::click_and_drag());
        self.interact(ui, &resp, &scene);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 10.0, Color32::from_rgb(0xEE, 0xF2, 0xF6));
        if p.network_schema.is_empty() {
            painter.text(rect.center(), Align2::CENTER_CENTER, t.t("view3d.empty"), FontId::proportional(14.0), pl.muted);
            return;
        }
        let cam = self.camera(&scene, rect);
        self.draw(&painter, &cam, &scene, rect);

        // Starea terenului
        ui.horizontal_wrapped(|ui| {
            let status = if map_ui::anchor(p).is_none() {
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
            self.zoom = (self.zoom / pinch * (-scroll / 400.0).exp()).clamp(0.05, 8.0);
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
        let focal = rect.height() / 2.0 / (25f32.to_radians()).tan();
        Camera { eye, right, up, fwd, focal, center: rect.center() }
    }

    fn draw(&self, painter: &egui::Painter, cam: &Camera, scene: &Scene, rect: Rect) {
        // Teren: triunghiuri sortate de la depărtare spre aproape, semitransparente
        if let Some(tr) = &scene.terrain {
            let light = norm([0.4, 1.0, 0.3]);
            let range = (tr.max_h - tr.min_h).max(0.01) as f32;
            let mut tris: Vec<(f32, [Pos2; 3], Color32)> = Vec::new();
            for j in 0..tr.ny - 1 {
                for i in 0..tr.nx - 1 {
                    let k = |a: usize, b: usize| b * tr.nx + a;
                    for idx in [[k(i, j), k(i + 1, j), k(i + 1, j + 1)], [k(i, j), k(i + 1, j + 1), k(i, j + 1)]] {
                        let v: Vec<[f32; 3]> = idx.iter().map(|&n| f3(tr.positions[n])).collect();
                        let n = norm(cross(sub(v[1], v[0]), sub(v[2], v[0])));
                        let shade = 0.6 + 0.4 * dot(if n[1] < 0.0 { [-n[0], -n[1], -n[2]] } else { n }, light).abs();
                        let h = idx.iter().map(|&n| tr.heights[n]).sum::<f64>() / 3.0;
                        let col = terrain_color(((h - tr.min_h) as f32 / range).clamp(0.0, 1.0), shade);
                        let pr: Vec<(Pos2, f32)> = v.iter().filter_map(|p| cam.project(*p)).collect();
                        if pr.len() == 3 {
                            tris.push(((pr[0].1 + pr[1].1 + pr[2].1) / 3.0, [pr[0].0, pr[1].0, pr[2].0], col));
                        }
                    }
                }
            }
            tris.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut mesh = Mesh::default();
            for (_, pts, col) in tris {
                let base = mesh.vertices.len() as u32;
                for p in pts {
                    mesh.colored_vertex(p, col);
                }
                mesh.add_triangle(base, base + 1, base + 2);
            }
            painter.add(Shape::mesh(mesh));
        }

        // Verticale punctate teren → conductă la capete
        for p in &scene.pipes {
            for (g, a) in [(p.ground_start, p.start), (p.ground_end, p.end)] {
                if let (Some(g), Some(a)) = (cam.project(f3(g)), cam.project(f3(a))) {
                    painter.add(Shape::dashed_line(&[g.0, a.0], Stroke::new(1.0, Color32::from_gray(120)), 4.0, 3.0));
                }
            }
        }
        // Conducte (grosime după De, limitată pe ecran)
        let mut pipes: Vec<(f32, &scene3d::Pipe)> =
            scene.pipes.iter().filter_map(|p| cam.project(f3([(p.start[0] + p.end[0]) / 2.0, (p.start[1] + p.end[1]) / 2.0, (p.start[2] + p.end[2]) / 2.0])).map(|(_, z)| (z, p))).collect();
        pipes.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (z, p) in &pipes {
            let (Some(a), Some(b)) = (cam.project(f3(p.start)), cam.project(f3(p.end))) else { continue };
            let d = if p.d_ext > 0.0 { p.d_ext / 1000.0 } else { 0.11 } as f32;
            let width = (d * cam.focal / z * 6.0).clamp(2.0, 12.0);
            let col = parse_color(p.color.as_deref()).filter(|c| *c != Color32::from_rgb(0x1a, 0x1a, 0x1a)).unwrap_or(Color32::from_rgb(0x25, 0x63, 0xEB));
            painter.line_segment([a.0, b.0], Stroke::new(width, col));
            if self.labels {
                let m = a.0 + (b.0 - a.0) / 2.0;
                let txt = if p.d_ext > 0.0 { format!("{} · De {} · {:.1} m", p.label, p.d_ext, p.length_m) } else { format!("{} · {:.1} m", p.label, p.length_m) };
                label(painter, m + Vec2::new(0.0, -12.0), &txt, Color32::from_black_alpha(185), Color32::WHITE);
            }
        }
        // Noduri
        for n in &scene.nodes {
            let (Some(g), Some(inv)) = (cam.project(f3(n.ground)), cam.project(f3(n.invert))) else { continue };
            let (col, h_px) = match n.kind {
                SchemaKind::Rezervor => (Color32::from_rgb(0xDC, 0x26, 0x26), 26.0),
                SchemaKind::Sursa => (Color32::from_rgb(0x33, 0x41, 0x55), 22.0),
                SchemaKind::Hidrant => (Color32::from_rgb(0xDC, 0x26, 0x26), 18.0),
                SchemaKind::Subsistem => (Color32::from_rgb(0x0D, 0x94, 0x88), 16.0),
                _ => (Color32::from_rgb(0x1D, 0x4E, 0xD8), 8.0),
            };
            painter.line_segment([inv.0, g.0], Stroke::new(2.0, col.gamma_multiply(0.7)));
            match n.kind {
                SchemaKind::Rezervor | SchemaKind::Sursa | SchemaKind::Subsistem => {
                    let r = Rect::from_min_max(g.0 + Vec2::new(-h_px * 0.7, -h_px), g.0 + Vec2::new(h_px * 0.7, 0.0));
                    painter.rect(r, 2.0, col.gamma_multiply(0.85), Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Middle);
                }
                SchemaKind::Hidrant => {
                    painter.line_segment([g.0, g.0 + Vec2::new(0.0, -h_px)], Stroke::new(5.0, col));
                    painter.circle_filled(g.0 + Vec2::new(0.0, -h_px), 4.0, col);
                }
                _ => {
                    painter.circle(g.0, h_px, Color32::WHITE, Stroke::new(2.0, col));
                }
            }
            if self.labels && !n.label.is_empty() {
                label(painter, g.0 + Vec2::new(0.0, -h_px - 12.0), &n.label, Color32::from_white_alpha(220), Color32::from_rgb(0x1E, 0x29, 0x3B));
            }
        }
        // Puncte topo
        for tp in &scene.topo {
            if let Some((s, _)) = cam.project(f3(*tp)) {
                let st = Stroke::new(1.5, Color32::from_rgb(0x15, 0x80, 0x3D));
                painter.line_segment([s + Vec2::new(-4.0, -4.0), s + Vec2::new(4.0, 4.0)], st);
                painter.line_segment([s + Vec2::new(-4.0, 4.0), s + Vec2::new(4.0, -4.0)], st);
            }
        }
        // Indicator de nord (axa −Z = nord pe plan)
        let o = rect.left_bottom() + Vec2::new(40.0, -40.0);
        let dir = |v: [f32; 3]| Vec2::new(dot(v, cam.right), -dot(v, cam.up)) * 24.0;
        painter.circle_filled(o, 30.0, Color32::from_white_alpha(170));
        for (v, c, txt) in [([0.0, 0.0, -1.0], Color32::from_rgb(0xDC, 0x26, 0x26), "N"), ([1.0, 0.0, 0.0], Color32::from_gray(90), "E"), ([0.0, 1.0, 0.0], Color32::from_gray(90), "↑")] {
            let e = o + dir(v);
            painter.line_segment([o, e], Stroke::new(2.0, c));
            painter.text(e + (e - o).normalized() * 8.0, Align2::CENTER_CENTER, txt, FontId::proportional(11.0), c);
        }
    }
}

fn label(painter: &egui::Painter, pos: Pos2, text: &str, bg: Color32, fg: Color32) {
    let g = painter.layout_no_wrap(text.into(), FontId::proportional(10.5), fg);
    let r = Rect::from_center_size(pos, g.size()).expand2(Vec2::new(5.0, 2.0));
    painter.rect_filled(r, 3.0, bg);
    painter.galley(r.min + Vec2::new(5.0, 2.0), g, fg);
}

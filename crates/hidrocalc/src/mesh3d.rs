//! Un mic randor 3D software pentru vederea rețelei: geometrie din triunghiuri cu
//! normale per vârf (umbrire netedă), iluminare ca în scena three.js din aplicația
//! React (ambientală + direcțională + emisferică) și desenare cu algoritmul pictorului
//! (toate triunghiurile și liniile sortate împreună după adâncime).

use eframe::egui::{self, Color32, Mesh, Pos2, Shape, Stroke};

pub type V3 = [f32; 3];

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn mul(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-9);
    mul(a, 1.0 / l)
}

/// Culoare liniară RGB 0..1 dintr-un `#rrggbb`.
pub fn rgb(hex: u32) -> V3 {
    [((hex >> 16) & 255) as f32 / 255.0, ((hex >> 8) & 255) as f32 / 255.0, (hex & 255) as f32 / 255.0]
}

pub fn lerp(a: V3, b: V3, t: f32) -> V3 {
    add(a, mul(sub(b, a), t))
}

#[derive(Clone, Copy)]
pub struct Vert {
    pub p: V3,
    pub n: V3,
    pub c: V3,
}

#[derive(Clone, Copy)]
pub struct Tri {
    pub v: [Vert; 3],
    pub alpha: f32,
    /// Fără culling: terenul se vede și de dedesubt.
    pub double_sided: bool,
}

#[derive(Clone, Copy)]
pub struct Line {
    pub a: V3,
    pub b: V3,
    pub color: Color32,
    pub width: f32,
    pub dashed: bool,
}

#[derive(Default)]
pub struct Scene3 {
    pub tris: Vec<Tri>,
    pub lines: Vec<Line>,
    /// Linii desenate înaintea întregii scene (grila de la bază), deci sub teren.
    pub underlay: Vec<Line>,
}

pub struct Light {
    pub dir: V3,
    pub ambient: f32,
    pub diffuse: f32,
    pub hemi: f32,
    pub sky: V3,
    pub ground: V3,
}

impl Light {
    pub fn standard() -> Self {
        // ambientLight 0.6 + directionalLight (span, 1.5 span, 0.5 span) 1.1 + hemisphere 0.3
        Self { dir: norm([1.0, 1.5, 0.5]), ambient: 0.45, diffuse: 0.75, hemi: 0.3, sky: [1.0, 1.0, 1.0], ground: rgb(0x64748B) }
    }

    pub fn shade(&self, c: V3, n: V3) -> V3 {
        let diff = dot(n, self.dir).max(0.0) * self.diffuse;
        let h = lerp(self.ground, self.sky, n[1] * 0.5 + 0.5);
        let k = |i: usize| (c[i] * (self.ambient + diff + h[i] * self.hemi)).clamp(0.0, 1.0);
        [k(0), k(1), k(2)]
    }
}

// ─── Geometrie ───────────────────────────────────────────────────────────────

fn ortho_basis(axis: V3) -> (V3, V3) {
    let helper = if axis[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let u = norm(cross(axis, helper));
    (u, cross(axis, u))
}

impl Scene3 {
    pub fn tri(&mut self, a: Vert, b: Vert, c: Vert, alpha: f32) {
        self.tris.push(Tri { v: [a, b, c], alpha, double_sided: false });
    }

    /// Cilindru (sau trunchi de con) între `a` și `b`, cu `seg` laturi; `caps` = capace.
    #[allow(clippy::too_many_arguments)]
    pub fn cylinder(&mut self, a: V3, b: V3, r0: f32, r1: f32, seg: usize, color: V3, alpha: f32, caps: bool) {
        let axis = sub(b, a);
        if dot(axis, axis) < 1e-12 {
            return;
        }
        let ax = norm(axis);
        let (u, v) = ortho_basis(ax);
        let ring = |r: f32, k: usize| {
            let t = k as f32 / seg as f32 * std::f32::consts::TAU;
            add(mul(u, t.cos() * r), mul(v, t.sin() * r))
        };
        let slope = (r0 - r1) / dot(axis, axis).sqrt();
        for k in 0..seg {
            let (o0, o1) = (ring(1.0, k), ring(1.0, k + 1));
            let n0 = norm(add(o0, mul(ax, slope)));
            let n1 = norm(add(o1, mul(ax, slope)));
            let p00 = add(a, mul(o0, r0));
            let p01 = add(a, mul(o1, r0));
            let p10 = add(b, mul(o0, r1));
            let p11 = add(b, mul(o1, r1));
            let vt = |p, n| Vert { p, n, c: color };
            self.tri(vt(p00, n0), vt(p01, n1), vt(p11, n1), alpha);
            if r1 > 0.0 {
                self.tri(vt(p00, n0), vt(p11, n1), vt(p10, n0), alpha);
            }
            if caps {
                let na = mul(ax, -1.0);
                self.tri(Vert { p: a, n: na, c: color }, Vert { p: p01, n: na, c: color }, Vert { p: p00, n: na, c: color }, alpha);
                if r1 > 0.0 {
                    self.tri(Vert { p: b, n: ax, c: color }, Vert { p: p10, n: ax, c: color }, Vert { p: p11, n: ax, c: color }, alpha);
                }
            }
        }
    }

    /// Cutie aliniată cu axele, cu centrul bazei în `base`.
    pub fn bx(&mut self, base: V3, w: f32, h: f32, d: f32, color: V3, alpha: f32) {
        let (x0, x1, y0, y1, z0, z1) = (base[0] - w / 2.0, base[0] + w / 2.0, base[1], base[1] + h, base[2] - d / 2.0, base[2] + d / 2.0);
        let faces: [(V3, [V3; 4]); 6] = [
            ([0.0, 1.0, 0.0], [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]]),
            ([0.0, -1.0, 0.0], [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]]),
            ([1.0, 0.0, 0.0], [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]]),
            ([-1.0, 0.0, 0.0], [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]]),
            ([0.0, 0.0, 1.0], [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]]),
            ([0.0, 0.0, -1.0], [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]]),
        ];
        for (n, q) in faces {
            let vt = |p| Vert { p, n, c: color };
            self.tri(vt(q[0]), vt(q[1]), vt(q[2]), alpha);
            self.tri(vt(q[0]), vt(q[2]), vt(q[3]), alpha);
        }
    }

    /// Sferă low-poly.
    pub fn sphere(&mut self, c: V3, r: f32, color: V3) {
        let (st, sl) = (6, 10);
        let pt = |i: usize, j: usize| {
            let th = i as f32 / st as f32 * std::f32::consts::PI;
            let ph = j as f32 / sl as f32 * std::f32::consts::TAU;
            [th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin()]
        };
        for i in 0..st {
            for j in 0..sl {
                let (a, b, cc, d) = (pt(i, j), pt(i + 1, j), pt(i + 1, j + 1), pt(i, j + 1));
                let vt = |n: V3| Vert { p: add(c, mul(n, r)), n, c: color };
                self.tri(vt(a), vt(b), vt(cc), 1.0);
                self.tri(vt(a), vt(cc), vt(d), 1.0);
            }
        }
    }

    pub fn line(&mut self, a: V3, b: V3, color: Color32, width: f32, dashed: bool) {
        self.lines.push(Line { a, b, color, width, dashed });
    }
}

// ─── Proiecție și desenare ───────────────────────────────────────────────────

pub struct Cam {
    pub eye: V3,
    pub right: V3,
    pub up: V3,
    pub fwd: V3,
    pub focal: f32,
    pub center: Pos2,
    pub near: f32,
}

impl Cam {
    pub fn project(&self, p: V3) -> Option<(Pos2, f32)> {
        let d = sub(p, self.eye);
        let z = dot(d, self.fwd);
        if z < self.near {
            return None;
        }
        Some((self.center + egui::vec2(dot(d, self.right) / z * self.focal, -dot(d, self.up) / z * self.focal), z))
    }
}

fn to_color(c: V3, alpha: f32) -> Color32 {
    let u = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(u(c[0]), u(c[1]), u(c[2]), u(alpha))
}

enum Item {
    Tri([Pos2; 3], [Color32; 3]),
    Line(Pos2, Pos2, Color32, f32, bool),
}

/// Desenează scena: proiectează, elimină fețele din spate, sortează de la depărtare
/// spre aproape și grupează triunghiurile consecutive într-un singur mesh.
pub fn render(painter: &egui::Painter, cam: &Cam, scene: &Scene3, light: &Light) {
    for l in &scene.underlay {
        if let (Some(a), Some(b)) = (cam.project(l.a), cam.project(l.b)) {
            painter.line_segment([a.0, b.0], Stroke::new(l.width, l.color));
        }
    }
    let mut items: Vec<(f32, Item)> = Vec::with_capacity(scene.tris.len() + scene.lines.len());
    for t in &scene.tris {
        let pr: Vec<(Pos2, f32)> = t.v.iter().filter_map(|v| cam.project(v.p)).collect();
        if pr.len() != 3 {
            continue;
        }
        let (a, b, c) = (pr[0].0, pr[1].0, pr[2].0);
        let area = (b - a).x * (c - a).y - (b - a).y * (c - a).x;
        // pe ecran y crește în jos, deci fețele orientate spre cameră au aria negativă
        let facing = area < 0.0;
        if !t.double_sided && !facing {
            continue;
        }
        let cols = t.v.map(|v| {
            let n = if facing || !t.double_sided { v.n } else { mul(v.n, -1.0) };
            to_color(light.shade(v.c, n), t.alpha)
        });
        let depth = (pr[0].1 + pr[1].1 + pr[2].1) / 3.0;
        items.push((depth, Item::Tri([a, b, c], cols)));
    }
    for l in &scene.lines {
        if let (Some(a), Some(b)) = (cam.project(l.a), cam.project(l.b)) {
            // liniile câștigă la egalitate cu fața pe care stau
            items.push(((a.1 + b.1) / 2.0 - 1e-3, Item::Line(a.0, b.0, l.color, l.width, l.dashed)));
        }
    }
    items.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut mesh = Mesh::default();
    let flush = |mesh: &mut Mesh| {
        if !mesh.is_empty() {
            painter.add(Shape::mesh(std::mem::take(mesh)));
        }
    };
    for (_, it) in items {
        match it {
            Item::Tri(p, c) => {
                let base = mesh.vertices.len() as u32;
                for (pp, cc) in p.iter().zip(c) {
                    mesh.colored_vertex(*pp, cc);
                }
                mesh.add_triangle(base, base + 1, base + 2);
            }
            Item::Line(a, b, c, w, dashed) => {
                flush(&mut mesh);
                if dashed {
                    painter.add(Shape::dashed_line(&[a, b], Stroke::new(w, c), 6.0, 4.0));
                } else {
                    painter.line_segment([a, b], Stroke::new(w, c));
                }
            }
        }
    }
    flush(&mut mesh);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_shapes_have_outward_normals_and_lighting_is_bounded() {
        let mut s = Scene3::default();
        s.bx([0.0, 0.0, 0.0], 2.0, 2.0, 2.0, [0.5; 3], 1.0);
        s.cylinder([0.0, 0.0, 0.0], [0.0, 3.0, 0.0], 1.0, 1.0, 12, [0.5; 3], 1.0, true);
        for t in &s.tris {
            let c = mul(add(add(t.v[0].p, t.v[1].p), t.v[2].p), 1.0 / 3.0);
            let geo_n = norm(cross(sub(t.v[1].p, t.v[0].p), sub(t.v[2].p, t.v[0].p)));
            // normala geometrică (ordinea vârfurilor) e aceeași cu normala declarată
            assert!(dot(geo_n, t.v[0].n) > 0.0, "triunghi întors la {c:?}");
        }
        let l = Light::standard();
        for n in [[0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0]] {
            let c = l.shade([1.0; 3], n);
            assert!(c.iter().all(|v| (0.0..=1.0).contains(v)));
        }
        assert!(l.shade([0.5; 3], [0.0, 1.0, 0.0])[0] > l.shade([0.5; 3], [0.0, -1.0, 0.0])[0]);
    }
}

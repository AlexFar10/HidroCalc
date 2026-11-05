//! Scena 3D a rețelei (portat din `buildScene3D.ts` + `terrainGrid.ts`).
//!
//! Coordonate: X = est [m], Y = sus [m] (cotă − bază, × exagerare), Z = sud [m],
//! centrate pe schemă. Conducta stă la cota terenului − adâncimea de pozare.

use crate::calculations::network::NetworkSection;
use crate::geo::MapAnchor;
use crate::model::{SchemaElement, SchemaKind};
use crate::schema::meters_per_px;
use serde::{Deserialize, Serialize};

const TOPO_INFLUENCE_M: f64 = 150.0;
const CONNECT_THRESH_PX: f64 = 24.0;
const CELL_M: f64 = 10.0;
const MIN_NODES: usize = 9;
const MAX_NODES: usize = 97;
const MARGIN_FRAC: f64 = 0.25;
const MIN_MARGIN_M: f64 = 50.0;

/// Grilă regulată de cote DEM peste zona schemei (cache în `networkCanvasSettings.terrainGrid`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerrainGrid {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub nx: usize,
    pub ny: usize,
    pub heights: Vec<Option<f64>>,
    pub key: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

pub fn terrain_key(a: &MapAnchor, rotation_deg: f64, scale_ratio: f64) -> String {
    format!("{:.7}|{:.7}|{:.2}|{:.2}|{:.5}|{:.3}|{}", a.lat, a.lng, a.world_x, a.world_y, a.tile_zoom, rotation_deg, scale_ratio)
}

pub fn schema_bounds(schema: &[SchemaElement]) -> Option<Bounds> {
    let mut b: Option<Bounds> = None;
    let mut add = |x: f64, y: f64| {
        b = Some(match b {
            None => Bounds { min_x: x, min_y: y, max_x: x, max_y: y },
            Some(o) => Bounds { min_x: o.min_x.min(x), min_y: o.min_y.min(y), max_x: o.max_x.max(x), max_y: o.max_y.max(y) },
        })
    };
    for e in schema.iter().filter(|e| e.kind != SchemaKind::Text) {
        add(e.x, e.y);
        if e.kind == SchemaKind::Tronson {
            add(e.x2.unwrap_or(e.x), e.y2.unwrap_or(e.y));
        }
    }
    b
}

pub fn grid_covers(g: Option<&TerrainGrid>, b: &Bounds, key: &str) -> bool {
    g.is_some_and(|g| g.key == key && g.min_x <= b.min_x && g.min_y <= b.min_y && g.max_x >= b.max_x && g.max_y >= b.max_y)
}

/// Nodurile grilei de teren (coordonate schemă) pentru zona dată, cu margine.
pub fn grid_nodes(b: &Bounds, scale_ratio: f64) -> (Bounds, usize, usize, Vec<(f64, f64)>) {
    let m = meters_per_px(scale_ratio);
    let (w, h) = ((b.max_x - b.min_x) * m, (b.max_y - b.min_y) * m);
    let margin = (w.max(h) * MARGIN_FRAC).max(MIN_MARGIN_M) / m;
    let gb = Bounds { min_x: b.min_x - margin, max_x: b.max_x + margin, min_y: b.min_y - margin, max_y: b.max_y + margin };
    let nodes = |len_px: f64| (((len_px * m) / CELL_M).ceil() as usize + 1).clamp(MIN_NODES, MAX_NODES);
    let (nx, ny) = (nodes(gb.max_x - gb.min_x), nodes(gb.max_y - gb.min_y));
    let mut pts = Vec::with_capacity(nx * ny);
    for j in 0..ny {
        let y = gb.min_y + (gb.max_y - gb.min_y) * j as f64 / (ny - 1) as f64;
        for i in 0..nx {
            pts.push((gb.min_x + (gb.max_x - gb.min_x) * i as f64 / (nx - 1) as f64, y));
        }
    }
    (gb, nx, ny, pts)
}

/// Interpolare biliniară în grilă; `None` în afara ei sau lângă noduri lipsă.
pub fn sample_grid(g: &TerrainGrid, x: f64, y: f64) -> Option<f64> {
    let fx = (x - g.min_x) / (g.max_x - g.min_x) * (g.nx - 1) as f64;
    let fy = (y - g.min_y) / (g.max_y - g.min_y) * (g.ny - 1) as f64;
    if !(fx >= 0.0 && fy >= 0.0 && fx <= (g.nx - 1) as f64 && fy <= (g.ny - 1) as f64) {
        return None;
    }
    let i0 = (fx.floor() as usize).min(g.nx - 2);
    let j0 = (fy.floor() as usize).min(g.ny - 2);
    let (tx, ty) = (fx - i0 as f64, fy - j0 as f64);
    let at = |i: usize, j: usize| g.heights.get(j * g.nx + i).copied().flatten();
    let (h00, h10, h01, h11) = (at(i0, j0)?, at(i0 + 1, j0)?, at(i0, j0 + 1)?, at(i0 + 1, j0 + 1)?);
    Some((h00 * (1.0 - tx) + h10 * tx) * (1.0 - ty) + (h01 * (1.0 - tx) + h11 * tx) * ty)
}

pub type Vec3 = [f64; 3];

#[derive(Debug, Clone)]
pub struct Pipe {
    pub label: String,
    pub start: Vec3,
    pub end: Vec3,
    pub ground_start: Vec3,
    pub ground_end: Vec3,
    pub d_ext: f64,
    pub color: Option<String>,
    pub length_m: f64,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: SchemaKind,
    pub label: String,
    pub ground: Vec3,
    pub invert: Vec3,
}

#[derive(Debug, Clone)]
pub struct Terrain {
    pub nx: usize,
    pub ny: usize,
    pub positions: Vec<Vec3>,
    pub heights: Vec<f64>,
    pub min_h: f64,
    pub max_h: f64,
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub terrain: Option<Terrain>,
    pub topo: Vec<Vec3>,
    pub pipes: Vec<Pipe>,
    pub nodes: Vec<Node>,
    pub center: Vec3,
    pub span: f64,
    pub base_elevation: f64,
}

pub struct Input<'a> {
    pub schema: &'a [SchemaElement],
    pub sections: &'a [NetworkSection],
    pub scale_ratio: f64,
    pub pipe_depth: f64,
    pub exaggeration: f64,
    pub reservoir_ground: f64,
    pub terrain: Option<&'a TerrainGrid>,
    /// Puncte topo în coordonate schemă: (x, y, cota).
    pub topo: &'a [(f64, f64, f64)],
}

pub fn build(inp: &Input) -> Scene {
    let m = meters_per_px(inp.scale_ratio);
    let grid = inp.terrain.filter(|g| g.heights.iter().any(Option::is_some) && g.nx >= 2 && g.ny >= 2);
    let residuals: Vec<(f64, f64, f64)> = match grid {
        Some(g) => inp.topo.iter().filter_map(|&(x, y, z)| sample_grid(g, x, y).map(|h| (x, y, z - h))).collect(),
        None => Vec::new(),
    };
    let r_inf = TOPO_INFLUENCE_M / m;
    let correction = |x: f64, y: f64| -> f64 {
        if residuals.is_empty() {
            return 0.0;
        }
        let (mut num, mut den) = (0.0, 1.0 / (r_inf * r_inf));
        for &(qx, qy, r) in &residuals {
            let d = (qx - x).hypot(qy - y);
            if d >= r_inf {
                continue;
            }
            let dd = d.max(1e-6);
            let w = (1.0 - d / r_inf).powi(2) / (dd * dd);
            num += w * r;
            den += w;
        }
        num / den
    };
    let terrain_at = |x: f64, y: f64| grid.and_then(|g| sample_grid(g, x, y)).map(|h| h + correction(x, y));

    let tronsoane: Vec<&SchemaElement> = inp.schema.iter().filter(|e| e.kind == SchemaKind::Tronson).collect();
    let nodes_raw: Vec<&SchemaElement> = inp.schema.iter().filter(|e| !matches!(e.kind, SchemaKind::Tronson | SchemaKind::Text)).collect();
    let by_name = |name: &str| inp.sections.iter().find(|s| s.group.is_none() && s.name == name);
    let mut ends: Vec<(f64, f64, Option<f64>)> = Vec::new();
    for tr in &tronsoane {
        let sec = tr.label.as_deref().and_then(by_name);
        ends.push((tr.x, tr.y, sec.and_then(|s| s.cota_up)));
        ends.push((tr.x2.unwrap_or(tr.x), tr.y2.unwrap_or(tr.y), sec.and_then(|s| s.cota_down)));
    }
    let known: Vec<(f64, f64, f64)> = ends.iter().filter_map(|&(x, y, c)| c.map(|c| (x, y, c))).collect();
    let res_ground = (inp.reservoir_ground.is_finite() && inp.reservoir_ground != 0.0).then_some(inp.reservoir_ground);
    let grid_vals: Vec<f64> = grid.map(|g| g.heights.iter().flatten().copied().collect()).unwrap_or_default();
    let fallback = if !known.is_empty() {
        known.iter().map(|e| e.2).sum::<f64>() / known.len() as f64
    } else if !grid_vals.is_empty() {
        grid_vals.iter().sum::<f64>() / grid_vals.len() as f64
    } else {
        res_ground.unwrap_or(0.0)
    };
    let ground_at = |x: f64, y: f64, own: Option<f64>| -> f64 {
        if let Some(o) = own {
            return o;
        }
        let mut best = None;
        let mut best_d = CONNECT_THRESH_PX;
        for &(kx, ky, c) in &known {
            let d = (kx - x).hypot(ky - y);
            if d <= best_d {
                best_d = d;
                best = Some(c);
            }
        }
        best.or_else(|| terrain_at(x, y)).unwrap_or(fallback)
    };

    let mut xs: Vec<f64> = ends.iter().map(|e| e.0).chain(nodes_raw.iter().map(|n| n.x)).collect();
    let mut ys: Vec<f64> = ends.iter().map(|e| e.1).chain(nodes_raw.iter().map(|n| n.y)).collect();
    if xs.is_empty() {
        xs.push(0.0);
        ys.push(0.0);
    }
    let (min_x, max_x) = xs.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let (min_y, max_y) = ys.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let (cx, cz) = ((min_x + max_x) / 2.0 * m, (min_y + max_y) / 2.0 * m);
    let span = ((max_x - min_x) * m).hypot((max_y - min_y) * m).max(10.0);

    let mut terrain_h: Option<Vec<f64>> = None;
    let (mut t_min, mut t_max) = (f64::INFINITY, f64::NEG_INFINITY);
    if let Some(g) = grid {
        let mut hs = vec![0.0; g.nx * g.ny];
        for j in 0..g.ny {
            let y = g.min_y + (g.max_y - g.min_y) * j as f64 / (g.ny - 1) as f64;
            for i in 0..g.nx {
                let x = g.min_x + (g.max_x - g.min_x) * i as f64 / (g.nx - 1) as f64;
                let h = g.heights[j * g.nx + i].unwrap_or(fallback) + correction(x, y);
                hs[j * g.nx + i] = h;
                t_min = t_min.min(h);
                t_max = t_max.max(h);
            }
        }
        terrain_h = Some(hs);
    }
    let mut all: Vec<f64> = known.iter().map(|e| e.2).collect();
    all.push(res_ground.unwrap_or(fallback));
    if terrain_h.is_some() {
        all.push(t_min);
    }
    let base = all.iter().cloned().fold(f64::INFINITY, f64::min) - inp.pipe_depth;
    let to_vec = |x: f64, y: f64, cota: f64| -> Vec3 { [x * m - cx, (cota - base) * inp.exaggeration, y * m - cz] };

    let pipes: Vec<Pipe> = tronsoane
        .iter()
        .enumerate()
        .map(|(i, tr)| {
            let sec = tr.label.as_deref().and_then(by_name);
            let (x2, y2) = (tr.x2.unwrap_or(tr.x), tr.y2.unwrap_or(tr.y));
            let g_up = ground_at(tr.x, tr.y, ends[i * 2].2);
            let g_dn = ground_at(x2, y2, ends[i * 2 + 1].2);
            Pipe {
                label: tr.label.clone().unwrap_or_else(|| "Tr".into()),
                start: to_vec(tr.x, tr.y, g_up - inp.pipe_depth),
                end: to_vec(x2, y2, g_dn - inp.pipe_depth),
                ground_start: to_vec(tr.x, tr.y, g_up),
                ground_end: to_vec(x2, y2, g_dn),
                d_ext: sec.map_or(0.0, |s| s.d_std_ext),
                color: sec.and_then(|s| s.color.clone()).or_else(|| tr.color.clone()),
                length_m: sec.map(|s| s.length).or(tr.override_length).unwrap_or(((x2 - tr.x).hypot(y2 - tr.y)) * m),
            }
        })
        .collect();
    let nodes: Vec<Node> = nodes_raw
        .iter()
        .map(|el| {
            let g = if el.kind == SchemaKind::Rezervor { res_ground.unwrap_or_else(|| ground_at(el.x, el.y, None)) } else { ground_at(el.x, el.y, None) };
            Node { kind: el.kind, label: el.label.clone().unwrap_or_default(), ground: to_vec(el.x, el.y, g), invert: to_vec(el.x, el.y, g - inp.pipe_depth) }
        })
        .collect();
    let terrain = match (grid, terrain_h) {
        (Some(g), Some(hs)) => {
            let mut positions = Vec::with_capacity(g.nx * g.ny);
            for j in 0..g.ny {
                let y = g.min_y + (g.max_y - g.min_y) * j as f64 / (g.ny - 1) as f64;
                for i in 0..g.nx {
                    let x = g.min_x + (g.max_x - g.min_x) * i as f64 / (g.nx - 1) as f64;
                    positions.push(to_vec(x, y, hs[j * g.nx + i]));
                }
            }
            Some(Terrain { nx: g.nx, ny: g.ny, positions, heights: hs, min_h: t_min, max_h: t_max })
        }
        _ => None,
    };
    let topo: Vec<Vec3> = inp.topo.iter().map(|&(x, y, z)| to_vec(x, y, z)).collect();
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut add = |v: &Vec3| {
        lo = lo.min(v[1]);
        hi = hi.max(v[1]);
    };
    for p in &pipes {
        [p.start, p.end, p.ground_start, p.ground_end].iter().for_each(&mut add);
    }
    nodes.iter().for_each(|n| add(&n.ground));
    if terrain.is_some() {
        add(&[0.0, (t_min - base) * inp.exaggeration, 0.0]);
        add(&[0.0, (t_max - base) * inp.exaggeration, 0.0]);
    }
    let cy = if lo.is_finite() { (lo + hi) / 2.0 } else { 0.0 };
    Scene { terrain, topo, pipes, nodes, center: [0.0, cy, 0.0], span, base_elevation: base }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_sits_at_ground_minus_depth_and_grid_samples() {
        let tr = SchemaElement { kind: SchemaKind::Tronson, x: 0.0, y: 0.0, x2: Some(1000.0), y2: Some(0.0), label: Some("Tr1".into()), ..Default::default() };
        let mut s = NetworkSection::new("a".into(), "Tr1".into());
        s.cota_up = Some(100.0);
        s.cota_down = Some(95.0);
        let inp = Input { schema: &[tr], sections: &[s], scale_ratio: 1000.0, pipe_depth: 1.1, exaggeration: 2.0, reservoir_ground: 0.0, terrain: None, topo: &[] };
        let sc = build(&inp);
        let p = &sc.pipes[0];
        assert!((sc.base_elevation - (95.0 - 1.1)).abs() < 1e-9);
        assert!((p.ground_start[1] - (100.0 - sc.base_elevation) * 2.0).abs() < 1e-9);
        assert!((p.ground_start[1] - p.start[1] - 2.2).abs() < 1e-9, "adâncimea × exagerarea");
        assert!((p.end[0] - p.start[0] - 264.58).abs() < 1e-6);

        let g = TerrainGrid { min_x: 0.0, min_y: 0.0, max_x: 10.0, max_y: 10.0, nx: 2, ny: 2, heights: vec![Some(0.0), Some(10.0), Some(10.0), Some(20.0)], key: String::new() };
        assert_eq!(sample_grid(&g, 5.0, 5.0), Some(10.0));
        assert_eq!(sample_grid(&g, 11.0, 5.0), None);
        let (_, nx, ny, pts) = grid_nodes(&Bounds { min_x: 0.0, min_y: 0.0, max_x: 1000.0, max_y: 0.0 }, 1000.0);
        assert!(nx >= MIN_NODES && ny >= MIN_NODES && pts.len() == nx * ny && nx <= MAX_NODES);
    }
}

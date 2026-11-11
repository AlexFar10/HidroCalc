//! Export al modelului 3D (ce se vede în Vedere 3D): GLB (glTF binar), STL binar și DXF 3D.
//!
//! Intrarea e geometria grupată pe obiecte din `view3d::model_groups`. Scena folosește Y în
//! sus (ca three.js), în metri; GLB păstrează axele, STL și DXF folosesc Z în sus
//! (`(x, y, z) → (x, −z, y)`, o rotație, deci orientarea rămâne corectă).

use crate::mesh3d::{cross, dot, norm, sub, Scene3, Tri, V3};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormat {
    Glb,
    Stl,
    Dxf,
}

type Groups = [(String, Scene3)];

/// Triunghiul cu înfășurarea aliniată la normalele vârfurilor (față spre exterior).
fn oriented(t: &Tri) -> [[f32; 3]; 3] {
    let [a, b, c] = [t.v[0].p, t.v[1].p, t.v[2].p];
    let face = cross(sub(b, a), sub(c, a));
    let avg = [t.v[0].n[0] + t.v[1].n[0] + t.v[2].n[0], t.v[0].n[1] + t.v[1].n[1] + t.v[2].n[1], t.v[0].n[2] + t.v[1].n[2] + t.v[2].n[2]];
    if dot(face, avg) < 0.0 {
        [a, c, b]
    } else {
        [a, b, c]
    }
}

fn nonempty(groups: &Groups) -> Result<(), String> {
    if groups.iter().any(|(_, g)| !g.tris.is_empty()) {
        Ok(())
    } else {
        Err("modelul 3D e gol".into())
    }
}

/// Z în sus: `(x, y, z) → (x, −z, y)`.
fn z_up(p: V3) -> V3 {
    [p[0], -p[2], p[1]]
}

// ─── STL ─────────────────────────────────────────────────────────────────────

pub fn stl(groups: &Groups) -> Result<Vec<u8>, String> {
    nonempty(groups)?;
    let tris: Vec<[V3; 3]> = groups.iter().flat_map(|(_, g)| g.tris.iter()).map(|t| oriented(t).map(z_up)).collect();
    let mut out = Vec::with_capacity(84 + tris.len() * 50);
    let mut header = b"HidroCalc 3D model (metri, Z in sus)".to_vec();
    header.resize(80, b' ');
    out.extend_from_slice(&header);
    out.extend_from_slice(&(tris.len() as u32).to_le_bytes());
    for [a, b, c] in tris {
        let n = norm(cross(sub(b, a), sub(c, a)));
        for v in [n, a, b, c] {
            for x in v {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    Ok(out)
}

// ─── DXF 3D ──────────────────────────────────────────────────────────────────

/// Numele unui strat DXF: litere mari, cifre, `_` și `-`.
fn layer_name(s: &str) -> String {
    let n: String = s
        .chars()
        .map(|c| match c {
            'ă' | 'â' | 'Ă' | 'Â' => 'A',
            'î' | 'Î' => 'I',
            'ș' | 'ş' | 'Ș' | 'Ş' => 'S',
            'ț' | 'ţ' | 'Ț' | 'Ţ' => 'T',
            c if c.is_ascii_alphanumeric() => c.to_ascii_uppercase(),
            _ => '_',
        })
        .collect();
    if n.is_empty() {
        "MODEL".into()
    } else {
        n
    }
}

/// Cea mai apropiată culoare AutoCAD (ACI) dintre cele de bază.
fn aci(c: V3) -> u32 {
    const BASE: [(u32, [f32; 3]); 10] = [
        (1, [1.0, 0.0, 0.0]),
        (2, [1.0, 1.0, 0.0]),
        (3, [0.0, 1.0, 0.0]),
        (4, [0.0, 1.0, 1.0]),
        (5, [0.0, 0.0, 1.0]),
        (6, [1.0, 0.0, 1.0]),
        (7, [1.0, 1.0, 1.0]),
        (8, [0.5, 0.5, 0.5]),
        (9, [0.75, 0.75, 0.75]),
        (30, [1.0, 0.5, 0.0]),
    ];
    BASE.iter()
        .min_by(|a, b| {
            let d = |x: &[f32; 3]| (x[0] - c[0]).powi(2) + (x[1] - c[1]).powi(2) + (x[2] - c[2]).powi(2);
            d(&a.1).total_cmp(&d(&b.1))
        })
        .map_or(7, |b| b.0)
}

/// DXF (AC1009) cu fețe `3DFACE`; fiecare obiect (teren, conductă, nod) e pe stratul lui.
pub fn dxf(groups: &Groups) -> Result<String, String> {
    nonempty(groups)?;
    let mut layers: Vec<(String, u32)> = Vec::new();
    for (name, g) in groups {
        let Some(first) = g.tris.first() else { continue };
        let ln = layer_name(name);
        if !layers.iter().any(|(n, _)| *n == ln) {
            layers.push((ln, aci(first.v[0].c)));
        }
    }
    let mut s = String::new();
    let mut put = |code: i32, v: &dyn std::fmt::Display| s.push_str(&format!("{code:>3}\n{v}\n"));
    put(0, &"SECTION");
    put(2, &"HEADER");
    put(9, &"$ACADVER");
    put(1, &"AC1009");
    put(9, &"$INSUNITS");
    put(70, &6);
    put(0, &"ENDSEC");
    put(0, &"SECTION");
    put(2, &"TABLES");
    put(0, &"TABLE");
    put(2, &"LTYPE");
    put(70, &1);
    put(0, &"LTYPE");
    put(2, &"CONTINUOUS");
    put(70, &0);
    put(3, &"Solid line");
    put(72, &65);
    put(73, &0);
    put(40, &"0.0");
    put(0, &"ENDTAB");
    put(0, &"TABLE");
    put(2, &"LAYER");
    put(70, &layers.len());
    for (name, color) in &layers {
        put(0, &"LAYER");
        put(2, name);
        put(70, &0);
        put(62, color);
        put(6, &"CONTINUOUS");
    }
    put(0, &"ENDTAB");
    put(0, &"ENDSEC");
    put(0, &"SECTION");
    put(2, &"ENTITIES");
    for (name, g) in groups {
        let ln = layer_name(name);
        for t in &g.tris {
            let [a, b, c] = oriented(t).map(z_up);
            put(0, &"3DFACE");
            put(8, &ln);
            // al patrulea colț = al treilea (față triunghiulară)
            for (k, p) in [a, b, c, c].iter().enumerate() {
                for (axis, base) in [(0, 10), (1, 20), (2, 30)] {
                    put(base + k as i32, &format!("{:.4}", p[axis]));
                }
            }
        }
    }
    put(0, &"ENDSEC");
    put(0, &"EOF");
    Ok(s)
}

// ─── GLB ─────────────────────────────────────────────────────────────────────

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[derive(Default)]
struct Bin {
    data: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Bin {
    /// Adaugă un accessor f32 (`VEC3` sau `VEC4`) și întoarce indexul lui.
    fn accessor(&mut self, values: &[f32], comps: usize, bounds: bool) -> usize {
        while !self.data.len().is_multiple_of(4) {
            self.data.push(0);
        }
        let offset = self.data.len();
        for v in values {
            self.data.extend_from_slice(&v.to_le_bytes());
        }
        self.views.push(json!({ "buffer": 0, "byteOffset": offset, "byteLength": values.len() * 4, "target": 34962 }));
        let count = values.len() / comps;
        let mut acc = json!({
            "bufferView": self.views.len() - 1,
            "componentType": 5126,
            "count": count,
            "type": if comps == 3 { "VEC3" } else { "VEC4" },
        });
        if bounds {
            let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in values.chunks(3) {
                for i in 0..3 {
                    mn[i] = mn[i].min(p[i]);
                    mx[i] = mx[i].max(p[i]);
                }
            }
            acc["min"] = json!(mn);
            acc["max"] = json!(mx);
        }
        self.accessors.push(acc);
        self.accessors.len() - 1
    }
}

/// glTF 2.0 binar: câte un nod pe obiect (teren, conducte, noduri), material pe culoare.
pub fn glb(groups: &Groups) -> Result<Vec<u8>, String> {
    nonempty(groups)?;
    let mut bin = Bin::default();
    let mut materials: Vec<Value> = Vec::new();
    let mut material_ix: BTreeMap<String, usize> = BTreeMap::new();
    let mut meshes: Vec<Value> = Vec::new();
    let mut nodes: Vec<Value> = Vec::new();

    for (name, g) in groups {
        if g.tris.is_empty() {
            continue;
        }
        // Triunghiurile unui obiect, împărțite pe material: transparente (teren, cu culori pe vârfuri)
        // și opace (câte o culoare).
        let mut opaque: BTreeMap<[u8; 3], Vec<&Tri>> = BTreeMap::new();
        let mut blend: Vec<&Tri> = Vec::new();
        for t in &g.tris {
            if t.alpha < 0.999 {
                blend.push(t);
            } else {
                let q = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
                opaque.entry([q(t.v[0].c[0]), q(t.v[0].c[1]), q(t.v[0].c[2])]).or_default().push(t);
            }
        }
        let mut prims: Vec<Value> = Vec::new();
        let mut add_prim = |bin: &mut Bin, tris: &[&Tri], mat: usize, colors: bool| {
            let mut pos = Vec::with_capacity(tris.len() * 9);
            let mut nor = Vec::with_capacity(tris.len() * 9);
            let mut col = Vec::new();
            for t in tris {
                let o = oriented(t);
                // normalele vârfurilor în aceeași ordine ca poziția rearanjată
                for p in o {
                    let v = t.v.iter().find(|v| v.p == p).unwrap_or(&t.v[0]);
                    pos.extend_from_slice(&p);
                    nor.extend_from_slice(&v.n);
                    if colors {
                        col.extend_from_slice(&[srgb_to_linear(v.c[0]), srgb_to_linear(v.c[1]), srgb_to_linear(v.c[2]), t.alpha]);
                    }
                }
            }
            let mut attrs = json!({ "POSITION": bin.accessor(&pos, 3, true), "NORMAL": bin.accessor(&nor, 3, false) });
            if colors {
                attrs["COLOR_0"] = json!(bin.accessor(&col, 4, false));
            }
            prims.push(json!({ "attributes": attrs, "material": mat, "mode": 4 }));
        };
        let mut material = |key: String, make: &dyn Fn() -> Value| -> usize {
            *material_ix.entry(key).or_insert_with(|| {
                materials.push(make());
                materials.len() - 1
            })
        };
        for (c, tris) in &opaque {
            let lin = [srgb_to_linear(c[0] as f32 / 255.0), srgb_to_linear(c[1] as f32 / 255.0), srgb_to_linear(c[2] as f32 / 255.0)];
            let m = material(format!("o{c:?}"), &|| {
                json!({
                    "name": format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]),
                    "pbrMetallicRoughness": { "baseColorFactor": [lin[0], lin[1], lin[2], 1.0], "metallicFactor": 0.0, "roughnessFactor": 0.8 },
                })
            });
            add_prim(&mut bin, tris, m, false);
        }
        if !blend.is_empty() {
            let m = material("teren".into(), &|| {
                json!({
                    "name": "Teren",
                    "doubleSided": true,
                    "alphaMode": "BLEND",
                    "pbrMetallicRoughness": { "baseColorFactor": [1.0, 1.0, 1.0, 1.0], "metallicFactor": 0.0, "roughnessFactor": 1.0 },
                })
            });
            add_prim(&mut bin, &blend, m, true);
        }
        meshes.push(json!({ "name": name, "primitives": prims }));
        nodes.push(json!({ "name": name, "mesh": meshes.len() - 1 }));
    }

    let root = json!({
        "asset": { "version": "2.0", "generator": "HidroCalc" },
        "scene": 0,
        "scenes": [{ "nodes": (0..nodes.len()).collect::<Vec<_>>() }],
        "nodes": nodes,
        "meshes": meshes,
        "materials": materials,
        "accessors": bin.accessors,
        "bufferViews": bin.views,
        "buffers": [{ "byteLength": bin.data.len() }],
    });
    let mut json_bytes = serde_json::to_vec(&root).map_err(|e| e.to_string())?;
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let mut data = bin.data;
    while data.len() % 4 != 0 {
        data.push(0);
    }
    let total = 12 + 8 + json_bytes.len() + 8 + data.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn export(groups: &Groups, format: ModelFormat) -> Result<Vec<u8>, String> {
    match format {
        ModelFormat::Glb => glb(groups),
        ModelFormat::Stl => stl(groups),
        ModelFormat::Dxf => dxf(groups).map(String::into_bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh3d::{rgb, Vert};

    fn sample() -> Vec<(String, Scene3)> {
        let mut pipe = Scene3::default();
        pipe.cylinder([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 0.2, 0.2, 8, rgb(0x2563EB), 1.0, true);
        let mut ground = Scene3::default();
        let v = |x: f32, z: f32| Vert { p: [x, 0.0, z], n: [0.0, 1.0, 0.0], c: rgb(0x4D7C0F) };
        ground.tris.push(Tri { v: [v(0.0, 0.0), v(0.0, 5.0), v(5.0, 0.0)], alpha: 0.55, layer: 1, double_sided: true });
        vec![("Teren".into(), ground), ("Conducta Tr1".into(), pipe)]
    }

    #[test]
    fn glb_is_valid_container_with_named_nodes() {
        let g = sample();
        let b = glb(&g).unwrap();
        assert_eq!(&b[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize, b.len());
        assert_eq!(b.len() % 4, 0);
        let jlen = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
        assert_eq!(&b[16..20], b"JSON");
        let json: Value = serde_json::from_slice(&b[20..20 + jlen]).unwrap();
        let names: Vec<&str> = json["nodes"].as_array().unwrap().iter().map(|n| n["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["Teren", "Conducta Tr1"]);
        assert_eq!(json["materials"].as_array().unwrap().len(), 2);
        let blen = json["buffers"][0]["byteLength"].as_u64().unwrap() as usize;
        let tail = 20 + jlen;
        assert_eq!(u32::from_le_bytes(b[tail..tail + 4].try_into().unwrap()) as usize, blen);
        assert_eq!(&b[tail + 4..tail + 8], b"BIN\0");
    }

    #[test]
    fn stl_has_header_count_and_50_bytes_per_triangle() {
        let g = sample();
        let n: usize = g.iter().map(|(_, s)| s.tris.len()).sum();
        let b = stl(&g).unwrap();
        assert_eq!(b.len(), 84 + 50 * n);
        assert_eq!(u32::from_le_bytes(b[80..84].try_into().unwrap()) as usize, n);
    }

    #[test]
    fn dxf_has_layers_and_3dfaces() {
        let d = dxf(&sample()).unwrap();
        assert!(d.contains("  0\n3DFACE\n"));
        assert!(d.contains("  8\nCONDUCTA_TR1\n"));
        assert!(d.contains("  2\nTEREN\n"));
        assert!(d.trim_end().ends_with("EOF"));
    }

    #[test]
    fn empty_model_is_an_error() {
        assert!(glb(&[]).is_err() && stl(&[]).is_err() && dxf(&[]).is_err());
    }
}

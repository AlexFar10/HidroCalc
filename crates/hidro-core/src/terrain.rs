//! Completarea automată a cotelor de teren ale tronsoanelor (portat din `terrainSync.ts`).
//!
//! Reguli: doar tronsoanele schemei principale; doar capetele fără cotă sau cu cotă
//! automată („dem”/„topo”); un punct topo din raza dată are prioritate față de DEM;
//! o cotă introdusă manual nu se suprascrie niciodată.

use crate::calculations::network::NetworkSection;
use crate::model::{SchemaElement, SchemaKind};

pub const DEFAULT_TOPO_RADIUS_M: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum End {
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub section_id: String,
    pub end: End,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Value {
    pub h: f64,
    pub source: &'static str,
}

fn get(s: &NetworkSection, end: End) -> (Option<f64>, Option<&str>) {
    match end {
        End::Up => (s.cota_up, s.cota_up_source.as_deref()),
        End::Down => (s.cota_down, s.cota_down_source.as_deref()),
    }
}

fn eligible(s: &NetworkSection, end: End) -> bool {
    let (v, src) = get(s, end);
    v.is_none() || matches!(src, Some("dem" | "topo"))
}

/// Capetele de tronson care pot primi cotă, cu poziția lor pe plan.
pub fn collect_targets(schema: &[SchemaElement], sections: &[NetworkSection]) -> Vec<Target> {
    let mut used = std::collections::HashSet::new();
    let mut out = Vec::new();
    for tr in schema.iter().filter(|e| e.kind == SchemaKind::Tronson) {
        let Some(sec) = crate::schema::section_for(tr, sections) else { continue };
        if !used.insert(sec.id.clone()) {
            continue;
        }
        if eligible(sec, End::Up) {
            out.push(Target { section_id: sec.id.clone(), end: End::Up, x: tr.x, y: tr.y });
        }
        if eligible(sec, End::Down) {
            out.push(Target { section_id: sec.id.clone(), end: End::Down, x: tr.x2.unwrap_or(tr.x), y: tr.y2.unwrap_or(tr.y) });
        }
    }
    out
}

/// Cota pentru fiecare țintă: punctul topo cel mai apropiat din rază, altfel DEM.
pub fn resolve(targets: &[Target], dem: &[Option<f64>], topo: &[(f64, f64, f64)], radius_px: f64) -> Vec<Option<Value>> {
    targets
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut best = None;
            let mut best_d = radius_px;
            for &(x, y, z) in topo {
                let d = (x - t.x).hypot(y - t.y);
                if d <= best_d {
                    best_d = d;
                    best = Some(z);
                }
            }
            if let Some(h) = best {
                return Some(Value { h, source: "topo" });
            }
            dem.get(i).copied().flatten().map(|h| Value { h, source: "dem" })
        })
        .collect()
}

/// Aplică valorile pe tronsoanele curente; `true` dacă s-a schimbat ceva.
pub fn apply(sections: &mut [NetworkSection], targets: &[Target], values: &[Option<Value>]) -> bool {
    let mut changed = false;
    for (t, v) in targets.iter().zip(values) {
        let Some(v) = v else { continue };
        let Some(s) = sections.iter_mut().find(|s| s.id == t.section_id) else { continue };
        if !eligible(s, t.end) {
            continue;
        }
        let (cur, src) = get(s, t.end);
        if src == Some(v.source) && cur.is_some_and(|c| (c - v.h).abs() < 0.005) {
            continue;
        }
        let h = (v.h * 100.0).round() / 100.0;
        match t.end {
            End::Up => {
                s.cota_up = Some(h);
                s.cota_up_source = Some(v.source.into());
            }
            End::Down => {
                s.cota_down = Some(h);
                s.cota_down_source = Some(v.source.into());
            }
        }
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_values_are_never_overwritten_and_topo_wins() {
        let tr = SchemaElement { kind: SchemaKind::Tronson, x: 0.0, y: 0.0, x2: Some(100.0), y2: Some(0.0), label: Some("Tr1".into()), ..Default::default() };
        let mut s = NetworkSection::new("a".into(), "Tr1".into());
        s.cota_up = Some(50.0);
        s.cota_up_source = Some("manual".into());
        let mut secs = vec![s];
        let targets = collect_targets(&[tr], &secs);
        assert_eq!(targets.len(), 1); // doar capătul aval
        let vals = resolve(&targets, &[Some(90.0)], &[(101.0, 1.0, 88.4)], 5.0);
        assert_eq!(vals[0], Some(Value { h: 88.4, source: "topo" }));
        assert!(apply(&mut secs, &targets, &vals));
        assert_eq!((secs[0].cota_up, secs[0].cota_down), (Some(50.0), Some(88.4)));
        assert!(!apply(&mut secs, &targets, &vals)); // idempotent
    }
}

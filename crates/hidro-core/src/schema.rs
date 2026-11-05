//! Operații pe schema rețelei: conversia pixeli → metri și importul topologiei
//! tronsoanelor din schemă (portat din `buildSectionsFromSchema`, Step8Network.tsx).

use crate::calculations::network::NetworkSection;
use crate::model::{SchemaElement, SchemaKind};

/// Metri pe pixel-lume la scara 1:`scale_ratio` (1 px = 0.26458 mm pe desen).
pub fn meters_per_px(scale_ratio: f64) -> f64 {
    scale_ratio * 0.26458 / 1000.0
}

/// Toleranța [px-lume] la care două capete sunt considerate conectate.
pub const CONNECT_TOLERANCE: f64 = 12.0;

/// Punctele de conexiune ale unui element care nu e tronson.
pub fn connection_points(el: &SchemaElement) -> Vec<(f64, f64)> {
    match el.kind {
        SchemaKind::Rezervor | SchemaKind::Sursa => {
            let w = el.w.unwrap_or(44.0);
            vec![(el.x + w / 2.0, el.y)]
        }
        SchemaKind::Hidrant | SchemaKind::CaminGrav | SchemaKind::CaminAer => {
            let r = el.r.unwrap_or(7.0);
            vec![(el.x - r, el.y), (el.x + r, el.y), (el.x, el.y - r), (el.x, el.y + r)]
        }
        _ => vec![(el.x, el.y)],
    }
}

/// Toate elementele (inclusiv din subsisteme), cu grupul lor ("" = schema principală).
pub fn flatten_with_group(schema: &[SchemaElement]) -> Vec<(&SchemaElement, String)> {
    fn go<'a>(els: &'a [SchemaElement], group: &str, out: &mut Vec<(&'a SchemaElement, String)>) {
        for el in els {
            if el.kind == SchemaKind::Subsistem {
                let g = el.label.clone().unwrap_or_else(|| "Subsistem".into());
                go(el.children.as_deref().unwrap_or_default(), &g, out);
            } else {
                out.push((el, group.to_string()));
            }
        }
    }
    let mut out = Vec::new();
    go(schema, "", &mut out);
    out
}

pub fn tronson_end(el: &SchemaElement) -> (f64, f64) {
    (el.x2.unwrap_or(el.x), el.y2.unwrap_or(el.y))
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

pub fn count_tronsoane(schema: &[SchemaElement]) -> usize {
    flatten_with_group(schema).iter().filter(|(e, _)| e.kind == SchemaKind::Tronson).count()
}

/// Tronsoanele deduse din schemă: lungimi din coordonate × scară (sau lungimea
/// suprascrisă), părinte din conexiuni, hidrant la capătul aval. Datele introduse
/// (cote, debite, ζ) se păstrează din tronsoanele existente cu același nume.
/// `new_id` generează id-uri pentru tronsoanele noi.
pub fn build_sections_from_schema(
    schema: &[SchemaElement],
    existing: &[NetworkSection],
    scale_ratio: f64,
    mut new_id: impl FnMut() -> String,
) -> Vec<NetworkSection> {
    let tagged = flatten_with_group(schema);
    let tronsoane: Vec<&(&SchemaElement, String)> = tagged.iter().filter(|(e, _)| e.kind == SchemaKind::Tronson).collect();
    if tronsoane.is_empty() {
        return existing.to_vec();
    }
    let nodes: Vec<&SchemaElement> =
        tagged.iter().map(|(e, _)| *e).filter(|e| !matches!(e.kind, SchemaKind::Tronson | SchemaKind::Text)).collect();

    let mpp = meters_per_px(scale_ratio);
    let mut matched = std::collections::HashSet::new();
    let mut sections: Vec<NetworkSection> = tronsoane
        .iter()
        .map(|(tr, group)| {
            let (ex, ey) = tronson_end(tr);
            let len_m = (ex - tr.x).hypot(ey - tr.y) * mpp;
            let name = tr.label.clone().unwrap_or_else(|| "Tr".into());
            let prev = existing.iter().find(|s| s.name == name && !matched.contains(&s.id));
            if let Some(p) = prev {
                matched.insert(p.id.clone());
            }
            let mut s = NetworkSection::new(prev.map_or_else(&mut new_id, |p| p.id.clone()), name);
            s.group = (!group.is_empty()).then(|| group.clone());
            s.length = tr.override_length.unwrap_or((len_m * 10.0).round() / 10.0);
            if let Some(p) = prev {
                s.cota_up = p.cota_up;
                s.cota_down = p.cota_down;
                s.cota_up_source = p.cota_up_source.clone();
                s.cota_down_source = p.cota_down_source.clone();
                s.q_extern = p.q_extern;
                s.q_linear = p.q_linear;
                s.zeta = p.zeta;
                s.color = p.color.clone();
            }
            s
        })
        .collect();

    let ids: Vec<String> = sections.iter().map(|s| s.id.clone()).collect();
    for (i, (tr, _)) in tronsoane.iter().enumerate() {
        let start = (tr.x, tr.y);
        let mut parent = None;
        // 1) începutul tronsonului e lângă capătul aval al altui tronson
        for (j, (other, _)) in tronsoane.iter().enumerate() {
            if other.id != tr.id && dist(tronson_end(other), start) < CONNECT_TOLERANCE {
                parent = Some(ids[j].clone());
                break;
            }
        }
        // 2) începutul e lângă un nod la care ajunge capătul aval al altui tronson
        if parent.is_none() {
            'nodes: for node in &nodes {
                let pts = connection_points(node);
                if !pts.iter().any(|&p| dist(p, start) < CONNECT_TOLERANCE) {
                    continue;
                }
                for (j, (other, _)) in tronsoane.iter().enumerate() {
                    if other.id == tr.id {
                        continue;
                    }
                    let end = tronson_end(other);
                    if pts.iter().any(|&p| dist(p, end) < CONNECT_TOLERANCE) {
                        parent = Some(ids[j].clone());
                        break 'nodes;
                    }
                }
            }
        }
        sections[i].parent_section_id = parent;
        let end = tronson_end(tr);
        sections[i].has_hydrant = nodes
            .iter()
            .any(|n| n.kind == SchemaKind::Hidrant && connection_points(n).iter().any(|&p| dist(p, end) < CONNECT_TOLERANCE));
    }

    // Grupurile în ordinea apariției (schema principală prima).
    let mut order: Vec<String> = vec![String::new()];
    for (_, g) in &tagged {
        if !g.is_empty() && !order.contains(g) {
            order.push(g.clone());
        }
    }
    sections.sort_by_key(|s| order.iter().position(|g| g == s.group.as_deref().unwrap_or("")).unwrap_or(usize::MAX));
    sections
}

/// Setează `overrideLength` pe tronsonul cu eticheta dată (caută și în subsisteme).
pub fn set_tronson_override_length(schema: &mut [SchemaElement], label: &str, len: Option<f64>) -> bool {
    for el in schema.iter_mut() {
        if el.kind == SchemaKind::Tronson && el.label.as_deref() == Some(label) {
            el.override_length = len;
            return true;
        }
        if let Some(ch) = el.children.as_mut() {
            if set_tronson_override_length(ch, label, len) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tr(id: &str, label: &str, a: (f64, f64), b: (f64, f64)) -> SchemaElement {
        SchemaElement { id: id.into(), kind: SchemaKind::Tronson, x: a.0, y: a.1, x2: Some(b.0), y2: Some(b.1), label: Some(label.into()), ..Default::default() }
    }

    #[test]
    fn topology_lengths_and_hydrants() {
        // R ── Tr1 ── (hidrant) ── Tr2 ;  Tr1 ── Tr3 (direct)
        let schema = vec![
            tr("a", "Tr1", (0.0, 0.0), (1000.0, 0.0)),
            SchemaElement { id: "h".into(), kind: SchemaKind::Hidrant, x: 1007.0, y: 0.0, ..Default::default() },
            tr("b", "Tr2", (1014.0, 0.0), (1500.0, 0.0)),
            tr("c", "Tr3", (1003.0, 2.0), (1003.0, 400.0)),
        ];
        let mut n = 0;
        let s = build_sections_from_schema(&schema, &[], 1000.0, || {
            n += 1;
            format!("id{n}")
        });
        assert_eq!(s.len(), 3);
        // 1000 px × 0.26458 m/px = 264.58 m → 264.6
        assert_eq!(s[0].length, 264.6);
        assert_eq!(s[0].parent_section_id, None);
        assert!(s[0].has_hydrant);
        assert_eq!(s[1].parent_section_id.as_deref(), Some("id1")); // prin hidrant
        assert_eq!(s[2].parent_section_id.as_deref(), Some("id1")); // direct
    }
}

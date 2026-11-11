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

// Dimensiunile implicite ale simbolurilor [px-lume], comune desenului, topologiei și exportului DXF.
pub const BOX_W: f64 = 52.0;
pub const BOX_H: f64 = 52.0;
pub const CIRCLE_R: f64 = 10.0;
pub const SUB_W: f64 = 180.0;
pub const SUB_H: f64 = 100.0;

/// Porturile unui subsistem: (x, y, e pe latura stângă). Fără porturi definite: unul pe fiecare latură.
pub fn subsystem_ports(el: &SchemaElement) -> Vec<(f64, f64, bool)> {
    let (w, h) = (el.w.unwrap_or(SUB_W), el.h.unwrap_or(SUB_H));
    let (bx, by) = (el.x - w / 2.0, el.y - h / 2.0);
    let sides: Vec<bool> = match &el.ports {
        Some(ports) => ports.iter().map(|p| p.get("side").and_then(serde_json::Value::as_str) == Some("L")).collect(),
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
                (bx, by + h / (nl + 1) as f64 * il as f64, true)
            } else {
                ir += 1;
                (bx + w, by + h / (nr + 1) as f64 * ir as f64, false)
            }
        })
        .collect()
}

/// Punctele de conexiune ale unui element care nu e tronson.
pub fn connection_points(el: &SchemaElement) -> Vec<(f64, f64)> {
    match el.kind {
        SchemaKind::Rezervor | SchemaKind::Sursa => vec![(el.x + el.w.unwrap_or(BOX_W) / 2.0, el.y)],
        SchemaKind::Hidrant | SchemaKind::CaminGrav | SchemaKind::CaminAer => {
            let r = el.r.unwrap_or(CIRCLE_R);
            vec![(el.x - r, el.y), (el.x + r, el.y), (el.x, el.y - r), (el.x, el.y + r)]
        }
        SchemaKind::Subsistem => subsystem_ports(el).into_iter().map(|(x, y, _)| (x, y)).collect(),
        SchemaKind::Text => vec![],
        SchemaKind::Tronson => vec![(el.x, el.y), tronson_end(el)],
    }
}

/// Tronsonul de calcul legat de un tronson din desen: după id-ul din desen, iar pentru
/// proiectele vechi (fără legătură salvată) după nume, în schema principală.
pub fn section_for<'a>(tr: &SchemaElement, sections: &'a [NetworkSection]) -> Option<&'a NetworkSection> {
    sections.iter().find(|s| s.schema_id.as_deref() == Some(tr.id.as_str())).or_else(|| {
        let label = tr.label.as_deref()?;
        sections.iter().find(|s| s.schema_id.is_none() && s.group.as_deref().unwrap_or("").is_empty() && s.name == label)
    })
}

/// Numele tronsoanelor de calcul legate de desen urmează etichetele din desen.
pub fn sync_section_names(schema: &[SchemaElement], sections: &mut [NetworkSection]) {
    if sections.iter().all(|s| s.schema_id.is_none()) {
        return;
    }
    for (el, _) in flatten_with_group(schema) {
        let (SchemaKind::Tronson, Some(label)) = (el.kind, &el.label) else { continue };
        if let Some(s) = sections.iter_mut().find(|s| s.schema_id.as_deref() == Some(el.id.as_str())) {
            if &s.name != label {
                s.name = label.clone();
            }
        }
    }
}

/// Elementul cu id-ul dat (caută și în subsisteme).
pub fn find_element_mut<'a>(schema: &'a mut [SchemaElement], id: &str) -> Option<&'a mut SchemaElement> {
    for el in schema.iter_mut() {
        if el.id == id {
            return Some(el);
        }
        if let Some(found) = el.children.as_mut().and_then(|ch| find_element_mut(ch, id)) {
            return Some(found);
        }
    }
    None
}

/// Prima etichetă liberă de forma „<prefix> <n>” printre elementele date.
pub fn next_free_label(prefix: &str, level: &[SchemaElement], kind: SchemaKind) -> String {
    let mut n = level.iter().filter(|e| e.kind == kind).count() + 1;
    loop {
        let label = format!("{prefix} {n}");
        if !level.iter().any(|e| e.label.as_deref() == Some(label.as_str())) {
            return label;
        }
        n += 1;
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

/// Tronsoanele deduse din schemă, fără să se piardă nimic din ce a introdus utilizatorul.
///
/// - un tronson din desen își regăsește tronsonul de calcul după id (sau, în proiectele
///   vechi, după nume) și îi păstrează toate datele: cote, debite, ζ, override-uri, culoare;
/// - din desen vin numele, grupul, lungimea (coordonate × scară sau lungimea suprascrisă),
///   părintele dedus din conexiuni și hidrantul de la capătul aval; un părinte ales manual
///   și o bifă de hidrant pusă manual rămân dacă desenul nu spune altceva;
/// - tronsoanele care nu (mai) sunt în desen rămân în listă, la sfârșit: le șterge utilizatorul.
///
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
    // Întâi legăturile după id, apoi (pentru restul) după nume.
    let mut prev_of: Vec<Option<&NetworkSection>> = tronsoane
        .iter()
        .map(|(tr, _)| existing.iter().find(|s| s.schema_id.as_deref() == Some(tr.id.as_str()) && matched.insert(s.id.clone())))
        .collect();
    for (i, (tr, _)) in tronsoane.iter().enumerate() {
        if prev_of[i].is_none() {
            let name = tr.label.as_deref().unwrap_or("Tr");
            prev_of[i] = existing.iter().find(|s| s.schema_id.is_none() && s.name == name && !matched.contains(&s.id));
            if let Some(p) = prev_of[i] {
                matched.insert(p.id.clone());
            }
        }
    }
    let mut sections: Vec<NetworkSection> = tronsoane
        .iter()
        .zip(&prev_of)
        .map(|((tr, group), prev)| {
            let (ex, ey) = tronson_end(tr);
            let len_m = (ex - tr.x).hypot(ey - tr.y) * mpp;
            let name = tr.label.clone().unwrap_or_else(|| "Tr".into());
            let mut s = match prev {
                Some(p) => NetworkSection { name, ..(*p).clone() },
                None => NetworkSection::new(new_id(), name),
            };
            s.schema_id = Some(tr.id.clone());
            s.group = (!group.is_empty()).then(|| group.clone());
            s.length = tr.override_length.unwrap_or((len_m * 10.0).round() / 10.0);
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
        // Desenul are prioritate; fără conexiune în desen rămâne părintele ales manual.
        if parent.is_some() {
            sections[i].parent_section_id = parent;
        }
        let end = tronson_end(tr);
        let drawn_hydrant = nodes
            .iter()
            .any(|n| n.kind == SchemaKind::Hidrant && connection_points(n).iter().any(|&p| dist(p, end) < CONNECT_TOLERANCE));
        sections[i].has_hydrant |= drawn_hydrant;
    }

    // 3) subsistemele: tronsonul din schema principală care ajunge la un port al subsistemului
    //    alimentează tronsoanele de capăt (fără părinte) din interiorul lui.
    for sub in schema.iter().filter(|e| e.kind == SchemaKind::Subsistem) {
        let ports = connection_points(sub);
        let feeder = tronsoane
            .iter()
            .position(|(tr, group)| group.is_empty() && ports.iter().any(|&p| dist(p, tronson_end(tr)) < CONNECT_TOLERANCE));
        let Some(feeder) = feeder else { continue };
        let group = sub.label.clone().unwrap_or_else(|| "Subsistem".into());
        for (i, (_, g)) in tronsoane.iter().enumerate() {
            if *g == group && sections[i].parent_section_id.is_none() {
                sections[i].parent_section_id = Some(ids[feeder].clone());
            }
        }
    }

    // Grupurile în ordinea apariției (schema principală prima).
    let mut order: Vec<String> = vec![String::new()];
    for (_, g) in &tagged {
        if !g.is_empty() && !order.contains(g) {
            order.push(g.clone());
        }
    }
    sections.sort_by_key(|s| order.iter().position(|g| g == s.group.as_deref().unwrap_or("")).unwrap_or(usize::MAX));
    // Tronsoanele care nu sunt în desen se păstrează (fără legătură cu desenul).
    sections.extend(existing.iter().filter(|s| !matched.contains(&s.id)).map(|s| NetworkSection { schema_id: None, ..s.clone() }));
    sections
}

/// Setează `overrideLength` pe tronsonul din desen legat de un tronson de calcul: după
/// id-ul din desen, altfel după etichetă (caută și în subsisteme).
pub fn set_tronson_override_length(schema: &mut [SchemaElement], schema_id: Option<&str>, label: &str, len: Option<f64>) -> bool {
    for el in schema.iter_mut() {
        let hit = match schema_id {
            Some(id) => el.id == id,
            None => el.label.as_deref() == Some(label),
        };
        if el.kind == SchemaKind::Tronson && hit {
            el.override_length = len;
            return true;
        }
        if let Some(ch) = el.children.as_mut() {
            if set_tronson_override_length(ch, schema_id, label, len) {
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

    /// Reimportul păstrează override-urile, tronsoanele manuale și legătura la redenumire.
    #[test]
    fn reimport_keeps_user_data_and_manual_sections() {
        let mut schema = vec![tr("a", "Tr 1", (0.0, 0.0), (1000.0, 0.0)), tr("b", "Tr 2", (1000.0, 0.0), (1500.0, 0.0))];
        let mut n = 0;
        let mut id = || {
            n += 1;
            format!("id{n}")
        };
        let mut secs = build_sections_from_schema(&schema, &[], 1000.0, &mut id);
        secs[1].q_c_override = Some(7.5);
        secs[1].has_hydrant = true; // bifă manuală, fără hidrant în desen
        secs[1].zeta = 3.0;
        let mut manual = NetworkSection::new("m".into(), "Manual".into());
        manual.parent_section_id = Some(secs[1].id.clone());
        secs.push(manual);
        schema[1].label = Some("Ramura B".into()); // redenumire în desen
        let again = build_sections_from_schema(&schema, &secs, 1000.0, &mut id);
        assert_eq!(again.len(), 3);
        let b = again.iter().find(|s| s.schema_id.as_deref() == Some("b")).unwrap();
        assert_eq!((b.name.as_str(), b.id.as_str()), ("Ramura B", "id2"));
        assert_eq!((b.q_c_override, b.has_hydrant, b.zeta), (Some(7.5), true, 3.0));
        assert_eq!(b.parent_section_id.as_deref(), Some("id1"));
        assert_eq!(again[2].name, "Manual");
        assert_eq!(section_for(&schema[1], &again).map(|s| s.id.as_str()), Some("id2"));
        // Tronson șters din desen: rămâne în listă, fără legătură.
        let after = build_sections_from_schema(&schema[..1], &again, 1000.0, &mut id);
        assert_eq!(after.len(), 3);
        assert!(after.iter().any(|s| s.name == "Ramura B" && s.schema_id.is_none()));
    }

    #[test]
    fn subsystem_roots_are_fed_by_the_main_section_and_labels_stay_unique() {
        let sub = SchemaElement {
            id: "sub".into(),
            kind: SchemaKind::Subsistem,
            x: 1090.0,
            y: 0.0,
            label: Some("Loc 1".into()),
            children: Some(vec![tr("c", "Tr 1", (0.0, 0.0), (300.0, 0.0)), tr("d", "Tr 2", (300.0, 0.0), (600.0, 0.0))]),
            ..Default::default()
        };
        // Portul stâng al subsistemului e la (1000, 0): acolo ajunge tronsonul principal.
        let schema = vec![tr("a", "Tr 1", (0.0, 0.0), (1000.0, 0.0)), sub];
        let mut n = 0;
        let s = build_sections_from_schema(&schema, &[], 1000.0, || {
            n += 1;
            format!("id{n}")
        });
        assert_eq!(s[1].parent_section_id.as_deref(), Some("id1"), "capătul subsistemului e alimentat din schema principală");
        assert_eq!(s[2].parent_section_id.as_deref(), Some("id2"));
        // După ștergerea lui „Tr 2” din trei, eticheta nouă nu dublează „Tr 3”.
        let level = vec![tr("x", "Tr 1", (0.0, 0.0), (1.0, 0.0)), tr("y", "Tr 3", (0.0, 0.0), (1.0, 0.0))];
        assert_eq!(next_free_label("Tr", &level, SchemaKind::Tronson), "Tr 4");
    }
}

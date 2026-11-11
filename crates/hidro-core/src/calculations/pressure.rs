use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SectionInput {
    pub id: String,
    pub name: String,
    pub parent_section_id: Option<String>,
    pub cota_down: Option<f64>,
    pub h_tot: f64,
}

#[derive(Debug, Clone)]
pub struct NodePressure {
    pub section_id: String,
    pub section_name: String,
    pub h_piez_down: f64,
    pub cota_down: Option<f64>,
    pub pressure_down: Option<f64>,
    pub pressure_down_bar: Option<f64>,
    pub warning: bool,
    pub has_cota: bool,
}

pub fn calculate_nodal_pressures(
    sections: Vec<SectionInput>,
    cota_radier: f64,
    hb_min: f64,
) -> Vec<NodePressure> {
    if sections.is_empty() {
        return vec![];
    }

    let section_ids: std::collections::HashSet<&str> =
        sections.iter().map(|s| s.id.as_str()).collect();

    let mut children_map: HashMap<Option<String>, Vec<usize>> = HashMap::new();
    for (i, s) in sections.iter().enumerate() {
        let pid = s
            .parent_section_id
            .as_deref()
            .filter(|p| section_ids.contains(p))
            .map(|p| p.to_string());
        children_map.entry(pid).or_default().push(i);
    }

    let mut results = Vec::new();

    fn walk(
        idx: usize,
        h_piez_up: f64,
        sections: &[SectionInput],
        children_map: &HashMap<Option<String>, Vec<usize>>,
        results: &mut Vec<NodePressure>,
        hb_min: f64,
    ) {
        let section = &sections[idx];
        let h_piez_down = h_piez_up - section.h_tot;
        let has_cota = section.cota_down.is_some();
        let pressure_down = section.cota_down.map(|c| h_piez_down - c);
        let pressure_down_bar = pressure_down.map(|p| p / 10.2);

        results.push(NodePressure {
            section_id: section.id.clone(),
            section_name: section.name.clone(),
            h_piez_down,
            cota_down: section.cota_down,
            pressure_down,
            pressure_down_bar,
            warning: pressure_down.map(|p| p < hb_min).unwrap_or(false),
            has_cota,
        });

        if let Some(children) = children_map.get(&Some(section.id.clone())) {
            for &ci in children {
                walk(ci, h_piez_down, sections, children_map, results, hb_min);
            }
        }
    }

    let roots: Vec<usize> = sections
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            s.parent_section_id
                .as_deref()
                .map(|p| !section_ids.contains(p))
                .unwrap_or(true)
        })
        .map(|(i, _)| i)
        .collect();

    for root_idx in roots {
        walk(root_idx, cota_radier, &sections, &children_map, &mut results, hb_min);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(id: &str, parent: Option<&str>, h_tot: f64, cota_down: f64) -> SectionInput {
        SectionInput {
            id: id.to_string(),
            name: id.to_string(),
            parent_section_id: parent.map(|p| p.to_string()),
            cota_down: Some(cota_down),
            h_tot,
        }
    }

    #[test]
    fn matches_cascading_head_loss() {
        let sections = vec![
            section("A", None, 10.0, 50.0),
            section("B", Some("A"), 5.0, 40.0),
        ];
        let results = calculate_nodal_pressures(sections, 100.0, 5.0);
        let a = results.iter().find(|r| r.section_id == "A").unwrap();
        let b = results.iter().find(|r| r.section_id == "B").unwrap();
        assert!((a.h_piez_down - 90.0).abs() < 1e-9);
        assert!((a.pressure_down.unwrap() - 40.0).abs() < 1e-9);
        assert!((b.h_piez_down - 85.0).abs() < 1e-9);
        assert!((b.pressure_down.unwrap() - 45.0).abs() < 1e-9);
    }
}

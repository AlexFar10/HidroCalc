use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const G: f64 = 9.81;

#[derive(Debug, Clone, Copy)]
pub struct PehdDiameter {
    pub d_ext: f64,
    pub d_int: f64,
}

impl PehdDiameter {
    /// Grosimea peretelui [mm].
    pub fn thickness(&self) -> f64 {
        (self.d_ext - self.d_int) / 2.0
    }
}

// NOTE: this table is duplicated in src/lib/pehd-diameters.ts (PEHD_DIAMETERS)
// — keep both in sync when the standard changes.
pub const PEHD_DIAMETERS: &[PehdDiameter] = &[
    PehdDiameter { d_ext: 20.0,  d_int: 15.2  },
    PehdDiameter { d_ext: 25.0,  d_int: 20.2  },
    PehdDiameter { d_ext: 32.0,  d_int: 27.2  },
    PehdDiameter { d_ext: 40.0,  d_int: 35.2  },
    PehdDiameter { d_ext: 50.0,  d_int: 45.2  },
    PehdDiameter { d_ext: 63.0,  d_int: 57.0  },
    PehdDiameter { d_ext: 75.0,  d_int: 67.8  },
    PehdDiameter { d_ext: 90.0,  d_int: 81.4  },
    PehdDiameter { d_ext: 110.0, d_int: 99.4  },
    PehdDiameter { d_ext: 125.0, d_int: 113.0 },
    PehdDiameter { d_ext: 140.0, d_int: 126.6 },
    PehdDiameter { d_ext: 160.0, d_int: 144.6 },
    PehdDiameter { d_ext: 180.0, d_int: 162.8 },
    PehdDiameter { d_ext: 200.0, d_int: 180.8 },
    PehdDiameter { d_ext: 225.0, d_int: 203.4 },
    PehdDiameter { d_ext: 250.0, d_int: 226.2 },
    PehdDiameter { d_ext: 280.0, d_int: 253.2 },
    PehdDiameter { d_ext: 315.0, d_int: 285.0 },
    PehdDiameter { d_ext: 355.0, d_int: 321.2 },
    PehdDiameter { d_ext: 400.0, d_int: 361.8 },
];

fn find_standard_diameter(d_calc_mm: f64) -> Option<&'static PehdDiameter> {
    PEHD_DIAMETERS.iter().find(|d| d.d_int >= d_calc_mm)
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NetworkSection {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub parent_section_id: Option<String>,
    pub length: f64,
    pub cota_up: Option<f64>,
    pub cota_down: Option<f64>,
    // Proveniența cotelor ('manual' | 'dem'); doar transportată, ca să nu se
    // piardă la round-trip-ul TS → Rust → TS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cota_up_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cota_down_source: Option<String>,
    pub q_extern: f64,
    pub q_linear: f64,
    pub zeta: f64,
    #[serde(default)]
    pub has_hydrant: bool,
    // Override-uri manuale din tabelul de rezultate (celule tip Excel). Când
    // sunt Some, se folosesc în locul valorilor calculate, iar tot ce depinde
    // de ele în aval se recalculează. None = revine la calcul.
    #[serde(default)]
    pub q_i_override: Option<f64>,
    #[serde(default)]
    pub q_f_override: Option<f64>,
    #[serde(default)]
    pub q_ie_override: Option<f64>,
    #[serde(default)]
    pub q_c_override: Option<f64>,
    #[serde(default)]
    pub d_calc_override: Option<f64>,
    #[serde(default)]
    pub d_std_int_override: Option<f64>,
    // Calculated - metoda debitului de drum: Qi (intrare) = Qf (ieșire) + q_own
    // (debitul propriu al tronsonului); Qc = (Qi+Qf)/2 e debitul de dimensionare.
    #[serde(default)]
    pub q_own: f64,
    #[serde(default)]
    pub q_ie: f64,
    #[serde(default)]
    pub q_f: f64,
    #[serde(default)]
    pub q_i: f64,
    pub q_c: f64,
    pub d_calc: f64,
    pub d_std_ext: f64,
    pub d_std_int: f64,
    pub v_real: f64,
    pub re: f64,
    /// 23·D/k — pragul folosit la clasificarea regimului de curgere (Re vs. acest prag).
    #[serde(default)]
    pub d_over_k: f64,
    pub regime: String,
    pub lambda: f64,
    pub h_lin: f64,
    pub h_loc: f64,
    pub h_tot: f64,
    pub slope: f64,
    /// Câmpuri necunoscute, păstrate la round-trip.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl NetworkSection {
    /// Tronson nou, gol (ca `addNetworkSection` din aplicația React).
    pub fn new(id: String, name: String) -> Self {
        Self { id, name, ..Self::default() }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConstants {
    pub roughness: f64,
    pub viscosity: f64,
    pub economic_velocity: f64,
}

fn detect_cycle(sections: &[NetworkSection]) -> bool {
    let section_ids: HashSet<&str> = sections.iter().map(|s| s.id.as_str()).collect();
    let section_map: HashMap<&str, &NetworkSection> =
        sections.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut visited: HashSet<String> = HashSet::new();
    let mut in_stack: HashSet<String> = HashSet::new();

    fn dfs(
        id: &str,
        section_ids: &HashSet<&str>,
        section_map: &HashMap<&str, &NetworkSection>,
        visited: &mut HashSet<String>,
        in_stack: &mut HashSet<String>,
    ) -> bool {
        if in_stack.contains(id) {
            return true;
        }
        if visited.contains(id) {
            return false;
        }
        visited.insert(id.to_string());
        in_stack.insert(id.to_string());
        if let Some(section) = section_map.get(id) {
            if let Some(parent) = &section.parent_section_id {
                if section_ids.contains(parent.as_str())
                    && dfs(parent, section_ids, section_map, visited, in_stack)
                {
                    return true;
                }
            }
        }
        in_stack.remove(id);
        false
    }

    for s in sections {
        if !visited.contains(&s.id)
            && dfs(&s.id, &section_ids, &section_map, &mut visited, &mut in_stack)
        {
            return true;
        }
    }
    false
}

// Metoda debitului de drum (Qi/Qf/Qc): fiecare tronson are un debit propriu
// distribuit pe lungimea sa (q_own = q_lin·L) plus, opțional, debitul unui
// hidrant conectat la capătul aval (q_ie). Debitul care iese din tronson (Qf)
// alimentează nodul aval (Q_extern + Q_ie) și tot ce se ramifică mai jos
// (Σ Qi al copiilor); debitul care intră (Qi) mai adaugă propriul debit al
// tronsonului. Qc = (Qi+Qf)/2 e media folosită la predimensionare.
fn calculate_route_flows(sections: Vec<NetworkSection>, hydrant_flow_ls: f64) -> Vec<NetworkSection> {
    if sections.is_empty() {
        return sections;
    }

    let section_ids: HashSet<&str> = sections.iter().map(|s| s.id.as_str()).collect();

    let mut children_map: HashMap<Option<String>, Vec<usize>> = HashMap::new();
    for (i, s) in sections.iter().enumerate() {
        let pid = s
            .parent_section_id
            .as_deref()
            .filter(|p| section_ids.contains(p))
            .map(|p| p.to_string());
        children_map.entry(pid).or_default().push(i);
    }

    let q_own_map: Vec<f64> = sections.iter().map(|s| s.q_linear * s.length).collect();
    let q_ie_map: Vec<f64> = sections
        .iter()
        .map(|s| if s.has_hydrant { hydrant_flow_ls } else { 0.0 })
        .collect();
    let mut q_f_map = vec![0.0f64; sections.len()];
    let mut q_i_map = vec![0.0f64; sections.len()];
    let mut visited = vec![false; sections.len()];

    fn calc_section(
        idx: usize,
        sections: &[NetworkSection],
        children_map: &HashMap<Option<String>, Vec<usize>>,
        q_own_map: &[f64],
        q_ie_map: &[f64],
        q_f_map: &mut Vec<f64>,
        q_i_map: &mut Vec<f64>,
        visited: &mut Vec<bool>,
    ) {
        if visited[idx] {
            return;
        }
        visited[idx] = true;

        let section = &sections[idx];
        let child_indices = children_map
            .get(&Some(section.id.clone()))
            .cloned()
            .unwrap_or_default();

        for &ci in &child_indices {
            calc_section(ci, sections, children_map, q_own_map, q_ie_map, q_f_map, q_i_map, visited);
        }

        let sum_child_qi: f64 = child_indices.iter().map(|&ci| q_i_map[ci]).sum();
        let q_f = section.q_extern + q_ie_map[idx] + sum_child_qi;
        q_f_map[idx] = q_f;
        q_i_map[idx] = q_f + q_own_map[idx];
    }

    let root_indices: Vec<usize> = sections
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

    for root_idx in root_indices {
        calc_section(
            root_idx,
            &sections,
            &children_map,
            &q_own_map,
            &q_ie_map,
            &mut q_f_map,
            &mut q_i_map,
            &mut visited,
        );
    }

    sections
        .into_iter()
        .enumerate()
        .map(|(i, mut s)| {
            s.q_own = q_own_map[i];
            // Override-urile manuale au prioritate; Qc urmează Qi/Qf (fie
            // calculate, fie forțate) dacă nu e el însuși forțat.
            s.q_ie = s.q_ie_override.unwrap_or(q_ie_map[i]);
            s.q_f = s.q_f_override.unwrap_or(q_f_map[i]);
            s.q_i = s.q_i_override.unwrap_or(q_i_map[i]);
            s.q_c = s.q_c_override.unwrap_or((s.q_i + s.q_f) / 2.0);
            s
        })
        .collect()
}

fn calculate_section_hydraulic(mut section: NetworkSection, constants: &NetworkConstants) -> NetworkSection {
    let q_m3s = section.q_c / 1000.0;
    if q_m3s <= 0.0 || section.length <= 0.0 {
        section.d_calc = 0.0;
        section.d_std_ext = 0.0;
        section.d_std_int = 0.0;
        section.v_real = 0.0;
        section.re = 0.0;
        section.d_over_k = 0.0;
        section.regime = "none".to_string();
        section.lambda = 0.0;
        section.h_lin = 0.0;
        section.h_loc = 0.0;
        section.h_tot = 0.0;
        section.slope = 0.0;
        return section;
    }

    // Dp: fie forțat manual, fie predimensionat din Qc și viteza economică.
    let d_calc_mm = match section.d_calc_override {
        Some(v) => v,
        None => {
            let d_calc_m =
                (4.0 * q_m3s / (std::f64::consts::PI * constants.economic_velocity)).sqrt();
            d_calc_m * 1000.0
        }
    };
    section.d_calc = d_calc_mm;

    // Di STAS: fie forțat manual (De STAS urmează rândul PEHD cel mai apropiat),
    // fie ales prin snapping la primul diametru standard ≥ Dp.
    let (d_std_ext, d_std_int) = match section.d_std_int_override {
        Some(di) if di > 0.0 => {
            let ext = PEHD_DIAMETERS
                .iter()
                .min_by(|a, b| {
                    (a.d_int - di)
                        .abs()
                        .partial_cmp(&(b.d_int - di).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|d| d.d_ext)
                .unwrap_or(di);
            (ext, di)
        }
        _ => match find_standard_diameter(d_calc_mm) {
            None => {
                section.d_std_ext = 0.0;
                section.d_std_int = 0.0;
                section.v_real = 0.0;
                section.re = 0.0;
                section.d_over_k = 0.0;
                section.regime = "oversized".to_string();
                section.lambda = 0.0;
                section.h_lin = 0.0;
                section.h_loc = 0.0;
                section.h_tot = 0.0;
                section.slope = 0.0;
                return section;
            }
            Some(d) => (d.d_ext, d.d_int),
        },
    };

    let d_std_m = d_std_int / 1000.0;
    section.d_std_ext = d_std_ext;
    section.d_std_int = d_std_int;

    let v_real = (4.0 * q_m3s) / (std::f64::consts::PI * d_std_m * d_std_m);
    let re = (v_real * d_std_m) / constants.viscosity;
    let k = constants.roughness;
    let d_over_k = d_std_m / k;

    let (lambda, regime) = if re < 4000.0 {
        (64.0 / re, "laminar".to_string())
    } else if re < 23.0 * d_over_k {
        (0.3164 / re.powf(0.25), "turbulent_smooth".to_string())
    } else if re < 560.0 * d_over_k {
        (0.11 * (k / d_std_m + 68.0 / re).powf(0.25), "turbulent_transition".to_string())
    } else {
        let log_arg = 3.7 * d_std_m / k;
        (1.0 / (2.0 * log_arg.log10()).powi(2), "turbulent_quadratic".to_string())
    };

    let h_lin = lambda * (section.length / d_std_m) * (v_real * v_real) / (2.0 * G);
    let h_loc = section.zeta * (v_real * v_real) / (2.0 * G);
    let h_tot = h_lin + h_loc;
    let slope = if section.length > 0.0 { h_lin / section.length } else { 0.0 };

    section.v_real = v_real;
    section.re = re;
    section.d_over_k = 23.0 * d_over_k;
    section.regime = regime;
    section.lambda = lambda;
    section.h_lin = h_lin;
    section.h_loc = h_loc;
    section.h_tot = h_tot;
    section.slope = slope;
    section
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "code", content = "message")]
pub enum HydraulicError {
    #[serde(rename = "ERR_CYCLE_DETECTED")]
    CycleDetected(String),
}

pub fn calculate_all_sections(
    sections: Vec<NetworkSection>,
    constants: NetworkConstants,
    hydrant_flow_ls: f64,
) -> Result<Vec<NetworkSection>, HydraulicError> {
    if detect_cycle(&sections) {
        return Err(HydraulicError::CycleDetected(
            "Topologie circulară detectată. Verificați conexiunile dintre tronsoane.".to_string(),
        ));
    }
    let with_flows = calculate_route_flows(sections, hydrant_flow_ls);
    Ok(with_flows
        .into_iter()
        .map(|s| calculate_section_hydraulic(s, &constants))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(
        id: &str,
        parent: Option<&str>,
        length: f64,
        q_extern: f64,
        q_linear: f64,
        has_hydrant: bool,
    ) -> NetworkSection {
        NetworkSection {
            id: id.to_string(),
            name: id.to_string(),
            color: None,
            group: None,
            parent_section_id: parent.map(|p| p.to_string()),
            length,
            cota_up: None,
            cota_down: None,
            cota_up_source: None,
            cota_down_source: None,
            q_extern,
            q_linear,
            zeta: 0.0,
            has_hydrant,
            q_i_override: None,
            q_f_override: None,
            q_ie_override: None,
            q_c_override: None,
            d_calc_override: None,
            d_std_int_override: None,
            q_own: 0.0,
            q_ie: 0.0,
            q_f: 0.0,
            q_i: 0.0,
            q_c: 0.0,
            d_calc: 0.0,
            d_std_ext: 0.0,
            d_std_int: 0.0,
            v_real: 0.0,
            re: 0.0,
            d_over_k: 0.0,
            regime: String::new(),
            lambda: 0.0,
            h_lin: 0.0,
            h_loc: 0.0,
            h_tot: 0.0,
            slope: 0.0,
            extra: Default::default(),
        }
    }

    // A (root) → B (child), B are debit propriu + un hidrant; A doar debit
    // propriu, fără consum extern.
    //   B: q_own = 0.01·100 = 1.0 l/s, q_ie = 5.0 l/s (hidrant) → Qf_B = 5.0,
    //      Qi_B = 6.0, Qc_B = 5.5
    //   A: q_own = 0.02·50 = 1.0 l/s → Qf_A = Qi_B = 6.0, Qi_A = 7.0, Qc_A = 6.5
    #[test]
    fn route_flow_cascades_from_leaf_to_root() {
        let sections = vec![
            section("A", None, 50.0, 0.0, 0.02, false),
            section("B", Some("A"), 100.0, 0.0, 0.01, true),
        ];
        let result = calculate_route_flows(sections, 5.0);
        let a = result.iter().find(|s| s.id == "A").unwrap();
        let b = result.iter().find(|s| s.id == "B").unwrap();

        assert!((b.q_own - 1.0).abs() < 1e-9);
        assert!((b.q_ie - 5.0).abs() < 1e-9);
        assert!((b.q_f - 5.0).abs() < 1e-9);
        assert!((b.q_i - 6.0).abs() < 1e-9);
        assert!((b.q_c - 5.5).abs() < 1e-9);

        assert!((a.q_own - 1.0).abs() < 1e-9);
        assert!((a.q_ie - 0.0).abs() < 1e-9);
        assert!((a.q_f - 6.0).abs() < 1e-9);
        assert!((a.q_i - 7.0).abs() < 1e-9);
        assert!((a.q_c - 6.5).abs() < 1e-9);
    }

    #[test]
    fn leaf_without_hydrant_or_extern_has_symmetric_qc() {
        let sections = vec![section("A", None, 100.0, 0.0, 0.005, false)];
        let result = calculate_route_flows(sections, 5.0);
        let a = &result[0];
        // Qf = 0, Qi = q_own = 0.5, Qc = 0.25
        assert!((a.q_f - 0.0).abs() < 1e-9);
        assert!((a.q_i - 0.5).abs() < 1e-9);
        assert!((a.q_c - 0.25).abs() < 1e-9);
    }

    fn constants() -> NetworkConstants {
        NetworkConstants {
            roughness: 0.0001,
            viscosity: 1.31e-6,
            economic_velocity: 1.0,
        }
    }

    // Override manual pe Qi/Qf: Qc urmează media lor forțată, iar restul
    // (Dp, Di STAS, Vef…) se recalculează din acest Qc.
    #[test]
    fn qi_qf_overrides_drive_qc_and_downstream() {
        let mut s = section("A", None, 100.0, 0.0, 0.0, false);
        s.q_i_override = Some(12.0);
        s.q_f_override = Some(8.0);
        let result = calculate_all_sections(vec![s], constants(), 5.0).unwrap();
        let a = &result[0];
        assert!((a.q_i - 12.0).abs() < 1e-9);
        assert!((a.q_f - 8.0).abs() < 1e-9);
        assert!((a.q_c - 10.0).abs() < 1e-9);
        assert!(a.d_calc > 0.0, "Dp trebuie recalculat din Qc forțat");
        assert!(a.d_std_int > 0.0 && a.v_real > 0.0 && a.re > 0.0);
    }

    // Override direct pe Qc are prioritate față de Qi/Qf.
    #[test]
    fn qc_override_wins_over_qi_qf() {
        let mut s = section("A", None, 100.0, 0.0, 0.02, false);
        s.q_c_override = Some(5.0);
        let result = calculate_all_sections(vec![s], constants(), 5.0).unwrap();
        assert!((result[0].q_c - 5.0).abs() < 1e-9);
    }

    // Di STAS forțat manual: se folosește ca atare, De STAS ia rândul PEHD cel
    // mai apropiat, iar Vef/Re se calculează cu diametrul forțat.
    #[test]
    fn d_std_int_override_forces_diameter_and_recomputes() {
        let mut s = section("A", None, 100.0, 0.0, 0.0, false);
        s.q_c_override = Some(10.0);
        s.d_std_int_override = Some(99.4); // rândul PEHD 110/99.4
        let result = calculate_all_sections(vec![s], constants(), 5.0).unwrap();
        let a = &result[0];
        assert!((a.d_std_int - 99.4).abs() < 1e-9);
        assert!((a.d_std_ext - 110.0).abs() < 1e-9);
        let q_m3s = 10.0 / 1000.0;
        let expected_v = 4.0 * q_m3s / (std::f64::consts::PI * 0.0994 * 0.0994);
        assert!((a.v_real - expected_v).abs() < 1e-6);
    }

    // Dp forțat manual, fără override pe Di STAS: snapping-ul folosește Dp-ul dat.
    #[test]
    fn d_calc_override_feeds_snapping() {
        let mut s = section("A", None, 100.0, 0.0, 0.0, false);
        s.q_c_override = Some(10.0);
        s.d_calc_override = Some(100.0);
        let result = calculate_all_sections(vec![s], constants(), 5.0).unwrap();
        let a = &result[0];
        assert!((a.d_calc - 100.0).abs() < 1e-9);
        // primul standard cu d_int ≥ 100 este 125/113.0
        assert!((a.d_std_int - 113.0).abs() < 1e-9);
    }
}

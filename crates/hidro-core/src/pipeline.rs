//! Lanțul complet de calcul al pașilor 3–7, rulat direct pe `ProjectData`.
//!
//! Ordinea: necesar (Q_n) → cerință (Q_s) → hidranți (V_RI, Q_RI) → rezervor → debite de dimensionare.

use crate::calculations::{design_flows, hydrants, network, pressure, reservoir, water_demand, water_need};
use crate::model::ProjectData;

#[derive(Debug, Clone)]
pub struct ProjectResults {
    pub water_need: water_need::WaterNeedResults,
    pub water_demand: water_demand::WaterDemandResults,
    pub hydrants: hydrants::HydrantResults,
    pub reservoir: reservoir::ReservoirResults,
    pub design_flows: design_flows::DesignFlowResults,
}

/// Populația de perspectivă N = N₀ × (1 + p/100)^t, rotunjită.
pub fn perspective_inhabitants(p: &ProjectData) -> f64 {
    let c = &p.consumers;
    water_need::calculate_perspective_inhabitants(c.inhabitants, c.growth_rate, Some(c.perspective_years))
}

pub fn compute(p: &ProjectData) -> ProjectResults {
    let c = &p.consumers;
    let wn = water_need::calculate_water_need(
        water_need::ConsumersData {
            inhabitants: c.inhabitants,
            perspective_inhabitants: c.perspective_inhabitants,
            q_gosp_per_capita: c.q_gosp_per_capita,
            public_consumers: c
                .public_consumers
                .iter()
                .map(|pc| water_need::PublicConsumer {
                    quantity: pc.quantity,
                    num_consumers: Some(pc.num_consumers),
                    q_specific: pc.q_specific,
                })
                .collect(),
        },
        water_need::WaterNeedConstants {
            kor_gosp: p.water_need_constants.kor_gosp,
            kor_public_med: p.water_need_constants.kor_public_med,
            kzi_gosp: p.water_need_constants.kzi_gosp,
            kzi_public: p.water_need_constants.kzi_public,
        },
    );

    let k = &p.water_demand_coefficients;
    let wd = water_demand::calculate_water_demand(
        water_demand::WaterNeedInput { q_n_zi_med: wn.q_n_zi_med, q_n_zi_max: wn.q_n_zi_max, q_n_or_max: wn.q_n_or_max },
        water_demand::WaterDemandCoefficients { kp: k.kp, kss: k.kss, ksr: k.ksr },
    );

    let h = &p.hydrants;
    let hy = hydrants::calculate_hydrants(
        hydrants::HydrantData {
            debit_hydrant: h.debit_hydrant,
            operating_time: h.operating_time,
            simultaneous_fires: h.simultaneous_fires,
            coeff_a: h.coeff_a,
            timp_refacere: h.timp_refacere,
            debit_hydrant_int: Some(h.debit_hydrant_int),
            operating_time_int: Some(h.operating_time_int),
            simultaneous_fires_int: Some(h.simultaneous_fires_int),
        },
        wd.q_s_or_max,
    );

    let r = &p.reservoir;
    let rz = reservoir::calculate_reservoir(
        reservoir::ReservoirData {
            cota_teren: r.cota_teren,
            hb: r.hb,
            i_med: r.i_med,
            l: r.l,
            c: r.c,
            coef_debit_min: r.coef_debit_min,
            t_av: r.t_av,
            ti: r.ti,
            qa: r.qa,
        },
        reservoir::DemandForReservoir { q_s_zi_max: wd.q_s_zi_max },
        reservoir::HydrantForReservoir { v_ri: hy.v_ri },
    );

    let df = design_flows::calculate_design_flows(
        design_flows::DemandInput { q_s_zi_max: wd.q_s_zi_max, q_s_or_max: wd.q_s_or_max, ks_safe: wd.ks_safe },
        design_flows::HydrantResultsInput { q_ri: hy.q_ri },
        design_flows::HydrantDataInput {
            simultaneous_fires_int: Some(h.simultaneous_fires_int),
            debit_hydrant_int: Some(h.debit_hydrant_int),
            simultaneous_fires: h.simultaneous_fires,
            debit_hydrant: h.debit_hydrant,
        },
        design_flows::CoeffInput { kp: k.kp, ksr: k.ksr },
    );

    ProjectResults { water_need: wn, water_demand: wd, hydrants: hy, reservoir: rz, design_flows: df }
}

/// Constantele hidraulice invalide (trebuie > 0), ca `validateNetworkConstants`.
// `!(x > 0)` respinge intenționat și NaN (o comparație `x <= 0` l-ar lăsa să treacă).
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn invalid_network_constants(p: &ProjectData) -> Vec<&'static str> {
    let c = &p.network_constants;
    let mut bad = Vec::new();
    if !(c.viscosity > 0.0) {
        bad.push("viscosity");
    }
    if !(c.roughness > 0.0) {
        bad.push("roughness");
    }
    if !(c.economic_velocity > 0.0) {
        bad.push("economicVelocity");
    }
    bad
}

#[derive(Debug, Clone, PartialEq)]
pub enum NetworkError {
    InvalidConstants(Vec<&'static str>),
    Cycle,
}

/// Calculul hidraulic al tuturor tronsoanelor (debite, diametre, pierderi).
/// Rezultatul e idempotent: rularea pe propria ieșire dă același rezultat.
pub fn compute_network(p: &ProjectData) -> Result<Vec<network::NetworkSection>, NetworkError> {
    let bad = invalid_network_constants(p);
    if !bad.is_empty() {
        return Err(NetworkError::InvalidConstants(bad));
    }
    let c = &p.network_constants;
    network::calculate_all_sections(
        p.network_sections.clone(),
        network::NetworkConstants { roughness: c.roughness, viscosity: c.viscosity, economic_velocity: c.economic_velocity },
        p.hydrants.debit_hydrant,
    )
    .map_err(|_| NetworkError::Cycle)
}

/// Presiunile nodale la capetele aval, pornind de la cota radierului rezervorului.
pub fn nodal_pressures(p: &ProjectData, cota_radier: f64) -> Vec<pressure::NodePressure> {
    pressure::calculate_nodal_pressures(
        p.network_sections
            .iter()
            .map(|s| pressure::SectionInput {
                id: s.id.clone(),
                name: s.name.clone(),
                parent_section_id: s.parent_section_id.clone(),
                cota_down: s.cota_down,
                h_tot: s.h_tot,
            })
            .collect(),
        cota_radier,
        p.reservoir.hb,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_project_runs_and_is_consistent() {
        let mut p = ProjectData::default();
        p.consumers.inhabitants = 1000.0;
        p.consumers.perspective_inhabitants = perspective_inhabitants(&p);
        assert_eq!(p.consumers.perspective_inhabitants, 1220.0); // 1000 × 1.01^20
        let r = compute(&p);
        // Q_n_zi_med_gosp = 1220 × 120 / 1000
        assert!((r.water_need.q_n_zi_med_gosp - 146.4).abs() < 1e-9);
        // ks = 1.02 + 1.03 − 1 = 1.05
        assert!((r.water_demand.ks - 1.05).abs() < 1e-12);
        assert!(r.design_flows.q_dim_network >= r.design_flows.q_iiv.min(r.design_flows.q_iic));
        assert!(r.reservoir.v_total >= r.reservoir.v_fire_reserve);
    }

    #[test]
    fn network_is_idempotent_and_detects_cycles() {
        use crate::calculations::network::NetworkSection;
        let mut p = ProjectData::default();
        let mut a = NetworkSection::new("a".into(), "Tr1".into());
        a.length = 200.0;
        a.q_linear = 0.01;
        let mut b = NetworkSection::new("b".into(), "Tr2".into());
        b.length = 150.0;
        b.q_extern = 1.5;
        b.has_hydrant = true;
        b.parent_section_id = Some("a".into());
        p.network_sections = vec![a, b];
        let first = compute_network(&p).unwrap();
        assert!(first[0].q_c > first[1].q_c);
        assert!(first[1].d_std_int > 0.0);
        p.network_sections = first.clone();
        assert_eq!(compute_network(&p).unwrap(), first);

        p.network_sections[0].parent_section_id = Some("b".into());
        assert_eq!(compute_network(&p), Err(NetworkError::Cycle));
        p.network_constants.viscosity = 0.0;
        assert!(matches!(compute_network(&p), Err(NetworkError::InvalidConstants(_))));
    }
}

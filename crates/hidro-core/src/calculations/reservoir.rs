use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReservoirData {
    pub cota_teren: f64,
    pub hb: f64,
    pub i_med: f64,
    pub l: f64,
    pub c: f64,
    pub coef_debit_min: f64,
    pub t_av: f64,
    pub ti: f64,
    pub qa: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DemandForReservoir {
    pub q_s_zi_max: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HydrantForReservoir {
    #[serde(rename = "vRI")]
    pub v_ri: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReservoirResults {
    pub cota_radier: f64,
    pub hr: f64,
    pub v_compensation: f64,
    pub v_fire_reserve: f64,
    pub v_emergency: f64,
    pub q_min: f64,
    pub v_total: f64,
}

pub fn calculate_reservoir(
    data: ReservoirData,
    demand: DemandForReservoir,
    hydrant_results: HydrantForReservoir,
) -> ReservoirResults {
    let hr = data.i_med * data.l;
    let cota_radier = data.cota_teren + data.hb + hr;
    let v_compensation = data.c * demand.q_s_zi_max;
    let v_fire_reserve = hydrant_results.v_ri;
    let coef_safe = f64::max(0.0, f64::min(1.0, data.coef_debit_min));
    let q_min = (coef_safe * demand.q_s_zi_max) / 24.0;
    let v_emergency = q_min * (data.t_av - data.ti) - data.qa * data.t_av;
    let v_total = v_compensation + v_fire_reserve + f64::max(0.0, v_emergency);

    ReservoirResults {
        cota_radier,
        hr,
        v_compensation,
        v_fire_reserve,
        v_emergency,
        q_min,
        v_total,
    }
}

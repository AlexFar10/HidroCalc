use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaterNeedInput {
    pub q_n_zi_med: f64,
    pub q_n_zi_max: f64,
    pub q_n_or_max: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaterDemandCoefficients {
    pub kp: f64,
    pub kss: f64,
    pub ksr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaterDemandResults {
    pub ks: f64,
    pub ks_safe: f64,
    pub ks_warning: bool,
    pub q_s_zi_med: f64,
    pub q_s_zi_max: f64,
    pub q_s_or_max: f64,
    pub q_s_or_max_ls: f64,
}

pub fn calculate_water_demand(
    need: WaterNeedInput,
    coeff: WaterDemandCoefficients,
) -> WaterDemandResults {
    let ks = (coeff.kss + coeff.ksr) - 1.0;
    let ks_warning = ks <= 0.0;
    let ks_safe = f64::max(ks, 0.01);

    let q_s_zi_med = need.q_n_zi_med * coeff.kp * ks_safe;
    let q_s_zi_max = need.q_n_zi_max * coeff.kp * ks_safe;
    let q_s_or_max = need.q_n_or_max * coeff.kp * ks_safe;
    let q_s_or_max_ls = (q_s_or_max * 1000.0) / 3600.0;

    WaterDemandResults {
        ks,
        ks_safe,
        ks_warning,
        q_s_zi_med,
        q_s_zi_max,
        q_s_or_max,
        q_s_or_max_ls,
    }
}

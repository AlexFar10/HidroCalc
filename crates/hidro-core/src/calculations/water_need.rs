use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicConsumer {
    pub quantity: f64,
    pub num_consumers: Option<f64>,
    pub q_specific: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsumersData {
    pub inhabitants: f64,
    pub perspective_inhabitants: f64,
    pub q_gosp_per_capita: f64,
    pub public_consumers: Vec<PublicConsumer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaterNeedConstants {
    pub kor_gosp: f64,
    pub kor_public_med: f64,
    pub kzi_gosp: f64,
    pub kzi_public: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaterNeedResults {
    pub q_n_zi_med_gosp: f64,
    pub q_n_zi_med_pub: f64,
    pub q_n_zi_med: f64,
    pub q_n_zi_max_gosp: f64,
    pub q_n_zi_max_pub: f64,
    pub q_n_zi_max: f64,
    pub q_n_or_max_gosp: f64,
    pub q_n_or_max_pub: f64,
    pub q_n_or_max: f64,
}

pub fn calculate_perspective_inhabitants(
    inhabitants: f64,
    growth_rate: f64,
    years: Option<f64>,
) -> f64 {
    let y = years.unwrap_or(20.0);
    (inhabitants * (1.0 + growth_rate / 100.0).powf(y)).round()
}

pub fn calculate_water_need(
    consumers: ConsumersData,
    constants: WaterNeedConstants,
) -> WaterNeedResults {
    let n = if consumers.perspective_inhabitants > 0.0 {
        consumers.perspective_inhabitants
    } else {
        consumers.inhabitants
    };
    let qg = consumers.q_gosp_per_capita;

    let q_n_zi_med_gosp = (n * qg) / 1000.0;
    let q_n_zi_med_pub: f64 = consumers
        .public_consumers
        .iter()
        .map(|c| (c.quantity * c.num_consumers.unwrap_or(0.0) * c.q_specific) / 1000.0)
        .sum();
    let q_n_zi_med = q_n_zi_med_gosp + q_n_zi_med_pub;
    let q_n_zi_max_gosp = q_n_zi_med_gosp * constants.kzi_gosp;
    let q_n_zi_max_pub = q_n_zi_med_pub * constants.kzi_public;
    let q_n_zi_max = q_n_zi_max_gosp + q_n_zi_max_pub;
    let q_n_or_max_gosp = (q_n_zi_max_gosp * constants.kor_gosp) / 24.0;
    let q_n_or_max_pub = (q_n_zi_max_pub * constants.kor_public_med) / 24.0;
    let q_n_or_max = q_n_or_max_gosp + q_n_or_max_pub;

    WaterNeedResults {
        q_n_zi_med_gosp,
        q_n_zi_med_pub,
        q_n_zi_med,
        q_n_zi_max_gosp,
        q_n_zi_max_pub,
        q_n_zi_max,
        q_n_or_max_gosp,
        q_n_or_max_pub,
        q_n_or_max,
    }
}

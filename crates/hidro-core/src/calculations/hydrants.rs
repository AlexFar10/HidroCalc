use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HydrantData {
    pub debit_hydrant: f64,
    pub operating_time: f64,
    pub simultaneous_fires: f64,
    pub coeff_a: f64,
    pub timp_refacere: f64,
    pub debit_hydrant_int: Option<f64>,
    pub operating_time_int: Option<f64>,
    pub simultaneous_fires_int: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydrantResults {
    #[serde(rename = "vRI")]
    pub v_ri: f64,
    #[serde(rename = "tRI")]
    pub t_ri: f64,
    #[serde(rename = "qRI")]
    pub q_ri: f64,
}

pub fn calculate_hydrants(data: HydrantData, q_s_or_max: f64) -> HydrantResults {
    let int_debit = data.debit_hydrant_int.unwrap_or(0.0);
    let int_time = data.operating_time_int.unwrap_or(0.0);
    let int_fires = data.simultaneous_fires_int.unwrap_or(0.0);

    let v_ri = data.coeff_a * q_s_or_max * data.operating_time
        + 3.6 * data.simultaneous_fires * data.debit_hydrant * data.operating_time
        + 3.6 * int_time * int_fires * int_debit;

    let t_ri = if data.timp_refacere > 0.0 { data.timp_refacere } else { 48.0 };
    let q_ri = if t_ri > 0.0 { (v_ri / t_ri) * 24.0 } else { 0.0 };

    HydrantResults { v_ri, t_ri, q_ri }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DemandInput {
    pub q_s_zi_max: f64,
    pub q_s_or_max: f64,
    pub ks_safe: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HydrantResultsInput {
    #[serde(rename = "qRI")]
    pub q_ri: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HydrantDataInput {
    pub simultaneous_fires_int: Option<f64>,
    pub debit_hydrant_int: Option<f64>,
    pub simultaneous_fires: f64,
    pub debit_hydrant: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoeffInput {
    pub kp: f64,
    pub ksr: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesignFlowResults {
    #[serde(rename = "qIC")]
    pub q_ic: f64,
    #[serde(rename = "qICPrime")]
    pub q_ic_prime: f64,
    #[serde(rename = "qIIC")]
    pub q_iic: f64,
    #[serde(rename = "qIIV")]
    pub q_iiv: f64,
    #[serde(rename = "qIIVLs")]
    pub q_iiv_ls: f64,
    #[serde(rename = "qDimNetwork")]
    pub q_dim_network: f64,
}

pub fn calculate_design_flows(
    demand: DemandInput,
    hydrant_results: HydrantResultsInput,
    hydrant_data: HydrantDataInput,
    coeff: CoeffInput,
) -> DesignFlowResults {
    let q_ic = demand.q_s_zi_max + coeff.kp * demand.ks_safe * hydrant_results.q_ri;
    let q_ic_prime = if demand.ks_safe != 0.0 {
        q_ic * coeff.ksr / demand.ks_safe
    } else {
        q_ic
    };
    let q_iic = demand.q_s_or_max
        + 3.6
            * hydrant_data.simultaneous_fires_int.unwrap_or(0.0)
            * hydrant_data.debit_hydrant_int.unwrap_or(0.0);
    let q_iiv = demand.q_s_or_max
        + 3.6 * hydrant_data.simultaneous_fires * hydrant_data.debit_hydrant;
    let q_iiv_ls = (q_iiv * 1000.0) / 3600.0;
    let q_dim_network = f64::max(q_iic, q_iiv);

    DesignFlowResults {
        q_ic,
        q_ic_prime,
        q_iic,
        q_iiv,
        q_iiv_ls,
        q_dim_network,
    }
}

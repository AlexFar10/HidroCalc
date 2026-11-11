#[derive(Debug, Clone)]
pub struct WaterNeedInput {
    pub q_n_zi_med: f64,
    pub q_n_zi_max: f64,
    pub q_n_or_max: f64,
}

#[derive(Debug, Clone)]
pub struct WaterDemandCoefficients {
    pub kp: f64,
    pub kss: f64,
    pub ksr: f64,
}

#[derive(Debug, Clone)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demand_applies_kp_and_ks_and_guards_non_positive_ks() {
        let need = WaterNeedInput { q_n_zi_med: 100.0, q_n_zi_max: 130.0, q_n_or_max: 36.0 };
        let r = calculate_water_demand(need.clone(), WaterDemandCoefficients { kp: 1.1, kss: 1.02, ksr: 1.03 });
        assert!((r.ks - 1.05).abs() < 1e-12 && !r.ks_warning);
        assert!((r.q_s_zi_max - 130.0 * 1.1 * 1.05).abs() < 1e-9);
        assert!((r.q_s_or_max_ls - r.q_s_or_max / 3.6).abs() < 1e-9);
        let bad = calculate_water_demand(need, WaterDemandCoefficients { kp: 1.1, kss: 0.4, ksr: 0.5 });
        assert!(bad.ks_warning && bad.ks_safe == 0.01);
    }
}

#[derive(Debug, Clone)]
pub struct DemandInput {
    pub q_s_zi_max: f64,
    pub q_s_or_max: f64,
    pub ks_safe: f64,
}

#[derive(Debug, Clone)]
pub struct HydrantResultsInput {
    pub q_ri: f64,
}

#[derive(Debug, Clone)]
pub struct HydrantDataInput {
    pub simultaneous_fires_int: f64,
    pub debit_hydrant_int: f64,
    pub simultaneous_fires: f64,
    pub debit_hydrant: f64,
}

#[derive(Debug, Clone)]
pub struct CoeffInput {
    pub kp: f64,
    pub ksr: f64,
}

#[derive(Debug, Clone)]
pub struct DesignFlowResults {
    pub q_ic: f64,
    pub q_ic_prime: f64,
    pub q_iic: f64,
    pub q_iiv: f64,
    pub q_iiv_ls: f64,
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
            * hydrant_data.simultaneous_fires_int
            * hydrant_data.debit_hydrant_int;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_is_sized_for_the_larger_fire_scenario() {
        let r = calculate_design_flows(
            DemandInput { q_s_zi_max: 240.0, q_s_or_max: 20.0, ks_safe: 1.05 },
            HydrantResultsInput { q_ri: 40.0 },
            HydrantDataInput { simultaneous_fires_int: 1.0, debit_hydrant_int: 2.5, simultaneous_fires: 1.0, debit_hydrant: 5.0 },
            CoeffInput { kp: 1.1, ksr: 1.03 },
        );
        assert!((r.q_ic - (240.0 + 1.1 * 1.05 * 40.0)).abs() < 1e-9);
        assert!((r.q_iic - 29.0).abs() < 1e-9 && (r.q_iiv - 38.0).abs() < 1e-9);
        assert_eq!(r.q_dim_network, r.q_iiv);
    }
}

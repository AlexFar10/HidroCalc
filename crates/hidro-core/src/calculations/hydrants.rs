#[derive(Debug, Clone)]
pub struct HydrantData {
    pub debit_hydrant: f64,
    pub operating_time: f64,
    pub simultaneous_fires: f64,
    pub coeff_a: f64,
    pub timp_refacere: f64,
    pub debit_hydrant_int: f64,
    pub operating_time_int: f64,
    pub simultaneous_fires_int: f64,
}

#[derive(Debug, Clone)]
pub struct HydrantResults {
    pub v_ri: f64,
    pub t_ri: f64,
    pub q_ri: f64,
}

pub fn calculate_hydrants(data: HydrantData, q_s_or_max: f64) -> HydrantResults {
    let v_ri = data.coeff_a * q_s_or_max * data.operating_time
        + 3.6 * data.simultaneous_fires * data.debit_hydrant * data.operating_time
        + 3.6 * data.operating_time_int * data.simultaneous_fires_int * data.debit_hydrant_int;

    let t_ri = if data.timp_refacere > 0.0 { data.timp_refacere } else { 48.0 };
    let q_ri = if t_ri > 0.0 { (v_ri / t_ri) * 24.0 } else { 0.0 };

    HydrantResults { v_ri, t_ri, q_ri }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_reserve_and_refill_flow() {
        let data = HydrantData {
            debit_hydrant: 5.0,
            operating_time: 3.0,
            simultaneous_fires: 1.0,
            coeff_a: 0.7,
            timp_refacere: 24.0,
            debit_hydrant_int: 2.5,
            operating_time_int: 0.5,
            simultaneous_fires_int: 1.0,
        };
        let r = calculate_hydrants(data.clone(), 10.0);
        // 0.7·10·3 + 3.6·1·5·3 + 3.6·0.5·1·2.5 = 21 + 54 + 4.5
        assert!((r.v_ri - 79.5).abs() < 1e-9);
        assert!((r.q_ri - 79.5).abs() < 1e-9); // refacere în 24 h
        let r = calculate_hydrants(HydrantData { timp_refacere: 0.0, ..data }, 10.0);
        assert_eq!(r.t_ri, 48.0, "timp de refacere lipsă → 48 h");
    }
}

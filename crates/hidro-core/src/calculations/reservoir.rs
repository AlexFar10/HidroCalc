#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct DemandForReservoir {
    pub q_s_zi_max: f64,
}

#[derive(Debug, Clone)]
pub struct HydrantForReservoir {
    pub v_ri: f64,
}

#[derive(Debug, Clone)]
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
    // `max`/`min` (nu `clamp`): un coeficient NaN devine 0, nu se propagă.
    #[allow(clippy::manual_clamp)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volumes_and_invert_level() {
        let data = ReservoirData { cota_teren: 100.0, hb: 14.0, i_med: 0.003, l: 4000.0, c: 0.15, coef_debit_min: 0.3, t_av: 12.0, ti: 2.0, qa: 0.0 };
        let r = calculate_reservoir(data.clone(), DemandForReservoir { q_s_zi_max: 240.0 }, HydrantForReservoir { v_ri: 80.0 });
        assert!((r.hr - 12.0).abs() < 1e-9 && (r.cota_radier - 126.0).abs() < 1e-9);
        assert!((r.v_compensation - 36.0).abs() < 1e-9);
        assert!((r.q_min - 3.0).abs() < 1e-9 && (r.v_emergency - 30.0).abs() < 1e-9);
        assert!((r.v_total - 146.0).abs() < 1e-9);
        // Volum de avarie negativ: nu scade volumul total.
        let r = calculate_reservoir(ReservoirData { qa: 10.0, ..data }, DemandForReservoir { q_s_zi_max: 240.0 }, HydrantForReservoir { v_ri: 80.0 });
        assert!(r.v_emergency < 0.0 && (r.v_total - 116.0).abs() < 1e-9);
    }
}

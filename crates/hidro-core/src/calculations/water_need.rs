#[derive(Debug, Clone)]
pub struct PublicConsumer {
    pub quantity: f64,
    pub num_consumers: f64,
    pub q_specific: f64,
}

#[derive(Debug, Clone)]
pub struct ConsumersData {
    pub inhabitants: f64,
    pub perspective_inhabitants: f64,
    pub q_gosp_per_capita: f64,
    pub public_consumers: Vec<PublicConsumer>,
}

#[derive(Debug, Clone)]
pub struct WaterNeedConstants {
    pub kor_gosp: f64,
    pub kor_public_med: f64,
    pub kzi_gosp: f64,
    pub kzi_public: f64,
}

#[derive(Debug, Clone)]
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

pub fn calculate_perspective_inhabitants(inhabitants: f64, growth_rate: f64, years: f64) -> f64 {
    (inhabitants * (1.0 + growth_rate / 100.0).powf(years)).round()
}

pub fn calculate_water_need(consumers: ConsumersData, constants: WaterNeedConstants) -> WaterNeedResults {
    let n = if consumers.perspective_inhabitants > 0.0 { consumers.perspective_inhabitants } else { consumers.inhabitants };
    let qg = consumers.q_gosp_per_capita;

    let q_n_zi_med_gosp = (n * qg) / 1000.0;
    let q_n_zi_med_pub: f64 = consumers.public_consumers.iter().map(|c| (c.quantity * c.num_consumers * c.q_specific) / 1000.0).sum();
    let q_n_zi_med = q_n_zi_med_gosp + q_n_zi_med_pub;
    let q_n_zi_max_gosp = q_n_zi_med_gosp * constants.kzi_gosp;
    let q_n_zi_max_pub = q_n_zi_med_pub * constants.kzi_public;
    let q_n_zi_max = q_n_zi_max_gosp + q_n_zi_max_pub;
    let q_n_or_max_gosp = (q_n_zi_max_gosp * constants.kor_gosp) / 24.0;
    let q_n_or_max_pub = (q_n_zi_max_pub * constants.kor_public_med) / 24.0;
    let q_n_or_max = q_n_or_max_gosp + q_n_or_max_pub;

    WaterNeedResults { q_n_zi_med_gosp, q_n_zi_med_pub, q_n_zi_med, q_n_zi_max_gosp, q_n_zi_max_pub, q_n_zi_max, q_n_or_max_gosp, q_n_or_max_pub, q_n_or_max }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn need_sums_domestic_and_public_with_their_coefficients() {
        assert_eq!(calculate_perspective_inhabitants(1000.0, 1.0, 20.0), 1220.0);
        let r = calculate_water_need(
            ConsumersData {
                inhabitants: 1000.0,
                perspective_inhabitants: 0.0, // 0 → se folosește populația actuală
                q_gosp_per_capita: 100.0,
                public_consumers: vec![PublicConsumer { quantity: 2.0, num_consumers: 50.0, q_specific: 30.0 }],
            },
            WaterNeedConstants { kor_gosp: 2.4, kor_public_med: 2.0, kzi_gosp: 1.3, kzi_public: 1.2 },
        );
        assert!((r.q_n_zi_med_gosp - 100.0).abs() < 1e-9);
        assert!((r.q_n_zi_med_pub - 3.0).abs() < 1e-9);
        assert!((r.q_n_zi_max - (130.0 + 3.6)).abs() < 1e-9);
        assert!((r.q_n_or_max - (130.0 * 2.4 / 24.0 + 3.6 * 2.0 / 24.0)).abs() < 1e-9);
    }
}

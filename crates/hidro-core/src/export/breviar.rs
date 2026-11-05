//! Breviarul de calcul PDF (portat din `exportBreviarPDF.ts`): fiecare mărime e
//! prezentată ca formulă → înlocuirea valorilor → rezultat. Formulele reproduc
//! implementarea din `calculations/` — orice modificare acolo se oglindește aici.

use super::pdf::{Align, Face, Writer, BLACK, PAGE_W};
use super::Tr;
use crate::calculations::network::NetworkSection;
use crate::model::ProjectData;
use crate::pipeline::ProjectResults;

const G: f64 = 9.81;

fn n(v: f64, d: usize) -> String {
    if v.is_finite() { format!("{v:.d$}") } else { "-".into() }
}

fn lambda_formula(s: &NetworkSection, k: f64) -> (String, String) {
    let d = s.d_std_int / 1000.0;
    match s.regime.as_str() {
        "laminar" => ("λ = 64 / Re".into(), format!("64 / {}", n(s.re, 0))),
        "turbulent_smooth" => ("λ = 0.3164 / Re^0.25  (Blasius)".into(), format!("0.3164 / {}^0.25", n(s.re, 0))),
        "turbulent_transition" => ("λ = 0.11 · (k/D + 68/Re)^0.25  (Altșul)".into(), format!("0.11 · ({k}/{} + 68/{})^0.25", n(d, 4), n(s.re, 0))),
        _ => ("λ = 1 / (2 · lg(3.7 · D/k))²  (Nikuradse)".into(), format!("1 / (2 · lg(3.7 · {}/{k}))²", n(d, 4))),
    }
}

pub fn build(p: &ProjectData, r: &ProjectResults, t: Tr) -> Result<Vec<u8>, String> {
    let tt = |k: &str| t(k, &[]);
    let regime = |rg: &str| {
        let k = format!("network.regime.{rg}");
        let s = tt(&k);
        if s == k { rg.to_string() } else { s }
    };
    let (c, k, co, h, rz, nc) = (&p.consumers, &p.water_need_constants, &p.water_demand_coefficients, &p.hydrants, &p.reservoir, &p.network_constants);
    let (need, dem, hr, fl, rr) = (&r.water_need, &r.water_demand, &r.hydrants, &r.design_flows, &r.reservoir);
    let secs = &p.network_sections;
    let g = &p.general;
    let mut w = Writer::new();
    let cx = PAGE_W / 2.0;

    // Copertă
    w.text_at(cx, 200.0, 22.0, Face::Bold, BLACK, &tt("export.breviar.title"), Align::Center);
    w.text_at(cx, 234.0, 13.0, Face::Regular, BLACK, &tt("export.breviar.subtitle"), Align::Center);
    w.text_at(cx, 285.0, 12.0, Face::Bold, BLACK, if g.name.is_empty() { "-" } else { &g.name }, Align::Center);
    for (i, l) in [
        format!("{} {}, {} {}", tt("export.pdf.localitate"), g.locality, tt("export.pdf.jud"), g.county),
        format!("{} {}", tt("export.pdf.beneficiar"), g.beneficiary),
        format!("{} {}", tt("export.pdf.proiectant"), g.designer),
        format!("{} {}", tt("export.pdf.data"), g.date),
        tt("export.breviar.norms"),
    ]
    .iter()
    .enumerate()
    {
        w.text_at(cx, 340.0 + i as f32 * 20.0, 10.0, Face::Regular, BLACK, l, Align::Center);
    }

    // 1. Necesarul
    w.new_page();
    w.title(&tt("export.breviar.s1"));
    let nn = if c.perspective_inhabitants > 0.0 { c.perspective_inhabitants } else { c.inhabitants };
    if c.perspective_inhabitants > 0.0 {
        w.calc(&tt("export.breviar.perspective"), "N = N₀ · (1 + p/100)^n", Some(&format!("{} · (1 + {}/100)^{}", c.inhabitants, c.growth_rate, c.perspective_years)), &format!("{} loc.", c.perspective_inhabitants));
    }
    w.calc(&tt("export.breviar.qZiMedGosp"), "Qn zi med gosp = N · qg / 1000", Some(&format!("{nn} · {} / 1000", c.q_gosp_per_capita)), &format!("{} m³/zi", n(need.q_n_zi_med_gosp, 3)));
    if !c.public_consumers.is_empty() {
        w.para(&tt("export.breviar.publicIntro"), 9.0, Face::Regular);
        let rows: Vec<Vec<String>> = c
            .public_consumers
            .iter()
            .map(|pc| {
                vec![
                    if pc.kind == "Altele" { pc.custom_type.clone() } else { pc.kind.clone() },
                    pc.quantity.to_string(),
                    pc.num_consumers.to_string(),
                    pc.q_specific.to_string(),
                    n(pc.quantity * pc.num_consumers * pc.q_specific / 1000.0, 3),
                ]
            })
            .collect();
        w.table(&[tt("export.pdf.s12ColInstitution"), "n [u]".into(), "Np".into(), "q [l/u·zi]".into(), "Q [m³/zi]".into()], &rows, 8.0);
    }
    w.calc(&tt("export.breviar.qZiMedPub"), "Qn zi med pub = Σ(nᵢ · Npᵢ · qᵢ) / 1000", None, &format!("{} m³/zi", n(need.q_n_zi_med_pub, 3)));
    w.calc(&tt("export.breviar.qZiMed"), "Qn zi med = Qn zi med gosp + Qn zi med pub", Some(&format!("{} + {}", n(need.q_n_zi_med_gosp, 3), n(need.q_n_zi_med_pub, 3))), &format!("{} m³/zi", n(need.q_n_zi_med, 3)));
    w.calc(
        &tt("export.breviar.qZiMax"),
        "Qn zi max = Qn zi med gosp · Kzi gosp + Qn zi med pub · Kzi pub",
        Some(&format!("{} · {} + {} · {}", n(need.q_n_zi_med_gosp, 3), k.kzi_gosp, n(need.q_n_zi_med_pub, 3), k.kzi_public)),
        &format!("{} + {} = {} m³/zi", n(need.q_n_zi_max_gosp, 3), n(need.q_n_zi_max_pub, 3), n(need.q_n_zi_max, 3)),
    );
    w.calc(
        &tt("export.breviar.qOrMax"),
        "Qn or max = (Qn zi max gosp · Kor gosp + Qn zi max pub · Kor pub) / 24",
        Some(&format!("({} · {} + {} · {}) / 24", n(need.q_n_zi_max_gosp, 3), k.kor_gosp, n(need.q_n_zi_max_pub, 3), k.kor_public_med)),
        &format!("{} m³/h", n(need.q_n_or_max, 3)),
    );

    // 2. Cerința
    w.title(&tt("export.breviar.s2"));
    w.calc(&tt("export.breviar.ks"), "Ks = Kss + Ksr − 1", Some(&format!("{} + {} − 1", co.kss, co.ksr)), &n(dem.ks, 4));
    if dem.ks_warning {
        w.para(&t("export.breviar.ksWarning", &[("val", &n(dem.ks_safe, 2))]), 8.5, Face::Regular);
    }
    let ks = n(dem.ks_safe, 4);
    w.calc(&tt("export.breviar.qsZiMed"), "Qs zi med = Kp · Ks · Qn zi med", Some(&format!("{} · {ks} · {}", co.kp, n(need.q_n_zi_med, 3))), &format!("{} m³/zi", n(dem.q_s_zi_med, 3)));
    w.calc(&tt("export.breviar.qsZiMax"), "Qs zi max = Kp · Ks · Qn zi max", Some(&format!("{} · {ks} · {}", co.kp, n(need.q_n_zi_max, 3))), &format!("{} m³/zi", n(dem.q_s_zi_max, 3)));
    w.calc(
        &tt("export.breviar.qsOrMax"),
        "Qs or max = Kp · Ks · Qn or max",
        Some(&format!("{} · {ks} · {}", co.kp, n(need.q_n_or_max, 3))),
        &format!("{} m³/h = {} l/s", n(dem.q_s_or_max, 3), n(dem.q_s_or_max_ls, 3)),
    );

    // 3. Incendiu
    w.title(&tt("export.breviar.s3"));
    w.calc(
        &tt("export.breviar.vri"),
        "V_RI = a · Qs or max · T_ie + 3.6 · n_ie · Q_ie · T_ie + 3.6 · n_ii · Q_ii · T_ii",
        Some(&format!(
            "{} · {} · {} + 3.6 · {} · {} · {} + 3.6 · {} · {} · {}",
            h.coeff_a,
            n(dem.q_s_or_max, 3),
            h.operating_time,
            h.simultaneous_fires,
            h.debit_hydrant,
            h.operating_time,
            h.simultaneous_fires_int,
            h.debit_hydrant_int,
            h.operating_time_int
        )),
        &format!("{} m³", n(hr.v_ri, 3)),
    );
    w.calc(&tt("export.breviar.qri"), "Q_RI = V_RI / T_RI · 24", Some(&format!("{} / {} · 24", n(hr.v_ri, 3), hr.t_ri)), &format!("{} m³/zi", n(hr.q_ri, 4)));

    // 4. Debite de dimensionare
    w.title(&tt("export.breviar.s4"));
    w.calc(&tt("export.breviar.qic"), "Q_IC = Qs zi max + Kp · Ks · Q_RI", Some(&format!("{} + {} · {ks} · {}", n(dem.q_s_zi_max, 3), co.kp, n(hr.q_ri, 4))), &format!("{} m³/zi", n(fl.q_ic, 3)));
    w.calc(&tt("export.breviar.qicPrime"), "Q'_IC = Q_IC · Ksr / Ks", Some(&format!("{} · {} / {ks}", n(fl.q_ic, 3), co.ksr)), &format!("{} m³/zi", n(fl.q_ic_prime, 3)));
    w.calc(&tt("export.breviar.qiic"), "Q_IIC = Qs or max + 3.6 · n_ii · Q_ii", Some(&format!("{} + 3.6 · {} · {}", n(dem.q_s_or_max, 3), h.simultaneous_fires_int, h.debit_hydrant_int)), &format!("{} m³/h", n(fl.q_iic, 3)));
    w.calc(
        &tt("export.breviar.qiiv"),
        "Q_IIV = Qs or max + 3.6 · n_ie · Q_ie",
        Some(&format!("{} + 3.6 · {} · {}", n(dem.q_s_or_max, 3), h.simultaneous_fires, h.debit_hydrant)),
        &format!("{} m³/h = {} l/s", n(fl.q_iiv, 3), n(fl.q_iiv_ls, 3)),
    );

    // 5. Rezervor
    w.title(&tt("export.breviar.s5"));
    w.calc(&tt("export.breviar.hr"), "h_r = I_med · l", Some(&format!("{} · {}", rz.i_med, rz.l)), &format!("{} m", n(rr.hr, 3)));
    w.calc(&tt("export.breviar.cotaRadier"), "C_radier = C_T + H_B + h_r", Some(&format!("{} + {} + {}", rz.cota_teren, rz.hb, n(rr.hr, 3))), &format!("{} m", n(rr.cota_radier, 2)));
    w.calc(&tt("export.breviar.vcomp"), "V_comp = c · Qs zi max", Some(&format!("{} · {}", rz.c, n(dem.q_s_zi_max, 3))), &format!("{} m³", n(rr.v_compensation, 3)));
    w.calc(&tt("export.breviar.vinc"), "V_inc = V_RI", None, &format!("{} m³", n(rr.v_fire_reserve, 3)));
    w.calc(&tt("export.breviar.qmin"), "Q_min = coef · Qs zi max / 24", Some(&format!("{} · {} / 24", rz.coef_debit_min, n(dem.q_s_zi_max, 3))), &format!("{} m³/h", n(rr.q_min, 3)));
    let neg = if rr.v_emergency < 0.0 { format!(" → 0 ({})", tt("export.breviar.negativeIgnored")) } else { String::new() };
    w.calc(
        &tt("export.breviar.vav"),
        "V_av = Q_min · (T_av − T_i) − Q_a · T_av",
        Some(&format!("{} · ({} − {}) − {} · {}", n(rr.q_min, 3), rz.t_av, rz.ti, rz.qa, rz.t_av)),
        &format!("{} m³{neg}", n(rr.v_emergency, 3)),
    );
    w.calc(
        &tt("export.breviar.vtot"),
        "V_rez = V_comp + V_inc + max(0, V_av)",
        Some(&format!("{} + {} + {}", n(rr.v_compensation, 3), n(rr.v_fire_reserve, 3), n(rr.v_emergency.max(0.0), 3))),
        &format!("{} m³", n(rr.v_total, 3)),
    );

    // 6. Rețeaua
    if !secs.is_empty() {
        w.new_page();
        w.title(&tt("export.breviar.s6"));
        w.para(&t("export.breviar.networkIntro", &[("k", &nc.roughness.to_string()), ("nu", &nc.viscosity.to_string()), ("vec", &nc.economic_velocity.to_string())]), 9.0, Face::Regular);
        for l in [
            "q_own = q_lin · L",
            "Qf = Q_extern + Q_ie + Σ(Qi copii)        Qi = Qf + q_own        Qc = (Qi + Qf) / 2",
            "Dp = √(4 · Qc / (π · v_ec))  →  Di STAS ≥ Dp (PEHD PE100 PN10)",
            "v = 4 · Qc / (π · Di²)        Re = v · Di / ν",
            "h_lin = λ · (L / Di) · v² / (2g)        h_loc = ζ · v² / (2g)        i = h_lin / L",
        ] {
            w.para(l, 8.5, Face::Mono);
        }
        w.gap(4.0);
        let rows: Vec<Vec<String>> = secs
            .iter()
            .map(|s| {
                vec![
                    s.name.clone(),
                    n(s.length, 1),
                    n(s.q_i, 3),
                    n(s.q_f, 3),
                    n(s.q_c, 3),
                    n(s.d_calc, 1),
                    n(s.d_std_ext, 0),
                    n(s.d_std_int, 1),
                    n(s.v_real, 3),
                    n(s.re, 0),
                    n(s.lambda, 5),
                    n(s.h_tot, 4),
                ]
            })
            .collect();
        w.table(
            &[tt("export.pdf.s7ColTronson"), "L [m]".into(), "Qi".into(), "Qf".into(), "Qc [l/s]".into(), "Dp [mm]".into(), "De".into(), "Di".into(), "v [m/s]".into(), "Re".into(), "λ".into(), "h_tot [m]".into()],
            &rows,
            6.5,
        );
        for s in secs {
            w.ensure(160.0);
            w.para(&format!("{} {}  (L = {} m)", tt("export.pdf.s7ColTronson"), s.name, n(s.length, 2)), 10.0, Face::Bold);
            if s.regime == "none" || s.regime == "oversized" {
                w.para(&regime(&s.regime), 8.5, Face::Regular);
                continue;
            }
            let d = s.d_std_int / 1000.0;
            let q = s.q_c / 1000.0;
            w.calc(
                &tt("export.breviar.flows"),
                "Qf = Q_extern + Q_ie + Σ(Qi aval);  Qi = Qf + q_lin · L;  Qc = (Qi + Qf)/2",
                Some(&format!(
                    "{} + {} + {};  {} + {} · {};  ({} + {})/2",
                    n(s.q_extern, 3),
                    n(s.q_ie, 3),
                    n(s.q_f - s.q_extern - s.q_ie, 3),
                    n(s.q_f, 3),
                    s.q_linear,
                    n(s.length, 2),
                    n(s.q_i, 3),
                    n(s.q_f, 3)
                )),
                &format!("Qf = {} l/s, Qi = {} l/s, Qc = {} l/s", n(s.q_f, 3), n(s.q_i, 3), n(s.q_c, 3)),
            );
            w.calc(
                &tt("export.breviar.dp"),
                "Dp = √(4 · Qc / (π · v_ec))",
                Some(&format!("√(4 · {} / (π · {}))", n(q, 6), nc.economic_velocity)),
                &format!("{} mm  →  De {} / Di {} mm", n(s.d_calc, 1), n(s.d_std_ext, 0), n(s.d_std_int, 1)),
            );
            w.calc(
                &tt("export.breviar.velocity"),
                "v = 4 · Qc / (π · Di²);  Re = v · Di / ν",
                Some(&format!("4 · {} / (π · {}²);  {} · {} / {}", n(q, 6), n(d, 4), n(s.v_real, 3), n(d, 4), nc.viscosity)),
                &format!("v = {} m/s, Re = {} → {}", n(s.v_real, 3), n(s.re, 0), regime(&s.regime)),
            );
            let (lf, ls) = lambda_formula(s, nc.roughness);
            w.calc(&tt("export.breviar.lambda"), &lf, Some(&ls), &n(s.lambda, 5));
            w.calc(
                &tt("export.breviar.headloss"),
                "h_lin = λ · (L/Di) · v²/(2g);  h_loc = ζ · v²/(2g)",
                Some(&format!("{} · ({}/{}) · {}²/(2·{G});  {} · {}²/(2·{G})", n(s.lambda, 5), n(s.length, 2), n(d, 4), n(s.v_real, 3), s.zeta, n(s.v_real, 3))),
                &format!("h_lin = {} m, h_loc = {} m, h_tot = {} m, i = {}", n(s.h_lin, 4), n(s.h_loc, 4), n(s.h_tot, 4), n(s.slope, 6)),
            );
        }
    }
    w.footer(1, &format!("{} — {}", g.name, tt("export.breviar.title")));
    w.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline;

    #[test]
    fn report_and_breviar_are_valid_pdfs() {
        let mut p = ProjectData::default();
        p.general.name = "Alimentare cu apă Ștefănești".into();
        p.consumers.inhabitants = 900.0;
        p.consumers.perspective_inhabitants = pipeline::perspective_inhabitants(&p);
        let mut a = NetworkSection::new("a".into(), "Tr 1".into());
        a.length = 250.0;
        a.q_linear = 0.01;
        a.zeta = 2.0;
        let mut b = NetworkSection::new("b".into(), "Tr 2".into());
        b.length = 140.0;
        b.q_extern = 1.0;
        b.parent_section_id = Some("a".into());
        p.network_sections = vec![a, b];
        p.network_sections = pipeline::compute_network(&p).unwrap();
        p.network_schema = vec![crate::model::SchemaElement {
            kind: crate::model::SchemaKind::Tronson,
            x: 0.0,
            y: 0.0,
            x2: Some(900.0),
            y2: Some(0.0),
            label: Some("Tr 1".into()),
            ..Default::default()
        }];
        let r = pipeline::compute(&p);
        let t = |k: &str, _: &[(&str, &str)]| k.to_string();
        let breviar = build(&p, &r, &t).unwrap();
        let report = super::super::report::build(&p, &r, &t, Some("Hartă OSM".into()), 1000.0).unwrap();
        for pdf in [&breviar, &report] {
            assert!(pdf.starts_with(b"%PDF-"));
            assert!(pdf.len() > 5_000);
        }
        if let Ok(dir) = std::env::var("HIDRO_PDF_OUT") {
            std::fs::write(format!("{dir}/breviar.pdf"), &breviar).unwrap();
            std::fs::write(format!("{dir}/raport.pdf"), &report).unwrap();
        }
    }
}

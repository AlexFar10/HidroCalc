//! Raportul PDF „Memoriu de calcul” (portat din `exportPDF.ts`).

use super::pdf::{Align, Face, Writer, BLACK, CONTENT_W, GREY, MARGIN, PAGE_W};
use super::{schema_pdf, Tr};
use crate::model::ProjectData;
use crate::pipeline::ProjectResults;

fn f(v: f64, d: usize) -> String {
    if v.is_finite() { format!("{v:.d$}") } else { "-".into() }
}

/// `map_line` = linia cu localitatea/coordonatele hărții (sau `None`).
pub fn build(p: &ProjectData, r: &ProjectResults, t: Tr, map_line: Option<String>, scale_ratio: f64) -> Result<Vec<u8>, String> {
    let tt = |k: &str| t(k, &[]);
    let (g, need, dem, hr, fl, rr) = (&p.general, &r.water_need, &r.water_demand, &r.hydrants, &r.design_flows, &r.reservoir);
    let secs = &p.network_sections;
    let mut w = Writer::new();
    let cx = PAGE_W / 2.0;

    // Copertă
    w.text_at(cx, 170.0, 20.0, Face::Bold, BLACK, &tt("export.pdf.title"), Align::Center);
    w.text_at(cx, 212.0, 14.0, Face::Bold, BLACK, &tt("export.pdf.subtitle"), Align::Center);
    w.text_at(cx, 270.0, 12.0, Face::Regular, BLACK, if g.name.is_empty() { "Proiect" } else { &g.name }, Align::Center);
    let info = [
        format!("{} {}, {} {}", tt("export.pdf.localitate"), g.locality, tt("export.pdf.jud"), g.county),
        format!("{} {}", tt("export.pdf.beneficiar"), g.beneficiary),
        format!("{} {}", tt("export.pdf.proiectant"), g.designer),
        format!("{} {}", tt("export.pdf.data"), g.date),
        format!("{} {}", tt("export.pdf.sursaDeApa"), g.water_source),
    ];
    for (i, l) in info.iter().enumerate() {
        w.text_at(cx, 326.0 + i as f32 * 20.0, 10.0, Face::Regular, BLACK, l, Align::Center);
    }

    // 1. Date de intrare
    w.new_page();
    w.title(&tt("export.pdf.s1Title"));
    w.subtitle(&tt("export.pdf.s11Title"));
    let c = &p.consumers;
    w.para(&format!("{} {}", tt("export.pdf.s11Inhabitants"), c.inhabitants), 9.0, Face::Regular);
    w.para(&t("export.pdf.s11GrowthRate", &[("rate", &c.growth_rate.to_string())]), 9.0, Face::Regular);
    w.para(&format!("{} {}", tt("export.pdf.s11Perspective"), c.perspective_inhabitants), 9.0, Face::Regular);
    if !c.public_consumers.is_empty() {
        w.subtitle(&tt("export.pdf.s12Title"));
        let head = ["s12ColInstitution", "s12ColQty", "s12ColUnitType", "s12ColQSpecific", "s12ColHours", "s12ColKor"].map(|k| tt(&format!("export.pdf.{k}")));
        let rows: Vec<Vec<String>> = c
            .public_consumers
            .iter()
            .map(|pc| {
                vec![
                    if pc.kind == "Altele" { pc.custom_type.clone() } else { pc.kind.clone() },
                    pc.quantity.to_string(),
                    pc.unit_type.clone(),
                    pc.q_specific.to_string(),
                    pc.hours_per_day.to_string(),
                    pc.kor.to_string(),
                ]
            })
            .collect();
        w.table(&head, &rows, 8.0);
    }

    // 2. Necesarul de apă
    w.title(&tt("export.pdf.s2Title"));
    let row = |k: &str, v: f64, u: &str| vec![tt(&format!("export.pdf.{k}")), f(v, 3), u.to_string()];
    w.table(
        &[tt("export.pdf.s2Param"), tt("export.pdf.s2Value"), tt("export.pdf.s2Unit")],
        &[
            row("s2QziMedGosp", need.q_n_zi_med_gosp, "m³/zi"),
            row("s2QziMedPub", need.q_n_zi_med_pub, "m³/zi"),
            row("s2QziMed", need.q_n_zi_med, "m³/zi"),
            row("s2QziMaxGosp", need.q_n_zi_max_gosp, "m³/zi"),
            row("s2QziMaxPub", need.q_n_zi_max_pub, "m³/zi"),
            row("s2QziMax", need.q_n_zi_max, "m³/zi"),
            row("s2QorMaxGosp", need.q_n_or_max_gosp, "m³/h"),
            row("s2QorMaxPub", need.q_n_or_max_pub, "m³/h"),
            row("s2QorMax", need.q_n_or_max, "m³/h"),
        ],
        8.0,
    );

    // 3. Cerința de apă
    w.title(&tt("export.pdf.s3Title"));
    let k = &p.water_demand_coefficients;
    w.para(&format!("Kp = {}, Kss = {}, Ksr = {}, Ks = {}", k.kp, k.kss, k.ksr, f(dem.ks, 4)), 9.0, Face::Regular);
    let r3 = |key: &str, a: f64, b: f64, u: &str| vec![tt(&format!("export.pdf.{key}")), f(a, 3), f(b, 3), u.to_string()];
    w.table(
        &[tt("export.pdf.s2Param"), tt("export.pdf.s3ColNeed"), tt("export.pdf.s3ColDemand"), tt("export.pdf.s2Unit")],
        &[
            r3("s3QziMed", need.q_n_zi_med, dem.q_s_zi_med, "m³/zi"),
            r3("s3QziMax", need.q_n_zi_max, dem.q_s_zi_max, "m³/zi"),
            r3("s3QorMax", need.q_n_or_max, dem.q_s_or_max, "m³/h"),
            r3("s3QorMax", need.q_n_or_max * 1000.0 / 3600.0, dem.q_s_or_max_ls, "l/s"),
        ],
        8.0,
    );

    // 4–6
    let h = &p.hydrants;
    w.title(&tt("export.pdf.s4Title"));
    w.para(&t("export.pdf.s4Qe", &[("qe", &h.debit_hydrant.to_string()), ("timp", &h.operating_time.to_string()), ("fires", &h.simultaneous_fires.to_string())]), 9.0, Face::Regular);
    w.para(&t("export.pdf.s4VRI", &[("vRI", &f(hr.v_ri, 3)), ("tRI", &hr.t_ri.to_string()), ("qRI", &f(hr.q_ri, 4))]), 9.0, Face::Regular);
    w.title(&tt("export.pdf.s5Title"));
    w.para(&t("export.pdf.s5QIC", &[("val", &f(fl.q_ic, 3))]), 9.0, Face::Regular);
    w.para(&t("export.pdf.s5QICR", &[("val", &f(fl.q_ic_prime, 3))]), 9.0, Face::Regular);
    w.para(&t("export.pdf.s5QIIC", &[("val", &f(fl.q_iic, 3))]), 9.0, Face::Regular);
    w.para(&t("export.pdf.s5QIIV", &[("val", &f(fl.q_iiv, 3)), ("ls", &f(fl.q_iiv_ls, 3))]), 9.0, Face::Regular);
    let rz = &p.reservoir;
    w.title(&tt("export.pdf.s6Title"));
    w.para(&t("export.pdf.s6Info", &[("amplasare", &rz.amplasare), ("forma", &rz.forma), ("material", &rz.material)]), 9.0, Face::Regular);
    for (key, v) in [("s6VComp", rr.v_compensation), ("s6VInc", rr.v_fire_reserve), ("s6VAvarie", rr.v_emergency), ("s6VTotal", rr.v_total)] {
        w.para(&t(&format!("export.pdf.{key}"), &[("val", &f(v, 3))]), 9.0, Face::Regular);
    }

    // 7. Rețeaua
    if !secs.is_empty() {
        w.new_page();
        w.title(&tt("export.pdf.s7Title"));
        let head = ["s7ColTronson", "s7ColL", "s7ColQ", "s7ColDext", "s7ColDint", "s7ColV", "s7ColRe", "s7ColRegim", "s7ColHlin", "s7ColHloc", "s7ColHtot", "s7ColI"].map(|k| tt(&format!("export.pdf.{k}")));
        let regime = |r: &str| {
            let k = format!("network.regime.{r}");
            let s = tt(&k);
            if s == k { r.to_string() } else { s }
        };
        let rows: Vec<Vec<String>> = secs
            .iter()
            .map(|s| {
                vec![
                    s.name.clone(),
                    f(s.length, 1),
                    f(s.q_c, 3),
                    f(s.d_std_ext, 0),
                    f(s.d_std_int, 1),
                    f(s.v_real, 3),
                    f(s.re, 0),
                    regime(&s.regime),
                    f(s.h_lin, 4),
                    f(s.h_loc, 4),
                    f(s.h_tot, 4),
                    f(s.slope, 6),
                ]
            })
            .collect();
        w.table(&head, &rows, 6.5);
    }

    // 8. Schema
    if !p.network_schema.is_empty() {
        w.new_page();
        w.title(&tt("export.pdf.s8SchemaTitle"));
        let hgt = CONTENT_W * 0.75;
        let top = w.y;
        schema_pdf::draw(&mut w, &p.network_schema, secs, scale_ratio, (MARGIN, top, CONTENT_W, hgt));
        w.y = top + hgt + 8.0;
        if let Some(l) = map_line {
            w.text_at(MARGIN, w.y + 8.0, 8.0, Face::Regular, GREY, &l, Align::Left);
        }
    }
    w.footer(1, &format!("{} — {}", g.name, tt("export.pdf.title")));
    w.finish()
}

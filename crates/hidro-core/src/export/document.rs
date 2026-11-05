//! Datele pentru documente (Memoriu tehnic .docx, liste de cantități .xlsx),
//! portat din `documentData.ts`. Cheile sunt contractul cu șabloanele utilizatorului
//! — nu se redenumesc (vezi `docs/sabloane-export.md` din aplicația React).

use super::quantities::{self, pipe_depth};
use super::{round, Tr};
use crate::calculations::network::NetworkSection;
use crate::model::ProjectData;
use crate::pipeline::ProjectResults;
use serde_json::{json, Value};

/// Număr rotunjit la `d` zecimale; `null` pentru valori lipsă sau infinite.
fn r(v: f64, d: i32) -> Value {
    if v.is_finite() {
        json!(round(v, d))
    } else {
        Value::Null
    }
}

fn ro(v: Option<f64>, d: i32) -> Value {
    v.map_or(Value::Null, |x| r(x, d))
}

/// `today` = data generării, formatată de apelant (ex. „05.11.2026”).
pub fn build(p: &ProjectData, res: &ProjectResults, secs: &[NetworkSection], today: &str, t: Tr) -> Value {
    let (g, c, k, co, h, rz, nc) = (
        &p.general,
        &p.consumers,
        &p.water_need_constants,
        &p.water_demand_coefficients,
        &p.hydrants,
        &p.reservoir,
        &p.network_constants,
    );
    let (need, demand, hr, flows, rr) = (&res.water_need, &res.water_demand, &res.hydrants, &res.design_flows, &res.reservoir);
    let qty = quantities::build(p, secs, Some(rr), t);
    let is_cyl = rz.forma.to_lowercase().starts_with("cilindr");
    let regime = |r: &str| {
        let k = format!("network.regime.{r}");
        let s = t(&k, &[]);
        if s == k { r.to_string() } else { s }
    };
    let tronsoane: Vec<Value> = secs
        .iter()
        .enumerate()
        .map(|(i, s)| {
            json!({
                "nr": i + 1,
                "nume": s.name,
                "grup": s.group.clone().filter(|g| !g.is_empty()).unwrap_or_else(|| t("export.doc.mainSchema", &[])),
                "amonte": secs.iter().find(|x| Some(&x.id) == s.parent_section_id.as_ref()).map(|x| x.name.clone()).unwrap_or_else(|| t("export.doc.source", &[])),
                "lungime": r(s.length, 2),
                "cota_amonte": ro(s.cota_up, 2),
                "cota_aval": ro(s.cota_down, 2),
                "q_lin": r(s.q_linear, 5),
                "q_extern": r(s.q_extern, 3),
                "q_ie": r(s.q_ie, 3),
                "q_i": r(s.q_i, 3),
                "q_f": r(s.q_f, 3),
                "q_c": r(s.q_c, 3),
                "d_calc": r(s.d_calc, 1),
                "de": s.d_std_ext,
                "di": r(s.d_std_int, 1),
                "v": r(s.v_real, 3),
                "re": r(s.re, 0),
                "regim": regime(&s.regime),
                "lambda": r(s.lambda, 5),
                "zeta": r(s.zeta, 2),
                "h_lin": r(s.h_lin, 4),
                "h_loc": r(s.h_loc, 4),
                "h_tot": r(s.h_tot, 4),
                "panta": r(s.slope, 6),
                "hidrant": if s.has_hydrant { t("export.doc.yes", &[]) } else { t("export.doc.no", &[]) },
            })
        })
        .collect();
    let v_min = secs.iter().map(|s| s.v_real).fold(f64::INFINITY, f64::min);
    let v_max = secs.iter().map(|s| s.v_real).fold(f64::NEG_INFINITY, f64::max);
    json!({
        "proiect": {
            "nume": g.name, "localitate": g.locality, "judet": g.county, "beneficiar": g.beneficiary,
            "proiectant": g.designer, "data": g.date, "sursa_apa": g.water_source, "data_generare": today,
        },
        "consum": {
            "locuitori": c.inhabitants, "rata_crestere": c.growth_rate, "ani_perspectiva": c.perspective_years,
            "locuitori_perspectiva": c.perspective_inhabitants, "q_gosp": c.q_gosp_per_capita,
            "are_consumatori_publici": !c.public_consumers.is_empty(),
            "consumatori_publici": c.public_consumers.iter().enumerate().map(|(i, pc)| json!({
                "nr": i + 1,
                "tip": if pc.kind == "Altele" { &pc.custom_type } else { &pc.kind },
                "cantitate": pc.quantity, "nr_consumatori": pc.num_consumers, "unitate": pc.unit_type,
                "q_specific": pc.q_specific, "ore_zi": pc.hours_per_day, "kor": pc.kor,
            })).collect::<Vec<_>>(),
        },
        "coeficienti": {
            "kor_gosp": k.kor_gosp, "kor_public": k.kor_public_med, "kzi_gosp": k.kzi_gosp, "kzi_public": k.kzi_public,
            "kp": co.kp, "kss": co.kss, "ksr": co.ksr, "ks": r(demand.ks, 4),
        },
        "necesar": {
            "qzi_med_gosp": r(need.q_n_zi_med_gosp, 3), "qzi_med_pub": r(need.q_n_zi_med_pub, 3), "qzi_med": r(need.q_n_zi_med, 3),
            "qzi_max_gosp": r(need.q_n_zi_max_gosp, 3), "qzi_max_pub": r(need.q_n_zi_max_pub, 3), "qzi_max": r(need.q_n_zi_max, 3),
            "qor_max_gosp": r(need.q_n_or_max_gosp, 3), "qor_max_pub": r(need.q_n_or_max_pub, 3), "qor_max": r(need.q_n_or_max, 3),
        },
        "cerinta": {
            "qzi_med": r(demand.q_s_zi_med, 3), "qzi_max": r(demand.q_s_zi_max, 3),
            "qor_max": r(demand.q_s_or_max, 3), "qor_max_ls": r(demand.q_s_or_max_ls, 3),
        },
        "incendiu": {
            "q_hidrant_ext": h.debit_hydrant, "p_min_ext": h.min_pressure, "t_ext": h.operating_time,
            "n_ext": h.simultaneous_fires, "a": h.coeff_a, "nr_hidranti_ext": h.num_hydrants,
            "q_hidrant_int": h.debit_hydrant_int, "p_min_int": h.min_pressure_int, "t_int": h.operating_time_int,
            "n_int": h.simultaneous_fires_int, "nr_hidranti_int": h.num_hydrants_int,
            "v_ri": r(hr.v_ri, 3), "t_ri": hr.t_ri, "q_ri": r(hr.q_ri, 4),
        },
        "debite": {
            "q_ic": r(flows.q_ic, 3), "q_ic_prim": r(flows.q_ic_prime, 3), "q_iic": r(flows.q_iic, 3),
            "q_iiv": r(flows.q_iiv, 3), "q_iiv_ls": r(flows.q_iiv_ls, 3), "q_dim_retea": r(flows.q_dim_network, 3),
        },
        "rezervor": {
            "amplasare": rz.amplasare, "material": rz.material, "forma": rz.forma, "pozitie": rz.pozitie,
            "nr_cuve": rz.num_cuves, "cota_teren": rz.cota_teren, "h_b": rz.hb, "i_med": rz.i_med, "l_retea": rz.l,
            "h_r": r(rr.hr, 3), "cota_radier": r(rr.cota_radier, 2), "c": rz.c,
            "v_comp": r(rr.v_compensation, 3), "v_inc": r(rr.v_fire_reserve, 3), "v_avarie": r(rr.v_emergency.max(0.0), 3),
            "q_min": r(rr.q_min, 3), "t_av": rz.t_av, "t_i": rz.ti, "q_a": rz.qa,
            "v_total": r(rr.v_total, 3), "v_total_rotunjit": rr.v_total.ceil(), "este_cilindric": is_cyl,
            "inaltime_totala": rz.dim_inaltime_totala, "inaltime": rz.dim_inaltime_rezervot,
            "diametru": rz.dim_diametru_rezervot, "latime": rz.dim_latime_rezervot, "lungime": rz.dim_lungime_rezervot,
            "capacitate_utila": rz.dim_capacitate_utila, "capacitate_totala": rz.dim_capacitate_totala,
        },
        "retea": {
            "nr_tronsoane": secs.len(),
            "lungime_totala": qty.total_length,
            "adancime_pozare": pipe_depth(p),
            "rugozitate": nc.roughness, "viscozitate": nc.viscosity, "viteza_economica": nc.economic_velocity,
            "h_total": r(secs.iter().map(|s| s.h_tot).sum(), 4),
            "v_min": if secs.is_empty() { Value::Null } else { r(v_min, 3) },
            "v_max": if secs.is_empty() { Value::Null } else { r(v_max, 3) },
            "are_tronsoane": !secs.is_empty(),
            "tronsoane": tronsoane,
            "diametre": qty.pipes.iter().enumerate().map(|(i, g)| json!({
                "nr": i + 1, "de": g.de, "di": g.di, "lungime": g.lungime, "tronsoane": g.tronsoane,
            })).collect::<Vec<_>>(),
        },
        // Cheile documentate (snake_case) + cele camelCase scrise de aplicația React.
        "terasamente": {
            "sapatura": qty.trench.sapatura, "nisip": qty.trench.nisip, "umplutura": qty.trench.umplutura,
            "latime_medie": qty.trench.latime_medie, "adancime_medie": qty.trench.adancime_medie,
            "latimeMedie": qty.trench.latime_medie, "adancimeMedie": qty.trench.adancime_medie,
        },
        "cantitati": qty.items,
    })
}

/// Rezolvă o cale „a.b.c” în date (comun pentru Word și Excel).
pub fn resolve_path<'a>(scope: &'a Value, path: &str) -> Option<&'a Value> {
    if path == "." || path == "this" {
        return Some(scope);
    }
    path.split('.').try_fold(scope, |cur, k| cur.get(k))
}

/// Formatare românească: virgulă zecimală, „da”/„nu” pentru boolean.
pub fn format_ro(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => if *b { "da" } else { "nu" }.into(),
        Value::Number(n) => {
            let f = n.as_f64().unwrap_or(0.0);
            let s = format!("{}", round(f, 6));
            // separator de mii „.” și zecimal „,”, ca toLocaleString('ro-RO')
            let (int, frac) = s.split_once('.').map_or((s.as_str(), None), |(a, b)| (a, Some(b)));
            let neg = int.starts_with('-');
            let digits = int.trim_start_matches('-');
            let mut grouped = String::new();
            for (i, ch) in digits.chars().enumerate() {
                if i > 0 && (digits.len() - i) % 3 == 0 {
                    grouped.push('.');
                }
                grouped.push(ch);
            }
            format!("{}{}{}", if neg { "-" } else { "" }, grouped, frac.map(|f| format!(",{f}")).unwrap_or_default())
        }
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn romanian_number_format() {
        assert_eq!(format_ro(&json!(1234567.5)), "1.234.567,5");
        assert_eq!(format_ro(&json!(-12.25)), "-12,25");
        assert_eq!(format_ro(&json!(120)), "120");
        assert_eq!(format_ro(&json!(true)), "da");
    }
}

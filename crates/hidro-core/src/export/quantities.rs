//! Liste de cantități (antemăsurătoare), portat din `quantities.ts`.
//! Terasamentele sunt ESTIMATIVE (secțiune dreptunghiulară de tranșee).

use super::{pipe_depth, round, Tr};
use crate::calculations::network::NetworkSection;
use crate::calculations::reservoir::ReservoirResults;
use crate::model::{ProjectData, SchemaKind};
use crate::schema::flatten_with_group;
use serde::Serialize;

pub const TRENCH_MIN_WIDTH: f64 = 0.6;
pub const TRENCH_EXTRA_WIDTH: f64 = 0.4;
pub const TRENCH_BEDDING_BELOW: f64 = 0.10;
pub const TRENCH_SAND_ABOVE: f64 = 0.15;

#[derive(Debug, Clone, Serialize)]
pub struct QuantityItem {
    pub nr: usize,
    pub categorie: String,
    pub denumire: String,
    pub um: String,
    pub cantitate: f64,
    pub observatii: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipeGroup {
    pub de: f64,
    pub di: f64,
    pub lungime: f64,
    pub tronsoane: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trench {
    pub sapatura: f64,
    pub nisip: f64,
    pub umplutura: f64,
    pub latime_medie: f64,
    pub adancime_medie: f64,
}

#[derive(Debug, Clone)]
pub struct Quantities {
    pub items: Vec<QuantityItem>,
    pub pipes: Vec<PipeGroup>,
    pub total_length: f64,
    pub trench: Trench,
}

pub fn build(p: &ProjectData, secs: &[NetworkSection], reservoir: Option<&ReservoirResults>, t: Tr) -> Quantities {
    let depth = pipe_depth(p);
    let mut pipes: Vec<PipeGroup> = Vec::new();
    for s in secs {
        match pipes.iter_mut().find(|g| g.de == s.d_std_ext) {
            Some(g) => {
                g.lungime += s.length;
                g.tronsoane = format!("{}, {}", g.tronsoane, s.name);
            }
            None => pipes.push(PipeGroup { de: s.d_std_ext, di: s.d_std_int, lungime: s.length, tronsoane: s.name.clone() }),
        }
    }
    for g in &mut pipes {
        g.lungime = round(g.lungime, 2);
    }
    pipes.sort_by(|a, b| a.de.total_cmp(&b.de));
    let total_length = round(secs.iter().map(|s| s.length).sum(), 2);

    let (mut sap, mut nisip, mut pipe_vol, mut w_w, mut d_w) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for s in secs {
        let de = s.d_std_ext / 1000.0;
        let b = TRENCH_MIN_WIDTH.max(de + TRENCH_EXTRA_WIDTH);
        let h = depth + de / 2.0 + TRENCH_BEDDING_BELOW;
        sap += s.length * b * h;
        nisip += s.length * b * (TRENCH_BEDDING_BELOW + de + TRENCH_SAND_ABOVE);
        pipe_vol += s.length * std::f64::consts::PI * de * de / 4.0;
        w_w += s.length * b;
        d_w += s.length * h;
    }
    nisip -= pipe_vol;
    let umplutura = (sap - nisip - pipe_vol).max(0.0);
    let trench = Trench {
        sapatura: round(sap, 2),
        nisip: round(nisip.max(0.0), 2),
        umplutura: round(umplutura, 2),
        latime_medie: if total_length > 0.0 { round(w_w / total_length, 2) } else { 0.0 },
        adancime_medie: if total_length > 0.0 { round(d_w / total_length, 2) } else { 0.0 },
    };

    let els = flatten_with_group(&p.network_schema);
    let count = |k: SchemaKind| els.iter().filter(|(e, _)| e.kind == k).count() as f64;
    let n_hyd = match count(SchemaKind::Hidrant) {
        0.0 => p.hydrants.num_hydrants,
        n => n,
    };

    let mut items: Vec<QuantityItem> = Vec::new();
    let mut push = |cat: &str, den: String, um: &str, q: f64, obs: String| {
        if q > 0.0 {
            items.push(QuantityItem { nr: items.len() + 1, categorie: cat.into(), denumire: den, um: um.into(), cantitate: round(q, 2), observatii: obs });
        }
    };
    let c_ter = t("export.qty.catEarthworks", &[]);
    let c_pipe = t("export.qty.catPipes", &[]);
    let c_eq = t("export.qty.catEquipment", &[]);
    let c_test = t("export.qty.catTests", &[]);
    let fmt2 = |v: f64| format!("{v}");
    push(&c_ter, t("export.qty.trenchExcavation", &[]), "m³", trench.sapatura, t("export.qty.trenchNote", &[("b", &fmt2(trench.latime_medie)), ("h", &fmt2(trench.adancime_medie))]));
    push(&c_ter, t("export.qty.sandBedding", &[]), "m³", trench.nisip, String::new());
    push(&c_ter, t("export.qty.backfill", &[]), "m³", trench.umplutura, String::new());
    for g in &pipes {
        let den = if g.de > 0.0 { t("export.qty.pipe", &[("de", &fmt2(g.de))]) } else { t("export.qty.pipeUnsized", &[]) };
        push(&c_pipe, den, "m", g.lungime, g.tronsoane.clone());
    }
    push(&c_pipe, t("export.qty.warningTape", &[]), "m", total_length, String::new());
    push(&c_eq, t("export.qty.hydrant", &[]), "buc", n_hyd, String::new());
    push(&c_eq, t("export.qty.caminGrav", &[]), "buc", count(SchemaKind::CaminGrav), String::new());
    push(&c_eq, t("export.qty.caminAer", &[]), "buc", count(SchemaKind::CaminAer), String::new());
    if let Some(r) = reservoir.filter(|r| r.v_total > 0.0) {
        push(&c_eq, t("export.qty.reservoir", &[("v", &fmt2(r.v_total.ceil()))]), "buc", 1.0, format!("{} / {}", p.reservoir.forma, p.reservoir.material));
    }
    push(&c_test, t("export.qty.pressureTest", &[]), "m", total_length, String::new());
    push(&c_test, t("export.qty.disinfection", &[]), "m", total_length, String::new());
    Quantities { items, pipes, total_length, trench }
}

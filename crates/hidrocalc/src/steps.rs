//! Conținutul celor 9 pași (pașii 8–9 sunt în `network_ui`).

use crate::i18n::I18n;
use crate::widgets::{self as w, pal, CardSize};
use eframe::egui::{self, RichText, Ui};
use hidro_core::defaults::{institution_default, INSTITUTION_DEFAULTS, UNIT_TYPE_PRESETS};
use hidro_core::model::{ProjectData, PublicConsumer};
use hidro_core::pipeline::ProjectResults;
use crate::network_ui::NetworkUi;

/// Afișează pasul curent; întoarce pasul spre care s-a navigat (Înapoi/Următorul).
pub fn show(ui: &mut Ui, t: &I18n, p: &mut ProjectData, r: &ProjectResults, net: &mut NetworkUi) -> Option<u32> {
    match p.current_step {
        1 => step1(ui, t, p),
        2 => step2(ui, t, p),
        3 => step3(ui, t, p, r),
        4 => step4(ui, t, p, r),
        5 => step5(ui, t, p, r),
        6 => step6(ui, t, p, r),
        7 => step7(ui, t, p, r),
        8 => crate::network_ui::step8(ui, t, p, net),
        _ => crate::network_ui::step9(ui, t, p, r, net),
    }
}

fn opts(t: &I18n, prefix: &str, items: &[(&str, &str)]) -> Vec<(String, String)> {
    items.iter().map(|(value, key)| (value.to_string(), t.t(&format!("{prefix}.{key}")))).collect()
}

// ─── Pasul 1 ────────────────────────────────────────────────────────────────

fn step1(ui: &mut Ui, t: &I18n, p: &mut ProjectData) -> Option<u32> {
    w::centered(ui, 720.0, |ui| {
        w::step_header(ui, &t.t("step1.title"), &t.t("step1.subtitle"));
        let g = &mut p.general;
        w::card(ui, |ui| {
            w::text_field(ui, &t.t("step1.projectName"), &mut g.name, "Ex: Alimentare cu apă sat");
            ui.add_space(6.0);
            ui.columns(2, |c| {
                w::text_field(&mut c[0], &t.t("step1.locality"), &mut g.locality, "Ex: Sat Example");
                w::text_field(&mut c[1], &t.t("step1.county"), &mut g.county, "Ex: Cluj");
            });
            ui.add_space(6.0);
            ui.columns(2, |c| {
                w::text_field(&mut c[0], &t.t("step1.beneficiary"), &mut g.beneficiary, "Ex: Primăria Comunei Example");
                w::text_field(&mut c[1], &t.t("step1.designer"), &mut g.designer, "Ex: SC Proiect SRL");
            });
            ui.add_space(6.0);
            ui.columns(2, |c| {
                w::text_field(&mut c[0], &t.t("step1.date"), &mut g.date, "AAAA-LL-ZZ");
                let sources = opts(
                    t,
                    "step1.waterSources",
                    &[
                        ("conductă magistrală existentă", "conductaMagistralExistenta"),
                        ("captare subterană", "captareSubterana"),
                        ("captare de suprafață", "captareDeSuprafata"),
                    ],
                );
                w::combo(&mut c[1], &t.t("step1.waterSource"), &mut g.water_source, &sources);
            });
        });
        w::step_nav(ui, t, None, Some(2))
    })
}

// ─── Pasul 2 ────────────────────────────────────────────────────────────────

fn step2(ui: &mut Ui, t: &I18n, p: &mut ProjectData) -> Option<u32> {
    let pl = pal(ui);
    w::centered(ui, 1040.0, |ui| {
        w::step_header(ui, &t.t("step2.title"), &t.t("step2.subtitle"));
        let c = &mut p.consumers;
        w::card(ui, |ui| {
            w::section_title(ui, &t.t("step2.sectionA"));
            ui.columns(4, |col| {
                w::num_field(&mut col[0], &t.t("step2.inhabitants"), &mut c.inhabitants, None, None);
                w::num_field(
                    &mut col[1],
                    &t.t("step2.growthRate"),
                    &mut c.growth_rate,
                    Some("%/an"),
                    Some(&format!(
                        "Rata anuală de creștere a populației, folosită la calculul pe perspectivă de {} ani. Sursă: NP 133.",
                        c.perspective_years
                    )),
                );
                w::num_field(
                    &mut col[2],
                    &t.t("step2.perspectiveYears"),
                    &mut c.perspective_years,
                    Some("ani"),
                    Some("Orizontul de timp (ani) pentru prognoza populației de perspectivă. Sursă: NP 133."),
                );
                w::computed_field(
                    &mut col[3],
                    &t.t("step2.perspectiveInhabitants"),
                    &w::fmt(c.perspective_inhabitants, 0),
                    Some("loc."),
                    Some("Populația de perspectivă, calculată automat: N₀ × (1 + p/100)^t."),
                    false,
                );
            });
            ui.add_space(6.0);
            ui.columns(4, |col| {
                w::num_field(
                    &mut col[0],
                    &t.t("step2.qGospPerCapita"),
                    &mut c.q_gosp_per_capita,
                    Some("l/om·zi"),
                    Some("Debitul specific de apă per locuitor pe zi (q_s). Sursă: NP 133 (uzual 80–150 l/om·zi)."),
                );
            });
        });
        ui.add_space(12.0);

        w::card(ui, |ui| {
            ui.horizontal(|ui| {
                w::section_title(ui, &t.t("step2.sectionB"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if w::primary_button(ui, &t.t("step2.addInstitution")).clicked() {
                        c.public_consumers.push(PublicConsumer { id: uuid::Uuid::new_v4().to_string(), ..Default::default() });
                    }
                });
            });
            if c.public_consumers.is_empty() {
                ui.add_space(14.0);
                ui.vertical_centered(|ui| ui.label(RichText::new(t.t("step2.noInstitutions")).color(pl.muted)));
                ui.add_space(14.0);
                return;
            }
            let kinds: Vec<(String, String)> = INSTITUTION_DEFAULTS.iter().map(|i| (i.kind.to_string(), i.kind.to_string())).collect();
            let mut remove = None;
            egui::Grid::new("public_consumers").striped(true).spacing([6.0, 6.0]).min_col_width(60.0).show(ui, |ui| {
                for h in [
                    t.t("step2.colInstitutionType"),
                    t.t("step2.colUnits"),
                    format!("{} (Npi)", t.t("step2.colConsumers")),
                    t.t("step2.colConsumerType"),
                    format!("{} (q_pi)", t.t("step2.colQSpecific")),
                    t.t("step2.colHoursPerDay"),
                    t.t("step2.colKor"),
                    String::new(),
                ] {
                    ui.label(RichText::new(h.to_uppercase()).size(10.5).color(pl.muted).strong());
                }
                ui.end_row();
                for (i, pc) in c.public_consumers.iter_mut().enumerate() {
                    let id = egui::Id::unique(("pc", &pc.id));
                    ui.vertical(|ui| {
                        ui.set_width(190.0);
                        let mut kind = pc.kind.clone();
                        if w::combo_input(ui, id.with("type"), &mut kind, &kinds, 185.0) {
                            pc.kind = kind;
                            if let Some(d) = institution_default(&pc.kind) {
                                pc.unit_type = d.unit_type.into();
                                pc.q_specific = d.q_specific;
                                pc.hours_per_day = d.hours_per_day;
                                pc.kor = d.kor;
                            }
                        }
                        if pc.kind == "Altele" {
                            ui.add(egui::TextEdit::singleline(&mut pc.custom_type).hint_text(t.t("step2.specifyType")).desired_width(185.0));
                        }
                    });
                    num_cell(ui, id.with("q"), &mut pc.quantity, 70.0);
                    num_cell(ui, id.with("n"), &mut pc.num_consumers, 80.0);
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut pc.unit_type).hint_text(t.t("step2.consumerTypePlaceholder")).desired_width(110.0));
                        ui.menu_button("▾", |ui| {
                            for u in UNIT_TYPE_PRESETS {
                                if ui.button(*u).clicked() {
                                    pc.unit_type = (*u).into();
                                    ui.close();
                                }
                            }
                        });
                    });
                    num_cell(ui, id.with("qs"), &mut pc.q_specific, 70.0);
                    num_cell(ui, id.with("h"), &mut pc.hours_per_day, 60.0);
                    ui.scope(|ui| {
                        num_cell(ui, id.with("kor"), &mut pc.kor, 60.0);
                    })
                    .response
                    .on_hover_text(t.t("step2.korTooltip"));
                    if ui.add(egui::Button::new(RichText::new("✕").color(pl.muted)).frame(false)).on_hover_text(t.t("common.delete")).clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
            if let Some(i) = remove {
                c.public_consumers.remove(i);
            }
        });
        w::step_nav(ui, t, Some(1), Some(3))
    })
}

fn num_cell(ui: &mut Ui, id: egui::Id, v: &mut f64, width: f32) {
    ui.scope(|ui| {
        ui.set_width(width);
        w::num_input(ui, id, v, 0.0);
    });
}

// ─── Pasul 3 ────────────────────────────────────────────────────────────────

fn constants_column(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.set_width(width);
        w::card(ui, add);
    });
}

fn step3(ui: &mut Ui, t: &I18n, p: &mut ProjectData, r: &ProjectResults) -> Option<u32> {
    let pl = pal(ui);
    w::centered(ui, 1080.0, |ui| {
        w::step_header(ui, &t.t("step3.title"), &t.t("step3.subtitle"));
        let n = if p.consumers.perspective_inhabitants > 0.0 { p.consumers.perspective_inhabitants } else { p.consumers.inhabitants };
        let qg = p.consumers.q_gosp_per_capita;
        let k = &mut p.water_need_constants;
        let wn = &r.water_need;
        ui.horizontal_top(|ui| {
            constants_column(ui, 230.0, |ui| {
                w::section_title(ui, &format!("{} NP 133", t.t("common.constants")));
                w::num_field(ui, &t.t("step3.korGosp"), &mut k.kor_gosp, None, Some("Coeficient de variație orară (K_o) pentru consumul gospodăresc. Sursă: NP 133."));
                w::num_field(ui, &t.t("step3.korPublicMed"), &mut k.kor_public_med, None, Some("Coeficient de variație orară mediu (K_o) pentru consumul public. Sursă: NP 133."));
                w::num_field(ui, &t.t("step3.kziGosp"), &mut k.kzi_gosp, None, Some("Coeficient de variație zilnică (K_zi) pentru consumul gospodăresc. Sursă: NP 133."));
                w::num_field(ui, &t.t("step3.kziPublic"), &mut k.kzi_public, None, Some("Coeficient de variație zilnică (K_zi) pentru consumul public. Sursă: NP 133."));
            });
            ui.add_space(12.0);
            ui.vertical(|ui| {
                let block = |ui: &mut Ui, title: &str, cards: [(&str, f64); 3], unit: &str, lines: Vec<String>| {
                    w::card(ui, |ui| {
                        ui.label(RichText::new(title.to_uppercase()).size(11.5).color(pl.muted).strong());
                        ui.columns(3, |c| {
                            for (i, (label, v)) in cards.iter().enumerate() {
                                w::result_card(&mut c[i], label, *v, unit, i == 2, CardSize::Normal);
                            }
                        });
                        ui.add_space(4.0);
                        w::formula(ui, &lines);
                    });
                    ui.add_space(10.0);
                };
                block(
                    ui,
                    &t.t("step3.dailyMeanFlow"),
                    [("Q_n_zi_med_gosp", wn.q_n_zi_med_gosp), ("Q_n_zi_med_pub", wn.q_n_zi_med_pub), ("Q_n_zi_med", wn.q_n_zi_med)],
                    "m³/zi",
                    vec![
                        format!("Q_n_zi_med_gosp = (Ng × qg) / 1000 = ({n} × {qg}) / 1000 = {} m³/zi", w::fmt(wn.q_n_zi_med_gosp, 3)),
                        format!("Q_n_zi_med_pub = Σ(Nr_unitati × Npi × qpi) / 1000 = {} m³/zi", w::fmt(wn.q_n_zi_med_pub, 3)),
                        format!("Q_n_zi_med = {} + {} = {} m³/zi", w::fmt(wn.q_n_zi_med_gosp, 3), w::fmt(wn.q_n_zi_med_pub, 3), w::fmt(wn.q_n_zi_med, 3)),
                    ],
                );
                block(
                    ui,
                    &t.t("step3.dailyMaxFlow"),
                    [("Q_n_zi_max_g", wn.q_n_zi_max_gosp), ("Q_n_zi_max_pub", wn.q_n_zi_max_pub), ("Q_n_zi_max", wn.q_n_zi_max)],
                    "m³/zi",
                    vec![
                        format!("Q_n_zi_max_g = Q_n_zi_med_gosp × k_zi_g = {} × {} = {} m³/zi", w::fmt(wn.q_n_zi_med_gosp, 3), k.kzi_gosp, w::fmt(wn.q_n_zi_max_gosp, 3)),
                        format!("Q_n_zi_max_pub = Q_n_zi_med_pub × k_zi_p = {} × {} = {} m³/zi", w::fmt(wn.q_n_zi_med_pub, 3), k.kzi_public, w::fmt(wn.q_n_zi_max_pub, 3)),
                        format!("Q_n_zi_max = {} + {} = {} m³/zi", w::fmt(wn.q_n_zi_max_gosp, 3), w::fmt(wn.q_n_zi_max_pub, 3), w::fmt(wn.q_n_zi_max, 3)),
                    ],
                );
                block(
                    ui,
                    &t.t("step3.hourlyMaxFlow"),
                    [("Q_n_or_max_gosp", wn.q_n_or_max_gosp), ("Q_n_or_max_pub", wn.q_n_or_max_pub), ("Q_n_or_max", wn.q_n_or_max)],
                    "m³/h",
                    vec![
                        format!("Q_n_or_max_gosp = Q_n_zi_max_g × k_or / 24 = {} × {} / 24 = {} m³/h", w::fmt(wn.q_n_zi_max_gosp, 3), k.kor_gosp, w::fmt(wn.q_n_or_max_gosp, 3)),
                        format!("Q_n_or_max_pub = Q_n_zi_max_pub × k_or_p_med / 24 = {} m³/h", w::fmt(wn.q_n_or_max_pub, 3)),
                        format!("Q_n_or_max = {} + {} = {} m³/h", w::fmt(wn.q_n_or_max_gosp, 3), w::fmt(wn.q_n_or_max_pub, 3), w::fmt(wn.q_n_or_max, 3)),
                    ],
                );
            });
        });
        w::step_nav(ui, t, Some(2), Some(4))
    })
}

// ─── Pasul 4 ────────────────────────────────────────────────────────────────

fn step4(ui: &mut Ui, t: &I18n, p: &mut ProjectData, r: &ProjectResults) -> Option<u32> {
    w::centered(ui, 1000.0, |ui| {
        w::step_header(ui, &t.t("step4.title"), &t.t("step4.subtitle"));
        let k = &mut p.water_demand_coefficients;
        let wd = &r.water_demand;
        let wn = &r.water_need;
        w::formula(
            ui,
            &[format!(
                "Q_s = k_p × k_s × Q_n    |    k_p = {}   k_s = (k_ss + k_sr) − 1 = ({} + {}) − 1 = {}",
                k.kp,
                k.kss,
                k.ksr,
                w::fmt(wd.ks, 4)
            )],
        );
        ui.add_space(8.0);
        if wd.ks_warning {
            w::alert(ui, &t.t("step4.ksWarning"), false);
            ui.add_space(8.0);
        }
        ui.horizontal_top(|ui| {
            constants_column(ui, 240.0, |ui| {
                w::section_title(ui, &t.t("step4.coefficients"));
                w::num_field(ui, &t.t("step4.kp"), &mut k.kp, None, Some("Coeficient de pierderi tehnice în rețeaua de distribuție (K_p). Sursă: NP 133."));
                w::num_field(ui, &t.t("step4.kss"), &mut k.kss, None, Some("Coeficient de siguranță privind acoperirea necesarului la sursă (K_ss). Sursă: NP 133."));
                w::num_field(ui, &t.t("step4.ksr"), &mut k.ksr, None, Some("Coeficient de siguranță privind rezerva/funcționarea rezervorului (K_sr). Sursă: NP 133."));
                w::computed_field(ui, "Ks = (Kss + Ksr) − 1", &w::fmt(wd.ks, 4), None, Some("Coeficient de siguranță global, calculat: Ks = (Kss + Ksr) − 1. Sursă: NP 133."), false);
            });
            ui.add_space(12.0);
            ui.vertical(|ui| {
                w::result_card(ui, "QS or max", wd.q_s_or_max_ls, "l/s", true, CardSize::Large);
                ui.add_space(4.0);
                w::formula(ui, &[format!("Q_s_or_max = {} × 1000 / 3600 = {} l/s", w::fmt(wd.q_s_or_max, 3), w::fmt(wd.q_s_or_max_ls, 3))]);
                ui.add_space(8.0);
                ui.columns(3, |c| {
                    let items = [
                        ("QS zi med", wd.q_s_zi_med, "m³/zi", wn.q_n_zi_med),
                        ("QS zi max", wd.q_s_zi_max, "m³/zi", wn.q_n_zi_max),
                        ("QS or max", wd.q_s_or_max, "m³/h", wn.q_n_or_max),
                    ];
                    for (i, (label, v, unit, qn)) in items.into_iter().enumerate() {
                        w::result_card(&mut c[i], label, v, unit, false, CardSize::Normal);
                        c[i].add_space(4.0);
                        w::formula(&mut c[i], &[format!("{} × {} × {} = {}", k.kp, w::fmt(wd.ks_safe, 4), w::fmt(qn, 3), w::fmt(v, 3))]);
                    }
                });
            });
        });
        w::step_nav(ui, t, Some(3), Some(5))
    })
}

// ─── Pasul 5 ────────────────────────────────────────────────────────────────

fn step5(ui: &mut Ui, t: &I18n, p: &mut ProjectData, r: &ProjectResults) -> Option<u32> {
    w::centered(ui, 960.0, |ui| {
        w::step_header(ui, &t.t("step5.title"), &t.t("step5.subtitle"));
        let h = &mut p.hydrants;
        let hr = &r.hydrants;
        ui.columns(2, |c| {
            w::result_card(&mut c[0], &t.t("step5.vRI"), hr.v_ri, "m³", true, CardSize::Large);
            w::result_card(&mut c[1], &t.t("step5.qRI"), hr.q_ri, "m³/zi", true, CardSize::Large);
        });
        ui.add_space(6.0);
        w::formula(
            ui,
            &[
                "V_RI = a × Q_s_or_max × T_ie + 3.6 × n_inc_ext × Q_ie × T_ie + 3.6 × T_ii × n_inc_int × Q_ii".into(),
                format!(
                    "     = {} × {} × {} + 3.6 × {} × {} × {} + 3.6 × {} × {} × {}",
                    h.coeff_a,
                    w::fmt(r.water_demand.q_s_or_max, 3),
                    h.operating_time,
                    h.simultaneous_fires,
                    h.debit_hydrant,
                    h.operating_time,
                    h.operating_time_int,
                    h.simultaneous_fires_int,
                    h.debit_hydrant_int
                ),
                format!("     = {} m³", w::fmt(hr.v_ri, 3)),
                format!("Q_RI = V_RI / T_Ri × 24 = {} / {} × 24 = {} m³/zi", w::fmt(hr.v_ri, 3), hr.t_ri, w::fmt(hr.q_ri, 3)),
            ],
        );
        ui.add_space(12.0);
        w::card(ui, |ui| {
            w::section_title(ui, &format!("{}  ·  sursă V_RI", t.t("step5.exterior")));
            ui.columns(3, |c| {
                w::num_field(&mut c[0], &t.t("step5.debitHydrant"), &mut h.debit_hydrant, Some("l/s"), Some("Debitul unui hidrant exterior pentru stingerea incendiului. Sursă: P 118/2-2013, art. 6.28."));
                w::num_field(&mut c[1], &t.t("step5.operatingTime"), &mut h.operating_time, Some("ore"), Some("Durata teoretică de funcționare a hidranților la incendiu. Sursă: P 118/2-2013, art. 6.19."));
                w::num_field(&mut c[2], &t.t("step5.simultaneousFires"), &mut h.simultaneous_fires, None, Some("Numărul de incendii simultane considerate la dimensionare. Sursă: P 118/2-2013, art. 6.23."));
            });
            ui.add_space(4.0);
            ui.columns(3, |c| {
                w::num_field(&mut c[0], &t.t("step5.coeffA"), &mut h.coeff_a, None, Some("Coeficient funcție de regimul de presiune al rețelei de distribuție. Sursă: NP 133."));
                w::num_field(&mut c[1], &t.t("step5.timpRefacere"), &mut h.timp_refacere, Some("ore"), Some("Timpul de refacere a rezervei intangibile de incendiu. Sursă: NP 133."));
            });
            ui.add_space(4.0);
            ui.columns(2, |c| {
                w::num_field(&mut c[0], &t.t("step5.minPressure"), &mut h.min_pressure, Some("bar"), Some("Presiunea minimă necesară la hidranții exteriori (informativ). Sursă: P 118/2-2013, art. 6.30."));
                w::num_field(&mut c[1], &t.t("step5.numHydrants"), &mut h.num_hydrants, None, Some("Numărul de hidranți exteriori (informativ, pentru documentație). Sursă: P 118/2-2013."));
            });
        });
        ui.add_space(12.0);
        w::card(ui, |ui| {
            w::section_title(ui, &t.t("step5.interior"));
            ui.columns(3, |c| {
                w::num_field(&mut c[0], &t.t("step5.debitHydrantInt"), &mut h.debit_hydrant_int, Some("l/s"), None);
                w::num_field(&mut c[1], &t.t("step5.operatingTimeInt"), &mut h.operating_time_int, Some("ore"), None);
                w::num_field(&mut c[2], &t.t("step5.simultaneousFiresInt"), &mut h.simultaneous_fires_int, None, None);
            });
            ui.add_space(4.0);
            ui.columns(3, |c| {
                w::num_field(&mut c[0], &t.t("step5.minPressureInt"), &mut h.min_pressure_int, Some("bar"), Some("Presiunea minimă la hidranții interiori (informativ, pentru documentație). Sursă: P 118/2-2013."));
                w::num_field(&mut c[1], &t.t("step5.numHydrantsInt"), &mut h.num_hydrants_int, None, Some("Numărul de hidranți interiori (informativ, pentru documentație). Sursă: P 118/2-2013."));
            });
        });
        w::step_nav(ui, t, Some(4), Some(6))
    })
}

// ─── Pasul 6 ────────────────────────────────────────────────────────────────

fn step6(ui: &mut Ui, t: &I18n, p: &mut ProjectData, r: &ProjectResults) -> Option<u32> {
    let pl = pal(ui);
    w::centered(ui, 960.0, |ui| {
        w::step_header(ui, &t.t("step6.title"), &t.t("step6.subtitle"));
        w::section_title(ui, &t.t("step6.flowSchema"));
        let k = &p.water_demand_coefficients;
        let h = &p.hydrants;
        let (wd, hy, df) = (&r.water_demand, &r.hydrants, &r.design_flows);
        let rows = [
            (
                "C", pl.cat_capture, t.t("step6.captare"), t.t("step6.captareSubtitle"), "Q_IC", df.q_ic, "m³/zi",
                vec![format!("Q_IC = Q_s_zi_max + kp × ks × Q_RI = {} + {} × {} × {} = {} m³/zi", w::fmt(wd.q_s_zi_max, 3), k.kp, w::fmt(wd.ks_safe, 4), w::fmt(hy.q_ri, 3), w::fmt(df.q_ic, 3))],
            ),
            (
                "T", pl.cat_treat, t.t("step6.stTratare"), t.t("step6.stTratareSubtitle"), "Q'_IC", df.q_ic_prime, "m³/zi",
                vec![format!("Q'_IC = Q_IC × k_sr / k_s = {} × {} / {} = {} m³/zi", w::fmt(df.q_ic, 3), k.ksr, w::fmt(wd.ks_safe, 4), w::fmt(df.q_ic_prime, 3))],
            ),
            (
                "R", pl.cat_rezervor, t.t("step6.rezervor"), t.t("step6.rezervorSubtitle"), "Q_IIC", df.q_iic, "m³/h",
                vec![
                    format!("Q_IIC = Q_s_or_max + 3.6 × n_inc_int × Q_ii = {} + 3.6 × {} × {} = {} m³/h", w::fmt(wd.q_s_or_max, 3), h.simultaneous_fires_int, h.debit_hydrant_int, w::fmt(df.q_iic, 3)),
                    format!("Q_IIV = Q_s_or_max + 3.6 × n_inc_ext × Q_ie = {} + 3.6 × {} × {} = {} m³/h = {} l/s", w::fmt(wd.q_s_or_max, 3), h.simultaneous_fires, h.debit_hydrant, w::fmt(df.q_iiv, 3), w::fmt(df.q_iiv_ls, 3)),
                ],
            ),
            ("D", pl.teal, t.t("step6.retea"), t.t("step6.reteaSubtitle"), "Q_IIV", df.q_iiv, "m³/h", vec![]),
        ];
        for (badge, color, title, subtitle, qlabel, qv, unit, lines) in rows {
            egui::Frame::new()
                .fill(pl.panel)
                .stroke(egui::Stroke::new(1.0, pl.border))
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(egui::Margin { left: 14, right: 16, top: 12, bottom: 12 })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        egui::Frame::new().fill(color).corner_radius(egui::CornerRadius::same(20)).inner_margin(egui::Margin::symmetric(11, 7)).show(ui, |ui| {
                            ui.label(RichText::new(badge).strong().color(egui::Color32::WHITE));
                        });
                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            ui.set_width((ui.available_width() - 150.0).max(200.0));
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&title).strong());
                                ui.label(RichText::new(&subtitle).size(12.0).color(pl.muted));
                            });
                            for l in &lines {
                                ui.add(egui::Label::new(RichText::new(l).monospace().size(11.5).color(pl.ink2)).wrap());
                            }
                        });
                        ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                            ui.label(RichText::new(qlabel).monospace().size(11.0).color(pl.muted));
                            ui.label(RichText::new(w::fmt(qv, 3)).monospace().size(22.0).strong().color(color));
                            ui.label(RichText::new(unit).size(11.0).color(pl.muted));
                        });
                    });
                });
            ui.add_space(8.0);
        }
        egui::Frame::new()
            .fill(pl.accent_soft)
            .stroke(egui::Stroke::new(2.0, pl.teal))
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(t.t("step6.qDimTitle").to_uppercase()).size(11.5).strong().color(pl.teal));
                ui.label(
                    RichText::new(format!(
                        "Q_dim = max(Q_IIC, Q_IIV) = max({}, {}) = {} m³/h",
                        w::fmt(df.q_iic, 3),
                        w::fmt(df.q_iiv, 3),
                        w::fmt(df.q_dim_network, 3)
                    ))
                    .monospace()
                    .size(14.0)
                    .strong(),
                );
                ui.label(RichText::new(t.t("step6.qDimExplanation")).size(12.0).color(pl.muted));
            });
        w::step_nav(ui, t, Some(5), Some(7))
    })
}

// ─── Pasul 7 ────────────────────────────────────────────────────────────────

fn step7(ui: &mut Ui, t: &I18n, p: &mut ProjectData, r: &ProjectResults) -> Option<u32> {
    let pl = pal(ui);
    let total_len = p.total_network_length();
    w::centered(ui, 900.0, |ui| {
        w::step_header(ui, &t.t("step7.title"), &t.t("step7.subtitle"));
        let rz = &mut p.reservoir;
        let rr = &r.reservoir;
        let cr_diff = rr.cota_radier - rz.cota_teren;
        let cr_warning = !(8.0..=20.0).contains(&cr_diff);

        w::card(ui, |ui| {
            w::section_title(ui, &t.t("step7.cotesGeometrie"));
            ui.columns(2, |c| {
                w::num_field(&mut c[0], &t.t("step7.cotaTopo"), &mut rz.cota_teren, Some("mdMN"), Some("Cota topografică minimă din localitate (mdMN), reper pentru calculul presiunilor. Date de teren."));
                w::num_field(&mut c[1], &t.t("step7.hb"), &mut rz.hb, Some("mH₂O"), Some("Presiunea de serviciu necesară la branșament, funcție de regimul maxim al clădirilor de locuit. Sursă: NP 133."));
            });
            ui.add_space(4.0);
            ui.columns(2, |c| {
                w::num_field(&mut c[0], &t.t("step7.iMed"), &mut rz.i_med, None, Some("Panta hidraulică medie a rețelei de distribuție. Sursă: NP 133."));
                w::num_field(&mut c[1], &t.t("step7.lTotala"), &mut rz.l, Some("m"), Some("Lungimea totală a rețelei de distribuție (poate fi importată din schemă). Date de proiect."));
                if total_len > 0.0 {
                    c[1].horizontal(|ui| {
                        ui.label(RichText::new(format!("{} {} m", t.t("step7.fromSections"), w::fmt(total_len, 1))).size(11.5).color(pl.muted));
                        if rz.l != total_len && ui.link(t.t("step7.importBtn")).clicked() {
                            rz.l = (total_len * 10.0).round() / 10.0;
                        }
                    });
                }
            });
            ui.add_space(4.0);
            ui.columns(2, |c| {
                w::computed_field(&mut c[0], &t.t("step7.hr"), &w::fmt(rr.hr, 4), Some("mH₂O"), Some("Pierderea de sarcină pe circuitul rezervor–branșament, calculată: i_med × L_totală. Sursă: NP 133."), false);
                w::computed_field(&mut c[1], &t.t("step7.cotaRadier"), &w::fmt(rr.cota_radier, 2), Some("mdMN"), None, cr_warning);
                if cr_warning {
                    c[1].label(RichText::new(t.tf("step7.crWarning", &[("diff", &w::fmt(cr_diff, 2))])).size(11.5).color(pl.danger));
                }
            });
            ui.add_space(6.0);
            w::formula(
                ui,
                &[
                    format!("h_r = I_med × l = {} × {} = {} m", rz.i_med, rz.l, w::fmt(rr.hr, 4)),
                    format!("C_R = C_T + H_B + h_r = {} + {} + {} = {} m", rz.cota_teren, rz.hb, w::fmt(rr.hr, 4), w::fmt(rr.cota_radier, 2)),
                ],
            );
        });
        ui.add_space(12.0);

        w::card(ui, |ui| {
            w::section_title(ui, &t.t("step7.caracteristici"));
            let amplasare = opts(t, "step7.amplasareOptions", &[("suprateran", "suprateran"), ("semiîngropat", "semiingropat"), ("îngropat", "ingropat"), ("castel de apă", "castelDeApa")]);
            let material = opts(t, "step7.materialOptions", &[("oțel", "otel"), ("beton armat", "betonArmat"), ("polietilenă", "polietilena")]);
            let forma = opts(t, "step7.formaOptions", &[("cilindric", "cilindric"), ("paralelipipedic", "paralelipipedic")]);
            let pozitie = opts(t, "step7.pozitieOptions", &[("de capăt", "deCapat"), ("de trecere", "deTrecere"), ("contra-rezervor", "contraRezervot")]);
            ui.columns(3, |c| {
                w::combo(&mut c[0], &t.t("step7.amplasare"), &mut rz.amplasare, &amplasare);
                w::combo(&mut c[1], &t.t("step7.material"), &mut rz.material, &material);
                w::combo(&mut c[2], &t.t("step7.forma"), &mut rz.forma, &forma);
            });
            ui.add_space(4.0);
            ui.columns(3, |c| {
                w::combo(&mut c[0], &t.t("step7.pozitie"), &mut rz.pozitie, &pozitie);
                if w::num_field(&mut c[1], &t.t("step7.numCuves"), &mut rz.num_cuves, None, None) && rz.num_cuves < 1.0 {
                    rz.num_cuves = 1.0;
                }
            });
            let cil = rz.forma == "cilindric";
            if cil || rz.forma == "paralelipipedic" {
                ui.add_space(8.0);
                ui.label(RichText::new(t.t(if cil { "step7.dimCilindric" } else { "step7.dimParalelipipedic" }).to_uppercase()).size(11.5).color(pl.muted).strong());
                let mut fields: Vec<(String, &mut f64, &str)> = vec![
                    (t.t("step7.dimInaltimeTotala"), &mut rz.dim_inaltime_totala, "mm"),
                    (t.t("step7.dimInaltimeRezervot"), &mut rz.dim_inaltime_rezervot, "mm"),
                ];
                if cil {
                    fields.push((t.t("step7.dimDiametruRezervot"), &mut rz.dim_diametru_rezervot, "mm"));
                } else {
                    fields.push((t.t("step7.dimLatimeRezervot"), &mut rz.dim_latime_rezervot, "mm"));
                    fields.push((t.t("step7.dimLungimeRezervot"), &mut rz.dim_lungime_rezervot, "mm"));
                }
                fields.push((t.t("step7.dimCapacitateUtila"), &mut rz.dim_capacitate_utila, "m³"));
                fields.push((t.t("step7.dimCapacitateTotala"), &mut rz.dim_capacitate_totala, "m³"));
                if cil {
                    fields.push((t.t("step7.dimPlatformaBetonata"), &mut rz.dim_platforma_betonata, "mm"));
                } else {
                    fields.push((t.t("step7.dimPlatformaBetonataL"), &mut rz.dim_platforma_betonata_lungime, "mm"));
                    fields.push((t.t("step7.dimPlatformaBetonatal"), &mut rz.dim_platforma_betonata_latime, "mm"));
                }
                for chunk in fields.chunks_mut(3) {
                    ui.columns(3, |c| {
                        for (i, (label, v, unit)) in chunk.iter_mut().enumerate() {
                            w::num_field(&mut c[i], label, v, Some(unit), None);
                        }
                    });
                    ui.add_space(4.0);
                }
            }
        });
        ui.add_space(12.0);

        w::card(ui, |ui| {
            w::section_title(ui, &t.t("step7.volumCompensare"));
            ui.label(RichText::new("V_comp = c × Q_s_zi_max [m³]").monospace().size(11.5).color(pl.muted));
            ui.columns(2, |c| {
                w::num_field(&mut c[0], &t.t("step7.cCoef"), &mut rz.c, None, Some("Valoare subunitară (< 1). Implicit 0.15 conform NP 133. Reprezintă fracțiunea din debitul zilnic maxim necesară pentru compensarea variațiilor orare."));
                w::computed_field(&mut c[1], "V_comp", &w::fmt(rr.v_compensation, 3), Some("m³"), Some("Volumul de compensare orară, calculat: V_comp = c × Q_s_zi_max. Sursă: NP 133."), false);
            });
            ui.add_space(4.0);
            w::formula(ui, &[format!("V_comp = c × Q_s_zi_max = {} × {} = {} m³", rz.c, w::fmt(r.water_demand.q_s_zi_max, 3), w::fmt(rr.v_compensation, 3))]);
        });
        ui.add_space(12.0);

        w::card(ui, |ui| {
            w::section_title(ui, &t.t("step7.volumAvarie"));
            ui.label(RichText::new("V_av = Q_min × (T_av – T_i) – Q_a × T_av [m³]").monospace().size(11.5).color(pl.muted));
            ui.columns(2, |c| {
                w::num_field(&mut c[0], &t.t("step7.coefDebitMin"), &mut rz.coef_debit_min, None, None);
                w::computed_field(&mut c[1], "Q_min", &w::fmt(rr.q_min, 3), Some("m³/h"), Some("Debitul minim orar, calculat: Q_min = coef × Q_s_zi_max / 24. Sursă: NP 133."), false);
            });
            ui.add_space(4.0);
            ui.columns(3, |c| {
                w::num_field(&mut c[0], &t.t("step7.tAv"), &mut rz.t_av, Some("h"), None);
                w::num_field(&mut c[1], &t.t("step7.ti"), &mut rz.ti, Some("h"), None);
                w::num_field(&mut c[2], &t.t("step7.qa"), &mut rz.qa, Some("m³/h"), None);
            });
            ui.add_space(4.0);
            w::computed_field(ui, "V_avarie", &w::fmt(rr.v_emergency, 3), Some("m³"), Some("Volumul de avarie, calculat: V_av = Q_min × (T_av − T_i) − Q_a × T_av. Sursă: NP 133."), false);
            ui.add_space(4.0);
            let mut lines = vec![format!(
                "V_av = Q_min × (T_av – T_i) – Q_a × T_av = {} × ({} – {}) – {} × {} = {} m³",
                w::fmt(rr.q_min, 3),
                rz.t_av,
                rz.ti,
                rz.qa,
                rz.t_av,
                w::fmt(rr.v_emergency, 3)
            )];
            if rr.v_emergency < 0.0 {
                lines.push(t.t("step7.vAvarieNote"));
            }
            w::formula(ui, &lines);
        });
        ui.add_space(12.0);

        w::card(ui, |ui| {
            w::section_title(ui, &t.t("step7.volumTotal"));
            ui.label(RichText::new("V_rez = V_compensare + V_rezervă_incendiu + V_avarie").monospace().size(11.5).color(pl.muted));
            ui.columns(2, |c| {
                w::result_card(&mut c[0], &t.t("step7.vCompCard"), rr.v_compensation, "m³", false, CardSize::Normal);
                w::result_card(&mut c[1], &t.t("step7.vIncCard"), rr.v_fire_reserve, "m³", false, CardSize::Normal);
            });
            ui.add_space(6.0);
            ui.columns(2, |c| {
                w::result_card(&mut c[0], &t.t("step7.vAvCard"), rr.v_emergency.max(0.0), "m³", false, CardSize::Normal);
                w::result_card(&mut c[1], &t.t("step7.vTotalCard"), rr.v_total, "m³", true, CardSize::Large);
            });
            ui.add_space(6.0);
            w::formula(
                ui,
                &[format!(
                    "V_rez = {} + {} + {} = {} m³",
                    w::fmt(rr.v_compensation, 3),
                    w::fmt(rr.v_fire_reserve, 3),
                    w::fmt(rr.v_emergency.max(0.0), 3),
                    w::fmt(rr.v_total, 3)
                )],
            );
        });
        w::step_nav(ui, t, Some(6), Some(8))
    })
}

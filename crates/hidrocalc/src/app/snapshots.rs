//! Randări fără fereastră ale ecranelor principale (verificare vizuală).
//! `cargo test -p hidrocalc snapshots -- --nocapture` → PNG-uri în `target/ui-snapshots/`.

use super::*;
use hidro_core::model::PublicConsumer;

pub(super) fn sample(app: &mut HidroApp) {
    app.create_project();
    let p = app.project.as_mut().unwrap();
    p.general.name = "Alimentare cu apă sat Ștefănești".into();
    p.general.locality = "Ștefănești".into();
    p.general.county = "Timiș".into();
    p.consumers.inhabitants = 1850.0;
    p.consumers.public_consumers.push(PublicConsumer {
        id: "pc1".into(),
        kind: "Școală gimnazială".into(),
        quantity: 1.0,
        num_consumers: 240.0,
        unit_type: "elevi".into(),
        q_specific: 50.0,
        ..Default::default()
    });
    p.reservoir.cota_teren = 102.5;
    p.reservoir.hb = 14.0;
    p.reservoir.i_med = 0.003;
    p.reservoir.l = 4200.0;
    p.reservoir.coef_debit_min = 0.3;
    p.reservoir.t_av = 12.0;
    p.reservoir.ti = 2.0;
    // Rețea: R → Tr1 → (hidrant) → Tr2 ; Tr1 → Tr3 ; Tr3 → Tr4
    use hidro_core::model::{SchemaElement, SchemaKind};
    let tr = |id: &str, label: &str, a: (f64, f64), b: (f64, f64)| SchemaElement {
        id: id.into(),
        kind: SchemaKind::Tronson,
        x: a.0,
        y: a.1,
        x2: Some(b.0),
        y2: Some(b.1),
        label: Some(label.into()),
        ..Default::default()
    };
    p.network_schema = vec![
        SchemaElement { id: "rez".into(), kind: SchemaKind::Rezervor, x: -44.0, y: 0.0, ..Default::default() },
        tr("a", "Tr1", (0.0, 0.0), (1200.0, 0.0)),
        SchemaElement { id: "h".into(), kind: SchemaKind::Hidrant, x: 1207.0, y: 0.0, ..Default::default() },
        tr("b", "Tr2", (1214.0, 0.0), (2000.0, 0.0)),
        tr("c", "Tr3", (1203.0, 2.0), (1203.0, 900.0)),
        tr("d", "Tr4", (1203.0, 900.0), (1900.0, 900.0)),
    ];
    let mut n = 0;
    p.network_sections = hidro_core::schema::build_sections_from_schema(&p.network_schema, &[], 1000.0, || {
        n += 1;
        format!("s{n}")
    });
    for (i, s) in p.network_sections.iter_mut().enumerate() {
        s.q_linear = 0.004 + 0.002 * i as f64;
        s.zeta = 2.0;
    }
    p.network_sections[1].q_extern = 1.2;
    for (s, (up, dn)) in p.network_sections.iter_mut().zip([(102.5, 99.8), (99.8, 97.2), (99.8, 98.9), (98.9, 98.4)]) {
        s.cota_up = Some(up);
        s.cota_down = Some(dn);
        s.cota_up_source = Some("manual".into());
        s.cota_down_source = Some("manual".into());
    }
}

#[test]
fn render_main_screens() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
    std::fs::create_dir_all(&out).unwrap();
    let dir = std::env::temp_dir().join(format!("hidrocalc-snap-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    for dark in [false, true] {
        let db_path = dir.join(format!("s{dark}.db"));
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1360.0, 880.0)).with_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light }).wgpu().build_eframe(|cc| {
            HidroApp::with_db(cc, Settings { lang: Lang::Ro, dark, ..Default::default() }, Db::open(&db_path))
        });
        let suffix = if dark { "dark" } else { "light" };
        h.run_steps(3);
        h.render().unwrap().save(out.join(format!("home-empty-{suffix}.png"))).unwrap();
        sample(h.state_mut());
        for step in 1..=9 {
            h.state_mut().project.as_mut().unwrap().current_step = step;
            if step == 8 {
                h.state_mut().net_ui.selected = Some("s3".into());
            }
            h.run_steps(3);
            h.render().unwrap().save(out.join(format!("step{step}-{suffix}.png"))).unwrap();
        }
        // Vederea 3D (fără teren: proiectul de test nu are hartă ancorată)
        {
            let st = h.state_mut();
            st.project.as_mut().unwrap().current_step = 8;
            st.net_ui.show_3d = true;
        }
        h.run_steps(3);
        h.render().unwrap().save(out.join(format!("step8-3d-{suffix}.png"))).unwrap();
        h.state_mut().net_ui.show_3d = false;
        // Ecran complet: fără bara laterală și pași
        h.state_mut().net_ui.expanded = true;
        h.run_steps(3);
        h.render().unwrap().save(out.join(format!("step8-fullscreen-{suffix}.png"))).unwrap();
        h.state_mut().net_ui.expanded = false;
        h.state_mut().close_project_for_test();
        h.run_steps(3);
        h.render().unwrap().save(out.join(format!("home-list-{suffix}.png"))).unwrap();
    }
    println!("snapshots: {}", out.canonicalize().unwrap().display());
    let _ = std::fs::remove_dir_all(dir);
}

/// Exportul JSON păstrează tot proiectul: schema desenată, setările canvasului, tronsoanele.
#[test]
fn json_export_roundtrip_keeps_schema() {
    let dir = std::env::temp_dir().join(format!("hidrocalc-json-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("j.db");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1360.0, 880.0))
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
    sample(h.state_mut());
    let p = h.state().project.clone().unwrap();
    assert!(!p.network_schema.is_empty(), "proiectul de test trebuie să aibă schemă");
    let text = serde_json::to_string_pretty(&p.to_export_json("0.1.0", "2026-11-10T10:00:00Z")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let back = ProjectData::from_export_value(value).unwrap();
    assert_eq!(back, p, "exportul JSON a pierdut date");
    let _ = std::fs::remove_dir_all(dir);
}

/// Modelul 3D al proiectului de test se exportă în toate cele trei formate.
#[test]
fn model_3d_exports_for_sample_project() {
    use crate::export3d::ModelFormat;
    let dir = std::env::temp_dir().join(format!("hidrocalc-3d-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("m.db");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1360.0, 880.0))
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
    sample(h.state_mut());
    h.state_mut().project.as_mut().unwrap().current_step = 8;
    h.run_steps(2);
    let p = h.state().project.as_ref().unwrap();
    let glb = crate::view3d::View3D::model_bytes(p, ModelFormat::Glb).unwrap();
    let stl = crate::view3d::View3D::model_bytes(p, ModelFormat::Stl).unwrap();
    let dxf = String::from_utf8(crate::view3d::View3D::model_bytes(p, ModelFormat::Dxf).unwrap()).unwrap();
    assert_eq!(&glb[0..4], b"glTF");
    let n = u32::from_le_bytes(stl[80..84].try_into().unwrap()) as usize;
    assert!(n > 100, "prea puține triunghiuri: {n}");
    assert_eq!(stl.len(), 84 + 50 * n);
    assert_eq!(dxf.matches("3DFACE").count(), n, "câte o față DXF pe triunghi");
    if let Ok(out) = std::env::var("HIDRO_3D_OUT") {
        std::fs::write(format!("{out}/model.glb"), &glb).unwrap();
        std::fs::write(format!("{out}/model.stl"), &stl).unwrap();
        std::fs::write(format!("{out}/model.dxf"), dxf).unwrap();
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// Panoul de export de la pasul 9, într-o fereastră înaltă (verificare vizuală).
#[test]
fn export_panel_snapshot() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
    std::fs::create_dir_all(&out).unwrap();
    for dark in [false, true] {
        let dir = std::env::temp_dir().join(format!("hidrocalc-expp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("x.db");
        let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 1900.0))
            .with_theme(theme)
            .wgpu()
            .build_eframe(|cc| HidroApp::with_db(cc, Settings { dark, ..Default::default() }, Db::open(&db_path)));
        sample(h.state_mut());
        h.state_mut().project.as_mut().unwrap().current_step = 9;
        h.state_mut().net_ui.export_sel.insert(ExportId::Json);
        h.state_mut().net_ui.export_sel.insert(ExportId::Glb);
        h.run_steps(4);
        h.render().unwrap().save(out.join(format!("export-panel-{}.png", if dark { "dark" } else { "light" }))).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Bara laterală cu paleta de desen și panoul hărții deschis (verificare vizuală, fără internet).
#[test]
fn step8_sidebar_with_map_snapshot() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
    std::fs::create_dir_all(&out).unwrap();
    for dark in [false, true] {
        let dir = std::env::temp_dir().join(format!("hidrocalc-sb-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("s.db");
        let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 1800.0))
            .with_theme(theme)
            .wgpu()
            .build_eframe(|cc| HidroApp::with_db(cc, Settings { dark, ..Default::default() }, Db::open(&db_path)));
        sample(h.state_mut());
        {
            let p = h.state_mut().project.as_mut().unwrap();
            let a = hidro_core::geo::MapAnchor { lat: 45.7537, lng: 21.2257, world_x: 0.0, world_y: 0.0, tile_zoom: hidro_core::geo::compute_tile_zoom(45.7537, 1000.0) };
            p.network_canvas_settings.map_anchor = Some(a);
            p.network_canvas_settings.set("autoTerrainCota", serde_json::json!(false));
            p.current_step = 8;
        }
        h.run_steps(3);
        h.state_mut().net_ui.canvas.map.open = true;
        h.run_steps(4);
        h.render().unwrap().save(out.join(format!("step8-sidebar-map-{}.png", if dark { "dark" } else { "light" }))).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Pasul 9 fără tronsoane (ce se poate face aici, blocat) și bara laterală ascunsă.
#[test]
fn step9_empty_and_collapsed_sidebar_snapshots() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
    std::fs::create_dir_all(&out).unwrap();
    let dir = std::env::temp_dir().join(format!("hidrocalc-e9-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("e.db");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1360.0, 1500.0))
        .with_theme(egui::Theme::Light)
        .wgpu()
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
    h.state_mut().create_project();
    h.state_mut().project.as_mut().unwrap().current_step = 9;
    h.run_steps(3);
    h.render().unwrap().save(out.join("step9-empty-light.png")).unwrap();
    h.state_mut().settings.sidebar_closed = true;
    h.run_steps(3);
    h.render().unwrap().save(out.join("step9-empty-collapsed.png")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

/// Scurtături și navigare cu tastatura: pași, bara laterală, ajutor, butoane desenate manual, vederea 3D.
#[test]
fn keyboard_navigation_works() {
    use egui_kittest::kittest::Queryable;
    let dir = std::env::temp_dir().join(format!("hidrocalc-kb-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("k.db");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1360.0, 900.0))
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
    sample(h.state_mut());
    h.state_mut().project.as_mut().unwrap().current_step = 3;
    h.run_steps(2);
    let step = |h: &egui_kittest::Harness<'_, HidroApp>| h.state().project.as_ref().unwrap().current_step;

    // Pași: Cmd+] / Cmd+[ și Cmd+număr
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::CloseBracket);
    h.run_steps(2);
    assert_eq!(step(&h), 4);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::OpenBracket);
    h.run_steps(2);
    assert_eq!(step(&h), 3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num8);
    h.run_steps(2);
    assert_eq!(step(&h), 8);

    // Bara laterală și ajutor
    assert!(!h.state().settings.sidebar_closed);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
    h.run_steps(2);
    assert!(h.state().settings.sidebar_closed);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
    h.run_steps(2);
    assert!(!h.state().settings.sidebar_closed);
    h.key_press(egui::Key::F1);
    h.run_steps(2);
    assert!(h.state().show_help);

    // Buton desenat manual (paleta): focus + Enter alege instrumentul
    h.key_press(egui::Key::F1);
    h.run_steps(2);
    assert!(!h.state().show_help);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Tronson").focus();
    h.run_steps(1);
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(h.state().net_ui.canvas.tool, crate::canvas::Tool::Tronson);

    // Vedere 3D: săgeată dreapta rotește
    h.state_mut().net_ui.show_3d = true;
    h.run_steps(3);
    let before = h.state().net_ui.view3d.yaw_for_test();
    let canvas_label = h.state().t.t("view3d.a11yLabel");
    h.get_by_role_and_label(egui::accesskit::Role::Canvas, canvas_label).focus();
    h.run_steps(1);
    h.key_press(egui::Key::ArrowRight);
    h.run_steps(2);
    assert_ne!(h.state().net_ui.view3d.yaw_for_test(), before, "săgeata trebuie să rotească vederea");
    let _ = std::fs::remove_dir_all(dir);
}

/// Erorile rămân până sunt închise; mesajele obișnuite dispar; contrastul ridicat schimbă paleta.
#[test]
fn toasts_persist_and_high_contrast_applies() {
    let dir = std::env::temp_dir().join(format!("hidrocalc-ac-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("a.db");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1360.0, 900.0))
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
    sample(h.state_mut());
    h.state_mut().toast("eroare de test", true);
    h.state_mut().toast("salvat", false);
    for t in &mut h.state_mut().toasts {
        t.since = Instant::now() - Duration::from_secs(60);
    }
    h.run_steps(2);
    let left: Vec<String> = h.state().toasts.iter().map(|t| t.text.clone()).collect();
    assert_eq!(left, ["eroare de test"], "doar eroarea trebuie să rămână");

    let ctx = h.ctx.clone();
    let before = theme::palette(&ctx).ink;
    let mut s = h.state().settings.clone();
    s.high_contrast = true;
    h.state_mut().set_settings(&ctx, s);
    h.run_steps(2);
    let after = theme::palette(&ctx).ink;
    assert_ne!(before, after, "paleta cu contrast ridicat trebuie să difere");
    let _ = std::fs::remove_dir_all(dir);
}

/// Ștergerea unui tronson din tabel cere confirmare (al doilea click).
#[test]
fn deleting_a_section_needs_confirmation() {
    use egui_kittest::kittest::Queryable;
    let dir = std::env::temp_dir().join(format!("hidrocalc-dc-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("d.db");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(2400.0, 1300.0)) // tabelul e lat: butonul de ștergere trebuie să încapă în fereastră
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
    sample(h.state_mut());
    h.state_mut().project.as_mut().unwrap().current_step = 8;
    h.run_steps(3);
    let n = |h: &egui_kittest::Harness<'_, HidroApp>| h.state().project.as_ref().unwrap().network_sections.len();
    let before = n(&h);
    assert!(before >= 2);
    let tip = h.state().t.t("common.delete");
    h.get_all_by_label(tip).next().expect("buton de ștergere").click();
    h.run_steps(2);
    assert_eq!(n(&h), before, "primul click doar cere confirmare");
    let confirm = h.state().t.t("home.deleteConfirm");
    h.get_all_by_label(confirm).next().expect("buton de confirmare").click();
    h.run_steps(2);
    assert_eq!(n(&h), before - 1, "al doilea click șterge tronsonul");
    let _ = std::fs::remove_dir_all(dir);
}

/// Fereastra de ajutor/accesibilitate și contrastul ridicat (verificare vizuală).
#[test]
fn help_window_and_high_contrast_snapshot() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
    std::fs::create_dir_all(&out).unwrap();
    for (dark, hc) in [(false, false), (true, true), (false, true)] {
        let dir = std::env::temp_dir().join(format!("hidrocalc-hw-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("h.db");
        let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .with_theme(theme)
            .wgpu()
            .build_eframe(|cc| HidroApp::with_db(cc, Settings { dark, high_contrast: hc, ..Default::default() }, Db::open(&db_path)));
        sample(h.state_mut());
        h.state_mut().project.as_mut().unwrap().current_step = 4;
        h.state_mut().show_help = true;
        h.run_steps(4);
        h.render().unwrap().save(out.join(format!("help-{}{}.png", if dark { "dark" } else { "light" }, if hc { "-hc" } else { "" }))).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Responsive minimal: conținutul umple spațiul (nu sare), iar sub 1100 px bara laterală se ascunde singură.
#[test]
fn layout_adapts_to_width_and_sidebar() {
    use egui_kittest::kittest::Queryable;
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
    std::fs::create_dir_all(&out).unwrap();
    let title_x = |h: &egui_kittest::Harness<'_, HidroApp>| h.get_by_label_contains("Debite ale necesarului").rect().min.x;
    let mk = |w: f32| {
        let dir = std::env::temp_dir().join(format!("hidrocalc-rw-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("r.db");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(w, 800.0))
            .with_theme(egui::Theme::Light)
            .wgpu()
            .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
        sample(h.state_mut());
        h.state_mut().project.as_mut().unwrap().current_step = 3;
        h.run_steps(3);
        (h, dir)
    };
    // 1360: bara deschisă, apoi ascunsă — conținutul pornește de la marginea disponibilă, fără salt în gol
    let (mut h, d1) = mk(1360.0);
    let open_x = title_x(&h);
    assert!((270.0..330.0).contains(&open_x), "cu bara deschisă conținutul începe lângă ea: {open_x}");
    h.render().unwrap().save(out.join("responsive-1360-open.png")).unwrap();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
    h.run_steps(3);
    let closed_x = title_x(&h);
    assert!(closed_x < 60.0, "cu bara ascunsă conținutul umple lățimea: {closed_x}");
    h.render().unwrap().save(out.join("responsive-1360-closed.png")).unwrap();
    // 1000 px: bara nu se mai ascunde singură; butonul/Cmd+B o ascund și o arată, la orice mărime
    let (mut h2, d2) = mk(1000.0);
    assert!(h2.state().sidebar_visible(), "bara laterală nu se ascunde automat");
    h2.render().unwrap().save(out.join("responsive-1000.png")).unwrap();
    h2.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
    h2.run_steps(3);
    assert!(!h2.state().sidebar_visible());
    h2.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
    h2.run_steps(3);
    assert!(h2.state().sidebar_visible());
    h2.state_mut().project.as_mut().unwrap().current_step = 2;
    h2.run_steps(3);
    h2.render().unwrap().save(out.join("responsive-step2-1000.png")).unwrap();
    // Interfață mărită (zoom 1.6 => ~625 pt lățime): butonul de ascundere merge în continuare
    let ctx = h2.ctx.clone();
    let mut s = h2.state().settings.clone();
    s.ui_scale_pct = 160;
    h2.state_mut().set_settings(&ctx, s);
    h2.run_steps(3);
    h2.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
    h2.run_steps(3);
    assert!(!h2.state().sidebar_visible(), "comutarea barei merge și când interfața e mare");
    h2.render().unwrap().save(out.join("responsive-1000-zoom160.png")).unwrap();
    for d in [d1, d2] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// Comutatorul de limbă (buton ca cel de temă) trece RO ↔ EN.
#[test]
fn language_switch_toggles_between_ro_and_en() {
    use egui_kittest::kittest::Queryable;
    let dir = std::env::temp_dir().join(format!("hidrocalc-ls-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("l.db");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1360.0, 900.0))
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
    sample(h.state_mut());
    h.run_steps(2);
    assert_eq!(h.state().settings.lang, Lang::Ro);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Switch to English").click();
    h.run_steps(2);
    assert_eq!(h.state().settings.lang, Lang::En);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Treci la română").click();
    h.run_steps(2);
    assert_eq!(h.state().settings.lang, Lang::Ro);
    let _ = std::fs::remove_dir_all(dir);
}

/// Desen și editare fără mouse în planul 2D: tronson din tastatură, selectare și mutare.
#[test]
fn canvas_keyboard_draw_select_and_move() {
    use egui_kittest::kittest::Queryable;
    let dir = std::env::temp_dir().join(format!("hidrocalc-kc-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("c.db");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1360.0, 900.0))
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
    sample(h.state_mut());
    h.state_mut().project.as_mut().unwrap().current_step = 8;
    h.run_steps(3);
    let count = |h: &egui_kittest::Harness<'_, HidroApp>| h.state().project.as_ref().unwrap().network_schema.len();
    let before = count(&h);
    let label = h.state().t.t("canvas.a11yLabel");
    h.get_by_role_and_label(egui::accesskit::Role::Canvas, label).focus();
    h.run_steps(1);

    // L = tronson; Enter = început; săgeți; Enter = sfârșit
    h.key_press(egui::Key::L);
    h.run_steps(1);
    assert_eq!(h.state().net_ui.canvas.tool, crate::canvas::Tool::Tronson);
    h.key_press(egui::Key::Enter);
    h.run_steps(1);
    for _ in 0..8 {
        h.key_press(egui::Key::ArrowRight);
        h.run_steps(1);
    }
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(count(&h), before + 1, "tronsonul făcut din tastatură trebuie adăugat");

    // V = Select; PageDown alege un element; săgeata dreapta îl mută
    h.key_press(egui::Key::V);
    h.run_steps(1);
    h.key_press(egui::Key::PageDown);
    h.run_steps(1);
    let sel = h.state().net_ui.canvas.selected.clone().expect("element ales");
    let x_of = |h: &egui_kittest::Harness<'_, HidroApp>| h.state().project.as_ref().unwrap().network_schema.iter().find(|e| e.id == sel).map(|e| e.x).unwrap();
    let x0 = x_of(&h);
    h.key_press(egui::Key::ArrowRight);
    h.run_steps(2);
    assert!((x_of(&h) - x0 - 10.0).abs() < 1e-6, "elementul trebuie să se mute cu 10");
    let _ = std::fs::remove_dir_all(dir);
}

/// Cum arată focusul de la tastatură pe butoane (verificare vizuală).
#[test]
fn focus_ring_snapshot() {
    use egui_kittest::kittest::Queryable;
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
    std::fs::create_dir_all(&out).unwrap();
    let dir = std::env::temp_dir().join(format!("hidrocalc-fr-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("f.db");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1360.0, 900.0))
        .with_theme(egui::Theme::Light)
        .wgpu()
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
    sample(h.state_mut());
    h.state_mut().project.as_mut().unwrap().current_step = 8;
    h.run_steps(3);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Hidrant").focus();
    h.run_steps(2);
    h.render().unwrap().save(out.join("focus-tool-button.png")).unwrap();
    h.get_by_role_and_label(egui::accesskit::Role::Button, "⌂  Acasă").focus();
    h.run_steps(2);
    h.render().unwrap().save(out.join("focus-std-button.png")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

/// Comutatoarele de temă/limbă din bara laterală, în ambele teme și după comutare (verificare vizuală).
#[test]
fn theme_toggle_snapshot() {
    use egui_kittest::kittest::Queryable;
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
    std::fs::create_dir_all(&out).unwrap();
    let dir = std::env::temp_dir().join(format!("hidrocalc-tt-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("t.db");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1360.0, 900.0))
        .with_theme(egui::Theme::Light)
        .wgpu()
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
    sample(h.state_mut());
    h.state_mut().project.as_mut().unwrap().current_step = 3;
    h.run_steps(3);
    h.render().unwrap().save(out.join("toggle-1-light.png")).unwrap();
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Comută la dark mode").click();
    h.run_steps(1);
    h.render().unwrap().save(out.join("toggle-2-after-click.png")).unwrap();
    h.run_steps(4);
    h.render().unwrap().save(out.join("toggle-3-dark.png")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

/// Coada de exporturi (fără dialoguri): fișierele ajung în folder, PNG-urile din captura pasului 8.
#[test]
fn export_job_writes_all_files_and_returns_to_step_9() {
    let dir = std::env::temp_dir().join(format!("hidrocalc-exp-{}", uuid::Uuid::new_v4()));
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let db_path = dir.join("e.db");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1360.0, 880.0))
        .wgpu()
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
    sample(h.state_mut());
    h.state_mut().project.as_mut().unwrap().current_step = 9;
    h.run_steps(2);
    let ids = [ExportId::Json, ExportId::Dxf2d, ExportId::Dxf3d, ExportId::Glb, ExportId::Stl, ExportId::DocMemoriu, ExportId::Png2d, ExportId::Png3d];
    h.state_mut().export_job = Some(ExportJob {
        folder: Some(out.clone()),
        queue: ids.into_iter().collect(),
        ok: Vec::new(),
        errs: Vec::new(),
        cap: None,
        restore: (9, false),
    });
    for _ in 0..400 {
        h.run_steps(1);
        std::thread::sleep(std::time::Duration::from_millis(15));
        if h.state().export_job.is_none() {
            break;
        }
    }
    assert!(h.state().export_job.is_none(), "exportul nu s-a terminat");
    assert_eq!(h.state().project.as_ref().unwrap().current_step, 9, "trebuie să revină la pasul 9");
    let mut names: Vec<String> = std::fs::read_dir(&out).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names.len(), 8, "{names:?}");
    for n in &names {
        let len = std::fs::metadata(out.join(n)).unwrap().len();
        assert!(len > 100, "{n} e prea mic ({len} B)");
    }
    let png = std::fs::read(out.join(names.iter().find(|n| n.contains("plan 2D") && n.ends_with(".png")).unwrap())).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    if let Ok(dest) = std::env::var("HIDRO_EXP_OUT") {
        for n in &names {
            std::fs::copy(out.join(n), std::path::Path::new(&dest).join(n)).unwrap();
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// Necesită internet (dale OSM reale): `cargo test -p hidrocalc map_snapshot -- --ignored`.
#[test]
#[ignore]
fn map_snapshot() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots");
    std::fs::create_dir_all(&out).unwrap();
    let dir = std::env::temp_dir().join(format!("hidrocalc-map-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("m.db");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1360.0, 880.0))
        .with_theme(egui::Theme::Light)
        .wgpu()
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
    sample(h.state_mut());
    {
        let p = h.state_mut().project.as_mut().unwrap();
        // Ancoră în Timișoara (Piața Victoriei), la capătul amonte al lui Tr1.
        let a = hidro_core::geo::MapAnchor {
            lat: 45.7537,
            lng: 21.2257,
            world_x: 0.0,
            world_y: 0.0,
            tile_zoom: hidro_core::geo::compute_tile_zoom(45.7537, 1000.0),
        };
        let o = &mut p.network_canvas_settings;
        o.map_anchor = Some(a);
        o.set("autoTerrainCota", serde_json::json!(false));
        p.current_step = 8;
    }
    for _ in 0..80 {
        h.run_steps(2);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    h.render().unwrap().save(out.join("step8-map.png")).unwrap();
    h.state_mut().net_ui.canvas.map.open = true;
    h.run_steps(4);
    h.render().unwrap().save(out.join("step8-map-panel.png")).unwrap();
    // 3D cu terenul DEM real
    h.state_mut().net_ui.show_3d = true;
    for _ in 0..100 {
        h.run_steps(2);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    h.render().unwrap().save(out.join("step8-3d-terrain.png")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

/// Fără bază de date: avertismentul apare în proiect (nu doar pe ecranul de start), iar o
/// salvare eșuată nu produce câte un mesaj de eroare la fiecare reîncercare.
#[test]
fn missing_database_warns_and_save_errors_do_not_pile_up() {
    use egui_kittest::kittest::Queryable;
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1360.0, 900.0))
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Err("disc indisponibil".into())));
    h.state_mut().create_project();
    h.state_mut().project.as_mut().unwrap().general.name = "Fără bază".into();
    h.run_steps(4);
    h.get_by_label_contains("NU se salvează");
    assert!(h.state().toasts.is_empty(), "fără bază de date nu se încearcă salvarea");

    // Bază de date care refuză scrierea: un singur mesaj, oricâte cadre trec.
    let dir = std::env::temp_dir().join(format!("hidrocalc-ro-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("r.db");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1360.0, 900.0))
        .build_eframe(|cc| HidroApp::with_db(cc, Settings::default(), Db::open(&db_path)));
    h.state_mut().create_project();
    h.state_mut().db.as_ref().unwrap().break_for_test();
    h.state_mut().project.as_mut().unwrap().general.name = "Modificat".into();
    for _ in 0..3 {
        std::thread::sleep(SAVE_DEBOUNCE + Duration::from_millis(30));
        h.run_steps(3);
    }
    assert_eq!(h.state().toasts.iter().filter(|t| t.error).count(), 1, "eroarea de salvare apare o singură dată");
    let _ = std::fs::remove_dir_all(dir);
}

//! Paleta de culori a aplicației (aceleași valori ca tema Tailwind din aplicația React).

use eframe::egui::{self, Color32, CornerRadius, Stroke, Visuals};

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub panel2: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub ink: Color32,
    pub ink2: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_fg: Color32,
    pub accent_soft: Color32,
    pub accent_ring: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
    pub warning: Color32,
    pub warning_soft: Color32,
    pub teal: Color32,
    pub cat_capture: Color32,
    pub cat_treat: Color32,
    pub cat_rezervor: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub const LIGHT: Palette = Palette {
    bg: hex(0xE8EEF5),
    panel: hex(0xF5F8FB),
    panel2: hex(0xDFE7F0),
    border: hex(0xCCD7E3),
    border_strong: hex(0xB0BFD0),
    ink: hex(0x0E1621),
    ink2: hex(0x243142),
    muted: hex(0x455366),
    accent: hex(0x2F5F93),
    accent_fg: hex(0xFFFFFF),
    accent_soft: hex(0xD6E3F1),
    accent_ring: hex(0x9DB6D1),
    danger: hex(0xA3433F),
    danger_soft: hex(0xEFDAD8),
    warning: hex(0x7D5D1E),
    warning_soft: hex(0xF1E8D4),
    teal: hex(0x2F5F93),
    cat_capture: hex(0x455366),
    cat_treat: hex(0x455366),
    cat_rezervor: hex(0x455366),
};

pub const DARK: Palette = Palette {
    bg: hex(0x12171E),
    panel: hex(0x1A212B),
    panel2: hex(0x232C38),
    border: hex(0x2D3745),
    border_strong: hex(0x3C4959),
    ink: hex(0xF1F4F8),
    ink2: hex(0xD5DCE6),
    muted: hex(0xA4AFBF),
    accent: hex(0x86AEDB),
    accent_fg: hex(0x0F1620),
    accent_soft: hex(0x22344A),
    accent_ring: hex(0x3D5B80),
    danger: hex(0xD98A86),
    danger_soft: hex(0x2E2121),
    warning: hex(0xCFAE72),
    warning_soft: hex(0x2B2619),
    teal: hex(0x86AEDB),
    cat_capture: hex(0xA4AFBF),
    cat_treat: hex(0xA4AFBF),
    cat_rezervor: hex(0xA4AFBF),
};

/// Varianta cu contrast ridicat (WCAG AAA ≥ 7:1 pentru text).
pub const LIGHT_HC: Palette = Palette {
    bg: hex(0xEEF2F7),
    panel: hex(0xFFFFFF),
    panel2: hex(0xE3EAF2),
    border: hex(0x6B7A8C),
    border_strong: hex(0x3F4D5E),
    ink: hex(0x05090F),
    ink2: hex(0x111A26),
    muted: hex(0x2E3B4B),
    accent: hex(0x0B3A73),
    accent_fg: hex(0xFFFFFF),
    accent_soft: hex(0xCFE0F5),
    accent_ring: hex(0x0B3A73),
    danger: hex(0x8A1F1C),
    danger_soft: hex(0xF7D9D8),
    warning: hex(0x6B4A00),
    warning_soft: hex(0xF6E8C4),
    teal: hex(0x0B3A73),
    cat_capture: hex(0x2E3B4B),
    cat_treat: hex(0x2E3B4B),
    cat_rezervor: hex(0x2E3B4B),
};

pub const DARK_HC: Palette = Palette {
    bg: hex(0x000000),
    panel: hex(0x0A0E14),
    panel2: hex(0x141B25),
    border: hex(0x8A99AD),
    border_strong: hex(0xB8C4D4),
    ink: hex(0xFFFFFF),
    ink2: hex(0xEEF2F7),
    muted: hex(0xC9D3E0),
    accent: hex(0xA8CCFF),
    accent_fg: hex(0x000000),
    accent_soft: hex(0x1B3354),
    accent_ring: hex(0xA8CCFF),
    danger: hex(0xFF9A94),
    danger_soft: hex(0x3A1210),
    warning: hex(0xFFD27A),
    warning_soft: hex(0x3A2A08),
    teal: hex(0xA8CCFF),
    cat_capture: hex(0xC9D3E0),
    cat_treat: hex(0xC9D3E0),
    cat_rezervor: hex(0xC9D3E0),
};

static HIGH_CONTRAST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn pal_for(dark: bool) -> &'static Palette {
    match (dark, HIGH_CONTRAST.load(std::sync::atomic::Ordering::Relaxed)) {
        (false, false) => &LIGHT,
        (true, false) => &DARK,
        (false, true) => &LIGHT_HC,
        (true, true) => &DARK_HC,
    }
}

pub fn palette(ctx: &egui::Context) -> &'static Palette {
    pal_for(ctx.global_style().visuals.dark_mode)
}

/// Pornește/oprește contrastul ridicat: schimbă paleta și refă stilurile ambelor teme.
pub fn set_high_contrast(ctx: &egui::Context, on: bool) {
    HIGH_CONTRAST.store(on, std::sync::atomic::Ordering::Relaxed);
    ctx.set_visuals_of(egui::Theme::Light, visuals(pal_for(false), false));
    ctx.set_visuals_of(egui::Theme::Dark, visuals(pal_for(true), true));
    ctx.request_repaint();
}

/// Animațiile (hover, deschideri) pot fi oprite pentru cine preferă mișcare redusă.
pub fn set_reduce_motion(ctx: &egui::Context, on: bool) {
    ctx.all_styles_mut(|s| s.animation_time = if on { 0.0 } else { 1.0 / 12.0 });
}

fn visuals(p: &Palette, dark: bool) -> Visuals {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.panel;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.extreme_bg_color = p.panel;
    v.text_edit_bg_color = Some(p.panel);
    v.faint_bg_color = p.panel2;
    v.code_bg_color = p.panel2;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.danger;
    v.override_text_color = Some(p.ink);
    v.selection.bg_fill = p.accent_ring;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.window_corner_radius = CornerRadius::same(10);
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(if dark { 90 } else { 30 }) };

    let r = CornerRadius::same(7);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.panel;
    w.noninteractive.weak_bg_fill = p.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.ink2);
    w.noninteractive.corner_radius = r;
    for (wv, fill, stroke) in
        [(&mut w.inactive, p.panel, p.border), (&mut w.hovered, p.accent_soft, p.border_strong), (&mut w.active, p.accent_soft, p.accent), (&mut w.open, p.accent_soft, p.border_strong)]
    {
        wv.bg_fill = fill;
        wv.weak_bg_fill = fill;
        wv.bg_stroke = Stroke::new(1.0, stroke);
        wv.fg_stroke = Stroke::new(1.0, p.ink);
        wv.corner_radius = r;
        wv.expansion = 0.0;
    }
    // Șina glisoarelor (și fundalul căsuțelor bifabile) trebuie să se vadă pe fundalul panoului.
    v.widgets.inactive.bg_fill = p.panel2;
    v.slider_trailing_fill = true;
    v
}

/// Roboto (text) și Roboto Mono (cifre/formule), cu DejaVu ca rezervă pentru
/// săgeți și simboluri (←, →, ✓, ✕, ⓘ, ▾ …) pe care Roboto nu le are.
fn install_fonts(ctx: &egui::Context) {
    use std::sync::Arc;
    let mut fonts = egui::FontDefinitions::default();
    for (name, data) in [
        ("Roboto", &include_bytes!("../../../assets/fonts/Roboto-Regular.ttf")[..]),
        ("RobotoMedium", &include_bytes!("../../../assets/fonts/Roboto-Medium.ttf")[..]),
        ("RobotoMono", &include_bytes!("../../../assets/fonts/RobotoMono-Regular.ttf")[..]),
        ("DejaVuSans", &include_bytes!("../../../assets/fonts/DejaVuSans.ttf")[..]),
        ("DejaVuSansMono", &include_bytes!("../../../assets/fonts/DejaVuSansMono.ttf")[..]),
    ] {
        fonts.font_data.insert(name.into(), Arc::new(egui::FontData::from_static(data)));
    }
    for (family, names) in [(egui::FontFamily::Proportional, ["Roboto", "DejaVuSans"]), (egui::FontFamily::Monospace, ["RobotoMono", "DejaVuSansMono"])] {
        let list = fonts.families.entry(family).or_default();
        list.retain(|n| n != "Ubuntu-Light" && n != "Hack");
        for (i, n) in names.iter().enumerate() {
            list.insert(i, (*n).into());
        }
    }
    fonts.families.insert(egui::FontFamily::Name("Medium".into()), vec!["RobotoMedium".into(), "DejaVuSans".into()]);
    ctx.set_fonts(fonts);
}

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_visuals_of(egui::Theme::Light, visuals(pal_for(false), false));
    ctx.set_visuals_of(egui::Theme::Dark, visuals(pal_for(true), true));
    ctx.all_styles_mut(|s| {
        s.interaction.tooltip_delay = 0.1;
        s.interaction.show_tooltips_only_when_still = false;
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.interact_size.y = 30.0;
        s.spacing.icon_width = 18.0;
        s.spacing.icon_spacing = 8.0;
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.5));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(14.5));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
        s.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.0));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
    });
}

#[cfg(test)]
mod contrast_tests {
    use super::*;

    fn lin(c: u8) -> f64 {
        let s = c as f64 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    fn lum(c: Color32) -> f64 {
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }
    /// Raportul de contrast WCAG între două culori.
    fn ratio(a: Color32, b: Color32) -> f64 {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn palettes_meet_wcag_contrast() {
        // (paletă, nume, minim pentru text)
        for (p, name, min) in [(&LIGHT, "light", 4.5), (&DARK, "dark", 4.5), (&LIGHT_HC, "light-hc", 7.0), (&DARK_HC, "dark-hc", 7.0)] {
            let pairs = [
                ("ink/panel", p.ink, p.panel),
                ("ink/bg", p.ink, p.bg),
                ("ink2/panel", p.ink2, p.panel),
                ("muted/panel", p.muted, p.panel),
                ("muted/bg", p.muted, p.bg),
                ("muted/panel2", p.muted, p.panel2),
                ("accent/panel", p.accent, p.panel),
                ("accent/bg", p.accent, p.bg),
                ("accent_fg/accent", p.accent_fg, p.accent),
                ("danger/panel", p.danger, p.panel),
                ("warning/panel", p.warning, p.panel),
                ("ink/accent_soft", p.ink, p.accent_soft),
            ];
            for (n, fg, bg) in pairs {
                let r = ratio(fg, bg);
                assert!(r >= min, "{name}: {n} are contrast {r:.2} (< {min})");
            }
        }
    }
}

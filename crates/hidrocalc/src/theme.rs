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
    bg: hex(0xF7F8FA),
    panel: hex(0xFFFFFF),
    panel2: hex(0xF2F4F7),
    border: hex(0xE1E4EA),
    border_strong: hex(0xC5CAD3),
    ink: hex(0x0F172A),
    ink2: hex(0x334155),
    muted: hex(0x64748B),
    accent: hex(0x1E3A5F),
    accent_fg: hex(0xFFFFFF),
    accent_soft: hex(0xE5ECF4),
    accent_ring: hex(0xA6BAD0),
    danger: hex(0x9B3D3A),
    danger_soft: hex(0xF0DCDB),
    warning: hex(0x8A6526),
    warning_soft: hex(0xFBF3E4),
    teal: hex(0x0F766E),
    cat_capture: hex(0x182F4D),
    cat_treat: hex(0x78A1CC),
    cat_rezervor: hex(0x64748B),
};

pub const DARK: Palette = Palette {
    bg: hex(0x0E131A),
    panel: hex(0x181F2A),
    panel2: hex(0x1F2735),
    border: hex(0x2A3445),
    border_strong: hex(0x3E4A5E),
    ink: hex(0xE5E9EF),
    ink2: hex(0xB1BAC8),
    muted: hex(0x7A8597),
    accent: hex(0x78A1CC),
    accent_fg: hex(0x0B121C),
    accent_soft: hex(0x1A2740),
    accent_ring: hex(0x3A567D),
    danger: hex(0xC97A77),
    danger_soft: hex(0x2A1B1B),
    warning: hex(0xC9A467),
    warning_soft: hex(0x2A2416),
    teal: hex(0x2DD4BF),
    cat_capture: hex(0xA6BAD0),
    cat_treat: hex(0x78A1CC),
    cat_rezervor: hex(0x94A3B8),
};

pub fn palette(ctx: &egui::Context) -> &'static Palette {
    if ctx.global_style().visuals.dark_mode {
        &DARK
    } else {
        &LIGHT
    }
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
    v.window_corner_radius = CornerRadius::same(12);

    let r = CornerRadius::same(7);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.panel;
    w.noninteractive.weak_bg_fill = p.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.ink2);
    w.noninteractive.corner_radius = r;
    for (wv, fill, stroke) in [
        (&mut w.inactive, p.panel2, p.border_strong),
        (&mut w.hovered, p.accent_soft, p.accent_ring),
        (&mut w.active, p.accent_soft, p.accent),
        (&mut w.open, p.accent_soft, p.accent_ring),
    ] {
        wv.bg_fill = fill;
        wv.weak_bg_fill = fill;
        wv.bg_stroke = Stroke::new(1.0, stroke);
        wv.fg_stroke = Stroke::new(1.0, p.ink);
        wv.corner_radius = r;
        wv.expansion = 0.0;
    }
    v
}

/// Fonturile implicite egui + DejaVu Sans ca rezervă pentru săgeți și simboluri
/// (←, →, ✓, ✕, ⓘ, ▾ …) pe care Ubuntu-Light nu le are.
fn install_fonts(ctx: &egui::Context) {
    use std::sync::Arc;
    let mut fonts = egui::FontDefinitions::default();
    for (name, data) in [
        ("DejaVuSans", &include_bytes!("../../../assets/fonts/DejaVuSans.ttf")[..]),
        ("DejaVuSansMono", &include_bytes!("../../../assets/fonts/DejaVuSansMono.ttf")[..]),
    ] {
        fonts.font_data.insert(name.into(), Arc::new(egui::FontData::from_static(data)));
    }
    for (family, name) in [(egui::FontFamily::Proportional, "DejaVuSans"), (egui::FontFamily::Monospace, "DejaVuSansMono")] {
        let list = fonts.families.entry(family).or_default();
        list.push(name.into());
        if !list.iter().any(|n| n == "DejaVuSans") {
            list.push("DejaVuSans".into());
        }
    }
    ctx.set_fonts(fonts);
}

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_visuals_of(egui::Theme::Light, visuals(&LIGHT, false));
    ctx.set_visuals_of(egui::Theme::Dark, visuals(&DARK, true));
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.interact_size.y = 28.0;
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(11.5));
        s.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.5));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
    });
}

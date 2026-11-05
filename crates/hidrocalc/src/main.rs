// Fără fereastră de consolă pe Windows în release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod charts;
mod db;
mod i18n;
mod map_ui;
mod network_ui;
mod steps;
mod terrain_sync;
mod theme;
mod tiles;
mod view3d;
mod widgets;

/// Iconița ferestrei (aceeași ca a aplicației Tauri).
fn window_icon() -> eframe::egui::IconData {
    let img = image::load_from_memory_with_format(include_bytes!("../../../assets/icons/icon.png"), image::ImageFormat::Png)
        .map(|i| i.to_rgba8())
        .unwrap_or_else(|_| image::RgbaImage::new(1, 1));
    eframe::egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("HidroCalc")
            .with_inner_size([1360.0, 880.0])
            .with_min_inner_size([960.0, 640.0])
            .with_icon(window_icon()),
        ..Default::default()
    };
    eframe::run_native("HidroCalc", options, Box::new(|cc| Ok(Box::new(app::HidroApp::new(cc)))))
}

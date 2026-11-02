//! FCDI Apps : lanceur des applications FCDI.

mod app;
mod catalog;
mod config;
mod i18n;
mod net;
mod system;

fn main() -> eframe::Result {
    let minimized = std::env::args().any(|a| a == "--minimized");
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../packaging/fcdi-apps.png")).ok();
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("FCDI Apps")
        .with_app_id("fcdi-apps")
        .with_inner_size([1180.0, 760.0])
        .with_min_inner_size([720.0, 420.0]);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("FCDI Apps", options, Box::new(move |cc| Ok(Box::new(app::Launcher::new(cc, minimized)))))
}

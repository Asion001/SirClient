//! SirClient desktop app.
//!
//! `sirclient --mock` starts with a simulated G7 Pro connected.
//! `sirclient --screenshot <dir>` saves a PNG of every page and exits.

// No console window for release builds on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod i18n;
mod pages;
mod theme;
mod widgets;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<String> = std::env::args().collect();
    let mock = args.iter().any(|a| a == "--mock" || a == "--demo");
    let shots = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);
    if let Some(d) = &shots {
        let _ = std::fs::create_dir_all(d);
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SirClient")
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "SirClient",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, mock, shots)))),
    )
}

//! MathPrime, grafische versie. Start met `mathprime [bestand.rkn]`.

// geen consolevenster op Windows in een release-build
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod keymap;
mod library;
mod paint;
mod icons;
mod instance;
mod recovery;
mod find;
mod ribbon;
mod settings;
mod tabs;
mod shot;
mod update;

fn main() -> eframe::Result {
    let file = std::env::args().nth(1).map(std::path::PathBuf::from);
    // al een venster open? Dan krijgt dat het bestand, en stopt deze start
    let inbox = match instance::start(file.as_deref()) {
        instance::Start::Handed => return Ok(()),
        instance::Start::Primary(inbox) => Some(inbox),
        instance::Start::Alone => None,
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1300.0, 850.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title("MathPrime")
            .with_icon(std::sync::Arc::new(eframe::egui::IconData {
                rgba: include_bytes!("../assets/mathprime-128.rgba").to_vec(),
                width: 128,
                height: 128,
            })),
        ..Default::default()
    };
    let result = eframe::run_native("MathPrime", options, Box::new(|cc| Ok(Box::new(app::App::new(cc, file, inbox)))));
    instance::stop();
    result
}

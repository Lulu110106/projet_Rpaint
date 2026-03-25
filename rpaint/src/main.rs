mod model;
mod logic;
mod ui_tools;
mod app;

use model::PaintApp;

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "RPaint Pro",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Box::new(PaintApp::default())),
    )
}
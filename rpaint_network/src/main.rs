mod events;
mod model;
mod logic;
mod ui_tools;
mod app;
mod server;
mod client;

use model::PaintApp;

#[tokio::main]
async fn main() -> eframe::Result<()> {
    eframe::run_native(
        "RPaint Pro",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Box::new(PaintApp::default())),
    )
}
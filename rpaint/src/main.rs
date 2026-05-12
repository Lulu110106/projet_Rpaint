mod events;
mod model;
mod logic;
mod ui_tools;
mod app;
mod server;
mod client;
mod layers;

use model::PaintApp;

// Point d'entrée de l'application desktop.
// eframe crée la fenêtre native et exécute ensuite PaintApp::update à chaque frame.
#[tokio::main]
async fn main() -> eframe::Result<()> {
    eframe::run_native(
        "RPaint Pro",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Box::new(PaintApp::default())),
    )
}
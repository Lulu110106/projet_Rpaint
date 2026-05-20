mod app;
mod client;
mod events;
mod layers;
mod logic;
mod model;
mod server;
mod ui_tools;

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

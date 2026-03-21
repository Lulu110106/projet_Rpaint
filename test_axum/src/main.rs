mod events;
mod server;
mod client;

use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("--host") => {
            let pseudo = args.get(2).map(|s| s.as_str()).unwrap_or("Host");
            server::run(pseudo).await;
        }
        Some("--join") => {
            let ip = args.get(2).map(|s| s.as_str()).unwrap_or("127.0.0.1");
            let pseudo = args.get(3).map(|s| s.as_str()).unwrap_or("Client");
            client::run(ip, pseudo).await;
        }
        _ => {
            eprintln!("Usage:");
            eprintln!("  cargo run -- --host <pseudo>");
            eprintln!("  cargo run -- --join <ip> <pseudo>");
            std::process::exit(1);
        }
    }
}
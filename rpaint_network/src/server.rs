use crate::events::{DrawLineEvent, NetworkEvent};
use crate::model::Line;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, oneshot};
use tokio::sync::mpsc;

struct AppState {
    tx: broadcast::Sender<String>,
}

static GLOBAL_TX: Mutex<Option<broadcast::Sender<String>>> = Mutex::new(None);
static GLOBAL_LOCAL_DRAW_TX: Mutex<Option<mpsc::UnboundedSender<Line>>> = Mutex::new(None);

pub fn set_local_draw_sink(tx: Option<mpsc::UnboundedSender<Line>>) {
    if let Ok(mut sink) = GLOBAL_LOCAL_DRAW_TX.lock() {
        *sink = tx;
    }
}

pub fn publish_draw_line(event: DrawLineEvent) -> bool {
    let payload = match serde_json::to_string(&NetworkEvent::DrawLine(event)) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let guard = GLOBAL_TX.lock();
    if let Ok(locked) = guard {
        if let Some(tx) = locked.as_ref() {
            return tx.send(payload).is_ok();
        }
    }
    false
}

pub async fn run(_pseudo: &str, shutdown: oneshot::Receiver<()>) {
    let (tx, _) = broadcast::channel::<String>(256);
    if let Ok(mut global) = GLOBAL_TX.lock() {
        *global = Some(tx.clone());
    }
    let state = Arc::new(AppState { tx: tx.clone() });
    
    print_local_ip();

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Serveur local démarré sur le port 3000");

    let server = axum::serve(listener, app).with_graceful_shutdown(async {
        let _ = shutdown.await;
        println!("Arrêt du serveur demandé...");
    });

    if let Err(err) = server.await {
        eprintln!("Erreur serveur: {err}");
    }
    if let Ok(mut global) = GLOBAL_TX.lock() {
        *global = None;
    }
    println!("Serveur arrêté.");
}

fn print_local_ip() {
    use std::net::UdpSocket;
    // Astuce : ouvrir un socket UDP sans envoyer de données
    // permet de connaître l'IP locale utilisée pour joindre internet
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("  IP locale : {}", addr.ip());
                println!("  Les clients du même réseau lancent :");
                println!("  cargo run -- --join {} <pseudo>", addr.ip());
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
            }
        }
    }
}


/* 
#[warn(unused)]
async fn start_tunnel() {
    let mut child = tokio::process::Command::new("ssh")
        .args([
            "-o", "StrictHostKeyChecking=no",
            "-o", "ServerAliveInterval=30",
            "-R", "80:localhost:3000",
            "localhost.run",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Impossible de lancer ssh (est-il installé ?)");

    // Lire stderr où localhost.run affiche l'URL
    if let Some(stderr) = child.stderr.take() {
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.contains("localhost.run") && line.contains("http") {
                // Extraire l'URL et la convertir en ws://
                if let Some(url) = extract_url(&line) {
                    let ws_url = url.replace("https://", "").replace("http://", "");
                    println!("┌─────────────────────────────────────────┐");
                    println!("│  Lien à partager :                      │");
                    println!("│  cargo run -- --join {}  │", ws_url);
                    println!("└─────────────────────────────────────────┘\n");
                }
            }
        }
    }

    child.wait().await.ok();
}
#[warn(unused)]
fn extract_url(line: &str) -> Option<String> {
    line.split_whitespace()
        .find(|s| s.starts_with("https://") || s.starts_with("http://"))
        .map(|s| s.trim_end_matches('.').to_string())
}
*/
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(Message::Text(text))) = receiver.next().await {
        if let Ok(NetworkEvent::DrawLine(draw)) = serde_json::from_str::<NetworkEvent>(&text) {
            if let Ok(sink) = GLOBAL_LOCAL_DRAW_TX.lock() {
                if let Some(tx) = sink.as_ref() {
                    let _ = tx.send(draw.to_line());
                }
            }
        }
        let _ = state.tx.send(text);
    }

    send_task.abort();
}
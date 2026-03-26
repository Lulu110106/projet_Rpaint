use crate::events::ChatMessage;
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
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;
use tokio::sync::broadcast;

struct AppState {
    tx: broadcast::Sender<String>,
}

pub async fn run(pseudo: &str) {
    let pseudo = pseudo.to_string();
    let (tx, _) = broadcast::channel::<String>(256);
    let state = Arc::new(AppState { tx: tx.clone() });
    
    print_local_ip();

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Serveur local démarré sur le port 3000");

    // Stdin async du host
    let tx_stdin = tx.clone();
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let mut lines = tokio::io::BufReader::new(stdin).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let content = line.trim().to_string();
            if content.is_empty() { continue; }
            let msg = ChatMessage { client_id: 0, pseudo: pseudo.clone(), content: content.clone() };
            let json = serde_json::to_string(&msg).unwrap();
            println!("[toi] {}", content);
            let _ = tx_stdin.send(json);
        }
    });

    axum::serve(listener, app).await.unwrap();
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

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let client_id = timestamp_id();
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
        if let Ok(mut msg) = serde_json::from_str::<ChatMessage>(&text) {
            msg.client_id = client_id;
            println!("[{}] {}", msg.pseudo, msg.content);
            let _ = state.tx.send(serde_json::to_string(&msg).unwrap());
        }
    }

    send_task.abort();
}

fn timestamp_id() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos()
}
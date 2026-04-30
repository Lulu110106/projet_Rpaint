use crate::events::NetworkEvent;
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

// État partagé du serveur websocket: un canal broadcast pour redistribuer les dessins.
struct AppState {
    tx: broadcast::Sender<String>,
}

// Références globales pour brancher l'UI locale au serveur sans passer par une architecture plus lourde.
static GLOBAL_TX: Mutex<Option<broadcast::Sender<String>>> = Mutex::new(None);
static GLOBAL_LOCAL_DRAW_TX: Mutex<Option<mpsc::UnboundedSender<NetworkEvent>>> = Mutex::new(None);

// Permet d'activer ou de désactiver l'envoi direct vers le canvas local.
pub fn set_local_draw_sink(tx: Option<mpsc::UnboundedSender<NetworkEvent>>) {
    if let Ok(mut sink) = GLOBAL_LOCAL_DRAW_TX.lock() {
        *sink = tx;
    }
}

// Publie n'importe quel NetworkEvent (DrawLine, DeleteLine...) vers tous les clients.
pub fn publish_network_event(event: NetworkEvent) -> bool {
    let payload = match serde_json::to_string(&event) {
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

// Lance le serveur websocket local sur le port 3000.
pub async fn run(_pseudo: &str, shutdown: oneshot::Receiver<()>) {
    let (tx, _) = broadcast::channel::<String>(256);
    if let Ok(mut global) = GLOBAL_TX.lock() {
        *global = Some(tx.clone());
    }
    let state = Arc::new(AppState { tx: tx.clone() });

    print_local_ip();

    // Route unique: /ws pour les clients de dessin.
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
    // Ouvrir un socket UDP sans envoyer de paquet force le système à révéler
    // l'interface réseau choisie pour sortir vers internet.
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


// Accepte une nouvelle connexion websocket et bascule le socket dans la boucle de traitement.
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

// Réplique les messages reçus à tous les clients et alimente aussi le canvas local si nécessaire.
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();

    // Un task séparé pousse les messages broadcast vers ce socket particulier.
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(Message::Text(text))) = receiver.next().await {
        // Si le client a envoyé un événement réseau (DrawLine / DeleteLine),
        // on le transmet d'abord au canvas local via le sink (pour que l'UI
        // applique la modification sans l'enregistrer dans undo), puis on
        // rebroadcast l'événement à tous les clients.
        if let Ok(ev) = serde_json::from_str::<NetworkEvent>(&text) {
            if let Ok(sink) = GLOBAL_LOCAL_DRAW_TX.lock() {
                if let Some(tx) = sink.as_ref() {
                    let _ = tx.send(ev.clone());
                }
            }
        }
        let _ = state.tx.send(text);
    }

    send_task.abort();
}
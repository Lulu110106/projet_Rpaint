use crate::events::NetworkEvent;
use crate::model::PaintProject;
use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use futures::{SinkExt, StreamExt};
use igd::PortMappingProtocol;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::sync::{broadcast, oneshot};

// État partagé du serveur websocket: un canal broadcast pour redistribuer les dessins.
struct AppState {
    tx: broadcast::Sender<String>,
    shutdown: broadcast::Sender<()>,
}

// Références globales pour brancher l'UI locale au serveur sans passer par une architecture plus lourde.
static GLOBAL_TX: Mutex<Option<broadcast::Sender<String>>> = Mutex::new(None);
static GLOBAL_LOCAL_DRAW_TX: Mutex<Option<mpsc::UnboundedSender<NetworkEvent>>> = Mutex::new(None);
static GLOBAL_PROJECT: Mutex<Option<PaintProject>> = Mutex::new(None);

// Permet d'activer ou de désactiver l'envoi direct vers le canvas local.
pub fn set_local_draw_sink(tx: Option<mpsc::UnboundedSender<NetworkEvent>>) {
    if let Ok(mut sink) = GLOBAL_LOCAL_DRAW_TX.lock() {
        *sink = tx;
    }
}

pub fn set_project_snapshot(project: PaintProject) {
    if let Ok(mut snapshot) = GLOBAL_PROJECT.lock() {
        *snapshot = Some(project);
    }
}

fn get_project_snapshot() -> Option<PaintProject> {
    GLOBAL_PROJECT
        .lock()
        .ok()
        .and_then(|snapshot| snapshot.as_ref().cloned())
}

// Publie n'importe quel NetworkEvent (DrawShape, DeleteShape...) vers tous les clients.
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
pub async fn run(_pseudo: &str, initial_project: PaintProject, shutdown: oneshot::Receiver<()>) {
    let (tx, _) = broadcast::channel::<String>(256);
    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    set_project_snapshot(initial_project);
    if let Ok(mut global) = GLOBAL_TX.lock() {
        *global = Some(tx.clone());
    }
    let state = Arc::new(AppState {
        tx: tx.clone(),
        shutdown: shutdown_tx.clone(),
    });

    // Route unique: /ws pour les clients de dessin.
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Serveur local démarré sur le port 3000");

    let shutdown_tx_for_graceful_shutdown = shutdown_tx.clone();
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let _ = shutdown.await;
        let _ = shutdown_tx_for_graceful_shutdown.send(());
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

pub fn local_endpoint(port: u16) -> Option<String> {
    detect_local_ipv4().map(|ip| format!("{}:{}", ip, port))
}

pub async fn enable_upnp_port_forward(local_port: u16) -> Result<(String, u16), String> {
    tokio::task::spawn_blocking(move || {
        let local_ip =
            detect_local_ipv4().ok_or_else(|| "Impossible de détecter l'IP locale".to_string())?;
        let gateway = igd::search_gateway(Default::default())
            .map_err(|e| format!("Gateway UPnP introuvable: {e}"))?;
        let local_addr = std::net::SocketAddrV4::new(local_ip, local_port);

        gateway
            .add_port(
                PortMappingProtocol::TCP,
                local_port,
                local_addr,
                0,
                "rpaint-upnp",
            )
            .map_err(|e| format!("Échec ouverture port UPnP: {e}"))?;

        let external_ip = gateway
            .get_external_ip()
            .map_err(|e| format!("Échec récupération IP publique: {e}"))?;

        Ok((external_ip.to_string(), local_port))
    })
    .await
    .map_err(|e| format!("Task UPnP interrompue: {e}"))?
}

pub async fn disable_upnp_port_forward(external_port: u16) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let gateway = igd::search_gateway(Default::default())
            .map_err(|e| format!("Gateway UPnP introuvable: {e}"))?;
        gateway
            .remove_port(PortMappingProtocol::TCP, external_port)
            .map_err(|e| format!("Échec fermeture port UPnP: {e}"))?;
        Ok(())
    })
    .await
    .map_err(|e| format!("Task UPnP interrompue: {e}"))?
}

// Accepte une nouvelle connexion websocket et bascule le socket dans la boucle de traitement.
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

// Réplique les messages reçus à tous les clients et alimente aussi le canvas local si nécessaire.
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();
    let mut shutdown_rx = state.shutdown.subscribe();

    if let Some(project) = get_project_snapshot() {
        if let Ok(payload) = serde_json::to_string(&NetworkEvent::SyncProject { project }) {
            let _ = sender.send(Message::Text(payload)).await;
        }
    }

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(msg) => {
                        if sender.send(Message::Text(msg)).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        // Si le client a envoyé un événement réseau (DrawShape / DeleteShape),
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
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            _ = shutdown_rx.recv() => {
                let _ = sender.send(Message::Close(None)).await;
                break;
            }
        }
    }
}

fn detect_local_ipv4() -> Option<std::net::Ipv4Addr> {
    use std::net::{IpAddr, UdpSocket};
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(v4) => Some(v4),
        IpAddr::V6(_) => None,
    }
}

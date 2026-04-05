use crate::events::{DrawLineEvent, NetworkEvent};
use crate::model::Line;
use futures::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub async fn run(
    host_ip: &str,
    _pseudo: &str,
    mut shutdown: oneshot::Receiver<()>,
    draw_tx: mpsc::UnboundedSender<Line>,
    mut outgoing_draw_rx: mpsc::UnboundedReceiver<DrawLineEvent>,
) {
    let client_id = timestamp_id();
    // Accepte "127.0.0.1" ou "abc123.localhost.run"
    let url = if host_ip.contains("localhost.run") {
        format!("wss://{}:443/ws", host_ip)  // TLS pour WAN
    } else {
        format!("ws://{}:3000/ws", host_ip)  // local sans TLS
    };
    let (ws_stream, _) = match connect_async(&url).await {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("Connexion échouée: {}", e);
            return;
        }
    };
    println!("Connecté au host.");

    let (mut write, mut read) = ws_stream.split();

    // Client GUI: on écoute le host jusqu'au signal leave/close.
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                println!("Déconnexion client demandée.");
                break;
            }
            outgoing = outgoing_draw_rx.recv() => {
                if let Some(mut draw) = outgoing {
                    draw.source_id = client_id;
                    if let Ok(payload) = serde_json::to_string(&NetworkEvent::DrawLine(draw)) {
                        if write.send(Message::Text(payload)).await.is_err() {
                            eprintln!("Erreur websocket client: envoi impossible.");
                            break;
                        }
                    }
                }
            }
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(raw))) => {
                        if let Ok(event) = serde_json::from_str::<NetworkEvent>(&raw) {
                            match event {
                                NetworkEvent::DrawLine(draw) => {
                                    if draw.source_id != client_id {
                                        let _ = draw_tx.send(draw.to_line());
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        eprintln!("Erreur websocket client: {}", e);
                        break;
                    }
                    None => {
                        println!("Host déconnecté.");
                        break;
                    }
                }
            }
        }
    }

    println!("Client arrêté.");
}

fn timestamp_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
}
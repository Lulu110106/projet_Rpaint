use crate::events::NetworkEvent;
use crate::model::timestamp_id;
use futures::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::Message};

// Boucle client websocket: envoie les traits dessinés localement et reçoit ceux du host.
pub async fn run(
    host_ip: &str,
    port: u16,
    _pseudo: &str,
    mut shutdown: oneshot::Receiver<()>,
    draw_tx: mpsc::UnboundedSender<NetworkEvent>,
    mut outgoing_draw_rx: mpsc::UnboundedReceiver<NetworkEvent>,
) {
    let client_id = timestamp_id();
    // Accepte un host brut + port, ou une URL ws:// / wss:// déjà complète.
    let url = if host_ip.starts_with("ws://") || host_ip.starts_with("wss://") {
        if host_ip.ends_with("/ws") { host_ip.to_string() } else { format!("{host_ip}/ws") }
    } else {
        format!("ws://{host_ip}:{port}/ws")
    };
    let (ws_stream, _) = match connect_async(&url).await {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("Connexion échouée: {}", e);
            let _ = draw_tx.send(NetworkEvent::SessionStatus { message: format!("Connexion impossible: {e}") });
            return;
        }
    };
    println!("Connecté au host.");

    let (mut write, mut read) = ws_stream.split();

    // Le client reste dans cette boucle jusqu'au leave ou à la fermeture du lien.
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                println!("Déconnexion client demandée.");
                break;
            }
            outgoing = outgoing_draw_rx.recv() => {
                // Un événement réseau (DrawShape / DeleteShape) local est envoyé au host.
                if let Some(mut ev) = outgoing {
                    match &mut ev {
                        NetworkEvent::DrawShape(d) => { d.source_id = client_id; }
                        NetworkEvent::DeleteShape(_) => {}
                        // Les événements de layers sont transmis tels quels
                        _ => {}
                    }
                    if let Ok(payload) = serde_json::to_string(&ev) {
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
                        // On ne traite que les événements de dessin sérialisés en JSON.
                        if let Ok(event) = serde_json::from_str::<NetworkEvent>(&raw) {
                            // Ignorer l'écho de nos propres DrawShape pour éviter les doublons locaux.
                            let is_own_draw = matches!(&event, NetworkEvent::DrawShape(d) if d.source_id == client_id);
                            if !is_own_draw {
                                let _ = draw_tx.send(event);
                            }
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        eprintln!("Erreur websocket client: {}", e);
                        let _ = draw_tx.send(NetworkEvent::SessionStatus { message: "Le host s'est déconnecté".to_string() });
                        break;
                    }
                    None => {
                        println!("Host déconnecté.");
                        let _ = draw_tx.send(NetworkEvent::SessionStatus { message: "Le host s'est déconnecté".to_string() });
                        break;
                    }
                }
            }
        }
    }

    println!("Client arrêté.");
}

// timestamp_id imported from model
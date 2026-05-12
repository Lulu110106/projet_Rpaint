use crate::events::NetworkEvent;
use crate::model::timestamp_id;
use futures::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::Message};

// Boucle client websocket: envoie les traits dessinés localement et reçoit ceux du host.
pub async fn run(
    host_ip: &str,
    _pseudo: &str,
    mut shutdown: oneshot::Receiver<()>,
    draw_tx: mpsc::UnboundedSender<NetworkEvent>,
    mut outgoing_draw_rx: mpsc::UnboundedReceiver<NetworkEvent>,
) {
    let client_id = timestamp_id();
    // On adapte l'URL selon un usage local ou via un tunnel public.
    let url = if host_ip.contains("localhost.run") {
        // Quand le host est exposé sur Internet, on passe en WSS sur le port 443.
        format!("wss://{}:443/ws", host_ip)
    } else {
        // En local, le serveur tourne en WS simple sur 3000.
        format!("ws://{}:3000/ws", host_ip)
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

// timestamp_id imported from model
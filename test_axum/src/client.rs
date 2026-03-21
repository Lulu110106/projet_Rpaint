use crate::events::ChatMessage;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub async fn run(host_ip: &str, pseudo: &str) {
    let pseudo = pseudo.to_string();
    // Accepte "127.0.0.1" ou "abc123.localhost.run"
    let url = if host_ip.contains("localhost.run") {
        format!("wss://{}:443/ws", host_ip)  // TLS pour WAN
    } else {
        format!("ws://{}:3000/ws", host_ip)  // local sans TLS
    };

    let (ws_stream, _) = connect_async(&url)
        .await
        .unwrap_or_else(|e| panic!("Connexion échouée : {}", e));

    println!("Connecté ! Tape tes messages :\n");

    let (mut write, mut read) = ws_stream.split();

    // Recevoir les messages
    let me = pseudo.clone();
    tokio::spawn(async move {
        while let Some(Ok(Message::Text(msg))) = read.next().await {
            if let Ok(m) = serde_json::from_str::<ChatMessage>(&msg) {
                if m.pseudo != me {
                    println!("[{}] {}", m.pseudo, m.content);
                }
                
            }
        }
        println!("Déconnecté du host.");
    });

    // Lire stdin et envoyer
    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let content = input.trim().to_string();
        if content.is_empty() { continue; }
        println!("[toi] {}", content);
        let msg = ChatMessage { client_id: 0, pseudo: pseudo.clone(), content };
        let json = serde_json::to_string(&msg).unwrap();
        if write.send(Message::Text(json)).await.is_err() {
            eprintln!("Erreur envoi");
            break;
        }
    }
}
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DrawingMessage {
    DrawLine {
        points: Vec<(f32, f32)>,
        color: u32,
        width: f32,
    },
    Delete {
        indices: Vec<usize>,
    },
    Modify {
        indices: Vec<usize>,
        colors: Vec<u32>,
        widths: Vec<f32>,
    },
    Move {
        indices: Vec<usize>,
        delta_x: f32,
        delta_y: f32,
    },
    Clear,
    Sync {
        lines_data: String,
    },
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum NetworkEvent {
    PeerDiscovered(String),
    PeerExpired(String),
    MessageReceived(DrawingMessage),
    Connected,
    Disconnected,
}

#[derive(Clone)]
pub struct NetworkManager {
    connected: bool,
    peers: Vec<String>,
}

impl NetworkManager {
    pub fn new() -> Self {
        Self {
            connected: false,
            peers: Vec::new(),
        }
    }

    pub fn connect(&mut self) -> Result<(), String> {
        self.connected = true;
        println!("[Network] Connexion établie");
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
        self.peers.clear();
        println!("[Network] Déconnexion");
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn broadcast_message(&self, message: DrawingMessage) -> Result<(), String> {
        if !self.connected {
            return Err("Non connecté au réseau".to_string());
        }
        
        if let Ok(json) = serde_json::to_string(&message) {
            println!("[Network] Message broadcast: {}", json);
        }
        Ok(())
    }
}


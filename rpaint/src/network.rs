use serde::{Deserialize, Serialize};
use std::net::{UdpSocket, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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

const MULTICAST_ADDR: &str = "239.255.77.77";
const MULTICAST_PORT: u16 = 7878;

pub struct NetworkManager {
    connected: bool,
    peer_count: Arc<Mutex<usize>>,
    sender: Option<Arc<UdpSocket>>,
    events: Arc<Mutex<Vec<NetworkEvent>>>,
}

impl Clone for NetworkManager {
    fn clone(&self) -> Self {
        Self {
            connected: self.connected,
            peer_count: Arc::clone(&self.peer_count),
            sender: self.sender.clone(),
            events: Arc::clone(&self.events),
        }
    }
}

impl NetworkManager {
    pub fn new() -> Self {
        Self {
            connected: false,
            peer_count: Arc::new(Mutex::new(0)),
            sender: None,
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn connect(&mut self) -> Result<(), String> {
        if self.connected {
            return Ok(());
        }

        // Créer le socket sender
        let sender = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("Failed to bind sender: {}", e))?;
        
        sender.set_nonblocking(true)
            .map_err(|e| format!("Failed to set nonblocking: {}", e))?;

        // Créer le socket receiver
        let receiver = UdpSocket::bind(format!("0.0.0.0:{}", MULTICAST_PORT))
            .map_err(|e| format!("Failed to bind receiver: {}", e))?;
        
        receiver.join_multicast_v4(
            &MULTICAST_ADDR.parse::<Ipv4Addr>().unwrap(),
            &Ipv4Addr::UNSPECIFIED
        ).map_err(|e| format!("Failed to join multicast: {}", e))?;

        receiver.set_nonblocking(true)
            .map_err(|e| format!("Failed to set receiver nonblocking: {}", e))?;

        self.sender = Some(Arc::new(sender));
        self.connected = true;

        // Thread pour recevoir les messages
        let events = Arc::clone(&self.events);
        let peer_count = Arc::clone(&self.peer_count);
        
        thread::spawn(move || {
            let mut buf = [0u8; 65536];
            let mut last_peer_check = std::time::Instant::now();
            
            println!("[Network] Listening on multicast {}:{}", MULTICAST_ADDR, MULTICAST_PORT);
            
            loop {
                // Recevoir les messages
                match receiver.recv_from(&mut buf) {
                    Ok((len, addr)) => {
                        if let Ok(msg) = serde_json::from_slice::<DrawingMessage>(&buf[..len]) {
                            println!("[Network] Received message from {}: {:?}", addr, msg);
                            if let Ok(mut ev) = events.lock() {
                                ev.push(NetworkEvent::MessageReceived(msg));
                            }
                            
                            // Incrémenter le compteur de pairs (simulation)
                            if last_peer_check.elapsed() > Duration::from_secs(1) {
                                if let Ok(mut count) = peer_count.lock() {
                                    *count = 1; // Au moins 1 pair si on reçoit des messages
                                }
                                last_peer_check = std::time::Instant::now();
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        eprintln!("[Network] Receive error: {}", e);
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        });

        println!("[Network] P2P connection established via UDP multicast");
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
        if let Ok(mut count) = self.peer_count.lock() {
            *count = 0;
        }
        self.sender = None;
        println!("[Network] Disconnected");
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn peer_count(&self) -> usize {
        self.peer_count.lock().map(|c| *c).unwrap_or(0)
    }

    pub fn broadcast_message(&self, message: DrawingMessage) -> Result<(), String> {
        if !self.connected {
            return Err("Not connected to network".to_string());
        }

        if let Some(sender) = &self.sender {
            let json = serde_json::to_string(&message)
                .map_err(|e| format!("Failed to serialize: {}", e))?;
            
            let addr = format!("{}:{}", MULTICAST_ADDR, MULTICAST_PORT);
            sender.send_to(json.as_bytes(), addr)
                .map_err(|e| format!("Failed to send: {}", e))?;
            
            println!("[Network] Broadcast message: {}", json);
        }
        
        Ok(())
    }

    pub fn poll_events(&mut self) -> Vec<NetworkEvent> {
        if let Ok(mut events) = self.events.lock() {
            let result = events.drain(..).collect();
            result
        } else {
            Vec::new()
        }
    }
}

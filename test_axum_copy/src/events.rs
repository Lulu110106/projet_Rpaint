use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChatMessage {
    pub client_id: u32,
    pub pseudo: String,
    pub content: String,
}
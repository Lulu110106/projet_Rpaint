use eframe::egui::{ Color32, Pos2, Rect, Vec2};
use crate::events::NetworkEvent;
use tokio::task::JoinHandle;
use tokio::sync::{mpsc, oneshot};
use crate::layers::{Layer, LayerManager};

// Mode d'outil actif dans l'interface.
#[derive(Clone, PartialEq)]
pub enum BrushMode { Freehand, StraightLine, Eraser, Select }

// Une forme dessinée (actuellement seulement les lignes, extensible pour rectangles, cercles, etc.)
#[derive(Clone)]
pub enum Shape {
    Line { id: u64, points: Vec<Pos2>, color: Color32, width: f32 },
}

impl Shape {
    /// Récupère l'identifiant unique de la forme.
    pub fn id(&self) -> u64 {
        match self {
            Shape::Line { id, .. } => *id,
        }
    }

    /// Récupère les points mutables (pour le déplacement).
    pub fn points_mut(&mut self) -> &mut Vec<Pos2> {
        match self {
            Shape::Line { points, .. } => points,
        }
    }

    /// Récupère les points (read-only).
    pub fn points(&self) -> &Vec<Pos2> {
        match self {
            Shape::Line { points, .. } => points,
        }
    }

    /// Récupère la couleur.
    pub fn color(&self) -> Color32 {
        match self {
            Shape::Line { color, .. } => *color,
        }
    }

    /// Définit la couleur.
    pub fn set_color(&mut self, new_color: Color32) {
        match self {
            Shape::Line { color, .. } => *color = new_color,
        }
    }

    /// Récupère l'épaisseur.
    pub fn width(&self) -> f32 {
        match self {
            Shape::Line { width, .. } => *width,
        }
    }

    /// Définit l'épaisseur.
    pub fn set_width(&mut self, new_width: f32) {
        match self {
            Shape::Line { width, .. } => *width = new_width,
        }
    }
}



// Chaque action modifie l'historique de dessin et peut être rejouée via undo/redo.
#[derive(Clone)]
pub enum PaintAction {
    Create(Vec<Shape>),
    Delete(Vec<usize>, Vec<Shape>),
    Modify(Vec<usize>, Vec<Shape>, Vec<Shape>),
    Move(Vec<usize>, Vec2),
    // Actions pour les layers
    CreateLayer { id: u64, name: String, old_active: u64 },
    DeleteLayer { id: u64, layer: Layer, position: usize },
    RenameLayer { id: u64, old_name: String, new_name: String },
    SetLayerVisibility { id: u64, visible: bool },
    ReorderLayers { from_idx: usize, to_idx: usize },
    SetActiveLayer { old_id: u64, new_id: u64 },
    // Actions réseau (ne se propagent pas)
    NetworkCreateLayer { id: u64, name: String, position: usize },
    NetworkDeleteLayer { id: u64 },
    NetworkRenameLayer { id: u64, name: String },
    NetworkSetLayerVisibility { id: u64, visible: bool },
    NetworkSetActiveLayer { id: u64 },
    NetworkReorderLayers { from_idx: usize, to_idx: usize },
}

// État global de l'application: dessin, sélection, presse-papiers, et réseau.
pub struct PaintApp {
    pub layer_manager: LayerManager,
    pub undo_stack: Vec<PaintAction>,
    pub redo_stack: Vec<PaintAction>,
    pub mode: BrushMode,
    pub brush_color: Color32,
    pub brush_size: f32,
    pub current_line: Vec<Pos2>,
    pub active_stroke_id: Option<u64>,
    pub selected_indices: Vec<usize>,
    pub selection_start_pos: Option<Pos2>,
    pub selection_rect: Option<Rect>,
    pub clipboard: Vec<Shape>,
    pub is_dragging_items: bool,
    pub drag_accumulated_delta: Vec2,
    pub custom_palette: Vec<Color32>,
    pub server_running: bool,
    pub host_name_input: String,
    pub server_shutdown_tx: Option<oneshot::Sender<()>>,
    pub server_task: Option<JoinHandle<()>>,
    pub join_host_input: String,
    pub join_pseudo_input: String,
    pub client_task: Option<JoinHandle<()>>,
    pub client_shutdown_tx: Option<oneshot::Sender<()>>,
    pub multi_host_mode: bool,
    pub incoming_draw_tx: mpsc::UnboundedSender<NetworkEvent>,
    pub incoming_draw_rx: mpsc::UnboundedReceiver<NetworkEvent>,
    pub outgoing_draw_tx: Option<mpsc::UnboundedSender<NetworkEvent>>,
    // State pour le panneau des layers
    pub layers_panel_rename_id: Option<u64>,
    pub layers_panel_rename_text: String,
    pub layers_drag_source: Option<usize>,
}

impl Default for PaintApp {
    fn default() -> Self {
        // Le canal interne sert à faire remonter les événements réseau
        // vers la boucle d'affichage d'egui.
        let (incoming_draw_tx, incoming_draw_rx) = mpsc::unbounded_channel();
        Self {
            layer_manager: LayerManager::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            mode: BrushMode::Freehand,
            brush_color: Color32::from_rgb(0, 120, 255),
            brush_size: 4.0,
            current_line: Vec::new(),
            active_stroke_id: None,
            selected_indices: Vec::new(),
            selection_start_pos: None,
            selection_rect: None,
            clipboard: Vec::new(),
            is_dragging_items: false,
            drag_accumulated_delta: Vec2::ZERO,
            custom_palette: Vec::new(),
            server_running: false,
            host_name_input: "Test".to_string(),
            server_shutdown_tx: None,
            server_task: None,
            join_host_input: "127.0.0.1".to_string(),
            join_pseudo_input: "Guest".to_string(),
            client_task: None,
            client_shutdown_tx: None,
            multi_host_mode: true,
            incoming_draw_tx,
            incoming_draw_rx,
            outgoing_draw_tx: None,
            layers_panel_rename_id: None,
            layers_panel_rename_text: String::new(),
            layers_drag_source: None,
        }
    }
}

// Génère un identifiant simple basé sur le timestamp (en nanosecondes).
pub fn timestamp_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
}
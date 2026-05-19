use serde::{Deserialize, Serialize};
use eframe::egui::{ Color32, Pos2, Rect, Vec2};
use crate::events::NetworkEvent;
use tokio::task::JoinHandle;
use tokio::sync::{mpsc, oneshot};
use crate::layers::{Layer, LayerManager};

// Support de sérialisation pour les types egui non sérialisables.
#[derive(Serialize, Deserialize)]
pub(crate) struct SerializablePos2 {
    x: f32,
    y: f32,
}

impl From<Pos2> for SerializablePos2 {
    fn from(value: Pos2) -> Self {
        Self { x: value.x, y: value.y }
    }
}

impl From<SerializablePos2> for Pos2 {
    fn from(value: SerializablePos2) -> Self {
        Pos2::new(value.x, value.y)
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SerializableColor32 {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl From<Color32> for SerializableColor32 {
    fn from(value: Color32) -> Self {
        Self { r: value.r(), g: value.g(), b: value.b(), a: value.a() }
    }
}

impl From<SerializableColor32> for Color32 {
    fn from(value: SerializableColor32) -> Self {
        Color32::from_rgba_unmultiplied(value.r, value.g, value.b, value.a)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SerializableShape {
    Line { id: u64, points: Vec<SerializablePos2>, color: SerializableColor32, width: f32 },
    Rectangle { id: u64, start: SerializablePos2, end: SerializablePos2, color: SerializableColor32, width: f32 },
    Oval { id: u64, start: SerializablePos2, end: SerializablePos2, color: SerializableColor32, width: f32 },
    RegularPolygon { id: u64, start: SerializablePos2, end: SerializablePos2, sides: u8, color: SerializableColor32, width: f32 },
    Star { id: u64, start: SerializablePos2, end: SerializablePos2, points: u8, color: SerializableColor32, width: f32 },
    Arrow { id: u64, start: SerializablePos2, end: SerializablePos2, color: SerializableColor32, width: f32 },
}

impl From<&Shape> for SerializableShape {
    fn from(shape: &Shape) -> Self {
        match shape {
            Shape::Line { id, points, color, width } => SerializableShape::Line {
                id: *id,
                points: points.iter().cloned().map(SerializablePos2::from).collect(),
                color: (*color).into(),
                width: *width,
            },
            Shape::Rectangle { id, start, end, color, width } => SerializableShape::Rectangle {
                id: *id,
                start: (*start).into(),
                end: (*end).into(),
                color: (*color).into(),
                width: *width,
            },
            Shape::Oval { id, start, end, color, width } => SerializableShape::Oval {
                id: *id,
                start: (*start).into(),
                end: (*end).into(),
                color: (*color).into(),
                width: *width,
            },
            Shape::RegularPolygon { id, start, end, sides, color, width } => SerializableShape::RegularPolygon {
                id: *id,
                start: (*start).into(),
                end: (*end).into(),
                sides: *sides,
                color: (*color).into(),
                width: *width,
            },
            Shape::Star { id, start, end, points, color, width } => SerializableShape::Star {
                id: *id,
                start: (*start).into(),
                end: (*end).into(),
                points: *points,
                color: (*color).into(),
                width: *width,
            },
            Shape::Arrow { id, start, end, color, width } => SerializableShape::Arrow {
                id: *id,
                start: (*start).into(),
                end: (*end).into(),
                color: (*color).into(),
                width: *width,
            },
        }
    }
}

impl From<SerializableShape> for Shape {
    fn from(shape: SerializableShape) -> Self {
        match shape {
            SerializableShape::Line { id, points, color, width } => Shape::Line {
                id,
                points: points.into_iter().map(Pos2::from).collect(),
                color: color.into(),
                width,
            },
            SerializableShape::Rectangle { id, start, end, color, width } => Shape::Rectangle {
                id,
                start: start.into(),
                end: end.into(),
                color: color.into(),
                width,
            },
            SerializableShape::Oval { id, start, end, color, width } => Shape::Oval {
                id,
                start: start.into(),
                end: end.into(),
                color: color.into(),
                width,
            },
            SerializableShape::RegularPolygon { id, start, end, sides, color, width } => Shape::RegularPolygon {
                id,
                start: start.into(),
                end: end.into(),
                sides,
                color: color.into(),
                width,
            },
            SerializableShape::Star { id, start, end, points, color, width } => Shape::Star {
                id,
                start: start.into(),
                end: end.into(),
                points,
                color: color.into(),
                width,
            },
            SerializableShape::Arrow { id, start, end, color, width } => Shape::Arrow {
                id,
                start: start.into(),
                end: end.into(),
                color: color.into(),
                width,
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct SerializableLayer {
    pub id: u64,
    pub name: String,
    pub visible: bool,
    pub elements: Vec<SerializableShape>,
}

impl From<&Layer> for SerializableLayer {
    fn from(layer: &Layer) -> Self {
        Self {
            id: layer.id,
            name: layer.name.clone(),
            visible: layer.visible,
            elements: layer.elements.iter().map(SerializableShape::from).collect(),
        }
    }
}

impl From<SerializableLayer> for Layer {
    fn from(layer: SerializableLayer) -> Self {
        Self {
            id: layer.id,
            name: layer.name,
            visible: layer.visible,
            elements: layer.elements.into_iter().map(Shape::from).collect(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct SerializableLayerManager {
    pub layers: Vec<SerializableLayer>,
    pub active_layer_id: u64,
}

impl From<&LayerManager> for SerializableLayerManager {
    fn from(manager: &LayerManager) -> Self {
        Self {
            layers: manager.layers.iter().map(SerializableLayer::from).collect(),
            active_layer_id: manager.active_layer_id,
        }
    }
}

impl From<SerializableLayerManager> for LayerManager {
    fn from(manager: SerializableLayerManager) -> Self {
        Self {
            layers: manager.layers.into_iter().map(Layer::from).collect(),
            active_layer_id: manager.active_layer_id,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PaintProject {
    pub layer_manager: SerializableLayerManager,
}

// Mode d'outil actif dans l'interface.
#[derive(Clone, PartialEq)]
pub enum BrushMode { Freehand, Shape, Eraser, Select }

// Mode de sélection : rectangle ou lasso (freehand).
#[derive(Clone, Copy, PartialEq)]
pub enum SelectionMode { Rectangle, Lasso }

impl SelectionMode {
    pub fn label(&self) -> &'static str {
        match self {
            SelectionMode::Rectangle => "Rectangle",
            SelectionMode::Lasso => "Lasso",
        }
    }
}

pub struct Camera {
    pub zoom: f32,
    pub offset: Vec2,
}

impl Camera {
    pub fn new() -> Self {
        Self { zoom: 1.0, offset: Vec2::ZERO }
    }

    pub fn to_screen(&self, p: Pos2) -> Pos2 {
        Pos2::new(p.x * self.zoom + self.offset.x, p.y * self.zoom + self.offset.y)
    }

    pub fn to_world(&self, p: Pos2) -> Pos2 {
        Pos2::new((p.x - self.offset.x) / self.zoom, (p.y - self.offset.y) / self.zoom)
    }

    pub fn zoom_at(&mut self, factor: f32, pivot: Pos2) {
        self.offset.x = pivot.x - (pivot.x - self.offset.x) * factor;
        self.offset.y = pivot.y - (pivot.y - self.offset.y) * factor;
        self.zoom = (self.zoom * factor).clamp(0.1, 20.0);
    }
}

// Forme sélectionnée dans le menu des formes.
#[derive(Clone, Copy, PartialEq)]
pub enum ShapeKind {
    Line,
    Rectangle,
    Oval,
    Triangle,
    Pentagon,
    Hexagon,
    Octagon,
    Star,
    Arrow,
}

impl ShapeKind {
    pub fn label(&self) -> &'static str {
        match self {
            ShapeKind::Line => "Ligne",
            ShapeKind::Rectangle => "Rectangle",
            ShapeKind::Oval => "Ovale",
            ShapeKind::Triangle => "Triangle",
            ShapeKind::Pentagon => "Pentagone",
            ShapeKind::Hexagon => "Hexagone",
            ShapeKind::Octagon => "Octogone",
            ShapeKind::Star => "Étoile",
            ShapeKind::Arrow => "Flèche",
        }
    }

    pub fn sides(&self) -> u8 {
        match self {
            ShapeKind::Triangle => 3,
            ShapeKind::Pentagon => 5,
            ShapeKind::Hexagon => 6,
            ShapeKind::Octagon => 8,
            _ => 0,
        }
    }
}

// Une forme dessinée: lignes, rectangles, ovales, polygones, étoiles, flèches.
#[derive(Clone)]
pub enum Shape {
    Line { id: u64, points: Vec<Pos2>, color: Color32, width: f32 },
    Rectangle { id: u64, start: Pos2, end: Pos2, color: Color32, width: f32 },
    Oval { id: u64, start: Pos2, end: Pos2, color: Color32, width: f32 },
    RegularPolygon { id: u64, start: Pos2, end: Pos2, sides: u8, color: Color32, width: f32 },
    Star { id: u64, start: Pos2, end: Pos2, points: u8, color: Color32, width: f32 },
    Arrow { id: u64, start: Pos2, end: Pos2, color: Color32, width: f32 },
}

impl Shape {
    /// Récupère l'identifiant unique de la forme.
    pub fn id(&self) -> u64 {
        match self {
            Shape::Line { id, .. }
            | Shape::Rectangle { id, .. }
            | Shape::Oval { id, .. }
            | Shape::RegularPolygon { id, .. }
            | Shape::Star { id, .. }
            | Shape::Arrow { id, .. } => *id,
        }
    }

    /// Récupère la couleur.
    pub fn color(&self) -> Color32 {
        match self {
            Shape::Line { color, .. }
            | Shape::Rectangle { color, .. }
            | Shape::Oval { color, .. }
            | Shape::RegularPolygon { color, .. }
            | Shape::Star { color, .. }
            | Shape::Arrow { color, .. } => *color,
        }
    }

    /// Définit la couleur.
    pub fn set_color(&mut self, new_color: Color32) {
        match self {
            Shape::Line { color, .. }
            | Shape::Rectangle { color, .. }
            | Shape::Oval { color, .. }
            | Shape::RegularPolygon { color, .. }
            | Shape::Star { color, .. }
            | Shape::Arrow { color, .. } => *color = new_color,
        }
    }

    /// Récupère l'épaisseur.
    pub fn width(&self) -> f32 {
        match self {
            Shape::Line { width, .. }
            | Shape::Rectangle { width, .. }
            | Shape::Oval { width, .. }
            | Shape::RegularPolygon { width, .. }
            | Shape::Star { width, .. }
            | Shape::Arrow { width, .. } => *width,
        }
    }

    /// Définit l'épaisseur.
    pub fn set_width(&mut self, new_width: f32) {
        match self {
            Shape::Line { width, .. }
            | Shape::Rectangle { width, .. }
            | Shape::Oval { width, .. }
            | Shape::RegularPolygon { width, .. }
            | Shape::Star { width, .. }
            | Shape::Arrow { width, .. } => *width = new_width,
        }
    }

    /// Traduit la forme d'un vecteur donné.
    pub fn translate(&mut self, delta: Vec2) {
        match self {
            Shape::Line { points, .. } => {
                for p in points { *p += delta; }
            }
            Shape::Rectangle { start, end, .. }
            | Shape::Oval { start, end, .. }
            | Shape::RegularPolygon { start, end, .. }
            | Shape::Star { start, end, .. }
            | Shape::Arrow { start, end, .. } => {
                *start += delta;
                *end += delta;
            }
        }
    }

    /// Retourne la boîte englobante de la forme.
    pub fn bounding_rect(&self) -> Rect {
        match self {
            Shape::Line { points, width, .. } => {
                let mut r = Rect::NOTHING;
                for p in points { r.extend_with(*p); }
                r.expand(*width / 2.0 + 5.0)
            }
            Shape::Rectangle { start, end, width, .. }
            | Shape::Oval { start, end, width, .. }
            | Shape::RegularPolygon { start, end, width, .. }
            | Shape::Star { start, end, width, .. }
            | Shape::Arrow { start, end, width, .. } => {
                let r = Rect::from_two_pos(*start, *end);
                r.expand(*width / 2.0 + 5.0)
            }
        }
    }

    /// Distance minimale entre le point et la forme.
    pub fn distance_to(&self, pos: Pos2) -> f32 {
        match self {
            Shape::Line { points, .. } => {
                points.windows(2)
                    .map(|w| distance_point_to_segment(pos, w[0], w[1]))
                    .fold(f32::INFINITY, f32::min)
            }
            Shape::Rectangle { start, end, .. } => {
                let rect = Rect::from_two_pos(*start, *end);
                let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
                corners.windows(2)
                    .map(|w| distance_point_to_segment(pos, w[0], w[1]))
                    .chain(std::iter::once(distance_point_to_segment(pos, corners[3], corners[0])))
                    .fold(f32::INFINITY, f32::min)
            }
            Shape::Oval { start, end, .. } => {
                let rect = Rect::from_two_pos(*start, *end);
                let center = rect.center();
                let a = rect.width() / 2.0;
                let b = rect.height() / 2.0;
                if a == 0.0 || b == 0.0 {
                    return (pos.x - center.x).abs().max((pos.y - center.y).abs());
                }
                let nx = (pos.x - center.x) / a;
                let ny = (pos.y - center.y) / b;
                ((nx * nx + ny * ny).sqrt() - 1.0).abs() * ((a + b) / 2.0)
            }
            Shape::RegularPolygon { start, end, sides, .. } => {
                let rect = Rect::from_two_pos(*start, *end);
                let points = regular_polygon_points(rect.center(), rect.width() / 2.0, rect.height() / 2.0, *sides as usize);
                polygon_distance(pos, &points)
            }
            Shape::Star { start, end, points, .. } => {
                let rect = Rect::from_two_pos(*start, *end);
                let pts = star_points(rect.center(), rect.width() / 2.0, rect.height() / 2.0, *points as usize);
                polygon_distance(pos, &pts)
            }
            Shape::Arrow { start, end, .. } => {
                distance_point_to_segment(pos, *start, *end)
            }
        }
    }
}

fn regular_polygon_points(center: Pos2, a: f32, b: f32, sides: usize) -> Vec<Pos2> {
    let mut points = Vec::new();
    let angle_step = std::f32::consts::TAU / sides as f32;
    for i in 0..=sides {
        let angle = i as f32 * angle_step - std::f32::consts::FRAC_PI_2;
        points.push(Pos2::new(center.x + a * angle.cos(), center.y + b * angle.sin()));
    }
    points
}

fn star_points(center: Pos2, a: f32, b: f32, points: usize) -> Vec<Pos2> {
    let mut pts = Vec::new();
    let outer = a.min(b);
    let inner = outer * 0.45;
    let total = points * 2;
    let angle_step = std::f32::consts::TAU / total as f32;
    for i in 0..=total {
        let angle = i as f32 * angle_step - std::f32::consts::FRAC_PI_2;
        let radius = if i % 2 == 0 { outer } else { inner };
        pts.push(Pos2::new(center.x + radius * angle.cos(), center.y + radius * angle.sin()));
    }
    pts
}

fn polygon_distance(pos: Pos2, points: &[Pos2]) -> f32 {
    points.windows(2)
        .map(|w| distance_point_to_segment(pos, w[0], w[1]))
        .chain(std::iter::once(distance_point_to_segment(pos, points[points.len() - 1], points[0])))
        .fold(f32::INFINITY, f32::min)
}

fn distance_point_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let l2 = a.distance_sq(b);
    if l2 == 0.0 { return p.distance(a); }
    let t = ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / l2;
    let t = t.clamp(0.0, 1.0);
    let proj = Pos2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y));
    p.distance(proj)
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
    pub selected_shape: ShapeKind,
    pub brush_color: Color32,
    pub brush_size: f32,
    pub current_line: Vec<Pos2>,
    pub active_stroke_id: Option<u64>,
    pub selected_indices: Vec<usize>,
    pub selection_start_pos: Option<Pos2>,
    pub selection_rect: Option<Rect>,
    pub selection_mode: SelectionMode,
    pub current_lasso: Vec<Pos2>,
    pub camera: Camera,
    pub clipboard: Vec<Shape>,
    pub is_dragging_items: bool,
    pub drag_accumulated_delta: Vec2,
    pub custom_palette: Vec<Color32>,
    pub save_load_status: String,
    pub save_load_file_path: String,
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
    pub last_layer_index: usize,
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
            selected_shape: ShapeKind::Line,
            brush_color: Color32::from_rgb(0, 120, 255),
            brush_size: 4.0,
            current_line: Vec::new(),
            active_stroke_id: None,
            selected_indices: Vec::new(),
            selection_start_pos: None,
            selection_rect: None,
            selection_mode: SelectionMode::Rectangle,
            current_lasso: Vec::new(),
            camera: Camera::new(),
            clipboard: Vec::new(),
            is_dragging_items: false,
            drag_accumulated_delta: Vec2::ZERO,
            custom_palette: Vec::new(),
            save_load_status: String::new(),
            save_load_file_path: "canvas.rpaint".to_string(),
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
            last_layer_index: 0,
        }
    }
}

// Génère un identifiant simple basé sur le timestamp (en nanosecondes).
pub fn timestamp_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
}
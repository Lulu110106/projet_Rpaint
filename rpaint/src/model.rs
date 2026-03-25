use eframe::egui::{ Color32, Pos2, Rect, Vec2};

#[derive(Clone, PartialEq)]
pub enum BrushMode { Freehand, StraightLine, Eraser, Select }

#[derive(Clone)]
pub struct Line {
    pub points: Vec<Pos2>,
    pub color: Color32,
    pub width: f32,
}

#[derive(Clone)]
pub enum PaintAction {
    Create(Vec<Line>),
    Delete(Vec<usize>, Vec<Line>),
    Modify(Vec<usize>, Vec<Line>, Vec<Line>),
    Move(Vec<usize>, Vec2),
}

pub struct PaintApp {
    pub lines: Vec<Line>,
    pub undo_stack: Vec<PaintAction>,
    pub redo_stack: Vec<PaintAction>,
    pub mode: BrushMode,
    pub brush_color: Color32,
    pub brush_size: f32,
    pub current_line: Vec<Pos2>,
    pub selected_indices: Vec<usize>,
    pub selection_start_pos: Option<Pos2>,
    pub selection_rect: Option<Rect>,
    pub clipboard: Vec<Line>,
    pub is_dragging_items: bool,
    pub drag_accumulated_delta: Vec2,
    pub custom_palette: Vec<Color32>,
}

impl Default for PaintApp {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            mode: BrushMode::Freehand,
            brush_color: Color32::from_rgb(0, 120, 255),
            brush_size: 4.0,
            current_line: Vec::new(),
            selected_indices: Vec::new(),
            selection_start_pos: None,
            selection_rect: None,
            clipboard: Vec::new(),
            is_dragging_items: false,
            drag_accumulated_delta: Vec2::ZERO,
            custom_palette: Vec::new(),
        }
    }
}
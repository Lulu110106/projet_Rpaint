use egui::{Color32, Pos2, Rect, Vec2};
use crate::models::{Line, PaintAction, BrushMode};
use crate::network::NetworkManager;

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
    
    pub network: NetworkManager,
}

impl Default for PaintApp {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            mode: BrushMode::Freehand,
            brush_color: Color32::from_rgb(0, 150, 255),
            brush_size: 4.0,
            current_line: Vec::new(),
            selected_indices: Vec::new(),
            selection_start_pos: None,
            selection_rect: None,
            clipboard: Vec::new(),
            is_dragging_items: false,
            drag_accumulated_delta: Vec2::ZERO,
            network: NetworkManager::new(),
        }
    }
}

impl PaintApp {
    pub fn execute(&mut self, action: PaintAction) {
        self.apply_action(&action);
        self.undo_stack.push(action);
        self.redo_stack.clear();
    }

    pub fn apply_action(&mut self, action: &PaintAction) {
        match action {
            PaintAction::Create(new_lines) => {
                for sline in new_lines {
                    self.lines.push(Line::from(sline));
                }
            }
            PaintAction::Delete(indices, _) => {
                let mut sorted = indices.clone();
                sorted.sort_by(|a, b| b.cmp(a));
                for idx in sorted {
                    if idx < self.lines.len() {
                        self.lines.remove(idx);
                    }
                }
            }
            PaintAction::Modify(indices, _, new_lines) => {
                for (i, &idx) in indices.iter().enumerate() {
                    if let Some(l) = self.lines.get_mut(idx) {
                        *l = Line::from(&new_lines[i]);
                    }
                }
            }
            PaintAction::Move(indices, dx, dy) => {
                for &idx in indices {
                    if let Some(l) = self.lines.get_mut(idx) {
                        for p in &mut l.points {
                            *p += Vec2::new(*dx, *dy);
                        }
                    }
                }
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(action) = self.undo_stack.pop() {
            match &action {
                PaintAction::Create(lines) => {
                    for _ in 0..lines.len() {
                        self.lines.pop();
                    }
                }
                PaintAction::Delete(indices, lines) => {
                    let mut combined: Vec<_> = indices.iter().zip(lines.iter()).collect();
                    combined.sort_by_key(|&(&idx, _)| idx);
                    for (&idx, line) in combined {
                        self.lines.insert(idx, Line::from(line));
                    }
                }
                PaintAction::Modify(indices, old_lines, _) => {
                    for (i, &idx) in indices.iter().enumerate() {
                        if let Some(l) = self.lines.get_mut(idx) {
                            *l = Line::from(&old_lines[i]);
                        }
                    }
                }
                PaintAction::Move(indices, dx, dy) => {
                    for &idx in indices {
                        if let Some(l) = self.lines.get_mut(idx) {
                            for p in &mut l.points {
                                *p -= Vec2::new(*dx, *dy);
                            }
                        }
                    }
                }
            }
            self.redo_stack.push(action);
            self.selected_indices.clear();
        }
    }

    pub fn redo(&mut self) {
        if let Some(action) = self.redo_stack.pop() {
            self.apply_action(&action);
            self.undo_stack.push(action);
        }
    }

    pub fn copy_selected(&mut self) {
        if self.selected_indices.is_empty() {
            return;
        }
        self.clipboard = self.selected_indices
            .iter()
            .filter_map(|&i| self.lines.get(i).cloned())
            .collect();
    }

    pub fn paste(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        let offset = Vec2::splat(20.0);
        let mut new_lines = self.clipboard.clone();
        for line in &mut new_lines {
            for p in &mut line.points {
                *p += offset;
            }
        }
        let serialized: Vec<_> = new_lines.iter().map(|l| crate::models::SerializableLine::from(l)).collect();
        self.execute(PaintAction::Create(serialized.clone()));
        self.clipboard = new_lines;
        let start_idx = self.lines.len() - self.clipboard.len();
        self.selected_indices = (start_idx..self.lines.len()).collect();
    }

    pub fn delete_selected(&mut self) {
        if self.selected_indices.is_empty() {
            return;
        }
        let mut indexed: Vec<_> = self.selected_indices
            .iter()
            .filter_map(|&i| self.lines.get(i).map(|l| (i, l.clone())))
            .collect();
        indexed.sort_by_key(|&(i, _)| i);
        let indices: Vec<usize> = indexed.iter().map(|(i, _)| *i).collect();
        let lines: Vec<_> = indexed.into_iter().map(|(_, l)| crate::models::SerializableLine::from(&l)).collect();
        self.execute(PaintAction::Delete(indices.clone(), lines));

        if self.network.is_connected() {
            let _ = self.network.broadcast_message(crate::network::DrawingMessage::Delete { indices });
        }

        self.selected_indices.clear();
    }

    pub fn clear_all(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        let indices = (0..self.lines.len()).collect();
        let lines: Vec<_> = self.lines.iter().map(|l| crate::models::SerializableLine::from(l)).collect();
        self.execute(PaintAction::Delete(indices, lines));
        self.selected_indices.clear();
    }

    pub fn get_line_rect(&self, idx: usize) -> Rect {
        if let Some(line) = self.lines.get(idx) {
            let mut r = Rect::NOTHING;
            for p in &line.points {
                r.extend_with(*p);
            }
            return r.expand(line.width / 2.0 + 5.0);
        }
        Rect::NOTHING
    }
}

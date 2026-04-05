use crate::model::*;
use eframe::egui::{Pos2, Rect, Vec2};

impl PaintApp {
    // --- ACTIONS DE BASE ---

    pub fn clear_all(&mut self) {
        if self.lines.is_empty() { return; }
        let indices = (0..self.lines.len()).collect();
        let lines = self.lines.clone();
        self.execute(PaintAction::Delete(indices, lines));
        self.selected_indices.clear();
    }

    pub fn delete_selected(&mut self) {
        if self.selected_indices.is_empty() { return; }
        let mut indexed: Vec<_> = self.selected_indices.iter()
            .filter_map(|&i| self.lines.get(i).map(|l| (i, l.clone())))
            .collect();
        indexed.sort_by_key(|&(i, _)| i);
        let indices = indexed.iter().map(|(i, _)| *i).collect();
        let lines = indexed.into_iter().map(|(_, l)| l).collect();
        self.execute(PaintAction::Delete(indices, lines));
        self.selected_indices.clear();
    }

    // --- PRESSE-PAPIER ---

    pub fn copy_selected(&mut self) {
        if self.selected_indices.is_empty() { return; }
        self.clipboard = self.selected_indices.iter()
            .filter_map(|&i| self.lines.get(i).cloned())
            .collect();
    }

    pub fn paste(&mut self) {
        if self.clipboard.is_empty() { return; }
        let offset = Vec2::splat(20.0);
        let mut new_lines = self.clipboard.clone();
        for line in &mut new_lines {
            for p in &mut line.points { *p += offset; }
        }
        self.execute(PaintAction::Create(new_lines.clone()));
        self.clipboard = new_lines; 
        let start_idx = self.lines.len() - self.clipboard.len();
        self.selected_indices = (start_idx..self.lines.len()).collect();
    }

    // --- SYSTÈME UNDO/REDO ---

    pub fn execute(&mut self, action: PaintAction) {
        self.apply_action(&action);
        self.undo_stack.push(action);
        self.redo_stack.clear();
    }

    pub fn apply_action(&mut self, action: &PaintAction) {
        match action {
            PaintAction::Create(new_lines) => {
                for l in new_lines { self.lines.push(l.clone()); }
            },
            PaintAction::Delete(indices, _) => {
                let mut sorted = indices.clone();
                sorted.sort_by(|a, b| b.cmp(a));
                for idx in sorted { 
                    if idx < self.lines.len() { self.lines.remove(idx); } 
                }
            },
            PaintAction::Modify(indices, _, new_lines) => {
                for (i, &idx) in indices.iter().enumerate() {
                    if let Some(l) = self.lines.get_mut(idx) { *l = new_lines[i].clone(); }
                }
            },
            PaintAction::Move(indices, delta) => {
                for &idx in indices {
                    if let Some(l) = self.lines.get_mut(idx) {
                        for p in &mut l.points { *p += *delta; }
                    }
                }
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(action) = self.undo_stack.pop() {
            match &action {
                PaintAction::Create(lines) => {
                    for _ in 0..lines.len() { self.lines.pop(); }
                },
                PaintAction::Delete(indices, lines) => {
                    let mut combined: Vec<_> = indices.iter().zip(lines.iter()).collect();
                    combined.sort_by_key(|&(&idx, _)| idx);
                    for (&idx, line) in combined { self.lines.insert(idx, line.clone()); }
                },
                PaintAction::Modify(indices, old_lines, _) => {
                    for (i, &idx) in indices.iter().enumerate() {
                        if let Some(l) = self.lines.get_mut(idx) { *l = old_lines[i].clone(); }
                    }
                },
                PaintAction::Move(indices, delta) => {
                    for &idx in indices {
                        if let Some(l) = self.lines.get_mut(idx) {
                            for p in &mut l.points { *p -= *delta; }
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

    // --- UTILITAIRES DE SÉLECTION ---

    pub fn get_line_rect(&self, idx: usize) -> Rect {
        if let Some(line) = self.lines.get(idx) {
            let mut r = Rect::NOTHING;
            for p in &line.points { r.extend_with(*p); }
            return r.expand(line.width / 2.0 + 5.0);
        }
        Rect::NOTHING
    }
}

pub fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let l2 = a.distance_sq(b);
    if l2 == 0.0 { return p.distance(a); }
    let t = ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / l2;
    p.distance(Pos2::new(a.x + t.clamp(0.0, 1.0) * (b.x - a.x), a.y + t.clamp(0.0, 1.0) * (b.y - a.y)))
}
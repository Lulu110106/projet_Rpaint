use eframe::egui;
use egui::{Color32, Pos2, Rect, Stroke, Vec2, Shape};

// --- TYPES ---

#[derive(Clone, PartialEq)]
enum BrushMode { Freehand, StraightLine, Eraser, Select }

#[derive(Clone)]
struct Line {
    points: Vec<Pos2>,
    color: Color32,
    width: f32,
}

#[derive(Clone)]
enum PaintAction {
    Create(Vec<Line>),
    Delete(Vec<usize>, Vec<Line>),
    Modify(Vec<usize>, Vec<Line>, Vec<Line>),
    Move(Vec<usize>, Vec2),
}

// --- APPLICATION ---

struct PaintApp {
    lines: Vec<Line>,
    undo_stack: Vec<PaintAction>,
    redo_stack: Vec<PaintAction>,
    
    mode: BrushMode,
    brush_color: Color32,
    brush_size: f32,
    current_line: Vec<Pos2>,
    
    selected_indices: Vec<usize>,
    selection_start_pos: Option<Pos2>,
    selection_rect: Option<Rect>,
    
    clipboard: Vec<Line>,
    
    is_dragging_items: bool,
    drag_accumulated_delta: Vec2,
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
        }
    }
}

impl PaintApp {
    fn get_line_rect(&self, idx: usize) -> Rect {
        if let Some(line) = self.lines.get(idx) {
            let mut r = Rect::NOTHING;
            for p in &line.points { r.extend_with(*p); }
            return r.expand(line.width / 2.0 + 5.0);
        }
        Rect::NOTHING
    }

    fn copy_selected(&mut self) {
        if self.selected_indices.is_empty() { return; }
        self.clipboard = self.selected_indices.iter()
            .filter_map(|&i| self.lines.get(i).cloned())
            .collect();
    }

    fn paste(&mut self) {
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

    fn execute(&mut self, action: PaintAction) {
        self.apply_action(&action);
        self.undo_stack.push(action);
        self.redo_stack.clear();
    }

    fn apply_action(&mut self, action: &PaintAction) {
        match action {
            PaintAction::Create(new_lines) => {
                for l in new_lines { self.lines.push(l.clone()); }
            },
            PaintAction::Delete(indices, _) => {
                let mut sorted = indices.clone();
                sorted.sort_by(|a, b| b.cmp(a));
                for idx in sorted { if idx < self.lines.len() { self.lines.remove(idx); } }
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

    fn undo(&mut self) {
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

    fn redo(&mut self) {
        if let Some(action) = self.redo_stack.pop() {
            self.apply_action(&action);
            self.undo_stack.push(action);
        }
    }

    fn delete_selected(&mut self) {
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
}

// --- FONCTIONS GLOBALES ---

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let l2 = a.distance_sq(b);
    if l2 == 0.0 { return p.distance(a); }
    let t = ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / l2;
    p.distance(Pos2::new(a.x + t.clamp(0.0, 1.0) * (b.x - a.x), a.y + t.clamp(0.0, 1.0) * (b.y - a.y)))
}

fn draw_dashed_rect(painter: &egui::Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(1.0, color);
    let dash_len = 6.0;
    let gap_len = 4.0;
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
    for w in corners.windows(2) {
        let (start, end) = (w[0], w[1]);
        let full_vec = end - start;
        let len = full_vec.length();
        if len < 0.1 { continue; }
        let dir = full_vec / len;
        let mut d = 0.0;
        while d < len {
            painter.line_segment([start + dir * d, start + dir * (d + dash_len).min(len)], stroke);
            d += dash_len + gap_len;
        }
    }
}

// --- TRAIT eframe::App ---

impl eframe::App for PaintApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(egui::Key::Z) { self.undo(); }
            if i.modifiers.command && i.key_pressed(egui::Key::Y) { self.redo(); }
            if i.modifiers.command && i.key_pressed(egui::Key::C) { self.copy_selected(); }
            if i.modifiers.command && i.key_pressed(egui::Key::V) { self.paste(); }
            if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) { self.delete_selected(); }
        });

        egui::SidePanel::left("toolbar").show(ctx, |ui| {
            ui.heading("🎨 Rust Paint");
            ui.separator();

            ui.label("Édition");
            ui.horizontal(|ui| {
                if ui.button("↩").on_hover_text("Undo").clicked() { self.undo(); }
                if ui.button("↪").on_hover_text("Redo").clicked() { self.redo(); }
                ui.separator();
                if ui.button("✂").on_hover_text("Copy").clicked() { self.copy_selected(); }
                if ui.button("📋").on_hover_text("Paste").clicked() { self.paste(); }
            });

            ui.separator();
            ui.label("Outils");
            let old_mode = self.mode.clone();
            ui.selectable_value(&mut self.mode, BrushMode::Freehand, "✏ Dessin");
            ui.selectable_value(&mut self.mode, BrushMode::StraightLine, "📏 Ligne");
            ui.selectable_value(&mut self.mode, BrushMode::Eraser, "🧽 Gomme");
            ui.selectable_value(&mut self.mode, BrushMode::Select, "🖱 Sélection");

            if self.mode != BrushMode::Select && old_mode == BrushMode::Select { self.selected_indices.clear(); }

            ui.separator();
            ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=50.0).text("Taille"));
            ui.color_edit_button_srgba(&mut self.brush_color);

            // --- SECTION ACTIONS DE SÉLECTION ---
            if !self.selected_indices.is_empty() {
                ui.separator();
                ui.label(format!("Sélection: {} objet(s)", self.selected_indices.len()));
                ui.vertical_centered_justified(|ui| {
                    if ui.button("🎨 Appliquer Couleur").clicked() {
                        let old = self.selected_indices.iter().filter_map(|&i| self.lines.get(i).cloned()).collect();
                        let new = self.selected_indices.iter().filter_map(|&i| {
                            let mut l = self.lines.get(i).cloned()?;
                            l.color = self.brush_color;
                            Some(l)
                        }).collect();
                        self.execute(PaintAction::Modify(self.selected_indices.clone(), old, new));
                    }
                    if ui.button("🗑 Supprimer").clicked() {
                        self.delete_selected();
                    }
                });
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
            let pointer = response.interact_pointer_pos();

            if let Some(pos) = pointer {
                match self.mode {
                    BrushMode::Freehand | BrushMode::StraightLine => {
                        if response.dragged() {
                            if self.mode == BrushMode::StraightLine {
                                if self.current_line.is_empty() { self.current_line.push(pos); }
                                if self.current_line.len() > 1 { self.current_line.pop(); }
                            }
                            self.current_line.push(pos);
                        } else if response.drag_released() && !self.current_line.is_empty() {
                            let points = std::mem::take(&mut self.current_line);
                            let line = Line { points, color: self.brush_color, width: self.brush_size };
                            self.execute(PaintAction::Create(vec![line]));
                        }
                    },
                    BrushMode::Eraser => {
                        if response.dragged() || response.clicked() {
                            let mut to_del = None;
                            for (i, line) in self.lines.iter().enumerate() {
                                if line.points.windows(2).any(|w| dist_to_segment(pos, w[0], w[1]) < self.brush_size) {
                                    to_del = Some(i); break;
                                }
                            }
                            if let Some(idx) = to_del {
                                let line = self.lines[idx].clone();
                                self.execute(PaintAction::Delete(vec![idx], vec![line]));
                            }
                        }
                    },
                    BrushMode::Select => {
                        if response.drag_started() {
                            let mut hit = self.selected_indices.iter().find(|&&i| 
                                self.get_line_rect(i).contains(pos)).cloned();
                            
                            if hit.is_none() {
                                hit = self.lines.iter().enumerate().find(|(_, l)| 
                                    l.points.windows(2).any(|w| dist_to_segment(pos, w[0], w[1]) < 10.0)).map(|(i, _)| i);
                            }

                            if let Some(idx) = hit {
                                if !self.selected_indices.contains(&idx) { self.selected_indices = vec![idx]; }
                                self.is_dragging_items = true;
                                self.drag_accumulated_delta = Vec2::ZERO;
                            } else {
                                self.selection_start_pos = Some(pos);
                                self.selected_indices.clear();
                            }
                        }
                        if response.dragged() {
                            if self.is_dragging_items {
                                let delta = response.drag_delta();
                                self.drag_accumulated_delta += delta;
                                for &idx in &self.selected_indices {
                                    if let Some(l) = self.lines.get_mut(idx) { for p in &mut l.points { *p += delta; } }
                                }
                            } else if let Some(start) = self.selection_start_pos {
                                self.selection_rect = Some(Rect::from_two_pos(start, pos));
                            }
                        }
                        if response.drag_released() {
                            if self.is_dragging_items {
                                let total = self.drag_accumulated_delta;
                                if total.length_sq() > 0.0 {
                                    for &idx in &self.selected_indices {
                                        if let Some(l) = self.lines.get_mut(idx) { for p in &mut l.points { *p -= total; } }
                                    }
                                    self.execute(PaintAction::Move(self.selected_indices.clone(), total));
                                }
                                self.is_dragging_items = false;
                            } else if let Some(rect) = self.selection_rect.take() {
                                self.selected_indices = self.lines.iter().enumerate()
                                    .filter(|(_, l)| l.points.iter().any(|p| rect.contains(*p)))
                                    .map(|(i, _)| i).collect();
                                self.selection_start_pos = None;
                            }
                        }
                    }
                }
            }

            for (i, line) in self.lines.iter().enumerate() {
                painter.add(Shape::line(line.points.clone(), Stroke::new(line.width, line.color)));
                if self.mode == BrushMode::Select && self.selected_indices.contains(&i) {
                    let r = self.get_line_rect(i);
                    draw_dashed_rect(&painter, r, Color32::WHITE);
                    draw_dashed_rect(&painter, r.expand(1.0), Color32::BLACK);
                }
            }

            if let Some(r) = self.selection_rect {
                painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(100, 150, 255, 30));
                painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_rgb(100, 150, 255)));
            }

            if !self.current_line.is_empty() {
                painter.add(Shape::line(self.current_line.clone(), Stroke::new(self.brush_size, self.brush_color)));
            }

            if self.mode == BrushMode::Eraser {
                if let Some(p) = ctx.pointer_latest_pos() {
                    painter.circle_stroke(p, self.brush_size, Stroke::new(1.0, Color32::LIGHT_RED));
                }
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Rust Paint Pro",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Box::new(PaintApp::default())),
    )
}
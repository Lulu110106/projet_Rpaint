mod app;
mod models;
mod network;
mod utils;

use app::PaintApp;
use eframe::egui;
<<<<<<< Updated upstream
use egui::{Color32, Pos2, Stroke};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Rust Paint Pro",
        options,
        Box::new(|_cc| Box::new(PaintApp::default())),
    )
}

#[derive(Clone, PartialEq)]
enum BrushMode {
    Freehand,
    StraightLine,
    Eraser,
}

struct Line {
    points: Vec<Pos2>,
    color: Color32,
    width: f32,
}

struct PaintApp {
    lines: Vec<Line>,
    redo_stack: Vec<Line>, // <-- Pile pour le Redo
    current_line: Vec<Pos2>,
    brush_color: Color32,
    brush_size: f32,
    mode: BrushMode,
}

impl Default for PaintApp {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            redo_stack: Vec::new(),
            current_line: Vec::new(),
            brush_color: Color32::LIGHT_BLUE,
            brush_size: 4.0,
            mode: BrushMode::Freehand,
        }
    }
}

impl PaintApp {
    // Logique pour annuler
    fn undo(&mut self) {
        if let Some(line) = self.lines.pop() {
            self.redo_stack.push(line);
        }
    }

    // Logique pour rétablir
    fn redo(&mut self) {
        if let Some(line) = self.redo_stack.pop() {
            self.lines.push(line);
        }
    }
}

impl eframe::App for PaintApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        
        // --- Gestion des raccourcis clavier ---
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            self.undo();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Y)) {
            self.redo();
        }

        // --- UI : Panneau de réglages ---
        egui::SidePanel::left("settings").show(ctx, |ui| {
            ui.heading("Outils");
            
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, BrushMode::Freehand, "✏ Main levée");
                ui.selectable_value(&mut self.mode, BrushMode::StraightLine, "📏 Ligne");
                ui.selectable_value(&mut self.mode, BrushMode::Eraser, "🧽 Gomme");
=======
use egui::{Color32, Rect, Shape, Stroke, Vec2};
use models::BrushMode;
use network::DrawingMessage;
use utils::{dist_to_segment, draw_dashed_rect};

impl eframe::App for PaintApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(egui::Key::Z) {
                self.undo();
            }
            if i.modifiers.command && i.key_pressed(egui::Key::Y) {
                self.redo();
            }
            if i.modifiers.command && i.key_pressed(egui::Key::C) {
                self.copy_selected();
            }
            if i.modifiers.command && i.key_pressed(egui::Key::V) {
                self.paste();
            }
            if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                self.delete_selected();
            }
        });

        egui::SidePanel::left("toolbar").show(ctx, |ui| {
            ui.heading("🎨 Rust Paint");
            ui.separator();

            ui.label("🌐 Réseau");
            ui.horizontal(|ui| {
                if ui
                    .button(if self.network.is_connected() {
                        "🟢 Connecté"
                    } else {
                        "🔴 Déconnecté"
                    })
                    .clicked()
                {
                    if self.network.is_connected() {
                        self.network.disconnect();
                    } else {
                        let _ = self.network.connect();
                    }
                }
                ui.label(format!("Pairs: {}", self.network.peer_count()));
            });
            ui.separator();

            ui.label("Édition");
            ui.horizontal(|ui| {
                if ui.button("↩").on_hover_text("Annuler").clicked() {
                    self.undo();
                }
                if ui.button("↪").on_hover_text("Rétablir").clicked() {
                    self.redo();
                }
                ui.separator();
                if ui.button("✂").on_hover_text("Copier").clicked() {
                    self.copy_selected();
                }
                if ui.button("📋").on_hover_text("Coller").clicked() {
                    self.paste();
                }
>>>>>>> Stashed changes
            });

            ui.separator();

            ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=50.0).text("Taille"));
            
            if self.mode != BrushMode::Eraser {
                ui.color_edit_button_srgba(&mut self.brush_color);
            } else {
                ui.label("Mode Gomme actif");
            }
            
            ui.separator();

<<<<<<< Updated upstream
            // Boutons Undo / Redo
            ui.horizontal(|ui| {
                if ui.button("↩ Annuler").on_hover_text("Ctrl+Z").clicked() {
                    self.undo();
                }
                if ui.button("↪ Rétablir").on_hover_text("Ctrl+Y").clicked() {
                    self.redo();
                }
            });

            if ui.button("🗑 Effacer tout").clicked() {
                self.lines.clear();
                self.redo_stack.clear();
            }
        });

        // --- Zone de dessin ---
        egui::CentralPanel::default().show(ctx, |ui| {
            let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::drag());
            
            let current_color = if self.mode == BrushMode::Eraser {
                ui.visuals().panel_fill
            } else {
                self.brush_color
            };

            // 1. Gestion des entrées
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                match self.mode {
                    BrushMode::Freehand | BrushMode::Eraser => {
                        if response.dragged() {
                            self.current_line.push(pointer_pos);
                        }
                    }
                    BrushMode::StraightLine => {
                        if response.dragged() {
                            if self.current_line.is_empty() {
                                self.current_line.push(pointer_pos);
                            }
                            if self.current_line.len() > 1 {
                                self.current_line.pop();
                            }
                            self.current_line.push(pointer_pos);
                        }
                    }
                }
            } else if !self.current_line.is_empty() {
                // Quand on termine un trait :
                // On vide la redo_stack car une nouvelle action invalide le futur précédent
                self.redo_stack.clear();
                
                self.lines.push(Line {
                    points: std::mem::take(&mut self.current_line),
                    color: current_color,
                    width: self.brush_size,
                });
            }

            // 2. Rendu : Historique
            for line in &self.lines {
                if line.points.len() >= 2 {
                    painter.add(egui::Shape::line(
                        line.points.clone(),
                        Stroke::new(line.width, line.color),
                    ));
                }
            }

            // 3. Rendu : Prévisualisation
            if self.current_line.len() >= 2 {
                painter.add(egui::Shape::line(
                    self.current_line.clone(),
                    Stroke::new(self.brush_size, current_color),
                ));
            }
        });
    }
}
=======
            if !self.selected_indices.is_empty() {
                ui.separator();
                ui.label(format!("Sélection: {}", self.selected_indices.len()));

                ui.vertical_centered_justified(|ui| {
                    if ui.button("🎨 Appliquer Couleur").clicked() {
                        let old: Vec<_> = self
                            .selected_indices
                            .iter()
                            .filter_map(|&i| self.lines.get(i).cloned())
                            .map(|l| models::SerializableLine::from(&l))
                            .collect();
                        let new: Vec<_> = self
                            .selected_indices
                            .iter()
                            .filter_map(|&i| {
                                let mut l = self.lines.get(i).cloned()?;
                                l.color = self.brush_color;
                                Some(models::SerializableLine::from(&l))
                            })
                            .collect();
                        self.execute(models::PaintAction::Modify(
                            self.selected_indices.clone(),
                            old,
                            new,
                        ));
                    }

                    if ui.button("📏 Appliquer Taille").clicked() {
                        let old: Vec<_> = self
                            .selected_indices
                            .iter()
                            .filter_map(|&i| self.lines.get(i).cloned())
                            .map(|l| models::SerializableLine::from(&l))
                            .collect();
                        let new: Vec<_> = self
                            .selected_indices
                            .iter()
                            .filter_map(|&i| {
                                let mut l = self.lines.get(i).cloned()?;
                                l.width = self.brush_size;
                                Some(models::SerializableLine::from(&l))
                            })
                            .collect();
                        self.execute(models::PaintAction::Modify(
                            self.selected_indices.clone(),
                            old,
                            new,
                        ));
                    }

                    if ui.button("🗑 Supprimer").clicked() {
                        self.delete_selected();
                    }
                });
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                if ui
                    .add_enabled(
                        !self.lines.is_empty(),
                        egui::Button::new("💣 Tout effacer"),
                    )
                    .clicked()
                {
                    self.clear_all();
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
            let pointer = response.interact_pointer_pos();

            if let Some(pos) = pointer {
                match self.mode {
                    BrushMode::Freehand | BrushMode::StraightLine => {
                        if response.dragged() {
                            if self.mode == BrushMode::StraightLine {
                                if self.current_line.is_empty() {
                                    self.current_line.push(pos);
                                }
                                if self.current_line.len() > 1 {
                                    self.current_line.pop();
                                }
                            }
                            self.current_line.push(pos);
                        } else if response.drag_released() && !self.current_line.is_empty() {
                            let points = std::mem::take(&mut self.current_line);
                            let line = models::Line {
                                points,
                                color: self.brush_color,
                                width: self.brush_size,
                            };
                            self.execute(models::PaintAction::Create(vec![
                                models::SerializableLine::from(&line),
                            ]));

                            if self.network.is_connected() {
                                let [r, g, b, a] = self.brush_color.to_srgba_unmultiplied();
                                let color =
                                    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8)
                                        | (b as u32);
                                let msg = DrawingMessage::DrawLine {
                                    points: line.points.iter().map(|p| (p.x, p.y)).collect(),
                                    color,
                                    width: line.width,
                                };
                                let _ = self.network.broadcast_message(msg);
                            }
                        }
                    }
                    BrushMode::Eraser => {
                        if response.dragged() || response.clicked() {
                            let mut to_del = None;
                            for (i, line) in self.lines.iter().enumerate() {
                                if line
                                    .points
                                    .windows(2)
                                    .any(|w| dist_to_segment(pos, w[0], w[1]) < self.brush_size)
                                {
                                    to_del = Some(i);
                                    break;
                                }
                            }
                            if let Some(idx) = to_del {
                                let line = self.lines[idx].clone();
                                self.execute(models::PaintAction::Delete(
                                    vec![idx],
                                    vec![models::SerializableLine::from(&line)],
                                ));
                            }
                        }
                    }
                    BrushMode::Select => {
                        if response.drag_started() {
                            let mut hit = self
                                .selected_indices
                                .iter()
                                .find(|&&i| self.get_line_rect(i).contains(pos))
                                .cloned();
                            if hit.is_none() {
                                hit = self
                                    .lines
                                    .iter()
                                    .enumerate()
                                    .find(|(_, l)| {
                                        l.points
                                            .windows(2)
                                            .any(|w| dist_to_segment(pos, w[0], w[1]) < 10.0)
                                    })
                                    .map(|(i, _)| i);
                            }
                            if let Some(idx) = hit {
                                if !self.selected_indices.contains(&idx) {
                                    self.selected_indices = vec![idx];
                                }
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
                                    if let Some(l) = self.lines.get_mut(idx) {
                                        for p in &mut l.points {
                                            *p += delta;
                                        }
                                    }
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
                                        if let Some(l) = self.lines.get_mut(idx) {
                                            for p in &mut l.points {
                                                *p -= total;
                                            }
                                        }
                                    }
                                    self.execute(models::PaintAction::Move(
                                        self.selected_indices.clone(),
                                        total.x,
                                        total.y,
                                    ));
                                }
                                self.is_dragging_items = false;
                            } else if let Some(rect) = self.selection_rect.take() {
                                self.selected_indices = self
                                    .lines
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, l)| l.points.iter().any(|p| rect.contains(*p)))
                                    .map(|(i, _)| i)
                                    .collect();
                                self.selection_start_pos = None;
                            }
                        }
                    }
                }
            }

            for (i, line) in self.lines.iter().enumerate() {
                painter.add(Shape::line(
                    line.points.clone(),
                    Stroke::new(line.width, line.color),
                ));
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
                painter.add(Shape::line(
                    self.current_line.clone(),
                    Stroke::new(self.brush_size, self.brush_color),
                ));
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
>>>>>>> Stashed changes

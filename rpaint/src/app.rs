use eframe::egui::{self, Color32, Rect, Shape, Stroke, Vec2};
use crate::model::{PaintApp, BrushMode, Line, PaintAction};
use crate::logic::{dist_to_segment};
use crate::ui_tools::{draw_dashed_rect};

impl eframe::App for PaintApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- 1. GESTION DES RACCOURCIS CLAVIERS ---
        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(egui::Key::Z) { self.undo(); }
            if i.modifiers.command && i.key_pressed(egui::Key::Y) { self.redo(); }
            if i.modifiers.command && i.key_pressed(egui::Key::C) { self.copy_selected(); }
            if i.modifiers.command && i.key_pressed(egui::Key::V) { self.paste(); }
            if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) { self.delete_selected(); }
        });

        // --- 2. BARRE D'OUTILS (GAUCHE) ---
        egui::SidePanel::left("toolbar").show(ctx, |ui| {
            ui.heading("🎨 Rust Paint");
            ui.separator();

            ui.label("Édition");
            ui.horizontal(|ui| {
                if ui.button("↩").on_hover_text("Annuler").clicked() { self.undo(); }
                if ui.button("↪").on_hover_text("Rétablir").clicked() { self.redo(); }
                ui.separator();
                if ui.button("✂").on_hover_text("Copier").clicked() { self.copy_selected(); }
                if ui.button("📋").on_hover_text("Coller").clicked() { self.paste(); }
            });

            ui.separator();
            ui.label("Outils");
            ui.selectable_value(&mut self.mode, BrushMode::Freehand, "✏ Dessin");
            ui.selectable_value(&mut self.mode, BrushMode::StraightLine, "📏 Ligne");
            ui.selectable_value(&mut self.mode, BrushMode::Eraser, "🧽 Gomme");
            ui.selectable_value(&mut self.mode, BrushMode::Select, "🖱 Sélection");

            ui.separator();
            ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=50.0).text("Taille"));
            ui.color_edit_button_srgba(&mut self.brush_color);

            // Menu contextuel si sélection active
            if !self.selected_indices.is_empty() {
                ui.separator();
                ui.label(format!("Sélection: {}", self.selected_indices.len()));
                
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
                    
                    if ui.button("📏 Appliquer Taille").clicked() {
                        let old = self.selected_indices.iter().filter_map(|&i| self.lines.get(i).cloned()).collect();
                        let new = self.selected_indices.iter().filter_map(|&i| {
                            let mut l = self.lines.get(i).cloned()?;
                            l.width = self.brush_size;
                            Some(l)
                        }).collect();
                        self.execute(PaintAction::Modify(self.selected_indices.clone(), old, new));
                    }

                    if ui.button("🗑 Supprimer").clicked() { self.delete_selected(); }
                });
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                if ui.add_enabled(!self.lines.is_empty(), egui::Button::new("💣 Tout effacer")).clicked() {
                    self.clear_all();
                }
            });
        });

        // --- 3. ZONE DE DESSIN (CENTRE) ---
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
                            let mut hit = self.selected_indices.iter().find(|&&i| self.get_line_rect(i).contains(pos)).cloned();
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
                                    // Annulation temporaire pour enregistrer le mouvement propre dans l'undo
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

            // --- 4. RENDU FINAL ---

            // Dessiner les lignes stockées
            for (i, line) in self.lines.iter().enumerate() {
                painter.add(Shape::line(line.points.clone(), Stroke::new(line.width, line.color)));
                if self.mode == BrushMode::Select && self.selected_indices.contains(&i) {
                    let r = self.get_line_rect(i);
                    draw_dashed_rect(&painter, r, Color32::WHITE);
                    draw_dashed_rect(&painter, r.expand(1.0), Color32::BLACK);
                }
            }

            // Dessiner le rectangle de sélection bleu transparent
            if let Some(r) = self.selection_rect {
                painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(100, 150, 255, 30));
                painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_rgb(100, 150, 255)));
            }

            // Dessiner la ligne en cours de tracé
            if !self.current_line.is_empty() {
                painter.add(Shape::line(self.current_line.clone(), Stroke::new(self.brush_size, self.brush_color)));
            }

            // Dessiner le curseur de la gomme
            if self.mode == BrushMode::Eraser {
                if let Some(p) = ctx.pointer_latest_pos() {
                    painter.circle_stroke(p, self.brush_size, Stroke::new(1.0, Color32::LIGHT_RED));
                }
            }
        });
    }
}
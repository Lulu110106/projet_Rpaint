use eframe::egui::{self, Color32, Rect, Shape, Stroke, Vec2};
use crate::model::{PaintApp, BrushMode, PaintAction};
use crate::model::Shape as PaintShape;
use crate::logic::{dist_to_segment};
use crate::ui_tools::{draw_dashed_rect};
use crate::server;
use crate::client;
use crate::events::{DrawLineEvent, NetworkEvent};
use crate::model::timestamp_id;
use std::time::Duration;


impl eframe::App for PaintApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // On vide les dessins reçus sur le canal réseau avant de redessiner l'interface.
        let mut received_any = false;
        while let Ok(ev) = self.incoming_draw_rx.try_recv() {
            match ev {
                NetworkEvent::DrawLine(draw) => {
                    let line = draw.to_line();
                    // Upsert par id pour avoir une prévisualisation temps réel
                    // (les updates d'un même trait remplacent la version précédente).
                    if let Some(existing) = self.lines.iter_mut().find(|l| l.id() == line.id()) {
                        *existing = line;
                    } else {
                        // Appliqué sans passer par execute pour ne pas remplir l'undo local.
                        self.apply_action(&PaintAction::Create(vec![line]));
                    }
                    received_any = true;
                }
                NetworkEvent::DeleteLine(id) => {
                    // Trouver la ligne par id et la supprimer (sans enregistrer dans undo).
                    if let Some((idx, _)) = self.lines.iter().enumerate().find(|(_, l)| l.id() == id) {
                        let line = self.lines[idx].clone();
                        self.apply_action(&PaintAction::Delete(vec![idx], vec![line]));
                        received_any = true;
                    }
                }
            }
        }
        // Le canvas doit rester fluide même sans interaction utilisateur.
        ctx.request_repaint_after(Duration::from_millis(16));
        if received_any {
            ctx.request_repaint();
        }

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
            ui.heading("🎨 RPaint");
            ui.separator();
            
            // --- 2.1 MULTI (Début)
            
            ui.heading("📶 Multi");
            if self.client_task.as_ref().is_some_and(|task| task.is_finished()) {
                self.client_task = None;
                self.client_shutdown_tx = None;
                self.outgoing_draw_tx = None;
            }
            let client_connected = self.client_task.is_some();

            // Forcer l'onglet actif selon l'état réseau en cours.
            if self.server_running {
                self.multi_host_mode = true;
            } else if client_connected {
                self.multi_host_mode = false;
            }

            // La sélection host/join n'est libre que tant qu'aucune session n'est active.
            if !self.server_running && !client_connected {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.multi_host_mode, true, "Mode host");
                    ui.selectable_value(&mut self.multi_host_mode, false, "Mode join");
                });
            }
            
            ui.horizontal(|ui| {
                if self.multi_host_mode {
                    ui.vertical(|ui| {
                        ui.label("Pseudo host");
                        ui.text_edit_singleline(&mut self.host_name_input);

                        if self.server_running {
                            ui.label("Serveur en cours de lancement");
                            if ui.button("stop").on_hover_text("Arrêter le serveur").clicked() {
                                if let Some(tx) = self.server_shutdown_tx.take() {
                                    let _ = tx.send(());
                                }
                                if let Some(task) = self.server_task.take() {
                                    task.abort();
                                }
                                server::set_local_draw_sink(None);
                                self.server_running = false;
                            }
                        } else if ui.button("host").on_hover_text("Héberger un canvas").clicked() {
                            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
                            self.server_shutdown_tx = Some(shutdown_tx);
                            self.server_running = true;
                            server::set_local_draw_sink(Some(self.incoming_draw_tx.clone()));
                            let host_name = self.host_name_input.trim().to_owned();
                            let task = tokio::spawn(async move {
                                let name = if host_name.is_empty() { "Host" } else { &host_name };
                                server::run(name, shutdown_rx).await;
                            });
                            self.server_task = Some(task);
                        }
                    });
                } else {
                    ui.vertical(|ui| {
                        ui.label("IP host a joindre");
                        ui.text_edit_singleline(&mut self.join_host_input);
                        ui.label("Pseudo client");
                        ui.text_edit_singleline(&mut self.join_pseudo_input);

                        if client_connected {
                            if ui.button("leave").on_hover_text("Quitter le canvas rejoint").clicked() {
                                if let Some(tx) = self.client_shutdown_tx.take() {
                                    let _ = tx.send(());
                                }
                                if let Some(task) = self.client_task.take() {
                                    task.abort();
                                }
                                self.outgoing_draw_tx = None;
                            }
                        } else if ui.button("join").on_hover_text("Rejoindre un canvas").clicked() {
                            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
                            self.client_shutdown_tx = Some(shutdown_tx);
                            let (outgoing_draw_tx, outgoing_draw_rx) = tokio::sync::mpsc::unbounded_channel();
                            self.outgoing_draw_tx = Some(outgoing_draw_tx);
                            let host_ip = self.join_host_input.trim().to_owned();
                            let pseudo = self.join_pseudo_input.trim().to_owned();
                            let incoming_draw_tx = self.incoming_draw_tx.clone();
                            let task = tokio::spawn(async move {
                                let host = if host_ip.is_empty() { "127.0.0.1" } else { &host_ip };
                                let name = if pseudo.is_empty() { "Guest" } else { &pseudo };
                                client::run(host, name, shutdown_rx, incoming_draw_tx, outgoing_draw_rx).await;
                            });
                            self.client_task = Some(task);
                        }
                    });
                }
            });
            
            

            // --- 2.1 MULTI (Fin)
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
            let palette = [
                egui::Color32::RED,
                egui::Color32::from_rgb(255, 165, 0), // orange
                egui::Color32::YELLOW,
                egui::Color32::GREEN,
                egui::Color32::BLUE,
                egui::Color32::from_rgb(128, 0, 128), // violet
                egui::Color32::BLACK,
                egui::Color32::WHITE,
            ];
            ui.horizontal(|ui| {
                ui.scope(|ui| {
                    ui.spacing_mut().interact_size = egui::vec2(160.0, 20.0);
                    ui.color_edit_button_srgba(&mut self.brush_color);
                });

                // Bouton étoile
                if ui.button("⭐").on_hover_text("Favoris").clicked()
                    && !palette.contains(&self.brush_color)
                    && !self.custom_palette.contains(&self.brush_color)
                {
                    self.custom_palette.push(self.brush_color);
                }
            });
            ui.horizontal_wrapped(|ui| {
                for color in &palette {
                    let size = egui::vec2(24.0, 24.0);
                    let (response, painter) = ui.allocate_painter(size, egui::Sense::click());

                    // Bordure si couleur sélectionnée
                    if self.brush_color == *color {
                        painter.rect_stroke(response.rect, 2.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
                    }

                    painter.rect_filled(response.rect, 2.0, *color);

                    if response.clicked() {
                        self.brush_color = *color;
                    }
                }
            });
            if self.custom_palette.len() != 0 {ui.separator();}
            ui.horizontal_wrapped(|ui| {
                for color in &self.custom_palette {
                    let size = egui::vec2(24.0, 24.0);
                    let (response, painter) = ui.allocate_painter(size, egui::Sense::click());

                    if self.brush_color == *color {
                        painter.rect_stroke(response.rect, 2.0, egui::Stroke::new(2.0, egui::Color32::GRAY));
                    }
                    painter.rect_filled(response.rect, 2.0, *color);

                    if response.clicked() {
                        self.brush_color = *color;
                    }
                }
            });

            // Menu contextuel si sélection active
            if !self.selected_indices.is_empty() {
                ui.separator();
                ui.label(format!("Sélection: {}", self.selected_indices.len()));
                
                ui.vertical_centered_justified(|ui| {
                    if ui.button("🎨 Appliquer Couleur").clicked() {
                        let old = self.selected_indices.iter().filter_map(|&i| self.lines.get(i).cloned()).collect();
                        let new = self.selected_indices.iter().filter_map(|&i| {
                            let mut l = self.lines.get(i).cloned()?;
                            l.set_color(self.brush_color);
                            Some(l)
                        }).collect();
                        self.execute(PaintAction::Modify(self.selected_indices.clone(), old, new));
                    }
                    
                    if ui.button("📏 Appliquer Taille").clicked() {
                        let old = self.selected_indices.iter().filter_map(|&i| self.lines.get(i).cloned()).collect();
                        let new = self.selected_indices.iter().filter_map(|&i| {
                            let mut l = self.lines.get(i).cloned()?;
                            l.set_width(self.brush_size);
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
                            if self.current_line.is_empty() {
                                self.active_stroke_id = Some(timestamp_id());
                                self.current_line.push(pos);
                            }

                            if self.mode == BrushMode::StraightLine {
                                if self.current_line.len() == 1 {
                                    self.current_line.push(pos);
                                } else {
                                    self.current_line[1] = pos;
                                }
                            } else {
                                let should_add = self.current_line
                                    .last()
                                    .map(|p| p.distance_sq(pos) > 0.25)
                                    .unwrap_or(true);
                                if should_add {
                                    self.current_line.push(pos);
                                }
                            }

                            // Envoi incrémental: même id pendant tout le drag => remote temps réel.
                            if let Some(stroke_id) = self.active_stroke_id {
                                let preview = PaintShape::Line {
                                    id: stroke_id,
                                    points: self.current_line.clone(),
                                    color: self.brush_color,
                                    width: self.brush_size,
                                };
                                let ev = NetworkEvent::DrawLine(DrawLineEvent::from_line(&preview));
                                if self.server_running {
                                    let _ = server::publish_network_event(ev);
                                } else if let Some(tx) = self.outgoing_draw_tx.as_ref() {
                                    let _ = tx.send(ev);
                                }
                            }
                        } else if response.drag_released() && !self.current_line.is_empty() {
                            let points = std::mem::take(&mut self.current_line);
                            let line_id = self.active_stroke_id.take().unwrap_or_else(timestamp_id);
                            let line = PaintShape::Line { id: line_id, points, color: self.brush_color, width: self.brush_size };
                            // Enregistrer localement (undo + propagation via execute)
                            self.execute(PaintAction::Create(vec![line.clone()]));
                        }
                    },
                    BrushMode::Eraser => {
                        if response.dragged() || response.clicked() {
                            let mut to_del = None;
                            for (i, line) in self.lines.iter().enumerate() {
                                if line.points().windows(2).any(|w| dist_to_segment(pos, w[0], w[1]) < self.brush_size) {
                                    to_del = Some(i); break;
                                }
                            }
                            if let Some(idx) = to_del {
                                let line = self.lines[idx].clone();
                                // execute gère aussi la propagation réseau du Delete.
                                self.execute(PaintAction::Delete(vec![idx], vec![line]));
                            }
                        }
                    },
                    BrushMode::Select => {
                        if response.drag_started() {
                            let mut hit = self.selected_indices.iter().find(|&&i| self.get_line_rect(i).contains(pos)).cloned();
                            if hit.is_none() {
                                hit = self.lines.iter().enumerate().find(|(_, l)| 
                                    l.points().windows(2).any(|w| dist_to_segment(pos, w[0], w[1]) < 10.0)).map(|(i, _)| i);
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
                                    if let Some(l) = self.lines.get_mut(idx) {
                                        for p in l.points_mut() { *p += delta; }
                                        // Envoyer la position mise à jour en temps réel.
                                        let ev = NetworkEvent::DrawLine(DrawLineEvent::from_line(l));
                                        if self.server_running {
                                            let _ = server::publish_network_event(ev);
                                        } else if let Some(tx) = self.outgoing_draw_tx.as_ref() {
                                            let _ = tx.send(ev);
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
                                    // Annulation temporaire pour enregistrer le mouvement propre dans l'undo
                                    for &idx in &self.selected_indices {
                                        if let Some(l) = self.lines.get_mut(idx) { for p in l.points_mut() { *p -= total; } }
                                    }
                                    self.execute(PaintAction::Move(self.selected_indices.clone(), total));
                                }
                                self.is_dragging_items = false;
                            } else if let Some(rect) = self.selection_rect.take() {
                                self.selected_indices = self.lines.iter().enumerate()
                                    .filter(|(_, l)| l.points().iter().any(|p| rect.contains(*p)))
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
                painter.add(egui::Shape::line(line.points().clone(), Stroke::new(line.width(), line.color())));
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

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(tx) = self.server_shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.server_task.take() {
            task.abort();
        }
        server::set_local_draw_sink(None);
        if let Some(task) = self.client_task.take() {
            task.abort();
        }
        if let Some(tx) = self.client_shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.outgoing_draw_tx = None;
        self.server_running = false;
    }
}
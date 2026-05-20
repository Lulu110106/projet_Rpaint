use eframe::egui::{self, Color32, Rect, Shape, Stroke, Vec2};
use crate::model::{Camera, PaintApp, BrushMode, PaintAction, PaintProject, ShapeKind, SelectionMode};
use crate::model::Shape as PaintShape;
use crate::ui_tools::{draw_dashed_rect, draw_ellipse, draw_regular_polygon, draw_star, draw_arrow, draw_lasso, is_shape_in_lasso};
use crate::server;
use crate::client;
use crate::events::{DrawShapeEvent, NetworkEvent};
use crate::model::timestamp_id;
use rfd::FileDialog;
use std::time::Duration;


impl eframe::App for PaintApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // On vide les dessins reçus sur le canal réseau avant de redessiner l'interface.
        let mut received_any = false;
        while let Ok(ev) = self.incoming_draw_rx.try_recv() {
            match ev {
                NetworkEvent::SyncProject { project } => {
                    self.replace_project(project);
                    received_any = true;
                }
                NetworkEvent::SessionStatus { message } => {
                    self.join_error_message = message;
                    received_any = true;
                }
                NetworkEvent::DrawShape(draw) => {
                    let shape = draw.to_shape();
                    // Ajouter au layer actif
                    // Upsert par id pour avoir une prévisualisation temps réel
                    // (les updates d'un même trait remplacent la version précédente).
                    if let Some(active_layer) = self.layer_manager.get_active_layer_mut() {
                        if let Some(existing) = active_layer.elements.iter_mut().find(|l| l.id() == shape.id()) {
                            *existing = shape;
                        } else {
                            // Appliqué sans passer par execute pour ne pas remplir l'undo local.
                            self.apply_action(&PaintAction::Create(vec![shape]));
                        }
                    }
                    received_any = true;
                }
                NetworkEvent::DeleteShape(id) => {
                    if let Some(active_layer) = self.layer_manager.get_active_layer() {
                        if let Some((idx, _)) = active_layer.elements.iter().enumerate().find(|(_, l)| l.id() == id) {
                            let shape = active_layer.elements[idx].clone();
                            self.apply_action(&PaintAction::Delete(vec![idx], vec![shape]));
                            received_any = true;
                        }
                    }
                }
                // Gestion des événements de layers
                NetworkEvent::CreateLayer { id, name, position } => {
                    if self.layer_manager.get_layer(id).is_none() {
                        self.apply_action(&PaintAction::NetworkCreateLayer { id, name: name.clone(), position });
                    }
                    received_any = true;
                }
                NetworkEvent::DeleteLayer { id } => {
                    self.apply_action(&PaintAction::NetworkDeleteLayer { id });
                    received_any = true;
                }
                NetworkEvent::RenameLayer { id, name } => {
                    self.apply_action(&PaintAction::NetworkRenameLayer { id, name: name.clone() });
                    received_any = true;
                }
                NetworkEvent::SetLayerVisibility { id, visible } => {
                    self.apply_action(&PaintAction::NetworkSetLayerVisibility { id, visible });
                    received_any = true;
                }
                NetworkEvent::SetActiveLayer { id } => {
                    self.apply_action(&PaintAction::NetworkSetActiveLayer { id });
                    received_any = true;
                }
                NetworkEvent::ReorderLayers { from_idx, to_idx } => {
                    self.apply_action(&PaintAction::NetworkReorderLayers { from_idx, to_idx });
                    received_any = true;
                }
            }
        }
        while let Ok(status) = self.net_status_rx.try_recv() {
            if let Some(endpoint) = status.strip_prefix("UPNP_ENDPOINT:") {
                self.upnp_public_endpoint = endpoint.to_string();
                self.upnp_status = "Port forwarding actif".to_string();
                if let Some((ip, port)) = endpoint.rsplit_once(':') {
                    self.join_ip_input = ip.to_string();
                    self.join_port_input = port.to_string();
                } else {
                    self.join_ip_input = endpoint.to_string();
                }
            } else if let Some(port) = status.strip_prefix("UPNP_MAPPED_PORT:") {
                self.upnp_mapped_port = port.parse::<u16>().ok();
            } else if let Some(msg) = status.strip_prefix("UPNP_STATUS:") {
                self.upnp_status = msg.to_string();
            }
        }
        // Le canvas doit rester fluide même sans interaction utilisateur.
        ctx.request_repaint_after(Duration::from_millis(16));
        if received_any {
            ctx.request_repaint();
        }

        // --- 1. GESTION DES RACCOURCIS CLAVIERS ---
        ctx.input(|i| {
            if i.modifiers.command && (i.key_pressed(egui::Key::Y) || (i.modifiers.shift && i.key_pressed(egui::Key::Z))) { self.redo(); }
            else if i.modifiers.command && i.key_pressed(egui::Key::Z) { self.undo(); }
            else if i.modifiers.command && i.key_pressed(egui::Key::C) { self.copy_selected(); }
            else if i.modifiers.command && i.key_pressed(egui::Key::V) { self.paste(); }
            else if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) { self.delete_selected(); }
        });

        // --- Menu principal (Top) ---
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Fichier", |ui| {
                    if ui.button("Quitter").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Aide", |ui| {
                    if ui.button("Afficher l'aide").clicked() {
                        self.show_help = true;
                        ui.close_menu();
                    }
                });
            });
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
            if self.server_task.as_ref().is_some_and(|task| task.is_finished()) {
                self.server_task = None;
                self.server_shutdown_tx = None;
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
                ui.checkbox(&mut self.multi_use_upnp, "WAN auto-forward (UPnP)");
            }
            
            ui.horizontal(|ui| {
                if self.multi_host_mode {
                    ui.vertical(|ui| {
                        ui.label("Pseudo host");
                        ui.add(egui::TextEdit::singleline(&mut self.host_name_input).hint_text("entrer un pseudo"));
                        if self.server_running && !self.host_local_endpoint.is_empty() {
                            ui.label(format!("IP locale: {}", self.host_local_endpoint));
                        }
                        if !self.upnp_public_endpoint.is_empty() {
                            ui.label(format!("IP public: {}", self.upnp_public_endpoint));
                        }
                        if !self.upnp_status.is_empty() {
                            ui.label(format!("UPnP: {}", self.upnp_status));
                        }
                        if !self.host_error_message.is_empty() {
                            ui.colored_label(egui::Color32::RED, &self.host_error_message);
                        }
                        

                        if self.server_running {
                            ui.label("Serveur en cours de lancement");
                            if ui.button("stop").on_hover_text("Arrêter le serveur").clicked() {
                                if let Some(tx) = self.server_shutdown_tx.take() {
                                    let _ = tx.send(());
                                }
                                if let Some(port) = self.upnp_mapped_port.take() {
                                    let status_tx = self.net_status_tx.clone();
                                    tokio::spawn(async move {
                                        match server::disable_upnp_port_forward(port).await {
                                            Ok(_) => {
                                                let _ = status_tx.send("UPNP_STATUS:Port forwarding supprimé".to_string());
                                            }
                                            Err(err) => {
                                                let _ = status_tx.send(format!("UPNP_STATUS:Erreur fermeture mapping: {err}"));
                                            }
                                        }
                                    });
                                }
                                server::set_local_draw_sink(None);
                                self.server_running = false;
                                self.host_local_endpoint.clear();
                            }
                        } else if ui.button("host").on_hover_text("Héberger un canvas").clicked() {
                            if self.host_name_input.trim().is_empty() {
                                self.host_error_message = "Veuillez entrer un pseudo".to_string();
                            } else {
                                self.host_error_message.clear();
                                let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
                                self.server_shutdown_tx = Some(shutdown_tx);
                                self.server_running = true;
                                self.host_local_endpoint = server::local_endpoint(3000)
                                    .unwrap_or_else(|| "IP locale indisponible".to_string());
                                self.upnp_public_endpoint.clear();
                                self.upnp_status.clear();
                                server::set_local_draw_sink(Some(self.incoming_draw_tx.clone()));
                                let host_name = self.host_name_input.trim().to_owned();
                                let project = PaintProject {
                                    layer_manager: (&self.layer_manager).into(),
                                };
                                let task = tokio::spawn(async move {
                                    let name = if host_name.is_empty() { "Host" } else { &host_name };
                                    server::run(name, project, shutdown_rx).await;
                                });
                                self.server_task = Some(task);

                                if self.multi_use_upnp {
                                    let status_tx = self.net_status_tx.clone();
                                    tokio::spawn(async move {
                                        match server::enable_upnp_port_forward(3000).await {
                                            Ok((external_ip, port)) => {
                                                let _ = status_tx.send(format!("UPNP_MAPPED_PORT:{port}"));
                                                let _ = status_tx.send(format!("UPNP_ENDPOINT:{external_ip}:{port}"));
                                            }
                                            Err(err) => {
                                                let _ = status_tx.send(format!("UPNP_STATUS:{err}"));
                                            }
                                        }
                                    });
                                }
                            }
                        }
                    });
                } else {
                    ui.vertical(|ui| {
                        ui.label("IP host à joindre");
                        ui.text_edit_singleline(&mut self.join_ip_input);
                        ui.label("Port");
                        ui.text_edit_singleline(&mut self.join_port_input);
                        
                        ui.label("Pseudo client");
                        ui.add(egui::TextEdit::singleline(&mut self.join_pseudo_input).hint_text("entrer un pseudo"));
                        if !self.join_error_message.is_empty() {
                            ui.colored_label(egui::Color32::RED, &self.join_error_message);
                        }
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
                            if self.join_pseudo_input.trim().is_empty() {
                                self.join_error_message = "Veuillez entrer un pseudo".to_string();
                            } else {
                                self.join_error_message.clear();
                                let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
                                self.client_shutdown_tx = Some(shutdown_tx);
                                let (outgoing_draw_tx, outgoing_draw_rx) = tokio::sync::mpsc::unbounded_channel();
                                self.outgoing_draw_tx = Some(outgoing_draw_tx);
                                let host_ip = self.join_ip_input.trim().to_owned();
                                let host_port = self.join_port_input.trim().parse::<u16>().unwrap_or(3000);
                                let pseudo = self.join_pseudo_input.trim().to_owned();
                                let incoming_draw_tx = self.incoming_draw_tx.clone();
                                let task = tokio::spawn(async move {
                                    let host = if host_ip.is_empty() { "127.0.0.1" } else { &host_ip };
                                    let name = if pseudo.is_empty() { "Guest" } else { &pseudo };
                                    client::run(host, host_port, name, shutdown_rx, incoming_draw_tx, outgoing_draw_rx).await;
                                });
                                self.client_task = Some(task);
                            }
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
            ui.horizontal(|ui| {
                ui.label("Chemin :");
                ui.add(egui::TextEdit::singleline(&mut self.save_load_file_path).desired_width(180.0));
                if ui.button("📁").on_hover_text("Choisir un fichier").clicked() {
                    let mut dialog = FileDialog::new().add_filter("RPaint", &["rpaint"]);
                    let path = std::path::Path::new(&self.save_load_file_path);
                    if let Some(parent) = path.parent() {
                        dialog = dialog.set_directory(parent);
                    }
                    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                        dialog = dialog.set_file_name(file_name);
                    }
                    if let Some(path) = dialog.save_file() {
                        if let Some(path_str) = path.to_str() {
                            let selected_path = path_str.to_string();
                            self.save_load_file_path = selected_path.clone();
                            if let Err(err) = self.save_project(&selected_path) {
                                self.save_load_status = format!("Erreur sauvegarde : {}", err);
                            }
                        }
                    }
                }
            });
            let project_path = self.save_load_file_path.clone();
            ui.horizontal(|ui| {
                if ui.button("💾 Sauvegarder").on_hover_text("Sauvegarder le canvas").clicked() {
                    if let Err(err) = self.save_project(&project_path) {
                        self.save_load_status = format!("Erreur sauvegarde : {}", err);
                    }
                }
                if ui.button("📂 Charger").on_hover_text("Charger un canvas").clicked() {
                    if let Err(err) = self.load_project(&project_path) {
                        self.save_load_status = format!("Erreur chargement : {}", err);
                    }
                }

                if ui.button("📷 Export PNG").on_hover_text("Exporter le canvas en PNG").clicked() {
                    // export to saves/<name>.png
                    let png_name = if project_path.ends_with(".rpaint") {
                        project_path.trim_end_matches(".rpaint").to_string()
                    } else {
                        project_path.clone()
                    };
                    let png_file = format!("{}.png", png_name);
                    if let Err(err) = self.export_png(&png_file) {
                        self.save_load_status = format!("Erreur export PNG : {}", err);
                    }
                }
            });
            if !self.save_load_status.is_empty() {
                ui.label(&self.save_load_status);
            }

            ui.separator();
            ui.label("Outils");
            ui.selectable_value(&mut self.mode, BrushMode::Freehand, "✏ Dessin");
            ui.selectable_value(&mut self.mode, BrushMode::Shape, format!("🔺 Forme: {}", self.selected_shape.label()));
            ui.menu_button("Changer forme", |ui| {
                let kinds = [
                    ShapeKind::Line,
                    ShapeKind::Rectangle,
                    ShapeKind::Oval,
                    ShapeKind::Triangle,
                    ShapeKind::Pentagon,
                    ShapeKind::Hexagon,
                    ShapeKind::Octagon,
                    ShapeKind::Star,
                    ShapeKind::Arrow,
                ];
                for kind in kinds {
                    if ui.selectable_value(&mut self.selected_shape, kind, kind.label()).clicked() {
                        self.mode = BrushMode::Shape;
                    }
                }
            });
            ui.selectable_value(&mut self.mode, BrushMode::Eraser, "🧽 Gomme");
            ui.selectable_value(&mut self.mode, BrushMode::Select, format!("🖱 Sélection: {}", self.selection_mode.label()));
            ui.menu_button("Changer mode sélection", |ui| {
                if ui.selectable_value(&mut self.selection_mode, SelectionMode::Rectangle, "Rectangle").clicked() {
                    self.mode = BrushMode::Select;
                }
                if ui.selectable_value(&mut self.selection_mode, SelectionMode::Lasso, "Lasso").clicked() {
                    self.mode = BrushMode::Select;
                }
            });

            ui.separator();
            ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=50.0).text("Taille"));
            ui.horizontal(|ui| {
                if ui.button("➖").clicked() {
                    self.camera.zoom_at(1.0 / 1.2, ui.min_rect().center());
                }
                ui.label(format!("Zoom: {}%", (self.camera.zoom * 100.0).round()));
                if ui.button("➕").clicked() {
                    self.camera.zoom_at(1.2, ui.min_rect().center());
                }
                if ui.button("🔄").on_hover_text("Réinitialiser zoom").clicked() {
                    self.camera = Camera::new();
                }
            });
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
                    self.custom_palette.insert(0, self.brush_color); // Ajoute au début
                    if self.custom_palette.len() > 12 {
                        self.custom_palette.pop(); // Supprime le dernier
                    }
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
            let colors: Vec<egui::Color32> = self.custom_palette.clone();
            ui.horizontal_wrapped(|ui| {
                let mut clicked_color: Option<egui::Color32> = None;

                for color in &colors {
                    let size = egui::vec2(24.0, 24.0);
                    let (response, painter) = ui.allocate_painter(size, egui::Sense::click());
                    if self.brush_color == *color {
                        painter.rect_stroke(response.rect, 2.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
                    }
                    painter.rect_filled(response.rect, 2.0, *color);
                    if response.clicked() {
                        clicked_color = Some(*color);
                    }
                }

                // Mutation après la boucle, borrow immutable libéré
                if let Some(color) = clicked_color {
                    if let Some(pos) = self.custom_palette.iter().position(|c| *c == color) {
                        self.custom_palette.remove(pos);
                        self.custom_palette.insert(0, color);
                    }
                    self.brush_color = color;
                }
            });

            // Menu contextuel si sélection active
            if !self.selected_indices.is_empty() {
                ui.separator();
                ui.label(format!("Sélection: {}", self.selected_indices.len()));
                
                ui.vertical_centered_justified(|ui| {
                    if ui.button("🎨 Appliquer Couleur").clicked() {
                        if let Some(layer) = self.layer_manager.get_active_layer() {
                            let old: Vec<_> = self.selected_indices.iter().filter_map(|&i| layer.elements.get(i).cloned()).collect();
                            let new: Vec<_> = self.selected_indices.iter().filter_map(|&i| {
                                let mut s = layer.elements.get(i).cloned()?;
                                s.set_color(self.brush_color);
                                Some(s)
                            }).collect();
                            self.execute(PaintAction::Modify(self.selected_indices.clone(), old, new));
                        }
                    }
                    
                    if ui.button("📏 Appliquer Taille").clicked() {
                        if let Some(layer) = self.layer_manager.get_active_layer() {
                            let old: Vec<_> = self.selected_indices.iter().filter_map(|&i| layer.elements.get(i).cloned()).collect();
                            let new: Vec<_> = self.selected_indices.iter().filter_map(|&i| {
                                let mut s = layer.elements.get(i).cloned()?;
                                s.set_width(self.brush_size);
                                Some(s)
                            }).collect();
                            self.execute(PaintAction::Modify(self.selected_indices.clone(), old, new));
                        }
                    }

                    if ui.button("🗑 Supprimer").clicked() { self.delete_selected(); }
                });
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                let can_clear = self.layer_manager.get_active_layer().map_or(false, |l| !l.elements.is_empty());
                if ui.add_enabled(can_clear, egui::Button::new("💣 Tout effacer")).clicked() {
                    self.clear_all();
                }
            });
        });

        // --- 5. PANNEAU DES LAYERS (DROITE) ---
        egui::SidePanel::right("layers_panel")
            .resizable(true)
            .default_width(170.0)
            .show(ctx, |ui| {
                ui.heading("📚 Layers");
                ui.separator();
                // Bouton pour créer un nouveau layer
                if ui.button("➕ Nouveau layer").clicked() {
                    self.create_new_layer();
                }

                // Afficher tous les layers
                let mut layer_to_delete = None;
                let mut layer_rename = None;
                let mut layer_visibility_toggle = None;
                let mut reorder_from = None;
                let mut reorder_to = None;
                let mut layer_to_select = None;

                egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                for (idx, layer) in self.layer_manager.layers.iter().enumerate() {
                    let is_active = layer.id == self.layer_manager.active_layer_id;
                    let bg_color = if is_active {
                        Color32::from_rgb(50, 100, 150)
                    } else {
                        Color32::from_rgb(40, 40, 40)
                    };

                    let frame = egui::Frame::default()
                        .fill(bg_color)
                        .inner_margin(4.0)
                        .outer_margin(4.0);

                    let layer_id = layer.id;
                    let layer_name = layer.name.clone();
                    let layer_visible = layer.visible;

                    let mut preview_rect = None;
                    let _frame_response = frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Aperçu du layer (miniature du contenu)
                            let (response, painter) = ui.allocate_painter(
                                egui::vec2(64.0, 64.0),
                                egui::Sense::click_and_drag(),
                            );
                            preview_rect = Some(response.rect);

                            // Fond de la preview
                            painter.rect_filled(response.rect, 2.0, Color32::from_rgb(60, 60, 60));

                            // Dessiner une miniature du contenu du layer
                            if !layer.elements.is_empty() {
                                let mut bounds = Rect::NOTHING;
                                for element in &layer.elements {
                                    bounds = bounds.union(element.bounding_rect());
                                }

                                if bounds.is_finite() && bounds.width() > 0.0 && bounds.height() > 0.0 {
                                    let padding = 6.0;
                                let preview_area = response.rect.shrink(padding);
                                let scale = (preview_area.width() / bounds.width()).min(preview_area.height() / bounds.height());
                                let to_preview_pos = |p: egui::Pos2| preview_area.min + (p - bounds.min) * scale;

                                for element in &layer.elements {
                                    let stroke = Stroke::new((element.width() * scale).max(0.5), element.color());
                                    
                                    #[allow(unreachable_patterns)] // cest pas grave si les nouveaux sont pas dans preview
                                    match element {
                                        PaintShape::Line { points, .. } => {
                                            let scaled_points: Vec<egui::Pos2> = points.iter()
                                                .map(|p| to_preview_pos(*p))
                                                .collect();
                                            if scaled_points.len() >= 2 {
                                                painter.add(Shape::line(scaled_points, stroke));
                                            } else if let Some(point) = scaled_points.first() {
                                                painter.circle_stroke(*point, 1.0, stroke);
                                            }
                                        }
                                        PaintShape::Rectangle { start, end, .. } => {
                                            let scaled_start = to_preview_pos(*start);
                                            let scaled_end = to_preview_pos(*end);
                                            painter.rect_stroke(Rect::from_two_pos(scaled_start, scaled_end), 0.0, stroke);
                                        }
                                        PaintShape::Oval { start, end, .. } => {
                                            let scaled_start = to_preview_pos(*start);
                                            let scaled_end = to_preview_pos(*end);
                                            draw_ellipse(&painter, Rect::from_two_pos(scaled_start, scaled_end), stroke);
                                        }
                                        PaintShape::RegularPolygon { start, end, sides, .. } => {
                                            let scaled_start = to_preview_pos(*start);
                                            let scaled_end = to_preview_pos(*end);
                                            draw_regular_polygon(&painter, Rect::from_two_pos(scaled_start, scaled_end), *sides as usize, stroke);
                                        }
                                        PaintShape::Star { start, end, points, .. } => {
                                            let scaled_start = to_preview_pos(*start);
                                            let scaled_end = to_preview_pos(*end);
                                            draw_star(&painter, Rect::from_two_pos(scaled_start, scaled_end), *points as usize, stroke);
                                        }
                                        PaintShape::Arrow { start, end, .. } => {
                                            let scaled_start = to_preview_pos(*start);
                                            let scaled_end = to_preview_pos(*end);
                                            draw_arrow(&painter, scaled_start, scaled_end, stroke);
                                        }
                                        _ => print!("Shape type not supported in the layer preview at this time. ")
                                    }
                                }
                                }
                            }

                            // Afficher le compteur d'elements en overlay
                            if !layer.elements.is_empty() {
                                painter.text(
                                    response.rect.right_bottom() + eframe::egui::vec2(-2.0, -2.0),
                                    eframe::egui::Align2::RIGHT_BOTTOM,
                                    format!("{}", layer.elements.len()),
                                    eframe::egui::FontId::proportional(10.0),
                                    Color32::WHITE,
                                );
                            }

                            ui.vertical_centered(|ui| {
                                // Nom du layer (editable)
                                if self.layers_panel_rename_id == Some(layer_id) {
                                    let mut text = self.layers_panel_rename_text.clone();
                                    let response = ui.text_edit_singleline(&mut text);
                                    self.layers_panel_rename_text = text;
                                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                        self.layers_panel_rename_id = None;
                                    }
                                    else if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                        if !self.layers_panel_rename_text.trim().is_empty() {
                                            layer_rename = Some((layer_id, self.layers_panel_rename_text.clone()));
                                        }
                                        self.layers_panel_rename_id = None;
                                    }
                                } else {
                                    let response = ui.add(egui::Label::new(&layer_name).sense(egui::Sense::click()));
                                    if response.double_clicked() {
                                        self.layers_panel_rename_id = Some(layer_id);
                                        self.layers_panel_rename_text = layer_name.clone();
                                    }
                                }

                                ui.horizontal(|ui| {
                                    // Bouton visibilite (oeil)
                                    let eye_icon = if layer_visible { "👁✅" } else { "👁❌" };
                                    if ui.button(eye_icon).clicked() {
                                        layer_visibility_toggle = Some(layer_id);
                                    }

                                    // Bouton supprimer
                                    if ui.button("🗑").clicked() {
                                        layer_to_delete = Some(layer_id);
                                    }
                                });
                            });
                        });
                    });
                    if let Some(preview_rect) = preview_rect {
                        let click_response = ui.allocate_rect(preview_rect, egui::Sense::click_and_drag());

                        if click_response.clicked() {
                            self.layers_panel_rename_id = None;

                            layer_to_select = Some(layer_id);
                        }

                        if click_response.drag_started() {
                            self.layers_panel_rename_id = None;

                            self.layers_drag_source = Some(idx);
                        }
                        if click_response.hovered() && self.layers_drag_source.is_some() && click_response.drag_released() {
                            if let Some(from_idx) = self.layers_drag_source.take() {
                                if from_idx != idx {
                                    reorder_from = Some(from_idx);
                                    reorder_to = Some(idx);
                                }
                            }
                        }
                    }
                }
                });

                // Effectuer les actions différées
                if let Some((layer_id, new_name)) = layer_rename {
                    self.rename_layer(layer_id, new_name);
                }
                if let Some(layer_id) = layer_to_select {
                    self.set_active_layer(layer_id);
                }
                if let Some(layer_id) = layer_to_delete {
                    self.delete_layer(layer_id);
                }
                if let Some(layer_id) = layer_visibility_toggle {
                    self.toggle_layer_visibility(layer_id);
                }
                if let (Some(from), Some(to)) = (reorder_from, reorder_to) {
                    self.reorder_layers(from, to);
                }
            });

        // Fenêtre d'aide
        if self.show_help {
            let mut show_help = self.show_help;
            let mut close_requested = false;
            egui::Window::new("Aide — RPaint").open(&mut show_help).show(ctx, |ui| {
                ui.label("Bienvenue dans RPaint — guide rapide pour commencer :");
                ui.separator();
                ui.heading("Outils");
                ui.label("- Sélectionnez les outils depuis la barre de gauche.");
                ui.label("- Dessiner : choisissez '✏ Dessin' puis tracez avec la souris.");
                ui.label("- Formes : utilisez 'Changer forme' pour sélectionner une forme.");
                ui.separator();
                ui.heading("Sélection & édition");
                ui.label("- Copier/Coller : Ctrl+C, Ctrl+V");
                ui.label("- Annuler/Rétablir : Ctrl+Z, Ctrl+Shift+Z");
                ui.label("- Supprimer : touche Delete / Backspace");
                ui.separator();
                ui.heading("Réseau (Multi)");
                ui.label("- Mode host : entrez un pseudo et cliquez 'host' pour héberger un canvas.");
                ui.label("- Mode join : entrez une IP, un port, un pseudo et cliquez 'join' pour vous connecter.");
                ui.label("- Le pseudo est obligatoire pour lancer host ou join.");
                ui.separator();
                if ui.button("Fermer").clicked() { close_requested = true; }
            });
            self.show_help = show_help && !close_requested;
        }

        // --- 3. ZONE DE DESSIN (CENTRE) ---
        egui::CentralPanel::default().show(ctx, |ui| {
            // Calculer la zone de dessin en tenant compte du panel des layers
            let available_size = ui.available_size();
            let drawing_size = egui::vec2(available_size.x, available_size.y);
            let (response, painter) = ui.allocate_painter(drawing_size, egui::Sense::click_and_drag());
            let canvas_rect = response.rect;
            let to_world = |camera: &Camera, p: egui::Pos2| camera.to_world(egui::Pos2::new(p.x - canvas_rect.min.x, p.y - canvas_rect.min.y));
            let to_screen = |camera: &Camera, p: egui::Pos2| canvas_rect.min + camera.to_screen(p).to_vec2();
            let delta_to_world = |d: egui::Vec2, zoom: f32| d / zoom;
            let rect_to_screen = |camera: &Camera, r: egui::Rect| Rect::from_two_pos(to_screen(camera, r.min), to_screen(camera, r.max));
            let pointer = response.interact_pointer_pos();

            if response.hovered() {
                let scroll = ctx.input(|i| i.scroll_delta.y);
                if scroll != 0.0 {
                    let factor = 1.15_f32.powf(scroll.signum());
                    if let Some(cursor) = response.hover_pos() {
                        self.camera.zoom_at(
                            factor,
                            egui::Pos2::new(cursor.x - canvas_rect.min.x, cursor.y - canvas_rect.min.y),
                        );
                    }
                }
            }

            let is_pan = response.dragged_by(egui::PointerButton::Middle) || response.dragged_by(egui::PointerButton::Secondary);
            if is_pan {
                self.camera.offset += response.drag_delta();
            }

            if let Some(screen_pos) = pointer {
                let pos = to_world(&self.camera, screen_pos);
                match self.mode {
                    BrushMode::Freehand | BrushMode::Shape => {
                        if response.dragged() && !is_pan {
                            if self.current_line.is_empty() {
                                self.active_stroke_id = Some(timestamp_id());
                                self.current_line.push(pos);
                            }

                            if self.mode == BrushMode::Shape {
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

                            if let Some(stroke_id) = self.active_stroke_id {
                                let preview = if self.mode == BrushMode::Freehand {
                                    PaintShape::Line {
                                        id: stroke_id,
                                        points: self.current_line.clone(),
                                        color: self.brush_color,
                                        width: self.brush_size,
                                    }
                                } else {
                                    let start = self.current_line[0];
                                    let end = self.current_line[1];
                                    match self.selected_shape {
                                        ShapeKind::Line => PaintShape::Line {
                                            id: stroke_id,
                                            points: self.current_line.clone(),
                                            color: self.brush_color,
                                            width: self.brush_size,
                                        },
                                        ShapeKind::Rectangle => PaintShape::Rectangle {
                                            id: stroke_id,
                                            start,
                                            end,
                                            color: self.brush_color,
                                            width: self.brush_size,
                                        },
                                        ShapeKind::Oval => PaintShape::Oval {
                                            id: stroke_id,
                                            start,
                                            end,
                                            color: self.brush_color,
                                            width: self.brush_size,
                                        },
                                        ShapeKind::Triangle
                                        | ShapeKind::Pentagon
                                        | ShapeKind::Hexagon
                                        | ShapeKind::Octagon => PaintShape::RegularPolygon {
                                            id: stroke_id,
                                            start,
                                            end,
                                            sides: self.selected_shape.sides(),
                                            color: self.brush_color,
                                            width: self.brush_size,
                                        },
                                        ShapeKind::Star => PaintShape::Star {
                                            id: stroke_id,
                                            start,
                                            end,
                                            points: 5,
                                            color: self.brush_color,
                                            width: self.brush_size,
                                        },
                                        ShapeKind::Arrow => PaintShape::Arrow {
                                            id: stroke_id,
                                            start,
                                            end,
                                            color: self.brush_color,
                                            width: self.brush_size,
                                        },
                                    }
                                };
                                let ev = NetworkEvent::DrawShape(DrawShapeEvent::from_shape(&preview));
                                if self.server_running {
                                    let _ = server::publish_network_event(ev);
                                } else if let Some(tx) = self.outgoing_draw_tx.as_ref() {
                                    let _ = tx.send(ev);
                                }
                            }
                        } else if response.drag_released() && !self.current_line.is_empty() {
                            let points = std::mem::take(&mut self.current_line);
                            let shape_id = self.active_stroke_id.take().unwrap_or_else(timestamp_id);
                            let shape = if self.mode == BrushMode::Freehand {
                                PaintShape::Line {
                                    id: shape_id,
                                    points,
                                    color: self.brush_color,
                                    width: self.brush_size,
                                }
                            } else {
                                let start = points[0];
                                let end = points.get(1).cloned().unwrap_or(start);
                                match self.selected_shape {
                                    ShapeKind::Line => PaintShape::Line {
                                        id: shape_id,
                                        points,
                                        color: self.brush_color,
                                        width: self.brush_size,
                                    },
                                    ShapeKind::Rectangle => PaintShape::Rectangle {
                                        id: shape_id,
                                        start,
                                        end,
                                        color: self.brush_color,
                                        width: self.brush_size,
                                    },
                                    ShapeKind::Oval => PaintShape::Oval {
                                        id: shape_id,
                                        start,
                                        end,
                                        color: self.brush_color,
                                        width: self.brush_size,
                                    },
                                    ShapeKind::Triangle
                                    | ShapeKind::Pentagon
                                    | ShapeKind::Hexagon
                                    | ShapeKind::Octagon => PaintShape::RegularPolygon {
                                        id: shape_id,
                                        start,
                                        end,
                                        sides: self.selected_shape.sides(),
                                        color: self.brush_color,
                                        width: self.brush_size,
                                    },
                                    ShapeKind::Star => PaintShape::Star {
                                        id: shape_id,
                                        start,
                                        end,
                                        points: 5,
                                        color: self.brush_color,
                                        width: self.brush_size,
                                    },
                                    ShapeKind::Arrow => PaintShape::Arrow {
                                        id: shape_id,
                                        start,
                                        end,
                                        color: self.brush_color,
                                        width: self.brush_size,
                                    },
                                }
                            };
                            self.execute(PaintAction::Create(vec![shape]));
                        }
                    },
                    BrushMode::Eraser => {
                        if response.dragged() || response.clicked() {
                            let mut to_del = None;
                            if let Some(layer) = self.layer_manager.get_active_layer() {
                                for (i, shape) in layer.elements.iter().enumerate() {
                                    if shape.distance_to(pos) < self.brush_size {
                                        to_del = Some(i);
                                        break;
                                    }
                                }
                            }
                            if let Some(idx) = to_del {
                                if let Some(layer) = self.layer_manager.get_active_layer() {
                                    if let Some(shape) = layer.elements.get(idx) {
                                        let s = shape.clone();
                                        self.execute(PaintAction::Delete(vec![idx], vec![s]));
                                    }
                                }
                            }
                        }
                    },
                    BrushMode::Select => {
                        match self.selection_mode {
                            SelectionMode::Rectangle => {
                                // Ancienne logique : rectangle de sélection
                                if response.drag_started() {
                                    let mut hit = self.selected_indices.iter().find(|&&i| self.get_shape_rect(i).contains(pos)).cloned();
                                    if hit.is_none() {
                                        if let Some(layer) = self.layer_manager.get_active_layer() {
                                            hit = layer.elements.iter().enumerate().find(|(_, l)| 
                                                l.distance_to(pos) < 10.0
                                            ).map(|(i, _)| i);
                                        }
                                    }
                                    if let Some(idx) = hit {
                                        if !self.selected_indices.contains(&idx) { self.selected_indices = vec![idx]; }
                                        self.is_dragging_items = true;
                                        self.drag_accumulated_delta = Vec2::ZERO;
                                    } else {
                                        self.selection_start_pos = Some(pos);
                                        self.selected_indices.clear();
                                        self.current_lasso.clear();
                                    }
                                }
                                if response.dragged() {
                                    if self.is_dragging_items {
                                        let delta = delta_to_world(response.drag_delta(), self.camera.zoom);
                                        self.drag_accumulated_delta += delta;
                                        if let Some(layer) = self.layer_manager.get_active_layer_mut() {
                                            for &idx in &self.selected_indices {
                                                if let Some(shape) = layer.elements.get_mut(idx) {
                                                    shape.translate(delta);
                                                    let ev = NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape));
                                                    if self.server_running {
                                                        let _ = server::publish_network_event(ev);
                                                    } else if let Some(tx) = self.outgoing_draw_tx.as_ref() {
                                                        let _ = tx.send(ev);
                                                    }
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
                                            if let Some(layer) = self.layer_manager.get_active_layer_mut() {
                                                for &idx in &self.selected_indices {
                                                    if let Some(shape) = layer.elements.get_mut(idx) {
                                                        shape.translate(-total);
                                                    }
                                                }
                                            }
                                            self.execute(PaintAction::Move(self.selected_indices.clone(), total));
                                        }
                                        self.is_dragging_items = false;
                                    } else if let Some(rect) = self.selection_rect.take() {
                                        if let Some(layer) = self.layer_manager.get_active_layer() {
                                            self.selected_indices = layer.elements.iter().enumerate()
                                                .filter(|(_, l)| l.bounding_rect().intersects(rect))
                                                .map(|(i, _)| i).collect();
                                        }
                                        self.selection_start_pos = None;
                                    }
                                }
                            },
                            SelectionMode::Lasso => {
                                // Nouveau mode : sélection par lasso (freehand)
                                if response.drag_started() {
                                    let mut hit = self.selected_indices.iter().find(|&&i| self.get_shape_rect(i).contains(pos)).cloned();
                                    if hit.is_none() {
                                        if let Some(layer) = self.layer_manager.get_active_layer() {
                                            hit = layer.elements.iter().enumerate().find(|(_, l)| 
                                                l.distance_to(pos) < 10.0
                                            ).map(|(i, _)| i);
                                        }
                                    }
                                    if let Some(idx) = hit {
                                        if !self.selected_indices.contains(&idx) { self.selected_indices = vec![idx]; }
                                        self.is_dragging_items = true;
                                        self.drag_accumulated_delta = Vec2::ZERO;
                                    } else {
                                        self.selected_indices.clear();
                                        self.current_lasso.clear();
                                        self.current_lasso.push(pos);
                                        self.selection_start_pos = Some(pos);
                                    }
                                }
                                if response.dragged() {
                                    if self.is_dragging_items {
                                        let delta = delta_to_world(response.drag_delta(), self.camera.zoom);
                                        self.drag_accumulated_delta += delta;
                                        if let Some(layer) = self.layer_manager.get_active_layer_mut() {
                                            for &idx in &self.selected_indices {
                                                if let Some(shape) = layer.elements.get_mut(idx) {
                                                    shape.translate(delta);
                                                    let ev = NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape));
                                                    if self.server_running {
                                                        let _ = server::publish_network_event(ev);
                                                    } else if let Some(tx) = self.outgoing_draw_tx.as_ref() {
                                                        let _ = tx.send(ev);
                                                    }
                                                }
                                            }
                                        }
                                    } else if !self.current_lasso.is_empty() {
                                        let should_add = self.current_lasso
                                            .last()
                                            .map(|p| p.distance_sq(pos) > 0.5)
                                            .unwrap_or(true);
                                        if should_add {
                                            self.current_lasso.push(pos);
                                        }
                                    }
                                }
                                if response.drag_released() {
                                    if self.is_dragging_items {
                                        let total = self.drag_accumulated_delta;
                                        if total.length_sq() > 0.0 {
                                            if let Some(layer) = self.layer_manager.get_active_layer_mut() {
                                                for &idx in &self.selected_indices {
                                                    if let Some(shape) = layer.elements.get_mut(idx) {
                                                        shape.translate(-total);
                                                    }
                                                }
                                            }
                                            self.execute(PaintAction::Move(self.selected_indices.clone(), total));
                                        }
                                        self.is_dragging_items = false;
                                    } else if !self.current_lasso.is_empty() {
                                        if let Some(layer) = self.layer_manager.get_active_layer() {
                                            self.selected_indices = layer.elements.iter().enumerate()
                                                .filter(|(_, l)| is_shape_in_lasso(l, &self.current_lasso))
                                                .map(|(i, _)| i).collect();
                                        }
                                        self.current_lasso.clear();
                                        self.selection_start_pos = None;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // --- 4. RENDU FINAL ---

            // Dessiner les éléments visibles (tous les layers, du fond vers le sommet)
            for shape in self.get_visible_elements().iter() {
                match shape {
                    PaintShape::Line { points, width, color, .. } => {
                        let scaled_points: Vec<egui::Pos2> = points.iter().map(|p| to_screen(&self.camera, *p)).collect();
                        painter.add(egui::Shape::line(scaled_points, Stroke::new(*width * self.camera.zoom, *color)));
                    }
                    PaintShape::Rectangle { start, end, width, color, .. } => {
                        painter.rect_stroke(Rect::from_two_pos(to_screen(&self.camera, *start), to_screen(&self.camera, *end)), 0.0, Stroke::new(*width * self.camera.zoom, *color));
                    }
                    PaintShape::Oval { start, end, width, color, .. } => {
                        let rect = Rect::from_two_pos(to_screen(&self.camera, *start), to_screen(&self.camera, *end));
                        draw_ellipse(&painter, rect, Stroke::new(*width * self.camera.zoom, *color));
                    }
                    PaintShape::RegularPolygon { start, end, sides, width, color, .. } => {
                        draw_regular_polygon(&painter, Rect::from_two_pos(to_screen(&self.camera, *start), to_screen(&self.camera, *end)), *sides as usize, Stroke::new(*width * self.camera.zoom, *color));
                    }
                    PaintShape::Star { start, end, points, width, color, .. } => {
                        draw_star(&painter, Rect::from_two_pos(to_screen(&self.camera, *start), to_screen(&self.camera, *end)), *points as usize, Stroke::new(*width * self.camera.zoom, *color));
                    }
                    PaintShape::Arrow { start, end, width, color, .. } => {
                        draw_arrow(&painter, to_screen(&self.camera, *start), to_screen(&self.camera, *end), Stroke::new(*width * self.camera.zoom, *color));
                    }
                }
            }
            // Dessiner les sélections avec rectangle pointillé (pour le layer actif uniquement)
            if self.mode == BrushMode::Select {
                if let Some(layer) = self.layer_manager.get_active_layer() {
                    for &idx in &self.selected_indices {
                        if let Some(shape) = layer.elements.get(idx) {
                            let r = shape.bounding_rect();
                            draw_dashed_rect(&painter, rect_to_screen(&self.camera, r), Color32::WHITE);
                            draw_dashed_rect(&painter, rect_to_screen(&self.camera, r.expand(1.0)), Color32::BLACK);
                        }
                    }
                }
            }

            // Dessiner le rectangle de sélection ou le lasso
            if self.selection_mode == SelectionMode::Rectangle {
                if let Some(r) = self.selection_rect {
                    let r_screen = rect_to_screen(&self.camera, r);
                    painter.rect_filled(r_screen, 0.0, Color32::from_rgba_unmultiplied(100, 150, 255, 30));
                    painter.rect_stroke(r_screen, 0.0, Stroke::new(1.0, Color32::from_rgb(100, 150, 255)));
                }
            } else if self.selection_mode == SelectionMode::Lasso && !self.current_lasso.is_empty() {
                let screen_lasso: Vec<egui::Pos2> = self.current_lasso.iter().map(|p| to_screen(&self.camera, *p)).collect();
                draw_lasso(&painter, &screen_lasso);
            }

            // Dessiner la forme en cours de tracé
            if !self.current_line.is_empty() {
                if self.mode == BrushMode::Shape {
                    let start = to_screen(&self.camera, self.current_line[0]);
                    let end = to_screen(&self.camera, self.current_line[1]);
                    match self.selected_shape {
                        ShapeKind::Line => {
                            let preview_points: Vec<egui::Pos2> = self.current_line.iter().map(|p| to_screen(&self.camera, *p)).collect();
                            painter.add(Shape::line(preview_points, Stroke::new(self.brush_size * self.camera.zoom, self.brush_color)));
                        }
                        ShapeKind::Rectangle => {
                            painter.rect_stroke(Rect::from_two_pos(start, end), 0.0, Stroke::new(self.brush_size * self.camera.zoom, self.brush_color));
                        }
                        ShapeKind::Oval => {
                            draw_ellipse(&painter, Rect::from_two_pos(start, end), Stroke::new(self.brush_size * self.camera.zoom, self.brush_color));
                        }
                        ShapeKind::Triangle
                        | ShapeKind::Pentagon
                        | ShapeKind::Hexagon
                        | ShapeKind::Octagon => {
                            draw_regular_polygon(&painter, Rect::from_two_pos(start, end), self.selected_shape.sides() as usize, Stroke::new(self.brush_size * self.camera.zoom, self.brush_color));
                        }
                        ShapeKind::Star => {
                            draw_star(&painter, Rect::from_two_pos(start, end), 5, Stroke::new(self.brush_size * self.camera.zoom, self.brush_color));
                        }
                        ShapeKind::Arrow => {
                            draw_arrow(&painter, start, end, Stroke::new(self.brush_size * self.camera.zoom, self.brush_color));
                        }
                    }
                } else {
                    let preview_points: Vec<egui::Pos2> = self.current_line.iter().map(|p| to_screen(&self.camera, *p)).collect();
                    painter.add(Shape::line(preview_points, Stroke::new(self.brush_size * self.camera.zoom, self.brush_color)));
                }
            }

            // Dessiner le curseur de la gomme
            if self.mode == BrushMode::Eraser {
                if let Some(p) = ctx.pointer_latest_pos() {
                    painter.circle_stroke(p, self.brush_size * self.camera.zoom, Stroke::new(1.0, Color32::LIGHT_RED));
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
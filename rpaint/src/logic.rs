use crate::model::*;
use crate::events::{DrawShapeEvent, NetworkEvent};
use crate::server;
use eframe::egui::{Rect, Vec2};

impl PaintApp {
    fn send_network_event(&self, event: NetworkEvent) {
        if self.server_running {
            let _ = server::publish_network_event(event);
        } else if let Some(tx) = self.outgoing_draw_tx.as_ref() {
            let _ = tx.send(event);
        }
    }

    // --- UTILITAIRES POUR ACCÉDER AUX ÉLÉMENTS DU LAYER ACTIF ---

    /// Retourne les indices corrigés pour le layer actif
    /// (les indices stockés dans selected_indices sont locaux au layer actif)
    fn get_active_layer_mut(&mut self) -> Option<&mut crate::layers::Layer> {
        self.layer_manager.get_active_layer_mut()
    }

    /// Crée une liste plate de tous les éléments visibles pour le rendu
    pub fn get_visible_elements(&self) -> Vec<&Shape> {
        self.layer_manager.get_visible_elements()
    }

    // --- ACTIONS DE BASE ---

    // Efface tout le canvas en enregistrant l'opération dans l'historique.
    pub fn clear_all(&mut self) {
        if let Some(layer) = self.get_active_layer_mut() {
            if layer.elements.is_empty() {
                return;
            }
            let indices = (0..layer.elements.len()).collect();
            let shapes = layer.elements.clone();
            self.execute(PaintAction::Delete(indices, shapes));
            self.selected_indices.clear();
        }
    }

    // Supprime uniquement les éléments actuellement sélectionnés.
    pub fn delete_selected(&mut self) {
        if self.selected_indices.is_empty() {
            return;
        }
        let mut indexed: Vec<_> = self.selected_indices
            .iter()
            .filter_map(|&i| {
                self.layer_manager
                    .get_active_layer()
                    .and_then(|l| l.elements.get(i).map(|s| (i, s.clone())))
            })
            .collect();
        indexed.sort_by_key(|&(i, _)| i);
        let indices = indexed.iter().map(|(i, _)| *i).collect();
        let shapes = indexed.into_iter().map(|(_, s)| s).collect();
        self.execute(PaintAction::Delete(indices, shapes));
        self.selected_indices.clear();
    }

    // --- PRESSE-PAPIER ---

    // Copie les lignes sélectionnées dans le presse-papiers interne.
    pub fn copy_selected(&mut self) {
        if self.selected_indices.is_empty() {
            return;
        }
        self.clipboard = self.selected_indices
            .iter()
            .filter_map(|&i| {
                self.layer_manager
                    .get_active_layer()
                    .and_then(|l| l.elements.get(i).cloned())
            })
            .collect();
    }

    // Colle le presse-papiers en décalant les points pour éviter la superposition exacte.
    pub fn paste(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        let offset = Vec2::splat(20.0);
        let mut new_shapes = self.clipboard.clone();
        for shape in &mut new_shapes {
            match shape {
                Shape::Line { points, id, .. } => {
                    for p in points {
                        *p += offset;
                    }
                    *id = timestamp_id();
                }
                Shape::Rectangle { start, end, id, .. }
                | Shape::Oval { start, end, id, .. }
                | Shape::RegularPolygon { start, end, id, .. }
                | Shape::Star { start, end, id, .. }
                | Shape::Arrow { start, end, id, .. } => {
                    *start += offset;
                    *end += offset;
                    *id = timestamp_id();
                }
            }
        }
        self.execute(PaintAction::Create(new_shapes.clone()));
        self.clipboard = new_shapes.clone();
        let start_idx = if let Some(layer) = self.layer_manager.get_active_layer() {
            layer.elements.len() - self.clipboard.len()
        } else {
            0
        };
        self.selected_indices = (start_idx..start_idx + self.clipboard.len()).collect();
    }

    // --- SYSTEME UNDO/REDO ---

    // Exécute une action, l'applique au modèle, puis la pousse dans la pile undo.
    pub fn execute(&mut self, action: PaintAction) {
        self.apply_action(&action);

        // Toute action locale de création/suppression est propagée.
        match &action {
            PaintAction::Create(shapes) => {
                for shape in shapes {
                    self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape)));
                }
            }
            PaintAction::Delete(_, shapes) => {
                for shape in shapes {
                    self.send_network_event(NetworkEvent::DeleteShape(shape.id()));

                }
            }
            PaintAction::Modify(_, _, new_shapes) => {
                for shape in new_shapes {
                    self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape)));
                }
            }
            PaintAction::Move(indices, _) => {
                if let Some(layer) = self.layer_manager.get_active_layer() {
                    for &idx in indices {
                        if let Some(shape) = layer.elements.get(idx) {
                            self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape)));
                        }
                    }
                }
            }

            // Actions de layers propagées
            PaintAction::CreateLayer { id, name, .. } => {
                self.send_network_event(NetworkEvent::CreateLayer { id: *id, name: name.clone(), position: 0 });
            }
            PaintAction::DeleteLayer { id, .. } => {
                self.send_network_event(NetworkEvent::DeleteLayer { id: *id });
            }
            PaintAction::RenameLayer { id, new_name, .. } => {
                self.send_network_event(NetworkEvent::RenameLayer { id: *id, name: new_name.clone() });
            }
            PaintAction::SetLayerVisibility { id, visible } => {
                self.send_network_event(NetworkEvent::SetLayerVisibility { id: *id, visible: *visible });
            }
            PaintAction::SetActiveLayer { new_id, .. } => {
                self.send_network_event(NetworkEvent::SetActiveLayer { id: *new_id });
            }
            PaintAction::ReorderLayers { from_idx, to_idx } => {
                self.send_network_event(NetworkEvent::ReorderLayers { from_idx: *from_idx, to_idx: *to_idx });
            }
            // Actions réseau - ignorées dans execute (elles sont déjà appliquées)
            PaintAction::NetworkCreateLayer { .. } | PaintAction::NetworkDeleteLayer { .. } | PaintAction::NetworkRenameLayer { .. } | PaintAction::NetworkSetLayerVisibility { .. } | PaintAction::NetworkSetActiveLayer { .. } | PaintAction::NetworkReorderLayers { .. } => {}
        }

        // Ne pousser dans l'historique que les actions locales (pas les Network*)
        if !matches!(action, PaintAction::NetworkCreateLayer { .. } | PaintAction::NetworkDeleteLayer { .. } | PaintAction::NetworkRenameLayer { .. } | PaintAction::NetworkSetLayerVisibility { .. } | PaintAction::NetworkSetActiveLayer { .. } | PaintAction::NetworkReorderLayers { .. }) {
            self.undo_stack.push(action.clone());
            self.redo_stack.clear();
        }
    }

    // Applique une action sans toucher à l'historique.
    pub fn apply_action(&mut self, action: &PaintAction) {
        match action {
            PaintAction::Create(new_shapes) => {
                if let Some(layer) = self.get_active_layer_mut() {
                    for s in new_shapes {
                        layer.add_element(s.clone());
                    }
                }
            }
            PaintAction::Delete(indices, _) => {
                if let Some(layer) = self.get_active_layer_mut() {
                    let mut sorted = indices.clone();
                    sorted.sort_by(|a, b| b.cmp(a));
                    for idx in sorted {
                        if idx < layer.elements.len() {
                            layer.elements.remove(idx);
                        }
                    }
                }
            }
            PaintAction::Modify(indices, _, new_shapes) => {
                if let Some(layer) = self.get_active_layer_mut() {
                    for (i, &idx) in indices.iter().enumerate() {
                        if let Some(s) = layer.elements.get_mut(idx) {
                            *s = new_shapes[i].clone();
                        }
                    }
                }
            }
            PaintAction::Move(indices, delta) => {
                if let Some(layer) = self.get_active_layer_mut() {
                    for &idx in indices {
                        if let Some(shape) = layer.elements.get_mut(idx) {
                            shape.translate(*delta);
                        }
                    }
                }
            }
            PaintAction::CreateLayer { id, name, .. } => {
                self.layer_manager.create_layer_at(*id, name.clone(), 0);
            }
            PaintAction::DeleteLayer {
                id,
                layer: _,
                position: _,
            } => {
                self.layer_manager.delete_layer(*id);
            }
            PaintAction::RenameLayer { id, new_name, .. } => {
                self.layer_manager.rename_layer(*id, new_name.clone());
            }
            PaintAction::SetLayerVisibility { id, visible } => {
                self.layer_manager.set_layer_visibility(*id, *visible);
            }
            PaintAction::ReorderLayers { from_idx, to_idx } => {
                self.layer_manager.reorder_layer(*from_idx, *to_idx);
            }
            PaintAction::SetActiveLayer { new_id, .. } => {
                self.layer_manager.set_active_layer(*new_id);
            }
            // Actions réseau (même logique que les actions normales mais ne se propagent pas)
            PaintAction::NetworkCreateLayer { id, name, position } => {
                self.layer_manager.create_layer_at(*id, name.clone(), *position);
            }
            PaintAction::NetworkDeleteLayer { id } => {
                self.layer_manager.delete_layer(*id);
            }
            PaintAction::NetworkRenameLayer { id, name } => {
                self.layer_manager.rename_layer(*id, name.clone());
            }
            PaintAction::NetworkSetLayerVisibility { id, visible } => {
                self.layer_manager.set_layer_visibility(*id, *visible);
            }
            PaintAction::NetworkSetActiveLayer { id } => {
                self.layer_manager.set_active_layer(*id);
            }
            PaintAction::NetworkReorderLayers { from_idx, to_idx } => {
                self.layer_manager.reorder_layer(*from_idx, *to_idx);
            }
        }
    }

    // Annule la dernière action enregistrée.
    pub fn undo(&mut self) {
        if let Some(action) = self.undo_stack.pop() {
            match &action {
                // Actions réseau ne devraient pas être dans l'historique
                PaintAction::NetworkCreateLayer { .. } | PaintAction::NetworkDeleteLayer { .. } | PaintAction::NetworkRenameLayer { .. } | PaintAction::NetworkSetLayerVisibility { .. } | PaintAction::NetworkSetActiveLayer { .. } | PaintAction::NetworkReorderLayers { .. } => {
                    // Ne rien faire - ces actions ne devraient pas être dans l'historique
                },
                PaintAction::Create(shapes) => {
                    // Supprimer par id
                    let mut ids: Vec<u64> = shapes.iter().map(|s| s.id()).collect();
                    ids.sort_unstable();
                    ids.dedup();
                    if let Some(layer) = self.get_active_layer_mut() {
                        layer.elements.retain(|existing| ids.binary_search(&existing.id()).is_err());
                    }
                    // Informer les autres clients que ces formes ont été annulées.
                    for s in shapes {
                        self.send_network_event(NetworkEvent::DeleteShape(s.id()));
                    }
                },
                PaintAction::Delete(indices, shapes) => {
                    let mut combined: Vec<_> = indices.iter().zip(shapes.iter()).collect();
                    combined.sort_by_key(|&(&idx, _)| idx);
                    if let Some(layer) = self.get_active_layer_mut() {
                        for (&idx, shape) in combined {
                            if idx <= layer.elements.len() {
                                layer.elements.insert(idx, shape.clone());
                            }
                        }
                    }

                    // Undo d'une suppression = recréer ces formes sur les autres clients.
                    for shape in shapes {
                        self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape)));
                    }
                },
                PaintAction::Modify(indices, old_shapes, _) => {
                    if let Some(layer) = self.get_active_layer_mut() {
                        for (i, &idx) in indices.iter().enumerate() {
                            if let Some(s) = layer.elements.get_mut(idx) {
                                *s = old_shapes[i].clone();
                            }
                        }
                    }

                    // Undo d'une modification = repousser l'ancienne version.
                    for shape in old_shapes {
                        self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape)));

                    }
                },
                PaintAction::Move(indices, delta) => {
                    if let Some(layer) = self.get_active_layer_mut() {
                        for &idx in indices {
                            if let Some(shape) = layer.elements.get_mut(idx) {
                                shape.translate(-*delta);
                            }
                        }
                    }

                    // Undo d'un déplacement = repousser la géométrie courante restaurée.
                    if let Some(layer) = self.layer_manager.get_active_layer() {
                        for &idx in indices {
                            if let Some(shape) = layer.elements.get(idx) {
                                self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape)));
                            }
                        }
                    }
                }
                PaintAction::CreateLayer { id, old_active, .. } => {
                    self.layer_manager.delete_layer(*id);
                    self.layer_manager.set_active_layer(*old_active);
                    self.send_network_event(NetworkEvent::DeleteLayer { id: *id });
                    self.send_network_event(NetworkEvent::SetActiveLayer { id: *old_active });
                }
                PaintAction::DeleteLayer {
                    id,
                    layer,
                    position,
                } => {
                    // Undo de suppression: restaurer le layer
                    self.layer_manager.layers.insert(*position, layer.clone());
                    self.layer_manager.active_layer_id = *id;
                    self.send_network_event(NetworkEvent::CreateLayer { id: *id, name: layer.name.clone(), position: *position });
                    self.send_network_event(NetworkEvent::SetActiveLayer { id: *id });
                }
                PaintAction::RenameLayer { id, old_name, .. } => {
                    self.layer_manager.rename_layer(*id, old_name.clone());
                    self.send_network_event(NetworkEvent::RenameLayer { id: *id, name: old_name.clone() });
                }
                PaintAction::SetLayerVisibility { id, visible } => {
                    let new_visible = !*visible;
                    self.layer_manager.set_layer_visibility(*id, new_visible);
                    self.send_network_event(NetworkEvent::SetLayerVisibility { id: *id, visible: new_visible });
                }
                PaintAction::ReorderLayers { from_idx, to_idx } => {
                    // Undo d'un réordonnancement: inverser l'ordre
                    self.layer_manager.reorder_layer(*to_idx, *from_idx);
                    self.send_network_event(NetworkEvent::ReorderLayers { from_idx: *to_idx, to_idx: *from_idx });
                }
                PaintAction::SetActiveLayer { old_id, .. } => {
                    self.layer_manager.set_active_layer(*old_id);
                    self.send_network_event(NetworkEvent::SetActiveLayer { id: *old_id });
                }
            }
            self.redo_stack.push(action);
            self.selected_indices.clear();
        }
    }

    // Rejoue une action qui vient d'être annulée.
    pub fn redo(&mut self) {
        if let Some(action) = self.redo_stack.pop() {
            self.apply_action(&action);

            // Repropager l'action rejouée pour garder les autres clients synchronisés.
            match &action {
                // Actions réseau ne devraient pas être dans l'historique
                PaintAction::NetworkCreateLayer { .. } | PaintAction::NetworkDeleteLayer { .. } | PaintAction::NetworkRenameLayer { .. } | PaintAction::NetworkSetLayerVisibility { .. } | PaintAction::NetworkSetActiveLayer { .. } | PaintAction::NetworkReorderLayers { .. } => {
                    // Ne rien faire - ces actions ne devraient pas être dans l'historique
                }
                PaintAction::Create(shapes) => {
                    for s in shapes {
                        self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(s)));
                    }
                }
                PaintAction::Delete(_, shapes) => {
                    for s in shapes {
                        self.send_network_event(NetworkEvent::DeleteShape(s.id()));
                    }
                }
                PaintAction::Modify(_, _, new_shapes) => {
                    for shape in new_shapes {
                        self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape)));
                    }
                }
                PaintAction::Move(indices, _) => {
                    if let Some(layer) = self.layer_manager.get_active_layer() {
                        for &idx in indices {
                            if let Some(shape) = layer.elements.get(idx) {
                                self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape)));
                            }
                        }
                    }
                }
                PaintAction::CreateLayer { id, name, .. } => {
                    self.send_network_event(NetworkEvent::CreateLayer { id: *id, name: name.clone(), position: 0 });
                }
                PaintAction::DeleteLayer { id, .. } => {
                    self.send_network_event(NetworkEvent::DeleteLayer { id: *id });
                }
                PaintAction::RenameLayer { id, new_name, .. } => {
                    self.send_network_event(NetworkEvent::RenameLayer { id: *id, name: new_name.clone() });
                }
                PaintAction::SetLayerVisibility { id, visible } => {
                    self.send_network_event(NetworkEvent::SetLayerVisibility { id: *id, visible: *visible });
                }
                PaintAction::ReorderLayers { from_idx, to_idx } => {
                    self.send_network_event(NetworkEvent::ReorderLayers { from_idx: *from_idx, to_idx: *to_idx });
                }
                PaintAction::SetActiveLayer { new_id, .. } => {
                    self.send_network_event(NetworkEvent::SetActiveLayer { id: *new_id });
                }
            }

            self.undo_stack.push(action);
        }
    }

    // --- UTILITAIRES DE SÉLECTION ---

    // Calcule une boîte englobante pour tester rapidement si un tracé est cliqué/sélectionné.
    pub fn get_shape_rect(&self, idx: usize) -> Rect {
        if let Some(layer) = self.layer_manager.get_active_layer() {
            if let Some(shape) = layer.elements.get(idx) {
                return shape.bounding_rect();
            }
        }
        Rect::NOTHING
    }

    // --- ACTIONS POUR LES LAYERS ---
    pub fn create_new_layer(&mut self) {
        let new_id = timestamp_id();
        self.last_layer_index += 1;
        let name = format!("Layer-{}", self.last_layer_index);
        let old_active = self.layer_manager.active_layer_id;
        self.execute(PaintAction::CreateLayer { id: new_id, name, old_active });
    }

    pub fn delete_layer(&mut self, layer_id: u64) -> bool {
        if self.layer_manager.layers.len() <= 1 { return false; }
        if let Some(idx) = self.layer_manager.get_layer_index(layer_id) {
            if let Some(layer) = self.layer_manager.get_layer(layer_id) {
                let layer_copy = layer.clone_for_undo();
                self.execute(PaintAction::DeleteLayer {
                    id: layer_id,
                    layer: layer_copy,
                    position: idx,
                });
                return true;
            }
        }
        false
    }

    pub fn rename_layer(&mut self, layer_id: u64, new_name: String) {
        if let Some(layer) = self.layer_manager.get_layer(layer_id) {
            let old_name = layer.name.clone();
            self.execute(PaintAction::RenameLayer {
                id: layer_id,
                old_name,
                new_name,
            });
        }
    }

    pub fn toggle_layer_visibility(&mut self, layer_id: u64) {
        if let Some(layer) = self.layer_manager.get_layer(layer_id) {
            let new_visible = !layer.visible;
            self.execute(PaintAction::SetLayerVisibility {
                id: layer_id,
                visible: new_visible,
            });
        }
    }

    pub fn reorder_layers(&mut self, from_idx: usize, to_idx: usize) {
        self.execute(PaintAction::ReorderLayers { from_idx, to_idx });
    }

    pub fn set_active_layer(&mut self, layer_id: u64) {
        let old_id = self.layer_manager.active_layer_id;
        if old_id != layer_id {
            self.execute(PaintAction::SetActiveLayer {
                old_id,
                new_id: layer_id,
            });
            self.selected_indices.clear();
        }
    }
}

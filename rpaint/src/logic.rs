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

    // --- ACTIONS DE BASE ---

    // Efface tout le canvas en enregistrant l'opération dans l'historique.
    pub fn clear_all(&mut self) {
        if self.lines.is_empty() { return; }
        let indices = (0..self.lines.len()).collect();
        let lines = self.lines.clone();
        self.execute(PaintAction::Delete(indices, lines));
        self.selected_indices.clear();
    }

    // Supprime uniquement les éléments actuellement sélectionnés.
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

    // Copie les lignes sélectionnées dans le presse-papiers interne.
    pub fn copy_selected(&mut self) {
        if self.selected_indices.is_empty() { return; }
        self.clipboard = self.selected_indices.iter()
            .filter_map(|&i| self.lines.get(i).cloned())
            .collect();
    }

    // Colle le presse-papiers en décalant les points pour éviter la superposition exacte.
    pub fn paste(&mut self) {
        if self.clipboard.is_empty() { return; }
        let offset = Vec2::splat(20.0);
        let mut new_lines = self.clipboard.clone();
        for shape in &mut new_lines {
            match shape {
                Shape::Line { points, id, .. } => {
                    for p in points { *p += offset; }
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
        self.execute(PaintAction::Create(new_lines.clone()));
        self.clipboard = new_lines;
        let start_idx = self.lines.len() - self.clipboard.len();
        self.selected_indices = (start_idx..self.lines.len()).collect();
    }

    // --- SYSTÈME UNDO/REDO ---

    // Exécute une action, l'applique au modèle, puis la pousse dans la pile undo.
    pub fn execute(&mut self, action: PaintAction) {
        self.apply_action(&action);

        // Toute action locale de création/suppression est propagée.
        match &action {
            PaintAction::Create(lines) => {
                for shape in lines {
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
                for &idx in indices {
                    if let Some(shape) = self.lines.get(idx) {
                        self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape)));
                    }
                }
            }
        }

        self.undo_stack.push(action);
        self.redo_stack.clear();
    }

    // Applique une action sans toucher à l'historique.
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
                    if let Some(shape) = self.lines.get_mut(idx) {
                        shape.translate(*delta);
                    }
                }
            }
        }
    }

    // Annule la dernière action enregistrée.
    pub fn undo(&mut self) {
        if let Some(action) = self.undo_stack.pop() {
            match &action {
                PaintAction::Create(shapes) => {
                    // Supprimer par id (et non par pop en fin de tableau),
                    // car des formes distantes peuvent avoir été ajoutées après.
                    let mut ids: Vec<u64> = shapes.iter().map(|l| l.id()).collect();
                    ids.sort_unstable();
                    ids.dedup();
                    self.lines.retain(|existing| ids.binary_search(&existing.id()).is_err());
                    // Informer les autres clients que ces formes ont été annulées.
                    for shape in shapes {
                        self.send_network_event(NetworkEvent::DeleteShape(shape.id()));
                    }
                },
                PaintAction::Delete(indices, shapes) => {
                    let mut combined: Vec<_> = indices.iter().zip(shapes.iter()).collect();
                    combined.sort_by_key(|&(&idx, _)| idx);
                    for (&idx, shape) in combined { self.lines.insert(idx, shape.clone()); }

                    // Undo d'une suppression = recréer ces formes sur les autres clients.
                    for shape in shapes {
                        self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape)));
                    }
                },
                PaintAction::Modify(indices, old_shapes, _) => {
                    for (i, &idx) in indices.iter().enumerate() {
                        if let Some(l) = self.lines.get_mut(idx) { *l = old_shapes[i].clone(); }
                    }

                    // Undo d'une modification = repousser l'ancienne version.
                    for shape in old_shapes {
                        self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape)));
                    }
                },
                PaintAction::Move(indices, delta) => {
                    for &idx in indices {
                        if let Some(shape) = self.lines.get_mut(idx) {
                            shape.translate(-*delta);
                        }
                    }

                    // Undo d'un déplacement = repousser la géométrie courante restaurée.
                    for &idx in indices {
                        if let Some(line) = self.lines.get(idx) {
                            self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(line)));
                        }
                    }
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
                    for &idx in indices {
                        if let Some(shape) = self.lines.get(idx) {
                            self.send_network_event(NetworkEvent::DrawShape(DrawShapeEvent::from_shape(shape)));
                        }
                    }
                }
            }

            self.undo_stack.push(action);
        }
    }

    // --- UTILITAIRES DE SÉLECTION ---

    // Calcule une boîte englobante pour tester rapidement si une forme est cliquée/sélectionnée.
    pub fn get_shape_rect(&self, idx: usize) -> Rect {
        if let Some(shape) = self.lines.get(idx) {
            return shape.bounding_rect();
        }
        Rect::NOTHING
    }
}

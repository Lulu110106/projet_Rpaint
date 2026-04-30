use crate::model::*;
use crate::events::{DrawLineEvent, NetworkEvent};
use crate::server;
use eframe::egui::{Pos2, Rect, Vec2};

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
                for line in lines {
                    self.send_network_event(NetworkEvent::DrawLine(DrawLineEvent::from_line(line)));
                }
            }
            PaintAction::Delete(_, lines) => {
                for line in lines {
                    self.send_network_event(NetworkEvent::DeleteLine(line.id()));
                }
            }
            PaintAction::Modify(_, _, new_lines) => {
                for line in new_lines {
                    self.send_network_event(NetworkEvent::DrawLine(DrawLineEvent::from_line(line)));
                }
            }
            PaintAction::Move(indices, _) => {
                for &idx in indices {
                    if let Some(line) = self.lines.get(idx) {
                        self.send_network_event(NetworkEvent::DrawLine(DrawLineEvent::from_line(line)));
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
                        match shape {
                            Shape::Line { points, .. } => {
                                for p in points { *p += *delta; }
                            }
                        }
                    }
                }
            }
        }
    }

    // Annule la dernière action enregistrée.
    pub fn undo(&mut self) {
        if let Some(action) = self.undo_stack.pop() {
            match &action {
                PaintAction::Create(lines) => {
                    // Supprimer par id (et non par pop en fin de tableau),
                    // car des lignes distantes peuvent avoir été ajoutées après.
                    let mut ids: Vec<u64> = lines.iter().map(|l| l.id()).collect();
                    ids.sort_unstable();
                    ids.dedup();
                    self.lines.retain(|existing| ids.binary_search(&existing.id()).is_err());
                    // Informer les autres clients que ces lignes ont été annulées.
                    for l in lines {
                        self.send_network_event(NetworkEvent::DeleteLine(l.id()));
                    }
                },
                PaintAction::Delete(indices, lines) => {
                    let mut combined: Vec<_> = indices.iter().zip(lines.iter()).collect();
                    combined.sort_by_key(|&(&idx, _)| idx);
                    for (&idx, line) in combined { self.lines.insert(idx, line.clone()); }

                    // Undo d'une suppression = recréer ces lignes sur les autres clients.
                    for line in lines {
                        self.send_network_event(NetworkEvent::DrawLine(DrawLineEvent::from_line(line)));
                    }
                },
                PaintAction::Modify(indices, old_lines, _) => {
                    for (i, &idx) in indices.iter().enumerate() {
                        if let Some(l) = self.lines.get_mut(idx) { *l = old_lines[i].clone(); }
                    }

                    // Undo d'une modification = repousser l'ancienne version.
                    for line in old_lines {
                        self.send_network_event(NetworkEvent::DrawLine(DrawLineEvent::from_line(line)));
                    }
                },
                PaintAction::Move(indices, delta) => {
                    for &idx in indices {
                        if let Some(shape) = self.lines.get_mut(idx) {
                            match shape {
                                Shape::Line { points, .. } => {
                                    for p in points { *p -= *delta; }
                                }
                            }
                        }
                    }

                    // Undo d'un déplacement = repousser la géométrie courante restaurée.
                    for &idx in indices {
                        if let Some(line) = self.lines.get(idx) {
                            self.send_network_event(NetworkEvent::DrawLine(DrawLineEvent::from_line(line)));
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
                PaintAction::Create(lines) => {
                    for l in lines {
                        self.send_network_event(NetworkEvent::DrawLine(DrawLineEvent::from_line(l)));
                    }
                }
                PaintAction::Delete(_, lines) => {
                    for l in lines {
                        self.send_network_event(NetworkEvent::DeleteLine(l.id()));
                    }
                }
                PaintAction::Modify(_, _, new_lines) => {
                    for line in new_lines {
                        self.send_network_event(NetworkEvent::DrawLine(DrawLineEvent::from_line(line)));
                    }
                }
                PaintAction::Move(indices, _) => {
                    for &idx in indices {
                        if let Some(line) = self.lines.get(idx) {
                            self.send_network_event(NetworkEvent::DrawLine(DrawLineEvent::from_line(line)));
                        }
                    }
                }
            }

            self.undo_stack.push(action);
        }
    }

    // --- UTILITAIRES DE SÉLECTION ---

    // Calcule une boîte englobante pour tester rapidement si un tracé est cliqué/sélectionné.
    pub fn get_line_rect(&self, idx: usize) -> Rect {
        if let Some(shape) = self.lines.get(idx) {
            match shape {
                Shape::Line { points, width, .. } => {
                    let mut r = Rect::NOTHING;
                    for p in points { r.extend_with(*p); }
                    return r.expand(width / 2.0 + 5.0);
                }
            }
        }
        Rect::NOTHING
    }
}

// Distance entre un point et un segment.
// Utilisé par la gomme et la sélection pour savoir si le pointeur "touche" un trait.
pub fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let l2 = a.distance_sq(b);
    if l2 == 0.0 { return p.distance(a); }
    let t = ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / l2;
    p.distance(Pos2::new(a.x + t.clamp(0.0, 1.0) * (b.x - a.x), a.y + t.clamp(0.0, 1.0) * (b.y - a.y)))
}
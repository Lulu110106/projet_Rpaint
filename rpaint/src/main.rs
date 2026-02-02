use eframe::egui;
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
            });

            ui.separator();

            ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=50.0).text("Taille"));
            
            if self.mode != BrushMode::Eraser {
                ui.color_edit_button_srgba(&mut self.brush_color);
            } else {
                ui.label("Mode Gomme actif");
            }
            
            ui.separator();

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
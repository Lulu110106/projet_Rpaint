use crate::model::Shape;
use eframe::egui::{Color32, Pos2};
use serde::{Deserialize, Serialize};

// Tous les messages réseau passent par cette enveloppe sérialisable.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum NetworkEvent {
    DrawShape(DrawShapeEvent),
    DeleteShape(u64), // supprime la ligne identifiée par son id
    SessionStatus {
        message: String,
    },
    // Événements pour les layers
    CreateLayer {
        id: u64,
        name: String,
        position: usize,
    },
    DeleteLayer {
        id: u64,
    },
    RenameLayer {
        id: u64,
        name: String,
    },
    SetLayerVisibility {
        id: u64,
        visible: bool,
    },
    SetActiveLayer {
        id: u64,
    },
    ReorderLayers {
        from_idx: usize,
        to_idx: usize,
    },
    SyncProject {
        project: crate::model::PaintProject,
    },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ShapeType {
    Line,
    Rectangle,
    Oval,
    RegularPolygon,
    Star,
    Arrow,
}

// Version réseau d'une forme: des types simples pour pouvoir sérialiser en JSON.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DrawShapeEvent {
    pub id: u64,
    pub shape_type: ShapeType,
    pub points: Vec<[f32; 2]>,
    pub color_rgba: [u8; 4],
    pub width: f32,
    pub sides: Option<u8>,
    pub star_points: Option<u8>,
    pub source_id: u64,
}

impl DrawShapeEvent {
    // Convertit une Shape en format transportable.
    pub fn from_shape(shape: &Shape) -> Self {
        let shape_type = match shape {
            Shape::Line { .. } => ShapeType::Line,
            Shape::Rectangle { .. } => ShapeType::Rectangle,
            Shape::Oval { .. } => ShapeType::Oval,
            Shape::RegularPolygon { .. } => ShapeType::RegularPolygon,
            Shape::Star { .. } => ShapeType::Star,
            Shape::Arrow { .. } => ShapeType::Arrow,
        };

        let points = match shape {
            Shape::Line { points, .. } => points.iter().map(|p| [p.x, p.y]).collect(),
            Shape::Rectangle { start, end, .. }
            | Shape::Oval { start, end, .. }
            | Shape::RegularPolygon { start, end, .. }
            | Shape::Star { start, end, .. }
            | Shape::Arrow { start, end, .. } => vec![[start.x, start.y], [end.x, end.y]],
        };

        let (sides, star_points) = match shape {
            Shape::RegularPolygon { sides, .. } => (Some(*sides), None),
            Shape::Star { points, .. } => (None, Some(*points)),
            _ => (None, None),
        };

        let c = shape.color();
        Self {
            id: shape.id(),
            shape_type,
            points,
            color_rgba: [c.r(), c.g(), c.b(), c.a()],
            width: shape.width(),
            sides,
            star_points,
            source_id: 0,
        }
    }

    // Reconstitue une Shape à partir des données reçues du réseau.
    pub fn to_shape(&self) -> Shape {
        let points = self
            .points
            .iter()
            .map(|p| Pos2::new(p[0], p[1]))
            .collect::<Vec<_>>();
        let c = self.color_rgba;
        let color = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);

        match self.shape_type {
            ShapeType::Line => Shape::Line {
                id: self.id,
                points,
                color,
                width: self.width,
            },
            ShapeType::Rectangle => {
                let start = points.get(0).cloned().unwrap_or_default();
                let end = points.get(1).cloned().unwrap_or_default();
                Shape::Rectangle {
                    id: self.id,
                    start,
                    end,
                    color,
                    width: self.width,
                }
            }
            ShapeType::Oval => {
                let start = points.get(0).cloned().unwrap_or_default();
                let end = points.get(1).cloned().unwrap_or_default();
                Shape::Oval {
                    id: self.id,
                    start,
                    end,
                    color,
                    width: self.width,
                }
            }
            ShapeType::RegularPolygon => {
                let start = points.get(0).cloned().unwrap_or_default();
                let end = points.get(1).cloned().unwrap_or_default();
                let sides = self.sides.unwrap_or(6);
                Shape::RegularPolygon {
                    id: self.id,
                    start,
                    end,
                    sides,
                    color,
                    width: self.width,
                }
            }
            ShapeType::Star => {
                let start = points.get(0).cloned().unwrap_or_default();
                let end = points.get(1).cloned().unwrap_or_default();
                let star_points = self.star_points.unwrap_or(5);
                Shape::Star {
                    id: self.id,
                    start,
                    end,
                    points: star_points,
                    color,
                    width: self.width,
                }
            }
            ShapeType::Arrow => {
                let start = points.get(0).cloned().unwrap_or_default();
                let end = points.get(1).cloned().unwrap_or_default();
                Shape::Arrow {
                    id: self.id,
                    start,
                    end,
                    color,
                    width: self.width,
                }
            }
        }
    }
}

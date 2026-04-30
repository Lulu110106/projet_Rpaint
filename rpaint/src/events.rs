use serde::{Deserialize, Serialize};
use eframe::egui::{Color32, Pos2};
use crate::model::Shape;

// Tous les messages réseau passent par cette enveloppe sérialisable.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum NetworkEvent {
    DrawLine(DrawLineEvent),
    DeleteLine(u64), // supprime la ligne identifiée par son id
}

// Version réseau d'une ligne: des types simples pour pouvoir sérialiser en JSON.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DrawLineEvent {
    pub id: u64,
    pub points: Vec<[f32; 2]>,
    pub color_rgba: [u8; 4],
    pub width: f32,
    pub source_id: u64,
}

impl DrawLineEvent {
    // Convertit une Shape::Line en format transportable.
    pub fn from_line(shape: &Shape) -> Self {
        match shape {
            Shape::Line { id, points, color, width } => Self {
                id: *id,
                points: points.iter().map(|p| [p.x, p.y]).collect(),
                color_rgba: [color.r(), color.g(), color.b(), color.a()],
                width: *width,
                source_id: 0,
            }
        }
    }

    // Reconstitue une Shape::Line à partir des données reçues du réseau.
    pub fn to_line(&self) -> Shape {
        let points = self.points.iter().map(|p| Pos2::new(p[0], p[1])).collect();
        let c = self.color_rgba;
        Shape::Line {
            id: self.id,
            points,
            color: Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]),
            width: self.width,
        }
    }
}
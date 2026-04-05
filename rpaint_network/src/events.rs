use serde::{Deserialize, Serialize};
use eframe::egui::{Color32, Pos2};
use crate::model::Line;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum NetworkEvent {
    DrawLine(DrawLineEvent),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DrawLineEvent {
    pub points: Vec<[f32; 2]>,
    pub color_rgba: [u8; 4],
    pub width: f32,
    pub source_id: u64,
}

impl DrawLineEvent {
    pub fn from_line(line: &Line) -> Self {
        Self {
            points: line.points.iter().map(|p| [p.x, p.y]).collect(),
            color_rgba: [line.color.r(), line.color.g(), line.color.b(), line.color.a()],
            width: line.width,
            source_id: 0,
        }
    }

    pub fn to_line(&self) -> Line {
        let points = self.points.iter().map(|p| Pos2::new(p[0], p[1])).collect();
        let c = self.color_rgba;
        Line {
            points,
            color: Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]),
            width: self.width,
        }
    }
}
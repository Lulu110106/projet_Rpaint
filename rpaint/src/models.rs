use egui::Pos2;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct Line {
    pub points: Vec<Pos2>,
    pub color: egui::Color32,
    pub width: f32,
}

#[derive(Clone, PartialEq)]
pub enum BrushMode {
    Freehand,
    StraightLine,
    Eraser,
    Select,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PaintAction {
    Create(Vec<SerializableLine>),
    Delete(Vec<usize>, Vec<SerializableLine>),
    Modify(Vec<usize>, Vec<SerializableLine>, Vec<SerializableLine>),
    Move(Vec<usize>, f32, f32),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableLine {
    pub points: Vec<(f32, f32)>,
    pub color: u32,
    pub width: f32,
}

impl From<&Line> for SerializableLine {
    fn from(line: &Line) -> Self {
        let [r, g, b, a] = line.color.to_srgba_unmultiplied();
        let color = ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        SerializableLine {
            points: line.points.iter().map(|p| (p.x, p.y)).collect(),
            color,
            width: line.width,
        }
    }
}

impl From<&SerializableLine> for Line {
    fn from(sline: &SerializableLine) -> Self {
        let color = sline.color;
        let egui_color = egui::Color32::from_rgba_unmultiplied(
            ((color >> 16) & 0xFF) as u8,
            ((color >> 8) & 0xFF) as u8,
            (color & 0xFF) as u8,
            ((color >> 24) & 0xFF) as u8,
        );
        Line {
            points: sline.points.iter().map(|(x, y)| Pos2::new(*x, *y)).collect(),
            color: egui_color,
            width: sline.width,
        }
    }
}

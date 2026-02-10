use egui::{Color32, Painter, Pos2, Rect, Stroke};

pub fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let l2 = a.distance_sq(b);
    if l2 == 0.0 {
        return p.distance(a);
    }
    let t = ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / l2;
    p.distance(Pos2::new(
        a.x + t.clamp(0.0, 1.0) * (b.x - a.x),
        a.y + t.clamp(0.0, 1.0) * (b.y - a.y),
    ))
}

pub fn draw_dashed_rect(painter: &Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(1.0, color);
    let dash_len = 6.0;
    let gap_len = 4.0;
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];

    for w in corners.windows(2) {
        let (start, end) = (w[0], w[1]);
        let full_vec = end - start;
        let len = full_vec.length();
        if len < 0.1 {
            continue;
        }
        let dir = full_vec / len;
        let mut d = 0.0;
        while d < len {
            painter.line_segment(
                [start + dir * d, start + dir * (d + dash_len).min(len)],
                stroke,
            );
            d += dash_len + gap_len;
        }
    }
}

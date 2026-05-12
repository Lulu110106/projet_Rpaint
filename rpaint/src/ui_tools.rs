use eframe::egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};

// Dessine un rectangle en pointillés, utilisé pour encadrer la sélection active.
pub fn draw_dashed_rect(painter: &Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(1.0, color);
    let dash_len = 6.0;
    let gap_len = 4.0;
    // On parcourt les 4 côtés du rectangle comme une boucle fermée.
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
    for w in corners.windows(2) {
        let (start, end) = (w[0], w[1]);
        let full_vec = end - start;
        let len = full_vec.length();
        if len == 0.0 { continue; }
        let dir = full_vec / len;
        let mut d = 0.0;
        while d < len {
            painter.line_segment([start + dir * d, start + dir * (d + dash_len).min(len)], stroke);
            d += dash_len + gap_len;
        }
    }
}

// Dessine une ellipse approximée par un polygone. Utile pour représenter des ovales.
pub fn draw_ellipse(painter: &Painter, rect: Rect, stroke: Stroke) {
    let center = rect.center();
    let a = rect.width() / 2.0;
    let b = rect.height() / 2.0;
    if a <= 0.0 || b <= 0.0 {
        return;
    }
    let samples = 40;
    let points: Vec<Pos2> = (0..=samples)
        .map(|i| {
            let angle = i as f32 / samples as f32 * std::f32::consts::TAU;
            Pos2::new(center.x + a * angle.cos(), center.y + b * angle.sin())
        })
        .collect();
    painter.add(eframe::egui::Shape::line(points, stroke));
}

// Dessine un polygone régulier dans un rectangle donné.
pub fn draw_regular_polygon(painter: &Painter, rect: Rect, sides: usize, stroke: Stroke) {
    if sides < 3 { return; }
    let center = rect.center();
    let a = rect.width() / 2.0;
    let b = rect.height() / 2.0;
    let mut points = Vec::new();
    let angle_step = std::f32::consts::TAU / sides as f32;
    for i in 0..=sides {
        let angle = i as f32 * angle_step - std::f32::consts::FRAC_PI_2;
        points.push(Pos2::new(center.x + a * angle.cos(), center.y + b * angle.sin()));
    }
    painter.add(eframe::egui::Shape::line(points, stroke));
}

// Dessine une étoile régulière à un nombre de branches donné.
pub fn draw_star(painter: &Painter, rect: Rect, points: usize, stroke: Stroke) {
    if points < 2 { return; }
    let center = rect.center();
    let a = rect.width() / 2.0;
    let b = rect.height() / 2.0;
    let total = points * 2;
    let angle_step = std::f32::consts::TAU / total as f32;
    let mut pts = Vec::new();
    for i in 0..=total {
        let angle = i as f32 * angle_step - std::f32::consts::FRAC_PI_2;
        let radius = if i % 2 == 0 { a.min(b) } else { a.min(b) * 0.45 };
        pts.push(Pos2::new(center.x + radius * angle.cos(), center.y + radius * angle.sin()));
    }
    painter.add(eframe::egui::Shape::line(pts, stroke));
}

// Dessine une flèche composée d'une ligne et d'une tête.
pub fn draw_arrow(painter: &Painter, start: Pos2, end: Pos2, stroke: Stroke) {
    painter.line_segment([start, end], stroke);
    let dir = end - start;
    let len = dir.length();
    if len == 0.0 { return; }
    let dir = dir / len;
    let perp = Vec2::new(-dir.y, dir.x);
    let head_len = 15.0_f32.min(len * 0.3);
    let left = end - dir * head_len + perp * (head_len * 0.5);
    let right = end - dir * head_len - perp * (head_len * 0.5);
    painter.line_segment([end, left], stroke);
    painter.line_segment([end, right], stroke);
}

use eframe::egui;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::theme::BLUE;

#[derive(Clone, Copy, Default)]
pub(crate) struct PadView {
    pub(crate) left_x: f32,
    pub(crate) left_y: f32,
    pub(crate) right_x: f32,
    pub(crate) right_y: f32,
    pub(crate) trigger_left: f32,
    pub(crate) trigger_right: f32,
    /// Same order as the `sources` list in `poll_controller`:
    /// 0-3 D-pad up/down/left/right, 4 Cross, 5 Circle, 6 Square, 7 Triangle,
    /// 8 L1, 9 R1, 10 L2, 11 R2, 12 Options, 13 Share, 14 L3, 15 R3, 16 PS button, 17 touchpad click.
    pub(crate) pressed: [bool; 18],
}

/// Splits a (possibly concave) polygon into triangles by ear clipping.
pub(crate) fn triangulate(points: &[Pos2]) -> Vec<u32> {
    let mut order: Vec<usize> = (0..points.len()).collect();
    let mut area = 0.0_f32;
    for i in 0..points.len() {
        let (a, b) = (points[i], points[(i + 1) % points.len()]);
        area += a.x * b.y - b.x * a.y;
    }
    if area < 0.0 {
        order.reverse();
    }
    let inside = |p: Pos2, a: Pos2, b: Pos2, c: Pos2| {
        let side = |u: Pos2, v: Pos2, w: Pos2| (v.x - u.x) * (w.y - u.y) - (v.y - u.y) * (w.x - u.x);
        side(a, b, p) >= 0.0 && side(b, c, p) >= 0.0 && side(c, a, p) >= 0.0
    };
    let mut triangles = Vec::new();
    while order.len() > 3 {
        let count = order.len();
        let mut clipped = false;
        for i in 0..count {
            let (ia, ib, ic) = (order[(i + count - 1) % count], order[i], order[(i + 1) % count]);
            let (a, b, c) = (points[ia], points[ib], points[ic]);
            if (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x) <= 0.0 {
                continue;
            }
            if order
                .iter()
                .any(|&j| j != ia && j != ib && j != ic && inside(points[j], a, b, c))
            {
                continue;
            }
            triangles.extend([ia as u32, ib as u32, ic as u32]);
            order.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if order.len() == 3 {
        triangles.extend(order.iter().map(|&i| i as u32));
    }
    triangles
}

/// Smooth DualShock 4 silhouette in a 400-wide design space, built once.
pub(crate) fn body_shape() -> &'static (Vec<Pos2>, Vec<u32>) {
    static BODY: std::sync::OnceLock<(Vec<Pos2>, Vec<u32>)> = std::sync::OnceLock::new();
    BODY.get_or_init(|| {
        // Right half, from top center clockwise down to bottom center.
        let right = [
            (200.0_f32, 92.0_f32),
            (252.0, 90.0),
            (292.0, 93.0),
            (320.0, 110.0),
            (338.0, 144.0),
            (348.0, 184.0),
            (344.0, 216.0),
            (328.0, 236.0),
            (304.0, 236.0),
            (284.0, 216.0),
            (268.0, 200.0),
            (240.0, 196.0),
            (200.0, 198.0),
        ];
        let mut control: Vec<Pos2> = right.iter().map(|&(x, y)| Pos2::new(x, y)).collect();
        for &(x, y) in right[1..right.len() - 1].iter().rev() {
            control.push(Pos2::new(400.0 - x, y));
        }
        let n = control.len();
        let mut points = Vec::new();
        for i in 0..n {
            let (p0, p1, p2, p3) = (
                control[(i + n - 1) % n],
                control[i],
                control[(i + 1) % n],
                control[(i + 2) % n],
            );
            for step in 0..8 {
                let t = step as f32 / 8.0;
                let (t2, t3) = (t * t, t * t * t);
                let spline = |a: f32, b: f32, c: f32, d: f32| {
                    0.5 * (2.0 * b
                        + (-a + c) * t
                        + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                        + (-a + 3.0 * b - 3.0 * c + d) * t3)
                };
                points.push(Pos2::new(
                    spline(p0.x, p1.x, p2.x, p3.x),
                    spline(p0.y, p1.y, p2.y, p3.y),
                ));
            }
        }
        let triangles = triangulate(&points);
        (points, triangles)
    })
}

pub(crate) fn draw_controller(ui: &mut egui::Ui, view: &PadView) {
    let width = ui.available_width().max(260.0);
    let scale = (width / 400.0).min(1.15);
    let (area, _) = ui.allocate_exact_size(Vec2::new(width, 182.0 * scale), Sense::hover());
    let painter = ui.painter_at(area);
    let origin = Pos2::new(area.center().x - 200.0 * scale, area.top());
    let p = |x: f32, y: f32| origin + Vec2::new(x, y - 60.0) * scale;

    let outline = Color32::from_rgb(10, 13, 20);
    let shell_light = Color32::from_rgb(58, 66, 86);
    let well = Color32::from_rgb(20, 24, 33);
    let idle = Color32::from_rgb(96, 108, 130);
    let lit = BLUE;

    // Triggers (L2/R2): small tabs behind the shoulders that fill as they are pulled.
    for (x0, value) in [(100.0_f32, view.trigger_left), (264.0_f32, view.trigger_right)] {
        let rect = Rect::from_min_max(p(x0, 72.0), p(x0 + 36.0, 95.0));
        let radius = egui::CornerRadius {
            nw: (14.0 * scale) as u8,
            ne: (14.0 * scale) as u8,
            sw: 0,
            se: 0,
        };
        painter.rect_filled(rect.expand(1.5), radius, outline);
        painter.rect_filled(rect, radius, shell_light);
        let fill = rect.height() * value.clamp(0.0, 1.0);
        if fill > 0.5 {
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - fill), rect.right_bottom()),
                radius,
                lit,
            );
        }
    }

    // Body: smooth silhouette with a light-to-dark vertical gradient and a crisp outline.
    let (design_points, triangles) = body_shape();
    let screen: Vec<Pos2> = design_points.iter().map(|q| p(q.x, q.y)).collect();
    let mut mesh = egui::Mesh::default();
    for (q, s) in design_points.iter().zip(&screen) {
        let t = ((q.y - 90.0) / 146.0).clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| (a + (b - a) * t) as u8;
        mesh.colored_vertex(*s, Color32::from_rgb(mix(56.0, 30.0), mix(64.0, 35.0), mix(84.0, 48.0)));
    }
    mesh.indices = triangles.clone();
    painter.add(egui::Shape::mesh(mesh));
    painter.add(egui::Shape::closed_line(screen, Stroke::new(2.0_f32, outline)));

    // Bumpers (L1/R1): strips along the shoulders.
    for (mirrored, down) in [(true, view.pressed[8]), (false, view.pressed[9])] {
        let x = |v: f32| if mirrored { 400.0 - v } else { v };
        let strip = vec![p(x(246.0), 96.0), p(x(270.0), 96.0), p(x(288.0), 98.0), p(x(300.0), 104.0)];
        painter.add(egui::Shape::line(strip.clone(), Stroke::new(8.0 * scale, outline)));
        painter.add(egui::Shape::line(
            strip,
            Stroke::new(5.5 * scale, if down { lit } else { shell_light }),
        ));
    }

    // Light bar glow and touchpad.
    painter.rect_filled(
        Rect::from_min_max(p(166.0, 95.0), p(234.0, 103.0)),
        egui::CornerRadius::same(4),
        Color32::from_rgba_unmultiplied(77, 111, 240, 55),
    );
    painter.rect_filled(
        Rect::from_min_max(p(174.0, 97.0), p(226.0, 100.0)),
        egui::CornerRadius::same(2),
        lit,
    );
    let pad = Rect::from_min_max(p(160.0, 106.0), p(240.0, 146.0));
    let pad_radius = egui::CornerRadius::same((10.0 * scale) as u8);
    painter.rect_filled(pad.expand(1.5), pad_radius, outline);
    painter.rect_filled(pad, pad_radius, if view.pressed[17] { lit } else { shell_light });

    // Share (left) and Options (right).
    for (x, down) in [(144.0_f32, view.pressed[13]), (256.0_f32, view.pressed[12])] {
        let rect = Rect::from_center_size(p(x, 110.0), Vec2::new(6.0, 12.0) * scale);
        painter.rect_filled(rect.expand(1.2), egui::CornerRadius::same(4), outline);
        painter.rect_filled(rect, egui::CornerRadius::same(3), if down { lit } else { idle });
    }

    // D-pad.
    let dpad = p(116.0, 134.0);
    painter.rect_filled(
        Rect::from_center_size(dpad, Vec2::splat(18.0 * scale)),
        egui::CornerRadius::same(2),
        well,
    );
    for (offset, size, index) in [
        (Vec2::new(0.0, -14.0), Vec2::new(15.0, 19.0), 0),
        (Vec2::new(0.0, 14.0), Vec2::new(15.0, 19.0), 1),
        (Vec2::new(-14.0, 0.0), Vec2::new(19.0, 15.0), 2),
        (Vec2::new(14.0, 0.0), Vec2::new(19.0, 15.0), 3),
    ] {
        let rect = Rect::from_center_size(dpad + offset * scale, size * scale);
        painter.rect_filled(rect.expand(1.2), egui::CornerRadius::same(4), outline);
        painter.rect_filled(
            rect,
            egui::CornerRadius::same(3),
            if view.pressed[index] { lit } else { well },
        );
    }

    // Face buttons with PlayStation symbols.
    let faces = p(284.0, 134.0);
    for (offset, index, color, symbol) in [
        (Vec2::new(0.0, -21.0), 7, Color32::from_rgb(76, 205, 172), 't'),
        (Vec2::new(21.0, 0.0), 5, Color32::from_rgb(235, 98, 112), 'o'),
        (Vec2::new(0.0, 21.0), 4, Color32::from_rgb(98, 150, 242), 'x'),
        (Vec2::new(-21.0, 0.0), 6, Color32::from_rgb(226, 140, 192), 's'),
    ] {
        let c = faces + offset * scale;
        let down = view.pressed[index];
        painter.circle_filled(c, 12.5 * scale, outline);
        painter.circle_filled(c, 11.0 * scale, if down { color } else { well });
        let stroke = Stroke::new(1.8 * scale, if down { Color32::WHITE } else { color });
        let k = scale;
        match symbol {
            't' => {
                painter.add(egui::Shape::closed_line(
                    vec![
                        c + Vec2::new(0.0, -5.5 * k),
                        c + Vec2::new(5.0 * k, 3.5 * k),
                        c + Vec2::new(-5.0 * k, 3.5 * k),
                    ],
                    stroke,
                ));
            }
            'o' => {
                painter.circle_stroke(c, 5.0 * k, stroke);
            }
            'x' => {
                painter.line_segment([c + Vec2::new(-4.5 * k, -4.5 * k), c + Vec2::new(4.5 * k, 4.5 * k)], stroke);
                painter.line_segment([c + Vec2::new(-4.5 * k, 4.5 * k), c + Vec2::new(4.5 * k, -4.5 * k)], stroke);
            }
            _ => {
                painter.rect_stroke(
                    Rect::from_center_size(c, Vec2::splat(9.0 * k)),
                    egui::CornerRadius::same(1),
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
        }
    }

    // PS button.
    painter.circle_filled(p(200.0, 168.0), 8.0 * scale, outline);
    painter.circle_filled(p(200.0, 168.0), 6.5 * scale, if view.pressed[16] { lit } else { idle });

    // Analog sticks move live; the ring lights up on L3/R3 click.
    for (center, x, y, down) in [
        (p(160.0, 172.0), view.left_x, view.left_y, view.pressed[14]),
        (p(240.0, 172.0), view.right_x, view.right_y, view.pressed[15]),
    ] {
        painter.circle_filled(center, 22.0 * scale, outline);
        painter.circle_filled(center, 20.0 * scale, well);
        let cap = center + Vec2::new(x.clamp(-1.0, 1.0), -y.clamp(-1.0, 1.0)) * (7.0 * scale);
        painter.circle_filled(cap, 14.0 * scale, shell_light);
        painter.circle_stroke(
            cap,
            14.0 * scale,
            Stroke::new(if down { 2.5 * scale } else { 1.0 }, if down { lit } else { idle }),
        );
        painter.circle_stroke(cap, 8.0 * scale, Stroke::new(1.0_f32, well));
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Pill<'a> {
    Text(&'a str),
    Symbol(char, Color32),
}

/// PlayStation face-button symbol: 't' triangle, 'o' circle, 'x' cross, anything else square.
pub(crate) fn draw_symbol(painter: &egui::Painter, c: Pos2, symbol: char, k: f32, stroke: Stroke) {
    match symbol {
        't' => {
            painter.add(egui::Shape::closed_line(
                vec![
                    c + Vec2::new(0.0, -5.5 * k),
                    c + Vec2::new(5.0 * k, 3.5 * k),
                    c + Vec2::new(-5.0 * k, 3.5 * k),
                ],
                stroke,
            ));
        }
        'o' => {
            painter.circle_stroke(c, 5.0 * k, stroke);
        }
        'x' => {
            painter.line_segment([c + Vec2::new(-4.5 * k, -4.5 * k), c + Vec2::new(4.5 * k, 4.5 * k)], stroke);
            painter.line_segment([c + Vec2::new(-4.5 * k, 4.5 * k), c + Vec2::new(4.5 * k, -4.5 * k)], stroke);
        }
        _ => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(9.0 * k)),
                egui::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Middle,
            );
        }
    }
}

/// One callout label: a small pill naming the PS4 control plus the Xbox output it sends.
/// `side` -1 puts the text left of `edge`, +1 right of it, 0 centres the row on it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn callout_row(
    painter: &egui::Painter,
    edge: f32,
    y: f32,
    side: i8,
    pill: Pill,
    text: &str,
    color: Color32,
    scale: f32,
) {
    let font = egui::FontId::proportional((11.5 * scale).clamp(9.5, 15.0));
    let pill_font = egui::FontId::proportional((8.5 * scale).clamp(7.5, 12.0));
    let text_galley = painter.layout_no_wrap(text.to_uppercase(), font, color);
    let pill_galley = match pill {
        Pill::Text(label) => Some(painter.layout_no_wrap(label.to_owned(), pill_font, color)),
        Pill::Symbol(..) => None,
    };
    let pill_w = pill_galley
        .as_ref()
        .map_or(24.0 * scale, |g| g.size().x + 12.0 * scale);
    let pill_h = 16.0 * scale;
    let gap = 7.0 * scale;
    let text_w = text_galley.size().x;
    let total = text_w + gap + pill_w;
    let start = if side < 0 {
        edge - total
    } else if side > 0 {
        edge
    } else {
        edge - total / 2.0
    };
    let (pill_x, text_x) = if side < 0 {
        (start + text_w + gap, start)
    } else {
        (start, start + pill_w + gap)
    };
    let pill_rect = Rect::from_min_size(
        Pos2::new(pill_x, y - pill_h / 2.0),
        Vec2::new(pill_w, pill_h),
    );
    let outline = match pill {
        Pill::Symbol(_, tint) => tint,
        Pill::Text(_) => color,
    };
    painter.rect_stroke(
        pill_rect,
        egui::CornerRadius::same((pill_h / 2.0) as u8),
        Stroke::new(1.2_f32, outline),
        egui::StrokeKind::Inside,
    );
    match (pill, pill_galley) {
        (Pill::Symbol(symbol, tint), _) => {
            draw_symbol(painter, pill_rect.center(), symbol, 0.95 * scale.max(0.7), Stroke::new(1.4_f32, tint));
        }
        (_, Some(galley)) => {
            let pos = pill_rect.center() - galley.size() / 2.0;
            painter.galley(pos, galley, color);
        }
        _ => {}
    }
    painter.galley(Pos2::new(text_x, y - text_galley.size().y / 2.0), text_galley, color);
}

/// Large line-art controller with callouts showing the Xbox output mapped to each PS4 control.
pub(crate) fn draw_controller_map(ui: &mut egui::Ui, view: &PadView, bindings: &[String]) {
    let width = ui.available_width().max(520.0);
    let scale = (width / 860.0).min(1.25);
    let (area, _) = ui.allocate_exact_size(Vec2::new(width, 440.0 * scale), Sense::hover());
    let painter = ui.painter_at(area);
    painter.rect_filled(area, egui::CornerRadius::same(12), Color32::from_rgb(13, 16, 22));

    let origin = Pos2::new(area.center().x - 430.0 * scale, area.top());
    let p = |x: f32, y: f32| origin + Vec2::new(x, y) * scale;
    // Controller space (the 400-wide design used by the compact preview) -> diagram space.
    let c = |x: f32, y: f32| p(430.0 + (x - 200.0) * 1.25, 86.0 + (y - 66.0) * 1.25);
    let u = scale * 1.25;

    let line = Color32::from_rgb(214, 221, 233);
    let dim = Color32::from_rgb(140, 151, 170);
    let label = Color32::from_rgb(222, 228, 238);
    let lit = Color32::from_rgb(110, 145, 255);
    let well = Color32::from_rgb(16, 19, 26);
    let thin = Stroke::new(1.4 * scale, line);
    let value = |index: usize| -> String {
        bindings.get(index).cloned().unwrap_or_else(|| "Unmapped".to_owned())
    };
    let tint = |index: usize| if view.pressed[index] { lit } else { label };

    // Trigger tabs (L2/R2) fill as they are pulled.
    for (x0, level, name) in [(100.0_f32, view.trigger_left, "L2"), (264.0_f32, view.trigger_right, "R2")] {
        let rect = Rect::from_min_max(c(x0, 70.0), c(x0 + 36.0, 95.0));
        let r = (14.0 * u) as u8;
        let radius = egui::CornerRadius { nw: r, ne: r, sw: 0, se: 0 };
        painter.rect_filled(rect, radius, well);
        let fill = rect.height() * level.clamp(0.0, 1.0);
        if fill > 0.5 {
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - fill), rect.right_bottom()),
                radius,
                lit,
            );
        }
        painter.rect_stroke(rect, radius, thin, egui::StrokeKind::Inside);
        painter.text(
            Pos2::new(rect.center().x, rect.top() + rect.height() * 0.4),
            egui::Align2::CENTER_CENTER,
            name,
            egui::FontId::proportional((8.5 * scale).clamp(7.0, 11.0)),
            if level > 0.5 { Color32::WHITE } else { dim },
        );
    }

    // Body: dark fill, bright outline and a thin inner rim.
    let (design_points, triangles) = body_shape();
    let screen: Vec<Pos2> = design_points.iter().map(|q| c(q.x, q.y)).collect();
    let mut mesh = egui::Mesh::default();
    for (q, s) in design_points.iter().zip(&screen) {
        let t = ((q.y - 90.0) / 146.0).clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| (a + (b - a) * t) as u8;
        mesh.colored_vertex(*s, Color32::from_rgb(mix(30.0, 15.0), mix(36.0, 18.0), mix(48.0, 25.0)));
    }
    mesh.indices = triangles.clone();
    painter.add(egui::Shape::mesh(mesh));
    let rim: Vec<Pos2> = design_points
        .iter()
        .map(|q| c(200.0 + (q.x - 200.0) * 0.965, 160.0 + (q.y - 160.0) * 0.94))
        .collect();
    painter.add(egui::Shape::closed_line(rim, Stroke::new(1.0_f32, Color32::from_rgb(60, 68, 86))));
    painter.add(egui::Shape::closed_line(screen, Stroke::new(1.7 * scale, line)));

    // Bumpers (L1 left, R1 right): hollow strips along the shoulders.
    for (mirrored, index) in [(true, 8_usize), (false, 9_usize)] {
        let x = |v: f32| if mirrored { 400.0 - v } else { v };
        let strip: Vec<Pos2> = [(246.0_f32, 96.0_f32), (270.0, 96.0), (288.0, 98.0), (300.0, 104.0)]
            .iter()
            .map(|&(vx, vy)| c(x(vx), vy))
            .collect();
        painter.add(egui::Shape::line(strip.clone(), Stroke::new(9.0 * u, line)));
        painter.add(egui::Shape::line(
            strip,
            Stroke::new(7.0 * u, if view.pressed[index] { lit } else { well }),
        ));
    }

    // Light bar, touchpad (with dot texture), Share and Options.
    painter.line_segment(
        [c(172.0, 98.0), c(228.0, 98.0)],
        Stroke::new(2.0 * scale, Color32::from_rgb(70, 100, 220)),
    );
    let pad = Rect::from_min_max(c(160.0, 106.0), c(240.0, 146.0));
    painter.rect_filled(pad, egui::CornerRadius::same((10.0 * scale) as u8), if view.pressed[17] { lit } else { well });
    painter.rect_stroke(pad, egui::CornerRadius::same((10.0 * scale) as u8), thin, egui::StrokeKind::Inside);
    for row in 0..8 {
        for col in 0..17 {
            painter.circle_filled(
                c(168.0 + col as f32 * 4.5, 113.0 + row as f32 * 4.0),
                0.75 * scale,
                Color32::from_rgb(110, 121, 142),
            );
        }
    }
    for (x, index) in [(144.0_f32, 13_usize), (256.0_f32, 12_usize)] {
        let rect = Rect::from_center_size(c(x, 110.0), Vec2::new(6.0, 12.0) * u);
        painter.rect_filled(rect, egui::CornerRadius::same(3), if view.pressed[index] { lit } else { well });
        painter.rect_stroke(rect, egui::CornerRadius::same(3), thin, egui::StrokeKind::Inside);
    }

    // D-pad: a plus shape with an arrow in each arm.
    let dpad = c(116.0, 134.0);
    let arms = [
        (Vec2::new(0.0, -14.0), Vec2::new(15.0, 19.0), 0_usize, Vec2::new(0.0, -1.0)),
        (Vec2::new(0.0, 14.0), Vec2::new(15.0, 19.0), 1, Vec2::new(0.0, 1.0)),
        (Vec2::new(-14.0, 0.0), Vec2::new(19.0, 15.0), 2, Vec2::new(-1.0, 0.0)),
        (Vec2::new(14.0, 0.0), Vec2::new(19.0, 15.0), 3, Vec2::new(1.0, 0.0)),
    ];
    let centre_rect = Rect::from_center_size(dpad, Vec2::splat(18.0 * u));
    painter.rect_filled(centre_rect.expand(1.4 * scale), egui::CornerRadius::same(2), line);
    for (offset, size, _, _) in arms {
        let rect = Rect::from_center_size(dpad + offset * u, size * u);
        painter.rect_filled(rect.expand(1.4 * scale), egui::CornerRadius::same(4), line);
    }
    painter.rect_filled(centre_rect, egui::CornerRadius::same(2), well);
    for (offset, size, index, dir) in arms {
        let rect = Rect::from_center_size(dpad + offset * u, size * u);
        painter.rect_filled(rect, egui::CornerRadius::same(3), if view.pressed[index] { lit } else { well });
        let middle = rect.center();
        let normal = Vec2::new(-dir.y, dir.x);
        let base = middle - dir * (2.5 * u);
        painter.add(egui::Shape::convex_polygon(
            vec![middle + dir * (5.5 * u), base + normal * (4.0 * u), base - normal * (4.0 * u)],
            if view.pressed[index] { Color32::WHITE } else { dim },
            Stroke::NONE,
        ));
    }

    // Face buttons.
    let faces = c(284.0, 134.0);
    let face_colors = [
        (Vec2::new(0.0, -21.0), 7_usize, Color32::from_rgb(76, 205, 172), 't'),
        (Vec2::new(21.0, 0.0), 5, Color32::from_rgb(235, 98, 112), 'o'),
        (Vec2::new(0.0, 21.0), 4, Color32::from_rgb(98, 150, 242), 'x'),
        (Vec2::new(-21.0, 0.0), 6, Color32::from_rgb(226, 140, 192), 's'),
    ];
    for (offset, index, color, symbol) in face_colors {
        let center = faces + offset * u;
        let down = view.pressed[index];
        painter.circle_filled(center, 11.5 * u, if down { color } else { well });
        painter.circle_stroke(center, 11.5 * u, thin);
        draw_symbol(&painter, center, symbol, u, Stroke::new(1.6 * scale, if down { Color32::WHITE } else { line }));
    }

    // PS button and speaker dots.
    painter.circle_filled(c(200.0, 168.0), 6.5 * u, if view.pressed[16] { lit } else { well });
    painter.circle_stroke(c(200.0, 168.0), 7.5 * u, thin);
    for row in 0..2 {
        for col in 0..3 {
            painter.circle_filled(
                c(195.0 + col as f32 * 5.0, 154.0 + row as f32 * 4.0),
                0.9 * scale,
                Color32::from_rgb(110, 121, 142),
            );
        }
    }

    // Analog sticks (live) with a click highlight for L3/R3.
    let sticks = [
        (c(160.0, 172.0), view.left_x, view.left_y, view.pressed[14]),
        (c(240.0, 172.0), view.right_x, view.right_y, view.pressed[15]),
    ];
    for (center, x, y, down) in sticks {
        painter.circle_filled(center, 22.0 * u, well);
        painter.circle_stroke(center, 22.0 * u, thin);
        let cap = center + Vec2::new(x.clamp(-1.0, 1.0), -y.clamp(-1.0, 1.0)) * (7.0 * u);
        painter.circle_filled(cap, 15.0 * u, if down { lit } else { well });
        painter.circle_stroke(cap, 15.0 * u, thin);
        painter.circle_stroke(cap, 9.0 * u, Stroke::new(1.0_f32, dim));
    }

    // ---- Callouts ----
    let leader = Stroke::new(1.1 * scale, dim);
    let dot = |at: Pos2| painter.circle_filled(at, 2.4 * scale, label);
    let (left_edge, right_edge) = (p(240.0, 0.0).x, p(620.0, 0.0).x);
    let inset = 6.0 * scale;

    // L2 / R2: elbow lines from above.
    let row_y = p(0.0, 74.0).y;
    let l2 = c(118.0, 70.0);
    painter.add(egui::Shape::line(vec![Pos2::new(left_edge + inset, row_y), Pos2::new(l2.x, row_y), l2], leader));
    dot(l2);
    callout_row(&painter, left_edge, row_y, -1, Pill::Text("L2"), &value(10), tint(10), scale);
    let r2 = c(282.0, 70.0);
    painter.add(egui::Shape::line(vec![Pos2::new(right_edge - inset, row_y), Pos2::new(r2.x, row_y), r2], leader));
    dot(r2);
    callout_row(&painter, right_edge, row_y, 1, Pill::Text("R2"), &value(11), tint(11), scale);

    // L1 / R1: straight lines to the bumpers.
    let l1 = c(130.0, 97.0);
    painter.add(egui::Shape::line(vec![Pos2::new(left_edge + inset, l1.y), l1], leader));
    dot(l1);
    callout_row(&painter, left_edge, l1.y, -1, Pill::Text("L1"), &value(8), tint(8), scale);
    let r1 = c(270.0, 97.0);
    painter.add(egui::Shape::line(vec![Pos2::new(right_edge - inset, r1.y), r1], leader));
    dot(r1);
    callout_row(&painter, right_edge, r1.y, 1, Pill::Text("R1"), &value(9), tint(9), scale);

    // Touchpad click: label at the top centre with a line straight down to the pad.
    let pad_top = c(200.0, 106.0);
    let touch_y = p(0.0, 16.0).y;
    painter.add(egui::Shape::line(
        vec![Pos2::new(pad_top.x, touch_y + 12.0 * scale), pad_top],
        leader,
    ));
    dot(pad_top);
    callout_row(&painter, pad_top.x, touch_y, 0, Pill::Text("TOUCHPAD"), &value(17), tint(17), scale);

    // Share / Options: labels sit a little outward so they clear the touchpad line.
    for (x, index, name, shift) in [
        (144.0_f32, 13_usize, "SHARE", -22.0_f32),
        (256.0_f32, 12_usize, "OPTIONS", 22.0_f32),
    ] {
        let at = c(x, 104.0);
        let y = p(0.0, 46.0).y;
        painter.add(egui::Shape::line(vec![Pos2::new(at.x, y + 12.0 * scale), at], leader));
        dot(at);
        callout_row(&painter, at.x + shift * scale, y, 0, Pill::Text(name), &value(index), tint(index), scale);
    }

    // D-pad bracket on the left.
    let row_gap = 19.0 * scale;
    let bracket_x = left_edge + 14.0 * scale;
    let serif = 5.0 * scale;
    let dpad_rows = [(0_usize, "UP"), (1, "DOWN"), (2, "LEFT"), (3, "RIGHT")];
    let top = dpad.y - 1.5 * row_gap;
    let bottom = dpad.y + 1.5 * row_gap;
    painter.add(egui::Shape::line(
        vec![
            Pos2::new(bracket_x - serif, top - 7.0 * scale),
            Pos2::new(bracket_x, top - 7.0 * scale),
            Pos2::new(bracket_x, bottom + 7.0 * scale),
            Pos2::new(bracket_x - serif, bottom + 7.0 * scale),
        ],
        leader,
    ));
    let dpad_anchor = Pos2::new(dpad.x - 24.0 * u, dpad.y);
    painter.line_segment([Pos2::new(bracket_x, dpad.y), dpad_anchor], leader);
    dot(dpad_anchor);
    for (i, (index, name)) in dpad_rows.iter().enumerate() {
        let y = top + i as f32 * row_gap;
        callout_row(&painter, bracket_x - 9.0 * scale, y, -1, Pill::Text(name), &value(*index), tint(*index), scale);
    }

    // Face-button bracket on the right.
    let bracket_x = right_edge - 14.0 * scale;
    let top = faces.y - 1.5 * row_gap;
    let bottom = faces.y + 1.5 * row_gap;
    painter.add(egui::Shape::line(
        vec![
            Pos2::new(bracket_x + serif, top - 7.0 * scale),
            Pos2::new(bracket_x, top - 7.0 * scale),
            Pos2::new(bracket_x, bottom + 7.0 * scale),
            Pos2::new(bracket_x + serif, bottom + 7.0 * scale),
        ],
        leader,
    ));
    let faces_anchor = Pos2::new(faces.x + 33.0 * u, faces.y);
    painter.line_segment([Pos2::new(bracket_x, faces.y), faces_anchor], leader);
    dot(faces_anchor);
    let face_rows = [
        (4_usize, 'x', Color32::from_rgb(98, 150, 242)),
        (5, 'o', Color32::from_rgb(235, 98, 112)),
        (6, 's', Color32::from_rgb(226, 140, 192)),
        (7, 't', Color32::from_rgb(76, 205, 172)),
    ];
    for (i, (index, symbol, color)) in face_rows.iter().enumerate() {
        let y = top + i as f32 * row_gap;
        callout_row(&painter, bracket_x + 9.0 * scale, y, 1, Pill::Symbol(*symbol, *color), &value(*index), tint(*index), scale);
    }

    // Sticks: lines run down and out to labels at the bottom.
    let line_y = p(0.0, 371.0).y;
    let left_stick = sticks[0].0;
    let right_stick = sticks[1].0;
    let left_bottom = Pos2::new(left_stick.x, left_stick.y + 22.0 * u);
    painter.add(egui::Shape::line(
        vec![left_bottom, Pos2::new(left_bottom.x, line_y), Pos2::new(left_edge + inset, line_y)],
        leader,
    ));
    dot(left_bottom);
    callout_row(&painter, left_edge, line_y - 15.0 * scale, -1, Pill::Text("L"), "Left stick", dim, scale);
    callout_row(&painter, left_edge, line_y + 15.0 * scale, -1, Pill::Text("L3"), &value(14), tint(14), scale);
    let right_bottom = Pos2::new(right_stick.x, right_stick.y + 22.0 * u);
    painter.add(egui::Shape::line(
        vec![right_bottom, Pos2::new(right_bottom.x, line_y), Pos2::new(right_edge - inset, line_y)],
        leader,
    ));
    dot(right_bottom);
    callout_row(&painter, right_edge, line_y - 15.0 * scale, 1, Pill::Text("R"), "Right stick", dim, scale);
    callout_row(&painter, right_edge, line_y + 15.0 * scale, 1, Pill::Text("R3"), &value(15), tint(15), scale);

    // PS button: label at the bottom centre, line straight down from the button.
    let ps = c(200.0, 168.0);
    let ps_edge = Pos2::new(ps.x, ps.y + 7.5 * u);
    let ps_y = p(0.0, 412.0).y;
    painter.add(egui::Shape::line(vec![ps_edge, Pos2::new(ps.x, ps_y - 12.0 * scale)], leader));
    dot(ps_edge);
    callout_row(&painter, ps.x, ps_y, 0, Pill::Text("PS"), &value(16), tint(16), scale);
}

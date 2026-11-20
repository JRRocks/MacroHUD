//! Icons of the HUD: the mock-up's SVG drawings (16 × 16 grid), redrawn with plain shapes.

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};

use super::style;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// Six dots: the HUD can be dragged here.
    Grip,
    Up,
    Down,
    Close,
    Play,
    Stop,
    Dot,
    Check,
    Warning,
    Info,
    Error,
    /// Mouse, left button filled.
    Mouse,
    /// Mouse with click marks (« Clic répété »).
    Repeat,
    /// Keyboard key (« Touche maintenue »).
    Key,
}

/// Paints `icon` centred in `rect`, scaled to the smaller side.
pub fn paint(p: &Painter, icon: Icon, rect: Rect, color: Color32) {
    let s = rect.width().min(rect.height()) / 16.0;
    let origin = rect.center() - vec2(8.0 * s, 8.0 * s);
    let at = |x: f32, y: f32| pos2(origin.x + x * s, origin.y + y * s);
    let radius = |r: f32| CornerRadius::same((r * s).round() as u8);
    match icon {
        Icon::Grip => {
            for (x, y) in [
                (5.5, 3.5),
                (10.5, 3.5),
                (5.5, 8.0),
                (10.5, 8.0),
                (5.5, 12.5),
                (10.5, 12.5),
            ] {
                p.circle_filled(at(x, y), 1.5 * s, color);
            }
        }
        Icon::Up => polyline(
            p,
            &[at(3.0, 10.5), at(8.0, 5.5), at(13.0, 10.5)],
            2.2 * s,
            color,
        ),
        Icon::Down => polyline(
            p,
            &[at(3.0, 5.5), at(8.0, 10.5), at(13.0, 5.5)],
            2.2 * s,
            color,
        ),
        Icon::Close => {
            polyline(p, &[at(4.0, 4.0), at(12.0, 12.0)], 2.2 * s, color);
            polyline(p, &[at(12.0, 4.0), at(4.0, 12.0)], 2.2 * s, color);
        }
        Icon::Play => {
            let points = vec![at(4.5, 2.5), at(13.5, 8.0), at(4.5, 13.5)];
            p.add(Shape::convex_polygon(points, color, Stroke::new(s, color)));
        }
        Icon::Stop => {
            p.rect_filled(
                Rect::from_min_max(at(3.0, 3.0), at(13.0, 13.0)),
                radius(1.5),
                color,
            );
        }
        Icon::Dot => {
            p.circle_filled(at(8.0, 8.0), 5.5 * s, color);
        }
        Icon::Check => polyline(
            p,
            &[at(2.5, 8.5), at(6.0, 12.0), at(13.5, 4.0)],
            2.4 * s,
            color,
        ),
        Icon::Warning => {
            let points = vec![at(8.0, 1.2), at(15.4, 14.6), at(0.6, 14.6)];
            p.add(Shape::convex_polygon(
                points,
                color,
                Stroke::new(1.2 * s, color),
            ));
            let bar = Rect::from_min_max(at(7.1, 5.4), at(8.9, 10.4));
            p.rect_filled(bar, radius(0.9), style::SIGN_INK);
            p.circle_filled(at(8.0, 12.3), 1.05 * s, style::SIGN_INK);
        }
        Icon::Info => {
            p.circle_filled(at(8.0, 8.0), 7.2 * s, color);
            p.circle_filled(at(8.0, 4.7), 1.1 * s, style::SIGN_INK);
            p.rect_filled(
                Rect::from_min_max(at(7.1, 6.8), at(8.9, 12.2)),
                radius(0.9),
                style::SIGN_INK,
            );
        }
        Icon::Error => {
            p.circle_filled(at(8.0, 8.0), 7.2 * s, color);
            p.rect_filled(
                Rect::from_min_max(at(7.1, 3.8), at(8.9, 9.2)),
                radius(0.9),
                style::SIGN_INK,
            );
            p.circle_filled(at(8.0, 11.7), 1.1 * s, style::SIGN_INK);
        }
        Icon::Mouse => {
            let body = Rect::from_min_max(at(3.5, 1.0), at(12.5, 15.0));
            let line = Stroke::new(1.6 * s, color);
            p.rect_stroke(body, radius(4.5), line, StrokeKind::Middle);
            // Left button: quarter of the rounded top, as a convex polygon.
            let mut button = vec![at(8.0, 1.0), at(8.0, 6.5), at(3.5, 6.5), at(3.5, 5.5)];
            for k in 1..6 {
                let angle = (180.0 + 90.0 * k as f32 / 6.0_f32).to_radians();
                button.push(at(8.0 + 4.5 * angle.cos(), 5.5 + 4.5 * angle.sin()));
            }
            p.add(Shape::convex_polygon(button, color, Stroke::NONE));
            p.line_segment([at(3.5, 6.5), at(12.5, 6.5)], line);
        }
        Icon::Repeat => {
            let body = Rect::from_min_max(at(5.5, 2.0), at(14.5, 15.0));
            let line = Stroke::new(1.6 * s, color);
            p.rect_stroke(body, radius(4.5), line, StrokeKind::Middle);
            p.line_segment([at(10.0, 2.0), at(10.0, 7.0)], line);
            p.line_segment([at(5.5, 7.0), at(14.5, 7.0)], line);
            polyline(p, &[at(1.0, 3.2), at(3.4, 4.4)], 1.6 * s, color);
            polyline(p, &[at(0.6, 7.0), at(3.2, 7.0)], 1.6 * s, color);
            polyline(p, &[at(1.0, 10.8), at(3.4, 9.6)], 1.6 * s, color);
        }
        Icon::Key => {
            let body = Rect::from_min_max(at(1.2, 2.2), at(14.8, 13.8));
            p.rect_stroke(
                body,
                radius(2.4),
                Stroke::new(1.6 * s, color),
                StrokeKind::Middle,
            );
            p.rect_filled(
                Rect::from_min_max(at(4.0, 4.6), at(12.0, 9.8)),
                radius(1.2),
                color,
            );
        }
    }
}

/// Polyline with round caps and joins, like the SVG (`stroke-linecap: round`).
fn polyline(p: &Painter, points: &[Pos2], width: f32, color: Color32) {
    for pair in points.windows(2) {
        p.line_segment([pair[0], pair[1]], Stroke::new(width, color));
    }
    for &point in points {
        p.circle_filled(point, width / 2.0, color);
    }
}

//! Íconos dibujados a mano con el Painter (trazo fino, estilo del mockup).
//! Todos reciben el centro `c` y el tamaño `s` de la caja donde entran.

use eframe::egui::{Color32, Painter, Pos2, Shape, Stroke, StrokeKind, Rect, Vec2, pos2, vec2};
use std::f32::consts::PI;

const W: f32 = 1.6;

fn p(c: Pos2, s: f32, x: f32, y: f32) -> Pos2 {
    pos2(c.x + x * s, c.y + y * s)
}

fn arc(c: Pos2, r: f32, from: f32, to: f32) -> Vec<Pos2> {
    let n = (((to - from).abs() * r) / 2.0).ceil().max(8.0) as usize;
    (0..=n)
        .map(|i| {
            let t = from + (to - from) * i as f32 / n as f32;
            pos2(c.x + r * t.cos(), c.y + r * t.sin())
        })
        .collect()
}

pub fn prev(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    pt.line_segment([p(c, s, -0.32, -0.3), p(c, s, -0.32, 0.3)], st);
    pt.add(Shape::closed_line(
        vec![p(c, s, 0.3, -0.3), p(c, s, -0.18, 0.0), p(c, s, 0.3, 0.3)],
        st,
    ));
}

pub fn next(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    pt.line_segment([p(c, s, 0.32, -0.3), p(c, s, 0.32, 0.3)], st);
    pt.add(Shape::closed_line(
        vec![p(c, s, -0.3, -0.3), p(c, s, 0.18, 0.0), p(c, s, -0.3, 0.3)],
        st,
    ));
}

pub fn restart(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    let r = 0.32 * s;
    // de la izquierda, por abajo y la derecha, hasta arriba a la izquierda
    pt.add(Shape::line(arc(c, r, PI, -0.75 * PI), st));
    let tip = pos2(c.x + r * (-0.75 * PI).cos(), c.y + r * (-0.75 * PI).sin());
    let h = 0.2 * s;
    pt.add(Shape::line(vec![tip + vec2(0.0, -h), tip, tip + vec2(h, 0.0)], st));
}

/// Ícono dentro del botón grande: círculo + triángulo (o pausa).
pub fn play_badge(pt: &Painter, c: Pos2, s: f32, col: Color32, playing: bool) {
    pt.circle_stroke(c, 0.45 * s, Stroke::new(W, col));
    if playing {
        let st = Stroke::new(W + 0.2, col);
        pt.line_segment([p(c, s, -0.1, -0.16), p(c, s, -0.1, 0.16)], st);
        pt.line_segment([p(c, s, 0.1, -0.16), p(c, s, 0.1, 0.16)], st);
    } else {
        pt.add(Shape::convex_polygon(
            vec![p(c, s, -0.09, -0.17), p(c, s, 0.19, 0.0), p(c, s, -0.09, 0.17)],
            col,
            Stroke::NONE,
        ));
    }
}

pub fn playlist(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    pt.line_segment([p(c, s, -0.4, -0.28), p(c, s, 0.25, -0.28)], st);
    pt.line_segment([p(c, s, -0.4, 0.0), p(c, s, 0.25, 0.0)], st);
    pt.line_segment([p(c, s, -0.4, 0.28), p(c, s, 0.0, 0.28)], st);
    pt.add(Shape::convex_polygon(
        vec![p(c, s, 0.15, 0.14), p(c, s, 0.42, 0.28), p(c, s, 0.15, 0.42)],
        col,
        Stroke::NONE,
    ));
}

pub fn volume(pt: &Painter, c: Pos2, s: f32, col: Color32, level: f32, muted: bool) {
    let st = Stroke::new(W, col);
    pt.add(Shape::closed_line(
        vec![
            p(c, s, -0.42, -0.12),
            p(c, s, -0.22, -0.12),
            p(c, s, 0.0, -0.34),
            p(c, s, 0.0, 0.34),
            p(c, s, -0.22, 0.12),
            p(c, s, -0.42, 0.12),
        ],
        st,
    ));
    let o = p(c, s, 0.02, 0.0);
    if muted || level <= 0.0 {
        pt.line_segment([p(c, s, 0.16, -0.13), p(c, s, 0.42, 0.13)], st);
        pt.line_segment([p(c, s, 0.16, 0.13), p(c, s, 0.42, -0.13)], st);
        return;
    }
    pt.add(Shape::line(arc(o, 0.17 * s, -0.3 * PI, 0.3 * PI), st));
    if level > 0.5 {
        pt.add(Shape::line(arc(o, 0.34 * s, -0.3 * PI, 0.3 * PI), st));
    }
}

pub fn subtitles(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    let r = Rect::from_center_size(c, vec2(0.84 * s, 0.68 * s));
    pt.rect_stroke(r, 2.5, st, StrokeKind::Middle);
    pt.line_segment([p(c, s, -0.24, 0.06), p(c, s, -0.08, 0.06)], st);
    pt.line_segment([p(c, s, 0.02, 0.06), p(c, s, 0.24, 0.06)], st);
    pt.line_segment([p(c, s, -0.24, 0.19), p(c, s, 0.08, 0.19)], st);
}

pub fn gear(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    let teeth = 8;
    let pts: Vec<Pos2> = (0..teeth * 4)
        .map(|i| {
            let a = (i as f32 + 0.5) / (teeth * 4) as f32 * 2.0 * PI;
            let r = if (i / 2) % 2 == 0 { 0.44 } else { 0.32 } * s;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    pt.add(Shape::closed_line(pts, st));
    pt.circle_stroke(c, 0.13 * s, st);
}

pub fn fullscreen(pt: &Painter, c: Pos2, s: f32, col: Color32, active: bool) {
    let st = Stroke::new(W, col);
    let (a, b) = (0.36, 0.14); // esquina y largo de cada brazo
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        // Afuera = maximizar, adentro = salir de pantalla completa.
        let (corner, dir) = if active {
            (p(c, s, sx * b, sy * b), -1.0)
        } else {
            (p(c, s, sx * a, sy * a), 1.0)
        };
        let l = (a - b) * s;
        pt.add(Shape::line(
            vec![
                corner + Vec2::new(0.0, -sy * l * dir),
                corner,
                corner + Vec2::new(-sx * l * dir, 0.0),
            ],
            st,
        ));
    }
}

pub fn win_minimize(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    pt.line_segment([p(c, s, -0.32, 0.0), p(c, s, 0.32, 0.0)], Stroke::new(W, col));
}

pub fn win_maximize(pt: &Painter, c: Pos2, s: f32, col: Color32, maximized: bool) {
    let st = Stroke::new(W, col);
    if maximized {
        let back = Rect::from_center_size(p(c, s, 0.08, -0.08), vec2(0.5 * s, 0.5 * s));
        pt.rect_stroke(back, 1.5, st, StrokeKind::Middle);
        let front = Rect::from_center_size(p(c, s, -0.08, 0.08), vec2(0.5 * s, 0.5 * s));
        pt.rect_filled(front, 1.5, crate::theme::BG_WINDOW);
        pt.rect_stroke(front, 1.5, st, StrokeKind::Middle);
    } else {
        let r = Rect::from_center_size(c, vec2(0.62 * s, 0.62 * s));
        pt.rect_stroke(r, 1.5, st, StrokeKind::Middle);
    }
}

pub fn close(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    pt.line_segment([p(c, s, -0.3, -0.3), p(c, s, 0.3, 0.3)], st);
    pt.line_segment([p(c, s, -0.3, 0.3), p(c, s, 0.3, -0.3)], st);
}

pub fn film(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W + 0.3, col);
    let r = Rect::from_center_size(c, vec2(0.8 * s, 0.9 * s));
    pt.rect_stroke(r, 3.0, st, StrokeKind::Middle);
    let (l, rr) = (r.left() + 0.2 * s, r.right() - 0.2 * s);
    pt.line_segment([pos2(l, r.top()), pos2(l, r.bottom())], st);
    pt.line_segment([pos2(rr, r.top()), pos2(rr, r.bottom())], st);
    pt.line_segment([pos2(l, c.y), pos2(rr, c.y)], st);
    for y in [-0.22, 0.0, 0.22] {
        let yy = c.y + y * s;
        pt.line_segment([pos2(r.left(), yy), pos2(l, yy)], st);
        pt.line_segment([pos2(rr, yy), pos2(r.right(), yy)], st);
    }
}

pub fn music_note(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W + 0.6, col);
    pt.circle_filled(p(c, s, -0.18, 0.26), 0.13 * s, col);
    pt.circle_filled(p(c, s, 0.28, 0.16), 0.13 * s, col);
    pt.line_segment([p(c, s, -0.06, 0.26), p(c, s, -0.06, -0.34)], st);
    pt.line_segment([p(c, s, 0.4, 0.16), p(c, s, 0.4, -0.44)], st);
    pt.add(Shape::convex_polygon(
        vec![
            p(c, s, -0.06, -0.34),
            p(c, s, 0.4, -0.44),
            p(c, s, 0.4, -0.3),
            p(c, s, -0.06, -0.2),
        ],
        col,
        Stroke::NONE,
    ));
}

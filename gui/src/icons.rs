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

/// Convierte coordenadas de una caja de 24×24 (estilo lucide) a la caja `s`.
fn l(c: Pos2, s: f32, x: f32, y: f32) -> Pos2 {
    pos2(c.x + (x - 12.0) / 20.0 * s, c.y + (y - 12.0) / 20.0 * s)
}

fn l_arc(c: Pos2, s: f32, cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Vec<Pos2> {
    arc(l(c, s, cx, cy), r / 20.0 * s, from, to)
}

pub fn shuffle(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    // flechas de la derecha
    pt.add(Shape::line(vec![l(c, s, 18.0, 2.5), l(c, s, 21.5, 6.0), l(c, s, 18.0, 9.5)], st));
    pt.add(Shape::line(vec![l(c, s, 18.0, 14.5), l(c, s, 21.5, 18.0), l(c, s, 18.0, 21.5)], st));
    // cruce: abajo-izquierda → arriba-derecha
    pt.add(Shape::line(
        vec![
            l(c, s, 2.5, 18.0),
            l(c, s, 4.5, 17.8),
            l(c, s, 6.6, 16.6),
            l(c, s, 12.6, 8.0),
            l(c, s, 14.8, 6.4),
            l(c, s, 16.8, 6.0),
            l(c, s, 21.0, 6.0),
        ],
        st,
    ));
    // arriba-izquierda → (por debajo del cruce) → abajo-derecha
    pt.add(Shape::line(
        vec![l(c, s, 2.5, 6.0), l(c, s, 4.6, 6.1), l(c, s, 6.6, 7.2), l(c, s, 8.0, 9.0)],
        st,
    ));
    pt.add(Shape::line(
        vec![l(c, s, 11.6, 14.6), l(c, s, 13.0, 16.6), l(c, s, 15.0, 17.8), l(c, s, 17.0, 18.0), l(c, s, 21.0, 18.0)],
        st,
    ));
}

pub fn repeat(pt: &Painter, c: Pos2, s: f32, col: Color32, one: bool) {
    let st = Stroke::new(W, col);
    pt.add(Shape::line(vec![l(c, s, 17.0, 2.5), l(c, s, 20.5, 6.0), l(c, s, 17.0, 9.5)], st));
    let mut top = vec![l(c, s, 3.5, 11.0)];
    top.extend(l_arc(c, s, 7.5, 10.0, 4.0, PI, 1.5 * PI));
    top.push(l(c, s, 20.5, 6.0));
    pt.add(Shape::line(top, st));

    pt.add(Shape::line(vec![l(c, s, 7.0, 14.5), l(c, s, 3.5, 18.0), l(c, s, 7.0, 21.5)], st));
    let mut bottom = vec![l(c, s, 20.5, 13.0)];
    bottom.extend(l_arc(c, s, 16.5, 14.0, 4.0, 0.0, 0.5 * PI));
    bottom.push(l(c, s, 3.5, 18.0));
    pt.add(Shape::line(bottom, st));

    if one {
        let fid = eframe::egui::FontId::new(s * 0.42, eframe::egui::FontFamily::Name("bold".into()));
        pt.text(l(c, s, 12.0, 12.2), eframe::egui::Align2::CENTER_CENTER, "1", fid, col);
    }
}

/// "+" que puede girar (animación del botón Agregar).
pub fn plus(pt: &Painter, c: Pos2, s: f32, col: Color32, angle: f32, width: f32) {
    let st = Stroke::new(width, col);
    let h = s * 0.5;
    let (sin, cos) = angle.sin_cos();
    let rot = |x: f32, y: f32| pos2(c.x + x * cos - y * sin, c.y + x * sin + y * cos);
    pt.line_segment([rot(-h, 0.0), rot(h, 0.0)], st);
    pt.line_segment([rot(0.0, -h), rot(0.0, h)], st);
}

/// Círculo con una "i" (botón de información).
pub fn info(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    pt.circle_stroke(c, 0.42 * s, st);
    pt.circle_filled(p(c, s, 0.0, -0.19), 0.06 * s + 0.4, col);
    pt.line_segment([p(c, s, 0.0, -0.04), p(c, s, 0.0, 0.22)], Stroke::new(W + 0.2, col));
}

/// Cámara de fotos (capturar fotograma).
pub fn camera(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    let body = Rect::from_center_size(p(c, s, 0.0, 0.06), vec2(0.9 * s, 0.62 * s));
    pt.rect_stroke(body, 2.5, st, StrokeKind::Middle);
    pt.add(Shape::line(
        vec![p(c, s, -0.18, -0.25), p(c, s, -0.1, -0.38), p(c, s, 0.1, -0.38), p(c, s, 0.18, -0.25)],
        st,
    ));
    pt.circle_stroke(p(c, s, 0.0, 0.07), 0.17 * s, st);
}

/// Tres deslizadores verticales (botón de efectos de audio).
pub fn sliders(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    for (x, knob) in [(-0.3, 0.12), (0.0, -0.16), (0.3, 0.05)] {
        pt.line_segment([p(c, s, x, -0.38), p(c, s, x, 0.38)], st);
        pt.circle_filled(p(c, s, x, knob), 0.11 * s, crate::theme::BG_WINDOW);
        pt.circle_stroke(p(c, s, x, knob), 0.11 * s, st);
    }
}

/// Tilde ✓ (actualización instalada).
pub fn check(pt: &Painter, c: Pos2, s: f32, col: Color32, width: f32) {
    pt.add(Shape::line(
        vec![p(c, s, -0.32, 0.02), p(c, s, -0.09, 0.25), p(c, s, 0.34, -0.22)],
        Stroke::new(width, col),
    ));
}

/// Flecha hacia abajo sobre una bandeja (buscar actualizaciones).
/// `drop` (0..1) baja la flecha un poco: se usa al pasar el mouse.
pub fn download(pt: &Painter, c: Pos2, s: f32, col: Color32, drop: f32) {
    let st = Stroke::new(W, col);
    let dy = 0.08 * drop;
    pt.line_segment([p(c, s, 0.0, -0.4 + dy), p(c, s, 0.0, 0.12 + dy)], st);
    pt.add(Shape::line(
        vec![p(c, s, -0.2, -0.08 + dy), p(c, s, 0.0, 0.12 + dy), p(c, s, 0.2, -0.08 + dy)],
        st,
    ));
    pt.add(Shape::line(
        vec![p(c, s, -0.38, 0.12), p(c, s, -0.38, 0.36), p(c, s, 0.38, 0.36), p(c, s, 0.38, 0.12)],
        st,
    ));
}

/// Arco que gira (algo está cargando). `t` en segundos.
pub fn spinner(pt: &Painter, c: Pos2, s: f32, col: Color32, t: f64) {
    let r = 0.36 * s;
    let start = (t * 5.0) as f32;
    let len = 1.6 + 0.9 * ((t * 2.2).sin() as f32);
    let n = 24;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = start + len * i as f32 / n as f32;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    pt.add(Shape::line(pts, Stroke::new(W + 0.2, col)));
}

/// Teclado (editar atajos).
pub fn keyboard(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    let body = Rect::from_center_size(c, vec2(0.92 * s, 0.6 * s));
    pt.rect_stroke(body, 2.5, st, StrokeKind::Middle);
    for row in 0..2 {
        for k in 0..4 {
            let x = -0.27 + 0.18 * k as f32;
            let y = -0.12 + 0.13 * row as f32;
            pt.circle_filled(p(c, s, x, y), 0.035 * s + 0.35, col);
        }
    }
    pt.line_segment([p(c, s, -0.2, 0.15), p(c, s, 0.2, 0.15)], st);
}

/// Hoja con renglones y una chispa (novedades de la versión).
pub fn news(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    let page = Rect::from_min_max(p(c, s, -0.32, -0.38), p(c, s, 0.22, 0.4));
    pt.rect_stroke(page, 2.0, st, StrokeKind::Middle);
    for (i, w) in [0.3, 0.3, 0.2].iter().enumerate() {
        let y = -0.14 + 0.16 * i as f32;
        pt.line_segment([p(c, s, -0.2, y), p(c, s, -0.2 + w, y)], st);
    }
    // chispa de 4 puntas arriba a la derecha
    let sc = p(c, s, 0.3, -0.3);
    let r = 0.17 * s;
    let q = 0.05 * s;
    pt.add(Shape::convex_polygon(
        vec![
            pos2(sc.x, sc.y - r),
            pos2(sc.x + q, sc.y - q),
            pos2(sc.x + r, sc.y),
            pos2(sc.x + q, sc.y + q),
            pos2(sc.x, sc.y + r),
            pos2(sc.x - q, sc.y + q),
            pos2(sc.x - r, sc.y),
            pos2(sc.x - q, sc.y - q),
        ],
        col,
        Stroke::NONE,
    ));
}

/// Tacho de basura (borrar playlist / atajo).
pub fn trash(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new(W, col);
    pt.line_segment([p(c, s, -0.36, -0.24), p(c, s, 0.36, -0.24)], st);
    pt.add(Shape::line(
        vec![p(c, s, -0.12, -0.24), p(c, s, -0.1, -0.36), p(c, s, 0.1, -0.36), p(c, s, 0.12, -0.24)],
        st,
    ));
    pt.add(Shape::line(
        vec![p(c, s, -0.27, -0.2), p(c, s, -0.21, 0.38), p(c, s, 0.21, 0.38), p(c, s, 0.27, -0.2)],
        st,
    ));
    pt.line_segment([p(c, s, -0.07, -0.06), p(c, s, -0.07, 0.24)], st);
    pt.line_segment([p(c, s, 0.07, -0.06), p(c, s, 0.07, 0.24)], st);
}

/// Triángulo de play chiquito (botón "Reproducir").
pub fn play_small(pt: &Painter, c: Pos2, s: f32, col: Color32) {
    pt.add(Shape::convex_polygon(
        vec![p(c, s, -0.3, -0.36), p(c, s, 0.38, 0.0), p(c, s, -0.3, 0.36)],
        col,
        Stroke::NONE,
    ));
}

/// Destello de 4 puntas (lados curvos hacia adentro), girado `angle`
/// radianes. Es el adorno del crédito del tutorial.
pub fn sparkle(pt: &Painter, c: Pos2, r: f32, angle: f32, col: Color32) {
    // un halo apenas más grande y transparente suaviza el borde (la malla
    // no tiene antialias)
    sparkle_mesh(pt, c, r + 0.9, angle, col.gamma_multiply(0.35));
    sparkle_mesh(pt, c, r, angle, col);
}

fn sparkle_mesh(pt: &Painter, c: Pos2, r: f32, angle: f32, col: Color32) {
    let n = 96u32;
    let (sa, ca) = angle.sin_cos();
    let mut mesh = eframe::egui::Mesh::default();
    mesh.colored_vertex(c, col);
    for i in 0..n {
        let t = i as f32 / n as f32 * std::f32::consts::TAU;
        let (st, ct) = t.sin_cos();
        // astroide un poco más finito: |x|^0.6 + |y|^0.6 = r^0.6
        let x = r * ct * ct.abs().powf(2.33);
        let y = r * st * st.abs().powf(2.33);
        mesh.colored_vertex(pos2(c.x + x * ca - y * sa, c.y + x * sa + y * ca), col);
    }
    for i in 1..=n {
        mesh.add_triangle(0, i, if i == n { 1 } else { i + 1 });
    }
    pt.add(Shape::mesh(mesh));
}

/// Terminal (Linux en el tutorial): ventanita con el prompt `>_`. El
/// cursor parpadea según `t` (segundos).
pub fn terminal(pt: &Painter, c: Pos2, s: f32, frame: Color32, accent: Color32, t: f64) {
    let r = Rect::from_center_size(c, vec2(s, s * 0.78));
    let round = s * 0.12;
    pt.rect_filled(r, round, Color32::from_rgb(0x14, 0x15, 0x19));
    pt.rect_stroke(r, round, Stroke::new(s * 0.035, frame), StrokeKind::Inside);
    // barra de arriba con tres puntitos
    let bar_y = r.top() + s * 0.12;
    pt.line_segment([pos2(r.left() + s * 0.03, bar_y + s * 0.07), pos2(r.right() - s * 0.03, bar_y + s * 0.07)], Stroke::new(s * 0.02, frame.gamma_multiply(0.6)));
    for (i, k) in [0.6f32, 0.4, 0.25].iter().enumerate() {
        pt.circle_filled(pos2(r.left() + s * (0.11 + 0.08 * i as f32), bar_y), s * 0.026, frame.gamma_multiply(*k + 0.3));
    }
    // ">" y "_"
    let st = Stroke::new(s * 0.065, accent);
    let gx = r.left() + s * 0.2;
    let gy = r.center().y + s * 0.08;
    pt.add(Shape::line(vec![pos2(gx, gy - s * 0.13), pos2(gx + s * 0.13, gy), pos2(gx, gy + s * 0.13)], st));
    if (t * 1.8).fract() < 0.6 {
        pt.line_segment([pos2(gx + s * 0.22, gy + s * 0.13), pos2(gx + s * 0.44, gy + s * 0.13)], Stroke::new(s * 0.065, Color32::WHITE));
    }
}

/// Ventana genérica de escritorio (Windows en el tutorial): barra de título
/// con minimizar, maximizar y cerrar. `rounded` = esquinas redondeadas.
pub fn app_window(pt: &Painter, c: Pos2, s: f32, frame: Color32, accent: Color32, rounded: bool) {
    let r = Rect::from_center_size(c, vec2(s, s * 0.78));
    let round = if rounded { s * 0.12 } else { s * 0.025 };
    pt.rect_filled(r, round, Color32::from_rgb(0x1B, 0x1D, 0x22));
    pt.rect_stroke(r, round, Stroke::new(s * 0.035, frame), StrokeKind::Inside);
    let bar = s * 0.17;
    let by = r.top() + bar / 2.0 + s * 0.01;
    pt.line_segment([pos2(r.left() + s * 0.03, r.top() + bar), pos2(r.right() - s * 0.03, r.top() + bar)], Stroke::new(s * 0.02, frame.gamma_multiply(0.6)));
    let st = Stroke::new(s * 0.022, frame);
    let g = s * 0.035;
    // ×  □  –  (de derecha a izquierda)
    let x3 = r.right() - s * 0.1;
    pt.line_segment([pos2(x3 - g, by - g), pos2(x3 + g, by + g)], st);
    pt.line_segment([pos2(x3 - g, by + g), pos2(x3 + g, by - g)], st);
    let x2 = x3 - s * 0.13;
    pt.rect_stroke(Rect::from_center_size(pos2(x2, by), vec2(2.0 * g, 2.0 * g)), 0.0, st, StrokeKind::Middle);
    let x1 = x2 - s * 0.13;
    pt.line_segment([pos2(x1 - g, by), pos2(x1 + g, by)], st);
    // contenido: un bloque de color y unas líneas de "texto"
    let top = r.top() + bar + s * 0.08;
    let block = Rect::from_min_size(pos2(r.left() + s * 0.1, top), vec2(s * 0.3, s * 0.36));
    pt.rect_filled(block, if rounded { s * 0.05 } else { s * 0.01 }, accent.gamma_multiply(0.85));
    for (i, w) in [0.38f32, 0.3, 0.34, 0.2].iter().enumerate() {
        let y = top + s * (0.03 + 0.1 * i as f32);
        let x = block.right() + s * 0.08;
        pt.line_segment([pos2(x, y), pos2(x + s * w, y)], Stroke::new(s * 0.035, Color32::from_white_alpha(70)));
    }
}

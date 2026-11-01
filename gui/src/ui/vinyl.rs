//! El disco de vinilo (⚙ → Disco de vinilo): con un audio, la portada va
//! como etiqueta en el medio de un disco que gira a 33⅓ vueltas por minuto
//! (por la velocidad de reproducción: a 2×, el doble de rápido). Arranca y
//! frena despacio, como un tocadiscos.
//!
//! Lo usan la vista de música, la vista clásica (con un audio) y Capyro
//! Studio. El brillo del disco no gira (la luz está quieta, como en uno de
//! verdad): lo que se ve girar es la etiqueta y unas marquitas de los surcos.

use std::f32::consts::{FRAC_PI_2, TAU};

use eframe::egui::{self, pos2, vec2, Color32, Mesh, Painter, Pos2, Shape, Stroke, TextureId};

/// Vueltas por segundo a velocidad normal (33⅓ rpm).
pub const RPS: f32 = 33.333 / 60.0;

/// Qué tan grande es la etiqueta (la portada) respecto del disco.
pub const LABEL: f32 = 0.42;

/// Avanza el giro: `vel` (vueltas por segundo) va hacia `target` con
/// inercia y el ángulo suma lo que giró en `dt`. Devuelve true si se mueve.
pub fn step(angle: &mut f32, vel: &mut f32, target: f32, dt: f32) -> bool {
    // ~0,6 s en arrancar o frenar, como un plato de verdad
    let k = (dt * 4.0).min(1.0);
    *vel += (target - *vel) * k;
    if target == 0.0 && vel.abs() < 0.003 {
        *vel = 0.0;
    }
    *angle = (*angle + *vel * TAU * dt).rem_euclid(TAU);
    *vel != 0.0
}

/// La etiqueta: o la portada (textura y su relación de aspecto), o el color
/// de acento con una nota.
pub enum Label {
    Cover(TextureId, f32),
    Plain,
}

/// Dibuja el disco con centro `c` y radio `r`, girado `angle` radianes.
/// `opacity` (0..1) para entrar suave; `sheen` (Medio y Alto) el brillo.
#[allow(clippy::too_many_arguments)]
pub fn draw(painter: &Painter, c: Pos2, r: f32, angle: f32, label: &Label, accent: Color32, opacity: f32, sheen: bool) {
    let o = opacity.clamp(0.0, 1.0);
    let k = |col: Color32| col.gamma_multiply(o);

    // el disco y su borde
    painter.circle_filled(c, r, k(Color32::from_rgb(14, 14, 17)));
    painter.circle_stroke(c, r - 0.5, Stroke::new(1.0, k(Color32::from_white_alpha(28))));

    // surcos: anillos finitos, más marcados entre "temas"
    let rl = r * LABEL;
    let n = ((r - rl) / 3.2).clamp(8.0, 40.0) as usize;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let rr = rl + 6.0 + t * (r - rl - 10.0);
        let gap = i % 9 == 4; // separación entre temas
        let a = if gap { 4 } else if i % 2 == 0 { 13 } else { 8 };
        painter.circle_stroke(c, rr, Stroke::new(if gap { 2.0 } else { 0.8 }, k(Color32::from_white_alpha(a))));
    }

    // brillo quieto: dos cuñas de luz enfrentadas (la luz no gira)
    if sheen {
        sheen_wedge(painter, c, rl + 4.0, r - 2.0, -FRAC_PI_2 - 0.9, 0.55, k(Color32::from_white_alpha(26)));
        sheen_wedge(painter, c, rl + 4.0, r - 2.0, FRAC_PI_2 - 0.9, 0.55, k(Color32::from_white_alpha(18)));
    }

    // marquitas en los surcos que giran con el disco (así se nota que gira)
    for (off, rr, len) in [(0.0f32, 0.62, 0.5f32), (2.4, 0.81, 0.35), (4.1, 0.72, 0.25)] {
        let a0 = angle + off;
        let pts: Vec<Pos2> = (0..=10)
            .map(|i| {
                let t = a0 + len * i as f32 / 10.0;
                c + vec2(t.cos(), t.sin()) * (r * rr)
            })
            .collect();
        painter.add(Shape::line(pts, Stroke::new(1.2, k(Color32::from_white_alpha(22)))));
    }

    // la etiqueta, girando
    match label {
        Label::Cover(tex, aspect) => {
            painter.add(Shape::mesh(label_mesh(*tex, *aspect, c, rl, angle, k(Color32::WHITE))));
        }
        Label::Plain => {
            painter.circle_filled(c, rl, k(accent.gamma_multiply(0.85)));
            painter.circle_filled(c, rl * 0.72, k(accent.lerp_to_gamma(Color32::BLACK, 0.25)));
            // una franja impresa que gira
            let d = vec2(angle.cos(), angle.sin());
            painter.line_segment([c + d * (rl * 0.3), c + d * (rl * 0.66)], Stroke::new(2.0, k(Color32::from_white_alpha(120))));
            crate::ui::icons::music_note(painter, c, rl * 0.55, k(Color32::WHITE));
        }
    }
    painter.circle_stroke(c, rl, Stroke::new(1.5, k(Color32::from_black_alpha(120))));
    // el agujero del medio
    painter.circle_filled(c, (r * 0.035).max(2.5), k(Color32::from_rgb(10, 10, 12)));
    painter.circle_stroke(c, (r * 0.035).max(2.5), Stroke::new(1.0, k(Color32::from_white_alpha(40))));
}

/// Círculo con la textura girada `angle` (recortada al centro si no es cuadrada).
fn label_mesh(tex: TextureId, aspect: f32, c: Pos2, r: f32, angle: f32, tint: Color32) -> Mesh {
    let mut m = Mesh::with_texture(tex);
    // lo que entra de la imagen: el cuadrado del medio
    let (su, sv) = if aspect >= 1.0 { (1.0 / aspect, 1.0) } else { (1.0, aspect) };
    m.vertices.push(egui::epaint::Vertex { pos: c, uv: pos2(0.5, 0.5), color: tint });
    let n = 72;
    for i in 0..=n {
        let t = TAU * i as f32 / n as f32;
        let (s, co) = (t - angle).sin_cos();
        m.vertices.push(egui::epaint::Vertex {
            pos: c + vec2(t.cos(), t.sin()) * r,
            uv: pos2(0.5 + 0.5 * co * su, 0.5 + 0.5 * s * sv),
            color: tint,
        });
    }
    for i in 1..=n as u32 {
        m.add_triangle(0, i, i + 1);
    }
    m
}

/// Un sector de anillo con luz que se apaga hacia los bordes del ángulo.
fn sheen_wedge(painter: &Painter, c: Pos2, r0: f32, r1: f32, start: f32, span: f32, col: Color32) {
    let mut m = Mesh::default();
    let n = 24;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let a = start + span * t;
        let fall = (t * std::f32::consts::PI).sin(); // 0 en las puntas, 1 al medio
        let cc = col.gamma_multiply(fall);
        let d = vec2(a.cos(), a.sin());
        m.colored_vertex(c + d * r0, cc);
        m.colored_vertex(c + d * r1, cc);
    }
    for i in 0..n as u32 {
        let b = i * 2;
        m.add_triangle(b, b + 1, b + 2);
        m.add_triangle(b + 1, b + 3, b + 2);
    }
    painter.add(Shape::mesh(m));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arranca_y_frena_con_inercia() {
        let (mut a, mut v) = (0.0f32, 0.0f32);
        // a velocidad normal, en 2 s ya gira a 33⅓
        for _ in 0..120 {
            step(&mut a, &mut v, RPS, 1.0 / 60.0);
        }
        assert!((v - RPS).abs() < 0.01, "{v}");
        // a 2× gira el doble
        for _ in 0..120 {
            step(&mut a, &mut v, RPS * 2.0, 1.0 / 60.0);
        }
        assert!((v - 2.0 * RPS).abs() < 0.02, "{v}");
        // en pausa frena hasta quedar quieto
        let mut moving = true;
        for _ in 0..240 {
            moving = step(&mut a, &mut v, 0.0, 1.0 / 60.0);
        }
        assert!(!moving && v == 0.0);
        assert!((0.0..TAU).contains(&a));
    }
}

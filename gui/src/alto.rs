//! Los extras del modo de rendimiento **Alto** (además de la iluminación
//! cinematográfica de [`crate::perf::Ambient`]):
//!
//! - **fondo con degradé dinámico**: tres manchas de color (el acento, el
//!   color de lo que se está viendo y uno que los acompaña) que se mueven
//!   despacio por detrás de toda la interfaz;
//! - **partículas ambientales**: polvito de luz que flota hacia arriba y
//!   titila (con música, late con los graves);
//! - **resplandor interactivo**: una luz suave que sigue al mouse, brillos en
//!   los botones al pasar por encima y una onda al hacer clic;
//! - **transiciones más elaboradas**: los menús se abren con un rebote y el
//!   contenido se descubre de arriba abajo con una línea de luz; al cambiar
//!   de archivo pasa un destello por el área de video.
//!
//! Todo se dibuja con mallas de colores por vértice (las pinta la placa de
//! video) y sin estado por partícula: las posiciones salen del tiempo y de un
//! hash. El movimiento solo corre con la ventana enfocada (desenfocada queda
//! quieta y no gasta nada) y a 15 cuadros por segundo cuando nada más se mueve (se mueve despacio:
//! no hace falta más).

use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Color32, Id, Pos2, Rect, Shape, Stroke};

use crate::perf::{self, Mode};

/// Cada cuánto se redibuja el fondo cuando no hay otra cosa moviéndose (15 por segundo).
pub const IDLE_FRAME: Duration = Duration::from_millis(66);

/// ¿Está activo el modo Alto?
pub fn on() -> bool {
    perf::get() == Mode::Alto
}

/// Estado del fondo animado (uno por ventana).
pub struct Backdrop {
    /// Reloj propio: avanza solo con la ventana enfocada.
    clock: f64,
    last: f64,
    /// Luz del mouse: dónde está (suavizado) y cuánto se ve.
    cursor: Pos2,
    cursor_level: f32,
    /// Graves de la música, suavizados (0..1).
    energy: f32,
    /// Segundo color del degradé (el de la imagen o uno derivado del acento).
    tint: [f32; 3],
    started: bool,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self {
            clock: 0.0,
            last: 0.0,
            cursor: pos2(0.0, 0.0),
            cursor_level: 0.0,
            energy: 0.0,
            tint: [0.0; 3],
            started: false,
        }
    }
}

impl Backdrop {
    /// Avanza un cuadro. `image` = color promedio de lo que se ve (si hay).
    /// Devuelve cada cuánto hace falta redibujar (None = está todo quieto).
    pub fn step(
        &mut self,
        now: f64,
        focused: bool,
        pointer: Option<Pos2>,
        energy: f32,
        accent: Color32,
        image: Option<[f32; 3]>,
    ) -> Option<Duration> {
        let dt = if self.started { ((now - self.last) as f32).clamp(0.0, 0.1) } else { 0.0 };
        self.last = now;
        let goal_tint = image.unwrap_or_else(|| shift_hue(accent, -38.0, 0.9));
        if !self.started {
            self.tint = goal_tint;
            self.started = true;
        }
        if focused {
            self.clock += dt as f64;
        }
        // el color sigue a la imagen despacio; la luz del mouse, rápido
        let a_slow = 1.0 - (-dt / 1.2).exp();
        let a_fast = 1.0 - (-dt / 0.07).exp();
        let mut moving = false;
        for k in 0..3 {
            let d = goal_tint[k] - self.tint[k];
            if d.abs() > 0.8 {
                self.tint[k] += d * a_slow;
                moving = true;
            } else {
                self.tint[k] = goal_tint[k];
            }
        }
        let goal_level = if pointer.is_some() && focused { 1.0 } else { 0.0 };
        if let Some(p) = pointer {
            if self.cursor_level <= 0.01 {
                self.cursor = p; // entra donde está, sin cruzar la ventana
            }
            let d = p - self.cursor;
            if d.length() > 0.5 {
                self.cursor += d * a_fast;
                moving = true;
            } else {
                self.cursor = p;
            }
        }
        let dl = goal_level - self.cursor_level;
        if dl.abs() > 0.01 {
            self.cursor_level += dl * (1.0 - (-dt / 0.18).exp());
            moving = true;
        } else {
            self.cursor_level = goal_level;
        }
        let de = energy - self.energy;
        self.energy += de * if de > 0.0 { a_fast } else { 1.0 - (-dt / 0.35).exp() };
        if focused {
            Some(IDLE_FRAME) // el degradé y las partículas se mueven siempre
        } else if moving || self.energy > 0.01 {
            Some(crate::app::FRAME_SMOOTH)
        } else {
            None
        }
    }

    /// El degradé dinámico de este cuadro, sobre `area` (se apaga en los
    /// bordes para respetar las esquinas redondeadas de la ventana).
    pub fn gradient(&self, area: Rect, accent: Color32) -> Gradient {
        let t = self.clock as f32;
        let pulse = 1.0 + 0.35 * self.energy;
        let diag = area.width().hypot(area.height());
        let at = |x: f32, y: f32| pos2(area.left() + x * area.width(), area.top() + y * area.height());
        Gradient {
            area,
            // (centro, radio, color, fuerza): el acento arriba, el color de
            // la imagen abajo y un tercero que los acompaña, dando vueltas
            blobs: [
                (at(0.5 + 0.38 * (t * 0.26).sin(), 0.12 + 0.18 * (t * 0.19).cos()), 0.75 * diag, rgb(accent), 0.20 * pulse),
                (at(0.5 + 0.42 * (t * 0.17 + 2.0).cos(), 0.85 + 0.14 * (t * 0.23).sin()), 0.8 * diag, self.tint, 0.16),
                (
                    at(0.15 + 0.7 * (0.5 + 0.5 * (t * 0.11 + 1.0).sin()), 0.5 + 0.3 * (t * 0.14).cos()),
                    0.6 * diag,
                    shift_hue(accent, 150.0, 0.55),
                    0.10,
                ),
            ],
        }
    }

    /// Polvito de luz que sube despacio y titila. No se dibuja sobre `hide`
    /// (la imagen del video o la carátula).
    pub fn paint_particles(&self, painter: &egui::Painter, area: Rect, accent: Color32, hide: Option<Rect>, n: u32) {
        if area.width() < 10.0 || area.height() < 10.0 {
            return;
        }
        let t = self.clock as f32;
        let tint = Color32::from_rgb(self.tint[0] as u8, self.tint[1] as u8, self.tint[2] as u8);
        let hide = hide.unwrap_or(Rect::NOTHING);
        let boost = 1.0 + 0.8 * self.energy;
        for i in 0..n {
            let speed = 6.0 + 14.0 * hash01(i, 1);
            let h = area.height() + 30.0;
            let travel = (hash01(i, 3) * h + t * speed * (1.0 + 0.6 * self.energy)) % h;
            let y = area.bottom() + 15.0 - travel;
            let x = area.left()
                + hash01(i, 2) * area.width()
                + (t * (0.25 + 0.5 * hash01(i, 4)) + i as f32 * 1.7).sin() * 16.0;
            let p = pos2(x, y);
            if hide.contains(p) || !area.contains(p) {
                continue;
            }
            // nacen abajo, se apagan arriba, y titilan
            let life = travel / h;
            let twinkle = 0.55 + 0.45 * (t * (1.3 + 2.0 * hash01(i, 6)) + i as f32).sin();
            let alpha = (life * 5.0).min(1.0) * (1.0 - life).powf(1.2) * twinkle * edge_fade(p, area, 20.0);
            let r = (0.8 + 1.6 * hash01(i, 5)) * boost.min(1.6);
            let col = match i % 3 {
                0 => accent,
                1 => tint,
                _ => Color32::WHITE,
            };
            // halo chiquito y el punto
            if r > 1.5 {
                painter.circle_filled(p, r * 3.4, col.gamma_multiply(0.10 * alpha * boost));
            }
            painter.circle_filled(p, r, col.gamma_multiply((0.75 * alpha * boost).min(0.95)));
        }
    }

    /// Luz suave que sigue al mouse (detrás de la interfaz). Con `skip`, no
    /// se dibuja si el mouse está bien adentro de ese rectángulo (ahí la
    /// dibuja el área de video, por encima de su fondo).
    pub fn paint_cursor(&self, painter: &egui::Painter, area: Rect, accent: Color32, skip: Option<Rect>) {
        if self.cursor_level <= 0.01 || !area.expand(80.0).contains(self.cursor) {
            return;
        }
        if skip.is_some_and(|r| r.shrink(180.0).contains(self.cursor)) {
            return;
        }
        let p = painter.clone().with_clip_rect(area.intersect(painter.clip_rect()));
        glow(&p, self.cursor, 180.0, accent, 0.13 * self.cursor_level);
        glow(&p, self.cursor, 60.0, Color32::WHITE, 0.025 * self.cursor_level);
    }
}

/// Degradé de fondo (tres manchas de color) congelado en un cuadro.
pub struct Gradient {
    area: Rect,
    blobs: [(Pos2, f32, [f32; 3], f32); 3],
}

impl Gradient {
    /// Color premultiplicado (r, g, b, a en 0..1) en `p`.
    pub fn at(&self, p: Pos2) -> [f32; 4] {
        let mut out = [0.0f32; 4];
        for (center, radius, c, strength) in self.blobs {
            let w = (1.0 - p.distance(center) / radius).clamp(0.0, 1.0);
            let w = w * w * (3.0 - 2.0 * w) * strength;
            for k in 0..3 {
                out[k] += c[k] / 255.0 * w;
            }
            out[3] += w;
        }
        let fade = edge_fade(p, self.area, 16.0);
        let scale = if out[3] * fade > 0.6 { 0.6 / out[3] } else { fade };
        out.map(|v| (v * scale).min(1.0))
    }

    /// Dibuja el degradé solo (cuando no hay luz de video con la que
    /// juntarlo). Lo que queda adentro de `under` (el área de video, que es
    /// opaca) no se manda a dibujar: con la GPU da igual, pero con una por
    /// software (llvmpipe) cada píxel que se mezcla cuesta.
    pub fn paint(&self, painter: &egui::Painter, under: Option<Rect>) {
        let area = self.area;
        let nx = ((area.width() / 40.0).ceil() as usize).clamp(4, 32);
        let ny = ((area.height() / 40.0).ceil() as usize).clamp(4, 26);
        let under = under.unwrap_or(Rect::NOTHING);
        let mut hidden = Vec::with_capacity((nx + 1) * (ny + 1));
        let mut mesh = egui::Mesh::default();
        mesh.reserve_vertices((nx + 1) * (ny + 1));
        mesh.reserve_triangles(nx * ny * 2);
        for j in 0..=ny {
            for i in 0..=nx {
                let p = pos2(
                    area.left() + area.width() * i as f32 / nx as f32,
                    area.top() + area.height() * j as f32 / ny as f32,
                );
                hidden.push(under.contains(p));
                mesh.colored_vertex(p, to_color(self.at(p)));
            }
        }
        let row = (nx + 1) as u32;
        for j in 0..ny as u32 {
            for i in 0..nx as u32 {
                let a = j * row + i;
                for t in [[a, a + 1, a + row], [a + 1, a + row + 1, a + row]] {
                    if !t.iter().all(|&v| hidden[v as usize]) {
                        mesh.add_triangle(t[0], t[1], t[2]);
                    }
                }
            }
        }
        painter.add(Shape::mesh(mesh));
    }
}

/// De premultiplicado en 0..1 a `Color32`.
pub fn to_color([r, g, b, a]: [f32; 4]) -> Color32 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_premultiplied(q(r), q(g), q(b), q(a))
}

/// Brillo redondo: `intensity` en el centro, nada en el borde (`radius`).
pub fn glow(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32, intensity: f32) {
    if intensity <= 0.002 || radius <= 0.5 {
        return;
    }
    let [r, g, b] = rgb(color);
    let a = intensity.clamp(0.0, 1.0);
    let mut mesh = egui::Mesh::default();
    let n = 40u32;
    mesh.colored_vertex(center, premul(r, g, b, a));
    // un anillo intermedio para que la caída sea suave (no un cono)
    for ring in [0.45f32, 1.0] {
        let ar = if ring < 1.0 { a * 0.42 } else { 0.0 };
        for i in 0..n {
            let ang = i as f32 / n as f32 * std::f32::consts::TAU;
            mesh.colored_vertex(center + vec2(ang.cos(), ang.sin()) * radius * ring, premul(r, g, b, ar));
        }
    }
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.add_triangle(0, 1 + i, 1 + j);
        let (a0, a1, b0, b1) = (1 + i, 1 + j, 1 + n + i, 1 + n + j);
        mesh.add_triangle(a0, b0, b1);
        mesh.add_triangle(a0, b1, a1);
    }
    painter.add(Shape::mesh(mesh));
}

/// Halo alrededor de una pastilla (botón "Agregar", chips).
pub fn glow_rect(painter: &egui::Painter, rect: Rect, color: Color32, intensity: f32) {
    if intensity <= 0.002 {
        return;
    }
    for (i, grow) in [3.0f32, 7.0, 12.0].iter().enumerate() {
        let r = rect.expand(*grow);
        let k = [0.22, 0.12, 0.05][i] * intensity;
        painter.rect_filled(r, r.height() / 2.0, color.gamma_multiply(k));
    }
}

/* ---------------- ondas al hacer clic ---------------- */

#[derive(Clone, Default)]
struct Ripples(Vec<(Pos2, f64, f32)>);

fn ripples_id() -> Id {
    Id::unique("capyro-alto-ripples")
}

/// Una onda que sale de `center` (se dibuja con [`paint_ripples`]).
pub fn ripple(ctx: &egui::Context, center: Pos2, radius: f32) {
    if !on() {
        return;
    }
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Ripples>(ripples_id()).0.push((center, now, radius)));
    ctx.request_repaint();
}

/// Dibuja las ondas vivas (arriba de los botones).
pub fn paint_ripples(ctx: &egui::Context, painter: &egui::Painter, accent: Color32) {
    let now = ctx.input(|i| i.time);
    let mut rs: Ripples = ctx.data(|d| d.get_temp(ripples_id())).unwrap_or_default();
    if rs.0.is_empty() {
        return;
    }
    const LIFE: f64 = 0.55;
    rs.0.retain(|r| now - r.1 < LIFE);
    for &(c, t0, radius) in &rs.0 {
        let k = ((now - t0) / LIFE) as f32;
        let e = 1.0 - (1.0 - k).powi(3);
        painter.circle_stroke(c, radius * (0.6 + 1.1 * e), Stroke::new(2.0 * (1.0 - k) + 0.5, accent.gamma_multiply(0.7 * (1.0 - k))));
    }
    let alive = !rs.0.is_empty();
    ctx.data_mut(|d| d.insert_temp(ripples_id(), rs));
    if alive {
        ctx.request_repaint_after(crate::app::FRAME_SMOOTH);
    }
}

/* ---------------- transiciones ---------------- */

/// Escala de apertura de un menú: en Alto, con un rebotecito (sale un poco
/// de más y vuelve); si no, la curva de siempre (`ease`).
pub fn pop(t: f32, ease: f32) -> f32 {
    if !on() {
        return ease;
    }
    let t = t.clamp(0.0, 1.0);
    let c = 2.2;
    1.0 + (c + 1.0) * (t - 1.0).powi(3) + c * (t - 1.0).powi(2)
}

/// Contenido de un menú que se descubre de arriba abajo, con una línea de
/// luz en el borde. Se llama cada cuadro en que el menú dibuja su
/// contenido; se dibuja en una capa de arriba, así tapa el contenido sin
/// tocar su código.
pub fn reveal(ctx: &egui::Context, id: Id, panel: Rect, radius: f32, open: bool, bg: Color32, accent: Color32) {
    if !on() {
        return;
    }
    // se llama solo mientras se ve el contenido: si el cuadro anterior no
    // se llamó, el menú se acaba de abrir y arranca la animación
    let (now, frame) = (ctx.input(|i| i.time), ctx.cumulative_frame_nr());
    let key = id.with("alto-reveal");
    let (last, t_open) = ctx.data(|d| d.get_temp::<(u64, f64)>(key)).unwrap_or((0, now));
    let t_open = if last + 1 < frame { now } else { t_open };
    ctx.data_mut(|d| d.insert_temp(key, (frame, t_open)));
    let k = ((now - t_open) / 0.45) as f32;
    if !open || k >= 1.0 {
        return;
    }
    let k = 1.0 - (1.0 - k.clamp(0.0, 1.0)).powi(2);
    let inner = panel.shrink(1.0);
    let y = inner.top() + 30.0 + (inner.height() - 30.0) * k;
    let layer = egui::LayerId::new(egui::Order::Foreground, id.with("alto-reveal-layer"));
    let p = ctx.layer_painter(layer).with_clip_rect(inner);
    let soft = 26.0;
    // borde difuminado (malla: de transparente a fondo)
    let mut mesh = egui::Mesh::default();
    let clear = Color32::TRANSPARENT;
    for (yy, col) in [(y - soft, clear), (y, bg)] {
        mesh.colored_vertex(pos2(inner.left(), yy), col);
        mesh.colored_vertex(pos2(inner.right(), yy), col);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    p.add(Shape::mesh(mesh));
    let below = Rect::from_min_max(pos2(inner.left(), y), inner.max);
    let cr = egui::CornerRadius { nw: 0, ne: 0, sw: radius as u8, se: radius as u8 };
    p.rect_filled(below, cr, bg);
    // la línea de luz
    let line = Rect::from_min_max(pos2(inner.left() + 8.0, y - 1.0), pos2(inner.right() - 8.0, y + 1.0));
    p.rect_filled(line.expand2(vec2(0.0, 3.0)), 3.0, accent.gamma_multiply(0.12));
    p.rect_filled(line, 1.0, accent.gamma_multiply(0.75));
    ctx.request_repaint_after(crate::app::FRAME_SMOOTH);
}

/// Destello diagonal que cruza `rect` (al cambiar de archivo). `k` = 0..1.
pub fn sweep(painter: &egui::Painter, rect: Rect, k: f32, accent: Color32) {
    if !(0.0..1.0).contains(&k) {
        return;
    }
    let p = painter.clone().with_clip_rect(rect.intersect(painter.clip_rect()));
    let e = 1.0 - (1.0 - k).powi(2);
    let w = rect.width() * 0.35;
    let skew = rect.height() * 0.35;
    let x = rect.left() - w - skew + (rect.width() + 2.0 * w + 2.0 * skew) * e;
    let fade = (1.0 - k).powf(0.6);
    let [r, g, b] = rgb(accent.lerp_to_gamma(Color32::WHITE, 0.4));
    let mid = premul(r, g, b, 0.16 * fade);
    let mut mesh = egui::Mesh::default();
    // tres columnas: transparente | brillo | transparente, inclinadas
    for (dx, col) in [(-w, Color32::TRANSPARENT), (0.0, mid), (w, Color32::TRANSPARENT)] {
        mesh.colored_vertex(pos2(x + dx + skew, rect.top()), col);
        mesh.colored_vertex(pos2(x + dx - skew, rect.bottom()), col);
    }
    for i in 0..2u32 {
        let a = i * 2;
        mesh.add_triangle(a, a + 2, a + 1);
        mesh.add_triangle(a + 2, a + 3, a + 1);
    }
    p.add(Shape::mesh(mesh));
}

/* ---------------- ayudantes ---------------- */


/// 0 en el borde de `area`, 1 a `edge` puntos hacia adentro (suave).
fn edge_fade(p: Pos2, area: Rect, edge: f32) -> f32 {
    let d = (p.x - area.left()).min(area.right() - p.x).min(p.y - area.top()).min(area.bottom() - p.y);
    let k = (d / edge).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

fn rgb(c: Color32) -> [f32; 3] {
    [c.r() as f32, c.g() as f32, c.b() as f32]
}

fn premul(r: f32, g: f32, b: f32, alpha: f32) -> Color32 {
    let a = alpha.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied((r * a) as u8, (g * a) as u8, (b * a) as u8, (a * 255.0) as u8)
}

/// Gira el tono de un color (grados) y le cambia la saturación (factor).
fn shift_hue(c: Color32, degrees: f32, sat: f32) -> [f32; 3] {
    let [r, g, b] = rgb(c).map(|v| v / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let mut h = if d < 1e-6 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max <= 0.0 { 0.0 } else { (d / max * sat).clamp(0.0, 1.0) };
    let v = max;
    h = (h + degrees).rem_euclid(360.0);
    let c2 = v * s;
    let x = c2 * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c2;
    let (r1, g1, b1) = match (h / 60.0) as u32 {
        0 => (c2, x, 0.0),
        1 => (x, c2, 0.0),
        2 => (0.0, c2, x),
        3 => (0.0, x, c2),
        4 => (x, 0.0, c2),
        _ => (c2, 0.0, x),
    };
    [(r1 + m) * 255.0, (g1 + m) * 255.0, (b1 + m) * 255.0]
}

/// Generador chiquito y determinista (las partículas no guardan estado).
fn hash01(i: u32, salt: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x % 10_000) as f32 / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tonos() {
        // girar 0° no cambia nada; 120° lleva el rojo al verde
        let red = Color32::from_rgb(255, 0, 0);
        let same = shift_hue(red, 0.0, 1.0);
        assert!((same[0] - 255.0).abs() < 1.0 && same[1] < 1.0 && same[2] < 1.0);
        let green = shift_hue(red, 120.0, 1.0);
        assert!(green[1] > 250.0 && green[0] < 1.0, "{green:?}");
        let gray = shift_hue(Color32::from_gray(128), 90.0, 1.0);
        assert!((gray[0] - gray[2]).abs() < 1.0);
    }

    #[test]
    fn el_fondo_se_queda_quieto_sin_foco() {
        let mut b = Backdrop::default();
        let accent = Color32::from_rgb(0xFF, 0x74, 0x17);
        assert_eq!(b.step(0.0, true, None, 0.0, accent, None), Some(IDLE_FRAME));
        b.step(1.0, true, None, 0.0, accent, None);
        let c = b.clock;
        assert!(c > 0.0);
        // sin foco: el reloj no avanza y, una vez quieto, no pide cuadros
        let mut t = 1.0;
        let mut last = Some(IDLE_FRAME);
        for _ in 0..200 {
            t += 0.05;
            last = b.step(t, false, None, 0.0, accent, None);
        }
        assert_eq!(b.clock, c);
        assert_eq!(last, None);
    }

    #[test]
    fn rebote() {
        assert!((pop(1.0, 1.0) - 1.0).abs() < 1e-6 || !on());
        assert!(edge_fade(pos2(0.0, 0.0), Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 10.0)), 4.0) == 0.0);
    }
}

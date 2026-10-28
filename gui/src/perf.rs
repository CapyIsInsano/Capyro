//! Modos de rendimiento: Bajo, Medio (el de siempre) y Alto.
//!
//! - **Bajo**: las mismas funciones, sin nada de más. Sin animaciones (todo
//!   aparece y se va al toque), sin pantalla de carga, sin visualizador, sin
//!   chispitas ni brillos, y la interfaz se redibuja menos seguido.
//! - **Medio**: Capyro como siempre.
//! - **Alto**: suma la iluminación cinematográfica ([`Ambient`]): los colores
//!   del video (o de la carátula) se derraman por toda la ventana.
//!
//! El modo vive en un atómico global para que cualquier parte de la interfaz
//! lo pueda consultar sin pasar la configuración de mano en mano.

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use eframe::egui::{self, pos2, Color32, Pos2, Rect};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    Bajo,
    #[default]
    Medio,
    Alto,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Bajo, Mode::Medio, Mode::Alto];

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "bajo" | "low" | "baja" => Some(Mode::Bajo),
            "medio" | "medium" | "normal" | "media" | "" => Some(Mode::Medio),
            "alto" | "high" | "alta" => Some(Mode::Alto),
            _ => None,
        }
    }

    /// Como se escribe en `Capyro.lua`.
    pub fn name(self) -> &'static str {
        match self {
            Mode::Bajo => "bajo",
            Mode::Medio => "medio",
            Mode::Alto => "alto",
        }
    }

    /// Como se ve en Ajustes.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Bajo => "Bajo",
            Mode::Medio => "Medio",
            Mode::Alto => "Alto",
        }
    }

    /// La línea que explica el modo, debajo de los botones.
    pub fn blurb(self) -> &'static str {
        match self {
            Mode::Bajo => "Sin animaciones ni extras. Para PCs justas",
            Mode::Medio => "Capyro como siempre",
            Mode::Alto => "Con iluminación cinematográfica",
        }
    }
}

static MODE: AtomicU8 = AtomicU8::new(1);

pub fn set(mode: Mode) {
    MODE.store(mode as u8, Ordering::Relaxed);
}

pub fn get() -> Mode {
    match MODE.load(Ordering::Relaxed) {
        0 => Mode::Bajo,
        2 => Mode::Alto,
        _ => Mode::Medio,
    }
}

/// ¿Hay animaciones? (todo menos Bajo)
pub fn animations() -> bool {
    get() != Mode::Bajo
}

/// Duración de una animación: en Bajo, cero (llega al final de una).
pub fn t(secs: f32) -> f32 {
    if animations() { secs } else { 0.0 }
}

/// Progreso (0..1) de una animación medida con el reloj: en Bajo, ya terminó.
pub fn k(progress: f32) -> f32 {
    if animations() { progress } else { 1.0 }
}

/// Reloj para lo que solo decora (pulsos, brillos que laten, cosas que se
/// balancean): en Bajo queda quieto en un instante que se ve bien.
pub fn clock(now: f64) -> f64 {
    if animations() { now } else { REST }
}

/// Instante "de reposo" de [`clock`]: los pulsos de 3 Hz quedan arriba.
const REST: f64 = std::f64::consts::PI / 6.0;

/// Cada cuánto se redibuja lo que se mueve solo (barra de progreso, ms,
/// visualizador): 30 por segundo; en Bajo, 20.
pub fn ambient() -> Duration {
    if animations() { crate::app::FRAME_AMBIENT } else { Duration::from_millis(50) }
}

/* ------------------------------------------------------------------------ */
/*  Iluminación cinematográfica                                             */
/* ------------------------------------------------------------------------ */

/// Columnas y filas de colores que se sacan de cada cuadro.
const GW: usize = 16;
const GH: usize = 9;
/// Segundos que tarda la luz en seguir al video (como la de YouTube: lenta).
const TAU: f32 = 0.55;

/// Los colores del video, promediados en una grilla chica y suavizados en
/// el tiempo. Sacarlos cuesta unos miles de lecturas por cuadro (se
/// muestrean pocos píxeles de cada celda) y dibujarlos, una malla con
/// colores por vértice: nada al lado de decodificar el video.
pub struct Ambient {
    target: [[f32; 3]; GW * GH],
    current: [[f32; 3]; GW * GH],
    /// Qué tanto se ve (sube cuando hay imagen, baja cuando no).
    level: f32,
    has_target: bool,
    /// Donde se dibujó la imagen el cuadro anterior (video o carátula).
    pub source: Option<Rect>,
    last_time: f64,
}

impl Default for Ambient {
    fn default() -> Self {
        Self {
            target: [[0.0; 3]; GW * GH],
            current: [[0.0; 3]; GW * GH],
            level: 0.0,
            has_target: false,
            source: None,
            last_time: 0.0,
        }
    }
}

impl Ambient {
    /// Saca los colores de un cuadro RGBA (`w`×`h`).
    pub fn sample(&mut self, rgba: &[u8], w: usize, h: usize) {
        if w == 0 || h == 0 || rgba.len() < w * h * 4 {
            return;
        }
        // 6×6 píxeles por celda alcanzan: es un promedio para una luz difusa
        const S: usize = 6;
        let mut grid = [[0.0f32; 3]; GW * GH];
        for gy in 0..GH {
            for gx in 0..GW {
                let (x0, x1) = (gx * w / GW, ((gx + 1) * w / GW).max(gx * w / GW + 1));
                let (y0, y1) = (gy * h / GH, ((gy + 1) * h / GH).max(gy * h / GH + 1));
                let mut acc = [0u32; 3];
                let mut n = 0u32;
                for sy in 0..S {
                    let y = (y0 + (y1 - y0) * (2 * sy + 1) / (2 * S)).min(h - 1);
                    for sx in 0..S {
                        let x = (x0 + (x1 - x0) * (2 * sx + 1) / (2 * S)).min(w - 1);
                        let i = (y * w + x) * 4;
                        acc[0] += rgba[i] as u32;
                        acc[1] += rgba[i + 1] as u32;
                        acc[2] += rgba[i + 2] as u32;
                        n += 1;
                    }
                }
                grid[gy * GW + gx] = acc.map(|c| c as f32 / n as f32);
            }
        }
        // un desenfoque chiquito entre celdas vecinas: la luz no tiene bordes
        for gy in 0..GH {
            for gx in 0..GW {
                let mut acc = [0.0f32; 3];
                let mut wsum = 0.0;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let (x, y) = (gx as i32 + dx, gy as i32 + dy);
                        if x < 0 || y < 0 || x >= GW as i32 || y >= GH as i32 {
                            continue;
                        }
                        let wgt = if dx == 0 && dy == 0 { 2.0 } else { 1.0 };
                        let c = grid[y as usize * GW + x as usize];
                        for k in 0..3 {
                            acc[k] += c[k] * wgt;
                        }
                        wsum += wgt;
                    }
                }
                self.target[gy * GW + gx] = vivid(acc.map(|c| c / wsum));
            }
        }
        if !self.has_target && self.level <= 0.001 {
            // primera imagen: arranca desde sus colores (sube solo el brillo)
            self.current = self.target;
        }
        self.has_target = true;
    }

    /// No hay imagen (nada cargado o un audio sin carátula): la luz se apaga.
    pub fn clear(&mut self) {
        self.has_target = false;
    }

    /// Acerca los colores a los del último cuadro. Devuelve `true` mientras
    /// todavía se están moviendo (hay que seguir dibujando).
    pub fn step(&mut self, now: f64) -> bool {
        let dt = ((now - self.last_time) as f32).clamp(0.0, 0.1);
        self.last_time = now;
        let a = 1.0 - (-dt / TAU).exp();
        let goal = if self.has_target { 1.0 } else { 0.0 };
        self.level += (goal - self.level) * a;
        let fading = (goal - self.level).abs() > 0.004;
        if !fading {
            self.level = goal;
        }
        let mut moving = false;
        for (c, t) in self.current.iter_mut().zip(self.target.iter()) {
            for k in 0..3 {
                let d = t[k] - c[k];
                if d.abs() > 0.6 {
                    moving = true;
                    c[k] += d * a;
                } else {
                    c[k] = t[k];
                }
            }
        }
        fading || moving && self.level > 0.0
    }

    pub fn visible(&self) -> bool {
        self.level > 0.003 && self.source.is_some()
    }

    /// Color (sin alfa) en el punto `uv` (0..1) de la imagen, interpolado.
    fn color_at(&self, u: f32, v: f32) -> [f32; 3] {
        let fx = (u * GW as f32 - 0.5).clamp(0.0, (GW - 1) as f32);
        let fy = (v * GH as f32 - 0.5).clamp(0.0, (GH - 1) as f32);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(GW - 1), (y0 + 1).min(GH - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let c = |x: usize, y: usize| self.current[y * GW + x];
        let mut out = [0.0; 3];
        for k in 0..3 {
            let top = c(x0, y0)[k] * (1.0 - tx) + c(x1, y0)[k] * tx;
            let bot = c(x0, y1)[k] * (1.0 - tx) + c(x1, y1)[k] * tx;
            out[k] = top * (1.0 - ty) + bot * ty;
        }
        out
    }

    /// Dibuja la luz sobre `area`: cada punto toma el color del borde de la
    /// imagen más cercano y se apaga con la distancia (`reach`, en puntos) y
    /// cerca de los bordes de `area` (así respeta las esquinas redondeadas).
    /// No se dibuja lo que va a quedar tapado por `under` (algo opaco que se
    /// dibuja encima) ni lo que no tiene luz: menos píxeles que mezclar,
    /// que con una GPU por software (llvmpipe) es lo que más cuesta.
    pub fn paint(&self, painter: &egui::Painter, area: Rect, strength: f32, reach: f32, under: Option<Rect>) {
        let Some(src) = self.source else { return };
        if !self.visible() || area.width() < 4.0 || area.height() < 4.0 {
            return;
        }
        // un vértice cada ~32 puntos (la GPU interpola entre ellos)
        let nx = ((area.width() / 32.0).ceil() as usize).clamp(4, 40);
        let ny = ((area.height() / 32.0).ceil() as usize).clamp(4, 30);
        let edge = 44.0f32.min(area.width().min(area.height()) * 0.25);
        let mut mesh = egui::Mesh::default();
        mesh.reserve_vertices((nx + 1) * (ny + 1));
        mesh.reserve_triangles(nx * ny * 2);
        // por vértice: (tiene luz, queda tapado)
        let mut flags = Vec::with_capacity((nx + 1) * (ny + 1));
        let under = under.unwrap_or(Rect::NOTHING);
        for j in 0..=ny {
            for i in 0..=nx {
                let p = pos2(
                    area.left() + area.width() * i as f32 / nx as f32,
                    area.top() + area.height() * j as f32 / ny as f32,
                );
                let q = src.clamp(p);
                let u = ((q.x - src.left()) / src.width().max(1.0)).clamp(0.0, 1.0);
                let v = ((q.y - src.top()) / src.height().max(1.0)).clamp(0.0, 1.0);
                let [r, g, b] = self.color_at(u, v);
                let d = p.distance(q);
                let near = (1.0 - d / reach).clamp(0.0, 1.0);
                let fade = edge_fade(p, area, edge);
                let alpha = strength * self.level * near * near.sqrt() * fade;
                let col = premul(r, g, b, alpha);
                flags.push((col.a() > 0, under.contains(p)));
                mesh.colored_vertex(p, col);
            }
        }
        let row = (nx + 1) as u32;
        // un triángulo se dibuja si algo tiene luz y no está todo tapado
        let any = |t: [u32; 3]| {
            let f = t.map(|v| flags[v as usize]);
            f.iter().any(|x| x.0) && !f.iter().all(|x| x.1)
        };
        for j in 0..ny as u32 {
            for i in 0..nx as u32 {
                let a = j * row + i;
                for t in [[a, a + 1, a + row], [a + 1, a + row + 1, a + row]] {
                    if any(t) {
                        mesh.add_triangle(t[0], t[1], t[2]);
                    }
                }
            }
        }
        if !mesh.indices.is_empty() {
            painter.add(egui::Shape::mesh(mesh));
        }
    }
}

/// 0 en el borde de `area`, 1 a `edge` puntos hacia adentro (suave).
fn edge_fade(p: Pos2, area: Rect, edge: f32) -> f32 {
    let d = (p.x - area.left())
        .min(area.right() - p.x)
        .min(p.y - area.top())
        .min(area.bottom() - p.y);
    let k = (d / edge).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// Un poco más saturado y luminoso: la luz de una pantalla, no un promedio gris.
fn vivid([r, g, b]: [f32; 3]) -> [f32; 3] {
    let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let sat = 1.35;
    let gain = 1.15;
    [r, g, b].map(|c| ((l + (c - l) * sat) * gain).clamp(0.0, 255.0))
}

fn premul(r: f32, g: f32, b: f32, alpha: f32) -> Color32 {
    let a = alpha.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied((r * a) as u8, (g * a) as u8, (b * a) as u8, (a * 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres() {
        for m in Mode::ALL {
            assert_eq!(Mode::parse(m.name()), Some(m));
        }
        assert_eq!(Mode::parse("High"), Some(Mode::Alto));
        assert_eq!(Mode::parse(" low "), Some(Mode::Bajo));
        assert_eq!(Mode::parse("normal"), Some(Mode::Medio));
        assert_eq!(Mode::parse("turbo"), None);
        assert_eq!(Mode::default(), Mode::Medio);
    }

    #[test]
    fn colores_del_cuadro() {
        // mitad izquierda roja, mitad derecha azul
        let (w, h) = (64, 36);
        let mut rgba = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                if x < w / 2 {
                    rgba[i] = 200;
                } else {
                    rgba[i + 2] = 200;
                }
                rgba[i + 3] = 255;
            }
        }
        let mut a = Ambient::default();
        a.sample(&rgba, w, h);
        a.source = Some(Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(160.0, 90.0)));
        // se suaviza en el tiempo, pero termina en los colores del cuadro
        let mut t = 0.0;
        while a.step(t) {
            t += 0.016;
            assert!(t < 10.0, "no converge");
        }
        assert!(a.visible());
        let left = a.color_at(0.0, 0.5);
        let right = a.color_at(1.0, 0.5);
        assert!(left[0] > 150.0 && left[2] < 30.0, "{left:?}");
        assert!(right[2] > 150.0 && right[0] < 30.0, "{right:?}");
        // sin imagen se apaga
        a.clear();
        while a.step(t) {
            t += 0.016;
        }
        assert!(!a.visible());
    }

    #[test]
    fn bordes() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(100.0, 100.0));
        assert_eq!(edge_fade(pos2(0.0, 50.0), r, 20.0), 0.0);
        assert_eq!(edge_fade(pos2(50.0, 50.0), r, 20.0), 1.0);
        assert_eq!(premul(255.0, 0.0, 0.0, 0.0), Color32::TRANSPARENT);
    }
}

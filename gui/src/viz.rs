//! Visualizador de audio: barras de frecuencia a partir de las últimas
//! muestras que sonaron (las da el núcleo). FFT propia, sin dependencias.

use eframe::egui::{pos2, vec2, Color32, Painter, Rect};

const N: usize = 1024; // muestras por análisis (~21 ms a 48 kHz)
pub const BANDS: usize = 40;

pub struct Visualizer {
    window: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
    pub samples: Vec<f32>,
    levels: [f32; BANDS],
    peaks: [f32; BANDS],
}

impl Visualizer {
    pub fn new() -> Self {
        // ventana de Hann: suaviza los bordes para que no aparezcan frecuencias falsas
        let window = (0..N)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (N - 1) as f32).cos())
            .collect();
        Self {
            window,
            re: vec![0.0; N],
            im: vec![0.0; N],
            samples: vec![0.0; N],
            levels: [0.0; BANDS],
            peaks: [0.0; BANDS],
        }
    }

    pub fn len(&self) -> usize {
        N
    }

    /// Calcula las bandas nuevas. `playing = false` hace que las barras caigan.
    pub fn update(&mut self, rate: f32, dt: f32, playing: bool) {
        let mut target = [0.0f32; BANDS];
        if playing && rate > 0.0 {
            for i in 0..N {
                self.re[i] = self.samples[i] * self.window[i];
                self.im[i] = 0.0;
            }
            fft(&mut self.re, &mut self.im);

            // bandas con escala logarítmica de 40 Hz a 16 kHz (como el oído)
            let (f_lo, f_hi) = (40.0f32, (rate / 2.0).min(16_000.0));
            let bin_hz = rate / N as f32;
            for (b, t) in target.iter_mut().enumerate() {
                let a = f_lo * (f_hi / f_lo).powf(b as f32 / BANDS as f32);
                let z = f_lo * (f_hi / f_lo).powf((b + 1) as f32 / BANDS as f32);
                let lo = ((a / bin_hz) as usize).max(1);
                let hi = ((z / bin_hz) as usize).clamp(lo + 1, N / 2);
                let mut peak = 0.0f32;
                for k in lo..hi {
                    peak = peak.max((self.re[k] * self.re[k] + self.im[k] * self.im[k]).sqrt());
                }
                // a decibeles y de -60..-6 dB a 0..1
                let db = 20.0 * (peak / (N as f32 / 4.0) + 1e-9).log10();
                *t = ((db + 60.0) / 54.0).clamp(0.0, 1.0);
            }
        }
        for b in 0..BANDS {
            // sube rápido, baja despacio; el "pico" baja todavía más lento
            let l = &mut self.levels[b];
            *l = if target[b] > *l { *l + (target[b] - *l) * 0.6 } else { (*l - dt * 1.6).max(target[b]) };
            let p = &mut self.peaks[b];
            *p = if *l > *p { *l } else { (*p - dt * 0.5).max(*l) };
        }
    }

    pub fn is_silent(&self) -> bool {
        self.peaks.iter().all(|&p| p < 0.002)
    }

    /// Barras centradas en `rect`, espejadas hacia arriba y abajo.
    pub fn draw(&self, painter: &Painter, rect: Rect, color: Color32) {
        let gap = 3.0;
        let w = ((rect.width() - gap * (BANDS as f32 - 1.0)) / BANDS as f32).max(1.0);
        let mid = rect.center().y;
        let half = rect.height() / 2.0;
        for b in 0..BANDS {
            let x = rect.left() + b as f32 * (w + gap);
            let h = (self.levels[b] * half).max(1.5);
            let alpha = 0.35 + 0.65 * self.levels[b];
            painter.rect_filled(
                Rect::from_min_max(pos2(x, mid - h), pos2(x + w, mid + h * 0.45)),
                w / 2.0,
                color.gamma_multiply(alpha),
            );
            let py = mid - (self.peaks[b] * half).max(1.5) - 3.0;
            painter.rect_filled(Rect::from_min_size(pos2(x, py), vec2(w, 2.0)), 1.0, color.gamma_multiply(0.8));
        }
    }
}

/// FFT radix-2 en el lugar (N potencia de 2).
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tono_de_1khz_cae_en_su_banda() {
        let rate = 48_000.0;
        let mut v = Visualizer::new();
        for (i, s) in v.samples.iter_mut().enumerate() {
            *s = (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / rate).sin() * 0.5;
        }
        for _ in 0..10 {
            v.update(rate, 0.016, true);
        }
        let loudest = (0..BANDS).max_by(|&a, &b| v.levels[a].total_cmp(&v.levels[b])).unwrap();
        // banda que contiene 1 kHz con escala log 40 Hz..16 kHz
        let expected = ((1000.0f32 / 40.0).ln() / (16_000.0f32 / 40.0).ln() * BANDS as f32) as usize;
        assert!(loudest.abs_diff(expected) <= 1, "loudest={loudest} expected={expected}");
        assert!(v.levels[loudest] > 0.7);
        assert!(v.levels[2] < 0.3); // graves casi en cero
    }

    #[test]
    fn silencio_baja_a_cero() {
        let mut v = Visualizer::new();
        for _ in 0..200 {
            v.update(48_000.0, 0.016, false);
        }
        assert!(v.is_silent());
    }
}

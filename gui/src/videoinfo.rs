//! Lo que dicen las burbujas de abajo a la izquierda del video: la
//! resolución "de nombre" (1080p, 4K...) y los cuadros por segundo.
//!
//! La etiqueta es solo el indicador: el video se ve en su resolución real.
//! Un video de 2786×1567 dice "1440p"; uno de 3840×1600 (cine, más ancho)
//! dice "4K", porque se mide como si fuera 16:9.

/// Escalones conocidos: alto equivalente en 16:9, nombre y apodo.
const TIERS: [(u32, &str, &str); 11] = [
    (144, "144p", ""),
    (240, "240p", ""),
    (360, "360p", ""),
    (480, "480p", "SD"),
    (720, "720p", "HD"),
    (1080, "1080p", "Full HD"),
    (1440, "1440p", "2K"),
    (2160, "4K", "2160p"),
    (2880, "5K", "2880p"),
    (4320, "8K", "4320p"),
    (8640, "16K", "8640p"),
];

/// ("1440p", "2K") para 2786×1567. Siempre da algo: lo más chico es 144p y
/// lo más grande, 16K.
pub fn resolution_label(width: u32, height: u32) -> (&'static str, &'static str) {
    let (long, short) = (width.max(height), width.min(height));
    // alto equivalente en 16:9 (así los videos verticales y los muy anchos
    // caen en el escalón que corresponde)
    let eq = short.max(long * 9 / 16) as f64;
    let mut pick = TIERS[0];
    for t in TIERS {
        // un poco de margen: 1916×1076 sigue siendo 1080p
        if t.0 as f64 <= eq * 1.04 {
            pick = t;
        }
    }
    (pick.1, pick.2)
}

/// "30", "29.97", "23.98", "59.94"...
pub fn fps_text(fps: f64) -> String {
    if fps <= 0.0 {
        return "?".to_owned();
    }
    if (fps - fps.round()).abs() < 0.01 {
        format!("{:.0}", fps.round())
    } else {
        format!("{fps:.2}")
    }
}

/// Cuenta los cuadros que de verdad se muestran (para ver si la PC da abasto).
#[derive(Default)]
pub struct FpsMeter {
    times: std::collections::VecDeque<f64>,
}

impl FpsMeter {
    pub fn frame(&mut self, now: f64) {
        self.times.push_back(now);
        while self.times.front().is_some_and(|t| now - t > 1.0) {
            self.times.pop_front();
        }
    }

    pub fn reset(&mut self) {
        self.times.clear();
    }

    /// Cuadros en el último segundo (None hasta tener un segundo entero).
    pub fn rate(&self, now: f64) -> Option<f64> {
        let first = *self.times.front()?;
        if now - first < 0.9 || now - self.times.back()? > 0.5 {
            return None;
        }
        Some(self.times.len() as f64 / (now - first).max(0.9))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etiquetas() {
        assert_eq!(resolution_label(2786, 1567), ("1440p", "2K"));
        assert_eq!(resolution_label(1920, 1080), ("1080p", "Full HD"));
        assert_eq!(resolution_label(1916, 1076), ("1080p", "Full HD"));
        assert_eq!(resolution_label(1080, 1920), ("1080p", "Full HD")); // vertical
        assert_eq!(resolution_label(3840, 1600), ("4K", "2160p")); // cine
        assert_eq!(resolution_label(3840, 2160).0, "4K");
        assert_eq!(resolution_label(1280, 720).0, "720p");
        assert_eq!(resolution_label(640, 360).0, "360p");
        assert_eq!(resolution_label(320, 240).0, "240p");
        assert_eq!(resolution_label(100, 60).0, "144p");
        assert_eq!(resolution_label(5120, 2880).0, "5K");
        assert_eq!(resolution_label(7680, 4320).0, "8K");
        assert_eq!(resolution_label(15360, 8640).0, "16K");
        assert_eq!(resolution_label(30000, 17000).0, "16K");
    }

    #[test]
    fn fps() {
        assert_eq!(fps_text(30.0), "30");
        assert_eq!(fps_text(30000.0 / 1001.0), "29.97");
        assert_eq!(fps_text(24000.0 / 1001.0), "23.98");
        assert_eq!(fps_text(0.0), "?");
        let mut m = FpsMeter::default();
        for i in 0..=30 {
            m.frame(i as f64 / 30.0);
        }
        let r = m.rate(1.0).unwrap();
        assert!((r - 30.0).abs() < 1.5, "{r}");
        assert_eq!(m.rate(3.0), None); // se cortó
    }
}

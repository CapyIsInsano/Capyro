//! Sonidos de la interfaz: cuáles hay, cuándo suenan y el clic de los botones.

use super::*;

/// Slots de efectos: 0..=2 capibara (logo), 3 error, 4 inicio, 5 actualización,
/// 6 clic de botón, 7 captura, 8 "lista para descargar", 9 cerrar/retraer.
pub(crate) const SOUNDS: [&[u8]; 11] = [
    include_bytes!("../../../assets/sounds/capybara1.mp3"),
    include_bytes!("../../../assets/sounds/capybara2.mp3"),
    include_bytes!("../../../assets/sounds/capybara3.mp3"),
    include_bytes!("../../../assets/sounds/error.mp3"),
    include_bytes!("../../../assets/sounds/inicio.mp3"),
    include_bytes!("../../../assets/sounds/actualizacion.mp3"),
    include_bytes!("../../../assets/sounds/click.mp3"),
    include_bytes!("../../../assets/sounds/photo.mp3"),
    include_bytes!("../../../assets/sounds/ready.mp3"),
    include_bytes!("../../../assets/sounds/retraction.mp3"),
    include_bytes!("../../../assets/sounds/efecto.mp3"),
];
pub(crate) const SFX_CAPYBARAS: usize = 3;
pub(crate) const SFX_ERROR: usize = 3;
pub(crate) const SFX_START: usize = 4;
pub(crate) const SFX_UPDATE: usize = 5;
pub(crate) const SFX_CLICK: usize = 6;
pub(crate) const SFX_PHOTO: usize = 7;
pub(crate) const SFX_READY: usize = 8;
pub(crate) const SFX_RETRACT: usize = 9;
/// Al cambiar de efecto (un preset: Rock, Lo-fi, Noche...).
pub(crate) const SFX_EFFECT: usize = 10;
/// Con `CAPYRO_DEBUG_SFX=1` cada sonido se anota en la terminal (para probar
/// sin parlantes o ver cuál suena).
pub(crate) fn debug_sfx(slot: usize) {
    const NAMES: [&str; 11] =
        ["capibara1", "capibara2", "capibara3", "error", "inicio", "actualizacion", "click", "photo", "ready", "retraction", "efecto"];
    if std::env::var_os("CAPYRO_DEBUG_SFX").is_some() {
        eprintln!("capyro: sonido {}", NAMES.get(slot).copied().unwrap_or("?"));
    }
}

impl CapyroApp {
    /// Uno de los tres sonidos de capibara, al azar (sin repetir el anterior).
    pub(crate) fn play_capybara(&mut self) {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        let mut pick = (self.rng % SFX_CAPYBARAS as u64) as usize;
        if pick == self.last_capy {
            pick = (pick + 1) % SFX_CAPYBARAS;
        }
        self.last_capy = pick;
        self.sfx(pick);
    }

    pub(crate) fn play_error(&self) {
        self.sfx(SFX_ERROR);
    }

    /// Todos los sonidos pasan por acá: así el clic del botón no pisa al
    /// sonido propio de lo que se tocó (error, capibara, captura...).
    pub(crate) fn sfx(&self, slot: usize) {
        debug_sfx(slot);
        self.core.sfx_play(slot);
        self.sfx_frame.set(self.last_frame);
    }

    /// Suena al elegir otro efecto (si los sonidos de la interfaz están prendidos).
    pub(crate) fn effect_sound(&self) {
        if self.cfg.click_sound {
            self.sfx(SFX_EFFECT);
        }
    }

    /// Final del frame: si se tocó un botón y nada sonó, suena el clic.
    /// Cerrar/retraer suena al revés que abrir: si en este frame algo se
    /// cerró (y nada se abrió), suena "retraction" en vez del clic.
    pub(crate) fn play_click_if_needed(&self, ctx: &egui::Context) {
        let clicked = ctx.data_mut(|d| d.remove_temp::<bool>(Id::unique(CLICK_FLAG))).unwrap_or(false);
        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        // menús, paneles y pantalla completa: un bit cada uno
        let open = [
            self.show_settings,
            self.show_info,
            self.show_fx,
            self.show_changes,
            self.show_keys,
            self.show_playlist,
            self.show_offer,
            fullscreen,
            self.studio_open(),
            self.converter_open(),
        ]
        .iter()
        .enumerate()
        .fold(0u32, |m, (i, &on)| m | (u32::from(on) << i));
        let before = self.ui_open.replace(open);
        let closed = before & !open != 0;
        let opened = open & !before != 0;
        if !self.cfg.click_sound || self.sfx_frame.get() == self.last_frame {
            return; // apagado, o ya sonó algo propio (error, ready, captura...)
        }
        if closed && !opened {
            debug_sfx(SFX_RETRACT);
            self.core.sfx_play(SFX_RETRACT);
        } else if clicked {
            debug_sfx(SFX_CLICK);
            self.core.sfx_play(SFX_CLICK);
        }
    }
}

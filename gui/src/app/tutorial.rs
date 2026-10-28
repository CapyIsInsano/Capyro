//! Tutorial de bienvenida.
//!
//! Sale solo la primera vez que se abre Capyro (y cuando se pide desde
//! Ajustes). Tiene cuatro partes, con música de fondo:
//!
//!   1. Presentación: logo arriba, párrafos que se escriben solos (Espacio
//!      para seguir) y abajo el perfil de GitHub del autor.
//!   2. Recorrido: aparece la interfaz (todavía sin funcionar) y se ilumina
//!      cada parte con su explicación.
//!   3. "¿Estás listo?": No → otra vuelta del recorrido; Sí → ...
//!   4. "Sistema detectado": Linux o Windows (10 u 11), con su versión.
//!   5. "¡Bienvenido a la manada!" y todo vuelve a la normalidad.

use std::path::PathBuf;

use eframe::egui::{
    self, epaint::RectShape, pos2, text::LayoutJob, vec2, Align, Align2, Color32, CursorIcon, Id, Key, Modifiers,
    Rect, Sense, Stroke, StrokeKind, TextureOptions, Ui, Vec2,
};

use super::{animate_button, decode_png, CapyroApp};
use crate::perf;
use crate::ffi::{OsInfo, State};
use crate::icons;
use crate::theme::*;

/// Música del tutorial (ya con el volumen al 45 %). Va como un MP3 de
/// verdad con tags y carátula: suena en el reproductor, así el área de video
/// muestra la carátula, el visualizador, el formato y los milisegundos
/// mientras se explican.
const TRACK: &[u8] = include_bytes!("../../../assets/sounds/tutorial-bienvenida-a-la-manada.mp3");
const TRACK_FILE: &str = "Capy y los Capibaras - Bienvenida a la manada.mp3";

/// Lo que había en el reproductor antes del tutorial (se devuelve al final).
pub(super) struct SavedQueue {
    pub playlist: Vec<String>,
    pub current: Option<usize>,
    pub position: f64,
    pub playing: bool,
    pub repeat: crate::ffi::Repeat,
    pub volume: f32,
}

/// Escribe la canción en ~/.cache/capyro/tutorial/ (si no estaba) y da la ruta.
fn track_path() -> Option<PathBuf> {
    let path = crate::paths::cache_dir().join("tutorial").join(TRACK_FILE);
    let ok = std::fs::metadata(&path).map(|m| m.len() == TRACK.len() as u64).unwrap_or(false);
    if !ok {
        std::fs::create_dir_all(path.parent()?).ok()?;
        std::fs::write(&path, TRACK).ok()?;
    }
    Some(path)
}
const AVATAR_PNG: &[u8] = include_bytes!("../../../assets/arturo.png");
const AUTHOR: &str = "Arturo";
const GITHUB_USER: &str = "CapyIsInsano";
const GITHUB_URL: &str = "https://github.com/CapyIsInsano";
/// Con qué se hizo Capyro (el link chiquito debajo del autor).
const CLAUDE_URL: &str = "https://www.anthropic.com/claude";

const INTRO: [&str; 4] = [
    "¡Hola! Bienvenido a Capyro.",
    "Capyro es un reproductor de audio y video para Linux y Windows: liviano, rápido y con un capibara de \
     mascota. Su núcleo está escrito en C, la interfaz en Rust, y todo se configura con Lua.",
    "La idea es simple: abrís un archivo, apretás play y listo. Just press play.",
    "Ahora te muestro cómo funciona todo. Todavía no toques nada: mirá y seguí con Espacio.",
];

struct Step {
    /// Parte de la interfaz que se ilumina (None = sin foco, cartel al centro).
    target: Option<&'static str>,
    title: &'static str,
    text: &'static str,
}

const STEPS: [Step; 16] = [
    Step {
        target: Some("video"),
        title: "El área de video",
        text: "Acá se ven tus videos, o la carátula y el visualizador cuando suena música. Arrastrá un archivo \
               encima o hacé clic para elegirlo. Doble clic: pantalla completa.",
    },
    Step {
        target: Some("badges"),
        title: "Formato y milisegundos",
        text: "Arriba a la izquierda vas a ver el formato del archivo (por ejemplo MP4 · H.264) y, debajo, los \
               milisegundos del segundo actual. En los videos, abajo a la izquierda, la resolución (1080p, 4K...) \
               y los fps. Todas se ocultan en Ajustes.",
    },
    Step {
        target: Some("add"),
        title: "Agregar archivos",
        text: "El botón naranja abre el selector de archivos de tu sistema. También con Ctrl+O.",
    },
    Step {
        target: Some("seek"),
        title: "La barra de progreso",
        text: "Hacé clic o arrastrá para saltar a cualquier parte. Si tocás el tiempo de la derecha, te muestra \
               cuánto falta. Las teclas 0 a 9 saltan del 0 al 90 %.",
    },
    Step {
        target: Some("controls"),
        title: "Los controles",
        text: "Aleatorio, anterior, volver al inicio, play/pausa, siguiente y repetir (no → toda la lista → uno solo). \
               Espacio también es play/pausa.",
    },
    Step {
        target: Some("list"),
        title: "La lista",
        text: "Tu cola de reproducción, que es temporal, y hasta 3 playlists guardadas. Pasá el mouse por una \
               canción de la cola y tocá + para agregarla a una playlist.",
    },
    Step {
        target: Some("info"),
        title: "Información",
        text: if cfg!(windows) {
            "La versión, los formatos con un resumen de cada uno, y el botón para buscar actualizaciones: te \
             muestra qué commits hay de nuevo y, si querés, baja la versión para Windows y se instala solo."
        } else {
            "La versión, los formatos con un resumen de cada uno, y el botón para buscar actualizaciones: te \
             muestra qué commits hay de nuevo y, si querés, se baja, se compila y se instala solo."
        },
    },
    Step {
        target: Some("changes"),
        title: "Novedades",
        text: "Lo nuevo de la versión que tenés. El puntito naranja aparece cuando hay algo que todavía no viste.",
    },
    Step {
        target: None,
        title: "Formatos",
        text: "Video: MP4, MOV, AVI, MKV y WebM. Audio: MP3, WAV, OGG, AAC, FLAC, Opus, M4A y AIFF. Están \
               todos en ⓘ → Formatos soportados. Los videos se decodifican por GPU cuando se puede. Si abrís \
               un formato que no está en la lista, Capyro te avisa con un sonido.",
    },
    Step {
        target: Some("fx"),
        title: "Efectos de audio",
        text: "El ecualizador: graves, medios y agudos. También el tono (sin cambiar la velocidad), el estéreo (hasta \
               mono) y el modo noche. Trae presets: Bajos, Voces, Lo-fi y Noche. Atajo: Ctrl+E.",
    },
    Step {
        target: Some("volume"),
        title: "Volumen",
        text: "Clic para silenciar. Con la rueda del mouse encima sube y baja de a un paso.",
    },
    Step {
        target: Some("subs"),
        title: "Subtítulos",
        text: "Todavía no están, pero llegan pronto.",
    },
    Step {
        target: Some("settings"),
        title: "Ajustes",
        text: "La velocidad (0.5× a 2×), la decodificación (automática, GPU o CPU), las burbujas de formato y \
               milisegundos, el editor de atajos, las capturas y este tutorial, por si lo querés repetir.",
    },
    Step {
        target: Some("fullscreen"),
        title: "Pantalla completa",
        text: "También con F o F11. Esc para salir.",
    },
    Step {
        target: Some("logo"),
        title: "El capibara",
        text: "Tocalo cuando termine el tutorial. En serio, tocalo.",
    },
    Step {
        target: None,
        title: "Atajos de teclado",
        text: "Espacio: play/pausa · ← →: adelantar y retroceder · ↑ ↓: volumen · M: silenciar · N y P: siguiente \
               y anterior · Shift+S: captura. Se cambian todos en Ajustes → Editar configuración.",
    },
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Phase {
    Intro(usize),
    /// La presentación se desvanece y aparece la interfaz.
    Reveal,
    Tour(usize),
    Ask,
    /// "Sistema detectado" (después del Sí, antes de la bienvenida).
    System,
    Welcome,
}

/// Cuánto dura "Detectando…" antes de mostrar el sistema.
const SCAN_TIME: f64 = 0.9;
/// La tarjeta del sistema se va sola a los...
const SYSTEM_TIME: f64 = 5.5;

pub(super) struct Tutorial {
    phase: Phase,
    /// Cuándo empezó el texto o la fase actual.
    t0: f64,
    start: f64,
    /// Se apretó Espacio mientras se escribía: mostrar todo.
    revealed_all: bool,
    /// Foco del recorrido: de dónde viene y hacia dónde va.
    spot_from: Rect,
    spot_to: Rect,
    spot_t0: f64,
    /// Se está cerrando (fundido de salida).
    closing: Option<f64>,
    /// La cola de antes (se devuelve al final).
    saved: SavedQueue,
    /// Veces que se contestó "No".
    rounds: u32,
    /// Duración del fundido de la música al cerrar.
    fade: f64,
    /// El sistema en el que corre (lo muestra la fase System).
    os: OsInfo,
}

/// Estado del destello del crédito: gira cuando le pasás el mouse (como
/// "pensando") y da una vuelta rápida con el clic.
#[derive(Clone, Copy, Default)]
struct Spark {
    angle: f32,
    speed: f32,
}

fn marker_path() -> Option<PathBuf> {
    crate::paths::state_dir().map(|d| d.join("tutorial-visto"))
}

/// ¿Es la primera vez que se abre Capyro?
pub(super) fn first_time() -> bool {
    marker_path().is_some_and(|p| !p.exists())
}

fn mark_seen() {
    if let Some(p) = marker_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&p, "El tutorial de bienvenida ya se vio. Borrá este archivo para verlo de nuevo al abrir Capyro.\n");
    }
}

/// Segundos que tarda en escribirse el carácter `c` (con pausas en la puntuación).
fn char_time(c: char) -> f32 {
    1.0 / 52.0
        + match c {
            '.' | '!' | '?' | ':' => 0.32,
            ',' | ';' => 0.13,
            '·' | '→' => 0.08,
            _ => 0.0,
        }
}

/// Cuántos caracteres se ven a los `elapsed` segundos.
fn revealed(text: &str, elapsed: f32) -> usize {
    let mut t = 0.0;
    let mut n = 0;
    for c in text.chars() {
        if t > elapsed {
            break;
        }
        n += 1;
        t += char_time(c);
    }
    n
}

fn total_time(text: &str) -> f32 {
    // la pausa del último carácter no cuenta
    text.chars().map(char_time).sum::<f32>() - text.chars().last().map_or(0.0, |c| char_time(c) - 1.0 / 52.0)
}

impl Tutorial {
    fn text(&self) -> Option<&'static str> {
        match self.phase {
            Phase::Intro(i) => Some(INTRO[i]),
            Phase::Tour(i) => Some(STEPS[i].text),
            _ => None,
        }
    }

    /// El texto actual ya se terminó de escribir.
    fn complete(&self, now: f64) -> bool {
        self.revealed_all || self.text().is_none_or(|t| (now - self.t0) as f32 >= total_time(t))
    }

    fn go(&mut self, phase: Phase, now: f64) {
        self.phase = phase;
        self.t0 = now;
        // en Bajo, el texto aparece entero (sin máquina de escribir)
        self.revealed_all = !perf::animations();
    }
}

impl CapyroApp {
    /// Arranca el tutorial (primera vez o desde Ajustes).
    pub(super) fn start_tutorial(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        self.close_menus();
        self.show_playlist = false;
        self.show_keys = false;
        self.show_offer = false;
        // se guarda la cola de ahora y suena la canción del tutorial
        let saved = SavedQueue {
            playlist: self.core.playlist(),
            current: self.core.playlist_current(),
            position: self.core.position(),
            playing: self.core.state() == State::Playing,
            repeat: self.core.repeat(),
            volume: self.core.volume(),
        };
        if saved.playing {
            self.core.toggle();
        }
        self.core.playlist_clear();
        match track_path() {
            Some(path) if self.core.playlist_add(&path).is_ok() => {
                self.core.set_repeat(crate::ffi::Repeat::One); // en loop mientras dure
                if self.core.playlist_select(0).is_ok() && self.core.state() != State::Playing {
                    self.core.toggle();
                }
            }
            _ => eprintln!("capyro: no pude preparar la música del tutorial"),
        }
        if self.avatar.is_none() {
            self.avatar = decode_png(AVATAR_PNG).map(|(w, h, rgba)| {
                let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                ctx.load_texture("capyro-avatar", img, TextureOptions::LINEAR)
            });
        }
        let center = Rect::from_center_size(ctx.content_rect().center(), Vec2::ZERO);
        self.tutorial = Some(Tutorial {
            phase: Phase::Intro(0),
            t0: now + 0.6, // el primer texto arranca cuando ya se ve la pantalla
            start: now,
            revealed_all: !perf::animations(),
            spot_from: center,
            spot_to: center,
            spot_t0: now,
            closing: None,
            saved,
            rounds: 0,
            fade: 0.8,
            os: crate::ffi::os_info(),
        });
    }

    /// La cola de antes del tutorial (para guardar la sesión mientras dura).
    pub(super) fn tutorial_saved(&self) -> Option<&SavedQueue> {
        self.tutorial.as_ref().map(|t| &t.saved)
    }

    /// Empieza a cerrarse: la música baja con un fundido de `fade` segundos.
    fn finish_tutorial(&mut self, now: f64, fade: f64) {
        if let Some(t) = &mut self.tutorial {
            if t.closing.is_none() {
                t.closing = Some(now);
                t.fade = fade;
                mark_seen();
            }
        }
    }

    /// Fin del tutorial: la cola, el repetir y el volumen vuelven a como estaban.
    fn restore_after_tutorial(&mut self, saved: SavedQueue) {
        self.core.playlist_clear();
        let mut cur = None;
        let mut n = 0;
        for (i, p) in saved.playlist.iter().enumerate() {
            let path = std::path::Path::new(p);
            if path.exists() && self.core.playlist_add(path).is_ok() {
                if saved.current == Some(i) {
                    cur = Some(n);
                }
                n += 1;
            }
        }
        self.core.set_repeat(saved.repeat);
        self.core.set_volume(saved.volume);
        if let Some(c) = cur {
            if self.core.playlist_select(c).is_ok() {
                if saved.position > 0.5 {
                    self.core.seek(saved.position);
                }
                if saved.playing {
                    self.core.toggle();
                }
            }
        }
    }

    /// Avanza: si el texto se está escribiendo lo completa; si no, pasa al siguiente.
    fn tutorial_next(&mut self, now: f64) {
        let rects = self.tutorial_targets();
        let Some(t) = &mut self.tutorial else { return };
        if t.closing.is_some() {
            return;
        }
        if !t.complete(now) {
            t.revealed_all = true;
            return;
        }
        match t.phase {
            Phase::Intro(i) if i + 1 < INTRO.len() => t.go(Phase::Intro(i + 1), now),
            Phase::Intro(_) => t.go(Phase::Reveal, now),
            Phase::Reveal => {}
            Phase::Tour(i) if i + 1 < STEPS.len() => {
                let to = target_rect(&rects, STEPS[i + 1].target);
                t.spot_from = t.spot_to;
                t.spot_to = to;
                t.spot_t0 = now;
                t.go(Phase::Tour(i + 1), now);
            }
            Phase::Tour(_) => {
                t.spot_from = t.spot_to;
                t.spot_to = Rect::from_center_size(t.spot_to.center(), Vec2::ZERO);
                t.spot_t0 = now;
                t.go(Phase::Ask, now);
            }
            Phase::System => {
                if now - t.t0 > SCAN_TIME {
                    t.go(Phase::Welcome, now);
                    self.play_capybara();
                }
            }
            Phase::Ask | Phase::Welcome => {}
        }
    }

    /// Dónde está cada parte de la interfaz (lo anota transport() en cada frame).
    fn tutorial_targets(&self) -> Vec<(&'static str, Rect)> {
        let mut v: Vec<(&'static str, Rect)> = self.ui_rects.iter().map(|(k, r)| (*k, *r)).collect();
        v.push(("info", self.info_rect));
        v.push(("changes", self.changes_rect));
        v.push(("fx", self.fx_rect));
        v.push(("settings", self.gear_rect));
        if let Some(video) = self.ui_rects.get("video") {
            v.push(("badges", Rect::from_min_size(video.min + vec2(4.0, 4.0), vec2(150.0, 66.0))));
        }
        v
    }

    /// Dibuja el tutorial encima de todo y se queda con la entrada.
    pub(super) fn tutorial_overlay(&mut self, ui: &mut Ui, full: Rect) {
        let Some(t) = &self.tutorial else { return };
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        // lo que solo decora (pulsos, esquinas que respiran): quieto en Bajo
        let deco = perf::clock(now);
        let accent = self.cfg.accent;

        // ¿terminó de cerrarse? (la música baja de a poco y después vuelve todo)
        if let Some(c) = t.closing {
            let k = ((now - c) / t.fade.max(0.1)).clamp(0.0, 1.0) as f32;
            self.core.set_volume(t.saved.volume * (1.0 - k));
            if k >= 1.0 && now - c > 0.6 {
                let t = self.tutorial.take().expect("tutorial");
                self.restore_after_tutorial(t.saved);
                return;
            }
        }
        let Some(t) = &self.tutorial else { return };
        let out = t.closing.map_or(1.0, |c| 1.0 - ease_out(((now - c) / t.fade.max(0.6)) as f32));
        let phase = t.phase;
        let phase_t0 = t.t0;

        // la reaparición de la interfaz termina sola
        if phase == Phase::Reveal && now - phase_t0 > 0.8 {
            let rects = self.tutorial_targets();
            let to = target_rect(&rects, STEPS[0].target);
            let t = self.tutorial.as_mut().expect("tutorial");
            t.spot_from = Rect::from_center_size(to.center(), Vec2::ZERO);
            t.spot_to = to;
            t.spot_t0 = now;
            t.go(Phase::Tour(0), now);
        }
        // el sistema detectado: se va solo a la bienvenida
        if phase == Phase::System && now - phase_t0 > SYSTEM_TIME {
            let t = self.tutorial.as_mut().expect("tutorial");
            t.go(Phase::Welcome, now);
            self.play_capybara();
        }
        // bienvenida: a los 3.6 s se va sola
        if phase == Phase::Welcome && now - phase_t0 > 3.6 {
            self.finish_tutorial(now, 1.5);
        }

        // todo el ratón y el teclado son del tutorial
        let block = ui.interact(full, Id::unique("capyro-tutorial-block"), Sense::click());
        let mut advance = block.clicked();
        let (space, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Space) || i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        advance |= space;

        let t = self.tutorial.as_ref().expect("tutorial");
        let p = {
            let mut p = ui.painter().clone();
            p.multiply_opacity(out);
            p
        };
        let mut link_clicked = false;
        let mut claude_clicked = false;
        let mut answer: Option<bool> = None;

        match t.phase {
            Phase::Intro(i) => {
                let a = ease_out(((now - t.start) / 0.6) as f32);
                self.draw_intro(ui, &p, full, i, a, now, &mut link_clicked, &mut claude_clicked);
            }
            Phase::Reveal => {
                let k = ease_in_out(((now - t.t0) / 0.8) as f32);
                // la presentación se va y la interfaz queda atenuada
                let mut pi = p.clone();
                pi.multiply_opacity(1.0 - k);
                let last = INTRO.len() - 1;
                self.draw_intro(ui, &pi, full, last, 1.0, now, &mut link_clicked, &mut claude_clicked);
                p.rect_filled(full, WINDOW_RADIUS, Color32::from_black_alpha((150.0 * k) as u8));
            }
            Phase::Tour(i) => {
                let k = ease_in_out(((now - t.spot_t0) / 0.42) as f32);
                let spot = lerp_rect(t.spot_from, t.spot_to, k);
                draw_spotlight(&p, full, spot, accent, deco, STEPS[i].target.is_some());
                if STEPS[i].target.is_some() {
                    draw_corners(&p, spot, accent, deco, k);
                }
                let shown = if t.revealed_all { usize::MAX } else { revealed(STEPS[i].text, (now - t.t0) as f32) };
                let complete = t.complete(now);
                let enter = ((now - t.t0) / 0.35) as f32;
                draw_bubble(&p, full, spot, &STEPS[i], i, shown, complete, enter, accent, deco, t.rounds);
            }
            Phase::Ask => {
                p.rect_filled(full, WINDOW_RADIUS, Color32::from_black_alpha(165));
                answer = self.draw_ask(ui, &p, full, now - t.t0, accent);
            }
            Phase::System => {
                p.rect_filled(full, WINDOW_RADIUS, Color32::from_black_alpha(165));
                draw_system(&p, full, &t.os, (now - t.t0) as f32, deco, accent);
            }
            Phase::Welcome => {
                p.rect_filled(full, WINDOW_RADIUS, Color32::from_black_alpha(165));
                self.draw_welcome(&p, full, (now - t.t0) as f32, accent);
            }
        }

        if link_clicked || claude_clicked {
            let url = if link_clicked { GITHUB_URL } else { CLAUDE_URL };
            if let Err(e) = super::changes_menu::open_url(url) {
                self.toast(&ctx, format!("No pude abrir el navegador: {e}"), true);
            }
        } else if let Some(yes) = answer {
            let t = self.tutorial.as_mut().expect("tutorial");
            if yes {
                // antes de la bienvenida: qué sistema se detectó
                t.go(Phase::System, now);
            } else {
                // otra vuelta del recorrido, desde el principio
                t.rounds += 1;
                let rects = self.tutorial_targets();
                let t = self.tutorial.as_mut().expect("tutorial");
                t.spot_from = Rect::from_center_size(full.center(), Vec2::ZERO);
                t.spot_to = target_rect(&rects, STEPS[0].target);
                t.spot_t0 = now;
                t.go(Phase::Tour(0), now);
            }
        } else if esc {
            self.finish_tutorial(now, 0.8);
        } else if advance && !matches!(phase, Phase::Ask | Phase::Welcome) {
            // (en System, Espacio pasa a la bienvenida una vez detectado)
            self.tutorial_next(now);
        }
        // 60 cuadros mientras algo entra, se mueve o se escribe; quieto (solo
        // las chispitas, halos y pulsos), 30
        let busy = self.tutorial.as_ref().is_none_or(|t| {
            let since = now - t.t0;
            t.closing.is_some()
                || since < 0.9
                || now - t.spot_t0 < 0.6
                || !t.complete(now)
                || matches!(t.phase, Phase::Reveal | Phase::Welcome)
        });
        // en Bajo no hay nada que se mueva: solo hace falta ver pasar el
        // tiempo (pasos que avanzan solos, la música que baja al cerrar)
        ctx.request_repaint_after(match (perf::animations(), busy) {
            (true, true) => super::FRAME_SMOOTH,
            (true, false) => super::FRAME_AMBIENT,
            (false, true) => perf::ambient(),
            (false, false) => std::time::Duration::from_millis(250),
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_intro(
        &self,
        ui: &Ui,
        p: &egui::Painter,
        full: Rect,
        para: usize,
        appear: f32,
        now: f64,
        link_clicked: &mut bool,
        claude_clicked: &mut bool,
    ) {
        let Some(t) = &self.tutorial else { return };
        let accent = self.cfg.accent;
        let mut p = p.clone();
        p.multiply_opacity(appear);
        p.rect_filled(full, WINDOW_RADIUS, BG_WINDOW);
        // brillo naranja arriba
        // degradé radial (malla con colores por vértice: sin escalones)
        {
            let c = pos2(full.center().x, full.top() + full.height() * 0.2);
            let mut mesh = egui::Mesh::default();
            let inner = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 34);
            mesh.colored_vertex(c, inner);
            let n = 72;
            for i in 0..=n {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                mesh.colored_vertex(c + vec2(a.cos(), a.sin()) * 300.0, Color32::TRANSPARENT);
            }
            for i in 1..=n as u32 {
                mesh.add_triangle(0, i, i + 1);
            }
            p.add(egui::Shape::mesh(mesh));
        }

        // chispitas que suben (en Bajo no hay)
        let anim = perf::animations();
        let since = (now - t.start) as f32;
        if anim {
            draw_embers(&p, full, accent, since, 34);
        }

        // logo, nombre y lema: entra saltando y después respira
        let bob = if anim { (since * 1.6).sin() * 3.0 } else { 0.0 };
        let logo_c = pos2(full.center().x, full.top() + full.height() * 0.2 + bob);
        // halos que salen del logo cada tanto
        for k in 0..if anim { 2 } else { 0 } {
            let ph = ((since / 2.6) + k as f32 * 0.5).fract();
            p.circle_stroke(logo_c, 50.0 + 70.0 * ph, Stroke::new(1.5, accent.gamma_multiply(0.35 * (1.0 - ph))));
        }
        if let Some(logo) = &self.logo {
            let pop = ease_back((since / 0.7).min(1.0));
            let breathe = if anim { 1.0 + 0.025 * (since * 2.2).sin() } else { 1.0 };
            let side = 92.0 * pop * breathe;
            p.image(logo.id(), Rect::from_center_size(logo_c, Vec2::splat(side)), uv(), Color32::WHITE);
        }
        p.text(logo_c + vec2(0.0, 66.0 - bob), Align2::CENTER_CENTER, "Capyro", bold(26.0), TEXT);
        p.text(logo_c + vec2(0.0, 90.0 - bob), Align2::CENTER_CENTER, "Just press play.", regular(13.0), TEXT_DIM);

        // el párrafo que se va escribiendo (centrado; el alto no salta)
        let text = INTRO[para];
        let width = (full.width() - 90.0).min(540.0);
        let font = regular(17.0);
        let full_job = centered_job(text, font.clone(), TEXT, width);
        let full_g = p.layout_job(full_job);
        let n = if t.revealed_all || !matches!(t.phase, Phase::Intro(_)) {
            usize::MAX
        } else {
            revealed(text, (now - t.t0).max(0.0) as f32)
        };
        let mut shown: String = text.chars().take(n).collect();
        // cursor de máquina de escribir mientras escribe (y un ratito después)
        let typing = n < text.chars().count();
        if typing || (anim && (now * 2.0) as i64 % 2 == 0 && !t.complete(now)) {
            shown.push('▌');
        }
        let g = p.layout_job(centered_job(&shown, font, TEXT, width));
        let top = full.top() + full.height() * 0.47;
        let fade = ease_out(((now - t.t0).max(0.0) / 0.35) as f32);
        let mut pt = p.clone();
        pt.multiply_opacity(fade);
        pt.galley(pos2(full.center().x, top + 12.0 * (1.0 - fade)), g, TEXT);

        // puntitos de progreso y "Espacio para continuar"
        let below = top + full_g.size().y + 26.0;
        let dots = INTRO.len();
        let widths: Vec<f32> = (0..dots)
            .map(|d| ui.ctx().animate_value_with_time(Id::unique(("capyro-tut-dot", d)), if d == para { 22.0 } else { 6.0 }, crate::perf::t(0.25)))
            .collect();
        let total: f32 = widths.iter().sum::<f32>() + 6.0 * (dots as f32 - 1.0);
        let mut x = full.center().x - total / 2.0;
        for (d, w) in widths.iter().enumerate() {
            let col = if d <= para { accent } else { Color32::from_white_alpha(40) };
            p.rect_filled(Rect::from_min_size(pos2(x, below - 3.0), vec2(*w, 6.0)), 3.0, col);
            x += w + 6.0;
        }
        if t.complete(now) && matches!(t.phase, Phase::Intro(_)) {
            // la tecla "Espacio" dibujada, que se hunde de a ratos
            let k = ease_out(((now - t.t0) as f32 - total_time(text)).max(0.0) / 0.3);
            let press = ((perf::clock(now) * 1.4).fract() < 0.12) as i32 as f32;
            let mut ph = p.clone();
            ph.multiply_opacity(k);
            let cy = below + 30.0;
            let kg = ph.layout_no_wrap("Espacio".to_owned(), bold(11.5), TEXT);
            let rg = ph.layout_no_wrap("para continuar".to_owned(), regular(12.5), TEXT_DIM);
            let kw = kg.size().x + 22.0;
            let w = kw + 10.0 + rg.size().x;
            let key = Rect::from_min_size(pos2(full.center().x - w / 2.0, cy - 12.0 + 2.0 * press), vec2(kw, 24.0));
            ph.rect_filled(key.translate(vec2(0.0, 3.0 - 2.0 * press)), 6.0, Color32::from_black_alpha(90));
            ph.rect_filled(key, 6.0, Color32::from_white_alpha(22).lerp_to_gamma(accent, 0.25 * press));
            ph.rect_stroke(key, 6.0, Stroke::new(1.0, Color32::from_white_alpha(50)), StrokeKind::Inside);
            ph.galley(key.center() - kg.size() / 2.0, kg, TEXT);
            ph.galley(pos2(key.right() + 10.0, cy - rg.size().y / 2.0), rg, TEXT_DIM);
        }

        // abajo: el perfil de GitHub del autor
        let card = Rect::from_center_size(pos2(full.center().x, full.bottom() - 84.0), vec2(250.0, 58.0));
        let a = animate_button(ui, Id::unique("capyro-tutorial-github"), card, appear > 0.9);
        p.text(pos2(card.center().x, card.top() - 12.0), Align2::CENTER_CENTER, "Hecho por", regular(11.0), TEXT_FAINT);
        p.rect_filled(card, 14.0, Color32::from_white_alpha((8.0 + 10.0 * a.hover) as u8));
        p.rect_stroke(card, 14.0, Stroke::new(1.0, Color32::from_white_alpha(16).lerp_to_gamma(accent, a.hover)), StrokeKind::Inside);
        let av = Rect::from_center_size(pos2(card.left() + 32.0, card.center().y), Vec2::splat(40.0));
        if let Some(tex) = &self.avatar {
            p.add(RectShape::filled(av, 20.0, Color32::WHITE).with_texture(tex.id(), uv()));
            p.circle_stroke(av.center(), 20.0, Stroke::new(1.5, accent.gamma_multiply(0.8)));
        }
        let tx = av.right() + 12.0;
        p.text(pos2(tx, card.center().y - 9.0), Align2::LEFT_CENTER, AUTHOR, bold(14.0), TEXT);
        let link = format!("github.com/{GITHUB_USER}");
        let lg = p.layout_no_wrap(link, regular(12.0), accent);
        let lw = lg.size().x;
        let ly = card.center().y + 9.0;
        p.galley(pos2(tx, ly - lg.size().y / 2.0), lg, accent);
        // flechita ↗
        let ac = pos2(tx + lw + 9.0 + 2.0 * a.hover, ly - 2.0 * a.hover);
        let st = Stroke::new(1.4, accent);
        p.line_segment([ac + vec2(-3.5, 3.5), ac + vec2(3.5, -3.5)], st);
        p.line_segment([ac + vec2(-0.5, -3.5), ac + vec2(3.5, -3.5)], st);
        p.line_segment([ac + vec2(3.5, -3.5), ac + vec2(3.5, 0.5)], st);
        if a.hover > 0.5 {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if a.resp.clicked() {
            *link_clicked = true;
        }

        // más abajo y más chiquito: con qué se hizo
        let cy = full.bottom() - 30.0;
        let pre = p.layout_no_wrap("Potenciado por ".to_owned(), regular(11.0), TEXT_FAINT);
        let name = p.layout_no_wrap("Claude".to_owned(), bold(11.0), TEXT_DIM);
        let post = p.layout_no_wrap(" · Anthropic".to_owned(), regular(11.0), TEXT_FAINT);
        let w = pre.size().x + name.size().x + post.size().x;
        // un poco a la derecha: a la izquierda va el destello
        let r = Rect::from_center_size(pos2(full.center().x + 12.0, cy), vec2(w + 26.0, 20.0));
        let b = animate_button(ui, Id::unique("capyro-tutorial-claude"), r, appear > 0.9);
        if b.hover > 0.001 {
            p.rect_filled(r, 10.0, Color32::from_white_alpha((8.0 * b.hover) as u8));
        }
        // a la izquierda, el destello: gira con el mouse encima y da una
        // vuelta rápida al hacer clic
        {
            let ctx = ui.ctx();
            let id = Id::unique("capyro-tutorial-spark");
            let dt = ctx.input(|i| i.stable_dt).min(0.05);
            let mut sp: Spark = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
            let target = if b.hover > 0.5 { 7.0 } else { 0.0 };
            sp.speed += (target - sp.speed) * (dt * 5.0).min(1.0);
            if b.resp.clicked() || b.resp.is_pointer_button_down_on() && sp.speed < 12.0 {
                sp.speed = 24.0;
            }
            sp.angle += sp.speed * dt;
            if b.hover < 0.5 && sp.speed.abs() < 1.0 {
                // se acomoda en la posición de reposo (simetría de 4 puntas)
                let q = std::f32::consts::FRAC_PI_2;
                let rest = (sp.angle / q).round() * q;
                sp.angle += (rest - sp.angle) * (dt * 8.0).min(1.0);
            }
            sp.angle %= std::f32::consts::TAU;
            ctx.data_mut(|d| d.insert_temp(id, sp));
            if !anim {
                sp.speed = 0.0; // en Bajo el destello no gira
            }
            if anim && (sp.speed.abs() > 0.01 || b.hover > 0.001) {
                ctx.request_repaint_after(super::FRAME_SMOOTH);
            }
            let sc = pos2(r.left() - 10.0, cy);
            let pulse = 1.0 + 0.12 * b.hover * ((perf::clock(now) * 6.0).sin() as f32 * 0.5 + 0.5);
            let col = TEXT_DIM.lerp_to_gamma(accent, b.hover);
            if b.hover > 0.001 {
                p.circle_filled(sc, 11.0, accent.gamma_multiply(0.12 * b.hover));
            }
            icons::sparkle(&p, sc, 9.0 * pulse, sp.angle, col);
            icons::sparkle(&p, sc + vec2(7.5, -7.0), 3.6 * pulse, -sp.angle * 1.4, col.gamma_multiply(0.8));
        }
        let mut x = r.left() + 8.0;
        let ty = cy - pre.size().y / 2.0;
        let (pw, nw) = (pre.size().x, name.size().x);
        p.galley(pos2(x, ty), pre, TEXT_FAINT);
        x += pw;
        p.galley(pos2(x, ty), name, TEXT_DIM);
        x += nw;
        p.galley(pos2(x, ty), post, TEXT_FAINT);
        let col = TEXT_FAINT.lerp_to_gamma(accent, b.hover);
        let ac = pos2(r.right() - 9.0 + 1.5 * b.hover, cy - 1.5 * b.hover);
        let st = Stroke::new(1.1, col);
        p.line_segment([ac + vec2(-2.5, 2.5), ac + vec2(2.5, -2.5)], st);
        p.line_segment([ac + vec2(-0.5, -2.5), ac + vec2(2.5, -2.5)], st);
        p.line_segment([ac + vec2(2.5, -2.5), ac + vec2(2.5, -0.5)], st);
        if b.hover > 0.5 {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if b.resp.clicked() {
            *claude_clicked = true;
        }

        p.text(
            pos2(full.right() - 18.0, full.bottom() - 16.0),
            Align2::RIGHT_CENTER,
            "Esc: saltar",
            regular(10.5),
            TEXT_FAINT,
        );
    }

    /// "¿Estás listo?" — devuelve Some(true/false) cuando se elige.
    fn draw_ask(&self, ui: &Ui, p: &egui::Painter, full: Rect, since: f64, accent: Color32) -> Option<bool> {
        let k = ease_out((since / 0.35) as f32);
        let pop = ease_back((since / 0.45) as f32);
        let size = vec2(360.0, 200.0) * (0.85 + 0.15 * pop);
        let card = Rect::from_center_size(full.center(), size);
        let mut p = p.clone();
        p.multiply_opacity(k);
        // brillo naranja que late detrás de la tarjeta
        let anim = perf::animations();
        let glow = if anim { 0.5 + 0.5 * ((since * 2.0).sin() as f32) } else { 0.5 };
        p.rect_filled(card.expand(6.0 + 4.0 * glow), 24.0, accent.gamma_multiply(0.06 + 0.05 * glow));
        p.rect_filled(card.translate(vec2(0.0, 6.0)), 18.0, Color32::from_black_alpha(90));
        p.rect_filled(card, 18.0, BG_PANEL);
        p.rect_stroke(card, 18.0, Stroke::new(1.0, accent.gamma_multiply(0.5)), StrokeKind::Inside);
        if let Some(logo) = &self.logo {
            // el capibara mira a los costados, como esperando la respuesta
            let (tilt, hop) = if anim { (((since * 1.8).sin() as f32) * 4.0, ((since * 3.6).sin() as f32).abs() * 3.0) } else { (0.0, 0.0) };
            let lc = pos2(card.center().x + tilt, card.top() + 40.0 - hop);
            p.image(logo.id(), Rect::from_center_size(lc, Vec2::splat(46.0)), uv(), Color32::WHITE);
        }
        let tk = ease_out(((since - 0.12) / 0.3) as f32);
        let mut pt = p.clone();
        pt.multiply_opacity(tk);
        pt.text(pos2(card.center().x, card.top() + 90.0 + 8.0 * (1.0 - tk)), Align2::CENTER_CENTER, "¿Estás listo?", bold(22.0), TEXT);
        if k < 0.9 && anim {
            ui.ctx().request_repaint_after(super::FRAME_SMOOTH);
        }
        let by = card.bottom() - 42.0;
        let mut out = None;
        for (n, (yes, label, x)) in [(true, "Sí", card.center().x - 66.0), (false, "No", card.center().x + 66.0)].into_iter().enumerate() {
            // los botones aparecen uno detrás del otro, con saltito
            let bk = perf::k(((since - 0.25 - 0.1 * n as f64) / 0.35) as f32);
            if bk <= 0.0 {
                continue;
            }
            let bpop = ease_back(bk);
            let r = Rect::from_center_size(pos2(x, by), vec2(116.0, 40.0));
            let a = animate_button(ui, Id::unique(("capyro-tutorial-ask", yes)), r, bk >= 1.0);
            let rr = Rect::from_center_size(r.center(), r.size() * (bpop + 0.04 * a.hover - 0.08 * a.press));
            if yes {
                if a.hover > 0.001 {
                    p.rect_filled(rr.expand(4.0 * a.hover), 24.0, accent.gamma_multiply(0.2 * a.hover));
                }
                p.rect_filled(rr, 20.0, accent.lerp_to_gamma(Color32::WHITE, 0.12 * a.hover));
                p.text(rr.center(), Align2::CENTER_CENTER, label, bold(15.0), Color32::WHITE);
            } else {
                p.rect_filled(rr, 20.0, Color32::from_white_alpha((8.0 + 12.0 * a.hover) as u8));
                p.rect_stroke(rr, 20.0, Stroke::new(1.0, Color32::from_white_alpha(40)), StrokeKind::Inside);
                p.text(rr.center(), Align2::CENTER_CENTER, label, bold(15.0), TEXT);
            }
            if a.resp.clicked() {
                out = Some(yes);
            }
        }
        out
    }

    fn draw_welcome(&self, p: &egui::Painter, full: Rect, since: f32, accent: Color32) {
        let c = full.center();
        let anim = perf::animations();
        // papelitos que caen (en Bajo no hay)
        let mut seed: u32 = 0x9E37_79B9;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % 10_000) as f32 / 10_000.0
        };
        for i in 0..if anim { 60 } else { 0 } {
            let x0 = full.left() + rnd() * full.width();
            let speed = 120.0 + 160.0 * rnd();
            let delay = rnd() * 0.8;
            let sway = (since * (2.0 + 2.0 * rnd()) + i as f32).sin() * 14.0;
            let y = full.top() - 20.0 + (since - delay).max(0.0) * speed;
            if y > full.bottom() + 10.0 {
                continue;
            }
            let col = match i % 4 {
                0 => accent,
                1 => Color32::WHITE,
                2 => accent.lerp_to_gamma(Color32::WHITE, 0.4),
                _ => Color32::from_rgb(0xFF, 0xC2, 0x8A),
            };
            let s = 3.0 + 3.0 * rnd();
            // papelito que gira (y se "da vuelta": el ancho cambia)
            let ang = since * (3.0 + 4.0 * rnd()) + i as f32;
            let flip = (since * (4.0 + 3.0 * rnd()) + i as f32).cos().abs().max(0.15);
            let (sa, ca) = ang.sin_cos();
            let c = pos2(x0 + sway, y);
            let (hw, hh) = (s * 0.5 * flip, s * 0.8);
            let pts = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)]
                .map(|(dx, dy)| pos2(c.x + dx * ca - dy * sa, c.y + dx * sa + dy * ca))
                .to_vec();
            p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
        }
        // rayos de luz que giran detrás del logo
        if anim {
            let lc = c - vec2(0.0, 70.0);
            let k = ease_out(since / 0.6);
            let n = 12;
            for i in 0..n {
                let a0 = since * 0.4 + i as f32 / n as f32 * std::f32::consts::TAU;
                let a1 = a0 + 0.13;
                let len = 170.0 * k;
                let mut mesh = egui::Mesh::default();
                mesh.colored_vertex(lc, accent.gamma_multiply(0.22 * k));
                mesh.colored_vertex(lc + vec2(a0.cos(), a0.sin()) * len, Color32::TRANSPARENT);
                mesh.colored_vertex(lc + vec2(a1.cos(), a1.sin()) * len, Color32::TRANSPARENT);
                mesh.add_triangle(0, 1, 2);
                p.add(egui::Shape::mesh(mesh));
            }
            // onda que sale al aparecer
            let ring = (since / 0.9).clamp(0.0, 1.0);
            if ring < 1.0 {
                p.circle_stroke(lc, 40.0 + 120.0 * ring, Stroke::new(3.0, accent.gamma_multiply(1.0 - ring)));
            }
        }
        // logo que salta y el título
        let pop = ease_back(since / 0.5);
        let bounce = if anim { (-since * 3.0).exp() * (since * 10.0).sin().abs() * 26.0 } else { 0.0 };
        if let Some(logo) = &self.logo {
            let side = 110.0 * pop;
            p.image(logo.id(), Rect::from_center_size(c - vec2(0.0, 70.0 + bounce), Vec2::splat(side)), uv(), Color32::WHITE);
        }
        // franja oscura difuminada para que el texto se lea sobre lo que haya atrás
        {
            let k = ease_out((since - 0.1) / 0.4);
            let band = Rect::from_center_size(c + vec2(0.0, 44.0), vec2(full.width(), 130.0));
            let mut mesh = egui::Mesh::default();
            let dark = Color32::from_black_alpha((225.0 * k) as u8);
            let (t, m, b) = (band.top(), band.center().y, band.bottom());
            for (y, col) in [(t, Color32::TRANSPARENT), (m, dark), (b, Color32::TRANSPARENT)] {
                mesh.colored_vertex(pos2(band.left(), y), col);
                mesh.colored_vertex(pos2(band.right(), y), col);
            }
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(1, 3, 2);
            mesh.add_triangle(2, 3, 4);
            mesh.add_triangle(3, 5, 4);
            p.add(egui::Shape::mesh(mesh));
        }
        let mut pt = p.clone();
        pt.multiply_opacity(ease_out((since - 0.2) / 0.4));
        let scale = 0.85 + 0.15 * ease_out((since - 0.2) / 0.4);
        pt.text(c + vec2(0.0, 18.0), Align2::CENTER_CENTER, "¡Bienvenido a la manada!", bold(30.0 * scale), TEXT);
        let mut ps = p.clone();
        ps.multiply_opacity(ease_out((since - 0.7) / 0.4));
        ps.text(c + vec2(0.0, 56.0), Align2::CENTER_CENTER, "Ya podés usar Capyro. Just press play.", regular(14.0), TEXT_DIM);
    }
}

/// "Sistema detectado": primero un "Detectando…" corto y después el ícono
/// (original, no el logo del sistema), el nombre y la versión.
fn draw_system(p: &egui::Painter, full: Rect, os: &OsInfo, since: f32, now: f64, accent: Color32) {
    let k = ease_out(since / 0.35);
    let size = vec2(380.0, 268.0) * (0.92 + 0.08 * k);
    let card = Rect::from_center_size(full.center(), size);
    let mut p = p.clone();
    p.multiply_opacity(k);
    p.rect_filled(card.translate(vec2(0.0, 6.0)), 18.0, Color32::from_black_alpha(90));
    p.rect_filled(card, 18.0, BG_PANEL);
    p.rect_stroke(card, 18.0, Stroke::new(1.0, accent.gamma_multiply(0.5)), StrokeKind::Inside);
    let cx = card.center().x;
    let icon_c = pos2(cx, card.top() + 74.0);

    let scan = (since as f64 / SCAN_TIME).clamp(0.0, 1.0) as f32;
    if scan < 1.0 {
        // "Detectando tu sistema…" con un spinner
        icons::spinner(&p, icon_c, 54.0, accent, now);
        p.text(pos2(cx, card.top() + 150.0), Align2::CENTER_CENTER, "Detectando tu sistema…", regular(15.0), TEXT_DIM);
        return;
    }
    let t = since - SCAN_TIME as f32;
    let pop = ease_out(t / 0.4);
    let bounce = if perf::animations() { (-t * 4.0).exp() * (t * 12.0).sin().abs() * 10.0 } else { 0.0 };
    // brillo detrás del ícono
    p.circle_filled(icon_c, 44.0 * pop, accent.gamma_multiply(0.12));
    let side = 72.0 * (0.6 + 0.4 * pop);
    let mut pi = p.clone();
    pi.multiply_opacity(pop);
    let ic = icon_c - vec2(0.0, bounce);
    if os.windows {
        icons::app_window(&pi, ic, side, Color32::from_gray(200), accent, os.major >= 11);
        if os.major >= 10 {
            // la versión en una pastillita, en la esquina del ícono
            let chip_c = ic + vec2(side * 0.48, side * 0.36);
            let g = pi.layout_no_wrap(os.major.to_string(), bold(12.0), Color32::WHITE);
            let chip = Rect::from_center_size(chip_c, vec2(g.size().x + 14.0, 20.0));
            pi.rect_filled(chip, 10.0, accent);
            pi.galley(chip.center() - g.size() / 2.0, g, Color32::WHITE);
        }
    } else {
        icons::terminal(&pi, ic, side, Color32::from_gray(200), accent, now);
    }

    let mut pt = p.clone();
    pt.multiply_opacity(ease_out((t - 0.15) / 0.35));
    pt.text(pos2(cx, card.top() + 136.0), Align2::CENTER_CENTER, "Sistema detectado", regular(12.5), TEXT_FAINT);
    // grande: la familia ("Linux", "Windows 11"); chico: distro o versión
    let (big, small) = if os.windows {
        let big = if os.name.is_empty() { "Windows".to_owned() } else { os.name.clone() };
        (big, os.version.clone())
    } else {
        let distro = if os.name.is_empty() || os.name == "Linux" { String::new() } else { os.name.clone() };
        let small = match (distro.is_empty(), os.version.is_empty()) {
            (false, false) => format!("{distro} · {}", os.version),
            (false, true) => distro,
            _ => os.version.clone(),
        };
        ("Linux".to_owned(), small)
    };
    pt.text(pos2(cx, card.top() + 162.0), Align2::CENTER_CENTER, big, bold(24.0), TEXT);
    if !small.is_empty() {
        let g = pt.layout_no_wrap(small, regular(12.5), TEXT_DIM);
        let w = g.size().x.min(card.width() - 40.0);
        let mut pc = pt.clone();
        pc.set_clip_rect(Rect::from_center_size(pos2(cx, card.top() + 188.0), vec2(w, 20.0)));
        pc.galley(pos2(cx - g.size().x.min(w) / 2.0, card.top() + 188.0 - g.size().y / 2.0), g, TEXT_DIM);
    }
    let mut pb = p.clone();
    pb.multiply_opacity(ease_out((t - 0.45) / 0.35));
    let line = format!(
        "Capyro está compilado para {} ({}): las actualizaciones instalan solo lo de tu sistema.",
        if os.windows { "Windows" } else { "Linux" },
        os.code
    );
    let g = pb.layout_job(centered_job(&line, regular(11.5), TEXT_FAINT, card.width() - 56.0));
    let gy = card.bottom() - 34.0 - g.size().y / 2.0;
    pb.galley(pos2(cx, gy), g, TEXT_FAINT);
    // abajo, fuera de la tarjeta: cómo seguir (y una barrita del tiempo que queda)
    if t > 0.8 {
        let pulse = 0.55 + 0.45 * ((now * 3.0).sin() as f32).abs();
        let mut ph = p.clone();
        ph.multiply_opacity(pulse * ease_out((t - 0.8) / 0.3));
        ph.text(pos2(cx, card.bottom() + 22.0), Align2::CENTER_CENTER, "Espacio para seguir  ▸", regular(12.0), TEXT_DIM);
    }
    let left = 1.0 - ((since as f64 - SCAN_TIME) / (SYSTEM_TIME - SCAN_TIME)).clamp(0.0, 1.0) as f32;
    let bar = Rect::from_min_size(pos2(card.left() + 18.0, card.bottom() - 6.0), vec2((card.width() - 36.0) * left, 2.0));
    p.rect_filled(bar, 1.0, accent.gamma_multiply(0.6));
}

// En el tutorial todas las curvas miden tiempo: en modo Bajo (sin
// animaciones) todo aparece ya terminado.
fn ease_out(t: f32) -> f32 {
    super::ease_out(perf::k(t))
}

fn ease_in_out(t: f32) -> f32 {
    super::ease_in_out(perf::k(t))
}

/// Sale un poco de más y vuelve (para las cosas que "saltan" al aparecer).
fn ease_back(t: f32) -> f32 {
    let t = perf::k(t).clamp(0.0, 1.0);
    let c = 1.70158;
    1.0 + (c + 1.0) * (t - 1.0).powi(3) + c * (t - 1.0).powi(2)
}

/// Generador chiquito y determinista (las partículas no cambian de un frame a otro).
fn hash01(i: u32, salt: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x % 10_000) as f32 / 10_000.0
}

/// Chispitas que suben despacio por el fondo.
fn draw_embers(p: &egui::Painter, full: Rect, accent: Color32, t: f32, n: u32) {
    for i in 0..n {
        let speed = 14.0 + 26.0 * hash01(i, 1);
        let x0 = full.left() + hash01(i, 2) * full.width();
        let phase = hash01(i, 3) * full.height();
        let travel = (phase + t * speed) % (full.height() + 40.0);
        let y = full.bottom() + 20.0 - travel;
        let x = x0 + (t * (0.6 + hash01(i, 4)) + i as f32).sin() * 10.0;
        // aparecen abajo y se apagan arriba
        let life = travel / (full.height() + 40.0);
        let alpha = (life * 4.0).min(1.0) * (1.0 - life).powf(1.5);
        let r = 1.0 + 1.8 * hash01(i, 5);
        let col = if i % 3 == 0 { Color32::WHITE } else { accent };
        p.circle_filled(pos2(x, y), r, col.gamma_multiply(0.55 * alpha));
    }
}

/// Esquinas tipo visor alrededor de lo iluminado, que respiran.
fn draw_corners(p: &egui::Painter, r: Rect, accent: Color32, now: f64, k: f32) {
    let pulse = 0.5 + 0.5 * ((now * 2.6).sin() as f32);
    let out = 4.0 + 3.0 * pulse;
    let r = r.expand(out);
    let len = (r.width().min(r.height()) * 0.25).clamp(8.0, 18.0) * k;
    let st = Stroke::new(2.5, accent);
    for (c, dx, dy) in [
        (r.left_top(), 1.0, 1.0),
        (r.right_top(), -1.0, 1.0),
        (r.left_bottom(), 1.0, -1.0),
        (r.right_bottom(), -1.0, -1.0),
    ] {
        p.line_segment([c, c + vec2(len * dx, 0.0)], st);
        p.line_segment([c, c + vec2(0.0, len * dy)], st);
    }
}

fn uv() -> Rect {
    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))
}

fn centered_job(text: &str, font: egui::FontId, color: Color32, width: f32) -> LayoutJob {
    let mut job = LayoutJob::simple(text.to_owned(), font, color, width);
    job.halign = Align::Center;
    job
}

fn lerp_rect(a: Rect, b: Rect, k: f32) -> Rect {
    Rect::from_min_max(a.min.lerp(b.min, k), a.max.lerp(b.max, k))
}

fn target_rect(rects: &[(&'static str, Rect)], target: Option<&str>) -> Rect {
    let found = target.and_then(|t| rects.iter().find(|(k, _)| *k == t).map(|(_, r)| *r));
    match found {
        Some(r) if r.is_positive() => r.expand(6.0),
        _ => {
            // sin foco: un punto en el centro de la ventana
            let c = rects.iter().find(|(k, _)| *k == "video").map_or(pos2(384.0, 300.0), |(_, r)| r.center());
            Rect::from_center_size(c, Vec2::ZERO)
        }
    }
}

/// Todo oscuro menos `hole`, con un borde naranja que late.
fn draw_spotlight(p: &egui::Painter, full: Rect, hole: Rect, accent: Color32, now: f64, focused: bool) {
    let dim = Color32::from_black_alpha(170);
    let h = hole.intersect(full);
    if h.width() <= 1.0 || h.height() <= 1.0 {
        p.rect_filled(full, WINDOW_RADIUS, dim);
        return;
    }
    p.rect_filled(Rect::from_min_max(full.min, pos2(full.right(), h.top())), 0.0, dim);
    p.rect_filled(Rect::from_min_max(pos2(full.left(), h.bottom()), full.max), 0.0, dim);
    p.rect_filled(Rect::from_min_max(pos2(full.left(), h.top()), pos2(h.left(), h.bottom())), 0.0, dim);
    p.rect_filled(Rect::from_min_max(pos2(h.right(), h.top()), pos2(full.right(), h.bottom())), 0.0, dim);
    if focused {
        let pulse = 0.5 + 0.5 * ((now * 2.6).sin() as f32);
        p.rect_stroke(h.expand(3.0 + 3.0 * pulse), 12.0, Stroke::new(4.0, accent.gamma_multiply(0.18 * pulse)), StrokeKind::Outside);
        p.rect_stroke(h, 10.0, Stroke::new(2.0, accent), StrokeKind::Outside);
    }
}

/// El cartel con la explicación del paso, cerca de lo iluminado.
#[allow(clippy::too_many_arguments)]
fn draw_bubble(
    p: &egui::Painter,
    full: Rect,
    spot: Rect,
    step: &Step,
    index: usize,
    shown: usize,
    complete: bool,
    enter: f32,
    accent: Color32,
    now: f64,
    rounds: u32,
) {
    let w = 340.0_f32.min(full.width() - 24.0);
    let inner = w - 32.0;
    let full_text = p.layout(step.text.to_owned(), regular(13.0), TEXT, inner);
    let h = 16.0 + 22.0 + 10.0 + full_text.size().y + 14.0 + 18.0 + 14.0;
    // abajo de lo iluminado si entra; si no, arriba; sin foco, al centro
    let focused = spot.width() > 1.0;
    let mut y = if !focused {
        full.center().y - h / 2.0
    } else if spot.bottom() + 14.0 + h < full.bottom() - 8.0 {
        spot.bottom() + 14.0
    } else {
        spot.top() - 14.0 - h
    };
    y = y.clamp(full.top() + 8.0, full.bottom() - 8.0 - h);
    let x = if focused { spot.center().x - w / 2.0 } else { full.center().x - w / 2.0 };
    let x = x.clamp(full.left() + 12.0, full.right() - 12.0 - w);
    // entra con un saltito (un poco más grande y vuelve)
    let pop = ease_back(enter);
    let fade = ease_out(enter);
    let base = Rect::from_min_size(pos2(x, y + 10.0 * (1.0 - fade)), vec2(w, h));
    let r = Rect::from_center_size(base.center(), base.size() * (0.9 + 0.1 * pop));

    let mut p = p.clone();
    p.multiply_opacity(fade);
    p.rect_filled(r.translate(vec2(0.0, 5.0)), 14.0, Color32::from_black_alpha(90));
    p.rect_filled(r, 14.0, BG_PANEL);
    p.rect_stroke(r, 14.0, Stroke::new(1.0, accent.gamma_multiply(0.45)), StrokeKind::Inside);

    // "3 / 16" y el título
    let x0 = r.left() + 16.0;
    let ty = r.top() + 16.0 + 11.0;
    let cg = p.layout_no_wrap(format!("{} / {}", index + 1, STEPS.len()), bold(10.5), accent);
    let chip = Rect::from_min_size(pos2(x0, ty - 9.0), cg.size() + vec2(12.0, 18.0 - cg.size().y));
    p.rect_filled(chip, 9.0, accent.gamma_multiply(0.16));
    p.galley(chip.center() - cg.size() / 2.0, cg, accent);
    let title = if index == 0 && rounds > 0 { "Otra vuelta: el área de video" } else { step.title };
    p.text(pos2(chip.right() + 8.0, ty), Align2::LEFT_CENTER, title, bold(14.0), TEXT);

    // el texto, escribiéndose
    let mut text: String = step.text.chars().take(shown).collect();
    if shown < step.text.chars().count() {
        text.push('▌');
    }
    let g = p.layout(text, regular(13.0), TEXT, inner);
    let text_top = ty + 9.0 + 10.0;
    p.galley(pos2(x0, text_top), g, TEXT);

    // progreso del recorrido: una barrita fina abajo del cartel
    let prog = (index as f32 + if complete { 1.0 } else { 0.5 }) / STEPS.len() as f32;
    let track = Rect::from_min_max(pos2(r.left() + 14.0, r.bottom() - 4.0), pos2(r.right() - 14.0, r.bottom() - 2.0));
    p.rect_filled(track, 1.0, Color32::from_white_alpha(16));
    p.rect_filled(Rect::from_min_size(track.min, vec2(track.width() * prog, track.height())), 1.0, accent);

    // pie
    let fy = r.bottom() - 16.0;
    p.text(pos2(x0, fy), Align2::LEFT_CENTER, "Esc: saltar", regular(10.5), TEXT_FAINT);
    if complete {
        let pulse = 0.55 + 0.45 * ((now * 3.0).sin() as f32).abs();
        let mut ph = p.clone();
        ph.multiply_opacity(pulse);
        let last = index + 1 == STEPS.len();
        ph.text(
            pos2(r.right() - 16.0, fy),
            Align2::RIGHT_CENTER,
            if last { "Espacio: terminar  ▸" } else { "Espacio: seguir  ▸" },
            bold(11.5),
            accent,
        );
    }
    // un puntito que une el cartel con lo iluminado
    if focused {
        let from = if r.top() > spot.bottom() { pos2(r.center().x.clamp(spot.left(), spot.right()), r.top()) } else { pos2(r.center().x.clamp(spot.left(), spot.right()), r.bottom()) };
        p.circle_filled(from, 3.0, accent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maquina_de_escribir() {
        let t = "Hola, che. Listo";
        assert_eq!(revealed(t, 0.0), 1);
        assert_eq!(revealed(t, 100.0), t.chars().count());
        // la coma y el punto frenan
        let normal = revealed("abcdefghij", 0.1);
        let with_stop = revealed("a.bcdefghij", 0.1);
        assert!(with_stop < normal);
        assert!(total_time(t) > 0.0 && revealed(t, total_time(t)) == t.chars().count());
    }

    #[test]
    fn textos() {
        for s in STEPS.iter() {
            assert!(!s.text.contains("  "), "doble espacio en {}", s.title);
        }
        for s in INTRO.iter() {
            assert!(!s.contains("  "));
        }
    }
}

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
//!   4. "¡Bienvenido a la manada!" y todo vuelve a la normalidad.

use std::path::PathBuf;

use eframe::egui::{
    self, epaint::RectShape, pos2, text::LayoutJob, vec2, Align, Align2, Color32, CursorIcon, Id, Key, Modifiers,
    Rect, Sense, Stroke, StrokeKind, TextureOptions, Ui, Vec2,
};

use super::{animate_button, decode_png, ease_in_out, ease_out, CapyroApp};
use crate::ffi::State;
use crate::theme::*;

/// Música del tutorial (ya con el volumen al 25 %).
const MUSIC: &[u8] = include_bytes!("../../../assets/sounds/tutorial.opus");
const AVATAR_PNG: &[u8] = include_bytes!("../../../assets/arturo.png");
const AUTHOR: &str = "Arturo";
const GITHUB_USER: &str = "CapyIsInsano";
const GITHUB_URL: &str = "https://github.com/CapyIsInsano";

const INTRO: [&str; 4] = [
    "¡Hola! Bienvenido a Capyro.",
    "Capyro es un reproductor de audio y video para Linux: liviano, rápido y con un capibara de mascota. \
     Su núcleo está escrito en C, la interfaz en Rust, y todo se configura con Lua.",
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
               milisegundos del segundo actual. Las dos burbujas se ocultan en Ajustes.",
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
        text: "La versión, los formatos con un resumen de cada uno, y el botón para buscar actualizaciones: te \
               muestra qué commits hay de nuevo y, si querés, se baja, se compila y se instala solo.",
    },
    Step {
        target: Some("changes"),
        title: "Novedades",
        text: "Lo nuevo de la versión que tenés. El puntito naranja aparece cuando hay algo que todavía no viste.",
    },
    Step {
        target: None,
        title: "Formatos",
        text: "Video: MP4, MOV y AVI. Audio: MP3, WAV, OGG y AAC. Los videos se decodifican por GPU cuando se \
               puede. Si abrís un formato que no está en la lista, Capyro te avisa con un sonido.",
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
    Welcome,
}

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
    /// Estaba sonando algo cuando empezó: se retoma al final.
    resume: bool,
    /// Veces que se contestó "No".
    rounds: u32,
}

fn marker_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("capyro").join("tutorial-visto"))
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
        self.revealed_all = false;
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
        let resume = self.core.state() == State::Playing;
        if resume {
            self.core.toggle(); // pausa mientras tanto
        }
        if self.avatar.is_none() {
            self.avatar = decode_png(AVATAR_PNG).map(|(w, h, rgba)| {
                let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                ctx.load_texture("capyro-avatar", img, TextureOptions::LINEAR)
            });
        }
        if let Err(e) = self.core.music_play(MUSIC, true, 1800) {
            eprintln!("capyro: no pude reproducir la música del tutorial: {}", e.0);
        }
        let center = Rect::from_center_size(ctx.content_rect().center(), Vec2::ZERO);
        self.tutorial = Some(Tutorial {
            phase: Phase::Intro(0),
            t0: now + 0.6, // el primer texto arranca cuando ya se ve la pantalla
            start: now,
            revealed_all: false,
            spot_from: center,
            spot_to: center,
            spot_t0: now,
            closing: None,
            resume,
            rounds: 0,
        });
    }

    fn finish_tutorial(&mut self, now: f64, music_fade: i32) {
        if let Some(t) = &mut self.tutorial {
            if t.closing.is_none() {
                t.closing = Some(now);
                self.core.music_stop(music_fade);
                mark_seen();
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
        let accent = self.cfg.accent;

        // ¿terminó de cerrarse?
        if let Some(c) = t.closing {
            if now - c > 0.6 {
                let resume = t.resume;
                self.tutorial = None;
                if resume && self.core.state() != State::Playing {
                    self.core.toggle();
                }
                return;
            }
        }
        let out = t.closing.map_or(1.0, |c| 1.0 - ease_out(((now - c) / 0.6) as f32));
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
        // bienvenida: a los 3.6 s se va sola
        if phase == Phase::Welcome && now - phase_t0 > 3.6 {
            self.finish_tutorial(now, 1500);
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
        let mut answer: Option<bool> = None;

        match t.phase {
            Phase::Intro(i) => {
                let a = ease_out(((now - t.start) / 0.6) as f32);
                self.draw_intro(ui, &p, full, i, a, now, &mut link_clicked);
            }
            Phase::Reveal => {
                let k = ease_in_out(((now - t.t0) / 0.8) as f32);
                // la presentación se va y la interfaz queda atenuada
                let mut pi = p.clone();
                pi.multiply_opacity(1.0 - k);
                let last = INTRO.len() - 1;
                self.draw_intro(ui, &pi, full, last, 1.0, now, &mut link_clicked);
                p.rect_filled(full, WINDOW_RADIUS, Color32::from_black_alpha((150.0 * k) as u8));
            }
            Phase::Tour(i) => {
                let k = ease_in_out(((now - t.spot_t0) / 0.42) as f32);
                let spot = lerp_rect(t.spot_from, t.spot_to, k);
                draw_spotlight(&p, full, spot, accent, now, STEPS[i].target.is_some());
                let shown = if t.revealed_all { usize::MAX } else { revealed(STEPS[i].text, (now - t.t0) as f32) };
                let complete = t.complete(now);
                let enter = ease_out(((now - t.t0) / 0.3) as f32);
                draw_bubble(&p, full, spot, &STEPS[i], i, shown, complete, enter, accent, now, t.rounds);
            }
            Phase::Ask => {
                p.rect_filled(full, WINDOW_RADIUS, Color32::from_black_alpha(165));
                answer = self.draw_ask(ui, &p, full, now - t.t0, accent);
            }
            Phase::Welcome => {
                p.rect_filled(full, WINDOW_RADIUS, Color32::from_black_alpha(165));
                self.draw_welcome(&p, full, (now - t.t0) as f32, accent);
            }
        }

        if link_clicked {
            if let Err(e) = super::changes_menu::open_url(GITHUB_URL) {
                self.toast(&ctx, format!("No pude abrir el navegador: {e}"), true);
            }
        } else if let Some(yes) = answer {
            let t = self.tutorial.as_mut().expect("tutorial");
            if yes {
                t.go(Phase::Welcome, now);
                self.play_capybara();
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
            self.finish_tutorial(now, 800);
        } else if advance && !matches!(phase, Phase::Ask | Phase::Welcome) {
            self.tutorial_next(now);
        }
        ctx.request_repaint();
    }

    fn draw_intro(
        &self,
        ui: &Ui,
        p: &egui::Painter,
        full: Rect,
        para: usize,
        appear: f32,
        now: f64,
        link_clicked: &mut bool,
    ) {
        let Some(t) = &self.tutorial else { return };
        let accent = self.cfg.accent;
        let mut p = p.clone();
        p.multiply_opacity(appear);
        p.rect_filled(full, WINDOW_RADIUS, BG_WINDOW);
        // brillo naranja arriba
        for i in 0..28 {
            let r = 270.0 - 9.0 * i as f32;
            p.circle_filled(pos2(full.center().x, full.top() + full.height() * 0.2), r, accent.gamma_multiply(0.0045));
        }

        // logo, nombre y lema
        let since = (now - t.start) as f32;
        let bob = (since * 1.6).sin() * 3.0;
        let logo_c = pos2(full.center().x, full.top() + full.height() * 0.2 + bob);
        if let Some(logo) = &self.logo {
            let side = 92.0 * (0.92 + 0.08 * appear);
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
        let shown: String = text.chars().take(n).collect();
        let g = p.layout_job(centered_job(&shown, font, TEXT, width));
        let top = full.top() + full.height() * 0.47;
        let fade = ease_out(((now - t.t0).max(0.0) / 0.3) as f32);
        let mut pt = p.clone();
        pt.multiply_opacity(fade);
        pt.galley(pos2(full.center().x, top), g, TEXT);

        // puntitos de progreso y "Espacio para continuar"
        let below = top + full_g.size().y + 26.0;
        let dots = INTRO.len();
        for d in 0..dots {
            let x = full.center().x + (d as f32 - (dots - 1) as f32 / 2.0) * 14.0;
            let col = if d <= para { accent } else { Color32::from_white_alpha(40) };
            p.circle_filled(pos2(x, below), if d == para { 3.5 } else { 2.5 }, col);
        }
        if t.complete(now) && matches!(t.phase, Phase::Intro(_)) {
            let pulse = 0.55 + 0.45 * ((now * 3.0).sin() as f32).abs();
            let mut ph = p.clone();
            ph.multiply_opacity(pulse);
            ph.text(pos2(full.center().x, below + 26.0), Align2::CENTER_CENTER, "Espacio para continuar  ▸", regular(12.5), TEXT_DIM);
        }

        // abajo: el perfil de GitHub del autor
        let card = Rect::from_center_size(pos2(full.center().x, full.bottom() - 62.0), vec2(250.0, 58.0));
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
        let size = vec2(360.0, 200.0) * (0.92 + 0.08 * k);
        let card = Rect::from_center_size(full.center(), size);
        let mut p = p.clone();
        p.multiply_opacity(k);
        p.rect_filled(card.translate(vec2(0.0, 6.0)), 18.0, Color32::from_black_alpha(90));
        p.rect_filled(card, 18.0, BG_PANEL);
        p.rect_stroke(card, 18.0, Stroke::new(1.0, accent.gamma_multiply(0.5)), StrokeKind::Inside);
        if let Some(logo) = &self.logo {
            p.image(logo.id(), Rect::from_center_size(pos2(card.center().x, card.top() + 40.0), Vec2::splat(46.0)), uv(), Color32::WHITE);
        }
        p.text(pos2(card.center().x, card.top() + 90.0), Align2::CENTER_CENTER, "¿Estás listo?", bold(22.0), TEXT);
        if k < 0.9 {
            return None;
        }
        let by = card.bottom() - 42.0;
        let mut out = None;
        for (yes, label, x) in [(true, "Sí", card.center().x - 66.0), (false, "No", card.center().x + 66.0)] {
            let r = Rect::from_center_size(pos2(x, by), vec2(116.0, 40.0));
            let a = animate_button(ui, Id::unique(("capyro-tutorial-ask", yes)), r, true);
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 + 0.04 * a.hover - 0.08 * a.press));
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
        // papelitos que caen
        let mut seed: u32 = 0x9E37_79B9;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % 10_000) as f32 / 10_000.0
        };
        for i in 0..60 {
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
            p.rect_filled(Rect::from_center_size(pos2(x0 + sway, y), vec2(s, s * 1.6)), 1.0, col);
        }
        // logo que salta y el título
        let pop = ease_out(since / 0.45);
        let bounce = (-since * 3.0).exp() * (since * 10.0).sin().abs() * 26.0;
        if let Some(logo) = &self.logo {
            let side = 110.0 * pop;
            p.image(logo.id(), Rect::from_center_size(c - vec2(0.0, 70.0 + bounce), Vec2::splat(side)), uv(), Color32::WHITE);
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
    let r = Rect::from_min_size(pos2(x, y + 8.0 * (1.0 - enter)), vec2(w, h));

    let mut p = p.clone();
    p.multiply_opacity(enter);
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
    let text: String = step.text.chars().take(shown).collect();
    let g = p.layout(text, regular(13.0), TEXT, inner);
    let text_top = ty + 9.0 + 10.0;
    p.galley(pos2(x0, text_top), g, TEXT);

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

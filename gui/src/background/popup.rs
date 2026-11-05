//! El mini reproductor del segundo plano: una ventanita al lado del ícono
//! de la bandeja con lo que suena (el disco de vinilo si está activado, si
//! no la portada), el visualizador a un costado, cuánto va y cuánto falta,
//! pausa, y los botones Abrir y Cerrar.
//!
//! Se va sola al tocar afuera (cuando pierde el foco), con Esc o con otro
//! clic en el ícono.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, Id, Key, epaint::RectShape, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions,
    Ui, Vec2, ViewportCommand,
};

use super::Event;
use crate::app::widgets::{animate_button, fit_text, fmt_time, pill, pill_w};
use crate::ffi::{Core, MediaKind, State};
use crate::system::config::{Config, DiscViz, VizMode};
use crate::ui::icons;
use crate::ui::theme::{self, *};
use crate::ui::viz::Visualizer;

/// Tamaño del mini reproductor (puntos).
pub const W: f32 = 460.0;
pub const H: f32 = 172.0;
const PAD: f32 = 14.0;
/// Largo de las barras del visualizador (a la derecha de la portada).
const REACH: f32 = 40.0;

/// Cómo se cerró.
pub enum Out {
    /// Se fue (clic afuera, Esc, otro clic en el ícono): sigue en segundo plano.
    Back,
    /// "Abrir": vuelve la ventana (con estos archivos, si llegaron).
    Open(Vec<PathBuf>),
    /// "Cerrar": cierra Capyro del todo.
    Quit,
}

/// Lo que sigue entre una apertura y otra: el giro del disco y cuándo se
/// cerró por tocar afuera.
#[derive(Default)]
pub struct Keep {
    angle: f32,
    vel: f32,
    blurred_at: Option<Instant>,
}

impl Keep {
    /// ¿Este clic en el ícono (hecho en `at`) es el que lo cerró? Tocar el
    /// ícono con el mini reproductor abierto primero le saca el foco (y se
    /// cierra) y después llega el clic: ese clic no tiene que volver a abrirlo.
    pub fn closed_by(&self, at: Instant) -> bool {
        self.blurred_at.is_some_and(|b| at.saturating_duration_since(b) < Duration::from_millis(600))
    }
}

/// Dónde va la ventanita: centrada sobre el clic, arriba del ícono si la
/// bandeja está abajo (Windows, la mayoría) o abajo si está arriba.
pub(crate) fn place(click: (i32, i32), scale: f32, monitor: Option<Vec2>) -> Option<Pos2> {
    let (cx, cy) = (click.0 as f32 / scale, click.1 as f32 / scale);
    match monitor {
        // algunas bandejas no dicen dónde (0, 0): abajo a la derecha
        Some(m) if click == (0, 0) => Some(pos2(m.x - W - 16.0, m.y - H - 56.0)),
        None if click == (0, 0) => None,
        Some(m) if cx <= m.x && cy <= m.y => {
            let y = if cy < m.y / 2.0 { cy + 14.0 } else { cy - H - 14.0 };
            Some(pos2((cx - W / 2.0).clamp(8.0, (m.x - W - 8.0).max(8.0)), y.clamp(8.0, (m.y - H - 8.0).max(8.0))))
        }
        // otro monitor (u otro tamaño): al lado del clic, sin pasarse de 0
        _ => Some(pos2((cx - W / 2.0).max(0.0), (cy - H - 14.0).max(0.0))),
    }
}

struct Mini<'a> {
    core: &'a Core,
    cfg: &'a Config,
    rx: &'a Receiver<Event>,
    out: &'a RefCell<Option<Out>>,
    keep: &'a mut Keep,
    click: (i32, i32),
    placed: bool,
    focused_once: bool,
    frames: u64,
    viz: Visualizer,
    last_time: Option<f64>,
    cover: Option<(TextureHandle, f32)>,
    cover_serial: u64,
    /// Arrastrando la barra: hasta dónde (0..1).
    seek_drag: Option<f32>,
}

impl Mini<'_> {
    fn finish(&mut self, ctx: &egui::Context, out: Out) {
        if self.out.borrow().is_none() {
            *self.out.borrow_mut() = Some(out);
        }
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    /// La portada (con un audio): la da el núcleo como "cuadro" y se copia
    /// solo cuando cambia.
    fn sync_cover(&mut self, ctx: &egui::Context) {
        if self.core.media_kind() != MediaKind::Audio {
            self.cover = None;
            self.cover_serial = u64::MAX;
            return;
        }
        let known = self.cover_serial;
        let fresh = self.core.with_video_frame(|f| {
            (f.serial != known).then(|| {
                let px: Vec<Color32> = bytemuck::cast_slice::<u8, Color32>(f.rgba).to_vec();
                (egui::ColorImage::new([f.width, f.height], px), f.serial, f.aspect)
            })
        });
        match fresh {
            None => {
                self.cover = None;
                self.cover_serial = u64::MAX;
            }
            Some(None) => {}
            Some(Some((img, serial, aspect))) => {
                self.cover = Some((ctx.load_texture("capyro-mini-cover", img, TextureOptions::LINEAR), aspect));
                self.cover_serial = serial;
            }
        }
    }

    fn viz_on(&self) -> bool {
        if self.cfg.vinyl {
            self.cfg.disc_viz() != DiscViz::Off
        } else {
            self.cfg.viz_mode() != VizMode::Off
        }
    }

    fn art(&self, painter: &egui::Painter, rect: Rect, kind: MediaKind) {
        let accent = self.cfg.accent;
        let anim = crate::system::perf::animations();
        if self.cfg.vinyl && kind == MediaKind::Audio {
            let label = match &self.cover {
                Some((tex, aspect)) => crate::ui::vinyl::Label::Cover(tex.id(), *aspect),
                None => crate::ui::vinyl::Label::Plain,
            };
            if anim {
                soft_shadow(painter, rect, rect.width() / 2.0, 12.0, 4.0, 90);
            }
            crate::ui::vinyl::draw(painter, rect.center(), rect.width() / 2.0, self.keep.angle, &label, accent, 1.0, anim);
            return;
        }
        let radius = 10.0;
        if anim {
            soft_shadow(painter, rect, radius, 12.0, 4.0, 90);
        }
        match (&self.cover, kind) {
            (Some((tex, aspect)), MediaKind::Audio) => {
                if (aspect - 1.0).abs() > 0.02 {
                    painter.rect_filled(rect, radius, BG_VIDEO);
                }
                let size = if *aspect >= 1.0 {
                    vec2(rect.width(), rect.width() / aspect)
                } else {
                    vec2(rect.height() * aspect, rect.height())
                };
                painter.add(
                    RectShape::filled(Rect::from_center_size(rect.center(), size), radius, Color32::WHITE)
                        .with_texture(tex.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))),
                );
            }
            _ => {
                painter.rect_filled(rect, radius, BG_PANEL);
                painter.rect_filled(rect, radius, accent.gamma_multiply(0.10));
                painter.rect_stroke(rect, radius, Stroke::new(1.0, accent.gamma_multiply(0.25)), StrokeKind::Inside);
                painter.circle_filled(rect.center(), rect.width() * 0.22, accent.gamma_multiply(0.16));
                if kind == MediaKind::Video {
                    icons::film(painter, rect.center(), rect.width() * 0.26, accent);
                } else {
                    icons::music_note(painter, rect.center(), rect.width() * 0.24, accent);
                }
            }
        }
    }

    /// Barra de progreso: tocar o arrastrar para ir a otra parte.
    fn progress(&mut self, ui: &Ui, rect: Rect, pos: f64, dur: f64) -> f64 {
        let accent = self.cfg.accent;
        let painter = ui.painter();
        let hit = rect.expand2(vec2(0.0, 7.0));
        let resp = ui.interact(hit, Id::unique("capyro-mini-seek"), if dur > 0.0 { Sense::click_and_drag() } else { Sense::hover() });
        let frac_at = |p: Pos2| ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.drag_started()) {
            self.seek_drag = Some(frac_at(p));
        }
        if resp.drag_stopped() || resp.clicked() {
            let f = resp.interact_pointer_pos().map(frac_at).or(self.seek_drag);
            if let Some(f) = f {
                self.core.seek(f as f64 * dur);
            }
            self.seek_drag = None;
        }
        let shown = match self.seek_drag {
            Some(f) => f as f64 * dur,
            None => pos,
        };
        let frac = if dur > 0.0 { (shown / dur).clamp(0.0, 1.0) as f32 } else { 0.0 };
        let hot = resp.hovered() || self.seek_drag.is_some();
        if hot && dur > 0.0 {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let h = if hot { 6.0 } else { 4.0 };
        let bar = Rect::from_center_size(rect.center(), vec2(rect.width(), h));
        painter.rect_filled(bar, h / 2.0, TRACK);
        let fill = Rect::from_min_max(bar.min, pos2(bar.left() + bar.width() * frac, bar.bottom()));
        painter.rect_filled(fill, h / 2.0, accent);
        if hot {
            painter.circle_filled(pos2(fill.right(), bar.center().y), 6.0, Color32::WHITE);
        }
        shown
    }
}

impl eframe::App for Mini<'_> {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if cfg!(windows) {
            theme::BG_WINDOW.to_normalized_gamma_f32()
        } else {
            [0.0, 0.0, 0.0, 0.0]
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let dt = self.last_time.map_or(0.0, |t| ((now - t) as f32).clamp(0.0, 0.1));
        self.last_time = Some(now);
        self.frames += 1;
        let core = self.core;
        core.update();

        // lo que llega de la bandeja mientras está abierta
        loop {
            match self.rx.try_recv() {
                Ok(Event::Popup(..)) => self.finish(&ctx, Out::Back), // otro clic en el ícono: se va
                Ok(Event::Toggle) => core.toggle(),
                Ok(Event::Open(files)) => self.finish(&ctx, Out::Open(files)),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.finish(&ctx, Out::Back);
                    break;
                }
            }
        }
        // Mac: archivos abiertos desde el Finder → vuelve la ventana con ellos
        #[cfg(target_os = "macos")]
        {
            let files = crate::system::macos::take_opened_files();
            if !files.is_empty() {
                self.finish(&ctx, Out::Open(files));
            }
        }

        // primer cuadro: al lado del ícono, con el tamaño de pantalla real
        let (focused, monitor, ppp) = ctx.input(|i| (i.viewport().focused, i.viewport().monitor_size, i.viewport().native_pixels_per_point));
        if !self.placed && monitor.is_some() {
            self.placed = true;
            super::remember_monitor(monitor, ppp);
            if let Some(p) = place(self.click, ppp.unwrap_or(1.0), monitor) {
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(p));
            }
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }
        // tocar afuera la cierra (cuando ya tuvo el foco alguna vez)
        match focused {
            Some(true) => self.focused_once = true,
            Some(false) if self.focused_once && self.frames > 2 && self.out.borrow().is_none() => {
                self.keep.blurred_at = Some(Instant::now());
                self.finish(&ctx, Out::Back);
            }
            _ => {}
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.finish(&ctx, Out::Back);
        }

        self.sync_cover(&ctx);
        let kind = core.media_kind();
        let state = core.state();
        let playing = state == State::Playing;
        let anim = crate::system::perf::animations();

        // el disco gira con la velocidad (en Bajo, quieto)
        if self.cfg.vinyl && kind == MediaKind::Audio {
            let target = if playing && anim { crate::ui::vinyl::RPS * core.speed() as f32 } else { 0.0 };
            crate::ui::vinyl::step(&mut self.keep.angle, &mut self.keep.vel, target, dt);
        }
        let viz = self.viz_on() && kind != MediaKind::None;
        if viz {
            let n = self.viz.len();
            let got = if playing { core.scope(&mut self.viz.samples[..n]) } else { 0 };
            self.viz.update(core.audio_rate() as f32, dt, playing && got == n);
        }

        /* --- dibujo --- */
        let full = ui.max_rect();
        let painter = ui.painter().clone();
        let radius = if cfg!(windows) { 0.0 } else { 14.0 };
        painter.rect_filled(full, radius, BG_WINDOW);
        painter.rect_stroke(full, radius, Stroke::new(1.0, Color32::from_white_alpha(18)), StrokeKind::Inside);
        let accent = self.cfg.accent;

        let side = H - PAD * 2.0;
        let art = Rect::from_min_size(pos2(full.left() + PAD, full.top() + PAD), vec2(side, side));
        self.art(&painter, art, kind);
        let info_left = if viz {
            self.viz.draw_side(&painter, art, REACH, 7.0, accent);
            art.right() + 7.0 + REACH + 12.0
        } else {
            art.right() + 18.0
        };
        let info = Rect::from_min_max(pos2(info_left, art.top()), pos2(full.right() - PAD, art.bottom()));
        let w = info.width();

        // arriba: de dónde viene
        painter.text(info.left_top() + vec2(0.0, 2.0), Align2::LEFT_TOP, "Capyro · segundo plano", regular(10.5), TEXT_FAINT);

        // título y artista
        let file = core.current_file();
        let (title, sub) = if file.is_empty() {
            ("Nada sonando".to_owned(), "Abrí Capyro para elegir qué escuchar".to_owned())
        } else {
            let t = core.title();
            let title = if t.trim().is_empty() { crate::app::widgets::file_name(&file) } else { t };
            let artist = core.artist();
            let sub = match kind {
                MediaKind::Video => "Video · suena solo el audio".to_owned(),
                _ if !artist.trim().is_empty() => artist,
                _ => core.album(),
            };
            (title, sub)
        };
        painter.text(info.left_top() + vec2(0.0, 22.0), Align2::LEFT_TOP, fit_text(&painter, &title, bold(15.0), w), bold(15.0), TEXT);
        if !sub.trim().is_empty() {
            painter.text(info.left_top() + vec2(0.0, 43.0), Align2::LEFT_TOP, fit_text(&painter, &sub, regular(12.0), w), regular(12.0), TEXT_DIM);
        }

        // progreso: cuánto va y cuánto falta
        let dur = core.duration();
        let bar = Rect::from_min_size(info.left_top() + vec2(0.0, 72.0), vec2(w, 4.0));
        let pos = self.progress(ui, bar, core.position(), dur);
        let times_y = bar.bottom() + 12.0;
        painter.text(pos2(info.left(), times_y), Align2::LEFT_CENTER, fmt_time(pos), regular(11.0), TEXT_DIM);
        let left = if dur > 0.0 { format!("-{}", fmt_time((dur - pos).max(0.0))) } else { "--:--".to_owned() };
        painter.text(pos2(info.right(), times_y), Align2::RIGHT_CENTER, left, regular(11.0), TEXT_DIM);

        // abajo: pausa · Abrir · Cerrar
        let row = info.bottom() - 15.0;
        let play_c = pos2(info.left() + 16.0, row);
        let pa = animate_button(ui, Id::unique("capyro-mini-play"), Rect::from_center_size(play_c, Vec2::splat(32.0)), !file.is_empty());
        let r = 16.0 * (1.0 + 0.06 * pa.hover - 0.08 * pa.press);
        painter.circle_filled(play_c, r, if file.is_empty() { Color32::from_white_alpha(14) } else { accent.lerp_to_gamma(Color32::WHITE, 0.12 * pa.hover) });
        play_glyph(&painter, play_c, r, playing);
        if pa.resp.clicked() {
            core.toggle();
        }
        let close_w = pill_w(ui, "Cerrar");
        let open_w = pill_w(ui, "Abrir").max(close_w);
        let close_r = Rect::from_min_max(pos2(info.right() - close_w, row - 14.0), pos2(info.right(), row + 14.0));
        let open_r = Rect::from_min_max(pos2(close_r.left() - 8.0 - open_w, row - 14.0), pos2(close_r.left() - 8.0, row + 14.0));
        if pill(ui, "mini-open", open_r, "Abrir", true, 0.0, accent, true) {
            self.finish(&ctx, Out::Open(Vec::new()));
        }
        if pill(ui, "mini-quit", close_r, "Cerrar", false, 0.0, accent, true) {
            self.finish(&ctx, Out::Quit);
        }

        // se sigue dibujando para el disco, las barras y el tiempo; y para
        // ver si llegó algo de la bandeja
        let moving = anim && (playing || self.keep.vel != 0.0 || (viz && !self.viz.is_silent()));
        ctx.request_repaint_after(if moving { Duration::from_millis(33) } else { Duration::from_millis(200) });
    }
}

/// Triángulo o pausa en blanco, adentro del círculo del botón.
fn play_glyph(painter: &egui::Painter, c: Pos2, r: f32, playing: bool) {
    let s = r * 0.42;
    if playing {
        for dx in [-s * 0.45, s * 0.45] {
            painter.rect_filled(Rect::from_center_size(c + vec2(dx, 0.0), vec2(s * 0.42, s * 1.5)), 1.5, Color32::WHITE);
        }
    } else {
        painter.add(egui::Shape::convex_polygon(
            vec![c + vec2(-s * 0.55, -s * 0.85), c + vec2(s * 0.95, 0.0), c + vec2(-s * 0.55, s * 0.85)],
            Color32::WHITE,
            Stroke::NONE,
        ));
    }
}

/// Abre el mini reproductor y espera a que se cierre.
pub fn show(core: &Core, cfg: &Config, renderer: eframe::Renderer, rx: &Receiver<Event>, click: (i32, i32), keep: &mut Keep) -> Out {
    // si ya está sonando, el disco ya gira (no arranca de cero cada vez)
    if core.state() == State::Playing && crate::system::perf::animations() {
        keep.vel = crate::ui::vinyl::RPS * core.speed() as f32;
    }
    keep.blurred_at = None;
    // Mac: sin ícono en el Dock Capyro no está "al frente": que lo pase, así
    // el mini reproductor recibe el foco (y se cierra al tocar afuera)
    #[cfg(target_os = "macos")]
    crate::system::macos::activate();
    let out = RefCell::new(None);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Capyro")
        .with_app_id("Capyro")
        .with_inner_size([W, H])
        .with_resizable(false)
        .with_decorations(false)
        .with_transparent(!cfg!(windows))
        .with_always_on_top()
        .with_taskbar(false)
        .with_window_type(egui::X11WindowType::Utility)
        .with_active(true);
    let start = place(click, super::scale(), super::monitor());
    if let Some(p) = start {
        viewport = viewport.with_position(p);
    }
    if let Some(icon) = crate::app::window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let options = eframe::NativeOptions {
        viewport,
        centered: start.is_none(),
        renderer,
        ..Default::default()
    };
    let result = eframe::run_native(
        "Capyro",
        options,
        Box::new(|cc| {
            theme::install_fonts(&cc.egui_ctx);
            theme::apply_visuals(&cc.egui_ctx, cfg.accent);
            Ok(Box::new(Mini {
                core,
                cfg,
                rx,
                out: &out,
                keep,
                click,
                placed: false,
                focused_once: false,
                frames: 0,
                viz: Visualizer::new(),
                last_time: None,
                cover: None,
                cover_serial: u64::MAX,
                seek_drag: None,
            }))
        }),
    );
    if let Err(e) = result {
        crate::system::errlog::log(&format!("no pude abrir el mini reproductor: {e}"));
        // sin mini reproductor: que vuelva la ventana (que sí anduvo)
        return Out::Open(Vec::new());
    }
    out.into_inner().unwrap_or(Out::Back)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn se_ubica_al_lado_del_icono() {
        let m = Some(vec2(1920.0, 1080.0));
        // bandeja abajo a la derecha (Windows, KDE): arriba del ícono, sin salirse
        let p = place((1900, 1070), 1.0, m).unwrap();
        assert!(p.x + W <= 1920.0 - 8.0 && p.y + H < 1070.0, "{p:?}");
        // bandeja arriba (GNOME, Xfce): abajo del ícono
        let p = place((960, 12), 1.0, m).unwrap();
        assert!(p.y > 12.0 && (p.x + W / 2.0 - 960.0).abs() < 1.0, "{p:?}");
        // pantalla con escala 2: las coordenadas vienen en píxeles
        let p = place((3800, 2140), 2.0, m).unwrap();
        assert!(p.x + W <= 1912.0 && p.y + H <= 1070.0 - 14.0 + 0.5, "{p:?}");
        // sin posición: abajo a la derecha (o centrada si no se sabe la pantalla)
        assert_eq!(place((0, 0), 1.0, m), Some(pos2(1920.0 - W - 16.0, 1080.0 - H - 56.0)));
        assert_eq!(place((0, 0), 1.0, None), None);
    }
}

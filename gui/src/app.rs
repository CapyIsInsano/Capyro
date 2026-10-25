//! La ventana de Capyro. Layout medido sobre el mockup (768×642):
//!
//!   ┌ header 46px ─ logo · Capyro / Just press play ─────── _ □ ✕ ┐
//!   │ área de video (flexible)                                     │
//!   │ ── barra de progreso ──────────────────────────────────────  │
//!   │ 00:00                                                  00:00 │
//!   │              ⏮   ⟲   (▶)   ⏭                                │
//!   └ ☰ Lista                                       🔊  cc  ⚙  ⛶  ┘

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, Id, Key, Modifiers, Pos2, Rect, Response,
    ResizeDirection, ScrollArea, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui,
    UiBuilder, Vec2, ViewportCommand,
};

use crate::config::{Action, Config};
use crate::ffi::{Core, MediaKind, Repeat, State};
use crate::icons;
use crate::theme::{self, *};

const LOGO_PNG: &[u8] = include_bytes!("../../assets/capyro_logo_128.png");
const ICON_PNG: &[u8] = include_bytes!("../../assets/capyro_logo_256.png");

/// Distancias desde el borde inferior del área de video (según el mockup).
const TRACK_Y: f32 = 14.0;
const TIMES_Y: f32 = 34.0;
const CONTROLS_Y: f32 = 81.0;
const BOTTOM_ROW_Y: f32 = 129.0;
const BELOW_VIDEO: f32 = 142.0;

struct Toast {
    text: String,
    start: f64,
    until: f64,
    error: bool,
}

/// Velocidades que ofrece el menú de ajustes (y entre las que saltan los atajos).
const SPEEDS: [f64; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

fn fmt_speed(s: f64) -> String {
    let t = format!("{s:.2}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    format!("{t}×")
}

/// Respuesta de un botón + sus animaciones (0..1, suavizadas en el tiempo).
struct Anim {
    resp: Response,
    hover: f32,
    press: f32,
}

impl Anim {
    /// Escala para el ícono: crece un poco al pasar el mouse, se hunde al clic.
    fn scale(&self) -> f32 {
        1.0 + 0.08 * self.hover - 0.14 * self.press
    }
    fn tint(&self, base: Color32, hot: Color32) -> Color32 {
        base.lerp_to_gamma(hot, self.hover)
    }
}

fn animate_button(ui: &Ui, id: Id, rect: Rect, enabled: bool) -> Anim {
    let resp = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
    let ctx = ui.ctx();
    let hovered = enabled && resp.hovered();
    let hover = ctx.animate_bool_with_time(id.with("hover"), hovered, 0.14);
    let press = ctx.animate_bool_with_time(id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.07);
    if hovered {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    Anim { resp, hover, press }
}

enum PlaylistOp {
    Select(usize),
    Remove(usize),
    Clear,
}

pub struct CapyroApp {
    core: Core,
    cfg: Config,
    logo: Option<TextureHandle>,
    show_playlist: bool,
    toast: Option<Toast>,
    last_title: String,
    /// Textura del frame actual (video o carátula) y su número de serie.
    video_tex: Option<TextureHandle>,
    video_serial: u64,
    video_aspect: f32,
    /// Toast que hay que mostrar en el primer frame (todavía no hay `ctx.time`).
    startup_notice: Option<String>,
    /// Diálogo de "Agregar archivos" abierto en otro hilo; llega el resultado acá.
    dialog: Option<Receiver<Vec<PathBuf>>>,
    /// Menú del engranaje abierto, y dónde está el engranaje (para anclarlo).
    show_settings: bool,
    gear_rect: Rect,
}

fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((w, h, img.into_raw()))
}

pub fn window_icon() -> Option<egui::IconData> {
    let (width, height, rgba) = decode_png(ICON_PNG)?;
    Some(egui::IconData { rgba, width, height })
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}

fn ellipsize(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn fmt_time(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

/// Ícono de recargar girado `angle` radianes (rotamos los puntos del ícono).
fn draw_rotated_reload(painter: &egui::Painter, c: Pos2, s: f32, col: Color32, angle: f32) {
    let (sin, cos) = angle.sin_cos();
    let rot = |p: Pos2| {
        pos2(
            c.x + (p.x - c.x) * cos - (p.y - c.y) * sin,
            c.y + (p.x - c.x) * sin + (p.y - c.y) * cos,
        )
    };
    let r = 0.32 * s;
    let (from, to) = (std::f32::consts::PI, -0.75 * std::f32::consts::PI);
    let n = 28;
    let arc: Vec<Pos2> = (0..=n)
        .map(|i| {
            let t = from + (to - from) * i as f32 / n as f32;
            rot(pos2(c.x + r * t.cos(), c.y + r * t.sin()))
        })
        .collect();
    let tip = pos2(c.x + r * to.cos(), c.y + r * to.sin());
    let h = 0.2 * s;
    let st = Stroke::new(1.6, col);
    painter.add(egui::Shape::line(arc, st));
    painter.add(egui::Shape::line(vec![rot(tip + vec2(0.0, -h)), rot(tip), rot(tip + vec2(h, 0.0))], st));
}

impl CapyroApp {
    pub fn new(cc: &eframe::CreationContext<'_>, core: Core, cfg: Config) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        theme::apply_visuals(&cc.egui_ctx, cfg.accent);

        let logo = decode_png(LOGO_PNG).map(|(w, h, rgba)| {
            let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            cc.egui_ctx.load_texture("capyro-logo", img, TextureOptions::LINEAR)
        });

        core.set_volume(cfg.start_volume);
        core.set_auto_next(cfg.auto_next);
        core.set_speed(cfg.start_speed);
        core.set_shuffle(cfg.start_shuffle);
        core.set_repeat(cfg.start_repeat);
        let startup_notice = core
            .audio_backend()
            .is_empty()
            .then(|| "No encontré dispositivo de audio: reproduzco sin sonido".to_owned());
        // Archivos pasados por línea de comandos: "just press play" → arrancar solo.
        if core.state() == State::Stopped {
            core.toggle();
        }

        Self {
            core,
            cfg,
            logo,
            show_playlist: false,
            toast: None,
            last_title: String::new(),
            video_tex: None,
            video_serial: 0,
            video_aspect: 16.0 / 9.0,
            startup_notice,
            dialog: None,
            show_settings: false,
            gear_rect: Rect::NOTHING,
        }
    }

    fn toast(&mut self, ctx: &egui::Context, text: impl Into<String>, error: bool) {
        let now = ctx.input(|i| i.time);
        self.toast = Some(Toast { text: text.into(), start: now, until: now + 2.5, error });
    }

    /* ---------------------------------------------------------------- */
    /* acciones                                                         */
    /* ---------------------------------------------------------------- */

    fn run_action(&mut self, ctx: &egui::Context, action: Action) {
        let loaded = self.core.state() != State::Idle;
        match action {
            Action::PlayPause => {
                if loaded {
                    self.core.toggle();
                } else {
                    self.toast(ctx, "Arrastra un archivo .mp4 o .mp3 para empezar", false);
                }
            }
            Action::Stop => self.core.stop(),
            Action::SeekForward => self.core.seek_relative(self.cfg.seek_step),
            Action::SeekBackward => self.core.seek_relative(-self.cfg.seek_step),
            Action::VolumeUp | Action::VolumeDown => {
                let step = if action == Action::VolumeUp { 1.0 } else { -1.0 };
                self.change_volume(ctx, step * self.cfg.volume_step);
            }
            Action::Mute => self.toggle_mute(ctx),
            Action::NextFile => {
                if self.core.next().is_err() {
                    let msg = if self.core.playlist_len() == 0 {
                        "La lista está vacía"
                    } else {
                        "Es el último de la lista (activa repetir para dar la vuelta)"
                    };
                    self.toast(ctx, msg, false);
                }
            }
            Action::PrevFile => {
                let _ = self.core.prev();
            }
            Action::TogglePlaylist => self.show_playlist = !self.show_playlist,
            Action::Fullscreen => {
                let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fs));
            }
            Action::OpenFiles => self.open_file_dialog(ctx),
            Action::ToggleShuffle => self.toggle_shuffle(ctx),
            Action::CycleRepeat => self.cycle_repeat(ctx),
            Action::SpeedUp | Action::SpeedDown => {
                let cur = self.core.speed();
                let next = if action == Action::SpeedUp {
                    SPEEDS.iter().copied().find(|&v| v > cur + 1e-6).unwrap_or(SPEEDS[SPEEDS.len() - 1])
                } else {
                    SPEEDS.iter().rev().copied().find(|&v| v < cur - 1e-6).unwrap_or(SPEEDS[0])
                };
                self.set_speed(ctx, next);
            }
            Action::SpeedReset => self.set_speed(ctx, 1.0),
            Action::Quit => ctx.send_viewport_cmd(ViewportCommand::Close),
        }
    }

    fn set_speed(&mut self, ctx: &egui::Context, speed: f64) {
        self.core.set_speed(speed);
        self.toast(ctx, format!("Velocidad {}", fmt_speed(self.core.speed())), false);
    }

    fn toggle_shuffle(&mut self, ctx: &egui::Context) {
        let on = !self.core.shuffle();
        self.core.set_shuffle(on);
        self.toast(ctx, if on { "Aleatorio activado" } else { "Aleatorio desactivado" }, false);
    }

    fn cycle_repeat(&mut self, ctx: &egui::Context) {
        let r = self.core.repeat().next();
        self.core.set_repeat(r);
        let msg = match r {
            Repeat::Off => "Sin repetir",
            Repeat::All => "Repetir lista",
            Repeat::One => "Repetir este archivo",
        };
        self.toast(ctx, msg, false);
    }

    fn change_volume(&mut self, ctx: &egui::Context, delta: f32) {
        let v = (self.core.volume() + delta).clamp(0.0, 1.0);
        self.core.set_volume(v);
        self.core.set_muted(false);
        self.toast(ctx, format!("Volumen {}%", (v * 100.0).round()), false);
    }

    fn toggle_mute(&mut self, ctx: &egui::Context) {
        let m = !self.core.muted();
        self.core.set_muted(m);
        self.toast(ctx, if m { "Silenciado" } else { "Sonido activado" }, false);
    }

    fn reload_config(&mut self, ctx: &egui::Context) {
        self.cfg = Config::load();
        theme::apply_visuals(ctx, self.cfg.accent);
        self.core.set_auto_next(self.cfg.auto_next);
        let src = self.cfg.source.clone();
        self.toast(ctx, format!("Config recargada ({src})"), false);
    }

    /// Agrega archivos a la lista. Si `play_now` (arrastrar y soltar) o si no
    /// había nada cargado, arranca el primero nuevo; si no, solo los encola.
    fn add_paths(&mut self, ctx: &egui::Context, paths: Vec<PathBuf>, play_now: bool) {
        let first_new = self.core.playlist_len();
        let was_idle = self.core.state() == State::Idle;
        let mut errors = Vec::new();
        for path in &paths {
            if let Err(e) = self.core.playlist_add(path) {
                errors.push(format!("{}: {}", file_name(&path.to_string_lossy()), e.0));
            }
        }
        let added = self.core.playlist_len() - first_new;
        if added > 0 && (play_now || was_idle) {
            match self.core.playlist_select(first_new) {
                Ok(()) => {
                    if self.core.state() != State::Playing {
                        self.core.toggle();
                    }
                }
                Err(e) => errors.push(e.0),
            }
        }
        if let Some(first) = errors.first() {
            let more = if errors.len() > 1 {
                format!(" (+{} más)", errors.len() - 1)
            } else {
                String::new()
            };
            self.toast(ctx, format!("{first}{more}"), true);
        } else if added > 0 && !(play_now || was_idle) {
            let msg = if added == 1 {
                "1 archivo agregado a la lista".to_owned()
            } else {
                format!("{added} archivos agregados a la lista")
            };
            self.toast(ctx, msg, false);
        }
    }

    /// Abre el selector de archivos del sistema (portal de escritorio, o
    /// zenity/kdialog si no hay portal) sin congelar la interfaz.
    fn open_file_dialog(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() {
            return; // ya hay uno abierto
        }
        // El diálogo se crea acá y se espera en otro hilo.
        let pending = rfd::AsyncFileDialog::new()
            .set_title("Agregar archivos a Capyro")
            .add_filter("Audio y video (mp4, mp3)", &["mp4", "mp3", "MP4", "MP3"])
            .pick_files();
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let files = pollster::block_on(pending)
                .unwrap_or_default()
                .into_iter()
                .map(|f| f.path().to_path_buf())
                .collect::<Vec<_>>();
            let _ = tx.send(files);
            ctx.request_repaint();
        });
        self.dialog = Some(rx);
    }

    fn poll_file_dialog(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.dialog else { return };
        match rx.try_recv() {
            Ok(files) => {
                self.dialog = None;
                if !files.is_empty() {
                    self.add_paths(ctx, files, false);
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.dialog = None;
                self.toast(ctx, "No se pudo abrir el selector de archivos", true);
            }
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // Archivos soltados sobre la ventana.
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        if !dropped.is_empty() {
            self.add_paths(ctx, dropped, true);
        }

        // Atajos definidos en capyro.lua (ya ordenados: más modificadores primero).
        let mut fired = Vec::new();
        ctx.input_mut(|i| {
            for b in &self.cfg.bindings {
                if i.consume_key(b.modifiers, b.key) {
                    fired.push(b.action);
                }
            }
        });
        // Escape siempre saca de pantalla completa.
        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        if fullscreen && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
        }
        for action in fired {
            self.run_action(ctx, action);
        }
    }

    /* ---------------------------------------------------------------- */
    /* piezas de la interfaz                                            */
    /* ---------------------------------------------------------------- */

    /// Sube a la GPU el frame nuevo del núcleo (si cambió desde el último).
    fn sync_video_texture(&mut self, ctx: &egui::Context) {
        let last = self.video_serial;
        let fresh = self.core.with_video_frame(|vf| {
            if vf.serial == last {
                return None;
            }
            let img = egui::ColorImage::from_rgba_unmultiplied([vf.width, vf.height], vf.rgba);
            Some((img, vf.serial, vf.aspect))
        });
        match fresh {
            None => self.video_tex = None, // no hay frame (nada cargado o mp3 sin carátula)
            Some(None) => {}               // mismo frame que ya tenemos
            Some(Some((img, serial, aspect))) => {
                match &mut self.video_tex {
                    Some(tex) => tex.set(img, TextureOptions::LINEAR),
                    None => {
                        self.video_tex = Some(ctx.load_texture("capyro-video", img, TextureOptions::LINEAR));
                    }
                }
                self.video_serial = serial;
                self.video_aspect = aspect;
            }
        }
    }

    /// Botón redondo de ícono: fondo que aparece y crece al pasar el mouse,
    /// y un "pulso" más fuerte mientras se mantiene apretado.
    fn icon_button(ui: &mut Ui, id: &str, center: Pos2, size: f32) -> Anim {
        let rect = Rect::from_center_size(center, Vec2::splat(size));
        let a = animate_button(ui, Id::unique(("capyro-btn", id)), rect, true);
        let t = a.hover.max(a.press);
        if t > 0.001 {
            let r = size * 0.5 * (0.7 + 0.3 * a.hover) * (1.0 - 0.06 * a.press);
            let alpha = (14.0 * a.hover + 16.0 * a.press) as u8;
            ui.painter().circle_filled(center, r, Color32::from_white_alpha(alpha));
        }
        a
    }

    fn header(&mut self, ui: &mut Ui, rect: Rect, maximized: bool) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();

        // Zona para arrastrar la ventana (todo el header menos los botones).
        let drag_rect = Rect::from_min_max(rect.min, pos2(rect.right() - 80.0, rect.bottom()));
        let drag = ui.interact(drag_rect, Id::unique("capyro-titlebar"), Sense::click_and_drag());
        if drag.double_clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        } else if drag.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        let logo_rect = Rect::from_min_size(rect.min, vec2(46.0, 46.0));
        if let Some(logo) = &self.logo {
            painter.image(
                logo.id(),
                logo_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let tx = logo_rect.right() + 8.0;
        painter.text(pos2(tx, rect.top() + 14.0), Align2::LEFT_CENTER, "Capyro", bold(17.0), TEXT);
        painter.text(
            pos2(tx, rect.top() + 36.0),
            Align2::LEFT_CENTER,
            "Just press play.",
            regular(12.0),
            TEXT_DIM,
        );

        let cy = rect.center().y;
        let close_c = pos2(rect.right() - 8.0, cy);
        let max_c = close_c - vec2(24.0, 0.0);
        let min_c = max_c - vec2(24.0, 0.0);

        let min = Self::icon_button(ui, "minimize", min_c, 24.0);
        icons::win_minimize(&painter, min_c, 14.0 * min.scale(), min.tint(ICON, Color32::WHITE));
        if min.resp.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }

        let max = Self::icon_button(ui, "maximize", max_c, 24.0);
        icons::win_maximize(&painter, max_c, 15.0 * max.scale(), max.tint(ICON, Color32::WHITE), maximized);
        if max.resp.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }

        // Cerrar: el fondo se vuelve rojo al pasar el mouse.
        let close_rect = Rect::from_center_size(close_c, Vec2::splat(24.0));
        let close = animate_button(ui, Id::unique(("capyro-btn", "close")), close_rect, true);
        if close.hover > 0.001 {
            painter.circle_filled(
                close_c,
                12.0 * (0.7 + 0.3 * close.hover),
                ERROR.gamma_multiply(0.9 * close.hover),
            );
        }
        icons::close(&painter, close_c, 13.0 * close.scale(), close.tint(ICON, Color32::WHITE));
        if close.resp.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    fn video_area(&mut self, ui: &mut Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 10.0, BG_VIDEO);

        let resp = ui.interact(rect, Id::unique("capyro-video"), Sense::click());
        if resp.double_clicked() {
            self.run_action(&ctx, Action::Fullscreen);
        } else if resp.clicked() {
            if self.core.state() == State::Idle {
                self.open_file_dialog(&ctx); // área vacía: clic = elegir archivos
            } else {
                self.core.toggle();
            }
        }

        let hovering_files = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let c = rect.center();
        let accent = self.cfg.accent;

        if hovering_files {
            painter.rect_stroke(rect.shrink(1.0), 10.0, Stroke::new(2.0, accent), StrokeKind::Inside);
            icons::film(&painter, c - vec2(0.0, 30.0), 28.0, accent);
            painter.text(c + vec2(0.0, 4.0), Align2::CENTER_CENTER, "Suelta para abrir", regular(14.0), TEXT);
            painter.text(c + vec2(0.0, 26.0), Align2::CENTER_CENTER, "Solo .mp4 y .mp3", regular(12.0), TEXT_FAINT);
        } else {
            match self.core.media_kind() {
                MediaKind::None => {
                    icons::film(&painter, c - vec2(0.0, 33.0), 28.0, Color32::from_rgb(0x55, 0x56, 0x5C));
                    painter.text(c, Align2::CENTER_CENTER, "Área de vídeo", regular(14.0), TEXT_DIM);
                    painter.text(
                        c + vec2(0.0, 27.0),
                        Align2::CENTER_CENTER,
                        "Arrastra un archivo o haz clic para elegirlo",
                        regular(12.0),
                        TEXT_FAINT,
                    );
                }
                MediaKind::Video => {
                    if let Some(tex) = &self.video_tex {
                        // Letterbox: el video entra entero manteniendo su aspecto.
                        let avail = rect.shrink(1.0).size();
                        let mut size = vec2(avail.x, avail.x / self.video_aspect);
                        if size.y > avail.y {
                            size = vec2(avail.y * self.video_aspect, avail.y);
                        }
                        let dest = Rect::from_center_size(c, size);
                        painter.rect_filled(dest, 0.0, Color32::BLACK);
                        painter.image(tex.id(), dest, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    } else {
                        icons::film(&painter, c - vec2(0.0, 33.0), 28.0, accent);
                        painter.text(c, Align2::CENTER_CENTER, "Cargando…", regular(14.0), TEXT_DIM);
                    }
                }
                MediaKind::Audio => {
                    let (title, artist) = self.track_labels();
                    let has_cover = self.video_tex.is_some();
                    let side = (rect.height() * 0.5).clamp(80.0, 260.0);
                    let art_center = c - vec2(0.0, if has_cover { 28.0 } else { 36.0 });
                    let text_y = if has_cover { art_center.y + side / 2.0 + 24.0 } else { c.y + 6.0 };

                    if let Some(tex) = &self.video_tex {
                        let art = Rect::from_center_size(art_center, Vec2::splat(side));
                        let mut size = vec2(side, side / self.video_aspect);
                        if size.y > side {
                            size = vec2(side * self.video_aspect, side);
                        }
                        painter.rect_filled(art.expand(1.0), 10.0, BG_PANEL);
                        painter.image(
                            tex.id(),
                            Rect::from_center_size(art_center, size),
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        painter.circle_filled(art_center, 30.0, accent.gamma_multiply(0.18));
                        icons::music_note(&painter, art_center, 28.0, accent);
                    }
                    painter.text(pos2(c.x, text_y), Align2::CENTER_CENTER, ellipsize(&title, 52), bold(15.0), TEXT);
                    if !artist.is_empty() {
                        painter.text(
                            pos2(c.x, text_y + 22.0),
                            Align2::CENTER_CENTER,
                            ellipsize(&artist, 60),
                            regular(12.5),
                            TEXT_DIM,
                        );
                    }
                }
            }
        }

        if self.show_playlist {
            self.playlist_panel(ui, rect);
        }
        self.draw_toast(ui, rect);
    }

    fn playlist_panel(&mut self, ui: &mut Ui, video: Rect) {
        // Entra deslizándose desde la derecha y se desvanece al salir.
        let t = ui.ctx().animate_bool_with_time(Id::unique("capyro-playlist-anim"), self.show_playlist, 0.18);
        if t <= 0.001 {
            return;
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let slide = (1.0 - ease) * 40.0;
        let panel = Rect::from_min_max(
            pos2((video.right() - 290.0).max(video.left() + 8.0) + slide, video.top() + 8.0),
            pos2(video.right() - 8.0 + slide, video.bottom() - 8.0),
        );
        let interactive = self.show_playlist && t > 0.5;
        let mut painter = ui.painter().clone();
        painter.set_opacity(ease);
        if interactive {
            // Bloquea los clics para que no lleguen al área de video de abajo.
            ui.interact(panel, Id::unique("capyro-playlist-bg"), Sense::click());
        }
        painter.rect_filled(panel.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(60));
        painter.rect_filled(panel, 10.0, BG_PANEL);
        painter.rect_stroke(panel, 10.0, Stroke::new(1.0, Color32::from_white_alpha(12)), StrokeKind::Inside);

        let list = self.core.playlist();
        let current = self.core.playlist_current();
        let accent = self.cfg.accent;
        let mut op = None;

        painter.text(
            pos2(panel.left() + 12.0, panel.top() + 18.0),
            Align2::LEFT_CENTER,
            format!("Lista · {}", list.len()),
            bold(13.0),
            TEXT,
        );
        if !list.is_empty() {
            let clear_pos = pos2(panel.right() - 12.0, panel.top() + 18.0);
            let clear_rect = Rect::from_center_size(clear_pos - vec2(22.0, 0.0), vec2(52.0, 22.0));
            let clear = animate_button(ui, Id::unique("capyro-playlist-clear"), clear_rect, interactive);
            if clear.hover > 0.001 {
                painter.rect_filled(clear_rect, 6.0, ERROR.gamma_multiply(0.15 * clear.hover));
            }
            painter.text(clear_pos, Align2::RIGHT_CENTER, "Limpiar", regular(12.0), clear.tint(TEXT_FAINT, ERROR));
            if clear.resp.clicked() {
                op = Some(PlaylistOp::Clear);
            }
        }

        let list_rect = Rect::from_min_max(pos2(panel.left() + 4.0, panel.top() + 36.0), panel.max - vec2(4.0, 6.0));
        if list.is_empty() {
            painter.text(
                list_rect.center(),
                Align2::CENTER_CENTER,
                "La lista está vacía",
                regular(12.0),
                TEXT_FAINT,
            );
        } else {
            let mut child = ui.new_child(UiBuilder::new().max_rect(list_rect));
            child.set_opacity(ease);
            if !interactive {
                child.disable();
            }
            ScrollArea::vertical().auto_shrink([false, false]).show(&mut child, |ui| {
                for (i, path) in list.iter().enumerate() {
                    let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
                    // OJO: el hover se mide sobre el rectángulo de la fila y no con
                    // resp.hovered(), porque al pasar sobre la ✕ el hover pasa a la ✕;
                    // antes eso escondía la ✕ y el clic caía en la fila.
                    let row_hovered = interactive && ui.rect_contains_pointer(row);
                    let hover = ui.ctx().animate_bool_with_time(
                        Id::unique(("capyro-row-hover", i)),
                        row_hovered,
                        0.12,
                    );
                    let is_current = current == Some(i);
                    let p = ui.painter();
                    if is_current {
                        p.rect_filled(row, 6.0, accent.gamma_multiply(0.14 + 0.06 * hover));
                        p.rect_filled(
                            Rect::from_min_size(row.min + vec2(0.0, 7.0), vec2(3.0, 16.0)),
                            1.5,
                            accent,
                        );
                    } else if hover > 0.001 {
                        p.rect_filled(row, 6.0, Color32::from_white_alpha((12.0 * hover) as u8));
                    }
                    p.text(
                        pos2(row.left() + 12.0, row.center().y),
                        Align2::LEFT_CENTER,
                        format!("{:>2}", i + 1),
                        regular(11.0),
                        TEXT_FAINT,
                    );
                    p.text(
                        pos2(row.left() + 34.0 + 3.0 * hover, row.center().y),
                        Align2::LEFT_CENTER,
                        ellipsize(&file_name(path), 28),
                        regular(12.5),
                        if is_current { accent } else { TEXT },
                    );

                    let mut removed = false;
                    if hover > 0.001 {
                        let xc = pos2(row.right() - 14.0, row.center().y);
                        let x = animate_button(
                            ui,
                            Id::unique(("capyro-playlist-rm", i)),
                            Rect::from_center_size(xc, Vec2::splat(22.0)),
                            row_hovered,
                        );
                        if x.hover > 0.001 {
                            ui.painter().circle_filled(xc, 10.0, ERROR.gamma_multiply(0.2 * x.hover));
                        }
                        let col = x.tint(TEXT_FAINT, ERROR).gamma_multiply(hover);
                        icons::close(ui.painter(), xc, 9.0 * x.scale(), col);
                        if x.resp.clicked() {
                            op = Some(PlaylistOp::Remove(i));
                            removed = true;
                        }
                        x.resp.on_hover_text("Quitar de la lista");
                    }
                    if row_hovered {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if !removed && resp.clicked() {
                        op = Some(PlaylistOp::Select(i));
                    }
                }
            });
        }

        match op {
            Some(PlaylistOp::Select(i)) => match self.core.playlist_select(i) {
                Ok(()) => {
                    if self.core.state() != State::Playing {
                        self.core.toggle();
                    }
                }
                Err(e) => {
                    let ctx = ui.ctx().clone();
                    self.toast(&ctx, e.0, true);
                }
            },
            Some(PlaylistOp::Remove(i)) => self.core.playlist_remove(i),
            Some(PlaylistOp::Clear) => self.core.playlist_clear(),
            None => {}
        }
    }

    /// Título del metadato (o nombre de archivo) y artista.
    fn track_labels(&self) -> (String, String) {
        let title = self.core.title();
        let title = if title.trim().is_empty() {
            file_name(&self.core.current_file())
        } else {
            title
        };
        (title, self.core.artist())
    }

    fn draw_toast(&mut self, ui: &mut Ui, video: Rect) {
        let now = ui.ctx().input(|i| i.time);
        let Some(t) = &self.toast else { return };
        if now > t.until {
            self.toast = None;
            return;
        }
        // Aparece subiendo un poco y se desvanece al final.
        let fade_in = ((now - t.start) / 0.15).clamp(0.0, 1.0) as f32;
        let fade_out = ((t.until - now) / 0.35).clamp(0.0, 1.0) as f32;
        let alpha = fade_in.min(fade_out);
        let rise = (1.0 - fade_in) * 8.0;

        let mut painter = ui.painter().clone();
        painter.set_opacity(alpha);
        let galley = painter.layout_no_wrap(t.text.clone(), regular(12.5), if t.error { ERROR } else { TEXT });
        let size = galley.size() + vec2(28.0, 14.0);
        let rect = Rect::from_center_size(pos2(video.center().x, video.bottom() - 28.0 + rise), size);
        painter.rect_filled(rect, size.y / 2.0, BG_PANEL);
        painter.rect_stroke(rect, size.y / 2.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
        painter.galley(rect.center() - galley.size() / 2.0, galley, TEXT);
        ui.ctx().request_repaint(); // la animación necesita frames
    }

    fn transport(&mut self, ui: &mut Ui, video_bottom: f32, inner: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();
        let accent = self.cfg.accent;
        let (left, right) = (inner.left(), inner.right());

        /* --- barra de progreso --- */
        let ty = video_bottom + TRACK_Y;
        let hit = Rect::from_min_max(pos2(left, ty - 8.0), pos2(right, ty + 8.0));
        let seek = ui.interact(hit, Id::unique("capyro-seek"), Sense::click_and_drag());
        let duration = self.core.duration();
        let can_seek = duration > 0.0;
        if can_seek && (seek.dragged() || seek.clicked()) {
            if let Some(pp) = seek.interact_pointer_pos() {
                let f = ((pp.x - left) / (right - left)).clamp(0.0, 1.0) as f64;
                self.core.seek(f * duration);
            }
        }
        let position = self.core.position();
        let frac = if can_seek { (position / duration).clamp(0.0, 1.0) as f32 } else { 0.0 };
        let active = can_seek && (seek.hovered() || seek.dragged());
        let grow = ctx.animate_bool_with_time(Id::unique("capyro-seek-grow"), active, 0.12);
        let th = 3.0 + 2.0 * grow;
        let track = Rect::from_min_max(pos2(left, ty - th / 2.0), pos2(right, ty + th / 2.0));
        painter.rect_filled(track, th / 2.0, TRACK);
        let fill_x = left + (right - left) * frac;
        if let Some(hp) = seek.hover_pos().filter(|_| active) {
            // vista previa de hasta dónde saltaría
            let hx = hp.x.clamp(left, right);
            if hx > fill_x {
                painter.rect_filled(
                    Rect::from_min_max(pos2(fill_x, track.min.y), pos2(hx, track.max.y)),
                    th / 2.0,
                    Color32::from_white_alpha(28),
                );
            }
        }
        painter.rect_filled(Rect::from_min_max(track.min, pos2(fill_x, track.max.y)), th / 2.0, accent);
        if grow > 0.001 {
            let pressed = ctx.animate_bool_with_time(Id::unique("capyro-seek-press"), seek.dragged(), 0.08);
            painter.circle_filled(pos2(fill_x, ty), 6.5 * grow + 1.5 * pressed, accent);
        }
        if active {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
            if let Some(hp) = seek.hover_pos() {
                let f = ((hp.x - left) / (right - left)).clamp(0.0, 1.0) as f64;
                let label = fmt_time(f * duration);
                let g = painter.layout_no_wrap(label, regular(11.0), TEXT);
                let r = Rect::from_center_size(pos2(hp.x, ty - 18.0), g.size() + vec2(10.0, 6.0));
                painter.rect_filled(r, 4.0, BG_PANEL);
                painter.galley(r.center() - g.size() / 2.0, g, TEXT);
            }
        }

        /* --- tiempos (+ velocidad si no es 1×) --- */
        let tty = video_bottom + TIMES_Y;
        painter.text(pos2(left, tty), Align2::LEFT_CENTER, fmt_time(position), regular(12.0), TEXT_DIM);
        let dur_rect = painter.text(pos2(right, tty), Align2::RIGHT_CENTER, fmt_time(duration), regular(12.0), TEXT_DIM);
        let speed = self.core.speed();
        if (speed - 1.0).abs() > 1e-6 {
            let g = painter.layout_no_wrap(fmt_speed(speed), bold(11.0), accent);
            let r = Rect::from_min_size(
                pos2(dur_rect.left() - g.size().x - 18.0, tty - g.size().y / 2.0 - 3.0),
                g.size() + vec2(10.0, 6.0),
            );
            painter.rect_filled(r, r.height() / 2.0, accent.gamma_multiply(0.15));
            painter.galley(r.center() - g.size() / 2.0, g, accent);
        }

        /* --- controles --- */
        let cy = video_bottom + CONTROLS_Y;
        let cx = inner.center().x;

        // aleatorio (izquierda)
        let shuffle_on = self.core.shuffle();
        let shuffle_c = pos2(cx - 105.0, cy);
        let sh = Self::icon_button(ui, "shuffle", shuffle_c, 30.0);
        let sh_on = ctx.animate_bool_with_time(Id::unique("capyro-shuffle-on"), shuffle_on, 0.15);
        let sh_col = sh.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, sh_on);
        icons::shuffle(&painter, shuffle_c, 16.0 * sh.scale(), sh_col);
        if sh_on > 0.001 {
            painter.circle_filled(shuffle_c + vec2(0.0, 13.0), 2.0 * sh_on, accent);
        }
        if sh.resp.clicked() {
            self.toggle_shuffle(&ctx);
        }
        sh.resp.on_hover_text(if shuffle_on { "Aleatorio: activado (Ctrl+S)" } else { "Aleatorio: desactivado (Ctrl+S)" });

        let prev_c = pos2(cx - 69.0, cy);
        let prev = Self::icon_button(ui, "prev", prev_c, 30.0);
        icons::prev(&painter, prev_c - vec2(2.0 * prev.press, 0.0), 16.0 * prev.scale(), prev.tint(ICON, Color32::WHITE));
        if prev.resp.clicked() {
            self.run_action(&ctx, Action::PrevFile);
        }

        let restart_c = pos2(cx - 33.0, cy);
        let restart = Self::icon_button(ui, "restart", restart_c, 30.0);
        icons::restart(&painter, restart_c, 16.0 * restart.scale(), restart.tint(ICON, Color32::WHITE));
        if restart.resp.clicked() {
            self.core.seek(0.0);
        }
        restart.resp.on_hover_text("Volver al inicio");

        // play / pausa: crece, brilla y el ícono se transforma
        let play_c = pos2(cx + 17.0, cy);
        let play = animate_button(ui, Id::unique("capyro-play"), Rect::from_center_size(play_c, Vec2::splat(46.0)), true);
        let playing = self.core.state() == State::Playing;
        let morph = ctx.animate_bool_with_time(Id::unique("capyro-play-morph"), playing, 0.16);
        let scale = 1.0 + 0.06 * play.hover - 0.1 * play.press;
        if play.hover > 0.001 {
            painter.circle_filled(play_c, 23.0 * scale + 6.0 * play.hover, accent.gamma_multiply(0.18 * play.hover));
        }
        let fill = accent.lerp_to_gamma(Color32::WHITE, 0.12 * play.hover).gamma_multiply(1.0 - 0.12 * play.press);
        painter.circle_filled(play_c, 23.0 * scale, fill);
        if morph < 0.999 {
            icons::play_badge(&painter, play_c, 21.0 * scale, Color32::WHITE.gamma_multiply(1.0 - morph), false);
        }
        if morph > 0.001 {
            icons::play_badge(&painter, play_c, 21.0 * scale, Color32::WHITE.gamma_multiply(morph), true);
        }
        if play.resp.clicked() {
            self.run_action(&ctx, Action::PlayPause);
        }

        let next_c = pos2(cx + 69.0, cy);
        let next = Self::icon_button(ui, "next", next_c, 30.0);
        icons::next(&painter, next_c + vec2(2.0 * next.press, 0.0), 16.0 * next.scale(), next.tint(ICON, Color32::WHITE));
        if next.resp.clicked() {
            self.run_action(&ctx, Action::NextFile);
        }

        // repetir (derecha): off → lista → uno
        let repeat = self.core.repeat();
        let repeat_c = pos2(cx + 105.0, cy);
        let rp = Self::icon_button(ui, "repeat", repeat_c, 30.0);
        let rp_on = ctx.animate_bool_with_time(Id::unique("capyro-repeat-on"), repeat != Repeat::Off, 0.15);
        let rp_col = rp.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, rp_on);
        icons::repeat(&painter, repeat_c, 16.0 * rp.scale(), rp_col, repeat == Repeat::One);
        if rp_on > 0.001 {
            painter.circle_filled(repeat_c + vec2(0.0, 13.0), 2.0 * rp_on, accent);
        }
        if rp.resp.clicked() {
            self.cycle_repeat(&ctx);
        }
        rp.resp.on_hover_text(match repeat {
            Repeat::Off => "Repetir: no (Ctrl+R)",
            Repeat::All => "Repetir: toda la lista (Ctrl+R)",
            Repeat::One => "Repetir: este archivo (Ctrl+R)",
        });

        /* --- botón "+ Agregar archivo(s)" (sobre los íconos de la derecha) --- */
        let pending = self.dialog.is_some();
        let label = if pending { "Abriendo…" } else { "Agregar archivo(s)" };
        let galley = painter.layout_no_wrap(label.to_owned(), bold(13.0), Color32::WHITE);
        let full_w = galley.size().x + 50.0;
        let controls_right = repeat_c.x + 22.0;
        // Si la ventana es angosta y no entra el texto, queda solo el "+".
        let compact = right - full_w < controls_right;
        let base = if compact { vec2(34.0, 34.0) } else { vec2(full_w, 34.0) };
        let base_rect = Rect::from_min_size(pos2(right - base.x, cy - base.y / 2.0), base);
        let add = animate_button(ui, Id::unique("capyro-add-files"), base_rect, !pending);
        let s = 1.0 + 0.03 * add.hover - 0.06 * add.press;
        let add_rect = Rect::from_center_size(base_rect.center(), base * s);
        if add.hover > 0.001 {
            painter.rect_filled(add_rect.expand(4.0 * add.hover), add_rect.height() / 2.0 + 4.0, accent.gamma_multiply(0.16 * add.hover));
        }
        let fill = if pending {
            accent.gamma_multiply(0.6)
        } else {
            accent.lerp_to_gamma(Color32::WHITE, 0.12 * add.hover).gamma_multiply(1.0 - 0.12 * add.press)
        };
        painter.rect_filled(add_rect, add_rect.height() / 2.0, fill);
        // el "+" gira un cuarto de vuelta al pasar el mouse
        let angle = add.hover * std::f32::consts::FRAC_PI_2;
        if compact {
            icons::plus(&painter, add_rect.center(), 14.0, Color32::WHITE, angle, 2.0);
        } else {
            let plus_c = pos2(add_rect.left() + 20.0, add_rect.center().y);
            icons::plus(&painter, plus_c, 11.0, Color32::WHITE, angle, 2.0);
            painter.galley(
                pos2(plus_c.x + 12.0, add_rect.center().y - galley.size().y / 2.0),
                galley,
                Color32::WHITE,
            );
        }
        if add.resp.clicked() {
            self.open_file_dialog(&ctx);
        }
        add.resp.on_hover_text("Agregar archivos .mp4 o .mp3 a la lista (Ctrl+O)");

        /* --- fila inferior --- */
        let by = video_bottom + BOTTOM_ROW_Y;

        let list_rect = Rect::from_min_max(pos2(left - 6.0, by - 14.0), pos2(left + 66.0, by + 14.0));
        let list = animate_button(ui, Id::unique("capyro-list"), list_rect, true);
        let list_on = ctx.animate_bool_with_time(Id::unique("capyro-list-on"), self.show_playlist, 0.15);
        let list_col = list.tint(ICON_DIM, TEXT).lerp_to_gamma(accent, list_on);
        let bg = (10.0 * list.hover + 10.0 * list.press) as u8;
        if bg > 0 {
            painter.rect_filled(list_rect, 7.0, Color32::from_white_alpha(bg));
        }
        icons::playlist(&painter, pos2(left + 7.0, by), 14.0 * list.scale(), list_col);
        painter.text(pos2(left + 24.0, by), Align2::LEFT_CENTER, "Lista", regular(13.0), list_col);
        if list.resp.clicked() {
            self.show_playlist = !self.show_playlist;
        }

        // volumen: clic = silenciar, rueda = ajustar
        let vol_c = pos2(right - 92.0, by);
        let vol = Self::icon_button(ui, "volume", vol_c, 26.0);
        let (volume, muted) = (self.core.volume(), self.core.muted());
        icons::volume(&painter, vol_c, 16.0 * vol.scale(), vol.tint(ICON_DIM, Color32::WHITE), volume, muted);
        if vol.resp.clicked() {
            self.toggle_mute(&ctx);
        }
        if vol.resp.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.5 {
                self.change_volume(&ctx, scroll.signum() * self.cfg.volume_step);
            }
        }
        if vol.hover > 0.001 {
            let label = if muted {
                "Silenciado".to_owned()
            } else {
                format!("{}%", (volume * 100.0).round())
            };
            painter.text(
                vol_c - vec2(16.0 + 4.0 * vol.hover, 0.0),
                Align2::RIGHT_CENTER,
                label,
                regular(11.0),
                TEXT_FAINT.gamma_multiply(vol.hover),
            );
        }
        vol.resp.on_hover_text("Clic: silenciar · Rueda: volumen");

        let subs_c = pos2(right - 64.0, by);
        let subs = Self::icon_button(ui, "subtitles", subs_c, 26.0);
        icons::subtitles(&painter, subs_c, 16.0 * subs.scale(), subs.tint(ICON_DIM, Color32::WHITE));
        subs.resp.on_hover_text("Subtítulos — próximamente");

        // engranaje: abre el menú de ajustes (y gira un poco al pasar el mouse)
        let gear_c = pos2(right - 36.0, by);
        let gear = Self::icon_button(ui, "settings", gear_c, 26.0);
        let gear_on = ctx.animate_bool_with_time(Id::unique("capyro-gear-on"), self.show_settings, 0.15);
        let gear_col = gear.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, gear_on);
        icons::gear(&painter, gear_c, 16.0 * gear.scale(), gear_col);
        self.gear_rect = Rect::from_center_size(gear_c, Vec2::splat(26.0));
        if gear.resp.clicked() {
            self.show_settings = !self.show_settings;
        }
        if !self.show_settings {
            gear.resp.on_hover_text("Ajustes");
        }

        let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let fs_c = pos2(right - 8.0, by);
        let fsb = Self::icon_button(ui, "fullscreen", fs_c, 26.0);
        icons::fullscreen(&painter, fs_c, 14.0 * fsb.scale(), fsb.tint(ICON_DIM, Color32::WHITE), fs);
        if fsb.resp.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fs));
        }
    }

    /// Menú del engranaje: velocidad y recargar configuración. Se abre
    /// creciendo desde el engranaje y se cierra al hacer clic afuera o con Esc.
    fn settings_menu(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let t = ctx.animate_bool_with_time(Id::unique("capyro-settings-anim"), self.show_settings, 0.15);
        if t <= 0.001 {
            return;
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let accent = self.cfg.accent;
        let size = vec2(272.0, 156.0);
        let anchor = pos2(self.gear_rect.right() + 6.0, self.gear_rect.top() - 10.0);
        let full = Rect::from_min_size(anchor - size, size);
        // crece desde la esquina del engranaje
        let panel = Rect::from_min_max(anchor - size * (0.85 + 0.15 * ease), anchor);
        let interactive = self.show_settings && t > 0.6;

        let mut painter = ui.painter().clone();
        painter.set_opacity(ease);
        if interactive {
            // Capa invisible sobre toda la ventana: un clic afuera solo cierra
            // el menú (no pausa el video ni aprieta lo que haya abajo).
            let outside = ui.interact(ui.max_rect(), Id::unique("capyro-settings-outside"), Sense::click());
            if outside.clicked() {
                self.show_settings = false;
            }
            ui.interact(full, Id::unique("capyro-settings-bg"), Sense::click());
        }
        painter.rect_filled(panel.translate(vec2(0.0, 4.0)), 12.0, Color32::from_black_alpha(70));
        painter.rect_filled(panel, 12.0, BG_PANEL);
        painter.rect_stroke(panel, 12.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
        if ease < 0.98 {
            ctx.request_repaint();
            return; // el contenido aparece cuando termina de abrirse
        }

        let x0 = panel.left() + 14.0;
        let x1 = panel.right() - 14.0;
        let mut y = panel.top() + 20.0;
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Ajustes", bold(13.5), TEXT);

        /* velocidad */
        y += 28.0;
        let speed = self.core.speed();
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Velocidad", regular(12.5), TEXT_DIM);
        painter.text(pos2(x1, y), Align2::RIGHT_CENTER, fmt_speed(speed), bold(12.5), accent);
        y += 24.0;
        let gap = 5.0;
        let chip_w = ((x1 - x0) - gap * (SPEEDS.len() as f32 - 1.0)) / SPEEDS.len() as f32;
        for (i, &v) in SPEEDS.iter().enumerate() {
            let r = Rect::from_min_size(pos2(x0 + i as f32 * (chip_w + gap), y - 13.0), vec2(chip_w, 26.0));
            let chip = animate_button(ui, Id::unique(("capyro-speed", i)), r, interactive);
            let selected = (speed - v).abs() < 1e-6;
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-speed-sel", i)), selected, 0.15);
            let bg = Color32::from_white_alpha((8.0 + 14.0 * chip.hover) as u8).lerp_to_gamma(accent, sel);
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.06 * chip.press));
            painter.rect_filled(rr, 7.0, bg);
            let col = TEXT_DIM.lerp_to_gamma(TEXT, chip.hover).lerp_to_gamma(Color32::WHITE, sel);
            painter.text(rr.center(), Align2::CENTER_CENTER, fmt_speed(v), if selected { bold(11.5) } else { regular(11.5) }, col);
            if chip.resp.clicked() && !selected {
                self.set_speed(&ctx, v);
            }
        }

        /* separador */
        y += 24.0;
        painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, Color32::from_white_alpha(14)));

        /* recargar configuración */
        y += 26.0;
        let row = Rect::from_min_max(pos2(x0 - 6.0, y - 18.0), pos2(x1 + 6.0, y + 18.0));
        let reload = animate_button(ui, Id::unique("capyro-settings-reload"), row, interactive);
        if reload.hover > 0.001 || reload.press > 0.001 {
            painter.rect_filled(row, 8.0, Color32::from_white_alpha((10.0 * reload.hover + 10.0 * reload.press) as u8));
        }
        // el ícono gira mientras se mantiene el mouse encima
        let spin = ctx.animate_bool_with_time(Id::unique("capyro-reload-spin"), reload.resp.hovered(), 0.35);
        let ic = pos2(x0 + 8.0, y - 1.0);
        let col = reload.tint(ICON, Color32::WHITE);
        draw_rotated_reload(&painter, ic, 15.0, col, spin * std::f32::consts::TAU * 0.5);
        painter.text(pos2(x0 + 24.0, y - 7.0), Align2::LEFT_CENTER, "Recargar configuración", regular(12.5), TEXT);
        painter.text(
            pos2(x0 + 24.0, y + 8.0),
            Align2::LEFT_CENTER,
            ellipsize(&self.cfg.source, 34),
            regular(10.5),
            TEXT_FAINT,
        );
        if reload.resp.clicked() {
            self.reload_config(&ctx);
        }

        // Esc también lo cierra
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.show_settings = false;
        }
    }

    /// Ventana sin bordes del sistema → bordes de 6px para redimensionar.
    fn resize_handles(ui: &mut Ui, full: Rect) {
        const B: f32 = 6.0;
        let (l, r, t, b) = (full.left(), full.right(), full.top(), full.bottom());
        let zones = [
            (Rect::from_min_max(pos2(l, t), pos2(l + B, t + B)), ResizeDirection::NorthWest, CursorIcon::ResizeNorthWest),
            (Rect::from_min_max(pos2(r - B, t), pos2(r, t + B)), ResizeDirection::NorthEast, CursorIcon::ResizeNorthEast),
            (Rect::from_min_max(pos2(l, b - B), pos2(l + B, b)), ResizeDirection::SouthWest, CursorIcon::ResizeSouthWest),
            (Rect::from_min_max(pos2(r - B, b - B), pos2(r, b)), ResizeDirection::SouthEast, CursorIcon::ResizeSouthEast),
            (Rect::from_min_max(pos2(l + B, t), pos2(r - B, t + B)), ResizeDirection::North, CursorIcon::ResizeNorth),
            (Rect::from_min_max(pos2(l + B, b - B), pos2(r - B, b)), ResizeDirection::South, CursorIcon::ResizeSouth),
            (Rect::from_min_max(pos2(l, t + B), pos2(l + B, b - B)), ResizeDirection::West, CursorIcon::ResizeWest),
            (Rect::from_min_max(pos2(r - B, t + B), pos2(r, b - B)), ResizeDirection::East, CursorIcon::ResizeEast),
        ];
        for (i, (rect, dir, cursor)) in zones.into_iter().enumerate() {
            let resp = ui.interact(rect, Id::unique(("capyro-resize", i)), Sense::drag());
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(cursor);
            }
            if resp.drag_started() {
                ui.ctx().send_viewport_cmd(ViewportCommand::BeginResize(dir));
            }
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = if self.core.current_file().is_empty() {
            "Capyro".to_owned()
        } else {
            format!("{} — Capyro", self.track_labels().0)
        };
        if title != self.last_title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }
}

impl eframe::App for CapyroApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // esquinas redondeadas transparentes
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(msg) = self.startup_notice.take() {
            self.toast(&ctx, msg, true);
        }
        self.core.update();
        self.handle_input(&ctx);
        self.update_title(&ctx);
        self.sync_video_texture(&ctx);
        self.poll_file_dialog(&ctx);

        let (maximized, fullscreen) = ctx.input(|i| {
            (
                i.viewport().maximized.unwrap_or(false),
                i.viewport().fullscreen.unwrap_or(false),
            )
        });
        let square = maximized || fullscreen;

        let full = ui.max_rect();
        let radius = if square { 0 } else { WINDOW_RADIUS };
        ui.painter().rect_filled(full, radius, BG_WINDOW);
        if !square {
            ui.painter().rect_stroke(
                full,
                radius,
                Stroke::new(1.0, Color32::from_white_alpha(10)),
                StrokeKind::Inside,
            );
        }

        let inner = full.shrink(PAD);
        let header = Rect::from_min_size(inner.min, vec2(inner.width(), 46.0));
        let video_bottom = (inner.bottom() - BELOW_VIDEO).max(header.bottom() + 60.0);
        let video = Rect::from_min_max(pos2(inner.left(), header.bottom() + 12.0), pos2(inner.right(), video_bottom));

        self.header(ui, header, maximized);
        self.video_area(ui, video);
        self.transport(ui, video_bottom, inner);
        self.settings_menu(ui);
        if !square {
            Self::resize_handles(ui, full);
        }

        if self.core.state() == State::Playing {
            if self.core.media_kind() == MediaKind::Video {
                ctx.request_repaint(); // al ritmo de la pantalla (vsync)
            } else {
                ctx.request_repaint_after(Duration::from_millis(33));
            }
        }
    }
}

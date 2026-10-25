//! La ventana de Capyro. Layout medido sobre el mockup (768×642):
//!
//!   ┌ header 46px ─ logo · Capyro / Just press play ─────── _ □ ✕ ┐
//!   │ área de video (flexible)                                     │
//!   │ ── barra de progreso ──────────────────────────────────────  │
//!   │ 00:00                                                  00:00 │
//!   │              ⏮   ⟲   (▶)   ⏭                                │
//!   └ ☰ Lista                                       🔊  cc  ⚙  ⛶  ┘

use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, Id, Key, Modifiers, Pos2, Rect, Response,
    ResizeDirection, ScrollArea, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui,
    UiBuilder, Vec2, ViewportCommand,
};

use crate::config::{Action, Config};
use crate::ffi::{Core, MediaKind, State};
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
    until: f64,
    error: bool,
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
        }
    }

    fn toast(&mut self, ctx: &egui::Context, text: impl Into<String>, error: bool) {
        let now = ctx.input(|i| i.time);
        self.toast = Some(Toast { text: text.into(), until: now + 2.5, error });
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
                    self.toast(ctx, "Es el último de la lista", false);
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
            Action::Quit => ctx.send_viewport_cmd(ViewportCommand::Close),
        }
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

    fn open_paths(&mut self, ctx: &egui::Context, paths: Vec<PathBuf>) {
        let first_new = self.core.playlist_len();
        let mut errors = Vec::new();
        for path in &paths {
            if let Err(e) = self.core.playlist_add(path) {
                errors.push(format!("{}: {}", file_name(&path.to_string_lossy()), e.0));
            }
        }
        if self.core.playlist_len() > first_new {
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
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // Archivos soltados sobre la ventana.
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        if !dropped.is_empty() {
            self.open_paths(ctx, dropped);
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

    fn icon_button(ui: &mut Ui, id: &str, center: Pos2, size: f32) -> Response {
        let rect = Rect::from_center_size(center, Vec2::splat(size));
        let resp = ui.interact(rect, Id::unique(("capyro-btn", id)), Sense::click());
        if resp.hovered() {
            ui.painter().circle_filled(center, size * 0.5, HOVER_BG);
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        resp
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

        if Self::icon_button(ui, "minimize", min_c, 22.0).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
        icons::win_minimize(&painter, min_c, 14.0, ICON);

        if Self::icon_button(ui, "maximize", max_c, 22.0).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }
        icons::win_maximize(&painter, max_c, 15.0, ICON, maximized);

        let close = Self::icon_button(ui, "close", close_c, 22.0);
        if close.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        icons::close(&painter, close_c, 13.0, if close.hovered() { ERROR } else { ICON });
    }

    fn video_area(&mut self, ui: &mut Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 10.0, BG_VIDEO);

        let resp = ui.interact(rect, Id::unique("capyro-video"), Sense::click());
        if resp.double_clicked() {
            self.run_action(&ctx, Action::Fullscreen);
        } else if resp.clicked() && self.core.state() != State::Idle {
            self.core.toggle();
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
                        "Arrastra un archivo para abrirlo",
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
        let panel = Rect::from_min_max(
            pos2((video.right() - 290.0).max(video.left() + 8.0), video.top() + 8.0),
            pos2(video.right() - 8.0, video.bottom() - 8.0),
        );
        let painter = ui.painter().clone();
        // Bloquea los clics para que no lleguen al área de video de abajo.
        ui.interact(panel, Id::unique("capyro-playlist-bg"), Sense::click());
        painter.rect_filled(panel, 8.0, BG_PANEL);
        painter.rect_stroke(panel, 8.0, Stroke::new(1.0, Color32::from_white_alpha(12)), StrokeKind::Inside);

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
            let clear_rect = Rect::from_center_size(clear_pos - vec2(22.0, 0.0), vec2(48.0, 20.0));
            let clear = ui.interact(clear_rect, Id::unique("capyro-playlist-clear"), Sense::click());
            let col = if clear.hovered() { TEXT } else { TEXT_FAINT };
            painter.text(clear_pos, Align2::RIGHT_CENTER, "Limpiar", regular(12.0), col);
            if clear.clicked() {
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
            ScrollArea::vertical().auto_shrink([false, false]).show(&mut child, |ui| {
                for (i, path) in list.iter().enumerate() {
                    let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
                    let is_current = current == Some(i);
                    let p = ui.painter();
                    if is_current {
                        p.rect_filled(row, 6.0, accent.gamma_multiply(0.14));
                        p.rect_filled(
                            Rect::from_min_size(row.min + vec2(0.0, 7.0), vec2(3.0, 16.0)),
                            1.5,
                            accent,
                        );
                    } else if resp.hovered() {
                        p.rect_filled(row, 6.0, HOVER_BG);
                    }
                    p.text(
                        pos2(row.left() + 12.0, row.center().y),
                        Align2::LEFT_CENTER,
                        format!("{:>2}", i + 1),
                        regular(11.0),
                        TEXT_FAINT,
                    );
                    p.text(
                        pos2(row.left() + 34.0, row.center().y),
                        Align2::LEFT_CENTER,
                        ellipsize(&file_name(path), 30),
                        regular(12.5),
                        if is_current { accent } else { TEXT },
                    );
                    if resp.hovered() {
                        let xc = pos2(row.right() - 14.0, row.center().y);
                        let x = ui.interact(
                            Rect::from_center_size(xc, Vec2::splat(18.0)),
                            Id::unique(("capyro-playlist-rm", i)),
                            Sense::click(),
                        );
                        icons::close(ui.painter(), xc, 9.0, if x.hovered() { ERROR } else { TEXT_FAINT });
                        if x.clicked() {
                            op = Some(PlaylistOp::Remove(i));
                            continue;
                        }
                    }
                    if resp.clicked() {
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
        let painter = ui.painter();
        let galley = painter.layout_no_wrap(t.text.clone(), regular(12.5), if t.error { ERROR } else { TEXT });
        let size = galley.size() + vec2(28.0, 14.0);
        let rect = Rect::from_center_size(pos2(video.center().x, video.bottom() - 28.0), size);
        painter.rect_filled(rect, size.y / 2.0, BG_PANEL);
        painter.rect_stroke(rect, size.y / 2.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
        painter.galley(rect.center() - galley.size() / 2.0, galley, TEXT);
        ui.ctx().request_repaint_after(Duration::from_secs_f64((t.until - now).max(0.0)));
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
        let th = if active { 5.0 } else { 3.0 };
        let track = Rect::from_min_max(pos2(left, ty - th / 2.0), pos2(right, ty + th / 2.0));
        painter.rect_filled(track, th / 2.0, TRACK);
        let fill_x = left + (right - left) * frac;
        painter.rect_filled(Rect::from_min_max(track.min, pos2(fill_x, track.max.y)), th / 2.0, accent);
        if active {
            painter.circle_filled(pos2(fill_x, ty), 6.0, accent);
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

        /* --- tiempos --- */
        let tty = video_bottom + TIMES_Y;
        painter.text(pos2(left, tty), Align2::LEFT_CENTER, fmt_time(position), regular(12.0), TEXT_DIM);
        painter.text(pos2(right, tty), Align2::RIGHT_CENTER, fmt_time(duration), regular(12.0), TEXT_DIM);

        /* --- controles --- */
        let cy = video_bottom + CONTROLS_Y;
        let cx = inner.center().x;

        let prev_c = pos2(cx - 69.0, cy);
        if Self::icon_button(ui, "prev", prev_c, 30.0).clicked() {
            self.run_action(&ctx, Action::PrevFile);
        }
        icons::prev(&painter, prev_c, 16.0, ICON);

        let restart_c = pos2(cx - 33.0, cy);
        let restart = Self::icon_button(ui, "restart", restart_c, 30.0);
        if restart.clicked() {
            self.core.seek(0.0);
        }
        icons::restart(&painter, restart_c, 16.0, ICON);
        restart.on_hover_text("Volver al inicio");

        let play_c = pos2(cx + 17.0, cy);
        let play = ui.interact(
            Rect::from_center_size(play_c, Vec2::splat(46.0)),
            Id::unique("capyro-play"),
            Sense::click(),
        );
        let playing = self.core.state() == State::Playing;
        let fill = if play.is_pointer_button_down_on() {
            accent.gamma_multiply(0.85)
        } else if play.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
            accent.lerp_to_gamma(Color32::WHITE, 0.12)
        } else {
            accent
        };
        painter.circle_filled(play_c, 23.0, fill);
        icons::play_badge(&painter, play_c, 21.0, Color32::WHITE, playing);
        if play.clicked() {
            self.run_action(&ctx, Action::PlayPause);
        }

        let next_c = pos2(cx + 69.0, cy);
        if Self::icon_button(ui, "next", next_c, 30.0).clicked() {
            self.run_action(&ctx, Action::NextFile);
        }
        icons::next(&painter, next_c, 16.0, ICON);

        /* --- fila inferior --- */
        let by = video_bottom + BOTTOM_ROW_Y;

        let list_rect = Rect::from_min_max(pos2(left - 4.0, by - 13.0), pos2(left + 64.0, by + 13.0));
        let list = ui.interact(list_rect, Id::unique("capyro-list"), Sense::click());
        let list_col = if self.show_playlist {
            accent
        } else if list.hovered() {
            TEXT
        } else {
            ICON_DIM
        };
        if list.hovered() {
            painter.rect_filled(list_rect, 6.0, HOVER_BG);
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        icons::playlist(&painter, pos2(left + 7.0, by), 14.0, list_col);
        painter.text(pos2(left + 24.0, by), Align2::LEFT_CENTER, "Lista", regular(13.0), list_col);
        if list.clicked() {
            self.show_playlist = !self.show_playlist;
        }

        // volumen: clic = silenciar, rueda = ajustar
        let vol_c = pos2(right - 92.0, by);
        let vol = Self::icon_button(ui, "volume", vol_c, 26.0);
        let (volume, muted) = (self.core.volume(), self.core.muted());
        icons::volume(&painter, vol_c, 16.0, ICON_DIM, volume, muted);
        if vol.clicked() {
            self.toggle_mute(&ctx);
        }
        if vol.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.5 {
                self.change_volume(&ctx, scroll.signum() * self.cfg.volume_step);
            }
            let label = if muted {
                "Silenciado".to_owned()
            } else {
                format!("{}%", (volume * 100.0).round())
            };
            painter.text(vol_c - vec2(18.0, 0.0), Align2::RIGHT_CENTER, label, regular(11.0), TEXT_FAINT);
        }
        vol.on_hover_text("Clic: silenciar · Rueda: volumen");

        let subs_c = pos2(right - 64.0, by);
        let subs = Self::icon_button(ui, "subtitles", subs_c, 26.0);
        icons::subtitles(&painter, subs_c, 16.0, ICON_DIM);
        subs.on_hover_text("Subtítulos — próximamente");

        let gear_c = pos2(right - 36.0, by);
        let gear = Self::icon_button(ui, "settings", gear_c, 26.0);
        icons::gear(&painter, gear_c, 16.0, ICON_DIM);
        if gear.clicked() {
            self.reload_config(&ctx);
        }
        gear.on_hover_text("Recargar configuración (capyro.lua)");

        let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let fs_c = pos2(right - 8.0, by);
        let fsb = Self::icon_button(ui, "fullscreen", fs_c, 26.0);
        icons::fullscreen(&painter, fs_c, 14.0, ICON_DIM, fs);
        if fsb.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fs));
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

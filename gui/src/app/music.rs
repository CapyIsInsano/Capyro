//! Vista de música (interfaz adaptativa, v0.3.2).
//!
//! Con un audio, Capyro deja el área de video y se arma así:
//!
//!   ┌ Lista ────────┐ ┌──── [FLAC] [123 ms] ───┐ ┌ Ajustes · Info · Novedades ┐
//!   │ Cola · 12     │ │      ┌──────────┐       │ │                            │
//!   │  1 tema.mp3   │ │      │ carátula │       │ │  (el menú elegido, fijo,   │
//!   │  2 otro.flac  │ │      └──────────┘       │ │   con scroll)              │
//!   │               │ │  Título · Artista       │ │                            │
//!   │               │ │  Álbum                  │ │                            │
//!   │               │ │  ━━━━━━━━━○──────       │ │                            │
//!   │               │ │  ⤮  ⏮  ↺  ▶  ⏭  ↻       │ │                            │
//!   │ [+ Agregar]   │ │  🔊 ━━━━━━━━━━  80 %    │ │                            │
//!   └───────────────┘ │  Efectos ›  [presets]   │ └────────────────────────────┘
//!                     └─────────────────────────┘
//!
//! Si la ventana no tiene lugar para las tres columnas (o la opción
//! `music_view` está apagada, o corre el tutorial) queda la vista clásica.
//! Todo reutiliza las mismas piezas que la vista clásica (barra, controles,
//! menús), así que no hay dos versiones de cada cosa que mantener.

use eframe::egui::{
    self, epaint::RectShape, pos2, vec2, Align2, Color32, CursorIcon, FontId, Id, Rect, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, Vec2,
};

use super::{animate_button, ease_out, fit_text, CapyroApp, FRAME_SMOOTH};
use crate::ffi::State;
use crate::icons;
use crate::theme::*;

/// Ancho mínimo (sin márgenes) para las tres columnas.
const MIN_WIDTH: f32 = 736.0;
/// Alto mínimo debajo del header.
const MIN_HEIGHT: f32 = 430.0;
const GAP: f32 = 12.0;

pub(super) struct MusicLayout {
    pub left: Rect,
    pub center: Rect,
    pub right: Rect,
}

/// Pestaña de la burbuja de funciones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum DockTab {
    #[default]
    Settings,
    Info,
    News,
}

impl DockTab {
    const ALL: [DockTab; 3] = [DockTab::Settings, DockTab::Info, DockTab::News];

    fn label(self) -> &'static str {
        match self {
            DockTab::Settings => "Ajustes",
            DockTab::Info => "Info",
            DockTab::News => "Novedades",
        }
    }
}

/// Las tres columnas para `area` (todo lo que hay debajo del header), o
/// None si no entran.
pub(super) fn layout_for(area: Rect) -> Option<MusicLayout> {
    if area.width() < MIN_WIDTH || area.height() < MIN_HEIGHT {
        return None;
    }
    let side = ((area.width() - 2.0 * GAP) * 0.29).clamp(230.0, 330.0);
    let left = Rect::from_min_max(area.min, pos2(area.left() + side, area.bottom()));
    let right = Rect::from_min_max(pos2(area.right() - side, area.top()), area.max);
    let center = Rect::from_min_max(pos2(left.right() + GAP, area.top()), pos2(right.left() - GAP, area.bottom()));
    Some(MusicLayout { left, center, right })
}

impl CapyroApp {
    /// La vista de música completa. `appear` (0..1) es la entrada al pasar
    /// de un video (o de la vista clásica) a un audio.
    pub(super) fn music_view(&mut self, ui: &mut Ui, l: &MusicLayout, appear: f32) {
        // (acá no hay un área de video opaca que tape la luz de Alto)
        self.ambient_under = None;
        let slide = 14.0 * (1.0 - appear);
        self.playlist_dock(ui, l.left.translate(vec2(-slide, 0.0)));
        self.functions_dock(ui, l.right.translate(vec2(slide, 0.0)));
        self.music_center(ui, l.center.translate(vec2(0.0, slide * 0.6)));
    }

    /// Columna del medio: carátula, datos del tema, barra, controles,
    /// volumen y efectos.
    fn music_center(&mut self, ui: &mut Ui, area: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();
        let accent = self.cfg.accent;
        let now = ctx.input(|i| i.time);
        let anim = crate::perf::animations();
        let alto = crate::alto::on();
        let cx = area.center().x;

        // ¿qué se muestra? (de eso dependen las alturas)
        let artist = self.np.artist.trim().to_owned();
        let album = self.np.album.trim().to_owned();
        let viz_on = self.cfg.visualizer && anim;
        let badges = self.cfg.show_format || self.cfg.show_millis;
        let badge_h = 30.0;
        let viz_h = if viz_on { 30.0 } else { 0.0 };
        let text_h = 26.0 + if artist.is_empty() { 0.0 } else { 20.0 } + if album.is_empty() { 0.0 } else { 18.0 };
        let fixed = 6.0 + badge_h + 14.0 + viz_h + text_h + 12.0 + 36.0 + 8.0 + 54.0 + 8.0 + 32.0 + 14.0 + 54.0 + 6.0;
        let side = (area.height() - fixed).min(area.width() - 36.0).clamp(90.0, 340.0);
        // la carátula "un poco arriba": del lugar que sobra, un tercio arriba
        let extra = (area.height() - fixed - side).max(0.0);
        let mut y = area.top() + 6.0;

        /* --- arriba: formato y milisegundos; a la derecha, pantalla completa --- */
        if badges {
            self.music_badges(ui, &painter, pos2(area.left() + 2.0, y + 12.0));
        }
        let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let fs_c = pos2(area.right() - 14.0, y + 12.0);
        let fsb = Self::icon_button(ui, "music-fullscreen", fs_c, 26.0);
        icons::fullscreen(&painter, fs_c, 14.0 * fsb.scale(), fsb.tint(ICON_DIM, Color32::WHITE), fs);
        if fsb.resp.clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fs));
        }
        fsb.resp.on_hover_text("Pantalla completa (F)");
        y += badge_h + extra * 0.33;

        /* --- la carátula --- */
        // al cambiar de tema, entra (Medio y Alto; en Bajo aparece directo)
        let enter = match self.track_at {
            Some(t0) if anim => ((now - t0) / 0.55).clamp(0.0, 1.0) as f32,
            _ => 1.0,
        };
        if enter < 1.0 {
            ctx.request_repaint_after(FRAME_SMOOTH);
        }
        let ce = ease_out(enter);
        let energy = if alto && self.core.state() == State::Playing { self.viz.energy() } else { 0.0 };
        let cover = Rect::from_center_size(pos2(cx, y + side / 2.0), Vec2::splat(side));
        let shown = Rect::from_center_size(cover.center(), cover.size() * (0.94 + 0.06 * ce) * (1.0 + 0.012 * energy));
        let radius = (side * 0.07).clamp(8.0, 18.0);
        // sombra (en Bajo, una sola capa)
        if anim {
            soft_shadow(&painter, shown, radius, 20.0, 6.0, 110);
        } else {
            soft_shadow(&painter, shown, radius, 8.0, 3.0, 80);
        }
        if alto && energy > 0.01 {
            crate::alto::glow_rect(&painter, shown, accent, 0.5 * energy);
        }
        let mut image_rect = None;
        if let Some(tex) = &self.video_tex {
            // la imagen entera adentro del cuadrado (hay carátulas que no son cuadradas)
            let a = self.video_aspect.max(0.05);
            let size = if a >= 1.0 { vec2(shown.width(), shown.width() / a) } else { vec2(shown.height() * a, shown.height()) };
            let img = Rect::from_center_size(shown.center(), size);
            if (a - 1.0).abs() > 0.02 {
                painter.rect_filled(shown, radius, BG_VIDEO);
            }
            let tint = Color32::WHITE.gamma_multiply(0.25 + 0.75 * ce);
            painter.add(RectShape::filled(img, radius, tint).with_texture(tex.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))));
            image_rect = Some(img);
        } else {
            // sin carátula: un cuadro con el color de acento y una nota
            painter.rect_filled(shown, radius, BG_PANEL);
            painter.rect_filled(shown, radius, accent.gamma_multiply(0.10 + 0.04 * ce));
            painter.rect_stroke(shown, radius, Stroke::new(1.0, accent.gamma_multiply(0.25)), StrokeKind::Inside);
            painter.circle_filled(shown.center(), side * 0.2, accent.gamma_multiply(0.16));
            icons::music_note(&painter, shown.center(), side * 0.22, accent);
        }
        if alto && image_rect.is_some() {
            self.ambient.source = image_rect; // la luz de Alto sale de la carátula
        }
        self.ui_rects.insert("video", cover);
        y = cover.bottom() + 14.0;

        /* --- visualizador, debajo de la carátula --- */
        if viz_on {
            if !self.viz.is_silent() {
                let band = Rect::from_min_max(pos2(cover.left(), y), pos2(cover.right(), y + 22.0));
                self.viz.draw(&painter, band, accent.gamma_multiply(0.5));
            }
            y += viz_h;
        }

        /* --- título, artista y álbum (entran subiendo al cambiar de tema) --- */
        let max_w = area.width() - 12.0;
        let ke = |delay: f32| ease_out(((enter - delay) / (1.0 - delay)).clamp(0.0, 1.0));
        let lines: [(String, FontId, Color32, f32, f32); 3] = [
            (self.np.title.clone(), bold(17.0), TEXT, 26.0, 0.1),
            (artist, regular(13.0), TEXT_DIM, 20.0, 0.2),
            (album, regular(12.0), TEXT_FAINT, 18.0, 0.3),
        ];
        for (text, font, col, h, delay) in lines {
            if text.is_empty() {
                continue;
            }
            let k = ke(delay);
            let shown = fit_text(&painter, &text, font.clone(), max_w);
            let cut = shown != text;
            let r = painter.text(pos2(cx, y + h / 2.0 + 8.0 * (1.0 - k)), Align2::CENTER_CENTER, shown, font, col.gamma_multiply(k));
            if cut {
                ui.interact(r, Id::unique(("capyro-music-line", delay.to_bits())), Sense::hover()).on_hover_text(text);
            }
            y += h;
        }
        y += 12.0;

        /* --- barra de progreso y tiempos --- */
        let (bl, br) = (area.left() + 8.0, area.right() - 8.0);
        self.seek_bar(ui, &painter, bl, br, y + 6.0, y + 26.0);
        y += 36.0 + 8.0;

        /* --- controles --- */
        let spread = ((area.width() / 2.0 - 24.0) / 105.0).min(1.0);
        self.control_row(ui, &painter, cx, y + 27.0, spread);
        y += 54.0 + 8.0;

        /* --- volumen --- */
        let row = Rect::from_min_max(pos2(bl, y), pos2(br, y + 32.0));
        self.volume_row(ui, &painter, row);
        y += 32.0 + 14.0;

        /* --- efectos de audio --- */
        painter.line_segment([pos2(bl, y - 7.0), pos2(br, y - 7.0)], Stroke::new(1.0, Color32::from_white_alpha(12)));
        let fx = Rect::from_min_max(pos2(bl, y), pos2(br, y + 54.0));
        self.fx_compact(ui, &painter, fx);
    }

    /// Formato y milisegundos en una fila (en la vista de música no hay un
    /// área de video donde ponerlos uno abajo del otro).
    fn music_badges(&self, ui: &Ui, painter: &egui::Painter, start: egui::Pos2) {
        let accent = self.cfg.accent;
        let mut x = start.x;
        let cy = start.y;
        const H: f32 = 24.0;
        if self.cfg.show_format {
            let ext = std::path::Path::new(&self.np.file)
                .extension()
                .map(|e| e.to_string_lossy().to_uppercase())
                .unwrap_or_default();
            if !ext.is_empty() {
                let eg = painter.layout_no_wrap(ext, bold(11.5), Color32::WHITE);
                let r = Rect::from_min_size(pos2(x, cy - H / 2.0), vec2(eg.size().x + 24.0, H));
                painter.rect_filled(r, 12.0, Color32::from_black_alpha(110));
                let er = Rect::from_center_size(r.center(), vec2(eg.size().x + 16.0, 16.0));
                painter.rect_filled(er, 8.0, accent);
                painter.galley(er.center() - eg.size() / 2.0, eg, Color32::WHITE);
                x = r.right() + 6.0;
            }
        }
        if self.cfg.show_millis {
            let ms = ((self.core.position().max(0.0).fract()) * 1000.0) as u32 % 1000;
            let ng = painter.layout_no_wrap(format!("{ms:03}"), FontId::monospace(12.0), Color32::WHITE);
            let nw = painter.layout_no_wrap("000".to_owned(), FontId::monospace(12.0), Color32::WHITE).size().x;
            let ug = painter.layout_no_wrap("ms".to_owned(), regular(11.0), TEXT_DIM);
            let r = Rect::from_min_size(pos2(x, cy - H / 2.0), vec2(16.0 + nw + 5.0 + ug.size().x + 12.0, H));
            painter.rect_filled(r, 12.0, Color32::from_black_alpha(110));
            let beat = 1.0 - ms as f32 / 1000.0;
            painter.circle_filled(pos2(r.left() + 9.0, cy), 2.5, accent.gamma_multiply(0.35 + 0.65 * beat));
            painter.galley(pos2(r.left() + 16.0, cy - ng.size().y / 2.0), ng, Color32::WHITE);
            painter.galley(pos2(r.left() + 16.0 + nw + 5.0, cy - ug.size().y / 2.0), ug, TEXT_DIM);
            if self.core.state() == State::Playing {
                ui.ctx().request_repaint_after(crate::perf::ambient());
            }
        }
    }

    /// Volumen: el parlante (clic = silenciar), una barra para arrastrar y
    /// el porcentaje. La rueda sobre toda la fila también lo cambia.
    fn volume_row(&mut self, ui: &mut Ui, painter: &egui::Painter, row: Rect) {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;
        let cy = row.center().y;
        let (volume, muted) = (self.core.volume(), self.core.muted());

        let vol_c = pos2(row.left() + 12.0, cy);
        self.ui_rects.insert("volume", Rect::from_center_size(vol_c, Vec2::splat(28.0)));
        let vol = Self::icon_button(ui, "music-volume", vol_c, 26.0);
        icons::volume(painter, vol_c, 16.0 * vol.scale(), vol.tint(ICON_DIM, Color32::WHITE), volume, muted);
        if vol.resp.clicked() {
            self.toggle_mute(&ctx);
        }
        vol.resp.on_hover_text(if muted { "Activar el sonido (M)" } else { "Silenciar (M)" });

        let track = Rect::from_min_max(pos2(vol_c.x + 22.0, cy - 2.0), pos2(row.right() - 46.0, cy + 2.0));
        let id = Id::unique("capyro-music-volume-bar");
        let resp = ui.interact(track.expand2(vec2(6.0, 10.0)), id, Sense::click_and_drag());
        let active = resp.hovered() || resp.dragged();
        let grow = ctx.animate_bool_with_time(id.with("grow"), active, crate::perf::t(0.12));
        if active {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.dragged() || resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let v = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
                // de a 1 %, como los atajos
                self.core.set_volume((v * 100.0).round() / 100.0);
                if muted {
                    self.core.set_muted(false);
                }
            }
        }
        let shown = if muted { 0.0 } else { self.core.volume() };
        let th = 4.0 + 2.0 * grow;
        let t = Rect::from_center_size(track.center(), vec2(track.width(), th));
        painter.rect_filled(t, th / 2.0, TRACK);
        let fill_x = t.left() + t.width() * shown;
        painter.rect_filled(Rect::from_min_max(t.min, pos2(fill_x, t.bottom())), th / 2.0, if muted { TEXT_FAINT } else { accent });
        if crate::alto::on() {
            crate::alto::glow(painter, pos2(fill_x, cy), 14.0 + 8.0 * grow, accent, 0.25 * grow);
        }
        painter.circle_filled(pos2(fill_x, cy), 5.0 + 2.0 * grow, Color32::WHITE);
        let label = if muted { "Mudo".to_owned() } else { format!("{:.0} %", self.core.volume() * 100.0) };
        painter.text(pos2(row.right(), cy), Align2::RIGHT_CENTER, label, regular(11.5), if muted { TEXT_FAINT } else { TEXT_DIM });

        self.wheel_volume(&ctx, ui.rect_contains_pointer(row));
    }

    /// Burbuja de funciones: Ajustes, Info (con los formatos ya abiertos) y
    /// Novedades, en pestañas, con scroll si no entra.
    fn functions_dock(&mut self, ui: &mut Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();
        let accent = self.cfg.accent;
        ui.interact(rect, Id::unique("capyro-functions-dock-bg"), Sense::click()); // que no atraviese
        crate::theme::soft_shadow(&painter, rect, 12.0, 6.0, 1.5, 50);
        painter.rect_filled(rect, 12.0, BG_PANEL);
        painter.rect_stroke(rect, 12.0, Stroke::new(1.0, Color32::from_white_alpha(12)), StrokeKind::Inside);

        /* --- pestañas --- */
        let unseen = self.changes_seen != crate::ffi::version();
        let updating = self.updater.is_running() || matches!(self.updater.last, Some(crate::updater::Outcome::Installed { .. }));
        let tabs = Rect::from_min_max(rect.min + vec2(8.0, 8.0), pos2(rect.right() - 8.0, rect.top() + 52.0));
        // debajo de las pestañas, a todo el ancho: el botón de Capyro Studio
        let studio_r = Rect::from_min_size(pos2(tabs.left(), tabs.bottom() + 6.0), vec2(tabs.width(), 32.0));
        if super::studio::studio_button(ui, "capyro-studio-btn", studio_r, accent, self.studio.is_some()) {
            self.open_studio(&ctx);
        }
        let tw = tabs.width() / DockTab::ALL.len() as f32;
        for (i, tab) in DockTab::ALL.into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(tabs.left() + i as f32 * tw, tabs.top()), vec2(tw, tabs.height())).shrink2(vec2(2.0, 0.0));
            let a = animate_button(ui, Id::unique(("capyro-dock-tab", i)), r, true);
            let selected = self.dock_tab == tab;
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-dock-tab-sel", i)), selected, crate::perf::t(0.15));
            let bg = Color32::from_white_alpha((6.0 + 10.0 * a.hover) as u8).lerp_to_gamma(accent.gamma_multiply(0.22), sel);
            painter.rect_filled(r, 9.0, bg);
            let col = TEXT_DIM.lerp_to_gamma(TEXT, a.hover).lerp_to_gamma(accent, sel);
            let ic = pos2(r.center().x, r.top() + 15.0);
            match tab {
                DockTab::Settings => icons::gear(&painter, ic, 15.0 * a.scale(), col),
                DockTab::Info => icons::info(&painter, ic, 16.0 * a.scale(), col),
                DockTab::News => icons::news(&painter, ic, 16.0 * a.scale(), col),
            }
            painter.text(pos2(r.center().x, r.bottom() - 11.0), Align2::CENTER_CENTER, tab.label(), if selected { bold(10.5) } else { regular(10.5) }, col);
            // puntitos: actualización (Info) y novedades sin ver
            let dot = match tab {
                DockTab::Info => updating,
                DockTab::News => unseen,
                DockTab::Settings => false,
            };
            if dot {
                painter.circle_filled(ic + vec2(11.0, -7.0), 3.0, accent);
            }
            if a.resp.clicked() {
                self.dock_tab = tab;
                if tab == DockTab::News && unseen {
                    self.changes_seen = crate::ffi::version();
                    self.save_session();
                }
            }
        }
        let sep_y = studio_r.bottom() + 6.0;
        painter.line_segment(
            [pos2(rect.left() + 10.0, sep_y), pos2(rect.right() - 10.0, sep_y)],
            Stroke::new(1.0, Color32::from_white_alpha(12)),
        );

        /* --- contenido (recortado, con scroll) --- */
        let content = Rect::from_min_max(pos2(rect.left(), sep_y + 2.0), pos2(rect.right(), rect.bottom() - 6.0));
        let mut child = ui.new_child(UiBuilder::new().max_rect(content));
        child.set_clip_rect(content.intersect(ui.clip_rect()));
        let idx = self.dock_tab as usize;
        let hovered = ui.rect_contains_pointer(content);
        if hovered && self.dock_tab != DockTab::News {
            let d = ctx.input(|i| i.smooth_scroll_delta.y);
            if d != 0.0 {
                self.dock_scroll[idx] -= d;
            }
        }
        let off = self.dock_scroll[idx].max(0.0);
        let at = Rect::from_min_size(content.min - vec2(0.0, off), content.size());
        let h = match self.dock_tab {
            DockTab::Settings => self.settings_menu(&mut child, Some(at)),
            DockTab::Info => self.info_menu(&mut child, Some(at)),
            DockTab::News => self.changes_menu(&mut child, Some(content)),
        };
        let max = (h - content.height()).max(0.0);
        self.dock_scroll[idx] = self.dock_scroll[idx].clamp(0.0, max);
        if max > 0.5 {
            // barrita de scroll y un degradé que avisa que hay más
            let track = Rect::from_min_max(pos2(rect.right() - 6.0, content.top() + 4.0), pos2(rect.right() - 3.0, content.bottom() - 4.0));
            let thumb_h = (track.height() * content.height() / h).max(24.0);
            let ty = track.top() + (track.height() - thumb_h) * (self.dock_scroll[idx] / max);
            painter.rect_filled(Rect::from_min_size(pos2(track.left(), ty), vec2(track.width(), thumb_h)), 1.5, Color32::from_white_alpha(if hovered { 60 } else { 30 }));
            if self.dock_scroll[idx] < max - 1.0 {
                fade_edge(&painter, Rect::from_min_max(pos2(content.left() + 1.0, content.bottom() - 22.0), pos2(content.right() - 8.0, content.bottom())), false);
            }
            if self.dock_scroll[idx] > 1.0 {
                fade_edge(&painter, Rect::from_min_max(pos2(content.left() + 1.0, content.top()), pos2(content.right() - 8.0, content.top() + 16.0)), true);
            }
        }
    }
}

/// Degradé del color del panel (arriba o abajo de un contenido con scroll).
fn fade_edge(painter: &egui::Painter, r: Rect, top: bool) {
    let mut mesh = egui::Mesh::default();
    let (solid, clear) = (BG_PANEL, Color32::TRANSPARENT);
    let (ct, cb) = if top { (solid, clear) } else { (clear, solid) };
    mesh.colored_vertex(r.left_top(), ct);
    mesh.colored_vertex(r.right_top(), ct);
    mesh.colored_vertex(r.left_bottom(), cb);
    mesh.colored_vertex(r.right_bottom(), cb);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columnas() {
        // ventana chica: vista clásica
        assert!(layout_for(Rect::from_min_size(pos2(12.0, 70.0), vec2(600.0, 560.0))).is_none());
        assert!(layout_for(Rect::from_min_size(pos2(12.0, 70.0), vec2(876.0, 300.0))).is_none());
        // la de siempre (900 de ancho): tres columnas que no se pisan
        let l = layout_for(Rect::from_min_size(pos2(12.0, 70.0), vec2(876.0, 560.0))).unwrap();
        assert!(l.left.right() + GAP <= l.center.left() + 0.01);
        assert!(l.center.right() + GAP <= l.right.left() + 0.01);
        assert!(l.center.width() >= 250.0, "{}", l.center.width());
        assert!((l.left.width() - l.right.width()).abs() < 0.01);
        // pantalla grande: los costados no crecen de más
        let l = layout_for(Rect::from_min_size(pos2(0.0, 0.0), vec2(1900.0, 1000.0))).unwrap();
        assert!(l.left.width() <= 330.0 && l.center.width() > 1000.0);
    }
}

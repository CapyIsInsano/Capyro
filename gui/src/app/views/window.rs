//! La ventana en sí: el encabezado (logo, título, minimizar/maximizar/cerrar),
//! los bordes para agrandarla, la pantalla de carga y los avisos (toasts).

use crate::app::*;

/// Mac: lo que ocupan los tres botones de colores del sistema (cerrar,
/// minimizar, pantalla completa) arriba a la izquierda. El logo va después.
pub(crate) const TRAFFIC_LIGHTS: f32 = if cfg!(target_os = "macos") { 64.0 } else { 0.0 };

pub(crate) struct Toast {
    text: String,
    start: f64,
    until: f64,
    error: bool,
}

impl CapyroApp {
    pub(crate) fn header(&mut self, ui: &mut Ui, rect: Rect, maximized: bool) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();

        // Zona para arrastrar la ventana (todo el header menos los botones;
        // en la Mac los botones son los del sistema, a la izquierda).
        let buttons_w = if cfg!(target_os = "macos") { 0.0 } else { 80.0 };
        let drag_rect = Rect::from_min_max(rect.min, pos2(rect.right() - buttons_w, rect.bottom()));
        let drag = ui.interact(drag_rect, Id::unique("capyro-titlebar"), Sense::click_and_drag());
        if drag.double_clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        } else if drag.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        // Logo: al tocarlo suena un capibara y rebota.
        let logo_rect = Rect::from_min_size(rect.min + vec2(TRAFFIC_LIGHTS, 0.0), vec2(46.0, 46.0));
        self.ui_rects.insert("logo", logo_rect);
        let logo_btn = animate_button(ui, Id::unique("capyro-logo"), logo_rect, true);
        let now = ctx.input(|i| i.time);
        if logo_btn.resp.clicked() {
            self.play_capybara();
            self.logo_click = Some(now);
        }
        let mut bounce = 0.0;
        if let Some(t0) = self.logo_click {
            let dt = (now - t0) as f32;
            if dt < 0.7 && crate::system::perf::animations() {
                bounce = (-dt * 7.0).exp() * (dt * 22.0).sin();
                ctx.request_repaint_after(FRAME_SMOOTH);
            } else {
                self.logo_click = None;
            }
        }
        let logo_scale = 1.0 + 0.05 * logo_btn.hover - 0.08 * logo_btn.press + 0.14 * bounce;
        if let Some(logo) = self.logo.as_ref().filter(|_| !self.hide_header_logo) {
            painter.image(
                logo.id(),
                Rect::from_center_size(logo_rect.center(), logo_rect.size() * logo_scale),
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

        // Mac: minimizar, maximizar y cerrar son los del sistema
        if cfg!(target_os = "macos") {
            return;
        }
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

    /// Pantalla de carga: logo, "Cargando…" y un ecualizador. Con `intro`
    /// (segundos desde que terminó de cargar) todo se desvanece y el logo
    /// vuela hasta su lugar en el header.
    pub(crate) fn splash(&self, ui: &Ui, full: Rect, now: f64, intro: Option<f32>, logo_dest: Rect) {
        let painter = ui.painter();
        let accent = self.cfg.accent;
        let since = (now - self.boot.start.unwrap_or(now)) as f32;
        let appear = ease_out(since / 0.3);
        let fade = intro.map_or(1.0, |t| 1.0 - ease_out(t / 0.28));

        let c = full.center() - vec2(0.0, 18.0);
        let logo_c = c - vec2(0.0, 30.0);
        let mut p = painter.clone();
        p.multiply_opacity(appear * fade);

        // brillo que respira detrás del logo
        let breathe = 0.5 + 0.5 * (since * 2.6).sin();
        p.circle_filled(logo_c, 70.0 + 8.0 * breathe, accent.gamma_multiply(0.05 + 0.03 * breathe));
        p.circle_filled(logo_c, 56.0, accent.gamma_multiply(0.06));

        p.text(c + vec2(0.0, 42.0), Align2::CENTER_CENTER, "Capyro", bold(21.0), TEXT);

        // "Cargando" fijo y los puntitos al lado (así el texto no baila)
        let g = p.layout_no_wrap("Cargando".to_owned(), regular(13.0), TEXT_DIM);
        let dots = (since * 3.2) as usize % 4;
        let tl = pos2(c.x - g.size().x / 2.0 - 6.0, c.y + 68.0 - g.size().y / 2.0);
        p.galley(tl, g.clone(), TEXT_DIM);
        p.text(pos2(tl.x + g.size().x + 1.0, c.y + 68.0), Align2::LEFT_CENTER, ".".repeat(dots), regular(13.0), TEXT_DIM);

        // ecualizador, como el del README
        let n = 7;
        let (bw, gap) = (4.0, 4.0);
        let total_w = n as f32 * bw + (n - 1) as f32 * gap;
        let base = c.y + 104.0;
        for i in 0..n {
            let ph = since * 7.0 + i as f32 * 1.1;
            let h = 5.0 + 15.0 * (0.5 + 0.5 * ph.sin()) * (0.6 + 0.4 * (ph * 0.37).cos().abs());
            let x = c.x - total_w / 2.0 + i as f32 * (bw + gap);
            p.rect_filled(Rect::from_min_max(pos2(x, base - h), pos2(x + bw, base)), 2.0, accent);
        }

        p.text(
            pos2(full.center().x, full.bottom() - 26.0),
            Align2::CENTER_CENTER,
            format!("v{} · Just press play.", ffi::version()),
            regular(11.0),
            TEXT_FAINT,
        );

        // el logo: aparece creciendo y, al terminar, vuela al header
        let Some(logo) = &self.logo else { return };
        let start_rect = Rect::from_center_size(logo_c, Vec2::splat(96.0 * (0.9 + 0.1 * appear)));
        let (rect, alpha) = match intro {
            None => (start_rect, appear),
            Some(t) => {
                let k = ease_in_out(t / 0.46);
                let r = Rect::from_min_max(start_rect.min.lerp(logo_dest.min, k), start_rect.max.lerp(logo_dest.max, k));
                (r, 1.0)
            }
        };
        let mut lp = painter.clone();
        lp.multiply_opacity(alpha);
        lp.image(logo.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }

    /// Ventana sin bordes del sistema → bordes de 6px para redimensionar.
    /// (En la Mac la ventana tiene el marco del sistema: se agranda sola.)
    pub(crate) fn resize_handles(ui: &mut Ui, full: Rect) {
        if cfg!(target_os = "macos") {
            return;
        }
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

    pub(crate) fn toast(&mut self, ctx: &egui::Context, text: impl Into<String>, error: bool) {
        self.toast_for(ctx, text, error, 2.5);
    }

    pub(crate) fn toast_for(&mut self, ctx: &egui::Context, text: impl Into<String>, error: bool, secs: f64) {
        let now = ctx.input(|i| i.time);
        self.toast = Some(Toast { text: ellipsize(&text.into(), 84), start: now, until: now + secs, error });
    }

    /* ---------------------------------------------------------------- */
    /* actualizaciones                                                  */
    /* ---------------------------------------------------------------- */

    pub(crate) fn draw_toast(&mut self, ui: &mut Ui, video: Rect) {
        let now = ui.ctx().input(|i| i.time);
        let Some(t) = &self.toast else { return };
        if now > t.until {
            self.toast = None;
            return;
        }
        // Aparece subiendo un poco y se desvanece al final.
        let fade_in = crate::system::perf::k(((now - t.start) / 0.15).clamp(0.0, 1.0) as f32);
        let fade_out = crate::system::perf::k(((t.until - now) / 0.35).clamp(0.0, 1.0) as f32);
        let alpha = fade_in.min(fade_out);
        let rise = (1.0 - fade_in) * 8.0;

        let mut painter = ui.painter().clone();
        painter.multiply_opacity(alpha);
        let galley = painter.layout_no_wrap(t.text.clone(), regular(12.5), if t.error { ERROR } else { TEXT });
        let size = galley.size() + vec2(28.0, 14.0);
        let rect = Rect::from_center_size(pos2(video.center().x, video.bottom() - 28.0 + rise), size);
        painter.rect_filled(rect, size.y / 2.0, BG_PANEL);
        painter.rect_stroke(rect, size.y / 2.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
        painter.galley(rect.center() - galley.size() / 2.0, galley, TEXT);
        // frames solo mientras entra o se va; quieto, hasta que empiece a irse
        if fade_in < 1.0 || fade_out < 1.0 {
            ui.ctx().request_repaint_after(FRAME_SMOOTH);
        } else {
            // (en Bajo no se desvanece: se va justo al final)
            let fade_at = if crate::system::perf::animations() { t.until - 0.35 } else { t.until };
            ui.ctx().request_repaint_after(Duration::from_secs_f64((fade_at - now).max(0.0)));
        }
    }
}

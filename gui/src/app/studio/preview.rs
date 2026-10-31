//! Vista previa de un tramo: barra con dos manijas, tocar y repetir.

use super::*;

fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

impl Studio {
    /// Reproduce el tramo desde su inicio.
    pub(super) fn play_preview(&mut self, core: &Core, now: f64) {
        if self.end <= self.start + MIN_SEGMENT * 0.5 {
            self.say(now, "El fin tiene que estar después del inicio", true);
            return;
        }
        self.preview = true;
        core.seek(self.start);
        if core.state() != State::Playing {
            core.toggle();
        }
    }

    /// Vigila el tramo mientras suena (se llama en cada cuadro).
    pub(super) fn tick_preview(&mut self, core: &Core) {
        if !self.preview {
            return;
        }
        if core.state() != State::Playing {
            self.preview = false;
            return;
        }
        if core.position() >= self.end {
            if self.repeat_preview {
                core.seek(self.start);
            } else {
                self.preview = false;
                core.toggle(); // pausa al llegar al fin del tramo
            }
        }
    }

    /// Vista previa de un tramo: barra con dos manijas, tocar y repetir.
    pub(super) fn preview_section(&mut self, ui: &mut Ui, core: &Core, accent: Color32, now: f64) {
        let w = ui.available_width();
        if self.saving.is_none() {
            self.duration = core.duration().max(0.0);
        }
        let duration = self.duration;
        let (head, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
        let painter = ui.painter().clone();
        painter.text(head.left_center(), Align2::LEFT_CENTER, "Vista previa del tramo", bold(13.0), TEXT);
        painter.text(
            head.right_center(),
            Align2::RIGHT_CENTER,
            "el archivo no cambia",
            regular(10.5),
            TEXT_FAINT,
        );
        if duration <= 0.0 {
            let (r, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
            painter.text(r.left_center(), Align2::LEFT_CENTER, "La duración no es conocida: no se puede elegir el tramo.", regular(12.0), TEXT_DIM);
            return;
        }
        self.start = self.start.clamp(0.0, duration);
        self.end = self.end.clamp(0.0, duration);
        if self.end <= self.start {
            self.end = (self.start + MIN_SEGMENT).min(duration);
        }

        /* --- la barra --- */
        ui.add_space(6.0);
        let (bar, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
        let track = Rect::from_min_max(pos2(bar.left() + 8.0, bar.center().y - 3.0), pos2(bar.right() - 8.0, bar.center().y + 3.0));
        let x_of = |s: f64| track.left() + (s / duration) as f32 * track.width();
        let s_of = |x: f32| (((x - track.left()) / track.width()).clamp(0.0, 1.0) as f64) * duration;
        let id = Id::unique("capyro-studio-range");
        let resp = ui.interact(bar, id, Sense::click_and_drag());
        let ctx = ui.ctx().clone();
        let hover = ctx.animate_bool_with_time(id.with("h"), resp.hovered() || resp.dragged(), crate::system::perf::t(0.12));
        if resp.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        let nearest = |x: f32, start: f64, end: f64| -> u8 {
            if (x - x_of(start)).abs() <= (x - x_of(end)).abs() { 0 } else { 1 }
        };
        if resp.double_clicked() {
            self.start = 0.0;
            self.end = duration;
        } else if let Some(p) = resp.interact_pointer_pos() {
            if resp.drag_started() {
                self.drag_handle = Some(nearest(p.x, self.start, self.end));
            }
            let handle = if resp.clicked() { Some(nearest(p.x, self.start, self.end)) } else { self.drag_handle };
            if resp.dragged() || resp.clicked() {
                let v = s_of(p.x);
                match handle {
                    Some(0) => self.start = v.min(self.end - MIN_SEGMENT).max(0.0),
                    Some(_) => self.end = v.max(self.start + MIN_SEGMENT).min(duration),
                    None => {}
                }
            }
        }
        if resp.drag_stopped() {
            self.drag_handle = None;
        }

        painter.rect_filled(track, 3.0, TRACK);
        let (xs, xe) = (x_of(self.start), x_of(self.end));
        let sel = Rect::from_min_max(pos2(xs, track.top()), pos2(xe, track.bottom()));
        if crate::ui::alto::on() {
            crate::ui::alto::glow_rect(&painter, sel, accent, 0.35 + 0.25 * hover);
        }
        painter.rect_filled(sel, 3.0, accent.gamma_multiply(0.85));
        // dónde está sonando
        let pos = core.position().clamp(0.0, duration);
        let xp = x_of(pos);
        painter.rect_filled(Rect::from_center_size(pos2(xp, track.center().y), vec2(2.0, 16.0)), 1.0, Color32::from_white_alpha(200));
        // manijas
        for (i, x) in [xs, xe].into_iter().enumerate() {
            let active = self.drag_handle == Some(i as u8);
            let hh = 20.0 + 4.0 * hover + if active { 2.0 } else { 0.0 };
            let r = Rect::from_center_size(pos2(x, track.center().y), vec2(8.0, hh));
            painter.rect_filled(r, 4.0, Color32::WHITE);
            painter.rect_stroke(r, 4.0, Stroke::new(1.5, accent), StrokeKind::Inside);
        }

        /* --- controles --- */
        ui.add_space(8.0);
        let (row, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
        let play_c = pos2(row.left() + 16.0, row.center().y);
        let play = animate_button(ui, Id::unique("capyro-studio-play"), Rect::from_center_size(play_c, vec2(32.0, 32.0)), true);
        if crate::ui::alto::on() {
            crate::ui::alto::glow(&painter, play_c, 30.0, accent, 0.25 * play.hover);
        }
        painter.circle_filled(play_c, 15.0 * (1.0 + 0.05 * play.hover - 0.08 * play.press), accent);
        if self.preview {
            painter.rect_filled(Rect::from_center_size(play_c, vec2(10.0, 10.0)), 2.0, Color32::WHITE);
        } else {
            icons::play_small(&painter, play_c + vec2(1.0, 0.0), 12.0, Color32::WHITE);
        }
        if play.resp.clicked() {
            if self.preview {
                self.preview = false;
                if core.state() == State::Playing {
                    core.toggle();
                }
            } else {
                self.play_preview(core, now);
            }
        }
        play.resp.on_hover_text(if self.preview { "Parar" } else { "Tocar solo este tramo" });
        let len = self.end - self.start;
        painter.text(
            pos2(row.left() + 42.0, row.center().y - 7.0),
            Align2::LEFT_CENTER,
            format!("{}  –  {}", fmt_time(self.start), fmt_time(self.end)),
            bold(12.5),
            TEXT,
        );
        painter.text(
            pos2(row.left() + 42.0, row.center().y + 8.0),
            Align2::LEFT_CENTER,
            format!("{} de tramo", if len < 60.0 { format!("{:.1} s", len) } else { fmt_time(len) }),
            regular(10.5),
            TEXT_FAINT,
        );

        // a la derecha: repetir + inicio/fin aquí + todo
        let mut x = row.right();
        let sw_c = pos2(x - 17.0, row.center().y);
        if switch(ui, &painter, "studio-repeat", sw_c, self.repeat_preview, accent, true) {
            self.repeat_preview = !self.repeat_preview;
        }
        x -= 40.0;
        painter.text(pos2(x, row.center().y), Align2::RIGHT_CENTER, "Repetir", regular(11.5), TEXT_DIM);
        x -= ui.painter().layout_no_wrap("Repetir".to_owned(), regular(11.5), TEXT).size().x + 16.0;
        let chips = [("start-here", "Inicio aquí"), ("end-here", "Fin aquí"), ("whole", "Todo")];
        let widths: Vec<f32> = chips.iter().map(|(_, t)| pill_w(ui, t)).collect();
        let total: f32 = widths.iter().sum::<f32>() + 6.0 * (chips.len() - 1) as f32;
        // a la derecha si entran; si no, en una fila más abajo
        let (mut cx, cy) = if x - total >= row.left() + 190.0 {
            (x - total, row.center().y)
        } else {
            let (row2, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
            (row2.left(), row2.center().y + 4.0)
        };
        for ((id, text), cw) in chips.into_iter().zip(widths) {
            let r = Rect::from_min_max(pos2(cx, cy - 12.0), pos2(cx + cw, cy + 12.0));
            cx += cw + 6.0;
            if pill(ui, id, r, text, false, 0.0, accent, true) {
                let p = core.position().clamp(0.0, duration);
                match id {
                    "whole" => {
                        self.start = 0.0;
                        self.end = duration;
                    }
                    "end-here" => self.end = p.max(self.start + MIN_SEGMENT).min(duration),
                    _ => self.start = p.min(self.end - MIN_SEGMENT).max(0.0),
                }
            }
        }
    }
}

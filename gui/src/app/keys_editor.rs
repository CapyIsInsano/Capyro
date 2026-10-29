//! Editor de la configuración (⚙ → Editar configuración): cada función con
//! sus teclas. Se quitan tocando la tecla, se agregan con "+" (y apretando la
//! combinación), y "Agregar atajo" ofrece las funciones que no tienen tecla.
//! Cada cambio se guarda en `~/.config/capyro/capyro.lua` al instante.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, Event, Id, Key, Modifiers, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, Vec2,
};

use super::{animate_button, CapyroApp};
use crate::config::{self, Action};
use crate::icons;
use crate::theme::*;

enum Op {
    Remove(Action, Modifiers, Key),
    RemoveAll(Action),
    Capture(Action),
    CancelCapture,
    ToggleAdding,
    Reset,
    Reload,
    Close,
}

impl CapyroApp {
    /// Mientras se espera una tecla: la toma, la asigna y se come todas las
    /// teclas del frame (para que no disparen atajos). Devuelve true si estaba
    /// esperando.
    pub(super) fn capture_key(&mut self, ctx: &egui::Context) -> bool {
        let Some(action) = self.keys_capture else { return false };
        let pressed = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                Event::Key { key, pressed: true, repeat: false, modifiers, .. } => Some((*key, *modifiers)),
                _ => None,
            })
        });
        ctx.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Key { .. } | Event::Text(_))));
        let Some((key, modifiers)) = pressed else { return true };
        self.keys_capture = None;
        if key == Key::Escape {
            return true; // Esc cancela
        }
        let combo = config::key_label(config::normalize_modifiers(modifiers), key);
        let stolen = self.cfg.assign(action, modifiers, key);
        let msg = match stolen {
            Some(prev) => format!("{combo} ahora es «{}» (antes era «{}»)", action.label(), prev.label()),
            None => format!("{combo} → {}", action.label()),
        };
        self.save_config(ctx, msg);
        true
    }

    pub(super) fn save_config(&mut self, ctx: &egui::Context, msg: String) {
        match self.cfg.save() {
            Ok(_) => self.toast(ctx, msg, false),
            Err(e) => {
                self.play_error();
                self.toast_for(ctx, format!("No pude guardar la configuración: {e}"), true, 4.0);
            }
        }
    }

    pub(super) fn keys_editor(&mut self, ui: &mut Ui, area: Rect) {
        let ctx = ui.ctx().clone();
        let t = ctx.animate_bool_with_time(Id::unique("capyro-keys-anim"), self.show_keys, crate::perf::t(0.18));
        if t <= 0.001 {
            self.keys_capture = None;
            self.keys_adding = false;
            return;
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let accent = self.cfg.accent;
        let now = ctx.input(|i| i.time);
        let size = vec2(area.width().min(560.0), area.height());
        let full = Rect::from_center_size(area.center(), size);
        let panel = Rect::from_center_size(area.center(), size * (0.96 + 0.04 * crate::alto::pop(t, ease)));
        let interactive = self.show_keys && t > 0.6;
        let mut ops: Vec<Op> = Vec::new();

        let mut painter = ui.painter().clone();
        painter.multiply_opacity(ease);
        if interactive {
            let outside = ui.interact(ui.max_rect(), Id::unique("capyro-keys-outside"), Sense::click());
            if outside.clicked() {
                ops.push(if self.keys_capture.is_some() { Op::CancelCapture } else { Op::Close });
            }
            ui.interact(full, Id::unique("capyro-keys-bg"), Sense::click());
        }
        // velo oscuro detrás, como un diálogo
        painter.rect_filled(ui.max_rect(), 0.0, Color32::from_black_alpha((90.0 * ease) as u8));
        crate::theme::soft_shadow(&painter, panel, 14.0, 12.0, 3.0, 90);
        painter.rect_filled(panel, 14.0, BG_PANEL);
        painter.rect_stroke(panel, 14.0, Stroke::new(1.0, Color32::from_white_alpha(16)), StrokeKind::Inside);
        if ease < 0.98 {
            ctx.request_repaint_after(super::FRAME_SMOOTH);
            return;
        }
        crate::alto::reveal(&ctx, Id::unique("capyro-keys-anim"), panel, 14.0, self.show_keys, BG_PANEL, accent);

        let x0 = panel.left() + 18.0;
        let x1 = panel.right() - 18.0;
        let top = panel.top();

        /* --- encabezado --- */
        let ic = pos2(x0 + 12.0, top + 30.0);
        painter.circle_filled(ic, 15.0, accent.gamma_multiply(0.16));
        icons::keyboard(&painter, ic, 18.0, accent);
        painter.text(pos2(x0 + 36.0, top + 22.0), Align2::LEFT_CENTER, "Editar configuración", bold(15.0), TEXT);
        let path = config::user_config_path().map(|p| crate::paths::tilde(&p)).unwrap_or_default();
        painter.text(
            pos2(x0 + 36.0, top + 40.0),
            Align2::LEFT_CENTER,
            format!("Atajos de teclado · se guardan solos en {path}"),
            regular(11.0),
            TEXT_FAINT,
        );
        let close_c = pos2(x1 - 6.0, top + 28.0);
        let close = Self::icon_button(ui, "keys-close", close_c, 26.0);
        icons::close(&painter, close_c, 12.0 * close.scale(), close.tint(ICON_DIM, Color32::WHITE));
        if close.resp.clicked() {
            ops.push(Op::Close);
        }

        // columnas
        let col_keys = x0 + 190.0;
        let hy = top + 70.0;
        painter.text(pos2(x0, hy), Align2::LEFT_CENTER, "FUNCIÓN", bold(10.0), TEXT_FAINT);
        painter.text(pos2(col_keys, hy), Align2::LEFT_CENTER, "TECLAS · tocá una para quitarla", bold(10.0), TEXT_FAINT);
        painter.line_segment([pos2(x0, hy + 12.0), pos2(x1, hy + 12.0)], Stroke::new(1.0, Color32::from_white_alpha(14)));

        /* --- filas --- */
        let footer_h = 58.0;
        let list = Rect::from_min_max(pos2(x0 - 8.0, hy + 16.0), pos2(x1 + 8.0, panel.bottom() - footer_h));
        let capture = self.keys_capture;
        let mut actions: Vec<Action> = Action::ALL
            .iter()
            .copied()
            .filter(|a| self.cfg.bindings.iter().any(|b| b.action == *a))
            .collect();
        // la que se está agregando todavía no tiene tecla: va al principio
        if let Some(a) = capture {
            if !actions.contains(&a) {
                actions.insert(0, a);
            }
        }
        let bindings = self.cfg.bindings.clone();
        let mut child = ui.new_child(UiBuilder::new().max_rect(list));
        if !interactive || self.keys_adding {
            child.disable();
        }
        ScrollArea::vertical().id_salt("capyro-keys-scroll").auto_shrink([false, false]).show(&mut child, |ui| {
            for action in &actions {
                let action = *action;
                let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
                let hovered = ui.is_enabled() && ui.rect_contains_pointer(row);
                let hover = ui.ctx().animate_bool_with_time(Id::unique(("capyro-keys-row", action.name())), hovered, crate::perf::t(0.12));
                let p = ui.painter().clone();
                let capturing = capture == Some(action);
                if capturing {
                    p.rect_filled(row, 8.0, accent.gamma_multiply(0.1));
                } else if hover > 0.001 {
                    p.rect_filled(row, 8.0, Color32::from_white_alpha((8.0 * hover) as u8));
                }
                let cy = row.center().y;
                p.text(pos2(row.left() + 8.0, cy), Align2::LEFT_CENTER, action.label(), regular(12.5), TEXT);

                // teclas
                let mut x = col_keys;
                for b in bindings.iter().filter(|b| b.action == action) {
                    let label = config::key_label(b.modifiers, b.key);
                    let g = p.layout_no_wrap(label, bold(11.5), TEXT);
                    let r = Rect::from_min_size(pos2(x, cy - 12.0), vec2(g.size().x + 18.0, 24.0));
                    let a = animate_button(ui, Id::unique(("capyro-key-chip", action.name(), config::key_spec(b.modifiers, b.key))), r, true);
                    let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.05 * a.press));
                    // tecla "física": borde de abajo más grueso
                    crate::theme::soft_shadow(&p, rr, 6.0, 6.0, 1.0, 70);
                    p.rect_filled(rr, 6.0, BG_WINDOW.lerp_to_gamma(ERROR, 0.25 * a.hover));
                    p.rect_stroke(rr, 6.0, Stroke::new(1.0, Color32::from_white_alpha(22).lerp_to_gamma(ERROR, a.hover)), StrokeKind::Inside);
                    p.galley(rr.center() - g.size() / 2.0 - vec2(4.0 * a.hover, 0.0), g, TEXT.lerp_to_gamma(Color32::WHITE, a.hover));
                    if a.hover > 0.05 {
                        icons::close(&p, pos2(rr.right() - 8.0, cy), 6.0, ERROR.gamma_multiply(a.hover));
                    }
                    if a.resp.clicked() {
                        ops.push(Op::Remove(action, b.modifiers, b.key));
                    }
                    a.resp.on_hover_text("Quitar esta tecla");
                    x = r.right() + 6.0;
                }

                // "+" o esperando tecla
                if capturing {
                    let pulse = 0.6 + 0.4 * ((crate::perf::clock(now) * 4.0).sin() as f32).abs();
                    let g = p.layout_no_wrap("Presioná una tecla…  (Esc cancela)".to_owned(), regular(11.5), Color32::WHITE);
                    let r = Rect::from_min_size(pos2(x, cy - 12.0), vec2(g.size().x + 20.0, 24.0));
                    p.rect_filled(r, 12.0, accent.gamma_multiply(pulse));
                    p.galley(r.center() - g.size() / 2.0, g, Color32::WHITE);
                    if crate::perf::animations() {
                        ui.ctx().request_repaint_after(super::FRAME_SMOOTH);
                    }
                } else {
                    let r = Rect::from_min_size(pos2(x, cy - 12.0), vec2(26.0, 24.0));
                    let a = animate_button(ui, Id::unique(("capyro-key-add", action.name())), r, true);
                    p.rect_stroke(
                        r,
                        6.0,
                        Stroke::new(1.0, Color32::from_white_alpha((14.0 + 30.0 * a.hover) as u8).lerp_to_gamma(accent, a.hover)),
                        StrokeKind::Inside,
                    );
                    icons::plus(&p, r.center(), 10.0, a.tint(TEXT_FAINT, accent), a.hover * std::f32::consts::FRAC_PI_2, 1.6);
                    if a.resp.clicked() {
                        ops.push(Op::Capture(action));
                    }
                    a.resp.on_hover_text("Agregar otra tecla");
                }

                // borrar la función entera
                if hover > 0.001 && !capturing {
                    let tc = pos2(row.right() - 16.0, cy);
                    let a = animate_button(ui, Id::unique(("capyro-key-trash", action.name())), Rect::from_center_size(tc, Vec2::splat(24.0)), hovered);
                    if a.hover > 0.001 {
                        p.circle_filled(tc, 11.0, ERROR.gamma_multiply(0.18 * a.hover));
                    }
                    icons::trash(&p, tc, 14.0, a.tint(TEXT_FAINT, ERROR).gamma_multiply(hover));
                    if a.resp.clicked() {
                        ops.push(Op::RemoveAll(action));
                    }
                    a.resp.on_hover_text("Quitar este atajo");
                }
            }
            if actions.is_empty() {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("No hay atajos. Agregá uno con el botón de abajo.").color(TEXT_FAINT));
                });
            }
        });

        /* --- pie --- */
        let fy = panel.bottom() - footer_h / 2.0;
        painter.line_segment(
            [pos2(x0, panel.bottom() - footer_h), pos2(x1, panel.bottom() - footer_h)],
            Stroke::new(1.0, Color32::from_white_alpha(14)),
        );
        let g = painter.layout_no_wrap("Agregar atajo".to_owned(), bold(12.5), Color32::WHITE);
        let add_rect = Rect::from_min_size(pos2(x0, fy - 16.0), vec2(g.size().x + 44.0, 32.0));
        let add = animate_button(ui, Id::unique("capyro-keys-add"), add_rect, interactive && capture.is_none());
        let ar = Rect::from_center_size(add_rect.center(), add_rect.size() * (1.0 + 0.03 * add.hover - 0.06 * add.press));
        if add.hover > 0.001 {
            painter.rect_filled(ar.expand(4.0 * add.hover), ar.height() / 2.0 + 4.0, accent.gamma_multiply(0.16 * add.hover));
        }
        painter.rect_filled(ar, ar.height() / 2.0, accent.lerp_to_gamma(Color32::WHITE, 0.12 * add.hover));
        let spin = if self.keys_adding { std::f32::consts::FRAC_PI_4 } else { add.hover * std::f32::consts::FRAC_PI_2 };
        icons::plus(&painter, pos2(ar.left() + 18.0, fy), 11.0, Color32::WHITE, spin, 2.0);
        painter.galley(pos2(ar.left() + 30.0, fy - g.size().y / 2.0), g, Color32::WHITE);
        if add.resp.clicked() {
            ops.push(Op::ToggleAdding);
        }

        // a la derecha: restablecer y recargar
        let mut rx = x1;
        for (id, label, op, tip) in [
            ("capyro-keys-reload", "Recargar archivo", 0, "Volver a leer capyro.lua (si lo editaste a mano)"),
            ("capyro-keys-reset", "Restablecer", 1, "Volver a los atajos de fábrica"),
        ] {
            let g = painter.layout_no_wrap(label.to_owned(), regular(12.0), TEXT_DIM);
            let r = Rect::from_min_max(pos2(rx - g.size().x - 16.0, fy - 14.0), pos2(rx, fy + 14.0));
            let a = animate_button(ui, Id::unique(id), r, interactive && capture.is_none());
            if a.hover > 0.001 {
                painter.rect_filled(r, 7.0, Color32::from_white_alpha((12.0 * a.hover) as u8));
            }
            painter.galley(r.center() - g.size() / 2.0, g, a.tint(TEXT_DIM, Color32::WHITE));
            if a.resp.clicked() {
                ops.push(if op == 0 { Op::Reload } else { Op::Reset });
            }
            a.resp.on_hover_text(tip);
            rx = r.left() - 6.0;
        }
        painter.text(
            pos2(ar.right() + 14.0, fy),
            Align2::LEFT_CENTER,
            "0–9 saltan al 0–90 % (fijos)",
            regular(10.5),
            TEXT_FAINT,
        );

        /* --- lista de funciones sin tecla --- */
        if self.keys_adding {
            let free: Vec<Action> = Action::ALL
                .iter()
                .copied()
                .filter(|a| !self.cfg.bindings.iter().any(|b| b.action == *a))
                .collect();
            let w = 230.0;
            let row_h = 28.0;
            let h = 34.0 + row_h * free.len().max(1) as f32 + 6.0;
            let max_h = add_rect.top() - panel.top() - 12.0;
            let pop = Rect::from_min_size(pos2(add_rect.left(), add_rect.top() - 8.0 - h.min(max_h)), vec2(w, h.min(max_h)));
            ui.interact(pop, Id::unique("capyro-keys-pop-bg"), Sense::click());
            crate::theme::soft_shadow(&painter, pop, 10.0, 8.0, 2.0, 80);
            painter.rect_filled(pop, 10.0, BG_WINDOW);
            painter.rect_stroke(pop, 10.0, Stroke::new(1.0, Color32::from_white_alpha(18)), StrokeKind::Inside);
            painter.text(pos2(pop.left() + 12.0, pop.top() + 18.0), Align2::LEFT_CENTER, "Funciones sin tecla", bold(12.0), TEXT);
            if free.is_empty() {
                painter.text(
                    pos2(pop.left() + 12.0, pop.top() + 46.0),
                    Align2::LEFT_CENTER,
                    "Todas ya tienen una tecla",
                    regular(12.0),
                    TEXT_FAINT,
                );
            }
            let inner = Rect::from_min_max(pos2(pop.left() + 4.0, pop.top() + 32.0), pop.max - vec2(4.0, 4.0));
            let mut pc = ui.new_child(UiBuilder::new().max_rect(inner));
            ScrollArea::vertical().id_salt("capyro-keys-free").auto_shrink([false, false]).show(&mut pc, |ui| {
                for a in free {
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::hover());
                    let b = animate_button(ui, Id::unique(("capyro-keys-free", a.name())), r, true);
                    if b.hover > 0.001 {
                        ui.painter().rect_filled(r, 6.0, accent.gamma_multiply(0.16 * b.hover));
                    }
                    ui.painter().text(pos2(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, a.label(), regular(12.5), b.tint(TEXT, Color32::WHITE));
                    if b.resp.clicked() {
                        ops.push(Op::Capture(a));
                    }
                }
            });
        }

        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            ops.push(if self.keys_adding { Op::ToggleAdding } else { Op::Close });
        }

        for op in ops {
            match op {
                Op::Remove(action, m, k) => {
                    self.cfg.bindings.retain(|b| !(b.action == action && b.modifiers == m && b.key == k));
                    let msg = format!("{} ya no hace «{}»", config::key_label(m, k), action.label());
                    self.save_config(&ctx, msg);
                }
                Op::RemoveAll(action) => {
                    self.cfg.bindings.retain(|b| b.action != action);
                    let msg = format!("«{}» quedó sin tecla", action.label());
                    self.save_config(&ctx, msg);
                }
                Op::Capture(action) => {
                    self.keys_adding = false;
                    self.keys_capture = Some(action);
                }
                Op::CancelCapture => self.keys_capture = None,
                Op::ToggleAdding => self.keys_adding = !self.keys_adding,
                Op::Reset => {
                    self.cfg.bindings = config::default_bindings();
                    self.save_config(&ctx, "Atajos de fábrica restablecidos".to_owned());
                }
                Op::Reload => self.reload_config(&ctx),
                Op::Close => {
                    self.show_keys = false;
                    self.keys_adding = false;
                    self.keys_capture = None;
                }
            }
        }
    }
}

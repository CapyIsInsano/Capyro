//! Menú de efectos de audio (botón ≡ a la izquierda del volumen).
//!
//! Graves / medios / agudos, tono, estéreo (hasta mono), modo noche,
//! presets y un interruptor general para comparar con y sin efectos.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, Id, Key, Modifiers, Painter, Rect, Sense, Stroke, StrokeKind, Ui,
};

use super::{animate_button, CapyroApp};
use crate::ffi::AudioFx;
use crate::theme::*;

/// Presets: solo tocan el ecualizador, el estéreo y el modo noche (no el tono).
const PRESETS: [(&str, AudioFx); 5] = [
    ("Normal", AudioFx { enabled: true, bass_db: 0.0, mid_db: 0.0, treble_db: 0.0, width: 1.0, night: false, eq15_on: false, eq15: [0.0; 15] }),
    ("Bajos", AudioFx { enabled: true, bass_db: 8.0, mid_db: 0.0, treble_db: 2.0, width: 1.1, night: false, eq15_on: false, eq15: [0.0; 15] }),
    ("Voces", AudioFx { enabled: true, bass_db: -4.0, mid_db: 6.0, treble_db: 2.0, width: 0.9, night: false, eq15_on: false, eq15: [0.0; 15] }),
    ("Lo-fi", AudioFx { enabled: true, bass_db: 3.0, mid_db: -3.0, treble_db: -10.0, width: 0.6, night: false, eq15_on: false, eq15: [0.0; 15] }),
    ("Noche", AudioFx { enabled: true, bass_db: -3.0, mid_db: 3.0, treble_db: 0.0, width: 1.0, night: true, eq15_on: false, eq15: [0.0; 15] }),
];

fn same_sound(a: &AudioFx, b: &AudioFx) -> bool {
    (a.bass_db - b.bass_db).abs() < 0.05
        && (a.mid_db - b.mid_db).abs() < 0.05
        && (a.treble_db - b.treble_db).abs() < 0.05
        && (a.width - b.width).abs() < 0.005
        && a.night == b.night
}

fn fmt_db(v: f32) -> String {
    if v.abs() < 0.05 {
        "0 dB".to_owned()
    } else {
        format!("{:+.1} dB", v)
    }
}

fn fmt_pitch(v: f32) -> String {
    match v.round() as i32 {
        0 => "Original".to_owned(),
        1 => "+1 semitono".to_owned(),
        -1 => "-1 semitono".to_owned(),
        n => format!("{n:+} semitonos"),
    }
}

fn fmt_width(v: f32) -> String {
    if v < 0.005 {
        "Mono".to_owned()
    } else if (v - 1.0).abs() < 0.005 {
        "Normal".to_owned()
    } else {
        format!("{:.0} %", v * 100.0)
    }
}

/// Muescas de rueda de este frame (sin el suavizado de egui).
fn wheel_notches(ctx: &egui::Context) -> f32 {
    ctx.input(|i| {
        i.events
            .iter()
            .map(|e| match e {
                egui::Event::MouseWheel { unit, delta, .. } => match unit {
                    egui::MouseWheelUnit::Line => delta.y,
                    egui::MouseWheelUnit::Point => delta.y / 50.0,
                    egui::MouseWheelUnit::Page => delta.y * 4.0,
                },
                _ => 0.0,
            })
            .sum()
    })
}

struct SliderSpec<'a> {
    id: &'a str,
    label: &'a str,
    value: f32,
    min: f32,
    max: f32,
    default: f32,
    step: f32,
    /// La barra se rellena desde el valor por defecto (±) y no desde el mínimo.
    from_default: bool,
    text: String,
}

struct SliderOut {
    value: f32,
    changed: bool,
    /// El usuario terminó de moverlo (soltó, clic, rueda o doble clic).
    committed: bool,
}

/// Interruptor tipo pastilla. Devuelve true si se tocó.
pub(super) fn switch(ui: &Ui, painter: &Painter, id: &str, center: egui::Pos2, on: bool, accent: Color32, interactive: bool) -> bool {
    let r = Rect::from_center_size(center, vec2(34.0, 18.0));
    let a = animate_button(ui, Id::unique(("capyro-fx-switch", id)), r.expand(4.0), interactive);
    let t = ui.ctx().animate_bool_with_time(Id::unique(("capyro-fx-switch-on", id)), on, crate::perf::t(0.15));
    let bg = Color32::from_white_alpha((28.0 + 14.0 * a.hover) as u8).lerp_to_gamma(accent, t);
    painter.rect_filled(r, 9.0, bg);
    let x = r.left() + 9.0 + t * (r.width() - 18.0);
    painter.circle_filled(pos2(x, r.center().y), 6.5 - 0.8 * a.press, Color32::WHITE);
    a.resp.clicked()
}

impl CapyroApp {
    fn fx_slider(&mut self, ui: &Ui, painter: &Painter, row: Rect, spec: SliderSpec<'_>, interactive: bool) -> SliderOut {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;
        let id = Id::unique(("capyro-fx-slider", spec.id));
        let resp = ui.interact(row, id, if interactive { Sense::click_and_drag() } else { Sense::hover() });
        let hovered = interactive && (resp.hovered() || resp.dragged());
        let hover = ctx.animate_bool_with_time(id.with("h"), hovered, crate::perf::t(0.12));
        if hovered {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let track = Rect::from_min_max(pos2(row.left(), row.bottom() - 10.0), pos2(row.right(), row.bottom() - 6.0));
        let span = spec.max - spec.min;
        let snap = |v: f32| ((v / spec.step).round() * spec.step).clamp(spec.min, spec.max);
        let mut value = spec.value;
        let mut committed = false;

        if resp.double_clicked() {
            value = spec.default;
            committed = true;
        } else if resp.dragged() || resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                value = snap(spec.min + (p.x - track.left()) / track.width() * span);
            }
            committed = resp.clicked();
        }
        if resp.drag_stopped() {
            committed = true;
        }
        if hovered && self.first_pass {
            let n = wheel_notches(&ctx).trunc();
            if n != 0.0 {
                value = snap(value + n * spec.step);
                committed = true;
            }
        }

        // textos
        let changed_from_default = (value - spec.default).abs() > spec.step * 0.25;
        painter.text(pos2(row.left(), row.top() + 8.0), Align2::LEFT_CENTER, spec.label, regular(12.0), TEXT_DIM.lerp_to_gamma(TEXT, hover));
        let shown = if (value - spec.value).abs() > f32::EPSILON { None } else { Some(spec.text.clone()) };
        let text = shown.unwrap_or_else(|| match spec.id {
            "pitch" => fmt_pitch(value),
            "width" => fmt_width(value),
            _ => fmt_db(value),
        });
        painter.text(
            pos2(row.right(), row.top() + 8.0),
            Align2::RIGHT_CENTER,
            text,
            if changed_from_default { bold(11.5) } else { regular(11.5) },
            if changed_from_default { accent } else { TEXT_FAINT },
        );

        // barra: fondo, marca del valor por defecto, relleno y perilla
        let th = 4.0 + 1.0 * hover;
        let track = Rect::from_center_size(track.center(), vec2(track.width(), th));
        painter.rect_filled(track, th / 2.0, TRACK);
        let x_of = |v: f32| track.left() + (v - spec.min) / span * track.width();
        let x_def = x_of(spec.default);
        let x_val = x_of(value);
        if spec.from_default {
            painter.rect_filled(
                Rect::from_min_max(pos2(x_def.min(x_val), track.top()), pos2(x_def.max(x_val), track.bottom())),
                th / 2.0,
                accent,
            );
            painter.line_segment(
                [pos2(x_def, track.top() - 3.0), pos2(x_def, track.bottom() + 3.0)],
                Stroke::new(1.0, Color32::from_white_alpha(60)),
            );
        } else {
            painter.rect_filled(Rect::from_min_max(track.min, pos2(x_val, track.bottom())), th / 2.0, accent);
        }
        let press = ctx.animate_bool_with_time(id.with("p"), resp.dragged(), crate::perf::t(0.08));
        painter.circle_filled(pos2(x_val, track.center().y), 5.0 + 1.5 * hover + 1.0 * press, Color32::WHITE);
        painter.circle_stroke(pos2(x_val, track.center().y), 5.0 + 1.5 * hover + 1.0 * press, Stroke::new(1.5, accent));

        SliderOut { value, changed: (value - spec.value).abs() > f32::EPSILON, committed }
    }

    /// El menú. Se ancla al botón de efectos y crece hacia arriba.
    pub(super) fn fx_menu(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let t = ctx.animate_bool_with_time(Id::unique("capyro-fx-anim"), self.show_fx, crate::perf::t(0.15));
        if t <= 0.001 {
            return;
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let accent = self.cfg.accent;
        let size = vec2(300.0, 330.0);
        let anchor = pos2(self.fx_rect.right() + 6.0, self.fx_rect.top() - 10.0);
        // que no se salga de la ventana por la izquierda
        let anchor = pos2(anchor.x.max(ui.max_rect().left() + 12.0 + size.x), anchor.y);
        let full = Rect::from_min_size(anchor - size, size);
        let panel = Rect::from_min_max(anchor - size * (0.85 + 0.15 * crate::alto::pop(t, ease)), anchor);
        let interactive = self.show_fx && t > 0.6;

        let mut painter = ui.painter().clone();
        painter.multiply_opacity(ease);
        if interactive {
            let outside = ui.interact(ui.max_rect(), Id::unique("capyro-fx-outside"), Sense::click());
            if outside.clicked() {
                self.show_fx = false;
            }
            ui.interact(full, Id::unique("capyro-fx-bg"), Sense::click());
        }
        crate::theme::soft_shadow(&painter, panel, 12.0, 8.0, 2.0, 70);
        painter.rect_filled(panel, 12.0, BG_PANEL);
        painter.rect_stroke(panel, 12.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
        if ease < 0.98 {
            ctx.request_repaint_after(super::FRAME_SMOOTH);
            return;
        }
        crate::alto::reveal(&ctx, Id::unique("capyro-fx-anim"), panel, 12.0, self.show_fx, BG_PANEL, accent);

        let mut fx = self.core.fx();
        let mut pitch = self.fx_pitch_drag.unwrap_or(self.core.pitch() as f32);
        let mut fx_changed = false;
        let x0 = panel.left() + 14.0;
        let x1 = panel.right() - 14.0;
        let mut y = panel.top() + 20.0;

        // encabezado + interruptor general
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Efectos de audio", bold(13.5), TEXT);
        let state = if fx.enabled { "Activados" } else { "Apagados" };
        painter.text(pos2(x1 - 44.0, y), Align2::RIGHT_CENTER, state, regular(11.0), TEXT_FAINT);
        if switch(ui, &painter, "master", pos2(x1 - 17.0, y), fx.enabled, accent, interactive) {
            fx.enabled = !fx.enabled;
            fx_changed = true;
        }

        // los controles se ven apagados si el interruptor general está apagado
        let mut body = painter.clone();
        if !fx.enabled {
            body.multiply_opacity(0.45);
        }

        // presets
        y += 28.0;
        let gap = 5.0;
        let chip_w = ((x1 - x0) - gap * (PRESETS.len() as f32 - 1.0)) / PRESETS.len() as f32;
        for (i, (name, preset)) in PRESETS.iter().enumerate() {
            let r = Rect::from_min_size(pos2(x0 + i as f32 * (chip_w + gap), y - 12.0), vec2(chip_w, 24.0));
            let chip = animate_button(ui, Id::unique(("capyro-fx-preset", i)), r, interactive);
            let selected = same_sound(&fx, preset);
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-fx-preset-sel", i)), selected, crate::perf::t(0.15));
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.06 * chip.press));
            let bg = Color32::from_white_alpha((8.0 + 14.0 * chip.hover) as u8).lerp_to_gamma(accent, sel);
            body.rect_filled(rr, 7.0, bg);
            let col = TEXT_DIM.lerp_to_gamma(TEXT, chip.hover).lerp_to_gamma(Color32::WHITE, sel);
            body.text(rr.center(), Align2::CENTER_CENTER, *name, if selected { bold(11.0) } else { regular(11.0) }, col);
            if chip.resp.clicked() {
                // los presets no tocan el ecualizador de 15 bandas (eso es de Capyro Studio)
                fx = AudioFx { enabled: true, eq15_on: fx.eq15_on, eq15: fx.eq15, ..*preset };
                fx_changed = true;
            }
        }

        // deslizadores
        y += 20.0;
        let row_h = 38.0;
        let row = |y: f32| Rect::from_min_max(pos2(x0, y), pos2(x1, y + row_h - 4.0));
        let specs: [(&str, &str, f32, f32, f32, f32, f32, bool); 4] = [
            ("bass", "Graves", fx.bass_db, -12.0, 12.0, 0.0, 0.5, true),
            ("mid", "Medios · voces", fx.mid_db, -12.0, 12.0, 0.0, 0.5, true),
            ("treble", "Agudos", fx.treble_db, -12.0, 12.0, 0.0, 0.5, true),
            ("width", "Estéreo", fx.width, 0.0, 2.0, 1.0, 0.05, true),
        ];
        for (id, label, value, min, max, default, step, from_default) in specs {
            let text = match id {
                "width" => fmt_width(value),
                _ => fmt_db(value),
            };
            let out = self.fx_slider(
                ui,
                &body,
                row(y),
                SliderSpec { id, label, value, min, max, default, step, from_default, text },
                interactive,
            );
            if out.changed {
                match id {
                    "bass" => fx.bass_db = out.value,
                    "mid" => fx.mid_db = out.value,
                    "treble" => fx.treble_db = out.value,
                    _ => fx.width = out.value,
                }
                fx_changed = true;
            }
            y += row_h;
        }

        // tono: se aplica al soltar (cambiarlo vuelve a decodificar el audio)
        let out = self.fx_slider(
            ui,
            &body,
            row(y),
            SliderSpec {
                id: "pitch",
                label: "Tono",
                value: pitch,
                min: -12.0,
                max: 12.0,
                default: 0.0,
                step: 1.0,
                from_default: true,
                text: fmt_pitch(pitch),
            },
            interactive,
        );
        if out.changed {
            pitch = out.value;
            self.fx_pitch_drag = Some(pitch);
        }
        if out.committed {
            self.core.set_pitch(pitch as f64);
            self.fx_pitch_drag = None;
        }
        y += row_h;

        // modo noche + restablecer
        y += 6.0;
        body.line_segment([pos2(x0, y - 8.0), pos2(x1, y - 8.0)], Stroke::new(1.0, Color32::from_white_alpha(14)));
        y += 12.0;
        if switch(ui, &body, "night", pos2(x0 + 17.0, y), fx.night, accent, interactive) {
            fx.night = !fx.night;
            fx_changed = true;
        }
        body.text(pos2(x0 + 42.0, y - 6.0), Align2::LEFT_CENTER, "Modo noche", regular(12.5), TEXT);
        body.text(pos2(x0 + 42.0, y + 8.0), Align2::LEFT_CENTER, "iguala lo fuerte y lo bajito", regular(10.0), TEXT_FAINT);

        let neutral = fx.is_neutral() && pitch.abs() < 0.5;
        let reset_g = painter.layout_no_wrap("Restablecer".to_owned(), regular(11.5), TEXT);
        let reset_r = Rect::from_min_size(pos2(x1 - reset_g.size().x - 20.0, y - 12.0), reset_g.size() + vec2(20.0, 10.0));
        let reset = animate_button(ui, Id::unique("capyro-fx-reset"), reset_r, interactive && !neutral);
        let rr = Rect::from_center_size(reset_r.center(), reset_r.size() * (1.0 - 0.06 * reset.press));
        painter.rect_filled(rr, 7.0, Color32::from_white_alpha((8.0 + 14.0 * reset.hover) as u8));
        let col = if neutral { TEXT_FAINT } else { reset.tint(TEXT_DIM, TEXT) };
        painter.galley(rr.center() - reset_g.size() / 2.0, reset_g, col);
        if reset.resp.clicked() {
            fx = AudioFx { enabled: fx.enabled, ..AudioFx::default() };
            fx_changed = true;
            self.core.set_pitch(0.0);
            self.fx_pitch_drag = None;
        }
        if !neutral {
            reset.resp.on_hover_text("Volver todo a la normalidad (también el tono)");
        }

        if fx_changed {
            self.core.set_fx(&fx);
        }

        // pie
        painter.text(
            pos2(x0, panel.bottom() - 14.0),
            Align2::LEFT_CENTER,
            "Doble clic en un control: volver a 0",
            regular(10.0),
            TEXT_FAINT,
        );

        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.show_fx = false;
        }
    }

    /// Efectos en chiquito, para la vista de música: el título (que abre el
    /// menú completo), el interruptor general y los presets. Ocupa `rect`
    /// (unos 54 puntos de alto).
    pub(super) fn fx_compact(&mut self, ui: &mut Ui, painter: &Painter, rect: Rect) {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;
        let mut fx = self.core.fx();
        let mut fx_changed = false;
        let (x0, x1) = (rect.left(), rect.right());
        let y = rect.top() + 10.0;

        // "Efectos de audio ›": abre el menú con todo (graves, tono, estéreo...)
        let tg = painter.layout_no_wrap("Efectos de audio".to_owned(), bold(12.0), TEXT);
        let head = Rect::from_min_size(pos2(x0 - 4.0, y - 11.0), vec2(tg.size().x + 44.0, 22.0));
        let open = animate_button(ui, Id::unique("capyro-fx-compact-open"), head, true);
        let on = ctx.animate_bool_with_time(Id::unique("capyro-fx-on"), self.show_fx, crate::perf::t(0.15));
        if open.hover > 0.001 || on > 0.001 {
            painter.rect_filled(head, 7.0, Color32::from_white_alpha((10.0 * open.hover.max(on)) as u8));
        }
        let col = open.tint(TEXT_DIM, TEXT).lerp_to_gamma(accent, on);
        crate::icons::sliders(painter, pos2(x0 + 6.0, y), 13.0, col);
        painter.galley(pos2(x0 + 16.0, y - tg.size().y / 2.0), tg, col);
        let chev = pos2(head.right() - 9.0 + 2.0 * open.hover, y);
        let st = Stroke::new(1.4, col);
        painter.line_segment([chev + vec2(-3.0, -4.0), chev], st);
        painter.line_segment([chev + vec2(-3.0, 4.0), chev], st);
        // el menú completo se abre arriba de esta sección, alineado a la
        // derecha de la columna (crece hacia arriba y a la izquierda)
        self.fx_rect = Rect::from_min_max(pos2(x1 - 26.0, head.top()), pos2(x1 - 6.0, head.bottom()));
        if open.resp.clicked() {
            let was = self.show_fx;
            self.close_menus();
            self.show_fx = !was;
        }
        open.resp.on_hover_text("Todos los efectos: graves, medios, agudos, tono, estéreo y modo noche (Ctrl+E)");

        // interruptor general
        if switch(ui, painter, "master-compact", pos2(x1 - 17.0, y), fx.enabled, accent, true) {
            fx.enabled = !fx.enabled;
            fx_changed = true;
        }

        // presets
        let cy = y + 30.0;
        let gap = 4.0;
        let n = PRESETS.len() as f32;
        let chip_w = ((x1 - x0) - gap * (n - 1.0)) / n;
        let fs = if chip_w < 46.0 { 10.5 } else { 11.0 };
        let mut body = painter.clone();
        if !fx.enabled {
            body.multiply_opacity(0.45);
        }
        for (i, (name, preset)) in PRESETS.iter().enumerate() {
            let r = Rect::from_min_size(pos2(x0 + i as f32 * (chip_w + gap), cy - 12.0), vec2(chip_w, 24.0));
            let chip = animate_button(ui, Id::unique(("capyro-fx-compact-preset", i)), r, true);
            let selected = fx.enabled && same_sound(&fx, preset);
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-fx-compact-sel", i)), selected, crate::perf::t(0.15));
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.06 * chip.press));
            if crate::alto::on() {
                crate::alto::glow_rect(&body, rr, accent, 0.6 * chip.hover);
            }
            let bg = Color32::from_white_alpha((8.0 + 14.0 * chip.hover) as u8).lerp_to_gamma(accent, sel);
            body.rect_filled(rr, 7.0, bg);
            let col = TEXT_DIM.lerp_to_gamma(TEXT, chip.hover).lerp_to_gamma(Color32::WHITE, sel);
            body.text(rr.center(), Align2::CENTER_CENTER, *name, if selected { bold(fs) } else { regular(fs) }, col);
            if chip.resp.clicked() {
                // los presets no tocan el ecualizador de 15 bandas (eso es de Capyro Studio)
                fx = AudioFx { enabled: true, eq15_on: fx.eq15_on, eq15: fx.eq15, ..*preset };
                fx_changed = true;
            }
        }
        if fx_changed {
            self.core.set_fx(&fx);
        }
    }

    /// ¿Hay algún efecto sonando? (para pintar el botón de naranja)
    pub(super) fn fx_active(&self) -> bool {
        let fx = self.core.fx();
        fx.enabled && (!fx.is_neutral() || self.core.pitch().abs() > 0.01)
    }
}

//! Piezas chicas que usan todos los paneles: botones animados, textos que
//! se recortan, curvas de animación y formatos de tiempo.

use super::*;
use eframe::egui::{epaint::RectShape, Shape};

/// Marca en la memoria de egui: "se tocó un botón en este frame".
pub(crate) const CLICK_FLAG: &str = "capyro-button-clicked";

pub(crate) fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

pub(crate) fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Respuesta de un botón + sus animaciones (0..1, suavizadas en el tiempo).
pub(crate) struct Anim {
    pub(crate) resp: Response,
    pub(crate) hover: f32,
    pub(crate) press: f32,
}

impl Anim {
    /// Escala para el ícono: crece un poco al pasar el mouse, se hunde al clic.
    pub(crate) fn scale(&self) -> f32 {
        1.0 + 0.08 * self.hover - 0.14 * self.press
    }
    pub(crate) fn tint(&self, base: Color32, hot: Color32) -> Color32 {
        base.lerp_to_gamma(hot, self.hover)
    }
}

/// `text` recortado con "…" para que entre en `max_w` puntos.
pub(crate) fn fit_text(painter: &egui::Painter, text: &str, font: FontId, max_w: f32) -> String {
    let width = |s: &str| painter.layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE).size().x;
    if width(text) <= max_w {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    // búsqueda binaria del largo que entra
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let s: String = chars[..mid].iter().collect::<String>().trim_end().to_owned() + "…";
        if width(&s) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo].iter().collect::<String>().trim_end().to_owned() + "…"
}

pub(crate) fn animate_button(ui: &Ui, id: Id, rect: Rect, enabled: bool) -> Anim {
    let resp = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
    let ctx = ui.ctx();
    let hovered = enabled && resp.hovered();
    let hover = ctx.animate_bool_with_time(id.with("hover"), hovered, crate::system::perf::t(0.14));
    if resp.clicked() {
        // el clic suena al final del frame (si el botón no hizo sonar otra cosa)
        ctx.data_mut(|d| d.insert_temp(Id::unique(CLICK_FLAG), true));
    }
    let press = ctx.animate_bool_with_time(id.with("press"), enabled && resp.is_pointer_button_down_on(), crate::system::perf::t(0.07));
    if hovered {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    Anim { resp, hover, press }
}

pub(crate) fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}

pub(crate) fn ellipsize(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Carpeta de Imágenes del usuario + "/Capyro".
pub(crate) fn default_screenshot_dir() -> PathBuf {
    crate::system::paths::default_screenshot_dir()
}

pub(crate) fn fmt_time(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

impl CapyroApp {
    /// Botón redondo de ícono: fondo que aparece y crece al pasar el mouse,
    /// y un "pulso" más fuerte mientras se mantiene apretado.
    pub(crate) fn icon_button(ui: &mut Ui, id: &str, center: Pos2, size: f32) -> Anim {
        let rect = Rect::from_center_size(center, Vec2::splat(size));
        let a = animate_button(ui, Id::unique(("capyro-btn", id)), rect, true);
        let t = a.hover.max(a.press);
        if t > 0.001 {
            let r = size * 0.5 * (0.7 + 0.3 * a.hover) * (1.0 - 0.06 * a.press);
            let alpha = (14.0 * a.hover + 16.0 * a.press) as u8;
            ui.painter().circle_filled(center, r, Color32::from_white_alpha(alpha));
        }
        if crate::ui::alto::on() {
            // Alto: brillo del color de acento al pasar y una onda al tocar
            let accent = ui.visuals().selection.bg_fill;
            crate::ui::alto::glow(ui.painter(), center, size * 1.1, accent, 0.20 * a.hover + 0.15 * a.press);
            if a.resp.clicked() {
                crate::ui::alto::ripple(ui.ctx(), center, size * 0.5);
            }
        }
        a
    }
}

/* ------------------------------------------------------------------------ */
/* piezas de los paneles (Studio, Converter)                                */
/* ------------------------------------------------------------------------ */

/// Divide `rect` en columnas con estos pesos (y `gap` entre ellas).
pub(crate) fn columns(rect: Rect, weights: &[f32], gap: f32) -> Vec<Rect> {
    let total: f32 = weights.iter().sum::<f32>().max(0.001);
    let free = rect.width() - gap * (weights.len().saturating_sub(1)) as f32;
    let mut x = rect.left();
    weights
        .iter()
        .map(|w| {
            let cw = free * w / total;
            let r = Rect::from_min_size(pos2(x, rect.top()), vec2(cw, rect.height()));
            x += cw + gap;
            r
        })
        .collect()
}

/// Campo de texto con su nombre arriba. Fondo suave, borde de acento al escribir.
#[allow(clippy::too_many_arguments)]
pub(crate) fn field(ui: &mut Ui, rect: Rect, id: &str, label: &str, value: &mut String, hint: &str, accent: Color32, rows: usize) {
    let painter = ui.painter().clone();
    painter.text(pos2(rect.left() + 2.0, rect.top() + 6.0), Align2::LEFT_CENTER, label, regular(10.5), TEXT_FAINT);
    let boxr = Rect::from_min_max(pos2(rect.left(), rect.top() + 15.0), rect.max);
    let bg = painter.add(Shape::Noop);
    let font = regular(13.0);
    let hint = egui::RichText::new(hint).color(TEXT_FAINT.gamma_multiply(0.75)).font(font.clone());
    let edit = if rows > 1 {
        egui::TextEdit::multiline(value).desired_rows(rows)
    } else {
        egui::TextEdit::singleline(value).align(Align2::LEFT_CENTER)
    };
    let resp = ui.put(
        boxr,
        edit.id_salt(("capyro-field", id))
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(9, 6)))
            .font(font)
            .text_color(TEXT)
            .hint_text(hint),
    );
    let focus = resp.has_focus();
    let hover = resp.hovered();
    let alpha = if focus { 14 } else if hover { 11 } else { 7 };
    painter.set(bg, RectShape::filled(boxr, 8.0, Color32::from_white_alpha(alpha)));
    if focus {
        painter.rect_stroke(boxr, 8.0, Stroke::new(1.2, accent.gamma_multiply(0.85)), StrokeKind::Inside);
    }
}

/// Campo grande sin nombre (el título y el artista): solo una línea abajo.
#[allow(clippy::too_many_arguments)]
pub(crate) fn big_field(ui: &mut Ui, rect: Rect, id: &str, value: &mut String, hint: &str, font: egui::FontId, color: Color32, accent: Color32) {
    let painter = ui.painter().clone();
    let bg = painter.add(Shape::Noop);
    let hint = egui::RichText::new(hint).color(TEXT_FAINT.gamma_multiply(0.75)).font(font.clone());
    let resp = ui.put(
        rect,
        egui::TextEdit::singleline(value)
            .id_salt(("capyro-big-field", id))
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(6, 2)))
            .font(font)
            .text_color(color)
            .hint_text(hint)
            .align(Align2::LEFT_CENTER),
    );
    let focus = resp.has_focus();
    let hover = resp.hovered();
    if focus || hover {
        painter.set(bg, RectShape::filled(rect, 8.0, Color32::from_white_alpha(if focus { 10 } else { 6 })));
    }
    let line_col = if focus { accent } else { Color32::from_white_alpha(if hover { 40 } else { 16 }) };
    painter.line_segment(
        [pos2(rect.left() + 6.0, rect.bottom()), pos2(rect.right() - 6.0, rect.bottom())],
        Stroke::new(if focus { 1.5 } else { 1.0 }, line_col),
    );
}

/// Botón de texto chiquito (sin fondo hasta pasar el mouse). Devuelve true si se tocó.
pub(crate) fn link(ui: &Ui, id: &str, at: Pos2, align: Align2, text: &str, color: Color32, enabled: bool) -> (bool, Rect) {
    let painter = ui.painter();
    let g = painter.layout_no_wrap(text.to_owned(), regular(11.5), color);
    let size = g.size() + vec2(14.0, 8.0);
    let rect = align.anchor_size(at, size);
    let a = animate_button(ui, Id::unique(("capyro-link", id)), rect, enabled);
    if a.hover > 0.001 {
        painter.rect_filled(rect, 7.0, Color32::from_white_alpha((12.0 * a.hover) as u8));
    }
    let col = if enabled { color.lerp_to_gamma(TEXT, a.hover) } else { TEXT_FAINT };
    painter.text(rect.center(), Align2::CENTER_CENTER, text, regular(11.5), col);
    (a.resp.clicked(), rect)
}

/// Pastilla: llena (acento) o de contorno. Devuelve true si se tocó.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pill(ui: &Ui, id: &str, rect: Rect, text: &str, filled: bool, selected: f32, accent: Color32, enabled: bool) -> bool {
    let painter = ui.painter();
    let a = animate_button(ui, Id::unique(("capyro-pill", id)), rect, enabled);
    let r = Rect::from_center_size(rect.center(), rect.size() * (1.0 - 0.05 * a.press));
    if crate::ui::alto::on() && enabled {
        crate::ui::alto::glow_rect(painter, r, accent, if filled { 0.5 } else { 0.4 } * a.hover);
    }
    let (bg, fg) = if filled {
        let bg = if enabled { accent.lerp_to_gamma(Color32::WHITE, 0.12 * a.hover) } else { Color32::from_white_alpha(14) };
        (bg, if enabled { Color32::WHITE } else { TEXT_FAINT })
    } else {
        let bg = Color32::from_white_alpha((8.0 + 14.0 * a.hover) as u8).lerp_to_gamma(accent, selected);
        let fg = if enabled { TEXT_DIM.lerp_to_gamma(TEXT, a.hover).lerp_to_gamma(Color32::WHITE, selected) } else { TEXT_FAINT };
        (bg, fg)
    };
    painter.rect_filled(r, r.height() / 2.0, bg);
    let font = if filled || selected > 0.5 { bold(11.5) } else { regular(11.5) };
    painter.text(r.center(), Align2::CENTER_CENTER, text, font, fg);
    a.resp.clicked()
}

/// Ancho de una pastilla para `text`.
pub(crate) fn pill_w(ui: &Ui, text: &str) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), bold(11.5), TEXT).size().x + 24.0
}

/// Línea separadora a lo ancho.
pub(crate) fn sep(ui: &Ui) {
    let r = ui.max_rect();
    let y = ui.cursor().top();
    ui.painter().line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(1.0, Color32::from_white_alpha(12)));
}

/// Interruptor tipo pastilla. Devuelve true si se tocó.
pub(crate) fn switch(ui: &Ui, painter: &egui::Painter, id: &str, center: Pos2, on: bool, accent: Color32, interactive: bool) -> bool {
    let r = Rect::from_center_size(center, vec2(34.0, 18.0));
    let a = animate_button(ui, Id::unique(("capyro-fx-switch", id)), r.expand(4.0), interactive);
    let t = ui.ctx().animate_bool_with_time(Id::unique(("capyro-fx-switch-on", id)), on, crate::system::perf::t(0.15));
    let bg = Color32::from_white_alpha((28.0 + 14.0 * a.hover) as u8).lerp_to_gamma(accent, t);
    painter.rect_filled(r, 9.0, bg);
    let x = r.left() + 9.0 + t * (r.width() - 18.0);
    painter.circle_filled(pos2(x, r.center().y), 6.5 - 0.8 * a.press, Color32::WHITE);
    a.resp.clicked()
}

/// Un chip de una fila de opciones.
pub(crate) struct Chip<'a> {
    pub(crate) label: &'a str,
    pub(crate) selected: bool,
    pub(crate) enabled: bool,
    /// Lo que dice al pasar el mouse.
    pub(crate) hint: Option<&'a str>,
}

/// Fila de chips (se acomodan en renglones si no entran). Ocupa su lugar en
/// `ui` y devuelve el que se tocó (si no estaba elegido).
pub(crate) fn chips(ui: &mut Ui, id: &str, items: &[Chip], accent: Color32) -> Option<usize> {
    let w = ui.available_width();
    let painter = ui.painter().clone();
    let (gap, h) = (6.0, 26.0);
    let mut rects = Vec::with_capacity(items.len());
    let (mut x, mut y) = (0.0f32, 0.0f32);
    for it in items {
        let tw = painter.layout_no_wrap(it.label.to_owned(), bold(11.5), TEXT).size().x + 22.0;
        if x > 0.0 && x + tw > w {
            x = 0.0;
            y += h + gap;
        }
        rects.push(Rect::from_min_size(pos2(x, y), vec2(tw, h)));
        x += tw + gap;
    }
    let (area, _) = ui.allocate_exact_size(vec2(w, y + h), Sense::hover());
    let mut clicked = None;
    for (i, (it, r)) in items.iter().zip(rects).enumerate() {
        let r = r.translate(area.min.to_vec2());
        let enabled = it.enabled && ui.is_enabled();
        let a = animate_button(ui, Id::unique((id, i)), r, enabled);
        let sel = ui.ctx().animate_bool_with_time(Id::unique((id, i, "sel")), it.selected, crate::system::perf::t(0.15));
        let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.06 * a.press));
        let bg = if it.enabled {
            Color32::from_white_alpha((8.0 + 14.0 * a.hover) as u8).lerp_to_gamma(accent, sel)
        } else {
            Color32::from_white_alpha(4)
        };
        painter.rect_filled(rr, 7.0, bg);
        let col = if !it.enabled {
            TEXT_FAINT.gamma_multiply(0.7)
        } else {
            TEXT_DIM.lerp_to_gamma(TEXT, a.hover).lerp_to_gamma(Color32::WHITE, sel)
        };
        let font = if it.selected { bold(11.5) } else { regular(11.5) };
        painter.text(rr.center(), Align2::CENTER_CENTER, it.label, font, col);
        if a.resp.clicked() && !it.selected {
            clicked = Some(i);
        }
        if let Some(hint) = it.hint {
            a.resp.on_hover_text(hint);
        }
    }
    clicked
}

/// Título chico de una opción (a la izquierda) y un dato (a la derecha).
pub(crate) fn option_label(ui: &mut Ui, text: &str, right: &str, accent: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    let p = ui.painter();
    p.text(pos2(r.left(), r.center().y), Align2::LEFT_CENTER, text, regular(12.0), TEXT_DIM);
    if !right.is_empty() {
        p.text(pos2(r.right(), r.center().y), Align2::RIGHT_CENTER, right, bold(11.5), accent);
    }
}

/// Deslizador de 0 a 1 con textos en las puntas. Devuelve true si cambió.
pub(crate) fn slider(ui: &mut Ui, id: &str, value: &mut f32, left: &str, right: &str, accent: Color32) -> bool {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 38.0), Sense::hover());
    let track = Rect::from_min_max(pos2(r.left() + 8.0, r.top() + 10.0), pos2(r.right() - 8.0, r.top() + 14.0));
    let enabled = ui.is_enabled();
    let hit = Rect::from_min_max(pos2(r.left(), r.top()), pos2(r.right(), r.top() + 24.0));
    let resp = ui.interact(hit, Id::unique((id, "slider")), if enabled { Sense::click_and_drag() } else { Sense::hover() });
    let mut changed = false;
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        let v = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
        if (v - *value).abs() > 1e-4 {
            *value = v;
            changed = true;
        }
    }
    if enabled && (resp.hovered() || resp.dragged()) {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let p = ui.painter();
    p.rect_filled(track, 2.0, Color32::from_white_alpha(22));
    let x = track.left() + track.width() * value.clamp(0.0, 1.0);
    p.rect_filled(Rect::from_min_max(track.min, pos2(x, track.bottom())), 2.0, if enabled { accent } else { TEXT_FAINT });
    let knob = if resp.dragged() { 8.0 } else if resp.hovered() { 7.5 } else { 7.0 };
    p.circle_filled(pos2(x, track.center().y), knob, Color32::WHITE);
    p.text(pos2(r.left() + 2.0, r.top() + 29.0), Align2::LEFT_CENTER, left, regular(10.5), TEXT_FAINT);
    p.text(pos2(r.right() - 2.0, r.top() + 29.0), Align2::RIGHT_CENTER, right, regular(10.5), TEXT_FAINT);
    changed
}

/// Barra de recorte: dos manijas (desde, hasta) sobre `0..total` segundos.
/// Devuelve true si cambió algo. Doble clic: todo.
pub(crate) fn range_bar(ui: &mut Ui, id: &str, start: &mut f64, end: &mut f64, total: f64, accent: Color32) -> bool {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
    let track = Rect::from_min_max(pos2(r.left() + 8.0, r.center().y - 2.0), pos2(r.right() - 8.0, r.center().y + 2.0));
    let total = total.max(0.001);
    let x_of = |t: f64| track.left() + track.width() * (t / total).clamp(0.0, 1.0) as f32;
    let t_of = |x: f32| ((x - track.left()) / track.width()).clamp(0.0, 1.0) as f64 * total;
    let enabled = ui.is_enabled();
    let resp = ui.interact(r, Id::unique((id, "range")), if enabled { Sense::click_and_drag() } else { Sense::hover() });
    let drag_id = Id::unique((id, "range-handle"));
    let mut changed = false;
    if resp.double_clicked() {
        *start = 0.0;
        *end = total;
        changed = true;
    } else if let Some(p) = resp.interact_pointer_pos() {
        if resp.drag_started() || resp.clicked() {
            // la manija más cerca del clic
            let h: u8 = if (p.x - x_of(*start)).abs() <= (p.x - x_of(*end)).abs() { 0 } else { 1 };
            ui.ctx().data_mut(|d| d.insert_temp(drag_id, h));
        }
        if resp.dragged() || resp.clicked() {
            let h = ui.ctx().data(|d| d.get_temp::<u8>(drag_id)).unwrap_or(0);
            let t = t_of(p.x);
            let min_len = (total * 0.01).clamp(0.2, 1.0);
            if h == 0 {
                *start = t.min(*end - min_len).max(0.0);
            } else {
                *end = t.max(*start + min_len).min(total);
            }
            changed = true;
        }
    }
    if enabled && (resp.hovered() || resp.dragged()) {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
    }
    let p = ui.painter();
    p.rect_filled(track, 2.0, Color32::from_white_alpha(22));
    let (a, b) = (x_of(*start), x_of(*end));
    p.rect_filled(Rect::from_min_max(pos2(a, track.top()), pos2(b, track.bottom())), 2.0, if enabled { accent } else { TEXT_FAINT });
    for x in [a, b] {
        let hr = Rect::from_center_size(pos2(x, track.center().y), vec2(8.0, 20.0));
        p.rect_filled(hr, 3.0, Color32::WHITE);
        p.rect_stroke(hr, 3.0, Stroke::new(1.0, accent), StrokeKind::Inside);
    }
    changed
}

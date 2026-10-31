//! Paneles que se ponen encima de la ventana (Capyro Studio y Capyro
//! Converter): el velo oscuro, el panel que crece al abrir, el encabezado con
//! su ícono y la ✕, el pie con el estado y el cuerpo con scroll.
//!
//! Respetan el modo de rendimiento: en Bajo aparecen y se van al toque, sin
//! sombra; en Medio, con su animación y la sombra; en Alto, además, el panel
//! se descubre con una línea de luz.

use super::*;

/// Alto del pie (la línea, el estado y los botones).
pub(crate) const FOOT_H: f32 = 62.0;

/// Lo que va arriba a la izquierda: un ícono en un círculo, el nombre y una
/// línea chiquita debajo (el archivo).
pub(crate) struct Head<'a> {
    pub(crate) title: &'a str,
    pub(crate) subtitle: &'a str,
    pub(crate) icon: fn(&egui::Painter, Pos2, f32, Color32),
}

/// Lo que dibujó [`begin`]: dónde va cada cosa y si se pidió cerrar.
pub(crate) struct Overlay {
    pub(crate) panel: Rect,
    pub(crate) painter: egui::Painter,
    /// Márgenes de adentro, arriba del panel y el centro del pie.
    pub(crate) x0: f32,
    pub(crate) x1: f32,
    pub(crate) foot_y: f32,
    /// Lo que queda entre el encabezado y el pie (con scroll).
    pub(crate) body: Rect,
    /// Se tocó la ✕, afuera del panel o Escape (sin estar escribiendo).
    pub(crate) close: bool,
}

/// Cómo va la animación de abrir/cerrar (0..1). Se llama siempre, aunque esté
/// cerrado: así la entrada arranca de cero.
pub(crate) fn anim(ctx: &egui::Context, id: &'static str, open: bool) -> f32 {
    let t = ctx.animate_bool_with_time(Id::unique(id), open, crate::system::perf::t(0.22));
    if (0.001..0.999).contains(&t) {
        ctx.request_repaint_after(FRAME_SMOOTH);
    }
    t
}

/// Dibuja el velo, el panel, el encabezado y la línea del pie. Devuelve None
/// mientras el panel crece (el contenido aparece después).
pub(crate) fn begin(ui: &mut Ui, full: Rect, id: &'static str, open: bool, t: f32, accent: Color32, head: &Head) -> Option<Overlay> {
    let ctx = ui.ctx().clone();
    let anim = crate::system::perf::animations();
    // escribiendo en un campo, Escape suelta el campo (no cierra)
    let typing = ctx.egui_wants_keyboard_input();

    let ease = ease_out(t);
    let area = full.shrink(16.0);
    let size = vec2(area.width().min(880.0), area.height().min(740.0));
    let full_panel = Rect::from_center_size(area.center(), size);
    let panel = Rect::from_center_size(area.center(), size * (0.96 + 0.04 * crate::ui::alto::pop(t, ease)));
    let interactive = open && t > 0.6;
    let radius = 16.0;
    let mut close = false;

    let mut painter = ui.painter().clone();
    painter.multiply_opacity(ease);
    // todo el mouse es del panel: lo de atrás no se toca (tampoco mientras se abre o se cierra)
    let outside = ui.interact(ui.max_rect(), Id::unique((id, "outside")), Sense::click());
    if interactive && outside.clicked() {
        close = true;
    }
    ui.interact(full_panel, Id::unique((id, "bg")), Sense::click());
    // velo oscuro detrás, como un diálogo
    painter.rect_filled(ui.max_rect(), 0.0, Color32::from_black_alpha((130.0 * ease) as u8));
    if anim {
        soft_shadow(&painter, panel, radius, 22.0, 6.0, 120);
    }
    painter.rect_filled(panel, radius, BG_PANEL);
    painter.rect_stroke(panel, radius, Stroke::new(1.0, Color32::from_white_alpha(16)), StrokeKind::Inside);
    if ease < 0.98 {
        return None;
    }
    crate::ui::alto::reveal(&ctx, Id::unique(id), panel, radius, open, BG_PANEL, accent);

    let x0 = panel.left() + 24.0;
    let x1 = panel.right() - 24.0;
    let top = panel.top();

    /* --- encabezado --- */
    let ic = pos2(x0 + 14.0, top + 32.0);
    painter.circle_filled(ic, 16.0, accent.gamma_multiply(0.16));
    (head.icon)(&painter, ic, 17.0, accent);
    painter.text(pos2(x0 + 40.0, top + 24.0), Align2::LEFT_CENTER, head.title, bold(16.0), TEXT);
    painter.text(
        pos2(x0 + 40.0, top + 42.0),
        Align2::LEFT_CENTER,
        fit_text(&painter, head.subtitle, regular(11.0), (x1 - x0) - 120.0),
        regular(11.0),
        TEXT_FAINT,
    );
    let close_c = pos2(x1 - 6.0, top + 30.0);
    let b = CapyroApp::icon_button(ui, &format!("{id}-close"), close_c, 28.0);
    icons::close(&painter, close_c, 12.0 * b.scale(), b.tint(ICON_DIM, Color32::WHITE));
    if b.resp.clicked() {
        close = true;
    }
    b.resp.on_hover_text("Cerrar (Esc)");
    if interactive && !typing && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        close = true;
    }

    /* --- pie --- */
    painter.line_segment(
        [pos2(panel.left() + 1.0, panel.bottom() - FOOT_H), pos2(panel.right() - 1.0, panel.bottom() - FOOT_H)],
        Stroke::new(1.0, Color32::from_white_alpha(14)),
    );
    let body = Rect::from_min_max(pos2(panel.left() + 1.0, top + 64.0), pos2(panel.right() - 1.0, panel.bottom() - FOOT_H - 1.0));
    Some(Overlay { panel, painter, x0, x1, foot_y: panel.bottom() - FOOT_H / 2.0, body, close })
}

/// El cuerpo con scroll (y margen igual al del encabezado). `fresh`: recién
/// abierto, el scroll arranca arriba (egui recuerda el de la vez anterior).
/// `disabled`: se ve pero no se toca (mientras se guarda o se convierte).
pub(crate) fn body(ui: &mut Ui, ov: &Overlay, scroll_id: &'static str, fresh: bool, disabled: bool, add: impl FnOnce(&mut Ui)) {
    let mut body_ui = ui.new_child(UiBuilder::new().max_rect(ov.body));
    if disabled {
        body_ui.disable();
    }
    body_ui.set_clip_rect(ov.body.intersect(ui.clip_rect()));
    let mut scroll = egui::ScrollArea::vertical().id_salt(scroll_id).auto_shrink([false, false]);
    if fresh {
        scroll = scroll.vertical_scroll_offset(0.0);
    }
    scroll.show(&mut body_ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin { left: 23, right: 23, top: 8, bottom: 16 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    });
}

/// Lo que se dice a la izquierda del pie.
pub(crate) enum Status<'a> {
    /// Trabajando (un spinner y el texto).
    Busy(&'a str),
    /// Salió bien (✓) o mal (en rojo).
    Done(&'a str),
    Error(&'a str),
    /// Cambios sin guardar (un puntito).
    Pending(&'a str),
    /// Una ayuda en gris.
    Hint(&'a str),
}

/// Dibuja el estado en el pie, hasta `max_w` de ancho.
pub(crate) fn status(ov: &Overlay, accent: Color32, now: f64, st: Status, max_w: f32) {
    let p = &ov.painter;
    let (x0, y) = (ov.x0, ov.foot_y);
    match st {
        Status::Busy(text) => {
            icons::spinner(p, pos2(x0 + 6.0, y), 12.0, accent, crate::system::perf::clock(now));
            p.text(pos2(x0 + 18.0, y), Align2::LEFT_CENTER, fit_text(p, text, regular(11.5), max_w - 18.0), regular(11.5), TEXT_DIM);
        }
        Status::Done(text) => {
            icons::check(p, pos2(x0 + 6.0, y), 12.0, accent, 1.8);
            p.text(pos2(x0 + 18.0, y), Align2::LEFT_CENTER, fit_text(p, text, regular(11.5), max_w - 18.0), regular(11.5), TEXT_DIM);
        }
        Status::Error(text) => {
            p.text(pos2(x0, y), Align2::LEFT_CENTER, fit_text(p, text, regular(11.5), max_w), regular(11.5), ERROR);
        }
        Status::Pending(text) => {
            p.circle_filled(pos2(x0 + 4.0, y), 4.0, accent);
            p.text(pos2(x0 + 16.0, y), Align2::LEFT_CENTER, fit_text(p, text, regular(11.5), max_w - 16.0), regular(11.5), TEXT_DIM);
        }
        Status::Hint(text) => {
            p.text(pos2(x0, y), Align2::LEFT_CENTER, fit_text(p, text, regular(10.5), max_w), regular(10.5), TEXT_FAINT);
        }
    }
}

/// Botón que abre un panel (Capyro Studio, Capyro Converter): ícono y nombre,
/// con contorno de acento. `progress`: si hay algo corriendo, una barrita
/// abajo que se va llenando. Devuelve true si se tocó.
#[allow(clippy::too_many_arguments)]
pub(crate) fn launcher(
    ui: &mut Ui,
    id: &str,
    r: Rect,
    accent: Color32,
    icon: fn(&egui::Painter, Pos2, f32, Color32),
    label: &str,
    open: bool,
    progress: Option<f32>,
) -> bool {
    let a = animate_button(ui, Id::unique(id), r, true);
    let painter = ui.painter().clone();
    let bg = Color32::from_white_alpha((6.0 + 10.0 * a.hover) as u8).lerp_to_gamma(accent.gamma_multiply(0.3), if open { 1.0 } else { 0.0 });
    painter.rect_filled(r, 9.0, bg);
    painter.rect_stroke(r, 9.0, Stroke::new(1.0, accent.gamma_multiply(0.5)), StrokeKind::Inside);
    let col = accent.lerp_to_gamma(Color32::WHITE, a.hover * 0.3);
    let cy = r.center().y;
    icon(&painter, pos2(r.left() + 18.0, cy), 14.0 * a.scale(), col);
    let text = fit_text(&painter, label, regular(12.5), r.width() - 40.0);
    painter.text(pos2(r.left() + 32.0, cy), Align2::LEFT_CENTER, text, regular(12.5), col);
    if let Some(p) = progress {
        let bar = Rect::from_min_max(pos2(r.left() + 8.0, r.bottom() - 4.0), pos2(r.right() - 8.0, r.bottom() - 2.0));
        painter.rect_filled(bar, 1.0, Color32::from_white_alpha(18));
        let fill = Rect::from_min_max(bar.min, pos2(bar.left() + bar.width() * p.clamp(0.0, 1.0), bar.bottom()));
        painter.rect_filled(fill, 1.0, accent);
    }
    a.resp.clicked()
}

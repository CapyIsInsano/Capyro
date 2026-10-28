//! Menú "Novedades" (botón al lado de ⓘ): los cambios de ESTA versión, y
//! para los anteriores, el enlace al CHANGELOG del repo.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, Id, Key, Modifiers, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui, UiBuilder,
};

use super::{animate_button, CapyroApp};
use crate::changes;
use crate::icons;
use crate::theme::*;

/// Abre una página en el navegador del sistema.
pub(super) fn open_url(url: &str) -> std::io::Result<()> {
    crate::paths::open_url(url)
}

impl CapyroApp {
    pub(super) fn changes_menu(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let t = ctx.animate_bool_with_time(Id::unique("capyro-changes-anim"), self.show_changes, crate::perf::t(0.15));
        if t <= 0.001 {
            return;
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let accent = self.cfg.accent;
        let width = 344.0;
        let inner_w = width - 32.0;

        // contenido: cada punto con su viñeta, ya cortado en renglones
        let release = self.release.clone();
        let mut items = Vec::new();
        let mut content_h = 0.0;
        if let Some(r) = &release {
            for it in &r.items {
                let indent = if it.level == 0 { 14.0 } else { 30.0 };
                let col = if it.level == 0 { TEXT } else { TEXT_DIM };
                let g = ui.painter().layout(it.text.clone(), regular(if it.level == 0 { 12.5 } else { 12.0 }), col, inner_w - indent);
                let gap = if it.level == 0 { 8.0 } else { 4.0 };
                content_h += g.size().y + gap;
                items.push((it.level, indent, g, gap));
            }
        }

        let header_h = 70.0;
        let footer_h = 66.0;
        let anchor = pos2(self.changes_rect.left() - 6.0, self.changes_rect.top() - 10.0);
        // no tapa el header de la ventana
        let max_h = (anchor.y - ui.max_rect().top() - 70.0).max(200.0);
        let size = vec2(width, (header_h + content_h + 8.0 + footer_h).min(max_h));
        let full = Rect::from_min_size(pos2(anchor.x, anchor.y - size.y), size);
        let grown = size * (0.85 + 0.15 * crate::alto::pop(t, ease));
        let panel = Rect::from_min_size(pos2(anchor.x, anchor.y - grown.y), grown);
        let interactive = self.show_changes && t > 0.6;

        let mut painter = ui.painter().clone();
        painter.multiply_opacity(ease);
        if interactive {
            let outside = ui.interact(ui.max_rect(), Id::unique("capyro-changes-outside"), Sense::click());
            if outside.clicked() {
                self.show_changes = false;
            }
            ui.interact(full, Id::unique("capyro-changes-bg"), Sense::click());
        }
        painter.rect_filled(panel.translate(vec2(0.0, 4.0)), 12.0, Color32::from_black_alpha(70));
        painter.rect_filled(panel, 12.0, BG_PANEL);
        painter.rect_stroke(panel, 12.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
        if ease < 0.98 {
            ctx.request_repaint_after(super::FRAME_SMOOTH);
            return;
        }
        crate::alto::reveal(&ctx, Id::unique("capyro-changes-anim"), panel, 12.0, self.show_changes, BG_PANEL, accent);
        let x0 = panel.left() + 16.0;
        let x1 = panel.right() - 16.0;
        let top = panel.top();

        /* encabezado */
        let ic = pos2(x0 + 14.0, top + 30.0);
        painter.circle_filled(ic, 16.0, accent.gamma_multiply(0.16));
        icons::news(&painter, ic, 18.0, accent);
        let name = painter.text(pos2(x0 + 38.0, top + 22.0), Align2::LEFT_CENTER, "Novedades", bold(15.0), TEXT);
        let version = release.as_ref().map(|r| r.version.clone()).unwrap_or_else(|| format!("v{}", crate::ffi::version()));
        let g = painter.layout_no_wrap(version, bold(11.0), accent);
        let chip = Rect::from_min_size(pos2(name.right() + 8.0, top + 22.0 - g.size().y / 2.0 - 3.0), g.size() + vec2(12.0, 6.0));
        painter.rect_filled(chip, chip.height() / 2.0, accent.gamma_multiply(0.16));
        painter.galley(chip.center() - g.size() / 2.0, g, accent);
        let subtitle = release.as_ref().map(|r| r.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| "Lo nuevo de esta versión".to_owned());
        painter.text(pos2(x0 + 38.0, top + 41.0), Align2::LEFT_CENTER, subtitle, regular(12.0), TEXT_DIM);
        painter.line_segment([pos2(x0, top + header_h - 8.0), pos2(x1, top + header_h - 8.0)], Stroke::new(1.0, Color32::from_white_alpha(14)));

        /* lista (con scroll si no entra) */
        let list = Rect::from_min_max(pos2(x0, top + header_h), pos2(x1 + 8.0, panel.bottom() - footer_h));
        let mut child = ui.new_child(UiBuilder::new().max_rect(list));
        ScrollArea::vertical().id_salt("capyro-changes-scroll").auto_shrink([false, false]).show(&mut child, |ui| {
            if items.is_empty() {
                ui.label(egui::RichText::new("No encontré las novedades de esta versión.").color(TEXT_FAINT));
            }
            for (level, indent, g, gap) in items {
                let (r, _) = ui.allocate_exact_size(vec2(inner_w, g.size().y + gap), Sense::hover());
                let p = ui.painter();
                let first_line = r.top() + 9.0;
                if level == 0 {
                    p.circle_filled(pos2(r.left() + 4.0, first_line), 2.6, accent);
                } else {
                    p.line_segment(
                        [pos2(r.left() + 18.0, first_line), pos2(r.left() + 24.0, first_line)],
                        Stroke::new(1.4, TEXT_FAINT),
                    );
                }
                p.galley(pos2(r.left() + indent, r.top()), g, TEXT);
            }
        });

        // si no entra todo, un degradé abajo avisa que hay más (con scroll)
        if content_h > list.height() {
            let steps = 10;
            for i in 0..steps {
                let k = (i + 1) as f32 / steps as f32;
                let y = list.bottom() - 22.0 + 2.2 * i as f32;
                painter.rect_filled(
                    Rect::from_min_max(pos2(list.left(), y), pos2(list.right() - 8.0, y + 2.2)),
                    0.0,
                    BG_PANEL.gamma_multiply(k * k),
                );
            }
        }

        /* pie: las versiones anteriores están en el repo */
        let fy = panel.bottom() - footer_h;
        painter.line_segment([pos2(x0, fy), pos2(x1, fy)], Stroke::new(1.0, Color32::from_white_alpha(14)));
        painter.text(
            pos2(x0, fy + 20.0),
            Align2::LEFT_CENTER,
            "¿Las versiones anteriores? Están en el repositorio:",
            regular(11.5),
            TEXT_FAINT,
        );
        let link = changes::REPO_URL.trim_start_matches("https://");
        let g = painter.layout_no_wrap(link.to_owned(), bold(12.0), accent);
        let lr = Rect::from_min_size(pos2(x0 - 4.0, fy + 32.0), g.size() + vec2(28.0, 8.0));
        let a = animate_button(ui, Id::unique("capyro-changes-link"), lr, interactive);
        if a.hover > 0.001 {
            painter.rect_filled(lr, 6.0, accent.gamma_multiply(0.14 * a.hover));
        }
        let gw = g.size().x;
        painter.galley(pos2(lr.left() + 4.0, lr.center().y - g.size().y / 2.0), g, accent);
        // flechita ↗
        let ac = pos2(lr.left() + 4.0 + gw + 10.0 + 2.0 * a.hover, lr.center().y - 2.0 * a.hover);
        let st = Stroke::new(1.5, accent);
        painter.line_segment([ac + vec2(-4.0, 4.0), ac + vec2(4.0, -4.0)], st);
        painter.line_segment([ac + vec2(-1.0, -4.0), ac + vec2(4.0, -4.0)], st);
        painter.line_segment([ac + vec2(4.0, -4.0), ac + vec2(4.0, 1.0)], st);
        if a.resp.clicked() {
            if let Err(e) = open_url(changes::CHANGELOG_URL) {
                self.play_error();
                self.toast(&ctx, format!("No pude abrir el navegador: {e}"), true);
            } else {
                self.toast(&ctx, "Abriendo el historial completo en el navegador…", false);
            }
        }
        a.resp.on_hover_text(changes::CHANGELOG_URL);

        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.show_changes = false;
        }
    }
}

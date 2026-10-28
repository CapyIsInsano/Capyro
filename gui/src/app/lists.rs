//! Panel "Lista": la Cola (temporal, como siempre) y hasta 3 playlists
//! guardadas, en pestañas.
//!
//!   ┌ [Cola] [Para correr] [Playlist 2] [+] ┐
//!   │ Cola · 5          Guardar en…  Limpiar │
//!   │  1  canción.mp3                  + ✕   │
//!   └────────────────────────────────────────┘

use std::path::Path;

use eframe::egui::{
    pos2, vec2, Align2, Color32, CursorIcon, Id, Key, Modifiers, Pos2, Rect, ScrollArea, Sense, Stroke, StrokeKind,
    TextEdit, Ui, UiBuilder, Vec2,
};

use super::{animate_button, ellipsize, file_name, CapyroApp};
use crate::ffi::State;
use crate::icons;
use crate::playlists::{MAX_NAME, MAX_PLAYLISTS};
use crate::theme::*;

/// A qué se le está eligiendo playlist en el menucito.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum PlTarget {
    /// Una canción de la Cola.
    File(String),
    /// Toda la Cola.
    Queue,
}

pub(super) struct PlPopup {
    pub target: PlTarget,
    pub anchor: Pos2,
}

enum Op {
    Tab(Option<usize>),
    NewPlaylist,
    QueueSelect(usize),
    QueueRemove(usize),
    QueueClear,
    OpenPopup(PlTarget, Pos2),
    ClosePopup,
    /// None = crear una nueva
    PopupPick(Option<usize>),
    PlayList(usize, usize),
    PlRemove(usize, usize),
    StartRename(usize),
    CommitRename(String),
    CancelRename,
    DeleteArm(usize),
    Delete(usize),
}

/// Pastillita de texto (pestañas y botones chicos).
fn pill(painter: &eframe::egui::Painter, rect: Rect, text: &str, bg: Color32, fg: Color32, bold_text: bool) {
    painter.rect_filled(rect, rect.height() / 2.0, bg);
    let font = if bold_text { bold(11.5) } else { regular(11.5) };
    painter.text(rect.center(), Align2::CENTER_CENTER, text, font, fg);
}

impl CapyroApp {
    pub(super) fn playlist_panel(&mut self, ui: &mut Ui, video: Rect) {
        // Entra deslizándose desde la derecha y se desvanece al salir.
        let ctx = ui.ctx().clone();
        let t = ctx.animate_bool_with_time(Id::unique("capyro-playlist-anim"), self.show_playlist, crate::perf::t(0.18));
        if t <= 0.001 {
            self.pl_popup = None;
            self.pl_rename = None;
            return;
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let slide = (1.0 - crate::alto::pop(t, ease)) * 40.0;
        let panel = Rect::from_min_max(
            pos2((video.right() - 300.0).max(video.left() + 8.0) + slide, video.top() + 8.0),
            pos2(video.right() - 8.0 + slide, video.bottom() - 8.0),
        );
        let interactive = self.show_playlist && t > 0.5;
        let mut painter = ui.painter().clone();
        painter.multiply_opacity(ease);
        if interactive {
            // Bloquea los clics para que no lleguen al área de video de abajo.
            ui.interact(panel, Id::unique("capyro-playlist-bg"), Sense::click());
        }
        painter.rect_filled(panel.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(60));
        painter.rect_filled(panel, 10.0, BG_PANEL);
        painter.rect_stroke(panel, 10.0, Stroke::new(1.0, Color32::from_white_alpha(12)), StrokeKind::Inside);

        let accent = self.cfg.accent;
        crate::alto::reveal(&ctx, Id::unique("capyro-playlist-anim"), panel, 10.0, self.show_playlist, BG_PANEL, accent);
        let now = ctx.input(|i| i.time);
        // la pestaña puede apuntar a una playlist que ya no existe
        if self.list_tab.is_some_and(|i| i >= self.playlists.lists.len()) {
            self.list_tab = None;
        }
        let tab = self.list_tab;
        let mut ops: Vec<Op> = Vec::new();
        let x0 = panel.left() + 10.0;
        let x1 = panel.right() - 10.0;

        /* --- pestañas: Cola + playlists + "+" --- */
        let ty = panel.top() + 20.0;
        let n = self.playlists.lists.len();
        let full = self.playlists.is_full();
        let cola_w = 50.0;
        let plus_w = if full { 0.0 } else { 26.0 };
        let gap = 5.0;
        let avail = (x1 - x0) - cola_w - plus_w - gap * (n as f32 + if full { 0.0 } else { 1.0 });
        // ancho de cada pestaña: el natural si entran todas; si no, se recortan
        // las más largas hasta que entren
        let natural: Vec<f32> = self
            .playlists
            .lists
            .iter()
            .map(|pl| painter.layout_no_wrap(pl.name.clone(), bold(11.5), TEXT).size().x + 18.0)
            .collect();
        let mut per = natural.iter().cloned().fold(0.0, f32::max);
        while per > 30.0 && natural.iter().map(|w| w.min(per)).sum::<f32>() > avail {
            per -= 2.0;
        }
        let mut x = x0;
        let mut tabs: Vec<(Option<usize>, String, f32)> = vec![(None, "Cola".to_owned(), cola_w)];
        for (i, pl) in self.playlists.lists.iter().enumerate() {
            // achica el nombre hasta que entre
            let mut name = pl.name.clone();
            let mut chars = name.chars().count();
            loop {
                let w = painter.layout_no_wrap(name.clone(), bold(11.5), TEXT).size().x + 18.0;
                if w <= per || chars <= 3 {
                    tabs.push((Some(i), name, w.min(per).max(30.0)));
                    break;
                }
                chars -= 1;
                name = ellipsize(&pl.name, chars);
            }
        }
        for (id, label, w) in tabs {
            let r = Rect::from_min_size(pos2(x, ty - 11.0), vec2(w, 22.0));
            let a = animate_button(ui, Id::unique(("capyro-pl-tab", id.map_or(usize::MAX, |i| i))), r, interactive);
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-pl-tab-sel", id.map_or(usize::MAX, |i| i))), tab == id, crate::perf::t(0.15));
            let bg = Color32::from_white_alpha((8.0 + 12.0 * a.hover) as u8).lerp_to_gamma(accent, sel);
            let fg = TEXT_DIM.lerp_to_gamma(TEXT, a.hover).lerp_to_gamma(Color32::WHITE, sel);
            pill(&painter, Rect::from_center_size(r.center(), r.size() * (1.0 - 0.05 * a.press)), &label, bg, fg, true);
            if a.resp.clicked() {
                ops.push(Op::Tab(id));
            }
            if let Some(i) = id {
                a.resp.on_hover_text(format!("{} · doble clic en el nombre para renombrarla", self.playlists.lists[i].name));
            } else {
                a.resp.on_hover_text("La cola de ahora (temporal)");
            }
            x += w + gap;
        }
        if !full {
            let r = Rect::from_min_size(pos2(x, ty - 11.0), vec2(plus_w, 22.0));
            let a = animate_button(ui, Id::unique("capyro-pl-new"), r, interactive);
            painter.rect_filled(r, 11.0, Color32::from_white_alpha((8.0 + 14.0 * a.hover) as u8));
            icons::plus(&painter, r.center(), 10.0, a.tint(TEXT_DIM, accent), a.hover * std::f32::consts::FRAC_PI_2, 1.8);
            if a.resp.clicked() {
                ops.push(Op::NewPlaylist);
            }
            a.resp.on_hover_text(format!("Nueva playlist ({} de {MAX_PLAYLISTS})", n + 1));
        }

        /* --- encabezado de la pestaña --- */
        let hy = panel.top() + 52.0;
        let queue = self.core.playlist();
        let current = self.core.playlist_current();
        let current_file = self.core.current_file();
        match tab {
            None => {
                let g = painter.layout_no_wrap(format!("Cola · {}", queue.len()), bold(13.0), TEXT);
                let gw = g.size().x;
                painter.galley(pos2(x0 + 2.0, hy - g.size().y / 2.0), g, TEXT);
                painter.text(pos2(x0 + 8.0 + gw, hy + 1.0), Align2::LEFT_CENTER, "temporal", regular(10.5), TEXT_FAINT);
                if !queue.is_empty() {
                    let clear_pos = pos2(x1 - 2.0, hy);
                    let clear_rect = Rect::from_center_size(clear_pos - vec2(22.0, 0.0), vec2(52.0, 22.0));
                    let clear = animate_button(ui, Id::unique("capyro-playlist-clear"), clear_rect, interactive);
                    if clear.hover > 0.001 {
                        painter.rect_filled(clear_rect, 6.0, ERROR.gamma_multiply(0.15 * clear.hover));
                    }
                    painter.text(clear_pos, Align2::RIGHT_CENTER, "Limpiar", regular(12.0), clear.tint(TEXT_FAINT, ERROR));
                    if clear.resp.clicked() {
                        ops.push(Op::QueueClear);
                    }
                    let save_rect = Rect::from_min_max(pos2(clear_rect.left() - 84.0, hy - 11.0), pos2(clear_rect.left() - 4.0, hy + 11.0));
                    let save = animate_button(ui, Id::unique("capyro-pl-save-queue"), save_rect, interactive);
                    if save.hover > 0.001 {
                        painter.rect_filled(save_rect, 6.0, accent.gamma_multiply(0.15 * save.hover));
                    }
                    painter.text(save_rect.center(), Align2::CENTER_CENTER, "Guardar en…", regular(12.0), save.tint(TEXT_DIM, accent));
                    if save.resp.clicked() {
                        ops.push(Op::OpenPopup(PlTarget::Queue, pos2(save_rect.right(), save_rect.bottom() + 4.0)));
                    }
                    save.resp.on_hover_text("Guardar toda la cola en una playlist");
                }
            }
            Some(ti) => {
                let pl = &self.playlists.lists[ti];
                // botones de la derecha primero (para saber cuánto lugar queda para el nombre)
                let trash_c = pos2(x1 - 10.0, hy);
                let armed = self.pl_delete_armed.is_some_and(|(i, t0)| i == ti && now - t0 < 3.0);
                let mut right_edge;
                if armed {
                    let g = painter.layout_no_wrap("¿Borrar?".to_owned(), bold(11.5), Color32::WHITE);
                    let r = Rect::from_min_max(pos2(x1 - g.size().x - 18.0, hy - 11.0), pos2(x1, hy + 11.0));
                    let a = animate_button(ui, Id::unique("capyro-pl-del-confirm"), r, interactive);
                    painter.rect_filled(r, 11.0, ERROR.lerp_to_gamma(Color32::WHITE, 0.12 * a.hover));
                    painter.galley(r.center() - g.size() / 2.0, g, Color32::WHITE);
                    if a.resp.clicked() {
                        ops.push(Op::Delete(ti));
                    }
                    ctx.request_repaint_after(std::time::Duration::from_millis(250));
                    right_edge = r.left() - 6.0;
                } else {
                    let a = Self::icon_button(ui, "pl-trash", trash_c, 24.0);
                    icons::trash(&painter, trash_c, 14.0 * a.scale(), a.tint(TEXT_FAINT, ERROR));
                    if a.resp.clicked() {
                        ops.push(Op::DeleteArm(ti));
                    }
                    a.resp.on_hover_text("Borrar esta playlist");
                    right_edge = trash_c.x - 16.0;
                }
                if !pl.files.is_empty() {
                    let g = painter.layout_no_wrap("Reproducir".to_owned(), bold(11.5), Color32::WHITE);
                    let r = Rect::from_min_max(pos2(right_edge - g.size().x - 34.0, hy - 12.0), pos2(right_edge, hy + 12.0));
                    let a = animate_button(ui, Id::unique("capyro-pl-play"), r, interactive);
                    let rr = Rect::from_center_size(r.center(), r.size() * (1.0 + 0.03 * a.hover - 0.06 * a.press));
                    painter.rect_filled(rr, 12.0, accent.lerp_to_gamma(Color32::WHITE, 0.12 * a.hover));
                    icons::play_small(&painter, pos2(rr.left() + 14.0, rr.center().y), 10.0, Color32::WHITE);
                    painter.galley(pos2(rr.left() + 24.0, rr.center().y - g.size().y / 2.0), g, Color32::WHITE);
                    if a.resp.clicked() {
                        ops.push(Op::PlayList(ti, 0));
                    }
                    a.resp.on_hover_text("Cargar la playlist en la cola y reproducir");
                    right_edge = r.left() - 6.0;
                }

                // nombre (doble clic = renombrar)
                let name_rect = Rect::from_min_max(pos2(x0, hy - 12.0), pos2(right_edge, hy + 12.0));
                let renaming = self.pl_rename.as_ref().is_some_and(|(i, _, _)| *i == ti);
                if renaming && interactive {
                    let (_, buf, focus) = self.pl_rename.as_mut().expect("renaming");
                    let mut edit_ui = ui.new_child(UiBuilder::new().max_rect(name_rect));
                    let resp = edit_ui.add(
                        TextEdit::singleline(buf)
                            .font(bold(12.5))
                            .char_limit(MAX_NAME)
                            .desired_width(name_rect.width() - 8.0)
                            .margin(vec2(6.0, 3.0)),
                    );
                    if *focus {
                        resp.request_focus();
                        *focus = false;
                    }
                    if resp.lost_focus() {
                        if ctx.input(|i| i.key_pressed(Key::Escape)) {
                            ops.push(Op::CancelRename);
                        } else {
                            ops.push(Op::CommitRename(buf.clone()));
                        }
                    }
                } else {
                    let count = format!(" · {}", pl.files.len());
                    let cg = painter.layout_no_wrap(count, regular(11.0), TEXT_FAINT);
                    let max_w = name_rect.width() - cg.size().x - 4.0;
                    let mut chars = pl.name.chars().count();
                    let mut g = painter.layout_no_wrap(pl.name.clone(), bold(13.0), TEXT);
                    while g.size().x > max_w && chars > 3 {
                        chars -= 1;
                        g = painter.layout_no_wrap(ellipsize(&pl.name, chars), bold(13.0), TEXT);
                    }
                    let nw = g.size().x;
                    let name_hit = Rect::from_min_size(pos2(x0, hy - 11.0), vec2(nw + 4.0, 22.0));
                    let a = animate_button(ui, Id::unique("capyro-pl-name"), name_hit, interactive);
                    if a.hover > 0.001 {
                        painter.rect_filled(name_hit.expand2(vec2(4.0, 0.0)), 5.0, Color32::from_white_alpha((10.0 * a.hover) as u8));
                    }
                    painter.galley(pos2(x0 + 2.0, hy - g.size().y / 2.0), g, TEXT);
                    painter.galley(pos2(x0 + 4.0 + nw, hy - cg.size().y / 2.0 + 1.0), cg, TEXT_FAINT);
                    if a.resp.double_clicked() {
                        ops.push(Op::StartRename(ti));
                    }
                    a.resp.on_hover_text("Doble clic para renombrar");
                }
            }
        }
        painter.line_segment(
            [pos2(x0, panel.top() + 70.0), pos2(x1, panel.top() + 70.0)],
            Stroke::new(1.0, Color32::from_white_alpha(12)),
        );

        /* --- filas --- */
        let list_rect = Rect::from_min_max(pos2(panel.left() + 4.0, panel.top() + 74.0), panel.max - vec2(4.0, 6.0));
        let (rows, highlight): (Vec<String>, Option<usize>) = match tab {
            None => (queue.clone(), current),
            Some(ti) => {
                let files = self.playlists.lists[ti].files.clone();
                let hl = files.iter().position(|f| *f == current_file);
                (files, hl)
            }
        };
        if rows.is_empty() {
            let msg = match tab {
                None => "La cola está vacía",
                Some(_) => "Vacía · tocá + en una canción de la Cola",
            };
            painter.text(list_rect.center(), Align2::CENTER_CENTER, msg, regular(12.0), TEXT_FAINT);
        } else {
            let mut child = ui.new_child(UiBuilder::new().max_rect(list_rect));
            child.multiply_opacity(ease);
            if !interactive || self.pl_popup.is_some() {
                child.disable();
            }
            let in_queue = tab.is_none();
            ScrollArea::vertical()
                .id_salt(("capyro-pl-scroll", tab.map_or(usize::MAX, |i| i)))
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    for (i, path) in rows.iter().enumerate() {
                        let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
                        // el hover se mide sobre la fila entera (ver v0.2.2: la ✕ se escondía)
                        let row_hovered = interactive && ui.is_enabled() && ui.rect_contains_pointer(row);
                        let hover = ui.ctx().animate_bool_with_time(Id::unique(("capyro-row-hover", in_queue, i)), row_hovered, crate::perf::t(0.12));
                        let is_current = highlight == Some(i);
                        let missing = !in_queue && !Path::new(path).exists();
                        let p = ui.painter();
                        if is_current {
                            p.rect_filled(row, 6.0, accent.gamma_multiply(0.14 + 0.06 * hover));
                            p.rect_filled(Rect::from_min_size(row.min + vec2(0.0, 7.0), vec2(3.0, 16.0)), 1.5, accent);
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
                        let name_chars = if hover > 0.5 && in_queue { 24 } else { 28 };
                        p.text(
                            pos2(row.left() + 34.0 + 3.0 * hover, row.center().y),
                            Align2::LEFT_CENTER,
                            ellipsize(&file_name(path), name_chars),
                            regular(12.5),
                            if missing {
                                TEXT_FAINT
                            } else if is_current {
                                accent
                            } else {
                                TEXT
                            },
                        );

                        let mut used = false;
                        if hover > 0.001 {
                            // ✕ quitar
                            let xc = pos2(row.right() - 14.0, row.center().y);
                            let x = animate_button(ui, Id::unique(("capyro-pl-rm", in_queue, i)), Rect::from_center_size(xc, Vec2::splat(22.0)), row_hovered);
                            if x.hover > 0.001 {
                                ui.painter().circle_filled(xc, 10.0, ERROR.gamma_multiply(0.2 * x.hover));
                            }
                            icons::close(ui.painter(), xc, 9.0 * x.scale(), x.tint(TEXT_FAINT, ERROR).gamma_multiply(hover));
                            if x.resp.clicked() {
                                used = true;
                                ops.push(match tab {
                                    None => Op::QueueRemove(i),
                                    Some(ti) => Op::PlRemove(ti, i),
                                });
                            }
                            x.resp.on_hover_text(if in_queue { "Quitar de la cola" } else { "Quitar de la playlist" });

                            // + agregar a una playlist (solo en la Cola)
                            if in_queue {
                                let pc = pos2(row.right() - 38.0, row.center().y);
                                let a = animate_button(ui, Id::unique(("capyro-pl-add", i)), Rect::from_center_size(pc, Vec2::splat(22.0)), row_hovered);
                                if a.hover > 0.001 {
                                    ui.painter().circle_filled(pc, 10.0, accent.gamma_multiply(0.2 * a.hover));
                                }
                                icons::plus(ui.painter(), pc, 10.0 * a.scale(), a.tint(TEXT_FAINT, accent).gamma_multiply(hover), 0.0, 1.6);
                                if a.resp.clicked() {
                                    used = true;
                                    ops.push(Op::OpenPopup(PlTarget::File(path.clone()), pos2(pc.x + 11.0, row.bottom())));
                                }
                                a.resp.on_hover_text("Agregar a una playlist");
                            }
                        }
                        if row_hovered {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        if !used && resp.clicked() {
                            ops.push(match tab {
                                None => Op::QueueSelect(i),
                                Some(ti) => Op::PlayList(ti, i),
                            });
                        }
                        if missing {
                            resp.on_hover_text("Este archivo ya no está en el disco");
                        }
                    }
                });
        }

        /* --- menucito "agregar a…" --- */
        if let Some(popup) = &self.pl_popup {
            let title = match &popup.target {
                PlTarget::File(_) => "Agregar a…",
                PlTarget::Queue => "Guardar la cola en…",
            };
            let lists: Vec<(String, usize)> = self.playlists.lists.iter().map(|p| (p.name.clone(), p.files.len())).collect();
            let rows = lists.len() + usize::from(!full);
            let w = 200.0;
            let h = 34.0 + 30.0 * rows as f32 + 6.0;
            let mut r = Rect::from_min_size(pos2(popup.anchor.x - w, popup.anchor.y), vec2(w, h));
            // que no se salga del panel
            if r.bottom() > panel.bottom() - 4.0 {
                r = r.translate(vec2(0.0, panel.bottom() - 4.0 - r.bottom()));
            }
            if r.left() < panel.left() + 4.0 {
                r = r.translate(vec2(panel.left() + 4.0 - r.left(), 0.0));
            }
            let outside = ui.interact(ui.max_rect(), Id::unique("capyro-pl-popup-outside"), Sense::click());
            if outside.clicked() {
                ops.push(Op::ClosePopup);
            }
            ui.interact(r, Id::unique("capyro-pl-popup-bg"), Sense::click());
            painter.rect_filled(r.translate(vec2(0.0, 4.0)), 10.0, Color32::from_black_alpha(80));
            painter.rect_filled(r, 10.0, BG_WINDOW);
            painter.rect_stroke(r, 10.0, Stroke::new(1.0, Color32::from_white_alpha(18)), StrokeKind::Inside);
            painter.text(pos2(r.left() + 12.0, r.top() + 18.0), Align2::LEFT_CENTER, title, bold(12.0), TEXT);
            let mut y = r.top() + 34.0;
            for (i, (name, count)) in lists.iter().enumerate() {
                let row = Rect::from_min_size(pos2(r.left() + 4.0, y), vec2(w - 8.0, 28.0));
                let a = animate_button(ui, Id::unique(("capyro-pl-pick", i)), row, true);
                if a.hover > 0.001 {
                    painter.rect_filled(row, 6.0, accent.gamma_multiply(0.16 * a.hover));
                }
                painter.text(pos2(row.left() + 10.0, row.center().y), Align2::LEFT_CENTER, ellipsize(name, 20), regular(12.5), a.tint(TEXT, Color32::WHITE));
                painter.text(pos2(row.right() - 10.0, row.center().y), Align2::RIGHT_CENTER, count.to_string(), regular(11.0), TEXT_FAINT);
                if a.resp.clicked() {
                    ops.push(Op::PopupPick(Some(i)));
                }
                y += 30.0;
            }
            if !full {
                let row = Rect::from_min_size(pos2(r.left() + 4.0, y), vec2(w - 8.0, 28.0));
                let a = animate_button(ui, Id::unique("capyro-pl-pick-new"), row, true);
                if a.hover > 0.001 {
                    painter.rect_filled(row, 6.0, accent.gamma_multiply(0.16 * a.hover));
                }
                icons::plus(&painter, pos2(row.left() + 16.0, row.center().y), 10.0, accent, 0.0, 1.8);
                painter.text(pos2(row.left() + 28.0, row.center().y), Align2::LEFT_CENTER, "Nueva playlist", regular(12.5), accent);
                if a.resp.clicked() {
                    ops.push(Op::PopupPick(None));
                }
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                ops.push(Op::ClosePopup);
            }
        }

        for op in ops {
            self.apply_list_op(&ctx, op);
        }
    }

    fn save_playlists(&mut self, ctx: &eframe::egui::Context) {
        if let Err(e) = self.playlists.save() {
            self.toast(ctx, format!("No pude guardar las playlists: {e}"), true);
        }
    }

    fn apply_list_op(&mut self, ctx: &eframe::egui::Context, op: Op) {
        match op {
            Op::Tab(t) => {
                self.list_tab = t;
                self.pl_rename = None;
                self.pl_popup = None;
            }
            Op::NewPlaylist => match self.playlists.create() {
                Some(i) => {
                    self.save_playlists(ctx);
                    self.list_tab = Some(i);
                    // recién creada: directo a ponerle nombre
                    self.pl_rename = Some((i, self.playlists.lists[i].name.clone(), true));
                }
                None => self.toast(ctx, format!("Por ahora el máximo es {MAX_PLAYLISTS} playlists"), false),
            },
            Op::QueueSelect(i) => match self.core.playlist_select(i) {
                Ok(()) => {
                    if self.core.state() != State::Playing {
                        self.core.toggle();
                    }
                }
                Err(e) => self.toast(ctx, e.0, true),
            },
            Op::QueueRemove(i) => self.core.playlist_remove(i),
            Op::QueueClear => self.core.playlist_clear(),
            Op::OpenPopup(target, anchor) => {
                if self.playlists.lists.is_empty() {
                    // sin playlists todavía: se crea una y se agrega directo
                    self.pl_popup = Some(PlPopup { target, anchor });
                    self.apply_list_op(ctx, Op::PopupPick(None));
                } else {
                    self.pl_popup = Some(PlPopup { target, anchor });
                }
            }
            Op::ClosePopup => self.pl_popup = None,
            Op::PopupPick(idx) => {
                let Some(popup) = self.pl_popup.take() else { return };
                let idx = match idx {
                    Some(i) => i,
                    None => match self.playlists.create() {
                        Some(i) => i,
                        None => {
                            self.toast(ctx, format!("Por ahora el máximo es {MAX_PLAYLISTS} playlists"), false);
                            return;
                        }
                    },
                };
                let files = match popup.target {
                    PlTarget::File(f) => vec![f],
                    PlTarget::Queue => self.core.playlist(),
                };
                let added = self.playlists.add(idx, &files);
                self.save_playlists(ctx);
                let name = self.playlists.lists[idx].name.clone();
                let msg = match (added, files.len()) {
                    (0, 1) => format!("Ya estaba en «{name}»"),
                    (0, _) => format!("Ya estaban todas en «{name}»"),
                    (1, 1) => format!("Agregada a «{name}»"),
                    (n, _) => format!("{n} canciones agregadas a «{name}»"),
                };
                self.toast(ctx, msg, false);
            }
            Op::PlayList(ti, start) => {
                let Some(pl) = self.playlists.lists.get(ti).cloned() else { return };
                let wanted = pl.files.get(start).cloned();
                self.core.playlist_clear();
                let mut missing = 0;
                let mut start_at = None;
                let mut added = 0;
                for f in &pl.files {
                    let path = Path::new(f);
                    if path.exists() && self.core.playlist_add(path).is_ok() {
                        if Some(f) == wanted.as_ref() {
                            start_at = Some(added);
                        }
                        added += 1;
                    } else {
                        missing += 1;
                    }
                }
                if added == 0 {
                    self.play_error();
                    self.toast(ctx, format!("Ningún archivo de «{}» está en el disco", pl.name), true);
                    return;
                }
                let first = start_at.unwrap_or(0);
                if let Err(e) = self.core.playlist_select(first) {
                    self.toast(ctx, e.0, true);
                    return;
                }
                if self.core.state() != State::Playing {
                    self.core.toggle();
                }
                if missing > 0 {
                    let s = if missing == 1 { "" } else { "s" };
                    self.toast(ctx, format!("«{}»: {missing} archivo{s} ya no está{} en el disco", pl.name, if missing == 1 { "" } else { "n" }), true);
                } else {
                    self.toast(ctx, format!("Reproduciendo «{}»", pl.name), false);
                }
            }
            Op::PlRemove(ti, i) => {
                self.playlists.remove_file(ti, i);
                self.save_playlists(ctx);
            }
            Op::StartRename(ti) => {
                let name = self.playlists.lists[ti].name.clone();
                self.pl_rename = Some((ti, name, true));
            }
            Op::CommitRename(name) => {
                if let Some((ti, _, _)) = self.pl_rename.take() {
                    if self.playlists.rename(ti, &name) {
                        self.save_playlists(ctx);
                    }
                }
            }
            Op::CancelRename => self.pl_rename = None,
            Op::DeleteArm(ti) => self.pl_delete_armed = Some((ti, ctx.input(|i| i.time))),
            Op::Delete(ti) => {
                let name = self.playlists.lists.get(ti).map(|p| p.name.clone()).unwrap_or_default();
                self.playlists.delete(ti);
                self.pl_delete_armed = None;
                self.pl_rename = None;
                self.list_tab = None;
                self.save_playlists(ctx);
                self.toast(ctx, format!("Playlist «{name}» borrada"), false);
            }
        }
    }
}

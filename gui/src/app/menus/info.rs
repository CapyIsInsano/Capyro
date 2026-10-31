//! Menú ⓘ: versión, formatos soportados, actualizaciones, borrar caché y
//! desinstalar.

use crate::app::*;

/// Lo que vuelve del hilo de "Borrar caché".
pub(crate) enum CacheMsg {
    Size(u64),
    Done(crate::system::cache::Cleaned),
}

impl CapyroApp {
    /// Menú de información: versión, formatos soportados y algunos datos.
    /// Crece desde el botón ⓘ (abajo a la izquierda), igual que el de ajustes.
    /// Con `dock`, fijo en la burbuja de funciones de la vista de música
    /// (con los formatos ya abiertos: ahí hay lugar). Devuelve el alto.
    pub(crate) fn info_menu(&mut self, ui: &mut Ui, dock: Option<Rect>) -> f32 {
        let ctx = ui.ctx().clone();
        let docked = dock.is_some();
        let t = if docked { 1.0 } else { ctx.animate_bool_with_time(Id::unique("capyro-info-anim"), self.show_info, crate::system::perf::t(0.15)) };
        if t <= 0.001 {
            self.info_format = None; // al volver a abrirlo arranca sin resumen abierto
            self.info_formats_open = false; // y con la lista de formatos cerrada
            return 0.0;
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let accent = self.cfg.accent;
        let width = dock.map_or(312.0, |d| d.width());
        let inner_w = width - 28.0;
        // (encabezado 64 + botón de formatos 36 + actualizaciones 54 + caché 36 + desinstalar 36 + pie 34)
        let base_h = 260.0;

        // --- resúmenes (acordeón): se calcula antes para saber el alto total ---
        // Cada formato tiene su propia animación; como solo uno puede estar
        // abierto, al cambiar de MP4 a MP3 uno se cierra mientras el otro se abre.
        let formats = self.formats.clone();
        let all: Vec<(String, bool)> = formats
            .0
            .iter()
            .map(|e| (e.clone(), true))
            .chain(formats.1.iter().map(|e| (e.clone(), false)))
            .collect();
        let mut cards = Vec::new();
        let mut region_h = 0.0;
        for (ext, is_video) in &all {
            let open = self.info_format.as_deref() == Some(ext.as_str());
            let k = ctx.animate_bool_with_time(Id::unique(("capyro-fmt-open", ext.as_str())), open, crate::system::perf::t(0.22));
            if k <= 0.001 {
                continue;
            }
            let k = 1.0 - (1.0 - k).powi(3);
            let info = crate::media::formats::info(ext);
            let kind = if *is_video { "Video" } else { "Audio" };
            let title = ui.painter().layout_no_wrap(
                format!("{} · {}", ext.to_uppercase(), info.name),
                bold(12.0),
                TEXT,
            );
            let kind_g = ui.painter().layout_no_wrap(kind.to_owned(), regular(10.5), TEXT_FAINT);
            let body = ui.painter().layout(info.summary.to_owned(), regular(11.5), TEXT_DIM, inner_w - 20.0);
            let card_h = 10.0 + title.size().y + 5.0 + body.size().y + 10.0;
            region_h += (card_h + 8.0) * k;
            cards.push((k, card_h, title, kind_g, body));
        }

        // --- la lista de formatos (se despliega con el botón) ---
        // Los chips se acomodan en renglones: con 13 formatos ya no entran
        // en una sola fila por tipo.
        let fopen = if docked {
            1.0
        } else {
            ctx.animate_bool_with_time(Id::unique("capyro-fmts-open"), self.info_formats_open, crate::system::perf::t(0.26))
        };
        let fk = 1.0 - (1.0 - fopen).powi(3);
        let mut chip_layout: Vec<(String, Rect)> = Vec::new();
        let mut group_labels: Vec<(&str, f32, bool, usize)> = Vec::new();
        let mut ry = 4.0;
        for (label, list, is_video) in [("Video", &formats.0, true), ("Audio", &formats.1, false)] {
            group_labels.push((label, ry + 8.0, is_video, list.len()));
            ry += 22.0;
            let mut cx = 0.0;
            for ext in list {
                let w = ui.painter().layout_no_wrap(ext.to_uppercase(), bold(11.0), TEXT).size().x + 16.0;
                if cx > 0.0 && cx + w > inner_w {
                    cx = 0.0;
                    ry += 28.0;
                }
                chip_layout.push((ext.clone(), Rect::from_min_size(pos2(cx, ry), vec2(w, 22.0))));
                cx += w + 6.0;
            }
            ry += 22.0 + 12.0;
        }
        let chips_h = ry;
        // los resúmenes van debajo de los chips, adentro de lo desplegado
        let region_h = (chips_h + region_h) * fk;

        let size = vec2(width, base_h + region_h);
        let (panel, interactive, painter) = if let Some(d) = dock {
            (Rect::from_min_size(d.min, size), true, ui.painter().clone())
        } else {
        let anchor = pos2(self.info_rect.left() - 6.0, self.info_rect.top() - 10.0);
        let full = Rect::from_min_size(pos2(anchor.x, anchor.y - size.y), size);
        let grown = size * (0.85 + 0.15 * crate::ui::alto::pop(t, ease));
        let panel = Rect::from_min_size(pos2(anchor.x, anchor.y - grown.y), grown);
        let interactive = self.show_info && t > 0.6;

        let mut painter = ui.painter().clone();
        painter.multiply_opacity(ease);
        if interactive {
            let outside = ui.interact(ui.max_rect(), Id::unique("capyro-info-outside"), Sense::click());
            if outside.clicked() {
                self.show_info = false;
            }
            ui.interact(full, Id::unique("capyro-info-bg"), Sense::click());
        }
        crate::ui::theme::soft_shadow(&painter, panel, 12.0, 8.0, 2.0, 70);
        painter.rect_filled(panel, 12.0, BG_PANEL);
        painter.rect_stroke(panel, 12.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
        if ease < 0.98 {
            ctx.request_repaint_after(FRAME_SMOOTH);
            return 0.0;
        }
        crate::ui::alto::reveal(&ctx, Id::unique("capyro-info-anim"), panel, 12.0, self.show_info, BG_PANEL, accent);
        (panel, interactive, painter)
        };

        let x0 = panel.left() + 14.0;
        let x1 = panel.right() - 14.0;
        let top = panel.top();

        // encabezado: logo + nombre + versión
        let logo_rect = Rect::from_min_size(pos2(x0, top + 14.0), vec2(40.0, 40.0));
        if let Some(logo) = &self.logo {
            painter.image(logo.id(), logo_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let tx = logo_rect.right() + 10.0;
        let name = painter.text(pos2(tx, top + 25.0), Align2::LEFT_CENTER, "Capyro", bold(15.0), TEXT);
        let g = painter.layout_no_wrap(format!("v{}", ffi::version()), bold(11.0), accent);
        let chip = Rect::from_min_size(pos2(name.right() + 8.0, top + 25.0 - g.size().y / 2.0 - 3.0), g.size() + vec2(12.0, 6.0));
        painter.rect_filled(chip, chip.height() / 2.0, accent.gamma_multiply(0.16));
        painter.galley(chip.center() - g.size() / 2.0, g, accent);
        painter.text(pos2(tx, top + 44.0), Align2::LEFT_CENTER, "Just press play.", regular(12.0), TEXT_DIM);

        // formatos: un botón que despliega la lista (son muchos para que
        // estén siempre a la vista)
        painter.line_segment([pos2(x0, top + 64.0), pos2(x1, top + 64.0)], Stroke::new(1.0, Color32::from_white_alpha(14)));
        let row = Rect::from_min_max(pos2(x0 - 6.0, top + 67.0), pos2(x1 + 6.0, top + 97.0));
        // (en la burbuja de la vista de música no es botón: la lista ya está abierta)
        let fb = animate_button(ui, Id::unique("capyro-fmts-toggle"), row, interactive && !docked);
        if fb.hover > 0.001 || fb.press > 0.001 || fk > 0.001 {
            let a = 10.0 * fb.hover + 10.0 * fb.press + 6.0 * fk;
            painter.rect_filled(row, 8.0, Color32::from_white_alpha(a as u8));
        }
        let cy = row.center().y;
        let icol = fb.tint(TEXT_DIM, TEXT).lerp_to_gamma(accent, fk);
        icons::film(&painter, pos2(x0 + 6.0, cy), 13.0, icol);
        icons::music_note(&painter, pos2(x0 + 21.0, cy), 13.0, icol);
        let label = if docked {
            "Formatos soportados"
        } else if self.info_formats_open {
            "Formatos soportados (cerrar)"
        } else {
            "Formatos soportados (abrir)"
        };
        let lr = painter.text(pos2(x0 + 34.0, cy), Align2::LEFT_CENTER, label, bold(12.5), TEXT);
        // a la derecha: cuántos son y una flechita que gira al abrir
        let total = formats.0.len() + formats.1.len();
        let chev = pos2(x1 - 4.0, cy);
        let st = Stroke::new(1.5, fb.tint(TEXT_FAINT, TEXT).lerp_to_gamma(accent, fk));
        let (dy, sx) = (3.0 * (1.0 - 2.0 * fk), 4.5);
        if !docked {
            painter.line_segment([chev + vec2(-sx, -dy), chev + vec2(0.0, dy)], st);
            painter.line_segment([chev + vec2(sx, -dy), chev + vec2(0.0, dy)], st);
        }
        let ng = painter.layout_no_wrap(total.to_string(), bold(10.5), accent);
        // la cantidad, al lado del texto (si entra antes de la flechita)
        let nchip = Rect::from_min_size(pos2(lr.right() + 7.0, cy - 9.0), vec2(ng.size().x + 12.0, 18.0));
        if nchip.right() < chev.x - 10.0 {
            painter.rect_filled(nchip, 9.0, accent.gamma_multiply(0.16));
            painter.galley(nchip.center() - ng.size() / 2.0, ng, accent);
        }
        if fb.hover > 0.5 {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        if fb.resp.clicked() && !docked {
            self.info_formats_open = !self.info_formats_open;
            if !self.info_formats_open {
                self.info_format = None;
            }
        }
        let fb_resp = fb.resp;
        fb_resp.on_hover_text(format!(
            "Video: {}\nAudio: {}",
            formats.0.join(", ").to_uppercase(),
            formats.1.join(", ").to_uppercase()
        ));

        // lo desplegado: los chips (tocá uno para ver qué es) y los resúmenes
        let region = Rect::from_min_size(pos2(x0, top + 100.0), vec2(inner_w, region_h));
        let clipped = painter.with_clip_rect(region.expand2(vec2(4.0, 0.0)).intersect(painter.clip_rect()));
        let mut clicked: Option<String> = None;
        if fk > 0.001 {
            let mut p = clipped.clone();
            p.multiply_opacity(fk);
            let origin = region.min + vec2(0.0, -8.0 * (1.0 - fk));
            let chips_live = interactive && fk > 0.9;
            for (label, ly, is_video, n) in &group_labels {
                let ic = origin + vec2(7.0, *ly);
                if *is_video {
                    icons::film(&p, ic, 13.0, TEXT_DIM);
                } else {
                    icons::music_note(&p, ic, 13.0, TEXT_DIM);
                }
                p.text(origin + vec2(20.0, *ly), Align2::LEFT_CENTER, *label, regular(12.0), TEXT_DIM);
                let hint = if *is_video { format!("{n} formatos · tocá uno") } else { format!("{n} formatos") };
                p.text(pos2(x1, origin.y + *ly), Align2::RIGHT_CENTER, hint, regular(10.5), TEXT_FAINT);
            }
            for (ext, rel) in &chip_layout {
                let r = rel.translate(origin.to_vec2());
                let selected = self.info_format.as_deref() == Some(ext.as_str());
                let g = p.layout_no_wrap(ext.to_uppercase(), bold(11.0), TEXT);
                let a = animate_button(ui, Id::unique(("capyro-fmt", ext.as_str())), r, chips_live);
                let sel = ctx.animate_bool_with_time(Id::unique(("capyro-fmt-sel", ext.as_str())), selected, crate::system::perf::t(0.15));
                let rr = Rect::from_center_size(r.center(), r.size() * (1.0 + 0.04 * a.hover - 0.08 * a.press));
                let bg = Color32::from_white_alpha((10.0 + 12.0 * a.hover) as u8).lerp_to_gamma(accent, sel);
                p.rect_filled(rr, 6.0, bg);
                let col = TEXT.lerp_to_gamma(Color32::WHITE, sel);
                p.galley(rr.center() - g.size() / 2.0, g, col);
                if a.resp.clicked() {
                    clicked = Some(ext.clone());
                }
            }

            // resúmenes (acordeón), debajo de los chips
            let mut cy = origin.y + chips_h;
            for (k, card_h, title, kind_g, body) in cards {
                let mut pc = p.clone();
                pc.multiply_opacity(k);
                let card = Rect::from_min_size(pos2(x0, cy - 6.0 * (1.0 - k)), vec2(inner_w, card_h));
                pc.rect_filled(card, 8.0, Color32::from_white_alpha(9));
                pc.rect_filled(Rect::from_min_size(card.min + vec2(0.0, 10.0), vec2(3.0, card_h - 20.0)), 1.5, accent);
                let ty = card.top() + 10.0;
                pc.galley(pos2(card.left() + 12.0, ty), title.clone(), TEXT);
                pc.galley(pos2(card.right() - 10.0 - kind_g.size().x, ty + 1.0), kind_g, TEXT_FAINT);
                pc.galley(pos2(card.left() + 12.0, ty + title.size().y + 5.0), body, TEXT_DIM);
                cy += (card_h + 8.0) * k;
            }
        }
        if (0.001..0.999).contains(&fopen) {
            ctx.request_repaint_after(FRAME_SMOOTH);
        }
        if let Some(ext) = clicked {
            // solo uno abierto a la vez; tocar el abierto lo cierra
            self.info_format = if self.info_format.as_deref() == Some(ext.as_str()) { None } else { Some(ext) };
        }

        // actualizaciones
        let upd_top = region.bottom() + 2.0;
        painter.line_segment([pos2(x0, upd_top), pos2(x1, upd_top)], Stroke::new(1.0, Color32::from_white_alpha(14)));
        self.update_row(ui, &painter, Rect::from_min_max(pos2(x0 - 6.0, upd_top + 6.0), pos2(x1 + 6.0, upd_top + 48.0)), interactive);
        // borrar caché y desinstalar (abajo de actualizaciones, arriba del pie)
        let cache_row = Rect::from_min_max(pos2(x0 - 6.0, upd_top + 50.0), pos2(x1 + 6.0, upd_top + 84.0));
        self.cache_row(ui, &painter, cache_row, interactive);
        let un_row = Rect::from_min_max(pos2(x0 - 6.0, upd_top + 86.0), pos2(x1 + 6.0, upd_top + 120.0));
        self.uninstall_row(ui, &painter, un_row, interactive);

        // pie: backend de audio y licencia
        let backend = self.core.audio_backend();
        let audio = if backend.is_empty() { "sin dispositivo".to_owned() } else { backend };
        let foot = format!("Audio: {audio}  ·  GPL-3.0  ·  {} ({})", self.cfg.system().name(), self.build_info.system);
        let foot = fit_text(&painter, &foot, regular(10.5), x1 - x0);
        painter.text(pos2(x0, panel.bottom() - 16.0), Align2::LEFT_CENTER, foot, regular(10.5), TEXT_FAINT);

        if !docked && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.show_info = false;
        }
        size.y
    }

    /// Fila "Buscar actualizaciones" del menú ⓘ, con su progreso y resultado.
    /// Botón "Desinstalar Capyro" del menú ⓘ. El primer clic lo arma (dice
    /// "Tocá otra vez"); el segundo, dentro de 4 s, abre el desinstalador
    /// de este sistema y cierra Capyro.
    pub(crate) fn uninstall_row(&mut self, ui: &mut Ui, painter: &egui::Painter, row: Rect, interactive: bool) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let armed = self.uninstall_armed.is_some_and(|t0| now - t0 < 4.0);
        let btn = animate_button(ui, Id::unique("capyro-uninstall-row"), row, interactive);
        let tint = if armed { theme::ERROR } else { TEXT_DIM };
        if armed {
            painter.rect_filled(row, 8.0, theme::ERROR.gamma_multiply(0.14));
        } else if btn.hover > 0.001 || btn.press > 0.001 {
            painter.rect_filled(row, 8.0, Color32::from_white_alpha((10.0 * btn.hover + 10.0 * btn.press) as u8));
        }
        let cy = row.center().y;
        let x0 = row.left() + 6.0;
        icons::trash(painter, pos2(x0 + 8.0, cy), 13.0, tint);
        let label = if armed { "Tocá otra vez para desinstalar" } else { "Desinstalar Capyro" };
        painter.text(pos2(x0 + 26.0, cy), Align2::LEFT_CENTER, label, bold(12.5), if armed { theme::ERROR } else { TEXT });
        if btn.hover > 0.5 {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        if armed {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        let btn_resp = btn.resp.on_hover_text(if cfg!(windows) {
            "Abre el desinstalador de Windows (el mismo de Aplicaciones instaladas)"
        } else {
            "Abre el desinstalador de Capyro (capyro-uninstall, junto al programa)"
        });
        if btn_resp.clicked() {
            if armed {
                self.uninstall_armed = None;
                match crate::system::uninstall::launch() {
                    Ok(crate::system::uninstall::Outcome::Done) => ctx.send_viewport_cmd(ViewportCommand::Close),
                    Ok(crate::system::uninstall::Outcome::NeedsRoot(cmd)) => {
                        ctx.copy_text(cmd.clone());
                        self.toast_for(&ctx, format!("Copié, pegalo en una terminal: {cmd}"), false, 10.0);
                    }
                    Err(e) => self.toast(&ctx, e, true),
                }
            } else {
                self.uninstall_armed = Some(now);
            }
        }
    }

    /// "Borrar caché": el primer toque calcula cuánto ocupa (en otro hilo) y
    /// arma; el segundo borra (también en otro hilo: puede ser mucho).
    pub(crate) fn cache_row(&mut self, ui: &mut Ui, painter: &egui::Painter, row: Rect, interactive: bool) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        // llegó algo del hilo
        if let Some(rx) = &self.cache_job {
            match rx.try_recv() {
                Ok(CacheMsg::Size(0)) => {
                    self.cache_job = None;
                    self.toast(&ctx, "La caché ya está vacía", false);
                }
                Ok(CacheMsg::Size(bytes)) => {
                    self.cache_job = None;
                    self.cache_armed = Some((now, bytes));
                }
                Ok(CacheMsg::Done(c)) => {
                    self.cache_job = None;
                    self.cache_armed = None;
                    if c.failed.is_empty() {
                        self.toast(&ctx, format!("Caché borrada: se liberaron {}", crate::system::cache::human(c.freed)), false);
                    } else {
                        for f in &c.failed {
                            crate::system::errlog::log(&format!("borrar caché: {f}"));
                        }
                        let msg = format!(
                            "Se liberaron {}, pero {} cosa(s) no se pudieron borrar (están en uso)",
                            crate::system::cache::human(c.freed),
                            c.failed.len()
                        );
                        self.toast_for(&ctx, msg, true, 5.0);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(100)),
                Err(mpsc::TryRecvError::Disconnected) => self.cache_job = None,
            }
        }
        let busy = self.cache_job.is_some();
        let updating = self.updater.is_running();
        let armed = !busy && self.cache_armed.is_some_and(|(t0, _)| now - t0 < 5.0);
        let btn = animate_button(ui, Id::unique("capyro-cache-row"), row, interactive && !busy && !updating);
        let tint = if armed { theme::ERROR } else if updating { TEXT_FAINT } else { TEXT_DIM };
        if armed {
            painter.rect_filled(row, 8.0, theme::ERROR.gamma_multiply(0.14));
        } else if btn.hover > 0.001 || btn.press > 0.001 {
            painter.rect_filled(row, 8.0, Color32::from_white_alpha((10.0 * btn.hover + 10.0 * btn.press) as u8));
        }
        let cy = row.center().y;
        let x0 = row.left() + 6.0;
        if busy {
            icons::spinner(painter, pos2(x0 + 8.0, cy), 13.0, TEXT_DIM, crate::system::perf::clock(now));
        } else {
            icons::sparkle(painter, pos2(x0 + 8.0, cy), 6.5, 0.0, tint);
        }
        let label = match (busy, armed) {
            (true, _) if self.cache_armed.is_some() => "Borrando la caché…".to_owned(),
            (true, _) => "Calculando cuánto ocupa…".to_owned(),
            (false, true) => {
                let bytes = self.cache_armed.map_or(0, |(_, b)| b);
                format!("Tocá otra vez para borrar {}", crate::system::cache::human(bytes))
            }
            (false, false) => "Borrar caché".to_owned(),
        };
        let col = if armed { theme::ERROR } else if updating { TEXT_FAINT } else { TEXT };
        let label = fit_text(painter, &label, bold(12.5), row.right() - x0 - 32.0);
        painter.text(pos2(x0 + 26.0, cy), Align2::LEFT_CENTER, label, bold(12.5), col);
        if armed || busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        let hint = if updating {
            "Se está actualizando Capyro: esperá a que termine"
        } else {
            "Borra lo que Capyro guarda para no repetirlo: el código y la compilación del actualizador, la canción del tutorial, los registros y lo que dejan las pruebas. Tu configuración, playlists y capturas no se tocan"
        };
        let resp = btn.resp.on_hover_text(hint);
        if resp.clicked() && !busy && !updating {
            let (tx, rx) = mpsc::channel();
            let wake = ctx.clone();
            if armed {
                // segundo toque: borrar (queda armado para mostrar "Borrando…")
                std::thread::spawn(move || {
                    let _ = tx.send(CacheMsg::Done(crate::system::cache::clear()));
                    wake.request_repaint();
                });
            } else {
                // primer toque: ver cuánto ocupa antes de armar
                self.cache_armed = None;
                std::thread::spawn(move || {
                    let _ = tx.send(CacheMsg::Size(crate::system::cache::size()));
                    wake.request_repaint();
                });
            }
            self.cache_job = Some(rx);
        }
        if !busy && !armed && self.cache_armed.is_some() {
            self.cache_armed = None; // se venció el segundo toque
        }
    }
}

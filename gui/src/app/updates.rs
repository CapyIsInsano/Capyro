//! Actualizaciones: buscar, instalar y reiniciar, la fila de ⓘ, el aviso
//! "Hay una actualización" y el cartel con el parche recién instalado.

use super::*;

/// Cuánto se ve el aviso con el parche recién instalado.
pub(crate) const BANNER_TIME: f64 = 9.0;

/// Aviso grande con el parche que se acaba de instalar.
pub(crate) struct UpdateBanner {
    start: f64,
    until: f64,
    commits: Vec<CommitInfo>,
    total: usize,
}

impl CapyroApp {
    /// Botón "Buscar actualizaciones" del menú ⓘ. Primero busca y muestra qué
    /// commits hay; con `install` (botón "Instalar" del aviso) compila e instala.
    /// Si ya se instaló una, el mismo botón reinicia Capyro.
    pub(crate) fn start_update(&mut self, ctx: &egui::Context, install: bool) {
        if self.updater.is_running() {
            return;
        }
        if matches!(self.updater.last, Some(Outcome::Installed { .. })) {
            self.restart(ctx);
            return;
        }
        // ya se sabe qué hay: volver a mostrar el aviso
        if !install && matches!(self.updater.last, Some(Outcome::Available { .. })) {
            self.show_offer = true;
            return;
        }
        self.show_offer = false;
        match updater::Job::new(
            &self.build_info,
            self.installed.as_ref(),
            self.cfg.update_repo.as_deref(),
            self.cfg.update_branch.as_deref(),
        ) {
            Ok(mut job) => {
                job.install_now = install;
                let c = ctx.clone();
                self.updater.start(job, move || c.request_repaint());
            }
            Err(message) => {
                self.play_error();
                self.toast_for(ctx, message.clone(), true, 4.0);
                self.updater.last = Some(Outcome::Failed { kind: FailKind::Repo, message });
            }
        }
    }

    /// Resultado del actualizador (una vez por búsqueda).
    pub(crate) fn poll_update(&mut self, ctx: &egui::Context) {
        if self.updater.is_running() {
            // spinner del menú y puntito del botón ⓘ
            ctx.request_repaint_after(Duration::from_millis(40));
        }
        let Some(out) = self.updater.poll() else { return };
        let now = ctx.input(|i| i.time);
        match out {
            Outcome::UpToDate { hash } => {
                self.toast_for(ctx, format!("Ya tenés la última versión ({hash})"), false, 3.0);
            }
            Outcome::Ahead => {
                self.toast_for(ctx, "Tu Capyro es más nuevo que el del repo", false, 3.0);
            }
            Outcome::Available { .. } => {
                // el aviso con la lista de commits y "Instalar"
                self.show_offer = true;
            }
            Outcome::Installed { commits, total } => {
                self.sfx(SFX_UPDATE);
                self.update_banner = Some(UpdateBanner { start: now, until: now + BANNER_TIME, commits, total });
            }
            Outcome::Failed { message, .. } => {
                self.play_error();
                self.toast_for(ctx, message, true, 5.0);
            }
        }
    }

    /// Guarda la sesión, abre el Capyro recién instalado y cierra este.
    pub(crate) fn restart(&mut self, ctx: &egui::Context) {
        self.save_session();
        let bindir = match &self.installed {
            Some(i) => i.bindir.clone(),
            None => std::path::PathBuf::from(&self.build_info.bindir),
        };
        match updater::relaunch(&bindir) {
            Ok(()) => {
                self.restarting = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            Err(e) => {
                self.play_error();
                self.toast_for(ctx, format!("No pude reiniciar Capyro: {e}"), true, 4.0);
            }
        }
    }

    /* ---------------------------------------------------------------- */
    /* acciones                                                         */
    /* ---------------------------------------------------------------- */

    /// "Versión actual: 1a2b3c4 · hace 2 días" (debajo de "Buscar actualizaciones").
    pub(crate) fn version_line(&self) -> String {
        let b = &self.build_info;
        if b.commit.is_empty() {
            return if self.installed.is_some() {
                "Compilado sin git (desde un .zip)".to_owned()
            } else {
                "Sin git · sin instalar (corre desde build/)".to_owned()
            };
        }
        let short: String = b.commit.chars().take(7).collect();
        let dirty = if b.dirty { "+cambios" } else { "" };
        if self.installed.is_none() {
            return format!("{short}{dirty} · sin instalar (corre desde build/)");
        }
        let when = updater::ago(b.commit_time);
        if when.is_empty() {
            format!("Versión actual: {short}{dirty}")
        } else {
            format!("Versión actual: {short}{dirty} · {when}")
        }
    }

    pub(crate) fn update_row(&mut self, ui: &mut Ui, painter: &egui::Painter, row: Rect, interactive: bool) {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;
        let now = ctx.input(|i| i.time);
        let progress = self.updater.progress();
        let running = progress.is_some();
        let installed = matches!(self.updater.last, Some(Outcome::Installed { .. }));

        let btn = animate_button(ui, Id::unique("capyro-update-row"), row, interactive && !running);
        if btn.hover > 0.001 || btn.press > 0.001 {
            painter.rect_filled(row, 8.0, Color32::from_white_alpha((10.0 * btn.hover + 10.0 * btn.press) as u8));
        }
        let x0 = row.left() + 6.0;
        let cy = row.center().y;
        let ic = pos2(x0 + 8.0, cy - 1.0);

        let mut sub_col = TEXT_FAINT;
        let mut tooltip = if cfg!(windows) {
            "Busca la última versión para Windows en el repo de GitHub; si hay, la baja y la instala sola".to_owned()
        } else {
            "Busca commits nuevos en el repo de GitHub; si hay, los baja, compila e instala solo".to_owned()
        };
        let (title, sub) = if let Some(pr) = &progress {
            icons::spinner(painter, ic, 15.0, accent, now);
            let secs = pr.elapsed.as_secs();
            let t = format!("{}:{:02}", secs / 60, secs % 60);
            let sub = if pr.detail.is_empty() { t } else { format!("{} · {t}", pr.detail) };
            tooltip = format!("Mientras tanto podés seguir usando Capyro. Detalle en {}", updater::log_path_display());
            (pr.stage.label().to_owned(), sub)
        } else {
            match &self.updater.last {
                Some(Outcome::Installed { commits, .. }) => {
                    painter.circle_filled(ic, 8.0, accent);
                    icons::check(painter, ic, 11.0, Color32::WHITE, 1.8);
                    tooltip = "Clic para reiniciar Capyro con la versión nueva".to_owned();
                    match commits.first() {
                        Some(c) => (format!("Actualizado a {}", c.hash), c.subject.clone()),
                        None => ("Actualizado".to_owned(), "Reiniciá para aplicar".to_owned()),
                    }
                }
                other => {
                    icons::download(painter, ic, 15.0 * btn.scale(), btn.tint(ICON, Color32::WHITE), btn.hover);
                    let sub = match other {
                        Some(Outcome::UpToDate { hash }) => format!("Ya tenés la última versión ({hash})"),
                        Some(Outcome::Ahead) => "Tu versión es más nueva que la del repo".to_owned(),
                        Some(Outcome::Available { commits, total, installable }) => {
                            tooltip = if *installable {
                                "Clic para ver los cambios e instalarlos".to_owned()
                            } else {
                                if cfg!(windows) {
                                    "Para que se actualice solo, instalá Capyro con CapyroInstaller".to_owned()
                                } else {
                                    "Para que se actualice solo, instalá Capyro (cmake --install build)".to_owned()
                                }
                            };
                            let s = if *total == 1 { "" } else { "s" };
                            let last = commits.first().map(|c| format!(" · último: {}", c.hash)).unwrap_or_default();
                            format!("{total} cambio{s} nuevo{s}{last}")
                        }
                        Some(Outcome::Failed { kind, message }) => {
                            sub_col = ERROR;
                            tooltip = if *kind == FailKind::Build {
                                format!("{message}\nDetalle en {}", updater::log_path_display())
                            } else {
                                message.clone()
                            };
                            message.clone()
                        }
                        _ => {
                            let b = &self.build_info;
                            if !b.subject.is_empty() {
                                let rama = if b.branch.is_empty() { String::new() } else { format!(" ({})", b.branch) };
                                tooltip = format!("{tooltip}\nEste Capyro sale de: {}{rama}", b.subject);
                            }
                            self.version_line()
                        }
                    };
                    ("Buscar actualizaciones".to_owned(), sub)
                }
            }
        };

        // a la derecha: "Reiniciar" cuando ya está instalada, "Ver" si hay algo nuevo
        let mut text_right = row.right() - 10.0;
        let available = !running && matches!(self.updater.last, Some(Outcome::Available { .. }));
        if installed || available {
            let label = if installed { "Reiniciar" } else { "Ver" };
            let g = painter.layout_no_wrap(label.to_owned(), bold(11.5), Color32::WHITE);
            let pill = Rect::from_min_size(
                pos2(row.right() - 8.0 - g.size().x - 20.0, cy - 12.0),
                vec2(g.size().x + 20.0, 24.0),
            );
            let fill = accent.lerp_to_gamma(Color32::WHITE, 0.12 * btn.hover);
            painter.rect_filled(pill, 12.0, fill);
            painter.galley(pill.center() - g.size() / 2.0, g, Color32::WHITE);
            text_right = pill.left() - 8.0;
        }
        let max_chars = (((text_right - (x0 + 24.0)) / 5.6) as usize).max(10);
        painter.text(pos2(x0 + 24.0, cy - 7.0), Align2::LEFT_CENTER, ellipsize(&title, max_chars), regular(12.5), TEXT);
        painter.text(pos2(x0 + 24.0, cy + 8.0), Align2::LEFT_CENTER, ellipsize(&sub, max_chars + 4), regular(10.5), sub_col);

        // barra "indeterminada": compilar no avisa cuánto falta
        if running {
            let bar = Rect::from_min_max(pos2(x0 + 24.0, row.bottom() - 3.0), pos2(row.right() - 12.0, row.bottom() - 1.0));
            painter.rect_filled(bar, 1.0, TRACK);
            let w = bar.width() * 0.3;
            let phase = ((now * 0.8) % 1.0) as f32;
            let x = bar.left() - w + (bar.width() + w) * ease_in_out(phase);
            let seg = Rect::from_min_max(
                pos2(x.max(bar.left()), bar.top()),
                pos2((x + w).min(bar.right()), bar.bottom()),
            );
            if seg.width() > 0.0 {
                painter.rect_filled(seg, 1.0, accent);
            }
        }

        if btn.resp.clicked() {
            self.start_update(&ctx, false);
        }
        if interactive {
            btn.resp.on_hover_text(tooltip);
        }
    }

    /// Aviso "Hay una actualización": qué commits se van a instalar (los que
    /// son más nuevos que el tuyo) y los botones Instalar / Ahora no.
    pub(crate) fn draw_update_offer(&mut self, ui: &mut Ui, video: Rect) {
        let ctx = ui.ctx().clone();
        let t = ctx.animate_bool_with_time(Id::unique("capyro-offer-anim"), self.show_offer, crate::system::perf::t(0.22));
        if t <= 0.001 {
            return;
        }
        let Some(Outcome::Available { commits, total, installable }) = self.updater.last.clone() else {
            self.show_offer = false;
            return;
        };
        let e = ease_out(t);
        let accent = self.cfg.accent;
        let now = ctx.input(|i| i.time);
        let interactive = self.show_offer && t > 0.6;

        const MAX_SHOWN: usize = 6;
        let shown: Vec<CommitInfo> = commits.iter().take(MAX_SHOWN).cloned().collect();
        let more = total.saturating_sub(shown.len());
        let width = (video.width() - 32.0).min(460.0);
        let h = 64.0 + 26.0 * shown.len() as f32 + if more > 0 { 20.0 } else { 0.0 } + 70.0;
        let rect = Rect::from_min_size(
            pos2(video.center().x - width / 2.0, video.top() + 14.0 - 16.0 * (1.0 - e)),
            vec2(width, h),
        );

        let mut p = ui.painter().clone();
        p.multiply_opacity(e);
        if interactive {
            ui.interact(rect, Id::unique("capyro-offer-bg"), Sense::click()); // que no pause el video
        }
        crate::ui::theme::soft_shadow(&p, rect, 14.0, 10.0, 2.5, 80);
        p.rect_filled(rect, 14.0, BG_PANEL);
        p.rect_stroke(rect, 14.0, Stroke::new(1.0, accent.gamma_multiply(0.5)), StrokeKind::Inside);

        // encabezado
        let hc = pos2(rect.left() + 30.0, rect.top() + 31.0);
        p.circle_filled(hc, 15.0, accent.gamma_multiply(0.18));
        let bob = ((crate::system::perf::clock(now) * 3.0).sin() as f32 * 0.5 + 0.5) * 0.6;
        icons::download(&p, hc, 17.0, accent, bob);
        let tx = hc.x + 26.0;
        p.text(pos2(tx, rect.top() + 22.0), Align2::LEFT_CENTER, "Hay una actualización", bold(14.0), TEXT);
        let s = if total == 1 { "" } else { "s" };
        let mine: String = self.build_info.commit.chars().take(7).collect();
        let sub = if mine.is_empty() {
            format!("{total} cambio{s} nuevo{s} en el repo")
        } else {
            format!("{total} cambio{s} más nuevo{s} que tu versión ({mine})")
        };
        p.text(pos2(tx, rect.top() + 40.0), Align2::LEFT_CENTER, sub, regular(11.5), TEXT_DIM);
        if interactive && crate::system::perf::animations() {
            ctx.request_repaint_after(FRAME_AMBIENT); // el ícono que flota
        }
        p.line_segment(
            [pos2(rect.left() + 16.0, rect.top() + 58.0), pos2(rect.right() - 16.0, rect.top() + 58.0)],
            Stroke::new(1.0, Color32::from_white_alpha(14)),
        );

        // los commits que se van a instalar (el de arriba es el más nuevo)
        let mut y = rect.top() + 76.0;
        for (i, c) in shown.iter().enumerate() {
            let hg = p.layout_no_wrap(c.hash.clone(), FontId::monospace(11.0), accent);
            let chip = Rect::from_min_size(pos2(rect.left() + 16.0, y - 10.0), hg.size() + vec2(12.0, 20.0 - hg.size().y));
            p.rect_filled(chip, 6.0, accent.gamma_multiply(0.14));
            p.galley(chip.center() - hg.size() / 2.0, hg, accent);
            let wg = p.layout_no_wrap(updater::ago(c.time), regular(10.5), TEXT_FAINT);
            p.galley(pos2(rect.right() - 16.0 - wg.size().x, y - wg.size().y / 2.0), wg.clone(), TEXT_FAINT);
            let avail = rect.right() - 24.0 - wg.size().x - (chip.right() + 8.0);
            let max_chars = ((avail / 6.4) as usize).max(8);
            p.text(
                pos2(chip.right() + 8.0, y),
                Align2::LEFT_CENTER,
                ellipsize(&c.subject, max_chars),
                if i == 0 { bold(12.0) } else { regular(12.0) },
                if i == 0 { TEXT } else { TEXT_DIM },
            );
            y += 26.0;
        }
        if more > 0 {
            let s = if more == 1 { "" } else { "s" };
            p.text(pos2(rect.left() + 16.0, y - 4.0), Align2::LEFT_CENTER, format!("+{more} cambio{s} más"), regular(11.0), TEXT_FAINT);
            y += 20.0;
        }

        // pie: Instalar / Ahora no
        p.line_segment(
            [pos2(rect.left() + 16.0, y - 6.0), pos2(rect.right() - 16.0, y - 6.0)],
            Stroke::new(1.0, Color32::from_white_alpha(14)),
        );
        let fy = rect.bottom() - 30.0;
        let mut install = false;
        let mut close = false;
        let close_label = if installable { "Ahora no" } else { "Cerrar" };
        let g = p.layout_no_wrap(close_label.to_owned(), regular(12.0), TEXT_DIM);
        let cr = Rect::from_min_max(pos2(rect.right() - 16.0 - g.size().x - 20.0, fy - 14.0), pos2(rect.right() - 16.0, fy + 14.0));
        let cb = animate_button(ui, Id::unique("capyro-offer-later"), cr, interactive);
        if cb.hover > 0.001 {
            p.rect_filled(cr, 7.0, Color32::from_white_alpha((12.0 * cb.hover) as u8));
        }
        p.galley(cr.center() - g.size() / 2.0, g, cb.tint(TEXT_DIM, Color32::WHITE));
        close |= cb.resp.clicked();
        if installable {
            let g = p.layout_no_wrap("Instalar".to_owned(), bold(12.5), Color32::WHITE);
            let ir = Rect::from_min_max(pos2(cr.left() - 8.0 - g.size().x - 44.0, fy - 15.0), pos2(cr.left() - 8.0, fy + 15.0));
            let ib = animate_button(ui, Id::unique("capyro-offer-install"), ir, interactive);
            let rr = Rect::from_center_size(ir.center(), ir.size() * (1.0 + 0.04 * ib.hover - 0.08 * ib.press));
            if ib.hover > 0.001 {
                p.rect_filled(rr.expand(3.0 * ib.hover), rr.height() / 2.0 + 3.0, accent.gamma_multiply(0.2 * ib.hover));
            }
            p.rect_filled(rr, rr.height() / 2.0, accent.lerp_to_gamma(Color32::WHITE, 0.12 * ib.hover));
            icons::download(&p, pos2(rr.left() + 18.0, rr.center().y), 13.0, Color32::WHITE, ib.hover);
            p.galley(pos2(rr.left() + 30.0, rr.center().y - g.size().y / 2.0), g, Color32::WHITE);
            install = ib.resp.clicked();
            p.text(
                pos2(rect.left() + 16.0, fy),
                Align2::LEFT_CENTER,
                "Se baja, compila e instala sola",
                regular(11.0),
                TEXT_FAINT,
            );
        } else {
            p.text(
                pos2(rect.left() + 16.0, fy),
                Align2::LEFT_CENTER,
                "Para instalarla, primero instalá Capyro",
                regular(11.0),
                TEXT_FAINT,
            );
        }
        if interactive && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            close = true;
        }
        if close {
            self.show_offer = false;
        }
        if install {
            self.sfx(SFX_READY); // a descargar
            self.show_offer = false;
            self.show_info = true; // para ver el progreso
            self.start_update(&ctx, true);
        }
    }

    /// Aviso con "el parche": los commits que se acaban de instalar.
    pub(crate) fn draw_update_banner(&mut self, ui: &mut Ui, video: Rect) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let accent = self.cfg.accent;
        let Some(b) = &mut self.update_banner else { return };
        if now > b.until {
            self.update_banner = None;
            return;
        }
        let fade_in = ease_out(crate::system::perf::k(((now - b.start) / 0.35) as f32));
        let fade_out = crate::system::perf::k(((b.until - now) / 0.45).clamp(0.0, 1.0) as f32);
        let alpha = fade_in.min(fade_out);

        let shown: Vec<CommitInfo> = b.commits.iter().take(3).cloned().collect();
        let more = b.total.saturating_sub(shown.len());
        let width = (video.width() - 32.0).min(440.0);
        let h = 62.0 + 26.0 * shown.len() as f32 + if more > 0 { 18.0 } else { 0.0 } + 10.0;
        let rect = Rect::from_min_size(
            pos2(video.center().x - width / 2.0, video.top() + 14.0 - 16.0 * (1.0 - fade_in)),
            vec2(width, h),
        );
        // con el mouse encima no se va
        if ui.rect_contains_pointer(rect) && fade_in >= 1.0 {
            b.until = b.until.max(now + 1.2);
        }
        let (start, until, total) = (b.start, b.until, b.total);

        let mut p = ui.painter().clone();
        p.multiply_opacity(alpha);
        ui.interact(rect, Id::unique("capyro-update-banner"), Sense::click()); // que no pause el video
        crate::ui::theme::soft_shadow(&p, rect, 14.0, 10.0, 2.5, 80);
        p.rect_filled(rect, 14.0, BG_PANEL);
        p.rect_stroke(rect, 14.0, Stroke::new(1.0, accent.gamma_multiply(0.5)), StrokeKind::Inside);

        // encabezado
        let hc = pos2(rect.left() + 30.0, rect.top() + 31.0);
        let pop = ease_out(crate::system::perf::k(((now - start - 0.15) / 0.3) as f32));
        p.circle_filled(hc, 15.0 + 5.0 * (1.0 - pop) * pop * 4.0 * 0.25, accent.gamma_multiply(0.2));
        p.circle_filled(hc, 12.0 * (0.6 + 0.4 * pop), accent);
        if pop > 0.05 {
            icons::check(&p, hc, 13.0 * pop, Color32::WHITE, 2.0);
        }
        let tx = hc.x + 24.0;
        p.text(pos2(tx, rect.top() + 22.0), Align2::LEFT_CENTER, "Capyro se actualizó", bold(14.0), TEXT);
        let cambios = if total == 1 {
            "1 cambio nuevo · reiniciá para aplicarlo".to_owned()
        } else {
            format!("{total} cambios nuevos · reiniciá para aplicarlos")
        };
        p.text(pos2(tx, rect.top() + 40.0), Align2::LEFT_CENTER, cambios, regular(11.5), TEXT_DIM);

        // botón Reiniciar
        let g = p.layout_no_wrap("Reiniciar".to_owned(), bold(12.0), Color32::WHITE);
        let pill = Rect::from_min_size(pos2(rect.right() - 16.0 - g.size().x - 24.0, hc.y - 14.0), vec2(g.size().x + 24.0, 28.0));
        let rb = animate_button(ui, Id::unique("capyro-banner-restart"), pill, alpha > 0.9);
        let pr = Rect::from_center_size(pill.center(), pill.size() * (1.0 + 0.04 * rb.hover - 0.08 * rb.press));
        if rb.hover > 0.001 {
            p.rect_filled(pr.expand(3.0 * rb.hover), pr.height() / 2.0 + 3.0, accent.gamma_multiply(0.2 * rb.hover));
        }
        p.rect_filled(pr, pr.height() / 2.0, accent.lerp_to_gamma(Color32::WHITE, 0.12 * rb.hover));
        p.galley(pr.center() - g.size() / 2.0, g, Color32::WHITE);

        // los commits
        p.line_segment(
            [pos2(rect.left() + 16.0, rect.top() + 58.0), pos2(rect.right() - 16.0, rect.top() + 58.0)],
            Stroke::new(1.0, Color32::from_white_alpha(14)),
        );
        let mut y = rect.top() + 76.0;
        for (i, c) in shown.iter().enumerate() {
            // cada línea entra un poquito después de la anterior
            let k = ease_out(crate::system::perf::k(((now - start - 0.2 - 0.08 * i as f64) / 0.3) as f32));
            let mut lp = p.clone();
            lp.multiply_opacity(k);
            let dx = 8.0 * (1.0 - k);
            let hg = lp.layout_no_wrap(c.hash.clone(), FontId::monospace(11.0), accent);
            let chip = Rect::from_min_size(pos2(rect.left() + 16.0 + dx, y - 10.0), hg.size() + vec2(12.0, 20.0 - hg.size().y));
            lp.rect_filled(chip, 6.0, accent.gamma_multiply(0.14));
            lp.galley(chip.center() - hg.size() / 2.0, hg, accent);
            let when = updater::ago(c.time);
            let wg = lp.layout_no_wrap(when, regular(10.5), TEXT_FAINT);
            lp.galley(pos2(rect.right() - 16.0 - wg.size().x, y - wg.size().y / 2.0), wg.clone(), TEXT_FAINT);
            let avail = rect.right() - 24.0 - wg.size().x - (chip.right() + 8.0);
            let max_chars = ((avail / 6.4) as usize).max(8);
            lp.text(
                pos2(chip.right() + 8.0, y),
                Align2::LEFT_CENTER,
                ellipsize(&c.subject, max_chars),
                if i == 0 { bold(12.0) } else { regular(12.0) },
                if i == 0 { TEXT } else { TEXT_DIM },
            );
            y += 26.0;
        }
        if more > 0 {
            p.text(pos2(rect.left() + 16.0, y - 4.0), Align2::LEFT_CENTER, format!("+{more} más"), regular(11.0), TEXT_FAINT);
        }

        // cuánto le queda al aviso
        let left = ((until - now) / BANNER_TIME).clamp(0.0, 1.0) as f32;
        let bar = Rect::from_min_size(pos2(rect.left() + 14.0, rect.bottom() - 4.0), vec2((rect.width() - 28.0) * left, 2.0));
        p.rect_filled(bar, 1.0, accent.gamma_multiply(0.5));

        if rb.resp.clicked() {
            self.restart(&ctx);
        }
        // la barrita del tiempo que queda (en Bajo, a menos cuadros)
        ctx.request_repaint_after(if crate::system::perf::animations() { FRAME_SMOOTH } else { crate::system::perf::ambient() });
    }
}

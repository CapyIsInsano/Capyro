//! Menú ⚙ (Ajustes): velocidad, decodificación, rendimiento, lo que se ve
//! y los accesos a la configuración, las capturas y el tutorial.

use crate::app::*;
use crate::system::config::{DiscViz, VizMode};

/// Alto de todo el contenido del menú ⚙.
pub(crate) const SETTINGS_H: f32 = 682.0;

/// Velocidades que ofrece el menú de ajustes (y entre las que saltan los atajos).
pub(crate) const SPEEDS: [f64; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

pub(crate) fn fmt_speed(s: f64) -> String {
    let t = format!("{s:.2}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    format!("{t}×")
}

impl CapyroApp {
    /// Menú del engranaje: velocidad y recargar configuración. Se abre
    /// creciendo desde el engranaje y se cierra al hacer clic afuera o con Esc.
    /// Con `dock`, se dibuja fijo ahí (la burbuja de funciones de la vista
    /// de música: sin animación de apertura ni fondo propio). Devuelve el
    /// alto del contenido (para el scroll de la burbuja).
    pub(crate) fn settings_menu(&mut self, ui: &mut Ui, dock: Option<Rect>) -> f32 {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;
        let size = vec2(288.0, SETTINGS_H);
        // en la vista clásica, si la ventana es bajita, el menú tiene scroll
        let mut scroll_child: Option<Ui> = None;
        let (panel, interactive, painter) = if let Some(d) = dock {
            (Rect::from_min_size(d.min, vec2(d.width(), size.y)), true, ui.painter().clone())
        } else {
        let t = ctx.animate_bool_with_time(Id::unique("capyro-settings-anim"), self.show_settings, crate::system::perf::t(0.15));
        if t <= 0.001 {
            return 0.0;
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let mut anchor = pos2(self.gear_rect.right() + 6.0, self.gear_rect.top() - 10.0);
        // lo que entra entre el borde de arriba y el engranaje (si no entra todo, scroll)
        let top_y = ui.max_rect().top() + 8.0;
        let vis_h = size.y.min((anchor.y - top_y).max(260.0));
        anchor.y = anchor.y.max(top_y + vis_h);
        let size = vec2(size.x, vis_h);
        let full = Rect::from_min_size(anchor - size, size);
        // crece desde la esquina del engranaje
        let panel = Rect::from_min_max(anchor - size * (0.85 + 0.15 * crate::ui::alto::pop(t, ease)), anchor);
        let interactive = self.show_settings && t > 0.6;

        let mut painter = ui.painter().clone();
        painter.multiply_opacity(ease);
        if interactive {
            // Capa invisible sobre toda la ventana: un clic afuera solo cierra
            // el menú (no pausa el video ni aprieta lo que haya abajo).
            let outside = ui.interact(ui.max_rect(), Id::unique("capyro-settings-outside"), Sense::click());
            if outside.clicked() {
                self.show_settings = false;
            }
            ui.interact(full, Id::unique("capyro-settings-bg"), Sense::click());
        }
        crate::ui::theme::soft_shadow(&painter, panel, 12.0, 8.0, 2.0, 70);
        painter.rect_filled(panel, 12.0, BG_PANEL);
        painter.rect_stroke(panel, 12.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
        if ease < 0.98 {
            ctx.request_repaint_after(FRAME_SMOOTH);
            return 0.0; // el contenido aparece cuando termina de abrirse
        }
        crate::ui::alto::reveal(&ctx, Id::unique("capyro-settings-anim"), panel, 12.0, self.show_settings, BG_PANEL, accent);
        (panel, interactive, painter)
        };
        let visible = panel;
        let (panel, painter) = if dock.is_none() && visible.height() < SETTINGS_H - 0.5 {
            // no entra: se corre con la rueda (y se recorta al panel)
            let max = SETTINGS_H - visible.height();
            if ui.rect_contains_pointer(visible) {
                self.settings_scroll -= ctx.input(|i| i.smooth_scroll_delta.y);
            }
            self.settings_scroll = self.settings_scroll.clamp(0.0, max);
            let clip = visible.shrink(1.0).intersect(ui.clip_rect());
            let mut child = ui.new_child(UiBuilder::new().max_rect(visible));
            child.set_clip_rect(clip);
            scroll_child = Some(child);
            let at = Rect::from_min_size(visible.min - vec2(0.0, self.settings_scroll), vec2(visible.width(), SETTINGS_H));
            (at, painter.with_clip_rect(clip))
        } else {
            self.settings_scroll = 0.0;
            (panel, painter)
        };
        let ui: &mut Ui = match scroll_child.as_mut() {
            Some(child) => child,
            None => ui,
        };

        let x0 = panel.left() + 14.0;
        let x1 = panel.right() - 14.0;
        let mut y = panel.top() + 20.0;
        // en la burbuja angosta de la vista de música, los chips con letra más chica
        let chip_fs = if x1 - x0 < 240.0 { 10.5 } else { 11.5 };
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Ajustes", bold(13.5), TEXT);

        /* velocidad */
        y += 28.0;
        let speed = self.core.speed();
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Velocidad", regular(12.5), TEXT_DIM);
        painter.text(pos2(x1, y), Align2::RIGHT_CENTER, fmt_speed(speed), bold(12.5), accent);
        y += 24.0;
        let gap = 5.0;
        let chip_w = ((x1 - x0) - gap * (SPEEDS.len() as f32 - 1.0)) / SPEEDS.len() as f32;
        for (i, &v) in SPEEDS.iter().enumerate() {
            let r = Rect::from_min_size(pos2(x0 + i as f32 * (chip_w + gap), y - 13.0), vec2(chip_w, 26.0));
            let chip = animate_button(ui, Id::unique(("capyro-speed", i)), r, interactive);
            let selected = (speed - v).abs() < 1e-6;
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-speed-sel", i)), selected, crate::system::perf::t(0.15));
            let bg = Color32::from_white_alpha((8.0 + 14.0 * chip.hover) as u8).lerp_to_gamma(accent, sel);
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.06 * chip.press));
            painter.rect_filled(rr, 7.0, bg);
            let col = TEXT_DIM.lerp_to_gamma(TEXT, chip.hover).lerp_to_gamma(Color32::WHITE, sel);
            painter.text(rr.center(), Align2::CENTER_CENTER, fmt_speed(v), if selected { bold(chip_fs) } else { regular(chip_fs) }, col);
            if chip.resp.clicked() && !selected {
                self.set_speed(&ctx, v);
            }
        }

        /* separador */
        y += 24.0;
        painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, Color32::from_white_alpha(14)));

        /* decodificación de video: automático / GPU / CPU */
        y += 22.0;
        let mode = self.cfg.decoding;
        let info = self.core.decode_info();
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Decodificación", regular(12.5), TEXT_DIM);
        // a la derecha, qué se está usando ahora (si hay un video abierto)
        if info.active {
            let (txt, col) = if info.hw { (format!("GPU · {}", info.api), accent) } else { ("CPU".to_owned(), TEXT_FAINT) };
            let g = painter.layout_no_wrap(txt, bold(11.0), col);
            let r = Rect::from_min_size(pos2(x1 - g.size().x - 12.0, y - g.size().y / 2.0 - 3.0), g.size() + vec2(12.0, 6.0));
            painter.rect_filled(r, r.height() / 2.0, if info.hw { accent.gamma_multiply(0.16) } else { Color32::from_white_alpha(10) });
            painter.galley(r.center() - g.size() / 2.0, g, col);
        }
        y += 24.0;
        let chip_w = ((x1 - x0) - gap * 2.0) / 3.0;
        for (i, m) in ffi::Hwdec::ALL.iter().copied().enumerate() {
            let r = Rect::from_min_size(pos2(x0 + i as f32 * (chip_w + gap), y - 13.0), vec2(chip_w, 26.0));
            let chip = animate_button(ui, Id::unique(("capyro-hwdec", i)), r, interactive);
            let selected = mode == m;
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-hwdec-sel", i)), selected, crate::system::perf::t(0.15));
            let bg = Color32::from_white_alpha((8.0 + 14.0 * chip.hover) as u8).lerp_to_gamma(accent, sel);
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.06 * chip.press));
            painter.rect_filled(rr, 7.0, bg);
            let col = TEXT_DIM.lerp_to_gamma(TEXT, chip.hover).lerp_to_gamma(Color32::WHITE, sel);
            // (en la burbuja angosta, "Automático" no entra)
            let label = if chip_w < 66.0 && m == ffi::Hwdec::Auto { "Auto" } else { m.label() };
            painter.text(rr.center(), Align2::CENTER_CENTER, label, if selected { bold(chip_fs) } else { regular(chip_fs) }, col);
            if chip.resp.clicked() && !selected {
                self.set_decoding(&ctx, m);
            }
            chip.resp.on_hover_text(match m {
                ffi::Hwdec::Auto => "Usa la GPU si se puede; si no, la CPU",
                ffi::Hwdec::Gpu => "Decodifica en la placa de video (VA-API, NVDEC o VDPAU). Si no se puede, te avisa y usa la CPU",
                ffi::Hwdec::Cpu => "Siempre por software (como hasta la v0.2.8)",
            });
        }
        y += 22.0;
        let status = if !info.active {
            "Se aplica a los videos (los audios no la usan)".to_owned()
        } else if info.hw {
            format!("{} decodificado por la GPU ({})", info.codec, info.api)
        } else {
            format!("{} por CPU · {}", info.codec, info.note)
        };
        let status_rect = Rect::from_min_max(pos2(x0, y - 8.0), pos2(x1, y + 8.0));
        let st = ui.interact(status_rect, Id::unique("capyro-hwdec-status"), Sense::hover());
        let shown = fit_text(&painter, &status, regular(10.5), x1 - x0);
        let cut = shown != status;
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, shown, regular(10.5), TEXT_FAINT);
        if cut {
            st.on_hover_text(status);
        }

        /* separador */
        y += 18.0;
        painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, Color32::from_white_alpha(14)));

        /* modo de rendimiento: bajo / medio / alto */
        y += 22.0;
        let perf_mode = self.cfg.performance;
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Rendimiento", regular(12.5), TEXT_DIM);
        y += 24.0;
        for (i, m) in crate::system::perf::Mode::ALL.iter().copied().enumerate() {
            let r = Rect::from_min_size(pos2(x0 + i as f32 * (chip_w + gap), y - 13.0), vec2(chip_w, 26.0));
            let chip = animate_button(ui, Id::unique(("capyro-perf", i)), r, interactive);
            let selected = perf_mode == m;
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-perf-sel", i)), selected, crate::system::perf::t(0.15));
            let bg = Color32::from_white_alpha((8.0 + 14.0 * chip.hover) as u8).lerp_to_gamma(accent, sel);
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.06 * chip.press));
            // Alto se ve "premium": brillo dorado y una chispa
            let premium = m == crate::system::perf::Mode::Alto;
            if premium && crate::ui::alto::on() {
                crate::ui::alto::glow_rect(&painter, rr, accent.lerp_to_gamma(Color32::from_rgb(255, 214, 120), 0.5), 0.55 + 0.25 * chip.hover);
            }
            painter.rect_filled(rr, 7.0, bg);
            if premium {
                let gold = Color32::from_rgb(255, 214, 120);
                painter.rect_stroke(rr, 7.0, Stroke::new(1.0, gold.gamma_multiply(0.25 + 0.45 * sel)), StrokeKind::Inside);
                let t = crate::system::perf::clock(ctx.input(|i| i.time)) as f32;
                icons::sparkle(&painter, pos2(rr.right() - 9.0, rr.top() + 8.0), 3.2 + 0.6 * (t * 2.0).sin().abs() * sel, t * 0.6, gold);
            }
            let col = TEXT_DIM.lerp_to_gamma(TEXT, chip.hover).lerp_to_gamma(Color32::WHITE, sel);
            painter.text(rr.center(), Align2::CENTER_CENTER, m.label(), if selected { bold(chip_fs) } else { regular(chip_fs) }, col);
            if chip.resp.clicked() && !selected {
                self.set_performance(&ctx, m);
            }
            chip.resp.on_hover_text(match m {
                crate::system::perf::Mode::Bajo => "Sin animaciones, sin pantalla de carga, sin visualizador ni brillos. Todo sigue funcionando, gastando lo mínimo",
                crate::system::perf::Mode::Medio => "Capyro como siempre",
                crate::system::perf::Mode::Alto => "Premium: suma la iluminación cinematográfica (los colores del video iluminan toda la ventana, como en YouTube), un aura de luz alrededor de la carátula, la carátula de vidrio que sigue al mouse, destellos en la barra, un fondo con degradé que se mueve, brillos, transiciones con más vida y partículas de luz",
            });
        }
        y += 22.0;
        let blurb = fit_text(&painter, self.cfg.performance.blurb(), regular(10.5), x1 - x0);
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, blurb, regular(10.5), TEXT_FAINT);

        /* separador */
        y += 18.0;
        painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, Color32::from_white_alpha(14)));

        /* con un audio: el visualizador (a los lados, en medio o apagado) y
         * la portada que late con los graves. En Bajo no hay ninguno. */
        let low = perf_mode == crate::system::perf::Mode::Bajo;
        // disco de vinilo (con un audio): la portada al medio de un disco que gira
        y += 22.0;
        let r = painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Disco de vinilo", regular(12.5), TEXT_DIM);
        ui.interact(r.expand(4.0), Id::unique("capyro-vinyl-label"), Sense::hover()).on_hover_text(if low {
            "Con un audio, la portada al medio de un disco de vinilo (en Bajo se ve, pero no gira)"
        } else {
            "Con un audio, la portada al medio de un disco de vinilo que gira: más rápido si la velocidad es mayor"
        });
        if switch(ui, &painter, "vinyl", pos2(x1 - 17.0, y), self.cfg.vinyl, accent, interactive) {
            self.cfg.vinyl = !self.cfg.vinyl;
            let msg = if self.cfg.vinyl { "Disco de vinilo activado" } else { "Disco de vinilo desactivado" };
            self.save_config(&ctx, msg.to_owned());
        }
        // el visualizador: con el disco, alrededor / abajo / desactivado;
        // si no, a los lados / en medio / apagado
        y += 30.0;
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Visualizador", regular(12.5), TEXT_DIM);
        if low {
            painter.text(pos2(x1, y), Align2::RIGHT_CENTER, "no en Bajo", regular(10.5), TEXT_FAINT);
        } else if self.cfg.vinyl {
            painter.text(pos2(x1, y), Align2::RIGHT_CENTER, "del disco", regular(10.5), TEXT_FAINT);
        }
        y += 24.0;
        let vw = ((x1 - x0) - gap * 2.0) / 3.0;
        let disc = self.cfg.vinyl;
        for i in 0..3 {
            let (label, selected, hint) = if disc {
                let m = DiscViz::ALL[i];
                let hint = match m {
                    DiscViz::Around => "Las barras rodean el disco",
                    DiscViz::Below => "Las barras van abajo del disco",
                    DiscViz::Off => "Sin barras",
                };
                (m.label(), self.cfg.vinyl_visualizer == m, hint)
            } else {
                let m = VizMode::ALL[i];
                let hint = match m {
                    VizMode::Sides => "Con un audio, barras a los dos lados de la portada",
                    VizMode::Middle => "Con un audio, barras sobre la parte de abajo de la portada",
                    VizMode::Off => "Sin barras",
                };
                (m.label(), self.cfg.visualizer == m, hint)
            };
            let r = Rect::from_min_size(pos2(x0 + i as f32 * (vw + gap), y - 13.0), vec2(vw, 26.0));
            let chip = animate_button(ui, Id::unique(("capyro-viz", i)), r, interactive && !low);
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-viz-sel", i)), selected && !low, crate::system::perf::t(0.15));
            let bg = Color32::from_white_alpha((8.0 + 14.0 * chip.hover) as u8).lerp_to_gamma(accent, sel);
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.06 * chip.press));
            painter.rect_filled(rr, 7.0, bg);
            let col = if low { TEXT_FAINT } else { TEXT_DIM.lerp_to_gamma(TEXT, chip.hover).lerp_to_gamma(Color32::WHITE, sel) };
            // "Desactivado" es largo: en la burbuja angosta, con letra más chica
            let mut fs = chip_fs;
            let font_of = |fs: f32| if selected && !low { bold(fs) } else { regular(fs) };
            while fs > 8.5 && painter.layout_no_wrap(label.to_owned(), font_of(fs), col).size().x > vw - 8.0 {
                fs -= 0.5;
            }
            let font = font_of(fs);
            painter.text(rr.center(), Align2::CENTER_CENTER, fit_text(&painter, label, font.clone(), vw - 6.0), font, col);
            if chip.resp.clicked() && !selected {
                if disc {
                    self.cfg.vinyl_visualizer = DiscViz::ALL[i];
                } else {
                    self.cfg.visualizer = VizMode::ALL[i];
                }
                self.save_config(&ctx, format!("Visualizador: {}", label.to_lowercase()));
            }
            chip.resp.on_hover_text(if low { "En el modo Bajo no hay visualizador (cambiá a Medio o Alto)" } else { hint });
        }
        y += 32.0;
        let r = painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Portada con los graves", regular(12.5), if low { TEXT_FAINT } else { TEXT_DIM });
        ui.interact(r.expand(4.0), Id::unique("capyro-pulse-label"), Sense::hover()).on_hover_text(if low {
            "En el modo Bajo la portada no se mueve (cambiá a Medio o Alto)"
        } else {
            "La portada se agranda y se achica con los graves de la música"
        });
        if switch(ui, &painter, "cover-pulse", pos2(x1 - 17.0, y), self.cfg.cover_pulse && !low, accent, interactive && !low) {
            self.cfg.cover_pulse = !self.cfg.cover_pulse;
            let msg = if self.cfg.cover_pulse { "La portada late con los graves" } else { "La portada queda quieta" };
            self.save_config(&ctx, msg.to_owned());
        }
        y += 18.0;
        painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, Color32::from_white_alpha(14)));

        /* qué se muestra (textos cortos: también entran en la burbuja de
         * funciones de la vista de música, que es más angosta) */
        y += 18.0;
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Formato del archivo", regular(12.5), TEXT_DIM);
        if switch(ui, &painter, "show-format", pos2(x1 - 17.0, y), self.cfg.show_format, accent, interactive) {
            self.cfg.show_format = !self.cfg.show_format;
            let msg = if self.cfg.show_format { "Formato visible" } else { "Formato oculto" };
            self.save_config(&ctx, msg.to_owned());
        }
        y += 30.0;
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Milisegundos", regular(12.5), TEXT_DIM);
        if switch(ui, &painter, "show-millis", pos2(x1 - 17.0, y), self.cfg.show_millis, accent, interactive) {
            self.cfg.show_millis = !self.cfg.show_millis;
            let msg = if self.cfg.show_millis { "Milisegundos visibles" } else { "Milisegundos ocultos" };
            self.save_config(&ctx, msg.to_owned());
        }
        y += 30.0;
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Resolución y fps", regular(12.5), TEXT_DIM);
        if switch(ui, &painter, "show-video-info", pos2(x1 - 17.0, y), self.cfg.show_video_info, accent, interactive) {
            self.cfg.show_video_info = !self.cfg.show_video_info;
            let msg = if self.cfg.show_video_info { "Resolución y fps visibles" } else { "Resolución y fps ocultos" };
            self.save_config(&ctx, msg.to_owned());
        }
        // la interfaz adaptativa (v0.3.2)
        y += 30.0;
        let r = painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Vista de música", regular(12.5), TEXT_DIM);
        ui.interact(r.expand(4.0), Id::unique("capyro-music-view-label"), Sense::hover())
            .on_hover_text("Con un audio: la carátula grande al medio, la lista a un costado y las funciones al otro (si la ventana tiene lugar)");
        if switch(ui, &painter, "music-view", pos2(x1 - 17.0, y), self.cfg.music_view, accent, interactive) {
            self.cfg.music_view = !self.cfg.music_view;
            let msg = if self.cfg.music_view { "Vista de música activada" } else { "Vista de música desactivada" };
            self.save_config(&ctx, msg.to_owned());
        }
        y += 30.0;
        let r = painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Ocultar barra (videos)", regular(12.5), TEXT_DIM);
        ui.interact(r.expand(4.0), Id::unique("capyro-hide-bar-label"), Sense::hover())
            .on_hover_text("Un botón abajo del video para esconder los controles con un clic (también con la tecla B)");
        if switch(ui, &painter, "hide-bar", pos2(x1 - 17.0, y), self.cfg.hide_bar_button, accent, interactive) {
            self.cfg.hide_bar_button = !self.cfg.hide_bar_button;
            if !self.cfg.hide_bar_button {
                self.bar_hidden = false; // sin el botón, la barra vuelve
            }
            let msg = if self.cfg.hide_bar_button { "Botón para ocultar la barra activado" } else { "Botón para ocultar la barra desactivado" };
            self.save_config(&ctx, msg.to_owned());
        }
        y += 16.0;
        painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, Color32::from_white_alpha(14)));

        /* editar configuración (atajos) */
        y += 26.0;
        let row = Rect::from_min_max(pos2(x0 - 6.0, y - 18.0), pos2(x1 + 6.0, y + 18.0));
        let edit = animate_button(ui, Id::unique("capyro-settings-edit"), row, interactive);
        if edit.hover > 0.001 || edit.press > 0.001 {
            painter.rect_filled(row, 8.0, Color32::from_white_alpha((10.0 * edit.hover + 10.0 * edit.press) as u8));
        }
        let ic = pos2(x0 + 8.0, y - 1.0);
        icons::keyboard(&painter, ic, 16.0 * edit.scale(), edit.tint(ICON, Color32::WHITE));
        painter.text(pos2(x0 + 24.0, y - 7.0), Align2::LEFT_CENTER, "Editar configuración", regular(12.5), TEXT);
        painter.text(
            pos2(x0 + 24.0, y + 8.0),
            Align2::LEFT_CENTER,
            format!("Atajos de teclado · {} teclas", self.cfg.bindings.len()),
            regular(10.5),
            TEXT_FAINT,
        );
        // flechita: abre otra pantalla
        let chev = pos2(x1 - 2.0 + 2.0 * edit.hover, y);
        let st = Stroke::new(1.5, edit.tint(TEXT_FAINT, TEXT));
        painter.line_segment([chev + vec2(-4.0, -5.0), chev], st);
        painter.line_segment([chev + vec2(-4.0, 5.0), chev], st);
        if edit.resp.clicked() {
            self.show_settings = false;
            self.show_keys = true;
        }

        /* captura del fotograma */
        y += 40.0;
        let is_video = self.core.media_kind() == MediaKind::Video;
        let row = Rect::from_min_max(pos2(x0 - 6.0, y - 18.0), pos2(x1 + 6.0, y + 18.0));
        let shot = animate_button(ui, Id::unique("capyro-settings-shot"), row, interactive);
        if shot.hover > 0.001 || shot.press > 0.001 {
            painter.rect_filled(row, 8.0, Color32::from_white_alpha((10.0 * shot.hover + 10.0 * shot.press) as u8));
        }
        let col = if is_video { shot.tint(ICON, Color32::WHITE) } else { TEXT_FAINT };
        icons::camera(&painter, pos2(x0 + 8.0, y - 1.0), 15.0 * shot.scale(), col);
        painter.text(
            pos2(x0 + 24.0, y - 7.0),
            Align2::LEFT_CENTER,
            "Capturar fotograma",
            regular(12.5),
            if is_video { TEXT } else { TEXT_FAINT },
        );
        painter.text(
            pos2(x0 + 24.0, y + 8.0),
            Align2::LEFT_CENTER,
            if is_video { "Shift+S · se guarda como PNG" } else { "Solo con video" },
            regular(10.5),
            TEXT_FAINT,
        );
        if shot.resp.clicked() {
            self.show_settings = false;
            self.take_screenshot(&ctx);
        }

        /* repetir el tutorial */
        y += 40.0;
        let row = Rect::from_min_max(pos2(x0 - 6.0, y - 18.0), pos2(x1 + 6.0, y + 18.0));
        let tut = animate_button(ui, Id::unique("capyro-settings-tutorial"), row, interactive);
        if tut.hover > 0.001 || tut.press > 0.001 {
            painter.rect_filled(row, 8.0, Color32::from_white_alpha((10.0 * tut.hover + 10.0 * tut.press) as u8));
        }
        if let Some(logo) = &self.logo {
            let side = 18.0 * tut.scale();
            painter.image(logo.id(), Rect::from_center_size(pos2(x0 + 8.0, y - 1.0), Vec2::splat(side)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        painter.text(pos2(x0 + 24.0, y - 7.0), Align2::LEFT_CENTER, "Ver el tutorial de nuevo", regular(12.5), TEXT);
        painter.text(pos2(x0 + 24.0, y + 8.0), Align2::LEFT_CENTER, "La bienvenida y el recorrido", regular(10.5), TEXT_FAINT);
        if tut.resp.clicked() {
            self.show_settings = false;
            self.start_tutorial(&ctx);
        }

        // con scroll: barrita a la derecha
        if dock.is_none() && visible.height() < SETTINGS_H - 0.5 {
            let max = SETTINGS_H - visible.height();
            let track = Rect::from_min_max(pos2(visible.right() - 6.0, visible.top() + 8.0), pos2(visible.right() - 3.0, visible.bottom() - 8.0));
            let thumb_h = (track.height() * visible.height() / SETTINGS_H).max(24.0);
            let ty = track.top() + (track.height() - thumb_h) * (self.settings_scroll / max);
            painter.rect_filled(Rect::from_min_size(pos2(track.left(), ty), vec2(track.width(), thumb_h)), 1.5, Color32::from_white_alpha(50));
        }

        // Esc también lo cierra
        if dock.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.show_settings = false;
        }
        size.y
    }
}

//! Vista clásica (videos, y audio en ventanas chicas): el video, sus
//! etiquetas (formato, resolución, fps), la barra de progreso y los controles.

use crate::app::*;

/// Distancias desde el borde inferior del área de video (según el mockup).
pub(crate) const TRACK_Y: f32 = 14.0;
pub(crate) const TIMES_Y: f32 = 34.0;
pub(crate) const CONTROLS_Y: f32 = 81.0;
pub(crate) const BOTTOM_ROW_Y: f32 = 129.0;
pub(crate) const BELOW_VIDEO: f32 = 142.0;

impl CapyroApp {
    /// Cada cuánto redibujar mientras suena un audio. Con los milisegundos
    /// a la vista, a 30 cuadros (20 en Bajo); si no, solo cuando algo cambia
    /// de verdad: el reloj (una vez por segundo) o la barra (cuando avanza un
    /// píxel). Una canción de 3 minutos pasa de 30 redibujados por segundo a
    /// unos 4. (El visualizador y el fondo de Alto piden los suyos aparte.)
    pub(crate) fn audio_repaint_interval(&self, bar_w: f32) -> Duration {
        if self.cfg.show_millis {
            return crate::system::perf::ambient();
        }
        let speed = self.core.speed().max(0.25);
        let pos = self.core.position().max(0.0);
        let dur = self.core.duration();
        let to_next_second = (1.0 - pos.fract()) / speed;
        let per_pixel = if dur > 0.0 { dur / f64::from(bar_w.max(50.0)) / speed } else { 1.0 };
        Duration::from_secs_f64(to_next_second.min(per_pixel).clamp(0.04, 0.5))
    }

    /// Esconde o muestra la barra de abajo (videos). Los menús que cuelgan
    /// de la barra se cierran.
    pub(crate) fn set_bar_hidden(&mut self, hidden: bool) {
        self.bar_hidden = hidden;
        self.close_menus();
        self.show_playlist = self.show_playlist && !hidden;
    }

    /// Botón para ocultar / mostrar la barra (videos): abajo al medio del
    /// video. Con la barra a la vista aparece al acercar el mouse; con la
    /// barra escondida, al mover el mouse (y se va solo después de un rato).
    pub(crate) fn bar_toggle(&mut self, ui: &mut Ui, video: Rect, now: f64) {
        let ctx = ui.ctx().clone();
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let near = pointer.is_some_and(|p| video.contains(p) && p.y > video.bottom() - 90.0);
        let since_move = now - self.last_pointer_move;
        let want = if self.bar_hidden { near || since_move < 2.5 } else { near };
        let r = Rect::from_center_size(pos2(video.center().x, video.bottom() - 14.0), vec2(46.0, 18.0));
        let b = animate_button(ui, Id::unique("capyro-bar-toggle"), r.expand2(vec2(6.0, 4.0)), true);
        let vis = ctx.animate_bool_with_time(Id::unique("capyro-bar-toggle-vis"), want || b.hover > 0.01, crate::system::perf::t(0.2));
        if self.bar_hidden && since_move < 2.5 {
            // para que se vaya solo cuando el mouse se queda quieto
            ctx.request_repaint_after(Duration::from_secs_f64(2.5 - since_move + 0.05));
        }
        if vis <= 0.01 && !b.resp.clicked() {
            return;
        }
        let mut p = ui.painter().clone();
        p.multiply_opacity(vis);
        let rr = Rect::from_center_size(r.center(), r.size() * (1.0 + 0.08 * b.hover - 0.08 * b.press));
        if crate::ui::alto::on() {
            crate::ui::alto::glow_rect(&p, rr, self.cfg.accent, b.hover);
        }
        p.rect_filled(rr, 9.0, Color32::from_black_alpha((150.0 + 50.0 * b.hover) as u8));
        p.rect_stroke(rr, 9.0, Stroke::new(1.0, Color32::from_white_alpha(30)), StrokeKind::Inside);
        // flechita: abajo = esconder, arriba = mostrar
        let c = rr.center();
        let dy = if self.bar_hidden { -2.5 } else { 2.5 };
        let st = Stroke::new(1.6, Color32::WHITE.lerp_to_gamma(self.cfg.accent, b.hover));
        p.line_segment([c + vec2(-5.0, -dy), c + vec2(0.0, dy)], st);
        p.line_segment([c + vec2(5.0, -dy), c + vec2(0.0, dy)], st);
        if b.resp.clicked() {
            self.set_bar_hidden(!self.bar_hidden);
        }
        b.resp.on_hover_text(if self.bar_hidden { "Mostrar la barra (B)" } else { "Ocultar la barra (B)" });
    }

    /// Sube a la GPU el frame nuevo del núcleo (si cambió desde el último).
    ///
    /// Con el núcleo bloqueado solo se copian los bytes (un memcpy: el audio
    /// y el decodificador esperan lo menos posible); el armado de la imagen
    /// va después. Los cuadros son opacos, así que RGBA "sin premultiplicar"
    /// y premultiplicado son lo mismo y no hay que tocar cada píxel.
    pub(crate) fn sync_video_texture(&mut self, ctx: &egui::Context) {
        let last = self.video_serial;
        let mut buf = std::mem::take(&mut self.frame_buf);
        let fresh = self.core.with_video_frame(|vf| {
            if vf.serial == last {
                return None;
            }
            buf.clear();
            buf.extend_from_slice(vf.rgba);
            Some(([vf.width, vf.height], vf.serial, vf.aspect))
        });
        let alto = self.cfg.performance == crate::system::perf::Mode::Alto;
        let fresh = fresh.map(|f| {
            f.map(|(size, serial, aspect)| {
                if alto {
                    self.ambient.sample(&buf, size[0], size[1]);
                }
                // los cuadros son opacos (alfa 255): RGBA y "premultiplicado"
                // son los mismos bytes, así que alcanza con una copia de
                // memoria (antes se armaba píxel por píxel)
                let pixels: Vec<Color32> = bytemuck::cast_slice::<u8, Color32>(&buf).to_vec();
                (egui::ColorImage::new(size, pixels), serial, aspect)
            })
        });
        self.frame_buf = buf;
        match fresh {
            None => {
                // no hay frame (nada cargado o mp3 sin carátula)
                self.video_tex = None;
                self.ambient.clear();
            }
            Some(None) => {}               // mismo frame que ya tenemos
            Some(Some((img, serial, aspect))) => {
                if self.core.state() == State::Playing && self.core.media_kind() == MediaKind::Video {
                    self.fps_meter.frame(ctx.input(|i| i.time));
                } else {
                    self.fps_meter.reset();
                }
                // los cuadros de video ya vienen del tamaño en que se ven (1:1): sin placa
                // de video, el filtro "más cercano" se ve igual y cuesta la cuarta parte
                // (las carátulas se animan y se escalan: ahí, el suave)
                let opts = if crate::system::perf::software() && self.core.media_kind() == MediaKind::Video {
                    TextureOptions::NEAREST
                } else {
                    TextureOptions::LINEAR
                };
                match &mut self.video_tex {
                    Some(tex) => tex.set(img, opts),
                    None => {
                        self.video_tex = Some(ctx.load_texture("capyro-video", img, opts));
                    }
                }
                self.video_serial = serial;
                self.video_aspect = aspect;
            }
        }
    }

    pub(crate) fn video_area(&mut self, ui: &mut Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 10.0, BG_VIDEO);
        // Alto: también adentro del área de video, alrededor de la imagen
        // (las franjas negras y el fondo de la carátula)
        let lit = self.cfg.performance == crate::system::perf::Mode::Alto;
        if lit {
            let reach = rect.width().max(rect.height()) * 0.36;
            self.ambient.paint(&painter, rect.shrink(2.0), 0.30, reach, self.ambient.source, None);
            self.ambient_under = Some(rect);
            if self.core.media_kind() != MediaKind::Video {
                // audio o nada cargado: el polvito y la luz del mouse también acá
                let accent = self.cfg.accent;
                let hide = self.ambient.source.filter(|_| self.video_tex.is_some());
                self.backdrop.paint_particles(&painter, rect.shrink(2.0), accent, hide, 18);
                self.backdrop.paint_cursor(&painter, rect.shrink(2.0), accent, None);
            }
        }
        // Alto: al cambiar de archivo, un destello cruza el área y el título entra
        let enter = match self.track_at {
            Some(t0) if lit => {
                let k = ((ctx.input(|i| i.time) - t0) / 0.9) as f32;
                if k >= 1.0 {
                    self.track_at = None;
                }
                k.clamp(0.0, 1.0)
            }
            _ => 1.0,
        };
        let mut source = None;

        // el núcleo convierte los cuadros al tamaño en que se ven, no más
        let ppp = ctx.pixels_per_point();
        let target = ((rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32);
        if target != self.video_target {
            self.video_target = target;
            self.core.set_video_target(target.0, target.1);
        }

        let resp = ui.interact(rect, Id::unique("capyro-video"), Sense::click());
        if resp.double_clicked() {
            self.run_action(&ctx, Action::Fullscreen);
        } else if resp.clicked() {
            if self.core.state() == State::Idle {
                self.open_file_dialog(&ctx); // área vacía: clic = elegir archivos
            } else {
                self.core.toggle();
            }
        }

        let hovering_files = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let c = rect.center();
        let accent = self.cfg.accent;

        if hovering_files {
            painter.rect_stroke(rect.shrink(1.0), 10.0, Stroke::new(2.0, accent), StrokeKind::Inside);
            icons::film(&painter, c - vec2(0.0, 30.0), 28.0, accent);
            painter.text(c + vec2(0.0, 4.0), Align2::CENTER_CENTER, "Suelta para abrir (archivos o carpetas)", regular(14.0), TEXT);
            // video en una línea y audio en otra: con 13 formatos, en una sola
            // no entraban en una ventana chica
            let (v, a) = &self.formats;
            for (i, list) in [v, a].into_iter().enumerate() {
                let line = list.join(" · ").to_uppercase();
                painter.text(c + vec2(0.0, 26.0 + 17.0 * i as f32), Align2::CENTER_CENTER, line, regular(12.0), TEXT_FAINT);
            }
        } else {
            match self.core.media_kind() {
                MediaKind::None => {
                    icons::film(&painter, c - vec2(0.0, 33.0), 28.0, Color32::from_rgb(0x55, 0x56, 0x5C));
                    painter.text(c, Align2::CENTER_CENTER, "Área de vídeo", regular(14.0), TEXT_DIM);
                    painter.text(
                        c + vec2(0.0, 27.0),
                        Align2::CENTER_CENTER,
                        "Arrastra un archivo o haz clic para elegirlo",
                        regular(12.0),
                        TEXT_FAINT,
                    );
                }
                MediaKind::Video => {
                    if let Some(tex) = &self.video_tex {
                        // Letterbox: el video entra entero manteniendo su aspecto.
                        let avail = rect.shrink(1.0).size();
                        let mut size = vec2(avail.x, avail.x / self.video_aspect);
                        if size.y > avail.y {
                            size = vec2(avail.y * self.video_aspect, avail.y);
                        }
                        let dest = Rect::from_center_size(c, size);
                        source = Some(dest);
                        painter.rect_filled(dest, 0.0, Color32::BLACK);
                        painter.image(tex.id(), dest, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    } else {
                        icons::film(&painter, c - vec2(0.0, 33.0), 28.0, accent);
                        painter.text(c, Align2::CENTER_CENTER, "Cargando…", regular(14.0), TEXT_DIM);
                    }
                }
                MediaKind::Audio => {
                    let (title, artist) = self.track_labels();
                    // (con el disco, el lugar es el de una portada aunque no tenga)
                    let has_cover = self.video_tex.is_some() || self.cfg.vinyl;
                    let side = (rect.height() * 0.5).clamp(80.0, 260.0);
                    let art_center = c - vec2(0.0, if has_cover { 28.0 } else { 36.0 });
                    let text_y = if has_cover { art_center.y + side / 2.0 + 24.0 } else { c.y + 6.0 };
                    let disc = self.cfg.vinyl;
                    let disc_viz = self.cfg.disc_viz();
                    let bars_below = if disc {
                        disc_viz == crate::system::config::DiscViz::Below
                    } else {
                        self.cfg.viz_mode() != crate::system::config::VizMode::Off
                    };
                    if bars_below && !self.viz.is_silent() {
                        // barras de frecuencia de fondo, debajo del título y el artista
                        let text_bottom = text_y + if artist.is_empty() { 14.0 } else { 34.0 };
                        let top = (rect.bottom() - rect.height() * 0.30).max(text_bottom + 6.0);
                        if rect.bottom() - 12.0 - top > 18.0 {
                            let band = Rect::from_min_max(
                                pos2(rect.left() + 24.0, top),
                                pos2(rect.right() - 24.0, rect.bottom() - 12.0),
                            );
                            self.viz.draw(&painter, band, accent.gamma_multiply(0.55));
                        }
                    }

                    if disc {
                        // disco de vinilo (⚙): la portada como etiqueta, girando
                        let around = disc_viz == crate::system::config::DiscViz::Around;
                        let r = side / 2.0 * if around { 0.84 } else { 1.0 };
                        let label = match &self.video_tex {
                            Some(t) => crate::ui::vinyl::Label::Cover(t.id(), self.video_aspect.max(0.05)),
                            None => crate::ui::vinyl::Label::Plain,
                        };
                        crate::ui::theme::soft_shadow(&painter, Rect::from_center_size(art_center, Vec2::splat(2.0 * r)), r, 12.0, 4.0, 90);
                        crate::ui::vinyl::draw(&painter, art_center, r, self.vinyl_angle, &label, accent, 1.0, crate::system::perf::animations());
                        if self.video_tex.is_some() {
                            source = Some(Rect::from_center_size(art_center, Vec2::splat(2.0 * r * crate::ui::vinyl::LABEL)));
                        }
                        if around && !self.viz.is_silent() {
                            self.viz.draw_around(&painter, art_center, r + 4.0, side / 2.0 - r + 10.0, accent.gamma_multiply(0.8));
                        }
                    } else if let Some(tex) = &self.video_tex {
                        let art = Rect::from_center_size(art_center, Vec2::splat(side));
                        let mut size = vec2(side, side / self.video_aspect);
                        if size.y > side {
                            size = vec2(side * self.video_aspect, side);
                        }
                        source = Some(Rect::from_center_size(art_center, size));
                        painter.rect_filled(art.expand(1.0), 10.0, BG_PANEL);
                        painter.image(
                            tex.id(),
                            Rect::from_center_size(art_center, size),
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        painter.circle_filled(art_center, 30.0, accent.gamma_multiply(0.18));
                        icons::music_note(&painter, art_center, 28.0, accent);
                    }
                    // (en Alto, al cambiar de canción el título sube y aparece; el artista, detrás)
                    let ke = |delay: f32| ease_out(((enter - delay) / (1.0 - delay)).clamp(0.0, 1.0));
                    let (k1, k2) = (ke(0.15), ke(0.3));
                    painter.text(
                        pos2(c.x, text_y + 10.0 * (1.0 - k1)),
                        Align2::CENTER_CENTER,
                        ellipsize(&title, 52),
                        bold(15.0),
                        TEXT.gamma_multiply(k1),
                    );
                    if !artist.is_empty() {
                        painter.text(
                            pos2(c.x, text_y + 22.0 + 10.0 * (1.0 - k2)),
                            Align2::CENTER_CENTER,
                            ellipsize(&artist, 60),
                            regular(12.5),
                            TEXT_DIM.gamma_multiply(k2),
                        );
                    }
                }
            }
        }

        if lit && source.is_some() {
            self.ambient.source = source;
        }
        if enter < 1.0 {
            crate::ui::alto::sweep(&painter, rect, enter, self.cfg.accent);
            ctx.request_repaint_after(FRAME_SMOOTH);
        }
        if !hovering_files && self.core.media_kind() != MediaKind::None {
            self.format_badge(ui, rect);
        }
        if !hovering_files && self.core.media_kind() == MediaKind::Video {
            self.video_info_badges(ui, rect);
        }
        if self.show_playlist {
            self.playlist_panel(ui, rect);
        }
    }

    /// Arriba a la izquierda del área de video, una debajo de la otra:
    /// el formato ("MP4 · H.264", "MP3"...) y los milisegundos del segundo
    /// actual. Cada una se oculta en Ajustes; si se oculta el formato, la de
    /// los milisegundos sube deslizándose a su lugar (y baja si vuelve).
    pub(crate) fn format_badge(&self, ui: &Ui, area: Rect) {
        let ctx = ui.ctx();
        let tf = ease_out(ctx.animate_bool_with_time(Id::unique("capyro-format-badge"), self.cfg.show_format, crate::system::perf::t(0.3)));
        let tm = ease_out(ctx.animate_bool_with_time(Id::unique("capyro-millis-badge"), self.cfg.show_millis, crate::system::perf::t(0.3)));
        if tf <= 0.001 && tm <= 0.001 {
            return;
        }
        let accent = self.cfg.accent;
        const H: f32 = 24.0;
        const GAP: f32 = 6.0;

        // formato
        if tf > 0.001 {
            let ext = Path::new(&self.np.file)
                .extension()
                .map(|e| e.to_string_lossy().to_uppercase())
                .unwrap_or_default();
            if !ext.is_empty() {
                let info = self.core.decode_info();
                let detail = if info.active && !info.codec.is_empty() {
                    if info.hw { format!("{} · GPU", info.codec) } else { info.codec.clone() }
                } else {
                    String::new()
                };
                let mut p = ui.painter().clone();
                p.multiply_opacity(tf);
                let eg = p.layout_no_wrap(ext, bold(11.5), Color32::WHITE);
                let dg = (!detail.is_empty()).then(|| p.layout_no_wrap(detail, regular(11.0), TEXT_DIM));
                let w = 12.0 + eg.size().x + dg.as_ref().map_or(0.0, |g| 10.0 + g.size().x) + 12.0;
                let r = Rect::from_min_size(area.min + vec2(10.0, 10.0 - 4.0 * (1.0 - tf)), vec2(w, H));
                p.rect_filled(r, 12.0, Color32::from_black_alpha(150));
                p.rect_stroke(r, 12.0, Stroke::new(1.0, Color32::from_white_alpha(18)), StrokeKind::Inside);
                let er = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 4.0), vec2(eg.size().x + 16.0, 16.0));
                p.rect_filled(er, 8.0, accent);
                p.galley(er.center() - eg.size() / 2.0, eg, Color32::WHITE);
                if let Some(g) = dg {
                    p.galley(pos2(er.right() + 6.0, r.center().y - g.size().y / 2.0), g, TEXT_DIM);
                }
            }
        }

        // milisegundos dentro del segundo actual (000..999)
        if tm > 0.001 {
            let ms = ((self.core.position().max(0.0).fract()) * 1000.0) as u32 % 1000;
            let mut p = ui.painter().clone();
            p.multiply_opacity(tm);
            // ancho fijo (con "000") para que la burbuja no tiemble
            let ng = p.layout_no_wrap(format!("{ms:03}"), FontId::monospace(12.0), Color32::WHITE);
            let nw = p.layout_no_wrap("000".to_owned(), FontId::monospace(12.0), Color32::WHITE).size().x;
            let ug = p.layout_no_wrap("ms".to_owned(), regular(11.0), TEXT_DIM);
            let w = 12.0 + nw + 5.0 + ug.size().x + 12.0;
            // baja un lugar si se ve el formato; sube deslizándose si no
            let y = area.top() + 10.0 + (H + GAP) * tf - 4.0 * (1.0 - tm);
            let r = Rect::from_min_size(pos2(area.left() + 10.0, y), vec2(w, H));
            p.rect_filled(r, 12.0, Color32::from_black_alpha(150));
            p.rect_stroke(r, 12.0, Stroke::new(1.0, Color32::from_white_alpha(18)), StrokeKind::Inside);
            // puntito que late una vez por segundo
            let beat = 1.0 - ms as f32 / 1000.0;
            p.circle_filled(pos2(r.left() + 9.0, r.center().y), 2.5, accent.gamma_multiply(0.35 + 0.65 * beat));
            let x = r.left() + 16.0;
            p.galley(pos2(x, r.center().y - ng.size().y / 2.0), ng, Color32::WHITE);
            p.galley(pos2(x + nw + 5.0, r.center().y - ug.size().y / 2.0), ug, TEXT_DIM);
            // que corran los números: 30 por segundo alcanzan para verlos correr
            // (con un video no hace falta: ya se redibuja con cada cuadro, y pedir
            // aparte suma cuadros de más que no coinciden con los del video)
            if self.core.state() == State::Playing && self.core.media_kind() != MediaKind::Video {
                ui.ctx().request_repaint_after(crate::system::perf::ambient());
            }
        }
        if (0.001..0.999).contains(&tf) || (0.001..0.999).contains(&tm) {
            ui.ctx().request_repaint_after(FRAME_SMOOTH);
        }
    }

    /// Abajo a la izquierda del video, una al lado de la otra: la resolución
    /// "de nombre" (1080p, 1440p · 2K, 4K...) y los cuadros por segundo. La
    /// etiqueta es solo el indicador: el video se ve tal cual es. Se ocultan
    /// juntas en Ajustes (entran y salen deslizándose).
    pub(crate) fn video_info_badges(&self, ui: &Ui, area: Rect) {
        let ctx = ui.ctx();
        let t = ease_out(ctx.animate_bool_with_time(Id::unique("capyro-videoinfo-badge"), self.cfg.show_video_info, crate::system::perf::t(0.3)));
        if t <= 0.001 {
            return;
        }
        let Some((w, h, fps)) = self.core.video_props() else { return };
        let accent = self.cfg.accent;
        const H: f32 = 24.0;
        let now = ctx.input(|i| i.time);
        let y = area.bottom() - 10.0 - H + 4.0 * (1.0 - t);
        let mut p = ui.painter().clone();
        p.multiply_opacity(t);

        // resolución
        let (label, alias) = crate::media::videoinfo::resolution_label(w, h);
        let lg = p.layout_no_wrap(label.to_owned(), bold(11.5), Color32::WHITE);
        let ag = (!alias.is_empty()).then(|| p.layout_no_wrap(alias.to_owned(), regular(11.0), TEXT_DIM));
        let rw = 12.0 + lg.size().x + ag.as_ref().map_or(0.0, |g| 10.0 + g.size().x) + 12.0;
        let r = Rect::from_min_size(pos2(area.left() + 10.0, y), vec2(rw, H));
        p.rect_filled(r, 12.0, Color32::from_black_alpha(150));
        p.rect_stroke(r, 12.0, Stroke::new(1.0, Color32::from_white_alpha(18)), StrokeKind::Inside);
        let lr = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 4.0), vec2(lg.size().x + 16.0, 16.0));
        p.rect_filled(lr, 8.0, accent);
        p.galley(lr.center() - lg.size() / 2.0, lg, Color32::WHITE);
        if let Some(g) = ag {
            p.galley(pos2(lr.right() + 6.0, r.center().y - g.size().y / 2.0), g, TEXT_DIM);
        }
        ui.interact(r, Id::unique("capyro-videoinfo-res"), Sense::hover())
            .on_hover_text(format!("El archivo es de {w}×{h}"));

        // fps: los del video; si la PC no llega, los que se ven, en otro color
        let playing = self.core.state() == State::Playing;
        let shown = if playing { self.fps_meter.rate(now) } else { None };
        let slow = matches!(shown, Some(s) if fps > 0.0 && s < fps * 0.85);
        let value = if slow { shown.unwrap_or(fps) } else { fps };
        let col = if slow { Color32::from_rgb(0xFF, 0xC9, 0x3C) } else { Color32::WHITE };
        let ng = p.layout_no_wrap(crate::media::videoinfo::fps_text(if slow { value.round() } else { value }), FontId::monospace(12.0), col);
        let ug = p.layout_no_wrap("fps".to_owned(), regular(11.0), TEXT_DIM);
        let fw = 16.0 + ng.size().x + 5.0 + ug.size().x + 12.0;
        let f = Rect::from_min_size(pos2(r.right() + 6.0, y), vec2(fw, H));
        p.rect_filled(f, 12.0, Color32::from_black_alpha(150));
        p.rect_stroke(f, 12.0, Stroke::new(1.0, Color32::from_white_alpha(18)), StrokeKind::Inside);
        // puntito: late mientras corre el video, quieto en pausa
        let beat = if playing { 0.6 + 0.4 * ((crate::system::perf::clock(now) * 2.0 * std::f64::consts::PI).sin() as f32).abs() } else { 0.45 };
        p.circle_filled(pos2(f.left() + 9.0, f.center().y), 2.5, if slow { col } else { accent.gamma_multiply(beat) });
        p.galley(pos2(f.left() + 16.0, f.center().y - ng.size().y / 2.0), ng.clone(), col);
        p.galley(pos2(f.left() + 16.0 + ng.size().x + 5.0, f.center().y - ug.size().y / 2.0), ug, TEXT_DIM);
        let tip = match shown {
            Some(s) if slow => format!(
                "El video es de {} fps pero se están viendo {:.0}: la PC no da abasto (probá ⚙ → Decodificación → GPU)",
                crate::media::videoinfo::fps_text(fps),
                s
            ),
            Some(s) => format!("{} fps del video · se están viendo {:.0}", crate::media::videoinfo::fps_text(fps), s),
            None => format!("Cuadros por segundo del video: {}", crate::media::videoinfo::fps_text(fps)),
        };
        ui.interact(f, Id::unique("capyro-videoinfo-fps"), Sense::hover()).on_hover_text(tip);

        if (0.001..0.999).contains(&t) {
            ctx.request_repaint_after(FRAME_SMOOTH);
        }
    }

    /// Barra de progreso (con vista previa al pasar el mouse) y los tiempos
    /// abajo. `ty` = altura de la barra, `tty` = la de los tiempos.
    pub(crate) fn seek_bar(&mut self, ui: &mut Ui, painter: &egui::Painter, left: f32, right: f32, ty: f32, tty: f32) {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;
        let hit = Rect::from_min_max(pos2(left, ty - 8.0), pos2(right, ty + 8.0));
        self.ui_rects.insert("seek", Rect::from_min_max(pos2(left, ty - 10.0), pos2(right, tty + 10.0)));
        let seek = ui.interact(hit, Id::unique("capyro-seek"), Sense::click_and_drag());
        let duration = self.core.duration();
        let can_seek = duration > 0.0;
        if can_seek && (seek.dragged() || seek.clicked()) {
            if let Some(pp) = seek.interact_pointer_pos() {
                let f = ((pp.x - left) / (right - left)).clamp(0.0, 1.0) as f64;
                self.core.seek(f * duration);
            }
        }
        let position = self.core.position();
        let frac = if can_seek { (position / duration).clamp(0.0, 1.0) as f32 } else { 0.0 };
        let active = can_seek && (seek.hovered() || seek.dragged());
        let grow = ctx.animate_bool_with_time(Id::unique("capyro-seek-grow"), active, crate::system::perf::t(0.12));
        let th = 3.0 + 2.0 * grow;
        let track = Rect::from_min_max(pos2(left, ty - th / 2.0), pos2(right, ty + th / 2.0));
        painter.rect_filled(track, th / 2.0, TRACK);
        let fill_x = left + (right - left) * frac;
        if let Some(hp) = seek.hover_pos().filter(|_| active) {
            // vista previa de hasta dónde saltaría
            let hx = hp.x.clamp(left, right);
            if hx > fill_x {
                painter.rect_filled(
                    Rect::from_min_max(pos2(fill_x, track.min.y), pos2(hx, track.max.y)),
                    th / 2.0,
                    Color32::from_white_alpha(28),
                );
            }
        }
        if crate::ui::alto::on() && can_seek {
            // Alto: la punta de la barra brilla (más con el mouse encima)
            let playing = self.core.state() == State::Playing;
            crate::ui::alto::glow(&painter, pos2(fill_x, ty), 16.0 + 10.0 * grow, accent, 0.12 * playing as u8 as f32 + 0.35 * grow);
        }
        painter.rect_filled(Rect::from_min_max(track.min, pos2(fill_x, track.max.y)), th / 2.0, accent);
        // Alto: un destello recorre lo que ya sonó, cada 3 segundos
        if crate::ui::alto::on() && self.core.state() == State::Playing && fill_x - left > 24.0 {
            let now = ui.ctx().input(|i| i.time);
            let ph = (now / 3.0).fract() as f32;
            let x = left + (fill_x - left) * ph;
            let clip = Rect::from_min_max(pos2(left, ty - 8.0), pos2(fill_x, ty + 8.0));
            let p = painter.clone().with_clip_rect(clip.intersect(painter.clip_rect()));
            crate::ui::alto::glow(&p, pos2(x, ty), 26.0, Color32::WHITE, 0.55 * (1.0 - (2.0 * ph - 1.0).abs()));
        }
        if grow > 0.001 {
            let pressed = ctx.animate_bool_with_time(Id::unique("capyro-seek-press"), seek.dragged(), crate::system::perf::t(0.08));
            painter.circle_filled(pos2(fill_x, ty), 6.5 * grow + 1.5 * pressed, accent);
        }
        if active {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
            if let Some(hp) = seek.hover_pos() {
                let f = ((hp.x - left) / (right - left)).clamp(0.0, 1.0) as f64;
                let label = fmt_time(f * duration);
                let g = painter.layout_no_wrap(label, regular(11.0), TEXT);
                let r = Rect::from_center_size(pos2(hp.x, ty - 18.0), g.size() + vec2(10.0, 6.0));
                painter.rect_filled(r, 4.0, BG_PANEL);
                painter.galley(r.center() - g.size() / 2.0, g, TEXT);
            }
        }

        /* --- tiempos (+ velocidad si no es 1×) --- */
        painter.text(pos2(left, tty), Align2::LEFT_CENTER, fmt_time(position), regular(12.0), TEXT_DIM);
        // Clic en el tiempo de la derecha: duración total ↔ tiempo que falta.
        let right_label = if self.show_remaining && duration > 0.0 {
            format!("-{}", fmt_time((duration - position).max(0.0)))
        } else {
            fmt_time(duration)
        };
        let g = painter.layout_no_wrap(right_label, regular(12.0), TEXT_DIM);
        let dur_rect = Rect::from_min_size(pos2(right - g.size().x, tty - g.size().y / 2.0), g.size());
        let dur_btn = animate_button(ui, Id::unique("capyro-time-toggle"), dur_rect.expand2(vec2(6.0, 4.0)), duration > 0.0);
        if dur_btn.hover > 0.001 {
            painter.rect_filled(dur_rect.expand2(vec2(6.0, 3.0)), 5.0, Color32::from_white_alpha((12.0 * dur_btn.hover) as u8));
        }
        painter.galley(dur_rect.min, g, dur_btn.tint(TEXT_DIM, TEXT));
        if dur_btn.resp.clicked() {
            self.show_remaining = !self.show_remaining;
        }
        dur_btn.resp.on_hover_text(if self.show_remaining { "Mostrar duración total" } else { "Mostrar tiempo restante" });
        let speed = self.core.speed();
        if (speed - 1.0).abs() > 1e-6 {
            let g = painter.layout_no_wrap(fmt_speed(speed), bold(11.0), accent);
            let r = Rect::from_min_size(
                pos2(dur_rect.left() - g.size().x - 18.0, tty - g.size().y / 2.0 - 3.0),
                g.size() + vec2(10.0, 6.0),
            );
            painter.rect_filled(r, r.height() / 2.0, accent.gamma_multiply(0.15));
            painter.galley(r.center() - g.size() / 2.0, g, accent);
        }

    }

    /// Aleatorio, anterior, desde el principio, play, siguiente y repetir,
    /// centrados en `cx` (`spread` < 1 los junta en espacios chicos).
    /// Devuelve dónde quedó el botón de repetir.
    pub(crate) fn control_row(&mut self, ui: &mut Ui, painter: &egui::Painter, cx: f32, cy: f32, spread: f32) -> Pos2 {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;

        // aleatorio (izquierda)
        let shuffle_on = self.core.shuffle();
        let shuffle_c = pos2(cx - 105.0 * spread, cy);
        let sh = Self::icon_button(ui, "shuffle", shuffle_c, 30.0);
        let sh_on = ctx.animate_bool_with_time(Id::unique("capyro-shuffle-on"), shuffle_on, crate::system::perf::t(0.15));
        let sh_col = sh.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, sh_on);
        icons::shuffle(&painter, shuffle_c, 16.0 * sh.scale(), sh_col);
        if sh_on > 0.001 {
            painter.circle_filled(shuffle_c + vec2(0.0, 13.0), 2.0 * sh_on, accent);
        }
        if sh.resp.clicked() {
            self.toggle_shuffle(&ctx);
        }
        sh.resp.on_hover_text(if shuffle_on { "Aleatorio: activado (Ctrl+S)" } else { "Aleatorio: desactivado (Ctrl+S)" });

        let prev_c = pos2(cx - 69.0 * spread, cy);
        let prev = Self::icon_button(ui, "prev", prev_c, 30.0);
        icons::prev(&painter, prev_c - vec2(2.0 * prev.press, 0.0), 16.0 * prev.scale(), prev.tint(ICON, Color32::WHITE));
        if prev.resp.clicked() {
            self.run_action(&ctx, Action::PrevFile);
        }

        let restart_c = pos2(cx - 33.0 * spread, cy);
        let restart = Self::icon_button(ui, "restart", restart_c, 30.0);
        icons::restart(&painter, restart_c, 16.0 * restart.scale(), restart.tint(ICON, Color32::WHITE));
        if restart.resp.clicked() {
            self.core.seek(0.0);
        }
        restart.resp.on_hover_text("Volver al inicio");

        // play / pausa: crece, brilla y el ícono se transforma
        let play_c = pos2(cx + 17.0 * spread, cy);
        let play = animate_button(ui, Id::unique("capyro-play"), Rect::from_center_size(play_c, Vec2::splat(46.0)), true);
        let playing = self.core.state() == State::Playing;
        let morph = ctx.animate_bool_with_time(Id::unique("capyro-play-morph"), playing, crate::system::perf::t(0.16));
        let scale = 1.0 + 0.06 * play.hover - 0.1 * play.press;
        if crate::ui::alto::on() {
            // Alto: halo que respira mientras suena y brilla más con el mouse
            let breathe = if playing { 0.10 + 0.06 * (ctx.input(|i| i.time) * 2.2).sin() as f32 } else { 0.04 };
            crate::ui::alto::glow(&painter, play_c, 62.0 + 10.0 * play.hover, accent, breathe + 0.28 * play.hover);
            if play.resp.clicked() {
                crate::ui::alto::ripple(&ctx, play_c, 26.0);
            }
        }
        if play.hover > 0.001 {
            painter.circle_filled(play_c, 23.0 * scale + 6.0 * play.hover, accent.gamma_multiply(0.18 * play.hover));
        }
        let fill = accent.lerp_to_gamma(Color32::WHITE, 0.12 * play.hover).gamma_multiply(1.0 - 0.12 * play.press);
        painter.circle_filled(play_c, 23.0 * scale, fill);
        if morph < 0.999 {
            icons::play_badge(&painter, play_c, 21.0 * scale, Color32::WHITE.gamma_multiply(1.0 - morph), false);
        }
        if morph > 0.001 {
            icons::play_badge(&painter, play_c, 21.0 * scale, Color32::WHITE.gamma_multiply(morph), true);
        }
        if play.resp.clicked() {
            self.run_action(&ctx, Action::PlayPause);
        }

        let next_c = pos2(cx + 69.0 * spread, cy);
        let next = Self::icon_button(ui, "next", next_c, 30.0);
        icons::next(&painter, next_c + vec2(2.0 * next.press, 0.0), 16.0 * next.scale(), next.tint(ICON, Color32::WHITE));
        if next.resp.clicked() {
            self.run_action(&ctx, Action::NextFile);
        }

        // repetir (derecha): off → lista → uno
        let repeat = self.core.repeat();
        let repeat_c = pos2(cx + 105.0 * spread, cy);
        self.ui_rects.insert("controls", Rect::from_min_max(pos2(cx - 123.0 * spread, cy - 25.0), pos2(cx + 123.0 * spread, cy + 25.0)));
        let rp = Self::icon_button(ui, "repeat", repeat_c, 30.0);
        let rp_on = ctx.animate_bool_with_time(Id::unique("capyro-repeat-on"), repeat != Repeat::Off, crate::system::perf::t(0.15));
        let rp_col = rp.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, rp_on);
        icons::repeat(&painter, repeat_c, 16.0 * rp.scale(), rp_col, repeat == Repeat::One);
        if rp_on > 0.001 {
            painter.circle_filled(repeat_c + vec2(0.0, 13.0), 2.0 * rp_on, accent);
        }
        if rp.resp.clicked() {
            self.cycle_repeat(&ctx);
        }
        rp.resp.on_hover_text(match repeat {
            Repeat::Off => "Repetir: no (Ctrl+R)",
            Repeat::All => "Repetir: toda la lista (Ctrl+R)",
            Repeat::One => "Repetir: este archivo (Ctrl+R)",
        });

        repeat_c
    }

    /// El botón naranja "+ Agregar archivo(s)" en `base_rect` (solo el "+"
    /// si es `compact`).
    pub(crate) fn add_files_button(&mut self, ui: &mut Ui, painter: &egui::Painter, base_rect: Rect, compact: bool) {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;
        let pending = self.dialog.is_some();
        let label = if pending { "Abriendo…" } else { "Agregar archivo(s)" };
        let galley = painter.layout_no_wrap(label.to_owned(), bold(13.0), Color32::WHITE);
        let add = animate_button(ui, Id::unique("capyro-add-files"), base_rect, !pending);
        let s = 1.0 + 0.03 * add.hover - 0.06 * add.press;
        let add_rect = Rect::from_center_size(base_rect.center(), base_rect.size() * s);
        if crate::ui::alto::on() {
            crate::ui::alto::glow_rect(&painter, add_rect, accent, add.hover);
            if add.resp.clicked() {
                crate::ui::alto::ripple(&ctx, add_rect.center(), add_rect.height() * 0.8);
            }
        }
        if add.hover > 0.001 {
            painter.rect_filled(add_rect.expand(4.0 * add.hover), add_rect.height() / 2.0 + 4.0, accent.gamma_multiply(0.16 * add.hover));
        }
        let fill = if pending {
            accent.gamma_multiply(0.6)
        } else {
            accent.lerp_to_gamma(Color32::WHITE, 0.12 * add.hover).gamma_multiply(1.0 - 0.12 * add.press)
        };
        painter.rect_filled(add_rect, add_rect.height() / 2.0, fill);
        // el "+" gira un cuarto de vuelta al pasar el mouse
        let angle = add.hover * std::f32::consts::FRAC_PI_2;
        if compact {
            icons::plus(&painter, add_rect.center(), 14.0, Color32::WHITE, angle, 2.0);
        } else {
            // el "+" y el texto, centrados (en la lista de la vista de música
            // el botón ocupa todo el ancho)
            let content_w = 12.0 + galley.size().x;
            let plus_c = pos2(add_rect.center().x - content_w / 2.0 + 2.0, add_rect.center().y);
            icons::plus(&painter, plus_c, 11.0, Color32::WHITE, angle, 2.0);
            painter.galley(
                pos2(plus_c.x + 12.0, add_rect.center().y - galley.size().y / 2.0),
                galley,
                Color32::WHITE,
            );
        }
        if add.resp.clicked() {
            self.open_file_dialog(&ctx);
        }
        add.resp.on_hover_text(format!("Agregar archivos ({}) a la lista (Ctrl+O)", self.formats_label(", ")));
    }

    pub(crate) fn transport(&mut self, ui: &mut Ui, video_bottom: f32, inner: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();
        let accent = self.cfg.accent;
        let (left, right) = (inner.left(), inner.right());

        /* --- barra de progreso --- */
        self.seek_bar(ui, &painter, left, right, video_bottom + TRACK_Y, video_bottom + TIMES_Y);

        /* --- controles --- */
        let cy = video_bottom + CONTROLS_Y;
        let cx = inner.center().x;
        let repeat_c = self.control_row(ui, &painter, cx, cy, 1.0);

        /* --- botón "+ Agregar archivo(s)" (sobre los íconos de la derecha) --- */
        let pending = self.dialog.is_some();
        let label = if pending { "Abriendo…" } else { "Agregar archivo(s)" };
        let full_w = painter.layout_no_wrap(label.to_owned(), bold(13.0), Color32::WHITE).size().x + 50.0;
        let controls_right = repeat_c.x + 22.0;
        // Si la ventana es angosta y no entra el texto, queda solo el "+".
        let compact = right - full_w < controls_right;
        let base = if compact { vec2(34.0, 34.0) } else { vec2(full_w, 34.0) };
        let base_rect = Rect::from_min_size(pos2(right - base.x, cy - base.y / 2.0), base);
        self.ui_rects.insert("add", base_rect);
        self.add_files_button(ui, &painter, base_rect, compact);

        /* --- botón de información (arriba de "Lista", a la izquierda) --- */
        let info_c = pos2(left + 9.0, cy);
        let info = Self::icon_button(ui, "info", info_c, 30.0);
        let info_on = ctx.animate_bool_with_time(Id::unique("capyro-info-on"), self.show_info, crate::system::perf::t(0.15));
        let info_col = info.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, info_on);
        icons::info(&painter, info_c, 17.0 * info.scale(), info_col);
        // puntito: actualizando (late) o lista para reiniciar
        let pending_restart = matches!(self.updater.last, Some(Outcome::Installed { .. }));
        let badge = ctx.animate_bool_with_time(
            Id::unique("capyro-info-badge"),
            self.updater.is_running() || pending_restart,
            crate::system::perf::t(0.2),
        );
        if badge > 0.001 {
            let beat = if self.updater.is_running() { 0.6 + 0.4 * (crate::system::perf::clock(ctx.input(|i| i.time)) * 5.0).sin().abs() as f32 } else { 1.0 };
            painter.circle_filled(info_c + vec2(0.0, 13.0), 2.0 * badge, accent.gamma_multiply(beat));
        }
        self.info_rect = Rect::from_center_size(info_c, Vec2::splat(30.0));
        if info.resp.clicked() {
            let open = !self.show_info;
            self.close_menus();
            self.show_info = open;
        }
        if !self.show_info {
            info.resp.on_hover_text(if self.updater.is_running() {
                "Acerca de Capyro · actualizando…"
            } else if pending_restart {
                "Acerca de Capyro · actualización lista"
            } else {
                "Acerca de Capyro"
            });
        }

        /* --- botón de novedades (al lado de ⓘ) --- */
        let ch_c = pos2(left + 43.0, cy);
        let ch = Self::icon_button(ui, "changes", ch_c, 30.0);
        let ch_on = ctx.animate_bool_with_time(Id::unique("capyro-changes-on"), self.show_changes, crate::system::perf::t(0.15));
        let ch_col = ch.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, ch_on);
        icons::news(&painter, ch_c, 17.0 * ch.scale(), ch_col);
        self.changes_rect = Rect::from_center_size(ch_c, Vec2::splat(30.0));
        // puntito mientras no se hayan visto las novedades de esta versión
        let unseen = self.changes_seen != ffi::version();
        let dot = ctx.animate_bool_with_time(Id::unique("capyro-changes-dot"), unseen, crate::system::perf::t(0.3));
        if dot > 0.001 {
            painter.circle_filled(ch_c + vec2(0.0, 13.0), 2.0 * dot, accent);
        }
        if ch.resp.clicked() {
            let open = !self.show_changes;
            self.close_menus();
            self.show_changes = open;
            if open && unseen {
                self.changes_seen = ffi::version();
                self.save_session();
            }
        }
        if !self.show_changes {
            ch.resp.on_hover_text(if unseen { "Novedades de esta versión · ¡hay cosas nuevas!" } else { "Novedades de esta versión" });
        }

        /* --- Capyro Studio y Capyro Converter (al lado de ⓘ y novedades; si
         * la ventana es angosta y chocan con los controles, en la fila de
         * abajo, al lado de "Lista") --- */
        let by = video_bottom + BOTTOM_ROW_Y;
        let controls_left = cx - 105.0 - 18.0;
        let (studio_c, conv_c) = if left + 128.0 < controls_left {
            (pos2(left + 77.0, cy), pos2(left + 111.0, cy))
        } else {
            (pos2(left + 92.0, by), pos2(left + 126.0, by))
        };
        self.launcher_icons(ui, &painter, studio_c, conv_c);

        /* --- fila inferior --- */

        let list_rect = Rect::from_min_max(pos2(left - 6.0, by - 14.0), pos2(left + 66.0, by + 14.0));
        self.ui_rects.insert("list", list_rect);
        let list = animate_button(ui, Id::unique("capyro-list"), list_rect, true);
        let list_on = ctx.animate_bool_with_time(Id::unique("capyro-list-on"), self.show_playlist, crate::system::perf::t(0.15));
        let list_col = list.tint(ICON_DIM, TEXT).lerp_to_gamma(accent, list_on);
        let bg = (10.0 * list.hover + 10.0 * list.press) as u8;
        if bg > 0 {
            painter.rect_filled(list_rect, 7.0, Color32::from_white_alpha(bg));
        }
        icons::playlist(&painter, pos2(left + 7.0, by), 14.0 * list.scale(), list_col);
        painter.text(pos2(left + 24.0, by), Align2::LEFT_CENTER, "Lista", regular(13.0), list_col);
        if list.resp.clicked() {
            self.show_playlist = !self.show_playlist;
        }

        // volumen: clic = silenciar, rueda = ajustar
        let vol_c = pos2(right - 92.0, by);
        self.ui_rects.insert("volume", Rect::from_center_size(vol_c, Vec2::splat(28.0)));
        let vol = Self::icon_button(ui, "volume", vol_c, 26.0);
        let (volume, muted) = (self.core.volume(), self.core.muted());
        icons::volume(&painter, vol_c, 16.0 * vol.scale(), vol.tint(ICON_DIM, Color32::WHITE), volume, muted);
        if vol.resp.clicked() {
            self.toggle_mute(&ctx);
        }
        // Rueda: un paso por "muesca". Antes se usaba el scroll suavizado, que
        // reparte una muesca en muchos frames y cada frame bajaba un paso
        // entero: una sola muesca se comía casi todo el volumen.
        // (Solo en la primera pasada del frame: egui a veces repite la pasada
        // con la misma entrada, p. ej. al mostrar un tooltip, y se contaba doble.)
        self.wheel_volume(&ctx, vol.resp.hovered());
        if vol.hover > 0.001 {
            // porcentaje en una pastillita arriba del ícono
            let label = if muted {
                "Silenciado".to_owned()
            } else {
                format!("{}%", (volume * 100.0).round())
            };
            let mut p = painter.clone();
            p.multiply_opacity(vol.hover);
            let g = p.layout_no_wrap(label, regular(11.0), TEXT);
            let r = Rect::from_center_size(vol_c - vec2(0.0, 24.0 + 3.0 * vol.hover), g.size() + vec2(12.0, 6.0));
            p.rect_filled(r, r.height() / 2.0, BG_PANEL);
            p.galley(r.center() - g.size() / 2.0, g, TEXT);
        }

        // efectos de audio: a la izquierda del volumen
        let fx_c = pos2(right - 120.0, by);
        let fxb = Self::icon_button(ui, "effects", fx_c, 26.0);
        let fx_on = ctx.animate_bool_with_time(Id::unique("capyro-fx-on"), self.show_fx, crate::system::perf::t(0.15));
        let fx_active = ctx.animate_bool_with_time(Id::unique("capyro-fx-active"), self.fx_active(), crate::system::perf::t(0.15));
        let fx_col = fxb
            .tint(ICON_DIM, Color32::WHITE)
            .lerp_to_gamma(accent, fx_on.max(fx_active));
        icons::sliders(&painter, fx_c, 16.0 * fxb.scale(), fx_col);
        if fx_active > 0.001 {
            painter.circle_filled(fx_c + vec2(0.0, 12.0), 2.0 * fx_active, accent);
        }
        self.fx_rect = Rect::from_center_size(fx_c, Vec2::splat(26.0));
        if fxb.resp.clicked() {
            let open = !self.show_fx;
            self.close_menus();
            self.show_fx = open;
        }
        if !self.show_fx {
            fxb.resp.on_hover_text("Efectos de audio (Ctrl+E)");
        }
        vol.resp.on_hover_text("Clic: silenciar · Rueda: volumen");

        let subs_c = pos2(right - 64.0, by);
        self.ui_rects.insert("subs", Rect::from_center_size(subs_c, Vec2::splat(28.0)));
        let subs = Self::icon_button(ui, "subtitles", subs_c, 26.0);
        icons::subtitles(&painter, subs_c, 16.0 * subs.scale(), subs.tint(ICON_DIM, Color32::WHITE));
        if subs.resp.clicked() {
            self.play_error();
            self.toast(&ctx, "Los subtítulos todavía no están disponibles", true);
        }
        subs.resp.on_hover_text("Subtítulos — próximamente");

        // engranaje: abre el menú de ajustes (y gira un poco al pasar el mouse)
        let gear_c = pos2(right - 36.0, by);
        let gear = Self::icon_button(ui, "settings", gear_c, 26.0);
        let gear_on = ctx.animate_bool_with_time(Id::unique("capyro-gear-on"), self.show_settings, crate::system::perf::t(0.15));
        let gear_col = gear.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, gear_on);
        icons::gear(&painter, gear_c, 16.0 * gear.scale(), gear_col);
        self.gear_rect = Rect::from_center_size(gear_c, Vec2::splat(26.0));
        if gear.resp.clicked() {
            let open = !self.show_settings;
            self.close_menus();
            self.show_settings = open;
        }
        if !self.show_settings {
            gear.resp.on_hover_text("Ajustes");
        }

        let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let fs_c = pos2(right - 8.0, by);
        self.ui_rects.insert("fullscreen", Rect::from_center_size(fs_c, Vec2::splat(28.0)));
        let fsb = Self::icon_button(ui, "fullscreen", fs_c, 26.0);
        icons::fullscreen(&painter, fs_c, 14.0 * fsb.scale(), fsb.tint(ICON_DIM, Color32::WHITE), fs);
        if fsb.resp.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fs));
        }
    }

    /// Los íconos de Capyro Studio y Capyro Converter (vista clásica).
    pub(crate) fn launcher_icons(&mut self, ui: &mut Ui, painter: &egui::Painter, studio_c: Pos2, conv_c: Pos2) {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;

        let st = Self::icon_button(ui, "studio", studio_c, 30.0);
        let st_on = ctx.animate_bool_with_time(Id::unique("capyro-studio-on"), self.studio_open(), crate::system::perf::t(0.15));
        icons::sliders(painter, studio_c, 17.0 * st.scale(), st.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, st_on));
        self.ui_rects.insert("studio", Rect::from_center_size(studio_c, Vec2::splat(30.0)));
        if st.resp.clicked() {
            self.open_studio(&ctx);
        }
        st.resp.on_hover_text("Capyro Studio: etiquetas, portada y ecualizador avanzado del archivo que está sonando");

        let cv = Self::icon_button(ui, "converter", conv_c, 30.0);
        let cv_on = ctx.animate_bool_with_time(Id::unique("capyro-converter-on"), self.converter_open(), crate::system::perf::t(0.15));
        icons::convert(painter, conv_c, 17.0 * cv.scale(), cv.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, cv_on));
        self.ui_rects.insert("converter", Rect::from_center_size(conv_c, Vec2::splat(30.0)));
        // convirtiendo (con el panel cerrado): un anillo que se llena
        if let Some(p) = self.converter_progress() {
            let r = 13.0;
            painter.circle_stroke(conv_c, r, Stroke::new(1.5, Color32::from_white_alpha(20)));
            let a0 = -std::f32::consts::FRAC_PI_2;
            let pts: Vec<Pos2> = (0..=32)
                .map(|i| {
                    let a = a0 + std::f32::consts::TAU * p * i as f32 / 32.0;
                    conv_c + vec2(a.cos(), a.sin()) * r
                })
                .collect();
            painter.add(egui::Shape::line(pts, Stroke::new(1.8, accent)));
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        if cv.resp.clicked() {
            self.open_converter(&ctx);
        }
        cv.resp.on_hover_text(match self.converter_progress() {
            Some(p) => format!("Capyro Converter · convirtiendo… {:.0} %", p * 100.0),
            None => "Capyro Converter: pasar video a audio, audio a video, y más".to_owned(),
        });
    }
}

//! La portada: el cuadro grande (tocarlo abre el selector de imágenes),
//! "Cambiar", "Quitar" y "Volver a la original".

use super::*;

impl Studio {
    /// Abre el selector de imágenes del sistema en otro hilo (no congela la ventana).
    pub(super) fn pick_cover(&mut self, ctx: &Context) {
        if self.picking.is_some() || !self.cover_ok || self.saving.is_some() {
            return;
        }
        let exts: Vec<String> =
            ["jpg", "jpeg", "png", "webp", "bmp"].iter().flat_map(|e| [e.to_string(), e.to_uppercase()]).collect();
        let pending = rfd::AsyncFileDialog::new()
            .set_title("Elegí la portada")
            .add_filter("Imágenes (jpg, png, webp, bmp)", &exts)
            .pick_file();
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let picked = match pollster::block_on(pending) {
                None => Picked::Cancelled,
                Some(f) => {
                    let path = f.path().to_path_buf();
                    // se decodifica acá, así una foto grande no frena la ventana
                    match ffi::read_image(&path, COVER_SHOW_SIDE) {
                        Ok(img) => Picked::Image(path, img),
                        Err(e) => Picked::Failed(e.0),
                    }
                }
            };
            let _ = tx.send(picked);
            ctx.request_repaint();
        });
        self.picking = Some(rx);
    }

    pub(super) fn poll_pick(&mut self, ctx: &Context, now: f64) {
        let Some(rx) = &self.picking else { return };
        let picked = match rx.try_recv() {
            Ok(p) => p,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Picked::Failed("se cerró el selector".to_owned()),
        };
        self.picking = None;
        match picked {
            Picked::Image(path, img) => {
                self.new_cover = Some(Picture::new(ctx, "capyro-studio-new-cover", &img));
                self.cover_edit = CoverEdit::Set(path);
                self.msg = None;
            }
            Picked::Failed(e) => self.say(now, format!("No pude abrir esa imagen: {e}"), true),
            Picked::Cancelled => {}
        }
    }

    /// La portada: al tocarla se abre el selector de imágenes.
    pub(super) fn cover_widget(&mut self, ui: &mut Ui, ctx: &Context, rect: Rect, accent: Color32, now: f64) {
        let painter = ui.painter().clone();
        let anim = crate::system::perf::animations();
        let alto = crate::ui::alto::on();
        let radius = (rect.width() * 0.07).clamp(8.0, 16.0);
        let a = animate_button(
            ui,
            Id::unique("capyro-studio-cover"),
            rect,
            self.cover_ok && self.picking.is_none() && self.saving.is_none(),
        );
        let shown = Rect::from_center_size(rect.center(), rect.size() * (1.0 + 0.015 * a.hover - 0.03 * a.press));

        if let Some(p) = self.shown_cover() {
            // Alto: la portada ilumina un poco alrededor con su color
            if alto {
                crate::ui::alto::glow(&painter, shown.center(), shown.width() * 0.95, p.tint, 0.32 + 0.1 * a.hover);
            }
            if anim {
                soft_shadow(&painter, shown, radius, 18.0, 6.0, 110);
            }
            // la imagen entera adentro del cuadrado (hay portadas que no son cuadradas)
            let size = if p.aspect >= 1.0 {
                vec2(shown.width(), shown.width() / p.aspect)
            } else {
                vec2(shown.height() * p.aspect, shown.height())
            };
            if (p.aspect - 1.0).abs() > 0.02 {
                painter.rect_filled(shown, radius, BG_VIDEO);
            }
            let img = Rect::from_center_size(shown.center(), size);
            painter.add(
                RectShape::filled(img, radius, Color32::WHITE)
                    .with_texture(p.tex.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))),
            );
        } else {
            if anim {
                soft_shadow(&painter, shown, radius, 14.0, 4.0, 80);
            }
            painter.rect_filled(shown, radius, BG_PANEL);
            painter.rect_filled(shown, radius, accent.gamma_multiply(0.10));
            painter.rect_stroke(shown, radius, Stroke::new(1.0, accent.gamma_multiply(0.25)), StrokeKind::Inside);
            painter.circle_filled(shown.center() - vec2(0.0, 10.0), shown.width() * 0.17, accent.gamma_multiply(0.16));
            if self.video {
                icons::film(&painter, shown.center() - vec2(0.0, 10.0), shown.width() * 0.2, accent);
            } else {
                icons::music_note(&painter, shown.center() - vec2(0.0, 10.0), shown.width() * 0.18, accent);
            }
            let text = if self.cover_ok { "Sin portada" } else { "Este formato no guarda portada" };
            painter.text(
                pos2(shown.center().x, shown.bottom() - 22.0),
                Align2::CENTER_CENTER,
                fit_text(&painter, text, regular(11.0), shown.width() - 16.0),
                regular(11.0),
                TEXT_FAINT,
            );
        }

        // al pasar el mouse: velo y "Cambiar portada"; mientras se elige, un spinner
        let veil = if self.picking.is_some() { 1.0 } else { a.hover };
        if veil > 0.001 {
            painter.rect_filled(shown, radius, Color32::from_black_alpha((150.0 * veil) as u8));
            let c = shown.center();
            let col = Color32::WHITE.gamma_multiply(veil);
            if self.picking.is_some() {
                icons::spinner(&painter, c - vec2(0.0, 10.0), 22.0, col, crate::system::perf::clock(now));
                painter.text(c + vec2(0.0, 18.0), Align2::CENTER_CENTER, "Eligiendo…", regular(12.0), col);
                if anim {
                    ctx.request_repaint_after(FRAME_SMOOTH);
                }
            } else {
                icons::camera(&painter, c - vec2(0.0, 10.0), 22.0, col);
                painter.text(c + vec2(0.0, 18.0), Align2::CENTER_CENTER, "Cambiar portada", bold(12.0), col);
            }
        }
        if a.resp.clicked() {
            self.pick_cover(ctx);
        }
        if self.cover_ok {
            a.resp.on_hover_text("Elegí una imagen (jpg, png, webp, bmp). Se guarda al tocar Guardar");
        }

        // abajo: cambiar / quitar / volver a la original
        if !self.cover_ok {
            return;
        }
        let y = rect.bottom() + 18.0;
        let cx = rect.center().x;
        let pending = self.cover_edit != CoverEdit::Keep;
        if pending {
            if link(ui, "cover-revert", pos2(cx, y), Align2::CENTER_CENTER, "Volver a la original", TEXT_DIM, true).0 {
                self.cover_edit = CoverEdit::Keep;
                self.new_cover = None;
            }
        } else {
            let has = self.cover.is_some();
            let (change, _) = link(ui, "cover-change", pos2(cx - 4.0, y), Align2::RIGHT_CENTER, "Cambiar", TEXT_DIM, self.picking.is_none());
            if change {
                self.pick_cover(ctx);
            }
            let (remove, _) = link(ui, "cover-remove", pos2(cx + 4.0, y), Align2::LEFT_CENTER, "Quitar", TEXT_DIM, has);
            if remove {
                self.cover_edit = CoverEdit::Remove;
            }
        }
    }
}

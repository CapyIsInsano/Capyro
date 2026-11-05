//! Dónde se guarda (nombre y carpeta), cuánto va a pesar más o menos, la
//! tarjeta con el resultado y el arranque de la conversión en otro hilo.

use super::*;

impl Converter {
    pub(super) fn output_section(&mut self, ui: &mut Ui, ctx: &Context, accent: Color32) {
        let (Some(input), Some(plan), Some(probe)) = (self.input.clone(), self.plan.clone(), self.probe.clone()) else { return };
        let est = plan.estimate(&probe).map(|b| format!("≈ {}", fmt_size(b))).unwrap_or_default();
        option_label(ui, "Guardar como", &est, accent);
        let ext = format!(".{}", plan.container.ext());
        let w = ui.available_width();
        let ext_w = ui.painter().layout_no_wrap(ext.clone(), bold(13.0), TEXT).size().x + 12.0;
        let (row, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
        let field_r = Rect::from_min_max(row.min, pos2(row.right() - ext_w, row.bottom()));
        let mut stem = self.out_stem.clone();
        big_field(ui, field_r, "conv-out-name", &mut stem, "Nombre del archivo", regular(13.5), TEXT, accent);
        // sin barras ni caracteres que Windows no acepta
        let clean: String = stem.chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')).collect();
        if clean != self.out_stem {
            self.out_stem = clean;
            self.finished = None;
        }
        ui.painter().text(pos2(row.right(), row.center().y), Align2::RIGHT_CENTER, &ext, bold(13.0), TEXT_DIM);

        let dir = self.out_dir.clone().or_else(|| input.parent().map(PathBuf::from)).unwrap_or_default();
        let (row, _) = ui.allocate_exact_size(vec2(w, 28.0), Sense::hover());
        let p = ui.painter().clone();
        let shown = crate::system::paths::tilde(&dir);
        let (change, cr) = link(ui, "conv-folder", pos2(row.right() + 7.0, row.center().y), Align2::RIGHT_CENTER, "Cambiar carpeta", TEXT_DIM, self.picking.is_none());
        let mut right = cr.left();
        if self.out_dir.is_some() {
            let (back, br) = link(ui, "conv-folder-back", pos2(cr.left(), row.center().y), Align2::RIGHT_CENTER, "La del archivo", TEXT_DIM, true);
            if back {
                self.out_dir = None;
            }
            right = br.left();
        }
        let text = format!("En {shown}");
        p.text(row.left_center(), Align2::LEFT_CENTER, fit_text(&p, &text, regular(11.0), right - row.left() - 8.0), regular(11.0), TEXT_FAINT);
        if change {
            self.pick_folder(ctx);
        }
        // si ya existe (o es el mismo archivo), el nombre que va a tener de verdad
        let real = self.output_path(&input, &plan);
        let typed = dir.join(format!("{}.{}", self.out_stem.trim(), plan.container.ext()));
        if real != typed {
            let (row, _) = ui.allocate_exact_size(vec2(w, 18.0), Sense::hover());
            let name = real.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let text = format!("Se guarda como «{name}» (no se pisa nada)");
            ui.painter().text(row.left_center(), Align2::LEFT_CENTER, fit_text(ui.painter(), &text, regular(10.5), w), regular(10.5), TEXT_FAINT);
        }
    }

    pub(super) fn result_section(&mut self, ui: &mut Ui, accent: Color32) {
        let Some(fin) = &self.finished else { return };
        let w = ui.available_width();
        match fin {
            Finished::Ok { output, size, secs, done } => {
                let (r, _) = ui.allocate_exact_size(vec2(w, 74.0), Sense::hover());
                let p = ui.painter().clone();
                p.rect_filled(r, 12.0, accent.gamma_multiply(0.12));
                p.rect_stroke(r, 12.0, Stroke::new(1.0, accent.gamma_multiply(0.45)), StrokeKind::Inside);
                icons::check(&p, pos2(r.left() + 22.0, r.top() + 22.0), 14.0, accent, 2.0);
                let name = output.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                p.text(pos2(r.left() + 40.0, r.top() + 22.0), Align2::LEFT_CENTER, fit_text(&p, &name, bold(13.0), w - 60.0), bold(13.0), TEXT);
                let encs: Vec<&str> = [done.video_encoder.as_str(), done.audio_encoder.as_str()].into_iter().filter(|s| !s.is_empty()).collect();
                let detail = format!("{} · en {} · {}", fmt_size(*size), fmt_secs(*secs), encs.join(" + "));
                p.text(pos2(r.left() + 40.0, r.top() + 40.0), Align2::LEFT_CENTER, fit_text(&p, &detail, regular(11.0), w - 60.0), regular(11.0), TEXT_DIM);
                let y = r.bottom() - 16.0;
                let (play, pr) = link(ui, "conv-res-play", pos2(r.left() + 33.0, y), Align2::LEFT_CENTER, "Reproducir", accent, true);
                let (add, ar) = link(ui, "conv-res-add", pos2(pr.right() + 4.0, y), Align2::LEFT_CENTER, "Agregar a la lista", TEXT_DIM, true);
                let (folder, _) = link(ui, "conv-res-folder", pos2(ar.right() + 4.0, y), Align2::LEFT_CENTER, "Abrir carpeta", TEXT_DIM, true);
                if play {
                    self.requests.push(Request::Play(output.clone()));
                }
                if add {
                    self.requests.push(Request::Enqueue(output.clone()));
                }
                if folder {
                    if let Some(d) = output.parent() {
                        self.requests.push(Request::OpenFolder(d.to_path_buf()));
                    }
                }
            }
            Finished::Failed(e) => {
                let (r, _) = ui.allocate_exact_size(vec2(w, 46.0), Sense::hover());
                let p = ui.painter();
                p.rect_filled(r, 12.0, ERROR.gamma_multiply(0.12));
                p.text(pos2(r.left() + 14.0, r.top() + 15.0), Align2::LEFT_CENTER, "No se pudo convertir", bold(12.5), ERROR);
                p.text(pos2(r.left() + 14.0, r.top() + 32.0), Align2::LEFT_CENTER, fit_text(p, e, regular(11.0), w - 28.0), regular(11.0), TEXT_DIM);
            }
            Finished::Cancelled => {}
        }
    }

    /// Dónde se va a guardar: la carpeta elegida (o la del archivo), el
    /// nombre escrito y, si ya existe, "(2)", "(3)"… Nunca el mismo archivo.
    fn output_path(&self, input: &std::path::Path, plan: &Plan) -> PathBuf {
        let dir = self.out_dir.clone().or_else(|| input.parent().map(PathBuf::from)).unwrap_or_default();
        let mut stem = self.out_stem.trim().to_owned();
        if stem.is_empty() {
            stem = "convertido".to_owned();
        }
        let ext = plan.container.ext();
        if dir.join(format!("{stem}.{ext}")) == input {
            stem.push_str(" (convertido)");
        }
        profiles::unique_path(&dir, &stem, ext, &|p| p.exists())
    }

    /// Arranca la conversión en otro hilo.
    pub(super) fn start(&mut self, ctx: &Context, now: f64, accent: Color32) {
        if self.job.is_some() {
            return;
        }
        let (Some(input), Some(plan), Some(probe)) = (self.input.clone(), self.plan.clone(), self.probe.clone()) else { return };
        let output = self.output_path(&input, &plan);
        let mut opts = plan.options();
        let mut tmp_cover = None;
        if plan.cover_video(&probe) {
            if let Some(img) = &self.cover_custom {
                opts.cover_image = Some(img.clone());
            } else if !probe.has_cover {
                // sin portada: el logo de Capyro sobre un degradé del color de acento
                tmp_cover = cover_card(accent);
                opts.cover_image = tmp_cover.clone();
            }
        }
        let progress = Arc::new(ConvertProgress::default());
        let (tx, rx) = mpsc::channel();
        let shared = progress.clone();
        let ctx2 = ctx.clone();
        let (inp, out) = (input.clone(), output.clone());
        std::thread::spawn(move || {
            let r = ffi::convert(&inp, &out, &opts, &shared);
            let _ = tx.send(r);
            ctx2.request_repaint();
        });
        self.finished = None;
        self.job = Some(Job { progress, rx, output, started: now, tmp_cover });
    }
}

/// Imagen cuadrada con el logo de Capyro sobre un degradé del acento (para
/// hacer un video de un audio que no tiene portada). Va a la carpeta temporal.
fn cover_card(accent: Color32) -> Option<PathBuf> {
    let logo = image::load_from_memory(super::super::ICON_PNG).ok()?.to_rgba8();
    let side = 1024u32;
    let mut img = image::RgbaImage::new(side, side);
    let (ar, ag, ab) = (accent.r() as f32, accent.g() as f32, accent.b() as f32);
    for (x, y, px) in img.enumerate_pixels_mut() {
        // del acento oscuro (arriba a la izquierda) a casi negro
        let t = ((x + y) as f32 / (2 * side) as f32).clamp(0.0, 1.0);
        let k = 0.55 * (1.0 - t) + 0.08;
        let d = (((x as f32 - 512.0).powi(2) + (y as f32 - 470.0).powi(2)).sqrt() / 724.0).min(1.0);
        let glow = 0.25 * (1.0 - d);
        let mix = |c: f32, base: f32| (base + (c - base) * (k + glow)).clamp(0.0, 255.0) as u8;
        *px = image::Rgba([mix(ar, 18.0), mix(ag, 16.0), mix(ab, 20.0), 255]);
    }
    let big = image::imageops::resize(&logo, 460, 460, image::imageops::FilterType::Lanczos3);
    image::imageops::overlay(&mut img, &big, ((side - 460) / 2) as i64, ((side - 460) / 2 - 20) as i64);
    // en una carpeta con el número del proceso ("Borrar caché" no la toca mientras se usa)
    let dir = std::env::temp_dir().join(format!("capyro-converter-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("portada.png");
    img.save(&path).ok()?;
    Some(path)
}

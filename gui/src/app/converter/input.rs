//! El archivo que se convierte: una tarjeta con lo que tiene (o un lugar
//! grande para elegirlo) y los selectores del sistema (archivo, imagen,
//! carpeta), que corren en otro hilo para no congelar la ventana.

use super::*;

/// Extensiones que se pueden elegir (las que abre Capyro).
const INPUT_EXTS: [&str; 22] = [
    "mp4", "mkv", "webm", "mov", "avi", "m4v", "wmv", "flv", "ts", "mts", "3gp", "mpg", "mp3", "m4a", "aac", "flac", "ogg",
    "opus", "wav", "wma", "aiff", "ape",
];

impl Converter {
    pub(super) fn input_section(&mut self, ui: &mut Ui, ctx: &Context, accent: Color32) {
        let w = ui.available_width();
        let Some(path) = self.input.clone() else {
            // sin archivo: un lugar grande para elegirlo
            let (r, _) = ui.allocate_exact_size(vec2(w, 120.0), Sense::hover());
            let a = animate_button(ui, Id::unique("capyro-conv-pick-big"), r, self.picking.is_none());
            let p = ui.painter();
            p.rect_filled(r, 12.0, Color32::from_white_alpha((5.0 + 6.0 * a.hover) as u8));
            p.rect_stroke(r, 12.0, Stroke::new(1.0, accent.gamma_multiply(0.35 + 0.3 * a.hover)), StrokeKind::Inside);
            p.circle_filled(r.center() - vec2(0.0, 18.0), 20.0, accent.gamma_multiply(0.16));
            icons::convert(p, r.center() - vec2(0.0, 18.0), 20.0, accent);
            p.text(r.center() + vec2(0.0, 18.0), Align2::CENTER_CENTER, "Elegí un archivo para convertir", bold(13.0), TEXT);
            p.text(r.center() + vec2(0.0, 36.0), Align2::CENTER_CENTER, "o arrastralo a la ventana", regular(11.0), TEXT_FAINT);
            if a.resp.clicked() {
                self.pick_input(ctx);
            }
            return;
        };

        let (r, _) = ui.allocate_exact_size(vec2(w, 64.0), Sense::hover());
        let p = ui.painter().clone();
        p.rect_filled(r, 12.0, Color32::from_white_alpha(7));
        let ic = pos2(r.left() + 32.0, r.center().y);
        p.circle_filled(ic, 20.0, accent.gamma_multiply(0.16));
        let video = self.probe.as_ref().is_some_and(|pr| pr.has_video);
        if video {
            icons::film(&p, ic, 18.0, accent);
        } else {
            icons::music_note(&p, ic, 18.0, accent);
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let tx = r.left() + 62.0;
        let (change, change_r) = link(ui, "conv-change", pos2(r.right() - 8.0, r.center().y), Align2::RIGHT_CENTER, "Cambiar archivo", TEXT_DIM, self.picking.is_none());
        let max_w = change_r.left() - tx - 10.0;
        p.text(pos2(tx, r.center().y - 9.0), Align2::LEFT_CENTER, fit_text(&p, &name, bold(13.5), max_w), bold(13.5), TEXT);
        let (detail, col) = match (&self.probe, &self.probe_err) {
            (Some(pr), _) => (describe(pr, &path), TEXT_FAINT),
            (None, Some(e)) => (e.clone(), ERROR),
            _ => (String::new(), TEXT_FAINT),
        };
        p.text(pos2(tx, r.center().y + 10.0), Align2::LEFT_CENTER, fit_text(&p, &detail, regular(11.0), max_w), regular(11.0), col);
        if change {
            self.pick_input(ctx);
        }
    }

    fn pick(&mut self, ctx: &Context, dialog: rfd::AsyncFileDialog, kind: u8) {
        if self.picking.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let picked = match kind {
                2 => pollster::block_on(dialog.pick_folder()).map_or(Picked::Cancelled, |f| Picked::Folder(f.path().to_path_buf())),
                1 => pollster::block_on(dialog.pick_file()).map_or(Picked::Cancelled, |f| Picked::Image(f.path().to_path_buf())),
                _ => pollster::block_on(dialog.pick_file()).map_or(Picked::Cancelled, |f| Picked::Input(f.path().to_path_buf())),
            };
            let _ = tx.send(picked);
            ctx.request_repaint();
        });
        self.picking = Some(rx);
    }

    pub(super) fn pick_input(&mut self, ctx: &Context) {
        let exts: Vec<String> = INPUT_EXTS.iter().flat_map(|e| [e.to_string(), e.to_uppercase()]).collect();
        let mut d = rfd::AsyncFileDialog::new().set_title("Elegí qué convertir").add_filter("Audio y video", &exts);
        if let Some(dir) = self.input.as_ref().and_then(|p| p.parent()) {
            d = d.set_directory(dir);
        }
        self.pick(ctx, d, 0);
    }

    pub(super) fn pick_image(&mut self, ctx: &Context) {
        let exts: Vec<String> = ["jpg", "jpeg", "png", "webp", "bmp"].iter().flat_map(|e| [e.to_string(), e.to_uppercase()]).collect();
        let d = rfd::AsyncFileDialog::new().set_title("Elegí la imagen del video").add_filter("Imágenes (jpg, png, webp, bmp)", &exts);
        self.pick(ctx, d, 1);
    }

    pub(super) fn pick_folder(&mut self, ctx: &Context) {
        let mut d = rfd::AsyncFileDialog::new().set_title("Elegí dónde guardar");
        if let Some(dir) = self.out_dir.clone().or_else(|| self.input.as_ref().and_then(|p| p.parent().map(PathBuf::from))) {
            d = d.set_directory(dir);
        }
        self.pick(ctx, d, 2);
    }

    pub(super) fn poll_pick(&mut self, _now: f64) {
        let Some(rx) = &self.picking else { return };
        let picked = match rx.try_recv() {
            Ok(p) => p,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Picked::Cancelled,
        };
        self.picking = None;
        match picked {
            Picked::Input(p) => {
                self.set_input(p);
                self.fresh = true;
            }
            Picked::Image(p) => {
                self.cover_custom = Some(p);
                self.finished = None;
            }
            Picked::Folder(p) => self.out_dir = Some(p),
            Picked::Cancelled => {}
        }
    }
}

/// "Video · 1920×1080 · 30 fps · H.264 + AAC · 0:40 · 27 MB"
fn describe(p: &MediaProbe, path: &std::path::Path) -> String {
    let mut parts = Vec::new();
    let codec = |c: &str| match c {
        "h264" => "H.264".to_owned(),
        "hevc" => "H.265".to_owned(),
        "mp3" | "mp3float" => "MP3".to_owned(),
        c if c.starts_with("pcm_") => "PCM".to_owned(),
        c => c.to_uppercase(),
    };
    if p.has_video {
        parts.push("Video".to_owned());
        parts.push(format!("{}×{}", p.width, p.height));
        if p.fps > 0.0 {
            parts.push(format!("{:.0} fps", p.fps));
        }
        let mut c = codec(&p.video_codec);
        if p.has_audio {
            c += &format!(" + {}", codec(&p.audio_codec));
        }
        parts.push(c);
    } else if p.has_audio {
        parts.push("Audio".to_owned());
        parts.push(codec(&p.audio_codec));
        if p.sample_rate > 0 {
            let k = p.sample_rate as f64 / 1000.0;
            parts.push(if (k - k.round()).abs() < 0.05 { format!("{k:.0} kHz") } else { format!("{k:.1} kHz").replace('.', ",") });
        }
        parts.push(match p.channels {
            1 => "Mono".to_owned(),
            2 => "Estéreo".to_owned(),
            n => format!("{n} canales"),
        });
        if p.has_cover {
            parts.push("con portada".to_owned());
        }
    }
    if p.duration > 0.0 {
        parts.push(fmt_clock(p.duration));
    }
    if let Ok(m) = std::fs::metadata(path) {
        parts.push(fmt_size(m.len()));
    }
    parts.join(" · ")
}

/// "3:20", "1:02:03".
pub(super) fn fmt_clock(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

//! Los perfiles (tarjetas) y las funciones avanzadas: formato, códecs,
//! calidad, resolución, cuadros por segundo, velocidad, GPU, kbps,
//! frecuencia, canales, volumen, recorte y etiquetas. Tocar cualquier cosa
//! avanzada pasa a "Personalizado".

use super::profiles::{ACodec, Container, Track, VCodec};
use super::*;

const RESOLUTIONS: [i32; 6] = [2160, 1440, 1080, 720, 480, 360];
const FPS: [i32; 3] = [60, 30, 24];
const KBPS: [i32; 6] = [96, 128, 160, 192, 256, 320];

impl Converter {
    pub(super) fn profiles_section(&mut self, ui: &mut Ui, accent: Color32) {
        let Some(probe) = self.probe.clone() else { return };
        option_label(ui, "Perfiles", if self.profile.is_none() { "Personalizado" } else { "" }, accent);
        ui.add_space(4.0);
        let w = ui.available_width();
        let gap = 8.0;
        let cols = if w > 560.0 { 3 } else { 2 };
        let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
        let ch = 52.0;
        let rows = Profile::ALL.len().div_ceil(cols);
        let (area, _) = ui.allocate_exact_size(vec2(w, rows as f32 * (ch + gap) - gap), Sense::hover());
        let painter = ui.painter().clone();
        for (i, pr) in Profile::ALL.into_iter().enumerate() {
            let (row, col) = (i / cols, i % cols);
            let r = Rect::from_min_size(area.min + vec2(col as f32 * (cw + gap), row as f32 * (ch + gap)), vec2(cw, ch));
            let why = pr.unavailable(&probe);
            let enabled = why.is_none() && ui.is_enabled();
            let a = animate_button(ui, Id::unique(("capyro-conv-profile", i)), r, enabled);
            let selected = self.profile == Some(pr);
            let sel = ui.ctx().animate_bool_with_time(Id::unique(("capyro-conv-profile-sel", i)), selected, crate::system::perf::t(0.15));
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.02 * a.press));
            if crate::ui::alto::on() && enabled {
                crate::ui::alto::glow_rect(&painter, rr, accent, 0.35 * a.hover + 0.25 * sel);
            }
            let bg = Color32::from_white_alpha(if why.is_some() { 3 } else { (7.0 + 8.0 * a.hover) as u8 });
            painter.rect_filled(rr, 10.0, bg.lerp_to_gamma(accent.gamma_multiply(0.22), sel));
            painter.rect_stroke(rr, 10.0, Stroke::new(1.0, accent.gamma_multiply(0.15 + 0.6 * sel)), StrokeKind::Inside);
            let (tc, bc) = if why.is_some() { (TEXT_FAINT, TEXT_FAINT.gamma_multiply(0.7)) } else { (TEXT, TEXT_DIM) };
            painter.text(pos2(rr.left() + 12.0, rr.top() + 17.0), Align2::LEFT_CENTER, fit_text(&painter, pr.label(), bold(12.5), cw - 24.0), bold(12.5), tc);
            let blurb = why.unwrap_or_else(|| pr.blurb(&probe));
            painter.text(pos2(rr.left() + 12.0, rr.top() + 35.0), Align2::LEFT_CENTER, fit_text(&painter, blurb, regular(10.5), cw - 24.0), regular(10.5), bc);
            if a.resp.clicked() && !selected {
                self.apply_profile(pr);
            }
            a.resp.on_hover_text(pr.blurb(&probe));
        }
    }

    /// Elige un perfil (el recorte, la GPU y la imagen elegida se mantienen).
    fn apply_profile(&mut self, pr: Profile) {
        let Some(probe) = &self.probe else { return };
        let mut plan = pr.plan(probe, &has_encoder);
        if let Some(old) = &self.plan {
            plan.opts.start = old.opts.start;
            plan.opts.end = old.opts.end;
            plan.opts.gpu = old.opts.gpu;
        }
        self.plan = Some(plan);
        self.profile = Some(pr);
        self.finished = None;
    }

    pub(super) fn advanced_section(&mut self, ui: &mut Ui, ctx: &Context, accent: Color32) {
        // encabezado que despliega
        let w = ui.available_width();
        let (r, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
        let a = animate_button(ui, Id::unique("capyro-conv-advanced"), r, true);
        let k = ctx.animate_bool_with_time(Id::unique("capyro-conv-advanced-open"), self.advanced, crate::system::perf::t(0.18));
        let p = ui.painter().clone();
        if a.hover > 0.001 {
            p.rect_filled(r, 8.0, Color32::from_white_alpha((8.0 * a.hover) as u8));
        }
        icons::sliders(&p, pos2(r.left() + 12.0, r.center().y), 13.0, TEXT_DIM.lerp_to_gamma(accent, k));
        p.text(pos2(r.left() + 28.0, r.center().y), Align2::LEFT_CENTER, "Funciones avanzadas", bold(12.5), TEXT);
        let chev = pos2(r.right() - 12.0, r.center().y);
        let st = Stroke::new(1.5, TEXT_DIM);
        let (dy, sx) = (3.0 * (1.0 - 2.0 * k), 4.5);
        p.line_segment([chev + vec2(-sx, -dy), chev + vec2(0.0, dy)], st);
        p.line_segment([chev + vec2(sx, -dy), chev + vec2(0.0, dy)], st);
        if a.resp.clicked() {
            self.advanced = !self.advanced;
        }
        if !self.advanced {
            return;
        }
        let (Some(probe), Some(mut plan)) = (self.probe.clone(), self.plan.clone()) else { return };
        let before = plan.clone();
        ui.add_space(8.0);

        /* --- formato --- */
        option_label(ui, "Formato", "", accent);
        let all: Vec<Container> = Container::VIDEO.into_iter().chain(Container::AUDIO).collect();
        let items: Vec<Chip> = all
            .iter()
            .map(|c| Chip {
                label: c.label(),
                selected: plan.container == *c,
                enabled: c.is_video() || probe.has_audio,
                hint: Some(if c.is_video() { "Video" } else { "Solo audio" }),
            })
            .collect();
        if let Some(i) = chips(ui, "conv-container", &items, accent) {
            plan.container = all[i];
            plan.fit_container(&probe, &has_encoder);
        }

        /* --- video --- */
        if plan.container.is_video() {
            ui.add_space(10.0);
            let cover = !probe.has_video;
            option_label(ui, if cover { "Video (con la portada)" } else { "Video" }, "", accent);
            let codecs: Vec<VCodec> = VCodec::ALL.into_iter().filter(|v| plan.container.video_codecs().contains(v)).collect();
            // con "Usar la placa de video", también los que solo tiene la placa
            let usable = |v: &VCodec| has_encoder(v.id()) || (plan.opts.gpu && ffi::has_encoder(v.id(), true));
            let mut items: Vec<Chip> = codecs
                .iter()
                .map(|v| Chip {
                    label: v.label(),
                    selected: plan.video == Track::Encode(*v),
                    enabled: usable(v),
                    hint: Some(if usable(v) {
                        v.hint()
                    } else if ffi::has_encoder(v.id(), true) {
                        "Solo con la placa de video: activá \"Usar la placa de video\""
                    } else {
                        "Este FFmpeg no trae con qué codificarlo"
                    }),
                })
                .collect();
            let can_copy = probe.has_video && plan.container.can_copy(&probe.video_codec);
            items.push(Chip {
                label: "Copiar",
                selected: plan.video == Track::Copy,
                enabled: can_copy,
                hint: Some(if can_copy { "El video tal cual: rápido y sin perder calidad" } else { "Ese video no entra tal cual en este formato" }),
            });
            items.push(Chip { label: "Sin video", selected: plan.video == Track::None, enabled: probe.has_audio, hint: None });
            if let Some(i) = chips(ui, "conv-vcodec", &items, accent) {
                plan.video = if i < codecs.len() {
                    Track::Encode(codecs[i])
                } else if i == codecs.len() {
                    Track::Copy
                } else {
                    Track::None
                };
            }
            if let Track::Encode(v) = plan.video {
                if cover {
                    ui.add_space(8.0);
                    let label = match &self.cover_custom {
                        Some(p) => p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                        None if probe.has_cover => "la portada del archivo".to_owned(),
                        None => "el logo de Capyro (el archivo no tiene portada)".to_owned(),
                    };
                    option_label(ui, &format!("Imagen: {label}"), "", accent);
                    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
                    let (pick, pr) = link(ui, "conv-pick-image", row.left_center(), Align2::LEFT_CENTER, "Elegir otra imagen…", accent, self.picking.is_none());
                    if pick {
                        self.pick_image(ctx);
                    }
                    if self.cover_custom.is_some()
                        && link(ui, "conv-reset-image", pos2(pr.right() + 6.0, row.center().y), Align2::LEFT_CENTER, "Volver a la portada", TEXT_DIM, true).0
                    {
                        self.cover_custom = None;
                    }
                }
                ui.add_space(10.0);
                let q = plan.opts.quality;
                let word = match q {
                    0..=30 => "Baja",
                    31..=55 => "Media",
                    56..=80 => "Alta",
                    _ => "Muy alta",
                };
                option_label(ui, "Calidad", &format!("{word} ({q})"), accent);
                let mut v01 = q as f32 / 100.0;
                if slider(ui, "conv-quality", &mut v01, "Más chico", "Mejor calidad", accent) {
                    plan.opts.quality = (v01 * 100.0).round() as i32;
                }

                ui.add_space(6.0);
                option_label(ui, if cover { "Tamaño del video" } else { "Resolución (lado corto)" }, "", accent);
                // con portada: el tamaño del video (1080p si no se eligió); con
                // video: el original o uno más chico
                let res: Vec<i32> = if cover {
                    vec![720, 1080, 1440, 2160]
                } else {
                    let short = probe.width.min(probe.height) as i32;
                    std::iter::once(0).chain(RESOLUTIONS.into_iter().filter(|&r| r < short)).collect()
                };
                let labels: Vec<String> = res.iter().map(|&r| if r == 0 { "Original".to_owned() } else { format!("{r}p") }).collect();
                let cur = if cover && plan.opts.max_height == 0 { 1080 } else { plan.opts.max_height };
                let items: Vec<Chip> =
                    res.iter().zip(&labels).map(|(&r, l)| Chip { label: l, selected: cur == r, enabled: true, hint: None }).collect();
                if let Some(i) = chips(ui, "conv-res", &items, accent) {
                    plan.opts.max_height = res[i];
                }

                if !cover {
                    ui.add_space(10.0);
                    option_label(ui, "Cuadros por segundo", "", accent);
                    let src = probe.fps;
                    let fps_opts: Vec<i32> = std::iter::once(0).chain(FPS.into_iter().filter(|&f| (f as f64) < src - 0.5)).collect();
                    let labels: Vec<String> =
                        fps_opts.iter().map(|&f| if f == 0 { format!("Original ({:.0})", src) } else { f.to_string() }).collect();
                    let items: Vec<Chip> =
                        fps_opts.iter().zip(&labels).map(|(&f, l)| Chip { label: l, selected: plan.opts.fps == f, enabled: true, hint: None }).collect();
                    if let Some(i) = chips(ui, "conv-fps", &items, accent) {
                        plan.opts.fps = fps_opts[i];
                    }
                }

                ui.add_space(10.0);
                option_label(ui, "Velocidad", "", accent);
                let speeds = [("Rápida", "Termina antes; el archivo pesa un poco más"), ("Normal", "El equilibrio"), ("Lenta", "Tarda más; el archivo queda más chico con la misma calidad")];
                let items: Vec<Chip> = speeds
                    .iter()
                    .enumerate()
                    .map(|(i, (l, h))| Chip { label: l, selected: plan.opts.speed == i as i32, enabled: true, hint: Some(h) })
                    .collect();
                if let Some(i) = chips(ui, "conv-speed", &items, accent) {
                    plan.opts.speed = i as i32;
                }

                ui.add_space(6.0);
                if v == VCodec::H264
                    && switch_row(ui, "conv-compat", "Compatibilidad máxima", "H.264 High 4.1, 8 bits y hasta 60 fps: lo abren hasta los equipos viejos", plan.opts.compat, accent)
                {
                    plan.opts.compat = !plan.opts.compat;
                }
                if ffi::has_encoder(v.id(), true)
                    && switch_row(
                        ui,
                        "conv-gpu",
                        "Usar la placa de video",
                        "Codifica con la placa (NVIDIA, Intel o AMD) si la tiene: mucho más rápido. Si no se puede, sigue con el procesador",
                        plan.opts.gpu,
                        accent,
                    )
                {
                    plan.opts.gpu = !plan.opts.gpu;
                }
            }
        }

        /* --- audio --- */
        if probe.has_audio {
            ui.add_space(10.0);
            option_label(ui, "Audio", "", accent);
            let codecs: Vec<ACodec> = ACodec::ALL.into_iter().filter(|a| plan.container.audio_codecs().contains(a)).collect();
            let mut items: Vec<Chip> = codecs
                .iter()
                .map(|a| Chip {
                    label: a.label(),
                    selected: plan.audio == Track::Encode(*a),
                    enabled: has_encoder(a.id()),
                    hint: if has_encoder(a.id()) { None } else { Some("Este FFmpeg no trae con qué codificarlo") },
                })
                .collect();
            let can_copy = plan.container.can_copy(&probe.audio_codec);
            items.push(Chip {
                label: "Copiar",
                selected: plan.audio == Track::Copy,
                enabled: can_copy,
                hint: Some(if can_copy { "El audio tal cual: rápido y sin perder calidad" } else { "Ese audio no entra tal cual en este formato" }),
            });
            let video_out = plan.container.is_video() && plan.video != Track::None;
            items.push(Chip { label: "Sin audio", selected: plan.audio == Track::None, enabled: video_out, hint: None });
            if let Some(i) = chips(ui, "conv-acodec", &items, accent) {
                plan.audio = if i < codecs.len() {
                    Track::Encode(codecs[i])
                } else if i == codecs.len() {
                    Track::Copy
                } else {
                    Track::None
                };
            }
            if let Track::Encode(a) = plan.audio {
                if !a.lossless() {
                    ui.add_space(10.0);
                    option_label(ui, "Calidad del audio", "", accent);
                    let cur = plan.audio_kbps();
                    let labels: Vec<String> = KBPS.iter().map(|k| format!("{k} kbps")).collect();
                    let items: Vec<Chip> = KBPS
                        .iter()
                        .zip(&labels)
                        .map(|(&k, l)| Chip { label: l, selected: cur == k, enabled: !(a == ACodec::Opus && k > 256), hint: None })
                        .collect();
                    if let Some(i) = chips(ui, "conv-kbps", &items, accent) {
                        plan.opts.audio_kbps = KBPS[i];
                    }
                }
                ui.add_space(10.0);
                option_label(ui, "Frecuencia", "", accent);
                let rates = [(0, "Original"), (44_100, "44,1 kHz"), (48_000, "48 kHz")];
                let items: Vec<Chip> = rates
                    .iter()
                    .map(|(r, l)| Chip { label: l, selected: plan.opts.sample_rate == *r, enabled: !(a == ACodec::Opus && *r == 44_100), hint: if a == ACodec::Opus { Some("Opus siempre va a 48 kHz") } else { None } })
                    .collect();
                if let Some(i) = chips(ui, "conv-rate", &items, accent) {
                    plan.opts.sample_rate = rates[i].0;
                }
                ui.add_space(10.0);
                option_label(ui, "Canales", "", accent);
                let chans = [(0, "Original"), (2, "Estéreo"), (1, "Mono")];
                let items: Vec<Chip> = chans.iter().map(|(c, l)| Chip { label: l, selected: plan.opts.channels == *c, enabled: true, hint: None }).collect();
                if let Some(i) = chips(ui, "conv-ch", &items, accent) {
                    plan.opts.channels = chans[i].0;
                }
                ui.add_space(6.0);
                if switch_row(ui, "conv-norm", "Emparejar el volumen", "Lleva todo a un volumen parejo (EBU R128), como las plataformas de música", plan.opts.normalize, accent) {
                    plan.opts.normalize = !plan.opts.normalize;
                }
            }
        }

        /* --- recorte --- */
        if probe.duration > 0.0 {
            ui.add_space(10.0);
            let start = plan.opts.start.max(0.0);
            let end = if plan.opts.end > 0.0 { plan.opts.end.min(probe.duration) } else { probe.duration };
            let whole = start <= 0.0 && plan.opts.end <= 0.0;
            let right = if whole {
                "Todo".to_owned()
            } else {
                format!("{} – {} ({})", input::fmt_clock(start), input::fmt_clock(end), input::fmt_clock(end - start))
            };
            option_label(ui, "Recortar", &right, accent);
            let (mut s, mut e) = (start, end);
            if range_bar(ui, "conv-trim", &mut s, &mut e, probe.duration, accent) {
                plan.opts.start = if s < 0.05 { 0.0 } else { s };
                plan.opts.end = if e > probe.duration - 0.05 { 0.0 } else { e };
            }
            let copying = plan.video == Track::Copy || plan.audio == Track::Copy;
            let hint = if copying && !whole {
                "Copiando tal cual, el recorte arranca en el cuadro clave más cercano (puede empezar un poco antes)"
            } else {
                "Arrastrá las manijas · doble clic: todo"
            };
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::hover());
            ui.painter().text(r.left_center(), Align2::LEFT_CENTER, hint, regular(10.5), TEXT_FAINT);
        }

        ui.add_space(6.0);
        if switch_row(ui, "conv-tags", "Conservar etiquetas y portada", "Título, artista, álbum, capítulos y la portada (si el formato la guarda)", plan.opts.keep_tags, accent) {
            plan.opts.keep_tags = !plan.opts.keep_tags;
        }

        if plan != before {
            self.plan = Some(plan);
            self.profile = None; // ya no es un perfil: "Personalizado"
            self.finished = None;
        }
    }
}

/// Renglón con un texto y un interruptor a la derecha. true = se tocó.
fn switch_row(ui: &mut Ui, id: &str, text: &str, hint: &str, on: bool, accent: Color32) -> bool {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
    let painter = ui.painter().clone();
    let t = painter.text(r.left_center(), Align2::LEFT_CENTER, text, regular(12.0), TEXT_DIM);
    ui.interact(t.expand(4.0), Id::unique((id, "label")), Sense::hover()).on_hover_text(hint);
    switch(ui, &painter, id, pos2(r.right() - 17.0, r.center().y), on, accent, ui.is_enabled())
}

//! Capyro Studio: un panel que se pone encima de la ventana para editar el
//! audio que está sonando: la portada, las etiquetas, una vista previa de un
//! tramo y el ecualizador avanzado de 15 bandas.
//!
//! Solo existe mientras está abierto (`CapyroApp::studio` es None si no): al
//! cerrarlo se suelta todo, también las imágenes, y no suma memoria.
//!
//! Respeta el modo de rendimiento: en **Bajo** aparece y se va al toque, sin
//! sombras, brillos ni curva; en **Medio**, con su animación, la sombra y la
//! curva del ecualizador; en **Alto**, además, la luz del color de la portada,
//! el brillo de la curva y el panel que se descubre con una línea de luz.
//!
//! Por ahora trabaja sobre audio (los videos no).

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{
    self, epaint::RectShape, pos2, vec2, Align2, Color32, ColorImage, Context, Id, Key, Modifiers, Pos2, Rect,
    Sense, Shape, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, UiBuilder,
};

use super::{animate_button, ease_out, fit_text, CapyroApp, FRAME_SMOOTH};
use crate::ffi::{self, AudioFx, Core, CoreError, CoverEdit, MediaKind, Rgba, State, Tags, EQ15_BANDS};
use crate::icons;
use crate::theme::*;

/// Frecuencias de las 15 bandas, para mostrarlas.
const BAND_LABELS: [&str; EQ15_BANDS] = [
    "25", "40", "63", "100", "160", "250", "400", "630", "1k", "1.6k", "2.5k", "4k", "6.3k", "10k", "16k",
];

/// Ecualizadores listos para usar (ganancia por banda en dB, de graves a agudos).
const EQ_PRESETS: [(&str, [f32; EQ15_BANDS]); 10] = [
    ("Plano", [0.0; EQ15_BANDS]),
    ("Graves", [6.0, 5.0, 4.0, 3.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("Voces", [-3.0, -2.0, -1.0, 0.0, 0.0, 1.0, 3.0, 4.0, 4.0, 3.0, 2.0, 1.0, 0.0, -1.0, -2.0]),
    ("Brillante", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0]),
    ("Rock", [5.0, 4.0, 3.0, 1.0, -1.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0]),
    ("Pop", [-1.0, -1.0, 0.0, 1.0, 2.0, 3.0, 3.0, 2.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0]),
    ("Jazz", [3.0, 2.0, 1.0, 2.0, -1.0, -1.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 4.0]),
    ("Clásica", [2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0]),
    ("Electrónica", [5.0, 4.0, 3.0, 1.0, 0.0, -2.0, -2.0, 0.0, 1.0, 2.0, 3.0, 3.0, 4.0, 5.0, 4.0]),
    ("Cine", [2.0, 2.0, 1.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 2.0]),
];

/// Lado más largo de las portadas que se muestran (las guardadas pueden ser más grandes).
const COVER_SHOW_SIDE: i32 = 512;
/// Segundos que dura armado el "cerrar sin guardar" (el segundo intento cierra).
const CLOSE_ARM_SECS: f64 = 4.0;
/// Mínimo de un tramo de la vista previa, en segundos.
const MIN_SEGMENT: f64 = 0.5;

/// Lo que vuelve del selector de imágenes (que corre en otro hilo).
enum Picked {
    Image(PathBuf, Rgba),
    Failed(String),
    Cancelled,
}

/// Una imagen ya subida a la placa de video, con su color medio (para la luz de Alto).
struct Picture {
    tex: TextureHandle,
    tint: Color32,
    aspect: f32,
}

impl Picture {
    fn new(ctx: &Context, name: &str, (w, h, rgba): &Rgba) -> Picture {
        let img = ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], rgba);
        let tex = ctx.load_texture(name, img, TextureOptions::LINEAR);
        // color medio, salteando píxeles (alcanza para una luz)
        let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
        for px in rgba.chunks_exact(4).step_by(17) {
            r += px[0] as u64;
            g += px[1] as u64;
            b += px[2] as u64;
            n += 1;
        }
        let n = n.max(1);
        let tint = Color32::from_rgb((r / n) as u8, (g / n) as u8, (b / n) as u8);
        Picture { tex, tint, aspect: *w as f32 / (*h).max(1) as f32 }
    }
}

pub struct Studio {
    /// Abierto (false = cerrándose: la animación de salida y después se suelta).
    open: bool,
    /// Archivo que se edita (el que está sonando al abrir).
    file: PathBuf,
    /// Lo que hay en los campos y lo que tiene el archivo.
    tags: Tags,
    saved: Tags,
    /// Último mensaje: (texto, es error, cuándo).
    msg: Option<(String, bool, f64)>,
    /// Portada: si el formato la guarda, la del archivo, el cambio pendiente
    /// y la imagen elegida (si se cambia).
    cover_ok: bool,
    cover: Option<Picture>,
    cover_edit: CoverEdit,
    new_cover: Option<Picture>,
    picking: Option<Receiver<Picked>>,
    /// Vista previa: tramo en segundos, si está sonando, si se repite y qué
    /// manija se arrastra (0 = inicio, 1 = fin).
    start: f64,
    end: f64,
    preview: bool,
    repeat_preview: bool,
    drag_handle: Option<u8>,
    /// Ecualizador de 15 bandas (se aplica con set_fx).
    eq_on: bool,
    eq: [f32; EQ15_BANDS],
    /// "Cerrar" con cambios sin guardar: el primer intento solo avisa.
    close_armed: Option<f64>,
    /// Recién abierto: el scroll arranca arriba (egui recuerda el de la vez anterior).
    fresh: bool,
}

impl Studio {
    /// Abre el editor para el archivo que está sonando. Error = texto para mostrar.
    fn open(core: &Core, ctx: &Context) -> Result<Studio, String> {
        let path = core.current_file();
        core.playlist_current().ok_or("Abrí un audio primero")?;
        if path.is_empty() {
            return Err("Abrí un audio primero".to_owned());
        }
        if core.media_kind() != MediaKind::Audio {
            return Err("Capyro Studio edita audio por ahora".to_owned());
        }
        let file = PathBuf::from(&path);
        let tags = ffi::read_tags(&file).map_err(|e| format!("No pude leer las etiquetas: {}", e.0))?;
        let cover = ffi::read_cover(&file, COVER_SHOW_SIDE).map(|img| Picture::new(ctx, "capyro-studio-cover", &img));
        let fx = core.fx();
        let duration = core.duration().max(0.0);
        Ok(Studio {
            open: true,
            cover_ok: ffi::cover_supported(&file),
            file,
            saved: tags.clone(),
            tags,
            msg: None,
            cover,
            cover_edit: CoverEdit::Keep,
            new_cover: None,
            picking: None,
            start: 0.0,
            end: duration,
            preview: false,
            repeat_preview: false,
            drag_handle: None,
            eq_on: fx.eq15_on,
            eq: fx.eq15,
            close_armed: None,
            fresh: true,
        })
    }

    fn dirty(&self) -> bool {
        self.tags != self.saved || self.cover_edit != CoverEdit::Keep
    }

    fn say(&mut self, now: f64, text: impl Into<String>, error: bool) {
        self.msg = Some((text.into(), error, now));
    }

    /// La portada que se ve: la elegida, ninguna (si se quita) o la del archivo.
    fn shown_cover(&self) -> Option<&Picture> {
        match self.cover_edit {
            CoverEdit::Set(_) => self.new_cover.as_ref(),
            CoverEdit::Remove => None,
            CoverEdit::Keep => self.cover.as_ref(),
        }
    }

    /// Vuelve todo a como está en el archivo.
    fn undo(&mut self) {
        self.tags = self.saved.clone();
        self.cover_edit = CoverEdit::Keep;
        self.new_cover = None;
        self.close_armed = None;
    }

    /// Guarda etiquetas y portada. Si el archivo es el que está sonando, lo
    /// suelta, lo reescribe y lo vuelve a abrir en el mismo punto (si sonaba,
    /// sigue). Si no está sonando, solo se reescribe.
    fn save(&mut self, core: &Core, ctx: &Context, now: f64) {
        self.preview = false;
        self.close_armed = None;
        let open_here = PathBuf::from(core.current_file()) == self.file;
        let pos = core.position();
        let was_playing = core.state() == State::Playing;
        let index = core.playlist_current();
        if open_here {
            core.release_file();
        }
        let result = ffi::write_tags_cover(&self.file, &self.tags, &self.cover_edit);
        // siempre se vuelve a abrir, haya salido bien o mal
        let reopened = if !open_here {
            Ok(())
        } else if let Some(i) = index {
            let r = core.playlist_select(i);
            if r.is_ok() {
                core.seek(pos);
                if was_playing {
                    core.toggle();
                }
            }
            r
        } else {
            Err(CoreError("no encuentro el archivo en la lista".to_owned()))
        };
        match (result, reopened) {
            (Ok(()), reopened) => {
                self.saved = self.tags.clone();
                if self.cover_edit != CoverEdit::Keep {
                    // lo que quedó en el archivo es lo que se ve
                    self.cover = match std::mem::replace(&mut self.cover_edit, CoverEdit::Keep) {
                        CoverEdit::Set(_) => ffi::read_cover(&self.file, COVER_SHOW_SIDE)
                            .map(|img| Picture::new(ctx, "capyro-studio-cover", &img)),
                        _ => None,
                    };
                    self.new_cover = None;
                }
                match reopened {
                    Ok(()) => self.say(now, "Guardado · la versión anterior quedó como .bak", false),
                    Err(e) => self.say(now, format!("Se guardó, pero no pude volver a abrirlo: {}", e.0), true),
                }
            }
            (Err(e), _) => self.say(now, format!("No se pudo guardar: {}", e.0), true),
        }
    }

    /// Abre el selector de imágenes del sistema en otro hilo (no congela la ventana).
    fn pick_cover(&mut self, ctx: &Context) {
        if self.picking.is_some() || !self.cover_ok {
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

    fn poll_pick(&mut self, ctx: &Context, now: f64) {
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

    /// Aplica el ecualizador de 15 bandas al motor (sin tocar los demás efectos).
    fn apply_eq(&self, core: &Core) {
        let mut fx: AudioFx = core.fx();
        fx.eq15_on = self.eq_on;
        fx.eq15 = self.eq;
        core.set_fx(&fx);
    }

    /// Reproduce el tramo desde su inicio.
    fn play_preview(&mut self, core: &Core, now: f64) {
        if self.end <= self.start + MIN_SEGMENT * 0.5 {
            self.say(now, "El fin tiene que estar después del inicio", true);
            return;
        }
        self.preview = true;
        core.seek(self.start);
        if core.state() != State::Playing {
            core.toggle();
        }
    }

    /// Vigila el tramo mientras suena (se llama en cada cuadro).
    fn tick_preview(&mut self, core: &Core) {
        if !self.preview {
            return;
        }
        if core.state() != State::Playing {
            self.preview = false;
            return;
        }
        if core.position() >= self.end {
            if self.repeat_preview {
                core.seek(self.start);
            } else {
                self.preview = false;
                core.toggle(); // pausa al llegar al fin del tramo
            }
        }
    }

    /// Cerrar: con cambios sin guardar, el primer intento solo avisa.
    fn try_close(&mut self, now: f64) {
        let armed = self.close_armed.is_some_and(|t| now - t < CLOSE_ARM_SECS);
        if self.dirty() && !armed {
            self.close_armed = Some(now);
            self.say(now, "Hay cambios sin guardar: cerrá de nuevo para descartarlos", true);
            return;
        }
        self.open = false;
        self.preview = false;
    }
}

/* ------------------------------------------------------------------------ */
/* piezas de la interfaz                                                    */
/* ------------------------------------------------------------------------ */

fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Divide `rect` en columnas con estos pesos (y `gap` entre ellas).
fn columns(rect: Rect, weights: &[f32], gap: f32) -> Vec<Rect> {
    let total: f32 = weights.iter().sum::<f32>().max(0.001);
    let free = rect.width() - gap * (weights.len().saturating_sub(1)) as f32;
    let mut x = rect.left();
    weights
        .iter()
        .map(|w| {
            let cw = free * w / total;
            let r = Rect::from_min_size(pos2(x, rect.top()), vec2(cw, rect.height()));
            x += cw + gap;
            r
        })
        .collect()
}

/// Campo de texto con su nombre arriba. Fondo suave, borde de acento al escribir.
#[allow(clippy::too_many_arguments)]
fn field(ui: &mut Ui, rect: Rect, id: &str, label: &str, value: &mut String, hint: &str, accent: Color32, rows: usize) {
    let painter = ui.painter().clone();
    painter.text(pos2(rect.left() + 2.0, rect.top() + 6.0), Align2::LEFT_CENTER, label, regular(10.5), TEXT_FAINT);
    let boxr = Rect::from_min_max(pos2(rect.left(), rect.top() + 15.0), rect.max);
    let bg = painter.add(Shape::Noop);
    let font = regular(13.0);
    let hint = egui::RichText::new(hint).color(TEXT_FAINT.gamma_multiply(0.75)).font(font.clone());
    let edit = if rows > 1 {
        egui::TextEdit::multiline(value).desired_rows(rows)
    } else {
        egui::TextEdit::singleline(value).align(Align2::LEFT_CENTER)
    };
    let resp = ui.put(
        boxr,
        edit.id_salt(("capyro-studio-field", id))
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(9, 6)))
            .font(font)
            .text_color(TEXT)
            .hint_text(hint),
    );
    let focus = resp.has_focus();
    let hover = resp.hovered();
    let alpha = if focus { 14 } else if hover { 11 } else { 7 };
    painter.set(bg, RectShape::filled(boxr, 8.0, Color32::from_white_alpha(alpha)));
    if focus {
        painter.rect_stroke(boxr, 8.0, Stroke::new(1.2, accent.gamma_multiply(0.85)), StrokeKind::Inside);
    }
}

/// Campo grande sin nombre (el título y el artista): solo una línea abajo.
#[allow(clippy::too_many_arguments)]
fn big_field(ui: &mut Ui, rect: Rect, id: &str, value: &mut String, hint: &str, font: egui::FontId, color: Color32, accent: Color32) {
    let painter = ui.painter().clone();
    let bg = painter.add(Shape::Noop);
    let hint = egui::RichText::new(hint).color(TEXT_FAINT.gamma_multiply(0.75)).font(font.clone());
    let resp = ui.put(
        rect,
        egui::TextEdit::singleline(value)
            .id_salt(("capyro-studio-big", id))
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(6, 2)))
            .font(font)
            .text_color(color)
            .hint_text(hint)
            .align(Align2::LEFT_CENTER),
    );
    let focus = resp.has_focus();
    let hover = resp.hovered();
    if focus || hover {
        painter.set(bg, RectShape::filled(rect, 8.0, Color32::from_white_alpha(if focus { 10 } else { 6 })));
    }
    let line_col = if focus { accent } else { Color32::from_white_alpha(if hover { 40 } else { 16 }) };
    painter.line_segment(
        [pos2(rect.left() + 6.0, rect.bottom()), pos2(rect.right() - 6.0, rect.bottom())],
        Stroke::new(if focus { 1.5 } else { 1.0 }, line_col),
    );
}

/// Botón de texto chiquito (sin fondo hasta pasar el mouse). Devuelve true si se tocó.
fn link(ui: &Ui, id: &str, at: Pos2, align: Align2, text: &str, color: Color32, enabled: bool) -> (bool, Rect) {
    let painter = ui.painter();
    let g = painter.layout_no_wrap(text.to_owned(), regular(11.5), color);
    let size = g.size() + vec2(14.0, 8.0);
    let rect = align.anchor_size(at, size);
    let a = animate_button(ui, Id::unique(("capyro-studio-link", id)), rect, enabled);
    if a.hover > 0.001 {
        painter.rect_filled(rect, 7.0, Color32::from_white_alpha((12.0 * a.hover) as u8));
    }
    let col = if enabled { color.lerp_to_gamma(TEXT, a.hover) } else { TEXT_FAINT };
    painter.text(rect.center(), Align2::CENTER_CENTER, text, regular(11.5), col);
    (a.resp.clicked(), rect)
}

/// Pastilla: llena (acento) o de contorno. Devuelve true si se tocó.
#[allow(clippy::too_many_arguments)]
fn pill(ui: &Ui, id: &str, rect: Rect, text: &str, filled: bool, selected: f32, accent: Color32, enabled: bool) -> bool {
    let painter = ui.painter();
    let a = animate_button(ui, Id::unique(("capyro-studio-pill", id)), rect, enabled);
    let r = Rect::from_center_size(rect.center(), rect.size() * (1.0 - 0.05 * a.press));
    if crate::alto::on() && enabled {
        crate::alto::glow_rect(painter, r, accent, if filled { 0.5 } else { 0.4 } * a.hover);
    }
    let (bg, fg) = if filled {
        let bg = if enabled { accent.lerp_to_gamma(Color32::WHITE, 0.12 * a.hover) } else { Color32::from_white_alpha(14) };
        (bg, if enabled { Color32::WHITE } else { TEXT_FAINT })
    } else {
        let bg = Color32::from_white_alpha((8.0 + 14.0 * a.hover) as u8).lerp_to_gamma(accent, selected);
        let fg = if enabled { TEXT_DIM.lerp_to_gamma(TEXT, a.hover).lerp_to_gamma(Color32::WHITE, selected) } else { TEXT_FAINT };
        (bg, fg)
    };
    painter.rect_filled(r, r.height() / 2.0, bg);
    let font = if filled || selected > 0.5 { bold(11.5) } else { regular(11.5) };
    painter.text(r.center(), Align2::CENTER_CENTER, text, font, fg);
    a.resp.clicked()
}

/// Ancho de una pastilla para `text`.
fn pill_w(ui: &Ui, text: &str) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), bold(11.5), TEXT).size().x + 24.0
}

/// Línea suave que pasa por todos los puntos (Catmull-Rom).
fn smooth(points: &[Pos2]) -> Vec<Pos2> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(points.len() * 8);
    for i in 0..points.len() - 1 {
        let p0 = points[i.saturating_sub(1)];
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = points[(i + 2).min(points.len() - 1)];
        for k in 0..8 {
            let t = k as f32 / 8.0;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * (2.0 * b + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push(pos2(f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y)));
        }
    }
    out.push(*points.last().expect("puntos"));
    out
}

/* ------------------------------------------------------------------------ */
/* secciones                                                                */
/* ------------------------------------------------------------------------ */

impl Studio {
    /// Portada (a la izquierda, o arriba si es angosto) + título, artista y el resto.
    fn info_section(&mut self, ui: &mut Ui, ctx: &Context, accent: Color32, now: f64) {
        let w = ui.available_width();
        let narrow = w < 560.0;
        let side = if narrow { (w * 0.55).clamp(120.0, 190.0) } else { (w * 0.28).clamp(140.0, 210.0) };
        let fields_h = 44.0 + 32.0 + 12.0 + 3.0 * 48.0 + 70.0;
        let cover_h = side + 34.0;
        let h = if narrow { cover_h + 14.0 + fields_h } else { cover_h.max(fields_h) };
        let (sec, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());

        let cover_rect = if narrow {
            Rect::from_min_size(pos2(sec.center().x - side / 2.0, sec.top()), vec2(side, side))
        } else {
            Rect::from_min_size(sec.min, vec2(side, side))
        };
        self.cover_widget(ui, ctx, cover_rect, accent, now);

        let fx0 = if narrow { sec.left() } else { cover_rect.right() + 24.0 };
        let fy0 = if narrow { cover_rect.bottom() + 48.0 } else { sec.top() };
        let fw = sec.right() - fx0;
        let mut y = fy0;
        let t = &mut self.tags;
        big_field(ui, Rect::from_min_size(pos2(fx0, y), vec2(fw, 42.0)), "title", &mut t.title, "Nombre de la canción", bold(24.0), TEXT, accent);
        y += 46.0;
        big_field(ui, Rect::from_min_size(pos2(fx0, y), vec2(fw, 30.0)), "artist", &mut t.artist, "Artista o intérprete", regular(15.0), TEXT_DIM, accent);
        y += 30.0 + 14.0;
        let row = |y: f32, h: f32| Rect::from_min_size(pos2(fx0, y), vec2(fw, h));
        let c = columns(row(y, 44.0), &[1.0, 1.0], 12.0);
        field(ui, c[0], "album", "Álbum", &mut t.album, "Álbum al que pertenece", accent, 1);
        field(ui, c[1], "album_artist", "Artista del álbum", &mut t.album_artist, "Artista principal del álbum", accent, 1);
        y += 48.0;
        let c = columns(row(y, 44.0), &[1.6, 0.8, 0.7, 0.7], 12.0);
        field(ui, c[0], "genre", "Género", &mut t.genre, "Género musical", accent, 1);
        field(ui, c[1], "year", "Año", &mut t.year, "—", accent, 1);
        field(ui, c[2], "track", "Pista", &mut t.track, "—", accent, 1);
        field(ui, c[3], "disc", "Disco", &mut t.disc, "—", accent, 1);
        y += 48.0;
        field(ui, row(y, 44.0), "composer", "Compositor", &mut t.composer, "Quién la compuso", accent, 1);
        y += 48.0;
        field(ui, row(y, 66.0), "comment", "Comentarios", &mut t.comment, "Una nota, la letra, de dónde la sacaste…", accent, 2);

        // WAV no guarda estas tres etiquetas (su formato no tiene dónde ponerlas)
        if self.file.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav")) {
            ui.painter().text(
                pos2(fx0 + 2.0, y + 74.0),
                Align2::LEFT_CENTER,
                "En WAV no se guardan el artista del álbum, el disco, el compositor ni la portada.",
                regular(10.5),
                TEXT_FAINT,
            );
            ui.add_space(16.0);
        }
    }

    /// La portada: al tocarla se abre el selector de imágenes.
    fn cover_widget(&mut self, ui: &mut Ui, ctx: &Context, rect: Rect, accent: Color32, now: f64) {
        let painter = ui.painter().clone();
        let anim = crate::perf::animations();
        let alto = crate::alto::on();
        let radius = (rect.width() * 0.07).clamp(8.0, 16.0);
        let a = animate_button(ui, Id::unique("capyro-studio-cover"), rect, self.cover_ok && self.picking.is_none());
        let shown = Rect::from_center_size(rect.center(), rect.size() * (1.0 + 0.015 * a.hover - 0.03 * a.press));

        if let Some(p) = self.shown_cover() {
            // Alto: la portada ilumina un poco alrededor con su color
            if alto {
                crate::alto::glow(&painter, shown.center(), shown.width() * 0.95, p.tint, 0.32 + 0.1 * a.hover);
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
            icons::music_note(&painter, shown.center() - vec2(0.0, 10.0), shown.width() * 0.18, accent);
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
                icons::spinner(&painter, c - vec2(0.0, 10.0), 22.0, col, crate::perf::clock(now));
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

    /// Vista previa de un tramo: barra con dos manijas, tocar y repetir.
    fn preview_section(&mut self, ui: &mut Ui, core: &Core, accent: Color32, now: f64) {
        let w = ui.available_width();
        let duration = core.duration().max(0.0);
        let (head, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
        let painter = ui.painter().clone();
        painter.text(head.left_center(), Align2::LEFT_CENTER, "Vista previa del tramo", bold(13.0), TEXT);
        painter.text(
            head.right_center(),
            Align2::RIGHT_CENTER,
            "el archivo no cambia",
            regular(10.5),
            TEXT_FAINT,
        );
        if duration <= 0.0 {
            let (r, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
            painter.text(r.left_center(), Align2::LEFT_CENTER, "La duración no es conocida: no se puede elegir el tramo.", regular(12.0), TEXT_DIM);
            return;
        }
        self.start = self.start.clamp(0.0, duration);
        self.end = self.end.clamp(0.0, duration);
        if self.end <= self.start {
            self.end = (self.start + MIN_SEGMENT).min(duration);
        }

        /* --- la barra --- */
        ui.add_space(6.0);
        let (bar, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
        let track = Rect::from_min_max(pos2(bar.left() + 8.0, bar.center().y - 3.0), pos2(bar.right() - 8.0, bar.center().y + 3.0));
        let x_of = |s: f64| track.left() + (s / duration) as f32 * track.width();
        let s_of = |x: f32| (((x - track.left()) / track.width()).clamp(0.0, 1.0) as f64) * duration;
        let id = Id::unique("capyro-studio-range");
        let resp = ui.interact(bar, id, Sense::click_and_drag());
        let ctx = ui.ctx().clone();
        let hover = ctx.animate_bool_with_time(id.with("h"), resp.hovered() || resp.dragged(), crate::perf::t(0.12));
        if resp.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        let nearest = |x: f32, start: f64, end: f64| -> u8 {
            if (x - x_of(start)).abs() <= (x - x_of(end)).abs() { 0 } else { 1 }
        };
        if resp.double_clicked() {
            self.start = 0.0;
            self.end = duration;
        } else if let Some(p) = resp.interact_pointer_pos() {
            if resp.drag_started() {
                self.drag_handle = Some(nearest(p.x, self.start, self.end));
            }
            let handle = if resp.clicked() { Some(nearest(p.x, self.start, self.end)) } else { self.drag_handle };
            if resp.dragged() || resp.clicked() {
                let v = s_of(p.x);
                match handle {
                    Some(0) => self.start = v.min(self.end - MIN_SEGMENT).max(0.0),
                    Some(_) => self.end = v.max(self.start + MIN_SEGMENT).min(duration),
                    None => {}
                }
            }
        }
        if resp.drag_stopped() {
            self.drag_handle = None;
        }

        painter.rect_filled(track, 3.0, TRACK);
        let (xs, xe) = (x_of(self.start), x_of(self.end));
        let sel = Rect::from_min_max(pos2(xs, track.top()), pos2(xe, track.bottom()));
        if crate::alto::on() {
            crate::alto::glow_rect(&painter, sel, accent, 0.35 + 0.25 * hover);
        }
        painter.rect_filled(sel, 3.0, accent.gamma_multiply(0.85));
        // dónde está sonando
        let pos = core.position().clamp(0.0, duration);
        let xp = x_of(pos);
        painter.rect_filled(Rect::from_center_size(pos2(xp, track.center().y), vec2(2.0, 16.0)), 1.0, Color32::from_white_alpha(200));
        // manijas
        for (i, x) in [xs, xe].into_iter().enumerate() {
            let active = self.drag_handle == Some(i as u8);
            let hh = 20.0 + 4.0 * hover + if active { 2.0 } else { 0.0 };
            let r = Rect::from_center_size(pos2(x, track.center().y), vec2(8.0, hh));
            painter.rect_filled(r, 4.0, Color32::WHITE);
            painter.rect_stroke(r, 4.0, Stroke::new(1.5, accent), StrokeKind::Inside);
        }

        /* --- controles --- */
        ui.add_space(8.0);
        let (row, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
        let play_c = pos2(row.left() + 16.0, row.center().y);
        let play = animate_button(ui, Id::unique("capyro-studio-play"), Rect::from_center_size(play_c, vec2(32.0, 32.0)), true);
        if crate::alto::on() {
            crate::alto::glow(&painter, play_c, 30.0, accent, 0.25 * play.hover);
        }
        painter.circle_filled(play_c, 15.0 * (1.0 + 0.05 * play.hover - 0.08 * play.press), accent);
        if self.preview {
            painter.rect_filled(Rect::from_center_size(play_c, vec2(10.0, 10.0)), 2.0, Color32::WHITE);
        } else {
            icons::play_small(&painter, play_c + vec2(1.0, 0.0), 12.0, Color32::WHITE);
        }
        if play.resp.clicked() {
            if self.preview {
                self.preview = false;
                if core.state() == State::Playing {
                    core.toggle();
                }
            } else {
                self.play_preview(core, now);
            }
        }
        play.resp.on_hover_text(if self.preview { "Parar" } else { "Tocar solo este tramo" });
        let len = self.end - self.start;
        painter.text(
            pos2(row.left() + 42.0, row.center().y - 7.0),
            Align2::LEFT_CENTER,
            format!("{}  →  {}", fmt_time(self.start), fmt_time(self.end)),
            bold(12.5),
            TEXT,
        );
        painter.text(
            pos2(row.left() + 42.0, row.center().y + 8.0),
            Align2::LEFT_CENTER,
            format!("{} de tramo", if len < 60.0 { format!("{:.1} s", len) } else { fmt_time(len) }),
            regular(10.5),
            TEXT_FAINT,
        );

        // a la derecha: repetir + inicio/fin aquí + todo
        let mut x = row.right();
        let sw_c = pos2(x - 17.0, row.center().y);
        if super::fx_menu::switch(ui, &painter, "studio-repeat", sw_c, self.repeat_preview, accent, true) {
            self.repeat_preview = !self.repeat_preview;
        }
        x -= 40.0;
        painter.text(pos2(x, row.center().y), Align2::RIGHT_CENTER, "Repetir", regular(11.5), TEXT_DIM);
        x -= ui.painter().layout_no_wrap("Repetir".to_owned(), regular(11.5), TEXT).size().x + 16.0;
        let chips = [("start-here", "Inicio aquí"), ("end-here", "Fin aquí"), ("whole", "Todo")];
        let widths: Vec<f32> = chips.iter().map(|(_, t)| pill_w(ui, t)).collect();
        let total: f32 = widths.iter().sum::<f32>() + 6.0 * (chips.len() - 1) as f32;
        // a la derecha si entran; si no, en una fila más abajo
        let (mut cx, cy) = if x - total >= row.left() + 190.0 {
            (x - total, row.center().y)
        } else {
            let (row2, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
            (row2.left(), row2.center().y + 4.0)
        };
        for ((id, text), cw) in chips.into_iter().zip(widths) {
            let r = Rect::from_min_max(pos2(cx, cy - 12.0), pos2(cx + cw, cy + 12.0));
            cx += cw + 6.0;
            if pill(ui, id, r, text, false, 0.0, accent, true) {
                let p = core.position().clamp(0.0, duration);
                match id {
                    "whole" => {
                        self.start = 0.0;
                        self.end = duration;
                    }
                    "end-here" => self.end = p.max(self.start + MIN_SEGMENT).min(duration),
                    _ => self.start = p.min(self.end - MIN_SEGMENT).max(0.0),
                }
            }
        }
    }

    /// Ecualizador avanzado: interruptor, perfiles y las 15 bandas.
    fn eq_section(&mut self, ui: &mut Ui, core: &Core, accent: Color32) {
        let w = ui.available_width();
        let anim = crate::perf::animations();
        let alto = crate::alto::on();
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();

        /* --- encabezado --- */
        let (head, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
        let tg = painter.layout_no_wrap("Ecualizador avanzado".to_owned(), bold(13.0), TEXT);
        let tw = tg.size().x;
        painter.galley(pos2(head.left(), head.center().y - tg.size().y / 2.0), tg, TEXT);
        painter.text(pos2(head.left() + tw + 8.0, head.center().y + 1.0), Align2::LEFT_CENTER, "15 bandas", regular(11.0), TEXT_FAINT);
        let mut changed = false;
        let sw_c = pos2(head.right() - 17.0, head.center().y);
        if super::fx_menu::switch(ui, &painter, "studio-eq", sw_c, self.eq_on, accent, true) {
            self.eq_on = !self.eq_on;
            changed = true;
        }
        painter.text(
            pos2(head.right() - 42.0, head.center().y),
            Align2::RIGHT_CENTER,
            if self.eq_on { "Activado" } else { "Apagado" },
            regular(11.0),
            TEXT_FAINT,
        );
        let (note, _) = ui.allocate_exact_size(vec2(w, 18.0), Sense::hover());
        let note_text = if self.eq_on {
            "Mientras está activado, el ecualizador normal (graves, medios y agudos) queda apagado."
        } else {
            "Al activarlo, reemplaza al ecualizador normal (graves, medios y agudos)."
        };
        painter.text(
            note.left_center(),
            Align2::LEFT_CENTER,
            fit_text(&painter, note_text, regular(10.5), w),
            regular(10.5),
            TEXT_FAINT,
        );

        /* --- perfiles (en filas que se acomodan al ancho) --- */
        ui.add_space(6.0);
        let gap = 6.0;
        let widths: Vec<f32> = EQ_PRESETS.iter().map(|(n, _)| pill_w(ui, n)).collect();
        let mut rows = 1;
        let mut x = 0.0;
        for cw in &widths {
            if x > 0.0 && x + cw > w {
                rows += 1;
                x = 0.0;
            }
            x += cw + gap;
        }
        let (area, _) = ui.allocate_exact_size(vec2(w, rows as f32 * 30.0), Sense::hover());
        let (mut x, mut y) = (area.left(), area.top());
        for (i, (name, gains)) in EQ_PRESETS.iter().enumerate() {
            let cw = widths[i];
            if x > area.left() && x + cw > area.right() {
                x = area.left();
                y += 30.0;
            }
            let r = Rect::from_min_size(pos2(x, y + 2.0), vec2(cw, 24.0));
            let selected = self.eq_on && self.eq.iter().zip(gains.iter()).all(|(a, b)| (a - b).abs() < 0.05);
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-studio-preset", i)), selected, crate::perf::t(0.15));
            if pill(ui, &format!("preset-{i}"), r, name, false, sel, accent, true) {
                self.eq = *gains;
                self.eq_on = true;
                changed = true;
            }
            x += cw + gap;
        }

        /* --- las bandas --- */
        ui.add_space(10.0);
        let (area, _) = ui.allocate_exact_size(vec2(w, 200.0), Sense::hover());
        let label_w = 30.0;
        let bands = Rect::from_min_max(pos2(area.left() + label_w, area.top()), area.max);
        let (yt, yb) = (bands.top() + 14.0, bands.bottom() - 34.0);
        let y0 = (yt + yb) / 2.0;
        let half = (yb - yt) / 2.0;
        let y_of = |db: f32| y0 - db.clamp(-12.0, 12.0) / 12.0 * half;
        let col_w = bands.width() / EQ15_BANDS as f32;
        let cx_of = |i: usize| bands.left() + col_w * (i as f32 + 0.5);

        // interacción primero (para dibujar ya con el valor nuevo)
        let mut hovers = [0.0f32; EQ15_BANDS];
        let mut active = [false; EQ15_BANDS];
        for i in 0..EQ15_BANDS {
            let col = Rect::from_min_max(pos2(cx_of(i) - col_w / 2.0, yt - 10.0), pos2(cx_of(i) + col_w / 2.0, yb + 10.0));
            let id = Id::unique(("capyro-studio-band", i));
            let resp = ui.interact(col, id, Sense::click_and_drag());
            let hot = resp.hovered() || resp.dragged();
            hovers[i] = ctx.animate_bool_with_time(id.with("h"), hot, crate::perf::t(0.12));
            active[i] = resp.dragged();
            if hot {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
            if resp.double_clicked() {
                self.eq[i] = 0.0;
                self.eq_on = true;
                changed = true;
            } else if resp.dragged() || resp.clicked() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let v = (((y0 - p.y) / half * 12.0) * 2.0).round() / 2.0;
                    let v = v.clamp(-12.0, 12.0);
                    if (v - self.eq[i]).abs() > f32::EPSILON || !self.eq_on {
                        self.eq[i] = v;
                        self.eq_on = true;
                        changed = true;
                    }
                }
            }
            resp.on_hover_text("Arrastrá para subir o bajar · doble clic: 0 dB");
        }

        // rejilla: +12, +6, 0, -6, -12
        for db in [12.0f32, 6.0, 0.0, -6.0, -12.0] {
            let y = y_of(db);
            let alpha = if db == 0.0 { 22 } else { 9 };
            painter.line_segment([pos2(bands.left(), y), pos2(bands.right(), y)], Stroke::new(1.0, Color32::from_white_alpha(alpha)));
            if db.abs() > 0.1 && (db.abs() - 6.0).abs() < 0.1 {
                continue; // ±6 sin número, para no amontonar
            }
            let text = if db == 0.0 { "0".to_owned() } else { format!("{db:+.0}") };
            painter.text(pos2(area.left() + label_w - 8.0, y), Align2::RIGHT_CENTER, text, regular(9.5), TEXT_FAINT);
        }

        let on = self.eq_on;
        let fill_col = if on { accent } else { Color32::from_white_alpha(60) };
        let points: Vec<Pos2> = (0..EQ15_BANDS).map(|i| pos2(cx_of(i), y_of(self.eq[i]))).collect();
        let curve = if anim { smooth(&points) } else { Vec::new() };

        // Alto: el área bajo la curva, con un poco de color
        if alto && on && curve.len() > 1 {
            let mut mesh = egui::Mesh::default();
            let top = accent.gamma_multiply(0.16);
            let base = accent.gamma_multiply(0.02);
            for p in &curve {
                mesh.colored_vertex(*p, top);
                mesh.colored_vertex(pos2(p.x, y0), base);
            }
            for k in 0..(curve.len() as u32 - 1) {
                let a = k * 2;
                mesh.add_triangle(a, a + 1, a + 2);
                mesh.add_triangle(a + 1, a + 3, a + 2);
            }
            painter.add(Shape::mesh(mesh));
        }

        // rieles y relleno desde 0
        for i in 0..EQ15_BANDS {
            let cx = cx_of(i);
            let tw = 4.0 + 1.5 * hovers[i];
            painter.rect_filled(Rect::from_min_max(pos2(cx - tw / 2.0, yt), pos2(cx + tw / 2.0, yb)), tw / 2.0, TRACK);
            let yv = y_of(self.eq[i]);
            if (yv - y0).abs() > 0.5 {
                let r = Rect::from_min_max(pos2(cx - tw / 2.0, yv.min(y0)), pos2(cx + tw / 2.0, yv.max(y0)));
                painter.rect_filled(r, tw / 2.0, fill_col.gamma_multiply(if on { 0.85 } else { 1.0 }));
            }
        }

        // la curva (Medio y Alto); en Alto, con brillo
        if curve.len() > 1 {
            let col = if on { accent } else { Color32::from_white_alpha(70) };
            if alto && on {
                painter.add(Shape::line(curve.clone(), Stroke::new(7.0, accent.gamma_multiply(0.16))));
            }
            painter.add(Shape::line(curve, Stroke::new(2.0, col.gamma_multiply(0.9))));
        }

        // perillas, valores y frecuencias
        for i in 0..EQ15_BANDS {
            let c = points[i];
            let r = 6.0 + 1.5 * hovers[i] + if active[i] { 1.0 } else { 0.0 };
            if alto && on && hovers[i] > 0.01 {
                crate::alto::glow(&painter, c, 18.0, accent, 0.35 * hovers[i]);
            }
            painter.circle_filled(c, r, Color32::WHITE);
            painter.circle_stroke(c, r, Stroke::new(1.5, if on { accent } else { TEXT_FAINT }));
            if hovers[i] > 0.01 {
                let v = self.eq[i];
                let text = if v.abs() < 0.05 { "0".to_owned() } else { format!("{v:+.1}") };
                let ty = (c.y - 16.0).max(area.top() + 6.0);
                painter.text(pos2(c.x, ty), Align2::CENTER_CENTER, text, bold(10.5), TEXT.gamma_multiply(hovers[i]));
            }
            let lc = TEXT_FAINT.lerp_to_gamma(TEXT, hovers[i]);
            painter.text(pos2(c.x, yb + 18.0), Align2::CENTER_CENTER, BAND_LABELS[i], regular(10.0), lc);
        }
        painter.text(pos2(area.left() + label_w - 8.0, yb + 18.0), Align2::RIGHT_CENTER, "Hz", regular(9.5), TEXT_FAINT);

        if changed {
            self.apply_eq(core);
        }
    }
}

/* ------------------------------------------------------------------------ */
/* el panel                                                                 */
/* ------------------------------------------------------------------------ */

impl CapyroApp {
    /// Abre Capyro Studio (o avisa por qué no se puede).
    pub(super) fn open_studio(&mut self, ctx: &egui::Context) {
        if self.studio.as_ref().is_some_and(|s| s.open) {
            return;
        }
        match Studio::open(&self.core, ctx) {
            Ok(st) => {
                self.close_menus();
                self.studio = Some(st);
            }
            Err(e) => self.toast(ctx, e, true),
        }
    }

    /// ¿Está Capyro Studio abierto? (los atajos que abren cosas no andan)
    pub(super) fn studio_open(&self) -> bool {
        self.studio.as_ref().is_some_and(|s| s.open)
    }

    /// Dibuja Capyro Studio encima de todo (si está abierto o cerrándose).
    pub(super) fn studio_overlay(&mut self, ui: &mut Ui, full: Rect) {
        let ctx = ui.ctx().clone();
        let open = self.studio_open();
        // se llama siempre: así la animación de entrada arranca de cero
        let t = ctx.animate_bool_with_time(Id::unique("capyro-studio-anim"), open, crate::perf::t(0.22));
        if self.studio.is_none() {
            return;
        }
        if !open && t <= 0.001 {
            self.studio = None; // cerrado: se suelta todo (imágenes incluidas)
            return;
        }
        if (0.001..0.999).contains(&t) {
            ctx.request_repaint_after(FRAME_SMOOTH);
        }
        let accent = self.cfg.accent;
        let now = ctx.input(|i| i.time);
        let anim = crate::perf::animations();
        let core = &self.core;
        let Some(st) = self.studio.as_mut() else { return };
        st.poll_pick(&ctx, now);
        st.tick_preview(core);
        if st.preview {
            ctx.request_repaint_after(crate::perf::ambient());
        }
        // escribiendo en un campo, Escape suelta el campo (no cierra)
        let typing = ctx.egui_wants_keyboard_input();

        let ease = ease_out(t);
        let area = full.shrink(16.0);
        let size = vec2(area.width().min(880.0), area.height().min(740.0));
        let full_panel = Rect::from_center_size(area.center(), size);
        let panel = Rect::from_center_size(area.center(), size * (0.96 + 0.04 * crate::alto::pop(t, ease)));
        let interactive = open && t > 0.6;
        let radius = 16.0;

        let mut painter = ui.painter().clone();
        painter.multiply_opacity(ease);
        // todo el mouse es del panel: lo de atrás no se toca (tampoco mientras se abre o se cierra)
        let outside = ui.interact(ui.max_rect(), Id::unique("capyro-studio-outside"), Sense::click());
        if interactive && outside.clicked() {
            st.try_close(now);
        }
        ui.interact(full_panel, Id::unique("capyro-studio-bg"), Sense::click());
        // velo oscuro detrás, como un diálogo
        painter.rect_filled(ui.max_rect(), 0.0, Color32::from_black_alpha((130.0 * ease) as u8));
        if anim {
            soft_shadow(&painter, panel, radius, 22.0, 6.0, 120);
        }
        painter.rect_filled(panel, radius, BG_PANEL);
        painter.rect_stroke(panel, radius, Stroke::new(1.0, Color32::from_white_alpha(16)), StrokeKind::Inside);
        if ease < 0.98 {
            return; // mientras crece, solo el fondo (después aparece el contenido)
        }
        crate::alto::reveal(&ctx, Id::unique("capyro-studio-anim"), panel, radius, open, BG_PANEL, accent);

        let x0 = panel.left() + 24.0;
        let x1 = panel.right() - 24.0;
        let top = panel.top();

        /* --- encabezado --- */
        let ic = pos2(x0 + 14.0, top + 32.0);
        painter.circle_filled(ic, 16.0, accent.gamma_multiply(0.16));
        icons::sliders(&painter, ic, 17.0, accent);
        painter.text(pos2(x0 + 40.0, top + 24.0), Align2::LEFT_CENTER, "Capyro Studio", bold(16.0), TEXT);
        let name = st.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        painter.text(
            pos2(x0 + 40.0, top + 42.0),
            Align2::LEFT_CENTER,
            fit_text(&painter, &name, regular(11.0), (x1 - x0) - 120.0),
            regular(11.0),
            TEXT_FAINT,
        );
        let close_c = pos2(x1 - 6.0, top + 30.0);
        let close = Self::icon_button(ui, "studio-close", close_c, 28.0);
        icons::close(&painter, close_c, 12.0 * close.scale(), close.tint(ICON_DIM, Color32::WHITE));
        if close.resp.clicked() {
            st.try_close(now);
        }
        close.resp.on_hover_text("Cerrar (Esc)");

        /* --- pie: estado y botones --- */
        let foot_h = 62.0;
        let foot_y = panel.bottom() - foot_h / 2.0;
        painter.line_segment(
            [pos2(panel.left() + 1.0, panel.bottom() - foot_h), pos2(panel.right() - 1.0, panel.bottom() - foot_h)],
            Stroke::new(1.0, Color32::from_white_alpha(14)),
        );
        let dirty = st.dirty();
        let save_w = pill_w(ui, "Guardar cambios") + 8.0;
        let save_r = Rect::from_min_max(pos2(x1 - save_w, foot_y - 15.0), pos2(x1, foot_y + 15.0));
        let mut do_save = pill(ui, "save", save_r, "Guardar cambios", true, 0.0, accent, dirty);
        if dirty {
            ui.ctx().input_mut(|i| {
                if i.consume_key(Modifiers::COMMAND, Key::S) {
                    do_save = true;
                }
            });
        }
        let undo_w = pill_w(ui, "Deshacer");
        let undo_r = Rect::from_min_max(pos2(save_r.left() - 8.0 - undo_w, foot_y - 15.0), pos2(save_r.left() - 8.0, foot_y + 15.0));
        if pill(ui, "undo", undo_r, "Deshacer", false, 0.0, accent, dirty) {
            st.undo();
            st.msg = None;
        }
        // a la izquierda: el último mensaje, o si hay cambios sin guardar
        let status_w = undo_r.left() - x0 - 16.0;
        let msg = st.msg.as_ref().filter(|(_, error, at)| *error || now - at < 6.0).cloned();
        match msg {
            Some((text, error, _)) => {
                let col = if error { ERROR } else { TEXT_DIM };
                if !error {
                    icons::check(&painter, pos2(x0 + 6.0, foot_y), 12.0, accent, 1.8);
                }
                let tx = if error { x0 } else { x0 + 18.0 };
                painter.text(pos2(tx, foot_y), Align2::LEFT_CENTER, fit_text(&painter, &text, regular(11.5), status_w), regular(11.5), col);
            }
            None if dirty => {
                painter.circle_filled(pos2(x0 + 4.0, foot_y), 4.0, accent);
                painter.text(pos2(x0 + 16.0, foot_y), Align2::LEFT_CENTER, "Cambios sin guardar", regular(11.5), TEXT_DIM);
            }
            None => {
                painter.text(pos2(x0, foot_y), Align2::LEFT_CENTER, "Doble clic en una banda: 0 dB · Ctrl+S: guardar", regular(10.5), TEXT_FAINT);
            }
        }

        /* --- el contenido (con scroll si no entra) --- */
        let body = Rect::from_min_max(pos2(panel.left() + 1.0, top + 64.0), pos2(panel.right() - 1.0, panel.bottom() - foot_h - 1.0));
        let mut body_ui = ui.new_child(UiBuilder::new().max_rect(body));
        body_ui.set_clip_rect(body.intersect(ui.clip_rect()));
        let mut scroll = egui::ScrollArea::vertical().id_salt("capyro-studio-scroll").auto_shrink([false, false]);
        if std::mem::take(&mut st.fresh) {
            scroll = scroll.vertical_scroll_offset(0.0);
        }
        scroll.show(&mut body_ui, |ui| {
                // margen interno igual que el del encabezado
                egui::Frame::NONE.inner_margin(egui::Margin { left: 23, right: 23, top: 8, bottom: 16 }).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    st.info_section(ui, &ctx, accent, now);
                    ui.add_space(18.0);
                    sep(ui);
                    ui.add_space(14.0);
                    st.preview_section(ui, core, accent, now);
                    ui.add_space(18.0);
                    sep(ui);
                    ui.add_space(14.0);
                    st.eq_section(ui, core, accent);
                });
            });

        if do_save && dirty {
            st.save(core, &ctx, now);
        }
        if !typing && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            st.try_close(now);
        }
    }
}

/// Línea separadora a lo ancho.
fn sep(ui: &Ui) {
    let r = ui.max_rect();
    let y = ui.cursor().top();
    ui.painter().line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(1.0, Color32::from_white_alpha(12)));
}

/// Botón grande de Capyro Studio, al lado de las pestañas de la burbuja.
/// Devuelve true si se tocó.
pub(super) fn studio_button(ui: &mut Ui, id: &str, r: Rect, accent: Color32, open: bool) -> bool {
    let a = animate_button(ui, Id::unique(id), r, true);
    let painter = ui.painter().clone();
    let bg = Color32::from_white_alpha((6.0 + 10.0 * a.hover) as u8).lerp_to_gamma(accent.gamma_multiply(0.3), if open { 1.0 } else { 0.0 });
    painter.rect_filled(r, 9.0, bg);
    painter.rect_stroke(r, 9.0, Stroke::new(1.0, accent.gamma_multiply(0.5)), StrokeKind::Inside);
    let col = accent.lerp_to_gamma(Color32::WHITE, a.hover * 0.3);
    let cy = r.center().y;
    crate::icons::sliders(&painter, Pos2::new(r.left() + 20.0, cy), 14.0 * a.scale(), col);
    painter.text(Pos2::new(r.left() + 36.0, cy), Align2::LEFT_CENTER, "Capyro Studio", egui::FontId::proportional(12.5), col);
    if open {
        painter.text(Pos2::new(r.right() - 12.0, cy), Align2::RIGHT_CENTER, "abierto", egui::FontId::proportional(10.0), TEXT_DIM);
    }
    a.resp.clicked()
}

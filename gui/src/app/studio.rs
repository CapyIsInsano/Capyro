//! Capyro Studio: un panel que se pone encima de la ventana para editar el
//! audio o el video que está sonando: la portada, las etiquetas, una vista
//! previa de un tramo y el ecualizador avanzado de 15 bandas.
//!
//! Solo existe mientras está abierto (`CapyroApp::studio` es None si no): al
//! cerrarlo se suelta todo, también las imágenes, y no suma memoria.
//!
//! Respeta el modo de rendimiento: en **Bajo** aparece y se va al toque, sin
//! sombras, brillos ni curva; en **Medio**, con su animación, la sombra y la
//! curva del ecualizador; en **Alto**, además, la luz del color de la portada,
//! el brillo de la curva y el panel que se descubre con una línea de luz.
//!
//! Con video: las etiquetas se guardan en todos los formatos; la portada, en
//! MP4/M4V (como pista de imagen) y MKV (como adjunto). Los subtítulos, los
//! capítulos y las pistas se copian tal cual. Guardar reescribe el archivo
//! entero, así que corre en otro hilo (un video grande tarda unos segundos).

mod cover; // la portada: elegir, quitar, volver a la original
mod eq; // ecualizador avanzado de 15 bandas
mod form; // título, artista y el resto de las etiquetas
mod preview; // vista previa de un tramo
mod save; // guardar en otro hilo y volver a abrir

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{
    self, epaint::RectShape, pos2, vec2, Align2, Color32, ColorImage, Context, Id, Pos2, Rect, Sense, Shape,
    Stroke, StrokeKind, TextureHandle, TextureOptions, Ui,
};

use super::overlay::{self, Status};
use super::widgets::*;
use super::{CapyroApp, FRAME_SMOOTH};
use crate::ffi::{self, AudioFx, Core, CoreError, CoverEdit, MediaKind, Rgba, State, Tags, EQ15_BANDS};
use crate::ui::icons;
use crate::ui::theme::*;



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

/// Un guardado en curso (el archivo se reescribe en otro hilo).
struct SaveJob {
    rx: Receiver<Result<(), CoreError>>,
    /// Lo que se mandó a guardar (si se sigue editando, sigue "sin guardar").
    tags: Tags,
    cover_edit: CoverEdit,
    /// Para volver a abrirlo donde estaba.
    open_here: bool,
    pos: f64,
    was_playing: bool,
    index: Option<usize>,
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
    /// Se eligió otro perfil del ecualizador en este cuadro (suena el efecto).
    preset_changed: bool,
    /// Es un video (cambia el dibujo sin portada y el aviso al guardar).
    video: bool,
    /// La duración (mientras se guarda el archivo está cerrado y no se sabe).
    duration: f64,
    /// Disco de vinilo: hacia dónde apunta (None = apagado). Lo pone el panel en cada cuadro.
    vinyl: Option<f32>,
    /// Guardando (mientras tanto no se edita ni se cierra).
    saving: Option<SaveJob>,
}

impl Studio {
    /// Abre el editor para el archivo que está sonando. Error = texto para mostrar.
    fn open(core: &Core, ctx: &Context) -> Result<Studio, String> {
        let path = core.current_file();
        core.playlist_current().ok_or("Abrí un audio o un video primero")?;
        if path.is_empty() {
            return Err("Abrí un audio o un video primero".to_owned());
        }
        let video = core.media_kind() == MediaKind::Video;
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
            preset_changed: false,
            video,
            duration,
            vinyl: None,
            saving: None,
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








    /// Cerrar: con cambios sin guardar, el primer intento solo avisa.
    fn try_close(&mut self, now: f64) {
        if self.saving.is_some() {
            return; // primero que termine de guardar
        }
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
/* el panel                                                                 */
/* ------------------------------------------------------------------------ */

impl CapyroApp {
    /// Abre Capyro Studio (o avisa por qué no se puede).
    pub(crate) fn open_studio(&mut self, ctx: &egui::Context) {
        if self.studio.as_ref().is_some_and(|s| s.open) || self.converter_open() {
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
    pub(crate) fn studio_open(&self) -> bool {
        self.studio.as_ref().is_some_and(|s| s.open)
    }

    /// ¿Hay un panel (Studio o Converter) encima de todo? Los atajos que
    /// abren cosas o cambian de archivo no andan.
    pub(crate) fn panel_open(&self) -> bool {
        self.studio_open() || self.converter_open()
    }

    /// Dibuja Capyro Studio encima de todo (si está abierto o cerrándose).
    pub(super) fn studio_overlay(&mut self, ui: &mut Ui, full: Rect) {
        let ctx = ui.ctx().clone();
        let open = self.studio_open();
        let t = overlay::anim(&ctx, "capyro-studio-anim", open);
        if self.studio.is_none() {
            return;
        }
        if !open && t <= 0.001 {
            self.studio = None; // cerrado: se suelta todo (imágenes incluidas)
            return;
        }
        let accent = self.cfg.accent;
        let now = ctx.input(|i| i.time);
        let core = &self.core;
        let vinyl = self.cfg.vinyl.then_some(self.vinyl_angle);
        let Some(st) = self.studio.as_mut() else { return };
        st.vinyl = vinyl;
        st.poll_pick(&ctx, now);
        st.poll_save(core, &ctx, now);
        st.tick_preview(core);
        if st.preview {
            ctx.request_repaint_after(crate::system::perf::ambient());
        }

        let name = st.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let head = overlay::Head { title: "Capyro Studio", subtitle: &name, icon: icons::sliders };
        let Some(ov) = overlay::begin(ui, full, "capyro-studio-anim", open, t, accent, &head) else { return };
        if ov.close {
            st.try_close(now);
        }

        /* --- pie: estado y botones --- */
        let (x0, x1, foot_y) = (ov.x0, ov.x1, ov.foot_y);
        let saving = st.saving.is_some();
        let dirty = st.dirty() && !saving;
        let save_w = pill_w(ui, "Guardar cambios") + 8.0;
        let save_r = Rect::from_min_max(pos2(x1 - save_w, foot_y - 15.0), pos2(x1, foot_y + 15.0));
        let mut do_save = pill(ui, "save", save_r, if saving { "Guardando…" } else { "Guardar cambios" }, true, 0.0, accent, dirty);
        if dirty && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            do_save = true;
        }
        let undo_w = pill_w(ui, "Deshacer");
        let undo_r = Rect::from_min_max(pos2(save_r.left() - 8.0 - undo_w, foot_y - 15.0), pos2(save_r.left() - 8.0, foot_y + 15.0));
        if pill(ui, "undo", undo_r, "Deshacer", false, 0.0, accent, dirty) {
            st.undo();
            st.msg = None;
        }
        // a la izquierda: guardando, el último mensaje o si hay cambios sin guardar
        let status_w = undo_r.left() - x0 - 16.0;
        let msg = st.msg.as_ref().filter(|(_, error, at)| *error || now - at < 6.0).cloned();
        let status = if saving {
            ctx.request_repaint_after(FRAME_SMOOTH);
            Status::Busy(if st.video { "Guardando… un video grande tarda unos segundos" } else { "Guardando…" })
        } else {
            match &msg {
                Some((text, true, _)) => Status::Error(text),
                Some((text, false, _)) => Status::Done(text),
                None if dirty => Status::Pending("Cambios sin guardar"),
                None => Status::Hint("Doble clic en una banda: 0 dB · Ctrl+S: guardar"),
            }
        };
        overlay::status(&ov, accent, now, status, status_w);

        /* --- el contenido (con scroll si no entra) --- */
        let fresh = std::mem::take(&mut st.fresh);
        overlay::body(ui, &ov, "capyro-studio-scroll", fresh, saving, |ui| {
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

        if do_save && dirty {
            st.save(core, &ctx);
        }
        if std::mem::take(&mut st.preset_changed) && self.cfg.click_sound {
            core.sfx_play(super::SFX_EFFECT);
            self.sfx_frame.set(self.last_frame);
        }
    }
}

//! Capyro Converter: un panel encima de la ventana (como Capyro Studio) para
//! pasar un archivo a otro formato: video a audio, audio a video con la
//! portada, achicar un video, sacar el audio tal cual, emparejar el volumen,
//! recortar…
//!
//! Arriba, el archivo (el que está sonando, otro elegido o uno arrastrado a
//! la ventana); después los perfiles listos (Alta calidad, Tamaño reducido,
//! Compatibilidad universal, Música sin pérdida, Video para celulares,
//! Extraer audio, Crear video con portada); abajo, las funciones avanzadas
//! (formato, códecs, calidad, resolución, cuadros por segundo, kbps,
//! canales, recorte, GPU) y dónde se guarda.
//!
//! La conversión corre en otro hilo: el panel se puede cerrar y seguir
//! escuchando música; el botón muestra cuánto va y al terminar avisa.
//!
//! Respeta el modo de rendimiento (ver `overlay.rs`).

mod input; // el archivo: elegir otro, qué tiene
mod options; // perfiles y funciones avanzadas
mod output; // dónde se guarda, el resultado y la imagen sin portada
pub(crate) mod profiles; // formatos, códecs y perfiles (sin interfaz)

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use eframe::egui::{self, pos2, vec2, Align2, Color32, Context, Id, Rect, Sense, Stroke, StrokeKind, Ui};

use super::overlay::{self, Status};
use super::widgets::*;
use super::{CapyroApp, FRAME_SMOOTH};
use crate::ffi::{self, ConvertDone, ConvertError, ConvertProgress, MediaProbe};
use crate::ui::icons;
use crate::ui::theme::*;
use profiles::{Plan, Profile};

/// Lo que vuelve de los selectores de archivos (corren en otro hilo).
enum Picked {
    Input(PathBuf),
    Image(PathBuf),
    Folder(PathBuf),
    Cancelled,
}

/// Una conversión corriendo en otro hilo.
struct Job {
    progress: Arc<ConvertProgress>,
    rx: Receiver<Result<ConvertDone, ConvertError>>,
    output: PathBuf,
    started: f64,
    /// Imagen armada para un audio sin portada (se borra al terminar).
    tmp_cover: Option<PathBuf>,
}

/// Cómo terminó la última conversión.
enum Finished {
    Ok { output: PathBuf, size: u64, secs: f64, done: ConvertDone },
    Failed(String),
    Cancelled,
}

/// Lo que el panel le pide a la app (se hace después de dibujarlo).
enum Request {
    Play(PathBuf),
    Enqueue(PathBuf),
    OpenFolder(PathBuf),
}

pub struct Converter {
    /// Visible (false = cerrado; si hay una conversión, sigue en segundo plano).
    open: bool,
    input: Option<PathBuf>,
    probe: Option<MediaProbe>,
    probe_err: Option<String>,
    /// Perfil elegido (None = "Personalizado": se tocó algo avanzado).
    profile: Option<Profile>,
    plan: Option<Plan>,
    advanced: bool,
    /// Carpeta de salida (None = la del archivo) y nombre sin extensión.
    out_dir: Option<PathBuf>,
    out_stem: String,
    /// Audio → video: imagen elegida a mano (None = la portada del archivo).
    cover_custom: Option<PathBuf>,
    picking: Option<Receiver<Picked>>,
    job: Option<Job>,
    finished: Option<Finished>,
    /// Recién abierto: el scroll arranca arriba.
    fresh: bool,
    requests: Vec<Request>,
}

impl Converter {
    fn new(input: Option<PathBuf>) -> Converter {
        let mut c = Converter {
            open: true,
            input: None,
            probe: None,
            probe_err: None,
            profile: None,
            plan: None,
            advanced: false,
            out_dir: None,
            out_stem: String::new(),
            cover_custom: None,
            picking: None,
            job: None,
            finished: None,
            fresh: true,
            requests: Vec::new(),
        };
        if let Some(p) = input {
            c.set_input(p);
        }
        c
    }

    /// Cambia el archivo de entrada: lo analiza y elige el perfil que más
    /// sirve (de un video, sacarle el audio; de un audio, video con portada).
    fn set_input(&mut self, path: PathBuf) {
        self.finished = None;
        self.cover_custom = None;
        self.out_stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "convertido".to_owned());
        match ffi::media_probe(&path) {
            Ok(p) if p.has_audio || p.has_video => {
                let first = if p.has_video { Profile::Universal } else { Profile::HighQuality };
                let keep = self.profile.filter(|pr| pr.unavailable(&p).is_none());
                let profile = keep.unwrap_or(first);
                self.plan = Some(profile.plan(&p, &has_encoder));
                self.profile = Some(profile);
                self.probe = Some(p);
                self.probe_err = None;
            }
            Ok(_) => {
                self.probe = None;
                self.plan = None;
                self.probe_err = Some("Este archivo no tiene audio ni video".to_owned());
            }
            Err(e) => {
                self.probe = None;
                self.plan = None;
                self.probe_err = Some(format!("No pude leerlo: {}", e.0));
            }
        }
        self.input = Some(path);
    }

    fn running(&self) -> bool {
        self.job.is_some()
    }

    fn try_close(&mut self) {
        self.open = false;
    }

    /// ¿Terminó la conversión? Devuelve un aviso para cuando el panel está cerrado.
    fn poll_job(&mut self, now: f64) -> Option<(String, bool)> {
        let job = self.job.as_ref()?;
        let result = match job.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => Err(ConvertError::Failed("se cortó la conversión".to_owned())),
        };
        let job = self.job.take()?;
        if let Some(t) = &job.tmp_cover {
            let _ = std::fs::remove_file(t);
        }
        let name = job.output.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let (finished, notice) = match result {
            Ok(done) => {
                let size = std::fs::metadata(&job.output).map(|m| m.len()).unwrap_or(0);
                let notice = format!("Conversión lista: {name}");
                (Finished::Ok { output: job.output, size, secs: now - job.started, done }, (notice, false))
            }
            Err(ConvertError::Cancelled) => (Finished::Cancelled, ("Conversión cancelada".to_owned(), false)),
            Err(ConvertError::Failed(e)) => {
                let notice = format!("No se pudo convertir: {e}");
                (Finished::Failed(e), (notice, true))
            }
        };
        self.finished = Some(finished);
        Some(notice)
    }
}

/// ¿Hay codificador para este códec en este FFmpeg?
fn has_encoder(codec: &str) -> bool {
    ffi::has_encoder(codec, false)
}

impl CapyroApp {
    /// Abre Capyro Converter con el archivo que está sonando (o el panel con
    /// la conversión que esté corriendo).
    pub(crate) fn open_converter(&mut self, ctx: &egui::Context) {
        if self.studio_open() {
            return;
        }
        let current = Some(PathBuf::from(self.core.current_file())).filter(|p| !p.as_os_str().is_empty());
        match self.converter.as_mut() {
            Some(c) if c.open => return,
            Some(c) => {
                c.open = true;
                c.fresh = true;
                // sin nada corriendo, arranca con lo que suena ahora
                if !c.running() && current.is_some() && c.input != current {
                    if let Some(p) = current {
                        c.set_input(p);
                    }
                }
            }
            None => self.converter = Some(Converter::new(current)),
        }
        self.close_menus();
        ctx.request_repaint();
    }

    pub(crate) fn converter_open(&self) -> bool {
        self.converter.as_ref().is_some_and(|c| c.open)
    }

    /// Cuánto va la conversión (None si no hay ninguna).
    pub(crate) fn converter_progress(&self) -> Option<f32> {
        self.converter.as_ref().and_then(|c| c.job.as_ref()).map(|j| j.progress.fraction())
    }

    /// Un archivo arrastrado con el panel abierto: es el que se convierte.
    pub(crate) fn converter_take_file(&mut self, path: PathBuf) -> bool {
        match self.converter.as_mut() {
            Some(c) if c.open && !c.running() => {
                c.set_input(path);
                c.fresh = true;
                true
            }
            _ => false,
        }
    }

    /// Se llama en cada cuadro (también con el panel cerrado): ¿terminó?
    pub(crate) fn poll_converter(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let Some(c) = self.converter.as_mut() else { return };
        if c.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        let open = c.open;
        if let Some((text, error)) = c.poll_job(now) {
            if error {
                self.play_error();
            } else if text.starts_with("Conversión lista") {
                self.sfx(super::SFX_READY);
            }
            if !open {
                self.toast_for(ctx, text, error, 6.0);
            }
        }
    }

    /// Dibuja Capyro Converter encima de todo (si está abierto o cerrándose).
    pub(crate) fn converter_overlay(&mut self, ui: &mut Ui, full: Rect) {
        let ctx = ui.ctx().clone();
        let open = self.converter_open();
        let t = overlay::anim(&ctx, "capyro-converter-anim", open);
        let Some(cv) = self.converter.as_mut() else { return };
        if !open && t <= 0.001 {
            // cerrado y sin nada corriendo ni resultado para mostrar: se suelta todo
            if !cv.running() && cv.finished.is_none() {
                self.converter = None;
            }
            return;
        }
        let accent = self.cfg.accent;
        let now = ctx.input(|i| i.time);
        cv.poll_pick(now);

        let name = cv.input.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
        let subtitle = name.unwrap_or_else(|| "Elegí un archivo para convertir".to_owned());
        let head = overlay::Head { title: "Capyro Converter", subtitle: &subtitle, icon: icons::convert };
        let Some(ov) = overlay::begin(ui, full, "capyro-converter-anim", open, t, accent, &head) else { return };
        if ov.close {
            cv.try_close();
        }

        /* --- pie: estado y botones --- */
        let (x0, x1, foot_y) = (ov.x0, ov.x1, ov.foot_y);
        let running = cv.running();
        let ready = cv.plan.is_some() && cv.input.is_some() && !running;
        let main_label = if running { "Cancelar" } else { "Convertir" };
        let main_w = pill_w(ui, "Convertir") + 18.0;
        let main_r = Rect::from_min_max(pos2(x1 - main_w, foot_y - 15.0), pos2(x1, foot_y + 15.0));
        let main = pill(ui, "convert-main", main_r, main_label, !running, 0.0, accent, ready || running);
        let status_w = main_r.left() - x0 - 16.0;
        let status_text;
        let status = if let Some(job) = &cv.job {
            let f = job.progress.fraction();
            let el = now - job.started;
            let eta = if f > 0.02 && el > 1.5 { Some(el * (1.0 - f as f64) / f as f64) } else { None };
            status_text = match eta {
                Some(s) => format!("Convirtiendo… {:.0} % · faltan {}", f * 100.0, fmt_secs(s)),
                None => format!("Convirtiendo… {:.0} %", f * 100.0),
            };
            ctx.request_repaint_after(FRAME_SMOOTH);
            Status::Busy(&status_text)
        } else {
            match &cv.finished {
                Some(Finished::Ok { output, .. }) => {
                    status_text = format!("Listo: {}", output.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
                    Status::Done(&status_text)
                }
                Some(Finished::Failed(e)) => Status::Error(e),
                Some(Finished::Cancelled) => Status::Hint("Cancelado (no quedó ningún archivo a medias)"),
                None if cv.input.is_none() => Status::Hint("Elegí un archivo, o arrastralo a la ventana"),
                None => Status::Hint("Se puede cerrar el panel mientras convierte"),
            }
        };
        overlay::status(&ov, accent, now, status, status_w);
        if running {
            // barrita de avance a lo ancho, arriba del pie
            let f = cv.job.as_ref().map_or(0.0, |j| j.progress.fraction());
            let y = ov.panel.bottom() - overlay::FOOT_H;
            let bar = Rect::from_min_max(pos2(ov.panel.left() + 1.0, y - 1.0), pos2(ov.panel.right() - 1.0, y + 1.0));
            ov.painter.rect_filled(bar, 1.0, Color32::from_white_alpha(10));
            ov.painter.rect_filled(Rect::from_min_max(bar.min, pos2(bar.left() + bar.width() * f, bar.bottom())), 1.0, accent);
        }

        /* --- el contenido --- */
        let fresh = std::mem::take(&mut cv.fresh);
        overlay::body(ui, &ov, "capyro-converter-scroll", fresh, running, |ui| {
            cv.input_section(ui, &ctx, accent);
            if cv.plan.is_some() {
                ui.add_space(16.0);
                sep(ui);
                ui.add_space(12.0);
                cv.profiles_section(ui, accent);
                ui.add_space(14.0);
                cv.advanced_section(ui, &ctx, accent);
                ui.add_space(16.0);
                sep(ui);
                ui.add_space(12.0);
                cv.output_section(ui, &ctx, accent);
            }
            if cv.finished.is_some() {
                ui.add_space(14.0);
                cv.result_section(ui, accent);
            }
        });

        if main {
            if running {
                if let Some(j) = &cv.job {
                    j.progress.cancel();
                }
            } else {
                cv.start(&ctx, now, accent);
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)) && ready {
            cv.start(&ctx, now, accent);
        }

        // lo que pidió el panel (agregar a la lista, abrir la carpeta)
        let requests = std::mem::take(&mut cv.requests);
        for r in requests {
            match r {
                Request::Play(p) => {
                    if let Some(c) = self.converter.as_mut() {
                        c.open = false;
                    }
                    self.add_paths(&ctx, vec![p], true);
                }
                Request::Enqueue(p) => {
                    self.add_paths(&ctx, vec![p], false);
                    self.toast(&ctx, "Agregado a la lista", false);
                }
                Request::OpenFolder(p) => {
                    if let Err(e) = crate::system::paths::open_url(&p.to_string_lossy()) {
                        self.toast(&ctx, format!("No pude abrir la carpeta: {e}"), true);
                    }
                }
            }
        }
    }
}

/// "1 min 20 s", "35 s".
fn fmt_secs(s: f64) -> String {
    let s = s.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{} h {} min", s / 3600, (s / 60) % 60)
    } else if s >= 60 {
        format!("{} min {} s", s / 60, s % 60)
    } else {
        format!("{s} s")
    }
}

/// "12,4 MB", "820 KB".
fn fmt_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.2} GB", b / 1e9).replace('.', ",")
    } else if b >= 1e6 {
        format!("{:.1} MB", b / 1e6).replace('.', ",")
    } else {
        format!("{:.0} KB", (b / 1e3).max(1.0))
    }
}

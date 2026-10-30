//! Capyro Studio: una ventana aparte para editar las etiquetas del audio que
//! está sonando, el ecualizador de 15 bandas y una vista previa de un tramo.
//!
//! La ventana solo existe mientras está abierta (`CapyroApp::studio` es None
//! si no): al cerrarla se libera todo y no suma memoria a Capyro.
//! Por ahora trabaja sobre audio (los videos no).

use std::path::PathBuf;

use eframe::egui::{self, Align2, Color32, Id, Pos2, Rect, Stroke, Ui};

use super::{animate_button, CapyroApp};
use crate::ffi::{self, AudioFx, Core, CoreError, MediaKind, State, Tags, EQ15_BANDS};
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

pub struct Studio {
    /// Archivo que se edita (el que está sonando al abrir).
    file: PathBuf,
    /// Lo que hay en los campos.
    tags: Tags,
    /// Lo que tiene el archivo (para saber si hay cambios sin guardar).
    saved: Tags,
    /// Último mensaje: (texto, es error).
    msg: Option<(String, bool)>,
    /// Vista previa: tramo en segundos y si se repite.
    start: f64,
    end: f64,
    preview: bool,
    repeat_preview: bool,
    /// Ecualizador de 15 bandas (se aplica con set_fx).
    eq_on: bool,
    eq: [f32; EQ15_BANDS],
}

impl Studio {
    /// Abre el editor para el archivo que está sonando. Error = texto para mostrar.
    fn open(core: &Core) -> Result<Studio, String> {
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
        let fx = core.fx();
        let duration = core.duration().max(0.0);
        Ok(Studio {
            file,
            saved: tags.clone(),
            tags,
            msg: None,
            start: 0.0,
            end: duration,
            preview: false,
            repeat_preview: false,
            eq_on: fx.eq15_on,
            eq: fx.eq15,
        })
    }

    fn dirty(&self) -> bool {
        self.tags != self.saved
    }

    /// Guarda las etiquetas. Si el archivo es el que está sonando, lo suelta,
    /// lo reescribe y lo vuelve a abrir en el mismo punto (si sonaba, sigue).
    /// Si no está sonando, solo se reescribe y no se toca la reproducción.
    fn save(&mut self, core: &Core) {
        self.preview = false;
        let open_here = PathBuf::from(core.current_file()) == self.file;
        let pos = core.position();
        let was_playing = core.state() == State::Playing;
        let index = core.playlist_current();
        if open_here {
            core.release_file();
        }
        let result = ffi::write_tags(&self.file, &self.tags);
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
            (Ok(()), Ok(())) => {
                self.saved = self.tags.clone();
                self.msg = Some(("Etiquetas guardadas (la versión anterior quedó como .bak)".to_owned(), false));
            }
            (Err(e), _) => self.msg = Some((format!("No se pudieron guardar: {}", e.0), true)),
            (Ok(()), Err(e)) => self.msg = Some((format!("Se guardó, pero no pude reabrirlo: {}", e.0), true)),
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
    fn play_preview(&mut self, core: &Core) {
        if self.end <= self.start + 0.05 {
            self.msg = Some(("El fin tiene que estar después del inicio".to_owned(), true));
            return;
        }
        self.preview = true;
        core.seek(self.start);
        if core.state() != State::Playing {
            core.toggle();
        }
    }

    /// Vigila el tramo mientras suena (llamar en cada cuadro).
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

    fn ui(&mut self, ui: &mut Ui, core: &Core, accent: Color32) {
        let duration = core.duration().max(0.0);
        let name = self.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

        ui.add_space(4.0);
        ui.heading("Capyro Studio");
        ui.label(egui::RichText::new(name).weak());
        ui.separator();

        // --- etiquetas ---
        ui.label(egui::RichText::new("Etiquetas").strong());
        egui::Grid::new("studio-tags").num_columns(2).spacing([12.0, 6.0]).striped(false).show(ui, |ui| {
            let rows: [(&str, &mut String); 10] = [
                ("Nombre de la canción", &mut self.tags.title),
                ("Artista o intérprete", &mut self.tags.artist),
                ("Álbum al que pertenece", &mut self.tags.album),
                ("Artista principal del álbum", &mut self.tags.album_artist),
                ("Género musical", &mut self.tags.genre),
                ("Año de lanzamiento", &mut self.tags.year),
                ("Número de pista", &mut self.tags.track),
                ("Número de disco", &mut self.tags.disc),
                ("Compositor", &mut self.tags.composer),
                ("Comentarios", &mut self.tags.comment),
            ];
            for (label, value) in rows {
                ui.label(label);
                ui.add(egui::TextEdit::singleline(value).desired_width(340.0));
                ui.end_row();
            }
        });
        // WAV no guarda estas tres etiquetas (su formato no tiene dónde ponerlas)
        if self.file.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav")) {
            ui.label(
                egui::RichText::new("En WAV no se guardan: Artista principal del álbum, Número de disco y Compositor.")
                    .small()
                    .weak(),
            );
        }
        ui.horizontal(|ui| {
            let dirty = self.dirty();
            if ui.add_enabled(dirty, egui::Button::new("Guardar etiquetas")).clicked() {
                self.save(core);
            }
            if ui.add_enabled(dirty, egui::Button::new("Deshacer cambios")).clicked() {
                self.tags = self.saved.clone();
            }
            if dirty {
                ui.label(egui::RichText::new("Cambios sin guardar").color(accent));
            }
        });
        if let Some((text, error)) = &self.msg {
            let col = if *error { ERROR } else { TEXT_DIM };
            ui.label(egui::RichText::new(text.as_str()).color(col));
        }
        ui.add_space(8.0);
        ui.separator();

        // --- vista previa: tramo ---
        ui.label(egui::RichText::new("Vista previa del tramo").strong());
        if duration <= 0.0 {
            ui.label("La duración no es conocida: no se puede elegir el tramo.");
        } else {
            self.end = self.end.clamp(0.0, duration);
            self.start = self.start.clamp(0.0, duration);
            ui.add(egui::Slider::new(&mut self.start, 0.0..=duration).text("Inicio (s)").custom_formatter(|v, _| fmt_time(v)));
            ui.add(egui::Slider::new(&mut self.end, 0.0..=duration).text("Fin (s)").custom_formatter(|v, _| fmt_time(v)));
            let len = (self.end - self.start).max(0.0);
            ui.label(format!("Duración del tramo: {}", fmt_time(len)));
            ui.horizontal(|ui| {
                if ui.button("Inicio = posición").clicked() {
                    self.start = core.position().clamp(0.0, duration);
                }
                if ui.button("Fin = posición").clicked() {
                    self.end = core.position().clamp(0.0, duration);
                }
                if ui.button("Tramo completo").clicked() {
                    self.start = 0.0;
                    self.end = duration;
                }
            });
            ui.horizontal(|ui| {
                if self.preview {
                    if ui.button("Parar tramo").clicked() {
                        self.preview = false;
                    }
                } else if ui.button("Reproducir tramo").clicked() {
                    self.play_preview(core);
                }
                ui.checkbox(&mut self.repeat_preview, "Repetir tramo");
            });
        }
        ui.add_space(8.0);
        ui.separator();

        // --- ecualizador de 15 bandas ---
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Ecualizador de 15 bandas").strong());
            if ui.checkbox(&mut self.eq_on, "Activado").changed() {
                self.apply_eq(core);
            }
        });
        ui.horizontal_wrapped(|ui| {
            for (name, gains) in EQ_PRESETS {
                if ui.button(name).clicked() {
                    self.eq = gains;
                    self.eq_on = true;
                    self.apply_eq(core);
                }
            }
        });
        ui.add_space(4.0);
        let mut changed = false;
        ui.horizontal(|ui| {
            for (i, gain) in self.eq.iter_mut().enumerate() {
                ui.vertical(|ui| {
                    ui.set_width(34.0);
                    let slider = egui::Slider::new(gain, -12.0..=12.0).vertical().show_value(false);
                    if ui.add(slider).changed() {
                        changed = true;
                    }
                    ui.label(egui::RichText::new(BAND_LABELS[i]).small());
                    ui.label(egui::RichText::new(format!("{:+.0}", *gain)).small().weak());
                });
            }
        });
        if changed {
            self.eq_on = true;
            self.apply_eq(core);
        }
        ui.add_space(8.0);
    }
}

fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0);
    let total = s.round() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

impl CapyroApp {
    /// Abre Capyro Studio (o avisa por qué no se puede).
    pub(super) fn open_studio(&mut self, ctx: &egui::Context) {
        if self.studio.is_some() {
            return;
        }
        match Studio::open(&self.core) {
            Ok(st) => self.studio = Some(st),
            Err(e) => self.toast(ctx, e, true),
        }
    }

    /// Dibuja la ventana de Capyro Studio (si está abierta) y vigila la vista previa.
    pub(super) fn studio_window(&mut self, ctx: &egui::Context) {
        let accent = self.cfg.accent;
        let core = &self.core;
        let slot = &mut self.studio;
        let Some(st) = slot.as_mut() else { return };
        st.tick_preview(core);
        if st.preview {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        let mut close = false;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("capyro-studio"),
            egui::ViewportBuilder::default()
                .with_title("Capyro Studio")
                .with_inner_size([700.0, 780.0])
                .with_min_inner_size([520.0, 480.0]),
            |ctx, _class| {
                if ctx.input(|i| i.viewport().close_requested()) {
                    close = true;
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| st.ui(ui, core, accent));
                });
            },
        );
        if close {
            // al cerrar se suelta todo (no queda nada en memoria)
            *slot = None;
        }
    }
}

/// Botón grande de Capyro Studio, al lado de las pestañas de la burbuja.
/// Devuelve true si se tocó.
pub(super) fn studio_button(ui: &mut Ui, id: &str, r: Rect, accent: Color32, open: bool) -> bool {
    let a = animate_button(ui, Id::unique(id), r, true);
    let painter = ui.painter().clone();
    let bg = Color32::from_white_alpha((6.0 + 10.0 * a.hover) as u8).lerp_to_gamma(accent.gamma_multiply(0.3), if open { 1.0 } else { 0.0 });
    painter.rect_filled(r, 9.0, bg);
    painter.rect_stroke(r, 9.0, Stroke::new(1.0, accent.gamma_multiply(0.5)), egui::StrokeKind::Inside);
    let col = accent.lerp_to_gamma(Color32::WHITE, a.hover * 0.3);
    let cy = r.center().y;
    crate::icons::sliders(&painter, Pos2::new(r.left() + 20.0, cy), 14.0 * a.scale(), col);
    painter.text(Pos2::new(r.left() + 36.0, cy), Align2::LEFT_CENTER, "Capyro Studio", egui::FontId::proportional(12.5), col);
    if open {
        painter.text(Pos2::new(r.right() - 12.0, cy), Align2::RIGHT_CENTER, "abierto", egui::FontId::proportional(10.0), TEXT_DIM);
    }
    a.resp.clicked()
}

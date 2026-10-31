//! Guardar: reescribe el archivo en otro hilo (un video grande tarda) y,
//! si es el que suena, lo vuelve a abrir en el mismo punto.

use super::*;

impl Studio {
    /// Guarda etiquetas y portada. Si el archivo es el que está sonando, lo
    /// suelta, lo reescribe (en otro hilo) y lo vuelve a abrir en el mismo
    /// punto (si sonaba, sigue). Si no está sonando, solo se reescribe.
    pub(super) fn save(&mut self, core: &Core, ctx: &Context) {
        if self.saving.is_some() {
            return;
        }
        self.preview = false;
        self.close_armed = None;
        self.msg = None;
        let open_here = PathBuf::from(core.current_file()) == self.file;
        let job_tags = self.tags.clone();
        let job_cover = self.cover_edit.clone();
        let (tx, rx) = mpsc::channel();
        let job = SaveJob {
            rx,
            tags: job_tags.clone(),
            cover_edit: job_cover.clone(),
            open_here,
            pos: core.position(),
            was_playing: core.state() == State::Playing,
            index: core.playlist_current(),
        };
        if open_here {
            core.release_file();
        }
        let file = self.file.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(ffi::write_tags_cover(&file, &job_tags, &job_cover));
            ctx.request_repaint();
        });
        self.saving = Some(job);
    }

    /// ¿Terminó de guardar? Vuelve a abrir el archivo y avisa cómo salió.
    pub(super) fn poll_save(&mut self, core: &Core, ctx: &Context, now: f64) {
        let Some(job) = &self.saving else { return };
        let result = match job.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err(CoreError("se cortó el guardado".to_owned())),
        };
        let Some(job) = self.saving.take() else { return };
        // siempre se vuelve a abrir, haya salido bien o mal
        let reopened = if !job.open_here {
            Ok(())
        } else if let Some(i) = job.index {
            let r = core.playlist_select(i);
            if r.is_ok() {
                core.seek(job.pos);
                if job.was_playing {
                    core.toggle();
                }
            }
            r
        } else {
            Err(CoreError("no encuentro el archivo en la lista".to_owned()))
        };
        match (result, reopened) {
            (Ok(()), reopened) => {
                self.saved = job.tags;
                if job.cover_edit != CoverEdit::Keep {
                    // lo que quedó en el archivo es lo que se ve
                    self.cover = match &job.cover_edit {
                        CoverEdit::Set(_) => ffi::read_cover(&self.file, COVER_SHOW_SIDE)
                            .map(|img| Picture::new(ctx, "capyro-studio-cover", &img)),
                        _ => None,
                    };
                    if self.cover_edit == job.cover_edit {
                        self.cover_edit = CoverEdit::Keep;
                        self.new_cover = None;
                    }
                }
                match reopened {
                    Ok(()) => self.say(now, "Guardado · la versión anterior quedó como .bak", false),
                    Err(e) => self.say(now, format!("Se guardó, pero no pude volver a abrirlo: {}", e.0), true),
                }
            }
            (Err(e), _) => self.say(now, format!("No se pudo guardar: {}", e.0), true),
        }
    }
}

//! Lo que hace Capyro cuando apretás algo: atajos de teclado, abrir y
//! agregar archivos, volumen, velocidad, capturas y guardar la sesión.

use super::*;

impl CapyroApp {
    pub(crate) fn run_action(&mut self, ctx: &egui::Context, action: Action) {
        let loaded = self.core.state() != State::Idle;
        match action {
            Action::PlayPause => {
                if loaded {
                    self.core.toggle();
                } else {
                    self.toast(ctx, "Agrega un archivo con \"+ Agregar archivo(s)\" para empezar", false);
                }
            }
            Action::Stop => self.core.stop(),
            Action::SeekForward => self.core.seek_relative(self.cfg.seek_step),
            Action::SeekBackward => self.core.seek_relative(-self.cfg.seek_step),
            Action::VolumeUp | Action::VolumeDown => {
                let step = if action == Action::VolumeUp { 1.0 } else { -1.0 };
                self.change_volume(ctx, step * self.cfg.volume_step);
            }
            Action::Mute => self.toggle_mute(ctx),
            Action::NextFile => {
                if self.core.next().is_err() {
                    let msg = if self.core.playlist_len() == 0 {
                        "La lista está vacía"
                    } else {
                        "Es el último de la lista (activa repetir para dar la vuelta)"
                    };
                    self.toast(ctx, msg, false);
                }
            }
            Action::PrevFile => {
                let _ = self.core.prev();
            }
            Action::TogglePlaylist => self.show_playlist = !self.show_playlist,
            Action::Fullscreen => {
                let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fs));
            }
            Action::OpenFiles => self.open_file_dialog(ctx),
            Action::ToggleShuffle => self.toggle_shuffle(ctx),
            Action::CycleRepeat => self.cycle_repeat(ctx),
            Action::SpeedUp | Action::SpeedDown => {
                let cur = self.core.speed();
                let next = if action == Action::SpeedUp {
                    SPEEDS.iter().copied().find(|&v| v > cur + 1e-6).unwrap_or(SPEEDS[SPEEDS.len() - 1])
                } else {
                    SPEEDS.iter().rev().copied().find(|&v| v < cur - 1e-6).unwrap_or(SPEEDS[0])
                };
                self.set_speed(ctx, next);
            }
            Action::SpeedReset => self.set_speed(ctx, 1.0),
            Action::Screenshot => self.take_screenshot(ctx),
            Action::ToggleEffects => {
                let open = !self.show_fx;
                self.close_menus();
                self.show_fx = open;
            }
            Action::EffectsBypass => {
                let mut fx = self.core.fx();
                fx.enabled = !fx.enabled;
                self.core.set_fx(&fx);
                self.toast(ctx, if fx.enabled { "Efectos activados" } else { "Efectos apagados" }, false);
            }
            Action::ToggleBar => {
                if self.core.media_kind() == MediaKind::Video && self.cfg.hide_bar_button {
                    self.set_bar_hidden(!self.bar_hidden);
                } else if !self.cfg.hide_bar_button {
                    self.toast(ctx, "Activá \"Ocultar barra (videos)\" en ⚙ para usarlo", false);
                } else {
                    self.toast(ctx, "La barra se oculta solo con videos", false);
                }
            }
            Action::Quit => {
                // el atajo de salir cierra del todo (también con el segundo plano)
                self.quit_for_real = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    pub(crate) fn set_speed(&mut self, ctx: &egui::Context, speed: f64) {
        self.core.set_speed(speed);
        self.toast(ctx, format!("Velocidad {}", fmt_speed(self.core.speed())), false);
    }

    pub(crate) fn toggle_shuffle(&mut self, ctx: &egui::Context) {
        let on = !self.core.shuffle();
        self.core.set_shuffle(on);
        self.toast(ctx, if on { "Aleatorio activado" } else { "Aleatorio desactivado" }, false);
    }

    pub(crate) fn cycle_repeat(&mut self, ctx: &egui::Context) {
        let r = self.core.repeat().next();
        self.core.set_repeat(r);
        let msg = match r {
            Repeat::Off => "Sin repetir",
            Repeat::All => "Repetir lista",
            Repeat::One => "Repetir este archivo",
        };
        self.toast(ctx, msg, false);
    }

    /// Guarda el fotograma que se está viendo como PNG (en otro hilo).
    pub(crate) fn take_screenshot(&mut self, ctx: &egui::Context) {
        if self.core.media_kind() != MediaKind::Video {
            self.play_error();
            self.toast(ctx, "Las capturas son para video", true);
            return;
        }
        if self.shot.is_some() {
            return;
        }
        // en tamaño original (lo que se ve en pantalla puede estar achicado)
        let Some((rgba, w, h)) = self.core.capture_frame() else {
            self.toast(ctx, "Todavía no hay imagen para capturar", true);
            return;
        };
        self.sfx(SFX_PHOTO); // "clic" de cámara
        let dir = self.cfg.screenshot_dir.clone().unwrap_or_else(default_screenshot_dir);
        let stem = Path::new(&self.np.file)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "capyro".to_owned());
        let at = fmt_time(self.core.position()).replace(':', "-");
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("{stem}_{at}_{stamp}.png"));
        let (tx, rx) = mpsc::channel();
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            let res = std::fs::create_dir_all(&dir)
                .map_err(|e| e.to_string())
                .and_then(|_| {
                    image::save_buffer(&path, &rgba, w, h, image::ExtendedColorType::Rgba8)
                        .map_err(|e| e.to_string())
                })
                .map(|_| path);
            let _ = tx.send(res);
            ctx2.request_repaint();
        });
        self.shot = Some(rx);
    }

    pub(crate) fn poll_screenshot(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.shot else { return };
        match rx.try_recv() {
            Ok(Ok(path)) => {
                self.shot = None;
                let shown = crate::system::paths::tilde(&path);
                self.toast(ctx, format!("Captura guardada: {}", ellipsize(&shown, 60)), false);
            }
            Ok(Err(e)) => {
                self.shot = None;
                self.play_error();
                self.toast(ctx, format!("No se pudo guardar la captura: {e}"), true);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => self.shot = None,
        }
    }

    pub(crate) fn session_snapshot(&self) -> Session {
        // durante el tutorial se guarda la cola de antes, no la canción del tutorial
        if let Some(s) = self.tutorial_saved() {
            return Session {
                volume: s.volume,
                muted: self.core.muted(),
                speed: self.core.speed(),
                shuffle: self.core.shuffle(),
                repeat: s.repeat,
                show_remaining: self.show_remaining,
                playlist: s.playlist.clone(),
                current: s.current,
                position: s.position,
                fx: self.core.fx(),
                pitch: self.core.pitch(),
                seen_changes: self.changes_seen.clone(),
            };
        }
        Session {
            volume: self.core.volume(),
            muted: self.core.muted(),
            speed: self.core.speed(),
            shuffle: self.core.shuffle(),
            repeat: self.core.repeat(),
            show_remaining: self.show_remaining,
            playlist: (*self.np.queue).clone(),
            current: self.core.playlist_current(),
            position: self.core.position(),
            fx: self.core.fx(),
            pitch: self.core.pitch(),
            seen_changes: self.changes_seen.clone(),
        }
    }

    pub(crate) fn save_session(&self) {
        if !self.cfg.remember_session {
            return;
        }
        if let Err(e) = self.session_snapshot().save() {
            eprintln!("capyro: no pude guardar la sesión: {e}");
        }
    }

    pub(crate) fn change_volume(&mut self, ctx: &egui::Context, delta: f32) {
        let v = (self.core.volume() + delta).clamp(0.0, 1.0);
        self.core.set_volume(v);
        self.core.set_muted(false);
        self.toast(ctx, format!("Volumen {}%", (v * 100.0).round()), false);
    }

    pub(crate) fn toggle_mute(&mut self, ctx: &egui::Context) {
        let m = !self.core.muted();
        self.core.set_muted(m);
        self.toast(ctx, if m { "Silenciado" } else { "Sonido activado" }, false);
    }

    /// Rueda del mouse sobre el volumen: un paso por "muesca" (sin el
    /// suavizado de egui, que repartía una muesca en muchos cuadros) y solo
    /// en la primera pasada del cuadro (egui a veces repite la pasada).
    pub(crate) fn wheel_volume(&mut self, ctx: &egui::Context, hovered: bool) {
        if hovered && self.first_pass {
            let notches: f32 = ctx.input(|i| {
                i.events
                    .iter()
                    .map(|e| match e {
                        egui::Event::MouseWheel { unit, delta, .. } => match unit {
                            egui::MouseWheelUnit::Line => delta.y,
                            egui::MouseWheelUnit::Point => delta.y / 50.0, // touchpad
                            egui::MouseWheelUnit::Page => delta.y * 4.0,
                        },
                        _ => 0.0,
                    })
                    .sum()
            });
            self.wheel_acc += notches;
            let steps = self.wheel_acc.trunc();
            if steps != 0.0 {
                self.wheel_acc -= steps;
                self.change_volume(ctx, steps * self.cfg.wheel_step);
            }
        } else if !hovered {
            self.wheel_acc = 0.0;
        }
    }

    /// Cambia cómo se decodifica el video: se aplica al instante (el núcleo
    /// rearma el decodificador) y queda guardado en capyro.lua.
    pub(crate) fn set_decoding(&mut self, ctx: &egui::Context, mode: ffi::Hwdec) {
        self.cfg.decoding = mode;
        self.core.set_hwdec(mode);
        debug_assert_eq!(self.core.hwdec(), mode);
        self.hwdec_warned = 0; // si la GPU no anda, avisar de nuevo
        let msg = match mode {
            ffi::Hwdec::Auto => "Decodificación automática (GPU si se puede)",
            ffi::Hwdec::Gpu => "Decodificación por GPU",
            ffi::Hwdec::Cpu => "Decodificación por CPU",
        };
        self.save_config(ctx, msg.to_owned());
    }

    /// Con "GPU" elegido: si el video terminó yendo por CPU, avisar una vez.
    pub(crate) fn check_hwdec(&mut self, ctx: &egui::Context) {
        if self.cfg.decoding != ffi::Hwdec::Gpu {
            return;
        }
        let info = self.core.decode_info();
        if info.active && !info.hw && info.serial != self.hwdec_warned {
            self.hwdec_warned = info.serial;
            self.play_error();
            self.toast_for(ctx, format!("GPU no disponible: {}. Uso la CPU", info.note), true, 5.0);
        }
    }

    /// Cambia el modo de rendimiento (⚙ → Rendimiento) y lo guarda.
    pub(crate) fn set_performance(&mut self, ctx: &egui::Context, mode: crate::system::perf::Mode) {
        self.cfg.performance = mode;
        apply_performance(ctx, mode);
        // la luz arranca de cero; y si es Alto, que saque los colores de la
        // imagen que ya está (una carátula no manda cuadros nuevos)
        self.ambient = Default::default();
        if mode == crate::system::perf::Mode::Alto {
            self.video_serial = u64::MAX;
        }
        let msg = match mode {
            crate::system::perf::Mode::Bajo => "Rendimiento bajo: sin animaciones ni extras",
            crate::system::perf::Mode::Medio => "Rendimiento medio",
            crate::system::perf::Mode::Alto => "Rendimiento alto: iluminación, fondo animado y partículas",
        };
        self.save_config(ctx, msg.to_owned());
    }

    pub(crate) fn reload_config(&mut self, ctx: &egui::Context) {
        self.cfg = Config::load();
        theme::apply_visuals(ctx, self.cfg.accent);
        apply_performance(ctx, self.cfg.performance);
        self.core.set_auto_next(self.cfg.auto_next);
        self.core.set_hwdec(self.cfg.decoding);
        let src = self.cfg.source.clone();
        self.toast(ctx, format!("Config recargada ({src})"), false);
    }

    /// Agrega archivos a la lista. Si `play_now` (arrastrar y soltar) o si no
    /// había nada cargado, arranca el primero nuevo; si no, solo los encola.
    pub(crate) fn add_paths(&mut self, ctx: &egui::Context, paths: Vec<PathBuf>, play_now: bool) {
        let first_new = self.core.playlist_len();
        let was_idle = self.core.state() == State::Idle;
        let mut errors = Vec::new();
        // una carpeta (arrastrada o por línea de comandos) = sus archivos
        // compatibles, con las subcarpetas, en orden alfabético
        let mut files = Vec::new();
        for path in paths {
            if path.is_dir() {
                let found = crate::media::formats::files_in_dir(&path, &self.formats);
                if found.is_empty() {
                    errors.push(format!("{}: no tiene archivos que Capyro pueda abrir", file_name(&path.to_string_lossy())));
                }
                files.extend(found);
            } else {
                files.push(path);
            }
        }
        let paths = files;
        for path in &paths {
            if let Err(e) = self.core.playlist_add(path) {
                errors.push(format!("{}: {}", file_name(&path.to_string_lossy()), e.0));
            }
        }
        let added = self.core.playlist_len() - first_new;
        if added > 0 && (play_now || was_idle) {
            match self.core.playlist_select(first_new) {
                Ok(()) => {
                    if self.core.state() != State::Playing {
                        self.core.toggle();
                    }
                }
                Err(e) => errors.push(e.0),
            }
        }
        if let Some(first) = errors.first() {
            self.play_error();
            let more = if errors.len() > 1 {
                format!(" (+{} más)", errors.len() - 1)
            } else {
                String::new()
            };
            self.toast(ctx, format!("{first}{more}"), true);
        } else if added > 0 && !(play_now || was_idle) {
            let msg = if added == 1 {
                "1 archivo agregado a la lista".to_owned()
            } else {
                format!("{added} archivos agregados a la lista")
            };
            self.toast(ctx, msg, false);
        }
    }

    /// Abre el selector de archivos del sistema (portal de escritorio, o
    /// zenity/kdialog si no hay portal) sin congelar la interfaz.
    pub(crate) fn open_file_dialog(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() {
            return; // ya hay uno abierto
        }
        // El diálogo se crea acá y se espera en otro hilo.
        // Filtro armado con la lista del núcleo (más las variantes en mayúscula,
        // porque algunos selectores distinguen .MP4 de .mp4).
        let (video, audio) = &self.formats;
        // (con las otras extensiones del mismo formato: .aif y .aifc son AIFF)
        let with_upper = |list: &[String]| -> Vec<String> {
            list.iter()
                .flat_map(|e| std::iter::once(e.as_str()).chain(crate::media::formats::aliases(e).iter().copied()))
                .flat_map(|e| [e.to_owned(), e.to_uppercase()])
                .collect()
        };
        let all: Vec<String> = with_upper(video).into_iter().chain(with_upper(audio)).collect();
        let pending = rfd::AsyncFileDialog::new()
            .set_title("Agregar archivos a Capyro")
            .add_filter(format!("Audio y video ({})", self.formats_label(", ")), &all)
            .add_filter(format!("Video ({})", video.join(", ")), &with_upper(video))
            .add_filter(format!("Audio ({})", audio.join(", ")), &with_upper(audio))
            .pick_files();
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let files = pollster::block_on(pending)
                .unwrap_or_default()
                .into_iter()
                .map(|f| f.path().to_path_buf())
                .collect::<Vec<_>>();
            let _ = tx.send(files);
            ctx.request_repaint_after(FRAME_SMOOTH);
        });
        self.dialog = Some(rx);
    }

    /// "mp4, mov, mp3, wav, ogg"
    pub(crate) fn formats_label(&self, sep: &str) -> String {
        let (v, a) = &self.formats;
        v.iter().chain(a.iter()).cloned().collect::<Vec<_>>().join(sep)
    }

    pub(crate) fn poll_file_dialog(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.dialog else { return };
        match rx.try_recv() {
            Ok(files) => {
                self.dialog = None;
                if !files.is_empty() {
                    self.add_paths(ctx, files, false);
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.dialog = None;
                self.toast(ctx, "No se pudo abrir el selector de archivos", true);
            }
        }
    }

    pub(crate) fn handle_input(&mut self, ctx: &egui::Context) {
        // Archivos soltados sobre la ventana (una sola vez por frame).
        let dropped: Vec<PathBuf> = if !self.first_pass {
            Vec::new()
        } else {
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect())
        };
        if !dropped.is_empty() {
            // con Capyro Converter abierto, el archivo es el que se convierte
            if dropped.len() == 1 && self.converter_take_file(dropped[0].clone()) {
                ctx.request_repaint();
            } else {
                // con un panel abierto se agregan a la lista sin cambiar de canción
                let play_now = !self.panel_open();
                self.add_paths(ctx, dropped, play_now);
            }
        }

        // Editor de atajos esperando una tecla: se la lleva toda.
        if self.first_pass && self.capture_key(ctx) {
            return;
        }
        // Escribiendo (renombrar playlist), editando atajos o en el tutorial: no disparar nada.
        if self.tutorial.is_some() || self.show_keys || self.pl_rename.is_some() || ctx.egui_wants_keyboard_input() {
            return;
        }

        // Atajos definidos en capyro.lua (ya ordenados: más modificadores primero).
        let mut fired = Vec::new();
        ctx.input_mut(|i| {
            for b in &self.cfg.bindings {
                if i.consume_key(b.modifiers, b.key) {
                    fired.push(b.action);
                }
            }
        });
        // Escape siempre saca de pantalla completa.
        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        if fullscreen && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
        }
        for action in fired {
            // con Capyro Studio abierto, nada que abra otro menú o cambie de archivo
            let blocked = matches!(
                action,
                Action::TogglePlaylist
                    | Action::OpenFiles
                    | Action::ToggleEffects
                    | Action::NextFile
                    | Action::PrevFile
                    | Action::ToggleBar
            );
            if blocked && self.panel_open() {
                continue;
            }
            self.run_action(ctx, action);
        }

        // 0..9 saltan al 0 %..90 % (como en YouTube).
        const DIGITS: [Key; 10] = [
            Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4,
            Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9,
        ];
        let duration = self.core.duration();
        if duration > 0.0 {
            for (i, key) in DIGITS.iter().enumerate() {
                if ctx.input_mut(|inp| inp.consume_key(Modifiers::NONE, *key)) {
                    self.core.seek(duration * i as f64 / 10.0);
                }
            }
        }
    }

    /* ---------------------------------------------------------------- */
    /* piezas de la interfaz                                            */
    /* ---------------------------------------------------------------- */
}

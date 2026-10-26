//! La ventana de Capyro. Layout medido sobre el mockup (768×642):
//!
//!   ┌ header 46px ─ logo · Capyro / Just press play ─────── _ □ ✕ ┐
//!   │ área de video (flexible)                                     │
//!   │ ── barra de progreso ──────────────────────────────────────  │
//!   │ 00:00                                                  00:00 │
//!   │              ⏮   ⟲   (▶)   ⏭                                │
//!   └ ☰ Lista                                       🔊  cc  ⚙  ⛶  ┘

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Id, Key, Modifiers, Pos2, Rect, Response,
    ResizeDirection, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui,
    UiBuilder, Vec2, ViewportCommand,
};

use crate::config::{Action, Config};
use crate::ffi::{self, BuildInfo, Core, MediaKind, Repeat, State};
use crate::updater::{self, CommitInfo, FailKind, Outcome, Updater};

mod changes_menu;
mod fx_menu;
mod keys_editor;
mod lists;
use lists::PlPopup;
use crate::changes::Release;
use crate::playlists::Playlists;
use crate::session::Session;
use crate::viz::Visualizer;
use crate::icons;
use crate::theme::{self, *};

const LOGO_PNG: &[u8] = include_bytes!("../../assets/capyro_logo_128.png");
const ICON_PNG: &[u8] = include_bytes!("../../assets/capyro_logo_256.png");

/// Slots de efectos: 0..=2 capibara (logo), 3 error, 4 inicio, 5 actualización,
/// 6 clic de botón, 7 captura, 8 "lista para descargar".
const SOUNDS: [&[u8]; 9] = [
    include_bytes!("../../assets/sounds/capybara1.mp3"),
    include_bytes!("../../assets/sounds/capybara2.mp3"),
    include_bytes!("../../assets/sounds/capybara3.mp3"),
    include_bytes!("../../assets/sounds/error.mp3"),
    include_bytes!("../../assets/sounds/inicio.mp3"),
    include_bytes!("../../assets/sounds/actualizacion.mp3"),
    include_bytes!("../../assets/sounds/click.mp3"),
    include_bytes!("../../assets/sounds/photo.mp3"),
    include_bytes!("../../assets/sounds/ready.mp3"),
];
const SFX_CAPYBARAS: usize = 3;
const SFX_ERROR: usize = 3;
const SFX_START: usize = 4;
const SFX_UPDATE: usize = 5;
const SFX_CLICK: usize = 6;
const SFX_PHOTO: usize = 7;
const SFX_READY: usize = 8;
/// Con `CAPYRO_DEBUG_SFX=1` cada sonido se anota en la terminal (para probar
/// sin parlantes o ver cuál suena).
fn debug_sfx(slot: usize) {
    const NAMES: [&str; 9] = ["capibara1", "capibara2", "capibara3", "error", "inicio", "actualizacion", "click", "photo", "ready"];
    if std::env::var_os("CAPYRO_DEBUG_SFX").is_some() {
        eprintln!("capyro: sonido {}", NAMES.get(slot).copied().unwrap_or("?"));
    }
}

/// Marca en la memoria de egui: "se tocó un botón en este frame".
const CLICK_FLAG: &str = "capyro-button-clicked";

/// Arranque: la pantalla de "Cargando" dura al menos esto, y después la
/// interfaz entra animada en INTRO_TIME.
const SPLASH_TIME: f64 = 1.0;
const INTRO_TIME: f32 = 0.62;
/// Cuánto se ve el aviso con el parche recién instalado.
const BANNER_TIME: f64 = 9.0;

/// Distancias desde el borde inferior del área de video (según el mockup).
const TRACK_Y: f32 = 14.0;
const TIMES_Y: f32 = 34.0;
const CONTROLS_Y: f32 = 81.0;
const BOTTOM_ROW_Y: f32 = 129.0;
const BELOW_VIDEO: f32 = 142.0;

struct Toast {
    text: String,
    start: f64,
    until: f64,
    error: bool,
}

/// Estado del arranque (pantalla de carga + entrada de la interfaz).
struct Boot {
    enabled: bool,
    /// `ctx.time` y número de frame del primer frame dibujado.
    start: Option<f64>,
    frame0: u64,
    /// Ya se cargaron los sonidos y la sesión.
    loaded: bool,
    /// Desde cuándo se ve la interfaz (termina la carga).
    ready_at: Option<f64>,
    /// Ya sonó el inicio (y arrancó la reproducción si correspondía).
    announced: bool,
    /// Archivos por línea de comandos: arrancar a sonar al terminar de cargar.
    autoplay: bool,
}

/// Aviso grande con el parche que se acaba de instalar.
struct UpdateBanner {
    start: f64,
    until: f64,
    commits: Vec<CommitInfo>,
    total: usize,
}

fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Velocidades que ofrece el menú de ajustes (y entre las que saltan los atajos).
const SPEEDS: [f64; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

fn fmt_speed(s: f64) -> String {
    let t = format!("{s:.2}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    format!("{t}×")
}

/// Respuesta de un botón + sus animaciones (0..1, suavizadas en el tiempo).
struct Anim {
    resp: Response,
    hover: f32,
    press: f32,
}

impl Anim {
    /// Escala para el ícono: crece un poco al pasar el mouse, se hunde al clic.
    fn scale(&self) -> f32 {
        1.0 + 0.08 * self.hover - 0.14 * self.press
    }
    fn tint(&self, base: Color32, hot: Color32) -> Color32 {
        base.lerp_to_gamma(hot, self.hover)
    }
}

fn animate_button(ui: &Ui, id: Id, rect: Rect, enabled: bool) -> Anim {
    let resp = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
    let ctx = ui.ctx();
    let hovered = enabled && resp.hovered();
    let hover = ctx.animate_bool_with_time(id.with("hover"), hovered, 0.14);
    if resp.clicked() {
        // el clic suena al final del frame (si el botón no hizo sonar otra cosa)
        ctx.data_mut(|d| d.insert_temp(Id::unique(CLICK_FLAG), true));
    }
    let press = ctx.animate_bool_with_time(id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.07);
    if hovered {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    Anim { resp, hover, press }
}

pub struct CapyroApp {
    core: Core,
    cfg: Config,
    logo: Option<TextureHandle>,
    show_playlist: bool,
    toast: Option<Toast>,
    last_title: String,
    /// Textura del frame actual (video o carátula) y su número de serie.
    video_tex: Option<TextureHandle>,
    video_serial: u64,
    video_aspect: f32,
    /// Toast que hay que mostrar en el primer frame (todavía no hay `ctx.time`).
    startup_notice: Option<String>,
    /// Diálogo de "Agregar archivos" abierto en otro hilo; llega el resultado acá.
    dialog: Option<Receiver<Vec<PathBuf>>>,
    /// Menú del engranaje abierto, y dónde está el engranaje (para anclarlo).
    show_settings: bool,
    gear_rect: Rect,
    /// Menú de información (versión y formatos), sobre "Lista".
    show_info: bool,
    info_rect: Rect,
    /// Formatos que acepta el núcleo, en minúscula: (video, audio).
    formats: (Vec<String>, Vec<String>),
    /// Formato cuyo resumen está abierto en el menú ⓘ (uno a la vez).
    info_format: Option<String>,
    /// Rueda sobre el volumen: fracciones de "muesca" acumuladas.
    wheel_acc: f32,
    /// Cuándo se tocó el logo (para el rebote) y último sonido que sonó.
    logo_click: Option<f64>,
    last_capy: usize,
    rng: u64,
    /// egui puede correr más de una pasada por frame con la MISMA entrada
    /// (pasadas de "medición"); lo que lee eventos crudos solo corre en la primera.
    last_frame: u64,
    first_pass: bool,
    /// Visualizador de audio (barras de fondo).
    viz: Visualizer,
    last_time: f64,
    /// Mostrar el tiempo que falta en vez de la duración total.
    show_remaining: bool,
    /// Guardado periódico de la sesión.
    last_session_save: f64,
    /// Captura de fotograma que se está guardando en otro hilo.
    shot: Option<Receiver<Result<PathBuf, String>>>,
    /// Menú de efectos de audio y dónde está su botón.
    show_fx: bool,
    fx_rect: Rect,
    /// Tono mientras se arrastra (se aplica al soltar).
    fx_pitch_drag: Option<f32>,
    /// Pantalla de carga y entrada animada.
    boot: Boot,
    /// El logo del header se esconde mientras el de la pantalla de carga vuela hacia él.
    hide_header_logo: bool,
    /// Actualizador (menú ⓘ) y de qué commit sale este binario.
    updater: Updater,
    build_info: BuildInfo,
    update_banner: Option<UpdateBanner>,
    /// Aviso "Hay una actualización" con los commits y el botón Instalar.
    show_offer: bool,
    /// Último decodificador por el que ya se avisó que la GPU no anduvo.
    hwdec_warned: u64,
    /// Frame en el que sonó el último efecto (para no tapar con el clic).
    sfx_frame: std::cell::Cell<u64>,
    /// Se está reiniciando para aplicar una actualización.
    restarting: bool,
    /// Playlists guardadas y qué pestaña de la Lista se ve (None = la Cola).
    playlists: Playlists,
    list_tab: Option<usize>,
    /// Menucito "Agregar a…" abierto.
    pl_popup: Option<PlPopup>,
    /// Renombrando: (playlist, texto, pedir foco).
    pl_rename: Option<(usize, String, bool)>,
    /// Tacho tocado una vez: (playlist, cuándo). El segundo toque borra.
    pl_delete_armed: Option<(usize, f64)>,
    /// Editor de atajos (⚙ → Editar configuración).
    show_keys: bool,
    keys_capture: Option<Action>,
    keys_adding: bool,
    /// Menú de novedades, su botón, lo que dice y la última versión que se vio.
    show_changes: bool,
    changes_rect: Rect,
    release: Option<Release>,
    changes_seen: String,
    /// Este proceso es el capyro instalado (no uno corriendo desde build/).
    installed: Option<updater::Install>,
}

fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((w, h, img.into_raw()))
}

pub fn window_icon() -> Option<egui::IconData> {
    let (width, height, rgba) = decode_png(ICON_PNG)?;
    Some(egui::IconData { rgba, width, height })
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}

fn ellipsize(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Carpeta de Imágenes del usuario (según xdg-user-dirs) + "/Capyro".
fn default_screenshot_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let config = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config"));
    let pictures = std::fs::read_to_string(config.join("user-dirs.dirs"))
        .ok()
        .and_then(|txt| {
            txt.lines().find_map(|l| {
                let v = l.trim().strip_prefix("XDG_PICTURES_DIR=")?;
                let v = v.trim_matches('"').replace("$HOME", &home.to_string_lossy());
                Some(PathBuf::from(v))
            })
        })
        .unwrap_or_else(|| home.join("Pictures"));
    pictures.join("Capyro")
}

fn fmt_time(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

impl CapyroApp {
    pub fn new(cc: &eframe::CreationContext<'_>, core: Core, cfg: Config) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        theme::apply_visuals(&cc.egui_ctx, cfg.accent);

        let logo = decode_png(LOGO_PNG).map(|(w, h, rgba)| {
            let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            cc.egui_ctx.load_texture("capyro-logo", img, TextureOptions::LINEAR)
        });

        core.set_volume(cfg.start_volume);
        core.set_auto_next(cfg.auto_next);
        core.set_speed(cfg.start_speed);
        core.set_shuffle(cfg.start_shuffle);
        core.set_repeat(cfg.start_repeat);
        core.set_hwdec(cfg.decoding);
        let startup_notice = core
            .audio_backend()
            .is_empty()
            .then(|| "No encontré dispositivo de audio: reproduzco sin sonido".to_owned());
        let splash = cfg.splash;
        // Recién actualizado (lo abrió el Capyro viejo al reiniciar): contarlo.
        let build_info = ffi::build_info();
        let startup_notice = match std::env::var_os("CAPYRO_UPDATED") {
            Some(_) if !build_info.commit.is_empty() => {
                // SAFETY: todavía no hay otros hilos leyendo el entorno.
                unsafe { std::env::remove_var("CAPYRO_UPDATED") };
                let short: String = build_info.commit.chars().take(7).collect();
                Some(format!("Capyro actualizado a {short} · {}", build_info.subject))
            }
            _ => startup_notice,
        };

        Self {
            core,
            cfg,
            logo,
            show_playlist: false,
            toast: None,
            last_title: String::new(),
            video_tex: None,
            video_serial: 0,
            video_aspect: 16.0 / 9.0,
            startup_notice,
            dialog: None,
            show_settings: false,
            gear_rect: Rect::NOTHING,
            show_info: false,
            info_rect: Rect::NOTHING,
            formats: (ffi::supported_video(), ffi::supported_audio()),
            info_format: None,
            wheel_acc: 0.0,
            logo_click: None,
            last_capy: usize::MAX,
            last_frame: u64::MAX,
            first_pass: true,
            viz: Visualizer::new(),
            last_time: 0.0,
            show_remaining: false,
            last_session_save: 0.0,
            shot: None,
            show_fx: false,
            fx_rect: Rect::NOTHING,
            fx_pitch_drag: None,
            boot: Boot {
                enabled: splash,
                start: None,
                frame0: 0,
                loaded: false,
                ready_at: None,
                announced: false,
                autoplay: true,
            },
            hide_header_logo: false,
            updater: Updater::new(),
            installed: updater::install_location(&build_info),
            playlists: Playlists::default(),
            list_tab: None,
            pl_popup: None,
            pl_rename: None,
            pl_delete_armed: None,
            show_keys: false,
            keys_capture: None,
            keys_adding: false,
            show_changes: false,
            changes_rect: Rect::NOTHING,
            release: crate::changes::current(),
            changes_seen: String::new(),
            build_info,
            update_banner: None,
            show_offer: false,
            hwdec_warned: 0,
            sfx_frame: std::cell::Cell::new(u64::MAX - 1),
            restarting: false,
            rng: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64 | 1)
                .unwrap_or(0x9E37_79B9_7F4A_7C15),
        }
    }

    /// La carga del arranque: sonidos (se decodifican en el núcleo) y la
    /// sesión anterior. Corre mientras se ve la pantalla de "Cargando".
    fn finish_loading(&mut self) {
        for (slot, data) in SOUNDS.iter().enumerate() {
            if let Err(e) = self.core.sfx_load(slot, data) {
                eprintln!("capyro: no se pudo cargar el sonido {slot}: {}", e.0);
            }
        }
        self.playlists = Playlists::load();

        // Sesión anterior: ajustes siempre; la lista solo si no se abrieron
        // archivos desde la línea de comandos.
        if !self.cfg.remember_session {
            return;
        }
        let Some(sess) = Session::load() else { return };
        self.changes_seen = sess.seen_changes.clone();
        let core = &self.core;
        core.set_volume(sess.volume);
        core.set_muted(sess.muted);
        core.set_speed(sess.speed);
        core.set_repeat(sess.repeat);
        core.set_fx(&sess.fx);
        core.set_pitch(sess.pitch);
        self.show_remaining = sess.show_remaining;
        core.set_shuffle(sess.shuffle);
        if core.playlist_len() > 0 || sess.playlist.is_empty() {
            return;
        }
        let mut restored = 0;
        let mut cur = None;
        for (i, p) in sess.playlist.iter().enumerate() {
            let path = Path::new(p);
            if path.exists() && core.playlist_add(path).is_ok() {
                if sess.current == Some(i) {
                    cur = Some(restored);
                }
                restored += 1;
            }
        }
        core.set_shuffle(sess.shuffle);
        if let Some(c) = cur {
            if core.playlist_select(c).is_ok() && sess.position > 3.0 && sess.position < core.duration() - 3.0 {
                core.seek(sess.position);
            }
        }
        if restored > 0 && self.startup_notice.is_none() {
            let pos = if cur.is_some() && sess.position > 3.0 {
                format!(" · seguís en {}", fmt_time(sess.position))
            } else {
                String::new()
            };
            let s = if restored == 1 { "" } else { "s" };
            self.startup_notice = Some(format!("Sesión restaurada: {restored} archivo{s}{pos}"));
        }
        // Al retomar no arranca a sonar solo: el usuario aprieta play.
        self.boot.autoplay = false;
    }

    /// Uno de los tres sonidos de capibara, al azar (sin repetir el anterior).
    fn play_capybara(&mut self) {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        let mut pick = (self.rng % SFX_CAPYBARAS as u64) as usize;
        if pick == self.last_capy {
            pick = (pick + 1) % SFX_CAPYBARAS;
        }
        self.last_capy = pick;
        self.sfx(pick);
    }

    fn play_error(&self) {
        self.sfx(SFX_ERROR);
    }

    /// Todos los sonidos pasan por acá: así el clic del botón no pisa al
    /// sonido propio de lo que se tocó (error, capibara, captura...).
    fn sfx(&self, slot: usize) {
        debug_sfx(slot);
        self.core.sfx_play(slot);
        self.sfx_frame.set(self.last_frame);
    }

    /// Final del frame: si se tocó un botón y nada sonó, suena el clic.
    fn play_click_if_needed(&self, ctx: &egui::Context) {
        let clicked = ctx.data_mut(|d| d.remove_temp::<bool>(Id::unique(CLICK_FLAG))).unwrap_or(false);
        if clicked && self.cfg.click_sound && self.sfx_frame.get() != self.last_frame {
            debug_sfx(SFX_CLICK);
            self.core.sfx_play(SFX_CLICK);
        }
    }

    fn toast(&mut self, ctx: &egui::Context, text: impl Into<String>, error: bool) {
        self.toast_for(ctx, text, error, 2.5);
    }

    fn toast_for(&mut self, ctx: &egui::Context, text: impl Into<String>, error: bool, secs: f64) {
        let now = ctx.input(|i| i.time);
        self.toast = Some(Toast { text: ellipsize(&text.into(), 84), start: now, until: now + secs, error });
    }

    /* ---------------------------------------------------------------- */
    /* actualizaciones                                                  */
    /* ---------------------------------------------------------------- */

    /// Botón "Buscar actualizaciones" del menú ⓘ. Primero busca y muestra qué
    /// commits hay; con `install` (botón "Instalar" del aviso) compila e instala.
    /// Si ya se instaló una, el mismo botón reinicia Capyro.
    fn start_update(&mut self, ctx: &egui::Context, install: bool) {
        if self.updater.is_running() {
            return;
        }
        if matches!(self.updater.last, Some(Outcome::Installed { .. })) {
            self.restart(ctx);
            return;
        }
        // ya se sabe qué hay: volver a mostrar el aviso
        if !install && matches!(self.updater.last, Some(Outcome::Available { .. })) {
            self.show_offer = true;
            return;
        }
        self.show_offer = false;
        match updater::Job::new(
            &self.build_info,
            self.installed.as_ref(),
            self.cfg.update_repo.as_deref(),
            self.cfg.update_branch.as_deref(),
        ) {
            Ok(mut job) => {
                job.install_now = install;
                let c = ctx.clone();
                self.updater.start(job, move || c.request_repaint());
            }
            Err(message) => {
                self.play_error();
                self.toast_for(ctx, message.clone(), true, 4.0);
                self.updater.last = Some(Outcome::Failed { kind: FailKind::Repo, message });
            }
        }
    }

    /// Resultado del actualizador (una vez por búsqueda).
    fn poll_update(&mut self, ctx: &egui::Context) {
        if self.updater.is_running() {
            // spinner del menú y puntito del botón ⓘ
            ctx.request_repaint_after(Duration::from_millis(40));
        }
        let Some(out) = self.updater.poll() else { return };
        let now = ctx.input(|i| i.time);
        match out {
            Outcome::UpToDate { hash } => {
                self.toast_for(ctx, format!("Ya tenés la última versión ({hash})"), false, 3.0);
            }
            Outcome::Ahead => {
                self.toast_for(ctx, "Tu Capyro es más nuevo que el del repo", false, 3.0);
            }
            Outcome::Available { .. } => {
                // el aviso con la lista de commits y "Instalar"
                self.show_offer = true;
            }
            Outcome::Installed { commits, total } => {
                self.sfx(SFX_UPDATE);
                self.update_banner = Some(UpdateBanner { start: now, until: now + BANNER_TIME, commits, total });
            }
            Outcome::Failed { message, .. } => {
                self.play_error();
                self.toast_for(ctx, message, true, 5.0);
            }
        }
    }

    /// Guarda la sesión, abre el Capyro recién instalado y cierra este.
    fn restart(&mut self, ctx: &egui::Context) {
        self.save_session();
        let bindir = match &self.installed {
            Some(i) => i.bindir.clone(),
            None => std::path::PathBuf::from(&self.build_info.bindir),
        };
        match updater::relaunch(&bindir) {
            Ok(()) => {
                self.restarting = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            Err(e) => {
                self.play_error();
                self.toast_for(ctx, format!("No pude reiniciar Capyro: {e}"), true, 4.0);
            }
        }
    }

    /* ---------------------------------------------------------------- */
    /* acciones                                                         */
    /* ---------------------------------------------------------------- */

    fn run_action(&mut self, ctx: &egui::Context, action: Action) {
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
            Action::Quit => ctx.send_viewport_cmd(ViewportCommand::Close),
        }
    }

    /// Los menús emergentes son de a uno.
    fn close_menus(&mut self) {
        self.show_settings = false;
        self.show_info = false;
        self.show_fx = false;
        self.show_changes = false;
    }

    fn set_speed(&mut self, ctx: &egui::Context, speed: f64) {
        self.core.set_speed(speed);
        self.toast(ctx, format!("Velocidad {}", fmt_speed(self.core.speed())), false);
    }

    fn toggle_shuffle(&mut self, ctx: &egui::Context) {
        let on = !self.core.shuffle();
        self.core.set_shuffle(on);
        self.toast(ctx, if on { "Aleatorio activado" } else { "Aleatorio desactivado" }, false);
    }

    fn cycle_repeat(&mut self, ctx: &egui::Context) {
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
    fn take_screenshot(&mut self, ctx: &egui::Context) {
        if self.core.media_kind() != MediaKind::Video {
            self.play_error();
            self.toast(ctx, "Las capturas son para video", true);
            return;
        }
        if self.shot.is_some() {
            return;
        }
        let Some((rgba, w, h)) = self.core.with_video_frame(|vf| (vf.rgba.to_vec(), vf.width as u32, vf.height as u32)) else {
            self.toast(ctx, "Todavía no hay imagen para capturar", true);
            return;
        };
        self.sfx(SFX_PHOTO); // "clic" de cámara
        let dir = self.cfg.screenshot_dir.clone().unwrap_or_else(default_screenshot_dir);
        let stem = Path::new(&self.core.current_file())
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

    fn poll_screenshot(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.shot else { return };
        match rx.try_recv() {
            Ok(Ok(path)) => {
                self.shot = None;
                let shown = match std::env::var_os("HOME") {
                    Some(h) => path.to_string_lossy().replacen(&*h.to_string_lossy(), "~", 1),
                    None => path.to_string_lossy().into_owned(),
                };
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

    fn session_snapshot(&self) -> Session {
        Session {
            volume: self.core.volume(),
            muted: self.core.muted(),
            speed: self.core.speed(),
            shuffle: self.core.shuffle(),
            repeat: self.core.repeat(),
            show_remaining: self.show_remaining,
            playlist: self.core.playlist(),
            current: self.core.playlist_current(),
            position: self.core.position(),
            fx: self.core.fx(),
            pitch: self.core.pitch(),
            seen_changes: self.changes_seen.clone(),
        }
    }

    fn save_session(&self) {
        if !self.cfg.remember_session {
            return;
        }
        if let Err(e) = self.session_snapshot().save() {
            eprintln!("capyro: no pude guardar la sesión: {e}");
        }
    }

    fn change_volume(&mut self, ctx: &egui::Context, delta: f32) {
        let v = (self.core.volume() + delta).clamp(0.0, 1.0);
        self.core.set_volume(v);
        self.core.set_muted(false);
        self.toast(ctx, format!("Volumen {}%", (v * 100.0).round()), false);
    }

    fn toggle_mute(&mut self, ctx: &egui::Context) {
        let m = !self.core.muted();
        self.core.set_muted(m);
        self.toast(ctx, if m { "Silenciado" } else { "Sonido activado" }, false);
    }

    /// Cambia cómo se decodifica el video: se aplica al instante (el núcleo
    /// rearma el decodificador) y queda guardado en capyro.lua.
    fn set_decoding(&mut self, ctx: &egui::Context, mode: ffi::Hwdec) {
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
    fn check_hwdec(&mut self, ctx: &egui::Context) {
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

    fn reload_config(&mut self, ctx: &egui::Context) {
        self.cfg = Config::load();
        theme::apply_visuals(ctx, self.cfg.accent);
        self.core.set_auto_next(self.cfg.auto_next);
        self.core.set_hwdec(self.cfg.decoding);
        let src = self.cfg.source.clone();
        self.toast(ctx, format!("Config recargada ({src})"), false);
    }

    /// Agrega archivos a la lista. Si `play_now` (arrastrar y soltar) o si no
    /// había nada cargado, arranca el primero nuevo; si no, solo los encola.
    fn add_paths(&mut self, ctx: &egui::Context, paths: Vec<PathBuf>, play_now: bool) {
        let first_new = self.core.playlist_len();
        let was_idle = self.core.state() == State::Idle;
        let mut errors = Vec::new();
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
    fn open_file_dialog(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() {
            return; // ya hay uno abierto
        }
        // El diálogo se crea acá y se espera en otro hilo.
        // Filtro armado con la lista del núcleo (más las variantes en mayúscula,
        // porque algunos selectores distinguen .MP4 de .mp4).
        let (video, audio) = &self.formats;
        let with_upper = |list: &[String]| -> Vec<String> {
            list.iter().flat_map(|e| [e.clone(), e.to_uppercase()]).collect()
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
            ctx.request_repaint();
        });
        self.dialog = Some(rx);
    }

    /// "mp4, mov, mp3, wav, ogg"
    fn formats_label(&self, sep: &str) -> String {
        let (v, a) = &self.formats;
        v.iter().chain(a.iter()).cloned().collect::<Vec<_>>().join(sep)
    }

    fn poll_file_dialog(&mut self, ctx: &egui::Context) {
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

    fn handle_input(&mut self, ctx: &egui::Context) {
        // Archivos soltados sobre la ventana (una sola vez por frame).
        let dropped: Vec<PathBuf> = if !self.first_pass {
            Vec::new()
        } else {
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect())
        };
        if !dropped.is_empty() {
            self.add_paths(ctx, dropped, true);
        }

        // Editor de atajos esperando una tecla: se la lleva toda.
        if self.first_pass && self.capture_key(ctx) {
            return;
        }
        // Escribiendo (renombrar playlist) o editando atajos: no disparar nada.
        if self.show_keys || self.pl_rename.is_some() || ctx.egui_wants_keyboard_input() {
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

    /// Sube a la GPU el frame nuevo del núcleo (si cambió desde el último).
    fn sync_video_texture(&mut self, ctx: &egui::Context) {
        let last = self.video_serial;
        let fresh = self.core.with_video_frame(|vf| {
            if vf.serial == last {
                return None;
            }
            let img = egui::ColorImage::from_rgba_unmultiplied([vf.width, vf.height], vf.rgba);
            Some((img, vf.serial, vf.aspect))
        });
        match fresh {
            None => self.video_tex = None, // no hay frame (nada cargado o mp3 sin carátula)
            Some(None) => {}               // mismo frame que ya tenemos
            Some(Some((img, serial, aspect))) => {
                match &mut self.video_tex {
                    Some(tex) => tex.set(img, TextureOptions::LINEAR),
                    None => {
                        self.video_tex = Some(ctx.load_texture("capyro-video", img, TextureOptions::LINEAR));
                    }
                }
                self.video_serial = serial;
                self.video_aspect = aspect;
            }
        }
    }

    /// Botón redondo de ícono: fondo que aparece y crece al pasar el mouse,
    /// y un "pulso" más fuerte mientras se mantiene apretado.
    fn icon_button(ui: &mut Ui, id: &str, center: Pos2, size: f32) -> Anim {
        let rect = Rect::from_center_size(center, Vec2::splat(size));
        let a = animate_button(ui, Id::unique(("capyro-btn", id)), rect, true);
        let t = a.hover.max(a.press);
        if t > 0.001 {
            let r = size * 0.5 * (0.7 + 0.3 * a.hover) * (1.0 - 0.06 * a.press);
            let alpha = (14.0 * a.hover + 16.0 * a.press) as u8;
            ui.painter().circle_filled(center, r, Color32::from_white_alpha(alpha));
        }
        a
    }

    fn header(&mut self, ui: &mut Ui, rect: Rect, maximized: bool) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();

        // Zona para arrastrar la ventana (todo el header menos los botones).
        let drag_rect = Rect::from_min_max(rect.min, pos2(rect.right() - 80.0, rect.bottom()));
        let drag = ui.interact(drag_rect, Id::unique("capyro-titlebar"), Sense::click_and_drag());
        if drag.double_clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        } else if drag.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        // Logo: al tocarlo suena un capibara y rebota.
        let logo_rect = Rect::from_min_size(rect.min, vec2(46.0, 46.0));
        let logo_btn = animate_button(ui, Id::unique("capyro-logo"), logo_rect, true);
        let now = ctx.input(|i| i.time);
        if logo_btn.resp.clicked() {
            self.play_capybara();
            self.logo_click = Some(now);
        }
        let mut bounce = 0.0;
        if let Some(t0) = self.logo_click {
            let dt = (now - t0) as f32;
            if dt < 0.7 {
                bounce = (-dt * 7.0).exp() * (dt * 22.0).sin();
                ctx.request_repaint();
            } else {
                self.logo_click = None;
            }
        }
        let logo_scale = 1.0 + 0.05 * logo_btn.hover - 0.08 * logo_btn.press + 0.14 * bounce;
        if let Some(logo) = self.logo.as_ref().filter(|_| !self.hide_header_logo) {
            painter.image(
                logo.id(),
                Rect::from_center_size(logo_rect.center(), logo_rect.size() * logo_scale),
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let tx = logo_rect.right() + 8.0;
        painter.text(pos2(tx, rect.top() + 14.0), Align2::LEFT_CENTER, "Capyro", bold(17.0), TEXT);
        painter.text(
            pos2(tx, rect.top() + 36.0),
            Align2::LEFT_CENTER,
            "Just press play.",
            regular(12.0),
            TEXT_DIM,
        );

        let cy = rect.center().y;
        let close_c = pos2(rect.right() - 8.0, cy);
        let max_c = close_c - vec2(24.0, 0.0);
        let min_c = max_c - vec2(24.0, 0.0);

        let min = Self::icon_button(ui, "minimize", min_c, 24.0);
        icons::win_minimize(&painter, min_c, 14.0 * min.scale(), min.tint(ICON, Color32::WHITE));
        if min.resp.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }

        let max = Self::icon_button(ui, "maximize", max_c, 24.0);
        icons::win_maximize(&painter, max_c, 15.0 * max.scale(), max.tint(ICON, Color32::WHITE), maximized);
        if max.resp.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }

        // Cerrar: el fondo se vuelve rojo al pasar el mouse.
        let close_rect = Rect::from_center_size(close_c, Vec2::splat(24.0));
        let close = animate_button(ui, Id::unique(("capyro-btn", "close")), close_rect, true);
        if close.hover > 0.001 {
            painter.circle_filled(
                close_c,
                12.0 * (0.7 + 0.3 * close.hover),
                ERROR.gamma_multiply(0.9 * close.hover),
            );
        }
        icons::close(&painter, close_c, 13.0 * close.scale(), close.tint(ICON, Color32::WHITE));
        if close.resp.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    fn video_area(&mut self, ui: &mut Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 10.0, BG_VIDEO);

        let resp = ui.interact(rect, Id::unique("capyro-video"), Sense::click());
        if resp.double_clicked() {
            self.run_action(&ctx, Action::Fullscreen);
        } else if resp.clicked() {
            if self.core.state() == State::Idle {
                self.open_file_dialog(&ctx); // área vacía: clic = elegir archivos
            } else {
                self.core.toggle();
            }
        }

        let hovering_files = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let c = rect.center();
        let accent = self.cfg.accent;

        if hovering_files {
            painter.rect_stroke(rect.shrink(1.0), 10.0, Stroke::new(2.0, accent), StrokeKind::Inside);
            icons::film(&painter, c - vec2(0.0, 30.0), 28.0, accent);
            painter.text(c + vec2(0.0, 4.0), Align2::CENTER_CENTER, "Suelta para abrir", regular(14.0), TEXT);
            let formats = self.formats_label(" · ").to_uppercase();
            painter.text(c + vec2(0.0, 26.0), Align2::CENTER_CENTER, formats, regular(12.0), TEXT_FAINT);
        } else {
            match self.core.media_kind() {
                MediaKind::None => {
                    icons::film(&painter, c - vec2(0.0, 33.0), 28.0, Color32::from_rgb(0x55, 0x56, 0x5C));
                    painter.text(c, Align2::CENTER_CENTER, "Área de vídeo", regular(14.0), TEXT_DIM);
                    painter.text(
                        c + vec2(0.0, 27.0),
                        Align2::CENTER_CENTER,
                        "Arrastra un archivo o haz clic para elegirlo",
                        regular(12.0),
                        TEXT_FAINT,
                    );
                }
                MediaKind::Video => {
                    if let Some(tex) = &self.video_tex {
                        // Letterbox: el video entra entero manteniendo su aspecto.
                        let avail = rect.shrink(1.0).size();
                        let mut size = vec2(avail.x, avail.x / self.video_aspect);
                        if size.y > avail.y {
                            size = vec2(avail.y * self.video_aspect, avail.y);
                        }
                        let dest = Rect::from_center_size(c, size);
                        painter.rect_filled(dest, 0.0, Color32::BLACK);
                        painter.image(tex.id(), dest, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    } else {
                        icons::film(&painter, c - vec2(0.0, 33.0), 28.0, accent);
                        painter.text(c, Align2::CENTER_CENTER, "Cargando…", regular(14.0), TEXT_DIM);
                    }
                }
                MediaKind::Audio => {
                    let (title, artist) = self.track_labels();
                    let has_cover = self.video_tex.is_some();
                    let side = (rect.height() * 0.5).clamp(80.0, 260.0);
                    let art_center = c - vec2(0.0, if has_cover { 28.0 } else { 36.0 });
                    let text_y = if has_cover { art_center.y + side / 2.0 + 24.0 } else { c.y + 6.0 };
                    if self.cfg.visualizer && !self.viz.is_silent() {
                        // barras de frecuencia de fondo, debajo del título y el artista
                        let text_bottom = text_y + if artist.is_empty() { 14.0 } else { 34.0 };
                        let top = (rect.bottom() - rect.height() * 0.30).max(text_bottom + 6.0);
                        if rect.bottom() - 12.0 - top > 18.0 {
                            let band = Rect::from_min_max(
                                pos2(rect.left() + 24.0, top),
                                pos2(rect.right() - 24.0, rect.bottom() - 12.0),
                            );
                            self.viz.draw(&painter, band, accent.gamma_multiply(0.55));
                        }
                    }

                    if let Some(tex) = &self.video_tex {
                        let art = Rect::from_center_size(art_center, Vec2::splat(side));
                        let mut size = vec2(side, side / self.video_aspect);
                        if size.y > side {
                            size = vec2(side * self.video_aspect, side);
                        }
                        painter.rect_filled(art.expand(1.0), 10.0, BG_PANEL);
                        painter.image(
                            tex.id(),
                            Rect::from_center_size(art_center, size),
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        painter.circle_filled(art_center, 30.0, accent.gamma_multiply(0.18));
                        icons::music_note(&painter, art_center, 28.0, accent);
                    }
                    painter.text(pos2(c.x, text_y), Align2::CENTER_CENTER, ellipsize(&title, 52), bold(15.0), TEXT);
                    if !artist.is_empty() {
                        painter.text(
                            pos2(c.x, text_y + 22.0),
                            Align2::CENTER_CENTER,
                            ellipsize(&artist, 60),
                            regular(12.5),
                            TEXT_DIM,
                        );
                    }
                }
            }
        }

        if self.show_playlist {
            self.playlist_panel(ui, rect);
        }
    }

    /// Título del metadato (o nombre de archivo) y artista.
    fn track_labels(&self) -> (String, String) {
        let title = self.core.title();
        let title = if title.trim().is_empty() {
            file_name(&self.core.current_file())
        } else {
            title
        };
        (title, self.core.artist())
    }

    fn draw_toast(&mut self, ui: &mut Ui, video: Rect) {
        let now = ui.ctx().input(|i| i.time);
        let Some(t) = &self.toast else { return };
        if now > t.until {
            self.toast = None;
            return;
        }
        // Aparece subiendo un poco y se desvanece al final.
        let fade_in = ((now - t.start) / 0.15).clamp(0.0, 1.0) as f32;
        let fade_out = ((t.until - now) / 0.35).clamp(0.0, 1.0) as f32;
        let alpha = fade_in.min(fade_out);
        let rise = (1.0 - fade_in) * 8.0;

        let mut painter = ui.painter().clone();
        painter.multiply_opacity(alpha);
        let galley = painter.layout_no_wrap(t.text.clone(), regular(12.5), if t.error { ERROR } else { TEXT });
        let size = galley.size() + vec2(28.0, 14.0);
        let rect = Rect::from_center_size(pos2(video.center().x, video.bottom() - 28.0 + rise), size);
        painter.rect_filled(rect, size.y / 2.0, BG_PANEL);
        painter.rect_stroke(rect, size.y / 2.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
        painter.galley(rect.center() - galley.size() / 2.0, galley, TEXT);
        ui.ctx().request_repaint(); // la animación necesita frames
    }

    fn transport(&mut self, ui: &mut Ui, video_bottom: f32, inner: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();
        let accent = self.cfg.accent;
        let (left, right) = (inner.left(), inner.right());

        /* --- barra de progreso --- */
        let ty = video_bottom + TRACK_Y;
        let hit = Rect::from_min_max(pos2(left, ty - 8.0), pos2(right, ty + 8.0));
        let seek = ui.interact(hit, Id::unique("capyro-seek"), Sense::click_and_drag());
        let duration = self.core.duration();
        let can_seek = duration > 0.0;
        if can_seek && (seek.dragged() || seek.clicked()) {
            if let Some(pp) = seek.interact_pointer_pos() {
                let f = ((pp.x - left) / (right - left)).clamp(0.0, 1.0) as f64;
                self.core.seek(f * duration);
            }
        }
        let position = self.core.position();
        let frac = if can_seek { (position / duration).clamp(0.0, 1.0) as f32 } else { 0.0 };
        let active = can_seek && (seek.hovered() || seek.dragged());
        let grow = ctx.animate_bool_with_time(Id::unique("capyro-seek-grow"), active, 0.12);
        let th = 3.0 + 2.0 * grow;
        let track = Rect::from_min_max(pos2(left, ty - th / 2.0), pos2(right, ty + th / 2.0));
        painter.rect_filled(track, th / 2.0, TRACK);
        let fill_x = left + (right - left) * frac;
        if let Some(hp) = seek.hover_pos().filter(|_| active) {
            // vista previa de hasta dónde saltaría
            let hx = hp.x.clamp(left, right);
            if hx > fill_x {
                painter.rect_filled(
                    Rect::from_min_max(pos2(fill_x, track.min.y), pos2(hx, track.max.y)),
                    th / 2.0,
                    Color32::from_white_alpha(28),
                );
            }
        }
        painter.rect_filled(Rect::from_min_max(track.min, pos2(fill_x, track.max.y)), th / 2.0, accent);
        if grow > 0.001 {
            let pressed = ctx.animate_bool_with_time(Id::unique("capyro-seek-press"), seek.dragged(), 0.08);
            painter.circle_filled(pos2(fill_x, ty), 6.5 * grow + 1.5 * pressed, accent);
        }
        if active {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
            if let Some(hp) = seek.hover_pos() {
                let f = ((hp.x - left) / (right - left)).clamp(0.0, 1.0) as f64;
                let label = fmt_time(f * duration);
                let g = painter.layout_no_wrap(label, regular(11.0), TEXT);
                let r = Rect::from_center_size(pos2(hp.x, ty - 18.0), g.size() + vec2(10.0, 6.0));
                painter.rect_filled(r, 4.0, BG_PANEL);
                painter.galley(r.center() - g.size() / 2.0, g, TEXT);
            }
        }

        /* --- tiempos (+ velocidad si no es 1×) --- */
        let tty = video_bottom + TIMES_Y;
        painter.text(pos2(left, tty), Align2::LEFT_CENTER, fmt_time(position), regular(12.0), TEXT_DIM);
        // Clic en el tiempo de la derecha: duración total ↔ tiempo que falta.
        let right_label = if self.show_remaining && duration > 0.0 {
            format!("-{}", fmt_time((duration - position).max(0.0)))
        } else {
            fmt_time(duration)
        };
        let g = painter.layout_no_wrap(right_label, regular(12.0), TEXT_DIM);
        let dur_rect = Rect::from_min_size(pos2(right - g.size().x, tty - g.size().y / 2.0), g.size());
        let dur_btn = animate_button(ui, Id::unique("capyro-time-toggle"), dur_rect.expand2(vec2(6.0, 4.0)), duration > 0.0);
        if dur_btn.hover > 0.001 {
            painter.rect_filled(dur_rect.expand2(vec2(6.0, 3.0)), 5.0, Color32::from_white_alpha((12.0 * dur_btn.hover) as u8));
        }
        painter.galley(dur_rect.min, g, dur_btn.tint(TEXT_DIM, TEXT));
        if dur_btn.resp.clicked() {
            self.show_remaining = !self.show_remaining;
        }
        dur_btn.resp.on_hover_text(if self.show_remaining { "Mostrar duración total" } else { "Mostrar tiempo restante" });
        let speed = self.core.speed();
        if (speed - 1.0).abs() > 1e-6 {
            let g = painter.layout_no_wrap(fmt_speed(speed), bold(11.0), accent);
            let r = Rect::from_min_size(
                pos2(dur_rect.left() - g.size().x - 18.0, tty - g.size().y / 2.0 - 3.0),
                g.size() + vec2(10.0, 6.0),
            );
            painter.rect_filled(r, r.height() / 2.0, accent.gamma_multiply(0.15));
            painter.galley(r.center() - g.size() / 2.0, g, accent);
        }

        /* --- controles --- */
        let cy = video_bottom + CONTROLS_Y;
        let cx = inner.center().x;

        // aleatorio (izquierda)
        let shuffle_on = self.core.shuffle();
        let shuffle_c = pos2(cx - 105.0, cy);
        let sh = Self::icon_button(ui, "shuffle", shuffle_c, 30.0);
        let sh_on = ctx.animate_bool_with_time(Id::unique("capyro-shuffle-on"), shuffle_on, 0.15);
        let sh_col = sh.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, sh_on);
        icons::shuffle(&painter, shuffle_c, 16.0 * sh.scale(), sh_col);
        if sh_on > 0.001 {
            painter.circle_filled(shuffle_c + vec2(0.0, 13.0), 2.0 * sh_on, accent);
        }
        if sh.resp.clicked() {
            self.toggle_shuffle(&ctx);
        }
        sh.resp.on_hover_text(if shuffle_on { "Aleatorio: activado (Ctrl+S)" } else { "Aleatorio: desactivado (Ctrl+S)" });

        let prev_c = pos2(cx - 69.0, cy);
        let prev = Self::icon_button(ui, "prev", prev_c, 30.0);
        icons::prev(&painter, prev_c - vec2(2.0 * prev.press, 0.0), 16.0 * prev.scale(), prev.tint(ICON, Color32::WHITE));
        if prev.resp.clicked() {
            self.run_action(&ctx, Action::PrevFile);
        }

        let restart_c = pos2(cx - 33.0, cy);
        let restart = Self::icon_button(ui, "restart", restart_c, 30.0);
        icons::restart(&painter, restart_c, 16.0 * restart.scale(), restart.tint(ICON, Color32::WHITE));
        if restart.resp.clicked() {
            self.core.seek(0.0);
        }
        restart.resp.on_hover_text("Volver al inicio");

        // play / pausa: crece, brilla y el ícono se transforma
        let play_c = pos2(cx + 17.0, cy);
        let play = animate_button(ui, Id::unique("capyro-play"), Rect::from_center_size(play_c, Vec2::splat(46.0)), true);
        let playing = self.core.state() == State::Playing;
        let morph = ctx.animate_bool_with_time(Id::unique("capyro-play-morph"), playing, 0.16);
        let scale = 1.0 + 0.06 * play.hover - 0.1 * play.press;
        if play.hover > 0.001 {
            painter.circle_filled(play_c, 23.0 * scale + 6.0 * play.hover, accent.gamma_multiply(0.18 * play.hover));
        }
        let fill = accent.lerp_to_gamma(Color32::WHITE, 0.12 * play.hover).gamma_multiply(1.0 - 0.12 * play.press);
        painter.circle_filled(play_c, 23.0 * scale, fill);
        if morph < 0.999 {
            icons::play_badge(&painter, play_c, 21.0 * scale, Color32::WHITE.gamma_multiply(1.0 - morph), false);
        }
        if morph > 0.001 {
            icons::play_badge(&painter, play_c, 21.0 * scale, Color32::WHITE.gamma_multiply(morph), true);
        }
        if play.resp.clicked() {
            self.run_action(&ctx, Action::PlayPause);
        }

        let next_c = pos2(cx + 69.0, cy);
        let next = Self::icon_button(ui, "next", next_c, 30.0);
        icons::next(&painter, next_c + vec2(2.0 * next.press, 0.0), 16.0 * next.scale(), next.tint(ICON, Color32::WHITE));
        if next.resp.clicked() {
            self.run_action(&ctx, Action::NextFile);
        }

        // repetir (derecha): off → lista → uno
        let repeat = self.core.repeat();
        let repeat_c = pos2(cx + 105.0, cy);
        let rp = Self::icon_button(ui, "repeat", repeat_c, 30.0);
        let rp_on = ctx.animate_bool_with_time(Id::unique("capyro-repeat-on"), repeat != Repeat::Off, 0.15);
        let rp_col = rp.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, rp_on);
        icons::repeat(&painter, repeat_c, 16.0 * rp.scale(), rp_col, repeat == Repeat::One);
        if rp_on > 0.001 {
            painter.circle_filled(repeat_c + vec2(0.0, 13.0), 2.0 * rp_on, accent);
        }
        if rp.resp.clicked() {
            self.cycle_repeat(&ctx);
        }
        rp.resp.on_hover_text(match repeat {
            Repeat::Off => "Repetir: no (Ctrl+R)",
            Repeat::All => "Repetir: toda la lista (Ctrl+R)",
            Repeat::One => "Repetir: este archivo (Ctrl+R)",
        });

        /* --- botón "+ Agregar archivo(s)" (sobre los íconos de la derecha) --- */
        let pending = self.dialog.is_some();
        let label = if pending { "Abriendo…" } else { "Agregar archivo(s)" };
        let galley = painter.layout_no_wrap(label.to_owned(), bold(13.0), Color32::WHITE);
        let full_w = galley.size().x + 50.0;
        let controls_right = repeat_c.x + 22.0;
        // Si la ventana es angosta y no entra el texto, queda solo el "+".
        let compact = right - full_w < controls_right;
        let base = if compact { vec2(34.0, 34.0) } else { vec2(full_w, 34.0) };
        let base_rect = Rect::from_min_size(pos2(right - base.x, cy - base.y / 2.0), base);
        let add = animate_button(ui, Id::unique("capyro-add-files"), base_rect, !pending);
        let s = 1.0 + 0.03 * add.hover - 0.06 * add.press;
        let add_rect = Rect::from_center_size(base_rect.center(), base * s);
        if add.hover > 0.001 {
            painter.rect_filled(add_rect.expand(4.0 * add.hover), add_rect.height() / 2.0 + 4.0, accent.gamma_multiply(0.16 * add.hover));
        }
        let fill = if pending {
            accent.gamma_multiply(0.6)
        } else {
            accent.lerp_to_gamma(Color32::WHITE, 0.12 * add.hover).gamma_multiply(1.0 - 0.12 * add.press)
        };
        painter.rect_filled(add_rect, add_rect.height() / 2.0, fill);
        // el "+" gira un cuarto de vuelta al pasar el mouse
        let angle = add.hover * std::f32::consts::FRAC_PI_2;
        if compact {
            icons::plus(&painter, add_rect.center(), 14.0, Color32::WHITE, angle, 2.0);
        } else {
            let plus_c = pos2(add_rect.left() + 20.0, add_rect.center().y);
            icons::plus(&painter, plus_c, 11.0, Color32::WHITE, angle, 2.0);
            painter.galley(
                pos2(plus_c.x + 12.0, add_rect.center().y - galley.size().y / 2.0),
                galley,
                Color32::WHITE,
            );
        }
        if add.resp.clicked() {
            self.open_file_dialog(&ctx);
        }
        add.resp.on_hover_text(format!("Agregar archivos ({}) a la lista (Ctrl+O)", self.formats_label(", ")));

        /* --- botón de información (arriba de "Lista", a la izquierda) --- */
        let info_c = pos2(left + 9.0, cy);
        let info = Self::icon_button(ui, "info", info_c, 30.0);
        let info_on = ctx.animate_bool_with_time(Id::unique("capyro-info-on"), self.show_info, 0.15);
        let info_col = info.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, info_on);
        icons::info(&painter, info_c, 17.0 * info.scale(), info_col);
        // puntito: actualizando (late) o lista para reiniciar
        let pending_restart = matches!(self.updater.last, Some(Outcome::Installed { .. }));
        let badge = ctx.animate_bool_with_time(
            Id::unique("capyro-info-badge"),
            self.updater.is_running() || pending_restart,
            0.2,
        );
        if badge > 0.001 {
            let beat = if self.updater.is_running() { 0.6 + 0.4 * (ctx.input(|i| i.time) * 5.0).sin().abs() as f32 } else { 1.0 };
            painter.circle_filled(info_c + vec2(0.0, 13.0), 2.0 * badge, accent.gamma_multiply(beat));
        }
        self.info_rect = Rect::from_center_size(info_c, Vec2::splat(30.0));
        if info.resp.clicked() {
            let open = !self.show_info;
            self.close_menus();
            self.show_info = open;
        }
        if !self.show_info {
            info.resp.on_hover_text(if self.updater.is_running() {
                "Acerca de Capyro · actualizando…"
            } else if pending_restart {
                "Acerca de Capyro · actualización lista"
            } else {
                "Acerca de Capyro"
            });
        }

        /* --- botón de novedades (al lado de ⓘ) --- */
        let ch_c = pos2(left + 43.0, cy);
        let ch = Self::icon_button(ui, "changes", ch_c, 30.0);
        let ch_on = ctx.animate_bool_with_time(Id::unique("capyro-changes-on"), self.show_changes, 0.15);
        let ch_col = ch.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, ch_on);
        icons::news(&painter, ch_c, 17.0 * ch.scale(), ch_col);
        self.changes_rect = Rect::from_center_size(ch_c, Vec2::splat(30.0));
        // puntito mientras no se hayan visto las novedades de esta versión
        let unseen = self.changes_seen != ffi::version();
        let dot = ctx.animate_bool_with_time(Id::unique("capyro-changes-dot"), unseen, 0.3);
        if dot > 0.001 {
            painter.circle_filled(ch_c + vec2(0.0, 13.0), 2.0 * dot, accent);
        }
        if ch.resp.clicked() {
            let open = !self.show_changes;
            self.close_menus();
            self.show_changes = open;
            if open && unseen {
                self.changes_seen = ffi::version();
                self.save_session();
            }
        }
        if !self.show_changes {
            ch.resp.on_hover_text(if unseen { "Novedades de esta versión · ¡hay cosas nuevas!" } else { "Novedades de esta versión" });
        }

        /* --- fila inferior --- */
        let by = video_bottom + BOTTOM_ROW_Y;

        let list_rect = Rect::from_min_max(pos2(left - 6.0, by - 14.0), pos2(left + 66.0, by + 14.0));
        let list = animate_button(ui, Id::unique("capyro-list"), list_rect, true);
        let list_on = ctx.animate_bool_with_time(Id::unique("capyro-list-on"), self.show_playlist, 0.15);
        let list_col = list.tint(ICON_DIM, TEXT).lerp_to_gamma(accent, list_on);
        let bg = (10.0 * list.hover + 10.0 * list.press) as u8;
        if bg > 0 {
            painter.rect_filled(list_rect, 7.0, Color32::from_white_alpha(bg));
        }
        icons::playlist(&painter, pos2(left + 7.0, by), 14.0 * list.scale(), list_col);
        painter.text(pos2(left + 24.0, by), Align2::LEFT_CENTER, "Lista", regular(13.0), list_col);
        if list.resp.clicked() {
            self.show_playlist = !self.show_playlist;
        }

        // volumen: clic = silenciar, rueda = ajustar
        let vol_c = pos2(right - 92.0, by);
        let vol = Self::icon_button(ui, "volume", vol_c, 26.0);
        let (volume, muted) = (self.core.volume(), self.core.muted());
        icons::volume(&painter, vol_c, 16.0 * vol.scale(), vol.tint(ICON_DIM, Color32::WHITE), volume, muted);
        if vol.resp.clicked() {
            self.toggle_mute(&ctx);
        }
        // Rueda: un paso por "muesca". Antes se usaba el scroll suavizado, que
        // reparte una muesca en muchos frames y cada frame bajaba un paso
        // entero: una sola muesca se comía casi todo el volumen.
        // (Solo en la primera pasada del frame: egui a veces repite la pasada
        // con la misma entrada, p. ej. al mostrar un tooltip, y se contaba doble.)
        if vol.resp.hovered() && self.first_pass {
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
                self.change_volume(&ctx, steps * self.cfg.wheel_step);
            }
        } else if !vol.resp.hovered() {
            self.wheel_acc = 0.0;
        }
        if vol.hover > 0.001 {
            // porcentaje en una pastillita arriba del ícono
            let label = if muted {
                "Silenciado".to_owned()
            } else {
                format!("{}%", (volume * 100.0).round())
            };
            let mut p = painter.clone();
            p.multiply_opacity(vol.hover);
            let g = p.layout_no_wrap(label, regular(11.0), TEXT);
            let r = Rect::from_center_size(vol_c - vec2(0.0, 24.0 + 3.0 * vol.hover), g.size() + vec2(12.0, 6.0));
            p.rect_filled(r, r.height() / 2.0, BG_PANEL);
            p.galley(r.center() - g.size() / 2.0, g, TEXT);
        }

        // efectos de audio: a la izquierda del volumen
        let fx_c = pos2(right - 120.0, by);
        let fxb = Self::icon_button(ui, "effects", fx_c, 26.0);
        let fx_on = ctx.animate_bool_with_time(Id::unique("capyro-fx-on"), self.show_fx, 0.15);
        let fx_active = ctx.animate_bool_with_time(Id::unique("capyro-fx-active"), self.fx_active(), 0.15);
        let fx_col = fxb
            .tint(ICON_DIM, Color32::WHITE)
            .lerp_to_gamma(accent, fx_on.max(fx_active));
        icons::sliders(&painter, fx_c, 16.0 * fxb.scale(), fx_col);
        if fx_active > 0.001 {
            painter.circle_filled(fx_c + vec2(0.0, 12.0), 2.0 * fx_active, accent);
        }
        self.fx_rect = Rect::from_center_size(fx_c, Vec2::splat(26.0));
        if fxb.resp.clicked() {
            let open = !self.show_fx;
            self.close_menus();
            self.show_fx = open;
        }
        if !self.show_fx {
            fxb.resp.on_hover_text("Efectos de audio (Ctrl+E)");
        }
        vol.resp.on_hover_text("Clic: silenciar · Rueda: volumen");

        let subs_c = pos2(right - 64.0, by);
        let subs = Self::icon_button(ui, "subtitles", subs_c, 26.0);
        icons::subtitles(&painter, subs_c, 16.0 * subs.scale(), subs.tint(ICON_DIM, Color32::WHITE));
        if subs.resp.clicked() {
            self.play_error();
            self.toast(&ctx, "Los subtítulos todavía no están disponibles", true);
        }
        subs.resp.on_hover_text("Subtítulos — próximamente");

        // engranaje: abre el menú de ajustes (y gira un poco al pasar el mouse)
        let gear_c = pos2(right - 36.0, by);
        let gear = Self::icon_button(ui, "settings", gear_c, 26.0);
        let gear_on = ctx.animate_bool_with_time(Id::unique("capyro-gear-on"), self.show_settings, 0.15);
        let gear_col = gear.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, gear_on);
        icons::gear(&painter, gear_c, 16.0 * gear.scale(), gear_col);
        self.gear_rect = Rect::from_center_size(gear_c, Vec2::splat(26.0));
        if gear.resp.clicked() {
            let open = !self.show_settings;
            self.close_menus();
            self.show_settings = open;
        }
        if !self.show_settings {
            gear.resp.on_hover_text("Ajustes");
        }

        let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let fs_c = pos2(right - 8.0, by);
        let fsb = Self::icon_button(ui, "fullscreen", fs_c, 26.0);
        icons::fullscreen(&painter, fs_c, 14.0 * fsb.scale(), fsb.tint(ICON_DIM, Color32::WHITE), fs);
        if fsb.resp.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fs));
        }
    }

    /// Menú de información: versión, formatos soportados y algunos datos.
    /// Crece desde el botón ⓘ (abajo a la izquierda), igual que el de ajustes.
    fn info_menu(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let t = ctx.animate_bool_with_time(Id::unique("capyro-info-anim"), self.show_info, 0.15);
        if t <= 0.001 {
            self.info_format = None; // al volver a abrirlo arranca sin resumen abierto
            return;
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let accent = self.cfg.accent;
        let width = 312.0;
        let inner_w = width - 28.0;
        let base_h = 244.0;

        // --- resúmenes (acordeón): se calcula antes para saber el alto total ---
        // Cada formato tiene su propia animación; como solo uno puede estar
        // abierto, al cambiar de MP4 a MP3 uno se cierra mientras el otro se abre.
        let formats = self.formats.clone();
        let all: Vec<(String, bool)> = formats
            .0
            .iter()
            .map(|e| (e.clone(), true))
            .chain(formats.1.iter().map(|e| (e.clone(), false)))
            .collect();
        let mut cards = Vec::new();
        let mut region_h = 0.0;
        for (ext, is_video) in &all {
            let open = self.info_format.as_deref() == Some(ext.as_str());
            let k = ctx.animate_bool_with_time(Id::unique(("capyro-fmt-open", ext.as_str())), open, 0.22);
            if k <= 0.001 {
                continue;
            }
            let k = 1.0 - (1.0 - k).powi(3);
            let info = crate::formats::info(ext);
            let kind = if *is_video { "Video" } else { "Audio" };
            let title = ui.painter().layout_no_wrap(
                format!("{} · {}", ext.to_uppercase(), info.name),
                bold(12.0),
                TEXT,
            );
            let kind_g = ui.painter().layout_no_wrap(kind.to_owned(), regular(10.5), TEXT_FAINT);
            let body = ui.painter().layout(info.summary.to_owned(), regular(11.5), TEXT_DIM, inner_w - 20.0);
            let card_h = 10.0 + title.size().y + 5.0 + body.size().y + 10.0;
            region_h += (card_h + 8.0) * k;
            cards.push((k, card_h, title, kind_g, body));
        }

        let size = vec2(width, base_h + region_h);
        let anchor = pos2(self.info_rect.left() - 6.0, self.info_rect.top() - 10.0);
        let full = Rect::from_min_size(pos2(anchor.x, anchor.y - size.y), size);
        let grown = size * (0.85 + 0.15 * ease);
        let panel = Rect::from_min_size(pos2(anchor.x, anchor.y - grown.y), grown);
        let interactive = self.show_info && t > 0.6;

        let mut painter = ui.painter().clone();
        painter.multiply_opacity(ease);
        if interactive {
            let outside = ui.interact(ui.max_rect(), Id::unique("capyro-info-outside"), Sense::click());
            if outside.clicked() {
                self.show_info = false;
            }
            ui.interact(full, Id::unique("capyro-info-bg"), Sense::click());
        }
        painter.rect_filled(panel.translate(vec2(0.0, 4.0)), 12.0, Color32::from_black_alpha(70));
        painter.rect_filled(panel, 12.0, BG_PANEL);
        painter.rect_stroke(panel, 12.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
        if ease < 0.98 {
            ctx.request_repaint();
            return;
        }

        let x0 = panel.left() + 14.0;
        let x1 = panel.right() - 14.0;
        let top = panel.top();

        // encabezado: logo + nombre + versión
        let logo_rect = Rect::from_min_size(pos2(x0, top + 14.0), vec2(40.0, 40.0));
        if let Some(logo) = &self.logo {
            painter.image(logo.id(), logo_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let tx = logo_rect.right() + 10.0;
        let name = painter.text(pos2(tx, top + 25.0), Align2::LEFT_CENTER, "Capyro", bold(15.0), TEXT);
        let g = painter.layout_no_wrap(format!("v{}", ffi::version()), bold(11.0), accent);
        let chip = Rect::from_min_size(pos2(name.right() + 8.0, top + 25.0 - g.size().y / 2.0 - 3.0), g.size() + vec2(12.0, 6.0));
        painter.rect_filled(chip, chip.height() / 2.0, accent.gamma_multiply(0.16));
        painter.galley(chip.center() - g.size() / 2.0, g, accent);
        painter.text(pos2(tx, top + 44.0), Align2::LEFT_CENTER, "Just press play.", regular(12.0), TEXT_DIM);

        // formatos
        painter.line_segment([pos2(x0, top + 64.0), pos2(x1, top + 64.0)], Stroke::new(1.0, Color32::from_white_alpha(14)));
        painter.text(pos2(x0, top + 80.0), Align2::LEFT_CENTER, "Formatos soportados", bold(12.5), TEXT);
        painter.text(pos2(x1, top + 80.0), Align2::RIGHT_CENTER, "tocá uno", regular(10.5), TEXT_FAINT);

        let mut clicked: Option<String> = None;
        let mut y = top + 108.0;
        for (label, list, is_video) in [("Video", &formats.0, true), ("Audio", &formats.1, false)] {
            let ic = pos2(x0 + 8.0, y);
            if is_video {
                icons::film(&painter, ic, 14.0, TEXT_DIM);
            } else {
                icons::music_note(&painter, ic, 14.0, TEXT_DIM);
            }
            painter.text(pos2(x0 + 22.0, y), Align2::LEFT_CENTER, label, regular(12.0), TEXT_DIM);
            let mut cx = x0 + 74.0;
            for ext in list {
                let selected = self.info_format.as_deref() == Some(ext.as_str());
                let g = painter.layout_no_wrap(ext.to_uppercase(), bold(11.0), TEXT);
                let r = Rect::from_min_size(pos2(cx, y - 11.0), vec2(g.size().x + 14.0, 22.0));
                let a = animate_button(ui, Id::unique(("capyro-fmt", ext.as_str())), r, interactive);
                let sel = ctx.animate_bool_with_time(Id::unique(("capyro-fmt-sel", ext.as_str())), selected, 0.15);
                let rr = Rect::from_center_size(r.center(), r.size() * (1.0 + 0.04 * a.hover - 0.08 * a.press));
                let bg = Color32::from_white_alpha((10.0 + 12.0 * a.hover) as u8).lerp_to_gamma(accent, sel);
                painter.rect_filled(rr, 6.0, bg);
                let col = TEXT.lerp_to_gamma(Color32::WHITE, sel);
                painter.galley(rr.center() - g.size() / 2.0, g, col);
                if a.resp.clicked() {
                    clicked = Some(ext.clone());
                }
                cx = r.right() + 6.0;
            }
            y += 30.0;
        }
        if let Some(ext) = clicked {
            // solo uno abierto a la vez; tocar el abierto lo cierra
            self.info_format = if self.info_format.as_deref() == Some(ext.as_str()) { None } else { Some(ext) };
        }

        // resúmenes
        let region = Rect::from_min_size(pos2(x0, top + 156.0), vec2(inner_w, region_h));
        let clipped = painter.with_clip_rect(region.expand2(vec2(2.0, 0.0)).intersect(painter.clip_rect()));
        let mut cy = region.top();
        for (k, card_h, title, kind_g, body) in cards {
            let mut p = clipped.clone();
            p.multiply_opacity(k);
            let card = Rect::from_min_size(pos2(x0, cy - 6.0 * (1.0 - k)), vec2(inner_w, card_h));
            p.rect_filled(card, 8.0, Color32::from_white_alpha(9));
            p.rect_filled(Rect::from_min_size(card.min + vec2(0.0, 10.0), vec2(3.0, card_h - 20.0)), 1.5, accent);
            let ty = card.top() + 10.0;
            p.galley(pos2(card.left() + 12.0, ty), title.clone(), TEXT);
            p.galley(pos2(card.right() - 10.0 - kind_g.size().x, ty + 1.0), kind_g, TEXT_FAINT);
            p.galley(pos2(card.left() + 12.0, ty + title.size().y + 5.0), body, TEXT_DIM);
            cy += (card_h + 8.0) * k;
        }

        // actualizaciones
        let upd_top = region.bottom() + 2.0;
        painter.line_segment([pos2(x0, upd_top), pos2(x1, upd_top)], Stroke::new(1.0, Color32::from_white_alpha(14)));
        self.update_row(ui, &painter, Rect::from_min_max(pos2(x0 - 6.0, upd_top + 6.0), pos2(x1 + 6.0, upd_top + 48.0)), interactive);

        // pie: backend de audio y licencia
        let backend = self.core.audio_backend();
        let audio = if backend.is_empty() { "sin dispositivo".to_owned() } else { backend };
        painter.text(
            pos2(x0, panel.bottom() - 16.0),
            Align2::LEFT_CENTER,
            format!("Audio: {audio}  ·  GPL-3.0  ·  Linux"),
            regular(10.5),
            TEXT_FAINT,
        );

        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.show_info = false;
        }
    }

    /// Menú del engranaje: velocidad y recargar configuración. Se abre
    /// creciendo desde el engranaje y se cierra al hacer clic afuera o con Esc.
    fn settings_menu(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let t = ctx.animate_bool_with_time(Id::unique("capyro-settings-anim"), self.show_settings, 0.15);
        if t <= 0.001 {
            return;
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let accent = self.cfg.accent;
        let size = vec2(288.0, 272.0);
        let anchor = pos2(self.gear_rect.right() + 6.0, self.gear_rect.top() - 10.0);
        let full = Rect::from_min_size(anchor - size, size);
        // crece desde la esquina del engranaje
        let panel = Rect::from_min_max(anchor - size * (0.85 + 0.15 * ease), anchor);
        let interactive = self.show_settings && t > 0.6;

        let mut painter = ui.painter().clone();
        painter.multiply_opacity(ease);
        if interactive {
            // Capa invisible sobre toda la ventana: un clic afuera solo cierra
            // el menú (no pausa el video ni aprieta lo que haya abajo).
            let outside = ui.interact(ui.max_rect(), Id::unique("capyro-settings-outside"), Sense::click());
            if outside.clicked() {
                self.show_settings = false;
            }
            ui.interact(full, Id::unique("capyro-settings-bg"), Sense::click());
        }
        painter.rect_filled(panel.translate(vec2(0.0, 4.0)), 12.0, Color32::from_black_alpha(70));
        painter.rect_filled(panel, 12.0, BG_PANEL);
        painter.rect_stroke(panel, 12.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
        if ease < 0.98 {
            ctx.request_repaint();
            return; // el contenido aparece cuando termina de abrirse
        }

        let x0 = panel.left() + 14.0;
        let x1 = panel.right() - 14.0;
        let mut y = panel.top() + 20.0;
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Ajustes", bold(13.5), TEXT);

        /* velocidad */
        y += 28.0;
        let speed = self.core.speed();
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Velocidad", regular(12.5), TEXT_DIM);
        painter.text(pos2(x1, y), Align2::RIGHT_CENTER, fmt_speed(speed), bold(12.5), accent);
        y += 24.0;
        let gap = 5.0;
        let chip_w = ((x1 - x0) - gap * (SPEEDS.len() as f32 - 1.0)) / SPEEDS.len() as f32;
        for (i, &v) in SPEEDS.iter().enumerate() {
            let r = Rect::from_min_size(pos2(x0 + i as f32 * (chip_w + gap), y - 13.0), vec2(chip_w, 26.0));
            let chip = animate_button(ui, Id::unique(("capyro-speed", i)), r, interactive);
            let selected = (speed - v).abs() < 1e-6;
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-speed-sel", i)), selected, 0.15);
            let bg = Color32::from_white_alpha((8.0 + 14.0 * chip.hover) as u8).lerp_to_gamma(accent, sel);
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.06 * chip.press));
            painter.rect_filled(rr, 7.0, bg);
            let col = TEXT_DIM.lerp_to_gamma(TEXT, chip.hover).lerp_to_gamma(Color32::WHITE, sel);
            painter.text(rr.center(), Align2::CENTER_CENTER, fmt_speed(v), if selected { bold(11.5) } else { regular(11.5) }, col);
            if chip.resp.clicked() && !selected {
                self.set_speed(&ctx, v);
            }
        }

        /* separador */
        y += 24.0;
        painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, Color32::from_white_alpha(14)));

        /* decodificación de video: automático / GPU / CPU */
        y += 22.0;
        let mode = self.cfg.decoding;
        let info = self.core.decode_info();
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Decodificación", regular(12.5), TEXT_DIM);
        // a la derecha, qué se está usando ahora (si hay un video abierto)
        if info.active {
            let (txt, col) = if info.hw { (format!("GPU · {}", info.api), accent) } else { ("CPU".to_owned(), TEXT_FAINT) };
            let g = painter.layout_no_wrap(txt, bold(11.0), col);
            let r = Rect::from_min_size(pos2(x1 - g.size().x - 12.0, y - g.size().y / 2.0 - 3.0), g.size() + vec2(12.0, 6.0));
            painter.rect_filled(r, r.height() / 2.0, if info.hw { accent.gamma_multiply(0.16) } else { Color32::from_white_alpha(10) });
            painter.galley(r.center() - g.size() / 2.0, g, col);
        }
        y += 24.0;
        let chip_w = ((x1 - x0) - gap * 2.0) / 3.0;
        for (i, m) in ffi::Hwdec::ALL.iter().copied().enumerate() {
            let r = Rect::from_min_size(pos2(x0 + i as f32 * (chip_w + gap), y - 13.0), vec2(chip_w, 26.0));
            let chip = animate_button(ui, Id::unique(("capyro-hwdec", i)), r, interactive);
            let selected = mode == m;
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-hwdec-sel", i)), selected, 0.15);
            let bg = Color32::from_white_alpha((8.0 + 14.0 * chip.hover) as u8).lerp_to_gamma(accent, sel);
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.06 * chip.press));
            painter.rect_filled(rr, 7.0, bg);
            let col = TEXT_DIM.lerp_to_gamma(TEXT, chip.hover).lerp_to_gamma(Color32::WHITE, sel);
            painter.text(rr.center(), Align2::CENTER_CENTER, m.label(), if selected { bold(11.5) } else { regular(11.5) }, col);
            if chip.resp.clicked() && !selected {
                self.set_decoding(&ctx, m);
            }
            chip.resp.on_hover_text(match m {
                ffi::Hwdec::Auto => "Usa la GPU si se puede; si no, la CPU",
                ffi::Hwdec::Gpu => "Decodifica en la placa de video (VA-API, NVDEC o VDPAU). Si no se puede, te avisa y usa la CPU",
                ffi::Hwdec::Cpu => "Siempre por software (como hasta la v0.2.8)",
            });
        }
        y += 22.0;
        let status = if !info.active {
            "Se aplica a los videos (los audios no la usan)".to_owned()
        } else if info.hw {
            format!("{} decodificado por la GPU ({})", info.codec, info.api)
        } else {
            format!("{} por CPU · {}", info.codec, info.note)
        };
        let status_rect = Rect::from_min_max(pos2(x0, y - 8.0), pos2(x1, y + 8.0));
        let st = ui.interact(status_rect, Id::unique("capyro-hwdec-status"), Sense::hover());
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, ellipsize(&status, 46), regular(10.5), TEXT_FAINT);
        if status.chars().count() > 46 {
            st.on_hover_text(status);
        }

        /* separador */
        y += 18.0;
        painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, Color32::from_white_alpha(14)));

        /* editar configuración (atajos) */
        y += 26.0;
        let row = Rect::from_min_max(pos2(x0 - 6.0, y - 18.0), pos2(x1 + 6.0, y + 18.0));
        let edit = animate_button(ui, Id::unique("capyro-settings-edit"), row, interactive);
        if edit.hover > 0.001 || edit.press > 0.001 {
            painter.rect_filled(row, 8.0, Color32::from_white_alpha((10.0 * edit.hover + 10.0 * edit.press) as u8));
        }
        let ic = pos2(x0 + 8.0, y - 1.0);
        icons::keyboard(&painter, ic, 16.0 * edit.scale(), edit.tint(ICON, Color32::WHITE));
        painter.text(pos2(x0 + 24.0, y - 7.0), Align2::LEFT_CENTER, "Editar configuración", regular(12.5), TEXT);
        painter.text(
            pos2(x0 + 24.0, y + 8.0),
            Align2::LEFT_CENTER,
            format!("Atajos de teclado · {} teclas", self.cfg.bindings.len()),
            regular(10.5),
            TEXT_FAINT,
        );
        // flechita: abre otra pantalla
        let chev = pos2(x1 - 2.0 + 2.0 * edit.hover, y);
        let st = Stroke::new(1.5, edit.tint(TEXT_FAINT, TEXT));
        painter.line_segment([chev + vec2(-4.0, -5.0), chev], st);
        painter.line_segment([chev + vec2(-4.0, 5.0), chev], st);
        if edit.resp.clicked() {
            self.show_settings = false;
            self.show_keys = true;
        }

        /* captura del fotograma */
        y += 40.0;
        let is_video = self.core.media_kind() == MediaKind::Video;
        let row = Rect::from_min_max(pos2(x0 - 6.0, y - 18.0), pos2(x1 + 6.0, y + 18.0));
        let shot = animate_button(ui, Id::unique("capyro-settings-shot"), row, interactive);
        if shot.hover > 0.001 || shot.press > 0.001 {
            painter.rect_filled(row, 8.0, Color32::from_white_alpha((10.0 * shot.hover + 10.0 * shot.press) as u8));
        }
        let col = if is_video { shot.tint(ICON, Color32::WHITE) } else { TEXT_FAINT };
        icons::camera(&painter, pos2(x0 + 8.0, y - 1.0), 15.0 * shot.scale(), col);
        painter.text(
            pos2(x0 + 24.0, y - 7.0),
            Align2::LEFT_CENTER,
            "Capturar fotograma",
            regular(12.5),
            if is_video { TEXT } else { TEXT_FAINT },
        );
        painter.text(
            pos2(x0 + 24.0, y + 8.0),
            Align2::LEFT_CENTER,
            if is_video { "Shift+S · se guarda como PNG" } else { "Solo con video" },
            regular(10.5),
            TEXT_FAINT,
        );
        if shot.resp.clicked() {
            self.show_settings = false;
            self.take_screenshot(&ctx);
        }

        // Esc también lo cierra
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.show_settings = false;
        }
    }

    /// "Versión actual: 1a2b3c4 · hace 2 días" (debajo de "Buscar actualizaciones").
    fn version_line(&self) -> String {
        let b = &self.build_info;
        if b.commit.is_empty() {
            return if self.installed.is_some() {
                "Compilado sin git (desde un .zip)".to_owned()
            } else {
                "Sin git · sin instalar (corre desde build/)".to_owned()
            };
        }
        let short: String = b.commit.chars().take(7).collect();
        let dirty = if b.dirty { "+cambios" } else { "" };
        if self.installed.is_none() {
            return format!("{short}{dirty} · sin instalar (corre desde build/)");
        }
        let when = updater::ago(b.commit_time);
        if when.is_empty() {
            format!("Versión actual: {short}{dirty}")
        } else {
            format!("Versión actual: {short}{dirty} · {when}")
        }
    }

    /// Fila "Buscar actualizaciones" del menú ⓘ, con su progreso y resultado.
    fn update_row(&mut self, ui: &mut Ui, painter: &egui::Painter, row: Rect, interactive: bool) {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;
        let now = ctx.input(|i| i.time);
        let progress = self.updater.progress();
        let running = progress.is_some();
        let installed = matches!(self.updater.last, Some(Outcome::Installed { .. }));

        let btn = animate_button(ui, Id::unique("capyro-update-row"), row, interactive && !running);
        if btn.hover > 0.001 || btn.press > 0.001 {
            painter.rect_filled(row, 8.0, Color32::from_white_alpha((10.0 * btn.hover + 10.0 * btn.press) as u8));
        }
        let x0 = row.left() + 6.0;
        let cy = row.center().y;
        let ic = pos2(x0 + 8.0, cy - 1.0);

        let mut sub_col = TEXT_FAINT;
        let mut tooltip = "Busca commits nuevos en el repo de GitHub; si hay, los baja, compila e instala solo".to_owned();
        let (title, sub) = if let Some(pr) = &progress {
            icons::spinner(painter, ic, 15.0, accent, now);
            let secs = pr.elapsed.as_secs();
            let t = format!("{}:{:02}", secs / 60, secs % 60);
            let sub = if pr.detail.is_empty() { t } else { format!("{} · {t}", pr.detail) };
            tooltip = format!("Mientras tanto podés seguir usando Capyro. Detalle en {}", updater::log_path_display());
            (pr.stage.label().to_owned(), sub)
        } else {
            match &self.updater.last {
                Some(Outcome::Installed { commits, .. }) => {
                    painter.circle_filled(ic, 8.0, accent);
                    icons::check(painter, ic, 11.0, Color32::WHITE, 1.8);
                    tooltip = "Clic para reiniciar Capyro con la versión nueva".to_owned();
                    match commits.first() {
                        Some(c) => (format!("Actualizado a {}", c.hash), c.subject.clone()),
                        None => ("Actualizado".to_owned(), "Reiniciá para aplicar".to_owned()),
                    }
                }
                other => {
                    icons::download(painter, ic, 15.0 * btn.scale(), btn.tint(ICON, Color32::WHITE), btn.hover);
                    let sub = match other {
                        Some(Outcome::UpToDate { hash }) => format!("Ya tenés la última versión ({hash})"),
                        Some(Outcome::Ahead) => "Tu versión es más nueva que la del repo".to_owned(),
                        Some(Outcome::Available { commits, total, installable }) => {
                            tooltip = if *installable {
                                "Clic para ver los cambios e instalarlos".to_owned()
                            } else {
                                "Para que se actualice solo, instalá Capyro (cmake --install build)".to_owned()
                            };
                            let s = if *total == 1 { "" } else { "s" };
                            let last = commits.first().map(|c| format!(" · último: {}", c.hash)).unwrap_or_default();
                            format!("{total} cambio{s} nuevo{s}{last}")
                        }
                        Some(Outcome::Failed { kind, message }) => {
                            sub_col = ERROR;
                            tooltip = if *kind == FailKind::Build {
                                format!("{message}\nDetalle en {}", updater::log_path_display())
                            } else {
                                message.clone()
                            };
                            message.clone()
                        }
                        _ => {
                            let b = &self.build_info;
                            if !b.subject.is_empty() {
                                let rama = if b.branch.is_empty() { String::new() } else { format!(" ({})", b.branch) };
                                tooltip = format!("{tooltip}\nEste Capyro sale de: {}{rama}", b.subject);
                            }
                            self.version_line()
                        }
                    };
                    ("Buscar actualizaciones".to_owned(), sub)
                }
            }
        };

        // a la derecha: "Reiniciar" cuando ya está instalada, "Ver" si hay algo nuevo
        let mut text_right = row.right() - 10.0;
        let available = !running && matches!(self.updater.last, Some(Outcome::Available { .. }));
        if installed || available {
            let label = if installed { "Reiniciar" } else { "Ver" };
            let g = painter.layout_no_wrap(label.to_owned(), bold(11.5), Color32::WHITE);
            let pill = Rect::from_min_size(
                pos2(row.right() - 8.0 - g.size().x - 20.0, cy - 12.0),
                vec2(g.size().x + 20.0, 24.0),
            );
            let fill = accent.lerp_to_gamma(Color32::WHITE, 0.12 * btn.hover);
            painter.rect_filled(pill, 12.0, fill);
            painter.galley(pill.center() - g.size() / 2.0, g, Color32::WHITE);
            text_right = pill.left() - 8.0;
        }
        let max_chars = (((text_right - (x0 + 24.0)) / 5.6) as usize).max(10);
        painter.text(pos2(x0 + 24.0, cy - 7.0), Align2::LEFT_CENTER, ellipsize(&title, max_chars), regular(12.5), TEXT);
        painter.text(pos2(x0 + 24.0, cy + 8.0), Align2::LEFT_CENTER, ellipsize(&sub, max_chars + 4), regular(10.5), sub_col);

        // barra "indeterminada": compilar no avisa cuánto falta
        if running {
            let bar = Rect::from_min_max(pos2(x0 + 24.0, row.bottom() - 3.0), pos2(row.right() - 12.0, row.bottom() - 1.0));
            painter.rect_filled(bar, 1.0, TRACK);
            let w = bar.width() * 0.3;
            let phase = ((now * 0.8) % 1.0) as f32;
            let x = bar.left() - w + (bar.width() + w) * ease_in_out(phase);
            let seg = Rect::from_min_max(
                pos2(x.max(bar.left()), bar.top()),
                pos2((x + w).min(bar.right()), bar.bottom()),
            );
            if seg.width() > 0.0 {
                painter.rect_filled(seg, 1.0, accent);
            }
        }

        if btn.resp.clicked() {
            self.start_update(&ctx, false);
        }
        if interactive {
            btn.resp.on_hover_text(tooltip);
        }
    }

    /// Aviso "Hay una actualización": qué commits se van a instalar (los que
    /// son más nuevos que el tuyo) y los botones Instalar / Ahora no.
    fn draw_update_offer(&mut self, ui: &mut Ui, video: Rect) {
        let ctx = ui.ctx().clone();
        let t = ctx.animate_bool_with_time(Id::unique("capyro-offer-anim"), self.show_offer, 0.22);
        if t <= 0.001 {
            return;
        }
        let Some(Outcome::Available { commits, total, installable }) = self.updater.last.clone() else {
            self.show_offer = false;
            return;
        };
        let e = ease_out(t);
        let accent = self.cfg.accent;
        let now = ctx.input(|i| i.time);
        let interactive = self.show_offer && t > 0.6;

        const MAX_SHOWN: usize = 6;
        let shown: Vec<CommitInfo> = commits.iter().take(MAX_SHOWN).cloned().collect();
        let more = total.saturating_sub(shown.len());
        let width = (video.width() - 32.0).min(460.0);
        let h = 64.0 + 26.0 * shown.len() as f32 + if more > 0 { 20.0 } else { 0.0 } + 70.0;
        let rect = Rect::from_min_size(
            pos2(video.center().x - width / 2.0, video.top() + 14.0 - 16.0 * (1.0 - e)),
            vec2(width, h),
        );

        let mut p = ui.painter().clone();
        p.multiply_opacity(e);
        if interactive {
            ui.interact(rect, Id::unique("capyro-offer-bg"), Sense::click()); // que no pause el video
        }
        p.rect_filled(rect.translate(vec2(0.0, 5.0)), 14.0, Color32::from_black_alpha(80));
        p.rect_filled(rect, 14.0, BG_PANEL);
        p.rect_stroke(rect, 14.0, Stroke::new(1.0, accent.gamma_multiply(0.5)), StrokeKind::Inside);

        // encabezado
        let hc = pos2(rect.left() + 30.0, rect.top() + 31.0);
        p.circle_filled(hc, 15.0, accent.gamma_multiply(0.18));
        let bob = ((now * 3.0).sin() as f32 * 0.5 + 0.5) * 0.6;
        icons::download(&p, hc, 17.0, accent, bob);
        let tx = hc.x + 26.0;
        p.text(pos2(tx, rect.top() + 22.0), Align2::LEFT_CENTER, "Hay una actualización", bold(14.0), TEXT);
        let s = if total == 1 { "" } else { "s" };
        let mine: String = self.build_info.commit.chars().take(7).collect();
        let sub = if mine.is_empty() {
            format!("{total} cambio{s} nuevo{s} en el repo")
        } else {
            format!("{total} cambio{s} más nuevo{s} que tu versión ({mine})")
        };
        p.text(pos2(tx, rect.top() + 40.0), Align2::LEFT_CENTER, sub, regular(11.5), TEXT_DIM);
        if interactive {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        p.line_segment(
            [pos2(rect.left() + 16.0, rect.top() + 58.0), pos2(rect.right() - 16.0, rect.top() + 58.0)],
            Stroke::new(1.0, Color32::from_white_alpha(14)),
        );

        // los commits que se van a instalar (el de arriba es el más nuevo)
        let mut y = rect.top() + 76.0;
        for (i, c) in shown.iter().enumerate() {
            let hg = p.layout_no_wrap(c.hash.clone(), FontId::monospace(11.0), accent);
            let chip = Rect::from_min_size(pos2(rect.left() + 16.0, y - 10.0), hg.size() + vec2(12.0, 20.0 - hg.size().y));
            p.rect_filled(chip, 6.0, accent.gamma_multiply(0.14));
            p.galley(chip.center() - hg.size() / 2.0, hg, accent);
            let wg = p.layout_no_wrap(updater::ago(c.time), regular(10.5), TEXT_FAINT);
            p.galley(pos2(rect.right() - 16.0 - wg.size().x, y - wg.size().y / 2.0), wg.clone(), TEXT_FAINT);
            let avail = rect.right() - 24.0 - wg.size().x - (chip.right() + 8.0);
            let max_chars = ((avail / 6.4) as usize).max(8);
            p.text(
                pos2(chip.right() + 8.0, y),
                Align2::LEFT_CENTER,
                ellipsize(&c.subject, max_chars),
                if i == 0 { bold(12.0) } else { regular(12.0) },
                if i == 0 { TEXT } else { TEXT_DIM },
            );
            y += 26.0;
        }
        if more > 0 {
            let s = if more == 1 { "" } else { "s" };
            p.text(pos2(rect.left() + 16.0, y - 4.0), Align2::LEFT_CENTER, format!("+{more} cambio{s} más"), regular(11.0), TEXT_FAINT);
            y += 20.0;
        }

        // pie: Instalar / Ahora no
        p.line_segment(
            [pos2(rect.left() + 16.0, y - 6.0), pos2(rect.right() - 16.0, y - 6.0)],
            Stroke::new(1.0, Color32::from_white_alpha(14)),
        );
        let fy = rect.bottom() - 30.0;
        let mut install = false;
        let mut close = false;
        let close_label = if installable { "Ahora no" } else { "Cerrar" };
        let g = p.layout_no_wrap(close_label.to_owned(), regular(12.0), TEXT_DIM);
        let cr = Rect::from_min_max(pos2(rect.right() - 16.0 - g.size().x - 20.0, fy - 14.0), pos2(rect.right() - 16.0, fy + 14.0));
        let cb = animate_button(ui, Id::unique("capyro-offer-later"), cr, interactive);
        if cb.hover > 0.001 {
            p.rect_filled(cr, 7.0, Color32::from_white_alpha((12.0 * cb.hover) as u8));
        }
        p.galley(cr.center() - g.size() / 2.0, g, cb.tint(TEXT_DIM, Color32::WHITE));
        close |= cb.resp.clicked();
        if installable {
            let g = p.layout_no_wrap("Instalar".to_owned(), bold(12.5), Color32::WHITE);
            let ir = Rect::from_min_max(pos2(cr.left() - 8.0 - g.size().x - 44.0, fy - 15.0), pos2(cr.left() - 8.0, fy + 15.0));
            let ib = animate_button(ui, Id::unique("capyro-offer-install"), ir, interactive);
            let rr = Rect::from_center_size(ir.center(), ir.size() * (1.0 + 0.04 * ib.hover - 0.08 * ib.press));
            if ib.hover > 0.001 {
                p.rect_filled(rr.expand(3.0 * ib.hover), rr.height() / 2.0 + 3.0, accent.gamma_multiply(0.2 * ib.hover));
            }
            p.rect_filled(rr, rr.height() / 2.0, accent.lerp_to_gamma(Color32::WHITE, 0.12 * ib.hover));
            icons::download(&p, pos2(rr.left() + 18.0, rr.center().y), 13.0, Color32::WHITE, ib.hover);
            p.galley(pos2(rr.left() + 30.0, rr.center().y - g.size().y / 2.0), g, Color32::WHITE);
            install = ib.resp.clicked();
            p.text(
                pos2(rect.left() + 16.0, fy),
                Align2::LEFT_CENTER,
                "Se baja, compila e instala sola",
                regular(11.0),
                TEXT_FAINT,
            );
        } else {
            p.text(
                pos2(rect.left() + 16.0, fy),
                Align2::LEFT_CENTER,
                "Para instalarla, primero instalá Capyro",
                regular(11.0),
                TEXT_FAINT,
            );
        }
        if interactive && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            close = true;
        }
        if close {
            self.show_offer = false;
        }
        if install {
            self.sfx(SFX_READY); // a descargar
            self.show_offer = false;
            self.show_info = true; // para ver el progreso
            self.start_update(&ctx, true);
        }
    }

    /// Aviso con "el parche": los commits que se acaban de instalar.
    fn draw_update_banner(&mut self, ui: &mut Ui, video: Rect) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let accent = self.cfg.accent;
        let Some(b) = &mut self.update_banner else { return };
        if now > b.until {
            self.update_banner = None;
            return;
        }
        let fade_in = ease_out(((now - b.start) / 0.35) as f32);
        let fade_out = ((b.until - now) / 0.45).clamp(0.0, 1.0) as f32;
        let alpha = fade_in.min(fade_out);

        let shown: Vec<CommitInfo> = b.commits.iter().take(3).cloned().collect();
        let more = b.total.saturating_sub(shown.len());
        let width = (video.width() - 32.0).min(440.0);
        let h = 62.0 + 26.0 * shown.len() as f32 + if more > 0 { 18.0 } else { 0.0 } + 10.0;
        let rect = Rect::from_min_size(
            pos2(video.center().x - width / 2.0, video.top() + 14.0 - 16.0 * (1.0 - fade_in)),
            vec2(width, h),
        );
        // con el mouse encima no se va
        if ui.rect_contains_pointer(rect) && fade_in >= 1.0 {
            b.until = b.until.max(now + 1.2);
        }
        let (start, until, total) = (b.start, b.until, b.total);

        let mut p = ui.painter().clone();
        p.multiply_opacity(alpha);
        ui.interact(rect, Id::unique("capyro-update-banner"), Sense::click()); // que no pause el video
        p.rect_filled(rect.translate(vec2(0.0, 5.0)), 14.0, Color32::from_black_alpha(80));
        p.rect_filled(rect, 14.0, BG_PANEL);
        p.rect_stroke(rect, 14.0, Stroke::new(1.0, accent.gamma_multiply(0.5)), StrokeKind::Inside);

        // encabezado
        let hc = pos2(rect.left() + 30.0, rect.top() + 31.0);
        let pop = ease_out(((now - start - 0.15) / 0.3) as f32);
        p.circle_filled(hc, 15.0 + 5.0 * (1.0 - pop) * pop * 4.0 * 0.25, accent.gamma_multiply(0.2));
        p.circle_filled(hc, 12.0 * (0.6 + 0.4 * pop), accent);
        if pop > 0.05 {
            icons::check(&p, hc, 13.0 * pop, Color32::WHITE, 2.0);
        }
        let tx = hc.x + 24.0;
        p.text(pos2(tx, rect.top() + 22.0), Align2::LEFT_CENTER, "Capyro se actualizó", bold(14.0), TEXT);
        let cambios = if total == 1 {
            "1 cambio nuevo · reiniciá para aplicarlo".to_owned()
        } else {
            format!("{total} cambios nuevos · reiniciá para aplicarlos")
        };
        p.text(pos2(tx, rect.top() + 40.0), Align2::LEFT_CENTER, cambios, regular(11.5), TEXT_DIM);

        // botón Reiniciar
        let g = p.layout_no_wrap("Reiniciar".to_owned(), bold(12.0), Color32::WHITE);
        let pill = Rect::from_min_size(pos2(rect.right() - 16.0 - g.size().x - 24.0, hc.y - 14.0), vec2(g.size().x + 24.0, 28.0));
        let rb = animate_button(ui, Id::unique("capyro-banner-restart"), pill, alpha > 0.9);
        let pr = Rect::from_center_size(pill.center(), pill.size() * (1.0 + 0.04 * rb.hover - 0.08 * rb.press));
        if rb.hover > 0.001 {
            p.rect_filled(pr.expand(3.0 * rb.hover), pr.height() / 2.0 + 3.0, accent.gamma_multiply(0.2 * rb.hover));
        }
        p.rect_filled(pr, pr.height() / 2.0, accent.lerp_to_gamma(Color32::WHITE, 0.12 * rb.hover));
        p.galley(pr.center() - g.size() / 2.0, g, Color32::WHITE);

        // los commits
        p.line_segment(
            [pos2(rect.left() + 16.0, rect.top() + 58.0), pos2(rect.right() - 16.0, rect.top() + 58.0)],
            Stroke::new(1.0, Color32::from_white_alpha(14)),
        );
        let mut y = rect.top() + 76.0;
        for (i, c) in shown.iter().enumerate() {
            // cada línea entra un poquito después de la anterior
            let k = ease_out(((now - start - 0.2 - 0.08 * i as f64) / 0.3) as f32);
            let mut lp = p.clone();
            lp.multiply_opacity(k);
            let dx = 8.0 * (1.0 - k);
            let hg = lp.layout_no_wrap(c.hash.clone(), FontId::monospace(11.0), accent);
            let chip = Rect::from_min_size(pos2(rect.left() + 16.0 + dx, y - 10.0), hg.size() + vec2(12.0, 20.0 - hg.size().y));
            lp.rect_filled(chip, 6.0, accent.gamma_multiply(0.14));
            lp.galley(chip.center() - hg.size() / 2.0, hg, accent);
            let when = updater::ago(c.time);
            let wg = lp.layout_no_wrap(when, regular(10.5), TEXT_FAINT);
            lp.galley(pos2(rect.right() - 16.0 - wg.size().x, y - wg.size().y / 2.0), wg.clone(), TEXT_FAINT);
            let avail = rect.right() - 24.0 - wg.size().x - (chip.right() + 8.0);
            let max_chars = ((avail / 6.4) as usize).max(8);
            lp.text(
                pos2(chip.right() + 8.0, y),
                Align2::LEFT_CENTER,
                ellipsize(&c.subject, max_chars),
                if i == 0 { bold(12.0) } else { regular(12.0) },
                if i == 0 { TEXT } else { TEXT_DIM },
            );
            y += 26.0;
        }
        if more > 0 {
            p.text(pos2(rect.left() + 16.0, y - 4.0), Align2::LEFT_CENTER, format!("+{more} más"), regular(11.0), TEXT_FAINT);
        }

        // cuánto le queda al aviso
        let left = ((until - now) / BANNER_TIME).clamp(0.0, 1.0) as f32;
        let bar = Rect::from_min_size(pos2(rect.left() + 14.0, rect.bottom() - 4.0), vec2((rect.width() - 28.0) * left, 2.0));
        p.rect_filled(bar, 1.0, accent.gamma_multiply(0.5));

        if rb.resp.clicked() {
            self.restart(&ctx);
        }
        ctx.request_repaint();
    }

    /// Pantalla de carga: logo, "Cargando…" y un ecualizador. Con `intro`
    /// (segundos desde que terminó de cargar) todo se desvanece y el logo
    /// vuela hasta su lugar en el header.
    fn splash(&self, ui: &Ui, full: Rect, now: f64, intro: Option<f32>, logo_dest: Rect) {
        let painter = ui.painter();
        let accent = self.cfg.accent;
        let since = (now - self.boot.start.unwrap_or(now)) as f32;
        let appear = ease_out(since / 0.3);
        let fade = intro.map_or(1.0, |t| 1.0 - ease_out(t / 0.28));

        let c = full.center() - vec2(0.0, 18.0);
        let logo_c = c - vec2(0.0, 30.0);
        let mut p = painter.clone();
        p.multiply_opacity(appear * fade);

        // brillo que respira detrás del logo
        let breathe = 0.5 + 0.5 * (since * 2.6).sin();
        p.circle_filled(logo_c, 70.0 + 8.0 * breathe, accent.gamma_multiply(0.05 + 0.03 * breathe));
        p.circle_filled(logo_c, 56.0, accent.gamma_multiply(0.06));

        p.text(c + vec2(0.0, 42.0), Align2::CENTER_CENTER, "Capyro", bold(21.0), TEXT);

        // "Cargando" fijo y los puntitos al lado (así el texto no baila)
        let g = p.layout_no_wrap("Cargando".to_owned(), regular(13.0), TEXT_DIM);
        let dots = (since * 3.2) as usize % 4;
        let tl = pos2(c.x - g.size().x / 2.0 - 6.0, c.y + 68.0 - g.size().y / 2.0);
        p.galley(tl, g.clone(), TEXT_DIM);
        p.text(pos2(tl.x + g.size().x + 1.0, c.y + 68.0), Align2::LEFT_CENTER, ".".repeat(dots), regular(13.0), TEXT_DIM);

        // ecualizador, como el del README
        let n = 7;
        let (bw, gap) = (4.0, 4.0);
        let total_w = n as f32 * bw + (n - 1) as f32 * gap;
        let base = c.y + 104.0;
        for i in 0..n {
            let ph = since * 7.0 + i as f32 * 1.1;
            let h = 5.0 + 15.0 * (0.5 + 0.5 * ph.sin()) * (0.6 + 0.4 * (ph * 0.37).cos().abs());
            let x = c.x - total_w / 2.0 + i as f32 * (bw + gap);
            p.rect_filled(Rect::from_min_max(pos2(x, base - h), pos2(x + bw, base)), 2.0, accent);
        }

        p.text(
            pos2(full.center().x, full.bottom() - 26.0),
            Align2::CENTER_CENTER,
            format!("v{} · Just press play.", ffi::version()),
            regular(11.0),
            TEXT_FAINT,
        );

        // el logo: aparece creciendo y, al terminar, vuela al header
        let Some(logo) = &self.logo else { return };
        let start_rect = Rect::from_center_size(logo_c, Vec2::splat(96.0 * (0.9 + 0.1 * appear)));
        let (rect, alpha) = match intro {
            None => (start_rect, appear),
            Some(t) => {
                let k = ease_in_out(t / 0.46);
                let r = Rect::from_min_max(start_rect.min.lerp(logo_dest.min, k), start_rect.max.lerp(logo_dest.max, k));
                (r, 1.0)
            }
        };
        let mut lp = painter.clone();
        lp.multiply_opacity(alpha);
        lp.image(logo.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }

    /// Ventana sin bordes del sistema → bordes de 6px para redimensionar.
    fn resize_handles(ui: &mut Ui, full: Rect) {
        const B: f32 = 6.0;
        let (l, r, t, b) = (full.left(), full.right(), full.top(), full.bottom());
        let zones = [
            (Rect::from_min_max(pos2(l, t), pos2(l + B, t + B)), ResizeDirection::NorthWest, CursorIcon::ResizeNorthWest),
            (Rect::from_min_max(pos2(r - B, t), pos2(r, t + B)), ResizeDirection::NorthEast, CursorIcon::ResizeNorthEast),
            (Rect::from_min_max(pos2(l, b - B), pos2(l + B, b)), ResizeDirection::SouthWest, CursorIcon::ResizeSouthWest),
            (Rect::from_min_max(pos2(r - B, b - B), pos2(r, b)), ResizeDirection::SouthEast, CursorIcon::ResizeSouthEast),
            (Rect::from_min_max(pos2(l + B, t), pos2(r - B, t + B)), ResizeDirection::North, CursorIcon::ResizeNorth),
            (Rect::from_min_max(pos2(l + B, b - B), pos2(r - B, b)), ResizeDirection::South, CursorIcon::ResizeSouth),
            (Rect::from_min_max(pos2(l, t + B), pos2(l + B, b - B)), ResizeDirection::West, CursorIcon::ResizeWest),
            (Rect::from_min_max(pos2(r - B, t + B), pos2(r, b - B)), ResizeDirection::East, CursorIcon::ResizeEast),
        ];
        for (i, (rect, dir, cursor)) in zones.into_iter().enumerate() {
            let resp = ui.interact(rect, Id::unique(("capyro-resize", i)), Sense::drag());
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(cursor);
            }
            if resp.drag_started() {
                ui.ctx().send_viewport_cmd(ViewportCommand::BeginResize(dir));
            }
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = if self.core.current_file().is_empty() {
            "Capyro".to_owned()
        } else {
            format!("{} — Capyro", self.track_labels().0)
        };
        if title != self.last_title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }
}

impl eframe::App for CapyroApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.updater.cancel();
        if !self.restarting {
            // al reiniciar ya se guardó antes de abrir el Capyro nuevo
            self.save_session();
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // esquinas redondeadas transparentes
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let frame = ctx.cumulative_frame_nr();
        self.first_pass = frame != self.last_frame;
        self.last_frame = frame;
        let now = ctx.input(|i| i.time);

        /* --- arranque: primero se dibuja "Cargando", después se carga --- */
        if self.boot.start.is_none() {
            self.boot.start = Some(now);
            self.boot.frame0 = frame;
        }
        if !self.boot.loaded && (!self.boot.enabled || frame > self.boot.frame0) {
            self.finish_loading();
            self.boot.loaded = true;
            let earliest = if self.boot.enabled { self.boot.start.unwrap_or(now) + SPLASH_TIME } else { now };
            self.boot.ready_at = Some(earliest.max(now));
        }
        let ready = self.boot.ready_at.is_some_and(|r| now >= r);
        if ready && !self.boot.announced {
            self.boot.announced = true;
            if self.cfg.startup_sound {
                self.sfx(SFX_START);
            }
            if self.boot.autoplay && self.core.state() == State::Stopped {
                self.core.toggle(); // archivos por línea de comandos: just press play
            }
        }
        if ready {
            if let Some(msg) = self.startup_notice.take() {
                let is_error = msg.starts_with("No encontré");
                let secs = if msg.starts_with("Capyro actualizado") { 4.5 } else { 2.5 };
                self.toast_for(&ctx, msg, is_error, secs);
            }
        }

        self.core.update();
        if ready {
            self.handle_input(&ctx);
        }
        self.update_title(&ctx);
        self.sync_video_texture(&ctx);
        self.poll_file_dialog(&ctx);
        self.poll_screenshot(&ctx);
        self.poll_update(&ctx);
        if ready {
            self.check_hwdec(&ctx);
        }

        // visualizador: se alimenta con lo último que sonó
        let dt = ((now - self.last_time) as f32).clamp(0.0, 0.1);
        self.last_time = now;
        if self.cfg.visualizer && self.first_pass && self.core.media_kind() == MediaKind::Audio {
            let playing = self.core.state() == State::Playing;
            let n = self.viz.len();
            let got = if playing { self.core.scope(&mut self.viz.samples[..n]) } else { 0 };
            self.viz.update(self.core.audio_rate() as f32, dt, playing && got == n);
            if !self.viz.is_silent() {
                ctx.request_repaint_after(Duration::from_millis(16));
            }
        }

        // la sesión se guarda cada 15 s (y al cerrar)
        if ready && self.cfg.remember_session && now - self.last_session_save > 15.0 {
            self.last_session_save = now;
            self.save_session();
        }

        let (maximized, fullscreen) = ctx.input(|i| {
            (
                i.viewport().maximized.unwrap_or(false),
                i.viewport().fullscreen.unwrap_or(false),
            )
        });
        let square = maximized || fullscreen;

        let full = ui.max_rect();
        let radius = if square { 0 } else { WINDOW_RADIUS };
        ui.painter().rect_filled(full, radius, BG_WINDOW);
        if !square {
            ui.painter().rect_stroke(
                full,
                radius,
                Stroke::new(1.0, Color32::from_white_alpha(10)),
                StrokeKind::Inside,
            );
        }

        let inner = full.shrink(PAD);
        let header = Rect::from_min_size(inner.min, vec2(inner.width(), 46.0));
        let video_bottom = (inner.bottom() - BELOW_VIDEO).max(header.bottom() + 60.0);
        let video = Rect::from_min_max(pos2(inner.left(), header.bottom() + 12.0), pos2(inner.right(), video_bottom));
        let logo_dest = Rect::from_min_size(header.min, vec2(46.0, 46.0));

        if !ready {
            // pantalla de carga (se puede arrastrar la ventana igual)
            let drag = ui.interact(full, Id::unique("capyro-splash-drag"), Sense::drag());
            if drag.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            self.splash(ui, full, now, None, logo_dest);
            ctx.request_repaint();
            return;
        }

        let intro = self.boot.ready_at.map_or(INTRO_TIME, |r| (now - r) as f32);
        if self.boot.enabled && intro < INTRO_TIME {
            // la interfaz entra por partes: header, video y controles
            let part = |delay: f32| ease_out((intro - delay) / 0.42);
            let (e1, e2, e3) = (part(0.0), part(0.08), part(0.16));
            self.hide_header_logo = true;
            let mut child = ui.new_child(UiBuilder::new().max_rect(full));
            child.multiply_opacity(e1);
            self.header(&mut child, header.translate(vec2(0.0, -12.0 * (1.0 - e1))), maximized);
            let mut child = ui.new_child(UiBuilder::new().max_rect(full));
            child.multiply_opacity(e2);
            self.video_area(&mut child, Rect::from_center_size(video.center(), video.size() * (0.96 + 0.04 * e2)));
            let mut child = ui.new_child(UiBuilder::new().max_rect(full));
            child.multiply_opacity(e3);
            self.transport(&mut child, video_bottom + 18.0 * (1.0 - e3), inner);
            self.draw_toast(&mut child, video);
            self.splash(ui, full, now, Some(intro), logo_dest);
            self.play_click_if_needed(&ctx);
            ctx.request_repaint();
            return;
        }
        self.hide_header_logo = false;

        self.header(ui, header, maximized);
        self.video_area(ui, video);
        self.transport(ui, video_bottom, inner);
        self.settings_menu(ui);
        self.info_menu(ui);
        self.fx_menu(ui);
        self.changes_menu(ui);
        self.keys_editor(ui, video.expand2(vec2(0.0, 30.0)));
        // avisos arriba de todo (también de los menús abiertos)
        self.draw_toast(ui, video);
        self.draw_update_banner(ui, video);
        self.draw_update_offer(ui, video);
        if !square {
            Self::resize_handles(ui, full);
        }

        self.play_click_if_needed(&ctx);

        if self.core.state() == State::Playing {
            if self.core.media_kind() == MediaKind::Video {
                ctx.request_repaint(); // al ritmo de la pantalla (vsync)
            } else {
                ctx.request_repaint_after(Duration::from_millis(33));
            }
        }
    }
}

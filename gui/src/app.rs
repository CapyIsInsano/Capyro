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

use crate::system::config::{Action, Config};
use crate::ffi::{self, BuildInfo, Core, MediaKind, Repeat, State};
use crate::updater::{self, CommitInfo, FailKind, Outcome, Updater};

mod actions;
mod converter;
mod overlay;
mod sounds;
mod studio;
mod tutorial;
mod updates;
pub(crate) mod widgets;
/// Las vistas: la ventana, la clásica (video) y la de música.
mod views {
    pub(super) mod classic;
    pub(super) mod music;
    pub(super) mod window;
}
/// Los menús y paneles que se abren desde los botones.
mod menus {
    pub(super) mod effects;
    pub(super) mod info;
    pub(super) mod keys;
    pub(super) mod lists;
    pub(super) mod news;
    pub(super) mod settings;
}
// lo de cada archivo se usa en todos lados
use menus::info::*;
use menus::settings::*;
use sounds::*;
use updates::*;
use views::classic::*;
use views::window::*;
use widgets::*;
use menus::lists::PlPopup;
use crate::system::changelog::Release;
use crate::system::playlists::Playlists;
use crate::system::session::Session;
use crate::ui::viz::Visualizer;
use crate::ui::icons;
use crate::ui::theme::{self, *};

const LOGO_PNG: &[u8] = include_bytes!("../../assets/capyro_logo_128.png");
pub(crate) const ICON_PNG: &[u8] = include_bytes!("../../assets/capyro_logo_256.png");

/// Con `CAPYRO_DEBUG_FPS=1`, una vez por segundo: cuántos frames se
/// dibujaron y cuánto CPU costó cada uno (para medir en PCs lentas).
fn debug_fps(ctx: &egui::Context, cpu: Option<f32>) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ON.get_or_init(|| std::env::var_os("CAPYRO_DEBUG_FPS").is_some()) {
        return;
    }
    let now = ctx.input(|i| i.time);
    let id = Id::unique("capyro-debug-fps");
    let (mut t0, mut n, mut sum, mut max) = ctx.data(|d| d.get_temp::<(f64, u32, f32, f32)>(id)).unwrap_or((now, 0, 0.0, 0.0));
    n += 1;
    let c = cpu.unwrap_or(0.0) * 1000.0;
    sum += c;
    max = max.max(c);
    if now - t0 >= 1.0 {
        eprintln!("capyro: {:.0} fps · CPU por frame {:.1} ms (máx {:.1})", n as f64 / (now - t0), sum / n as f32, max);
        if std::env::var_os("CAPYRO_DEBUG_FPS").is_some_and(|v| v == "2") {
            for cause in ctx.repaint_causes() {
                eprintln!("capyro:   repintado por {cause}");
            }
        }
        t0 = now;
        n = 0;
        sum = 0.0;
        max = 0.0;
    }
    ctx.data_mut(|d| d.insert_temp(id, (t0, n, sum, max)));
}

/// Animaciones (menús, transiciones, botones): hasta 60 cuadros por segundo,
/// aunque la pantalla no tenga vsync.
pub(crate) const FRAME_SMOOTH: Duration = Duration::from_millis(16);
/// Lo que se mueve todo el tiempo (milisegundos, visualizador, chispitas del
/// tutorial): 30 por segundo se ven igual y cuestan la mitad.
pub(crate) const FRAME_AMBIENT: Duration = Duration::from_millis(33);

/// Arranque: la pantalla de "Cargando" dura al menos esto, y después la
/// interfaz entra animada en INTRO_TIME.
const SPLASH_TIME: f64 = 1.0;
const INTRO_TIME: f32 = 0.62;

/// Lo que está cargado y la lista, pedidos al núcleo solo cuando cambian
/// (antes se pedían varias veces por cuadro: textos copiados y, con la lista
/// abierta, una llamada por cada archivo de la lista, 30 veces por segundo).
#[derive(Default)]
struct NowPlaying {
    serial: Option<u64>,
    /// Ruta del archivo cargado ("" = nada).
    file: String,
    /// Título del metadato o, si no tiene, el nombre del archivo.
    title: String,
    artist: String,
    album: String,
    /// Vista de música: "2024 · Lo-fi" (año y género) y "MP3 · 44,1 kHz · 320 kbps".
    extra: String,
    tech: String,
    /// La cola (compartida: no se copia en cada cuadro).
    queue: std::rc::Rc<Vec<String>>,
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

/// Aplica el modo de rendimiento: el global de `perf` y las animaciones
/// propias de egui (tooltips, scroll), que en Bajo también se apagan.
pub(crate) fn apply_performance(ctx: &egui::Context, mode: crate::system::perf::Mode) {
    crate::system::perf::set(mode);
    let low = mode == crate::system::perf::Mode::Bajo;
    ctx.all_styles_mut(|s| {
        let d = egui::Style::default();
        s.animation_time = if low { 0.0 } else { d.animation_time };
        s.scroll_animation = if low { egui::style::ScrollAnimation::none() } else { d.scroll_animation };
    });
}

pub struct CapyroApp {
    core: Core,
    cfg: Config,
    logo: Option<TextureHandle>,
    show_playlist: bool,
    toast: Option<Toast>,
    last_title: String,
    /// Archivo actual, metadatos y cola (ver [`NowPlaying`]).
    np: NowPlaying,
    /// ¿Siguen en el disco los archivos de las playlists? (y cuándo se miró)
    exists_cache: std::collections::HashMap<String, (bool, f64)>,
    /// Interfaz adaptativa: ¿se está mostrando la vista de música? y desde
    /// cuándo (para la transición).
    layout_music: bool,
    layout_since: f64,
    /// Pestaña y scroll de la burbuja de funciones de la vista de música.
    dock_tab: views::music::DockTab,
    /// Capyro Studio: solo existe mientras está abierto (al cerrarlo se suelta todo).
    studio: Option<studio::Studio>,
    /// Capyro Converter: existe mientras está abierto o convirtiendo.
    converter: Option<converter::Converter>,
    dock_scroll: [f32; 3],
    /// Videos: la barra de abajo escondida, y cuándo se movió el mouse por
    /// última vez (el botón para volver a mostrarla aparece al moverlo).
    bar_hidden: bool,
    last_pointer_move: f64,
    /// Modo Alto: fondo animado (degradé, partículas, luz del mouse).
    backdrop: crate::ui::alto::Backdrop,
    /// Archivo actual (para notar cuándo cambia) y cuándo cambió.
    track_key: String,
    track_at: Option<f64>,
    /// Textura del frame actual (video o carátula) y su número de serie.
    video_tex: Option<TextureHandle>,
    video_serial: u64,
    /// Copia del último cuadro (se reutiliza: sin reservar memoria por cuadro).
    frame_buf: Vec<u8>,
    /// Cuadros de video que de verdad se mostraron (burbuja de fps).
    fps_meter: crate::media::videoinfo::FpsMeter,
    /// Tamaño del área de video que se le pasó al núcleo la última vez.
    video_target: (u32, u32),
    /// Iluminación cinematográfica (modo de rendimiento Alto).
    ambient: crate::system::perf::Ambient,
    /// Área de video del cuadro anterior (lo que tapa la luz de la ventana).
    ambient_under: Option<Rect>,
    video_aspect: f32,
    /// Toast que hay que mostrar en el primer frame (todavía no hay `ctx.time`).
    startup_notice: Option<String>,
    /// Diálogo de "Agregar archivos" abierto en otro hilo; llega el resultado acá.
    dialog: Option<Receiver<Vec<PathBuf>>>,
    /// Menú del engranaje abierto, y dónde está el engranaje (para anclarlo).
    show_settings: bool,
    gear_rect: Rect,
    /// ⚙ en una ventana bajita: cuánto se corrió con la rueda.
    settings_scroll: f32,
    /// Disco de vinilo: hacia dónde apunta (radianes) y a qué velocidad gira
    /// (vueltas por segundo; arranca y frena despacio).
    vinyl_angle: f32,
    vinyl_vel: f32,
    /// Menú de información (versión y formatos), sobre "Lista".
    show_info: bool,
    info_rect: Rect,
    /// Formatos que acepta el núcleo, en minúscula: (video, audio).
    formats: (Vec<String>, Vec<String>),
    /// Formato cuyo resumen está abierto en el menú ⓘ (uno a la vez).
    info_format: Option<String>,
    /// ⓘ → "Formatos soportados (abrir)": la lista de formatos desplegada.
    info_formats_open: bool,
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
    /// Qué menús/paneles estaban abiertos el frame anterior (para el sonido de cerrar).
    ui_open: std::cell::Cell<u32>,
    /// Dónde quedó cada parte de la interfaz este frame (para el tutorial).
    ui_rects: std::collections::HashMap<&'static str, Rect>,
    /// Tutorial de bienvenida: en curso, foto del autor y si falta mostrarlo.
    tutorial: Option<tutorial::Tutorial>,
    avatar: Option<TextureHandle>,
    tutorial_pending: bool,
    /// Se está reiniciando para aplicar una actualización.
    restarting: bool,
    /// Cerrar de verdad (atajo de salir, desinstalar): sin segundo plano.
    quit_for_real: bool,
    /// La ventana volvió del segundo plano: sin pantalla de carga ni sonido
    /// de inicio, y sin tocar lo que ya está sonando. Trae los archivos que
    /// llegaron de otro Capyro (si hay) para agregarlos.
    resumed: Option<Vec<PathBuf>>,
    /// Playlists guardadas y qué pestaña de la Lista se ve (None = la Cola).
    playlists: Playlists,
    list_tab: Option<usize>,
    /// Menucito "Agregar a…" abierto.
    pl_popup: Option<PlPopup>,
    /// Renombrando: (playlist, texto, pedir foco).
    pl_rename: Option<(usize, String, bool)>,
    /// Tacho tocado una vez: (playlist, cuándo). El segundo toque borra.
    pl_delete_armed: Option<(usize, f64)>,
    /// "Desinstalar Capyro": primer clic (armado) y cuándo; el segundo confirma.
    uninstall_armed: Option<f64>,
    /// "Borrar caché": armado (cuándo y cuánto ocupa) y el trabajo en otro hilo.
    cache_armed: Option<(f64, u64)>,
    cache_job: Option<Receiver<CacheMsg>>,
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

/// Nombre de lo que dibuja la ventana (la placa de video, o "llvmpipe" si es el procesador).
fn renderer_name(cc: &eframe::CreationContext<'_>) -> String {
    if let Some(gl) = cc.gl.as_ref() {
        use eframe::glow::HasContext as _;
        // SAFETY: el contexto de OpenGL está activo mientras se crea la app.
        return unsafe { gl.get_parameter_string(eframe::glow::RENDERER) };
    }
    #[cfg(any(windows, target_os = "macos"))]
    if let Some(rs) = cc.wgpu_render_state.as_ref() {
        let info = rs.adapter.get_info();
        let cpu = info.device_type == eframe::wgpu::DeviceType::Cpu;
        return format!("{}{}", info.name, if cpu { " (cpu)" } else { "" });
    }
    String::new()
}

pub fn window_icon() -> Option<egui::IconData> {
    let (width, height, rgba) = decode_png(ICON_PNG)?;
    Some(egui::IconData { rgba, width, height })
}

impl CapyroApp {
    /// `resumed`: la ventana vuelve del segundo plano (ver background.rs),
    /// con los archivos que haya que agregar.
    pub(crate) fn new(cc: &eframe::CreationContext<'_>, core: Core, cfg: Config, resumed: Option<Vec<PathBuf>>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        theme::apply_visuals(&cc.egui_ctx, cfg.accent);
        apply_performance(&cc.egui_ctx, cfg.performance);
        // Mac: que un archivo abierto desde el Finder despierte a la ventana
        #[cfg(target_os = "macos")]
        crate::system::macos::set_wake(Some(cc.egui_ctx.clone()));
        // sin placa de video (se dibuja con el procesador): redibujar menos seguido
        let gpu = renderer_name(cc);
        crate::system::perf::set_software(crate::system::perf::is_software_renderer(&gpu));
        eprintln!(
            "capyro: dibujo con {}{}",
            if gpu.is_empty() { "?" } else { gpu.as_str() },
            if crate::system::perf::software() { " (sin placa de video: menos cuadros por segundo)" } else { "" }
        );

        let logo = decode_png(LOGO_PNG).map(|(w, h, rgba)| {
            let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            cc.egui_ctx.load_texture("capyro-logo", img, TextureOptions::LINEAR)
        });

        // al volver del segundo plano, lo que suena sigue como estaba
        if resumed.is_none() {
            core.set_volume(cfg.start_volume);
            core.set_speed(cfg.start_speed);
            core.set_shuffle(cfg.start_shuffle);
            core.set_repeat(cfg.start_repeat);
        }
        core.set_auto_next(cfg.auto_next);
        core.set_hwdec(cfg.decoding);
        let startup_notice = core
            .audio_backend()
            .is_empty()
            .then(|| "No encontré dispositivo de audio: reproduzco sin sonido".to_owned());
        // en Bajo no hay pantalla de carga: la interfaz aparece directo
        let splash = cfg.splash && cfg.performance != crate::system::perf::Mode::Bajo && resumed.is_none();
        // Lo que dejó la actualización anterior (Windows: los .old).
        updater::cleanup_after_update();
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
        // no se pudo quedar en segundo plano (la bandeja falló): contarlo
        let startup_notice = crate::background::take_notice().or(startup_notice);

        Self {
            core,
            cfg,
            logo,
            show_playlist: false,
            toast: None,
            last_title: String::new(),
            np: NowPlaying::default(),
            exists_cache: Default::default(),
            layout_music: false,
            layout_since: 0.0,
            dock_tab: Default::default(),
            studio: None,
            converter: None,
            dock_scroll: [0.0; 3],
            bar_hidden: false,
            last_pointer_move: 0.0,
            backdrop: Default::default(),
            track_key: String::new(),
            track_at: None,
            video_tex: None,
            video_serial: 0,
            frame_buf: Vec::new(),
            fps_meter: Default::default(),
            video_target: (0, 0),
            ambient: Default::default(),
            ambient_under: None,
            video_aspect: 16.0 / 9.0,
            startup_notice,
            dialog: None,
            show_settings: false,
            settings_scroll: 0.0,
            vinyl_angle: 0.0,
            vinyl_vel: 0.0,
            gear_rect: Rect::NOTHING,
            show_info: false,
            info_rect: Rect::NOTHING,
            formats: (ffi::supported_video(), ffi::supported_audio()),
            info_format: None,
            info_formats_open: false,
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
            uninstall_armed: None,
            cache_armed: None,
            cache_job: None,
            show_keys: false,
            keys_capture: None,
            keys_adding: false,
            show_changes: false,
            changes_rect: Rect::NOTHING,
            release: crate::system::changelog::current(),
            changes_seen: String::new(),
            build_info,
            update_banner: None,
            show_offer: false,
            hwdec_warned: 0,
            sfx_frame: std::cell::Cell::new(u64::MAX - 1),
            ui_open: std::cell::Cell::new(0),
            ui_rects: std::collections::HashMap::new(),
            tutorial: None,
            avatar: None,
            tutorial_pending: false,
            restarting: false,
            quit_for_real: false,
            resumed,
            rng: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64 | 1)
                .unwrap_or(0x9E37_79B9_7F4A_7C15),
        }
    }

    /// La carga del arranque: sonidos (se decodifican en el núcleo) y la
    /// sesión anterior. Corre mientras se ve la pantalla de "Cargando".
    pub(crate) fn finish_loading(&mut self) {
        if self.resumed.is_some() {
            self.finish_resume();
            return;
        }
        for (slot, data) in SOUNDS.iter().enumerate() {
            if let Err(e) = self.core.sfx_load(slot, data) {
                eprintln!("capyro: no se pudo cargar el sonido {slot}: {}", e.0);
            }
        }
        self.playlists = Playlists::load();
        // Mac: Capyro se abrió con archivos desde el Finder (doble clic, "Abrir
        // con…"): son como los de la línea de comandos
        #[cfg(target_os = "macos")]
        self.add_launch_files(crate::system::macos::take_opened_files());
        // primera vez: el tutorial arranca cuando termina de entrar la interfaz
        self.tutorial_pending = tutorial::first_time();
        if self.tutorial_pending {
            self.boot.autoplay = false;
        }

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

    /// Archivos con los que se abrió Capyro (en la Mac, desde el Finder): a la
    /// lista y el primero que abra queda cargado (suena solo, como con los de
    /// la línea de comandos).
    #[cfg(target_os = "macos")]
    fn add_launch_files(&mut self, files: Vec<PathBuf>) {
        let first_new = self.core.playlist_len();
        for f in &files {
            if let Err(e) = self.core.playlist_add(f) {
                eprintln!("capyro: {}: {}", f.display(), e.0);
            }
        }
        for i in first_new..self.core.playlist_len() {
            if self.core.playlist_select(i).is_ok() {
                break;
            }
        }
    }

    /// Volver del segundo plano: los sonidos ya están en el núcleo y la
    /// lista sigue sonando; de la sesión, solo lo que es de la ventana.
    fn finish_resume(&mut self) {
        self.playlists = Playlists::load();
        self.boot.autoplay = false;
        if self.cfg.remember_session {
            if let Some(sess) = Session::load() {
                self.changes_seen = sess.seen_changes;
                self.show_remaining = sess.show_remaining;
            }
        }
    }

    /// Al cerrar la ventana: ¿Capyro queda sonando en la bandeja?
    fn goes_to_background(&self) -> bool {
        self.cfg.background
            && !self.quit_for_real
            && !self.restarting
            && self.tutorial.is_none()
            && crate::background::available()
    }

    /// Los menús emergentes son de a uno.
    pub(crate) fn close_menus(&mut self) {
        self.show_settings = false;
        self.show_info = false;
        self.show_fx = false;
        self.show_changes = false;
    }

    /// Título del metadato (o nombre de archivo) y artista.
    pub(crate) fn track_labels(&self) -> (String, String) {
        (self.np.title.clone(), self.np.artist.clone())
    }

    /// Vuelve a pedirle al núcleo el archivo, los metadatos y la cola, solo
    /// si cambiaron (una llamada barata por cuadro).
    pub(crate) fn refresh_now_playing(&mut self) {
        let serial = self.core.change_serial();
        if self.np.serial == Some(serial) {
            return;
        }
        let file = self.core.current_file();
        let title = self.core.title();
        let title = if title.trim().is_empty() { file_name(&file) } else { title };
        // año, género y datos técnicos: solo al cambiar de archivo (se lee el archivo)
        let (extra, tech) = if file == self.np.file {
            (std::mem::take(&mut self.np.extra), std::mem::take(&mut self.np.tech))
        } else if file.is_empty() || self.core.media_kind() != MediaKind::Audio {
            (String::new(), String::new())
        } else {
            let path = Path::new(&file);
            let extra = ffi::read_tags(path)
                .map(|t| {
                    let year: String = t.year.trim().chars().take(4).collect();
                    [year, t.genre.trim().to_owned()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
                })
                .unwrap_or_default();
            (extra, ffi::audio_info(path).map(|a| a.summary()).unwrap_or_default())
        };
        self.np = NowPlaying {
            serial: Some(serial),
            title,
            artist: self.core.artist(),
            album: self.core.album(),
            extra,
            tech,
            queue: std::rc::Rc::new(self.core.playlist()),
            file,
        };
    }

    pub(crate) fn update_title(&mut self, ctx: &egui::Context) {
        // cambió el archivo: transición de entrada (modo Alto)
        let key = &self.np.file;
        if *key != self.track_key {
            if !key.is_empty() {
                self.track_at = Some(ctx.input(|i| i.time));
            }
            self.track_key = key.clone();
        }
        let title = if self.np.file.is_empty() {
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
        #[cfg(target_os = "macos")]
        crate::system::macos::set_wake(None);
        crate::background::set_pending(self.goes_to_background());
        if !self.restarting {
            // al reiniciar ya se guardó antes de abrir el Capyro nuevo
            self.save_session();
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if theme::OPAQUE_WINDOW {
            theme::BG_WINDOW.to_normalized_gamma_f32()
        } else {
            [0.0, 0.0, 0.0, 0.0] // esquinas redondeadas transparentes
        }
    }

    fn ui(&mut self, ui: &mut Ui, frame_info: &mut eframe::Frame) {
        crate::system::render::FIRST_FRAME.store(true, std::sync::atomic::Ordering::Relaxed);
        let ctx = ui.ctx().clone();
        debug_fps(&ctx, frame_info.info().cpu_usage);
        let frame = ctx.cumulative_frame_nr();
        self.first_pass = frame != self.last_frame;
        self.last_frame = frame;
        let now = ctx.input(|i| i.time);

        if ctx.input(|i| i.pointer.delta() != Vec2::ZERO) {
            self.last_pointer_move = now;
        }
        // para ubicar el mini reproductor del segundo plano antes de abrirlo
        if self.first_pass {
            let (size, ppp) = ctx.input(|i| (i.viewport().monitor_size, i.viewport().native_pixels_per_point));
            crate::background::remember_monitor(size, ppp);
        }

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
            // volvió del segundo plano con archivos (se abrió Capyro otra vez)
            if let Some(files) = self.resumed.as_mut().map(std::mem::take).filter(|f| !f.is_empty()) {
                self.add_paths(&ctx, files, true);
            }
            if self.cfg.startup_sound && self.resumed.is_none() {
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
        // Mac: archivos abiertos desde el Finder con Capyro ya abierto
        #[cfg(target_os = "macos")]
        if ready && self.first_pass {
            let _ = crate::system::macos::take_reopen(); // con la ventana abierta no hace nada
            let files = crate::system::macos::take_opened_files();
            if !files.is_empty() {
                let play_now = !self.panel_open();
                self.add_paths(&ctx, files, play_now);
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
        }
        self.refresh_now_playing();
        if ready {
            self.handle_input(&ctx);
        }
        self.update_title(&ctx);
        self.sync_video_texture(&ctx);
        self.poll_file_dialog(&ctx);
        self.poll_screenshot(&ctx);
        self.poll_update(&ctx);
        self.poll_converter(&ctx);
        if ready {
            self.check_hwdec(&ctx);
        }

        // visualizador: se alimenta con lo último que sonó
        let dt = ((now - self.last_time) as f32).clamp(0.0, 0.1);
        self.last_time = now;
        // disco de vinilo: gira a 33⅓ por la velocidad mientras suena (en Bajo, quieto)
        if self.cfg.vinyl && self.core.media_kind() == MediaKind::Audio {
            let playing = self.core.state() == State::Playing;
            let target = if playing && crate::system::perf::animations() {
                crate::ui::vinyl::RPS * self.core.speed() as f32
            } else {
                0.0
            };
            if crate::ui::vinyl::step(&mut self.vinyl_angle, &mut self.vinyl_vel, target, dt) {
                ctx.request_repaint_after(crate::system::perf::ambient());
            }
        }
        if self.cfg.wants_spectrum() && self.first_pass && self.core.media_kind() == MediaKind::Audio {
            let playing = self.core.state() == State::Playing;
            let n = self.viz.len();
            let got = if playing { self.core.scope(&mut self.viz.samples[..n]) } else { 0 };
            self.viz.update(self.core.audio_rate() as f32, dt, playing && got == n);
            if !self.viz.is_silent() {
                // las barras se suavizan solas (viz.rs): a 30 cuadros se ven igual de fluidas
                ctx.request_repaint_after(crate::system::perf::ambient());
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
        let inner = full.shrink(PAD);
        let header = Rect::from_min_size(inner.min, vec2(inner.width(), 46.0));
        let content = Rect::from_min_max(pos2(inner.left(), header.bottom() + 12.0), inner.max);

        /* --- interfaz adaptativa: ¿audio o video? --- */
        // audio + opción activada + lugar en la ventana = vista de música
        // (nunca durante el tutorial: explica la vista clásica)
        let music = if self.cfg.music_view && self.tutorial.is_none() && self.core.media_kind() == MediaKind::Audio {
            views::music::layout_for(content)
        } else {
            None
        };
        if music.is_some() != self.layout_music {
            self.layout_music = music.is_some();
            self.layout_since = now;
            self.close_menus();
            self.show_playlist = false;
        }
        // entrada de la vista nueva (Medio y Alto; en Bajo, directa)
        let appear = ease_out(crate::system::perf::k(((now - self.layout_since) / 0.32) as f32));
        if appear < 1.0 {
            ctx.request_repaint_after(FRAME_SMOOTH);
        }
        // videos: la barra de abajo se puede esconder
        let bar_ok = music.is_none() && self.cfg.hide_bar_button && self.tutorial.is_none() && self.core.media_kind() == MediaKind::Video;
        let bar_k = ctx.animate_bool_with_time(Id::unique("capyro-bar-hidden"), self.bar_hidden && bar_ok, crate::system::perf::t(0.28));
        let bar_e = 1.0 - (1.0 - bar_k).powi(3);
        if (0.001..0.999).contains(&bar_k) {
            ctx.request_repaint_after(FRAME_SMOOTH);
        }
        let video_bottom = (inner.bottom() - BELOW_VIDEO * (1.0 - bar_e)).max(header.bottom() + 60.0);
        let video = Rect::from_min_max(pos2(inner.left(), header.bottom() + 12.0), pos2(inner.right(), video_bottom));
        let logo_dest = Rect::from_min_size(header.min + vec2(views::window::TRAFFIC_LIGHTS, 0.0), vec2(46.0, 46.0));

        // el fondo de la ventana, sin pintar lo que después tapa algo opaco
        // (el video o los paneles): menos trabajo para la placa de video
        let intro_t = self.boot.ready_at.map_or(INTRO_TIME, |r| (now - r) as f32);
        let settled = ready && !(self.boot.enabled && intro_t < INTRO_TIME) && appear >= 1.0;
        let holes: Vec<(Rect, f32)> = match (&music, settled) {
            (Some(l), true) => vec![(l.left, 12.0), (l.right, 12.0)],
            (None, true) => vec![(video, 10.0)],
            _ => Vec::new(),
        };
        theme::fill_window(ui.painter(), full, radius, &holes);
        if !square {
            ui.painter().rect_stroke(
                full,
                radius,
                Stroke::new(1.0, Color32::from_white_alpha(10)),
                StrokeKind::Inside,
            );
        }

        // Alto: la luz del video se derrama por toda la ventana
        if self.cfg.performance == crate::system::perf::Mode::Alto {
            if self.ambient.step(now) {
                ctx.request_repaint_after(crate::system::perf::ambient());
            }
            let accent = self.cfg.accent;
            let (focused, pointer) = ctx.input(|i| (i.focused, i.pointer.hover_pos()));
            let audio_playing = self.core.media_kind() == MediaKind::Audio && self.core.state() == State::Playing;
            let energy = if audio_playing && self.cfg.wants_spectrum() { self.viz.energy() } else { 0.0 };
            if let Some(d) = self.backdrop.step(now, focused, pointer, energy, accent, self.ambient.mean()) {
                ctx.request_repaint_after(d);
            }
            let area = full.shrink(1.0);
            // degradé que se mueve, la luz del video, el polvito y la luz del mouse
            let reach = full.width().max(full.height()) * 0.42;
            // (el área de video es opaca: ahí no hace falta, tiene su propia luz)
            let under = self.ambient_under.map(|r: Rect| r.shrink(4.0));
            let gradient = self.backdrop.gradient(area, accent);
            if self.ambient.visible() {
                // luz del video y degradé en una sola malla (una sola pasada)
                // (en la vista de música la carátula está a la vista y sin
                // un área de video alrededor: un poco menos de luz)
                let strength = if self.layout_music { 0.30 } else { 0.42 };
                self.ambient.paint(ui.painter(), area, strength, reach, under, Some(&gradient));
            } else {
                gradient.paint(ui.painter(), under);
            }
            self.backdrop.paint_particles(ui.painter(), area, accent, under, 28);
            self.backdrop.paint_cursor(ui.painter(), area, accent, self.ambient_under);
        }


        if !ready {
            // pantalla de carga (se puede arrastrar la ventana igual)
            let drag = ui.interact(full, Id::unique("capyro-splash-drag"), Sense::drag());
            if drag.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            self.splash(ui, full, now, None, logo_dest);
            ctx.request_repaint_after(FRAME_SMOOTH);
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
            if let Some(l) = &music {
                let mut child = ui.new_child(UiBuilder::new().max_rect(full));
                child.multiply_opacity(e2);
                self.music_view(&mut child, l, e3);
                self.layout_since = now - 1.0; // ya entró con la intro
            } else {
                let mut child = ui.new_child(UiBuilder::new().max_rect(full));
                child.multiply_opacity(e2);
                self.video_area(&mut child, Rect::from_center_size(video.center(), video.size() * (0.96 + 0.04 * e2)));
                let mut child = ui.new_child(UiBuilder::new().max_rect(full));
                child.multiply_opacity(e3);
                self.transport(&mut child, video_bottom + 18.0 * (1.0 - e3), inner);
                self.draw_toast(&mut child, video);
            }
            self.splash(ui, full, now, Some(intro), logo_dest);
            self.play_click_if_needed(&ctx);
            ctx.request_repaint_after(FRAME_SMOOTH);
            return;
        }
        self.hide_header_logo = false;
        if self.tutorial_pending {
            self.tutorial_pending = false;
            self.start_tutorial(&ctx);
        }
        self.header(ui, header, maximized);
        // dónde van los avisos y el editor de atajos
        let notice_area;
        if let Some(l) = &music {
            /* --- vista de música --- */
            let mut child = ui.new_child(UiBuilder::new().max_rect(full));
            if appear < 1.0 {
                child.multiply_opacity(appear);
            }
            self.music_view(&mut child, l, appear);
            // de los menús que se abren, en esta vista queda el de efectos
            // (los demás están fijos en la burbuja de la derecha)
            self.fx_menu(ui);
            notice_area = l.center;
            self.keys_editor(ui, content);
        } else {
            /* --- vista clásica --- */
            self.ui_rects.insert("video", video);
            let mut child = ui.new_child(UiBuilder::new().max_rect(full));
            if appear < 1.0 {
                child.multiply_opacity(appear);
            }
            self.video_area(&mut child, video);
            if bar_e < 0.999 {
                // la barra baja y se esconde (recortada debajo del video)
                let mut bar = ui.new_child(UiBuilder::new().max_rect(full));
                bar.set_clip_rect(Rect::from_min_max(pos2(full.left(), video_bottom), full.max).intersect(ui.clip_rect()));
                if bar_e > 0.001 || appear < 1.0 {
                    bar.multiply_opacity((1.0 - bar_e) * appear);
                }
                self.transport(&mut bar, video_bottom, inner);
            }
            if bar_ok && !self.show_playlist {
                self.bar_toggle(ui, video, now);
            }
            self.settings_menu(ui, None);
            self.info_menu(ui, None);
            self.fx_menu(ui);
            self.changes_menu(ui, None);
            notice_area = video;
            self.keys_editor(ui, video.expand2(vec2(0.0, 30.0)));
        }
        // avisos arriba de todo (también de los menús abiertos)
        let toast_area = if music.is_some() {
            Rect::from_min_max(notice_area.min, pos2(notice_area.right(), notice_area.top() + 80.0))
        } else {
            notice_area
        };
        self.draw_update_banner(ui, notice_area);
        self.draw_update_offer(ui, notice_area);
        // Capyro Studio y Capyro Converter: paneles encima de todo (si están abiertos)
        self.studio_overlay(ui, full);
        self.converter_overlay(ui, full);
        // los avisos, arriba de los paneles
        self.draw_toast(ui, toast_area);
        if !square {
            Self::resize_handles(ui, full);
        }
        // las ondas de los clics (Alto), arriba de los botones
        crate::ui::alto::paint_ripples(&ctx, ui.painter(), self.cfg.accent);
        // el tutorial va encima de todo y se queda con el mouse y el teclado
        self.tutorial_overlay(ui, full);

        self.play_click_if_needed(&ctx);

        if self.core.state() == State::Playing {
            if self.core.media_kind() == MediaKind::Video {
                // al ritmo del video (24/30 cuadros), no al de la pantalla
                match self.core.video_next_due() {
                    Some(due) => ctx.request_repaint_after(Duration::from_secs_f64(due.clamp(0.002, 0.1))),
                    None => ctx.request_repaint_after(Duration::from_millis(8)), // el decodificador viene atrás
                }
            } else {
                let bar_w = match &music {
                    Some(l) => l.center.width(),
                    None => inner.width(),
                };
                ctx.request_repaint_after(self.audio_repaint_interval(bar_w));
            }
        }
    }
}

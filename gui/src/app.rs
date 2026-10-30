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
mod music;
mod tutorial;
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
/// 6 clic de botón, 7 captura, 8 "lista para descargar", 9 cerrar/retraer.
const SOUNDS: [&[u8]; 10] = [
    include_bytes!("../../assets/sounds/capybara1.mp3"),
    include_bytes!("../../assets/sounds/capybara2.mp3"),
    include_bytes!("../../assets/sounds/capybara3.mp3"),
    include_bytes!("../../assets/sounds/error.mp3"),
    include_bytes!("../../assets/sounds/inicio.mp3"),
    include_bytes!("../../assets/sounds/actualizacion.mp3"),
    include_bytes!("../../assets/sounds/click.mp3"),
    include_bytes!("../../assets/sounds/photo.mp3"),
    include_bytes!("../../assets/sounds/ready.mp3"),
    include_bytes!("../../assets/sounds/retraction.mp3"),
];
const SFX_CAPYBARAS: usize = 3;
const SFX_ERROR: usize = 3;
const SFX_START: usize = 4;
const SFX_UPDATE: usize = 5;
const SFX_CLICK: usize = 6;
const SFX_PHOTO: usize = 7;
const SFX_READY: usize = 8;
const SFX_RETRACT: usize = 9;
/// Con `CAPYRO_DEBUG_SFX=1` cada sonido se anota en la terminal (para probar
/// sin parlantes o ver cuál suena).
fn debug_sfx(slot: usize) {
    const NAMES: [&str; 10] =
        ["capibara1", "capibara2", "capibara3", "error", "inicio", "actualizacion", "click", "photo", "ready", "retraction"];
    if std::env::var_os("CAPYRO_DEBUG_SFX").is_some() {
        eprintln!("capyro: sonido {}", NAMES.get(slot).copied().unwrap_or("?"));
    }
}

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

/// Aplica el modo de rendimiento: el global de `perf` y las animaciones
/// propias de egui (tooltips, scroll), que en Bajo también se apagan.
pub(crate) fn apply_performance(ctx: &egui::Context, mode: crate::perf::Mode) {
    crate::perf::set(mode);
    let low = mode == crate::perf::Mode::Bajo;
    ctx.all_styles_mut(|s| {
        let d = egui::Style::default();
        s.animation_time = if low { 0.0 } else { d.animation_time };
        s.scroll_animation = if low { egui::style::ScrollAnimation::none() } else { d.scroll_animation };
    });
}

/// `text` recortado con "…" para que entre en `max_w` puntos.
pub(crate) fn fit_text(painter: &egui::Painter, text: &str, font: FontId, max_w: f32) -> String {
    let width = |s: &str| painter.layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE).size().x;
    if width(text) <= max_w {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    // búsqueda binaria del largo que entra
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let s: String = chars[..mid].iter().collect::<String>().trim_end().to_owned() + "…";
        if width(&s) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo].iter().collect::<String>().trim_end().to_owned() + "…"
}

fn animate_button(ui: &Ui, id: Id, rect: Rect, enabled: bool) -> Anim {
    let resp = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
    let ctx = ui.ctx();
    let hovered = enabled && resp.hovered();
    let hover = ctx.animate_bool_with_time(id.with("hover"), hovered, crate::perf::t(0.14));
    if resp.clicked() {
        // el clic suena al final del frame (si el botón no hizo sonar otra cosa)
        ctx.data_mut(|d| d.insert_temp(Id::unique(CLICK_FLAG), true));
    }
    let press = ctx.animate_bool_with_time(id.with("press"), enabled && resp.is_pointer_button_down_on(), crate::perf::t(0.07));
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
    /// Archivo actual, metadatos y cola (ver [`NowPlaying`]).
    np: NowPlaying,
    /// ¿Siguen en el disco los archivos de las playlists? (y cuándo se miró)
    exists_cache: std::collections::HashMap<String, (bool, f64)>,
    /// Interfaz adaptativa: ¿se está mostrando la vista de música? y desde
    /// cuándo (para la transición).
    layout_music: bool,
    layout_since: f64,
    /// Pestaña y scroll de la burbuja de funciones de la vista de música.
    dock_tab: music::DockTab,
    dock_scroll: [f32; 3],
    /// Videos: la barra de abajo escondida, y cuándo se movió el mouse por
    /// última vez (el botón para volver a mostrarla aparece al moverlo).
    bar_hidden: bool,
    last_pointer_move: f64,
    /// Modo Alto: fondo animado (degradé, partículas, luz del mouse).
    backdrop: crate::alto::Backdrop,
    /// Archivo actual (para notar cuándo cambia) y cuándo cambió.
    track_key: String,
    track_at: Option<f64>,
    /// Textura del frame actual (video o carátula) y su número de serie.
    video_tex: Option<TextureHandle>,
    video_serial: u64,
    /// Copia del último cuadro (se reutiliza: sin reservar memoria por cuadro).
    frame_buf: Vec<u8>,
    /// Cuadros de video que de verdad se mostraron (burbuja de fps).
    fps_meter: crate::videoinfo::FpsMeter,
    /// Tamaño del área de video que se le pasó al núcleo la última vez.
    video_target: (u32, u32),
    /// Iluminación cinematográfica (modo de rendimiento Alto).
    ambient: crate::perf::Ambient,
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

/// Carpeta de Imágenes del usuario + "/Capyro".
fn default_screenshot_dir() -> PathBuf {
    crate::paths::default_screenshot_dir()
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
        apply_performance(&cc.egui_ctx, cfg.performance);

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
        // en Bajo no hay pantalla de carga: la interfaz aparece directo
        let splash = cfg.splash && cfg.performance != crate::perf::Mode::Bajo;
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
            ui_open: std::cell::Cell::new(0),
            ui_rects: std::collections::HashMap::new(),
            tutorial: None,
            avatar: None,
            tutorial_pending: false,
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
    /// Cerrar/retraer suena al revés que abrir: si en este frame algo se
    /// cerró (y nada se abrió), suena "retraction" en vez del clic.
    fn play_click_if_needed(&self, ctx: &egui::Context) {
        let clicked = ctx.data_mut(|d| d.remove_temp::<bool>(Id::unique(CLICK_FLAG))).unwrap_or(false);
        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        // menús, paneles y pantalla completa: un bit cada uno
        let open = [
            self.show_settings,
            self.show_info,
            self.show_fx,
            self.show_changes,
            self.show_keys,
            self.show_playlist,
            self.show_offer,
            fullscreen,
        ]
        .iter()
        .enumerate()
        .fold(0u32, |m, (i, &on)| m | (u32::from(on) << i));
        let before = self.ui_open.replace(open);
        let closed = before & !open != 0;
        let opened = open & !before != 0;
        if !self.cfg.click_sound || self.sfx_frame.get() == self.last_frame {
            return; // apagado, o ya sonó algo propio (error, ready, captura...)
        }
        if closed && !opened {
            debug_sfx(SFX_RETRACT);
            self.core.sfx_play(SFX_RETRACT);
        } else if clicked {
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
            Action::ToggleBar => {
                if self.core.media_kind() == MediaKind::Video && self.cfg.hide_bar_button {
                    self.set_bar_hidden(!self.bar_hidden);
                } else if !self.cfg.hide_bar_button {
                    self.toast(ctx, "Activá \"Ocultar barra (videos)\" en ⚙ para usarlo", false);
                } else {
                    self.toast(ctx, "La barra se oculta solo con videos", false);
                }
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

    /// Cada cuánto redibujar mientras suena un audio. Con los milisegundos
    /// a la vista, a 30 cuadros (20 en Bajo); si no, solo cuando algo cambia
    /// de verdad: el reloj (una vez por segundo) o la barra (cuando avanza un
    /// píxel). Una canción de 3 minutos pasa de 30 redibujados por segundo a
    /// unos 4. (El visualizador y el fondo de Alto piden los suyos aparte.)
    fn audio_repaint_interval(&self, bar_w: f32) -> Duration {
        if self.cfg.show_millis {
            return crate::perf::ambient();
        }
        let speed = self.core.speed().max(0.25);
        let pos = self.core.position().max(0.0);
        let dur = self.core.duration();
        let to_next_second = (1.0 - pos.fract()) / speed;
        let per_pixel = if dur > 0.0 { dur / f64::from(bar_w.max(50.0)) / speed } else { 1.0 };
        Duration::from_secs_f64(to_next_second.min(per_pixel).clamp(0.04, 0.5))
    }

    /// Esconde o muestra la barra de abajo (videos). Los menús que cuelgan
    /// de la barra se cierran.
    fn set_bar_hidden(&mut self, hidden: bool) {
        self.bar_hidden = hidden;
        self.close_menus();
        self.show_playlist = self.show_playlist && !hidden;
    }

    /// Botón para ocultar / mostrar la barra (videos): abajo al medio del
    /// video. Con la barra a la vista aparece al acercar el mouse; con la
    /// barra escondida, al mover el mouse (y se va solo después de un rato).
    fn bar_toggle(&mut self, ui: &mut Ui, video: Rect, now: f64) {
        let ctx = ui.ctx().clone();
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let near = pointer.is_some_and(|p| video.contains(p) && p.y > video.bottom() - 90.0);
        let since_move = now - self.last_pointer_move;
        let want = if self.bar_hidden { near || since_move < 2.5 } else { near };
        let r = Rect::from_center_size(pos2(video.center().x, video.bottom() - 14.0), vec2(46.0, 18.0));
        let b = animate_button(ui, Id::unique("capyro-bar-toggle"), r.expand2(vec2(6.0, 4.0)), true);
        let vis = ctx.animate_bool_with_time(Id::unique("capyro-bar-toggle-vis"), want || b.hover > 0.01, crate::perf::t(0.2));
        if self.bar_hidden && since_move < 2.5 {
            // para que se vaya solo cuando el mouse se queda quieto
            ctx.request_repaint_after(Duration::from_secs_f64(2.5 - since_move + 0.05));
        }
        if vis <= 0.01 && !b.resp.clicked() {
            return;
        }
        let mut p = ui.painter().clone();
        p.multiply_opacity(vis);
        let rr = Rect::from_center_size(r.center(), r.size() * (1.0 + 0.08 * b.hover - 0.08 * b.press));
        if crate::alto::on() {
            crate::alto::glow_rect(&p, rr, self.cfg.accent, b.hover);
        }
        p.rect_filled(rr, 9.0, Color32::from_black_alpha((150.0 + 50.0 * b.hover) as u8));
        p.rect_stroke(rr, 9.0, Stroke::new(1.0, Color32::from_white_alpha(30)), StrokeKind::Inside);
        // flechita: abajo = esconder, arriba = mostrar
        let c = rr.center();
        let dy = if self.bar_hidden { -2.5 } else { 2.5 };
        let st = Stroke::new(1.6, Color32::WHITE.lerp_to_gamma(self.cfg.accent, b.hover));
        p.line_segment([c + vec2(-5.0, -dy), c + vec2(0.0, dy)], st);
        p.line_segment([c + vec2(5.0, -dy), c + vec2(0.0, dy)], st);
        if b.resp.clicked() {
            self.set_bar_hidden(!self.bar_hidden);
        }
        b.resp.on_hover_text(if self.bar_hidden { "Mostrar la barra (B)" } else { "Ocultar la barra (B)" });
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

    fn poll_screenshot(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.shot else { return };
        match rx.try_recv() {
            Ok(Ok(path)) => {
                self.shot = None;
                let shown = crate::paths::tilde(&path);
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

    /// Cambia el modo de rendimiento (⚙ → Rendimiento) y lo guarda.
    fn set_performance(&mut self, ctx: &egui::Context, mode: crate::perf::Mode) {
        self.cfg.performance = mode;
        apply_performance(ctx, mode);
        // la luz arranca de cero; y si es Alto, que saque los colores de la
        // imagen que ya está (una carátula no manda cuadros nuevos)
        self.ambient = Default::default();
        if mode == crate::perf::Mode::Alto {
            self.video_serial = u64::MAX;
        }
        let msg = match mode {
            crate::perf::Mode::Bajo => "Rendimiento bajo: sin animaciones ni extras",
            crate::perf::Mode::Medio => "Rendimiento medio",
            crate::perf::Mode::Alto => "Rendimiento alto: iluminación, fondo animado y partículas",
        };
        self.save_config(ctx, msg.to_owned());
    }

    fn reload_config(&mut self, ctx: &egui::Context) {
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
    fn add_paths(&mut self, ctx: &egui::Context, paths: Vec<PathBuf>, play_now: bool) {
        let first_new = self.core.playlist_len();
        let was_idle = self.core.state() == State::Idle;
        let mut errors = Vec::new();
        // una carpeta (arrastrada o por línea de comandos) = sus archivos
        // compatibles, con las subcarpetas, en orden alfabético
        let mut files = Vec::new();
        for path in paths {
            if path.is_dir() {
                let found = crate::formats::files_in_dir(&path, &self.formats);
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
    fn open_file_dialog(&mut self, ctx: &egui::Context) {
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
                .flat_map(|e| std::iter::once(e.as_str()).chain(crate::formats::aliases(e).iter().copied()))
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
    ///
    /// Con el núcleo bloqueado solo se copian los bytes (un memcpy: el audio
    /// y el decodificador esperan lo menos posible); el armado de la imagen
    /// va después. Los cuadros son opacos, así que RGBA "sin premultiplicar"
    /// y premultiplicado son lo mismo y no hay que tocar cada píxel.
    fn sync_video_texture(&mut self, ctx: &egui::Context) {
        let last = self.video_serial;
        let mut buf = std::mem::take(&mut self.frame_buf);
        let fresh = self.core.with_video_frame(|vf| {
            if vf.serial == last {
                return None;
            }
            buf.clear();
            buf.extend_from_slice(vf.rgba);
            Some(([vf.width, vf.height], vf.serial, vf.aspect))
        });
        let alto = self.cfg.performance == crate::perf::Mode::Alto;
        let fresh = fresh.map(|f| {
            f.map(|(size, serial, aspect)| {
                if alto {
                    self.ambient.sample(&buf, size[0], size[1]);
                }
                // los cuadros son opacos (alfa 255): RGBA y "premultiplicado"
                // son los mismos bytes, así que alcanza con una copia de
                // memoria (antes se armaba píxel por píxel)
                let pixels: Vec<Color32> = bytemuck::cast_slice::<u8, Color32>(&buf).to_vec();
                (egui::ColorImage::new(size, pixels), serial, aspect)
            })
        });
        self.frame_buf = buf;
        match fresh {
            None => {
                // no hay frame (nada cargado o mp3 sin carátula)
                self.video_tex = None;
                self.ambient.clear();
            }
            Some(None) => {}               // mismo frame que ya tenemos
            Some(Some((img, serial, aspect))) => {
                if self.core.state() == State::Playing && self.core.media_kind() == MediaKind::Video {
                    self.fps_meter.frame(ctx.input(|i| i.time));
                } else {
                    self.fps_meter.reset();
                }
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
        if crate::alto::on() {
            // Alto: brillo del color de acento al pasar y una onda al tocar
            let accent = ui.visuals().selection.bg_fill;
            crate::alto::glow(ui.painter(), center, size * 1.1, accent, 0.20 * a.hover + 0.15 * a.press);
            if a.resp.clicked() {
                crate::alto::ripple(ui.ctx(), center, size * 0.5);
            }
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
        self.ui_rects.insert("logo", logo_rect);
        let logo_btn = animate_button(ui, Id::unique("capyro-logo"), logo_rect, true);
        let now = ctx.input(|i| i.time);
        if logo_btn.resp.clicked() {
            self.play_capybara();
            self.logo_click = Some(now);
        }
        let mut bounce = 0.0;
        if let Some(t0) = self.logo_click {
            let dt = (now - t0) as f32;
            if dt < 0.7 && crate::perf::animations() {
                bounce = (-dt * 7.0).exp() * (dt * 22.0).sin();
                ctx.request_repaint_after(FRAME_SMOOTH);
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
        // Alto: también adentro del área de video, alrededor de la imagen
        // (las franjas negras y el fondo de la carátula)
        let lit = self.cfg.performance == crate::perf::Mode::Alto;
        if lit {
            let reach = rect.width().max(rect.height()) * 0.36;
            self.ambient.paint(&painter, rect.shrink(2.0), 0.30, reach, self.ambient.source, None);
            self.ambient_under = Some(rect);
            if self.core.media_kind() != MediaKind::Video {
                // audio o nada cargado: el polvito y la luz del mouse también acá
                let accent = self.cfg.accent;
                let hide = self.ambient.source.filter(|_| self.video_tex.is_some());
                self.backdrop.paint_particles(&painter, rect.shrink(2.0), accent, hide, 18);
                self.backdrop.paint_cursor(&painter, rect.shrink(2.0), accent, None);
            }
        }
        // Alto: al cambiar de archivo, un destello cruza el área y el título entra
        let enter = match self.track_at {
            Some(t0) if lit => {
                let k = ((ctx.input(|i| i.time) - t0) / 0.9) as f32;
                if k >= 1.0 {
                    self.track_at = None;
                }
                k.clamp(0.0, 1.0)
            }
            _ => 1.0,
        };
        let mut source = None;

        // el núcleo convierte los cuadros al tamaño en que se ven, no más
        let ppp = ctx.pixels_per_point();
        let target = ((rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32);
        if target != self.video_target {
            self.video_target = target;
            self.core.set_video_target(target.0, target.1);
        }

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
            painter.text(c + vec2(0.0, 4.0), Align2::CENTER_CENTER, "Suelta para abrir (archivos o carpetas)", regular(14.0), TEXT);
            // video en una línea y audio en otra: con 13 formatos, en una sola
            // no entraban en una ventana chica
            let (v, a) = &self.formats;
            for (i, list) in [v, a].into_iter().enumerate() {
                let line = list.join(" · ").to_uppercase();
                painter.text(c + vec2(0.0, 26.0 + 17.0 * i as f32), Align2::CENTER_CENTER, line, regular(12.0), TEXT_FAINT);
            }
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
                        source = Some(dest);
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
                    if self.cfg.visualizer && crate::perf::animations() && !self.viz.is_silent() {
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
                        source = Some(Rect::from_center_size(art_center, size));
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
                    // (en Alto, al cambiar de canción el título sube y aparece; el artista, detrás)
                    let ke = |delay: f32| ease_out(((enter - delay) / (1.0 - delay)).clamp(0.0, 1.0));
                    let (k1, k2) = (ke(0.15), ke(0.3));
                    painter.text(
                        pos2(c.x, text_y + 10.0 * (1.0 - k1)),
                        Align2::CENTER_CENTER,
                        ellipsize(&title, 52),
                        bold(15.0),
                        TEXT.gamma_multiply(k1),
                    );
                    if !artist.is_empty() {
                        painter.text(
                            pos2(c.x, text_y + 22.0 + 10.0 * (1.0 - k2)),
                            Align2::CENTER_CENTER,
                            ellipsize(&artist, 60),
                            regular(12.5),
                            TEXT_DIM.gamma_multiply(k2),
                        );
                    }
                }
            }
        }

        if lit && source.is_some() {
            self.ambient.source = source;
        }
        if enter < 1.0 {
            crate::alto::sweep(&painter, rect, enter, self.cfg.accent);
            ctx.request_repaint_after(FRAME_SMOOTH);
        }
        if !hovering_files && self.core.media_kind() != MediaKind::None {
            self.format_badge(ui, rect);
        }
        if !hovering_files && self.core.media_kind() == MediaKind::Video {
            self.video_info_badges(ui, rect);
        }
        if self.show_playlist {
            self.playlist_panel(ui, rect);
        }
    }

    /// Arriba a la izquierda del área de video, una debajo de la otra:
    /// el formato ("MP4 · H.264", "MP3"...) y los milisegundos del segundo
    /// actual. Cada una se oculta en Ajustes; si se oculta el formato, la de
    /// los milisegundos sube deslizándose a su lugar (y baja si vuelve).
    fn format_badge(&self, ui: &Ui, area: Rect) {
        let ctx = ui.ctx();
        let tf = ease_out(ctx.animate_bool_with_time(Id::unique("capyro-format-badge"), self.cfg.show_format, crate::perf::t(0.3)));
        let tm = ease_out(ctx.animate_bool_with_time(Id::unique("capyro-millis-badge"), self.cfg.show_millis, crate::perf::t(0.3)));
        if tf <= 0.001 && tm <= 0.001 {
            return;
        }
        let accent = self.cfg.accent;
        const H: f32 = 24.0;
        const GAP: f32 = 6.0;

        // formato
        if tf > 0.001 {
            let ext = Path::new(&self.np.file)
                .extension()
                .map(|e| e.to_string_lossy().to_uppercase())
                .unwrap_or_default();
            if !ext.is_empty() {
                let info = self.core.decode_info();
                let detail = if info.active && !info.codec.is_empty() {
                    if info.hw { format!("{} · GPU", info.codec) } else { info.codec.clone() }
                } else {
                    String::new()
                };
                let mut p = ui.painter().clone();
                p.multiply_opacity(tf);
                let eg = p.layout_no_wrap(ext, bold(11.5), Color32::WHITE);
                let dg = (!detail.is_empty()).then(|| p.layout_no_wrap(detail, regular(11.0), TEXT_DIM));
                let w = 12.0 + eg.size().x + dg.as_ref().map_or(0.0, |g| 10.0 + g.size().x) + 12.0;
                let r = Rect::from_min_size(area.min + vec2(10.0, 10.0 - 4.0 * (1.0 - tf)), vec2(w, H));
                p.rect_filled(r, 12.0, Color32::from_black_alpha(150));
                p.rect_stroke(r, 12.0, Stroke::new(1.0, Color32::from_white_alpha(18)), StrokeKind::Inside);
                let er = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 4.0), vec2(eg.size().x + 16.0, 16.0));
                p.rect_filled(er, 8.0, accent);
                p.galley(er.center() - eg.size() / 2.0, eg, Color32::WHITE);
                if let Some(g) = dg {
                    p.galley(pos2(er.right() + 6.0, r.center().y - g.size().y / 2.0), g, TEXT_DIM);
                }
            }
        }

        // milisegundos dentro del segundo actual (000..999)
        if tm > 0.001 {
            let ms = ((self.core.position().max(0.0).fract()) * 1000.0) as u32 % 1000;
            let mut p = ui.painter().clone();
            p.multiply_opacity(tm);
            // ancho fijo (con "000") para que la burbuja no tiemble
            let ng = p.layout_no_wrap(format!("{ms:03}"), FontId::monospace(12.0), Color32::WHITE);
            let nw = p.layout_no_wrap("000".to_owned(), FontId::monospace(12.0), Color32::WHITE).size().x;
            let ug = p.layout_no_wrap("ms".to_owned(), regular(11.0), TEXT_DIM);
            let w = 12.0 + nw + 5.0 + ug.size().x + 12.0;
            // baja un lugar si se ve el formato; sube deslizándose si no
            let y = area.top() + 10.0 + (H + GAP) * tf - 4.0 * (1.0 - tm);
            let r = Rect::from_min_size(pos2(area.left() + 10.0, y), vec2(w, H));
            p.rect_filled(r, 12.0, Color32::from_black_alpha(150));
            p.rect_stroke(r, 12.0, Stroke::new(1.0, Color32::from_white_alpha(18)), StrokeKind::Inside);
            // puntito que late una vez por segundo
            let beat = 1.0 - ms as f32 / 1000.0;
            p.circle_filled(pos2(r.left() + 9.0, r.center().y), 2.5, accent.gamma_multiply(0.35 + 0.65 * beat));
            let x = r.left() + 16.0;
            p.galley(pos2(x, r.center().y - ng.size().y / 2.0), ng, Color32::WHITE);
            p.galley(pos2(x + nw + 5.0, r.center().y - ug.size().y / 2.0), ug, TEXT_DIM);
            if self.core.state() == State::Playing {
                // que corran los números: 30 por segundo alcanzan para verlos correr
                ui.ctx().request_repaint_after(crate::perf::ambient());
            }
        }
        if (0.001..0.999).contains(&tf) || (0.001..0.999).contains(&tm) {
            ui.ctx().request_repaint_after(FRAME_SMOOTH);
        }
    }

    /// Abajo a la izquierda del video, una al lado de la otra: la resolución
    /// "de nombre" (1080p, 1440p · 2K, 4K...) y los cuadros por segundo. La
    /// etiqueta es solo el indicador: el video se ve tal cual es. Se ocultan
    /// juntas en Ajustes (entran y salen deslizándose).
    fn video_info_badges(&self, ui: &Ui, area: Rect) {
        let ctx = ui.ctx();
        let t = ease_out(ctx.animate_bool_with_time(Id::unique("capyro-videoinfo-badge"), self.cfg.show_video_info, crate::perf::t(0.3)));
        if t <= 0.001 {
            return;
        }
        let Some((w, h, fps)) = self.core.video_props() else { return };
        let accent = self.cfg.accent;
        const H: f32 = 24.0;
        let now = ctx.input(|i| i.time);
        let y = area.bottom() - 10.0 - H + 4.0 * (1.0 - t);
        let mut p = ui.painter().clone();
        p.multiply_opacity(t);

        // resolución
        let (label, alias) = crate::videoinfo::resolution_label(w, h);
        let lg = p.layout_no_wrap(label.to_owned(), bold(11.5), Color32::WHITE);
        let ag = (!alias.is_empty()).then(|| p.layout_no_wrap(alias.to_owned(), regular(11.0), TEXT_DIM));
        let rw = 12.0 + lg.size().x + ag.as_ref().map_or(0.0, |g| 10.0 + g.size().x) + 12.0;
        let r = Rect::from_min_size(pos2(area.left() + 10.0, y), vec2(rw, H));
        p.rect_filled(r, 12.0, Color32::from_black_alpha(150));
        p.rect_stroke(r, 12.0, Stroke::new(1.0, Color32::from_white_alpha(18)), StrokeKind::Inside);
        let lr = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 4.0), vec2(lg.size().x + 16.0, 16.0));
        p.rect_filled(lr, 8.0, accent);
        p.galley(lr.center() - lg.size() / 2.0, lg, Color32::WHITE);
        if let Some(g) = ag {
            p.galley(pos2(lr.right() + 6.0, r.center().y - g.size().y / 2.0), g, TEXT_DIM);
        }
        ui.interact(r, Id::unique("capyro-videoinfo-res"), Sense::hover())
            .on_hover_text(format!("El archivo es de {w}×{h}"));

        // fps: los del video; si la PC no llega, los que se ven, en otro color
        let playing = self.core.state() == State::Playing;
        let shown = if playing { self.fps_meter.rate(now) } else { None };
        let slow = matches!(shown, Some(s) if fps > 0.0 && s < fps * 0.85);
        let value = if slow { shown.unwrap_or(fps) } else { fps };
        let col = if slow { Color32::from_rgb(0xFF, 0xC9, 0x3C) } else { Color32::WHITE };
        let ng = p.layout_no_wrap(crate::videoinfo::fps_text(if slow { value.round() } else { value }), FontId::monospace(12.0), col);
        let ug = p.layout_no_wrap("fps".to_owned(), regular(11.0), TEXT_DIM);
        let fw = 16.0 + ng.size().x + 5.0 + ug.size().x + 12.0;
        let f = Rect::from_min_size(pos2(r.right() + 6.0, y), vec2(fw, H));
        p.rect_filled(f, 12.0, Color32::from_black_alpha(150));
        p.rect_stroke(f, 12.0, Stroke::new(1.0, Color32::from_white_alpha(18)), StrokeKind::Inside);
        // puntito: late mientras corre el video, quieto en pausa
        let beat = if playing { 0.6 + 0.4 * ((crate::perf::clock(now) * 2.0 * std::f64::consts::PI).sin() as f32).abs() } else { 0.45 };
        p.circle_filled(pos2(f.left() + 9.0, f.center().y), 2.5, if slow { col } else { accent.gamma_multiply(beat) });
        p.galley(pos2(f.left() + 16.0, f.center().y - ng.size().y / 2.0), ng.clone(), col);
        p.galley(pos2(f.left() + 16.0 + ng.size().x + 5.0, f.center().y - ug.size().y / 2.0), ug, TEXT_DIM);
        let tip = match shown {
            Some(s) if slow => format!(
                "El video es de {} fps pero se están viendo {:.0}: la PC no da abasto (probá ⚙ → Decodificación → GPU)",
                crate::videoinfo::fps_text(fps),
                s
            ),
            Some(s) => format!("{} fps del video · se están viendo {:.0}", crate::videoinfo::fps_text(fps), s),
            None => format!("Cuadros por segundo del video: {}", crate::videoinfo::fps_text(fps)),
        };
        ui.interact(f, Id::unique("capyro-videoinfo-fps"), Sense::hover()).on_hover_text(tip);

        if (0.001..0.999).contains(&t) {
            ctx.request_repaint_after(FRAME_SMOOTH);
        }
    }

    /// Título del metadato (o nombre de archivo) y artista.
    fn track_labels(&self) -> (String, String) {
        (self.np.title.clone(), self.np.artist.clone())
    }

    /// Vuelve a pedirle al núcleo el archivo, los metadatos y la cola, solo
    /// si cambiaron (una llamada barata por cuadro).
    fn refresh_now_playing(&mut self) {
        let serial = self.core.change_serial();
        if self.np.serial == Some(serial) {
            return;
        }
        let file = self.core.current_file();
        let title = self.core.title();
        let title = if title.trim().is_empty() { file_name(&file) } else { title };
        self.np = NowPlaying {
            serial: Some(serial),
            title,
            artist: self.core.artist(),
            album: self.core.album(),
            queue: std::rc::Rc::new(self.core.playlist()),
            file,
        };
    }

    fn draw_toast(&mut self, ui: &mut Ui, video: Rect) {
        let now = ui.ctx().input(|i| i.time);
        let Some(t) = &self.toast else { return };
        if now > t.until {
            self.toast = None;
            return;
        }
        // Aparece subiendo un poco y se desvanece al final.
        let fade_in = crate::perf::k(((now - t.start) / 0.15).clamp(0.0, 1.0) as f32);
        let fade_out = crate::perf::k(((t.until - now) / 0.35).clamp(0.0, 1.0) as f32);
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
        // frames solo mientras entra o se va; quieto, hasta que empiece a irse
        if fade_in < 1.0 || fade_out < 1.0 {
            ui.ctx().request_repaint_after(FRAME_SMOOTH);
        } else {
            // (en Bajo no se desvanece: se va justo al final)
            let fade_at = if crate::perf::animations() { t.until - 0.35 } else { t.until };
            ui.ctx().request_repaint_after(Duration::from_secs_f64((fade_at - now).max(0.0)));
        }
    }

    /// Rueda del mouse sobre el volumen: un paso por "muesca" (sin el
    /// suavizado de egui, que repartía una muesca en muchos cuadros) y solo
    /// en la primera pasada del cuadro (egui a veces repite la pasada).
    fn wheel_volume(&mut self, ctx: &egui::Context, hovered: bool) {
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

    /// Barra de progreso (con vista previa al pasar el mouse) y los tiempos
    /// abajo. `ty` = altura de la barra, `tty` = la de los tiempos.
    fn seek_bar(&mut self, ui: &mut Ui, painter: &egui::Painter, left: f32, right: f32, ty: f32, tty: f32) {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;
        let hit = Rect::from_min_max(pos2(left, ty - 8.0), pos2(right, ty + 8.0));
        self.ui_rects.insert("seek", Rect::from_min_max(pos2(left, ty - 10.0), pos2(right, tty + 10.0)));
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
        let grow = ctx.animate_bool_with_time(Id::unique("capyro-seek-grow"), active, crate::perf::t(0.12));
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
        if crate::alto::on() && can_seek {
            // Alto: la punta de la barra brilla (más con el mouse encima)
            let playing = self.core.state() == State::Playing;
            crate::alto::glow(&painter, pos2(fill_x, ty), 16.0 + 10.0 * grow, accent, 0.12 * playing as u8 as f32 + 0.35 * grow);
        }
        painter.rect_filled(Rect::from_min_max(track.min, pos2(fill_x, track.max.y)), th / 2.0, accent);
        if grow > 0.001 {
            let pressed = ctx.animate_bool_with_time(Id::unique("capyro-seek-press"), seek.dragged(), crate::perf::t(0.08));
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

    }

    /// Aleatorio, anterior, desde el principio, play, siguiente y repetir,
    /// centrados en `cx` (`spread` < 1 los junta en espacios chicos).
    /// Devuelve dónde quedó el botón de repetir.
    fn control_row(&mut self, ui: &mut Ui, painter: &egui::Painter, cx: f32, cy: f32, spread: f32) -> Pos2 {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;

        // aleatorio (izquierda)
        let shuffle_on = self.core.shuffle();
        let shuffle_c = pos2(cx - 105.0 * spread, cy);
        let sh = Self::icon_button(ui, "shuffle", shuffle_c, 30.0);
        let sh_on = ctx.animate_bool_with_time(Id::unique("capyro-shuffle-on"), shuffle_on, crate::perf::t(0.15));
        let sh_col = sh.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, sh_on);
        icons::shuffle(&painter, shuffle_c, 16.0 * sh.scale(), sh_col);
        if sh_on > 0.001 {
            painter.circle_filled(shuffle_c + vec2(0.0, 13.0), 2.0 * sh_on, accent);
        }
        if sh.resp.clicked() {
            self.toggle_shuffle(&ctx);
        }
        sh.resp.on_hover_text(if shuffle_on { "Aleatorio: activado (Ctrl+S)" } else { "Aleatorio: desactivado (Ctrl+S)" });

        let prev_c = pos2(cx - 69.0 * spread, cy);
        let prev = Self::icon_button(ui, "prev", prev_c, 30.0);
        icons::prev(&painter, prev_c - vec2(2.0 * prev.press, 0.0), 16.0 * prev.scale(), prev.tint(ICON, Color32::WHITE));
        if prev.resp.clicked() {
            self.run_action(&ctx, Action::PrevFile);
        }

        let restart_c = pos2(cx - 33.0 * spread, cy);
        let restart = Self::icon_button(ui, "restart", restart_c, 30.0);
        icons::restart(&painter, restart_c, 16.0 * restart.scale(), restart.tint(ICON, Color32::WHITE));
        if restart.resp.clicked() {
            self.core.seek(0.0);
        }
        restart.resp.on_hover_text("Volver al inicio");

        // play / pausa: crece, brilla y el ícono se transforma
        let play_c = pos2(cx + 17.0 * spread, cy);
        let play = animate_button(ui, Id::unique("capyro-play"), Rect::from_center_size(play_c, Vec2::splat(46.0)), true);
        let playing = self.core.state() == State::Playing;
        let morph = ctx.animate_bool_with_time(Id::unique("capyro-play-morph"), playing, crate::perf::t(0.16));
        let scale = 1.0 + 0.06 * play.hover - 0.1 * play.press;
        if crate::alto::on() {
            // Alto: halo que respira mientras suena y brilla más con el mouse
            let breathe = if playing { 0.10 + 0.06 * (ctx.input(|i| i.time) * 2.2).sin() as f32 } else { 0.04 };
            crate::alto::glow(&painter, play_c, 62.0 + 10.0 * play.hover, accent, breathe + 0.28 * play.hover);
            if play.resp.clicked() {
                crate::alto::ripple(&ctx, play_c, 26.0);
            }
        }
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

        let next_c = pos2(cx + 69.0 * spread, cy);
        let next = Self::icon_button(ui, "next", next_c, 30.0);
        icons::next(&painter, next_c + vec2(2.0 * next.press, 0.0), 16.0 * next.scale(), next.tint(ICON, Color32::WHITE));
        if next.resp.clicked() {
            self.run_action(&ctx, Action::NextFile);
        }

        // repetir (derecha): off → lista → uno
        let repeat = self.core.repeat();
        let repeat_c = pos2(cx + 105.0 * spread, cy);
        self.ui_rects.insert("controls", Rect::from_min_max(pos2(cx - 123.0 * spread, cy - 25.0), pos2(cx + 123.0 * spread, cy + 25.0)));
        let rp = Self::icon_button(ui, "repeat", repeat_c, 30.0);
        let rp_on = ctx.animate_bool_with_time(Id::unique("capyro-repeat-on"), repeat != Repeat::Off, crate::perf::t(0.15));
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

        repeat_c
    }

    /// El botón naranja "+ Agregar archivo(s)" en `base_rect` (solo el "+"
    /// si es `compact`).
    fn add_files_button(&mut self, ui: &mut Ui, painter: &egui::Painter, base_rect: Rect, compact: bool) {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;
        let pending = self.dialog.is_some();
        let label = if pending { "Abriendo…" } else { "Agregar archivo(s)" };
        let galley = painter.layout_no_wrap(label.to_owned(), bold(13.0), Color32::WHITE);
        let add = animate_button(ui, Id::unique("capyro-add-files"), base_rect, !pending);
        let s = 1.0 + 0.03 * add.hover - 0.06 * add.press;
        let add_rect = Rect::from_center_size(base_rect.center(), base_rect.size() * s);
        if crate::alto::on() {
            crate::alto::glow_rect(&painter, add_rect, accent, add.hover);
            if add.resp.clicked() {
                crate::alto::ripple(&ctx, add_rect.center(), add_rect.height() * 0.8);
            }
        }
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
            // el "+" y el texto, centrados (en la lista de la vista de música
            // el botón ocupa todo el ancho)
            let content_w = 12.0 + galley.size().x;
            let plus_c = pos2(add_rect.center().x - content_w / 2.0 + 2.0, add_rect.center().y);
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
    }

    fn transport(&mut self, ui: &mut Ui, video_bottom: f32, inner: Rect) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();
        let accent = self.cfg.accent;
        let (left, right) = (inner.left(), inner.right());

        /* --- barra de progreso --- */
        self.seek_bar(ui, &painter, left, right, video_bottom + TRACK_Y, video_bottom + TIMES_Y);

        /* --- controles --- */
        let cy = video_bottom + CONTROLS_Y;
        let cx = inner.center().x;
        let repeat_c = self.control_row(ui, &painter, cx, cy, 1.0);

        /* --- botón "+ Agregar archivo(s)" (sobre los íconos de la derecha) --- */
        let pending = self.dialog.is_some();
        let label = if pending { "Abriendo…" } else { "Agregar archivo(s)" };
        let full_w = painter.layout_no_wrap(label.to_owned(), bold(13.0), Color32::WHITE).size().x + 50.0;
        let controls_right = repeat_c.x + 22.0;
        // Si la ventana es angosta y no entra el texto, queda solo el "+".
        let compact = right - full_w < controls_right;
        let base = if compact { vec2(34.0, 34.0) } else { vec2(full_w, 34.0) };
        let base_rect = Rect::from_min_size(pos2(right - base.x, cy - base.y / 2.0), base);
        self.ui_rects.insert("add", base_rect);
        self.add_files_button(ui, &painter, base_rect, compact);

        /* --- botón de información (arriba de "Lista", a la izquierda) --- */
        let info_c = pos2(left + 9.0, cy);
        let info = Self::icon_button(ui, "info", info_c, 30.0);
        let info_on = ctx.animate_bool_with_time(Id::unique("capyro-info-on"), self.show_info, crate::perf::t(0.15));
        let info_col = info.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, info_on);
        icons::info(&painter, info_c, 17.0 * info.scale(), info_col);
        // puntito: actualizando (late) o lista para reiniciar
        let pending_restart = matches!(self.updater.last, Some(Outcome::Installed { .. }));
        let badge = ctx.animate_bool_with_time(
            Id::unique("capyro-info-badge"),
            self.updater.is_running() || pending_restart,
            crate::perf::t(0.2),
        );
        if badge > 0.001 {
            let beat = if self.updater.is_running() { 0.6 + 0.4 * (crate::perf::clock(ctx.input(|i| i.time)) * 5.0).sin().abs() as f32 } else { 1.0 };
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
        let ch_on = ctx.animate_bool_with_time(Id::unique("capyro-changes-on"), self.show_changes, crate::perf::t(0.15));
        let ch_col = ch.tint(ICON_DIM, Color32::WHITE).lerp_to_gamma(accent, ch_on);
        icons::news(&painter, ch_c, 17.0 * ch.scale(), ch_col);
        self.changes_rect = Rect::from_center_size(ch_c, Vec2::splat(30.0));
        // puntito mientras no se hayan visto las novedades de esta versión
        let unseen = self.changes_seen != ffi::version();
        let dot = ctx.animate_bool_with_time(Id::unique("capyro-changes-dot"), unseen, crate::perf::t(0.3));
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
        self.ui_rects.insert("list", list_rect);
        let list = animate_button(ui, Id::unique("capyro-list"), list_rect, true);
        let list_on = ctx.animate_bool_with_time(Id::unique("capyro-list-on"), self.show_playlist, crate::perf::t(0.15));
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
        self.ui_rects.insert("volume", Rect::from_center_size(vol_c, Vec2::splat(28.0)));
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
        self.wheel_volume(&ctx, vol.resp.hovered());
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
        let fx_on = ctx.animate_bool_with_time(Id::unique("capyro-fx-on"), self.show_fx, crate::perf::t(0.15));
        let fx_active = ctx.animate_bool_with_time(Id::unique("capyro-fx-active"), self.fx_active(), crate::perf::t(0.15));
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
        self.ui_rects.insert("subs", Rect::from_center_size(subs_c, Vec2::splat(28.0)));
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
        let gear_on = ctx.animate_bool_with_time(Id::unique("capyro-gear-on"), self.show_settings, crate::perf::t(0.15));
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
        self.ui_rects.insert("fullscreen", Rect::from_center_size(fs_c, Vec2::splat(28.0)));
        let fsb = Self::icon_button(ui, "fullscreen", fs_c, 26.0);
        icons::fullscreen(&painter, fs_c, 14.0 * fsb.scale(), fsb.tint(ICON_DIM, Color32::WHITE), fs);
        if fsb.resp.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fs));
        }
    }

    /// Menú de información: versión, formatos soportados y algunos datos.
    /// Crece desde el botón ⓘ (abajo a la izquierda), igual que el de ajustes.
    /// Con `dock`, fijo en la burbuja de funciones de la vista de música
    /// (con los formatos ya abiertos: ahí hay lugar). Devuelve el alto.
    fn info_menu(&mut self, ui: &mut Ui, dock: Option<Rect>) -> f32 {
        let ctx = ui.ctx().clone();
        let docked = dock.is_some();
        let t = if docked { 1.0 } else { ctx.animate_bool_with_time(Id::unique("capyro-info-anim"), self.show_info, crate::perf::t(0.15)) };
        if t <= 0.001 {
            self.info_format = None; // al volver a abrirlo arranca sin resumen abierto
            self.info_formats_open = false; // y con la lista de formatos cerrada
            return 0.0;
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let accent = self.cfg.accent;
        let width = dock.map_or(312.0, |d| d.width());
        let inner_w = width - 28.0;
        // (encabezado 64 + botón de formatos 36 + actualizaciones 54 + desinstalar 36 + pie 34)
        let base_h = 224.0;

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
            let k = ctx.animate_bool_with_time(Id::unique(("capyro-fmt-open", ext.as_str())), open, crate::perf::t(0.22));
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

        // --- la lista de formatos (se despliega con el botón) ---
        // Los chips se acomodan en renglones: con 13 formatos ya no entran
        // en una sola fila por tipo.
        let fopen = if docked {
            1.0
        } else {
            ctx.animate_bool_with_time(Id::unique("capyro-fmts-open"), self.info_formats_open, crate::perf::t(0.26))
        };
        let fk = 1.0 - (1.0 - fopen).powi(3);
        let mut chip_layout: Vec<(String, Rect)> = Vec::new();
        let mut group_labels: Vec<(&str, f32, bool, usize)> = Vec::new();
        let mut ry = 4.0;
        for (label, list, is_video) in [("Video", &formats.0, true), ("Audio", &formats.1, false)] {
            group_labels.push((label, ry + 8.0, is_video, list.len()));
            ry += 22.0;
            let mut cx = 0.0;
            for ext in list {
                let w = ui.painter().layout_no_wrap(ext.to_uppercase(), bold(11.0), TEXT).size().x + 16.0;
                if cx > 0.0 && cx + w > inner_w {
                    cx = 0.0;
                    ry += 28.0;
                }
                chip_layout.push((ext.clone(), Rect::from_min_size(pos2(cx, ry), vec2(w, 22.0))));
                cx += w + 6.0;
            }
            ry += 22.0 + 12.0;
        }
        let chips_h = ry;
        // los resúmenes van debajo de los chips, adentro de lo desplegado
        let region_h = (chips_h + region_h) * fk;

        let size = vec2(width, base_h + region_h);
        let (panel, interactive, painter) = if let Some(d) = dock {
            (Rect::from_min_size(d.min, size), true, ui.painter().clone())
        } else {
        let anchor = pos2(self.info_rect.left() - 6.0, self.info_rect.top() - 10.0);
        let full = Rect::from_min_size(pos2(anchor.x, anchor.y - size.y), size);
        let grown = size * (0.85 + 0.15 * crate::alto::pop(t, ease));
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
        crate::theme::soft_shadow(&painter, panel, 12.0, 8.0, 2.0, 70);
        painter.rect_filled(panel, 12.0, BG_PANEL);
        painter.rect_stroke(panel, 12.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
        if ease < 0.98 {
            ctx.request_repaint_after(FRAME_SMOOTH);
            return 0.0;
        }
        crate::alto::reveal(&ctx, Id::unique("capyro-info-anim"), panel, 12.0, self.show_info, BG_PANEL, accent);
        (panel, interactive, painter)
        };

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

        // formatos: un botón que despliega la lista (son muchos para que
        // estén siempre a la vista)
        painter.line_segment([pos2(x0, top + 64.0), pos2(x1, top + 64.0)], Stroke::new(1.0, Color32::from_white_alpha(14)));
        let row = Rect::from_min_max(pos2(x0 - 6.0, top + 67.0), pos2(x1 + 6.0, top + 97.0));
        // (en la burbuja de la vista de música no es botón: la lista ya está abierta)
        let fb = animate_button(ui, Id::unique("capyro-fmts-toggle"), row, interactive && !docked);
        if fb.hover > 0.001 || fb.press > 0.001 || fk > 0.001 {
            let a = 10.0 * fb.hover + 10.0 * fb.press + 6.0 * fk;
            painter.rect_filled(row, 8.0, Color32::from_white_alpha(a as u8));
        }
        let cy = row.center().y;
        let icol = fb.tint(TEXT_DIM, TEXT).lerp_to_gamma(accent, fk);
        icons::film(&painter, pos2(x0 + 6.0, cy), 13.0, icol);
        icons::music_note(&painter, pos2(x0 + 21.0, cy), 13.0, icol);
        let label = if docked {
            "Formatos soportados"
        } else if self.info_formats_open {
            "Formatos soportados (cerrar)"
        } else {
            "Formatos soportados (abrir)"
        };
        let lr = painter.text(pos2(x0 + 34.0, cy), Align2::LEFT_CENTER, label, bold(12.5), TEXT);
        // a la derecha: cuántos son y una flechita que gira al abrir
        let total = formats.0.len() + formats.1.len();
        let chev = pos2(x1 - 4.0, cy);
        let st = Stroke::new(1.5, fb.tint(TEXT_FAINT, TEXT).lerp_to_gamma(accent, fk));
        let (dy, sx) = (3.0 * (1.0 - 2.0 * fk), 4.5);
        if !docked {
            painter.line_segment([chev + vec2(-sx, -dy), chev + vec2(0.0, dy)], st);
            painter.line_segment([chev + vec2(sx, -dy), chev + vec2(0.0, dy)], st);
        }
        let ng = painter.layout_no_wrap(total.to_string(), bold(10.5), accent);
        // la cantidad, al lado del texto (si entra antes de la flechita)
        let nchip = Rect::from_min_size(pos2(lr.right() + 7.0, cy - 9.0), vec2(ng.size().x + 12.0, 18.0));
        if nchip.right() < chev.x - 10.0 {
            painter.rect_filled(nchip, 9.0, accent.gamma_multiply(0.16));
            painter.galley(nchip.center() - ng.size() / 2.0, ng, accent);
        }
        if fb.hover > 0.5 {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        if fb.resp.clicked() && !docked {
            self.info_formats_open = !self.info_formats_open;
            if !self.info_formats_open {
                self.info_format = None;
            }
        }
        let fb_resp = fb.resp;
        fb_resp.on_hover_text(format!(
            "Video: {}\nAudio: {}",
            formats.0.join(", ").to_uppercase(),
            formats.1.join(", ").to_uppercase()
        ));

        // lo desplegado: los chips (tocá uno para ver qué es) y los resúmenes
        let region = Rect::from_min_size(pos2(x0, top + 100.0), vec2(inner_w, region_h));
        let clipped = painter.with_clip_rect(region.expand2(vec2(4.0, 0.0)).intersect(painter.clip_rect()));
        let mut clicked: Option<String> = None;
        if fk > 0.001 {
            let mut p = clipped.clone();
            p.multiply_opacity(fk);
            let origin = region.min + vec2(0.0, -8.0 * (1.0 - fk));
            let chips_live = interactive && fk > 0.9;
            for (label, ly, is_video, n) in &group_labels {
                let ic = origin + vec2(7.0, *ly);
                if *is_video {
                    icons::film(&p, ic, 13.0, TEXT_DIM);
                } else {
                    icons::music_note(&p, ic, 13.0, TEXT_DIM);
                }
                p.text(origin + vec2(20.0, *ly), Align2::LEFT_CENTER, *label, regular(12.0), TEXT_DIM);
                let hint = if *is_video { format!("{n} formatos · tocá uno") } else { format!("{n} formatos") };
                p.text(pos2(x1, origin.y + *ly), Align2::RIGHT_CENTER, hint, regular(10.5), TEXT_FAINT);
            }
            for (ext, rel) in &chip_layout {
                let r = rel.translate(origin.to_vec2());
                let selected = self.info_format.as_deref() == Some(ext.as_str());
                let g = p.layout_no_wrap(ext.to_uppercase(), bold(11.0), TEXT);
                let a = animate_button(ui, Id::unique(("capyro-fmt", ext.as_str())), r, chips_live);
                let sel = ctx.animate_bool_with_time(Id::unique(("capyro-fmt-sel", ext.as_str())), selected, crate::perf::t(0.15));
                let rr = Rect::from_center_size(r.center(), r.size() * (1.0 + 0.04 * a.hover - 0.08 * a.press));
                let bg = Color32::from_white_alpha((10.0 + 12.0 * a.hover) as u8).lerp_to_gamma(accent, sel);
                p.rect_filled(rr, 6.0, bg);
                let col = TEXT.lerp_to_gamma(Color32::WHITE, sel);
                p.galley(rr.center() - g.size() / 2.0, g, col);
                if a.resp.clicked() {
                    clicked = Some(ext.clone());
                }
            }

            // resúmenes (acordeón), debajo de los chips
            let mut cy = origin.y + chips_h;
            for (k, card_h, title, kind_g, body) in cards {
                let mut pc = p.clone();
                pc.multiply_opacity(k);
                let card = Rect::from_min_size(pos2(x0, cy - 6.0 * (1.0 - k)), vec2(inner_w, card_h));
                pc.rect_filled(card, 8.0, Color32::from_white_alpha(9));
                pc.rect_filled(Rect::from_min_size(card.min + vec2(0.0, 10.0), vec2(3.0, card_h - 20.0)), 1.5, accent);
                let ty = card.top() + 10.0;
                pc.galley(pos2(card.left() + 12.0, ty), title.clone(), TEXT);
                pc.galley(pos2(card.right() - 10.0 - kind_g.size().x, ty + 1.0), kind_g, TEXT_FAINT);
                pc.galley(pos2(card.left() + 12.0, ty + title.size().y + 5.0), body, TEXT_DIM);
                cy += (card_h + 8.0) * k;
            }
        }
        if (0.001..0.999).contains(&fopen) {
            ctx.request_repaint_after(FRAME_SMOOTH);
        }
        if let Some(ext) = clicked {
            // solo uno abierto a la vez; tocar el abierto lo cierra
            self.info_format = if self.info_format.as_deref() == Some(ext.as_str()) { None } else { Some(ext) };
        }

        // actualizaciones
        let upd_top = region.bottom() + 2.0;
        painter.line_segment([pos2(x0, upd_top), pos2(x1, upd_top)], Stroke::new(1.0, Color32::from_white_alpha(14)));
        self.update_row(ui, &painter, Rect::from_min_max(pos2(x0 - 6.0, upd_top + 6.0), pos2(x1 + 6.0, upd_top + 48.0)), interactive);
        // desinstalar (abajo de actualizaciones, arriba del pie)
        let un_row = Rect::from_min_max(pos2(x0 - 6.0, upd_top + 50.0), pos2(x1 + 6.0, upd_top + 84.0));
        self.uninstall_row(ui, &painter, un_row, interactive);

        // pie: backend de audio y licencia
        let backend = self.core.audio_backend();
        let audio = if backend.is_empty() { "sin dispositivo".to_owned() } else { backend };
        let foot = format!("Audio: {audio}  ·  GPL-3.0  ·  {} ({})", self.cfg.system().name(), self.build_info.system);
        let foot = fit_text(&painter, &foot, regular(10.5), x1 - x0);
        painter.text(pos2(x0, panel.bottom() - 16.0), Align2::LEFT_CENTER, foot, regular(10.5), TEXT_FAINT);

        if !docked && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.show_info = false;
        }
        size.y
    }

    /// Menú del engranaje: velocidad y recargar configuración. Se abre
    /// creciendo desde el engranaje y se cierra al hacer clic afuera o con Esc.
    /// Con `dock`, se dibuja fijo ahí (la burbuja de funciones de la vista
    /// de música: sin animación de apertura ni fondo propio). Devuelve el
    /// alto del contenido (para el scroll de la burbuja).
    fn settings_menu(&mut self, ui: &mut Ui, dock: Option<Rect>) -> f32 {
        let ctx = ui.ctx().clone();
        let accent = self.cfg.accent;
        let size = vec2(288.0, 556.0);
        let (panel, interactive, painter) = if let Some(d) = dock {
            (Rect::from_min_size(d.min, vec2(d.width(), size.y)), true, ui.painter().clone())
        } else {
        let t = ctx.animate_bool_with_time(Id::unique("capyro-settings-anim"), self.show_settings, crate::perf::t(0.15));
        if t <= 0.001 {
            return 0.0;
        }
        let ease = 1.0 - (1.0 - t).powi(3);
        let mut anchor = pos2(self.gear_rect.right() + 6.0, self.gear_rect.top() - 10.0);
        // en ventanas bajitas no se sale por arriba (tapa el engranaje)
        anchor.y = anchor.y.max(ui.max_rect().top() + 8.0 + size.y);
        let full = Rect::from_min_size(anchor - size, size);
        // crece desde la esquina del engranaje
        let panel = Rect::from_min_max(anchor - size * (0.85 + 0.15 * crate::alto::pop(t, ease)), anchor);
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
        crate::theme::soft_shadow(&painter, panel, 12.0, 8.0, 2.0, 70);
        painter.rect_filled(panel, 12.0, BG_PANEL);
        painter.rect_stroke(panel, 12.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
        if ease < 0.98 {
            ctx.request_repaint_after(FRAME_SMOOTH);
            return 0.0; // el contenido aparece cuando termina de abrirse
        }
        crate::alto::reveal(&ctx, Id::unique("capyro-settings-anim"), panel, 12.0, self.show_settings, BG_PANEL, accent);
        (panel, interactive, painter)
        };

        let x0 = panel.left() + 14.0;
        let x1 = panel.right() - 14.0;
        let mut y = panel.top() + 20.0;
        // en la burbuja angosta de la vista de música, los chips con letra más chica
        let chip_fs = if x1 - x0 < 240.0 { 10.5 } else { 11.5 };
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
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-speed-sel", i)), selected, crate::perf::t(0.15));
            let bg = Color32::from_white_alpha((8.0 + 14.0 * chip.hover) as u8).lerp_to_gamma(accent, sel);
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.06 * chip.press));
            painter.rect_filled(rr, 7.0, bg);
            let col = TEXT_DIM.lerp_to_gamma(TEXT, chip.hover).lerp_to_gamma(Color32::WHITE, sel);
            painter.text(rr.center(), Align2::CENTER_CENTER, fmt_speed(v), if selected { bold(chip_fs) } else { regular(chip_fs) }, col);
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
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-hwdec-sel", i)), selected, crate::perf::t(0.15));
            let bg = Color32::from_white_alpha((8.0 + 14.0 * chip.hover) as u8).lerp_to_gamma(accent, sel);
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.06 * chip.press));
            painter.rect_filled(rr, 7.0, bg);
            let col = TEXT_DIM.lerp_to_gamma(TEXT, chip.hover).lerp_to_gamma(Color32::WHITE, sel);
            // (en la burbuja angosta, "Automático" no entra)
            let label = if chip_w < 66.0 && m == ffi::Hwdec::Auto { "Auto" } else { m.label() };
            painter.text(rr.center(), Align2::CENTER_CENTER, label, if selected { bold(chip_fs) } else { regular(chip_fs) }, col);
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
        let shown = fit_text(&painter, &status, regular(10.5), x1 - x0);
        let cut = shown != status;
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, shown, regular(10.5), TEXT_FAINT);
        if cut {
            st.on_hover_text(status);
        }

        /* separador */
        y += 18.0;
        painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, Color32::from_white_alpha(14)));

        /* modo de rendimiento: bajo / medio / alto */
        y += 22.0;
        let perf_mode = self.cfg.performance;
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Rendimiento", regular(12.5), TEXT_DIM);
        y += 24.0;
        for (i, m) in crate::perf::Mode::ALL.iter().copied().enumerate() {
            let r = Rect::from_min_size(pos2(x0 + i as f32 * (chip_w + gap), y - 13.0), vec2(chip_w, 26.0));
            let chip = animate_button(ui, Id::unique(("capyro-perf", i)), r, interactive);
            let selected = perf_mode == m;
            let sel = ctx.animate_bool_with_time(Id::unique(("capyro-perf-sel", i)), selected, crate::perf::t(0.15));
            let bg = Color32::from_white_alpha((8.0 + 14.0 * chip.hover) as u8).lerp_to_gamma(accent, sel);
            let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.06 * chip.press));
            painter.rect_filled(rr, 7.0, bg);
            let col = TEXT_DIM.lerp_to_gamma(TEXT, chip.hover).lerp_to_gamma(Color32::WHITE, sel);
            painter.text(rr.center(), Align2::CENTER_CENTER, m.label(), if selected { bold(chip_fs) } else { regular(chip_fs) }, col);
            if chip.resp.clicked() && !selected {
                self.set_performance(&ctx, m);
            }
            chip.resp.on_hover_text(match m {
                crate::perf::Mode::Bajo => "Sin animaciones, sin pantalla de carga, sin visualizador ni brillos. Todo sigue funcionando, gastando lo mínimo",
                crate::perf::Mode::Medio => "Capyro como siempre",
                crate::perf::Mode::Alto => "Suma la iluminación cinematográfica (los colores del video iluminan toda la ventana, como en YouTube), un fondo con degradé que se mueve, brillos que siguen al mouse, transiciones con más vida y partículas de luz",
            });
        }
        y += 22.0;
        let blurb = fit_text(&painter, self.cfg.performance.blurb(), regular(10.5), x1 - x0);
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, blurb, regular(10.5), TEXT_FAINT);

        /* separador */
        y += 18.0;
        painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, Color32::from_white_alpha(14)));

        /* qué se muestra (textos cortos: también entran en la burbuja de
         * funciones de la vista de música, que es más angosta) */
        y += 18.0;
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Formato del archivo", regular(12.5), TEXT_DIM);
        if fx_menu::switch(ui, &painter, "show-format", pos2(x1 - 17.0, y), self.cfg.show_format, accent, interactive) {
            self.cfg.show_format = !self.cfg.show_format;
            let msg = if self.cfg.show_format { "Formato visible" } else { "Formato oculto" };
            self.save_config(&ctx, msg.to_owned());
        }
        y += 30.0;
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Milisegundos", regular(12.5), TEXT_DIM);
        if fx_menu::switch(ui, &painter, "show-millis", pos2(x1 - 17.0, y), self.cfg.show_millis, accent, interactive) {
            self.cfg.show_millis = !self.cfg.show_millis;
            let msg = if self.cfg.show_millis { "Milisegundos visibles" } else { "Milisegundos ocultos" };
            self.save_config(&ctx, msg.to_owned());
        }
        y += 30.0;
        painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Resolución y fps", regular(12.5), TEXT_DIM);
        if fx_menu::switch(ui, &painter, "show-video-info", pos2(x1 - 17.0, y), self.cfg.show_video_info, accent, interactive) {
            self.cfg.show_video_info = !self.cfg.show_video_info;
            let msg = if self.cfg.show_video_info { "Resolución y fps visibles" } else { "Resolución y fps ocultos" };
            self.save_config(&ctx, msg.to_owned());
        }
        // la interfaz adaptativa (v0.3.2)
        y += 30.0;
        let r = painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Vista de música", regular(12.5), TEXT_DIM);
        ui.interact(r.expand(4.0), Id::unique("capyro-music-view-label"), Sense::hover())
            .on_hover_text("Con un audio: la carátula grande al medio, la lista a un costado y las funciones al otro (si la ventana tiene lugar)");
        if fx_menu::switch(ui, &painter, "music-view", pos2(x1 - 17.0, y), self.cfg.music_view, accent, interactive) {
            self.cfg.music_view = !self.cfg.music_view;
            let msg = if self.cfg.music_view { "Vista de música activada" } else { "Vista de música desactivada" };
            self.save_config(&ctx, msg.to_owned());
        }
        y += 30.0;
        let r = painter.text(pos2(x0, y), Align2::LEFT_CENTER, "Ocultar barra (videos)", regular(12.5), TEXT_DIM);
        ui.interact(r.expand(4.0), Id::unique("capyro-hide-bar-label"), Sense::hover())
            .on_hover_text("Un botón abajo del video para esconder los controles con un clic (también con la tecla B)");
        if fx_menu::switch(ui, &painter, "hide-bar", pos2(x1 - 17.0, y), self.cfg.hide_bar_button, accent, interactive) {
            self.cfg.hide_bar_button = !self.cfg.hide_bar_button;
            if !self.cfg.hide_bar_button {
                self.bar_hidden = false; // sin el botón, la barra vuelve
            }
            let msg = if self.cfg.hide_bar_button { "Botón para ocultar la barra activado" } else { "Botón para ocultar la barra desactivado" };
            self.save_config(&ctx, msg.to_owned());
        }
        y += 16.0;
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

        /* repetir el tutorial */
        y += 40.0;
        let row = Rect::from_min_max(pos2(x0 - 6.0, y - 18.0), pos2(x1 + 6.0, y + 18.0));
        let tut = animate_button(ui, Id::unique("capyro-settings-tutorial"), row, interactive);
        if tut.hover > 0.001 || tut.press > 0.001 {
            painter.rect_filled(row, 8.0, Color32::from_white_alpha((10.0 * tut.hover + 10.0 * tut.press) as u8));
        }
        if let Some(logo) = &self.logo {
            let side = 18.0 * tut.scale();
            painter.image(logo.id(), Rect::from_center_size(pos2(x0 + 8.0, y - 1.0), Vec2::splat(side)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        painter.text(pos2(x0 + 24.0, y - 7.0), Align2::LEFT_CENTER, "Ver el tutorial de nuevo", regular(12.5), TEXT);
        painter.text(pos2(x0 + 24.0, y + 8.0), Align2::LEFT_CENTER, "La bienvenida y el recorrido", regular(10.5), TEXT_FAINT);
        if tut.resp.clicked() {
            self.show_settings = false;
            self.start_tutorial(&ctx);
        }

        // Esc también lo cierra
        if dock.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.show_settings = false;
        }
        size.y
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
    /// Botón "Desinstalar Capyro" del menú ⓘ. El primer clic lo arma (dice
    /// "Tocá otra vez"); el segundo, dentro de 4 s, abre el desinstalador
    /// de este sistema y cierra Capyro.
    fn uninstall_row(&mut self, ui: &mut Ui, painter: &egui::Painter, row: Rect, interactive: bool) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let armed = self.uninstall_armed.is_some_and(|t0| now - t0 < 4.0);
        let btn = animate_button(ui, Id::unique("capyro-uninstall-row"), row, interactive);
        let tint = if armed { theme::ERROR } else { TEXT_DIM };
        if armed {
            painter.rect_filled(row, 8.0, theme::ERROR.gamma_multiply(0.14));
        } else if btn.hover > 0.001 || btn.press > 0.001 {
            painter.rect_filled(row, 8.0, Color32::from_white_alpha((10.0 * btn.hover + 10.0 * btn.press) as u8));
        }
        let cy = row.center().y;
        let x0 = row.left() + 6.0;
        icons::trash(painter, pos2(x0 + 8.0, cy), 13.0, tint);
        let label = if armed { "Tocá otra vez para desinstalar" } else { "Desinstalar Capyro" };
        painter.text(pos2(x0 + 26.0, cy), Align2::LEFT_CENTER, label, bold(12.5), if armed { theme::ERROR } else { TEXT });
        if btn.hover > 0.5 {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        if armed {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        let btn_resp = btn.resp.on_hover_text(if cfg!(windows) {
            "Abre el desinstalador de Windows (el mismo de Aplicaciones instaladas)"
        } else {
            "Abre el desinstalador de Capyro (capyro-uninstall, junto al programa)"
        });
        if btn_resp.clicked() {
            if armed {
                self.uninstall_armed = None;
                match crate::uninstall::launch() {
                    Ok(crate::uninstall::Outcome::Done) => ctx.send_viewport_cmd(ViewportCommand::Close),
                    Ok(crate::uninstall::Outcome::NeedsRoot(cmd)) => {
                        ctx.copy_text(cmd.clone());
                        self.toast_for(&ctx, format!("Copié, pegalo en una terminal: {cmd}"), false, 10.0);
                    }
                    Err(e) => self.toast(&ctx, e, true),
                }
            } else {
                self.uninstall_armed = Some(now);
            }
        }
    }

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
        let mut tooltip = if cfg!(windows) {
            "Busca la última versión para Windows en el repo de GitHub; si hay, la baja y la instala sola".to_owned()
        } else {
            "Busca commits nuevos en el repo de GitHub; si hay, los baja, compila e instala solo".to_owned()
        };
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
                                if cfg!(windows) {
                                    "Para que se actualice solo, instalá Capyro con CapyroInstaller".to_owned()
                                } else {
                                    "Para que se actualice solo, instalá Capyro (cmake --install build)".to_owned()
                                }
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
        let t = ctx.animate_bool_with_time(Id::unique("capyro-offer-anim"), self.show_offer, crate::perf::t(0.22));
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
        crate::theme::soft_shadow(&p, rect, 14.0, 10.0, 2.5, 80);
        p.rect_filled(rect, 14.0, BG_PANEL);
        p.rect_stroke(rect, 14.0, Stroke::new(1.0, accent.gamma_multiply(0.5)), StrokeKind::Inside);

        // encabezado
        let hc = pos2(rect.left() + 30.0, rect.top() + 31.0);
        p.circle_filled(hc, 15.0, accent.gamma_multiply(0.18));
        let bob = ((crate::perf::clock(now) * 3.0).sin() as f32 * 0.5 + 0.5) * 0.6;
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
        if interactive && crate::perf::animations() {
            ctx.request_repaint_after(FRAME_AMBIENT); // el ícono que flota
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
        let fade_in = ease_out(crate::perf::k(((now - b.start) / 0.35) as f32));
        let fade_out = crate::perf::k(((b.until - now) / 0.45).clamp(0.0, 1.0) as f32);
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
        crate::theme::soft_shadow(&p, rect, 14.0, 10.0, 2.5, 80);
        p.rect_filled(rect, 14.0, BG_PANEL);
        p.rect_stroke(rect, 14.0, Stroke::new(1.0, accent.gamma_multiply(0.5)), StrokeKind::Inside);

        // encabezado
        let hc = pos2(rect.left() + 30.0, rect.top() + 31.0);
        let pop = ease_out(crate::perf::k(((now - start - 0.15) / 0.3) as f32));
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
            let k = ease_out(crate::perf::k(((now - start - 0.2 - 0.08 * i as f64) / 0.3) as f32));
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
        // la barrita del tiempo que queda (en Bajo, a menos cuadros)
        ctx.request_repaint_after(if crate::perf::animations() { FRAME_SMOOTH } else { crate::perf::ambient() });
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
        if !self.restarting {
            // al reiniciar ya se guardó antes de abrir el Capyro nuevo
            self.save_session();
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if cfg!(windows) {
            theme::BG_WINDOW.to_normalized_gamma_f32()
        } else {
            [0.0, 0.0, 0.0, 0.0] // esquinas redondeadas transparentes
        }
    }

    fn ui(&mut self, ui: &mut Ui, frame_info: &mut eframe::Frame) {
        crate::render::FIRST_FRAME.store(true, std::sync::atomic::Ordering::Relaxed);
        let ctx = ui.ctx().clone();
        debug_fps(&ctx, frame_info.info().cpu_usage);
        let frame = ctx.cumulative_frame_nr();
        self.first_pass = frame != self.last_frame;
        self.last_frame = frame;
        let now = ctx.input(|i| i.time);

        if ctx.input(|i| i.pointer.delta() != Vec2::ZERO) {
            self.last_pointer_move = now;
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
        self.refresh_now_playing();
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
        if self.cfg.visualizer && crate::perf::animations() && self.first_pass && self.core.media_kind() == MediaKind::Audio {
            let playing = self.core.state() == State::Playing;
            let n = self.viz.len();
            let got = if playing { self.core.scope(&mut self.viz.samples[..n]) } else { 0 };
            self.viz.update(self.core.audio_rate() as f32, dt, playing && got == n);
            if !self.viz.is_silent() {
                // las barras se suavizan solas (viz.rs): a 30 cuadros se ven igual de fluidas
                ctx.request_repaint_after(FRAME_AMBIENT);
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
            music::layout_for(content)
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
        let appear = ease_out(crate::perf::k(((now - self.layout_since) / 0.32) as f32));
        if appear < 1.0 {
            ctx.request_repaint_after(FRAME_SMOOTH);
        }
        // videos: la barra de abajo se puede esconder
        let bar_ok = music.is_none() && self.cfg.hide_bar_button && self.tutorial.is_none() && self.core.media_kind() == MediaKind::Video;
        let bar_k = ctx.animate_bool_with_time(Id::unique("capyro-bar-hidden"), self.bar_hidden && bar_ok, crate::perf::t(0.28));
        let bar_e = 1.0 - (1.0 - bar_k).powi(3);
        if (0.001..0.999).contains(&bar_k) {
            ctx.request_repaint_after(FRAME_SMOOTH);
        }
        let video_bottom = (inner.bottom() - BELOW_VIDEO * (1.0 - bar_e)).max(header.bottom() + 60.0);
        let video = Rect::from_min_max(pos2(inner.left(), header.bottom() + 12.0), pos2(inner.right(), video_bottom));
        let logo_dest = Rect::from_min_size(header.min, vec2(46.0, 46.0));

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
        if self.cfg.performance == crate::perf::Mode::Alto {
            if self.ambient.step(now) {
                ctx.request_repaint_after(FRAME_AMBIENT);
            }
            let accent = self.cfg.accent;
            let (focused, pointer) = ctx.input(|i| (i.focused, i.pointer.hover_pos()));
            let audio_playing = self.core.media_kind() == MediaKind::Audio && self.core.state() == State::Playing;
            let energy = if audio_playing && self.cfg.visualizer { self.viz.energy() } else { 0.0 };
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
        self.draw_toast(ui, toast_area);
        self.draw_update_banner(ui, notice_area);
        self.draw_update_offer(ui, notice_area);
        if !square {
            Self::resize_handles(ui, full);
        }
        // las ondas de los clics (Alto), arriba de los botones
        crate::alto::paint_ripples(&ctx, ui.painter(), self.cfg.accent);
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

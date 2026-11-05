//! Segundo plano (⚙ → Segundo plano): al cerrar la ventana, Capyro sigue
//! sonando con un ícono en la bandeja del sistema. Clic (o clic derecho) en
//! el ícono abre el mini reproductor: lo que suena, el disco de vinilo o la
//! portada, el visualizador, cuánto va y cuánto falta, y los botones Abrir
//! (vuelve la ventana) y Cerrar (cierra Capyro del todo).
//!
//! Mientras está en segundo plano no hay ventana: el bucle de eventos de la
//! ventana termina, el núcleo sigue sonando y este módulo atiende la
//! bandeja. Con un video suena solo el audio (no se decodifica la imagen).
//! Si se abre Capyro otra vez (o un archivo con Capyro), le avisa a este y
//! vuelve la ventana, en vez de abrirse un segundo Capyro.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use crate::ffi::Core;
use crate::system::config::Config;

pub mod instance;
mod popup;
#[cfg(target_os = "linux")]
#[path = "background/tray_linux.rs"]
mod tray;
#[cfg(windows)]
#[path = "background/tray_windows.rs"]
mod tray;
#[cfg(target_os = "macos")]
#[path = "background/tray_macos.rs"]
mod tray;
/// Otros sistemas: sin bandeja (cerrar la ventana cierra Capyro).
#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
mod tray {
    pub struct Tray;
    pub fn start(_tx: std::sync::mpsc::Sender<super::Event>) -> Result<Tray, String> {
        Err("este sistema no tiene bandeja".to_owned())
    }
    pub fn available() -> bool {
        false
    }
}

/// Lo que llega de la bandeja (o de otro Capyro que se abrió).
pub enum Event {
    /// Clic en el ícono, en esta posición de la pantalla (píxeles), y cuándo.
    Popup(i32, i32, Instant),
    /// Clic del medio: pausar o seguir.
    Toggle,
    /// Se abrió Capyro otra vez (con estos archivos, si hay).
    Open(Vec<PathBuf>),
}

/// Cómo termina el segundo plano.
pub enum Wake {
    /// Vuelve la ventana (y se agregan estos archivos).
    Open(Vec<PathBuf>),
    /// Cerrar Capyro del todo.
    Quit,
}

/// Lo que hay que contar al volver la ventana (si la bandeja falló).
static NOTICE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn take_notice() -> Option<String> {
    NOTICE.lock().ok()?.take()
}

/// La ventana pidió quedar en segundo plano al cerrarse.
static PENDING: AtomicBool = AtomicBool::new(false);
/// La bandeja no anduvo en esta sesión: cerrar la ventana cierra Capyro.
static BROKEN: AtomicBool = AtomicBool::new(false);

pub fn set_pending(on: bool) {
    PENDING.store(on, Ordering::SeqCst);
}

pub fn take_pending() -> bool {
    PENDING.swap(false, Ordering::SeqCst)
}

/// ¿Hay dónde poner el ícono? (en Linux, una bandeja con StatusNotifierItem)
pub fn available() -> bool {
    !BROKEN.load(Ordering::Relaxed) && tray::available()
}

/// Lo mismo, pero recordado unos segundos (para el menú ⚙, que se dibuja
/// en cada cuadro y no debería preguntarle a D-Bus cada vez). La primera
/// vez se pregunta en el momento; después, en otro hilo.
pub fn available_cached() -> bool {
    static AT: AtomicU64 = AtomicU64::new(0);
    static OK: AtomicBool = AtomicBool::new(false);
    static ASKING: AtomicBool = AtomicBool::new(false);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_secs().max(1));
    let at = AT.load(Ordering::Relaxed);
    if at == 0 {
        AT.store(now, Ordering::Relaxed);
        OK.store(available(), Ordering::Relaxed);
    } else if now >= at + 10 && !ASKING.swap(true, Ordering::SeqCst) {
        AT.store(now, Ordering::Relaxed);
        std::thread::spawn(|| {
            OK.store(available(), Ordering::Relaxed);
            ASKING.store(false, Ordering::SeqCst);
        });
    }
    OK.load(Ordering::Relaxed)
}

/* --- lo que el mini reproductor necesita saber de la ventana --- */

/// Tamaño del monitor (en puntos) y escala de la última ventana, para
/// ubicar el mini reproductor al lado del ícono antes de crearlo.
static MONITOR: AtomicU64 = AtomicU64::new(0);
static SCALE: AtomicU32 = AtomicU32::new(0);

pub fn remember_monitor(size: Option<eframe::egui::Vec2>, pixels_per_point: Option<f32>) {
    if let Some(s) = size.filter(|s| s.x > 0.0 && s.y > 0.0) {
        MONITOR.store(((s.x as u64) << 32) | s.y as u64, Ordering::Relaxed);
    }
    if let Some(p) = pixels_per_point.filter(|p| *p > 0.0) {
        SCALE.store(p.to_bits(), Ordering::Relaxed);
    }
}

pub(crate) fn monitor() -> Option<eframe::egui::Vec2> {
    let m = MONITOR.load(Ordering::Relaxed);
    (m != 0).then(|| eframe::egui::vec2((m >> 32) as f32, (m & 0xFFFF_FFFF) as f32))
}

pub(crate) fn scale() -> f32 {
    let s = f32::from_bits(SCALE.load(Ordering::Relaxed));
    if s > 0.0 {
        s
    } else {
        1.0
    }
}

/// La posición y la canción, para que al cerrar del todo (o si se apaga la
/// PC) la próxima vez siga desde acá.
fn save_progress(core: &Core, cfg: &Config) {
    if !cfg.remember_session {
        return;
    }
    let Some(mut s) = crate::system::session::Session::load() else { return };
    s.playlist = core.playlist();
    s.current = core.playlist_current();
    s.position = core.position();
    if let Err(e) = s.save() {
        eprintln!("capyro: no pude guardar la sesión: {e}");
    }
}

/// Lo próximo que pasó (o `Timeout` si en `wait` no pasó nada). En la Mac,
/// mientras espera atiende los eventos del sistema (los clics en la barra de
/// menús llegan por ahí) y los archivos que abre el Finder.
fn next_event(rx: &Receiver<Event>, wait: Duration) -> Result<Event, RecvTimeoutError> {
    #[cfg(target_os = "macos")]
    {
        use std::sync::mpsc::TryRecvError;
        let poll = |rx: &Receiver<Event>| {
            // abrieron Capyro.app de nuevo (Launchpad, Spotlight): vuelve la ventana
            if crate::system::macos::take_reopen() {
                return Some(Ok(Event::Open(Vec::new())));
            }
            let files = crate::system::macos::take_opened_files();
            if !files.is_empty() {
                return Some(Ok(Event::Open(files)));
            }
            match rx.try_recv() {
                Ok(e) => Some(Ok(e)),
                Err(TryRecvError::Disconnected) => Some(Err(RecvTimeoutError::Disconnected)),
                Err(TryRecvError::Empty) => None,
            }
        };
        if let Some(r) = poll(rx) {
            return r;
        }
        crate::system::macos::pump(wait);
        poll(rx).unwrap_or(Err(RecvTimeoutError::Timeout))
    }
    #[cfg(not(target_os = "macos"))]
    {
        rx.recv_timeout(wait)
    }
}

/// Corre el segundo plano hasta que se pida la ventana o cerrar.
pub fn run(core: &Core, cfg: &Config, renderer: eframe::Renderer) -> Wake {
    let (tx, rx) = mpsc::channel();
    let tray = match tray::start(tx.clone()) {
        Ok(t) => t,
        Err(e) => {
            // sin ícono no habría cómo volver: se vuelve a abrir la ventana
            crate::system::errlog::log(&format!("no pude quedar en segundo plano: {e}"));
            BROKEN.store(true, Ordering::Relaxed);
            if let Ok(mut n) = NOTICE.lock() {
                *n = Some("No pude quedar en segundo plano: no encontré la bandeja del sistema".to_owned());
            }
            return Wake::Open(Vec::new());
        }
    };
    let listener = instance::listen(tx);
    eprintln!("capyro: en segundo plano (bandeja del sistema)");
    // Mac: sin ícono en el Dock mientras tanto (queda el de la barra de menús)
    #[cfg(target_os = "macos")]
    {
        crate::system::macos::set_background(true);
        let _ = crate::system::macos::take_reopen(); // uno viejo, de con la ventana abierta
    }
    // un video sigue sonando, pero sin decodificar la imagen
    core.set_video_enabled(false);
    // nada más que el ícono: el disco del mini reproductor arranca quieto
    let mut keep = popup::Keep::default();
    let mut last_save = Instant::now();

    let wake = loop {
        core.update();
        if last_save.elapsed() > Duration::from_secs(15) {
            last_save = Instant::now();
            save_progress(core, cfg);
        }
        match next_event(&rx, Duration::from_millis(40)) {
            // el clic que lo cerró al tocar el ícono no lo vuelve a abrir
            Ok(Event::Popup(_, _, at)) if keep.closed_by(at) => {}
            Ok(Event::Popup(x, y, _)) => match popup::show(core, cfg, renderer, &rx, (x, y), &mut keep) {
                popup::Out::Back => {}
                popup::Out::Open(files) => break Wake::Open(files),
                popup::Out::Quit => break Wake::Quit,
            },
            Ok(Event::Toggle) => core.toggle(),
            Ok(Event::Open(files)) => break Wake::Open(files),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break Wake::Quit,
        }
    };
    drop(listener);
    drop(tray);
    match wake {
        Wake::Open(_) => {
            core.set_video_enabled(true);
            // Mac: vuelve el ícono del Dock y Capyro pasa al frente
            #[cfg(target_os = "macos")]
            crate::system::macos::set_background(false);
        }
        Wake::Quit => save_progress(core, cfg),
    }
    wake
}

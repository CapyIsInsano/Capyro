//! Capyro — interfaz gráfica.
//!
//! Este crate se compila como `staticlib` y se linkea al ejecutable en C.
//! `main.c` crea el núcleo y llama a `capyro_gui_run`, que corre el event
//! loop de eframe/egui hasta que se cierra la ventana.

mod app; // la ventana: vistas, menús, Capyro Studio y Capyro Converter
mod background; // segundo plano: el ícono en la bandeja y el mini reproductor
mod ffi; // el puente con el núcleo en C
mod updater; // buscar e instalar actualizaciones

/// Lo que se dibuja: colores y fuentes, íconos, el modo Alto, el disco de vinilo y el visualizador.
mod ui {
    pub mod alto;
    pub mod icons;
    pub mod theme;
    pub mod vinyl;
    pub mod viz;
}

/// Lo que toca el sistema: configuración, carpetas, sesión, playlists,
/// caché, registros, rendimiento, cómo se dibuja la ventana, Wine y la Mac.
mod system {
    pub mod cache;
    pub mod changelog;
    pub mod config;
    #[cfg(any(target_os = "macos", test))]
    pub mod dock;
    pub mod errlog;
    #[cfg(target_os = "macos")]
    pub mod macos;
    pub mod paths;
    pub mod perf;
    pub mod playlists;
    pub mod render;
    pub mod session;
    pub mod uninstall;
    pub mod wine;
}

/// Datos de los formatos: resúmenes de cada uno y nombres de resoluciones.
mod media {
    pub mod formats;
    pub mod videoinfo;
}

use system::{config, errlog, render, wine};

use std::ffi::c_int;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use eframe::egui;

#[no_mangle]
pub extern "C" fn capyro_gui_run(core: *mut ffi::CapyroCore) -> c_int {
    if core.is_null() {
        eprintln!("capyro: capyro_gui_run recibió un núcleo nulo");
        return 1;
    }
    // Un panic no debe cruzar la frontera hacia C.
    errlog::install_panic_hook();
    match catch_unwind(AssertUnwindSafe(|| run(core))) {
        Ok(code) => code,
        Err(_) => {
            errlog::log("la interfaz se cerró por un error interno");
            2
        }
    }
}

fn run(core: *mut ffi::CapyroCore) -> c_int {
    // SAFETY: main.c pasa el puntero de capyro_core_init() y lo libera
    // recién después de que esta función retorna.
    let core = unsafe { ffi::Core::from_raw(core) };
    // ¿hay un Capyro en segundo plano? se le pasan los archivos y vuelve él
    if background::instance::wake_existing() {
        eprintln!("capyro: ya había un Capyro en segundo plano: vuelve su ventana");
        return 0;
    }
    let mut cfg = config::Config::load();
    eprintln!("capyro {} · config: {} · backend: {:?}", ffi::version(), cfg.source, cfg.backend);
    // Mac: los archivos que abre el Finder (doble clic, "Abrir con…") llegan
    // como eventos, no como argumentos: hay que estar listos antes de la ventana
    #[cfg(target_os = "macos")]
    system::macos::install_open_handler();
    // en Wine: que el bucle de eventos no gire en vacío (ver wine.rs)
    wine::install();

    // la ventana; al cerrarla con el segundo plano activado, la bandeja
    // (background.rs) hasta que se pida la ventana otra vez o cerrar
    let mut resumed: Option<Vec<std::path::PathBuf>> = None;
    let mut renderer: Option<eframe::Renderer> = None;
    loop {
        match open_window(core.share(), &cfg, renderer, resumed.take()) {
            Ok(r) => renderer = Some(r),
            Err(code) => return code,
        }
        if !background::take_pending() {
            return 0;
        }
        match background::run(&core, &cfg, renderer.unwrap_or_default()) {
            background::Wake::Open(files) => {
                // la configuración pudo cambiar a mano mientras tanto
                cfg = config::Config::load();
                resumed = Some(files);
            }
            background::Wake::Quit => return 0,
        }
    }
}

/// Abre la ventana y espera a que se cierre. Devuelve con qué se dibujó (o
/// el código de salida si no hubo forma de abrirla).
fn open_window(
    core: ffi::Core,
    cfg: &config::Config,
    known: Option<eframe::Renderer>,
    resumed: Option<Vec<std::path::PathBuf>>,
) -> Result<eframe::Renderer, c_int> {
    // OpenGL o Direct3D (ver render.rs): el que ande (al volver del segundo
    // plano, el que ya anduvo)
    let renderers = match known {
        Some(r) => vec![r],
        None => render::order(cfg.renderer),
    };

    // el núcleo y la config pasan a la app del intento que funcione
    let parts = std::rc::Rc::new(std::cell::RefCell::new(Some((core, cfg.clone(), resumed))));
    let mut errors = Vec::new();
    for (i, &renderer) in renderers.iter().enumerate() {
        if known.is_none() {
            render::watch(renderer, renderers.get(i + 1).copied());
        }
        let slot = parts.clone();
        let result = eframe::run_native(
            "Capyro",
            native_options(cfg, renderer),
            Box::new(move |cc| {
                let (core, cfg, resumed) = slot.borrow_mut().take().ok_or("el núcleo ya se usó")?;
                Ok(Box::new(app::CapyroApp::new(cc, core, cfg, resumed)))
            }),
        );
        match result {
            Ok(()) => return Ok(renderer),
            Err(e) => {
                let msg = format!("{renderer:?}: {e}");
                errlog::log(&format!("no se pudo abrir la ventana con {msg}"));
                errors.push(msg);
                // si la app ya se creó (y se llevó el núcleo) no hay otro intento
                if parts.borrow().is_none() {
                    break;
                }
            }
        }
    }
    errlog::fatal(&format!(
        "No se pudo abrir la ventana de Capyro.\n\n{}\n\nProbá actualizar los drivers de video.",
        errors.join("\n")
    ));
    Err(1)
}

fn native_options(cfg: &config::Config, renderer: eframe::Renderer) -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Capyro")
        .with_app_id("Capyro") // lo que muestran la barra de tareas y el dock
        .with_inner_size([900.0, 660.0]) // v0.3.2: lugar para las tres columnas de la vista de música
        .with_min_inner_size([520.0, 430.0])
        // transparente (esquinas redondeadas) solo en Linux: en Windows
        // muchos drivers no la soportan y la ventana no aparece
        .with_transparent(!ui::theme::OPAQUE_WINDOW)
        .with_drag_and_drop(true);
    // Linux y Windows: sin marco (el encabezado de Capyro tiene sus botones).
    // Mac: el marco del sistema, pero invisible: quedan los tres botones de
    // colores arriba a la izquierda (como en todas las apps de la Mac), la
    // sombra y las esquinas redondeadas; el resto es el encabezado de Capyro.
    viewport = if cfg!(target_os = "macos") {
        viewport
            .with_decorations(true)
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false)
    } else {
        viewport.with_decorations(false)
    };
    if let Some(icon) = app::window_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }

    #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
    let mut options = eframe::NativeOptions {
        viewport,
        centered: true,
        renderer,
        ..Default::default()
    };
    // X11 o Wayland: solo existe en Linux (en Windows no se compila).
    #[cfg(target_os = "linux")]
    {
        use winit::platform::{wayland::EventLoopBuilderExtWayland, x11::EventLoopBuilderExtX11};
        match cfg.backend {
            config::Backend::X11 => {
                options.event_loop_builder = Some(Box::new(|b| {
                    b.with_x11();
                }))
            }
            config::Backend::Wayland => {
                options.event_loop_builder = Some(Box::new(|b| {
                    b.with_wayland();
                }))
            }
            config::Backend::Auto => {}
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = cfg;
    options
}

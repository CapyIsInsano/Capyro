//! Capyro — interfaz gráfica.
//!
//! Este crate se compila como `staticlib` y se linkea al ejecutable en C.
//! `main.c` crea el núcleo y llama a `capyro_gui_run`, que corre el event
//! loop de eframe/egui hasta que se cierra la ventana.

mod app;
mod config;
mod ffi;
mod icons;
mod theme;

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
    match catch_unwind(AssertUnwindSafe(|| run(core))) {
        Ok(code) => code,
        Err(_) => {
            eprintln!("capyro: la interfaz se cerró por un error interno");
            2
        }
    }
}

fn run(core: *mut ffi::CapyroCore) -> c_int {
    // SAFETY: main.c pasa el puntero de capyro_core_init() y lo libera
    // recién después de que esta función retorna.
    let core = unsafe { ffi::Core::from_raw(core) };
    let cfg = config::Config::load();
    eprintln!("capyro {} · config: {}", ffi::version(), cfg.source);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Capyro")
        .with_app_id("capyro")
        .with_inner_size([768.0, 642.0])
        .with_min_inner_size([520.0, 430.0])
        .with_decorations(false)
        .with_transparent(true)
        .with_drag_and_drop(true);
    if let Some(icon) = app::window_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        centered: true,
        ..Default::default()
    };

    let result = eframe::run_native(
        "Capyro",
        options,
        Box::new(move |cc| Ok(Box::new(app::CapyroApp::new(cc, core, cfg)))),
    );

    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("capyro: no se pudo abrir la ventana: {e}");
            1
        }
    }
}

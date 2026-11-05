//! Lo que es solo de la Mac y no trae winit:
//!
//! - **Abrir archivos desde el Finder** (doble clic, "Abrir con…", soltarlos
//!   en el ícono del Dock): macOS no los pasa como argumentos, se los manda a
//!   la app con un evento. NSApplication se los da al "delegado" de la app
//!   con `application:openURLs:`, pero el delegado es el de winit, que no lo
//!   tiene. Se lo agregamos a NSObject (del que hereda el de winit) antes de
//!   que arranque la ventana: así llegan también los que abrieron Capyro.
//!   Quedan en una cola que la ventana (o el segundo plano) revisa.
//! - **Segundo plano**: el ícono del Dock se va mientras Capyro suena desde
//!   la barra de menús ([`set_background`]) y, como no hay ventana, hay que
//!   atender los eventos de macOS a mano ([`pump`]).
//! - **Desinstalar**: Capyro.app a la Papelera ([`move_to_trash`]).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
use objc2::{sel, AnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSEventMask};
use objc2_foundation::{NSArray, NSDate, NSDefaultRunLoopMode, NSFileManager, NSString, NSURL};

/// Archivos que mandó el Finder y todavía nadie agregó.
static OPENED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
/// Para despertar a la ventana cuando llegan (si hay una abierta).
static WAKE: Mutex<Option<eframe::egui::Context>> = Mutex::new(None);
/// Se abrió Capyro otra vez (Launchpad, Spotlight, el Finder) sin ventanas.
static REOPEN: AtomicBool = AtomicBool::new(false);

/// `applicationShouldHandleReopen:hasVisibleWindows:`: abrir Capyro.app
/// cuando ya está abierto. Sin ventanas (segundo plano), vuelve la ventana.
unsafe extern "C-unwind" fn reopen(_this: *mut AnyObject, _cmd: Sel, _app: *mut AnyObject, visible: Bool) -> Bool {
    if !visible.as_bool() {
        REOPEN.store(true, Ordering::SeqCst);
    }
    Bool::YES
}

/// ¿Se pidió volver a abrir Capyro? (y lo olvida)
pub fn take_reopen() -> bool {
    REOPEN.swap(false, Ordering::SeqCst)
}

/// `application:openURLs:` (lo llama NSApplication, en el hilo principal).
unsafe extern "C-unwind" fn open_urls(_this: *mut AnyObject, _cmd: Sel, _app: *mut AnyObject, urls: *mut NSArray<NSURL>) {
    // SAFETY: NSApplication pasa un NSArray<NSURL> válido durante la llamada.
    let Some(urls) = (unsafe { urls.as_ref() }) else { return };
    let mut got = Vec::new();
    for url in urls.iter() {
        if let Some(path) = url.path() {
            got.push(PathBuf::from(path.to_string()));
        }
    }
    if got.is_empty() {
        return;
    }
    eprintln!("capyro: el Finder abre {} archivo(s)", got.len());
    if let Ok(mut q) = OPENED.lock() {
        q.extend(got);
    }
    if let Some(ctx) = WAKE.lock().ok().and_then(|c| c.clone()) {
        ctx.request_repaint();
    }
}

/// Antes de abrir la ventana: que NSApplication nos pase los archivos.
pub fn install_open_handler() {
    static DONE: std::sync::Once = std::sync::Once::new();
    DONE.call_once(|| {
        let Some(cls) = AnyClass::get(c"NSObject") else { return };
        // SAFETY: la firma (void, id, SEL, id, NSArray*) es la de
        // application:openURLs: ("v@:@@"); la función vive todo el programa.
        let added = unsafe {
            let imp: Imp = std::mem::transmute::<
                unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject, *mut NSArray<NSURL>),
                Imp,
            >(open_urls);
            objc2::ffi::class_addMethod(
                cls as *const AnyClass as *mut AnyClass,
                sel!(application:openURLs:),
                imp,
                c"v@:@@".as_ptr(),
            )
        };
        if !added.as_bool() {
            eprintln!("capyro: no pude recibir los archivos que abre el Finder");
        }
        // BOOL es `bool` en Apple Silicon y `signed char` en Intel
        let types = if cfg!(target_arch = "aarch64") { c"B@:@B" } else { c"c@:@c" };
        // SAFETY: misma firma que applicationShouldHandleReopen:hasVisibleWindows:.
        unsafe {
            let imp: Imp =
                std::mem::transmute::<unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject, Bool) -> Bool, Imp>(reopen);
            objc2::ffi::class_addMethod(
                cls as *const AnyClass as *mut AnyClass,
                sel!(applicationShouldHandleReopen:hasVisibleWindows:),
                imp,
                types.as_ptr(),
            );
        }
    });
}

/// Los archivos que llegaron desde el Finder (y los saca de la cola).
pub fn take_opened_files() -> Vec<PathBuf> {
    OPENED.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
}

/// La ventana que hay que despertar cuando llegan archivos (`None` al cerrarla).
pub fn set_wake(ctx: Option<eframe::egui::Context>) {
    if let Ok(mut w) = WAKE.lock() {
        *w = ctx;
    }
}

/// `true`: sin ícono en el Dock (Capyro vive en la barra de menús); `false`:
/// vuelve el ícono y Capyro pasa al frente.
pub fn set_background(on: bool) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    if on {
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    } else {
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
    }
}

/// Que la app pase al frente (el mini reproductor, aunque Capyro no tenga
/// ícono en el Dock).
pub fn activate() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    #[allow(deprecated)]
    NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
}

/// Atiende los eventos de macOS (clics en la barra de menús, archivos del
/// Finder, cerrar sesión...) durante hasta `timeout`. Sin ventana, nadie más
/// lo hace: sin esto la Mac diría que Capyro "no responde".
pub fn pump(timeout: Duration) {
    let Some(mtm) = MainThreadMarker::new() else {
        std::thread::sleep(timeout);
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    // el primero espera (hasta `timeout`); los que siguen, sin esperar
    let mut until = NSDate::dateWithTimeIntervalSinceNow(timeout.as_secs_f64());
    loop {
        // SAFETY: NSDefaultRunLoopMode es una constante de Foundation.
        let mode = unsafe { NSDefaultRunLoopMode };
        let event = app.nextEventMatchingMask_untilDate_inMode_dequeue(NSEventMask::Any, Some(&until), mode, true);
        let Some(event) = event else { break };
        app.sendEvent(&event);
        until = NSDate::distantPast();
    }
}

/// Mueve `path` (Capyro.app) a la Papelera, como arrastrarlo con el Finder.
pub fn move_to_trash(path: &Path) -> Result<(), String> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, None)
        .map_err(|e| e.localizedDescription().to_string())
}

/// El Capyro.app en el que corre este Capyro (si corre desde uno).
pub fn bundle_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    // Capyro.app/Contents/MacOS/Capyro
    let app = exe.parent()?.parent()?.parent()?.to_path_buf();
    app.extension().is_some_and(|e| e == "app").then_some(app)
}

/// El logo en PNG como imagen de AppKit, de `size` puntos.
pub fn ns_image(png: &[u8], size: f64) -> Option<Retained<objc2_app_kit::NSImage>> {
    let data = objc2_foundation::NSData::with_bytes(png);
    let img = objc2_app_kit::NSImage::initWithData(objc2_app_kit::NSImage::alloc(), &data)?;
    img.setSize(objc2_foundation::NSSize::new(size, size));
    Some(img)
}

/* ------------------------------------------------------------------------ */
/* el Dock                                                                  */
/* ------------------------------------------------------------------------ */

/// Saca a Capyro del Dock (si lo agregó el instalador): cambia la lista de
/// apps fijas del Dock (`com.apple.dock persistent-apps`) con `defaults` y
/// reinicia el Dock. Si algo no sale, no toca nada.
pub fn remove_from_dock(app: &Path) {
    let Ok(out) = std::process::Command::new("/usr/bin/defaults").args(["export", "com.apple.dock", "-"]).output() else {
        return;
    };
    if !out.status.success() {
        return;
    }
    let xml = String::from_utf8_lossy(&out.stdout);
    let Some(fixed) = super::dock::dock_without(&xml, &app.to_string_lossy()) else { return };
    let Ok(mut child) = std::process::Command::new("/usr/bin/defaults")
        .args(["import", "com.apple.dock", "-"])
        .stdin(std::process::Stdio::piped())
        .spawn()
    else {
        return;
    };
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(fixed.as_bytes());
    }
    if child.wait().is_ok_and(|s| s.success()) {
        let _ = std::process::Command::new("/usr/bin/killall").arg("Dock").status();
    }
}

//! Errores que tienen que verse aunque no haya terminal: en Windows Capyro
//! no tiene consola, así que además va un cartel y un log en
//! %LOCALAPPDATA%\\Capyro\\capyro.log. En Linux, a stderr como siempre.

pub fn log(msg: &str) {
    eprintln!("capyro: {msg}");
    #[cfg(windows)]
    {
        use std::io::Write;
        let path = crate::paths::cache_dir().with_file_name("capyro.log");
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(f, "{msg}");
        }
    }
}

/// Error que cierra Capyro: log y, en Windows, un cartel.
pub fn fatal(msg: &str) {
    log(msg);
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(hwnd: *mut std::ffi::c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
        }
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let text = wide(msg);
        let caption = wide("Capyro");
        // SAFETY: cadenas UTF-16 terminadas en 0 que viven durante la llamada.
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), 0x10); // MB_ICONERROR
        }
    }
}

/// En Windows, un panic muestra el cartel (si no, Capyro se cierra mudo).
pub fn install_panic_hook() {
    #[cfg(windows)]
    {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            fatal(&format!("Capyro se cerró por un error:\n{info}"));
            default(info);
        }));
    }
}

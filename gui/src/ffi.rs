//! Frontera con el núcleo en C. Debe coincidir con `core/include/capyro_core.h`.
//!
//! `Core` envuelve el puntero opaco y expone métodos seguros: el resto de la
//! GUI nunca toca `unsafe`.

use std::ffi::{c_char, c_int, c_long, CStr, CString};
use std::path::Path;

#[repr(C)]
pub struct CapyroCore {
    _private: [u8; 0],
}

/// Debe coincidir con `CapyroVideoFrame` de capyro_core.h.
#[repr(C)]
struct RawVideoFrame {
    rgba: *const u8,
    width: i32,
    height: i32,
    stride: i32,
    aspect: f32,
    serial: u64,
}

/// Vista prestada del frame actual; solo vive dentro de `Core::with_video_frame`.
pub struct VideoFrame<'a> {
    pub rgba: &'a [u8],
    pub width: usize,
    pub height: usize,
    pub aspect: f32,
    pub serial: u64,
}

extern "C" {
    fn capyro_core_version() -> *const c_char;
    fn capyro_core_error_string(err: c_int) -> *const c_char;
    fn capyro_core_update(core: *mut CapyroCore);
    fn capyro_core_set_auto_next(core: *mut CapyroCore, enabled: bool);
    fn capyro_core_audio_backend(core: *mut CapyroCore) -> *const c_char;
    fn capyro_core_get_title(core: *mut CapyroCore) -> *const c_char;
    fn capyro_core_get_artist(core: *mut CapyroCore) -> *const c_char;
    fn capyro_core_lock_video_frame(core: *mut CapyroCore, out: *mut RawVideoFrame) -> bool;
    fn capyro_core_unlock_video_frame(core: *mut CapyroCore);

    fn capyro_core_toggle(core: *mut CapyroCore);
    fn capyro_core_stop(core: *mut CapyroCore);
    fn capyro_core_seek(core: *mut CapyroCore, seconds: f64);
    fn capyro_core_seek_relative(core: *mut CapyroCore, delta: f64);

    fn capyro_core_set_volume(core: *mut CapyroCore, volume: f32);
    fn capyro_core_get_volume(core: *mut CapyroCore) -> f32;
    fn capyro_core_set_muted(core: *mut CapyroCore, muted: bool);
    fn capyro_core_get_muted(core: *mut CapyroCore) -> bool;

    fn capyro_core_get_state(core: *mut CapyroCore) -> c_int;
    fn capyro_core_get_media_kind(core: *mut CapyroCore) -> c_int;
    fn capyro_core_get_position(core: *mut CapyroCore) -> f64;
    fn capyro_core_get_duration(core: *mut CapyroCore) -> f64;
    fn capyro_core_get_current_file(core: *mut CapyroCore) -> *const c_char;

    fn capyro_core_playlist_add(core: *mut CapyroCore, path: *const c_char) -> c_int;
    fn capyro_core_playlist_count(core: *mut CapyroCore) -> usize;
    fn capyro_core_playlist_get(core: *mut CapyroCore, index: usize) -> *const c_char;
    fn capyro_core_playlist_current(core: *mut CapyroCore) -> c_long;
    fn capyro_core_playlist_select(core: *mut CapyroCore, index: usize) -> c_int;
    fn capyro_core_playlist_next(core: *mut CapyroCore) -> c_int;
    fn capyro_core_playlist_prev(core: *mut CapyroCore) -> c_int;
    fn capyro_core_playlist_remove(core: *mut CapyroCore, index: usize);
    fn capyro_core_playlist_clear(core: *mut CapyroCore);
}

const CAPYRO_OK: c_int = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    None,
    Video,
    Audio,
}

/// Error del núcleo, ya traducido a texto por `capyro_core_error_string`.
#[derive(Debug, Clone)]
pub struct CoreError(pub String);

fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: el núcleo siempre devuelve cadenas terminadas en NUL que viven
    // al menos hasta la próxima llamada que modifique la lista.
    unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
}

fn check(code: c_int) -> Result<(), CoreError> {
    if code == CAPYRO_OK {
        Ok(())
    } else {
        // SAFETY: función pura del núcleo, sin estado.
        Err(CoreError(cstr_to_string(unsafe { capyro_core_error_string(code) })))
    }
}

fn path_to_cstring(path: &Path) -> Option<CString> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        CString::new(path.as_os_str().as_bytes()).ok()
    }
    #[cfg(not(unix))]
    {
        // TODO(windows): el núcleo usa fopen(); para rutas con caracteres no
        // ASCII habría que pasar a _wfopen con UTF-16.
        CString::new(path.to_string_lossy().as_bytes()).ok()
    }
}

pub fn version() -> String {
    // SAFETY: devuelve un literal estático.
    cstr_to_string(unsafe { capyro_core_version() })
}

/// Vista segura sobre el núcleo. No es dueña del puntero: lo crea y lo
/// destruye `main.c`, que vive más que toda la GUI.
pub struct Core {
    ptr: *mut CapyroCore,
}

// Todas las llamadas son `unsafe` sólo por ser FFI; el puntero es válido
// durante toda la vida de `Core` (garantizado por main.c).
impl Core {
    /// # Safety
    /// `ptr` debe venir de `capyro_core_init()` y seguir vivo mientras exista `Core`.
    pub unsafe fn from_raw(ptr: *mut CapyroCore) -> Self {
        Self { ptr }
    }

    /// Fin de archivo y paso al siguiente; se llama una vez por frame.
    pub fn update(&self) {
        unsafe { capyro_core_update(self.ptr) }
    }
    pub fn set_auto_next(&self, enabled: bool) {
        unsafe { capyro_core_set_auto_next(self.ptr, enabled) }
    }
    pub fn audio_backend(&self) -> String {
        cstr_to_string(unsafe { capyro_core_audio_backend(self.ptr) })
    }
    pub fn title(&self) -> String {
        cstr_to_string(unsafe { capyro_core_get_title(self.ptr) })
    }
    pub fn artist(&self) -> String {
        cstr_to_string(unsafe { capyro_core_get_artist(self.ptr) })
    }

    /// Presta el frame visible (video o carátula) mientras dura `f`. El núcleo
    /// queda bloqueado durante la llamada: copiar y salir rápido.
    pub fn with_video_frame<R>(&self, f: impl FnOnce(&VideoFrame<'_>) -> R) -> Option<R> {
        let mut raw = RawVideoFrame {
            rgba: std::ptr::null(),
            width: 0,
            height: 0,
            stride: 0,
            aspect: 1.0,
            serial: 0,
        };
        if !unsafe { capyro_core_lock_video_frame(self.ptr, &mut raw) } {
            return None;
        }
        // Se suelta el lock aunque `f` haga panic.
        struct Unlock(*mut CapyroCore);
        impl Drop for Unlock {
            fn drop(&mut self) {
                unsafe { capyro_core_unlock_video_frame(self.0) }
            }
        }
        let _guard = Unlock(self.ptr);

        let (w, h) = (raw.width.max(0) as usize, raw.height.max(0) as usize);
        if raw.rgba.is_null() || w == 0 || h == 0 || raw.stride as usize != w * 4 {
            return None;
        }
        // SAFETY: el núcleo garantiza w*h*4 bytes válidos mientras está bloqueado.
        let rgba = unsafe { std::slice::from_raw_parts(raw.rgba, w * h * 4) };
        Some(f(&VideoFrame {
            rgba,
            width: w,
            height: h,
            aspect: if raw.aspect > 0.0 { raw.aspect } else { w as f32 / h as f32 },
            serial: raw.serial,
        }))
    }

    pub fn toggle(&self) {
        unsafe { capyro_core_toggle(self.ptr) }
    }
    pub fn stop(&self) {
        unsafe { capyro_core_stop(self.ptr) }
    }
    pub fn seek(&self, seconds: f64) {
        unsafe { capyro_core_seek(self.ptr, seconds) }
    }
    pub fn seek_relative(&self, delta: f64) {
        unsafe { capyro_core_seek_relative(self.ptr, delta) }
    }

    pub fn volume(&self) -> f32 {
        unsafe { capyro_core_get_volume(self.ptr) }
    }
    pub fn set_volume(&self, v: f32) {
        unsafe { capyro_core_set_volume(self.ptr, v) }
    }
    pub fn muted(&self) -> bool {
        unsafe { capyro_core_get_muted(self.ptr) }
    }
    pub fn set_muted(&self, m: bool) {
        unsafe { capyro_core_set_muted(self.ptr, m) }
    }

    pub fn state(&self) -> State {
        match unsafe { capyro_core_get_state(self.ptr) } {
            1 => State::Playing,
            2 => State::Paused,
            3 => State::Stopped,
            _ => State::Idle,
        }
    }
    pub fn media_kind(&self) -> MediaKind {
        match unsafe { capyro_core_get_media_kind(self.ptr) } {
            1 => MediaKind::Video,
            2 => MediaKind::Audio,
            _ => MediaKind::None,
        }
    }
    pub fn position(&self) -> f64 {
        unsafe { capyro_core_get_position(self.ptr) }
    }
    pub fn duration(&self) -> f64 {
        unsafe { capyro_core_get_duration(self.ptr) }
    }
    pub fn current_file(&self) -> String {
        cstr_to_string(unsafe { capyro_core_get_current_file(self.ptr) })
    }

    /// Agrega a la lista sin cambiar lo que está sonando.
    pub fn playlist_add(&self, path: &Path) -> Result<(), CoreError> {
        let c = path_to_cstring(path)
            .ok_or_else(|| CoreError("ruta inválida".to_owned()))?;
        check(unsafe { capyro_core_playlist_add(self.ptr, c.as_ptr()) })
    }
    pub fn playlist_len(&self) -> usize {
        unsafe { capyro_core_playlist_count(self.ptr) }
    }
    pub fn playlist(&self) -> Vec<String> {
        let n = unsafe { capyro_core_playlist_count(self.ptr) };
        (0..n)
            .map(|i| cstr_to_string(unsafe { capyro_core_playlist_get(self.ptr, i) }))
            .collect()
    }
    pub fn playlist_current(&self) -> Option<usize> {
        let c = unsafe { capyro_core_playlist_current(self.ptr) };
        usize::try_from(c).ok()
    }
    pub fn playlist_select(&self, index: usize) -> Result<(), CoreError> {
        check(unsafe { capyro_core_playlist_select(self.ptr, index) })
    }
    pub fn next(&self) -> Result<(), CoreError> {
        check(unsafe { capyro_core_playlist_next(self.ptr) })
    }
    pub fn prev(&self) -> Result<(), CoreError> {
        check(unsafe { capyro_core_playlist_prev(self.ptr) })
    }
    pub fn playlist_remove(&self, index: usize) {
        unsafe { capyro_core_playlist_remove(self.ptr, index) }
    }
    pub fn playlist_clear(&self) {
        unsafe { capyro_core_playlist_clear(self.ptr) }
    }
}

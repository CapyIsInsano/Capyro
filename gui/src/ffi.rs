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

/// Efectos de audio. Debe coincidir con `CapyroAudioFx` de capyro_core.h.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioFx {
    pub enabled: bool,
    pub bass_db: f32,
    pub mid_db: f32,
    pub treble_db: f32,
    pub width: f32,
    pub night: bool,
}

impl Default for AudioFx {
    fn default() -> Self {
        Self { enabled: true, bass_db: 0.0, mid_db: 0.0, treble_db: 0.0, width: 1.0, night: false }
    }
}

impl AudioFx {
    /// Todo en su valor neutro (aunque el interruptor esté prendido).
    pub fn is_neutral(&self) -> bool {
        self.bass_db.abs() < 0.05
            && self.mid_db.abs() < 0.05
            && self.treble_db.abs() < 0.05
            && (self.width - 1.0).abs() < 0.005
            && !self.night
    }
}

/// Debe coincidir con `CapyroBuildInfo` de capyro_core.h.
#[repr(C)]
struct RawBuildInfo {
    commit: *const c_char,
    branch: *const c_char,
    remote: *const c_char,
    subject: *const c_char,
    commit_time: i64,
    dirty: bool,
    prefix: *const c_char,
    bindir: *const c_char,
    build_type: *const c_char,
    dist: bool,
    native: bool,
    system: *const c_char,
}

/// Debe coincidir con `CapyroOsInfo` de capyro_core.h.
#[repr(C)]
struct RawOsInfo {
    family: c_int,
    major: c_int,
    code: [c_char; 4],
    name: [c_char; 64],
    version: [c_char; 64],
}

/// El sistema en el que corre Capyro (lo muestra el tutorial).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OsInfo {
    /// true = Windows, false = Linux
    pub windows: bool,
    /// Windows: 10 u 11. Linux: 0.
    pub major: i32,
    /// "Lin" o "Win"
    pub code: String,
    /// "Windows 11", "Gentoo Linux"...
    pub name: String,
    /// "23H2 · build 22631", "kernel 6.18.0"...
    pub version: String,
}

/// De qué commit sale este binario y dónde se instala (lo usa el actualizador).
#[derive(Debug, Clone, Default)]
pub struct BuildInfo {
    pub commit: String,
    pub branch: String,
    pub remote: String,
    pub subject: String,
    pub commit_time: i64,
    pub dirty: bool,
    pub prefix: String,
    pub bindir: String,
    pub build_type: String,
    pub dist: bool,
    pub native: bool,
    /// Para qué sistema se compiló: "Lin" o "Win".
    pub system: String,
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
/// Debe coincidir con `CapyroVideoProps` de capyro_core.h.
#[repr(C)]
struct RawVideoProps {
    width: i32,
    height: i32,
    fps: f64,
}

/// Debe coincidir con `CapyroImage` de capyro_core.h.
#[repr(C)]
struct RawImage {
    rgba: *mut u8,
    width: i32,
    height: i32,
}

pub struct VideoFrame<'a> {
    pub rgba: &'a [u8],
    pub width: usize,
    pub height: usize,
    pub aspect: f32,
    pub serial: u64,
}

extern "C" {
    fn capyro_core_version() -> *const c_char;
    fn capyro_core_build_info() -> *const RawBuildInfo;
    fn capyro_core_os_info(out: *mut RawOsInfo);
    fn capyro_core_supported_video() -> *const c_char;
    fn capyro_core_supported_audio() -> *const c_char;
    fn capyro_core_error_string(err: c_int) -> *const c_char;
    fn capyro_core_update(core: *mut CapyroCore);
    fn capyro_core_set_fx(core: *mut CapyroCore, fx: *const AudioFx);
    fn capyro_core_get_fx(core: *mut CapyroCore, out: *mut AudioFx);
    fn capyro_core_set_pitch(core: *mut CapyroCore, semitones: f64);
    fn capyro_core_set_hwdec(core: *mut CapyroCore, mode: c_int);
    fn capyro_core_get_hwdec(core: *mut CapyroCore) -> c_int;
    fn capyro_core_get_decode_info(core: *mut CapyroCore, out: *mut RawDecodeInfo);
    fn capyro_core_get_pitch(core: *mut CapyroCore) -> f64;
    fn capyro_core_audio_rate(core: *mut CapyroCore) -> std::ffi::c_uint;
    fn capyro_core_get_scope(core: *mut CapyroCore, out: *mut f32, n: usize) -> usize;
    fn capyro_core_sfx_load(core: *mut CapyroCore, slot: c_int, data: *const u8, size: usize) -> c_int;
    fn capyro_core_sfx_play(core: *mut CapyroCore, slot: c_int);
    fn capyro_core_set_speed(core: *mut CapyroCore, speed: f64);
    fn capyro_core_get_speed(core: *mut CapyroCore) -> f64;
    fn capyro_core_set_repeat(core: *mut CapyroCore, mode: c_int);
    fn capyro_core_get_repeat(core: *mut CapyroCore) -> c_int;
    fn capyro_core_set_shuffle(core: *mut CapyroCore, enabled: bool);
    fn capyro_core_get_shuffle(core: *mut CapyroCore) -> bool;
    fn capyro_core_set_auto_next(core: *mut CapyroCore, enabled: bool);
    fn capyro_core_audio_backend(core: *mut CapyroCore) -> *const c_char;
    fn capyro_core_get_title(core: *mut CapyroCore) -> *const c_char;
    fn capyro_core_get_artist(core: *mut CapyroCore) -> *const c_char;
    fn capyro_core_get_album(core: *mut CapyroCore) -> *const c_char;
    fn capyro_core_get_change_serial(core: *mut CapyroCore) -> u64;
    fn capyro_core_lock_video_frame(core: *mut CapyroCore, out: *mut RawVideoFrame) -> bool;
    fn capyro_core_unlock_video_frame(core: *mut CapyroCore);
    fn capyro_core_set_video_target(core: *mut CapyroCore, width: c_int, height: c_int);
    fn capyro_core_get_video_props(core: *mut CapyroCore, out: *mut RawVideoProps) -> bool;
    fn capyro_core_video_next_due(core: *mut CapyroCore) -> f64;
    fn capyro_core_capture_frame(core: *mut CapyroCore, out: *mut RawImage) -> bool;
    fn capyro_core_free_image(img: *mut RawImage);

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
pub enum Repeat {
    Off,
    All,
    One,
}

impl Repeat {
    /// Off → All → One → Off (el ciclo del botón).
    pub fn next(self) -> Self {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

/// Cómo se decodifica el video. Debe coincidir con `CapyroHwdec`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hwdec {
    Auto,
    Gpu,
    Cpu,
}

impl Hwdec {
    pub const ALL: [Hwdec; 3] = [Hwdec::Auto, Hwdec::Gpu, Hwdec::Cpu];

    pub fn label(self) -> &'static str {
        match self {
            Hwdec::Auto => "Automático",
            Hwdec::Gpu => "GPU",
            Hwdec::Cpu => "CPU",
        }
    }
    /// Como va en capyro.lua.
    pub fn name(self) -> &'static str {
        match self {
            Hwdec::Auto => "auto",
            Hwdec::Gpu => "gpu",
            Hwdec::Cpu => "cpu",
        }
    }
    fn to_c(self) -> c_int {
        match self {
            Hwdec::Auto => 0,
            Hwdec::Gpu => 1,
            Hwdec::Cpu => 2,
        }
    }
    fn from_c(v: c_int) -> Self {
        match v {
            1 => Hwdec::Gpu,
            2 => Hwdec::Cpu,
            _ => Hwdec::Auto,
        }
    }
}

/// Debe coincidir con `CapyroDecodeInfo` de capyro_core.h.
#[repr(C)]
struct RawDecodeInfo {
    active: bool,
    hw: bool,
    api: [c_char; 16],
    codec: [c_char; 24],
    note: [c_char; 128],
    serial: u64,
}

/// Qué está decodificando el video ahora.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DecodeInfo {
    /// Hay un video abierto.
    pub active: bool,
    /// Lo decodifica la GPU.
    pub hw: bool,
    /// "VA-API", "NVDEC", "VDPAU" o "CPU".
    pub api: String,
    pub codec: String,
    /// Por qué no se usa la GPU ("" si se usa).
    pub note: String,
    pub serial: u64,
}

fn carray_to_string(a: &[c_char]) -> String {
    let bytes: Vec<u8> = a.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
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
        // En Linux una ruta son bytes: se pasan tal cual (aunque no sean UTF-8).
        CString::new(path.as_os_str().as_bytes()).ok()
    }
    #[cfg(windows)]
    {
        // En Windows el núcleo (y FFmpeg) reciben UTF-8 y lo pasan a UTF-16.
        CString::new(path.to_str()?).ok()
    }
}

/// Extensiones que acepta el núcleo (en minúscula, sin punto).
pub fn supported_video() -> Vec<String> {
    split_exts(cstr_to_string(unsafe { capyro_core_supported_video() }))
}

pub fn supported_audio() -> Vec<String> {
    split_exts(cstr_to_string(unsafe { capyro_core_supported_audio() }))
}

fn split_exts(list: String) -> Vec<String> {
    list.split(',').map(|e| e.trim().to_owned()).filter(|e| !e.is_empty()).collect()
}

pub fn build_info() -> BuildInfo {
    // SAFETY: devuelve un puntero a un struct estático con literales.
    let raw = unsafe { &*capyro_core_build_info() };
    BuildInfo {
        commit: cstr_to_string(raw.commit),
        branch: cstr_to_string(raw.branch),
        remote: cstr_to_string(raw.remote),
        subject: cstr_to_string(raw.subject),
        commit_time: raw.commit_time,
        dirty: raw.dirty,
        prefix: cstr_to_string(raw.prefix),
        bindir: cstr_to_string(raw.bindir),
        build_type: cstr_to_string(raw.build_type),
        dist: raw.dist,
        native: raw.native,
        system: cstr_to_string(raw.system),
    }
}

/// Qué sistema es este (nombre y versión, para mostrar).
pub fn os_info() -> OsInfo {
    let mut raw = RawOsInfo { family: 0, major: 0, code: [0; 4], name: [0; 64], version: [0; 64] };
    // SAFETY: el núcleo llena el struct; los textos terminan en NUL.
    unsafe { capyro_core_os_info(&mut raw) };
    let text = |b: &[c_char]| {
        let bytes: Vec<u8> = b.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
        String::from_utf8_lossy(&bytes).into_owned()
    };
    OsInfo {
        windows: raw.family == 2,
        major: raw.major,
        code: text(&raw.code),
        name: text(&raw.name),
        version: text(&raw.version),
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
    /// Decodifica un efecto de sonido (archivo entero en memoria) en `slot`.
    pub fn sfx_load(&self, slot: usize, data: &[u8]) -> Result<(), CoreError> {
        check(unsafe { capyro_core_sfx_load(self.ptr, slot as c_int, data.as_ptr(), data.len()) })
    }
    pub fn sfx_play(&self, slot: usize) {
        unsafe { capyro_core_sfx_play(self.ptr, slot as c_int) }
    }
    pub fn speed(&self) -> f64 {
        unsafe { capyro_core_get_speed(self.ptr) }
    }
    pub fn set_speed(&self, speed: f64) {
        unsafe { capyro_core_set_speed(self.ptr, speed) }
    }
    pub fn repeat(&self) -> Repeat {
        match unsafe { capyro_core_get_repeat(self.ptr) } {
            1 => Repeat::All,
            2 => Repeat::One,
            _ => Repeat::Off,
        }
    }
    pub fn set_repeat(&self, r: Repeat) {
        let mode = match r {
            Repeat::Off => 0,
            Repeat::All => 1,
            Repeat::One => 2,
        };
        unsafe { capyro_core_set_repeat(self.ptr, mode) }
    }
    pub fn shuffle(&self) -> bool {
        unsafe { capyro_core_get_shuffle(self.ptr) }
    }
    pub fn set_shuffle(&self, on: bool) {
        unsafe { capyro_core_set_shuffle(self.ptr, on) }
    }
    pub fn fx(&self) -> AudioFx {
        let mut fx = AudioFx::default();
        unsafe { capyro_core_get_fx(self.ptr, &mut fx) };
        fx
    }
    pub fn set_fx(&self, fx: &AudioFx) {
        unsafe { capyro_core_set_fx(self.ptr, fx) }
    }
    pub fn pitch(&self) -> f64 {
        unsafe { capyro_core_get_pitch(self.ptr) }
    }
    pub fn set_pitch(&self, semitones: f64) {
        unsafe { capyro_core_set_pitch(self.ptr, semitones) }
    }
    pub fn set_hwdec(&self, mode: Hwdec) {
        unsafe { capyro_core_set_hwdec(self.ptr, mode.to_c()) }
    }
    pub fn hwdec(&self) -> Hwdec {
        Hwdec::from_c(unsafe { capyro_core_get_hwdec(self.ptr) })
    }
    pub fn decode_info(&self) -> DecodeInfo {
        // SAFETY: struct POD; el núcleo lo llena entero.
        let mut raw: RawDecodeInfo = unsafe { std::mem::zeroed() };
        unsafe { capyro_core_get_decode_info(self.ptr, &mut raw) };
        DecodeInfo {
            active: raw.active,
            hw: raw.hw,
            api: carray_to_string(&raw.api),
            codec: carray_to_string(&raw.codec),
            note: carray_to_string(&raw.note),
            serial: raw.serial,
        }
    }
    pub fn audio_rate(&self) -> u32 {
        unsafe { capyro_core_audio_rate(self.ptr) }
    }
    /// Llena `out` con las últimas muestras que sonaron (mono).
    pub fn scope(&self, out: &mut [f32]) -> usize {
        unsafe { capyro_core_get_scope(self.ptr, out.as_mut_ptr(), out.len()) }
    }
    pub fn audio_backend(&self) -> String {
        cstr_to_string(unsafe { capyro_core_audio_backend(self.ptr) })
    }
    pub fn title(&self) -> String {
        cstr_to_string(unsafe { capyro_core_get_title(self.ptr) })
    }
    pub fn album(&self) -> String {
        cstr_to_string(unsafe { capyro_core_get_album(self.ptr) })
    }

    /// Cambia cuando cambia el archivo cargado o la lista (ver [`NowPlaying`]).
    pub fn change_serial(&self) -> u64 {
        unsafe { capyro_core_get_change_serial(self.ptr) }
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
    /// Tamaño del área de video en pantalla (píxeles reales): el núcleo
    /// convierte los cuadros a ese tamaño y no más grandes.
    pub fn set_video_target(&self, width: u32, height: u32) {
        unsafe { capyro_core_set_video_target(self.ptr, width.min(i32::MAX as u32) as c_int, height.min(i32::MAX as u32) as c_int) }
    }

    /// Resolución original y fps del video abierto (None = no es un video).
    pub fn video_props(&self) -> Option<(u32, u32, f64)> {
        let mut p = RawVideoProps { width: 0, height: 0, fps: 0.0 };
        unsafe { capyro_core_get_video_props(self.ptr, &mut p) }
            .then(|| (p.width.max(0) as u32, p.height.max(0) as u32, p.fps))
    }

    /// Segundos hasta el próximo cuadro de video (None = no hay en cola).
    pub fn video_next_due(&self) -> Option<f64> {
        let d = unsafe { capyro_core_video_next_due(self.ptr) };
        (d >= 0.0).then_some(d)
    }

    /// El cuadro visible en su tamaño original (para las capturas).
    pub fn capture_frame(&self) -> Option<(Vec<u8>, u32, u32)> {
        let mut img = RawImage { rgba: std::ptr::null_mut(), width: 0, height: 0 };
        // SAFETY: si devuelve true, `rgba` tiene width*height*4 bytes hasta free_image.
        if !unsafe { capyro_core_capture_frame(self.ptr, &mut img) } || img.rgba.is_null() {
            return None;
        }
        let (w, h) = (img.width.max(0) as usize, img.height.max(0) as usize);
        let data = unsafe { std::slice::from_raw_parts(img.rgba, w * h * 4) }.to_vec();
        unsafe { capyro_core_free_image(&mut img) };
        Some((data, w as u32, h as u32))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_info_tiene_el_tamanio_de_c() {
        // 2 bool + 16 + 24 + 128 chars = 170 → alineado a 8 para el u64
        assert_eq!(std::mem::size_of::<RawDecodeInfo>(), 184);
        assert_eq!(std::mem::align_of::<RawDecodeInfo>(), 8);
    }

    #[test]
    fn video_props_tiene_el_tamanio_de_c() {
        assert_eq!(std::mem::size_of::<RawVideoProps>(), 16);
    }

    #[test]
    fn os_info_tiene_el_tamanio_de_c() {
        // (lo que devuelve el núcleo lo prueba test_core.c: acá no se linkea el C)
        assert_eq!(std::mem::size_of::<RawOsInfo>(), 4 + 4 + 4 + 64 + 64);
    }
}

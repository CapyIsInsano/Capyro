//! Capyro Converter del lado de Rust: qué tiene un archivo, convertir (en el
//! hilo que llama, con el avance y la cancelación compartidos) y qué
//! codificadores hay.

use std::ffi::{c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use super::{capyro_core_error_string, carray_to_string, cstr_to_string, path_to_cstring, put_cstr, CoreError};

#[repr(C)]
struct RawProbe {
    has_video: bool,
    has_audio: bool,
    has_cover: bool,
    duration: f64,
    width: i32,
    height: i32,
    fps: f64,
    video_codec: [c_char; 32],
    audio_codec: [c_char; 32],
    sample_rate: i32,
    channels: i32,
    bits: i32,
    bit_rate: i64,
    audio_copy_ext: [c_char; 8],
}

#[repr(C)]
struct RawOptions {
    video_codec: [c_char; 16],
    audio_codec: [c_char; 16],
    quality: i32,
    max_height: i32,
    fps: i32,
    speed: i32,
    audio_kbps: i32,
    sample_rate: i32,
    channels: i32,
    normalize: bool,
    gpu: bool,
    compat: bool,
    keep_tags: bool,
    start: f64,
    end: f64,
    cover_image: [c_char; 1024],
}

#[repr(C)]
struct RawResult {
    video_encoder: [c_char; 32],
    audio_encoder: [c_char; 32],
    message: [c_char; 256],
}

type ProgressFn = extern "C" fn(*mut c_void, f64) -> c_int;

extern "C" {
    fn capyro_media_probe(path: *const c_char, out: *mut RawProbe) -> c_int;
    fn capyro_convert(
        input: *const c_char,
        output: *const c_char,
        opt: *const RawOptions,
        progress: ProgressFn,
        user: *mut c_void,
        result: *mut RawResult,
    ) -> c_int;
    fn capyro_convert_has_encoder(codec: *const c_char, gpu: bool) -> bool;
}

const ERR_CANCELLED: c_int = 10;

/// Qué tiene un archivo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaProbe {
    pub has_video: bool,
    pub has_audio: bool,
    pub has_cover: bool,
    /// Segundos (0 = no se sabe).
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub video_codec: String,
    pub audio_codec: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub bits: u32,
    pub bit_rate: u64,
    /// Extensión para sacar el audio sin recodificar ("m4a", "mp3", "opus"…).
    pub audio_copy_ext: String,
}

pub fn media_probe(path: &Path) -> Result<MediaProbe, CoreError> {
    let c = path_to_cstring(path).ok_or_else(|| CoreError("ruta inválida".to_owned()))?;
    // SAFETY: todo ceros es un RawProbe válido.
    let mut raw: RawProbe = unsafe { std::mem::zeroed() };
    let code = unsafe { capyro_media_probe(c.as_ptr(), &mut raw) };
    if code != 0 {
        return Err(CoreError(cstr_to_string(unsafe { capyro_core_error_string(code) })));
    }
    Ok(MediaProbe {
        has_video: raw.has_video,
        has_audio: raw.has_audio,
        has_cover: raw.has_cover,
        duration: raw.duration.max(0.0),
        width: raw.width.max(0) as u32,
        height: raw.height.max(0) as u32,
        fps: raw.fps.max(0.0),
        video_codec: carray_to_string(&raw.video_codec),
        audio_codec: carray_to_string(&raw.audio_codec),
        sample_rate: raw.sample_rate.max(0) as u32,
        channels: raw.channels.max(0) as u32,
        bits: raw.bits.max(0) as u32,
        bit_rate: raw.bit_rate.max(0) as u64,
        audio_copy_ext: carray_to_string(&raw.audio_copy_ext),
    })
}

/// Cómo convertir (ver `CapyroConvertOptions` en capyro_core.h).
#[derive(Debug, Clone, PartialEq)]
pub struct ConvertOptions {
    /// "" = sin video, "copy" = tal cual, o "h264", "hevc", "vp9", "av1", "mpeg4".
    pub video_codec: String,
    /// "" = sin audio, "copy", o "aac", "mp3", "opus", "vorbis", "flac", "alac", "pcm".
    pub audio_codec: String,
    /// 0..100 (100 = la mejor).
    pub quality: i32,
    /// Lado corto máximo (720, 1080…); 0 = el original.
    pub max_height: i32,
    /// 0 = los del original.
    pub fps: i32,
    /// 0 rápido, 1 normal, 2 lento (archivo más chico).
    pub speed: i32,
    /// 0 = lo normal del códec.
    pub audio_kbps: i32,
    pub sample_rate: i32,
    pub channels: i32,
    pub normalize: bool,
    pub gpu: bool,
    pub compat: bool,
    pub keep_tags: bool,
    /// Recorte en segundos (`end` <= 0: hasta el final).
    pub start: f64,
    pub end: f64,
    /// Audio → video: la imagen (None = la portada del archivo).
    pub cover_image: Option<PathBuf>,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        ConvertOptions {
            video_codec: String::new(),
            audio_codec: String::new(),
            quality: 70,
            max_height: 0,
            fps: 0,
            speed: 1,
            audio_kbps: 0,
            sample_rate: 0,
            channels: 0,
            normalize: false,
            gpu: false,
            compat: false,
            keep_tags: true,
            start: 0.0,
            end: 0.0,
            cover_image: None,
        }
    }
}

/// Avance y cancelación, compartidos entre la ventana y el hilo que convierte.
#[derive(Debug, Default)]
pub struct ConvertProgress {
    fraction: AtomicU32,
    cancel: AtomicBool,
}

impl ConvertProgress {
    /// 0..1.
    pub fn fraction(&self) -> f32 {
        f32::from_bits(self.fraction.load(Ordering::Relaxed))
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

extern "C" fn on_progress(user: *mut c_void, fraction: f64) -> c_int {
    // SAFETY: `user` es el &ConvertProgress que pasó `convert`, que vive
    // mientras dura la llamada.
    let p = unsafe { &*(user as *const ConvertProgress) };
    p.fraction.store((fraction as f32).clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    c_int::from(p.cancelled())
}

/// Qué se usó para codificar ("libx264", "aac", "copiar"…; vacío si no hay esa pista).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConvertDone {
    pub video_encoder: String,
    pub audio_encoder: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConvertError {
    Cancelled,
    Failed(String),
}

/// Convierte `input` en `output` (bloquea: llamarla desde otro hilo).
pub fn convert(input: &Path, output: &Path, opts: &ConvertOptions, progress: &ConvertProgress) -> Result<ConvertDone, ConvertError> {
    let bad = || ConvertError::Failed("ruta inválida".to_owned());
    let cin = path_to_cstring(input).ok_or_else(bad)?;
    let cout = path_to_cstring(output).ok_or_else(bad)?;
    // SAFETY: todo ceros es un RawOptions válido.
    let mut raw: RawOptions = unsafe { std::mem::zeroed() };
    put_cstr(&mut raw.video_codec, &opts.video_codec);
    put_cstr(&mut raw.audio_codec, &opts.audio_codec);
    raw.quality = opts.quality;
    raw.max_height = opts.max_height;
    raw.fps = opts.fps;
    raw.speed = opts.speed;
    raw.audio_kbps = opts.audio_kbps;
    raw.sample_rate = opts.sample_rate;
    raw.channels = opts.channels;
    raw.normalize = opts.normalize;
    raw.gpu = opts.gpu;
    raw.compat = opts.compat;
    raw.keep_tags = opts.keep_tags;
    raw.start = opts.start;
    raw.end = opts.end;
    if let Some(img) = &opts.cover_image {
        let s = img.to_string_lossy();
        if s.len() >= raw.cover_image.len() {
            return Err(ConvertError::Failed("la ruta de la imagen es muy larga".to_owned()));
        }
        put_cstr(&mut raw.cover_image, &s);
    }
    // SAFETY: todo ceros es un RawResult válido.
    let mut res: RawResult = unsafe { std::mem::zeroed() };
    let user = progress as *const ConvertProgress as *mut c_void;
    let code = unsafe { capyro_convert(cin.as_ptr(), cout.as_ptr(), &raw, on_progress, user, &mut res) };
    match code {
        0 => Ok(ConvertDone {
            video_encoder: carray_to_string(&res.video_encoder),
            audio_encoder: carray_to_string(&res.audio_encoder),
        }),
        ERR_CANCELLED => Err(ConvertError::Cancelled),
        _ => {
            let msg = carray_to_string(&res.message);
            Err(ConvertError::Failed(if msg.is_empty() {
                cstr_to_string(unsafe { capyro_core_error_string(code) })
            } else {
                msg
            }))
        }
    }
}

/// ¿Hay con qué codificar a `codec`? (con `gpu`: de la placa de video).
pub fn has_encoder(codec: &str, gpu: bool) -> bool {
    let Ok(c) = std::ffi::CString::new(codec) else { return false };
    unsafe { capyro_convert_has_encoder(c.as_ptr(), gpu) }
}

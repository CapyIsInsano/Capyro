//! Formatos, códecs y perfiles de Capyro Converter (sin interfaz: se prueba
//! solo). Un perfil arma las opciones según lo que tiene el archivo: "Alta
//! calidad" de un video es un MP4 H.264 casi sin pérdida; de un audio, un
//! MP3 a 320 kbps.

use crate::ffi::{ConvertOptions, MediaProbe};

/// El formato de salida (la extensión del archivo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Container {
    Mp4,
    Mkv,
    Webm,
    Mov,
    Mp3,
    M4a,
    Flac,
    Ogg,
    Opus,
    Wav,
}

impl Container {
    pub(crate) const VIDEO: [Container; 4] = [Container::Mp4, Container::Mkv, Container::Webm, Container::Mov];
    pub(crate) const AUDIO: [Container; 6] =
        [Container::Mp3, Container::M4a, Container::Flac, Container::Ogg, Container::Opus, Container::Wav];

    pub(crate) fn ext(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Mkv => "mkv",
            Container::Webm => "webm",
            Container::Mov => "mov",
            Container::Mp3 => "mp3",
            Container::M4a => "m4a",
            Container::Flac => "flac",
            Container::Ogg => "ogg",
            Container::Opus => "opus",
            Container::Wav => "wav",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Container::Mp4 => "MP4",
            Container::Mkv => "MKV",
            Container::Webm => "WebM",
            Container::Mov => "MOV",
            Container::Mp3 => "MP3",
            Container::M4a => "M4A",
            Container::Flac => "FLAC",
            Container::Ogg => "OGG",
            Container::Opus => "Opus",
            Container::Wav => "WAV",
        }
    }

    pub(crate) fn from_ext(ext: &str) -> Option<Container> {
        let e = ext.to_ascii_lowercase();
        Container::VIDEO.into_iter().chain(Container::AUDIO).find(|c| c.ext() == e)
    }

    pub(crate) fn is_video(self) -> bool {
        Container::VIDEO.contains(&self)
    }

    /// Los códecs de video que entran.
    pub(crate) fn video_codecs(self) -> &'static [VCodec] {
        match self {
            Container::Mp4 => &[VCodec::H264, VCodec::Hevc, VCodec::Av1],
            Container::Mkv => &[VCodec::H264, VCodec::Hevc, VCodec::Vp9, VCodec::Av1],
            Container::Webm => &[VCodec::Vp9, VCodec::Av1],
            Container::Mov => &[VCodec::H264, VCodec::Hevc],
            _ => &[],
        }
    }

    /// Los códecs de audio que entran.
    pub(crate) fn audio_codecs(self) -> &'static [ACodec] {
        match self {
            Container::Mp4 => &[ACodec::Aac, ACodec::Mp3, ACodec::Opus, ACodec::Flac, ACodec::Alac],
            Container::Mkv => &[ACodec::Aac, ACodec::Mp3, ACodec::Opus, ACodec::Vorbis, ACodec::Flac, ACodec::Pcm],
            Container::Webm => &[ACodec::Opus, ACodec::Vorbis],
            Container::Mov => &[ACodec::Aac, ACodec::Alac, ACodec::Pcm],
            Container::Mp3 => &[ACodec::Mp3],
            Container::M4a => &[ACodec::Aac, ACodec::Alac],
            Container::Flac => &[ACodec::Flac],
            Container::Ogg => &[ACodec::Vorbis, ACodec::Opus, ACodec::Flac],
            Container::Opus => &[ACodec::Opus],
            Container::Wav => &[ACodec::Pcm],
        }
    }

    /// ¿Se puede copiar tal cual el códec `name` (como lo dice FFmpeg) acá?
    pub(crate) fn can_copy(self, name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        if self == Container::Mkv {
            return true; // Matroska acepta casi todo
        }
        if let Some(v) = VCodec::from_ffmpeg(name) {
            return self.video_codecs().contains(&v);
        }
        if let Some(a) = ACodec::from_ffmpeg(name) {
            return self.audio_codecs().contains(&a);
        }
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VCodec {
    H264,
    Hevc,
    Vp9,
    Av1,
}

impl VCodec {
    pub(crate) const ALL: [VCodec; 4] = [VCodec::H264, VCodec::Hevc, VCodec::Vp9, VCodec::Av1];

    /// Como lo entiende el núcleo.
    pub(crate) fn id(self) -> &'static str {
        match self {
            VCodec::H264 => "h264",
            VCodec::Hevc => "hevc",
            VCodec::Vp9 => "vp9",
            VCodec::Av1 => "av1",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            VCodec::H264 => "H.264",
            VCodec::Hevc => "H.265",
            VCodec::Vp9 => "VP9",
            VCodec::Av1 => "AV1",
        }
    }

    pub(crate) fn hint(self) -> &'static str {
        match self {
            VCodec::H264 => "Se ve en todos lados (celulares, TVs, navegadores, editores)",
            VCodec::Hevc => "Mismo video en la mitad de espacio; algunos equipos viejos no lo abren",
            VCodec::Vp9 => "El de YouTube y los navegadores; libre",
            VCodec::Av1 => "El más chico de todos, pero lento de codificar y no lo abre todo",
        }
    }

    pub(crate) fn from_ffmpeg(name: &str) -> Option<VCodec> {
        Some(match name {
            "h264" => VCodec::H264,
            "hevc" => VCodec::Hevc,
            "vp9" => VCodec::Vp9,
            "av1" => VCodec::Av1,
            _ => return None,
        })
    }

    /// Cuánto achica respecto de H.264 (para estimar el tamaño).
    fn factor(self) -> f64 {
        match self {
            VCodec::H264 => 1.0,
            VCodec::Hevc => 0.6,
            VCodec::Vp9 => 0.65,
            VCodec::Av1 => 0.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ACodec {
    Aac,
    Mp3,
    Opus,
    Vorbis,
    Flac,
    Alac,
    Pcm,
}

impl ACodec {
    pub(crate) const ALL: [ACodec; 7] =
        [ACodec::Aac, ACodec::Mp3, ACodec::Opus, ACodec::Vorbis, ACodec::Flac, ACodec::Alac, ACodec::Pcm];

    pub(crate) fn id(self) -> &'static str {
        match self {
            ACodec::Aac => "aac",
            ACodec::Mp3 => "mp3",
            ACodec::Opus => "opus",
            ACodec::Vorbis => "vorbis",
            ACodec::Flac => "flac",
            ACodec::Alac => "alac",
            ACodec::Pcm => "pcm",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            ACodec::Aac => "AAC",
            ACodec::Mp3 => "MP3",
            ACodec::Opus => "Opus",
            ACodec::Vorbis => "Vorbis",
            ACodec::Flac => "FLAC",
            ACodec::Alac => "ALAC",
            ACodec::Pcm => "PCM",
        }
    }

    /// Sin pérdida (no tiene kbps).
    pub(crate) fn lossless(self) -> bool {
        matches!(self, ACodec::Flac | ACodec::Alac | ACodec::Pcm)
    }

    pub(crate) fn from_ffmpeg(name: &str) -> Option<ACodec> {
        Some(match name {
            "aac" => ACodec::Aac,
            "mp3" | "mp3float" => ACodec::Mp3,
            "opus" => ACodec::Opus,
            "vorbis" => ACodec::Vorbis,
            "flac" => ACodec::Flac,
            "alac" => ACodec::Alac,
            n if n.starts_with("pcm_") => ACodec::Pcm,
            _ => return None,
        })
    }
}

/// Qué va en cada pista: un códec, copiar tal cual o nada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Track<C> {
    Encode(C),
    Copy,
    None,
}

/// Todo lo que se elige (los perfiles lo llenan; "Avanzado" lo cambia).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Plan {
    pub(crate) container: Container,
    pub(crate) video: Track<VCodec>,
    pub(crate) audio: Track<ACodec>,
    pub(crate) opts: ConvertOptions,
}

impl Plan {
    /// Las opciones para el núcleo (los códecs como texto).
    pub(crate) fn options(&self) -> ConvertOptions {
        let mut o = self.opts.clone();
        o.video_codec = match self.video {
            Track::Encode(c) => c.id().to_owned(),
            Track::Copy => "copy".to_owned(),
            Track::None => String::new(),
        };
        o.audio_codec = match self.audio {
            Track::Encode(c) => c.id().to_owned(),
            Track::Copy => "copy".to_owned(),
            Track::None => String::new(),
        };
        if !self.container.is_video() {
            o.video_codec.clear();
        }
        o
    }

    /// ¿El video sale de la portada (el archivo no tiene video)?
    pub(crate) fn cover_video(&self, probe: &MediaProbe) -> bool {
        self.container.is_video() && !matches!(self.video, Track::None) && !probe.has_video
    }

    /// Acomoda los códecs a lo que entra en el formato (al cambiarlo).
    pub(crate) fn fit_container(&mut self, probe: &MediaProbe, has: &dyn Fn(&str) -> bool) {
        let c = self.container;
        if c.is_video() {
            let ok = match self.video {
                Track::Encode(v) => c.video_codecs().contains(&v),
                Track::Copy => probe.has_video && c.can_copy(&probe.video_codec),
                Track::None => true,
            };
            if !ok || (matches!(self.video, Track::None) && !probe.has_video && probe.has_audio) {
                self.video = c
                    .video_codecs()
                    .iter()
                    .copied()
                    .find(|v| has(v.id()))
                    .map_or(Track::None, Track::Encode);
            }
        } else {
            self.video = Track::None;
        }
        let ok = match self.audio {
            Track::Encode(a) => c.audio_codecs().contains(&a),
            Track::Copy => probe.has_audio && c.can_copy(&probe.audio_codec),
            Track::None => true,
        };
        if !ok {
            self.audio = c
                .audio_codecs()
                .iter()
                .copied()
                .find(|a| has(a.id()))
                .map_or(Track::None, Track::Encode);
        }
    }

    /// Tamaño aproximado del resultado, en bytes (None si no se puede saber).
    pub(crate) fn estimate(&self, probe: &MediaProbe) -> Option<u64> {
        let full = probe.duration;
        if full <= 0.0 {
            return None;
        }
        let end = if self.opts.end > 0.0 { self.opts.end.min(full) } else { full };
        let dur = (end - self.opts.start.max(0.0)).max(0.0);
        let mut bits = 0.0;
        match self.audio {
            Track::Encode(a) if a.lossless() => {
                let rate = if self.opts.sample_rate > 0 { self.opts.sample_rate as f64 } else { probe.sample_rate.max(44_100) as f64 };
                let ch = if self.opts.channels > 0 { self.opts.channels as f64 } else { probe.channels.max(1) as f64 };
                let depth = if probe.bits > 16 { 24.0 } else { 16.0 };
                let ratio = if a == ACodec::Pcm { 1.0 } else { 0.6 };
                bits += rate * ch * depth * ratio * dur;
            }
            Track::Encode(_) => bits += self.audio_kbps() as f64 * 1000.0 * dur,
            Track::Copy => bits += (probe.bit_rate as f64).min(320_000.0) * dur * if probe.has_video { 0.04 } else { 1.0 },
            Track::None => {}
        }
        if self.container.is_video() {
            match self.video {
                Track::Encode(v) => {
                    let (w, h, fps) = if self.cover_video(probe) {
                        let h = if self.opts.max_height > 0 { self.opts.max_height } else { 1080 } as f64;
                        (h * 16.0 / 9.0, h, 5.0)
                    } else {
                        let (w, h) = (probe.width.max(1) as f64, probe.height.max(1) as f64);
                        let short = w.min(h);
                        let k = if self.opts.max_height > 0 { (self.opts.max_height as f64 / short).min(1.0) } else { 1.0 };
                        let fps = if self.opts.fps > 0 { (self.opts.fps as f64).min(probe.fps.max(1.0)) } else { probe.fps.max(24.0) };
                        (w * k, h * k, fps)
                    };
                    let q = self.opts.quality.clamp(0, 100) as f64 / 100.0;
                    // bits por píxel de H.264 a esa calidad (a ojo, como el núcleo)
                    let bpp = if self.cover_video(probe) { 0.01 } else { 0.035 + (0.16 - 0.035) * q * q } * v.factor();
                    bits += w * h * fps * bpp * dur;
                }
                Track::Copy => bits += probe.bit_rate as f64 * dur * 0.96,
                Track::None => {}
            }
        }
        (bits > 0.0).then_some((bits / 8.0) as u64)
    }

    /// Los kbps del audio (los que eligió o los normales del códec).
    pub(crate) fn audio_kbps(&self) -> i32 {
        if self.opts.audio_kbps > 0 {
            return self.opts.audio_kbps;
        }
        match self.audio {
            Track::Encode(ACodec::Opus) => 128,
            _ => 192,
        }
    }
}

/// Los perfiles listos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Profile {
    HighQuality,
    Small,
    Universal,
    Lossless,
    Mobile,
    ExtractAudio,
    CoverVideo,
}

impl Profile {
    pub(crate) const ALL: [Profile; 7] = [
        Profile::HighQuality,
        Profile::Small,
        Profile::Universal,
        Profile::Lossless,
        Profile::Mobile,
        Profile::ExtractAudio,
        Profile::CoverVideo,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Profile::HighQuality => "Alta calidad",
            Profile::Small => "Tamaño reducido",
            Profile::Universal => "Compatibilidad universal",
            Profile::Lossless => "Música sin pérdida",
            Profile::Mobile => "Video para celulares",
            Profile::ExtractAudio => "Extraer audio",
            Profile::CoverVideo => "Crear video con portada",
        }
    }

    /// Qué hace con este archivo (una línea).
    pub(crate) fn blurb(self, probe: &MediaProbe) -> &'static str {
        let video = probe.has_video;
        match self {
            Profile::HighQuality if video => "MP4 · H.264 casi sin pérdida · AAC 256 kbps",
            Profile::HighQuality => "MP3 a 320 kbps",
            Profile::Small if video => "MP4 · H.265 a 720p · AAC 96 kbps",
            Profile::Small => "Opus a 96 kbps (suena igual, pesa poco)",
            Profile::Universal if video => "MP4 · H.264 (High 4.1) hasta 1080p · AAC estéreo",
            Profile::Universal => "MP3 a 192 kbps, estéreo",
            Profile::Lossless => "FLAC: el audio sin perder nada",
            Profile::Mobile if video => "MP4 · H.264 a 720p, 30 fps · AAC 128 kbps",
            Profile::Mobile => "Un MP4 a 720p con la portada, para mandar por chat",
            Profile::ExtractAudio => "El audio tal cual, sin recodificar (rápido y sin pérdida)",
            Profile::CoverVideo => "MP4 a 1080p con la portada de fondo",
        }
    }

    /// Por qué no se puede con este archivo (None = se puede).
    pub(crate) fn unavailable(self, probe: &MediaProbe) -> Option<&'static str> {
        match self {
            _ if !probe.has_audio && !probe.has_video => Some("El archivo no tiene audio ni video"),
            Profile::Lossless | Profile::ExtractAudio if !probe.has_audio => Some("El archivo no tiene audio"),
            Profile::CoverVideo if probe.has_video => Some("Ya es un video"),
            Profile::CoverVideo if !probe.has_audio => Some("El archivo no tiene audio"),
            _ => None,
        }
    }

    /// El plan de este perfil para este archivo. `has` dice si hay
    /// codificador para un códec (H.265 puede faltar).
    pub(crate) fn plan(self, probe: &MediaProbe, has: &dyn Fn(&str) -> bool) -> Plan {
        let video = probe.has_video;
        let base = ConvertOptions { keep_tags: true, ..ConvertOptions::default() };
        let h264_or = |v: VCodec| if has(v.id()) { v } else { VCodec::H264 };
        let mut p = match self {
            Profile::HighQuality if video => Plan {
                container: Container::Mp4,
                video: Track::Encode(VCodec::H264),
                audio: Track::Encode(ACodec::Aac),
                opts: ConvertOptions { quality: 92, speed: 2, audio_kbps: 256, ..base },
            },
            Profile::HighQuality => Plan {
                container: Container::Mp3,
                video: Track::None,
                audio: Track::Encode(ACodec::Mp3),
                opts: ConvertOptions { audio_kbps: 320, ..base },
            },
            Profile::Small if video => Plan {
                container: Container::Mp4,
                video: Track::Encode(h264_or(VCodec::Hevc)),
                audio: Track::Encode(ACodec::Aac),
                opts: ConvertOptions { quality: 35, max_height: 720, speed: 1, audio_kbps: 96, ..base },
            },
            Profile::Small => Plan {
                container: Container::Opus,
                video: Track::None,
                audio: Track::Encode(ACodec::Opus),
                opts: ConvertOptions { audio_kbps: 96, ..base },
            },
            Profile::Universal if video => Plan {
                container: Container::Mp4,
                video: Track::Encode(VCodec::H264),
                audio: Track::Encode(ACodec::Aac),
                opts: ConvertOptions { quality: 70, max_height: 1080, channels: 2, audio_kbps: 192, compat: true, ..base },
            },
            Profile::Universal => Plan {
                container: Container::Mp3,
                video: Track::None,
                audio: Track::Encode(ACodec::Mp3),
                opts: ConvertOptions { audio_kbps: 192, channels: 2, ..base },
            },
            Profile::Lossless => Plan {
                container: Container::Flac,
                video: Track::None,
                audio: Track::Encode(ACodec::Flac),
                opts: ConvertOptions { speed: 2, ..base },
            },
            Profile::Mobile => Plan {
                container: Container::Mp4,
                video: Track::Encode(VCodec::H264),
                audio: Track::Encode(ACodec::Aac),
                opts: ConvertOptions {
                    quality: 58,
                    max_height: 720,
                    fps: 30,
                    channels: 2,
                    audio_kbps: 128,
                    compat: true,
                    ..base
                },
            },
            Profile::ExtractAudio => Plan {
                container: Container::from_ext(&probe.audio_copy_ext).unwrap_or(Container::Mkv),
                video: Track::None,
                audio: Track::Copy,
                opts: base,
            },
            Profile::CoverVideo => Plan {
                container: Container::Mp4,
                video: Track::Encode(VCodec::H264),
                audio: Track::Encode(ACodec::Aac),
                opts: ConvertOptions { quality: 70, max_height: 1080, audio_kbps: 192, compat: true, ..base },
            },
        };
        // "Extraer audio" de un códec que no tiene archivo propio (AC-3, DTS…): MKA → MKV
        if self == Profile::ExtractAudio && !p.container.can_copy(&probe.audio_codec) {
            p.container = Container::Mkv;
        }
        p.fit_container(probe, has);
        p
    }
}

/// Ruta de salida que no pise nada: "nombre.mp4", "nombre (2).mp4"…
pub(crate) fn unique_path(dir: &std::path::Path, stem: &str, ext: &str, exists: &dyn Fn(&std::path::Path) -> bool) -> std::path::PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !exists(&first) {
        return first;
    }
    for n in 2.. {
        let p = dir.join(format!("{stem} ({n}).{ext}"));
        if !exists(&p) {
            return p;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video() -> MediaProbe {
        MediaProbe {
            has_video: true,
            has_audio: true,
            duration: 60.0,
            width: 1920,
            height: 1080,
            fps: 30.0,
            video_codec: "h264".into(),
            audio_codec: "aac".into(),
            sample_rate: 48_000,
            channels: 2,
            bit_rate: 6_000_000,
            audio_copy_ext: "m4a".into(),
            ..MediaProbe::default()
        }
    }

    fn audio() -> MediaProbe {
        MediaProbe {
            has_audio: true,
            has_cover: true,
            duration: 200.0,
            audio_codec: "mp3".into(),
            sample_rate: 44_100,
            channels: 2,
            bit_rate: 320_000,
            audio_copy_ext: "mp3".into(),
            ..MediaProbe::default()
        }
    }

    const ALL: &dyn Fn(&str) -> bool = &|_| true;

    #[test]
    fn perfiles_de_video_y_de_audio() {
        let p = Profile::HighQuality.plan(&video(), ALL);
        assert_eq!((p.container, p.video, p.audio), (Container::Mp4, Track::Encode(VCodec::H264), Track::Encode(ACodec::Aac)));
        let p = Profile::HighQuality.plan(&audio(), ALL);
        assert_eq!((p.container, p.audio, p.opts.audio_kbps), (Container::Mp3, Track::Encode(ACodec::Mp3), 320));
        // tamaño reducido: H.265 si hay; si no, H.264
        assert_eq!(Profile::Small.plan(&video(), ALL).video, Track::Encode(VCodec::Hevc));
        let sin_hevc = |c: &str| c != "hevc";
        assert_eq!(Profile::Small.plan(&video(), &sin_hevc).video, Track::Encode(VCodec::H264));
        // extraer: el audio tal cual, en su archivo
        let p = Profile::ExtractAudio.plan(&video(), ALL);
        assert_eq!((p.container, p.audio, p.video), (Container::M4a, Track::Copy, Track::None));
        assert_eq!(p.options().video_codec, "");
        assert_eq!(p.options().audio_codec, "copy");
        // portada: de un audio sale un MP4 con video
        let p = Profile::CoverVideo.plan(&audio(), ALL);
        assert!(p.cover_video(&audio()));
        assert_eq!(p.options().video_codec, "h264");
        assert!(Profile::CoverVideo.unavailable(&video()).is_some());
        assert!(Profile::Lossless.unavailable(&MediaProbe { has_video: true, ..MediaProbe::default() }).is_some());
    }

    #[test]
    fn el_formato_acomoda_los_codecs() {
        let mut p = Profile::HighQuality.plan(&video(), ALL);
        p.container = Container::Webm;
        p.fit_container(&video(), ALL);
        assert_eq!((p.video, p.audio), (Track::Encode(VCodec::Vp9), Track::Encode(ACodec::Opus)));
        p.container = Container::Flac;
        p.fit_container(&video(), ALL);
        assert_eq!((p.video, p.audio), (Track::None, Track::Encode(ACodec::Flac)));
        // copiar: solo si el códec entra
        assert!(Container::Mp4.can_copy("aac"));
        assert!(!Container::Mp3.can_copy("aac"));
        assert!(!Container::Webm.can_copy("h264"));
        assert!(Container::Mkv.can_copy("ac3"));
        // un audio a MP4: el video sale de la portada
        let mut p = Profile::HighQuality.plan(&audio(), ALL);
        p.container = Container::Mp4;
        p.fit_container(&audio(), ALL);
        assert_eq!(p.video, Track::Encode(VCodec::H264));
    }

    #[test]
    fn estimacion_razonable() {
        // MP3 320 de 200 s: unos 8 MB
        let p = Profile::HighQuality.plan(&audio(), ALL);
        let mb = p.estimate(&audio()).unwrap() as f64 / 1e6;
        assert!((7.5..8.5).contains(&mb), "{mb}");
        // recortar achica
        let mut q = p.clone();
        q.opts.end = 100.0;
        assert!(q.estimate(&audio()).unwrap() < p.estimate(&audio()).unwrap());
        // un video más chico pesa menos
        let big = Profile::HighQuality.plan(&video(), ALL).estimate(&video()).unwrap();
        let small = Profile::Small.plan(&video(), ALL).estimate(&video()).unwrap();
        assert!(small * 4 < big, "{small} {big}");
    }

    #[test]
    fn nombres_que_no_pisan() {
        let dir = std::path::Path::new("/x");
        let taken = |p: &std::path::Path| p.ends_with("tema.mp3") || p.ends_with("tema (2).mp3");
        assert_eq!(unique_path(dir, "tema", "mp3", &taken), dir.join("tema (3).mp3"));
        assert_eq!(unique_path(dir, "otro", "mp3", &taken), dir.join("otro.mp3"));
    }
}

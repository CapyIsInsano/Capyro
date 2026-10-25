//! Resúmenes de los formatos que acepta Capyro (para el menú ⓘ).
//!
//! La lista de formatos sale del núcleo en C; acá solo está el texto que se
//! muestra al tocar cada uno. Si el núcleo suma un formato que no está acá,
//! se muestra igual con un resumen genérico.

pub struct FormatInfo {
    /// Nombre completo del formato.
    pub name: &'static str,
    /// Resumen de dos o tres líneas.
    pub summary: &'static str,
}

pub fn info(ext: &str) -> FormatInfo {
    match ext {
        "mp4" => FormatInfo {
            name: "MPEG-4 Parte 14",
            summary: "El contenedor de video más usado (2001). Normalmente trae video H.264 o \
                      H.265 con audio AAC. Ideal para compartir y para streaming.",
        },
        "mov" => FormatInfo {
            name: "QuickTime File Format",
            summary: "Formato de Apple (1991) y la base sobre la que se hizo el MP4. Es el que \
                      graban los iPhone y muchas cámaras: H.264, HEVC o ProRes.",
        },
        "avi" => FormatInfo {
            name: "Audio Video Interleave",
            summary: "Clásico de Microsoft (1992). Muy compatible, pero viejo: no está pensado \
                      para streaming. Suele traer video Xvid/DivX con audio MP3.",
        },
        "mp3" => FormatInfo {
            name: "MPEG-1 Audio Layer III",
            summary: "El formato de música más universal (1993). Comprime con pérdida para \
                      ocupar poco y puede traer carátula y datos del tema (ID3).",
        },
        "wav" => FormatInfo {
            name: "Waveform Audio File Format",
            summary: "Audio sin comprimir (PCM) de Microsoft e IBM (1991). Máxima calidad, \
                      pero archivos grandes: ~10 MB por minuto en calidad de CD.",
        },
        "ogg" => FormatInfo {
            name: "Ogg (Xiph.org)",
            summary: "Contenedor libre y sin patentes (2000). Casi siempre trae audio Vorbis u \
                      Opus; suena mejor que un MP3 del mismo tamaño.",
        },
        "aac" => FormatInfo {
            name: "Advanced Audio Coding",
            summary: "El sucesor del MP3 (1997): mejor calidad con el mismo tamaño. Es el audio \
                      de YouTube, Apple Music y de la mayoría de los MP4.",
        },
        _ => FormatInfo {
            name: "Formato soportado",
            summary: "Capyro lo abre con FFmpeg.",
        },
    }
}

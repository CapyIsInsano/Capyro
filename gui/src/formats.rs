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
        "mkv" => FormatInfo {
            name: "Matroska",
            summary: "Contenedor libre y abierto (2002) que acepta casi cualquier cosa: video \
                      H.264, HEVC, VP9 o AV1, varios audios y subtítulos en un solo archivo.",
        },
        "webm" => FormatInfo {
            name: "WebM",
            summary: "El formato de video de la web (Google, 2010), hecho sobre Matroska: video \
                      VP8, VP9 o AV1 con audio Vorbis u Opus. Libre de patentes.",
        },
        "flac" => FormatInfo {
            name: "Free Lossless Audio Codec",
            summary: "Audio sin pérdida (2001): suena idéntico al original y ocupa la mitad que \
                      un WAV. Ideal para guardar música en calidad de CD o mejor.",
        },
        "opus" => FormatInfo {
            name: "Opus (en Ogg)",
            summary: "El códec de audio más moderno (2012), libre. Suena muy bien con archivos \
                      chicos y es el que usan WhatsApp, Discord y YouTube.",
        },
        "m4a" => FormatInfo {
            name: "MPEG-4 Audio",
            summary: "Un MP4 con solo audio: AAC (con pérdida) o ALAC (sin pérdida). Es el \
                      formato de iTunes y de los audios grabados en el celular.",
        },
        "aiff" => FormatInfo {
            name: "Audio Interchange File Format",
            summary: "El WAV de Apple (1988): audio sin comprimir con calidad de estudio. \
                      También se ve como .aif o .aifc.",
        },
        _ => FormatInfo {
            name: "Formato soportado",
            summary: "Capyro lo abre con FFmpeg.",
        },
    }
}

/// Otras extensiones del mismo formato (el núcleo también las acepta, pero
/// en la lista de formatos aparece una sola vez).
pub fn aliases(ext: &str) -> &'static [&'static str] {
    match ext {
        "aiff" => &["aif", "aifc"],
        _ => &[],
    }
}

/// ¿Capyro abre este archivo? (por la extensión, como el núcleo)
pub fn is_supported(path: &std::path::Path, formats: &(Vec<String>, Vec<String>)) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else { return false };
    let ext = ext.to_ascii_lowercase();
    formats
        .0
        .iter()
        .chain(formats.1.iter())
        .any(|f| *f == ext || aliases(f).contains(&ext.as_str()))
}

/// Los archivos compatibles de una carpeta y sus subcarpetas, en orden
/// alfabético (sin distinguir mayúsculas), sin los ocultos. Con tope de
/// profundidad y de cantidad, por si alguien suelta la carpeta personal.
pub fn files_in_dir(dir: &std::path::Path, formats: &(Vec<String>, Vec<String>)) -> Vec<std::path::PathBuf> {
    const MAX_FILES: usize = 5000;
    const MAX_DEPTH: usize = 6;
    fn walk(
        dir: &std::path::Path,
        depth: usize,
        formats: &(Vec<String>, Vec<String>),
        out: &mut Vec<std::path::PathBuf>,
    ) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        entries.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default());
        // primero los archivos de esta carpeta, después las subcarpetas
        let (dirs, files): (Vec<_>, Vec<_>) = entries.into_iter().partition(|p| p.is_dir());
        for f in files {
            if out.len() >= MAX_FILES {
                return;
            }
            let hidden = f.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
            if !hidden && is_supported(&f, formats) {
                out.push(f);
            }
        }
        if depth < MAX_DEPTH {
            for d in dirs {
                if !d.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')) {
                    walk(&d, depth + 1, formats, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, 0, formats, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todos_tienen_resumen() {
        for ext in ["mp4", "mov", "avi", "mkv", "webm", "mp3", "wav", "ogg", "aac", "flac", "opus", "m4a", "aiff"] {
            let i = info(ext);
            assert_ne!(i.name, "Formato soportado", "{ext} sin resumen");
            assert!(i.summary.len() > 60 && !i.summary.contains("  "), "{ext}");
        }
        assert_eq!(aliases("aiff"), &["aif", "aifc"]);
        assert!(aliases("mp3").is_empty());
    }

    #[test]
    fn carpetas() {
        let formats = (vec!["mp4".to_owned(), "mkv".to_owned()], vec!["mp3".to_owned(), "aiff".to_owned()]);
        let dir = std::env::temp_dir().join(format!("capyro-carpeta-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Disco 2")).unwrap();
        std::fs::create_dir_all(dir.join(".oculta")).unwrap();
        for f in ["b.MP3", "a.aif", "notas.txt", ".escondido.mp3", "Disco 2/c.mkv", ".oculta/d.mp4"] {
            std::fs::write(dir.join(f), b"").unwrap();
        }
        let got: Vec<String> = files_in_dir(&dir, &formats)
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(got, ["a.aif", "b.MP3", "Disco 2/c.mkv"]);
        assert!(is_supported(std::path::Path::new("x.AIFC"), &formats));
        assert!(!is_supported(std::path::Path::new("x.wma"), &formats));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

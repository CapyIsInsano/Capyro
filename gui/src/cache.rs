//! "Borrar caché" (menú ⓘ): todo lo que Capyro guarda para no volver a
//! hacerlo y se puede tirar sin perder nada del usuario.
//!
//! - la carpeta de caché: el código y la compilación del actualizador (lo
//!   más pesado), la canción del tutorial, el registro de la actualización;
//! - el registro de errores (`capyro.log`, al lado de la caché);
//! - lo que dejan en la carpeta temporal las pruebas y las actualizaciones
//!   (`capyro-upd-test-*`, `capyro-carpeta-*`, `capyro-swap-*`, `capyro/`);
//! - en Windows, el recuerdo de con qué se dibujó la última vez (`renderer.txt`,
//!   se vuelve a elegir solo).
//!
//! No se toca: la configuración, las playlists, la sesión, las capturas ni
//! la marca del tutorial.

use std::path::{Path, PathBuf};

/// Lo que dejan las pruebas y las actualizaciones en la carpeta temporal.
const TEMP_PREFIXES: [&str; 3] = ["capyro-upd-test-", "capyro-carpeta-", "capyro-swap-"];

/// Cómo salió la limpieza.
#[derive(Debug, Default, Clone)]
pub struct Cleaned {
    /// Bytes liberados.
    pub freed: u64,
    /// Lo que no se pudo borrar (ruta: motivo).
    pub failed: Vec<String>,
}

/// Todo lo que se borraría (solo lo que existe).
pub fn targets() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let cache = crate::paths::cache_dir();
    out.push(cache.with_file_name("capyro.log"));
    out.push(cache);
    let tmp = std::env::temp_dir();
    out.push(tmp.join("capyro"));
    // las de este mismo proceso (si hay) siguen en uso: esas no
    let mine = format!("-{}", std::process::id());
    if let Ok(entries) = std::fs::read_dir(&tmp) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if TEMP_PREFIXES.iter().any(|p| name.starts_with(p)) && !name.ends_with(&mine) {
                out.push(e.path());
            }
        }
    }
    #[cfg(windows)]
    if let Some(state) = crate::paths::state_dir() {
        out.push(state.join("renderer.txt"));
    }
    out.sort();
    out.dedup();
    out.retain(|p| p.symlink_metadata().is_ok());
    out
}

/// Cuánto ocupa una ruta (archivo o carpeta, sin seguir enlaces).
fn size_of(p: &Path) -> u64 {
    let Ok(meta) = p.symlink_metadata() else { return 0 };
    if !meta.is_dir() {
        return meta.len();
    }
    std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| size_of(&e.path())).sum()).unwrap_or(0)
}

/// Cuánto ocupa todo lo que se borraría.
pub fn size() -> u64 {
    targets().iter().map(|p| size_of(p)).sum()
}

/// Borra todo. Lo que no se puede (un archivo abierto en Windows, permisos)
/// queda anotado y se sigue con lo demás.
pub fn clear() -> Cleaned {
    let mut done = Cleaned::default();
    for p in targets() {
        let bytes = size_of(&p);
        let is_dir = p.symlink_metadata().is_ok_and(|m| m.is_dir());
        let res = if is_dir { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
        match res {
            Ok(()) => done.freed += bytes,
            Err(e) => {
                // lo que sí se pudo borrar adentro también cuenta
                done.freed += bytes.saturating_sub(size_of(&p));
                done.failed.push(format!("{}: {e}", crate::paths::tilde(&p)));
            }
        }
    }
    done
}

/// "412 MB", "3,1 GB", "860 KB".
pub fn human(bytes: u64) -> String {
    const K: f64 = 1024.0;
    let b = bytes as f64;
    if b >= K * K * K {
        format!("{:.1} GB", b / (K * K * K)).replace('.', ",")
    } else if b >= K * K {
        format!("{:.0} MB", b / (K * K))
    } else if b >= K {
        format!("{:.0} KB", b / K)
    } else {
        format!("{bytes} bytes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tamanos() {
        assert_eq!(human(0), "0 bytes");
        assert_eq!(human(2048), "2 KB");
        assert_eq!(human(412 * 1024 * 1024), "412 MB");
        assert_eq!(human(3 * 1024 * 1024 * 1024 + 200 * 1024 * 1024), "3,2 GB");
    }

    #[test]
    fn borra_una_carpeta_de_prueba() {
        // una carpeta como las que dejan las pruebas (de "otro" proceso)
        let dir = std::env::temp_dir().join("capyro-carpeta-999999999");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub").join("a.bin"), vec![0u8; 5000]).unwrap();
        assert!(targets().contains(&dir));
        assert!(size_of(&dir) >= 5000);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(!targets().contains(&dir));
    }
}

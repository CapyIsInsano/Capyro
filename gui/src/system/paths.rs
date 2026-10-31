//! Dónde guarda Capyro sus cosas, según el sistema.
//!
//! | qué            | Linux                               | Windows                                   |
//! |----------------|-------------------------------------|-------------------------------------------|
//! | config         | `~/.config/capyro/capyro.lua`       | `Documentos\Capyro\Config\Capyro.lua` (*) |
//! | listas         | `~/.local/share/capyro/`            | `Documentos\Capyro\`                      |
//! | sesión, marcas | `~/.local/state/capyro/`            | `%LOCALAPPDATA%\Capyro\state\`            |
//! | caché          | `~/.cache/capyro/`                  | `%LOCALAPPDATA%\Capyro\cache\`            |
//! | capturas       | Imágenes (xdg-user-dirs)`/Capyro`   | Imágenes`\Capyro`                         |
//!
//! (*) o la que se eligió en el instalador: queda anotada en
//! `capyro-config.txt`, al lado de `capyro.exe`.
//!
//! Las variables `XDG_*` se respetan en Linux; `CAPYRO_CONFIG` en los dos.

use std::path::{Path, PathBuf};

/// La carpeta personal del usuario.
pub fn home() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        dirs::home_dir()
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from)
    }
}

/// `$XDG_<VAR>` si es absoluta; si no `~/<fallback>`.
#[cfg(not(windows))]
fn xdg(var: &str, fallback: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home().map(|h| h.join(fallback)))
}

/// `%LOCALAPPDATA%\Capyro`.
#[cfg(windows)]
fn local_app_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("Capyro"))
}

/// Archivo junto al ejecutable donde el instalador de Windows anota la
/// ruta de la config que eligió el usuario.
#[cfg(windows)]
pub const CONFIG_POINTER: &str = "capyro-config.txt";

/// La ruta de la config que dejó anotada el instalador (Windows).
#[cfg(windows)]
pub fn installer_config_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let txt = std::fs::read_to_string(exe.parent()?.join(CONFIG_POINTER)).ok()?;
    let line = txt.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#'))?;
    Some(PathBuf::from(line))
}

/// Donde se guarda (y se busca primero) la config del usuario.
pub fn user_config() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CAPYRO_CONFIG").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    #[cfg(windows)]
    {
        installer_config_path().or_else(|| dirs::document_dir().map(|d| d.join("Capyro").join("Config").join("Capyro.lua")))
    }
    #[cfg(not(windows))]
    {
        xdg("XDG_CONFIG_HOME", ".config").map(|b| b.join("capyro").join("capyro.lua"))
    }
}

/// Listas de reproducción guardadas.
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        dirs::document_dir().map(|d| d.join("Capyro"))
    }
    #[cfg(not(windows))]
    {
        xdg("XDG_DATA_HOME", ".local/share").map(|b| b.join("capyro"))
    }
}

/// Sesión, marca del tutorial y otras cosas que no son del usuario.
pub fn state_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        local_app_dir().map(|d| d.join("state"))
    }
    #[cfg(not(windows))]
    {
        xdg("XDG_STATE_HOME", ".local/state").map(|b| b.join("capyro"))
    }
}

/// Caché: código del actualizador, la canción del tutorial...
pub fn cache_dir() -> PathBuf {
    #[cfg(windows)]
    let base = local_app_dir().map(|d| d.join("cache"));
    #[cfg(not(windows))]
    let base = xdg("XDG_CACHE_HOME", ".cache").map(|b| b.join("capyro"));
    base.unwrap_or_else(|| std::env::temp_dir().join("capyro"))
}

/// Carpeta de Imágenes del usuario + "Capyro".
pub fn default_screenshot_dir() -> PathBuf {
    #[cfg(windows)]
    {
        dirs::picture_dir()
            .or_else(|| home().map(|h| h.join("Pictures")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Capyro")
    }
    #[cfg(not(windows))]
    {
        // según xdg-user-dirs (XDG_PICTURES_DIR en ~/.config/user-dirs.dirs)
        let home = home().unwrap_or_else(|| PathBuf::from("."));
        let config = xdg("XDG_CONFIG_HOME", ".config").unwrap_or_else(|| home.join(".config"));
        let pictures = std::fs::read_to_string(config.join("user-dirs.dirs"))
            .ok()
            .and_then(|txt| {
                txt.lines().find_map(|l| {
                    let v = l.trim().strip_prefix("XDG_PICTURES_DIR=")?;
                    let v = v.trim_matches('"').replace("$HOME", &home.to_string_lossy());
                    Some(PathBuf::from(v))
                })
            })
            .unwrap_or_else(|| home.join("Pictures"));
        pictures.join("Capyro")
    }
}

/// `~/algo` → `/home/vos/algo` (y `~\algo` en Windows).
pub fn expand_tilde(v: &str) -> PathBuf {
    let rest = v.strip_prefix("~/").or_else(|| if cfg!(windows) { v.strip_prefix("~\\") } else { None });
    match (rest, home()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ if v == "~" => home().unwrap_or_else(|| PathBuf::from(v)),
        _ => PathBuf::from(v),
    }
}

/// Para mostrar: la carpeta personal como `~`.
pub fn tilde(path: &Path) -> String {
    let s = path.to_string_lossy().into_owned();
    let Some(home) = home() else { return s };
    let h = home.to_string_lossy();
    match s.strip_prefix(&*h) {
        Some(rest) if !h.is_empty() && (rest.starts_with('/') || rest.starts_with('\\')) => format!("~{rest}"),
        _ => s,
    }
}

/// Abre una URL (o carpeta) con el programa por defecto del sistema.
pub fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    let r = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // explorer.exe abre URLs con el navegador por defecto y carpetas con el Explorador
        std::process::Command::new("explorer.exe").arg(url).creation_flags(CREATE_NO_WINDOW).spawn()
    };
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    r.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_ida_y_vuelta() {
        let Some(h) = home() else { return };
        let p = h.join("Música").join("a.mp3");
        let shown = tilde(&p);
        assert!(shown.starts_with('~'), "{shown}");
        let back = expand_tilde(&shown.replace('\\', "/"));
        assert_eq!(back, p);
    }

    #[test]
    fn rutas_del_sistema() {
        let cfg = user_config().unwrap();
        if cfg!(windows) {
            assert!(cfg.ends_with("Capyro.lua") || std::env::var_os("CAPYRO_CONFIG").is_some());
        } else if std::env::var_os("CAPYRO_CONFIG").is_none() {
            assert!(cfg.ends_with("capyro/capyro.lua"));
        }
        assert!(cache_dir().to_string_lossy().to_lowercase().contains("capyro"));
    }
}

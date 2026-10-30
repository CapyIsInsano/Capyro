//! "Desinstalar Capyro" (menú ⓘ). Cada sistema tiene el suyo:
//! - Windows: `uninstall.exe`, al lado de capyro.exe (el mismo de
//!   "Aplicaciones instaladas"). Se abre aparte y Capyro se cierra.
//! - Linux: `capyro-uninstall`, que CMake deja junto al binario. Si Capyro
//!   está en tu carpeta (~/.local) se corre acá mismo. Si está en /usr o
//!   /usr/local hace falta permiso de administrador: no lo pedimos en silencio
//!   (sin un agente de contraseña eso no hace nada); se copia el comando para
//!   pegarlo en una terminal.

use std::path::{Path, PathBuf};

/// El desinstalador que está al lado del ejecutable, si está.
pub fn uninstaller() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let p = dir.join(if cfg!(windows) { "uninstall.exe" } else { "capyro-uninstall" });
    p.is_file().then_some(p)
}

/// Qué pasó al pedir la desinstalación.
pub enum Outcome {
    /// Ya corrió (o se abrió el desinstalador de Windows): Capyro se cierra.
    Done,
    /// Hace falta permiso: este es el comando que hay que correr en una terminal.
    NeedsRoot(String),
}

/// Lo hace. Error = texto para mostrar al usuario.
pub fn launch() -> Result<Outcome, String> {
    let Some(prog) = uninstaller() else {
        return Err(if cfg!(windows) {
            "No encontré uninstall.exe al lado de capyro.exe".to_owned()
        } else {
            "Esta copia no trae desinstalador: sacala con tu gestor de paquetes".to_owned()
        });
    };
    run(&prog)
}

#[cfg(windows)]
fn run(prog: &Path) -> Result<Outcome, String> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    std::process::Command::new(prog)
        .arg("--uninstall")
        .current_dir(prog.parent().unwrap_or(Path::new(".")))
        .stdin(std::process::Stdio::null())
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .map(|_| Outcome::Done)
        .map_err(|e| format!("No pude abrir el desinstalador: {e}"))
}

#[cfg(not(windows))]
fn run(prog: &Path) -> Result<Outcome, String> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if !home.is_some_and(|h| prog.starts_with(&h)) {
        return Ok(Outcome::NeedsRoot(format!("sudo sh {}", prog.display())));
    }
    // en tu carpeta: corre ahora y se espera el resultado (es rápido)
    let out = std::process::Command::new("sh")
        .arg(prog)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("No pude correr el desinstalador: {e}"))?;
    if out.status.success() {
        Ok(Outcome::Done)
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("No se pudo desinstalar: {}", err.trim().lines().next().unwrap_or("error")))
    }
}

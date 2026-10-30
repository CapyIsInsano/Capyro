//! "Desinstalar Capyro" (menú ⓘ). Cada sistema tiene el suyo:
//! - Windows: `uninstall.exe`, al lado de capyro.exe. Es el mismo que
//!   aparece en "Aplicaciones instaladas" (el instalador en modo desinstalar).
//! - Linux: `capyro-uninstall`, que CMake deja junto al binario.
//!
//! No bloquea la ventana: el desinstalador corre aparte y Capyro se cierra.

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// El desinstalador que está al lado del ejecutable, si está.
pub fn uninstaller() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let p = dir.join(if cfg!(windows) { "uninstall.exe" } else { "capyro-uninstall" });
    p.is_file().then_some(p)
}

/// Abre el desinstalador. Error = texto para mostrar al usuario.
pub fn launch() -> Result<(), String> {
    let Some(prog) = uninstaller() else {
        return Err(if cfg!(windows) {
            "No encontré uninstall.exe al lado de capyro.exe".to_owned()
        } else {
            "Esta copia no trae desinstalador: sacala con tu gestor de paquetes".to_owned()
        });
    };
    spawn(&prog)
}

#[cfg(windows)]
fn spawn(prog: &std::path::Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    Command::new(prog)
        .arg("--uninstall")
        .current_dir(prog.parent().unwrap_or(std::path::Path::new(".")))
        .stdin(Stdio::null())
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("No pude abrir el desinstalador: {e}"))
}

#[cfg(not(windows))]
fn spawn(prog: &std::path::Path) -> Result<(), String> {
    // En tu carpeta (~/.local, por ejemplo) alcanza con sh. Si está en
    // /usr o /usr/local hace falta permiso de administrador: con pkexec
    // sale la ventana de contraseña del sistema; sin pkexec, se dice el comando.
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mine = home.is_some_and(|h| prog.starts_with(&h));
    let sudo = if mine { None } else { Some(which("pkexec")) };
    let cmd_text = format!("sudo sh {}", prog.display());
    let mut cmd = match sudo {
        None => {
            let mut c = Command::new("sh");
            c.arg(prog);
            c
        }
        Some(true) => {
            let mut c = Command::new("pkexec");
            c.arg("sh").arg(prog);
            c
        }
        Some(false) => return Err(format!("Hace falta permiso: corré {cmd_text}")),
    };
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().map(|_| ()).map_err(|e| format!("No pude abrir el desinstalador: {e}"))
}

#[cfg(not(windows))]
fn which(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

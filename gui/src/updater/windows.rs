//! Actualizador en Windows: baja la versión para Windows publicada en el
//! repo y reemplaza los archivos instalados. No compila nada ni instala nada
//! que sea de Linux.
//!
//! Usa `curl.exe` y `tar.exe`, que vienen con Windows 10 (1803 o más nuevo)
//! y Windows 11. Un .exe o una .dll que están en uso no se pueden pisar pero
//! sí renombrar: los viejos quedan como `*.old` y se borran la próxima vez
//! que abre Capyro ([`cleanup_after_update`]).

use std::ffi::OsString;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::release::{self, Release, WINDOWS_ASSET, WINDOWS_TAG};
use super::*;
use crate::ffi::BuildInfo;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;

/// Clave del registro que escribe CapyroInstaller (Agregar o quitar programas).
const UNINSTALL_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Capyro";

pub(super) fn try_run(w: &mut Worker, job: &Job, cache: &Path) -> Result<Outcome, Fail> {
    w.try_run_windows(job, cache)
}

/// Instalado = la carpeta del .exe tiene la config instalada al lado (la
/// deja `cmake --install` y también CapyroInstaller).
pub(super) fn install_location(_info: &BuildInfo) -> Option<Install> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let installed = dir.join("config").join("capyro.lua").is_file() || dir.join(crate::paths::CONFIG_POINTER).is_file();
    installed.then(|| Install { prefix: dir.to_path_buf(), bindir: dir.to_path_buf() })
}

/// Borra los `*.old` que dejó la actualización (ya nadie los usa).
pub(super) fn cleanup_after_update() {
    let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) else { return };
    std::thread::spawn(move || {
        for f in files_under(&dir, 3) {
            if f.extension().is_some_and(|e| e == "old") {
                let _ = std::fs::remove_file(&f);
            }
        }
        let _ = std::fs::remove_dir_all(crate::paths::cache_dir().join("update"));
    });
}

pub(super) fn relaunch(bindir: &Path) -> std::io::Result<()> {
    Command::new(bindir.join("capyro.exe"))
        .env("CAPYRO_UPDATED", "1") // el nuevo muestra "Capyro actualizado a …"
        .current_dir(bindir)
        .stdin(Stdio::null())
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .map(|_| ())
}

/// Sin ventanas de consola que aparecen y desaparecen.
pub(super) fn prepare(cmd: &mut Command, nice: bool) {
    let mut flags = CREATE_NO_WINDOW;
    if nice {
        flags |= BELOW_NORMAL_PRIORITY_CLASS;
    }
    cmd.creation_flags(flags);
}

pub(super) fn kill_tree(pid: u32) {
    let _ = Command::new(system_tool("taskkill.exe"))
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

pub(super) fn child_path() -> OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

/// `C:\Windows\System32\<name>` si existe; si no, el del PATH.
fn system_tool(name: &str) -> PathBuf {
    let sys = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("windir"))
        .map(|r| PathBuf::from(r).join("System32").join(name));
    match sys {
        Some(p) if p.is_file() => p,
        _ => std::env::split_paths(&child_path())
            .map(|d| d.join(name))
            .find(|p| p.is_file())
            .unwrap_or_else(|| PathBuf::from(name)),
    }
}

fn have_tool(name: &str) -> bool {
    system_tool(name).is_file()
}

/// Todos los archivos debajo de `root` (hasta `depth` carpetas).
fn files_under(root: &Path, depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(root) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() && depth > 0 => out.extend(files_under(&p, depth - 1)),
            Ok(t) if t.is_file() => out.push(p),
            _ => {}
        }
    }
    out
}

/// Errores de curl (código de salida) → sin conexión / repo / otro.
fn classify_curl(code: Option<i32>, out: &str) -> FailKind {
    match code {
        // 5/6: no resuelve, 7: no conecta, 28: tiempo, 35: TLS, 52/56: se cortó
        Some(5 | 6 | 7 | 28 | 35 | 52 | 56) => FailKind::Offline,
        Some(22) if out.contains("404") => FailKind::Repo,
        _ => classify_git(out),
    }
}

impl Worker {
    fn curl_get(&mut self, url: &str) -> Result<String, (Option<i32>, String)> {
        let mut cmd = Command::new(system_tool("curl.exe"));
        cmd.args([
            "-sS",
            "-L",
            "--fail",
            "--connect-timeout",
            "15",
            "--max-time",
            "60",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "X-GitHub-Api-Version: 2022-11-28",
            "-A",
            "Capyro-updater",
            url,
        ]);
        self.exec(cmd, false, |_, _| {})
    }

    fn api_fail(code: Option<i32>, out: &str, not_found: &str) -> Fail {
        match classify_curl(code, out) {
            FailKind::Offline => (FailKind::Offline, "Sin conexión: no pude llegar a GitHub".into()),
            FailKind::Repo => (FailKind::Repo, not_found.to_owned()),
            _ if out.contains("403") || out.contains("429") => {
                (FailKind::Other, "GitHub dice que esperes un rato (límite de consultas)".into())
            }
            _ => (FailKind::Other, format!("GitHub falló: {}", last_meaningful(out))),
        }
    }

    fn try_run_windows(&mut self, job: &Job, cache: &Path) -> Result<Outcome, Fail> {
        // Un solo Capyro actualizándose a la vez: el lock se abre sin compartir.
        let _lock = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(cache.join("update.lock"))
            .map_err(|_| (FailKind::Other, "Otra ventana de Capyro ya se está actualizando".to_owned()))?;

        if !have_tool("curl.exe") || !have_tool("tar.exe") {
            return Err((
                FailKind::MissingTool,
                "Para actualizarse Capyro necesita curl y tar (vienen con Windows 10 1803 o más nuevo)".into(),
            ));
        }
        let Some(slug) = github_slug(&job.repo) else {
            return Err((
                FailKind::Repo,
                "En Windows las actualizaciones salen de GitHub: poné update_repo = \"usuario/Capyro\"".into(),
            ));
        };

        /* 1. ¿qué hay publicado para Windows? */
        self.set_stage(Stage::Checking);
        let url = format!("https://api.github.com/repos/{slug}/releases/tags/{WINDOWS_TAG}");
        let json = self
            .curl_get(&url)
            .map_err(|(c, o)| Self::api_fail(c, &o, "El repo todavía no tiene una versión para Windows publicada"))?;
        let rel: Release = release::parse_release(&json).map_err(|e| (FailKind::Repo, e))?;
        self.logln(&format!(
            "publicado: {} · commit {} · versión {}",
            rel.name,
            rel.commit.as_deref().unwrap_or("?"),
            rel.version.as_deref().unwrap_or("?")
        ));
        let built = job.built_commit.to_ascii_lowercase();
        if rel.commit.as_deref() == Some(built.as_str()) {
            return Ok(Outcome::UpToDate { hash: short(&built) });
        }
        self.check_cancel()?;

        // qué cambió desde el instalado ("el parche")
        let fallback = |rel: &Release| CommitInfo {
            hash: rel.commit.as_deref().map(short).unwrap_or_else(|| rel.version.clone().unwrap_or_default()),
            subject: if rel.name.is_empty() { "Versión para Windows".into() } else { rel.name.clone() },
            time: rel.published,
        };
        let (commits, total) = match (&rel.commit, built.is_empty()) {
            (Some(new), false) => {
                let url = format!("https://api.github.com/repos/{slug}/compare/{built}...{new}");
                match self.curl_get(&url).map(|j| release::parse_compare(&j)) {
                    Ok(Ok(c)) if c.status == "identical" => return Ok(Outcome::UpToDate { hash: short(new) }),
                    Ok(Ok(c)) if c.status == "behind" => return Ok(Outcome::Ahead),
                    Ok(Ok(c)) if !c.commits.is_empty() => {
                        let total = c.ahead_by.max(c.commits.len());
                        (c.commits, total)
                    }
                    Err((code, out)) if classify_curl(code, &out) == FailKind::Offline => {
                        return Err(Self::api_fail(code, &out, ""));
                    }
                    // el commit instalado no está en el repo (compilado de otro lado)
                    _ => (vec![fallback(&rel)], 1),
                }
            }
            (Some(new), true) => {
                let url = format!("https://api.github.com/repos/{slug}/commits/{new}");
                let c = self.curl_get(&url).ok().and_then(|j| release::parse_commit(&j));
                (vec![c.unwrap_or_else(|| fallback(&rel))], 1)
            }
            (None, _) => (vec![fallback(&rel)], 1),
        };
        if !job.installed || !job.install_now {
            return Ok(Outcome::Available { commits, total, installable: job.installed });
        }
        self.check_cancel()?;

        // ¿se puede escribir donde está instalado?
        let dest = job.bindir.clone();
        let probe = dest.join(".capyro-update-test");
        if std::fs::write(&probe, b"ok").is_err() {
            return Err((
                FailKind::Permission,
                format!(
                    "No tengo permiso para escribir en {}. Volvé a instalar con CapyroInstaller (o abrí Capyro como administrador)",
                    dest.display()
                ),
            ));
        }
        let _ = std::fs::remove_file(&probe);

        /* 2. bajar el zip */
        self.set_stage(Stage::Downloading);
        let work = cache.join("update");
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).map_err(|e| (FailKind::Other, format!("No pude crear {}: {e}", work.display())))?;
        let zip = work.join(WINDOWS_ASSET);
        self.download(&rel.asset_url, &zip, rel.asset_size)?;
        self.check_cancel()?;

        /* 3. descomprimir */
        self.set_stage(Stage::Installing);
        self.set_detail("descomprimiendo".into());
        let unpacked = work.join("nuevo");
        std::fs::create_dir_all(&unpacked).map_err(|e| (FailKind::Other, e.to_string()))?;
        let mut tar = Command::new(system_tool("tar.exe"));
        tar.arg("-xf").arg(&zip).arg("-C").arg(&unpacked);
        self.exec(tar, false, |_, _| {})
            .map_err(|(_, out)| (FailKind::Other, format!("No pude descomprimir el zip: {}", last_meaningful(&out))))?;
        // el zip puede traer todo adentro de una carpeta
        let root = if unpacked.join("capyro.exe").is_file() {
            unpacked.clone()
        } else {
            std::fs::read_dir(&unpacked)
                .ok()
                .and_then(|rd| rd.flatten().map(|e| e.path()).find(|p| p.join("capyro.exe").is_file()))
                .ok_or((FailKind::Other, "El zip no trae capyro.exe".to_owned()))?
        };

        /* 4. reemplazar los archivos */
        self.set_detail("copiando".into());
        swap_in(&root, &dest).map_err(|e| (FailKind::Permission, format!("No se pudo instalar: {e}")))?;
        let _ = std::fs::remove_dir_all(&work);
        if let Some(v) = &rel.version {
            self.set_uninstall_version(v);
        }
        self.logln("listo");
        Ok(Outcome::Installed { commits, total })
    }

    /// curl al archivo; mientras baja, "12,3 de 30,1 MB" en la interfaz.
    fn download(&mut self, url: &str, to: &Path, size: u64) -> Result<(), Fail> {
        let mut cmd = Command::new(system_tool("curl.exe"));
        cmd.args(["-sS", "-L", "--fail", "--retry", "2", "--connect-timeout", "15", "-A", "Capyro-updater", "-o"])
            .arg(to)
            .arg(url);
        let done = Arc::new(AtomicBool::new(false));
        let watcher = {
            let (done, shared, wake, to) = (done.clone(), self.shared.clone(), self.wake.clone(), to.to_path_buf());
            std::thread::spawn(move || {
                let mb = |b: u64| format!("{:.1}", b as f64 / 1_048_576.0).replace('.', ",");
                while !done.load(Ordering::Relaxed) {
                    let got = std::fs::metadata(&to).map(|m| m.len()).unwrap_or(0);
                    let text = if size > 0 { format!("{} de {} MB", mb(got), mb(size)) } else { format!("{} MB", mb(got)) };
                    lock(&shared).detail = text;
                    wake();
                    std::thread::sleep(Duration::from_millis(200));
                }
            })
        };
        let r = self.exec(cmd, false, |_, _| {});
        done.store(true, Ordering::Relaxed);
        let _ = watcher.join();
        r.map_err(|(code, out)| match classify_curl(code, &out) {
            FailKind::Offline => (FailKind::Offline, "Sin conexión: se cortó la descarga".to_owned()),
            _ => (FailKind::Other, format!("No pude bajar la actualización: {}", last_meaningful(&out))),
        })?;
        let got = std::fs::metadata(to).map(|m| m.len()).unwrap_or(0);
        if size > 0 && got != size {
            return Err((FailKind::Other, format!("La descarga quedó incompleta ({got} de {size} bytes)")));
        }
        Ok(())
    }

    /// La versión que muestra "Aplicaciones instaladas" (si se instaló con
    /// CapyroInstaller).
    fn set_uninstall_version(&mut self, version: &str) {
        let reg = system_tool("reg.exe");
        let mut q = Command::new(&reg);
        q.args(["query", UNINSTALL_KEY]);
        if self.exec(q, false, |_, _| {}).is_err() {
            return;
        }
        let mut add = Command::new(&reg);
        add.args(["add", UNINSTALL_KEY, "/v", "DisplayVersion", "/t", "REG_SZ", "/d", version, "/f"]);
        let _ = self.exec(add, false, |_, _| {});
    }
}

/// Copia todo lo de `src` a `dest`. Lo que ya existe se renombra a `.old`
/// antes (así se puede con el .exe y las DLL en uso). Si algo falla, vuelve
/// todo como estaba. La config instalada no se pisa si ya existe (puede
/// tener cambios del usuario).
fn swap_in(src: &Path, dest: &Path) -> std::io::Result<()> {
    let mut done: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    let result = (|| {
        for file in files_under(src, 8) {
            let rel = file.strip_prefix(src).map_err(std::io::Error::other)?;
            let target = dest.join(rel);
            let in_config = rel.components().next().is_some_and(|c| c.as_os_str().eq_ignore_ascii_case("config"));
            if in_config && target.exists() {
                continue;
            }
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let backup = if target.exists() {
                let mut old = target.as_os_str().to_owned();
                old.push(".old");
                let old = PathBuf::from(old);
                if old.exists() && std::fs::remove_file(&old).is_err() {
                    // el .old de la vez anterior sigue en uso: otro nombre
                    let mut alt = old.as_os_str().to_owned();
                    alt.push(format!("{}", std::process::id()));
                    std::fs::rename(&target, PathBuf::from(&alt))?;
                    Some(PathBuf::from(alt))
                } else {
                    std::fs::rename(&target, &old)?;
                    Some(old)
                }
            } else {
                None
            };
            done.push((target.clone(), backup));
            std::fs::copy(&file, &target)?;
        }
        Ok(())
    })();
    if result.is_err() {
        for (target, backup) in done.into_iter().rev() {
            let _ = std::fs::remove_file(&target);
            if let Some(b) = backup {
                let _ = std::fs::rename(&b, &target);
            }
        }
        return result;
    }
    // DLL que la versión nueva ya no trae (por ejemplo avcodec-62.dll cuando
    // pasó a avcodec-63.dll): a .old, y se borran al abrir.
    let new_names: std::collections::HashSet<std::ffi::OsString> = std::fs::read_dir(src)
        .map(|rd| rd.flatten().map(|e| e.file_name().to_ascii_lowercase()).collect())
        .unwrap_or_default();
    if let Ok(rd) = std::fs::read_dir(dest) {
        for e in rd.flatten() {
            let p = e.path();
            let is_dll = p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dll"));
            if is_dll && !new_names.contains(&e.file_name().to_ascii_lowercase()) {
                let mut old = p.as_os_str().to_owned();
                old.push(".old");
                let _ = std::fs::rename(&p, PathBuf::from(old));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reemplaza_lo_instalado() {
        let base = std::env::temp_dir().join(format!("capyro-swap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (src, dest) = (base.join("nuevo"), base.join("instalado"));
        for d in [src.join("config"), dest.join("config")] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(src.join("capyro.exe"), "exe nuevo").unwrap();
        std::fs::write(src.join("avcodec-63.dll"), "dll nueva").unwrap();
        std::fs::write(src.join("config").join("capyro.lua"), "config de ejemplo").unwrap();
        std::fs::write(dest.join("capyro.exe"), "exe viejo").unwrap();
        std::fs::write(dest.join("avcodec-62.dll"), "dll vieja").unwrap();
        std::fs::write(dest.join("config").join("capyro.lua"), "mi config").unwrap();
        std::fs::write(dest.join("uninstall.exe"), "desinstalador").unwrap();

        swap_in(&src, &dest).unwrap();
        let read = |p: &str| std::fs::read_to_string(dest.join(p)).unwrap();
        assert_eq!(read("capyro.exe"), "exe nuevo");
        assert_eq!(read("capyro.exe.old"), "exe viejo");
        assert_eq!(read("avcodec-63.dll"), "dll nueva");
        // la DLL que ya no se usa queda como .old (se borra al abrir)
        assert!(!dest.join("avcodec-62.dll").exists());
        assert_eq!(read("avcodec-62.dll.old"), "dll vieja");
        // la config instalada no se pisa, y lo que no viene en el zip queda
        assert_eq!(read("config/capyro.lua"), "mi config");
        assert_eq!(read("uninstall.exe"), "desinstalador");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn errores_de_curl() {
        assert_eq!(classify_curl(Some(6), ""), FailKind::Offline);
        assert_eq!(classify_curl(Some(22), "The requested URL returned error: 404"), FailKind::Repo);
        assert_eq!(classify_curl(Some(1), "algo"), FailKind::Other);
    }
}

//! Actualizador en la Mac: baja el Capyro.app publicado en el repo (release
//! `macos`, la arma el CI) para la arquitectura de esta Mac y lo cambia por
//! el instalado. No compila nada.
//!
//! Usa `/usr/bin/curl` y `/usr/bin/ditto`, que vienen con macOS. El
//! Capyro.app se reemplaza entero (la config y las listas están en
//! `~/Library/Application Support/Capyro`, no adentro): el nuevo queda al
//! lado con otro nombre y se cambian de lugar, así nunca queda uno a medias.
//! El viejo se borra al volver a abrir Capyro ([`cleanup_after_update`]).
//!
//! Es lo mismo que hace `updater/windows.rs` (la misma API de GitHub), con
//! las herramientas de la Mac.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::release::{self, Release, MACOS_TAG};
use super::*;
use crate::ffi::BuildInfo;

const CURL: &str = "/usr/bin/curl";
const DITTO: &str = "/usr/bin/ditto";
/// Al lado del instalado, mientras se cambian: el nuevo y el viejo.
const STAGED: &str = ".Capyro-nuevo.app";
const OLD: &str = ".Capyro-viejo.app";

pub(super) fn try_run(w: &mut Worker, job: &Job, cache: &Path) -> Result<Outcome, Fail> {
    w.try_run_macos(job, cache)
}

/// Instalado = corre desde un Capyro.app que está en una carpeta de
/// Aplicaciones (`/Applications` o `~/Applications`, donde lo deja
/// CapyroInstaller). `bindir` es el .app en sí.
pub(super) fn install_location(_info: &BuildInfo) -> Option<Install> {
    let app = crate::system::macos::bundle_path()?;
    let parent = app.parent()?.to_path_buf();
    let in_apps = parent.file_name().is_some_and(|n| n == "Applications" || n == "Aplicaciones");
    in_apps.then_some(Install { prefix: parent, bindir: app })
}

/// Borra el Capyro.app viejo que dejó la actualización (y lo bajado).
pub(super) fn cleanup_after_update() {
    let Some(app) = crate::system::macos::bundle_path() else { return };
    std::thread::spawn(move || {
        if let Some(parent) = app.parent() {
            for name in [OLD, STAGED] {
                let p = parent.join(name);
                if p.exists() {
                    let _ = std::fs::remove_dir_all(&p);
                }
            }
        }
        let _ = std::fs::remove_dir_all(crate::system::paths::cache_dir().join("update"));
    });
}

/// `bindir` es el Capyro.app.
pub(super) fn relaunch(bindir: &Path) -> std::io::Result<()> {
    Command::new(bindir.join("Contents/MacOS/Capyro"))
        .env("CAPYRO_UPDATED", "1") // el nuevo muestra "Capyro actualizado a …"
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

/// En la Mac no hace falta nada (no se compila: no hay que bajar la prioridad).
pub(super) fn prepare(_cmd: &mut Command, _nice: bool) {}

pub(super) fn kill_tree(pid: u32) {
    let _ = Command::new("/bin/kill").arg(pid.to_string()).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

pub(super) fn child_path() -> OsString {
    std::env::var_os("PATH")
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "/usr/bin:/bin:/usr/sbin:/sbin".into())
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

/// Un solo Capyro actualizándose a la vez: el archivo se crea "solo si no
/// existe" y se borra al terminar. Uno de hace más de una hora es de una
/// actualización que se cortó (Capyro se cerró a la fuerza): se ignora.
struct Lock(PathBuf);

impl Lock {
    fn take(path: PathBuf) -> Option<Lock> {
        let stale = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > Duration::from_secs(3600));
        if stale {
            let _ = std::fs::remove_file(&path);
        }
        std::fs::OpenOptions::new().write(true).create_new(true).open(&path).ok()?;
        Some(Lock(path))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl Worker {
    fn curl_get(&mut self, url: &str) -> Result<String, (Option<i32>, String)> {
        let mut cmd = Command::new(CURL);
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

    fn try_run_macos(&mut self, job: &Job, cache: &Path) -> Result<Outcome, Fail> {
        let _lock = Lock::take(cache.join("update.lock"))
            .ok_or((FailKind::Other, "Otra ventana de Capyro ya se está actualizando".to_owned()))?;
        if !Path::new(CURL).is_file() || !Path::new(DITTO).is_file() {
            return Err((FailKind::MissingTool, "Para actualizarse Capyro necesita curl y ditto (vienen con macOS)".into()));
        }
        let Some(slug) = github_slug(&job.repo) else {
            return Err((
                FailKind::Repo,
                "En la Mac las actualizaciones salen de GitHub: poné update_repo = \"usuario/Capyro\"".into(),
            ));
        };
        let asset = release::macos_asset();

        /* 1. ¿qué hay publicado para la Mac? */
        self.set_stage(Stage::Checking);
        let url = format!("https://api.github.com/repos/{slug}/releases/tags/{MACOS_TAG}");
        let json = self
            .curl_get(&url)
            .map_err(|(c, o)| Self::api_fail(c, &o, "El repo todavía no tiene una versión para la Mac publicada"))?;
        let rel: Release = release::parse_release(&json, asset).map_err(|e| (FailKind::Repo, e))?;
        self.logln(&format!(
            "publicado: {} · commit {} · versión {} · {asset}",
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
            subject: if rel.name.is_empty() { "Versión para la Mac".into() } else { rel.name.clone() },
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

        // ¿se puede escribir en la carpeta de Aplicaciones?
        let app = job.bindir.clone();
        let Some(apps) = app.parent().map(Path::to_path_buf) else {
            return Err((FailKind::Other, "No sé dónde está instalado Capyro".into()));
        };
        let probe = apps.join(".capyro-update-test");
        if std::fs::write(&probe, b"ok").is_err() {
            return Err((
                FailKind::Permission,
                format!("No tengo permiso para escribir en {}. Volvé a instalar con CapyroInstaller", apps.display()),
            ));
        }
        let _ = std::fs::remove_file(&probe);

        /* 2. bajar el zip */
        self.set_stage(Stage::Downloading);
        let work = cache.join("update");
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).map_err(|e| (FailKind::Other, format!("No pude crear {}: {e}", work.display())))?;
        let zip = work.join(asset);
        self.download(&rel.asset_url, &zip, rel.asset_size)?;
        self.check_cancel()?;

        /* 3. descomprimir (ditto respeta permisos, enlaces y la firma) */
        self.set_stage(Stage::Installing);
        self.set_detail("descomprimiendo".into());
        let unpacked = work.join("nuevo");
        std::fs::create_dir_all(&unpacked).map_err(|e| (FailKind::Other, e.to_string()))?;
        let mut ditto = Command::new(DITTO);
        ditto.arg("-x").arg("-k").arg(&zip).arg(&unpacked);
        self.exec(ditto, false, |_, _| {})
            .map_err(|(_, out)| (FailKind::Other, format!("No pude descomprimir el zip: {}", last_meaningful(&out))))?;
        let new_app = find_app(&unpacked).ok_or((FailKind::Other, "El zip no trae Capyro.app".to_owned()))?;

        /* 4. cambiar el .app */
        self.set_detail("copiando".into());
        swap_app(&new_app, &app).map_err(|e| (FailKind::Permission, format!("No se pudo instalar: {e}")))?;
        let _ = std::fs::remove_dir_all(&work);
        // que el Finder se entere ("Abrir con…" con los formatos de la versión nueva)
        let _ = Command::new("/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister")
            .arg("-f")
            .arg(&app)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        self.logln("listo");
        Ok(Outcome::Installed { commits, total })
    }

    /// curl al archivo; mientras baja, "12,3 de 30,1 MB" en la interfaz.
    fn download(&mut self, url: &str, to: &Path, size: u64) -> Result<(), Fail> {
        let mut cmd = Command::new(CURL);
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
}

/// El Capyro.app de adentro de lo descomprimido (arriba de todo o en una carpeta).
fn find_app(dir: &Path) -> Option<PathBuf> {
    let ok = |p: &Path| p.join("Contents/MacOS/Capyro").is_file();
    let direct = dir.join("Capyro.app");
    if ok(&direct) {
        return Some(direct);
    }
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find_map(|p| {
        if p.extension().is_some_and(|e| e == "app") && ok(&p) {
            Some(p)
        } else if p.is_dir() {
            let inner = p.join("Capyro.app");
            ok(&inner).then_some(inner)
        } else {
            None
        }
    })
}

/// Pone `new_app` en el lugar de `app`: primero se copia al lado (con otro
/// nombre), después el instalado pasa a `.Capyro-viejo.app` y el nuevo toma
/// su nombre. Si algo falla, queda el de antes.
fn swap_app(new_app: &Path, app: &Path) -> std::io::Result<()> {
    let parent = app.parent().ok_or_else(|| std::io::Error::other("sin carpeta"))?;
    let staged = parent.join(STAGED);
    let old = parent.join(OLD);
    for p in [&staged, &old] {
        if p.exists() {
            std::fs::remove_dir_all(p)?;
        }
    }
    // mismo disco: se mueve; si no (la caché está en otro), ditto lo copia
    if std::fs::rename(new_app, &staged).is_err() {
        let st = Command::new(DITTO).arg(new_app).arg(&staged).status()?;
        if !st.success() {
            let _ = std::fs::remove_dir_all(&staged);
            return Err(std::io::Error::other("no pude copiar el Capyro.app nuevo"));
        }
    }
    if app.exists() {
        std::fs::rename(app, &old)?;
    }
    if let Err(e) = std::fs::rename(&staged, app) {
        let _ = std::fs::rename(&old, app);
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cambia_el_app() {
        let base = std::env::temp_dir().join(format!("capyro-mac-swap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let mk = |root: &Path, text: &str| {
            let exe = root.join("Contents/MacOS");
            std::fs::create_dir_all(&exe).unwrap();
            std::fs::write(exe.join("Capyro"), text).unwrap();
        };
        let apps = base.join("Applications");
        let app = apps.join("Capyro.app");
        mk(&app, "viejo");
        let unpacked = base.join("nuevo");
        mk(&unpacked.join("Capyro.app"), "nuevo");
        let found = find_app(&unpacked).unwrap();
        swap_app(&found, &app).unwrap();
        assert_eq!(std::fs::read_to_string(app.join("Contents/MacOS/Capyro")).unwrap(), "nuevo");
        assert_eq!(std::fs::read_to_string(apps.join(OLD).join("Contents/MacOS/Capyro")).unwrap(), "viejo");
        let _ = std::fs::remove_dir_all(&base);
    }
}

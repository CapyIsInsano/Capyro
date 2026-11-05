//! Actualizador en Linux: git + cmake (ver el comentario de `updater.rs`).

use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::*;
use crate::ffi::BuildInfo;

pub(super) fn try_run(w: &mut Worker, job: &Job, cache: &Path) -> Result<Outcome, Fail> {
    w.try_run_unix(job, cache)
}

/// Prefijo con el que se compiló, o uno distinto puesto al instalar.
pub(super) fn install_location(info: &BuildInfo) -> Option<Install> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let configured = Path::new(&info.bindir).join("capyro");
    if configured.canonicalize().ok().as_deref() == Some(exe.as_path()) {
        return Some(Install { prefix: PathBuf::from(&info.prefix), bindir: PathBuf::from(&info.bindir) });
    }
    let bindir = exe.parent()?;
    let prefix = bindir.parent()?;
    let looks_installed = bindir.file_name()? == "bin" && prefix.join("share/capyro/capyro.lua").is_file();
    looks_installed.then(|| Install { prefix: prefix.to_path_buf(), bindir: bindir.to_path_buf() })
}

pub(super) fn cleanup_after_update() {}

pub(super) fn relaunch(bindir: &Path) -> std::io::Result<()> {
    Command::new(bindir.join("capyro"))
        .env("CAPYRO_UPDATED", "1") // el nuevo muestra "Capyro actualizado a …"
        .stdin(Stdio::null())
        .process_group(0)
        .spawn()
        .map(|_| ())
}

/// Grupo de procesos propio (para cortar todo cmake/cargo de una) y, al
/// compilar, prioridad baja: Capyro sigue sonando bien mientras tanto.
pub(super) fn prepare(cmd: &mut Command, nice: bool) {
    cmd.process_group(0);
    if nice {
        // SAFETY: setpriority es async-signal-safe; corre en el hijo antes del exec.
        unsafe {
            cmd.pre_exec(|| {
                libc::setpriority(libc::PRIO_PROCESS, 0, 10);
                Ok(())
            });
        }
    }
}

pub(super) fn kill_tree(pid: u32) {
    // SAFETY: kill() a un grupo de procesos que creamos nosotros.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGTERM);
    }
}

/// PATH + ~/.cargo/bin (Capyro se abre desde el escritorio, donde rustup
/// muchas veces no está en el PATH).
pub(super) fn child_path() -> OsString {
    let mut parts: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_else(|| vec![PathBuf::from("/usr/local/bin"), PathBuf::from("/usr/bin"), PathBuf::from("/bin")]);
    if let Some(home) = std::env::var_os("HOME") {
        let cargo = PathBuf::from(home).join(".cargo/bin");
        if cargo.is_dir() && !parts.contains(&cargo) {
            parts.push(cargo);
        }
    }
    std::env::join_paths(parts).unwrap_or_default()
}

fn find_tool(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&child_path())
        .map(|d| d.join(name))
        .find(|p| {
            std::fs::metadata(p)
                .map(|m| m.is_file() && std::os::unix::fs::PermissionsExt::mode(&m.permissions()) & 0o111 != 0)
                .unwrap_or(false)
        })
}

fn writable(dir: &Path) -> bool {
    let Ok(c) = std::ffi::CString::new(dir.as_os_str().as_encoded_bytes()) else { return false };
    // SAFETY: cadena terminada en NUL válida.
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}

/// La carpeta más cercana que exista (para chequear permisos antes de crearla).
fn existing_ancestor(p: &Path) -> &Path {
    let mut cur = p;
    while !cur.exists() {
        match cur.parent() {
            Some(parent) => cur = parent,
            None => break,
        }
    }
    cur
}

impl Worker {
    fn git(&mut self, args: &[&str]) -> Result<String, Fail> {
        let mut cmd = Command::new("git");
        cmd.args(["-c", "http.lowSpeedLimit=1000", "-c", "http.lowSpeedTime=20"]).args(args);
        self.exec(cmd, false, |_, _| {}).map_err(|(_, out)| {
            let kind = classify_git(&out);
            let msg = match kind {
                FailKind::Offline => "Sin conexión: no pude llegar a GitHub".to_owned(),
                FailKind::Repo => "No encontré el repo (¿es privado o está mal la URL?)".to_owned(),
                _ => format!("git falló: {}", last_meaningful(&out)),
            };
            (kind, msg)
        })
    }


    fn try_run_unix(&mut self, job: &Job, cache: &Path) -> Result<Outcome, Fail> {

        // Un solo Capyro actualizándose a la vez.
        let lock_file = std::fs::File::create(cache.join("update.lock"))
            .map_err(|e| (FailKind::Other, format!("No pude crear el lock: {e}")))?;
        // SAFETY: descriptor válido mientras viva lock_file (todo este método).
        if unsafe { libc::flock(std::os::fd::AsRawFd::as_raw_fd(&lock_file), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err((FailKind::Other, "Otra ventana de Capyro ya se está actualizando".into()));
        }

        if find_tool("git").is_none() {
            return Err((FailKind::MissingTool, "Para actualizarse Capyro necesita git".into()));
        }

        /* 1. ¿hay algo nuevo? */
        self.set_stage(Stage::Checking);
        let refspec = match &job.branch {
            Some(b) => format!("refs/heads/{b}"),
            None => "HEAD".to_owned(),
        };
        let out = self.git(&["ls-remote", "--symref", &job.repo, &refspec])?;
        let mut remote_hash = None;
        let mut branch = job.branch.clone();
        for line in out.lines() {
            if let Some(rest) = line.strip_prefix("ref: refs/heads/") {
                branch.get_or_insert_with(|| rest.split('\t').next().unwrap_or("main").to_owned());
            } else if let Some((hash, _)) = line.split_once('\t') {
                if hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                    remote_hash = Some(hash.to_owned());
                }
            }
        }
        let Some(remote_hash) = remote_hash else {
            return Err((FailKind::Repo, "El repo está vacío o no tiene esa rama".into()));
        };
        let branch = branch.unwrap_or_else(|| "main".to_owned());
        if remote_hash == job.built_commit {
            return Ok(Outcome::UpToDate { hash: short(&remote_hash) });
        }
        self.check_cancel()?;

        /* 2. bajar el código */
        self.set_stage(Stage::Downloading);
        let src = cache.join("src");
        let src_s = src.to_string_lossy().into_owned();
        let fetched = if src.join(".git").is_dir() {
            let r = self
                .git(&["-C", &src_s, "remote", "set-url", "origin", &job.repo])
                .and_then(|_| self.git(&["-C", &src_s, "fetch", "--depth", "50", "--no-tags", "origin", &branch]))
                .and_then(|_| self.git(&["-C", &src_s, "reset", "--hard", "FETCH_HEAD"]))
                .and_then(|_| self.git(&["-C", &src_s, "clean", "-fdq"]));
            match r {
                Ok(_) => true,
                Err(e) if e.0 == FailKind::Offline => return Err(e),
                Err(_) => {
                    // clon roto: se borra y se vuelve a bajar
                    let _ = std::fs::remove_dir_all(&src);
                    false
                }
            }
        } else {
            let _ = std::fs::remove_dir_all(&src);
            false
        };
        if !fetched {
            self.git(&["clone", "--depth", "50", "--no-tags", "--branch", &branch, &job.repo, &src_s])?;
        }

        // ¿el binario es más nuevo que el repo? (commits sin subir)
        let built = job.built_commit.as_str();
        let built_known = !built.is_empty()
            && self.git(&["-C", &src_s, "cat-file", "-e", &format!("{built}^{{commit}}")]).is_ok();
        if built_known && self.git(&["-C", &src_s, "merge-base", "--is-ancestor", "HEAD", built]).is_ok() {
            return Ok(Outcome::Ahead);
        }

        // los commits nuevos: "el parche"
        let range = format!("{built}..HEAD");
        let mut log_args = vec!["-C", &src_s, "log", "--format=%h%x1f%s%x1f%ct", "-n", "20"];
        if built_known {
            log_args.push(&range);
        } else {
            log_args.push("HEAD");
        }
        let commits: Vec<CommitInfo> = self
            .git(&log_args)?
            .lines()
            .filter_map(|l| {
                let mut p = l.split('\x1f');
                Some(CommitInfo {
                    hash: p.next()?.to_owned(),
                    subject: p.next()?.to_owned(),
                    time: p.next()?.trim().parse().unwrap_or(0),
                })
            })
            .collect();
        let mut commits = commits;
        if !built_known {
            // no sé desde dónde: muestro solo el último
            commits.truncate(1);
        }
        let total = if built_known {
            self.git(&["-C", &src_s, "rev-list", "--count", &range])?.trim().parse().unwrap_or(commits.len())
        } else {
            1
        };
        if commits.is_empty() {
            return Err((FailKind::Other, "No pude leer los commits nuevos".into()));
        }
        // primero se muestra qué hay de nuevo; se instala solo si el usuario dice que sí
        if !job.installed || !job.install_now {
            return Ok(Outcome::Available { commits, total, installable: job.installed });
        }
        let Some(cmake) = find_tool("cmake") else {
            return Err((FailKind::MissingTool, "Para actualizarse Capyro necesita cmake".into()));
        };
        if find_tool("cargo").is_none() {
            return Err((FailKind::MissingTool, "Para actualizarse Capyro necesita Rust (cargo)".into()));
        }
        self.check_cancel()?;

        /* 3. compilar */
        self.set_stage(Stage::Building);
        let build = cache.join("build");
        let onoff = |b: bool| if b { "ON" } else { "OFF" };
        let mut configure = Command::new(&cmake);
        configure
            .arg("-S")
            .arg(&src)
            .arg("-B")
            .arg(&build)
            .arg(format!("-DCMAKE_BUILD_TYPE={}", job.build_type))
            .arg(format!("-DCMAKE_INSTALL_PREFIX={}", job.prefix))
            .arg(format!("-DCMAKE_INSTALL_BINDIR={}", job.bindir.display()))
            .arg(format!("-DCAPYRO_DIST={}", onoff(job.dist)))
            .arg(format!("-DCAPYRO_NATIVE={}", onoff(job.native)))
            // solo lo de este sistema (Lin): nada de Windows
            .arg(format!("-DCAPYRO_SYSTEM={}", job.system.code()))
            .arg("-DCAPYRO_BUILD_TESTS=OFF");
        self.set_detail("configurando".into());
        self.exec(configure, true, |_, _| {}).map_err(|(_, out)| self.build_error(&out))?;

        let jobs = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
        let mut compile = Command::new(&cmake);
        compile.arg("--build").arg(&build).args(["--target", "capyro", "--parallel", &jobs.to_string()]);
        let mut last_detail = Instant::now() - Duration::from_secs(1);
        self.exec(compile, true, |w, line| {
            // no despertar a la interfaz por cada línea
            if let Some(d) = progress_detail(line) {
                if last_detail.elapsed() > Duration::from_millis(120) {
                    last_detail = Instant::now();
                    w.set_detail(d);
                }
            }
        })
        .map_err(|(_, out)| self.build_error(&out))?;
        self.check_cancel()?;

        /* 4. instalar */
        self.set_stage(Stage::Installing);
        self.install(job, &cmake, &build)?;
        Ok(Outcome::Installed { commits, total })
    }

    fn build_error(&self, out: &str) -> Fail {
        if self.cancelled() {
            return (FailKind::Other, "cancelado".into());
        }
        if classify_git(out) == FailKind::Offline {
            // CMake baja Corrosion/miniaudio la primera vez
            return (FailKind::Offline, "Sin conexión: no pude bajar las dependencias".into());
        }
        let line = out
            .lines()
            .rev()
            .find(|l| l.contains("error") && !l.contains("warning"))
            .map(|l| l.trim().to_owned())
            .unwrap_or_else(|| last_meaningful(out));
        (FailKind::Build, format!("Falló la compilación: {}", ellipsize(&line, 90)))
    }

    /// `cmake --install`. Antes se borra el ejecutable viejo: no se puede
    /// sobrescribir un binario que está corriendo ("Text file busy"), pero sí
    /// borrarlo (este proceso sigue usando su copia hasta que se reinicie).
    fn install(&mut self, job: &Job, cmake: &Path, build: &Path) -> Result<(), Fail> {
        const SCRIPT: &str = r#"set -e; rm -f -- "$1/capyro"; exec "$2" --install "$3""#;
        let share = Path::new(&job.prefix).join("share");
        let can_write = writable(&job.bindir) && writable(existing_ancestor(&share));
        let mut cmd = if can_write {
            let mut c = Command::new("/bin/sh");
            c.args(["-c", SCRIPT, "sh"]);
            c
        } else if let Some(pkexec) = find_tool("pkexec") {
            self.set_detail("pidiendo permiso de administrador".into());
            let mut c = Command::new(pkexec);
            c.args(["/bin/sh", "-c", SCRIPT, "sh"]);
            c
        } else {
            return Err((
                FailKind::Permission,
                format!(
                    "No tengo permiso para escribir en {}. Instalá a mano: sudo cmake --install {}",
                    job.bindir.display(),
                    build.display()
                ),
            ));
        };
        cmd.arg(&job.bindir).arg(cmake).arg(build);
        self.exec(cmd, false, |_, _| {}).map(|_| ()).map_err(|(code, out)| match code {
            // pkexec: 126 = se canceló la contraseña, 127 = sin agente de polkit
            Some(126) if !can_write => (FailKind::Permission, "Se canceló el permiso de administrador".into()),
            Some(127) if !can_write => (
                FailKind::Permission,
                format!("No hay agente de polkit para pedir la contraseña. Instalá a mano: sudo cmake --install {}", build.display()),
            ),
            _ => (FailKind::Permission, format!("No se pudo instalar: {}", last_meaningful(&out))),
        })
    }
}

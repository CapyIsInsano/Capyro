//! Actualizador de Capyro.
//!
//! Capyro se instala compilándolo, así que actualizarse es lo mismo que haría
//! uno a mano, pero solo:
//!
//!   1. `git ls-remote` al repo → ¿el último commit es distinto al de este binario?
//!   2. baja el código a `~/.cache/capyro/src` (clon liviano, se reutiliza)
//!   3. `cmake` + `cmake --build` en `~/.cache/capyro/build` (incremental: la
//!      primera vez tarda, las siguientes son segundos)
//!   4. `cmake --install` al mismo lugar donde está instalado (con `pkexec`
//!      si esa carpeta es del sistema)
//!
//! Todo corre en un hilo aparte; la interfaz consulta el estado en cada frame
//! con [`Updater::progress`] y recibe el resultado con [`Updater::poll`].

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::ffi::BuildInfo;

/// Un commit del repo, para mostrar "el parche".
#[derive(Debug, Clone, PartialEq)]
pub struct CommitInfo {
    /// Hash corto (7 caracteres).
    pub hash: String,
    pub subject: String,
    /// Fecha del commit (unix).
    pub time: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Checking,
    Downloading,
    Building,
    Installing,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Stage::Checking => "Buscando actualizaciones…",
            Stage::Downloading => "Descargando…",
            Stage::Building => "Compilando…",
            Stage::Installing => "Instalando…",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailKind {
    /// Sin internet (o no se llega a GitHub).
    Offline,
    /// El repo no existe, es privado o la URL está mal.
    Repo,
    /// Falta git, cmake o cargo.
    MissingTool,
    /// Falló la compilación.
    Build,
    /// No se pudo escribir donde está instalado.
    Permission,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Ya está en el último commit.
    UpToDate { hash: String },
    /// Este binario es más nuevo que lo que hay en el repo.
    Ahead,
    /// Hay commits nuevos (todavía no se instaló nada). `commits` = los que se
    /// instalarían, el primero es el último; `total` = cuántos son en total.
    /// `installable` = false si este Capyro corre desde build/ (sin instalar).
    Available { commits: Vec<CommitInfo>, total: usize, installable: bool },
    /// Instalado: hay que reiniciar. `commits` = los nuevos (el primero es el último).
    Installed { commits: Vec<CommitInfo>, total: usize },
    Failed { kind: FailKind, message: String },
}

/// Repo oficial, para cuando se compiló sin git.
pub const DEFAULT_REPO: &str = "CapyIsInsano/Capyro";

/// Qué actualizar y cómo volver a compilarlo (sale de los datos del build).
#[derive(Debug, Clone)]
pub struct Job {
    pub repo: String,
    pub branch: Option<String>,
    pub built_commit: String,
    pub installed: bool,
    pub bindir: PathBuf,
    pub prefix: String,
    pub build_type: String,
    pub dist: bool,
    pub native: bool,
    /// false = solo buscar y mostrar los commits; true = compilar e instalar.
    pub install_now: bool,
}

impl Job {
    /// `install` sale de [`install_location`]; `repo_override` es la opción
    /// `update_repo` de capyro.lua.
    pub fn new(
        info: &BuildInfo,
        install: Option<&Install>,
        repo_override: Option<&str>,
        branch: Option<&str>,
    ) -> Result<Self, String> {
        let raw = repo_override
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .unwrap_or(info.remote.trim());
        // compilado sin git (desde un .zip): el repo oficial
        let raw = if raw.is_empty() { DEFAULT_REPO } else { raw };
        if raw.contains("TU_USUARIO") {
            return Err("El repo todavía dice TU_USUARIO: poné el tuyo en update_repo (capyro.lua)".into());
        }
        // Se reinstala donde está ahora (aunque se haya usado --prefix al instalar).
        let (prefix, bindir) = match install {
            Some(i) => (i.prefix.to_string_lossy().into_owned(), i.bindir.clone()),
            None => (info.prefix.clone(), PathBuf::from(&info.bindir)),
        };
        Ok(Self {
            repo: normalize_repo(raw),
            branch: branch.map(str::trim).filter(|b| !b.is_empty()).map(str::to_owned),
            built_commit: info.commit.clone(),
            installed: install.is_some(),
            bindir,
            prefix,
            build_type: if info.build_type.is_empty() { "Release".into() } else { info.build_type.clone() },
            dist: info.dist,
            native: info.native,
            install_now: false,
        })
    }
}

/// `git@github.com:capy/Capyro.git` o `capy/Capyro` → URL https (sin llaves
/// SSH: Capyro corre desde el escritorio y ahí no siempre hay agente).
pub fn normalize_repo(raw: &str) -> String {
    let r = raw.trim();
    if let Some(rest) = r.strip_prefix("git@github.com:") {
        return format!("https://github.com/{}", with_git(rest));
    }
    if let Some(rest) = r.strip_prefix("ssh://git@github.com/") {
        return format!("https://github.com/{}", with_git(rest));
    }
    let shorthand = !r.contains(':')
        && !r.starts_with('/')
        && !r.starts_with('.')
        && !r.starts_with('~')
        && r.matches('/').count() == 1;
    if shorthand {
        return format!("https://github.com/{}", with_git(r));
    }
    r.to_owned()
}

fn with_git(path: &str) -> String {
    let p = path.trim_end_matches('/');
    if p.ends_with(".git") {
        p.to_owned()
    } else {
        format!("{p}.git")
    }
}

/// Dónde está instalado el Capyro que está corriendo.
#[derive(Debug, Clone, PartialEq)]
pub struct Install {
    pub prefix: PathBuf,
    pub bindir: PathBuf,
}

/// `None` si este proceso no es un Capyro instalado (corre desde `build/`).
/// Reconoce el prefijo con el que se compiló y también uno distinto puesto al
/// instalar (`cmake --install build --prefix ~/.local`).
pub fn install_location(info: &BuildInfo) -> Option<Install> {
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

/// "hace 5 min", "hace 3 días"...
pub fn ago(ts: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(ts);
    let s = (now - ts).max(0);
    let plural = |n: i64, one: &str, many: &str| format!("hace {n} {}", if n == 1 { one } else { many });
    match s {
        0..=59 => "recién".to_owned(),
        60..=3599 => format!("hace {} min", s / 60),
        3600..=86_399 => plural(s / 3600, "hora", "horas"),
        86_400..=2_591_999 => plural(s / 86_400, "día", "días"),
        2_592_000..=31_535_999 => plural(s / 2_592_000, "mes", "meses"),
        _ => plural(s / 31_536_000, "año", "años"),
    }
}

/* ------------------------------------------------------------------------ */
/* estado compartido con la interfaz                                       */
/* ------------------------------------------------------------------------ */

struct Shared {
    stage: Option<Stage>,
    /// Lo que se está haciendo ahora: "eframe", "player.c"...
    detail: String,
    started: Instant,
    outcome: Option<Outcome>,
    /// Grupo de procesos del comando actual (para cortarlo al cerrar).
    pgid: Option<i32>,
    cancel: bool,
}

/// Lo que ve la interfaz mientras trabaja.
pub struct Progress {
    pub stage: Stage,
    pub detail: String,
    pub elapsed: Duration,
}

pub struct Updater {
    shared: Arc<Mutex<Shared>>,
    running: bool,
    /// Último resultado (para el menú ⓘ).
    pub last: Option<Outcome>,
}

impl Default for Updater {
    fn default() -> Self {
        Self::new()
    }
}

impl Updater {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(Shared {
                stage: None,
                detail: String::new(),
                started: Instant::now(),
                outcome: None,
                pgid: None,
                cancel: false,
            })),
            running: false,
            last: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Arranca en otro hilo. `wake` se llama cuando hay algo nuevo que mostrar.
    pub fn start(&mut self, job: Job, wake: impl Fn() + Send + Sync + 'static) {
        if self.running {
            return;
        }
        self.running = true;
        self.last = None;
        {
            let mut s = lock(&self.shared);
            s.stage = Some(Stage::Checking);
            s.detail.clear();
            s.started = Instant::now();
            s.outcome = None;
            s.cancel = false;
        }
        let shared = self.shared.clone();
        let wake = Arc::new(wake);
        std::thread::Builder::new()
            .name("capyro-update".into())
            .spawn(move || {
                let mut w = Worker { shared: shared.clone(), wake: wake.clone(), log: None, tail: Vec::new() };
                let outcome = w.run(&job);
                let mut s = lock(&shared);
                s.stage = None;
                s.pgid = None;
                if !s.cancel {
                    s.outcome = Some(outcome);
                }
                drop(s);
                wake();
            })
            .expect("no se pudo crear el hilo del actualizador");
    }

    pub fn progress(&self) -> Option<Progress> {
        let s = lock(&self.shared);
        s.stage.map(|stage| Progress { stage, detail: s.detail.clone(), elapsed: s.started.elapsed() })
    }

    /// El resultado, una sola vez cuando termina.
    pub fn poll(&mut self) -> Option<Outcome> {
        let out = lock(&self.shared).outcome.take()?;
        self.running = false;
        self.last = Some(out.clone());
        Some(out)
    }

    /// Al cerrar Capyro: corta lo que se esté compilando.
    pub fn cancel(&self) {
        let mut s = lock(&self.shared);
        s.cancel = true;
        if let Some(pgid) = s.pgid {
            // SAFETY: kill() a un grupo de procesos que creamos nosotros.
            unsafe {
                libc::kill(-pgid, libc::SIGTERM);
            }
        }
    }
}

fn lock(m: &Mutex<Shared>) -> std::sync::MutexGuard<'_, Shared> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Vuelve a abrir Capyro (el ejecutable recién instalado).
pub fn relaunch(bindir: &Path) -> std::io::Result<()> {
    Command::new(bindir.join("capyro"))
        .env("CAPYRO_UPDATED", "1") // el nuevo muestra "Capyro actualizado a …"
        .stdin(Stdio::null())
        .process_group(0)
        .spawn()
        .map(|_| ())
}

/* ------------------------------------------------------------------------ */
/* el trabajo en sí                                                         */
/* ------------------------------------------------------------------------ */

type Fail = (FailKind, String);

struct Worker {
    shared: Arc<Mutex<Shared>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    log: Option<std::fs::File>,
    /// Últimas líneas del comando actual (para los mensajes de error).
    tail: Vec<String>,
}

fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(|| std::env::temp_dir());
    base.join("capyro")
}

/// `~/.cache/capyro/update.log` con `~` para mostrar.
pub fn log_path_display() -> String {
    let p = cache_dir().join("update.log").to_string_lossy().into_owned();
    match std::env::var_os("HOME") {
        Some(h) => p.replacen(&*h.to_string_lossy(), "~", 1),
        None => p,
    }
}

/// PATH + ~/.cargo/bin (Capyro se abre desde el escritorio, donde rustup
/// muchas veces no está en el PATH).
fn child_path() -> OsString {
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

/// Errores de git → sin conexión / repo / otro.
pub fn classify_git(output: &str) -> FailKind {
    let o = output.to_ascii_lowercase();
    const NET: [&str; 12] = [
        "could not resolve host",
        "could not resolve proxy",
        "temporary failure in name resolution",
        "failed to connect",
        "couldn't connect",
        "network is unreachable",
        "connection timed out",
        "operation timed out",
        "connection refused",
        "connection reset",
        "tunnel failed",
        "no route to host",
    ];
    const REPO: [&str; 6] = [
        "repository not found",
        "not found",
        "authentication failed",
        "could not read username",
        "terminal prompts disabled",
        "does not appear to be a git repository",
    ];
    if NET.iter().any(|n| o.contains(n)) {
        FailKind::Offline
    } else if REPO.iter().any(|n| o.contains(n)) {
        FailKind::Repo
    } else {
        FailKind::Other
    }
}

/// "   Compiling eframe v0.37.0" → "eframe"; "[ 40%] Building C object core/…/player.c.o" → "player.c"
pub fn progress_detail(line: &str) -> Option<String> {
    let l = line.trim();
    if let Some(rest) = l.strip_prefix("Compiling ") {
        return rest.split_whitespace().next().map(str::to_owned);
    }
    if l.starts_with('[') && l.contains("%]") {
        if let Some(obj) = l.split("object ").nth(1) {
            let file = obj.rsplit('/').next().unwrap_or(obj);
            return Some(file.trim_end_matches(".o").to_owned());
        }
        if l.contains("Linking") {
            return Some("enlazando".to_owned());
        }
    }
    if l.starts_with("Finished ") {
        return Some("enlazando".to_owned());
    }
    None
}

impl Worker {
    fn set_stage(&self, stage: Stage) {
        let mut s = lock(&self.shared);
        s.stage = Some(stage);
        s.detail.clear();
        drop(s);
        (self.wake)();
    }

    fn set_detail(&self, d: String) {
        lock(&self.shared).detail = d;
        (self.wake)();
    }

    fn cancelled(&self) -> bool {
        lock(&self.shared).cancel
    }

    fn logln(&mut self, line: &str) {
        if let Some(f) = &mut self.log {
            let _ = writeln!(f, "{line}");
        }
    }

    /// Corre un comando con stdout+stderr juntos, línea por línea al log.
    /// Devuelve la salida (o el error con la salida) y el código.
    fn exec(&mut self, mut cmd: Command, nice: bool, mut on_line: impl FnMut(&Self, &str)) -> Result<String, (Option<i32>, String)> {
        let (reader, writer) = std::io::pipe().map_err(|e| (None, e.to_string()))?;
        let writer2 = writer.try_clone().map_err(|e| (None, e.to_string()))?;
        cmd.stdin(Stdio::null())
            .stdout(writer)
            .stderr(writer2)
            .env("PATH", child_path())
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "true")
            .env("SSH_ASKPASS", "true")
            .env("TERM", "dumb")
            .env("CARGO_TERM_COLOR", "never")
            .env("CARGO_TERM_PROGRESS_WHEN", "never")
            .env_remove("CLICOLOR_FORCE")
            .process_group(0);
        if nice {
            // SAFETY: setpriority es async-signal-safe; corre en el hijo antes del exec.
            unsafe {
                cmd.pre_exec(|| {
                    libc::setpriority(libc::PRIO_PROCESS, 0, 10);
                    Ok(())
                });
            }
        }
        let shown = format!("$ {:?} {:?}", cmd.get_program(), cmd.get_args().collect::<Vec<_>>());
        self.logln(&shown);
        let mut child = cmd.spawn().map_err(|e| (None, e.to_string()))?;
        drop(cmd); // suelta nuestras copias del extremo de escritura (si no, nunca llega el EOF)
        lock(&self.shared).pgid = Some(child.id() as i32);

        self.tail.clear();
        let mut all = String::new();
        for line in BufReader::new(reader).lines() {
            let Ok(line) = line else { break };
            self.logln(&line);
            on_line(self, &line);
            all.push_str(&line);
            all.push('\n');
            self.tail.push(line);
            if self.tail.len() > 60 {
                self.tail.remove(0);
            }
        }
        let status = child.wait().map_err(|e| (None, e.to_string()))?;
        lock(&self.shared).pgid = None;
        self.logln(&format!("→ {status}"));
        if status.success() {
            Ok(all)
        } else {
            Err((status.code(), all))
        }
    }

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

    fn run(&mut self, job: &Job) -> Outcome {
        match self.try_run(job) {
            Ok(o) => o,
            Err((kind, message)) => {
                self.logln(&format!("ERROR: {message}"));
                Outcome::Failed { kind, message }
            }
        }
    }

    fn try_run(&mut self, job: &Job) -> Result<Outcome, Fail> {
        let cache = cache_dir();
        std::fs::create_dir_all(&cache).map_err(|e| (FailKind::Other, format!("No pude crear {}: {e}", cache.display())))?;
        self.log = std::fs::File::create(cache.join("update.log")).ok();
        self.logln(&format!("Capyro {} · actualizando desde {}", crate::ffi::version(), job.repo));

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
        if self.cancelled() {
            return Err((FailKind::Other, "cancelado".into()));
        }

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
        if self.cancelled() {
            return Err((FailKind::Other, "cancelado".into()));
        }

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
        if self.cancelled() {
            return Err((FailKind::Other, "cancelado".into()));
        }

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

fn short(hash: &str) -> String {
    hash.chars().take(7).collect()
}

fn last_meaningful(out: &str) -> String {
    out.lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('→'))
        .map(|l| ellipsize(l, 90))
        .unwrap_or_else(|| "sin detalles".to_owned())
}

fn ellipsize(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let mut o: String = s.chars().take(max - 1).collect();
        o.push('…');
        o
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(normalize_repo("git@github.com:capy/Capyro.git"), "https://github.com/capy/Capyro.git");
        assert_eq!(normalize_repo("ssh://git@github.com/capy/Capyro"), "https://github.com/capy/Capyro.git");
        assert_eq!(normalize_repo("capy/Capyro"), "https://github.com/capy/Capyro.git");
        assert_eq!(normalize_repo("https://github.com/capy/Capyro.git"), "https://github.com/capy/Capyro.git");
        assert_eq!(normalize_repo("/home/capy/Capyro"), "/home/capy/Capyro");
    }

    #[test]
    fn errores_de_git() {
        assert_eq!(
            classify_git("fatal: unable to access 'https://github.com/x/y.git/': Could not resolve host: github.com"),
            FailKind::Offline
        );
        assert_eq!(classify_git("remote: Repository not found.\nfatal: repository 'x' not found"), FailKind::Repo);
        assert_eq!(
            classify_git("fatal: could not read Username for 'https://github.com': terminal prompts disabled"),
            FailKind::Repo
        );
        assert_eq!(classify_git("fatal: algo raro"), FailKind::Other);
    }

    #[test]
    fn progreso() {
        assert_eq!(progress_detail("   Compiling eframe v0.37.0"), Some("eframe".into()));
        assert_eq!(
            progress_detail("[ 40%] Building C object core/CMakeFiles/capyro_core.dir/src/player.c.o"),
            Some("player.c".into())
        );
        assert_eq!(progress_detail("[100%] Linking C executable capyro"), Some("enlazando".into()));
        assert_eq!(progress_detail("warning: algo"), None);
    }

    #[test]
    fn hace() {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        assert_eq!(ago(now - 10), "recién");
        assert_eq!(ago(now - 300), "hace 5 min");
        assert_eq!(ago(now - 3 * 86_400), "hace 3 días");
        assert_eq!(ago(now - 3600), "hace 1 hora");
        assert_eq!(ago(0), "");
    }
}

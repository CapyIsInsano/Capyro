//! Actualizador de Capyro. Instala solo lo del sistema en el que corre:
//!
//! **Linux** (`updater/unix.rs`): Capyro se instala compilándolo, así que
//! actualizarse es lo mismo que haría uno a mano, pero solo:
//!
//!   1. `git ls-remote` al repo → ¿el último commit es distinto al de este binario?
//!   2. baja el código a `~/.cache/capyro/src` (clon liviano, se reutiliza)
//!   3. `cmake -DCAPYRO_SYSTEM=Lin` + `cmake --build` en `~/.cache/capyro/build`
//!      (incremental: la primera vez tarda, las siguientes son segundos)
//!   4. `cmake --install` al mismo lugar donde está instalado (con `pkexec`
//!      si esa carpeta es del sistema)
//!
//! **Windows** (`updater/windows.rs`): no hace falta compilar nada. Se baja
//! `capyro-windows-x64.zip` de la versión para Windows publicada en el repo
//! (GitHub Releases, la arma el CI) y se reemplazan los archivos de la
//! carpeta donde está instalado.
//!
//! Todo corre en un hilo aparte; la interfaz consulta el estado en cada frame
//! con [`Updater::progress`] y recibe el resultado con [`Updater::poll`].

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::System;
use crate::ffi::BuildInfo;

// Cada sistema compila solo su forma de actualizarse.
#[cfg(any(windows, test))]
mod release;
#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as sys;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as sys;

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
// (en Windows no se compila nada: no hay Building)
#[cfg_attr(windows, allow(dead_code))]
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
    /// Falta git, cmake o cargo (Linux) o curl/tar (Windows).
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
// branch, prefix, build_type... son para compilar (Linux)
#[cfg_attr(windows, allow(dead_code))]
pub struct Job {
    /// Para qué sistema se instala (`options.system`): solo se baja,
    /// compila e instala lo de este sistema.
    pub system: System,
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
            system: System::current(),
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
/// En Linux reconoce el prefijo con el que se compiló y también uno distinto
/// puesto al instalar (`cmake --install build --prefix ~/.local`). En Windows
/// es la carpeta del .exe, si tiene la config instalada al lado.
pub fn install_location(info: &BuildInfo) -> Option<Install> {
    sys::install_location(info)
}

/// "owner/repo" de una URL de GitHub (para la API de Releases).
#[cfg(any(windows, test))]
pub fn github_slug(url: &str) -> Option<String> {
    let rest = url
        .trim()
        .strip_prefix("https://github.com/")
        .or_else(|| url.trim().strip_prefix("http://github.com/"))?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    let ok = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    (ok(owner) && ok(repo) && parts.next().is_none()).then(|| format!("{owner}/{repo}"))
}

/// Al abrir Capyro: borra lo que dejó la actualización anterior (en Windows,
/// los `.old` de los archivos que estaban en uso). En Linux no hace nada.
pub fn cleanup_after_update() {
    sys::cleanup_after_update();
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
    /// Proceso (y en Linux grupo de procesos) del comando actual, para
    /// cortarlo al cerrar.
    pid: Option<u32>,
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
                pid: None,
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
                s.pid = None;
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

    /// Al cerrar Capyro: corta lo que se esté compilando o bajando.
    pub fn cancel(&self) {
        let mut s = lock(&self.shared);
        s.cancel = true;
        if let Some(pid) = s.pid {
            sys::kill_tree(pid);
        }
    }
}

fn lock(m: &Mutex<Shared>) -> std::sync::MutexGuard<'_, Shared> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Vuelve a abrir Capyro (el ejecutable recién instalado).
pub fn relaunch(bindir: &Path) -> std::io::Result<()> {
    sys::relaunch(bindir)
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
    crate::paths::cache_dir()
}

/// `~/.cache/capyro/update.log` con `~` para mostrar.
pub fn log_path_display() -> String {
    crate::paths::tilde(&cache_dir().join("update.log"))
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
#[cfg_attr(windows, allow(dead_code))]
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
            .env("PATH", sys::child_path())
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "true")
            .env("SSH_ASKPASS", "true")
            .env("TERM", "dumb")
            .env("CARGO_TERM_COLOR", "never")
            .env("CARGO_TERM_PROGRESS_WHEN", "never")
            .env_remove("CLICOLOR_FORCE");
        // Linux: grupo de procesos propio (y prioridad baja para compilar).
        // Windows: sin ventana de consola.
        sys::prepare(&mut cmd, nice);
        let shown = format!("$ {:?} {:?}", cmd.get_program(), cmd.get_args().collect::<Vec<_>>());
        self.logln(&shown);
        let mut child = cmd.spawn().map_err(|e| (None, e.to_string()))?;
        drop(cmd); // suelta nuestras copias del extremo de escritura (si no, nunca llega el EOF)
        lock(&self.shared).pid = Some(child.id());

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
        lock(&self.shared).pid = None;
        self.logln(&format!("→ {status}"));
        if status.success() {
            Ok(all)
        } else {
            Err((status.code(), all))
        }
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
        self.logln(&format!(
            "Capyro {} · sistema {} ({}) · actualizando desde {}",
            env!("CARGO_PKG_VERSION"),
            job.system.name(),
            job.system.code(),
            job.repo
        ));
        sys::try_run(self, job, &cache)
    }

    fn check_cancel(&self) -> Result<(), Fail> {
        if self.cancelled() {
            Err((FailKind::Other, "cancelado".into()))
        } else {
            Ok(())
        }
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
    fn slug_de_github() {
        assert_eq!(github_slug("https://github.com/CapyIsInsano/Capyro.git").as_deref(), Some("CapyIsInsano/Capyro"));
        assert_eq!(github_slug("https://github.com/capy/Capyro/").as_deref(), Some("capy/Capyro"));
        assert_eq!(github_slug(&normalize_repo("git@github.com:capy/Capyro.git")).as_deref(), Some("capy/Capyro"));
        assert_eq!(github_slug("https://gitlab.com/capy/Capyro"), None);
        assert_eq!(github_slug("/home/capy/Capyro"), None);
        assert_eq!(github_slug("https://github.com/capy/Capyro/tree/main"), None);
    }

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

    /// De punta a punta contra un repo de git local: ve el commit nuevo
    /// (sin compilar ni instalar nada).
    #[cfg(unix)]
    #[test]
    fn encuentra_commits_nuevos() {
        use std::process::Command;
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let base = std::env::temp_dir().join(format!("capyro-upd-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let repo = base.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(["-c", "user.name=Capy", "-c", "user.email=capy@example.com", "-c", "commit.gpgsign=false"])
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::write(repo.join("a.txt"), "uno").unwrap();
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "Primero"]);
        let first = git(&["rev-parse", "HEAD"]);
        std::fs::write(repo.join("a.txt"), "dos").unwrap();
        git(&["commit", "-q", "-am", "Segundo: el salto a Windows"]);

        // la caché del actualizador, aparte
        // SAFETY: solo esta prueba usa XDG_CACHE_HOME (las demás no la leen).
        unsafe { std::env::set_var("XDG_CACHE_HOME", base.join("cache")) };
        let job = Job {
            system: System::current(),
            repo: repo.display().to_string(),
            branch: None,
            built_commit: first,
            installed: true,
            bindir: base.join("bin"),
            prefix: base.display().to_string(),
            build_type: "Release".into(),
            dist: false,
            native: false,
            install_now: false,
        };
        let mut up = Updater::new();
        up.start(job, || {});
        let start = Instant::now();
        let out = loop {
            if let Some(o) = up.poll() {
                break o;
            }
            assert!(start.elapsed() < Duration::from_secs(60), "el actualizador no terminó");
            std::thread::sleep(Duration::from_millis(50));
        };
        match out {
            Outcome::Available { commits, total, installable } => {
                assert_eq!(total, 1);
                assert!(installable);
                assert_eq!(commits[0].subject, "Segundo: el salto a Windows");
            }
            other => panic!("esperaba Available, vino {other:?}"),
        }
        let log = std::fs::read_to_string(base.join("cache/capyro/update.log")).unwrap();
        assert!(log.contains(&format!("sistema {} ({})", System::current().name(), System::current().code())), "{log}");
        let _ = std::fs::remove_dir_all(&base);
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

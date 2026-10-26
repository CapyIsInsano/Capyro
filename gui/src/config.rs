//! Configuración en Lua (`capyro.lua`): atajos de teclado y opciones.

use std::path::PathBuf;

use eframe::egui::{Color32, Key, Modifiers};
use mlua::{Lua, Table, Value};

/// La config por defecto va embebida, así Capyro arranca aunque no exista
/// ningún archivo en disco.
pub const DEFAULT_CONFIG: &str = include_str!("../../config/capyro.lua");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    PlayPause,
    Stop,
    SeekForward,
    SeekBackward,
    VolumeUp,
    VolumeDown,
    Mute,
    NextFile,
    PrevFile,
    TogglePlaylist,
    Fullscreen,
    OpenFiles,
    ToggleShuffle,
    CycleRepeat,
    SpeedUp,
    SpeedDown,
    SpeedReset,
    Screenshot,
    ToggleEffects,
    EffectsBypass,
    Quit,
}

/// Backend de ventana en Linux. Con Wayland nativo winit todavía no recibe
/// archivos arrastrados; con X11 (vía XWayland) sí.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Auto,
    X11,
    Wayland,
}

impl Action {
    /// Todas, en el orden en que aparecen en el editor y en capyro.lua.
    pub const ALL: [Action; 21] = [
        Action::PlayPause,
        Action::Stop,
        Action::SeekForward,
        Action::SeekBackward,
        Action::VolumeUp,
        Action::VolumeDown,
        Action::Mute,
        Action::NextFile,
        Action::PrevFile,
        Action::TogglePlaylist,
        Action::Fullscreen,
        Action::OpenFiles,
        Action::ToggleShuffle,
        Action::CycleRepeat,
        Action::SpeedDown,
        Action::SpeedUp,
        Action::SpeedReset,
        Action::Screenshot,
        Action::ToggleEffects,
        Action::EffectsBypass,
        Action::Quit,
    ];

    /// Nombre en capyro.lua.
    pub fn name(self) -> &'static str {
        match self {
            Self::PlayPause => "play_pause",
            Self::Stop => "stop",
            Self::SeekForward => "seek_forward",
            Self::SeekBackward => "seek_backward",
            Self::VolumeUp => "volume_up",
            Self::VolumeDown => "volume_down",
            Self::Mute => "mute",
            Self::NextFile => "next_file",
            Self::PrevFile => "prev_file",
            Self::TogglePlaylist => "toggle_playlist",
            Self::Fullscreen => "fullscreen",
            Self::OpenFiles => "open_files",
            Self::ToggleShuffle => "toggle_shuffle",
            Self::CycleRepeat => "cycle_repeat",
            Self::SpeedUp => "speed_up",
            Self::SpeedDown => "speed_down",
            Self::SpeedReset => "speed_reset",
            Self::Screenshot => "screenshot",
            Self::ToggleEffects => "toggle_effects",
            Self::EffectsBypass => "effects_bypass",
            Self::Quit => "quit",
        }
    }

    /// Lo que se ve en el editor de atajos.
    pub fn label(self) -> &'static str {
        match self {
            Self::PlayPause => "Play / pausa",
            Self::Stop => "Detener",
            Self::SeekForward => "Adelantar",
            Self::SeekBackward => "Retroceder",
            Self::VolumeUp => "Subir volumen",
            Self::VolumeDown => "Bajar volumen",
            Self::Mute => "Silenciar",
            Self::NextFile => "Siguiente",
            Self::PrevFile => "Anterior",
            Self::TogglePlaylist => "Mostrar la lista",
            Self::Fullscreen => "Pantalla completa",
            Self::OpenFiles => "Agregar archivos",
            Self::ToggleShuffle => "Aleatorio",
            Self::CycleRepeat => "Repetir",
            Self::SpeedUp => "Más velocidad",
            Self::SpeedDown => "Menos velocidad",
            Self::SpeedReset => "Velocidad normal",
            Self::Screenshot => "Capturar fotograma",
            Self::ToggleEffects => "Efectos de audio",
            Self::EffectsBypass => "Prender / apagar efectos",
            Self::Quit => "Salir",
        }
    }

    fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "play_pause" => Self::PlayPause,
            "stop" => Self::Stop,
            "seek_forward" => Self::SeekForward,
            "seek_backward" => Self::SeekBackward,
            "volume_up" => Self::VolumeUp,
            "volume_down" => Self::VolumeDown,
            "mute" => Self::Mute,
            "next_file" => Self::NextFile,
            "prev_file" => Self::PrevFile,
            "toggle_playlist" => Self::TogglePlaylist,
            "fullscreen" => Self::Fullscreen,
            "open_files" => Self::OpenFiles,
            "toggle_shuffle" => Self::ToggleShuffle,
            "cycle_repeat" => Self::CycleRepeat,
            "speed_up" => Self::SpeedUp,
            "speed_down" => Self::SpeedDown,
            "speed_reset" => Self::SpeedReset,
            "screenshot" => Self::Screenshot,
            "toggle_effects" => Self::ToggleEffects,
            "effects_bypass" => Self::EffectsBypass,
            "quit" => Self::Quit,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Binding {
    pub modifiers: Modifiers,
    pub key: Key,
    pub action: Action,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bindings: Vec<Binding>,
    pub seek_step: f64,
    pub volume_step: f32,
    /// Paso por muesca de la rueda sobre el ícono de volumen.
    pub wheel_step: f32,
    pub start_volume: f32,
    pub accent: Color32,
    pub auto_next: bool,
    pub backend: Backend,
    /// Recordar lista, posición, volumen... entre una vez y otra.
    pub remember_session: bool,
    /// Barras de frecuencia de fondo mientras suena un audio.
    pub visualizer: bool,
    /// Carpeta de las capturas (vacío = la de Imágenes del sistema + /Capyro).
    pub screenshot_dir: Option<std::path::PathBuf>,
    /// Valores con los que arranca Capyro.
    pub start_speed: f64,
    pub start_shuffle: bool,
    pub start_repeat: crate::ffi::Repeat,
    /// Pantalla de "Cargando" al abrir, y el sonido de inicio.
    pub splash: bool,
    pub startup_sound: bool,
    /// Repo y rama de donde se bajan las actualizaciones (vacío = de donde se compiló).
    pub update_repo: Option<String>,
    pub update_branch: Option<String>,
    /// Decodificación de video: automático, GPU o CPU.
    pub decoding: crate::ffi::Hwdec,
    /// De dónde salió (para el log): ruta o "embebida".
    pub source: String,
}

impl Config {
    /// Busca y carga `capyro.lua`. Nunca falla: ante cualquier problema
    /// avisa por stderr y usa la configuración embebida.
    pub fn load() -> Self {
        for path in candidate_paths() {
            let Ok(src) = std::fs::read_to_string(&path) else {
                continue;
            };
            let name = path.display().to_string();
            match parse(&src, &name) {
                Ok(cfg) => return cfg,
                Err(e) => {
                    eprintln!("capyro: error en {name}: {e}\ncapyro: uso la configuración por defecto");
                    break;
                }
            }
        }
        parse(DEFAULT_CONFIG, "embebida").expect("la config embebida tiene que ser válida")
    }
}

/* ------------------------------------------------------------------------ */
/* guardar (editor de atajos)                                               */
/* ------------------------------------------------------------------------ */

/// Donde se guarda lo que se edita desde Capyro: `CAPYRO_CONFIG` si está,
/// si no `~/.config/capyro/capyro.lua` (el primero que se busca al abrir).
/// Nunca el de ejemplo del repo ni el de /usr/share.
pub fn user_config_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CAPYRO_CONFIG") {
        return Some(PathBuf::from(p));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("capyro").join("capyro.lua"))
}

/// "Ctrl+Shift+Right": como se escribe en capyro.lua.
pub fn key_spec(m: Modifiers, key: Key) -> String {
    let mut s = String::new();
    if m.command || m.ctrl {
        s += "Ctrl+";
    }
    if m.shift {
        s += "Shift+";
    }
    if m.alt {
        s += "Alt+";
    }
    s += match key {
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        other => other.name(),
    };
    s
}

/// "Ctrl+→", "Espacio": como se muestra en pantalla.
pub fn key_label(m: Modifiers, key: Key) -> String {
    let k = match key {
        Key::Space => "Espacio",
        Key::ArrowRight => "→",
        Key::ArrowLeft => "←",
        Key::ArrowUp => "↑",
        Key::ArrowDown => "↓",
        Key::Backspace => "⌫",
        Key::Escape => "Esc",
        Key::Enter => "Enter",
        Key::Tab => "Tab",
        Key::PageUp => "RePág",
        Key::PageDown => "AvPág",
        Key::Home => "Inicio",
        Key::End => "Fin",
        Key::Delete => "Supr",
        Key::Insert => "Insert",
        other => other.symbol_or_name(),
    };
    let mut s = String::new();
    if m.command || m.ctrl {
        s += "Ctrl+";
    }
    if m.shift {
        s += "Shift+";
    }
    if m.alt {
        s += "Alt+";
    }
    s + k
}

/// Ctrl y Command son la misma tecla en Linux: se guarda como COMMAND.
pub fn normalize_modifiers(m: Modifiers) -> Modifiers {
    let mut out = Modifiers::NONE;
    if m.ctrl || m.command {
        out = out | Modifiers::COMMAND;
    }
    if m.shift {
        out = out | Modifiers::SHIFT;
    }
    if m.alt {
        out = out | Modifiers::ALT;
    }
    out
}

/// Los atajos de la config por defecto (para "Restablecer").
pub fn default_bindings() -> Vec<Binding> {
    parse(DEFAULT_CONFIG, "embebida").map(|c| c.bindings).unwrap_or_default()
}

fn lua_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let t = format!("{v:.2}");
        t.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

impl Config {
    /// El archivo entero, armado sobre el de ejemplo (con todos sus
    /// comentarios) pero con los atajos y las opciones de esta config.
    pub fn to_lua(&self) -> String {
        let home = std::env::var("HOME").unwrap_or_default();
        let option_value = |name: &str| -> Option<String> {
            Some(match name {
                "seek_step" => fmt_num(self.seek_step),
                "volume_step" => fmt_num((self.volume_step as f64 * 100.0).round()),
                "wheel_step" => fmt_num((self.wheel_step as f64 * 100.0).round()),
                "start_volume" => fmt_num((self.start_volume as f64 * 100.0).round()),
                "auto_next" => self.auto_next.to_string(),
                "speed" => {
                    let t = fmt_num(self.start_speed);
                    if t.contains('.') { t } else { format!("{t}.0") }
                }
                "shuffle" => self.start_shuffle.to_string(),
                "[\"repeat\"]" => lua_quote(match self.start_repeat {
                    crate::ffi::Repeat::Off => "off",
                    crate::ffi::Repeat::All => "all",
                    crate::ffi::Repeat::One => "one",
                }),
                "accent" => lua_quote(&format!("#{:02X}{:02X}{:02X}", self.accent.r(), self.accent.g(), self.accent.b())),
                "remember_session" => self.remember_session.to_string(),
                "visualizer" => self.visualizer.to_string(),
                "screenshot_dir" => {
                    let p = self.screenshot_dir.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                    let p = match p.strip_prefix(&home) {
                        Some(rest) if !home.is_empty() && rest.starts_with('/') => format!("~{rest}"),
                        _ => p,
                    };
                    lua_quote(&p)
                }
                "splash" => self.splash.to_string(),
                "startup_sound" => self.startup_sound.to_string(),
                "update_repo" => lua_quote(self.update_repo.as_deref().unwrap_or("")),
                "update_branch" => lua_quote(self.update_branch.as_deref().unwrap_or("")),
                "decoding" => lua_quote(self.decoding.name()),
                "backend" => lua_quote(match self.backend {
                    Backend::Auto => "auto",
                    Backend::X11 => "x11",
                    Backend::Wayland => "wayland",
                }),
                _ => return None,
            })
        };

        let mut out = String::from("-- Guardado desde Capyro (⚙ → Editar configuración).\n");
        let mut section = "";
        for line in DEFAULT_CONFIG.lines() {
            let t = line.trim_start();
            if t.starts_with("local keys") {
                section = "keys";
                out += "local keys = {\n";
                for action in Action::ALL {
                    let specs: Vec<String> = self
                        .bindings
                        .iter()
                        .filter(|b| b.action == action)
                        .map(|b| lua_quote(&key_spec(b.modifiers, b.key)))
                        .collect();
                    if specs.is_empty() {
                        continue;
                    }
                    let value = if specs.len() == 1 { specs[0].clone() } else { format!("{{ {} }}", specs.join(", ")) };
                    out += &format!("    {:<15} = {},\n", action.name(), value);
                }
                continue;
            }
            if section == "keys" {
                if line.starts_with('}') {
                    section = "";
                    out += "}\n";
                }
                continue;
            }
            if t.starts_with("local options") {
                section = "options";
            } else if section == "options" && line.starts_with('}') {
                section = "";
            }
            if section == "options" && !t.starts_with("--") {
                if let Some(eq) = line.find('=') {
                    let name = line[..eq].trim();
                    if let Some(v) = option_value(name) {
                        // la coma que cierra el valor (fuera de comillas)
                        let rest = &line[eq + 1..];
                        let mut in_str = false;
                        let mut end = None;
                        for (i, c) in rest.char_indices() {
                            match c {
                                '"' => in_str = !in_str,
                                ',' if !in_str => {
                                    end = Some(i);
                                    break;
                                }
                                _ => {}
                            }
                        }
                        if let Some(end) = end {
                            let lead = &rest[..rest.len() - rest.trim_start().len()];
                            let tail = &rest[end + 1..];
                            // la columna de los comentarios queda donde estaba
                            let old_len = rest[..end].trim().chars().count();
                            let pad = old_len.saturating_sub(v.chars().count());
                            out += &format!("{}={}{},{}{}\n", &line[..eq], lead, v, " ".repeat(pad), tail);
                            continue;
                        }
                    }
                }
            }
            out += line;
            out += "\n";
        }
        out
    }

    /// Guarda en [`user_config_path`] (con copia `.bak` del anterior).
    pub fn save(&mut self) -> std::io::Result<PathBuf> {
        let path = user_config_path().ok_or_else(|| std::io::Error::other("no hay HOME"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        if path.exists() {
            let _ = std::fs::copy(&path, path.with_extension("lua.bak"));
        }
        let text = self.to_lua();
        // que lo que se guarda se pueda volver a leer
        parse(&text, "nueva").map_err(|e| std::io::Error::other(e.to_string()))?;
        let tmp = path.with_extension("lua.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &path)?;
        self.source = path.display().to_string();
        Ok(path)
    }

    /// Asigna una tecla a una acción. Si otra acción la usaba, se la saca y
    /// la devuelve (para avisar).
    pub fn assign(&mut self, action: Action, modifiers: Modifiers, key: Key) -> Option<Action> {
        let modifiers = normalize_modifiers(modifiers);
        let mut stolen = None;
        self.bindings.retain(|b| {
            let same = b.key == key && normalize_modifiers(b.modifiers) == modifiers;
            if same && b.action != action {
                stolen = Some(b.action);
            }
            !same
        });
        self.bindings.push(Binding { modifiers, key, action });
        self.sort_bindings();
        stolen
    }

    pub fn sort_bindings(&mut self) {
        self.bindings.sort_by_key(|b| std::cmp::Reverse(modifier_count(b.modifiers)));
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os("CAPYRO_CONFIG") {
        out.push(PathBuf::from(p));
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        out.push(PathBuf::from(xdg).join("capyro/capyro.lua"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        out.push(PathBuf::from(home).join(".config/capyro/capyro.lua"));
    }
    out.push(PathBuf::from("config/capyro.lua"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join("config/capyro.lua"));
            out.push(dir.join("../share/capyro/capyro.lua"));
        }
    }
    out
}

fn parse(src: &str, name: &str) -> mlua::Result<Config> {
    let lua = Lua::new();
    let root: Table = lua.load(src).set_name(name).eval()?;

    let mut cfg = Config {
        bindings: Vec::new(),
        seek_step: 5.0,
        volume_step: 0.05,
        wheel_step: 0.05,
        start_volume: 1.0,
        accent: Color32::from_rgb(0xFF, 0x74, 0x17),
        auto_next: true,
        backend: Backend::Auto,
        remember_session: true,
        visualizer: true,
        screenshot_dir: None,
        start_speed: 1.0,
        start_shuffle: false,
        start_repeat: crate::ffi::Repeat::Off,
        splash: true,
        startup_sound: true,
        update_repo: None,
        update_branch: None,
        decoding: crate::ffi::Hwdec::Auto,
        source: name.to_owned(),
    };

    if let Some(keys) = root.get::<Option<Table>>("keys")? {
        for pair in keys.pairs::<String, Value>() {
            let (action_name, value) = pair?;
            let Some(action) = Action::from_name(&action_name) else {
                eprintln!("capyro: {name}: acción desconocida '{action_name}'");
                continue;
            };
            for spec in key_specs(&value)? {
                match parse_key_spec(&spec) {
                    Some((modifiers, key)) => cfg.bindings.push(Binding { modifiers, key, action }),
                    None => eprintln!("capyro: {name}: tecla inválida '{spec}' en {action_name}"),
                }
            }
        }
    }

    if let Some(opts) = root.get::<Option<Table>>("options")? {
        if let Some(v) = opts.get::<Option<f64>>("seek_step")? {
            cfg.seek_step = v.max(0.1);
        }
        if let Some(v) = opts.get::<Option<f64>>("volume_step")? {
            cfg.volume_step = (v as f32 / 100.0).clamp(0.01, 1.0);
        }
        if let Some(v) = opts.get::<Option<f64>>("wheel_step")? {
            cfg.wheel_step = (v as f32 / 100.0).clamp(0.01, 0.25);
        }
        if let Some(v) = opts.get::<Option<f64>>("start_volume")? {
            cfg.start_volume = (v as f32 / 100.0).clamp(0.0, 1.0);
        }
        if let Some(b) = opts.get::<Option<String>>("backend")? {
            cfg.backend = match b.to_ascii_lowercase().as_str() {
                "x11" => Backend::X11,
                "wayland" => Backend::Wayland,
                "auto" => Backend::Auto,
                other => {
                    eprintln!("capyro: {name}: backend desconocido '{other}' (auto, x11 o wayland)");
                    Backend::Auto
                }
            };
        }
        if let Some(v) = opts.get::<Option<f64>>("speed")? {
            cfg.start_speed = v.clamp(0.5, 2.0);
        }
        if let Some(v) = opts.get::<Option<bool>>("shuffle")? {
            cfg.start_shuffle = v;
        }
        if let Some(r) = opts.get::<Option<String>>("repeat")? {
            cfg.start_repeat = match r.to_ascii_lowercase().as_str() {
                "all" | "todo" | "lista" => crate::ffi::Repeat::All,
                "one" | "uno" => crate::ffi::Repeat::One,
                "off" | "no" => crate::ffi::Repeat::Off,
                other => {
                    eprintln!("capyro: {name}: repeat desconocido '{other}' (off, all u one)");
                    crate::ffi::Repeat::Off
                }
            };
        }
        if let Some(v) = opts.get::<Option<bool>>("remember_session")? {
            cfg.remember_session = v;
        }
        if let Some(v) = opts.get::<Option<bool>>("visualizer")? {
            cfg.visualizer = v;
        }
        if let Some(v) = opts.get::<Option<String>>("screenshot_dir")? {
            let v = v.trim();
            if !v.is_empty() {
                let expanded = match (v.strip_prefix("~/"), std::env::var_os("HOME")) {
                    (Some(rest), Some(home)) => std::path::PathBuf::from(home).join(rest),
                    _ => std::path::PathBuf::from(v),
                };
                cfg.screenshot_dir = Some(expanded);
            }
        }
        if let Some(v) = opts.get::<Option<bool>>("splash")? {
            cfg.splash = v;
        }
        if let Some(v) = opts.get::<Option<bool>>("startup_sound")? {
            cfg.startup_sound = v;
        }
        if let Some(v) = opts.get::<Option<String>>("update_repo")? {
            cfg.update_repo = Some(v.trim().to_owned()).filter(|v| !v.is_empty());
        }
        if let Some(v) = opts.get::<Option<String>>("update_branch")? {
            cfg.update_branch = Some(v.trim().to_owned()).filter(|v| !v.is_empty());
        }
        if let Some(v) = opts.get::<Option<String>>("decoding")? {
            cfg.decoding = match v.trim().to_ascii_lowercase().as_str() {
                "gpu" | "hw" | "hardware" => crate::ffi::Hwdec::Gpu,
                "cpu" | "sw" | "software" => crate::ffi::Hwdec::Cpu,
                "auto" | "automatico" | "automático" | "" => crate::ffi::Hwdec::Auto,
                other => {
                    eprintln!("capyro: {name}: decoding desconocido '{other}' (auto, gpu o cpu)");
                    crate::ffi::Hwdec::Auto
                }
            };
        }
        if let Some(v) = opts.get::<Option<bool>>("auto_next")? {
            cfg.auto_next = v;
        }
        if let Some(hex) = opts.get::<Option<String>>("accent")? {
            match parse_hex_color(&hex) {
                Some(c) => cfg.accent = c,
                None => eprintln!("capyro: {name}: color inválido '{hex}' (usá \"#RRGGBB\")"),
            }
        }
    }

    // Primero los atajos con más modificadores, así "Ctrl+L" gana sobre "L".
    cfg.bindings
        .sort_by_key(|b| std::cmp::Reverse(modifier_count(b.modifiers)));
    Ok(cfg)
}

fn key_specs(value: &Value) -> mlua::Result<Vec<String>> {
    Ok(match value {
        Value::String(s) => vec![s.to_string_lossy()],
        Value::Table(t) => t
            .sequence_values::<String>()
            .collect::<mlua::Result<Vec<_>>>()?,
        _ => Vec::new(),
    })
}

fn modifier_count(m: Modifiers) -> u8 {
    m.alt as u8 + m.shift as u8 + m.command as u8 + m.ctrl as u8
}

/// "Ctrl+Shift+Right" → (CTRL|SHIFT, ArrowRight)
pub fn parse_key_spec(spec: &str) -> Option<(Modifiers, Key)> {
    let mut modifiers = Modifiers::NONE;
    let mut parts: Vec<&str> = spec.split('+').map(str::trim).collect();
    // "Ctrl++" o "Plus": la tecla '+' sola
    if spec.ends_with("++") {
        parts.retain(|p| !p.is_empty());
        parts.push("Plus");
    }
    let key_name = parts.pop()?;
    for m in parts {
        match m.to_ascii_lowercase().as_str() {
            // COMMAND = la tecla Ctrl en Linux
            "ctrl" | "control" | "cmd" | "command" => modifiers = modifiers | Modifiers::COMMAND,
            "shift" => modifiers = modifiers | Modifiers::SHIFT,
            "alt" | "option" => modifiers = modifiers | Modifiers::ALT,
            _ => return None,
        }
    }
    Some((modifiers, Key::from_name(key_name)?))
}

fn parse_hex_color(s: &str) -> Option<Color32> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let cfg = parse(DEFAULT_CONFIG, "embebida").unwrap();
        assert!(cfg.bindings.len() >= 21);
        assert!(cfg.remember_session && cfg.visualizer);
        assert!(cfg.splash && cfg.startup_sound);
        assert_eq!(cfg.update_repo, None);
        assert_eq!(cfg.backend, Backend::Auto);
        assert_eq!(cfg.seek_step, 5.0);
        // Ctrl+L tiene que quedar antes que cualquier binding sin modificadores
        assert!(!cfg.bindings.first().unwrap().modifiers.is_none());
    }

    #[test]
    fn key_specs() {
        assert_eq!(parse_key_spec("Space"), Some((Modifiers::NONE, Key::Space)));
        assert_eq!(parse_key_spec("ctrl+q"), Some((Modifiers::COMMAND, Key::Q)));
        assert_eq!(
            parse_key_spec("Shift+Right"),
            Some((Modifiers::SHIFT, Key::ArrowRight))
        );
        assert_eq!(parse_key_spec("Hyper+X"), None);
        assert_eq!(parse_key_spec("NoExiste"), None);
    }

    #[test]
    fn guardar_y_volver_a_leer() {
        let mut cfg = parse(DEFAULT_CONFIG, "embebida").unwrap();
        cfg.bindings.retain(|b| b.action != Action::Stop);
        assert_eq!(cfg.assign(Action::Mute, Modifiers::CTRL, Key::M), None);
        // Ctrl+L era de la lista: pasa a silenciar
        assert_eq!(cfg.assign(Action::Mute, Modifiers::COMMAND, Key::L), Some(Action::TogglePlaylist));
        cfg.seek_step = 7.5;
        cfg.accent = Color32::from_rgb(0x12, 0xAB, 0xEF);
        cfg.update_repo = Some("CapyIsInsano/Capyro".into());
        cfg.decoding = crate::ffi::Hwdec::Cpu;
        let text = cfg.to_lua();
        let back = parse(&text, "prueba").unwrap();
        assert!(!back.bindings.iter().any(|b| b.action == Action::Stop));
        assert!(!back.bindings.iter().any(|b| b.action == Action::TogglePlaylist));
        let mute: Vec<String> = back
            .bindings
            .iter()
            .filter(|b| b.action == Action::Mute)
            .map(|b| key_spec(b.modifiers, b.key))
            .collect();
        assert!(mute.contains(&"M".to_owned()) && mute.contains(&"Ctrl+M".to_owned()) && mute.contains(&"Ctrl+L".to_owned()));
        assert_eq!(back.seek_step, 7.5);
        assert_eq!(back.accent, Color32::from_rgb(0x12, 0xAB, 0xEF));
        assert_eq!(back.update_repo.as_deref(), Some("CapyIsInsano/Capyro"));
        assert_eq!(back.decoding, crate::ffi::Hwdec::Cpu);
        assert_eq!(back.splash, cfg.splash);
        // los comentarios del ejemplo siguen ahí
        assert!(text.contains("-- segundos que avanza/retrocede"));
    }

    #[test]
    fn nombres_de_teclas() {
        for action in Action::ALL {
            assert_eq!(Action::from_name(action.name()), Some(action));
        }
        for key in [Key::Space, Key::ArrowRight, Key::OpenBracket, Key::Backspace, Key::F11, Key::Comma, Key::Num5] {
            let spec = key_spec(Modifiers::COMMAND | Modifiers::SHIFT, key);
            assert_eq!(parse_key_spec(&spec), Some((Modifiers::COMMAND | Modifiers::SHIFT, key)), "{spec}");
        }
        assert_eq!(key_label(Modifiers::COMMAND, Key::ArrowRight), "Ctrl+→");
    }

    #[test]
    fn hex() {
        assert_eq!(parse_hex_color("#ff7417"), Some(Color32::from_rgb(255, 116, 23)));
        assert_eq!(parse_hex_color("nope"), None);
    }
}

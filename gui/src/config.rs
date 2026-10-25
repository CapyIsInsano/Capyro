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
    fn hex() {
        assert_eq!(parse_hex_color("#ff7417"), Some(Color32::from_rgb(255, 116, 23)));
        assert_eq!(parse_hex_color("nope"), None);
    }
}

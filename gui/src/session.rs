//! Sesión: lo que había abierto al cerrar Capyro (lista, archivo, posición,
//! volumen...) para retomarlo la próxima vez.
//!
//! Se guarda como Lua en `$XDG_STATE_HOME/capyro/session.lua`
//! (normalmente `~/.local/state/capyro/session.lua`; en Windows
//! `%LOCALAPPDATA%\Capyro\state\session.lua`) y se lee con el mismo
//! intérprete que la configuración.

use std::path::PathBuf;

use mlua::{Lua, Table};

use crate::ffi::{AudioFx, Repeat};

#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub volume: f32,
    pub muted: bool,
    pub speed: f64,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub show_remaining: bool,
    pub playlist: Vec<String>,
    pub current: Option<usize>,
    pub position: f64,
    pub fx: AudioFx,
    pub pitch: f64,
    /// Versión cuyas novedades ya se abrieron (para el puntito del botón).
    pub seen_changes: String,
}

pub fn path() -> Option<PathBuf> {
    crate::paths::state_dir().map(|d| d.join("session.lua"))
}

fn repeat_str(r: Repeat) -> &'static str {
    match r {
        Repeat::Off => "off",
        Repeat::All => "all",
        Repeat::One => "one",
    }
}

/// Cadena de Lua entre comillas, con los escapes necesarios.
fn lua_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl Session {
    pub fn to_lua(&self) -> String {
        let mut s = String::from("-- Sesión de Capyro (se reescribe sola; podés borrarla sin problema)\nreturn {\n");
        s += &format!("    volume = {:.3},\n", self.volume);
        s += &format!("    muted = {},\n", self.muted);
        s += &format!("    speed = {:.2},\n", self.speed);
        s += &format!("    shuffle = {},\n", self.shuffle);
        s += &format!("    [\"repeat\"] = \"{}\",\n", repeat_str(self.repeat));
        s += &format!("    show_remaining = {},\n", self.show_remaining);
        if let Some(c) = self.current {
            s += &format!("    current = {},\n", c + 1); // Lua cuenta desde 1
        }
        s += &format!("    position = {:.2},\n", self.position);
        let f = &self.fx;
        s += &format!(
            "    fx = {{ enabled = {}, bass = {:.1}, mid = {:.1}, treble = {:.1}, width = {:.2}, night = {}, pitch = {:.0} }},\n",
            f.enabled, f.bass_db, f.mid_db, f.treble_db, f.width, f.night, self.pitch
        );
        let bands: Vec<String> = f.eq15.iter().map(|g| format!("{g:.1}")).collect();
        s += &format!("    eq15 = {{ on = {}, bands = {{ {} }} }},\n", f.eq15_on, bands.join(", "));
        s += &format!("    seen_changes = {},\n", lua_quote(&self.seen_changes));
        s += "    playlist = {\n";
        for p in &self.playlist {
            s += &format!("        {},\n", lua_quote(p));
        }
        s += "    },\n}\n";
        s
    }

    pub fn from_lua(src: &str) -> mlua::Result<Self> {
        let lua = Lua::new();
        let t: Table = lua.load(src).set_name("session.lua").eval()?;
        let playlist: Vec<String> = match t.get::<Option<Table>>("playlist")? {
            Some(list) => list.sequence_values::<String>().collect::<mlua::Result<_>>()?,
            None => Vec::new(),
        };
        let current = t
            .get::<Option<i64>>("current")?
            .and_then(|c| usize::try_from(c - 1).ok())
            .filter(|&c| c < playlist.len());
        let mut fx = AudioFx::default();
        let mut pitch = 0.0;
        if let Some(t) = t.get::<Option<Table>>("fx")? {
            fx.enabled = t.get::<Option<bool>>("enabled")?.unwrap_or(true);
            fx.bass_db = t.get::<Option<f32>>("bass")?.unwrap_or(0.0).clamp(-12.0, 12.0);
            fx.mid_db = t.get::<Option<f32>>("mid")?.unwrap_or(0.0).clamp(-12.0, 12.0);
            fx.treble_db = t.get::<Option<f32>>("treble")?.unwrap_or(0.0).clamp(-12.0, 12.0);
            fx.width = t.get::<Option<f32>>("width")?.unwrap_or(1.0).clamp(0.0, 2.0);
            fx.night = t.get::<Option<bool>>("night")?.unwrap_or(false);
            pitch = t.get::<Option<f64>>("pitch")?.unwrap_or(0.0).clamp(-12.0, 12.0).round();
        }
        // ecualizador de 15 bandas (Capyro Studio); las sesiones viejas no lo tienen
        if let Some(e) = t.get::<Option<Table>>("eq15")? {
            fx.eq15_on = e.get::<Option<bool>>("on")?.unwrap_or(false);
            if let Some(b) = e.get::<Option<Table>>("bands")? {
                for (i, slot) in fx.eq15.iter_mut().enumerate() {
                    if let Some(g) = b.get::<Option<f32>>((i + 1) as i64)? {
                        *slot = g.clamp(-12.0, 12.0);
                    }
                }
            }
        }
        Ok(Self {
            fx,
            pitch,
            volume: t.get::<Option<f32>>("volume")?.unwrap_or(1.0).clamp(0.0, 1.0),
            muted: t.get::<Option<bool>>("muted")?.unwrap_or(false),
            speed: t.get::<Option<f64>>("speed")?.unwrap_or(1.0).clamp(0.5, 2.0),
            shuffle: t.get::<Option<bool>>("shuffle")?.unwrap_or(false),
            repeat: match t.get::<Option<String>>("repeat")?.as_deref() {
                Some("all") => Repeat::All,
                Some("one") => Repeat::One,
                _ => Repeat::Off,
            },
            show_remaining: t.get::<Option<bool>>("show_remaining")?.unwrap_or(false),
            playlist,
            current,
            position: t.get::<Option<f64>>("position")?.unwrap_or(0.0).max(0.0),
            seen_changes: t.get::<Option<String>>("seen_changes")?.unwrap_or_default(),
        })
    }

    pub fn load() -> Option<Self> {
        let src = std::fs::read_to_string(path()?).ok()?;
        match Self::from_lua(&src) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("capyro: no pude leer la sesión guardada: {e}");
                None
            }
        }
    }

    /// Escribe a un archivo temporal y lo renombra: nunca queda a medias.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = path() else { return Ok(()) };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("lua.tmp");
        std::fs::write(&tmp, self.to_lua())?;
        std::fs::rename(&tmp, &path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta() {
        let s = Session {
            volume: 0.42,
            muted: true,
            speed: 1.25,
            shuffle: true,
            repeat: Repeat::One,
            show_remaining: true,
            playlist: vec![
                "/música/Capy \"en vivo\".mp3".to_owned(),
                "C:\\raro\\nombre\nde archivo.ogg".to_owned(),
            ],
            current: Some(1),
            position: 83.5,
            fx: AudioFx {
                enabled: false,
                bass_db: 6.5,
                mid_db: -3.0,
                treble_db: 2.0,
                width: 0.4,
                night: true,
                eq15_on: true,
                eq15: [1.5, 0.0, -2.0, 0.0, 0.0, 3.0, 0.0, 0.0, -1.0, 0.0, 0.0, 2.5, 0.0, 0.0, -4.0],
                ..AudioFx::default()
            },
            pitch: -3.0,
            seen_changes: "v0.2.8-alpha".to_owned(),
        };
        let back = Session::from_lua(&s.to_lua()).unwrap();
        assert_eq!(back, Session { volume: 0.42, ..s.clone() });
    }

    #[test]
    fn current_fuera_de_rango_se_ignora() {
        let s = Session::from_lua("return { playlist = { \"a.mp3\" }, current = 7 }").unwrap();
        assert_eq!(s.current, None);
        assert_eq!(s.playlist.len(), 1);
        assert_eq!(s.fx, AudioFx::default()); // sesiones viejas (sin fx) siguen andando
    }
}

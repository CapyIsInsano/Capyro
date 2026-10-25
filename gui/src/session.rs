//! Sesión: lo que había abierto al cerrar Capyro (lista, archivo, posición,
//! volumen...) para retomarlo la próxima vez.
//!
//! Se guarda como Lua en `$XDG_STATE_HOME/capyro/session.lua`
//! (normalmente `~/.local/state/capyro/session.lua`) y se lee con el mismo
//! intérprete que la configuración.

use std::path::PathBuf;

use mlua::{Lua, Table};

use crate::ffi::Repeat;

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
}

pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("capyro").join("session.lua"))
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
        Ok(Self {
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
        };
        let back = Session::from_lua(&s.to_lua()).unwrap();
        assert_eq!(back, Session { volume: 0.42, ..s });
    }

    #[test]
    fn current_fuera_de_rango_se_ignora() {
        let s = Session::from_lua("return { playlist = { \"a.mp3\" }, current = 7 }").unwrap();
        assert_eq!(s.current, None);
        assert_eq!(s.playlist.len(), 1);
    }
}

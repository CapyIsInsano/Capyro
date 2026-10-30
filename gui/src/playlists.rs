//! Playlists guardadas (por ahora, hasta 3).
//!
//! La "Cola" de siempre sigue siendo temporal: lo que agregás para escuchar
//! ahora. Las playlists quedan guardadas en
//! `$XDG_DATA_HOME/capyro/playlists.lua` (normalmente
//! `~/.local/share/capyro/playlists.lua`; en Windows
//! `Documentos\Capyro\playlists.lua`) y se cargan en la cola para sonar.

use std::path::PathBuf;

use mlua::{Lua, Table};

/// Cuántas playlists se pueden tener a la vez.
pub const MAX_PLAYLISTS: usize = 3;
/// Largo máximo del nombre.
pub const MAX_NAME: usize = 24;

#[derive(Debug, Clone, PartialEq)]
pub struct Playlist {
    pub name: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Playlists {
    pub lists: Vec<Playlist>,
}

pub fn path() -> Option<PathBuf> {
    crate::paths::data_dir().map(|d| d.join("playlists.lua"))
}

fn lua_quote(s: &str) -> String {
    let mut out = String::from("\"");
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

/// Nombre limpio: sin saltos de línea, sin espacios de más y no muy largo.
pub fn clean_name(name: &str) -> String {
    let joined = name.split_whitespace().collect::<Vec<_>>().join(" ");
    joined.chars().take(MAX_NAME).collect()
}

impl Playlists {
    pub fn is_full(&self) -> bool {
        self.lists.len() >= MAX_PLAYLISTS
    }

    /// Crea una playlist vacía con un nombre libre ("Playlist 1", "Playlist 2"...).
    /// Devuelve su índice, o `None` si ya hay [`MAX_PLAYLISTS`].
    pub fn create(&mut self) -> Option<usize> {
        if self.is_full() {
            return None;
        }
        let name = (1..)
            .map(|n| format!("Playlist {n}"))
            .find(|n| !self.lists.iter().any(|p| &p.name == n))
            .unwrap_or_default();
        self.lists.push(Playlist { name, files: Vec::new() });
        Some(self.lists.len() - 1)
    }

    /// Agrega archivos (sin repetir los que ya están). Devuelve cuántos entraron.
    pub fn add(&mut self, idx: usize, files: &[String]) -> usize {
        let Some(pl) = self.lists.get_mut(idx) else { return 0 };
        let mut added = 0;
        for f in files {
            if !pl.files.contains(f) {
                pl.files.push(f.clone());
                added += 1;
            }
        }
        added
    }

    pub fn remove_file(&mut self, idx: usize, file: usize) {
        if let Some(pl) = self.lists.get_mut(idx) {
            if file < pl.files.len() {
                pl.files.remove(file);
            }
        }
    }

    pub fn rename(&mut self, idx: usize, name: &str) -> bool {
        let name = clean_name(name);
        match self.lists.get_mut(idx) {
            Some(pl) if !name.is_empty() => {
                pl.name = name;
                true
            }
            _ => false,
        }
    }

    pub fn delete(&mut self, idx: usize) {
        if idx < self.lists.len() {
            self.lists.remove(idx);
        }
    }

    pub fn to_lua(&self) -> String {
        let mut s = String::from("-- Playlists de Capyro (se editan desde la app; máximo 3)\nreturn {\n");
        for pl in &self.lists {
            s += &format!("    {{\n        name = {},\n        files = {{\n", lua_quote(&pl.name));
            for f in &pl.files {
                s += &format!("            {},\n", lua_quote(f));
            }
            s += "        },\n    },\n";
        }
        s += "}\n";
        s
    }

    pub fn from_lua(src: &str) -> mlua::Result<Self> {
        let lua = Lua::new();
        let t: Table = lua.load(src).set_name("playlists.lua").eval()?;
        let mut lists = Vec::new();
        for entry in t.sequence_values::<Table>() {
            let entry = entry?;
            let name = clean_name(&entry.get::<Option<String>>("name")?.unwrap_or_default());
            let files: Vec<String> = match entry.get::<Option<Table>>("files")? {
                Some(f) => f.sequence_values::<String>().collect::<mlua::Result<_>>()?,
                None => Vec::new(),
            };
            let name = if name.is_empty() { format!("Playlist {}", lists.len() + 1) } else { name };
            lists.push(Playlist { name, files });
            if lists.len() == MAX_PLAYLISTS {
                break;
            }
        }
        Ok(Self { lists })
    }

    pub fn load() -> Self {
        let Some(p) = path() else { return Self::default() };
        let Ok(src) = std::fs::read_to_string(&p) else { return Self::default() };
        Self::from_lua(&src).unwrap_or_else(|e| {
            eprintln!("capyro: no pude leer {}: {e}", p.display());
            Self::default()
        })
    }

    /// Escribe a un temporal y lo renombra: nunca queda a medias.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(p) = path() else { return Ok(()) };
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = p.with_extension("lua.tmp");
        std::fs::write(&tmp, self.to_lua())?;
        std::fs::rename(&tmp, &p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta() {
        let mut p = Playlists::default();
        let a = p.create().unwrap();
        let b = p.create().unwrap();
        assert_eq!(p.lists[a].name, "Playlist 1");
        assert_eq!(p.lists[b].name, "Playlist 2");
        assert_eq!(p.add(a, &["/música/uno.mp3".into(), "/música/\"dos\".ogg".into()]), 2);
        assert_eq!(p.add(a, &["/música/uno.mp3".into()]), 0, "no repite");
        assert!(p.rename(b, "  Para   correr \n"));
        assert_eq!(p.lists[b].name, "Para correr");
        assert!(!p.rename(b, "   "));
        let back = Playlists::from_lua(&p.to_lua()).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn maximo_tres() {
        let mut p = Playlists::default();
        for _ in 0..MAX_PLAYLISTS {
            assert!(p.create().is_some());
        }
        assert!(p.is_full());
        assert_eq!(p.create(), None);
        p.delete(0);
        // el nombre libre se reutiliza
        let i = p.create().unwrap();
        assert_eq!(p.lists[i].name, "Playlist 1");
    }
}

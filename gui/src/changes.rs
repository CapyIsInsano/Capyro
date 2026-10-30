//! "Novedades": los cambios de ESTA versión, sacados del CHANGELOG.md que va
//! embebido en el binario (así nunca quedan desactualizados).

/// Repo oficial de Capyro (el resto del historial está ahí).
pub const REPO_URL: &str = "https://github.com/CapyIsInsano/Capyro";
/// CHANGELOG completo en GitHub (HEAD = la rama principal, se llame como se llame).
pub const CHANGELOG_URL: &str = "https://github.com/CapyIsInsano/Capyro/blob/HEAD/CHANGELOG.md";

const CHANGELOG: &str = include_str!("../../CHANGELOG.md");

/// Una línea de la lista: nivel 0 = punto principal, 1 = sub-punto.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub level: u8,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    /// "v0.2.8-alpha"
    pub version: String,
    /// "Playlists y atajos"
    pub title: String,
    pub items: Vec<Item>,
}

/// Saca el markdown que no se dibuja: **negrita**, `código`, <kbd>, [links](url).
pub fn plain(md: &str) -> String {
    let mut s = md.replace("<kbd>", "").replace("</kbd>", "").replace("**", "").replace('`', "");
    // [texto](url) → texto
    while let Some(open) = s.find('[') {
        let Some(mid) = s[open..].find("](").map(|i| open + i) else { break };
        let Some(close) = s[mid..].find(')').map(|i| mid + i) else { break };
        let text = s[open + 1..mid].to_owned();
        s.replace_range(open..=close, &text);
    }
    s
}

/// La primera versión del changelog (la actual).
pub fn current() -> Option<Release> {
    parse_first(CHANGELOG)
}

pub fn parse_first(md: &str) -> Option<Release> {
    let mut lines = md.lines().skip_while(|l| !l.starts_with("## v"));
    let head = lines.next()?.trim_start_matches("## ").trim();
    let (version, title) = match head.split_once('·') {
        Some((v, t)) => (v.trim().to_owned(), t.trim().to_owned()),
        None => (head.to_owned(), String::new()),
    };
    let mut items: Vec<Item> = Vec::new();
    for line in lines {
        if line.starts_with("## ") {
            break;
        }
        let indent = line.len() - line.trim_start().len();
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(rest) = t.strip_prefix("- ") {
            items.push(Item { level: u8::from(indent >= 2), text: plain(rest) });
        } else if let Some(last) = items.last_mut() {
            // continuación de la línea anterior
            last.text.push(' ');
            last.text += &plain(t);
        }
    }
    Some(Release { version, title, items })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_actual_es_esta_version() {
        let r = current().expect("el CHANGELOG tiene que tener al menos una versión");
        // la versión del crate (0.2.8) tiene que ser la primera del changelog
        assert!(r.version.starts_with(&format!("v{}", env!("CARGO_PKG_VERSION"))), "{}", r.version);
        assert!(!r.items.is_empty());
        assert!(r.items.iter().all(|i| !i.text.contains("**") && !i.text.contains('`')));
    }

    #[test]
    fn markdown() {
        let md = "# Título\n\n## v1.0 · Algo\n\n- **Uno** con <kbd>Ctrl</kbd>+<kbd>E</kbd>:\n  - sub `código`;\n  sigue acá\n- [link](http://x) fin\n\n## v0.9 · Vieja\n- no\n";
        let r = parse_first(md).unwrap();
        assert_eq!(r.version, "v1.0");
        assert_eq!(r.title, "Algo");
        assert_eq!(
            r.items,
            vec![
                Item { level: 0, text: "Uno con Ctrl+E:".into() },
                Item { level: 1, text: "sub código; sigue acá".into() },
                Item { level: 0, text: "link fin".into() },
            ]
        );
    }
}

//! La lista de apps fijas del Dock de la Mac (`com.apple.dock`,
//! `persistent-apps`): sacar la de Capyro al desinstalar. Es texto (el XML
//! que da `defaults export`), así que se prueba en cualquier sistema.

/// La config del Dock (XML) sin las entradas de `persistent-apps` que
/// apuntan a `app`. `None` si no había ninguna (o el XML no es el esperado).
pub fn dock_without(xml: &str, app: &str) -> Option<String> {
    let key = xml.find("<key>persistent-apps</key>")?;
    let arr_start = key + xml[key..].find("<array>")? + "<array>".len();
    // el </array> que cierra este array (puede haber otros adentro)
    let mut depth = 1usize;
    let mut i = arr_start;
    let arr_end = loop {
        let rest = &xml[i..];
        let open = rest.find("<array>");
        let close = rest.find("</array>")?;
        match open {
            Some(o) if o < close => {
                depth += 1;
                i += o + "<array>".len();
            }
            _ => {
                depth -= 1;
                if depth == 0 {
                    break i + close;
                }
                i += close + "</array>".len();
            }
        }
    };
    // los <dict> de primer nivel del array (cada uno es una app del Dock)
    let body = &xml[arr_start..arr_end];
    let mut kept = String::new();
    let mut removed = false;
    let mut pos = 0usize;
    while let Some(off) = body[pos..].find("<dict>") {
        let start = pos + off;
        kept.push_str(&body[pos..start]);
        let mut depth = 0usize;
        let mut j = start;
        let end = loop {
            let rest = &body[j..];
            let open = rest.find("<dict>");
            let close = rest.find("</dict>")?;
            match open {
                Some(o) if o < close => {
                    depth += 1;
                    j += o + "<dict>".len();
                }
                _ => {
                    depth -= 1;
                    j += close + "</dict>".len();
                    if depth == 0 {
                        break j;
                    }
                }
            }
        };
        let entry = &body[start..end];
        let target = app.trim_end_matches('/');
        let points_here = entry.contains(&format!("file://{}/", url_path(target))) || entry.contains(&format!("<string>{target}</string>"));
        if points_here {
            removed = true;
        } else {
            kept.push_str(entry);
        }
        pos = end;
    }
    kept.push_str(&body[pos..]);
    removed.then(|| format!("{}{}{}", &xml[..arr_start], kept, &xml[arr_end..]))
}

/// `/Applications/Mi App.app` → `/Applications/Mi%20App.app` (como lo guarda el Dock).
fn url_path(p: &str) -> String {
    p.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saca_a_capyro_del_dock() {
        let xml = r#"<plist><dict><key>persistent-apps</key><array><dict><key>tile-data</key><dict><key>file-data</key><dict><key>_CFURLString</key><string>file:///System/Applications/Music.app/</string></dict></dict></dict><dict><key>tile-data</key><dict><key>file-data</key><dict><key>_CFURLString</key><string>file:///Applications/Capyro.app/</string></dict></dict></dict></array><key>persistent-others</key><array/></dict></plist>"#;
        let out = dock_without(xml, "/Applications/Capyro.app").unwrap();
        assert!(out.contains("Music.app") && !out.contains("Capyro.app"), "{out}");
        assert!(out.contains("<key>persistent-others</key><array/>"));
        assert_eq!(dock_without(&out, "/Applications/Capyro.app"), None);
        assert_eq!(url_path("/Users/capy/Mis Apps/Capyro.app"), "/Users/capy/Mis%20Apps/Capyro.app");
    }
}

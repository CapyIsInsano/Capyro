//! La versión para Windows publicada en GitHub (Releases): qué trae y qué
//! cambió desde la que está instalada. Solo se compila para Windows (y para
//! las pruebas, que corren en cualquier sistema).
//!
//! El CI del repo (`.github/workflows/windows.yml`) compila Capyro para
//! Windows en cada push y lo publica en la release con la etiqueta
//! [`WINDOWS_TAG`], con el zip [`WINDOWS_ASSET`] y en la descripción:
//!
//! ```text
//! commit: <hash completo>
//! version: 0.3.0-alpha
//! ```

use serde_json::Value;

use super::CommitInfo;

/// Etiqueta de la release que mantiene el CI (siempre la última de `main`).
pub const WINDOWS_TAG: &str = "windows";
/// El paquete: capyro.exe, las DLL y config\capyro.lua.
pub const WINDOWS_ASSET: &str = "capyro-windows-x64.zip";

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    /// Commit del que sale (hash completo), si se sabe.
    pub commit: Option<String>,
    /// "0.3.0-alpha"
    pub version: Option<String>,
    pub name: String,
    /// Fecha de publicación (unix).
    pub published: i64,
    pub asset_url: String,
    pub asset_size: u64,
}

fn is_hash(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// "clave: valor" en la descripción de la release.
fn body_field<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    body.lines().find_map(|l| {
        let l = l.trim().trim_start_matches(['-', '*', ' ']);
        let (k, v) = l.split_once(':')?;
        (k.trim().eq_ignore_ascii_case(key)).then(|| v.trim().trim_matches('`'))
    })
}

pub fn parse_release(json: &str) -> Result<Release, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("respuesta rara de GitHub: {e}"))?;
    if let Some(msg) = v.get("message").and_then(Value::as_str) {
        if v.get("assets").is_none() {
            return Err(msg.to_owned());
        }
    }
    let body = v.get("body").and_then(Value::as_str).unwrap_or("");
    let commit = v
        .get("target_commitish")
        .and_then(Value::as_str)
        .filter(|c| is_hash(c))
        .or_else(|| body_field(body, "commit").filter(|c| is_hash(c)))
        .map(|c| c.to_ascii_lowercase());
    let asset = v
        .get("assets")
        .and_then(Value::as_array)
        .and_then(|a| a.iter().find(|x| x.get("name").and_then(Value::as_str) == Some(WINDOWS_ASSET)))
        .ok_or_else(|| format!("la versión publicada no trae {WINDOWS_ASSET}"))?;
    Ok(Release {
        commit,
        version: body_field(body, "version").map(str::to_owned).filter(|s| !s.is_empty()),
        name: v.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        published: v.get("published_at").and_then(Value::as_str).map(iso_to_unix).unwrap_or(0),
        asset_url: asset
            .get("browser_download_url")
            .and_then(Value::as_str)
            .ok_or("el zip no tiene URL")?
            .to_owned(),
        asset_size: asset.get("size").and_then(Value::as_u64).unwrap_or(0),
    })
}

/// Resultado de `GET /repos/{repo}/compare/{instalado}...{publicado}`.
#[derive(Debug, Clone, PartialEq)]
pub struct Compare {
    /// "identical", "ahead" (hay cosas nuevas), "behind" (el instalado es
    /// más nuevo) o "diverged".
    pub status: String,
    pub ahead_by: usize,
    /// Los commits nuevos, el primero es el último (hasta 20).
    pub commits: Vec<CommitInfo>,
}

fn commit_info(c: &Value) -> Option<CommitInfo> {
    let sha = c.get("sha").and_then(Value::as_str)?;
    let commit = c.get("commit")?;
    let subject = commit.get("message").and_then(Value::as_str).unwrap_or("").lines().next().unwrap_or("");
    let time = commit
        .get("committer")
        .or_else(|| commit.get("author"))
        .and_then(|p| p.get("date"))
        .and_then(Value::as_str)
        .map(iso_to_unix)
        .unwrap_or(0);
    Some(CommitInfo { hash: sha.chars().take(7).collect(), subject: subject.to_owned(), time })
}

pub fn parse_compare(json: &str) -> Result<Compare, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("respuesta rara de GitHub: {e}"))?;
    let status = v.get("status").and_then(Value::as_str).ok_or("GitHub no pudo comparar las versiones")?;
    // vienen del más viejo al más nuevo
    let mut commits: Vec<CommitInfo> = v
        .get("commits")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(commit_info).collect())
        .unwrap_or_default();
    commits.reverse();
    commits.truncate(20);
    Ok(Compare {
        status: status.to_owned(),
        ahead_by: v.get("ahead_by").and_then(Value::as_u64).unwrap_or(commits.len() as u64) as usize,
        commits,
    })
}

/// `GET /repos/{repo}/commits/{sha}` → el commit.
pub fn parse_commit(json: &str) -> Option<CommitInfo> {
    commit_info(&serde_json::from_str(json).ok()?)
}

/// "2026-10-26T14:25:00Z" → segundos desde 1970 (UTC).
pub fn iso_to_unix(s: &str) -> i64 {
    let num = |a: usize, b: usize| s.get(a..b).and_then(|t| t.parse::<i64>().ok());
    let (Some(y), Some(mo), Some(d)) = (num(0, 4), num(5, 7), num(8, 10)) else { return 0 };
    let (h, mi, se) = (num(11, 13).unwrap_or(0), num(14, 16).unwrap_or(0), num(17, 19).unwrap_or(0));
    // días desde 1970-01-01 (algoritmo de Howard Hinnant)
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (mo + if mo > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    days * 86_400 + h * 3600 + mi * 60 + se
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELEASE: &str = r#"{
        "tag_name": "windows",
        "target_commitish": "main",
        "name": "Capyro para Windows",
        "published_at": "2026-10-26T14:25:00Z",
        "body": "Compilado por el CI.\n\ncommit: 0123456789abcdef0123456789ABCDEF01234567\nversion: 0.3.0-alpha\n",
        "assets": [
            {"name": "otra-cosa.txt", "size": 3, "browser_download_url": "https://x/otra"},
            {"name": "capyro-windows-x64.zip", "size": 31457280,
             "browser_download_url": "https://github.com/CapyIsInsano/Capyro/releases/download/windows/capyro-windows-x64.zip"}
        ]
    }"#;

    #[test]
    fn release() {
        let r = parse_release(RELEASE).unwrap();
        assert_eq!(r.commit.as_deref(), Some("0123456789abcdef0123456789abcdef01234567"));
        assert_eq!(r.version.as_deref(), Some("0.3.0-alpha"));
        assert_eq!(r.asset_size, 31_457_280);
        assert!(r.asset_url.ends_with("/capyro-windows-x64.zip"));
        assert_eq!(r.published, 1_793_024_700);
        // target_commitish con hash gana
        let other = RELEASE.replace("\"main\"", "\"fedcba9876543210fedcba9876543210fedcba98\"");
        assert_eq!(parse_release(&other).unwrap().commit.as_deref(), Some("fedcba9876543210fedcba9876543210fedcba98"));
        // sin el zip
        assert!(parse_release(&RELEASE.replace("capyro-windows-x64.zip\"", "otro.zip\"")).is_err());
        // 404 de la API
        assert_eq!(parse_release(r#"{"message":"Not Found","status":"404"}"#).unwrap_err(), "Not Found");
    }

    #[test]
    fn compare() {
        let json = r#"{"status":"ahead","ahead_by":3,"behind_by":0,"commits":[
            {"sha":"aaaaaaa1111","commit":{"message":"Primero\n\ndetalle","committer":{"date":"2026-10-01T00:00:00Z"}}},
            {"sha":"bbbbbbb2222","commit":{"message":"Segundo","committer":{"date":"2026-10-02T00:00:00Z"}}},
            {"sha":"ccccccc3333","commit":{"message":"Tercero","committer":{"date":"2026-10-03T00:00:00Z"}}}
        ]}"#;
        let c = parse_compare(json).unwrap();
        assert_eq!(c.status, "ahead");
        assert_eq!(c.ahead_by, 3);
        assert_eq!(c.commits[0].hash, "ccccccc");
        assert_eq!(c.commits[0].subject, "Tercero");
        assert_eq!(c.commits[2].subject, "Primero");
        assert_eq!(c.commits[0].time - c.commits[1].time, 86_400);
        assert!(parse_compare(r#"{"message":"No common ancestor"}"#).is_err());
    }

    #[test]
    fn fechas() {
        assert_eq!(iso_to_unix("1970-01-01T00:00:00Z"), 0);
        assert_eq!(iso_to_unix("2000-03-01T00:00:00Z"), 951_868_800);
        assert_eq!(iso_to_unix("2024-02-29T12:00:00Z"), 1_709_208_000);
        assert_eq!(iso_to_unix("basura"), 0);
    }
}

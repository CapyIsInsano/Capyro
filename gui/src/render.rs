//! Con qué se dibuja la ventana: OpenGL o Direct3D 12 (wgpu, solo Windows).
//!
//! Un solo Capyro para todas las PC de Windows: en "auto" prueba primero el
//! que anduvo la última vez (queda anotado en `renderer.txt`, en la carpeta
//! de estado) y, si no hay, Direct3D. En Wine, siempre OpenGL primero (ver
//! [`crate::wine`]). Si un modo da error al arrancar se pasa
//! al otro en el momento; y si no da error pero la ventana no llega a
//! dibujarse (pasa con algunos drivers de OpenGL), un vigilante vuelve a
//! abrir Capyro con el otro modo.
//!
//! Se fuerza uno con `renderer = "opengl"` / `"direct3d"` en capyro.lua o con
//! la variable `CAPYRO_RENDERER`. En Linux siempre es OpenGL.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Lo marca la app en su primer frame: la ventana anda.
pub static FIRST_FRAME: AtomicBool = AtomicBool::new(false);

/// Número de intento: el vigilante de un intento que ya terminó (con error)
/// no hace nada.
static ATTEMPT: AtomicUsize = AtomicUsize::new(0);

/// `options.renderer` de capyro.lua.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pref {
    Auto,
    OpenGl,
    Direct3d,
}

impl Pref {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Some(Pref::Auto),
            "opengl" | "gl" | "glow" => Some(Pref::OpenGl),
            "direct3d" | "d3d" | "d3d12" | "directx" | "wgpu" => Some(Pref::Direct3d),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Pref::Auto => "auto",
            Pref::OpenGl => "opengl",
            Pref::Direct3d => "direct3d",
        }
    }
}

/// En qué orden probar (el primero que abra, gana).
pub fn order(cfg_pref: Pref) -> Vec<eframe::Renderer> {
    #[cfg(windows)]
    {
        use eframe::Renderer::{Glow, Wgpu};
        let env = std::env::var("CAPYRO_RENDERER").ok().and_then(|v| Pref::parse(&v));
        match env.unwrap_or(cfg_pref) {
            Pref::OpenGl => vec![Glow],
            Pref::Direct3d => vec![Wgpu],
            // En Wine, OpenGL va directo a la placa de video de Linux; Direct3D 12
            // necesita Vulkan (las placas viejas, como la de una X220, no tienen) y
            // si no hay, Wine dibuja con el procesador.
            Pref::Auto if crate::wine::is_wine() => vec![Glow, Wgpu],
            Pref::Auto => match remembered() {
                Some(Glow) => vec![Glow, Wgpu],
                _ => vec![Wgpu, Glow],
            },
        }
    }
    #[cfg(not(windows))]
    {
        let _ = cfg_pref;
        vec![eframe::Renderer::Glow]
    }
}

#[cfg(windows)]
fn memo_path() -> Option<std::path::PathBuf> {
    crate::paths::state_dir().map(|d| d.join("renderer.txt"))
}

#[cfg(windows)]
fn remembered() -> Option<eframe::Renderer> {
    let s = std::fs::read_to_string(memo_path()?).ok()?;
    match Pref::parse(s.lines().next()?)? {
        Pref::OpenGl => Some(eframe::Renderer::Glow),
        Pref::Direct3d => Some(eframe::Renderer::Wgpu),
        Pref::Auto => None,
    }
}

#[cfg(windows)]
fn remember(r: eframe::Renderer) {
    let Some(p) = memo_path() else { return };
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let name = if r == eframe::Renderer::Glow { "opengl" } else { "direct3d" };
    let _ = std::fs::write(p, format!("{name}\n# con qué se dibujó Capyro la última vez (se borra y se vuelve a elegir solo)\n"));
}

/// Vigila que la ventana llegue a dibujarse con `renderer`. Si a los 10 s no
/// hubo ni un frame, abre Capyro de nuevo con `other` y cierra este.
pub fn watch(renderer: eframe::Renderer, other: Option<eframe::Renderer>) {
    let attempt = ATTEMPT.fetch_add(1, Ordering::SeqCst) + 1;
    #[cfg(windows)]
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(10) {
            if ATTEMPT.load(Ordering::SeqCst) != attempt {
                return; // este intento falló y ya se está probando otro
            }
            if FIRST_FRAME.load(Ordering::Relaxed) {
                remember(renderer);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        crate::errlog::log(&format!("la ventana no se dibujó con {renderer:?}"));
        let retry = std::env::var_os("CAPYRO_RENDERER_RETRY").is_some();
        match other {
            Some(o) if !retry => {
                let name = if o == eframe::Renderer::Glow { "opengl" } else { "direct3d" };
                crate::errlog::log(&format!("vuelvo a abrir Capyro con {name}"));
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe)
                        .args(std::env::args_os().skip(1))
                        .env("CAPYRO_RENDERER", name)
                        .env("CAPYRO_RENDERER_RETRY", "1")
                        .spawn();
                }
                std::process::exit(0);
            }
            _ => {
                crate::errlog::fatal(
                    "La ventana de Capyro no se pudo dibujar ni con OpenGL ni con Direct3D.\n\nProbá actualizar los drivers de video.",
                );
                std::process::exit(1);
            }
        }
    });
    #[cfg(not(windows))]
    {
        let _ = (renderer, other, attempt);
        let _ = FIRST_FRAME.load(Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres() {
        for p in [Pref::Auto, Pref::OpenGl, Pref::Direct3d] {
            assert_eq!(Pref::parse(p.name()), Some(p));
        }
        assert_eq!(Pref::parse("D3D"), Some(Pref::Direct3d));
        assert_eq!(Pref::parse("vulkan"), None);
        if !cfg!(windows) {
            assert_eq!(order(Pref::Direct3d), vec![eframe::Renderer::Glow]);
        }
    }
}

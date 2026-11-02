//! Linux: el ícono en la bandeja con StatusNotifierItem (lo que usan KDE,
//! Cinnamon, Xfce, waybar, GNOME con la extensión AppIndicator…), por D-Bus.
//!
//! No publica un menú de texto: así, con el clic derecho la bandeja le pide
//! a Capyro que muestre el suyo (`ContextMenu`) y Capyro abre el mini
//! reproductor. El clic izquierdo (`Activate`) hace lo mismo, porque algunas
//! bandejas solo mandan ese. El del medio pausa o sigue.

use std::sync::mpsc::Sender;
use std::time::Instant;

use zbus::blocking::{connection, Connection, Proxy};
use zbus::interface;

use super::Event;

type Pixmap = Vec<(i32, i32, Vec<u8>)>;

struct Item {
    tx: Sender<Event>,
    icon: Pixmap,
}

#[interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    fn context_menu(&self, x: i32, y: i32) {
        let _ = self.tx.send(Event::Popup(x, y, Instant::now()));
    }

    fn activate(&self, x: i32, y: i32) {
        let _ = self.tx.send(Event::Popup(x, y, Instant::now()));
    }

    fn secondary_activate(&self, _x: i32, _y: i32) {
        let _ = self.tx.send(Event::Toggle);
    }

    fn scroll(&self, _delta: i32, _orientation: &str) {}

    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        "capyro"
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        "Capyro"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    fn window_id(&self) -> i32 {
        0
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn icon_name(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> Pixmap {
        self.icon.clone()
    }

    #[zbus(property)]
    fn overlay_icon_name(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn overlay_icon_pixmap(&self) -> Pixmap {
        Vec::new()
    }

    #[zbus(property)]
    fn attention_icon_name(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn attention_icon_pixmap(&self) -> Pixmap {
        Vec::new()
    }

    #[zbus(property)]
    fn attention_movie_name(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn tool_tip(&self) -> (String, Pixmap, String, String) {
        (String::new(), Vec::new(), "Capyro".to_owned(), "Sonando en segundo plano · clic: el mini reproductor".to_owned())
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }
}

/// El ícono mientras exista (al soltarlo, desaparece de la bandeja).
pub struct Tray {
    _conn: Connection,
}

/// El logo en ARGB32 (byte de mayor peso primero), en varios tamaños.
fn pixmaps() -> Pixmap {
    let Ok(img) = image::load_from_memory(crate::app::ICON_PNG) else { return Vec::new() };
    let img = img.to_rgba8();
    [22u32, 32, 48, 64]
        .into_iter()
        .map(|s| {
            let r = image::imageops::resize(&img, s, s, image::imageops::FilterType::Lanczos3);
            let argb = r.pixels().flat_map(|p| [p[3], p[0], p[1], p[2]]).collect();
            (s as i32, s as i32, argb)
        })
        .collect()
}

pub fn start(tx: Sender<Event>) -> Result<Tray, String> {
    let name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
    let item = Item { tx, icon: pixmaps() };
    let conn = connection::Builder::session()
        .and_then(|b| b.name(name.as_str()))
        .and_then(|b| b.serve_at("/StatusNotifierItem", item))
        .and_then(|b| b.build())
        .map_err(|e| format!("D-Bus: {e}"))?;
    let watcher = Proxy::new(&conn, "org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher", "org.kde.StatusNotifierWatcher")
        .map_err(|e| format!("bandeja: {e}"))?;
    watcher
        .call_method("RegisterStatusNotifierItem", &(name.as_str(),))
        .map_err(|e| format!("no hay bandeja del sistema: {e}"))?;
    Ok(Tray { _conn: conn })
}

/// ¿Hay una bandeja que muestre el ícono?
pub fn available() -> bool {
    let Ok(conn) = Connection::session() else { return false };
    let Ok(dbus) = zbus::blocking::fdo::DBusProxy::new(&conn) else { return false };
    let Ok(name) = "org.kde.StatusNotifierWatcher".try_into() else { return false };
    dbus.name_has_owner(name).unwrap_or(false)
}

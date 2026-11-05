//! Mac: el ícono en la barra de menús (NSStatusItem), arriba a la derecha,
//! al lado del reloj. Clic o clic derecho: el mini reproductor; clic del
//! medio (o con ⌥ Opción): pausar o seguir.
//!
//! Los clics llegan como eventos de AppKit: los atiende [`super::pump`]
//! (en la Mac, el bucle del segundo plano los procesa con
//! `crate::system::macos::pump`), en el hilo principal.

use std::sync::mpsc::Sender;
use std::time::Instant;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSEventType, NSScreen, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength,
};
use objc2_foundation::NSString;

use super::Event;

/// Lo que guarda el objeto que recibe los clics.
pub struct Ivars {
    tx: Sender<Event>,
}

define_class!(
    // SAFETY: NSObject no pide nada especial a sus subclases; esta solo
    // agrega un método y no se usa fuera del hilo principal.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "CapyroTrayTarget"]
    #[ivars = Ivars]
    struct Target;

    impl Target {
        #[unsafe(method(clicked:))]
        fn clicked(&self, _sender: Option<&AnyObject>) {
            let mtm = self.mtm();
            let event = NSApplication::sharedApplication(mtm).currentEvent();
            let middle = event.as_ref().is_some_and(|e| {
                e.r#type() == NSEventType::OtherMouseUp || e.modifierFlags().contains(NSEventModifierFlags::Option)
            });
            let ev = if middle {
                Event::Toggle
            } else {
                // dónde se hizo clic, con el origen arriba a la izquierda (como
                // las ventanas) y en "píxeles" (place() divide por la escala)
                let p = NSEvent::mouseLocation();
                let top = NSScreen::screens(mtm).firstObject().map_or(0.0, |s| s.frame().size.height);
                let k = super::scale() as f64;
                Event::Popup((p.x * k) as i32, ((top - p.y) * k) as i32, Instant::now())
            };
            let _ = self.ivars().tx.send(ev);
        }
    }

    unsafe impl NSObjectProtocol for Target {}
);

impl Target {
    fn new(mtm: MainThreadMarker, tx: Sender<Event>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars { tx });
        // SAFETY: init de NSObject.
        unsafe { msg_send![super(this), init] }
    }
}

/// El ícono mientras exista (al soltarlo, se va de la barra de menús).
pub struct Tray {
    item: Retained<NSStatusItem>,
    _target: Retained<Target>,
}

impl Drop for Tray {
    fn drop(&mut self) {
        NSStatusBar::systemStatusBar().removeStatusItem(&self.item);
    }
}

pub fn start(tx: Sender<Event>) -> Result<Tray, String> {
    let mtm = MainThreadMarker::new().ok_or("la barra de menús solo se toca desde el hilo principal")?;
    let target = Target::new(mtm, tx);
    let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
    let button = item.button(mtm).ok_or("macOS no me dio un botón en la barra de menús")?;
    match crate::system::macos::ns_image(crate::app::ICON_PNG, 18.0) {
        Some(img) => button.setImage(Some(&img)),
        None => button.setTitle(&NSString::from_str("Capyro")),
    }
    button.setToolTip(Some(&NSString::from_str("Capyro · sonando en segundo plano")));
    // SAFETY: el destino vive lo mismo que el ícono (los guarda Tray) y
    // responde a clicked:.
    unsafe {
        button.setTarget(Some(&target));
        button.setAction(Some(sel!(clicked:)));
    }
    button.sendActionOn(NSEventMask::LeftMouseUp | NSEventMask::RightMouseUp | NSEventMask::OtherMouseUp);
    Ok(Tray { item, _target: target })
}

/// En la Mac la barra de menús siempre está.
pub fn available() -> bool {
    true
}

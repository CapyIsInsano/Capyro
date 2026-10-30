//! Capyro corriendo en Wine (la versión de Windows en Linux).
//!
//! **El problema**: en Wine (9, 10 y la versión de desarrollo) Capyro se comía
//! casi un núcleo entero sin hacer nada, entre Capyro y `wineserver`.
//!
//! winit (lo que maneja la ventana) pide cada redibujo con
//! `RedrawWindow(..., RDW_INTERNALPAINT)`. El servidor de Wine suma 1 a la
//! cuenta de "pintura pendiente" del hilo al recibirlo, pero cuando entrega
//! el WM_PAINT apaga la marca **sin restar** (`find_window_to_repaint`, en
//! server/window.c). La cuenta nunca vuelve a 0, el aviso QS_PAINT queda
//! prendido para siempre, y el bucle de eventos se despierta al toque, no
//! encuentra nada que hacer y vuelve a esperar: miles de vueltas por segundo.
//!
//! **El arreglo**: [`install`] cambia, solo en Wine, la `RedrawWindow` que usa
//! este programa (su entrada en la tabla de importaciones del .exe) por una
//! que pide el mismo redibujo invalidando la ventana (`RDW_INVALIDATE`). Llega
//! el mismo WM_PAINT, pero esa cuenta Wine sí la resta cuando la ventana se
//! pinta. En Windows no se toca nada.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::OnceLock;

    type Hwnd = *mut c_void;
    type RedrawFn = unsafe extern "system" fn(Hwnd, *const c_void, *mut c_void, u32) -> i32;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
        fn VirtualProtect(addr: *mut c_void, size: usize, new: u32, old: *mut u32) -> i32;
    }

    const PAGE_READWRITE: u32 = 0x04;
    const RDW_INVALIDATE: u32 = 0x0001;
    const RDW_INTERNALPAINT: u32 = 0x0002;
    const RDW_NOERASE: u32 = 0x0020;
    const RDW_NOFRAME: u32 = 0x0800;

    /// La `RedrawWindow` de verdad (la que había en la tabla antes del cambio).
    static ORIGINAL: AtomicUsize = AtomicUsize::new(0);

    /// ¿Corre en Wine? (ntdll de Wine exporta `wine_get_version`; la de Windows no)
    pub fn is_wine() -> bool {
        static WINE: OnceLock<bool> = OnceLock::new();
        *WINE.get_or_init(|| unsafe {
            let name: Vec<u16> = "ntdll.dll\0".encode_utf16().collect();
            let ntdll = GetModuleHandleW(name.as_ptr());
            !ntdll.is_null() && !GetProcAddress(ntdll, c"wine_get_version".as_ptr().cast()).is_null()
        })
    }

    /// `RedrawWindow` con la pintura interna cambiada por una invalidación.
    unsafe extern "system" fn redraw_window(hwnd: Hwnd, rect: *const c_void, rgn: *mut c_void, flags: u32) -> i32 {
        let flags = if flags & RDW_INTERNALPAINT != 0 {
            (flags & !RDW_INTERNALPAINT) | RDW_INVALIDATE | RDW_NOERASE | RDW_NOFRAME
        } else {
            flags
        };
        let original = ORIGINAL.load(Ordering::Relaxed);
        if original == 0 {
            return 0;
        }
        // SAFETY: es la dirección que estaba en la tabla de importaciones.
        let f: RedrawFn = unsafe { std::mem::transmute::<usize, RedrawFn>(original) };
        unsafe { f(hwnd, rect, rgn, flags) }
    }

    unsafe fn read_u16(p: *const u8) -> u16 {
        unsafe { std::ptr::read_unaligned(p.cast::<u16>()) }
    }
    unsafe fn read_u32(p: *const u8) -> u32 {
        unsafe { std::ptr::read_unaligned(p.cast::<u32>()) }
    }
    unsafe fn cstr_eq(p: *const u8, s: &[u8], ignore_case: bool) -> bool {
        for (i, &c) in s.iter().enumerate() {
            let b = unsafe { *p.add(i) };
            let same = if ignore_case { b.eq_ignore_ascii_case(&c) } else { b == c };
            if !same {
                return false;
            }
        }
        unsafe { *p.add(s.len()) == 0 }
    }

    /// Cambia `RedrawWindow` (de user32) en la tabla de importaciones de este
    /// .exe. Devuelve cuántas entradas cambió.
    unsafe fn hook_redraw_window() -> usize {
        let base = unsafe { GetModuleHandleW(std::ptr::null()) }.cast::<u8>().cast_const();
        if base.is_null() || unsafe { read_u16(base) } != 0x5A4D {
            return 0; // "MZ"
        }
        let nt = unsafe { base.add(read_u32(base.add(0x3C)) as usize) };
        if unsafe { read_u32(nt) } != 0x0000_4550 {
            return 0; // "PE\0\0"
        }
        let opt = unsafe { nt.add(24) };
        if unsafe { read_u16(opt) } != 0x20B {
            return 0; // solo PE32+ (64 bits)
        }
        // directorio 1 = importaciones
        let imports_rva = unsafe { read_u32(opt.add(112 + 8)) } as usize;
        if imports_rva == 0 {
            return 0;
        }
        let mut changed = 0;
        let mut desc = unsafe { base.add(imports_rva) };
        loop {
            let names_rva = unsafe { read_u32(desc) } as usize; // OriginalFirstThunk
            let dll_rva = unsafe { read_u32(desc.add(12)) } as usize;
            let iat_rva = unsafe { read_u32(desc.add(16)) } as usize; // FirstThunk
            if dll_rva == 0 {
                break;
            }
            if names_rva != 0 && unsafe { cstr_eq(base.add(dll_rva), b"user32.dll", true) } {
                let mut i = 0;
                loop {
                    let entry = unsafe { std::ptr::read_unaligned(base.add(names_rva + i * 8).cast::<u64>()) };
                    if entry == 0 {
                        break;
                    }
                    // bit alto = importación por número (no tiene nombre)
                    if entry & (1 << 63) == 0 {
                        let name = unsafe { base.add(entry as u32 as usize + 2) }; // salta el "hint"
                        if unsafe { cstr_eq(name, b"RedrawWindow", false) } {
                            let slot = unsafe { base.add(iat_rva + i * 8) }.cast_mut().cast::<usize>();
                            let mut old = 0u32;
                            if unsafe { VirtualProtect(slot.cast(), 8, PAGE_READWRITE, &mut old) } != 0 {
                                let current = unsafe { std::ptr::read_volatile(slot) };
                                if current != redraw_window as RedrawFn as usize {
                                    ORIGINAL.store(current, Ordering::Relaxed);
                                    unsafe { std::ptr::write_volatile(slot, redraw_window as RedrawFn as usize) };
                                    changed += 1;
                                }
                                unsafe { VirtualProtect(slot.cast(), 8, old, &mut old) };
                            }
                        }
                    }
                    i += 1;
                }
            }
            desc = unsafe { desc.add(20) };
        }
        changed
    }

    /// En Wine, arregla los redibujos de winit (ver arriba). Llamar antes de
    /// abrir la ventana. En Windows no hace nada.
    pub fn install() {
        if !is_wine() {
            return;
        }
        let n = unsafe { hook_redraw_window() };
        if std::env::var_os("CAPYRO_DEBUG_WINE").is_some() {
            eprintln!("capyro: wine: RedrawWindow cambiada en {n} lugar(es) de la tabla de importaciones");
        }
    }
}

#[cfg(windows)]
pub use imp::{install, is_wine};

/// En Linux (y macOS) no hay Wine de por medio.
#[cfg(not(windows))]
#[allow(dead_code)] // solo se usa en la versión de Windows
pub fn is_wine() -> bool {
    false
}

#[cfg(not(windows))]
pub fn install() {}

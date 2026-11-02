//! Windows: el ícono en el área de notificación (Shell_NotifyIcon), con una
//! ventana invisible en su propio hilo que recibe los clics. Clic izquierdo o
//! derecho: el mini reproductor; clic del medio: pausar o seguir.

use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIcon, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW,
    GetWindowLongPtrW, PostMessageW, PostQuitMessage, RegisterClassExW, SetWindowLongPtrW, TranslateMessage, GWLP_USERDATA,
    HWND_MESSAGE, MSG, WM_APP, WM_CLOSE, WM_DESTROY, WM_LBUTTONUP, WM_MBUTTONUP, WM_RBUTTONUP, WNDCLASSEXW,
};

use super::Event;

const WM_TRAY: u32 = WM_APP + 1;

/// El ícono mientras exista (al soltarlo, se va).
pub struct Tray {
    hwnd: usize,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe { PostMessageW(self.hwnd as HWND, WM_CLOSE, 0, 0) };
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            let tx = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Sender<Event>;
            if !tx.is_null() {
                let ev = match lp as u32 {
                    WM_LBUTTONUP | WM_RBUTTONUP => {
                        let mut p = POINT { x: 0, y: 0 };
                        GetCursorPos(&mut p);
                        Some(Event::Popup(p.x, p.y, std::time::Instant::now()))
                    }
                    WM_MBUTTONUP => Some(Event::Toggle),
                    _ => None,
                };
                if let Some(ev) = ev {
                    let _ = (*tx).send(ev);
                }
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// El logo de 32×32 como ícono de Windows.
unsafe fn make_icon() -> *mut core::ffi::c_void {
    let Ok(img) = image::load_from_memory(crate::app::ICON_PNG) else { return std::ptr::null_mut() };
    let img = image::imageops::resize(&img.to_rgba8(), 32, 32, image::imageops::FilterType::Lanczos3);
    let bgra: Vec<u8> = img.pixels().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
    // máscara de 1 bit por píxel en cero: con 32 bits manda el alfa
    let and_mask = vec![0u8; 32 * 32 / 8];
    CreateIcon(GetModuleHandleW(std::ptr::null()), 32, 32, 1, 32, and_mask.as_ptr(), bgra.as_ptr())
}

pub fn start(tx: Sender<Event>) -> Result<Tray, String> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<usize, String>>();
    let thread = std::thread::spawn(move || unsafe {
        let hinst = GetModuleHandleW(std::ptr::null());
        let class = wide("CapyroBandeja");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: hinst,
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassExW(&wc);
        let title = wide("Capyro");
        let hwnd = CreateWindowExW(0, class.as_ptr(), title.as_ptr(), 0, 0, 0, 0, 0, HWND_MESSAGE, std::ptr::null_mut(), hinst, std::ptr::null());
        if hwnd.is_null() {
            let _ = ready_tx.send(Err("no pude crear la ventana de la bandeja".to_owned()));
            return;
        }
        // el canal vive mientras viva el hilo
        let tx_box = Box::new(tx);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &*tx_box as *const Sender<Event> as isize);
        let icon = make_icon();
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY;
        nid.hIcon = icon;
        for (dst, src) in nid.szTip.iter_mut().zip(wide("Capyro · sonando en segundo plano")) {
            *dst = src;
        }
        if Shell_NotifyIconW(NIM_ADD, &nid) == 0 {
            let _ = ready_tx.send(Err("Windows no aceptó el ícono de la bandeja".to_owned()));
            DestroyWindow(hwnd);
            DestroyIcon(icon);
            return;
        }
        let _ = ready_tx.send(Ok(hwnd as usize));
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        Shell_NotifyIconW(NIM_DELETE, &nid);
        DestroyIcon(icon);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        drop(tx_box);
    });
    match ready_rx.recv() {
        Ok(Ok(hwnd)) => Ok(Tray { hwnd, thread: Some(thread) }),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => Err("se cortó el hilo de la bandeja".to_owned()),
    }
}

/// En Windows siempre hay área de notificación.
pub fn available() -> bool {
    true
}

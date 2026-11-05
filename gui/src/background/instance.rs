//! Un solo Capyro mientras está en segundo plano: si se abre otro (desde el
//! menú, o con un archivo), le pasa los archivos al que ya suena y se va.
//!
//! El que está en segundo plano escucha en 127.0.0.1 (un puerto cualquiera)
//! y anota el puerto y una clave en `segundo-plano.txt`, en la carpeta de
//! estado. Solo mientras está en segundo plano: con la ventana abierta,
//! abrir Capyro otra vez abre otro (como siempre).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use super::Event;

const MAGIC: &str = "CAPYRO-SEGUNDO-PLANO";

fn note_path() -> Option<PathBuf> {
    crate::system::paths::state_dir().map(|d| d.join("segundo-plano.txt"))
}

/// Escucha mientras exista (al soltarlo, deja de escuchar y borra la nota).
pub struct Listener {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    note: Option<PathBuf>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if let Some(p) = &self.note {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn token() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    let mut x = t ^ (std::process::id() as u64).rotate_left(32) ^ 0x9E37_79B9_7F4A_7C15;
    // un par de vueltas de xorshift para que no se parezca al reloj
    for _ in 0..4 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
    }
    format!("{x:016x}")
}

/// Lee un pedido: la clave y los archivos, uno por renglón.
fn serve(stream: TcpStream, key: &str) -> Option<Vec<PathBuf>> {
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    let mut reader = BufReader::new(stream.try_clone().ok()?.take(1 << 20));
    let mut first = String::new();
    reader.read_line(&mut first).ok()?;
    if first.trim_end() != format!("{MAGIC} {key}") {
        return None;
    }
    let files = reader
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect();
    let mut stream = stream;
    let _ = stream.write_all(b"ok\n");
    Some(files)
}

pub fn listen(tx: Sender<Event>) -> Option<Listener> {
    let note = note_path()?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
    listener.set_nonblocking(true).ok()?;
    let port = listener.local_addr().ok()?.port();
    let key = token();
    if let Some(dir) = note.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&note, format!("{port}\n{key}\n")).ok()?;
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let thread = std::thread::spawn(move || {
        while !stop2.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    if let Some(files) = serve(stream, &key) {
                        let _ = tx.send(Event::Open(files));
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(120)),
                Err(_) => std::thread::sleep(Duration::from_millis(500)),
            }
        }
    });
    Some(Listener { stop, thread: Some(thread), note: Some(note) })
}

/// Al arrancar: si hay un Capyro en segundo plano, le pasa los archivos de
/// la línea de comandos y devuelve true (este se cierra y aquel vuelve).
pub fn wake_existing() -> bool {
    let Some(note) = note_path() else { return false };
    let Ok(text) = std::fs::read_to_string(&note) else { return false };
    let mut lines = text.lines();
    let (Some(port), Some(key)) = (lines.next().and_then(|p| p.trim().parse::<u16>().ok()), lines.next()) else {
        return false;
    };
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(400)) else {
        // quedó la nota de un Capyro que ya no está
        let _ = std::fs::remove_file(&note);
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let files: Vec<String> = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| !p.to_string_lossy().starts_with('-') && p.exists())
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| !p.contains('\n'))
        .collect();
    let mut msg = format!("{MAGIC} {}\n", key.trim());
    for f in &files {
        msg.push_str(f);
        msg.push('\n');
    }
    if stream.write_all(msg.as_bytes()).is_err() || stream.shutdown(std::net::Shutdown::Write).is_err() {
        return false;
    }
    let mut reply = String::new();
    let _ = BufReader::new(stream).read_line(&mut reply);
    reply.trim() == "ok"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_pedido_trae_la_clave_y_los_archivos() {
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = l.local_addr().unwrap();
        let client = std::thread::spawn(move || {
            let mut s = TcpStream::connect(addr).unwrap();
            s.write_all(format!("{MAGIC} abc\n/musica/uno.flac\n/musica/dos.mp3\n").as_bytes()).unwrap();
            s.shutdown(std::net::Shutdown::Write).unwrap();
            let mut r = String::new();
            BufReader::new(s).read_line(&mut r).unwrap();
            r
        });
        let (s, _) = l.accept().unwrap();
        let files = serve(s, "abc").unwrap();
        assert_eq!(files, vec![PathBuf::from("/musica/uno.flac"), PathBuf::from("/musica/dos.mp3")]);
        assert_eq!(client.join().unwrap(), "ok\n");

        // con otra clave no pasa nada
        let client = std::thread::spawn(move || {
            let mut s = TcpStream::connect(addr).unwrap();
            let _ = s.write_all(format!("{MAGIC} otra\n/x\n").as_bytes());
        });
        let (s, _) = l.accept().unwrap();
        assert!(serve(s, "abc").is_none());
        client.join().unwrap();
        assert_eq!(token().len(), 16);
    }
}

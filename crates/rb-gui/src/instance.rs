//! Eén venster voor alles: een tweede start (bijvoorbeeld een dubbelklik op een `.rkn` in de
//! Verkenner) geeft het bestand door aan het venster dat al open is, en sluit dan zelf.
//!
//! Het eerste venster luistert op een lokale poort (127.0.0.1, door Windows gekozen) en schrijft
//! `poort sleutel` naar `instance.txt` in de instellingenmap. Een tweede start leest dat, stuurt
//! `sleutel\npad\n` en wacht op `ok`. Zonder geldige sleutel doet het eerste venster niets. Is er
//! geen venster (oud bestand, poort dicht), dan wordt de nieuwe start zelf het eerste venster.

use eframe::egui;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Wat een tweede start doorgeeft: een bestand om te openen, of alleen "kom naar voren".
pub type Request = Option<PathBuf>;

/// De verzoeken die binnenkwamen, voor de interface; `ctx` om de interface wakker te maken.
#[derive(Clone, Default)]
pub struct Inbox {
    queue: Arc<Mutex<Vec<Request>>>,
    ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl Inbox {
    pub fn set_ctx(&self, ctx: &egui::Context) {
        *self.ctx.lock().unwrap() = Some(ctx.clone());
    }

    /// Alle verzoeken sinds de vorige keer.
    pub fn take(&self) -> Vec<Request> {
        std::mem::take(&mut *self.queue.lock().unwrap())
    }

    fn push(&self, r: Request) {
        self.queue.lock().unwrap().push(r);
        if let Some(ctx) = self.ctx.lock().unwrap().as_ref() {
            ctx.request_repaint();
        }
    }
}

pub enum Start {
    /// Doorgegeven aan het venster dat al open was: deze start moet stoppen.
    Handed,
    /// Dit is het eerste venster; verzoeken komen binnen via de inbox.
    Primary(Inbox),
    /// Geen samenwerking (schermafdruk voor tests, of geen instellingenmap).
    Alone,
}

/// Wat dit venster in `instance.txt` schreef (om bij het afsluiten alleen dat weg te halen).
static WRITTEN: OnceLock<String> = OnceLock::new();

fn contact_file() -> Option<PathBuf> {
    Some(crate::keymap::config_dir()?.join("instance.txt"))
}

/// Een sleutel die alleen dit venster en wie `instance.txt` kan lezen kennen.
fn new_key() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    h.write_u32(std::process::id());
    let a = h.finish();
    h.write_u64(a);
    format!("{a:016x}{:016x}", h.finish())
}

/// Lees `poort sleutel`.
fn parse_contact(text: &str) -> Option<(u16, String)> {
    let mut parts = text.split_whitespace();
    let port = parts.next()?.parse::<u16>().ok()?;
    let key = parts.next()?.to_string();
    Some((port, key))
}

/// Probeer het bestand door te geven aan een venster dat al open is.
fn hand_over(file: Option<&Path>) -> bool {
    let Some((port, key)) = contact_file().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| parse_contact(&t)) else {
        return false;
    };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(400)) else { return false };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    // een relatief pad hoort bij de map van deze start, niet bij die van het andere venster
    let path = file.map(|f| std::fs::canonicalize(f).unwrap_or_else(|_| std::env::current_dir().map_or(f.to_path_buf(), |d| d.join(f))));
    let msg = format!("{key}\n{}\n", path.map_or(String::new(), |p| p.display().to_string()));
    if s.write_all(msg.as_bytes()).is_err() {
        return false;
    }
    let mut reply = String::new();
    BufReader::new(s).read_line(&mut reply).is_ok() && reply.trim() == "ok"
}

/// Luister naar latere starts; schrijf de poort en sleutel weg.
fn listen() -> Option<Inbox> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    let key = new_key();
    let file = contact_file()?;
    std::fs::create_dir_all(file.parent()?).ok()?;
    let line = format!("{port} {key}\n");
    std::fs::write(&file, &line).ok()?;
    let _ = WRITTEN.set(line);
    let inbox = Inbox::default();
    let out = inbox.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut lines = BufReader::new(&stream).lines();
            if !matches!(lines.next(), Some(Ok(k)) if k.trim() == key) {
                continue;
            }
            let path = lines.next().and_then(Result::ok).map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).map(PathBuf::from);
            out.push(path);
            let _ = (&stream).write_all(b"ok\n");
        }
    });
    Some(inbox)
}

/// Bij het starten: doorgeven aan een open venster, of zelf het eerste venster worden.
pub fn start(file: Option<&Path>) -> Start {
    // schermafdrukken voor tests draaien los, behalve met MATHPRIME_EEN_VENSTER (test van dit bestand)
    let shot = std::env::var_os("MATHPRIME_SCREENSHOT").is_some() && std::env::var_os("MATHPRIME_EEN_VENSTER").is_none();
    if shot || std::env::var_os("MATHPRIME_NIEUW_VENSTER").is_some() {
        return Start::Alone;
    }
    if hand_over(file) {
        return Start::Handed;
    }
    match listen() {
        Some(inbox) => Start::Primary(inbox),
        None => Start::Alone,
    }
}

/// Bij het afsluiten: het contactbestand weghalen, als een latere start het niet al overschreef.
pub fn stop() {
    let (Some(mine), Some(f)) = (WRITTEN.get(), contact_file()) else { return };
    if std::fs::read_to_string(&f).is_ok_and(|t| &t == mine) {
        let _ = std::fs::remove_file(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contactregel_lezen() {
        assert_eq!(parse_contact("53124 9f3c\n"), Some((53124, "9f3c".to_string())));
        assert_eq!(parse_contact("geen poort"), None);
        assert_eq!(parse_contact(""), None);
        assert_ne!(new_key(), new_key());
    }

    #[test]
    fn verzoek_komt_aan() {
        // een luisteraar zoals in listen(), maar zonder instellingenmap
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let inbox = Inbox::default();
        let out = inbox.clone();
        std::thread::spawn(move || {
            let stream = listener.incoming().next().unwrap().unwrap();
            let mut lines = BufReader::new(&stream).lines();
            assert_eq!(lines.next().unwrap().unwrap(), "sleutel");
            out.push(lines.next().and_then(Result::ok).filter(|p| !p.is_empty()).map(PathBuf::from));
            (&stream).write_all(b"ok\n").unwrap();
        });
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.write_all(b"sleutel\nC:/bladen/balk.rkn\n").unwrap();
        let mut reply = String::new();
        BufReader::new(s).read_line(&mut reply).unwrap();
        assert_eq!(reply.trim(), "ok");
        assert_eq!(inbox.take(), vec![Some(PathBuf::from("C:/bladen/balk.rkn"))]);
    }
}

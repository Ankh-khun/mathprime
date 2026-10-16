//! Kijken of er een nieuwere versie is, en die desgewenst installeren. Alles via `curl.exe` (zit in
//! Windows 10 en 11; geen extra crate), op een eigen thread.
//!
//! - Vanzelf: hooguit één keer per dag (`update.txt` houdt het tijdstip bij). Geen verbinding, geen release
//!   of een fout: dan meldt MathPrime niets. Uit te zetten in Instellingen ("updates = uit").
//! - Met de hand: Help → Zoeken naar updates, of de knop in Instellingen; dan komt er altijd een antwoord.
//! - "Downloaden en installeren": de setup uit de release gaat naar de tijdelijke map en start met
//!   `--wacht <pid> --starten`; MathPrime sluit (met de gewone vraag om op te slaan), de setup installeert
//!   en start de nieuwe versie.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::{SystemTime, UNIX_EPOCH};

const LATEST: &str = "https://api.github.com/repos/Ankh-khun/mathprime/releases/latest";
const DAY: u64 = 24 * 60 * 60;

/// Een nieuwere versie dan deze.
#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    pub version: String,
    /// De releasepagina.
    pub url: String,
    /// Het installatieprogramma (`MathPrime-<versie>-setup.exe`) uit de release, als dat er is.
    pub setup: Option<String>,
}

/// De tekst van een JSON-veld `"key": "…"` vanaf `from` (genoeg voor het antwoord van GitHub; geen
/// volledige parser). Geeft ook de positie na de waarde.
fn json_str_from(json: &str, key: &str, from: usize) -> Option<(String, usize)> {
    let at = from + json.get(from..)?.find(&format!("\"{key}\""))? + key.len() + 2;
    let rest = json[at..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let start = json.len() - rest.len();
    let mut out = String::new();
    let mut chars = rest.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, start + i + 1)),
            '\\' => out.push(chars.next()?.1),
            c => out.push(c),
        }
    }
    None
}

fn json_str(json: &str, key: &str) -> Option<String> {
    json_str_from(json, key, 0).map(|(s, _)| s)
}

/// `v1.10.2` → [1, 10, 2].
fn parts(v: &str) -> Option<Vec<u64>> {
    v.trim().trim_start_matches('v').split('.').map(|p| p.parse().ok()).collect()
}

/// Is `latest` nieuwer dan `current`?
pub fn newer(latest: &str, current: &str) -> bool {
    matches!((parts(latest), parts(current)), (Some(a), Some(b)) if a > b)
}

/// Het antwoord van GitHub lezen: een update als de release nieuwer is dan deze versie.
pub fn parse_release(json: &str, current: &str) -> Option<Update> {
    let tag = json_str(json, "tag_name")?;
    let url = json_str(json, "html_url")?;
    // de bijlagen: de eerste download die op -setup.exe eindigt
    let mut setup = None;
    let mut at = 0;
    while let Some((link, next)) = json_str_from(json, "browser_download_url", at) {
        if link.to_ascii_lowercase().ends_with("-setup.exe") {
            setup = Some(link);
            break;
        }
        at = next;
    }
    newer(&tag, current).then(|| Update { version: tag.trim_start_matches('v').to_string(), url, setup })
}

fn curl() -> std::process::Command {
    let mut cmd = std::process::Command::new("curl.exe");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: geen consolevenster
    }
    cmd
}

/// De nieuwste release opvragen. `Ok(None)`: deze versie is de nieuwste.
fn fetch() -> Result<Option<Update>, String> {
    let out = curl()
        .args(["-s", "-f", "-m", "15", "-H", "Accept: application/vnd.github+json", "-A", "MathPrime", LATEST])
        .output()
        .map_err(|e| format!("curl.exe: {e}"))?;
    if !out.status.success() {
        return Err(crate::taal::nl_en("GitHub is niet bereikbaar", "GitHub cannot be reached").to_string());
    }
    let json = String::from_utf8_lossy(&out.stdout);
    if json_str(&json, "tag_name").is_none() {
        return Err(crate::taal::nl_en("onverwacht antwoord van GitHub", "unexpected answer from GitHub").to_string());
    }
    Ok(parse_release(&json, env!("CARGO_PKG_VERSION")))
}

/// Waar het zoeken en bijwerken staat.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    /// Met de hand gezocht: dit is de nieuwste versie.
    Latest,
    Failed(String),
    Newer(Update),
    Downloading(Update),
    /// De setup staat klaar en is gestart; MathPrime sluit.
    Installing,
}

enum Event {
    Checked(Result<Option<Update>, String>),
    Downloaded(Result<PathBuf, String>),
}

#[derive(Default)]
pub struct State {
    pub phase: Phase,
    rx: Option<Receiver<Event>>,
    /// Vanzelf gezocht: alleen iets melden als er een nieuwere versie is.
    silent: bool,
}

impl State {
    fn spawn(&mut self, ctx: &eframe::egui::Context, job: impl FnOnce() -> Event + Send + 'static) {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(job());
            ctx.request_repaint();
        });
        self.rx = Some(rx);
    }

    /// Eén keer per dag vanzelf kijken.
    pub fn daily(&mut self, config_dir: Option<PathBuf>, ctx: &eframe::egui::Context) {
        let Some(now) = SystemTime::now().duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs()) else { return };
        let Some(stamp) = config_dir.map(|d| d.join("update.txt")) else { return };
        let last: u64 = std::fs::read_to_string(&stamp).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
        if now.saturating_sub(last) < DAY {
            return;
        }
        let _ = std::fs::write(&stamp, now.to_string());
        self.silent = true;
        self.spawn(ctx, || Event::Checked(fetch()));
    }

    /// Met de hand zoeken (knop of menu).
    pub fn check(&mut self, ctx: &eframe::egui::Context) {
        if matches!(self.phase, Phase::Checking | Phase::Downloading(_) | Phase::Installing) {
            return;
        }
        self.silent = false;
        self.phase = Phase::Checking;
        self.spawn(ctx, || Event::Checked(fetch()));
    }

    /// De setup van `u` downloaden naar de tijdelijke map.
    pub fn download(&mut self, ctx: &eframe::egui::Context) {
        let Phase::Newer(u) = self.phase.clone() else { return };
        let Some(link) = u.setup.clone() else { return };
        let path = std::env::temp_dir().join(format!("MathPrime-{}-setup.exe", u.version));
        self.phase = Phase::Downloading(u);
        self.spawn(ctx, move || {
            let out = curl().args(["-s", "-f", "-L", "-m", "600", "-A", "MathPrime", "-o"]).arg(&path).arg(&link).output();
            Event::Downloaded(match out {
                Ok(o) if o.status.success() && path.metadata().is_ok_and(|m| m.len() > 100_000) => Ok(path),
                Ok(_) => Err(crate::taal::nl_en("downloaden mislukt", "download failed").to_string()),
                Err(e) => Err(format!("curl.exe: {e}")),
            })
        });
    }

    /// De nieuwere versie, als die er is.
    pub fn newer(&self) -> Option<&Update> {
        match &self.phase {
            Phase::Newer(u) | Phase::Downloading(u) => Some(u),
            _ => None,
        }
    }

    /// Antwoorden verwerken. Geeft het pad van de gedownloade setup als die klaarstaat.
    pub fn poll(&mut self) -> Option<PathBuf> {
        let ev = self.rx.as_ref()?.try_recv().ok()?;
        self.rx = None;
        match ev {
            Event::Checked(Ok(Some(u))) => self.phase = Phase::Newer(u),
            Event::Checked(Ok(None)) => self.phase = if self.silent { Phase::Idle } else { Phase::Latest },
            Event::Checked(Err(e)) => self.phase = if self.silent { Phase::Idle } else { Phase::Failed(e) },
            Event::Downloaded(Ok(p)) => {
                self.phase = Phase::Installing;
                return Some(p);
            }
            Event::Downloaded(Err(e)) => self.phase = Phase::Failed(e),
        }
        None
    }
}

/// Start de gedownloade setup: die wacht tot dit proces weg is, installeert en start MathPrime weer.
pub fn run_setup(path: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new(path)
        .args(["--wacht", &std::process::id().to_string(), "--starten"])
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versies_vergelijken() {
        assert!(newer("v1.10.0", "1.9.3"));
        assert!(newer("2.0.0", "1.99.99"));
        assert!(!newer("v1.4.0", "1.4.0"));
        assert!(!newer("v1.3.9", "1.4.0"));
        assert!(!newer("nightly", "1.4.0"));
    }

    #[test]
    fn antwoord_van_github() {
        let json = r#"{"url":"x","html_url":"https://github.com/Ankh-khun/mathprime/releases/tag/v9.0.0","id":1,
            "tag_name": "v9.0.0","name":"MathPrime \"9\"","assets":[
            {"name":"MathPrime-9.0.0-windows-x64.zip","browser_download_url":"https://github.com/Ankh-khun/mathprime/releases/download/v9.0.0/MathPrime-9.0.0-windows-x64.zip"},
            {"name":"MathPrime-9.0.0-setup.exe","browser_download_url":"https://github.com/Ankh-khun/mathprime/releases/download/v9.0.0/MathPrime-9.0.0-setup.exe"}]}"#;
        let u = parse_release(json, "1.4.0").unwrap();
        assert_eq!(u.version, "9.0.0");
        assert_eq!(u.url, "https://github.com/Ankh-khun/mathprime/releases/tag/v9.0.0");
        assert_eq!(u.setup.as_deref(), Some("https://github.com/Ankh-khun/mathprime/releases/download/v9.0.0/MathPrime-9.0.0-setup.exe"));
        assert!(parse_release(json, "9.0.0").is_none());
        assert!(parse_release(r#"{"message":"Not Found"}"#, "1.0.0").is_none());
        assert_eq!(json_str(json, "name").as_deref(), Some("MathPrime \"9\""));
        // zonder setup in de release: alleen de pagina
        let kaal = r#"{"html_url":"https://x/v9","tag_name":"v9.0.0","assets":[]}"#;
        assert_eq!(parse_release(kaal, "1.0.0").unwrap().setup, None);
    }
}

//! Kijken of er een nieuwere versie is: de nieuwste release op GitHub, hooguit één keer per dag, op de
//! achtergrond met `curl.exe` (zit in Windows 10 en 11; geen extra crate). Geen verbinding, geen release
//! of een fout: dan meldt MathPrime niets. Uit te zetten in Instellingen ("updates = uit").

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
}

/// De tekst van een JSON-veld `"key": "…"` (genoeg voor het antwoord van GitHub; geen volledige parser).
fn json_str(json: &str, key: &str) -> Option<String> {
    let at = json.find(&format!("\"{key}\""))? + key.len() + 2;
    let rest = json[at..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => out.push(chars.next()?),
            c => out.push(c),
        }
    }
    None
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
    newer(&tag, current).then(|| Update { version: tag.trim_start_matches('v').to_string(), url })
}

/// Begin met kijken, als dat vandaag nog niet gebeurd is (`update.txt` in de configuratiemap houdt het
/// tijdstip bij). Het antwoord komt later via de ontvanger.
pub fn start(config_dir: Option<PathBuf>, ctx: eframe::egui::Context) -> Option<Receiver<Update>> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let stamp = config_dir?.join("update.txt");
    let last: u64 = std::fs::read_to_string(&stamp).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
    if now.saturating_sub(last) < DAY {
        return None;
    }
    let _ = std::fs::write(&stamp, now.to_string());
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut cmd = std::process::Command::new("curl.exe");
        cmd.args(["-s", "-f", "-m", "10", "-H", "Accept: application/vnd.github+json", "-A", "MathPrime", LATEST]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: geen consolevenster
        }
        let Ok(out) = cmd.output() else { return };
        if !out.status.success() {
            return;
        }
        if let Some(u) = parse_release(&String::from_utf8_lossy(&out.stdout), env!("CARGO_PKG_VERSION")) {
            let _ = tx.send(u);
            ctx.request_repaint();
        }
    });
    Some(rx)
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
            "tag_name": "v9.0.0","name":"MathPrime \"9\""}"#;
        let u = parse_release(json, "1.4.0").unwrap();
        assert_eq!(u, Update { version: "9.0.0".into(), url: "https://github.com/Ankh-khun/mathprime/releases/tag/v9.0.0".into() });
        assert!(parse_release(json, "9.0.0").is_none());
        assert!(parse_release(r#"{"message":"Not Found"}"#, "1.0.0").is_none());
        assert_eq!(json_str(json, "name").as_deref(), Some("MathPrime \"9\""));
    }
}

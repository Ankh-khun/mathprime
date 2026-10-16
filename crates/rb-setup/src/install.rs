//! Het eigenlijke installeren: de bestanden uitpakken in een tijdelijke map en daar `install.ps1` starten
//! (snelkoppelingen, bestandstypen, PATH en de vermelding in Instellingen → Apps; hetzelfde script als
//! voorheen, nu zonder venster). Draait op een eigen thread en meldt de voortgang via `report`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Een stap van de installatie.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Tekst en voortgang (0 tot 1).
    Step(String, f32),
    /// Klaar: de map waarin MathPrime staat.
    Done(PathBuf),
    Failed(String),
}

pub struct Job {
    pub files: Vec<(String, Vec<u8>)>,
    pub target: PathBuf,
    pub desktop: bool,
    /// Eerst wachten tot dit proces (MathPrime dat de update startte) gesloten is.
    pub wait_for: Option<u32>,
    pub english: bool,
    /// Alleen kopiëren (install.ps1 -AlleenBestanden), voor tests.
    pub files_only: bool,
}

/// De standaard installatiemap: %LOCALAPPDATA%\Programs\MathPrime.
pub fn default_target() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Programs")
        .join("MathPrime")
}

fn hidden(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Wacht tot proces `pid` weg is (hooguit tien minuten).
fn wait_for_exit(pid: u32) {
    let script = format!("Wait-Process -Id {pid} -Timeout 600 -ErrorAction SilentlyContinue");
    let _ = hidden(Command::new("powershell.exe").args(["-NoProfile", "-Command", &script])).status();
}

pub fn run(job: Job, report: &dyn Fn(Msg)) {
    match install(&job, report) {
        Ok(()) => report(Msg::Done(job.target.clone())),
        Err(e) => report(Msg::Failed(e)),
    }
}

fn install(job: &Job, report: &dyn Fn(Msg)) -> Result<(), String> {
    let tr = |nl: &str, en: &str| if job.english { en.to_string() } else { nl.to_string() };
    if let Some(pid) = job.wait_for {
        report(Msg::Step(tr("Wachten tot MathPrime gesloten is…", "Waiting for MathPrime to close…"), 0.0));
        wait_for_exit(pid);
    }
    let temp = std::env::temp_dir().join(format!("MathPrime-setup-{}", std::process::id()));
    let result = unpack_and_run(job, &temp, report, &tr);
    let _ = std::fs::remove_dir_all(&temp);
    result
}

fn unpack_and_run(job: &Job, temp: &Path, report: &dyn Fn(Msg), tr: &dyn Fn(&str, &str) -> String) -> Result<(), String> {
    std::fs::create_dir_all(temp).map_err(|e| format!("{}: {e}", temp.display()))?;
    let n = job.files.len().max(1);
    for (k, (name, data)) in job.files.iter().enumerate() {
        report(Msg::Step(format!("{} {name}", tr("Uitpakken:", "Unpacking:")), 0.1 + 0.5 * k as f32 / n as f32));
        // alleen platte namen: niets buiten de tijdelijke map
        if name.contains(['/', '\\']) || name.contains("..") {
            return Err(format!("{}: {name}", tr("ongeldige bestandsnaam in het pakket", "invalid file name in the package")));
        }
        std::fs::write(temp.join(name), data).map_err(|e| format!("{name}: {e}"))?;
    }
    report(Msg::Step(tr("Installeren, snelkoppelingen en bestandstypen…", "Installing, shortcuts and file types…"), 0.7));
    let script = temp.join("install.ps1");
    if !script.exists() {
        return Err(tr("install.ps1 ontbreekt in het pakket", "install.ps1 is missing from the package"));
    }
    let target = job.target.to_string_lossy().into_owned();
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]).arg(&script).args(["-Stil", "-Doel", &target]);
    if !job.desktop {
        cmd.arg("-GeenBureaublad");
    }
    if job.files_only {
        cmd.arg("-AlleenBestanden");
    }
    let out = hidden(&mut cmd).output().map_err(|e| format!("powershell.exe: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if text.is_empty() { err } else { text });
    }
    report(Msg::Step(tr("Klaar", "Done"), 1.0));
    Ok(())
}

/// Start de geïnstalleerde MathPrime.
pub fn launch(target: &Path) {
    let _ = Command::new(target.join("mathprime.exe")).spawn();
}

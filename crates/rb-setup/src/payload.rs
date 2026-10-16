//! De programmabestanden zitten achter in het installatieprogramma: `[exe][zip][lengte zip, u64 LE][MAGIC]`.
//! `mathprime-setup --pak <map> <uit.exe>` maakt zo'n pakket van het eigen exe-bestand en een map.

use std::path::Path;

const MAGIC: &[u8; 8] = b"MPSETUP1";

/// Plak de bestanden van `dir` (zonder submappen) als ZIP achter `stub`.
pub fn pack(stub: &[u8], dir: &Path) -> Result<Vec<u8>, String> {
    let mut files = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if e.path().is_file() {
            let data = std::fs::read(e.path()).map_err(|err| format!("{}: {err}", e.path().display()))?;
            files.push((e.file_name().to_string_lossy().into_owned(), data));
        }
    }
    if files.is_empty() {
        return Err(format!("{} is leeg", dir.display()));
    }
    Ok(attach(stub, &files))
}

/// Plak een ZIP met `files` achter `stub`.
pub fn attach(stub: &[u8], files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let zip = rb_zipxml::zip::write(files);
    let mut out = stub.to_vec();
    out.extend_from_slice(&zip);
    out.extend_from_slice(&(zip.len() as u64).to_le_bytes());
    out.extend_from_slice(MAGIC);
    out
}

/// De bestanden achter in `exe`; `None` als er geen pakket aan zit.
pub fn files(exe: &[u8]) -> Option<Result<Vec<(String, Vec<u8>)>, String>> {
    let n = exe.len();
    if n < 16 || &exe[n - 8..] != MAGIC {
        return None;
    }
    let len = u64::from_le_bytes(exe[n - 16..n - 8].try_into().ok()?) as usize;
    let start = (n - 16).checked_sub(len)?;
    Some(rb_zipxml::zip::read(&exe[start..n - 16]).map(|m| m.into_iter().collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heen_en_terug() {
        let stub = b"MZ dit is het programma".to_vec();
        assert!(files(&stub).is_none());
        let inhoud = vec![("versie.txt".to_string(), b"1.8.0".to_vec()), ("mathprime.exe".to_string(), vec![7u8; 5000])];
        let exe = attach(&stub, &inhoud);
        assert!(exe.starts_with(&stub));
        let mut terug = files(&exe).unwrap().unwrap();
        terug.sort();
        let mut verwacht = inhoud.clone();
        verwacht.sort();
        assert_eq!(terug, verwacht);
    }
}

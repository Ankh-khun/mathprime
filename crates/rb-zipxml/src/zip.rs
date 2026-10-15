//! Een minimale ZIP-lezer: de centrale map, en per bestand "stored" of "deflate".

use std::collections::BTreeMap;

fn u16_at(b: &[u8], k: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(k..k + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], k: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(k..k + 4)?.try_into().ok()?))
}

/// Alle bestanden in het archief: naam → inhoud. Namen zonder `/` vooraan.
pub fn read(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let bad = || "geen geldig ZIP-bestand (een .mcdx is een ZIP-archief)".to_string();
    // het einde van de centrale map: handtekening 0x06054b50, zoekend vanaf het eind
    let eocd = (0..bytes.len().saturating_sub(21)).rev().find(|&k| u32_at(bytes, k) == Some(0x0605_4b50)).ok_or_else(bad)?;
    let count = u16_at(bytes, eocd + 10).ok_or_else(bad)? as usize;
    let mut p = u32_at(bytes, eocd + 16).ok_or_else(bad)? as usize;
    let mut out = BTreeMap::new();
    for _ in 0..count {
        if u32_at(bytes, p) != Some(0x0201_4b50) {
            return Err(bad());
        }
        let method = u16_at(bytes, p + 10).ok_or_else(bad)?;
        let csize = u32_at(bytes, p + 20).ok_or_else(bad)? as usize;
        let usize_ = u32_at(bytes, p + 24).ok_or_else(bad)? as usize;
        let nlen = u16_at(bytes, p + 28).ok_or_else(bad)? as usize;
        let elen = u16_at(bytes, p + 30).ok_or_else(bad)? as usize;
        let clen = u16_at(bytes, p + 32).ok_or_else(bad)? as usize;
        let local = u32_at(bytes, p + 42).ok_or_else(bad)? as usize;
        let name = String::from_utf8_lossy(bytes.get(p + 46..p + 46 + nlen).ok_or_else(bad)?).trim_start_matches('/').to_string();
        p += 46 + nlen + elen + clen;
        if u32_at(bytes, local) != Some(0x0403_4b50) {
            return Err(bad());
        }
        let lnlen = u16_at(bytes, local + 26).ok_or_else(bad)? as usize;
        let lelen = u16_at(bytes, local + 28).ok_or_else(bad)? as usize;
        let start = local + 30 + lnlen + lelen;
        let data = bytes.get(start..start + csize).ok_or_else(bad)?;
        let content = match method {
            0 => data.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec(data).map_err(|e| format!("{name}: uitpakken mislukt ({e:?})"))?,
            m => return Err(format!("{name}: onbekende compressiemethode {m}")),
        };
        if content.len() != usize_ && method == 8 {
            return Err(format!("{name}: onverwachte grootte na uitpakken"));
        }
        if !name.ends_with('/') {
            out.insert(name, content);
        }
    }
    Ok(out)
}

/// CRC-32 (zoals in ZIP en PNG).
pub fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (n, t) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c = table[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// Schrijf een ZIP-archief (deflate). De volgorde van `files` blijft behouden.
pub fn write(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let packed = miniz_oxide::deflate::compress_to_vec(data, 6);
        let crc = crc32(data);
        let offset = out.len() as u32;
        // gemeenschappelijk deel van lokale en centrale kop: versie, vlaggen, methode, tijd, crc, groottes, naam
        let common = |v: &mut Vec<u8>| {
            v.extend_from_slice(&20u16.to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
            v.extend_from_slice(&8u16.to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
            v.extend_from_slice(&0x21u16.to_le_bytes()); // 1 januari 1980
            v.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&(packed.len() as u32).to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(name.len() as u16).to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
        };
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        common(&mut out);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&packed);
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        common(&mut central);
        central.extend_from_slice(&[0; 6]); // commentaar, schijf, interne attributen
        central.extend_from_slice(&0u32.to_le_bytes()); // externe attributen
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let (cd_offset, cd_size) = (out.len() as u32, central.len() as u32);
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn schrijven_en_teruglezen() {
        let files = vec![("a.txt".to_string(), b"hallo hallo hallo".to_vec()), ("map/b.xml".to_string(), vec![1, 2, 3, 0, 255])];
        let back = super::read(&super::write(&files)).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back["a.txt"], b"hallo hallo hallo");
        assert_eq!(back["map/b.xml"], vec![1, 2, 3, 0, 255]);
        // bekende CRC van "123456789"
        assert_eq!(super::crc32(b"123456789"), 0xCBF4_3926);
    }
}

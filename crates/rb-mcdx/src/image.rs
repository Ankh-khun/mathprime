//! BMP (zoals Mathcad soms afbeeldingen bewaart) omzetten naar PNG.

fn le16(b: &[u8], k: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(k..k + 2)?.try_into().ok()?))
}

fn le32(b: &[u8], k: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(k..k + 4)?.try_into().ok()?))
}

/// Een BMP (8, 24 of 32 bit, ongecomprimeerd) als RGBA: (breedte, hoogte, pixels).
pub fn bmp_to_rgba(b: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    if !b.starts_with(b"BM") {
        return None;
    }
    let offset = le32(b, 10)? as usize;
    let header = le32(b, 14)? as usize;
    let w = le32(b, 18)? as i32;
    let h = le32(b, 22)? as i32;
    let bpp = le16(b, 28)?;
    let compression = le32(b, 30)?;
    if w <= 0 || h == 0 || !(compression == 0 || (compression == 3 && bpp == 32)) {
        return None;
    }
    let (w, top_down, h) = (w as usize, h < 0, h.unsigned_abs() as usize);
    let palette: Vec<[u8; 4]> = if bpp == 8 {
        let n = match le32(b, 46)? {
            0 => 256,
            n => n as usize,
        };
        (0..n).filter_map(|k| b.get(14 + header + 4 * k..14 + header + 4 * k + 4).map(|c| [c[2], c[1], c[0], 255])).collect()
    } else {
        Vec::new()
    };
    let stride = (w * bpp as usize).div_ceil(32) * 4;
    let mut px = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = if top_down { y } else { h - 1 - y };
        let r = b.get(offset + row * stride..offset + row * stride + stride)?;
        for x in 0..w {
            let p = match bpp {
                8 => *palette.get(r[x] as usize)?,
                24 => [r[3 * x + 2], r[3 * x + 1], r[3 * x], 255],
                // de alfa van 32-bit BMP's is vaak 0; dan als ondoorzichtig behandelen
                32 => [r[4 * x + 2], r[4 * x + 1], r[4 * x], r[4 * x + 3]],
                _ => return None,
            };
            px.extend_from_slice(&p);
        }
    }
    if bpp == 32 && px.chunks(4).all(|p| p[3] == 0) {
        for p in px.chunks_mut(4) {
            p[3] = 255;
        }
    }
    Some((w, h, px))
}

/// RGBA als PNG (8 bit, zonder filters).
pub fn png(w: usize, h: usize, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(h * (1 + 4 * w));
    for y in 0..h {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * 4 * w..(y + 1) * 4 * w]);
    }
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crate::zip::crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 7));
    chunk(b"IEND", &[]);
    out
}

//! Een PNG-lezer (zonder interlacing): grijs, RGB, palet, met of zonder alfa, 1–16 bit.
//! Voor afbeeldingen op het werkblad en in de PDF.

/// Een afbeelding als RGBA, 8 bit per kanaal, rij voor rij.
#[derive(Debug, Clone, PartialEq)]
pub struct Rgba {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// Breedte en hoogte uit de kop, zonder de afbeelding uit te pakken.
pub fn size(png: &[u8]) -> Option<(usize, usize)> {
    if png.len() < 24 || &png[..8] != b"\x89PNG\r\n\x1a\n" || &png[12..16] != b"IHDR" {
        return None;
    }
    Some((be32(&png[16..20]) as usize, be32(&png[20..24]) as usize))
}

pub fn decode(png: &[u8]) -> Result<Rgba, String> {
    if png.len() < 8 || &png[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("geen PNG-afbeelding".into());
    }
    let mut p = 8;
    let (mut w, mut h, mut depth, mut ctype, mut interlace) = (0usize, 0usize, 0u8, 0u8, 0u8);
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut trns: Vec<u8> = Vec::new();
    let mut idat = Vec::new();
    while p + 8 <= png.len() {
        let len = be32(&png[p..]) as usize;
        let kind = &png[p + 4..p + 8];
        let data = png.get(p + 8..p + 8 + len).ok_or("PNG is afgebroken")?;
        match kind {
            b"IHDR" => {
                w = be32(data) as usize;
                h = be32(&data[4..]) as usize;
                depth = data[8];
                ctype = data[9];
                interlace = data[12];
            }
            b"PLTE" => palette = data.chunks(3).map(|c| [c[0], c[1], c[2]]).collect(),
            b"tRNS" => trns = data.to_vec(),
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        p += 12 + len;
    }
    if interlace != 0 {
        return Err("PNG met interlacing wordt niet ondersteund".into());
    }
    if w == 0 || h == 0 || w * h > 40_000_000 {
        return Err("PNG met een ongeldige grootte".into());
    }
    let channels = match ctype {
        0 => 1,
        2 => 3,
        3 => 1,
        4 => 2,
        6 => 4,
        _ => return Err(format!("onbekend PNG-kleurtype {ctype}")),
    };
    let bits = channels * depth as usize;
    let stride = (w * bits).div_ceil(8);
    let bpp = bits.div_ceil(8).max(1);
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&idat).map_err(|e| format!("PNG uitpakken mislukt ({e:?})"))?;
    if raw.len() < h * (stride + 1) {
        return Err("PNG-gegevens zijn te kort".into());
    }
    // filters ongedaan maken
    let mut img = vec![0u8; h * stride];
    for y in 0..h {
        let filter = raw[y * (stride + 1)];
        let src = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        let (prev, cur) = img.split_at_mut(y * stride);
        let prev = if y > 0 { &prev[(y - 1) * stride..] } else { &[][..] };
        let cur = &mut cur[..stride];
        for x in 0..stride {
            let a = if x >= bpp { cur[x - bpp] as i32 } else { 0 };
            let b = if y > 0 { prev[x] as i32 } else { 0 };
            let c = if x >= bpp && y > 0 { prev[x - bpp] as i32 } else { 0 };
            let v = src[x] as i32
                + match filter {
                    0 => 0,
                    1 => a,
                    2 => b,
                    3 => (a + b) / 2,
                    4 => {
                        let pp = a + b - c;
                        let (pa, pb, pc) = ((pp - a).abs(), (pp - b).abs(), (pp - c).abs());
                        if pa <= pb && pa <= pc {
                            a
                        } else if pb <= pc {
                            b
                        } else {
                            c
                        }
                    }
                    f => return Err(format!("onbekend PNG-filter {f}")),
                };
            cur[x] = v as u8;
        }
    }
    // naar RGBA
    let sample = |row: &[u8], i: usize| -> u16 {
        match depth {
            8 => row[i] as u16,
            16 => u16::from_be_bytes([row[2 * i], row[2 * i + 1]]),
            d => {
                let per = 8 / d as usize;
                let byte = row[i / per];
                let shift = 8 - d as usize * (i % per + 1);
                ((byte >> shift) & ((1 << d) - 1)) as u16
            }
        }
    };
    let to8 = |v: u16| -> u8 {
        match depth {
            16 => (v >> 8) as u8,
            8 => v as u8,
            d => (v as u32 * 255 / ((1u32 << d) - 1)) as u8,
        }
    };
    let mut pixels = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = &img[y * stride..(y + 1) * stride];
        for x in 0..w {
            let px = match ctype {
                0 => {
                    let v = sample(row, x);
                    let alpha = if trns.len() >= 2 && v == u16::from_be_bytes([trns[0], trns[1]]) { 0 } else { 255 };
                    let g = to8(v);
                    [g, g, g, alpha]
                }
                2 => {
                    let (r, g, b) = (sample(row, 3 * x), sample(row, 3 * x + 1), sample(row, 3 * x + 2));
                    let key = |k: usize| u16::from_be_bytes([trns[2 * k], trns[2 * k + 1]]);
                    let alpha = if trns.len() >= 6 && (r, g, b) == (key(0), key(1), key(2)) { 0 } else { 255 };
                    [to8(r), to8(g), to8(b), alpha]
                }
                3 => {
                    let i = sample(row, x) as usize;
                    let c = palette.get(i).copied().unwrap_or([0, 0, 0]);
                    [c[0], c[1], c[2], trns.get(i).copied().unwrap_or(255)]
                }
                4 => {
                    let g = to8(sample(row, 2 * x));
                    [g, g, g, to8(sample(row, 2 * x + 1))]
                }
                _ => [to8(sample(row, 4 * x)), to8(sample(row, 4 * x + 1)), to8(sample(row, 4 * x + 2)), to8(sample(row, 4 * x + 3))],
            };
            pixels.extend_from_slice(&px);
        }
    }
    Ok(Rgba { width: w, height: h, pixels })
}

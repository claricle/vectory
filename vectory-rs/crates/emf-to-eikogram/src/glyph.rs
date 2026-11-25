//! Glyph-index → Unicode mapping, mirroring the reference fontindex_to_utf8:
//! find the font file via fc-match (fontconfig), parse the font's cmap,
//! build the reverse map (glyph id → codepoint, last codepoint wins),
//! reverse for the RTL charsets, then truncate at the first null.

use std::collections::HashMap;
use std::process::Command;

/// ETO_GLYPH_INDEX per MS-WMF ExtTextOutOptions.
pub const ETO_GLYPH_INDEX: u32 = 0x0010;

const HEBREW_CHARSET: u8 = 177;
const ARABIC_CHARSET: u8 = 178;

/// Maps glyph ids through the font's reverse cmap. `None` mirrors the
/// reference returning NULL when the font cannot be found (empty CDATA).
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn map_glyphs(
    glyph_ids: &[u16],
    face_name: Option<&str>,
    weight: i32,
    italic: bool,
    charset: u8,
) -> Option<String> {
    let map = build_reverse_map(face_name, weight, italic)?;
    let codepoints: Vec<u32> = glyph_ids.iter().map(|gid| map.get(gid).copied().unwrap_or(0)).collect();

    let mut units: Vec<u16> = Vec::with_capacity(codepoints.len());
    for cp in codepoints {
        if let Ok(c) = u16::try_from(cp) {
            units.push(c);
        } else if let Some(s) = char::from_u32(cp) {
            // Encode as UTF-16 pair(s).
            let mut buf = [0u16; 2];
            units.extend_from_slice(s.encode_utf16(&mut buf));
            continue;
        } else {
            units.push(0);
        }
    }
    let mut text = decode_utf16_with_question_mark(&units);

    if charset == HEBREW_CHARSET || charset == ARABIC_CHARSET {
        text = text.chars().rev().collect();
    }

    if let Some(null_pos) = text.find('\0') {
        text.truncate(null_pos);
    }
    Some(text)
}

/// fc-match -f "%{file}\n" <family> — the reference's fontconfig lookup.
fn find_font_path(face_name: Option<&str>) -> Option<String> {
    let face = face_name?;
    if face.is_empty() {
        return None;
    }
    let output = Command::new("fc-match")
        .arg("-f")
        .arg("%{file}\n")
        .arg(face)
        .output()
        .ok()?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

fn build_reverse_map(
    face_name: Option<&str>,
    _weight: i32,
    _italic: bool,
) -> Option<HashMap<u16, u32>> {
    let path = find_font_path(face_name)?;
    let bytes = std::fs::read(&path).ok()?;
    parse_cmap_reverse(&bytes)
}

/// Parses the sfnt table directory, finds cmap, and builds the reverse map.
fn parse_cmap_reverse(bytes: &[u8]) -> Option<HashMap<u16, u32>> {
    if bytes.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let mut cmap: Option<(usize, usize)> = None;
    for i in 0..num_tables {
        let off = 12 + i * 16;
        if off + 16 > bytes.len() {
            break;
        }
        if &bytes[off..off + 4] == b"cmap" {
            let cmap_offset = u32::from_be_bytes([bytes[off + 8], bytes[off + 9], bytes[off + 10], bytes[off + 11]]) as usize;
            let cmap_length = u32::from_be_bytes([bytes[off + 12], bytes[off + 13], bytes[off + 14], bytes[off + 15]]) as usize;
            cmap = Some((cmap_offset, cmap_length));
            break;
        }
    }
    let (cmap_offset, cmap_length) = cmap?;
    let end = (cmap_offset + cmap_length).min(bytes.len());
    if cmap_offset >= end {
        return None;
    }
    let cmap_bytes = &bytes[cmap_offset..end];
    parse_unicode_mappings(cmap_bytes)
}

/// Unicode mappings (codepoint → glyph id) from the first usable Unicode
/// subtable, then reversed with last-codepoint-wins (matching FreeType's
/// FT_Get_Next_Char iteration order).
fn parse_unicode_mappings(data: &[u8]) -> Option<HashMap<u16, u32>> {
    if data.len() < 4 {
        return None;
    }
    let num_tables = u16::from_be_bytes([data[2], data[3]]) as usize;
    let mut chosen: Option<(usize, (u16, u16))> = None;
    for i in 0..num_tables {
        let rec = 4 + i * 8;
        if rec + 8 > data.len() {
            break;
        }
        let platform_id = u16::from_be_bytes([data[rec], data[rec + 1]]);
        let encoding_id = u16::from_be_bytes([data[rec + 2], data[rec + 3]]);
        let offset = u32::from_be_bytes([data[rec + 4], data[rec + 5], data[rec + 6], data[rec + 7]]) as usize;
        let is_unicode = (platform_id == 0)
            || (platform_id == 3 && (encoding_id == 1 || encoding_id == 10));
        if is_unicode {
            // Prefer the highest-offset (usually most complete) subtable,
            // matching the last-usable-record selection.
            chosen = match chosen {
                Some((_, existing)) if existing >= (platform_id, encoding_id) => chosen,
                _ => Some((offset, (platform_id, encoding_id))),
            };
        }
    }
    let (sub_offset, _) = chosen?;
    let sub = data.get(sub_offset..).unwrap_or(&[]);
    if sub.is_empty() {
        return None;
    }
    let format = u16::from_be_bytes([sub[0], sub[1]]);
    let mut forward: Vec<(u32, u16)> = Vec::new();
    match format {
        0 => {
            if sub.len() < 262 {
                return None;
            }
            for (cp, gid) in sub[4..260].iter().enumerate() {
                if *gid != 0 {
                    forward.push((cp as u32, u16::from(*gid)));
                }
            }
        }
        4 => {
            if sub.len() < 14 {
                return None;
            }
            let seg_count = u16::from_be_bytes([sub[6], sub[7]]) as usize / 2;
            let end_codes_at = 14;
            let start_codes_at = end_codes_at + seg_count * 2 + 2;
            let id_delta_at = start_codes_at + seg_count * 2;
            let id_range_offset_at = id_delta_at + seg_count * 2;
            for seg in 0..seg_count {
                let end_code = u16_at(sub, end_codes_at + seg * 2)?;
                let start_code = u16_at(sub, start_codes_at + seg * 2)?;
                let id_delta = u16_at(sub, id_delta_at + seg * 2)?;
                let id_range_offset = u16_at(sub, id_range_offset_at + seg * 2)?;
                if start_code == 0xFFFF {
                    continue;
                }
                for cp in start_code..=end_code {
                    let gid = if id_range_offset == 0 {
                        cp.wrapping_add(id_delta)
                    } else {
                        let glyph_index_at =
                            id_range_offset_at + seg * 2 + (usize::from(cp - start_code)) * 2 + usize::from(id_range_offset);
                        let raw = u16_at(sub, glyph_index_at)?;
                        if raw == 0 {
                            cp.wrapping_add(id_delta)
                        } else {
                            raw.wrapping_add(id_delta)
                        }
                    };
                    if gid != 0 {
                        forward.push((u32::from(cp), gid));
                    }
                }
            }
        }
        6 => {
            if sub.len() < 10 {
                return None;
            }
            let first = u16::from_be_bytes([sub[6], sub[7]]);
            let count = u16::from_be_bytes([sub[8], sub[9]]) as usize;
            for i in 0..count {
                let at = 10 + i * 4;
                let cp = u16_at(sub, at)?;
                let gid = u16_at(sub, at + 2)?;
                if gid != 0 {
                    forward.push((u32::from(first) + i as u32, gid));
                }
            }
        }
        12 => {
            if sub.len() < 16 {
                return None;
            }
            let n_groups = u32::from_be_bytes([sub[12], sub[13], sub[14], sub[15]]) as usize;
            for g in 0..n_groups {
                let at = 16 + g * 12;
                let start = u32_at(sub, at)?;
                let end = u32_at(sub, at + 4)?;
                let start_gid = u32_at(sub, at + 8)?;
                let span = end.saturating_sub(start).saturating_add(1);
                if span > 0x10_0000 {
                    continue;
                }
                for k in 0..span {
                    let gid = start_gid + k;
                    if gid != 0 && gid <= u32::from(u16::MAX) {
                        forward.push((start + k, gid as u16));
                    }
                }
            }
        }
        _ => return None,
    }

    let mut reverse: HashMap<u16, u32> = HashMap::new();
    for (cp, gid) in forward {
        // Last codepoint wins (matches FreeType's FT_Get_Next_Char).
        reverse.insert(gid, cp);
    }
    Some(reverse)
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    if at + 2 > data.len() {
        return None;
    }
    Some(u16::from_be_bytes([data[at], data[at + 1]]))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    if at + 4 > data.len() {
        return None;
    }
    Some(u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]))
}

/// UTF-16LE decode with the reference's `replace: "?"` behaviour (lone
/// surrogates become "?").
#[must_use]
pub fn decode_utf16_with_question_mark(units: &[u16]) -> String {
    let mut out = String::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        let unit = units[i];
        if (0xD800..0xDC00).contains(&unit) {
            if i + 1 < units.len() && (0xDC00..0xE000).contains(&units[i + 1]) {
                let cp = 0x10000 + ((u32::from(unit) - 0xD800) << 10) + (u32::from(units[i + 1]) - 0xDC00);
                if let Some(c) = char::from_u32(cp) {
                    out.push(c);
                }
                i += 2;
                continue;
            }
            out.push('?');
            i += 1;
            continue;
        }
        if (0xDC00..0xE000).contains(&unit) {
            out.push('?');
            i += 1;
            continue;
        }
        out.push(char::from_u32(u32::from(unit)).unwrap_or('?'));
        i += 1;
    }
    out
}

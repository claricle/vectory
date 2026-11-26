//! The raster payloads of the reverse lane: image DIBs (PNG and JPEG kept
//! in their original compression), pattern DIBs (decoded pixels re-encoded
//! as bottom-up 32-bpp BGRA), and the JPEG marker walk. The byte-exact
//! DIB/PNG machinery belongs to the emf workspace's bitmap lane
//! (claricle/emf#89); until that lane publishes, the pieces this lowering
//! needs are vendored here with provenance comments, to be replaced by the
//! crate dependency.

use eikogram::{Raster, RasterFormat};

/// Builds the DIB payload of an image record: the 40-byte header carrying
/// the original compression (BI_PNG / BI_JPEG) followed by the untouched
/// compressed bytes.
///
/// # Errors
///
/// Fails when the payload is not a supported image or its dimensions
/// cannot be read.
pub fn image_dib(raster: &Raster) -> Result<Vec<u8>, String> {
    match &raster.format {
        RasterFormat::Png => {
            let (width, height) = png_dims(&raster.payload)?;
            let mut dib = dib_header(width, height, 5, raster.payload.len() as u32);
            dib.extend_from_slice(&raster.payload);
            Ok(dib)
        }
        RasterFormat::Jpeg { .. } => {
            let (width, height) =
                jpeg_dims(&raster.payload).ok_or_else(|| "no JPEG frame".to_string())?;
            let mut dib = dib_header(width, height, 4, raster.payload.len() as u32);
            dib.extend_from_slice(&raster.payload);
            Ok(dib)
        }
    }
}

/// Builds the DIB payload of a raster pattern: the decoded pixels as a
/// bottom-up 32-bpp BGRA array under a BI_RGB header. The image's pixel
/// dimensions must equal the pattern's declared dimensions (validated at
/// ingest; re-checked here).
///
/// # Errors
///
/// Fails when the PNG payload cannot be decoded or its dimensions differ.
pub fn pattern_dib(raster: &Raster, width: i64, height: i64) -> Result<Vec<u8>, String> {
    let decoded = decode_png_rgba(&raster.payload)?;
    if i64::from(decoded.0) != width || i64::from(decoded.1) != height {
        return Err("pattern dimensions disagree with the payload".to_string());
    }
    let mut dib = dib_header(
        width as u32,
        height as u32,
        0,
        width as u32 * height as u32 * 4,
    );
    dib.extend_from_slice(&bgra_bottom_up(width as u32, height as u32, &decoded.2));
    Ok(dib)
}

/// The source dimensions of a built DIB (the header's width and height).
#[must_use]
pub fn dib_dims(dib: &[u8]) -> (u32, u32) {
    if dib.len() < 12 {
        return (0, 0);
    }
    let read = |at: usize| -> u32 {
        u32::from_le_bytes(dib[at..at + 4].try_into().expect("sliced above"))
    };
    (read(4), read(8))
}

/// The bitmap-info and bits buffers of a built DIB, in wire order.
#[must_use]
pub fn dib_buffers(dib: &[u8]) -> emf_core::BitmapBuffers {
    split_dib(dib)
}

/// The bitmap-info and bits buffers of a pattern DIB, in wire order.
#[must_use]
pub fn brush_buffers(dib: &[u8]) -> emf_core::BitmapBuffers {
    split_dib(dib)
}

fn split_dib(dib: &[u8]) -> emf_core::BitmapBuffers {
    let mut buffers = emf_core::BitmapBuffers::default();
    if dib.len() >= 40 {
        buffers.source_info = Some(emf_core::BitmapBuffer {
            padding: Vec::new(),
            data: dib[..40].to_vec(),
        });
        if dib.len() > 40 {
            buffers.source_bits = Some(emf_core::BitmapBuffer {
                padding: Vec::new(),
                data: dib[40..].to_vec(),
            });
        } else {
            buffers.source_bits = Some(emf_core::BitmapBuffer {
                padding: Vec::new(),
                data: Vec::new(),
            });
        }
    } else if !dib.is_empty() {
        buffers.source_bits = Some(emf_core::BitmapBuffer {
            padding: Vec::new(),
            data: dib.to_vec(),
        });
    }
    buffers
}

fn dib_header(width: u32, height: u32, compression: u32, image_bytes: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(40);
    let word = |out: &mut Vec<u8>, value: u32| out.extend_from_slice(&value.to_le_bytes());
    let half = |out: &mut Vec<u8>, value: u16| out.extend_from_slice(&value.to_le_bytes());
    word(&mut out, 40); // biSize
    word(&mut out, width); // biWidth
    word(&mut out, height); // biHeight (positive: bottom-up)
    half(&mut out, 1); // biPlanes
    half(&mut out, 32); // biBitCount
    word(&mut out, compression); // biCompression
    word(&mut out, image_bytes); // biSizeImage
    word(&mut out, 0); // biXPelsPerMeter
    word(&mut out, 0); // biYPelsPerMeter
    word(&mut out, 0); // biClrUsed
    word(&mut out, 0); // biClrImportant
    out
}

/// RGBA to bottom-up BGRA (rows reversed, channels swapped).
fn bgra_bottom_up(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let row = width as usize * 4;
    let mut out = Vec::with_capacity(rgba.len());
    for src_row in (0..height as usize).rev() {
        let start = src_row * row;
        let src = &rgba[start..start + row];
        for pixel in src.as_chunks::<4>().0 {
            out.push(pixel[2]);
            out.push(pixel[1]);
            out.push(pixel[0]);
            out.push(pixel[3]);
        }
    }
    out
}

// --- JPEG marker walk (the reference handler's frame search) ---

const JPEG_TERMINAL_MARKERS: [u8; 2] = [0xDA, 0xD9];
const JPEG_NON_FRAME_MARKERS: [u8; 3] = [0xC4, 0xC8, 0xCC];

/// The (width, height) of the first frame marker in a JPEG stream.
#[must_use]
pub fn jpeg_dims(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut offset = 2usize;
    while offset + 4 <= bytes.len() {
        if bytes[offset] != 0xFF {
            return None;
        }
        while bytes.get(offset) == Some(&0xFF) {
            offset += 1;
        }
        let marker = *bytes.get(offset)?;
        if JPEG_TERMINAL_MARKERS.contains(&marker) {
            return None;
        }
        if offset + 2 >= bytes.len() {
            return None;
        }
        let length = u16::from_be_bytes([bytes[offset + 1], bytes[offset + 2]]) as usize;
        if length < 2 || offset + 1 + length > bytes.len() {
            return None;
        }
        if (0xC0..=0xCF).contains(&marker) && !JPEG_NON_FRAME_MARKERS.contains(&marker) {
            if length < 8 {
                return None;
            }
            let height = u16::from_be_bytes([bytes[offset + 4], bytes[offset + 5]]) as u32;
            let width = u16::from_be_bytes([bytes[offset + 6], bytes[offset + 7]]) as u32;
            if width > 0 && height > 0 {
                return Some((width, height));
            }
            return None;
        }
        offset += 1 + length;
    }
    None
}

// --- PNG decode (the reference decoder's RGBA8 path) ---

/// The dimensions of a PNG payload (from its IHDR chunk).
///
/// # Errors
///
/// Fails when the signature or IHDR is missing or malformed.
pub fn png_dims(bytes: &[u8]) -> Result<(u32, u32), String> {
    let decoded = png_header(bytes)?;
    Ok((decoded.0, decoded.1))
}

fn png_header(bytes: &[u8]) -> Result<(u32, u32, u8, u8, bool), String> {
    const SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < 33 || bytes[..8] != SIGNATURE {
        return Err("not a PNG payload".to_string());
    }
    let read = |at: usize| -> u32 {
        u32::from_be_bytes(bytes[at..at + 4].try_into().expect("sliced above"))
    };
    if &bytes[12..16] != b"IHDR" {
        return Err("no IHDR chunk".to_string());
    }
    let width = read(16);
    let height = read(20);
    let depth = bytes[24];
    let color = bytes[25];
    let interlace = bytes[28];
    Ok((width, height, depth, color, interlace != 0))
}

/// Decodes a PNG payload to packed RGBA bytes.
///
/// # Errors
///
/// Fails on unsupported formats (interlaced, non-8-bit depths), corrupt
/// streams, or inflate failures.
pub fn decode_png_rgba(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let (width, height, depth, color, interlace) = png_header(bytes)?;
    if interlace || depth != 8 {
        return Err("unsupported PNG format (interlaced or non-8-bit)".to_string());
    }
    let channels = match color {
        0 => 1usize,
        2 => 3,
        3 => 1,
        4 => 2,
        6 => 4,
        _ => return Err("unsupported PNG colour type".to_string()),
    };
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();
    let mut offset = 8usize;
    while offset + 8 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("sliced above"))
            as usize;
        let kind = &bytes[offset + 4..offset + 8];
        let data_at = offset + 8;
        if data_at + length > bytes.len() {
            return Err("truncated PNG chunk".to_string());
        }
        match kind {
            b"PLTE" => {
                palette = bytes[data_at..data_at + length]
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|c| [c[0], c[1], c[2]])
                    .collect();
            }
            b"IDAT" => idat.extend_from_slice(&bytes[data_at..data_at + length]),
            b"IEND" => break,
            _ => {}
        }
        offset = data_at + length + 4;
    }
    let stride = width as usize * channels;
    let raw_len = (stride + 1) * height as usize;
    let raw = inflate(&idat, raw_len)?;
    let pixels = defilter(&raw, width as usize, height as usize, channels)?;
    Ok((
        width,
        height,
        to_rgba(pixels, width as usize, height as usize, channels, &palette),
    ))
}

fn to_rgba(
    pixels: Vec<u8>,
    width: usize,
    height: usize,
    channels: usize,
    palette: &[[u8; 3]],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(width * height * 4);
    for index in 0..width * height {
        let at = index * channels;
        match channels {
            1 => {
                // Grayscale or palette-indexed.
                let value = pixels[at];
                if palette.is_empty() {
                    out.extend_from_slice(&[value, value, value, 255]);
                } else {
                    let entry = palette.get(value as usize).copied().unwrap_or([0, 0, 0]);
                    out.extend_from_slice(&[entry[0], entry[1], entry[2], 255]);
                }
            }
            2 => {
                out.extend_from_slice(&[pixels[at], pixels[at], pixels[at], pixels[at + 1]]);
            }
            3 => {
                out.extend_from_slice(&[pixels[at], pixels[at + 1], pixels[at + 2], 255]);
            }
            _ => {
                out.extend_from_slice(&pixels[at..at + 4]);
            }
        }
    }
    out
}

/// Un-applies the five PNG row filters in place over the raw scanlines.
fn defilter(raw: &[u8], width: usize, height: usize, channels: usize) -> Result<Vec<u8>, String> {
    let stride = width * channels;
    let mut out = vec![0u8; stride * height];
    let mut at = 0usize;
    let mut previous: Vec<u8> = Vec::new();
    for row in 0..height {
        if at + 1 + stride > raw.len() {
            return Err("truncated PNG scanlines".to_string());
        }
        let filter = raw[at];
        let line = &raw[at + 1..at + 1 + stride];
        let mut target = vec![0u8; stride];
        for index in 0..stride {
            let a = if index >= channels {
                target[index - channels]
            } else {
                0
            };
            let b = previous.get(index).copied().unwrap_or(0);
            let c = if index >= channels {
                previous.get(index - channels).copied().unwrap_or(0)
            } else {
                0
            };
            target[index] = match filter {
                0 => line[index],
                1 => line[index].wrapping_add(a),
                2 => line[index].wrapping_add(b),
                3 => line[index].wrapping_add(((i32::from(a) + i32::from(b)) / 2) as u8),
                4 => {
                    let pa = i32::from(b) - i32::from(c);
                    let pb = i32::from(a) - i32::from(c);
                    let pc = pa + pb - i32::from(c);
                    let predictor = if pc < 0 {
                        pa.abs().min(pb.abs()).min(pc.abs())
                    } else {
                        pa.min(pb).min(pc)
                    };
                    let predicted = if predictor == pa.abs() {
                        a
                    } else if predictor == pb.abs() {
                        b
                    } else {
                        c
                    };
                    line[index].wrapping_add(predicted)
                }
                _ => return Err("unknown PNG filter".to_string()),
            };
        }
        out[row * stride..(row + 1) * stride].copy_from_slice(&target);
        previous = target;
        at += 1 + stride;
    }
    Ok(out)
}

/// One-shot zlib inflate of `data` into an exactly `expected`-byte buffer,
/// through libz (the same dependency the forward lane compresses with).
fn inflate(data: &[u8], expected: usize) -> Result<Vec<u8>, String> {
    let mut dest = vec![0u8; expected];
    let mut dest_len = u64::try_from(expected).map_err(|_| "size overflow".to_string())?;
    let status = unsafe {
        libz_sys::uncompress(
            dest.as_mut_ptr(),
            &mut dest_len,
            data.as_ptr(),
            data.len() as u64,
        )
    };
    if status != 0 {
        return Err("inflate failed".to_string());
    }
    if dest_len as usize != expected {
        return Err("inflate length mismatch".to_string());
    }
    Ok(dest)
}

//! Raster payload machinery, vendored from the emf workspace's dib/png lane
//! (claricle/emf#89, crates dib-model + png-model + dib-png) until that lane
//! publishes; at that point this module flips to the crate dependency. The
//! byte-exact contracts are that lane's: DIB decode mirrors the Ruby
//! DibDecoder (including the global alpha-zero rule); PNG encode reproduces
//! libpng 1.6's default write path (MAD filter heuristic, zlib level 6 with
//! Z_FILTERED, windowBits reduction, optimize_cmf, 8192-byte IDAT chunks).

/// What the DIB decode returns, mirroring the Ruby decoder's color types.
pub enum Decoded {
    /// An RGBA surface (row-major, top-down, width * height * 4 bytes).
    Rgba {
        width: i32,
        height: i32,
        rgba: Vec<u8>,
    },
    /// BI_JPEG: embedded JPEG bytes, passed through uninterpreted.
    Jpeg(Vec<u8>),
    /// BI_PNG: embedded PNG bytes, passed through uninterpreted.
    Png(Vec<u8>),
}

/// Decodes DIB bytes exactly as the Ruby ground truth does.
///
/// * `bits_offset` — the record's cb_bmi (the padded form reads pixels from
///   that offset into `dib`).
/// * `mono_palette` — the 1-bpp brush override (text/bk colours from the
///   device context), replacing the palette wholesale.
///
/// `None` mirrors the Ruby decoder returning nil.
#[must_use]
pub fn decode_dib(dib: &[u8], bits_offset: Option<usize>, mono_palette: Option<[[u8; 4]; 2]>) -> Option<Decoded> {
    if dib.len() < 40 {
        return None;
    }
    let header = BitmapInfoHeader::parse(dib);
    match header.compression {
        0 | 3 => decode_bi_rgb(dib, &header, bits_offset, mono_palette).map(|decoded| Decoded::Rgba {
            width: decoded.0,
            height: decoded.1,
            rgba: decoded.2,
        }),
        4 => Some(Decoded::Jpeg(payload_after_header(dib, &header))),
        5 => Some(Decoded::Png(payload_after_header(dib, &header))),
        _ => None,
    }
}

fn payload_after_header(dib: &[u8], header: &BitmapInfoHeader) -> Vec<u8> {
    let start = usize::try_from(header.header_size).unwrap_or(usize::MAX);
    if start > dib.len() {
        Vec::new()
    } else {
        dib[start..].to_vec()
    }
}

/// BITMAPINFOHEADER fields (the 40-byte window every decode reads).
pub struct BitmapInfoHeader {
    pub header_size: u32,
    pub width: i32,
    pub height: i32,
    pub bit_count: u16,
    pub compression: u32,
}

impl BitmapInfoHeader {
    /// Byte-safe read of the first 40 bytes (short reads yield zeros).
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Self {
        let u32_at = |at: usize| read_u32(bytes, at);
        BitmapInfoHeader {
            header_size: u32_at(0),
            width: u32_at(4) as i32,
            height: u32_at(8) as i32,
            bit_count: u16::try_from(u32_at(14) & 0xffff).unwrap_or(0),
            compression: u32_at(16),
        }
    }

    /// libuemf's `get_real_color_icount`: nominal palette size capped at the
    /// pixel area.
    #[must_use]
    pub fn real_palette_size(&self, bytes: &[u8]) -> u32 {
        let clr_used = read_u32(bytes, 32);
        let nominal = match self.bit_count {
            1 | 4 | 8 => {
                if clr_used > 0 {
                    clr_used
                } else {
                    1 << self.bit_count
                }
            }
            _ => 0,
        };
        if nominal == 0 {
            return 0;
        }
        let area = i64::from(self.width.abs()) * i64::from(self.height.abs());
        u32::try_from(i64::from(nominal).min(area)).unwrap_or(nominal)
    }
}

/// Ruby `decode_bi_rgb`. A negative width raises in the Ruby decoder and is
/// mirrored as `None`. Returns (width, height, rgba).
#[allow(clippy::type_complexity)]
fn decode_bi_rgb(
    dib: &[u8],
    header: &BitmapInfoHeader,
    bits_offset: Option<usize>,
    mono_palette: Option<[[u8; 4]; 2]>,
) -> Option<(i32, i32, Vec<u8>)> {
    if header.width < 0 {
        return None;
    }
    let width = header.width as usize;
    let height_abs = header.height.unsigned_abs() as usize;
    let top_down = header.height < 0;
    let bit_count = usize::from(header.bit_count);

    let pal_start = header.header_size as usize;
    let pal_entries = header.real_palette_size(dib) as usize;
    let palette: Vec<[u8; 4]> = match mono_palette {
        Some(mono) => mono.to_vec(),
        None => (0..pal_entries)
            .map(|i| {
                let at = pal_start + i * 4;
                let blue = getbyte(dib, at);
                let green = getbyte(dib, at + 1);
                let red = getbyte(dib, at + 2);
                let alpha = getbyte(dib, at + 3);
                [red, green, blue, alpha]
            })
            .collect(),
    };

    let pixel_offset = bits_offset.unwrap_or(pal_start + pal_entries * 4);
    let row_stride = (bit_count * width).div_ceil(32) * 4;
    let mut pixels = vec![0u8; width.checked_mul(height_abs)?.checked_mul(4)?];

    for row in 0..height_abs {
        let src_row = if top_down { row } else { height_abs - 1 - row };
        let row_start = pixel_offset + src_row * row_stride;
        match bit_count {
            24 => decode_rgb24_row(dib, row_start, width, &mut pixels, row * width * 4),
            32 => decode_rgb32_row(dib, row_start, width, &mut pixels, row * width * 4),
            16 => decode_rgb16_row(dib, row_start, width, &mut pixels, row * width * 4),
            8 => decode_indexed_row(dib, row_start, width, 1, &palette, &mut pixels, row * width * 4),
            4 => decode_indexed_row(dib, row_start, width, 2, &palette, &mut pixels, row * width * 4),
            1 => decode_indexed_row(dib, row_start, width, 8, &palette, &mut pixels, row * width * 4),
            _ => return None,
        }
    }

    // libemf2svg's rgb2png global alpha check: if every alpha byte is zero,
    // force all alpha to 0xFF.
    let alpha_all_zero = pixels.iter().skip(3).step_by(4).all(|a| *a == 0);
    if alpha_all_zero {
        for a in pixels.iter_mut().skip(3).step_by(4) {
            *a = 255;
        }
    }

    Some((header.width, header.height.unsigned_abs() as i32, pixels))
}

fn getbyte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    if at + 4 > bytes.len() {
        return 0;
    }
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn decode_rgb24_row(dib: &[u8], row_start: usize, width: usize, pixels: &mut [u8], out: usize) {
    for x in 0..width {
        let b = getbyte(dib, row_start + x * 3);
        let g = getbyte(dib, row_start + x * 3 + 1);
        let r = getbyte(dib, row_start + x * 3 + 2);
        let at = out + x * 4;
        pixels[at] = r;
        pixels[at + 1] = g;
        pixels[at + 2] = b;
        pixels[at + 3] = 255;
    }
}

fn decode_rgb32_row(dib: &[u8], row_start: usize, width: usize, pixels: &mut [u8], out: usize) {
    for x in 0..width {
        let b = getbyte(dib, row_start + x * 4);
        let g = getbyte(dib, row_start + x * 4 + 1);
        let r = getbyte(dib, row_start + x * 4 + 2);
        let a = getbyte(dib, row_start + x * 4 + 3);
        let at = out + x * 4;
        pixels[at] = r;
        pixels[at + 1] = g;
        pixels[at + 2] = b;
        pixels[at + 3] = a;
    }
}

/// 16-bpp BI_RGB is always 5:5:5 per GDI semantics (libuemf's shift shape);
/// alpha decodes to 0 and rides the global alpha-zero rule to 255.
fn decode_rgb16_row(dib: &[u8], row_start: usize, width: usize, pixels: &mut [u8], out: usize) {
    for x in 0..width {
        let lo = getbyte(dib, row_start + x * 2);
        let hi = getbyte(dib, row_start + x * 2 + 1);
        let b = (lo & 0x1F) << 3;
        let mut g = (lo >> 5) | ((hi & 0x03) << 3);
        g <<= 3;
        let r = (hi & 0x7C) << 1;
        let at = out + x * 4;
        pixels[at] = r;
        pixels[at + 1] = g;
        pixels[at + 2] = b;
        pixels[at + 3] = 0;
    }
}

/// `pixels_per_byte` is 8 (1-bpp), 2 (4-bpp) or 1 (8-bpp).
fn decode_indexed_row(
    dib: &[u8],
    row_start: usize,
    width: usize,
    pixels_per_byte: usize,
    palette: &[[u8; 4]],
    pixels: &mut [u8],
    out: usize,
) {
    for x in 0..width {
        let idx = match pixels_per_byte {
            8 => {
                let byte = getbyte(dib, row_start + x / 8);
                let shift = 7 - (x % 8);
                (byte >> shift) & 0x01
            }
            2 => {
                let byte = getbyte(dib, row_start + x / 2);
                let shift = if x % 2 == 0 { 4 } else { 0 };
                (byte >> shift) & 0x0F
            }
            _ => getbyte(dib, row_start + x),
        };
        let entry = palette.get(usize::from(idx)).copied().unwrap_or([0, 0, 0, 255]);
        let at = out + x * 4;
        pixels[at..at + 4].copy_from_slice(&entry);
    }
}

/// Encodes RGBA-8 rows as a PNG byte stream using libpng 1.6's default write
/// profile (see the module docs).
///
/// # Errors
///
/// Fails on zero dimensions or a short pixel buffer.
pub fn encode_png_rgba8(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    const ZBUF_SIZE: usize = 8192;
    if width == 0 || height == 0 {
        return Err("zero image dimension".to_string());
    }
    let row_bytes = usize::try_from(width).ok().and_then(|w| w.checked_mul(4)).ok_or("row overflow")?;
    if rgba.len() < row_bytes.checked_mul(height as usize).ok_or("image overflow")? {
        return Err("pixel buffer too short".to_string());
    }

    // png_image_size: uncompressed scanline stream.
    let image_size: u64 = if row_bytes < 32768 && height < 32768 {
        (row_bytes as u64 + 1) * u64::from(height)
    } else {
        0xffff_ffff
    };

    // png_deflate_claim: reduce windowBits while data fits the half window.
    let mut window_bits = 15;
    if image_size <= 16384 {
        let mut half_window_size: u64 = 1 << (window_bits - 1);
        while image_size + 262 <= half_window_size {
            half_window_size >>= 1;
            window_bits -= 1;
        }
    }

    // Filter selection per row (libpng: prev_row starts zeroed).
    let mut filtered = Vec::with_capacity(usize::try_from(image_size).unwrap_or(0));
    let mut prev = vec![0u8; row_bytes];
    for row in 0..height as usize {
        let start = row * row_bytes;
        let current = &rgba[start..start + row_bytes];
        let (filter_type, data) = find_filter(current, &prev);
        filtered.push(filter_type);
        filtered.extend_from_slice(&data);
        prev.copy_from_slice(current);
    }

    let mut compressed = zlib_compress(&filtered, 6, window_bits)?;
    optimize_cmf(&mut compressed, image_size);

    // Chunk assembly: IHDR, IDAT(s) split at the zbuffer size, IEND.
    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    push_chunk(&mut out, b"IHDR", &ihdr);
    for idat in compressed.chunks(ZBUF_SIZE) {
        push_chunk(&mut out, b"IDAT", idat);
    }
    push_chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&u32::try_from(data.len()).expect("chunk length").to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let table = crc_table();
    let mut crc = 0xffff_ffffu32;
    for byte in kind.iter().copied().chain(data.iter().copied()) {
        crc = table[((crc ^ u32::from(byte)) & 0xff) as usize] ^ (crc >> 8);
    }
    out.extend_from_slice(&(crc ^ 0xffff_ffff).to_be_bytes());
}

static CRC_TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();

fn crc_table() -> &'static [u32; 256] {
    CRC_TABLE.get_or_init(|| {
        let mut table = [0u32; 256];
        for (n, slot) in table.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *slot = c;
        }
        table
    })
}

/// libpng's `png_write_find_filter` MAD heuristic for the full filter set.
fn find_filter(row: &[u8], prev: &[u8]) -> (u8, Vec<u8>) {
    let bpp = 4; // RGBA-8
    let row_bytes = row.len();
    let mut mins: u64 = u64::MAX - 256;
    let mut best: Option<(u8, Vec<u8>)> = None;

    let mad = |v: u8| -> u64 {
        if v < 128 {
            u64::from(v)
        } else {
            u64::from(256 - u16::from(v))
        }
    };

    // NONE
    {
        let sum: u64 = row.iter().copied().map(mad).sum();
        if sum < mins {
            mins = sum;
            best = Some((0, row.to_vec()));
        }
    }

    // SUB
    {
        let mut data = Vec::with_capacity(row_bytes);
        let mut sum: u64 = 0;
        for (i, &x) in row.iter().enumerate() {
            let left = if i >= bpp { row[i - bpp] } else { 0 };
            let v = x.wrapping_sub(left);
            data.push(v);
            sum += mad(v);
        }
        if sum < mins {
            mins = sum;
            best = Some((1, data));
        }
    }

    // UP
    {
        let mut data = Vec::with_capacity(row_bytes);
        let mut sum: u64 = 0;
        for (i, &x) in row.iter().enumerate() {
            let v = x.wrapping_sub(prev[i]);
            data.push(v);
            sum += mad(v);
            if sum > mins {
                break;
            }
        }
        if sum < mins {
            mins = sum;
            best = Some((2, data));
        }
    }

    // AVG
    {
        let mut data = Vec::with_capacity(row_bytes);
        let mut sum: u64 = 0;
        for (i, &x) in row.iter().enumerate() {
            let a = if i >= bpp { row[i - bpp] } else { 0 };
            let b = prev[i];
            let v = x.wrapping_sub((((u16::from(a)) + u16::from(b)) / 2) as u8);
            data.push(v);
            sum += mad(v);
            if i >= bpp && sum > mins {
                break;
            }
        }
        if sum < mins {
            mins = sum;
            best = Some((3, data));
        }
    }

    // PAETH
    {
        let mut data = Vec::with_capacity(row_bytes);
        let mut sum: u64 = 0;
        for (i, &x) in row.iter().enumerate() {
            let a = if i >= bpp { row[i - bpp] } else { 0 };
            let b = prev[i];
            let c = if i >= bpp { prev[i - bpp] } else { 0 };
            let v = x.wrapping_sub(paeth_predictor(a, b, c));
            data.push(v);
            sum += mad(v);
            if i >= bpp && sum > mins {
                break;
            }
        }
        if sum < mins {
            best = Some((4, data));
        }
    }

    best.unwrap_or_else(|| (0, row.to_vec()))
}

fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let p = i16::from(a) + i16::from(b) - i16::from(c);
    let pa = p.abs_diff(i16::from(a));
    let pb = p.abs_diff(i16::from(b));
    let pc = p.abs_diff(i16::from(c));
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// libpng's `optimize_cmf`: shrink the zlib header's window-size field when
/// the uncompressed data fits a smaller window.
fn optimize_cmf(data: &mut [u8], data_size: u64) {
    if data_size <= 16384 && data.len() >= 2 {
        let z_cmf = data[0];
        if (z_cmf & 0x0f) == 8 && (z_cmf & 0xf0) <= 0x70 {
            let mut z_cinfo = z_cmf >> 4;
            let mut half_z_window_size: u64 = 1 << (z_cinfo + 7);
            if data_size <= half_z_window_size {
                loop {
                    half_z_window_size >>= 1;
                    z_cinfo -= 1;
                    if z_cinfo == 0 || data_size > half_z_window_size {
                        break;
                    }
                }
                let z_cmf = (z_cmf & 0x0f) | (z_cinfo << 4);
                data[0] = z_cmf;
                let mut tmp = u32::from(data[1] & 0xe0);
                tmp += 0x1f - ((u32::from(z_cmf) << 8) + tmp) % 0x1f;
                data[1] = tmp as u8;
            }
        }
    }
}

/// zlib deflate through stock zlib with libpng's write parameters.
fn zlib_compress(input: &[u8], level: i32, window_bits: i32) -> Result<Vec<u8>, String> {
    use std::os::raw::c_int;

    type ZStream = libz_sys::z_stream;
    const Z_NO_FLUSH: c_int = 0;
    const Z_FINISH: c_int = 4;
    const Z_OK: c_int = 0;
    const Z_STREAM_END: c_int = 1;
    const Z_DEFLATED: c_int = 8;
    const Z_FILTERED: c_int = 1;

    unsafe {
        let mut uninit = std::mem::MaybeUninit::<ZStream>::zeroed();
        let ret = libz_sys::deflateInit2_(
            uninit.as_mut_ptr(),
            level,
            Z_DEFLATED,
            window_bits,
            8, // memLevel: libpng default
            Z_FILTERED,
            libz_sys::zlibVersion(),
            std::mem::size_of::<ZStream>() as c_int,
        );
        if ret != Z_OK {
            return Err("deflateInit2 failed".to_string());
        }
        let strm = uninit.assume_init_mut();
        let mut out = Vec::with_capacity(input.len() / 2 + 64);
        let mut cursor = 0usize;
        loop {
            let avail_in = (input.len() - cursor).min(u32::MAX as usize);
            strm.next_in = input[cursor..].as_ptr().cast_mut().cast();
            strm.avail_in = avail_in as libz_sys::uInt;
            let flush = if cursor + avail_in == input.len() { Z_FINISH } else { Z_NO_FLUSH };
            loop {
                let before = out.len();
                strm.next_out = out.spare_capacity_mut().as_mut_ptr().cast();
                strm.avail_out = out.capacity().saturating_sub(before) as libz_sys::uInt;
                if strm.avail_out == 0 {
                    out.reserve(4096);
                    continue;
                }
                let ret = libz_sys::deflate(strm, flush);
                let written = out.capacity().saturating_sub(before) - strm.avail_out as usize;
                out.set_len(before + written);
                if ret == Z_STREAM_END {
                    libz_sys::deflateEnd(strm);
                    return Ok(out);
                }
                if ret != Z_OK {
                    libz_sys::deflateEnd(strm);
                    return Err("deflate failed".to_string());
                }
                if strm.avail_out == 0 {
                    out.reserve(4096);
                    continue;
                }
                if flush == Z_FINISH {
                    libz_sys::deflateEnd(strm);
                    return Err("deflate unfinished".to_string());
                }
                break;
            }
            cursor += avail_in;
        }
    }
}

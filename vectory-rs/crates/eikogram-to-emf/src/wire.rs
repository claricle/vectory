//! Wire framing for the EMF model: typed record payloads to byte streams.
//!
//! emf-core's document model at the pinned revision is parse-only (its
//! `Document` fields and payload-length computations are crate-private), so
//! a lowering cannot hand it constructed records directly. This module is
//! the constructor bridge: it frames the typed [`emf_core::RecordData`]
//! values the lowering built into the byte stream `emf_core::parse` accepts,
//! and every framed document goes through `emf_core::parse` +
//! `emf_core::serialize` before leaving this crate — a framing mistake
//! fails loudly as a parse or layout error instead of drifting silently.
//! When emf-core grows a public document builder (claricle/emf), this
//! module shrinks to that call.

use emf_core::{
    BitmapBuffer, BitmapRecord, EofRecord, ExtTextOut, ParseFailure, ParseLimits, RecordData,
    SerializeFailure, SerializeLimits,
};

/// A framing failure: a payload the wire layout cannot express.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameFailure {
    /// A record family whose payload this bridge cannot frame.
    Unsupported,
    /// A record payload length exceeds the wire size field.
    SizeOverflow,
    /// The framed document failed emf-core's checked parse (a layout bug).
    Parse(String),
    /// emf-core's serializer refused the parsed document.
    Serialize(String),
}

impl std::fmt::Display for FrameFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameFailure::Unsupported => write!(f, "record family not frameable by the bridge"),
            FrameFailure::SizeOverflow => write!(f, "record payload exceeds the wire size field"),
            FrameFailure::Parse(message) => write!(f, "framed document failed parse: {message}"),
            FrameFailure::Serialize(message) => {
                write!(f, "parsed document failed serialize: {message}")
            }
        }
    }
}

impl std::error::Error for FrameFailure {}

impl From<ParseFailure> for FrameFailure {
    fn from(failure: ParseFailure) -> Self {
        FrameFailure::Parse(failure.to_string())
    }
}

impl From<SerializeFailure> for FrameFailure {
    fn from(failure: SerializeFailure) -> Self {
        FrameFailure::Serialize(failure.to_string())
    }
}

/// Frames the header and records into bytes, validates them through
/// emf-core's checked parse, and returns emf-core's serialization of the
/// parsed document (the final byte stream).
///
/// # Errors
///
/// Fails when a payload cannot be framed or the framed document fails
/// emf-core's checked parse or serialize.
pub fn frame_document(
    header_bytes: &[u8],
    records: &[RecordData],
) -> Result<Vec<u8>, FrameFailure> {
    let mut out = Vec::new();
    out.extend_from_slice(header_bytes);
    for record in records {
        let payload = payload_bytes(record)?;
        let kind = record_kind(record).ok_or(FrameFailure::Unsupported)?;
        let mut size = 8usize
            .checked_add(payload.len())
            .ok_or(FrameFailure::SizeOverflow)?;
        let pad = if matches!(record, RecordData::Comment { .. }) {
            let pad = (4 - size % 4) % 4;
            size += pad;
            pad
        } else {
            0
        };
        let size = u32::try_from(size).map_err(|_| FrameFailure::SizeOverflow)?;
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&payload);
        out.resize(out.len() + pad, 0);
    }

    let document = emf_core::parse(&out, &ParseLimits::default())?;
    Ok(emf_core::serialize_with_limits(
        &document,
        &SerializeLimits::default(),
    )?)
}

/// The wire record type of a typed payload (the public mirror of
/// emf-core's crate-private mapping).
#[must_use]
pub fn record_kind(record: &RecordData) -> Option<u32> {
    Some(match record {
        RecordData::GradientFill(_) => 118,
        RecordData::Bitmap(b) => bitmap_kind(b),
        RecordData::RealizePalette => 52,
        RecordData::SelectPalette { .. } => 48,
        RecordData::CreatePalette { .. } => 49,
        RecordData::SetPaletteEntries { .. } => 50,
        RecordData::ResizePalette { .. } => 51,
        RecordData::CreateFontIndirectW(_) => 82,
        RecordData::ExtCreatePen(_) => 95,
        RecordData::ExtTextOutA(_) => 83,
        RecordData::ExtTextOutW(_) => 84,
        RecordData::PolyTextOutA(_) => 96,
        RecordData::PolyTextOutW(_) => 97,
        RecordData::SetColorAdjustment(_) => 23,
        RecordData::Raw => return None,
        RecordData::Eof(_) => 14,
        RecordData::ScaleViewportExtEx { .. } => 31,
        RecordData::ScaleWindowExtEx { .. } => 32,
        RecordData::SetWorldTransform { .. } => 35,
        RecordData::ModifyWorldTransform { .. } => 36,
        RecordData::AngleArc { .. } => 41,
        RecordData::SetMiterLimit { .. } => 58,
        RecordData::SetWindowExtEx { .. } => 9,
        RecordData::SetWindowOrgEx { .. } => 10,
        RecordData::SetViewportExtEx { .. } => 11,
        RecordData::SetViewportOrgEx { .. } => 12,
        RecordData::SetBrushOrgEx { .. } => 13,
        RecordData::OffsetClipRgn { .. } => 26,
        RecordData::MoveToEx { .. } => 27,
        RecordData::SetMapMode { .. } => 17,
        RecordData::SetBkMode { .. } => 18,
        RecordData::SetPolyFillMode { .. } => 19,
        RecordData::SetRop2 { .. } => 20,
        RecordData::SetStretchBltMode { .. } => 21,
        RecordData::SetTextColor { .. } => 24,
        RecordData::SetBkColor { .. } => 25,
        RecordData::PolyBezier { .. } => 2,
        RecordData::Polygon { .. } => 3,
        RecordData::Polyline { .. } => 4,
        RecordData::PolyBezierTo { .. } => 5,
        RecordData::PolylineTo { .. } => 6,
        RecordData::PolyPolyline { .. } => 7,
        RecordData::PolyPolygon { .. } => 8,
        RecordData::PolyPolyline16 { .. } => 90,
        RecordData::PolyPolygon16 { .. } => 91,
        RecordData::PolyBezier16 { .. } => 85,
        RecordData::Polygon16 { .. } => 86,
        RecordData::Polyline16 { .. } => 87,
        RecordData::PolyBezierTo16 { .. } => 88,
        RecordData::PolylineTo16 { .. } => 89,
        RecordData::PolyDraw { .. } => 56,
        RecordData::PolyDraw16 { .. } => 92,
        RecordData::FillRgn { .. } => 71,
        RecordData::FrameRgn { .. } => 72,
        RecordData::InvertRgn { .. } => 73,
        RecordData::PaintRgn { .. } => 74,
        RecordData::ExtSelectClipRgn { .. } => 75,
        RecordData::Comment { .. } => 70,
        RecordData::Ellipse { .. } => 42,
        RecordData::Rectangle { .. } => 43,
        RecordData::RoundRect { .. } => 44,
        RecordData::SetTextAlign { .. } => 22,
        RecordData::RestoreDc { .. } => 34,
        RecordData::SelectObject { .. } => 37,
        RecordData::DeleteObject { .. } => 40,
        RecordData::SetArcDirection { .. } => 57,
        RecordData::SelectClipPath { .. } => 67,
        RecordData::SetMapperFlags { .. } => 16,
        RecordData::SetLayout { .. } => 115,
        RecordData::SetIcmMode { .. } => 98,
        RecordData::ExtFloodFill { .. } => 53,
        RecordData::SetTextJustification { .. } => 120,
        RecordData::SetMetaRgn => 28,
        RecordData::FlattenPath => 65,
        RecordData::WidenPath => 66,
        RecordData::SaveDc => 33,
        RecordData::BeginPath => 59,
        RecordData::EndPath => 60,
        RecordData::CloseFigure => 61,
        RecordData::AbortPath => 68,
        RecordData::FillPath { .. } => 62,
        RecordData::StrokeAndFillPath { .. } => 63,
        RecordData::StrokePath { .. } => 64,
        RecordData::LineTo { .. } => 54,
        RecordData::Arc { .. } => 45,
        RecordData::ArcTo { .. } => 55,
        RecordData::Chord { .. } => 46,
        RecordData::Pie { .. } => 47,
        RecordData::ExcludeClipRect { .. } => 29,
        RecordData::IntersectClipRect { .. } => 30,
        RecordData::CreatePen { .. } => 38,
        RecordData::CreateBrushIndirect { .. } => 39,
        RecordData::SetPixelV { .. } => 15,
    })
}

fn bitmap_kind(bitmap: &BitmapRecord) -> u32 {
    match bitmap {
        BitmapRecord::BitBlt { .. } => 76,
        BitmapRecord::StretchBlt { .. } => 77,
        BitmapRecord::MaskBlt { .. } => 78,
        BitmapRecord::PlgBlt { .. } => 79,
        BitmapRecord::SetDibitsToDevice { .. } => 80,
        BitmapRecord::StretchDibits { .. } => 81,
        BitmapRecord::CreateMonoBrush { .. } => 93,
        BitmapRecord::CreateDibPatternBrushPt { .. } => 94,
        BitmapRecord::AlphaBlend { .. } => 114,
        BitmapRecord::TransparentBlt { .. } => 116,
    }
}

/// The full framed wire bytes of one record: the head, payload, and any
/// comment padding. Used for the drawing-hint fingerprints.
///
/// # Errors
///
/// [`FrameFailure::Unsupported`] for a family the bridge does not frame.
pub fn record_bytes(record: &RecordData) -> Vec<u8> {
    let payload = payload_bytes(record).unwrap_or_default();
    let kind = record_kind(record).unwrap_or(0);
    let mut size = 8usize + payload.len();
    let pad = if matches!(record, RecordData::Comment { .. }) {
        let pad = (4 - size % 4) % 4;
        size += pad;
        pad
    } else {
        0
    };
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    out.resize(out.len() + pad, 0);
    out
}

/// The payload bytes of one typed record (everything after the eight-byte
/// record head).
///
/// # Errors
///
/// [`FrameFailure::Unsupported`] for a family the bridge does not frame;
/// [`FrameFailure::SizeOverflow`] when a length exceeds the wire field.
pub fn payload_bytes(record: &RecordData) -> Result<Vec<u8>, FrameFailure> {
    let mut out = Vec::new();
    match record {
        RecordData::Raw
        | RecordData::RealizePalette
        | RecordData::SetMetaRgn
        | RecordData::FlattenPath
        | RecordData::WidenPath
        | RecordData::SaveDc
        | RecordData::BeginPath
        | RecordData::EndPath
        | RecordData::CloseFigure
        | RecordData::AbortPath => {}
        RecordData::Eof(EofRecord {
            n_pal_entries,
            off_pal_entries,
            n_size_last,
        }) => {
            word(&mut out, 12);
            word(&mut out, *n_pal_entries);
            word(&mut out, *off_pal_entries);
            word(&mut out, *n_size_last);
        }
        RecordData::ScaleViewportExtEx { scale } | RecordData::ScaleWindowExtEx { scale } => {
            word(&mut out, scale.x_num as u32);
            word(&mut out, scale.x_denom as u32);
            word(&mut out, scale.y_num as u32);
            word(&mut out, scale.y_denom as u32);
        }
        RecordData::SetWorldTransform { xform } => xform_bytes(&mut out, xform),
        RecordData::ModifyWorldTransform { xform, modify_mode } => {
            xform_bytes(&mut out, xform);
            word(&mut out, *modify_mode);
        }
        RecordData::SetMiterLimit { miter_limit } => word(&mut out, miter_limit.to_bits()),
        RecordData::SetWindowExtEx { extent } | RecordData::SetViewportExtEx { extent } => {
            size_bytes(&mut out, *extent)
        }
        RecordData::SetWindowOrgEx { origin }
        | RecordData::SetViewportOrgEx { origin }
        | RecordData::SetBrushOrgEx { origin }
        | RecordData::MoveToEx { origin }
        | RecordData::LineTo { origin } => point_bytes(&mut out, *origin),
        RecordData::OffsetClipRgn { offset } => point_bytes(&mut out, *offset),
        RecordData::SetMapMode { map_mode }
        | RecordData::SetBkMode { bk_mode: map_mode }
        | RecordData::SetPolyFillMode { poly_fill_mode: map_mode }
        | RecordData::SetRop2 { rop2: map_mode }
        | RecordData::SetStretchBltMode { stretch_mode: map_mode }
        | RecordData::SetTextAlign {
            text_align_mode: map_mode,
        }
        | RecordData::SetArcDirection {
            arc_direction: map_mode,
        }
        | RecordData::SelectClipPath {
            region_mode: map_mode,
        }
        | RecordData::SetMapperFlags { flags: map_mode }
        | RecordData::SetLayout { mode: map_mode }
        | RecordData::SetIcmMode { mode: map_mode } => word(&mut out, *map_mode),
        RecordData::SetTextColor { color } | RecordData::SetBkColor { color } => {
            color_bytes(&mut out, *color)
        }
        RecordData::PolyBezier { bounds, points }
        | RecordData::Polygon { bounds, points }
        | RecordData::Polyline { bounds, points }
        | RecordData::PolyBezierTo { bounds, points }
        | RecordData::PolylineTo { bounds, points } => {
            rect_bytes(&mut out, *bounds);
            word(&mut out, points.len() as u32);
            for point in points {
                point_bytes(&mut out, *point);
            }
        }
        RecordData::PolyPolyline {
            bounds,
            poly_counts,
            points,
        }
        | RecordData::PolyPolygon {
            bounds,
            poly_counts,
            points,
        } => {
            rect_bytes(&mut out, *bounds);
            word(&mut out, poly_counts.len() as u32);
            word(&mut out, points.len() as u32);
            for count in poly_counts {
                word(&mut out, *count);
            }
            for point in points {
                point_bytes(&mut out, *point);
            }
        }
        RecordData::Ellipse { bounds } | RecordData::Rectangle { bounds } => {
            rect_bytes(&mut out, *bounds)
        }
        RecordData::RoundRect { bounds, corner } => {
            rect_bytes(&mut out, *bounds);
            size_bytes(&mut out, *corner);
        }
        RecordData::Arc { bounds, start, end }
        | RecordData::ArcTo { bounds, start, end }
        | RecordData::Chord { bounds, start, end }
        | RecordData::Pie { bounds, start, end } => {
            rect_bytes(&mut out, *bounds);
            point_bytes(&mut out, *start);
            point_bytes(&mut out, *end);
        }
        RecordData::RestoreDc { saved_dc } => word(&mut out, *saved_dc as u32),
        RecordData::SelectObject { ih_object } | RecordData::DeleteObject { ih_object } => {
            word(&mut out, *ih_object)
        }
        RecordData::FillPath { bounds }
        | RecordData::StrokePath { bounds }
        | RecordData::StrokeAndFillPath { bounds } => rect_bytes(&mut out, *bounds),
        RecordData::ExcludeClipRect { clip } | RecordData::IntersectClipRect { clip } => {
            rect_bytes(&mut out, *clip)
        }
        RecordData::CreatePen {
            ih_pen,
            pen_style,
            width,
            color,
        } => {
            word(&mut out, *ih_pen);
            word(&mut out, *pen_style);
            point_bytes(&mut out, *width);
            color_bytes(&mut out, *color);
        }
        RecordData::CreateBrushIndirect {
            ih_brush,
            brush_style,
            color,
            brush_hatch,
        } => {
            word(&mut out, *ih_brush);
            word(&mut out, *brush_style);
            color_bytes(&mut out, *color);
            word(&mut out, *brush_hatch);
        }
        RecordData::ExtSelectClipRgn { region_mode, region } => {
            let data_len = region
                .as_ref()
                .map_or(0, |r| 32 + r.rectangles.len() * 16 + r.trailing_bytes.len());
            word(&mut out, data_len as u32);
            word(&mut out, *region_mode);
            if let Some(region) = region {
                region_bytes(&mut out, region);
            }
        }
        RecordData::Comment { data } => {
            word(&mut out, data.len() as u32);
            out.extend_from_slice(data);
        }
        RecordData::CreateFontIndirectW(font_record) => font_bytes(&mut out, font_record)?,
        RecordData::ExtTextOutW(text) => text_bytes_w(&mut out, text)?,
        RecordData::ExtTextOutA(text) => text_bytes_a(&mut out, text)?,
        RecordData::Bitmap(bitmap) => bitmap_bytes(&mut out, bitmap)?,
        _ => return Err(FrameFailure::Unsupported),
    }
    Ok(out)
}

fn region_bytes(out: &mut Vec<u8>, region: &emf_core::RegionData) {
    word(out, region.region_type);
    word(out, region.rectangles.len() as u32);
    word(
        out,
        (32 + region.rectangles.len() * 16 + region.trailing_bytes.len()) as u32,
    );
    rect_bytes(out, region.bounds);
    for rectangle in &region.rectangles {
        rect_bytes(out, *rectangle);
    }
    out.extend_from_slice(&region.trailing_bytes);
}

fn font_bytes(
    out: &mut Vec<u8>,
    font_record: &emf_core::FontRecord,
) -> Result<(), FrameFailure> {
    let emf_core::Font::LogFont(log_font) = &font_record.font else {
        return Err(FrameFailure::Unsupported);
    };
    word(out, font_record.ih_object);
    word(out, log_font.height as u32);
    word(out, log_font.width as u32);
    word(out, log_font.escapement as u32);
    word(out, log_font.orientation as u32);
    word(out, log_font.weight as u32);
    out.push(log_font.italic);
    out.push(log_font.underline);
    out.push(log_font.strike_out);
    out.push(log_font.char_set);
    out.push(log_font.out_precision);
    out.push(log_font.clip_precision);
    out.push(log_font.quality);
    out.push(log_font.pitch_and_family);
    for unit in &log_font.face_name {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    Ok(())
}

/// The text record layout emf-core decodes: `(string_offset, dx_offset)`
/// relative to the payload start, plus the trailing payload end. Mirrors
/// the pinned emf-core layout computation for the shapes this crate
/// writes.
fn text_layout<T>(text: &ExtTextOut<T>) -> Option<(usize, Option<usize>, usize)> {
    let unit = std::mem::size_of::<T>();
    let mut end: usize = if text.rectangle.is_some() { 76 } else { 60 };
    let mut string = 0;
    let mut dx = None;
    for is_dx in [text.dx_first, !text.dx_first] {
        if is_dx {
            if let Some(values) = &text.dx {
                end += text.dx_padding.len();
                end += (4 - end % 4) % 4;
                dx = Some(end);
                end += values.len().checked_mul(4)?;
            }
        } else if text.string_present || !text.text.is_empty() {
            end += text.string_padding.len();
            end += (unit - end % unit) % unit;
            string = end;
            end += text.text.len().checked_mul(unit)?;
        }
    }
    Some((string, dx, end))
}

fn text_bytes_w(out: &mut Vec<u8>, text: &ExtTextOut<u16>) -> Result<(), FrameFailure> {
    text_bytes_impl(out, text, &|out, unit| {
        out.extend_from_slice(&unit.to_le_bytes());
    })
}

fn text_bytes_a(out: &mut Vec<u8>, text: &ExtTextOut<u8>) -> Result<(), FrameFailure> {
    text_bytes_impl(out, text, &|out: &mut Vec<u8>, unit: &u8| {
        out.push(*unit);
    })
}

fn text_bytes_impl<T>(
    out: &mut Vec<u8>,
    text: &ExtTextOut<T>,
    write_unit: &dyn Fn(&mut Vec<u8>, &T),
) -> Result<(), FrameFailure> {
    let (string_at, dx_at, _end) = text_layout(text).ok_or(FrameFailure::SizeOverflow)?;
    rect_bytes(out, text.bounds);
    word(out, text.graphics_mode);
    word(out, text.ex_scale.to_bits());
    word(out, text.ey_scale.to_bits());
    point_bytes(out, text.reference);
    word(out, text.text.len() as u32);
    word(out, string_at as u32);
    word(out, text.options);
    if let Some(rectangle) = &text.rectangle {
        rect_bytes(out, *rectangle);
    }
    word(out, dx_at.map_or(0, |at| at as u32));
    for is_dx in [text.dx_first, !text.dx_first] {
        if is_dx {
            if let Some(values) = &text.dx {
                out.resize(out.len() + text.dx_padding.len(), 0);
                out.resize(dx_at.unwrap_or(0), 0);
                for value in values {
                    word(out, *value);
                }
            }
        } else if text.string_present || !text.text.is_empty() {
            out.resize(out.len() + text.string_padding.len(), 0);
            out.resize(string_at, 0);
            for unit in &text.text {
                write_unit(out, unit);
            }
        }
    }
    Ok(())
}

/// The fixed payload sizes (after the record head) of the bitmap families
/// this bridge frames; buffer offsets follow emf-core's `shape` table.
fn bitmap_fixed_payload(kind: u32) -> Option<usize> {
    match kind {
        76 => Some(92),
        81 => Some(72),
        94 => Some(24),
        _ => None,
    }
}

fn bitmap_bytes(out: &mut Vec<u8>, bitmap: &BitmapRecord) -> Result<(), FrameFailure> {
    let kind = bitmap.record_type();
    let fixed = bitmap_fixed_payload(kind).ok_or(FrameFailure::Unsupported)?;
    let buffers = bitmap.buffers();
    // The bridge writes contiguous, unpadded buffers: info first, then bits.
    let info_present = buffers.source_info.is_some();
    let bits_present = buffers.source_bits.is_some();
    let mut cursor = fixed;
    let info_offset = info_present.then(|| {
        let at = cursor;
        cursor += buffers
            .source_info
            .as_ref()
            .map_or(0, |b| b.data.len() + b.padding.len());
        at
    });
    let bits_offset = bits_present.then(|| {
        let at = cursor;
        cursor += buffers
            .source_bits
            .as_ref()
            .map_or(0, |b| b.data.len() + b.padding.len());
        at
    });
    let info_len = |buffers: &emf_core::BitmapBuffers| {
        buffers
            .source_info
            .as_ref()
            .map_or(0, |b: &BitmapBuffer| b.data.len() as u32)
    };
    let bits_len = |buffers: &emf_core::BitmapBuffers| {
        buffers
            .source_bits
            .as_ref()
            .map_or(0, |b: &BitmapBuffer| b.data.len() as u32)
    };
    let off = |at: Option<usize>| at.map_or(0, |value| value as u32);

    match bitmap {
        BitmapRecord::CreateDibPatternBrushPt { ih_brush, usage, .. } => {
            word(out, *ih_brush);
            word(out, *usage);
            word(out, off(info_offset));
            word(out, info_len(buffers));
            word(out, off(bits_offset));
            word(out, bits_len(buffers));
        }
        BitmapRecord::StretchDibits {
            transfer,
            raster_operation,
            destination_extent,
        } => {
            rect_bytes(out, transfer.bounds);
            point_bytes(out, transfer.destination);
            point_bytes(out, transfer.source);
            point_bytes(out, transfer.source_extent);
            word(out, off(info_offset));
            word(out, info_len(buffers));
            word(out, off(bits_offset));
            word(out, bits_len(buffers));
            word(out, transfer.usage);
            word(out, *raster_operation);
            point_bytes(out, *destination_extent);
        }
        BitmapRecord::BitBlt {
            transfer,
            raster_operation,
        } => {
            rect_bytes(out, transfer.bounds);
            point_bytes(out, transfer.destination);
            point_bytes(out, transfer.destination_extent);
            word(out, *raster_operation);
            point_bytes(out, transfer.source.position);
            xform_bytes(out, &transfer.source.transform);
            color_bytes(out, transfer.source.background_color);
            word(out, transfer.source.usage);
            word(out, off(info_offset));
            word(out, info_len(buffers));
            word(out, off(bits_offset));
            word(out, bits_len(buffers));
        }
        _ => return Err(FrameFailure::Unsupported),
    }
    for buffer in [buffers.source_info.as_ref(), buffers.source_bits.as_ref()] {
        if let Some(BitmapBuffer { padding, data }) = buffer {
            out.resize(out.len() + padding.len(), 0);
            out.extend_from_slice(data);
        }
    }
    Ok(())
}

fn word(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn point_bytes(out: &mut Vec<u8>, point: emf_core::Point) {
    word(out, point.x as u32);
    word(out, point.y as u32);
}

fn rect_bytes(out: &mut Vec<u8>, rect: emf_core::Rect) {
    word(out, rect.left as u32);
    word(out, rect.top as u32);
    word(out, rect.right as u32);
    word(out, rect.bottom as u32);
}

fn size_bytes(out: &mut Vec<u8>, size: emf_core::Size) {
    word(out, size.cx as u32);
    word(out, size.cy as u32);
}

fn color_bytes(out: &mut Vec<u8>, color: emf_core::ColorRef) {
    out.push(color.red);
    out.push(color.green);
    out.push(color.blue);
    out.push(color.reserved);
}

fn xform_bytes(out: &mut Vec<u8>, xform: &emf_core::XForm) {
    for value in [
        xform.m11,
        xform.m12,
        xform.m21,
        xform.m22,
        xform.dx,
        xform.dy,
    ] {
        word(out, value.to_bits());
    }
}

/// The EOF record the translation emits.
#[must_use]
pub fn eof() -> RecordData {
    RecordData::Eof(EofRecord {
        n_pal_entries: 0,
        off_pal_entries: 16,
        n_size_last: 20,
    })
}

/// UTF-16 code units of a string, in order.
#[must_use]
pub fn utf16_units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

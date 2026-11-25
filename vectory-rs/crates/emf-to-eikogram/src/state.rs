//! GDI state: the device-context snapshot (with the coordinate state shared
//! across snapshots), the 1-indexed object table, and the non-popping save/
//! restore stack — ported from the reference renderer's containers.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use eikogram::Color;

/// Coordinate state that lives OUTSIDE the device-context stack in the
/// reference implementation (map mode, window/viewport extents and origins).
/// SaveDC/RestoreDC never saves/restores it: every snapshot observes the
/// same live values through the shared reference.
#[derive(Clone, Debug)]
pub struct CoordinateState {
    pub map_mode: u32,
    pub window_ext: (i32, i32),
    pub viewport_ext: (i32, i32),
    pub window_org: (i32, i32),
    pub viewport_org: (i32, i32),
    pub window_ext_set: bool,
    pub viewport_ext_set: bool,
}

impl Default for CoordinateState {
    fn default() -> Self {
        CoordinateState {
            map_mode: 1, // MM_TEXT
            window_ext: (0, 0),
            viewport_ext: (0, 0),
            window_org: (0, 0),
            viewport_org: (0, 0),
            window_ext_set: false,
            viewport_ext_set: false,
        }
    }
}

/// Pen value object: style, width, colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Pen {
    pub style: u32,
    pub width: i32,
    pub color: Color,
}

impl Default for Pen {
    fn default() -> Self {
        Pen { style: 0, width: 1, color: Color::black() }
    }
}

impl Pen {
    pub fn null() -> Self {
        Pen { style: 5, width: 1, color: Color::black() }
    }
    pub fn solid(style: u32, width: i32, color: Color) -> Self {
        Pen { style, width, color }
    }
    pub fn is_null(&self) -> bool {
        self.style & 0xFF == 5
    }
}

/// Brush value object: style, colour, hatch, and the pattern-image reference
/// (monopattern brushes reference a defined raster pattern).
#[derive(Clone, Debug, PartialEq)]
pub struct Brush {
    pub style: u32,
    pub color: Color,
    pub hatch: u32,
    pub image_id: Option<u32>,
}

impl Default for Brush {
    fn default() -> Self {
        Brush { style: 0, color: Color::black(), hatch: 0, image_id: None }
    }
}

impl Brush {
    pub fn solid(color: Color) -> Self {
        Brush { style: 0, color, hatch: 0, image_id: None }
    }
    pub fn null() -> Self {
        Brush { style: 1, color: Color::white(), hatch: 0, image_id: None }
    }
    pub fn is_null(&self) -> bool {
        self.style == 1
    }
}

/// Font value object: the LOGFONT subset the reference renderer carries.
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    pub face_name: Option<String>,
    pub height: i32,
    pub weight: i32,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub escapement: i32,
    pub charset: u8,
}

impl Default for Font {
    fn default() -> Self {
        Font {
            face_name: None,
            height: 0,
            weight: 0,
            italic: false,
            underline: false,
            strikeout: false,
            escapement: 0,
            charset: 0,
        }
    }
}

/// A clip region: rectangular bounds (as carried coordinates) or a captured
/// path geometry.
#[derive(Clone, Debug, PartialEq)]
pub enum ClipRegion {
    Bounds(emf_core::Rect),
    Path(eikogram::PathGeometry),
}

/// Snapshot of GDI drawing state. Snapshots share the coordinate state.
#[derive(Clone, Debug)]
pub struct DeviceContext {
    pub pen: Pen,
    pub brush: Brush,
    pub font: Font,
    pub text_color: Color,
    pub bk_color: Color,
    pub bk_mode: u32,
    pub poly_fill_mode: u32,
    pub text_align: u32,
    pub stretch_mode: u32,
    pub miter_limit: f64,
    /// The world transform as the six EMF wire values.
    pub world_transform: [f32; 6],
    pub brush_origin: (i32, i32),
    pub icm_mode: u32,
    pub layout: u32,
    pub mapper_flags: u32,
    pub arc_direction: i32,
    pub clip_region: Option<ClipRegion>,
    pub clip_id: Option<u32>,
    /// Shared live coordinate state (never snapshotted).
    pub coordinate_state: Rc<RefCell<CoordinateState>>,
}

impl Default for DeviceContext {
    fn default() -> Self {
        DeviceContext {
            pen: Pen::default(),
            brush: Brush::default(),
            font: Font::default(),
            text_color: Color::black(),
            bk_color: Color::white(),
            bk_mode: 1, // TRANSPARENT
            poly_fill_mode: 1,
            text_align: 0,
            stretch_mode: 1,
            miter_limit: 10.0,
            world_transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            brush_origin: (0, 0),
            icm_mode: 0,
            layout: 0,
            mapper_flags: 0,
            arc_direction: 0,
            clip_region: None,
            clip_id: None,
            coordinate_state: Rc::new(RefCell::new(CoordinateState::default())),
        }
    }
}

/// A stored GDI object.
#[derive(Clone, Debug, PartialEq)]
pub enum GdiObject {
    Pen(Pen),
    Brush(Brush),
    Font(Font),
}

/// Object table keyed by EMF handle (1-indexed in practice).
#[derive(Default)]
pub struct ObjectTable {
    entries: HashMap<u32, GdiObject>,
}

impl ObjectTable {
    pub fn store(&mut self, index: u32, object: GdiObject) {
        self.entries.insert(index, object);
    }

    pub fn lookup(&self, index: u32) -> Option<&GdiObject> {
        self.entries.get(&index)
    }

    pub fn delete(&mut self, index: u32) {
        self.entries.remove(&index);
    }
}

/// SaveDC/RestoreDC stack. Restore WALKS the stack to the target level but
/// does NOT remove entries (Windows GDI level semantics).
#[derive(Default)]
pub struct SaveStack {
    stack: Vec<DeviceContext>,
}

impl SaveStack {
    pub fn push(&mut self, dc: &DeviceContext) {
        self.stack.push(dc.clone());
    }

    /// The snapshot at the target level; the stack is not modified.
    /// Negative N counts back from the top (-1 = top); positive N is the
    /// 1-based absolute level.
    pub fn restore(&self, saved_dc: i32) -> Option<&DeviceContext> {
        let target_index = if saved_dc < 0 {
            self.stack.len() as i64 + i64::from(saved_dc)
        } else {
            i64::from(saved_dc) - 1
        };
        if target_index < 0 || target_index >= self.stack.len() as i64 {
            return None;
        }
        self.stack.get(target_index as usize)
    }
}

/// The BSD libc rand() sequence (seed 1) that drives clip-path IDs. The
/// reference reads 5000 precomputed values; the table ships as data.
pub struct BsdRand {
    values: Vec<u32>,
    index: usize,
}

impl Default for BsdRand {
    fn default() -> Self {
        BsdRand { values: load_sequence(), index: 0 }
    }
}

impl BsdRand {
    pub fn next(&mut self) -> u32 {
        let value = self.values.get(self.index).copied().unwrap_or(0);
        self.index += 1;
        value
    }
}

fn load_sequence() -> Vec<u32> {
    let raw = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/bsd_rand_sequence.txt"
    ));
    raw.lines().filter_map(|line| line.trim().parse::<u32>().ok()).collect()
}

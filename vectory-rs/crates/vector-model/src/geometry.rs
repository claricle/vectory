//! Geometry vocabulary: points and matrices in output space (y-down document
//! pixels), path segments, and the linear join style of authored geometry.
#![allow(dead_code)]

/// A 2D affine matrix in output space. `dx`/`dy` are already resolved into
/// output units by the producing transformer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix {
    pub m11: f64,
    pub m12: f64,
    pub m21: f64,
    pub m22: f64,
    pub dx: f64,
    pub dy: f64,
}

impl Matrix {
    #[must_use]
    pub fn identity() -> Self {
        Matrix {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            dx: 0.0,
            dy: 0.0,
        }
    }
}

/// How coordinate pairs join in the linear form of authored geometry. Both
/// styles exist in reference authoring; the pair separator differs, and the
/// choice is carried so serialization is byte-deterministic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordJoin {
    /// `x,y ` — a comma joins the pair, a space terminates it.
    Tuple,
    /// `x y ` — a space joins and terminates.
    Sequence,
}

/// One path segment in output space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    MoveTo {
        x: f64,
        y: f64,
    },
    LineTo {
        x: f64,
        y: f64,
    },
    CubicTo {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
    },
    /// An elliptical arc to the endpoint. The axis rotation is carried (it is
    /// always a whole number in reference geometry) alongside the two
    /// extent-choice flags.
    ArcTo {
        rx: f64,
        ry: f64,
        rotation: f64,
        large: bool,
        sweep: bool,
        x: f64,
        y: f64,
    },
    Close,
}

/// Authored path geometry: an ordered segment list plus the pair-join style
/// of its authoring family.
#[derive(Clone, Debug, PartialEq)]
pub struct PathGeometry {
    pub segments: Vec<Segment>,
    pub join: CoordJoin,
}

impl PathGeometry {
    #[must_use]
    pub fn new(join: CoordJoin) -> Self {
        PathGeometry {
            segments: Vec::new(),
            join,
        }
    }

    pub fn push(&mut self, segment: Segment) {
        self.segments.push(segment);
    }
}

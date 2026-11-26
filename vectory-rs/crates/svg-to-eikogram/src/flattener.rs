//! The path flattener: supported commands resolved to absolute points
//! without joining figures, and the elliptical-arc to cubic conversion,
//! mirroring the reference `PathFlattener` and `EllipticalArc`.

/// One flattened subpath: its shape family, absolute points, bounds, and
/// its segment list (letter + resolved points per segment).
#[derive(Clone, Debug, PartialEq)]
pub struct SubPath {
    pub shape_kind: ShapeKind,
    pub points: Vec<(f64, f64)>,
    pub bounds: Bounds,
    pub segments: Vec<(String, Vec<(f64, f64)>)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Polygon,
    Polyline,
    PolyBezier,
    Path,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Bounds {
    #[must_use]
    pub fn empty() -> Self {
        Bounds {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        }
    }
}

/// Flattens commands into subpaths.
#[must_use]
pub fn flatten(
    commands: &[crate::path_data::Command],
    preserve_initial_line: bool,
) -> Vec<SubPath> {
    let mut pen = PenState::default();
    let mut subpaths: Vec<SubPath> = Vec::new();
    let mut segments: Vec<(String, Vec<(f64, f64)>)> = Vec::new();
    for command in commands {
        let letter = command.upcased();
        if letter == "M" && !segments.is_empty() {
            subpaths.push(build_result(std::mem::take(&mut segments)));
        }
        if segments.is_empty()
            && letter != "M"
            && !(preserve_initial_line && subpaths.is_empty() && letter == "L")
        {
            segments.push(("M".to_string(), vec![pen.current]));
        }
        if letter == "A" {
            for (kind, points) in resolve_arc(&mut pen, command) {
                segments.push((
                    if kind == SegmentKind::Line {
                        "L".to_string()
                    } else {
                        "C".to_string()
                    },
                    points,
                ));
            }
            continue;
        }
        let points = resolve_command(&mut pen, command);
        if let Some(points) = points {
            segments.push((letter.clone(), points));
        }
        if letter != "Z" {
            continue;
        }
        if let Some((_, first_points)) = segments.first() {
            if let Some(first) = first_points.first() {
                pen.close_to(*first);
            }
        }
        subpaths.push(build_result(std::mem::take(&mut segments)));
    }
    if !segments.is_empty() {
        subpaths.push(build_result(segments));
    }
    subpaths
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SegmentKind {
    Line,
    Cubic,
}

fn resolve_command(
    pen: &mut PenState,
    command: &crate::path_data::Command,
) -> Option<Vec<(f64, f64)>> {
    match command.upcased().as_str() {
        "M" | "L" => {
            pen.line_to(command);
            Some(vec![pen.current])
        }
        "H" => {
            pen.h_line_to(command);
            Some(vec![pen.current])
        }
        "V" => {
            pen.v_line_to(command);
            Some(vec![pen.current])
        }
        "C" => {
            pen.cubic_to(command);
            Some(std::mem::take(&mut pen.pending_curve_points))
        }
        "Z" => Some(Vec::new()),
        _ => None,
    }
}

fn build_result(segments: Vec<(String, Vec<(f64, f64)>)>) -> SubPath {
    let points: Vec<(f64, f64)> = segments
        .iter()
        .flat_map(|(_, points)| points.iter().copied())
        .collect();
    let letters: Vec<&str> = segments.iter().map(|(letter, _)| letter.as_str()).collect();
    let shape_kind = if letters.contains(&"C") {
        if letters
            .iter()
            .all(|letter| *letter == "M" || *letter == "C")
        {
            ShapeKind::PolyBezier
        } else {
            ShapeKind::Path
        }
    } else if letters.last() == Some(&"Z") {
        ShapeKind::Polygon
    } else {
        ShapeKind::Polyline
    };
    SubPath {
        shape_kind,
        bounds: bounds_for(&points),
        points,
        segments,
    }
}

#[must_use]
pub fn bounds_for(points: &[(f64, f64)]) -> Bounds {
    if points.is_empty() {
        return Bounds::empty();
    }
    let (mut left, mut top, mut right, mut bottom) =
        (points[0].0, points[0].1, points[0].0, points[0].1);
    for (x, y) in points {
        left = left.min(*x);
        right = right.max(*x);
        top = top.min(*y);
        bottom = bottom.max(*y);
    }
    Bounds {
        left,
        top,
        right,
        bottom,
    }
}

/// The pen state: current position and pending bezier control points.
#[derive(Default)]
struct PenState {
    current: (f64, f64),
    pending_curve_points: Vec<(f64, f64)>,
}

impl PenState {
    fn close_to(&mut self, point: (f64, f64)) {
        self.current = point;
    }

    fn line_to(&mut self, command: &crate::path_data::Command) {
        let (x, y) = (command.args[0], command.args[1]);
        self.current = if command.absolute() {
            (x, y)
        } else {
            (self.current.0 + x, self.current.1 + y)
        };
    }

    fn h_line_to(&mut self, command: &crate::path_data::Command) {
        let x = command.args[0];
        self.current = if command.absolute() {
            (x, self.current.1)
        } else {
            (self.current.0 + x, self.current.1)
        };
    }

    fn v_line_to(&mut self, command: &crate::path_data::Command) {
        let y = command.args[0];
        self.current = if command.absolute() {
            (self.current.0, y)
        } else {
            (self.current.0, self.current.1 + y)
        };
    }

    fn cubic_to(&mut self, command: &crate::path_data::Command) {
        let (x1, y1, x2, y2, x, y) = (
            command.args[0],
            command.args[1],
            command.args[2],
            command.args[3],
            command.args[4],
            command.args[5],
        );
        if command.absolute() {
            self.pending_curve_points = vec![(x1, y1), (x2, y2), (x, y)];
            self.current = (x, y);
        } else {
            let (cx, cy) = self.current;
            self.pending_curve_points =
                vec![(cx + x1, cy + y1), (cx + x2, cy + y2), (cx + x, cy + y)];
            self.current = (cx + x, cy + y);
        }
    }
}

/// The segment kinds an arc resolves to.
fn resolve_arc(
    pen: &mut PenState,
    command: &crate::path_data::Command,
) -> Vec<(SegmentKind, Vec<(f64, f64)>)> {
    let cubics = elliptical_arc_to_cubics(pen.current, &command.args, command.absolute());
    pen.set_current((command.args[5], command.args[6]), command.absolute());
    cubics
        .into_iter()
        .map(|(kind, points)| {
            (
                if kind {
                    SegmentKind::Line
                } else {
                    SegmentKind::Cubic
                },
                points,
            )
        })
        .collect()
}

impl PenState {
    fn set_current(&mut self, point: (f64, f64), absolute: bool) {
        self.current = if absolute {
            point
        } else {
            (self.current.0 + point.0, self.current.1 + point.1)
        };
    }
}

/// Converts an endpoint-form elliptical arc to cubic segments. `true`
/// kinds are degenerate lines.
#[must_use]
pub fn elliptical_arc_to_cubics(
    start: (f64, f64),
    args: &[f64],
    absolute: bool,
) -> Vec<(bool, Vec<(f64, f64)>)> {
    let finish = endpoint(start, args, absolute);
    if start == finish {
        return Vec::new();
    }
    let rx = args[0];
    let ry = args[1];
    let rotation = args[2];
    let large_arc = args[3];
    let sweep = args[4];
    if rx == 0.0 || ry == 0.0 {
        return vec![(true, vec![finish])];
    }
    let geometry = ellipse_geometry(
        start,
        finish,
        rx.abs(),
        ry.abs(),
        rotation,
        large_arc,
        sweep,
    );
    cubic_segments(&geometry, finish)
}

fn endpoint(start: (f64, f64), args: &[f64], absolute: bool) -> (f64, f64) {
    let (x, y) = (args[5], args[6]);
    if absolute {
        (x, y)
    } else {
        (start.0 + x, start.1 + y)
    }
}

struct ArcGeometry {
    center: (f64, f64),
    rx: f64,
    ry: f64,
    cos_phi: f64,
    sin_phi: f64,
    theta: f64,
    delta: f64,
}

fn ellipse_geometry(
    start: (f64, f64),
    finish: (f64, f64),
    mut rx: f64,
    mut ry: f64,
    rotation: f64,
    large_arc: f64,
    sweep: f64,
) -> ArcGeometry {
    let phi = rotation * std::f64::consts::PI / 180.0;
    let (cos_phi, sin_phi) = phi.sin_cos();
    let dx = (start.0 - finish.0) / 2.0;
    let dy = (start.1 - finish.1) / 2.0;
    let x1 = cos_phi * dx + sin_phi * dy;
    let y1 = -sin_phi * dx + cos_phi * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let factor = lambda.sqrt();
        rx *= factor;
        ry *= factor;
    }
    let (cx1, cy1) = center_offset(rx, ry, x1, y1, large_arc, sweep);
    let center = center_point(start, finish, cx1, cy1, cos_phi, sin_phi);
    let (theta, delta) = arc_angles(rx, ry, x1, y1, cx1, cy1, large_arc, sweep);
    ArcGeometry {
        center,
        rx,
        ry,
        cos_phi,
        sin_phi,
        theta,
        delta,
    }
}

fn center_offset(rx: f64, ry: f64, x1: f64, y1: f64, large_arc: f64, sweep: f64) -> (f64, f64) {
    let numerator =
        ((rx * rx) * (ry * ry) - (rx * rx) * (y1 * y1) - (ry * ry) * (x1 * x1)).max(0.0);
    let denominator = (rx * rx) * (y1 * y1) + (ry * ry) * (x1 * x1);
    let mut factor = if denominator == 0.0 {
        0.0
    } else {
        (numerator / denominator).sqrt()
    };
    if large_arc == sweep {
        factor = -factor;
    }
    (factor * rx * y1 / ry, -factor * ry * x1 / rx)
}

fn center_point(
    start: (f64, f64),
    finish: (f64, f64),
    cx1: f64,
    cy1: f64,
    cos_phi: f64,
    sin_phi: f64,
) -> (f64, f64) {
    (
        cos_phi * cx1 - sin_phi * cy1 + (start.0 + finish.0) / 2.0,
        sin_phi * cx1 + cos_phi * cy1 + (start.1 + finish.1) / 2.0,
    )
}

#[allow(clippy::too_many_arguments)]
fn arc_angles(
    rx: f64,
    ry: f64,
    x1: f64,
    y1: f64,
    cx1: f64,
    cy1: f64,
    large_arc: f64,
    sweep: f64,
) -> (f64, f64) {
    let ux = (x1 - cx1) / rx;
    let uy = (y1 - cy1) / ry;
    let vx = (-x1 - cx1) / rx;
    let vy = (-y1 - cy1) / ry;
    let theta = uy.atan2(ux);
    let mut delta = (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let two_pi = 2.0 * std::f64::consts::PI;
    if sweep == 0.0 && delta > 0.0 {
        delta -= two_pi;
    }
    if sweep != 0.0 && delta < 0.0 {
        delta += two_pi;
    }
    if large_arc != 0.0 && delta.abs() < std::f64::consts::PI {
        delta += if sweep == 0.0 { -two_pi } else { two_pi };
    }
    if large_arc == 0.0 && delta.abs() > std::f64::consts::PI {
        delta += if sweep == 0.0 { two_pi } else { -two_pi };
    }
    (theta, delta)
}

fn cubic_segments(geometry: &ArcGeometry, finish: (f64, f64)) -> Vec<(bool, Vec<(f64, f64)>)> {
    let count = (geometry.delta.abs() / (std::f64::consts::PI / 2.0)).ceil() as usize;
    let step = geometry.delta / count as f64;
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let a = geometry.theta + index as f64 * step;
        let b = a + step;
        let alpha = (4.0 / 3.0) * ((b - a) / 4.0).tan();
        let (point_a, derivative_a) = point_and_derivative(geometry, a);
        let (point_b, derivative_b) = point_and_derivative(geometry, b);
        let point_b = if index == count - 1 { finish } else { point_b };
        out.push((
            false,
            vec![
                (
                    point_a.0 + alpha * derivative_a.0,
                    point_a.1 + alpha * derivative_a.1,
                ),
                (
                    point_b.0 - alpha * derivative_b.0,
                    point_b.1 - alpha * derivative_b.1,
                ),
                point_b,
            ],
        ));
    }
    out
}

fn point_and_derivative(geometry: &ArcGeometry, angle: f64) -> ((f64, f64), (f64, f64)) {
    let (cos_angle, sin_angle) = angle.sin_cos();
    let point = (
        geometry.center.0 + geometry.rx * geometry.cos_phi * cos_angle
            - geometry.ry * geometry.sin_phi * sin_angle,
        geometry.center.1
            + geometry.rx * geometry.sin_phi * cos_angle
            + geometry.ry * geometry.cos_phi * sin_angle,
    );
    let derivative = (
        -geometry.rx * geometry.cos_phi * sin_angle - geometry.ry * geometry.sin_phi * cos_angle,
        -geometry.rx * geometry.sin_phi * sin_angle + geometry.ry * geometry.cos_phi * cos_angle,
    );
    (point, derivative)
}

/// The clip-side flattener: absolute M/L/H/V/Z only; relative commands and
/// curves fall through (the caller treats the shape as non-rectangular).
pub struct PathFlatten;

impl PathFlatten {
    /// The flattened absolute points, or `None` for unsupported commands.
    #[must_use]
    pub fn flatten_for_clip(commands: &[crate::path_data::Command]) -> Option<Vec<(f64, f64)>> {
        let mut points = Vec::new();
        let mut current = (0.0_f64, 0.0_f64);
        for command in commands {
            match command.upcased().as_str() {
                "M" | "L" => {
                    current = (command.args[0], command.args[1]);
                    points.push(current);
                }
                "H" => {
                    current = (command.args[0], current.1);
                    points.push(current);
                }
                "V" => {
                    current = (current.0, command.args[0]);
                    points.push(current);
                }
                "Z" => {}
                _ => return None,
            }
        }
        Some(points)
    }
}

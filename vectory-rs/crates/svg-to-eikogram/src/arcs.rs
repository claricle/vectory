//! Native circular arc recovery: the renderer's exact arc spellings that
//! map back to the arc/chord/pie families, including the mapped forms
//! whose radii carry a temporary device origin. Mirrors the reference
//! `CircularArc`, `MappedArc`, and `MappedPie`.

use crate::path_data::Command;

/// The recovered arc: every value the lowering emits, in canvas units
/// (pre-scale; the ingest applies the mapping and origin shifts).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedArc {
    pub kind: ArcFamily,
    pub sweep: bool,
    /// The entry move point.
    pub prefix: (f64, f64),
    /// The exit move point (the finish; the start for chord and pie).
    pub finish: (f64, f64),
    pub center: (f64, f64),
    pub radius: f64,
    /// The two endpoint radials (the start and end directions).
    pub radials: [(f64, f64); 2],
    /// The mapped family's device origin (subtracted from every value
    /// before scaling).
    pub origin: Option<(f64, f64)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcFamily {
    Arc,
    Chord,
    Pie,
}

/// The supported command-letter shapes per family.
fn supported_commands(commands: &[Command], kind: ArcFamily) -> bool {
    let letters: Vec<&str> = commands.iter().map(|c| c.letter.as_str()).collect();
    match kind {
        ArcFamily::Arc => letters == ["M", "M", "A"],
        ArcFamily::Chord => letters == ["M", "M", "A", "Z"],
        ArcFamily::Pie => letters == ["M", "M", "A", "L", "Z"],
    }
}

fn point_of(command: &Command) -> Option<(f64, f64)> {
    Some((
        command.args.first().copied()?,
        command.args.get(1).copied()?,
    ))
}

/// Recovers a native circle from the exact arc spelling.
#[must_use]
pub fn from_commands(commands: &[Command], kind: ArcFamily) -> Option<ResolvedArc> {
    if !supported_commands(commands, kind) {
        return None;
    }
    let prefix = point_of(commands.first()?)?;
    let start = point_of(commands.get(1)?)?;
    let args = &commands.get(2)?.args;
    if args.len() < 7 {
        return None;
    }
    let (radius, ry, rotation, large, sweep, x, y) = (
        args[0], args[1], args[2], args[3], args[4], args[5], args[6],
    );
    if radius <= 0.0 || radius != ry || radius != radius.round() {
        return None;
    }
    if rotation != 0.0 || large != sweep || !matches!(sweep, 0.0 | 1.0) {
        return None;
    }
    let finish = (x, y);
    let center = circle_center(start, finish, radius)?;
    if kind == ArcFamily::Pie && point_of(commands.get(3)?) != Some(center) {
        return None;
    }
    let radials = [start, finish].map(|point| {
        let radial = (point.0 - center.0, point.1 - center.1);
        (radial.0.round(), radial.1.round())
    });
    for (point, radial) in [start, finish].iter().zip(radials.iter()) {
        if !projects_to(*point, *radial, center, radius) {
            return None;
        }
    }
    Some(ResolvedArc {
        kind,
        sweep: sweep == 1.0,
        prefix,
        finish: if kind == ArcFamily::Arc {
            finish
        } else {
            start
        },
        center,
        radius,
        radials,
        origin: None,
    })
}

fn circle_center(start: (f64, f64), finish: (f64, f64), radius: f64) -> Option<(f64, f64)> {
    let (x, y) = finish;
    let (dx, dy) = (finish.0 - start.0, finish.1 - start.1);
    let distance = dx.hypot(dy);
    if !(distance > 0.0 && distance <= 2.0 * radius) {
        return None;
    }
    let height = ((radius * radius) - (distance * distance) / 4.0)
        .max(0.0)
        .sqrt();
    let center = (
        (start.0 + x) / 2.0 + (dy * height) / distance,
        (start.1 + y) / 2.0 - (dx * height) / distance,
    );
    if (center.0 - center.0.round()).abs() < 0.001 && (center.1 - center.1.round()).abs() < 0.001 {
        Some((center.0.round(), center.1.round()))
    } else {
        None
    }
}

fn projects_to(point: (f64, f64), radial: (f64, f64), center: (f64, f64), radius: f64) -> bool {
    let length = radial.0.hypot(radial.1);
    if length <= 0.0 {
        return false;
    }
    let check = |value: f64, origin: f64, delta: f64| -> bool {
        crate::number::fmt4(value) == crate::number::fmt4(origin + (radius * delta) / length)
    };
    check(point.0, center.0, radial.0) && check(point.1, center.1, radial.1)
}

/// The int32 range the scalers bound values by.
pub const INT32_RANGE: std::ops::RangeInclusive<i64> = i32::MIN as i64..=i32::MAX as i64;

/// Recovers a mapped near-semicircle: an integer centre offset from the
/// midpoint whose mapping origin reproduces the endpoint rounding.
///
/// `scaled` is the canvas mapping factor; the recovery only runs on the
/// fixed scale.
#[must_use]
pub fn mapped_arc(commands: &[Command], factor: u64) -> Option<ResolvedArc> {
    if factor == 0 || !supported_commands(commands, ArcFamily::Arc) {
        return None;
    }
    let prefix = point_of(commands.first()?)?;
    let start = point_of(commands.get(1)?)?;
    let args = &commands.get(2)?.args;
    if args.len() < 7 {
        return None;
    }
    let (rx, ry, rotation, large, sweep, x, y) = (
        args[0], args[1], args[2], args[3], args[4], args[5], args[6],
    );
    if rotation != 0.0 || large != sweep || !matches!(sweep, 0.0 | 1.0) {
        return None;
    }
    let finish = (x, y);
    let midpoint = ((start.0 + finish.0) / 2.0, (start.1 + finish.1) / 2.0);
    for offset_x in [-1.0, 0.0, 1.0] {
        for offset_y in [-1.0, 0.0, 1.0] {
            let center = (midpoint.0 + offset_x, midpoint.1 + offset_y);
            let radius = (start.0 - center.0).hypot(start.1 - center.1).round();
            if radius <= 0.0 {
                continue;
            }
            let origin = (rx - radius, ry - radius);
            let integral = origin.0 == origin.0.trunc()
                && origin.1 == origin.1.trunc()
                && INT32_RANGE.contains(&(origin.0 as i64))
                && INT32_RANGE.contains(&(origin.1 as i64));
            if !integral {
                continue;
            }
            let origin = (origin.0.trunc(), origin.1.trunc());
            let radial_start = recover_radial(start, center, radius, origin, factor);
            let radial_finish = recover_radial(finish, center, radius, origin, factor);
            let (Some(radial_start), Some(radial_finish)) = (radial_start, radial_finish) else {
                continue;
            };
            return Some(ResolvedArc {
                kind: ArcFamily::Arc,
                sweep: sweep == 1.0,
                prefix,
                finish,
                center,
                radius,
                radials: [radial_start, radial_finish],
                origin: Some(origin),
            });
        }
    }
    None
}

/// Recovers the mapped pie: the C formatter maps radii as points, adding
/// the viewport origin; recover it only when an integer circle reproduces
/// every byte.
#[must_use]
pub fn mapped_pie(commands: &[Command], factor: u64) -> Option<ResolvedArc> {
    if factor == 0 || !supported_commands(commands, ArcFamily::Pie) {
        return None;
    }
    let prefix = point_of(commands.first()?)?;
    let start = point_of(commands.get(1)?)?;
    let args = &commands.get(2)?.args;
    let center = point_of(commands.get(3)?)?;
    if args.len() < 7 {
        return None;
    }
    let (rx, ry, rotation, large, sweep, x, y) = (
        args[0], args[1], args[2], args[3], args[4], args[5], args[6],
    );
    if rotation != 0.0 || large != sweep || !matches!(sweep, 0.0 | 1.0) {
        return None;
    }
    let distance = (start.0 - center.0).hypot(start.1 - center.1);
    let origin = ((rx - distance).round(), (ry - distance).round());
    let radius = rx - origin.0;
    if radius <= 0.0 || (radius - (ry - origin.1)).abs() >= 1e-5 {
        return None;
    }
    if !INT32_RANGE.contains(&(origin.0 as i64)) || !INT32_RANGE.contains(&(origin.1 as i64)) {
        return None;
    }
    let radial_start = recover_radial(start, center, radius, origin, factor)?;
    let radial_end = recover_radial((x, y), center, radius, origin, factor)?;
    Some(ResolvedArc {
        kind: ArcFamily::Pie,
        sweep: sweep == 1.0,
        prefix,
        finish: start,
        center,
        radius,
        radials: [radial_start, radial_end],
        origin: Some(origin),
    })
}

/// Inverts the C circle mapping while preserving endpoint rounding and
/// local state: a bounded search over four-decimal rounding intervals.
fn recover_radial(
    point: (f64, f64),
    center: (f64, f64),
    radius: f64,
    origin: (f64, f64),
    factor: u64,
) -> Option<(f64, f64)> {
    let factor = factor as f64;
    let logical_center = (
        ((center.0 - origin.0) * factor).round(),
        ((center.1 - origin.1) * factor).round(),
    );
    let logical_radius = (radius * factor).round();
    let offsets: [(f64, f64); 9] = [
        (-0.000_049, -0.000_049),
        (-0.000_049, 0.0),
        (-0.000_049, 0.000_049),
        (0.0, -0.000_049),
        (0.0, 0.0),
        (0.0, 0.000_049),
        (0.000_049, -0.000_049),
        (0.000_049, 0.0),
        (0.000_049, 0.000_049),
    ];
    for multiple in [1.0, 10.0, 100.0] {
        for (offset_x, offset_y) in offsets {
            let radial_x = ((point.0 - center.0 + offset_x) * factor * multiple).round();
            let radial_y = ((point.1 - center.1 + offset_y) * factor * multiple).round();
            if !INT32_RANGE.contains(&(radial_x as i64 + logical_center.0 as i64))
                || !INT32_RANGE.contains(&(radial_y as i64 + logical_center.1 as i64))
            {
                continue;
            }
            let length = radial_x.hypot(radial_y);
            if length == 0.0 {
                continue;
            }
            let projected_x =
                ((logical_center.0 + (logical_radius * radial_x) / length) / factor) + origin.0;
            let projected_y =
                ((logical_center.1 + (logical_radius * radial_y) / length) / factor) + origin.1;
            let matches = crate::number::fmt4(point.0) == crate::number::fmt4(projected_x)
                && crate::number::fmt4(point.1) == crate::number::fmt4(projected_y);
            if matches {
                return Some((radial_x / factor, radial_y / factor));
            }
        }
    }
    None
}

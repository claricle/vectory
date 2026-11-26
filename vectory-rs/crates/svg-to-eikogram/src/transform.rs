//! SVG transform parsing: the transform function list composed into a row
//! vector matrix, matching the reference `Svg::TransformParser` (newest
//! function pre-multiplies; rotate composes around the pivot).

use eikogram::Matrix;

/// Parses a `transform` attribute into its matrix (identity when absent).
///
/// # Errors
///
/// Fails on an unknown transform function or a wrong argument count.
pub fn parse(string: Option<&str>) -> Result<Matrix, String> {
    let Some(string) = string else {
        return Ok(Matrix::identity());
    };
    if string.trim().is_empty() {
        return Ok(Matrix::identity());
    }
    let mut composed = Matrix::identity();
    for (name, args) in functions(string) {
        let matrix = build(&name.to_ascii_lowercase(), &args)?;
        composed = multiply(&matrix, &composed);
    }
    Ok(composed)
}

fn functions(string: &str) -> Vec<(String, Vec<f64>)> {
    // `([a-zA-Z]+)\s*\(([^)]*)\)`
    let mut out = Vec::new();
    let bytes = string.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index].is_ascii_alphabetic() {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
                index += 1;
            }
            let name = string[start..index].to_string();
            let mut probe = index;
            while probe < bytes.len() && bytes[probe].is_ascii_whitespace() {
                probe += 1;
            }
            if probe < bytes.len() && bytes[probe] == b'(' {
                let Some(close) = string[probe..].find(')') else {
                    index = probe;
                    continue;
                };
                let args_text = &string[probe + 1..probe + close];
                let args: Vec<f64> = args_text
                    .split([' ', '\t', '\n', '\r', ','])
                    .filter(|part| !part.is_empty())
                    .filter_map(|part| part.parse::<f64>().ok())
                    .collect();
                out.push((name, args));
                index = probe + close + 1;
            }
        } else {
            index += 1;
        }
    }
    out
}

fn build(name: &str, args: &[f64]) -> Result<Matrix, String> {
    let arg = |index: usize, default: f64| args.get(index).copied().unwrap_or(default);
    match name {
        "matrix" => {
            if args.len() != 6 {
                return Err("matrix() requires 6 args".to_string());
            }
            Ok(Matrix {
                m11: args[0],
                m12: args[1],
                m21: args[2],
                m22: args[3],
                dx: args[4],
                dy: args[5],
            })
        }
        "translate" => Ok(Matrix {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            dx: arg(0, 0.0),
            dy: arg(1, 0.0),
        }),
        "scale" => {
            let sx = arg(0, 1.0);
            let sy = arg(1, sx);
            Ok(Matrix {
                m11: sx,
                m12: 0.0,
                m21: 0.0,
                m22: sy,
                dx: 0.0,
                dy: 0.0,
            })
        }
        "rotate" => {
            let deg = arg(0, 0.0);
            let cx = arg(1, 0.0);
            let cy = arg(2, 0.0);
            let rad = deg * std::f64::consts::PI / 180.0;
            let (cos, sin) = rad.sin_cos();
            let rot = Matrix {
                m11: cos,
                m12: sin,
                m21: -sin,
                m22: cos,
                dx: 0.0,
                dy: 0.0,
            };
            if cx == 0.0 && cy == 0.0 {
                Ok(rot)
            } else {
                Ok(multiply(
                    &multiply(&translate(-cx, -cy), &rot),
                    &translate(cx, cy),
                ))
            }
        }
        "skewx" => {
            let rad = arg(0, 0.0) * std::f64::consts::PI / 180.0;
            Ok(Matrix {
                m11: 1.0,
                m12: 0.0,
                m21: rad.tan(),
                m22: 1.0,
                dx: 0.0,
                dy: 0.0,
            })
        }
        "skewy" => {
            let rad = arg(0, 0.0) * std::f64::consts::PI / 180.0;
            Ok(Matrix {
                m11: 1.0,
                m12: rad.tan(),
                m21: 0.0,
                m22: 1.0,
                dx: 0.0,
                dy: 0.0,
            })
        }
        other => Err(format!("unknown SVG transform function: {other}")),
    }
}

fn translate(tx: f64, ty: f64) -> Matrix {
    Matrix {
        m11: 1.0,
        m12: 0.0,
        m21: 0.0,
        m22: 1.0,
        dx: tx,
        dy: ty,
    }
}

/// The row-vector multiply `v * (a * b) == (v * a) * b`.
#[must_use]
pub fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    Matrix {
        m11: a.m11 * b.m11 + a.m12 * b.m21,
        m12: a.m11 * b.m12 + a.m12 * b.m22,
        m21: a.m21 * b.m11 + a.m22 * b.m21,
        m22: a.m21 * b.m12 + a.m22 * b.m22,
        dx: a.dx * b.m11 + a.dy * b.m21 + b.dx,
        dy: a.dx * b.m12 + a.dy * b.m22 + b.dy,
    }
}

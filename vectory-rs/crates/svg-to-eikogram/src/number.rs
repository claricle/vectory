//! Number formatting: the reference `%.4f` form (C printf semantics on the
//! actual IEEE-754 bits, half-even at the fourth decimal) and the
//! shortest-round-trip float form, vendored from the VIM-to-SVG emitter's
//! module with the ingest-side string helpers this transformer adds.

/// Appends the `%.4f` text of `n` to `out` with C printf semantics.
///
/// Mirrors the reference implementation exactly: the fast path handles
/// values whose scaled fraction sits safely away from a rounding tie
/// (|frac - 0.5| > 1e-6, scaled < 1e9); everything else goes through the
/// exact decimal path — a 20-decimal fixed rendering of the double, rounded
/// half-even at the fourth decimal.
pub fn append_fmt4(out: &mut String, n: f64) {
    let f = n;
    let scaled = f.abs() * 10_000.0;
    if f != 0.0 && scaled < 1e9 {
        let floor = scaled.floor();
        let frac = scaled - floor;
        if (frac - 0.5).abs() > 1e-6 {
            let units = if frac > 0.5 { floor + 1.0 } else { floor };
            let units = units as u64;
            let whole = units / 10_000;
            let fraction = units % 10_000;
            if f.is_sign_negative() {
                out.push('-');
            }
            push_u64(out, whole);
            out.push('.');
            // %04d fraction
            if fraction < 10 {
                out.push_str("000");
            } else if fraction < 100 {
                out.push_str("00");
            } else if fraction < 1000 {
                out.push('0');
            }
            push_u64(out, fraction);
            return;
        }
    }
    append_fmt4_exact(out, f);
}

/// The exact decimal path: 20-decimal fixed rendering (what `%.20f`
/// produces), rounded half-even at the fourth decimal.
fn append_fmt4_exact(out: &mut String, value: f64) {
    if !value.is_finite() {
        // The reference renderers validate dimensions before formatting;
        // non-finite values never reach emission. Emit Ruby's form anyway.
        if value.is_nan() {
            out.push_str("NaN");
        } else if value > 0.0 {
            out.push_str("Inf");
        } else {
            out.push_str("-Inf");
        }
        return;
    }
    // 20-decimal fixed rendering of the double (Rust's fixed formatting is
    // exact, matching glibc's %.20f).
    let rendered = format!("{value:.20}");
    append_rounded_decimal(out, &rendered);
}

/// Rounds a fixed-point decimal string (optional sign, digits, '.', up to 20
/// fraction digits) half-even at 4 decimals and appends the `%.4f` text.
fn append_rounded_decimal(out: &mut String, rendered: &str) {
    let bytes = rendered.as_bytes();
    let mut idx = 0;
    let negative = bytes.first() == Some(&b'-');
    if negative {
        idx = 1;
    }
    let mut int_digits: Vec<u8> = Vec::new();
    let mut frac_digits: Vec<u8> = Vec::new();
    let mut in_frac = false;
    for &b in &bytes[idx..] {
        match b {
            b'.' => in_frac = true,
            b'0'..=b'9' => {
                if in_frac {
                    frac_digits.push(b - b'0');
                } else {
                    int_digits.push(b - b'0');
                }
            }
            _ => {}
        }
    }
    // Keep 4 fraction digits plus the round-decision remainder.
    while frac_digits.len() < 5 {
        frac_digits.push(0);
    }
    let keep: Vec<u8> = frac_digits[..4].to_vec();
    let rest: Vec<u8> = frac_digits[4..].to_vec();

    // Decide the round direction half-even: compare `rest` against 1/2 ulp.
    let mut round_up = false;
    let first = rest[0];
    match first.cmp(&5) {
        std::cmp::Ordering::Greater => round_up = true,
        std::cmp::Ordering::Equal => {
            let any_nonzero = rest[1..].iter().any(|&d| d != 0);
            if any_nonzero {
                round_up = true;
            } else {
                // Tie: half-even rounds up iff the kept part is odd.
                let kept = keep[0] as u32 * 1000
                    + keep[1] as u32 * 100
                    + keep[2] as u32 * 10
                    + keep[3] as u32;
                round_up = kept % 2 == 1;
            }
        }
        std::cmp::Ordering::Less => {}
    }

    // Apply the carry into keep, then into the integer digits.
    let mut keep = keep;
    if round_up {
        let mut pos: isize = 3;
        loop {
            if pos == -1 {
                // Carry past the decimal point: 9999 → 0000 and +1 int.
                keep = vec![0, 0, 0, 0];
                carry_into_int(&mut int_digits);
                break;
            }
            let digit = &mut keep[pos as usize];
            if *digit == 9 {
                *digit = 0;
                pos -= 1;
            } else {
                *digit += 1;
                break;
            }
        }
    }

    // The sign survives the rounding, including the negative-zero form
    // (Ruby carries BigDecimal's sign through `to_f` into `%.4f`).
    if negative {
        out.push('-');
    }
    if int_digits.is_empty() {
        out.push('0');
    } else {
        for d in &int_digits {
            out.push((b'0' + d) as char);
        }
    }
    out.push('.');
    for d in &keep {
        out.push((b'0' + d) as char);
    }
}

fn carry_into_int(digits: &mut Vec<u8>) {
    if digits.is_empty() {
        digits.push(1);
        return;
    }
    let mut pos = digits.len() - 1;
    loop {
        let digit = &mut digits[pos];
        if *digit == 9 {
            *digit = 0;
            if pos == 0 {
                digits.insert(0, 1);
                return;
            }
            pos -= 1;
        } else {
            *digit += 1;
            return;
        }
    }
}

fn push_u64(out: &mut String, mut value: u64) {
    let mut buf = [0u8; 20];
    let mut len = 0;
    loop {
        buf[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for i in (0..len).rev() {
        out.push(buf[i] as char);
    }
}

/// `%.4f` of `n`.
#[must_use]
pub fn fmt4(n: f64) -> String {
    let mut out = String::new();
    append_fmt4(&mut out, n);
    out
}

/// The Ruby `Float#to_s` form: shortest round-trip, integral floats carry
/// `.0`, and magnitudes outside [1e-4, 1e15) render in scientific notation
/// with the `d.dddde±XX` shape.
#[must_use]
pub fn ruby_float_to_s(value: f64) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0".to_string()
        } else {
            "0.0".to_string()
        };
    }
    let abs = value.abs();
    // Ruby keeps the fixed form only in [1e-4, 1e15); outside it the
    // scientific form applies (probe-verified boundaries).
    if !(1e-4..1e15).contains(&abs) {
        return ruby_scientific(value);
    }
    // Shortest round-trip decimal, then force a `.0` when integral.
    let shortest = format!("{value}");
    if shortest.contains('.') || shortest.contains('e') || shortest.contains('E') {
        shortest
    } else {
        format!("{shortest}.0")
    }
}

/// Ruby's scientific form: one mantissa digit, `e` sign, two exponent
/// digits; integral mantissas carry `.0`.
fn ruby_scientific(value: f64) -> String {
    let shortest = format!("{value:e}");
    // Rust: "-1.2345e-5" / "1e16"; Ruby: "-1.2345e-05" / "1.0e+16".
    let (mantissa, exponent) = shortest.split_once('e').expect("scientific form");
    let (sign, digits) = match exponent.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("+", exponent.strip_prefix('+').unwrap_or(exponent)),
    };
    let mantissa = if mantissa.contains('.') {
        mantissa.to_string()
    } else {
        format!("{mantissa}.0")
    };
    let padded_exponent = if digits.len() < 2 {
        format!("0{digits}")
    } else {
        digits.to_string()
    };
    format!("{mantissa}e{sign}{padded_exponent}")
}

/// The Ruby integer form of an f64 (integral values only).

/// Standard base64 (no line breaks), the reference `pack("m0")` form.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt4_matches_reference_values() {
        // The documented tie case: 459.04375 stores as 459.04374999… and
        // rounds DOWN at the true bits.
        assert_eq!(fmt4(459.04375), "459.0437");
        assert_eq!(fmt4(0.0), "0.0000");
        assert_eq!(fmt4(1.0), "1.0000");
        assert_eq!(fmt4(-0.5), "-0.5000");
        assert_eq!(fmt4(0.5), "0.5000");
        // Half-even at the tie.
        assert_eq!(fmt4(0.06255f64), "0.0625"); // stored below tie
        assert_eq!(fmt4(-1.0), "-1.0000");
        // 123456.78915 stores slightly below the tie; Ruby rounds down.
        assert_eq!(fmt4(123456.789_15), "123456.7891");
        assert_eq!(fmt4(-0.00001), "-0.0000");
    }

    #[test]
    fn ruby_float_forms() {
        assert_eq!(ruby_float_to_s(1.0), "1.0");
        assert_eq!(ruby_float_to_s(0.1), "0.1");
        assert_eq!(ruby_float_to_s(123.0), "123.0");
        assert_eq!(ruby_float_to_s(0.001), "0.001");
        assert_eq!(ruby_float_to_s(0.0001), "0.0001");
        assert_eq!(ruby_float_to_s(0.00009), "9.0e-05");
        assert_eq!(ruby_float_to_s(0.00001), "1.0e-05");
        assert_eq!(ruby_float_to_s(123456789012345.0), "123456789012345.0");
        assert_eq!(ruby_float_to_s(1234567890123456.0), "1.234567890123456e+15");
        assert_eq!(ruby_float_to_s(-2.5), "-2.5");
    }
}

//! Canonical text rotation recovery: the renderer's exact rotate+translate
//! spelling carries the LOGFONT escapement and vertical alignment; the
//! recovered native grid values use exact rational interval intersection,
//! mirroring the reference `Svg::Elements::Text.canonical_rotation`.

/// An exact rational (sign in the numerator, denominator positive,
/// unreduced but exact in i128).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rational {
    num: i128,
    den: i128,
}

impl Rational {
    #[must_use]
    pub fn new(num: i128, den: i128) -> Self {
        let den = if den < 0 { -den } else { den };
        let num = if den < 0 { -num } else { num };
        Rational { num, den }
    }

    #[must_use]
    pub fn integer(value: i128) -> Self {
        Rational::new(value, 1)
    }

    /// Parses a decimal text form (`-12.34`) exactly.
    #[must_use]
    pub fn parse_decimal(text: &str) -> Option<Self> {
        let (integers, decimals) = text.split_once('.')?;
        let negative = integers.starts_with('-');
        let integers = integers.trim_start_matches('-');
        if integers.is_empty()
            || decimals.is_empty()
            || !integers.chars().all(|c| c.is_ascii_digit())
            || !decimals.chars().all(|c| c.is_ascii_digit())
        {
            return None;
        }
        let whole: i128 = integers.parse().ok()?;
        let fraction: i128 = decimals.parse().ok()?;
        let den = 10i128.pow(decimals.len() as u32);
        let num = whole * den + fraction;
        Some(Rational::new(if negative { -num } else { num }, den))
    }

    #[must_use]
    pub fn add(self, other: Self) -> Self {
        Rational::new(
            self.num * other.den + other.num * self.den,
            self.den * other.den,
        )
    }

    #[must_use]
    pub fn sub(self, other: Self) -> Self {
        Rational::new(
            self.num * other.den - other.num * self.den,
            self.den * other.den,
        )
    }

    #[must_use]
    pub fn mul(self, other: Self) -> Self {
        Rational::new(self.num * other.num, self.den * other.den)
    }

    #[must_use]
    pub fn div(self, other: Self) -> Self {
        Rational::new(self.num * other.den, self.den * other.num)
    }

    #[must_use]
    pub fn scale(self, factor: i128) -> Self {
        Rational::new(self.num * factor, self.den)
    }

    #[must_use]
    pub fn cmp_to(self, other: Self) -> std::cmp::Ordering {
        (self.num * other.den).cmp(&(other.num * self.den))
    }

    #[must_use]
    pub fn less(self, other: Self) -> bool {
        self.cmp_to(other) == std::cmp::Ordering::Less
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.num == 0
    }

    /// Ruby's `Rational#round` (half away from zero).
    #[must_use]
    pub fn round(self) -> i128 {
        let (num, den) = (self.num, self.den);
        if num >= 0 {
            (2 * num + den) / (2 * den)
        } else {
            -((2 * (-num) + den) / (2 * den))
        }
    }

    /// The value as f64 (numerator and denominator fit i64 here).
    #[must_use]
    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }
}

/// The recovered rotation facts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotation {
    /// The whole-degree angle.
    pub angle: i32,
    /// The vertical alignment the recovery chose.
    pub vertical_alignment: Alignment,
    /// The LOGFONT escapement (tenths of a degree; the zero-angle quirk
    /// maps angle 0 to escapement 1).
    pub escapement: i32,
    /// The recovered native height (the finer mapping's source value).
    pub native_height: Option<f64>,
    /// The recovered native baseline.
    pub native_y: Option<f64>,
}

/// The vertical alignment of a recovered rotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Top,
    Baseline,
}

/// The reference `AttributeParser.float` prefix parse.
fn attr_float(value: Option<&str>, default: f64) -> f64 {
    crate::attributes::parse_float(value, default)
}

/// The attr float's exact rational: through its shortest decimal text, as
/// the reference compares (`float.to_s.to_r`).
fn attr_rational(value: Option<&str>, default: f64) -> Option<Rational> {
    Rational::parse_decimal(&crate::number::ruby_float_to_s(attr_float(value, default)))
}

/// Recovers the canonical rotation facts of a `<text>` element.
#[must_use]
pub fn canonical_rotation(node: &vectory::Element) -> Option<Rotation> {
    if node.attribute("dx").is_some() || node.attribute("dy").is_some() {
        return None;
    }
    // `rotate\((0|-?[1-9]\d*), DEC, DEC\) translate\(0, DEC\)`
    let transform = node.attribute("transform")?;
    let (angle, x, pivot, offset) = parse_rotation_spelling(transform)?;
    if !(-359..=359).contains(&angle) {
        return None;
    }
    if x != attr_rational(node.attribute("x"), 0.0)? {
        return None;
    }
    let y = attr_rational(node.attribute("y"), 0.0)?;
    let height = attr_rational(node.attribute("font-size"), 12.0)?;

    let half = Rational::new(1, 20_000);
    let ratio = Rational::new(9, 10);
    let low_a = height.sub(half).mul(ratio);
    let low_b = offset.sub(half);
    let low = max_of(low_a, low_b);
    let high_a = height.add(half).mul(ratio);
    let high_b = offset.add(half);
    let high = min_of(high_a, high_b);
    if !low.less(high) {
        return None;
    }

    let alignment = if pivot == y {
        if height.is_zero() {
            Alignment::Baseline
        } else {
            Alignment::Top
        }
    } else {
        let inner_low = max_of(low, pivot.sub(y).sub(half.scale(2)));
        let inner_high = min_of(high, pivot.sub(y).add(half.scale(2)));
        if inner_low.less(inner_high) {
            Alignment::Baseline
        } else {
            return None;
        }
    };

    let (native_height, native_y) = if alignment == Alignment::Baseline {
        recover_baseline(height, y, pivot, offset)?
    } else {
        (None, None)
    };

    Some(Rotation {
        angle,
        vertical_alignment: alignment,
        escapement: if angle == 0 { 1 } else { -angle * 10 },
        native_height,
        native_y,
    })
}

fn max_of(a: Rational, b: Rational) -> Rational {
    if b.cmp_to(a) == std::cmp::Ordering::Greater {
        b
    } else {
        a
    }
}

fn min_of(a: Rational, b: Rational) -> Rational {
    if b.cmp_to(a) == std::cmp::Ordering::Less {
        b
    } else {
        a
    }
}

/// Parses `rotate(A, X, P) translate(0, O)`; the decimals are the exact
/// four-decimal rationals.
fn parse_rotation_spelling(transform: &str) -> Option<(i32, Rational, Rational, Rational)> {
    let inner = transform.strip_prefix("rotate(")?;
    let close = inner.find(')')?;
    let rotate_args = &inner[..close];
    let rest = inner[close + 1..].trim_start();
    let rest = rest.strip_prefix("translate(0, ")?;
    let rest = rest.strip_suffix(')')?;
    if rest.is_empty() || rest.contains(' ') {
        return None;
    }
    let parts: Vec<&str> = rotate_args.split(", ").collect();
    if parts.len() != 3 {
        return None;
    }
    let angle = parse_angle(parts[0])?;
    let x = parse_four_decimal(parts[1])?;
    let pivot = parse_four_decimal(parts[2])?;
    let offset = parse_four_decimal(rest)?;
    Some((angle, x, pivot, offset))
}

fn parse_angle(value: &str) -> Option<i32> {
    if value == "0" {
        return Some(0);
    }
    let body = value.strip_prefix('-').unwrap_or(value);
    if body.is_empty() || body.starts_with('0') || !body.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    value.parse::<i32>().ok()
}

fn parse_four_decimal(value: &str) -> Option<Rational> {
    let (integers, decimals) = value.split_once('.')?;
    if decimals.len() != 4 {
        return None;
    }
    Rational::parse_decimal(value).filter(|_| {
        integers
            .trim_start_matches('-')
            .chars()
            .all(|c| c.is_ascii_digit())
    })
}

/// The baseline recovery: chooses an interior point of the four source
/// intervals on the finer native grid.
fn recover_baseline(
    height: Rational,
    y: Rational,
    pivot: Rational,
    offset: Rational,
) -> Option<(Option<f64>, Option<f64>)> {
    // mapped_height = (height * 10_000).round * 0.0001, in floats.
    let mapped_height = (height.to_f64() * 10_000.0).round() * 0.0001;
    let mapped_y = (y.to_f64() * 10_000.0).round() * 0.0001;
    // Proceed only when the direct mapping does NOT reproduce the pivot.
    let direct = crate::number::fmt4(mapped_y + (mapped_height * 0.9));
    let pivot_text = crate::number::fmt4(pivot.to_f64());
    if direct == pivot_text {
        return None;
    }
    let half = Rational::new(1, 20_000);
    let ratio = Rational::new(9, 10);
    let two_half = half.scale(2);
    let low1 = height.sub(half);
    let low2 = offset.sub(half).div(ratio);
    let low3 = pivot.sub(y).sub(two_half).div(ratio);
    let high1 = height.add(half);
    let high2 = offset.add(half).div(ratio);
    let high3 = pivot.sub(y).add(two_half).div(ratio);
    let low = max_of(max_of(low1, low2), low3);
    let high = min_of(min_of(high1, high2), high3);
    if !low.less(high) {
        return None;
    }
    let native_height_int = low
        .add(high)
        .scale(1_000_000)
        .div(Rational::integer(2))
        .round();
    let native_height = Rational::new(native_height_int, 1_000_000);
    let low_y = max_of(y.sub(half), pivot.sub(half).sub(native_height.mul(ratio)));
    let high_y = min_of(y.add(half), pivot.add(half).sub(native_height.mul(ratio)));
    if !low_y.less(high_y) {
        return None;
    }
    let native_y_int = low_y
        .add(high_y)
        .scale(1_000_000)
        .div(Rational::integer(2))
        .round();
    let native_y = Rational::new(native_y_int, 1_000_000);

    let native_height_f = native_height.to_f64();
    let native_y_f = native_y.to_f64();
    let actual = [
        crate::number::fmt4(native_height_f),
        crate::number::fmt4(native_y_f),
        crate::number::fmt4(native_height_f * 0.9),
        crate::number::fmt4(native_y_f + (native_height_f * 0.9)),
    ];
    let expected = [
        crate::number::fmt4(height.to_f64()),
        crate::number::fmt4(y.to_f64()),
        crate::number::fmt4(offset.to_f64()),
        crate::number::fmt4(pivot.to_f64()),
    ];
    if actual != expected {
        return None;
    }
    Some((Some(native_height_f), Some(native_y_f)))
}

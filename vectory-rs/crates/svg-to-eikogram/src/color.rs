//! SVG colour parsing: hex, functional, and the CSS named colours, with
//! the null sentinel, matching the reference `Svg::Color`.

/// An sRGB colour, or the null colour (`none` / empty).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvgColor {
    Null,
    Rgb { red: u8, green: u8, blue: u8 },
}

impl SvgColor {
    /// Parses a colour value; `none`, empty, and absent parse null.
    ///
    /// # Errors
    ///
    /// Fails on an unsupported colour form, as the reference raises.
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(SvgColor::Null);
        };
        let value = value.trim();
        if value.is_empty() || value == "none" {
            return Ok(SvgColor::Null);
        }
        if is_hex3(value) {
            let digits: Vec<char> = value[1..].chars().collect();
            return Ok(SvgColor::Rgb {
                red: (digits[0].to_digit(16).expect("hex digit") * 2) as u8,
                green: (digits[1].to_digit(16).expect("hex digit") * 2) as u8,
                blue: (digits[2].to_digit(16).expect("hex digit") * 2) as u8,
            });
        }
        if is_hex6(value) {
            let value = u32::from_str_radix(&value[1..], 16).unwrap_or(0);
            return Ok(SvgColor::Rgb {
                red: (value >> 16) as u8,
                green: ((value >> 8) & 0xFF) as u8,
                blue: (value & 0xFF) as u8,
            });
        }
        if let Some(rgb) = parse_rgb_function(value) {
            return Ok(rgb);
        }
        let lowered = value.to_ascii_lowercase();
        if let Some(rgb) = named(&lowered) {
            return Ok(SvgColor::Rgb {
                red: rgb.0,
                green: rgb.1,
                blue: rgb.2,
            });
        }
        Err(format!("unsupported SVG color: {value:?}"))
    }

    /// The channels, with the null colour reading black.
    #[must_use]
    pub fn channels(self) -> (u8, u8, u8) {
        match self {
            SvgColor::Null => (0, 0, 0),
            SvgColor::Rgb { red, green, blue } => (red, green, blue),
        }
    }
}

fn is_hex3(value: &str) -> bool {
    value.len() == 4 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn is_hex6(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn parse_rgb_function(value: &str) -> Option<SvgColor> {
    let rest = value.strip_prefix("rgb(")?.strip_suffix(')')?;
    let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
    if parts.len() != 3 {
        return None;
    }
    let channels: Vec<i32> = parts
        .iter()
        .map(|part| part.parse::<i32>().unwrap_or(0).clamp(0, 255))
        .collect();
    Some(SvgColor::Rgb {
        red: channels[0] as u8,
        green: channels[1] as u8,
        blue: channels[2] as u8,
    })
}

#[must_use]
pub fn named(lowered: &str) -> Option<(u8, u8, u8)> {
    Some(match lowered {
        "black" => (0x00, 0x00, 0x00),
        "silver" => (0xC0, 0xC0, 0xC0),
        "gray" | "grey" => (0x80, 0x80, 0x80),
        "white" => (0xFF, 0xFF, 0xFF),
        "maroon" => (0x80, 0x00, 0x00),
        "red" => (0xFF, 0x00, 0x00),
        "purple" => (0x80, 0x00, 0x80),
        "fuchsia" | "magenta" => (0xFF, 0x00, 0xFF),
        "green" => (0x00, 0x80, 0x00),
        "lime" => (0x00, 0xFF, 0x00),
        "olive" => (0x80, 0x80, 0x00),
        "yellow" => (0xFF, 0xFF, 0x00),
        "navy" => (0x00, 0x00, 0x80),
        "blue" => (0x00, 0x00, 0xFF),
        "teal" => (0x00, 0x80, 0x80),
        "aqua" | "cyan" => (0x00, 0xFF, 0xFF),
        _ => return None,
    })
}

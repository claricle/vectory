//! SVG path-data parsing: command letters and absolute/relative arguments,
//! matching the reference `Svg::PathData` parser (implicit `M`→`L`
//! repeats, implicit lineto continuation).

/// One parsed path command: its letter (as written) and its arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    pub letter: String,
    pub args: Vec<f64>,
}

impl Command {
    #[must_use]
    pub fn absolute(&self) -> bool {
        self.letter.chars().next().is_some_and(char::is_uppercase)
    }

    #[must_use]
    pub fn upcased(&self) -> String {
        self.letter.to_uppercase()
    }
}

/// Argument counts per command letter.
fn arg_count(letter: &str) -> Option<usize> {
    Some(match letter {
        "M" | "m" | "L" | "l" | "T" | "t" => 2,
        "H" | "h" | "V" | "v" => 1,
        "Z" | "z" => 0,
        "S" | "s" | "Q" | "q" => 4,
        "C" | "c" => 6,
        "A" | "a" => 7,
        _ => return None,
    })
}

/// Parses a `d` attribute into commands. Unknown letters and malformed
/// groups are errors, as the reference raises.
///
/// # Errors
///
/// Fails on a path beginning with a number, an unknown command, or a
/// truncated argument group.
pub fn parse(string: Option<&str>) -> Result<Vec<Command>, String> {
    let Some(string) = string else {
        return Ok(Vec::new());
    };
    if string.is_empty() {
        return Ok(Vec::new());
    }
    let mut commands: Vec<Command> = Vec::new();
    let tokens = tokenize(string);
    let mut current: Option<String> = None;
    let mut args: Vec<f64> = Vec::new();
    for token in tokens {
        match token {
            Token::Letter(letter) => {
                flush(&mut commands, current.take(), &args)?;
                args.clear();
                let count =
                    arg_count(&letter).ok_or_else(|| format!("unknown path command: {letter}"))?;
                if count == 0 {
                    commands.push(Command {
                        letter: letter.clone(),
                        args: Vec::new(),
                    });
                } else {
                    current = Some(letter);
                }
            }
            Token::Number(value) => {
                let letter = current
                    .clone()
                    .ok_or_else(|| format!("path data begins with a number: {string:?}"))?;
                args.push(value);
                if args.len() == arg_count(&letter).unwrap_or(0) {
                    commands.push(Command {
                        letter: letter.clone(),
                        args: std::mem::take(&mut args),
                    });
                    // M repeats as L (and m as l); all else repeats itself.
                    let repeat = match letter.as_str() {
                        "M" => "L".to_string(),
                        "m" => "l".to_string(),
                        other => other.to_string(),
                    };
                    current = Some(repeat);
                }
            }
        }
    }
    flush(&mut commands, current.take(), &args)?;
    Ok(commands)
}

fn flush(commands: &mut Vec<Command>, letter: Option<String>, args: &[f64]) -> Result<(), String> {
    let Some(letter) = letter else {
        return Ok(());
    };
    if args.is_empty() {
        return Ok(());
    }
    let expected = arg_count(&letter).unwrap_or(0);
    if args.len() != expected {
        return Err(format!(
            "path command {letter} expected {expected} args, got {}",
            args.len()
        ));
    }
    commands.push(Command {
        letter,
        args: args.to_vec(),
    });
    Ok(())
}

enum Token {
    Letter(String),
    Number(f64),
}

fn tokenize(string: &str) -> Tokenizer<'_> {
    Tokenizer {
        bytes: string.as_bytes(),
        position: 0,
    }
}

struct Tokenizer<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Iterator for Tokenizer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        self.skip_separators();
        if self.position >= self.bytes.len() {
            return None;
        }
        let byte = self.bytes[self.position];
        if byte.is_ascii_alphabetic() {
            self.position += 1;
            return Some(Token::Letter((byte as char).to_string()));
        }
        self.read_number()
    }
}

impl Tokenizer<'_> {
    fn skip_separators(&mut self) {
        while self.position < self.bytes.len()
            && matches!(
                self.bytes[self.position],
                b' ' | b'\t' | b'\n' | b'\r' | b','
            )
        {
            self.position += 1;
        }
    }

    fn read_number(&mut self) -> Option<Token> {
        let start = self.position;
        let mut end = start;
        if end < self.bytes.len() && (self.bytes[end] == b'-' || self.bytes[end] == b'+') {
            end += 1;
        }
        let digits_start = end;
        while end < self.bytes.len() && self.bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end < self.bytes.len() && self.bytes[end] == b'.' {
            end += 1;
            while end < self.bytes.len() && self.bytes[end].is_ascii_digit() {
                end += 1;
            }
        }
        if end == digits_start {
            // Skip up to the next number, as the scan has always done.
            while end < self.bytes.len()
                && !self.bytes[end].is_ascii_digit()
                && self.bytes[end] != b'-'
                && self.bytes[end] != b'+'
                && self.bytes[end] != b'.'
            {
                end += 1;
            }
            self.position = end;
            if end >= self.bytes.len() {
                return None;
            }
            return self.read_number();
        }
        if end < self.bytes.len() && (self.bytes[end] == b'e' || self.bytes[end] == b'E') {
            let mut exp_end = end + 1;
            if exp_end < self.bytes.len()
                && (self.bytes[exp_end] == b'-' || self.bytes[exp_end] == b'+')
            {
                exp_end += 1;
            }
            let exp_digits = exp_end;
            while exp_end < self.bytes.len() && self.bytes[exp_end].is_ascii_digit() {
                exp_end += 1;
            }
            if exp_end > exp_digits {
                end = exp_end;
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..end]).ok()?;
        self.position = end;
        text.parse::<f64>().ok().map(Token::Number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commands_with_implicit_repeats() {
        let commands = parse(Some("M 1,2 L 3,4 5,6 Z")).expect("parse");
        let letters: Vec<&str> = commands.iter().map(|c| c.letter.as_str()).collect();
        assert_eq!(letters, vec!["M", "L", "L", "Z"]);
    }

    #[test]
    fn parses_trailing_space_after_z() {
        let d = "M 251.0000,-670.3333 L 251.0000,-553.0000 L 251.0000,-670.3333 Z ";
        let commands = parse(Some(d)).expect("parse");
        let letters: Vec<&str> = commands.iter().map(|c| c.letter.as_str()).collect();
        assert_eq!(letters, vec!["M", "L", "L", "Z"], "letters for {d:?}");
    }
}

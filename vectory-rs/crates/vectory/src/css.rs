//! A small CSS tokenizer/parser mirroring the crass subset vectory's
//! `SvgClassSuffixer` consumes (the recorded CSS decision for vectory-rs:
//! port the used subset in Rust rather than adopt an external CSS crate —
//! the suffixer depends on crass-exact token positions and raw slices for
//! byte-exact insertions, and no Rust crate reproduces that contract).
//!
//! Token model parity with crass:
//! * `pos` — position in the CSS-normalized stream (per CSS syntax level 3,
//!   CRLF/CR/FF normalize to LF before tokenizing, so a former CRLF counts
//!   as ONE position unit; positions are in that normalized text);
//! * `raw` — the exact normalized source slice;
//! * `value` — the decoded value (CSS escapes resolved);
//! * functions and simple blocks carry their inner tokens.
//!
//! The parser side reproduces only what `SvgClassSuffixer#css_insertions`
//! walks: top-level qualified rules and at-rules, with preludes and blocks
//! kept as raw token streams (declaration values stay opaque, so CSS nesting
//! survives).

/// A crass-shaped token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub pos: usize,
    pub raw: String,
    pub value: String,
    /// Function name (without the `(`), at-keyword name (without the `@`).
    pub name: Option<String>,
    /// Inner tokens of a function or simple block.
    pub tokens: Vec<Token>,
    /// For simple blocks: the opening character.
    pub start: Option<char>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Ident,
    Function,
    AtKeyword,
    Hash,
    Delim,
    Whitespace,
    Comment,
    Colon,
    Semicolon,
    Comma,
    String,
    Number,
    Percentage,
    Dimension,
    SimpleBlock,
    Other,
}

impl Token {
    fn scalar(kind: TokenKind, pos: usize, raw: String, value: String) -> Token {
        Token {
            kind,
            pos,
            raw,
            value,
            name: None,
            tokens: Vec::new(),
            start: None,
        }
    }

    pub fn is_trivia(&self) -> bool {
        matches!(self.kind, TokenKind::Whitespace | TokenKind::Comment)
    }
}

/// Applies CSS syntax-level preprocessing: CRLF/CR/FF become LF.
#[must_use]
pub fn preprocess(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut chars = css.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push('\n');
            }
            '\u{c}' => out.push('\n'),
            _ => out.push(c),
        }
    }
    out
}

struct Scanner {
    chars: Vec<char>,
    pos: usize,
}

impl Scanner {
    fn new(css: &str) -> Self {
        Scanner {
            chars: preprocess(css).chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn slice(&self, start: usize, end: usize) -> String {
        self.chars[start..end].iter().collect()
    }

    fn eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    /// Consumes an escape: the `\` must already be consumed (pos points at
    /// the character after it). Returns the decoded value.
    fn consume_escape(&mut self) -> String {
        let mut hex = String::new();
        while hex.len() < 6 {
            match self.peek() {
                Some(c) if c.is_ascii_hexdigit() => {
                    hex.push(c);
                    self.pos += 1;
                }
                _ => break,
            }
        }
        if !hex.is_empty() {
            if matches!(self.peek(), Some(c) if c.is_whitespace()) {
                self.pos += 1;
            }
            let code = u32::from_str_radix(&hex, 16).unwrap_or(0xFFFD);
            return char::from_u32(code)
                .filter(|c| *c != '\0')
                .map(String::from)
                .unwrap_or_else(|| "\u{FFFD}".to_string());
        }
        match self.bump() {
            Some('\n') | None => String::new(),
            Some(c) => c.to_string(),
        }
    }

    fn consume_ident_sequence(&mut self) -> (String, usize) {
        let start = self.pos;
        let mut value = String::new();
        loop {
            match self.peek() {
                Some('\\') => {
                    self.pos += 1;
                    value.push_str(&self.consume_escape());
                }
                Some(c) if is_ident_char(c) => {
                    value.push(c);
                    self.pos += 1;
                }
                _ => break,
            }
        }
        (value, start)
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '-' || !c.is_ascii()
}

fn is_ident_char(c: char) -> bool {
    c.is_alphabetic() || c.is_ascii_digit() || c == '-' || c == '_' || !c.is_ascii()
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// Tokenizes CSS into crass-shaped tokens (preprocessing applied inside).
/// `pos_offset` shifts every token position — nested tokenization keeps
/// positions absolute in the whole normalized stylesheet, as crass does.
#[must_use]
pub fn tokenize(css: &str) -> Vec<Token> {
    tokenize_at(css, 0)
}

fn tokenize_at(css: &str, pos_offset: usize) -> Vec<Token> {
    let mut scanner = Scanner::new(css);
    let mut tokens = Vec::new();

    macro_rules! push {
        ($token:expr) => {{
            let mut token = $token;
            token.pos += pos_offset;
            tokens.push(token);
        }};
    }

    while !scanner.eof() {
        let start = scanner.pos;
        let c = scanner.peek().unwrap();

        // Comment.
        if c == '/' && scanner.peek_at(1) == Some('*') {
            scanner.pos += 2;
            while !scanner.eof() {
                if scanner.peek() == Some('*') && scanner.peek_at(1) == Some('/') {
                    scanner.pos += 2;
                    break;
                }
                scanner.pos += 1;
            }
            let raw = scanner.slice(start, scanner.pos);
            push!(Token::scalar(TokenKind::Comment, start, raw, String::new()));
            continue;
        }

        // Whitespace run.
        if c.is_whitespace() {
            while matches!(scanner.peek(), Some(w) if w.is_whitespace()) {
                scanner.pos += 1;
            }
            let raw = scanner.slice(start, scanner.pos);
            push!(Token::scalar(
                TokenKind::Whitespace,
                start,
                raw.clone(),
                raw
            ));
            continue;
        }

        // String.
        if c == '"' || c == '\'' {
            let quote = c;
            scanner.pos += 1;
            let mut value = String::new();
            loop {
                match scanner.bump() {
                    Some('\\') => value.push_str(&scanner.consume_escape()),
                    Some(w) if w == quote => break,
                    Some('\n') | None => break,
                    Some(w) => value.push(w),
                }
            }
            let raw = scanner.slice(start, scanner.pos.min(scanner.chars.len()));
            push!(Token::scalar(TokenKind::String, start, raw, value));
            continue;
        }

        // Hash.
        if c == '#' {
            scanner.pos += 1;
            let (value, _) = scanner.consume_ident_sequence();
            let raw = scanner.slice(start, scanner.pos);
            push!(Token::scalar(TokenKind::Hash, start, raw.clone(), value));
            continue;
        }

        // At-keyword.
        if c == '@' && scanner.peek_at(1).is_some_and(is_ident_start) {
            scanner.pos += 1;
            let (value, _) = scanner.consume_ident_sequence();
            let raw = scanner.slice(start, scanner.pos);
            push!(Token {
                kind: TokenKind::AtKeyword,
                pos: start,
                raw: raw.clone(),
                value: value.clone(),
                name: Some(value),
                tokens: Vec::new(),
                start: None,
            });
            continue;
        }

        // Number / dimension / percentage.
        if is_digit(c)
            || (c == '+' || c == '-' || c == '.')
                && scanner
                    .peek_at(1)
                    .is_some_and(|next| is_digit(next) || next == '.')
                && !(c == '.' && scanner.peek_at(1) == Some('.'))
        {
            scanner.pos += 1;
            while matches!(scanner.peek(), Some(w) if is_digit(w) || w == '.') {
                scanner.pos += 1;
            }
            if matches!(scanner.peek(), Some('e') | Some('E')) {
                let mut lookahead = 1;
                if matches!(scanner.peek_at(1), Some('+') | Some('-')) {
                    lookahead = 2;
                }
                if scanner.peek_at(lookahead).is_some_and(is_digit) {
                    scanner.pos += lookahead + 1;
                }
            }
            if matches!(scanner.peek(), Some('%')) {
                scanner.pos += 1;
                let raw = scanner.slice(start, scanner.pos);
                push!(Token::scalar(
                    TokenKind::Percentage,
                    start,
                    raw.clone(),
                    raw
                ));
                continue;
            }
            let (ident, _) = if matches!(scanner.peek(), Some(c) if is_ident_start(c)) {
                scanner.consume_ident_sequence()
            } else {
                (String::new(), scanner.pos)
            };
            let raw = scanner.slice(start, scanner.pos);
            let kind = if ident.is_empty() {
                TokenKind::Number
            } else {
                TokenKind::Dimension
            };
            push!(Token::scalar(kind, start, raw.clone(), raw));
            continue;
        }

        // Ident / function.
        if c == '\\' || is_ident_start(c) {
            let (value, _) = scanner.consume_ident_sequence();
            let raw = scanner.slice(start, scanner.pos);
            if scanner.peek() == Some('(') {
                let name = raw.clone();
                scanner.pos += 1;
                let inner_start = scanner.pos;
                let inner = consume_block(&mut scanner, '(', ')');
                let inner_text = scanner.slice(inner_start, inner_start + inner);
                push!(Token {
                    kind: TokenKind::Function,
                    pos: start,
                    raw: scanner.slice(start, scanner.pos),
                    value: raw,
                    name: Some(name),
                    tokens: tokenize_at(&inner_text, pos_offset + inner_start),
                    start: None,
                });
            } else {
                push!(Token::scalar(TokenKind::Ident, start, raw, value));
            }
            continue;
        }

        // url(...) — consumed as a function-shaped token without inner
        // tokenization (the suffixer never descends into it).
        if (c == 'u' || c == 'U') && normalized_starts_with_ignore_case(&scanner, "url(") {
            scanner.pos += 4;
            let _ = consume_block(&mut scanner, '(', ')');
            let raw = scanner.slice(start, scanner.pos);
            push!(Token::scalar(TokenKind::Other, start, raw.clone(), raw));
            continue;
        }

        // Simple blocks.
        if c == '{' || c == '(' || c == '[' {
            let close = match c {
                '{' => '}',
                '(' => ')',
                _ => ']',
            };
            scanner.pos += 1;
            let inner_start = scanner.pos;
            let inner = consume_block(&mut scanner, c, close);
            let inner_text = scanner.slice(inner_start, inner_start + inner);
            push!(Token {
                kind: TokenKind::SimpleBlock,
                pos: start,
                raw: scanner.slice(start, scanner.pos),
                value: String::new(),
                name: None,
                tokens: tokenize_at(&inner_text, pos_offset + inner_start),
                start: Some(c),
            });
            continue;
        }

        // Single-character tokens.
        scanner.pos += 1;
        let raw = c.to_string();
        let kind = match c {
            ':' => TokenKind::Colon,
            ';' => TokenKind::Semicolon,
            ',' => TokenKind::Comma,
            _ => TokenKind::Delim,
        };
        push!(Token::scalar(kind, start, raw.clone(), raw));
    }
    tokens
}

fn normalized_starts_with_ignore_case(scanner: &Scanner, pattern: &str) -> bool {
    let lower: Vec<char> = pattern.to_lowercase().chars().collect();
    for (offset, expected) in lower.iter().enumerate() {
        match scanner.peek_at(offset) {
            Some(c) if c.to_lowercase().next() == Some(*expected) => {}
            _ => return false,
        }
    }
    true
}

/// Consumes until the matching `close` for `open`, tracking nesting.
/// Returns the number of chars consumed BEFORE the final close (the inner
/// span); leaves pos after the close.
fn consume_block(scanner: &mut Scanner, open: char, close: char) -> usize {
    let inner_start = scanner.pos;
    let mut depth = 1usize;
    while !scanner.eof() {
        let c = scanner.peek().unwrap();
        match c {
            '\\' => {
                scanner.pos += 1;
                scanner.consume_escape();
            }
            '\'' | '"' => {
                scanner.pos += 1;
                loop {
                    match scanner.bump() {
                        Some('\\') => {
                            scanner.consume_escape();
                        }
                        Some(w) if w == c => break,
                        Some('\n') | None => break,
                        _ => {}
                    }
                }
            }
            _ => {
                if c == open {
                    depth += 1;
                } else if c == close {
                    depth -= 1;
                    if depth == 0 {
                        scanner.pos += 1;
                        return scanner.pos - 1 - inner_start;
                    }
                }
                scanner.pos += 1;
            }
        }
    }
    let _ = depth;
    scanner.pos - inner_start
}

// ---------------------------------------------------------------------------
// Parser: consume_rules with preludes/blocks, crass-shaped.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    Qualified {
        prelude: Vec<Token>,
        block: Vec<Token>,
    },
    AtRule {
        name: String,
        prelude: Vec<Token>,
        block: Option<Vec<Token>>,
    },
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    reconsume: bool,
}

impl Parser {
    fn consume(&mut self) -> Option<Token> {
        if self.reconsume {
            self.reconsume = false;
            return self.tokens.get(self.index).cloned();
        }
        let token = self.tokens.get(self.index).cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }
}

/// Parses top-level rules (crass `consume_rules(top_level: true)`).
#[must_use]
pub fn parse_rules(css: &str) -> Vec<Rule> {
    let tokens = tokenize(css);
    let mut parser = Parser {
        tokens,
        index: 0,
        reconsume: false,
    };
    consume_rules(&mut parser)
}

fn consume_rules(parser: &mut Parser) -> Vec<Rule> {
    let mut rules = Vec::new();
    let mut prelude: Vec<Token> = Vec::new();
    while let Some(token) = parser.consume() {
        match token.kind {
            TokenKind::AtKeyword => {
                let (name, at_prelude, block) = consume_at_rule(parser, &token);
                rules.push(Rule::AtRule {
                    name,
                    prelude: at_prelude,
                    block,
                });
                prelude.clear();
            }
            TokenKind::SimpleBlock if token.start == Some('{') => {
                rules.push(Rule::Qualified {
                    prelude: std::mem::take(&mut prelude),
                    block: token.tokens,
                });
            }
            TokenKind::Semicolon => {
                // Stray semicolon: crass emits no rule for the bare prelude.
                prelude.clear();
            }
            _ => prelude.push(token),
        }
    }
    rules
}

fn consume_at_rule(
    parser: &mut Parser,
    at_keyword: &Token,
) -> (String, Vec<Token>, Option<Vec<Token>>) {
    let mut prelude = Vec::new();
    let mut block = None;
    while let Some(token) = parser.consume() {
        match token.kind {
            TokenKind::Semicolon => break,
            TokenKind::SimpleBlock if token.start == Some('{') => {
                block = Some(token.tokens);
                break;
            }
            _ => prelude.push(token),
        }
    }
    (at_keyword.name.clone().unwrap_or_default(), prelude, block)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_idents_delims_and_functions() {
        let tokens = tokenize(".a:is(.b, .c)");
        assert_eq!(tokens[0].kind, TokenKind::Delim);
        assert_eq!(tokens[1].kind, TokenKind::Ident);
        assert_eq!(tokens[1].value, "a");
        let function = tokens
            .iter()
            .find(|t| t.kind == TokenKind::Function)
            .unwrap();
        assert_eq!(function.name.as_deref(), Some("is"));
        assert!(function.tokens.iter().any(|t| t.value == "b"));
    }

    #[test]
    fn decodes_css_escapes() {
        let tokens = tokenize(r"\66 oo\+bar");
        assert_eq!(tokens[0].kind, TokenKind::Ident);
        assert_eq!(tokens[0].value, "foo+bar");
        assert_eq!(tokens[0].raw, r"\66 oo\+bar");
    }

    #[test]
    fn short_hex_escape_absorbs_hex_continuation() {
        let tokens = tokenize(r"foo\31b");
        assert_eq!(tokens[0].value, "foo\u{31b}");
        let tokens = tokenize(r"foo\31 b");
        assert_eq!(tokens[0].value, "foo1b");
    }

    #[test]
    fn parses_qualified_and_at_rules() {
        let rules = parse_rules(".a {} @media screen { .b {} } @custom { x: .c; }");
        assert_eq!(rules.len(), 3);
        assert!(matches!(rules[0], Rule::Qualified { .. }));
        match &rules[1] {
            Rule::AtRule { name, block, .. } => {
                assert_eq!(name, "media");
                assert!(block.is_some());
            }
            _ => panic!("expected at rule"),
        }
        match &rules[2] {
            Rule::AtRule { name, .. } => assert_eq!(name, "custom"),
            _ => panic!("expected at rule"),
        }
    }

    #[test]
    fn counts_crlf_as_one_position() {
        let tokens = tokenize("/*x*/\r\n.foo");
        let ident = tokens.last().unwrap();
        assert_eq!(ident.kind, TokenKind::Ident);
        // Position units: the CRLF counts as ONE normalized position, so
        // the ident lands at unit 7 (/,*,x,*,/,\n,. = 7 units before `foo`).
        assert_eq!(ident.pos, 7);
        assert_eq!(ident.raw, "foo");
    }

    #[test]
    fn handles_unterminated_blocks_and_strings() {
        let tokens = tokenize(".foo { fill: red");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::SimpleBlock));
        let tokens = tokenize(".a{content:\"<!-- keep -->\"}");
        let block = tokens
            .iter()
            .find(|t| t.kind == TokenKind::SimpleBlock)
            .unwrap();
        assert!(block.tokens.iter().any(|t| t.kind == TokenKind::String));
    }
}

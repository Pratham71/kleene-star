//! Regular expressions: the syntax tree, a parser, and a printer.
//!
//! Syntax: letters and digits are symbols, `|` is union, juxtaposition is
//! concatenation, `*` `+` `?` are postfix, `.` is any symbol, `()` is the
//! empty string and `[]` is the empty language. Spaces are ignored.

use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Regex {
    /// The empty language: matches nothing.
    Empty,
    /// The empty string.
    Eps,
    Sym(char),
    /// Any one symbol of the alphabet.
    Any,
    Concat(Box<Regex>, Box<Regex>),
    Alt(Box<Regex>, Box<Regex>),
    Star(Box<Regex>),
}

use Regex::*;

impl Regex {
    // The three constructors below simplify as they build, using identities
    // such as `[] | r = r` and `() r = r`. This keeps generated regexes short.

    pub fn alt(a: Regex, b: Regex) -> Regex {
        match (a, b) {
            (Empty, r) | (r, Empty) => r,
            (a, b) if a == b => a,
            (a, b) => Alt(Box::new(a), Box::new(b)),
        }
    }

    pub fn concat(a: Regex, b: Regex) -> Regex {
        match (a, b) {
            (Empty, _) | (_, Empty) => Empty,
            (Eps, r) | (r, Eps) => r,
            (a, b) => Concat(Box::new(a), Box::new(b)),
        }
    }

    pub fn star(a: Regex) -> Regex {
        match a {
            Empty | Eps => Eps,
            Star(inner) => Star(inner),
            a => Star(Box::new(a)),
        }
    }

    /// Collects every symbol written in the regex.
    pub fn symbols(&self, out: &mut BTreeSet<char>) {
        match self {
            Empty | Eps | Any => {}
            Sym(c) => {
                out.insert(*c);
            }
            Concat(a, b) | Alt(a, b) => {
                a.symbols(out);
                b.symbols(out);
            }
            Star(a) => a.symbols(out),
        }
    }

    /// Prints with as few parentheses as possible. `level` is the binding
    /// strength the context requires: 0 union, 1 concatenation, 2 postfix.
    fn fmt_at(&self, f: &mut fmt::Formatter, level: u8) -> fmt::Result {
        let own = match self {
            Alt(a, b) if **a == Eps || **b == Eps => 2, // printed as `r?`
            Alt(..) => 0,
            Concat(..) => 1,
            Star(_) => 2,
            _ => 3,
        };
        if own < level {
            write!(f, "(")?;
        }
        match self {
            Empty => write!(f, "[]")?,
            Eps => write!(f, "()")?,
            Sym(c) => write!(f, "{c}")?,
            Any => write!(f, ".")?,
            Alt(a, b) if **b == Eps => {
                a.fmt_at(f, 3)?;
                write!(f, "?")?;
            }
            Alt(a, b) if **a == Eps => {
                b.fmt_at(f, 3)?;
                write!(f, "?")?;
            }
            Alt(a, b) => {
                a.fmt_at(f, 0)?;
                write!(f, "|")?;
                b.fmt_at(f, 0)?;
            }
            Concat(a, b) => {
                a.fmt_at(f, 1)?;
                b.fmt_at(f, 1)?;
            }
            Star(a) => {
                a.fmt_at(f, 3)?;
                write!(f, "*")?;
            }
        }
        if own < level {
            write!(f, ")")?;
        }
        Ok(())
    }
}

impl fmt::Display for Regex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt_at(f, 0)
    }
}

/// A parse failure: byte offset into the regex text, and a message.
pub type ParseError = (usize, String);

/// Parses regex source text (without the surrounding slashes).
pub fn parse(src: &str) -> Result<Regex, ParseError> {
    let mut p = Parser { chars: src.char_indices().collect(), pos: 0, end: src.len() };
    let regex = p.alt()?;
    match p.peek() {
        None => Ok(regex),
        Some((at, ')')) => Err((at, "unmatched `)`".into())),
        Some((at, c)) => Err((at, format!("unexpected `{c}`"))),
    }
}

struct Parser {
    chars: Vec<(usize, char)>,
    pos: usize,
    end: usize,
}

impl Parser {
    /// The next character that is not a space, with its byte offset.
    fn peek(&mut self) -> Option<(usize, char)> {
        while self.pos < self.chars.len() && self.chars[self.pos].1 == ' ' {
            self.pos += 1;
        }
        self.chars.get(self.pos).copied()
    }

    // alt := concat ('|' concat)*
    fn alt(&mut self) -> Result<Regex, ParseError> {
        let mut left = self.concat()?;
        while let Some((_, '|')) = self.peek() {
            self.pos += 1;
            left = Regex::alt(left, self.concat()?);
        }
        Ok(left)
    }

    // concat := postfix*        (nothing at all means the empty string)
    fn concat(&mut self) -> Result<Regex, ParseError> {
        let mut left = Eps;
        while let Some((_, c)) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            left = Regex::concat(left, self.postfix()?);
        }
        Ok(left)
    }

    // postfix := atom ('*' | '+' | '?')*
    fn postfix(&mut self) -> Result<Regex, ParseError> {
        let mut inner = self.atom()?;
        loop {
            inner = match self.peek() {
                Some((_, '*')) => Regex::star(inner),
                Some((_, '+')) => Regex::concat(inner.clone(), Regex::star(inner)),
                Some((_, '?')) => Regex::alt(inner, Eps),
                _ => return Ok(inner),
            };
            self.pos += 1;
        }
    }

    // atom := symbol | '.' | '(' alt ')' | '[' ']' | '\' any
    fn atom(&mut self) -> Result<Regex, ParseError> {
        let (at, c) = self.peek().ok_or((self.end, "unexpected end of regex".to_string()))?;
        self.pos += 1;
        match c {
            '(' => {
                let inner = self.alt()?;
                match self.peek() {
                    Some((_, ')')) => {
                        self.pos += 1;
                        Ok(inner)
                    }
                    _ => Err((at, "this `(` is never closed".into())),
                }
            }
            '[' => match self.peek() {
                Some((_, ']')) => {
                    self.pos += 1;
                    Ok(Empty)
                }
                _ => Err((at, "expected `[]`, the empty language".into())),
            },
            '.' => Ok(Any),
            '\\' => match self.chars.get(self.pos) {
                Some(&(_, escaped)) => {
                    self.pos += 1;
                    Ok(Sym(escaped))
                }
                None => Err((at, "`\\` must be followed by a character".into())),
            },
            '*' | '+' | '?' => Err((at, format!("nothing for `{c}` to repeat"))),
            c if c.is_alphanumeric() => Ok(Sym(c)),
            c => Err((at, format!("`{c}` is not a symbol; symbols are letters and digits"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(src: &str) -> String {
        parse(src).unwrap().to_string()
    }

    #[test]
    fn parses_and_prints() {
        assert_eq!(roundtrip("(0|1)*01"), "(0|1)*01");
        assert_eq!(roundtrip("a b | c"), "ab|c");
        assert_eq!(roundtrip("a(b|c)"), "a(b|c)");
        assert_eq!(roundtrip("(ab)*"), "(ab)*");
        assert_eq!(roundtrip("a?"), "a?");
        assert_eq!(roundtrip("(ab)?c"), "(ab)?c");
        assert_eq!(roundtrip("a+"), "aa*");
        assert_eq!(roundtrip("()"), "()");
        assert_eq!(roundtrip("[]"), "[]");
        assert_eq!(roundtrip("."), ".");
    }

    #[test]
    fn simplifies_while_building() {
        assert_eq!(roundtrip("a|a"), "a");
        assert_eq!(roundtrip("a**"), "a*");
        assert_eq!(roundtrip("()a"), "a");
        assert_eq!(roundtrip("[]a|b"), "b");
        assert_eq!(roundtrip("[]*"), "()");
    }

    #[test]
    fn reports_errors_with_offsets() {
        assert_eq!(parse("a)").unwrap_err().0, 1);
        assert_eq!(parse("(ab").unwrap_err().0, 0);
        assert_eq!(parse("*a").unwrap_err().0, 0);
        assert_eq!(parse("a#").unwrap_err().0, 1);
    }

    #[test]
    fn collects_symbols() {
        let mut set = BTreeSet::new();
        parse("(0|1)*a").unwrap().symbols(&mut set);
        assert_eq!(set, BTreeSet::from(['0', '1', 'a']));
    }
}

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

    /// `a+`, one or more: `a a*`. There is no separate node for it, so `a`
    /// is copied; but `(a+)+` is just `a+` and `(a*)+` is `a*`, so repeated
    /// `+` does not double the size each time.
    pub fn plus(a: Regex) -> Regex {
        match a {
            Star(_) | Empty | Eps => Regex::star(a),
            Concat(ref x, ref y) if matches!(&**y, Star(z) if z == x) => a,
            a => Regex::concat(a.clone(), Regex::star(a)),
        }
    }

    /// `a?`, zero or one: `a|()`. `(a+)?` is `a*`.
    pub fn optional(a: Regex) -> Regex {
        match a {
            Star(_) | Eps => a,
            Concat(ref x, ref y) if matches!(&**y, Star(z) if z == x) => Regex::star(*x.clone()),
            a => Regex::alt(a, Eps),
        }
    }

    /// The number of nodes in the tree.
    pub fn size(&self) -> usize {
        match self {
            Empty | Eps | Any | Sym(_) => 1,
            Star(a) => 1 + a.size(),
            Concat(a, b) | Alt(a, b) => 1 + a.size() + b.size(),
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

/// How deeply a regex may nest, counting each symbol of a concatenation and
/// each alternative as a level, since they nest in the tree. The algorithms
/// on regexes are recursive, so this keeps them from overflowing the stack.
pub const MAX_DEPTH: usize = 500;
/// The most nodes a parsed regex may have. `+` copies what it repeats, so
/// without a limit a short regex could become enormous.
pub const MAX_SIZE: usize = 20_000;

/// Parses regex source text (without the surrounding slashes).
pub fn parse(src: &str) -> Result<Regex, ParseError> {
    let mut p = Parser { chars: src.char_indices().collect(), pos: 0, end: src.len(), depth: 0 };
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
    /// How deeply the part being parsed is nested; see `MAX_DEPTH`.
    depth: usize,
}

impl Parser {
    /// The next character that is not a space, with its byte offset.
    fn peek(&mut self) -> Option<(usize, char)> {
        while self.pos < self.chars.len() && self.chars[self.pos].1 == ' ' {
            self.pos += 1;
        }
        self.chars.get(self.pos).copied()
    }

    /// Goes one level deeper, failing past `MAX_DEPTH`.
    fn nest(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            let at = self.chars.get(self.pos).map_or(self.end, |&(at, _)| at);
            return Err((at, format!("this regex is too long or nested too deeply (the limit is {MAX_DEPTH} levels)")));
        }
        Ok(())
    }

    // alt := concat ('|' concat)*
    fn alt(&mut self) -> Result<Regex, ParseError> {
        let outer = self.depth;
        let mut left = self.concat()?;
        while let Some((_, '|')) = self.peek() {
            self.pos += 1;
            self.nest()?;
            left = Regex::alt(left, self.concat()?);
        }
        self.depth = outer;
        Ok(left)
    }

    // concat := postfix*        (nothing at all means the empty string)
    fn concat(&mut self) -> Result<Regex, ParseError> {
        let outer = self.depth;
        let mut left = Eps;
        while let Some((_, c)) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            self.nest()?;
            left = Regex::concat(left, self.postfix()?);
        }
        self.depth = outer;
        Ok(left)
    }

    // postfix := atom ('*' | '+' | '?')*
    fn postfix(&mut self) -> Result<Regex, ParseError> {
        let mut inner = self.atom()?;
        loop {
            let at = match self.peek() {
                Some((at, '*' | '+' | '?')) => at,
                _ => return Ok(inner),
            };
            inner = match self.chars[self.pos].1 {
                '*' => Regex::star(inner),
                '+' => Regex::plus(inner),
                _ => Regex::optional(inner),
            };
            self.pos += 1;
            if inner.size() > MAX_SIZE {
                return Err((at, format!("this regex is too large (more than {MAX_SIZE} parts once `+` is expanded)")));
            }
        }
    }

    // atom := symbol | '.' | '(' alt ')' | '[' ']' | '\' any
    fn atom(&mut self) -> Result<Regex, ParseError> {
        let (at, c) = self.peek().ok_or((self.end, "unexpected end of regex".to_string()))?;
        self.pos += 1;
        match c {
            '(' => {
                self.nest()?;
                let inner = self.alt()?;
                self.depth -= 1;
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

    // Regression: each `+` used to copy everything before it, so 30 of them
    // made a regex of a billion nodes and the program hung.
    #[test]
    fn repeated_plus_does_not_blow_up() {
        let r = parse(&format!("a{}", "+".repeat(40))).unwrap();
        assert_eq!(r.to_string(), "aa*");
        let r = parse(&format!("{}a{}", "(".repeat(40), ")+".repeat(40))).unwrap();
        assert_eq!(r.to_string(), "aa*");
        assert_eq!(parse("(ab)+?").unwrap().to_string(), "(ab)*");
        assert_eq!(parse("a*+").unwrap().to_string(), "a*");
        // Growth that cannot be avoided is refused, not attempted.
        let doubling = format!("{}x{}", "(".repeat(30), "y)+".repeat(30));
        assert!(parse(&doubling).unwrap_err().1.contains("too large"));
    }

    // Regression: a regex nested thousands deep overflowed the stack.
    #[test]
    fn deep_regexes_are_refused() {
        crate::with_big_stack(deep_regex_cases);
    }

    fn deep_regex_cases() {
        let deep = format!("{}a{}", "(".repeat(MAX_DEPTH + 1), ")".repeat(MAX_DEPTH + 1));
        assert!(parse(&deep).unwrap_err().1.contains("nested too deeply"));
        assert!(parse(&"a".repeat(MAX_DEPTH + 1)).is_err());
        assert!(parse(&"a".repeat(MAX_DEPTH - 1)).is_ok());
    }
}

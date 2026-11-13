//! Tokens: the words and punctuation the lexer cuts source text into.

/// Which source file a span is in: an index into the `SourceMap`
/// (`super::source`), which holds each file's name and text.
///
/// A number rather than a file name keeps `Span` small and `Copy`. The file
/// being run, or everything typed at the REPL, is always `FileId::MAIN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct FileId(pub u32);

impl FileId {
    /// The file being run, or the REPL's input.
    pub const MAIN: FileId = FileId(0);
}

/// Where something is in the source text. Every token, syntax-tree node and
/// error carries one, which is what makes precise error messages possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    /// Byte offsets into the file's text: `start..end`.
    pub start: usize,
    pub end: usize,
    /// Line number of `start`, counting from 1. (A `u32`, like the file id,
    /// keeps a span at three words; errors carry several spans.)
    pub line: u32,
    /// The file the offsets are in.
    pub file: FileId,
}

impl Span {
    /// A span in the main file. Most code makes spans by joining the spans
    /// of tokens; this is for the few places that start from offsets.
    pub fn new(start: usize, end: usize, line: u32) -> Span {
        Span { start, end, line, file: FileId::MAIN }
    }

    /// A span covering everything from the start of `self` to the end of
    /// `other`, which must be in the same file.
    pub fn to(self, other: Span) -> Span {
        Span { end: other.end.max(self.end), ..self }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    // Literals and names
    Int(i64),
    Str(String),
    /// A regex literal; holds the text between the slashes.
    Regex(String),
    Ident(String),

    // Keywords
    Alphabet,
    Dfa,
    Nfa,
    Pda,
    Tm,
    Grammar,
    Over,
    Start,
    Accept,
    Reject,
    Eps,
    Fn,
    Let,
    Return,
    If,
    Else,
    For,
    In,
    Print,
    Test,
    Draw,
    Trace,
    On,
    True,
    False,
    And,
    Or,
    Not,
    Accepts,

    // Punctuation
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Assign,    // =
    EqEq,      // ==
    NotEq,     // !=
    Lt,        // <
    Le,        // <=
    Gt,        // >
    Ge,        // >=
    Plus,      // +
    Minus,     // -
    Star,      // *
    Percent,   // %
    DotDot,    // ..
    DashDash,  // --   opens a transition label
    LongArrow, // -->  closes a transition label
    Arrow,     // ->   return type, grammar rules
    Bar,       // |    between grammar alternatives
    Slash,     // /    between pop and push in a PDA move

    Eof,
}

impl Tok {
    /// Every keyword, for editors to offer as completions.
    pub const KEYWORDS: &'static [&'static str] = &[
        "alphabet", "dfa", "nfa", "pda", "tm", "grammar", "over", "start", "accept", "reject", "eps", "fn",
        "let", "return", "if", "else", "for", "in", "print", "test", "draw", "trace", "on", "true", "false",
        "and", "or", "not", "accepts",
    ];

    pub fn keyword(word: &str) -> Option<Tok> {
        Some(match word {
            "alphabet" => Tok::Alphabet,
            "dfa" => Tok::Dfa,
            "nfa" => Tok::Nfa,
            "pda" => Tok::Pda,
            "tm" => Tok::Tm,
            "grammar" => Tok::Grammar,
            "over" => Tok::Over,
            "start" => Tok::Start,
            "accept" => Tok::Accept,
            "reject" => Tok::Reject,
            "eps" => Tok::Eps,
            "fn" => Tok::Fn,
            "let" => Tok::Let,
            "return" => Tok::Return,
            "if" => Tok::If,
            "else" => Tok::Else,
            "for" => Tok::For,
            "in" => Tok::In,
            "print" => Tok::Print,
            "test" => Tok::Test,
            "draw" => Tok::Draw,
            "trace" => Tok::Trace,
            "on" => Tok::On,
            "true" => Tok::True,
            "false" => Tok::False,
            "and" => Tok::And,
            "or" => Tok::Or,
            "not" => Tok::Not,
            "accepts" => Tok::Accepts,
            _ => return None,
        })
    }

    /// How the token is described in an error message.
    pub fn describe(&self) -> String {
        match self {
            Tok::Int(n) => format!("number `{n}`"),
            Tok::Str(_) => "a string".into(),
            Tok::Regex(_) => "a regex".into(),
            Tok::Ident(name) => format!("name `{name}`"),
            Tok::Eof => "the end of the file".into(),
            other => format!("`{}`", other.text()),
        }
    }

    fn text(&self) -> &'static str {
        match self {
            Tok::Alphabet => "alphabet",
            Tok::Dfa => "dfa",
            Tok::Nfa => "nfa",
            Tok::Pda => "pda",
            Tok::Tm => "tm",
            Tok::Grammar => "grammar",
            Tok::Over => "over",
            Tok::Start => "start",
            Tok::Accept => "accept",
            Tok::Reject => "reject",
            Tok::Eps => "eps",
            Tok::Fn => "fn",
            Tok::Let => "let",
            Tok::Return => "return",
            Tok::If => "if",
            Tok::Else => "else",
            Tok::For => "for",
            Tok::In => "in",
            Tok::Print => "print",
            Tok::Test => "test",
            Tok::Draw => "draw",
            Tok::Trace => "trace",
            Tok::On => "on",
            Tok::True => "true",
            Tok::False => "false",
            Tok::And => "and",
            Tok::Or => "or",
            Tok::Not => "not",
            Tok::Accepts => "accepts",
            Tok::LBrace => "{",
            Tok::RBrace => "}",
            Tok::LParen => "(",
            Tok::RParen => ")",
            Tok::LBracket => "[",
            Tok::RBracket => "]",
            Tok::Comma => ",",
            Tok::Colon => ":",
            Tok::Assign => "=",
            Tok::EqEq => "==",
            Tok::NotEq => "!=",
            Tok::Lt => "<",
            Tok::Le => "<=",
            Tok::Gt => ">",
            Tok::Ge => ">=",
            Tok::Plus => "+",
            Tok::Minus => "-",
            Tok::Star => "*",
            Tok::Percent => "%",
            Tok::DotDot => "..",
            Tok::DashDash => "--",
            Tok::LongArrow => "-->",
            Tok::Arrow => "->",
            Tok::Bar => "|",
            Tok::Slash => "/",
            Tok::Int(_) | Tok::Str(_) | Tok::Regex(_) | Tok::Ident(_) | Tok::Eof => "",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::Tok;

    #[test]
    fn keyword_list_matches_the_lexer() {
        for word in Tok::KEYWORDS {
            assert!(Tok::keyword(word).is_some(), "`{word}` is listed but not a keyword");
        }
        // Every keyword the lexer knows is in the list.
        let known = include_str!("token.rs")
            .lines()
            .filter(|line| line.trim_start().starts_with('"') && line.contains("=> Tok::"))
            .count();
        assert_eq!(known, Tok::KEYWORDS.len());
    }
}

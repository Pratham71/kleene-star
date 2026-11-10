//! Tokens: the words and punctuation the lexer cuts source text into.

/// Where something is in the source text. Every token, syntax-tree node and
/// error carries one, which is what makes precise error messages possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Byte offsets into the source: `start..end`.
    pub start: usize,
    pub end: usize,
    /// Line number of `start`, counting from 1.
    pub line: usize,
}

impl Span {
    /// A span covering everything from the start of `self` to the end of `other`.
    pub fn to(self, other: Span) -> Span {
        Span { start: self.start, end: other.end.max(self.end), line: self.line }
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
    Over,
    Start,
    Accept,
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
    Arrow,     // ->   return type

    Eof,
}

impl Tok {
    pub fn keyword(word: &str) -> Option<Tok> {
        Some(match word {
            "alphabet" => Tok::Alphabet,
            "dfa" => Tok::Dfa,
            "nfa" => Tok::Nfa,
            "over" => Tok::Over,
            "start" => Tok::Start,
            "accept" => Tok::Accept,
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
            Tok::Over => "over",
            Tok::Start => "start",
            Tok::Accept => "accept",
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
            Tok::Int(_) | Tok::Str(_) | Tok::Regex(_) | Tok::Ident(_) | Tok::Eof => "",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

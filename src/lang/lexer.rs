//! The lexer: turns source text into a list of tokens.

use super::error::{Diag, ErrorKind};
use super::token::{Span, Tok, Token};

/// A `# ...` comment. The parser never sees comments; only the formatter
/// (`kstar fmt`) needs them, to put them back where they were.
#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    /// The comment including its `#`, without trailing spaces.
    pub text: String,
    pub span: Span,
}

/// Lexes `src` starting at byte `from`, which is on line `first_line`.
/// (The REPL keeps all input in one growing string and lexes only the new
/// part; a file is lexed with `from = 0, first_line = 1`.)
pub fn lex(src: &str, from: usize, first_line: usize) -> Result<Vec<Token>, Diag> {
    lex_into(src, from, first_line, &mut Vec::new())
}

/// Lexes a whole file and also returns its comments, in order.
pub fn lex_with_comments(src: &str) -> Result<(Vec<Token>, Vec<Comment>), Diag> {
    let mut comments = Vec::new();
    let tokens = lex_into(src, 0, 1, &mut comments)?;
    Ok((tokens, comments))
}

fn lex_into(
    src: &str,
    from: usize,
    first_line: usize,
    comments: &mut Vec<Comment>,
) -> Result<Vec<Token>, Diag> {
    let mut tokens = Vec::new();
    let mut line = first_line;
    let mut i = from;
    let rest = |i: usize| src[i..].chars();

    while let Some(c) = rest(i).next() {
        let start = i;
        let span = move |end: usize| Span { start, end, line };
        let error = move |end: usize, message: String| Diag::new(ErrorKind::Syntax, span(end), message);

        // Whitespace and comments
        if c == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += c.len_utf8();
            continue;
        }
        if c == '#' {
            i += src[i..].find('\n').unwrap_or(src.len() - i);
            let text = src[start..i].trim_end();
            comments.push(Comment { text: text.to_string(), span: span(start + text.len()) });
            continue;
        }

        // Numbers
        if c.is_ascii_digit() {
            while rest(i).next().is_some_and(|c| c.is_ascii_digit()) {
                i += 1;
            }
            let value = src[start..i]
                .parse::<i64>()
                .map_err(|_| error(i, "this number is too large".into()))?;
            tokens.push(Token { tok: Tok::Int(value), span: span(i) });
            continue;
        }

        // Names and keywords
        if c.is_alphabetic() || c == '_' {
            while let Some(c) = rest(i).next().filter(|c| c.is_alphanumeric() || *c == '_') {
                i += c.len_utf8();
            }
            let word = &src[start..i];
            let tok = Tok::keyword(word).unwrap_or_else(|| Tok::Ident(word.to_string()));
            tokens.push(Token { tok, span: span(i) });
            continue;
        }

        // Strings: "..." with \" and \\ as the only escapes
        if c == '"' {
            let mut text = String::new();
            i += 1;
            loop {
                match rest(i).next() {
                    None | Some('\n') => {
                        return Err(error(i, "this string is never closed".into())
                            .hint("add a closing `\"` on the same line"));
                    }
                    Some('"') => {
                        i += 1;
                        break;
                    }
                    Some('\\') => match rest(i + 1).next() {
                        Some(e @ ('"' | '\\')) => {
                            text.push(e);
                            i += 2;
                        }
                        _ => {
                            return Err(Diag::new(
                                ErrorKind::Syntax,
                                Span { start: i, end: i + 1, line },
                                "unknown escape in string",
                            )
                            .hint("only `\\\"` and `\\\\` are escapes"));
                        }
                    },
                    Some(c) => {
                        text.push(c);
                        i += c.len_utf8();
                    }
                }
            }
            tokens.push(Token { tok: Tok::Str(text), span: span(i) });
            continue;
        }

        // A `/` right after a name, number, `eps` or closing bracket is the
        // slash of a PDA move (`a, Z/AZ`): a regex can never follow those.
        let after_operand = tokens.last().is_some_and(|t: &Token| {
            matches!(t.tok, Tok::Ident(_) | Tok::Int(_) | Tok::Eps | Tok::RParen | Tok::RBracket)
        });
        if c == '/' && after_operand {
            i += 1;
            tokens.push(Token { tok: Tok::Slash, span: span(i) });
            continue;
        }

        // Regex literals: /.../  (the language has no division, so `/` is free)
        if c == '/' {
            i += 1;
            let body = i;
            loop {
                match rest(i).next() {
                    None | Some('\n') => {
                        return Err(error(i, "this regex is never closed".into())
                            .hint("a regex is written between slashes: /(0|1)*/"));
                    }
                    Some('/') => break,
                    Some('\\') => i += 1 + rest(i + 1).next().map_or(0, |c| c.len_utf8()),
                    Some(c) => i += c.len_utf8(),
                }
            }
            let text = src[body..i].to_string();
            i += 1;
            tokens.push(Token { tok: Tok::Regex(text), span: span(i) });
            continue;
        }

        // Punctuation, longest match first
        let three = src.get(i..i + 3).unwrap_or("");
        let two = src.get(i..i + 2).unwrap_or("");
        let (tok, len) = if three == "-->" {
            (Tok::LongArrow, 3)
        } else {
            match two {
                "--" => (Tok::DashDash, 2),
                "->" => (Tok::Arrow, 2),
                "==" => (Tok::EqEq, 2),
                "!=" => (Tok::NotEq, 2),
                "<=" => (Tok::Le, 2),
                ">=" => (Tok::Ge, 2),
                ".." => (Tok::DotDot, 2),
                _ => match c {
                    '{' => (Tok::LBrace, 1),
                    '}' => (Tok::RBrace, 1),
                    '(' => (Tok::LParen, 1),
                    ')' => (Tok::RParen, 1),
                    '[' => (Tok::LBracket, 1),
                    ']' => (Tok::RBracket, 1),
                    ',' => (Tok::Comma, 1),
                    ':' => (Tok::Colon, 1),
                    '=' => (Tok::Assign, 1),
                    '<' => (Tok::Lt, 1),
                    '>' => (Tok::Gt, 1),
                    '+' => (Tok::Plus, 1),
                    '-' => (Tok::Minus, 1),
                    '*' => (Tok::Star, 1),
                    '%' => (Tok::Percent, 1),
                    '|' => (Tok::Bar, 1),
                    ';' => {
                        return Err(error(i + 1, "unexpected `;`".into())
                            .hint("Kleene Star has no semicolons; just end the line"));
                    }
                    other => {
                        return Err(error(
                            i + other.len_utf8(),
                            format!("unexpected character `{other}`"),
                        ));
                    }
                },
            }
        };
        i += len;
        tokens.push(Token { tok, span: span(i) });
    }

    tokens.push(Token { tok: Tok::Eof, span: Span { start: src.len(), end: src.len(), line } });
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        lex(src, 0, 1).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn lexes_a_transition() {
        assert_eq!(
            toks("q0 --0,1--> q1"),
            vec![
                Tok::Ident("q0".into()),
                Tok::DashDash,
                Tok::Int(0),
                Tok::Comma,
                Tok::Int(1),
                Tok::LongArrow,
                Tok::Ident("q1".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn lexes_literals_and_comments() {
        assert_eq!(
            toks("let r = /(0|1)*\\/a/ # comment\n\"a\\\"b\""),
            vec![
                Tok::Let,
                Tok::Ident("r".into()),
                Tok::Assign,
                Tok::Regex("(0|1)*\\/a".into()),
                Tok::Str("a\"b".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn lexes_ranges_and_arrows() {
        assert_eq!(
            toks("0..n -> a - b"),
            vec![
                Tok::Int(0),
                Tok::DotDot,
                Tok::Ident("n".into()),
                Tok::Arrow,
                Tok::Ident("a".into()),
                Tok::Minus,
                Tok::Ident("b".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn slash_after_an_operand_is_not_a_regex() {
        assert_eq!(
            toks("a, Z/AZ eps/eps"),
            vec![
                Tok::Ident("a".into()),
                Tok::Comma,
                Tok::Ident("Z".into()),
                Tok::Slash,
                Tok::Ident("AZ".into()),
                Tok::Eps,
                Tok::Slash,
                Tok::Eps,
                Tok::Eof
            ]
        );
        assert_eq!(toks("= /a|b/")[1], Tok::Regex("a|b".into()));
        assert_eq!(toks("S -> a | eps")[3], Tok::Bar);
    }

    #[test]
    fn keeps_comments_on_request() {
        let (tokens, comments) = lex_with_comments("let x = 1  # one   \n# two\n").unwrap();
        assert_eq!(tokens.len(), 5);
        assert_eq!(comments[0].text, "# one");
        assert_eq!(comments[1], Comment { text: "# two".into(), span: Span { start: 20, end: 25, line: 2 } });
    }

    #[test]
    fn tracks_lines() {
        let tokens = lex("a\n\n  b", 0, 1).unwrap();
        assert_eq!(tokens[1].span, Span { start: 5, end: 6, line: 3 });
    }

    #[test]
    fn reports_bad_input() {
        assert!(lex("\"abc", 0, 1).is_err());
        assert!(lex("/abc", 0, 1).is_err());
        assert!(lex("let x = 1;", 0, 1).unwrap_err().hint.is_some());
        assert!(lex("@", 0, 1).is_err());
    }
}

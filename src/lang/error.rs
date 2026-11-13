//! Errors, and how they are shown.
//!
//! Every problem the language can report is a `Diag` (diagnostic): a kind, a
//! stable code like `E0200`, a message, the place in the source, an optional
//! hint, and for errors that happen while running, the chain of calls that
//! led there. A diagnostic is either an error, which stops the program, or a
//! warning, which does not.

use super::codes;
use super::source::SourceMap;
use super::token::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The text is not a valid program.
    Syntax,
    /// A name is unknown, or defined twice.
    Name,
    /// A value of the wrong type is used.
    Type,
    /// A machine definition is not a valid DFA or NFA.
    Machine,
    /// A symbol is used with an alphabet it does not belong to.
    Alphabet,
    /// Arithmetic went wrong.
    Math,
    /// Functions called each other too deeply.
    Recursion,
    /// A search (a PDA run, a derivation) took too many steps to finish.
    Limit,
    /// A value is the right type but unsuitable, like a word a grammar
    /// cannot derive.
    Value,
    /// A file could not be written.
    Io,
    /// Not an error: something that is allowed but probably a mistake.
    Warning,
}

impl ErrorKind {
    pub fn name(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "SyntaxError",
            ErrorKind::Name => "NameError",
            ErrorKind::Type => "TypeError",
            ErrorKind::Machine => "MachineError",
            ErrorKind::Alphabet => "AlphabetError",
            ErrorKind::Math => "MathError",
            ErrorKind::Recursion => "RecursionError",
            ErrorKind::Limit => "LimitError",
            ErrorKind::Value => "ValueError",
            ErrorKind::Io => "IOError",
            ErrorKind::Warning => "Warning",
        }
    }

    /// The code an error of this kind gets when no more specific one fits.
    pub fn default_code(self) -> &'static str {
        match self {
            ErrorKind::Syntax => codes::SYNTAX,
            ErrorKind::Name => codes::UNDEFINED,
            ErrorKind::Type => codes::TYPE_MISMATCH,
            ErrorKind::Machine => codes::BAD_MACHINE,
            ErrorKind::Alphabet => codes::NOT_IN_ALPHABET,
            ErrorKind::Math => codes::OVERFLOW,
            ErrorKind::Recursion => codes::RECURSION,
            ErrorKind::Limit => codes::GAVE_UP,
            ErrorKind::Value => codes::NO_DERIVATION,
            ErrorKind::Io => codes::IO,
            ErrorKind::Warning => codes::UNUSED,
        }
    }
}

/// One line of a traceback: which function was running, and where it was.
#[derive(Debug, Clone)]
pub struct TraceFrame {
    pub func: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Diag {
    pub kind: ErrorKind,
    /// A stable code, `E....` for errors and `W....` for warnings, which
    /// `kstar explain` describes at length.
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    pub hint: Option<String>,
    /// Outermost call first. Empty for errors found before the program runs.
    pub trace: Vec<TraceFrame>,
}

impl Diag {
    pub fn new(kind: ErrorKind, span: Span, message: impl Into<String>) -> Diag {
        Diag { kind, code: kind.default_code(), message: message.into(), span, hint: None, trace: Vec::new() }
    }

    /// A warning: reported, but the program still runs.
    pub fn warning(code: &'static str, span: Span, message: impl Into<String>) -> Diag {
        Diag::new(ErrorKind::Warning, span, message).code(code)
    }

    pub fn code(mut self, code: &'static str) -> Diag {
        self.code = code;
        self
    }

    pub fn is_warning(&self) -> bool {
        self.kind == ErrorKind::Warning
    }

    /// The last line of the report: `TypeError[E0300]: ...`.
    pub fn headline(&self) -> String {
        format!("{}[{}]: {}", self.kind.name(), self.code, self.message)
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Diag {
        self.hint = Some(hint.into());
        self
    }

    /// Formats the error for the terminal.
    ///
    /// An error found before running shows the place and the problem:
    ///
    /// ```text
    /// Error in demo.kstar, line 5:
    ///     q0 --2--> q1
    ///          ^
    /// AlphabetError: symbol '2' is not in alphabet B
    ///   hint: B = {0, 1}
    /// ```
    ///
    /// An error while running shows the calls that led to it, like Python:
    ///
    /// ```text
    /// Traceback (most recent call last):
    ///   demo.kstar, line 28, in <main>
    ///     test div_by(0) accepts "110"
    ///   demo.kstar, line 17, in div_by
    ///     q[i] --0--> q[(2 * i) % n]
    ///                   ^^^^^^^^^^^
    /// MathError: modulo by zero
    /// ```
    ///
    /// Each line of the report names the file its span is in, so a
    /// traceback can pass through several files; `sources` supplies their
    /// names and text.
    pub fn render(&self, sources: &SourceMap) -> String {
        self.render_styled(sources, false)
    }

    /// Like `render`, with terminal colours when `color` is true: errors in
    /// red, warnings in yellow, hints in cyan.
    pub fn render_styled(&self, sources: &SourceMap, color: bool) -> String {
        let paint = Paint(color);
        let accent = if self.is_warning() { YELLOW } else { RED };
        let mut out = String::new();
        if self.trace.is_empty() {
            let what = if self.is_warning() { "Warning" } else { "Error" };
            let file = sources.name(self.span.file);
            out.push_str(&paint.bold(&format!("{what} in {file}, line {}:", self.span.line)));
            out.push('\n');
            out.push_str(&excerpt(sources.text(self.span.file), self.span, "    ", Some((paint, accent))));
        } else {
            out.push_str(&paint.bold("Traceback (most recent call last):"));
            out.push('\n');
            // Runaway recursion makes hundreds of frames; show the first and
            // last few and say how many were left out.
            const SHOWN: usize = 5;
            let total = self.trace.len();
            for (i, frame) in self.trace.iter().enumerate() {
                if total > 2 * SHOWN + 1 && i >= SHOWN && i < total - SHOWN {
                    if i == SHOWN {
                        out.push_str(&format!("  ... {} more calls ...\n", total - 2 * SHOWN));
                    }
                    continue;
                }
                let file = sources.name(frame.span.file);
                out.push_str(&format!("  {file}, line {}, in {}\n", frame.span.line, frame.func));
                let carets = (i + 1 == total).then_some((paint, accent));
                out.push_str(&excerpt(sources.text(frame.span.file), frame.span, "    ", carets));
            }
        }
        out.push_str(&paint.paint(accent, &self.headline()));
        if let Some(hint) = &self.hint {
            out.push_str(&format!("\n  {} {hint}", paint.paint(CYAN, "hint:")));
        }
        out
    }
}

const RED: &str = "1;31";
const YELLOW: &str = "1;33";
const CYAN: &str = "36";
const GREEN: &str = "1;32";

/// Wraps text in ANSI colour codes, or leaves it alone when colour is off.
#[derive(Debug, Clone, Copy)]
pub struct Paint(pub bool);

impl Paint {
    fn paint(self, style: &str, text: &str) -> String {
        if self.0 {
            format!("\x1b[{style}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn bold(self, text: &str) -> String {
        self.paint("1", text)
    }

    pub fn red(self, text: &str) -> String {
        self.paint(RED, text)
    }

    pub fn green(self, text: &str) -> String {
        self.paint(GREEN, text)
    }

    pub fn yellow(self, text: &str) -> String {
        self.paint(YELLOW, text)
    }
}

/// The source line containing `span`, indented, optionally with a row of
/// carets underneath (in the given colour) marking exactly which part is meant.
fn excerpt(src: &str, span: Span, indent: &str, carets: Option<(Paint, &str)>) -> String {
    // A span may end inside a character, or past the end; never slice there.
    let floor = |offset: usize| {
        let mut offset = offset.min(src.len());
        while !src.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    };
    let start = floor(span.start);
    let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = src[start..].find('\n').map_or(src.len(), |i| start + i);
    let line = src[line_start..line_end].trim_end();
    let shown = line.trim_start();
    // Counted in characters, like the carets: the indent may not be ASCII.
    let skipped = line[..line.len() - shown.len()].chars().count();

    let mut out = format!("{indent}{shown}\n");
    if let Some((paint, style)) = carets {
        let before = src[line_start..start].chars().count().saturating_sub(skipped);
        let end = floor(span.end).clamp(start, (line_start + line.len()).max(start));
        let width = src[start..end].chars().count().max(1);
        out.push_str(&format!("{indent}{}{}\n", " ".repeat(before), paint.paint(style, &"^".repeat(width))));
    }
    out
}

/// The candidate closest to `name`, if it is close enough to be a likely typo.
pub fn did_you_mean<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (name.chars().count() / 3).max(1);
    candidates
        .map(|c| (edit_distance(name, c), c))
        .filter(|&(d, _)| d <= limit)
        .min_by_key(|&(d, _)| d)
        .map(|(_, c)| c)
}

/// Levenshtein distance: the number of single-character edits between two words.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == *cb { diagonal } else { diagonal + 1 };
            diagonal = row[j + 1];
            row[j + 1] = cost.min(row[j] + 1).min(diagonal + 1);
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_close_names() {
        let names = ["minimize", "to_dfa", "union"];
        assert_eq!(did_you_mean("minimise", names.into_iter()), Some("minimize"));
        assert_eq!(did_you_mean("to_dfs", names.into_iter()), Some("to_dfa"));
        assert_eq!(did_you_mean("banana", names.into_iter()), None);
    }

    #[test]
    fn renders_carets_under_the_span() {
        let src = "let a = 1\n    let b = oops + 1\n";
        let start = src.find("oops").unwrap();
        let d = Diag::new(ErrorKind::Name, Span::new(start, start + 4, 2), "unknown").hint("define it first");
        assert_eq!(
            d.render(&SourceMap::new("x.kstar", src)),
            "Error in x.kstar, line 2:\n    let b = oops + 1\n            ^^^^\nNameError[E0200]: unknown\n  hint: define it first"
        );
    }

    // Regression: a span ending inside a multi-byte character (a regex error
    // at a byte-order mark) made rendering panic.
    #[test]
    fn spans_inside_characters_render() {
        let src = "let r = /a\u{feff}/\n";
        let at = src.find('\u{feff}').unwrap();
        let sources = SourceMap::new("x.kstar", src);
        let d = Diag::new(ErrorKind::Syntax, Span::new(at, at + 1, 1), "odd");
        assert!(d.render(&sources).contains("^"));
        let past = Diag::new(ErrorKind::Syntax, Span::new(999, 2000, 1), "past the end");
        assert!(past.render(&sources).contains("past the end"));
    }

    // Regression: carets were misplaced under a line indented with
    // non-ASCII spaces, because the indent was counted in bytes.
    #[test]
    fn carets_line_up_after_a_wide_indent() {
        let src = "\u{3000}\u{3000}oops here\n";
        let start = src.find("here").unwrap();
        let d = Diag::new(ErrorKind::Name, Span::new(start, start + 4, 1), "x");
        let shown = d.render(&SourceMap::new("x.kstar", src));
        assert!(shown.contains("    oops here\n         ^^^^\n"), "{shown}");
    }

    // A traceback can pass through several files: each frame is shown with
    // the name and the line of the file it is in.
    #[test]
    fn a_traceback_names_the_file_of_each_frame() {
        use super::super::token::FileId;
        let mut sources = SourceMap::new("main.kstar", "let a = 1\nprint half(3)\n");
        let lib = sources.add("lib.kstar", "fn half(n: Int) -> Int {\n    return n % 0\n}\n");
        let call = sources.text(FileId::MAIN).find("half(3)").unwrap();
        let at = sources.text(lib).find("n % 0").unwrap();
        let mut d = Diag::new(ErrorKind::Math, Span { start: at, end: at + 5, line: 2, file: lib }, "modulo by zero");
        d.trace = vec![
            TraceFrame { func: "<main>".into(), span: Span::new(call, call + 7, 2) },
            TraceFrame { func: "half".into(), span: d.span },
        ];
        assert_eq!(
            d.render(&sources),
            "Traceback (most recent call last):\n  main.kstar, line 2, in <main>\n    print half(3)\n  lib.kstar, line 2, in half\n    return n % 0\n           ^^^^^\nMathError[E0600]: modulo by zero"
        );
    }
}

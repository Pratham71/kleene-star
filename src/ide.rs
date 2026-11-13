//! Answers to the questions an editor asks: what is wrong, what is this,
//! where is it defined, where is it used, what can I type here?
//!
//! Everything here works on byte offsets into the source and uses the real
//! lexer, parser and checker; nothing is parsed a second way. The language
//! server (`src/bin/kstar-lsp.rs`) only converts between these offsets and the
//! line/column positions of the Language Server Protocol.

use crate::lang::builtins;
use crate::lang::check::Checker;
use crate::lang::error::Diag;
use crate::lang::index::{DefKind, Index};
use crate::lang::lexer::lex;
use crate::lang::module::{Loader, Reader};
use crate::lang::source::SourceMap;
use crate::lang::stdlib;
use crate::lang::token::{FileId, Span, Tok};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// An error or a warning in a program.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub span: Span,
    /// `TypeError[E0300]: ...`, the way the command line shows it.
    pub message: String,
    pub hint: Option<String>,
    /// The code alone, like `E0300`.
    pub code: &'static str,
    /// A warning rather than an error.
    pub warning: bool,
}

impl From<Diag> for Problem {
    fn from(diag: Diag) -> Problem {
        Problem { span: diag.span, message: diag.headline(), code: diag.code, warning: diag.is_warning(), hint: diag.hint }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Keyword,
    Function,
    Variable,
    State,
    Symbol,
    Module,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: String,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SignatureHelp {
    /// The whole signature, like `union(a, b) -> Dfa`.
    pub label: String,
    pub params: Vec<String>,
    /// Which parameter the cursor is in, counting from 0.
    pub active: usize,
    pub doc: Option<String>,
}

/// What kind of thing an outline entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Alphabet,
    Machine,
    Grammar,
    Function,
    Value,
    /// A state inside a machine, or a variable inside a grammar.
    Part,
}

/// One entry of a file's outline, for the editor's outline view and
/// "go to symbol".
#[derive(Debug, Clone, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// A short description, like `dfa over B`.
    pub detail: String,
    /// The whole definition.
    pub span: Span,
    /// Just the name.
    pub name_span: Span,
    /// States of a machine, variables of a grammar.
    pub children: Vec<Symbol>,
}

/// Everything known about one version of a file.
pub struct Analysis {
    pub src: String,
    /// The file and every file it imports. A definition in another file
    /// has a span in one of them.
    pub sources: SourceMap,
    /// Where each imported file is, for editors to open it.
    pub paths: HashMap<FileId, PathBuf>,
    /// Every error, then (only if there are no errors) every warning.
    pub problems: Vec<Problem>,
    pub index: Index,
    /// The top-level definitions, in order.
    pub outline: Vec<Symbol>,
    /// True when the text did not parse and `index` was carried over from an
    /// earlier version. Its names are still useful for completion, but its
    /// positions no longer match the text.
    pub stale: bool,
}

impl Analysis {
    /// Lexes, parses and checks `src`, exactly as `kstar check` does. The
    /// index holds whatever the checker learned, even from a program with
    /// errors. Imports are relative to the current folder.
    pub fn new(src: &str) -> Analysis {
        Analysis::open(src, None, None)
    }

    /// Like `new`, for the file at `path`, whose imports are relative to its
    /// folder. `read`, if given, reads the imported files (an editor knows
    /// the text of the files it has open); otherwise they are read from disk.
    ///
    /// An error inside an imported file is shown at the import in this file
    /// that leads to it, since that is the file being edited.
    pub fn open(src: &str, path: Option<&Path>, read: Option<Reader>) -> Analysis {
        let src = src.replace("\r\n", "\n");
        let mut loader = Loader::from_disk(path.map(Path::to_path_buf));
        if let Some(read) = read {
            loader.set_reader(read);
        }
        // The file is called by its own name, so imported files are named
        // relative to it: `lib/utils.kstar`.
        let name = path.and_then(Path::file_name).map_or("<editor>".into(), |n| n.to_string_lossy().to_string());
        let mut sources = SourceMap::new(&name, &src);
        let mut checker = Checker::new(false);
        let checked = crate::lang::check_source(FileId::MAIN, &mut sources, 0, 1, false, &mut checker, &mut loader);
        let sites = loader.import_sites(&checked.program);
        let problems = checked
            .errors
            .into_iter()
            .chain(checked.warnings)
            .map(|diag| {
                let file = diag.span.file;
                if file == FileId::MAIN {
                    return Problem::from(diag);
                }
                let place = format!("{}, line {}", sources.name(file), diag.span.line);
                let mut problem = Problem::from(diag);
                problem.message = format!("in {place}: {}", problem.message);
                problem.span = sites.get(&file).copied().unwrap_or_default();
                problem
            })
            .collect();
        let outline = outline(&checked.program, &checker.index);
        let paths = (1..sources.len() as u32)
            .map(FileId)
            .filter_map(|file| loader.path(file).map(|p| (file, p.to_path_buf())))
            .collect();
        Analysis { src, sources, paths, problems, index: checker.index, outline, stale: false }
    }

    /// The file in the standard style, as `kstar fmt` writes it, or `None`
    /// if it does not parse (or is already formatted).
    pub fn formatted(&self) -> Option<String> {
        crate::format_program("<editor>", &self.src).ok().filter(|f| *f != self.src)
    }

    /// The errors alone, without the warnings.
    pub fn errors(&self) -> impl Iterator<Item = &Problem> {
        self.problems.iter().filter(|p| !p.warning)
    }

    /// Like `new`, but if the text does not parse, keeps the names from
    /// `previous` so completion keeps working while the user is typing.
    pub fn with_fallback(src: &str, previous: Option<&Analysis>) -> Analysis {
        Analysis::open_with_fallback(src, previous, None, None)
    }

    /// `open` and `with_fallback` together, for the language server.
    pub fn open_with_fallback(
        src: &str,
        previous: Option<&Analysis>,
        path: Option<&Path>,
        read: Option<Reader>,
    ) -> Analysis {
        let mut analysis = Analysis::open(src, path, read);
        if analysis.index.defs.is_empty() {
            if let Some(previous) = previous.filter(|p| !p.index.defs.is_empty()) {
                analysis.index = previous.index.clone();
                analysis.stale = true;
            }
        }
        analysis
    }

    // ---- names ----

    /// The definition under `offset`, whether the cursor is on the
    /// definition itself or on a use of it.
    fn symbol_at(&self, offset: usize) -> Option<usize> {
        if self.stale {
            return None;
        }
        // Every use is in this file; a definition may be in another one.
        let touches = |span: &Span| span.file == FileId::MAIN && span.start <= offset && offset <= span.end;
        self.index
            .refs
            .iter()
            .find(|(span, _)| touches(span))
            .map(|&(_, def)| def)
            .or_else(|| self.index.defs.iter().position(|d| touches(&d.span)))
    }

    /// Where the thing under `offset` is defined. The span may be in an
    /// imported file: `file_path` says where that is. For a name taken with
    /// `from ... import`, this is where its module defines it.
    pub fn definition(&self, offset: usize) -> Option<Span> {
        let def = &self.index.defs[self.symbol_at(offset)?];
        let span = def.origin.unwrap_or(def.span);
        (span.file != FileId::STD).then_some(span)
    }

    /// Where file `file` (from a span) is, or `None` for this file.
    pub fn file_path(&self, file: FileId) -> Option<&Path> {
        self.paths.get(&file).map(PathBuf::as_path)
    }

    /// The text of file `file`, to turn its spans into lines and columns.
    pub fn file_text(&self, file: FileId) -> &str {
        if file == FileId::MAIN {
            &self.src
        } else {
            self.sources.text(file)
        }
    }

    /// Every place the thing under `offset` is used, in order, optionally
    /// including its definition.
    pub fn references(&self, offset: usize, include_definition: bool) -> Vec<Span> {
        let Some(def) = self.symbol_at(offset) else { return Vec::new() };
        let mut spans: Vec<Span> = self.index.refs.iter().filter(|(_, d)| *d == def).map(|&(s, _)| s).collect();
        let definition = self.index.defs[def].span;
        spans.retain(|s| *s != definition);
        if include_definition && definition.file == FileId::MAIN {
            spans.push(definition);
        }
        spans.sort_by_key(|s| s.start);
        spans
    }

    // ---- hover ----

    /// What to show when the mouse rests on `offset`: the span it is about,
    /// and Markdown text.
    pub fn hover(&self, offset: usize) -> Option<(Span, String)> {
        let offset = char_boundary(&self.src, offset);
        if self.stale {
            return None;
        }
        if let Some(def) = self.symbol_at(offset) {
            let def = &self.index.defs[def];
            let span = self.word_at(offset).unwrap_or(def.span);
            let text = match def.kind {
                // These details are sentences, not code.
                DefKind::State | DefKind::GrammarVariable => def.detail.clone(),
                _ => code(&def.detail),
            };
            return Some((span, text));
        }
        let touches = |span: &Span| span.start <= offset && offset <= span.end;
        if let Some(call) = self.index.calls.iter().find(|c| touches(&c.name_span)) {
            if let Some(doc) = builtins::doc(&call.name) {
                return Some((call.name_span, format!("{}\n\n{}", code(doc.signature), doc.doc)));
            }
        }
        // Otherwise, the type of the smallest expression around the cursor.
        let (span, ty) = self
            .index
            .exprs
            .iter()
            .filter(|(span, _)| touches(span))
            .min_by_key(|(span, _)| span.end - span.start)?;
        let text = &self.src[span.start..span.end];
        let shown = if text.len() <= 40 && !text.contains('\n') { format!("{text}: {ty}") } else { ty.to_string() };
        Some((*span, code(&shown)))
    }

    /// The identifier-like word around `offset`, if any.
    fn word_at(&self, offset: usize) -> Option<Span> {
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let offset = char_boundary(&self.src, offset);
        let before = self.src[..offset].char_indices().rev().find(|&(_, c)| !is_word(c));
        // Just after the character that ends the word, however wide it is.
        let start = before.map_or(0, |(i, c)| i + c.len_utf8());
        let end = self.src[offset..].find(|c: char| !is_word(c)).map_or(self.src.len(), |i| offset + i);
        (start < end).then(|| Span::new(start, end, self.src[..start].matches('\n').count() as u32 + 1))
    }

    // ---- completion ----

    /// What could be typed at `offset`.
    pub fn completions(&self, offset: usize) -> Vec<Completion> {
        let offset = char_boundary(&self.src, offset);
        let inside = |(start, end): (usize, usize)| start <= offset && offset <= end;
        if let Some(path) = self.qualifier_before(offset) {
            return self.members(&path, offset);
        }
        let mut out: Vec<Completion> = Vec::new();
        let push = |out: &mut Vec<Completion>, c: Completion| {
            if !out.iter().any(|o| o.label == c.label) {
                out.push(c);
            }
        };

        // Inside a machine or grammar: its states (or variables) and the
        // symbols of its alphabet come first.
        let body = if self.stale {
            None
        } else {
            self.index.bodies.iter().filter(|b| inside((b.span.start, b.span.end))).min_by_key(|b| b.span.end - b.span.start)
        };
        if let Some(body) = body {
            for def in self.index.defs.iter().rev() {
                if matches!(def.kind, DefKind::State | DefKind::GrammarVariable) && def.visible == (body.span.start, body.span.end) {
                    let c = Completion { label: def.name.clone(), kind: CompletionKind::State, detail: def.detail.clone(), doc: None };
                    push(&mut out, c);
                }
            }
            for symbol in &body.symbols {
                let c = Completion { label: symbol.to_string(), kind: CompletionKind::Symbol, detail: "alphabet symbol".into(), doc: None };
                push(&mut out, c);
            }
            for word in ["start", "accept", "reject", "eps", "for", "in"] {
                push(&mut out, keyword(word));
            }
        }

        // Names that can be seen here, innermost first. (Definitions in
        // other files are only reachable through their module's name.)
        for def in self.index.defs.iter().rev().filter(|d| d.span.file == FileId::MAIN) {
            let usable = match def.kind {
                DefKind::State | DefKind::GrammarVariable => false,
                DefKind::Function => true,
                _ if self.stale => def.visible.1 == usize::MAX,
                _ => inside(def.visible),
            };
            if usable {
                let kind = completion_kind(def.kind, &def.detail);
                push(&mut out, Completion { label: def.name.clone(), kind, detail: def.detail.clone(), doc: None });
            }
        }
        let std = Completion {
            label: "std".into(),
            kind: CompletionKind::Module,
            detail: "the standard library".into(),
            doc: None,
        };
        push(&mut out, std);
        for name in builtins::NAMES {
            let doc = builtins::doc(name).expect("every built-in is documented");
            let c = Completion {
                label: name.to_string(),
                kind: CompletionKind::Function,
                detail: doc.signature.to_string(),
                doc: Some(doc.doc.to_string()),
            };
            push(&mut out, c);
        }
        for word in Tok::KEYWORDS {
            push(&mut out, keyword(word));
        }
        out
    }

    /// When the cursor is just after `name.` (or `name.par`), the qualified
    /// name before the last dot: `utils`, or `std.math`.
    fn qualifier_before(&self, offset: usize) -> Option<String> {
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let before = &self.src[..offset];
        let partial = before.trim_end_matches(is_word);
        let path = partial.strip_suffix('.')?;
        let start = path.trim_end_matches(|c: char| is_word(c) || c == '.').len();
        let path = &path[start..];
        (!path.is_empty() && !path.starts_with('.') && !path.contains("..")).then(|| path.to_string())
    }

    /// The names inside the module `path` (`utils` or `std.math`), or the
    /// modules of the standard library for `std`.
    fn members(&self, path: &str, offset: usize) -> Vec<Completion> {
        if path == "std" {
            return stdlib::MODULES
                .iter()
                .map(|m| Completion {
                    label: m.name.to_string(),
                    kind: CompletionKind::Module,
                    detail: format!("std.{}", m.name),
                    doc: Some(m.doc.to_string()),
                })
                .collect();
        }
        if let Some(exports) = path.strip_prefix("std.").and_then(stdlib::exports) {
            let mut found: Vec<Completion> = exports
                .functions
                .values()
                .map(|f| Completion {
                    label: f.def.name.clone(),
                    kind: CompletionKind::Function,
                    detail: f.def.detail.clone(),
                    doc: None,
                })
                .collect();
            found.sort_by(|a, b| a.label.cmp(&b.label));
            return found;
        }
        let namespace = self
            .index
            .namespaces
            .iter()
            .rev()
            .find(|n| n.name == path && (self.stale || (n.visible.0 <= offset && offset <= n.visible.1)));
        let Some(namespace) = namespace else { return Vec::new() };
        namespace
            .members
            .iter()
            .map(|def| Completion {
                label: def.name.clone(),
                kind: completion_kind(def.kind, &def.detail),
                detail: def.detail.clone(),
                doc: None,
            })
            .collect()
    }

    // ---- signature help ----

    /// The function being called around `offset`, and which argument the
    /// cursor is in. This reads the tokens before the cursor rather than the
    /// syntax tree, so it works while the call is still being typed.
    pub fn signature_help(&self, offset: usize) -> Option<SignatureHelp> {
        let offset = char_boundary(&self.src, offset);
        let tokens = lex(&self.src, 0, 1).or_else(|_| lex(&self.src[..offset], 0, 1)).ok()?;
        // Open brackets before the cursor; for `(` after a name, the name
        // and how many commas have been passed.
        let mut open: Vec<Option<(String, usize)>> = Vec::new();
        let mut previous: Option<&Tok> = None;
        // The qualified name just read, like `utils.double`.
        let mut name_so_far = String::new();
        for token in tokens.iter().take_while(|t| t.span.end <= offset && t.tok != Tok::Eof) {
            match &token.tok {
                Tok::Ident(part) if previous == Some(&Tok::Dot) => name_so_far = format!("{name_so_far}.{part}"),
                Tok::Ident(part) => name_so_far = part.clone(),
                Tok::Dot => {}
                _ => {}
            }
            match &token.tok {
                Tok::LParen => open.push(match previous {
                    Some(Tok::Ident(_)) => Some((name_so_far.clone(), 0)),
                    _ => None,
                }),
                Tok::LBrace | Tok::LBracket => open.push(None),
                Tok::RParen | Tok::RBrace | Tok::RBracket => {
                    open.pop();
                }
                Tok::Comma => {
                    if let Some(Some((_, commas))) = open.last_mut() {
                        *commas += 1;
                    }
                }
                _ => {}
            }
            previous = Some(&token.tok);
        }
        let (name, active) = open.into_iter().rev().find_map(|frame| frame)?;
        if let Some(doc) = builtins::doc(&name) {
            return Some(SignatureHelp {
                label: doc.signature.to_string(),
                params: doc.params.iter().map(|p| p.to_string()).collect(),
                active,
                doc: Some(doc.doc.to_string()),
            });
        }
        let (detail, doc) = self.function_detail(&name)?;
        // The detail is `fn name(a: Int, b: Str) -> Dfa`.
        let inside = detail.split_once('(')?.1.rsplit_once(')')?.0;
        let params = if inside.is_empty() { Vec::new() } else { inside.split(", ").map(String::from).collect() };
        Some(SignatureHelp { label: detail.trim_start_matches("fn ").to_string(), params, active, doc })
    }

    /// The description (`fn name(...) -> T`) and documentation of the
    /// function that `name` calls: one in this file, one taken from a module
    /// with `from`, or a qualified `module.name`.
    fn function_detail(&self, name: &str) -> Option<(String, Option<String>)> {
        if let Some((path, member)) = name.rsplit_once('.') {
            if let Some(exports) = path.strip_prefix("std.").and_then(stdlib::exports) {
                return exports.functions.get(member).map(|f| (f.def.detail.clone(), None));
            }
            let namespace = self.index.namespaces.iter().rev().find(|n| n.name == path)?;
            let def = namespace.members.iter().find(|d| d.name == member && d.detail.starts_with("fn "))?;
            return Some((def.detail.clone(), None));
        }
        let is_function = |kind: DefKind, detail: &str| {
            kind == DefKind::Function || (kind == DefKind::Imported && detail.starts_with("fn "))
        };
        let def = self
            .index
            .defs
            .iter()
            .filter(|d| d.span.file == FileId::MAIN)
            .find(|d| is_function(d.kind, &d.detail) && d.name == name)?;
        // A function taken under a new name (`from m import double as
        // twice`) shows the name it is called by.
        let detail = match def.kind {
            DefKind::Imported => {
                let rest = def.detail.split_once('(').map_or("", |(_, rest)| rest);
                format!("fn {name}({rest}")
            }
            _ => def.detail.clone(),
        };
        Some((detail, None))
    }
}

/// How a definition is shown in a completion list.
fn completion_kind(kind: DefKind, detail: &str) -> CompletionKind {
    match kind {
        DefKind::Function => CompletionKind::Function,
        DefKind::Module => CompletionKind::Module,
        DefKind::Imported if detail.starts_with("fn ") => CompletionKind::Function,
        _ => CompletionKind::Variable,
    }
}

fn code(text: &str) -> String {
    format!("```kstar\n{text}\n```")
}

fn keyword(word: &str) -> Completion {
    Completion { label: word.to_string(), kind: CompletionKind::Keyword, detail: "keyword".into(), doc: None }
}

// ---- positions ----

/// The byte offset of an LSP position: a line (from 0) and a character
/// counted in UTF-16 code units, as the protocol requires.
pub fn offset_at(src: &str, line: u32, character: u32) -> usize {
    let mut start = 0;
    for _ in 0..line {
        match src[start..].find('\n') {
            Some(i) => start += i + 1,
            None => return src.len(),
        }
    }
    let mut units = 0;
    for (i, c) in src[start..].char_indices() {
        if c == '\n' || units >= character as usize {
            return start + i;
        }
        units += c.len_utf16();
    }
    src.len()
}

/// The outline of a program: its top-level definitions, with the states of
/// each machine and the variables of each grammar inside them.
fn outline(program: &[crate::lang::ast::Stmt], index: &Index) -> Vec<Symbol> {
    use crate::lang::ast::StmtKind;
    // The states (or grammar variables) defined inside `body`.
    let parts = |body: Span, kind: DefKind| -> Vec<Symbol> {
        index
            .defs
            .iter()
            .filter(|d| d.kind == kind && body.start <= d.span.start && d.span.end <= body.end)
            .map(|d| Symbol {
                name: d.name.clone(),
                kind: SymbolKind::Part,
                detail: String::new(),
                span: d.span,
                name_span: d.span,
                children: Vec::new(),
            })
            .collect()
    };
    let symbol = |name: &crate::lang::ast::Name, kind, detail: String, span, children| Symbol {
        name: name.text.clone(),
        kind,
        detail,
        span,
        name_span: name.span,
        children,
    };
    program
        .iter()
        .filter_map(|stmt| {
            Some(match &stmt.kind {
                StmtKind::Alphabet { name, symbols } => {
                    let shown: Vec<String> = symbols.iter().map(|(c, _)| c.to_string()).collect();
                    symbol(name, SymbolKind::Alphabet, format!("{{{}}}", shown.join(", ")), stmt.span, Vec::new())
                }
                StmtKind::MachineDef { name, machine } => symbol(
                    name,
                    SymbolKind::Machine,
                    format!("{} over {}", machine.kind.keyword(), machine.alphabet.text),
                    stmt.span,
                    parts(machine.body, DefKind::State),
                ),
                StmtKind::GrammarDef { name, grammar } => symbol(
                    name,
                    SymbolKind::Grammar,
                    format!("grammar over {}", grammar.alphabet.text),
                    stmt.span,
                    parts(grammar.body, DefKind::GrammarVariable),
                ),
                StmtKind::Fn(f) => {
                    let params: Vec<String> = f.params.iter().map(|(n, ty)| format!("{}: {ty}", n.text)).collect();
                    let detail = format!("({}) -> {}", params.join(", "), f.ret);
                    symbol(&f.name, SymbolKind::Function, detail, stmt.span, Vec::new())
                }
                StmtKind::Let { name, .. } => {
                    let ty = index.defs.iter().find(|d| d.span == name.span).and_then(|d| d.ty.clone());
                    let detail = ty.map(|t| t.to_string()).unwrap_or_default();
                    symbol(name, SymbolKind::Value, detail, stmt.span, Vec::new())
                }
                _ => return None,
            })
        })
        .collect()
}

/// `offset`, moved back to the start of the character it is in, and no
/// further than the end of `src`. Offsets from outside may be anywhere.
fn char_boundary(src: &str, offset: usize) -> usize {
    let mut offset = offset.min(src.len());
    while !src.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// The LSP position (line from 0, UTF-16 character) of a byte offset.
pub fn position_of(src: &str, offset: usize) -> (u32, u32) {
    let offset = char_boundary(src, offset);
    let line_start = src[..offset].rfind('\n').map_or(0, |i| i + 1);
    let line = src[..line_start].matches('\n').count();
    let character: usize = src[line_start..offset].chars().map(char::len_utf16).sum();
    (line as u32, character as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROGRAM: &str = "\
alphabet B = {0, 1}
dfa Even over B {
    start  even
    accept even
    even --0--> odd
    even --1--> even
    odd  --0--> even
    odd  --1--> odd
}
fn twice(n: Int) -> Int {
    return n * 2
}
let count = twice(3)
print Even accepts \"00\", count
";

    /// The offset of the `nth` (from 0) occurrence of `needle`, plus `extra`.
    fn at(src: &str, needle: &str, nth: usize, extra: usize) -> usize {
        src.match_indices(needle).nth(nth).unwrap().0 + extra
    }

    fn text(src: &str, span: Span) -> &str {
        &src[span.start..span.end]
    }

    #[test]
    fn a_good_program_has_no_problem() {
        assert_eq!(Analysis::new(PROGRAM).problems, vec![]);
    }

    #[test]
    fn reports_the_first_error_with_its_span_and_hint() {
        let src = "let x = 1\nprint y\n";
        let problem = Analysis::new(src).problems[0].clone();
        assert_eq!(problem.message, "NameError[E0200]: `y` is not defined");
        assert_eq!(text(src, problem.span), "y");
        let problem = Analysis::new("let x = 1;\nprint x").problems[0].clone();
        assert!(problem.message.starts_with("SyntaxError"));
        assert!(problem.hint.unwrap().contains("semicolons"));
    }

    #[test]
    fn hover_shows_types_signatures_and_states() {
        let a = Analysis::new(PROGRAM);
        let hover = |needle: &str, nth: usize| a.hover(at(PROGRAM, needle, nth, 1)).unwrap().1;
        assert_eq!(hover("count", 1), "```kstar\nlet count: Int\n```");
        assert_eq!(hover("twice", 1), "```kstar\nfn twice(n: Int) -> Int\n```");
        assert_eq!(hover("n * 2", 0), "```kstar\nn: Int (parameter)\n```");
        assert_eq!(hover("Even", 1), "```kstar\ndfa Even over B\n```");
        assert_eq!(hover("B", 1), "```kstar\nalphabet B = {0, 1}\n```");
        assert_eq!(hover("odd", 2), "state odd of DFA `Even`");
        assert_eq!(hover("even", 1), "state even of DFA `Even` (start, accepting)");
        // An expression: its type.
        assert_eq!(a.hover(at(PROGRAM, "\"00\"", 0, 1)).unwrap().1, "```kstar\n\"00\": Str\n```");
    }

    #[test]
    fn hover_on_a_builtin_shows_its_documentation() {
        let src = "let m = minimize(/0*1/)\n";
        let (span, text) = Analysis::new(src).hover(at(src, "minimize", 0, 2)).unwrap();
        assert_eq!(&src[span.start..span.end], "minimize");
        assert!(text.starts_with("```kstar\nminimize(lang) -> Dfa\n```"));
        assert!(text.contains("smallest DFA"));
    }

    #[test]
    fn goes_to_definitions() {
        let a = Analysis::new(PROGRAM);
        let def = |needle: &str, nth: usize| a.definition(at(PROGRAM, needle, nth, 0)).map(|s| s.start);
        assert_eq!(def("count", 1), Some(at(PROGRAM, "count", 0, 0)));
        assert_eq!(def("twice", 1), Some(at(PROGRAM, "twice", 0, 0)));
        assert_eq!(def("n * 2", 0), Some(at(PROGRAM, "n: Int", 0, 0)));
        assert_eq!(def("odd", 3), Some(at(PROGRAM, "odd", 0, 0)));
        assert_eq!(def("B", 1), Some(at(PROGRAM, "B", 0, 0)));
        assert_eq!(def("print", 0), None);
    }

    #[test]
    fn finds_all_references() {
        let a = Analysis::new(PROGRAM);
        let refs = |needle: &str, nth: usize, with_def: bool| {
            a.references(at(PROGRAM, needle, nth, 0), with_def).iter().map(|s| s.start).collect::<Vec<_>>()
        };
        // `even`: its definition (start) and five more mentions.
        assert_eq!(refs("even", 0, true).len(), 6);
        assert_eq!(refs("even", 0, false).len(), 5);
        assert_eq!(refs("twice", 0, true), vec![at(PROGRAM, "twice", 0, 0), at(PROGRAM, "twice", 1, 0)]);
        assert_eq!(refs("Even", 0, true).len(), 2);
    }

    #[test]
    fn local_names_stay_local() {
        let src = "let x = 1\nif true {\n    let inner = 2\n    print inner\n}\nprint x\n";
        let a = Analysis::new(src);
        let labels = |offset: usize| a.completions(offset).into_iter().map(|c| c.label).collect::<Vec<_>>();
        assert!(labels(at(src, "print inner", 0, 0)).contains(&"inner".to_string()));
        assert!(!labels(at(src, "print x", 0, 0)).contains(&"inner".to_string()));
        assert!(labels(at(src, "print x", 0, 0)).contains(&"x".to_string()));
    }

    #[test]
    fn completes_keywords_builtins_names_and_machine_parts() {
        let a = Analysis::new(PROGRAM);
        let everywhere = a.completions(PROGRAM.len());
        let find = |list: &[Completion], label: &str| list.iter().find(|c| c.label == label).cloned();
        assert_eq!(find(&everywhere, "minimize").unwrap().detail, "minimize(lang) -> Dfa");
        assert!(find(&everywhere, "minimize").unwrap().doc.is_some());
        assert_eq!(find(&everywhere, "count").unwrap().kind, CompletionKind::Variable);
        assert_eq!(find(&everywhere, "twice").unwrap().kind, CompletionKind::Function);
        assert_eq!(find(&everywhere, "trace").unwrap().kind, CompletionKind::Keyword);
        assert!(find(&everywhere, "odd").is_none(), "states belong inside their machine");
        // Inside the machine: its states and its alphabet's symbols.
        let inside = a.completions(at(PROGRAM, "odd  --1", 0, 0));
        assert_eq!(find(&inside, "odd").unwrap().kind, CompletionKind::State);
        assert_eq!(find(&inside, "1").unwrap().kind, CompletionKind::Symbol);
        assert_eq!(inside[0].kind, CompletionKind::State);
    }

    #[test]
    fn completion_survives_a_half_typed_line() {
        let good = Analysis::new(PROGRAM);
        let typing = format!("{PROGRAM}let y = twi");
        let broken = Analysis::with_fallback(&format!("{typing}("), Some(&good));
        assert!(broken.errors().next().is_some());
        // The statements before the broken line still parse and are checked,
        // so the names come from this version of the text.
        assert!(!broken.stale);
        let labels: Vec<String> = broken.completions(typing.len()).into_iter().map(|c| c.label).collect();
        assert!(labels.contains(&"twice".to_string()));
        assert!(labels.contains(&"count".to_string()));
    }

    #[test]
    fn signature_help_tracks_the_argument() {
        let src = "let u = union(/0/, minimize({\"a\", \"b\"}), \nlet d = twice(";
        let a = Analysis::with_fallback(src, Some(&Analysis::new(PROGRAM)));
        let help = a.signature_help(at(src, "/0/", 0, 1)).unwrap();
        assert_eq!((help.label.as_str(), help.active), ("union(a, b) -> Dfa", 0));
        assert_eq!(help.params, ["a", "b"]);
        // Inside the set: still the first argument of minimize.
        let help = a.signature_help(at(src, "\"b\"", 0, 0)).unwrap();
        assert_eq!((help.label.as_str(), help.active), ("minimize(lang) -> Dfa", 0));
        // Just after `minimize(...)`, the second argument of union.
        let help = a.signature_help(at(src, "), ", 0, 1)).unwrap();
        assert_eq!((help.label.as_str(), help.active), ("union(a, b) -> Dfa", 1));
        // A user function, from the last good version of the file.
        let help = a.signature_help(src.len()).unwrap();
        assert_eq!((help.label.as_str(), help.params.clone()), ("twice(n: Int) -> Int", vec!["n: Int".to_string()]));
        assert_eq!(a.signature_help(0), None);
    }

    #[test]
    fn positions_count_utf16_units() {
        let src = "let a = 1\nprint \"é😀\", a\n";
        let offset = at(src, ", a", 0, 2);
        // `print "` is 7 units, é is 1, 😀 is 2, then `", ` is 3: 13 in all.
        assert_eq!(position_of(src, offset), (1, 13));
        assert_eq!(offset_at(src, 1, 13), offset);
        assert_eq!(offset_at(src, 0, 99), at(src, "\n", 0, 0));
        assert_eq!(offset_at(src, 9, 0), src.len());
    }

    // Regression: offsets inside a character, or just after an emoji, made
    // hover, signature help and positions panic.
    #[test]
    fn offsets_inside_characters_are_safe() {
        let src = "let x = \"\u{1F600}\"\nprint len(\u{1F600}x)";
        let a = Analysis::new(src);
        for offset in 0..=src.len() + 2 {
            let _ = a.hover(offset);
            let _ = a.signature_help(offset);
            let _ = a.completions(offset);
            let _ = position_of(src, offset);
        }
    }

    #[test]
    fn outline_lists_definitions_with_their_parts() {
        let a = Analysis::new(PROGRAM);
        let names: Vec<(&str, SymbolKind)> = a.outline.iter().map(|s| (s.name.as_str(), s.kind)).collect();
        assert_eq!(
            names,
            [("B", SymbolKind::Alphabet), ("Even", SymbolKind::Machine), ("twice", SymbolKind::Function), ("count", SymbolKind::Value)]
        );
        let even = &a.outline[1];
        assert_eq!(even.detail, "dfa over B");
        let states: Vec<&str> = even.children.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(states, ["even", "odd"]);
        assert!(text(PROGRAM, even.span).starts_with("dfa Even over B {") && text(PROGRAM, even.span).ends_with('}'));
        assert_eq!(text(PROGRAM, even.name_span), "Even");
        assert_eq!(a.outline[2].detail, "(n: Int) -> Int");
        assert_eq!(a.outline[3].detail, "Int");
    }

    #[test]
    fn formatting() {
        assert_eq!(Analysis::new(PROGRAM).formatted(), None, "already formatted");
        assert_eq!(Analysis::new("let x=1\nprint   x\n").formatted().as_deref(), Some("let x = 1\nprint x\n"));
        assert_eq!(Analysis::new("let x = (\n").formatted(), None, "does not parse");
    }

    // ---- imports ----

    const LIB: &str = "alphabet B = {0, 1}\nlet Size = 2\nfn double(n: Int) -> Int {\n    return n * 2\n}\n";

    /// `main` analysed as `dir/main.kstar`, next to `lib.kstar` holding `lib`.
    fn with_lib(main: &str, lib: &'static str) -> Analysis {
        let read: Reader = Box::new(move |path| match path.to_string_lossy().replace('\\', "/").as_str() {
            "dir/lib.kstar" => Ok(lib.to_string()),
            other => Err(std::io::Error::new(std::io::ErrorKind::NotFound, other.to_string())),
        });
        Analysis::open(main, Some(Path::new("dir/main.kstar")), Some(read))
    }

    #[test]
    fn goes_to_definitions_in_other_files() {
        let main = "import \"lib.kstar\"\nfrom \"lib.kstar\" import double as twice\nprint lib.Size, twice(lib.double(1))\n";
        let a = with_lib(main, LIB);
        assert_eq!(a.problems, vec![]);
        let def = a.definition(at(main, "Size", 0, 1)).unwrap();
        assert_ne!(def.file, FileId::MAIN);
        assert_eq!(text(a.file_text(def.file), def), "Size");
        assert_eq!(a.file_path(def.file), Some(Path::new("dir/lib.kstar")));
        // A renamed function goes to where its module defines it.
        let def = a.definition(at(main, "twice(", 0, 1)).unwrap();
        assert_eq!(text(a.file_text(def.file), def), "double");
        // The module's name goes to the import.
        let def = a.definition(at(main, "lib.Size", 0, 1)).unwrap();
        assert_eq!(def.file, FileId::MAIN);
        assert_eq!(text(main, def), "\"lib.kstar\"");
    }

    #[test]
    fn hover_and_completion_after_a_module_name() {
        let main = "import \"lib.kstar\"\nprint lib.double(lib.Size)\nprint lib.\n";
        let a = with_lib(main, LIB);
        assert_eq!(a.hover(at(main, "double", 0, 1)).unwrap().1, "```kstar\nfn double(n: Int) -> Int\n```");
        assert_eq!(a.hover(at(main, "lib.d", 0, 1)).unwrap().1, "```kstar\nimport \"lib.kstar\"\n```");
        let labels: Vec<String> = a.completions(main.len() - 1).into_iter().map(|c| c.label).collect();
        assert_eq!(labels, ["B", "Size", "double"]);
        let kinds: Vec<CompletionKind> = a.completions(main.len() - 1).into_iter().map(|c| c.kind).collect();
        assert_eq!(kinds[2], CompletionKind::Function);
        // Elsewhere, the module's names are not offered on their own.
        let everywhere: Vec<String> = a.completions(at(main, "print lib.d", 0, 0)).into_iter().map(|c| c.label).collect();
        assert!(everywhere.contains(&"lib".to_string()) && !everywhere.contains(&"Size".to_string()));
    }

    #[test]
    fn signature_help_for_imported_functions() {
        let main = "import \"lib.kstar\"\nfrom \"lib.kstar\" import double as twice\nprint lib.double(1), twice(2)\n";
        let a = with_lib(main, LIB);
        let help = a.signature_help(at(main, "(1", 0, 1)).unwrap();
        assert_eq!(help.label, "double(n: Int) -> Int");
        let help = a.signature_help(at(main, "(2", 0, 1)).unwrap();
        assert_eq!(help.label, "twice(n: Int) -> Int");
    }

    #[test]
    fn errors_in_an_imported_file_are_shown_at_the_import() {
        let main = "import \"lib.kstar\"\nprint lib.Size\n";
        let a = with_lib(main, "let Size = 1 + true\n");
        assert_eq!(a.problems.len(), 1, "{:?}", a.problems);
        let problem = &a.problems[0];
        assert!(problem.message.starts_with("in lib.kstar, line 1: TypeError[E0302]"), "{}", problem.message);
        assert_eq!(text(main, problem.span), "\"lib.kstar\"");
        // An unused import is a warning in the file itself.
        let a = with_lib("import \"lib.kstar\"\n", LIB);
        assert_eq!(a.problems[0].code, "W0202");
    }
}

//! The formatter behind `kstar fmt`: rewrites a program in one canonical style.
//!
//! The style, in short:
//!
//! * four spaces per level of `{ ... }`, one statement per line;
//! * single spaces around operators (`a + b`, `x accepts w`), none around `..`;
//! * inside a machine, `start` and `accept` line up, and the transitions are
//!   padded so that every `--` and every `-->` sits in the same column;
//! * comments stay where they were, and runs of blank lines shrink to one.
//!
//! How it works: the program is parsed as usual, and the syntax tree is
//! printed back out. The tree does not hold everything the source did, so a
//! few things come from elsewhere:
//!
//! * comments, which the parser never sees, come from `lex_with_comments`;
//! * numbers, strings and regexes are copied from the source text exactly;
//! * blank lines are found by comparing the line numbers of neighbours;
//! * the positions of `{`, `}` and `else` are looked up in the token list.
//!
//! Formatting must never change what a program means. As a safety net,
//! `format` lexes its own output and checks that it has exactly the same
//! tokens and comments as the input; if not, it reports a bug instead of
//! returning a changed program.

use super::ast::*;
use super::error::{Diag, ErrorKind};
use super::lexer::{lex_with_comments, Comment};
use super::parser::Parser;
use super::token::{Span, Tok, Token};

const INDENT: &str = "    ";

/// Formats a whole program. `src` must use `\n` line endings.
pub fn format(src: &str) -> Result<String, Diag> {
    let (tokens, comments) = lex_with_comments(src)?;
    let program = Parser::new(tokens.clone(), src, false).program()?;

    let mut f = Formatter {
        src,
        tokens: &tokens,
        comments: &comments,
        next_comment: 0,
        out: Vec::new(),
        cur: None,
        depth: 0,
        last_line: 0,
        block_start: true,
        trailing: Vec::new(),
        line_starts: std::iter::once(0)
            .chain(src.match_indices('\n').map(|(i, _)| i + 1))
            .collect(),
    };
    for stmt in &program {
        f.statement(stmt);
    }
    f.flush_comments(usize::MAX);
    f.align_trailing_comments();

    let mut text = f.out.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    same_program(src, &text)?;
    Ok(text)
}

/// The safety net: are `before` and `after` the same program, token for
/// token and comment for comment? A comma just before a closing bracket is
/// ignored, since the formatter drops those (`{0, 1,}` becomes `{0, 1}`).
fn same_program(before: &str, after: &str) -> Result<(), Diag> {
    let essence = |src: &str| -> Option<(Vec<Tok>, Vec<String>)> {
        let (tokens, comments) = lex_with_comments(src).ok()?;
        let mut toks: Vec<Tok> = Vec::new();
        for token in tokens {
            if matches!(token.tok, Tok::RBrace | Tok::RParen | Tok::RBracket) && toks.last() == Some(&Tok::Comma) {
                toks.pop();
            }
            toks.push(token.tok);
        }
        Some((toks, comments.into_iter().map(|c| c.text).collect()))
    };
    if essence(before) == essence(after) {
        return Ok(());
    }
    Err(Diag::new(
        ErrorKind::Syntax,
        Span { start: 0, end: 0, line: 1 },
        "formatting would change this program, so it was left alone",
    )
    .code(super::codes::FORMAT_UNSAFE)
    .hint("this is a bug in `kstar fmt`; please report it with the file that caused it"))
}

struct Formatter<'a> {
    src: &'a str,
    tokens: &'a [Token],
    comments: &'a [Comment],
    /// The first comment not written out yet.
    next_comment: usize,
    /// Finished output lines.
    out: Vec<String>,
    /// The output line being built, if any.
    cur: Option<String>,
    /// How many `{ ... }` we are inside.
    depth: usize,
    /// The source line of the last thing written (0 before anything).
    last_line: usize,
    /// True right after an opening `{`, where blank lines are dropped.
    block_start: bool,
    /// Byte offset where each source line starts, for `line_of`.
    line_starts: Vec<usize>,
    /// Output lines that end in a comment: (line index, where the code ends).
    trailing: Vec<(usize, usize)>,
}

impl Formatter<'_> {
    // ---- lines, blank lines and comments ----

    /// The source line (from 1) that byte `offset` is on.
    fn line_of(&self, offset: usize) -> usize {
        self.line_starts.partition_point(|&start| start <= offset)
    }

    fn indent(&self) -> String {
        INDENT.repeat(self.depth)
    }

    /// Keeps one blank line before something on source line `line` if the
    /// source had one or more there.
    fn blank_line_before(&mut self, line: usize) {
        if !self.block_start && self.last_line != 0 && line > self.last_line + 1 {
            self.out.push(String::new());
        }
    }

    /// Writes every comment that starts before `offset`. A comment on the
    /// same source line as the code just written stays at the end of that
    /// line; any other comment gets a line of its own.
    fn flush_comments(&mut self, offset: usize) {
        while let Some(comment) = self.comments.get(self.next_comment).filter(|c| c.span.start < offset) {
            self.next_comment += 1;
            let last_index = self.out.len().wrapping_sub(1);
            match self.out.last_mut() {
                Some(last) if comment.span.line == self.last_line && !last.is_empty() => {
                    self.trailing.push((last_index, last.len()));
                    last.push_str("  ");
                    last.push_str(&comment.text);
                }
                _ => {
                    self.blank_line_before(comment.span.line);
                    let line = format!("{}{}", self.indent(), comment.text);
                    self.out.push(line);
                    self.block_start = false;
                }
            }
            self.last_line = comment.span.line;
        }
    }

    /// Lines up the comments at the ends of neighbouring lines, the way
    /// people often do by hand:
    ///
    /// ```text
    /// test div_by(3) accepts "110"          # 6
    /// test not (div_by(3) accepts "111")    # 7
    /// ```
    ///
    /// becomes the same with exactly two spaces before the longest line's `#`.
    fn align_trailing_comments(&mut self) {
        for run in self.trailing.chunk_by(|a, b| b.0 == a.0 + 1) {
            let width = run.iter().map(|&(line, code)| self.out[line][..code].chars().count()).max();
            for &(line, code) in run {
                let comment = self.out[line][code..].trim_start().to_string();
                let code_text = self.out[line][..code].to_string();
                let gap = width.unwrap_or(0) - code_text.chars().count() + 2;
                self.out[line] = format!("{code_text}{}{comment}", " ".repeat(gap));
            }
        }
    }

    /// Begins a new output line for the source text starting at `offset`.
    fn start_line(&mut self, offset: usize) {
        self.flush_comments(offset);
        self.blank_line_before(self.line_of(offset));
        self.block_start = false;
        self.cur = Some(self.indent());
    }

    /// Begins the line holding a closing `}` at `close`: never after a
    /// blank line, and only after the comments that come before it.
    fn start_close_line(&mut self, close: usize) {
        self.flush_comments(close);
        self.cur = Some(self.indent());
    }

    fn push(&mut self, text: &str) {
        self.cur.get_or_insert_with(String::new).push_str(text);
    }

    /// Finishes the current line; `end` is where its source text ended.
    fn end_line(&mut self, end: usize) {
        let line = self.cur.take().unwrap_or_default();
        self.out.push(line.trim_end().to_string());
        self.last_line = self.line_of(end.saturating_sub(1));
    }

    // ---- finding tokens the syntax tree does not record ----

    /// The first token of kind `tok` starting at or after `offset`.
    fn find(&self, tok: Tok, offset: usize) -> Span {
        let from = self.tokens.partition_point(|t| t.span.start < offset);
        self.tokens[from..]
            .iter()
            .find(|t| t.tok == tok)
            .map(|t| t.span)
            .expect("the parser accepted this program, so the token is there")
    }

    /// The token right after the one at `offset`.
    fn token_after(&self, offset: usize) -> &Token {
        let at = self.tokens.partition_point(|t| t.span.start <= offset);
        &self.tokens[at.min(self.tokens.len() - 1)]
    }

    fn text(&self, span: Span) -> &str {
        &self.src[span.start..span.end]
    }

    // ---- statements ----

    fn statement(&mut self, stmt: &Stmt) {
        self.start_line(stmt.span.start);
        match &stmt.kind {
            StmtKind::Alphabet { name, symbols } => {
                let symbols: Vec<String> = symbols.iter().map(|(c, _)| c.to_string()).collect();
                self.push(&format!("alphabet {} = {{{}}}", name.text, symbols.join(", ")));
            }
            StmtKind::MachineDef { name, machine } => {
                self.push(&format!("{} {} ", machine.kind.keyword(), name.text));
                let close = self.machine_body(machine);
                return self.end_line(close.end);
            }
            StmtKind::GrammarDef { name, grammar } => {
                self.push(&format!("grammar {} ", name.text));
                let close = self.grammar_body(grammar);
                return self.end_line(close.end);
            }
            StmtKind::Let { name, value } => {
                self.push(&format!("let {} = ", name.text));
                self.expr(value);
            }
            StmtKind::Fn(f) => {
                let params: Vec<String> =
                    f.params.iter().map(|(name, ty)| format!("{}: {ty}", name.text)).collect();
                self.push(&format!("fn {}({}) -> {} ", f.name.text, params.join(", "), f.ret));
                let close = self.block(f.name.span.end, &f.body);
                return self.end_line(close.end);
            }
            StmtKind::Return(value) => {
                self.push("return ");
                self.expr(value);
            }
            StmtKind::If { .. } => return self.if_statement(stmt),
            StmtKind::For { var, items, body } => {
                self.push(&format!("for {} in ", var.text));
                self.expr(items);
                self.push(" ");
                let close = self.block(items.span.end, body);
                return self.end_line(close.end);
            }
            StmtKind::Print(values) => {
                self.push("print ");
                self.list(values);
            }
            StmtKind::Test(cond) => {
                self.push("test ");
                self.expr(cond);
            }
            StmtKind::Draw(value) => {
                self.push("draw ");
                self.expr(value);
            }
            StmtKind::Trace { machine, word } => {
                self.push("trace ");
                self.expr(machine);
                self.push(" on ");
                self.expr(word);
            }
        }
        self.end_line(stmt.span.end);
    }

    /// `if ... { } else if ... { } else { }`, written onto the current line.
    fn if_statement(&mut self, stmt: &Stmt) {
        let StmtKind::If { cond, then, otherwise } = &stmt.kind else { unreachable!() };
        self.push("if ");
        self.expr(cond);
        self.push(" ");
        let close = self.block(cond.span.end, then);
        let Some(otherwise) = otherwise else { return self.end_line(close.end) };
        self.push(" else ");
        // `else if` and `else { if ... }` parse to the same tree; the token
        // after `else` tells them apart.
        let else_token = self.token_after(close.start).span;
        match (otherwise.as_slice(), &self.token_after(else_token.start).tok) {
            ([chained], Tok::If) => self.if_statement(chained),
            _ => {
                let close = self.block(else_token.end, otherwise);
                self.end_line(close.end);
            }
        }
    }

    /// Writes `{`, the statements one level deeper, and `}`, leaving the
    /// line with the `}` open so the caller can continue it (`} else {`).
    /// `open_after` is where to start looking for the `{`. Returns the span
    /// of the `}`.
    fn block(&mut self, open_after: usize, stmts: &[Stmt]) -> Span {
        let open = self.find(Tok::LBrace, open_after);
        let close = self.find(Tok::RBrace, stmts.last().map_or(open.end, |s| s.span.end));
        self.braces(open, close, |f| {
            for stmt in stmts {
                f.statement(stmt);
            }
        });
        close
    }

    /// The shared part of every `{ ... }`: an empty pair stays `{}` on one
    /// line; otherwise the contents go on their own lines, one level deeper.
    fn braces(&mut self, open: Span, close: Span, contents: impl FnOnce(&mut Self)) {
        self.push("{");
        let comment_inside = self.comments.get(self.next_comment).is_some_and(|c| c.span.start < close.start);
        let nothing_inside = self.token_after(open.start).span == close && !comment_inside;
        if nothing_inside {
            self.push("}");
            return;
        }
        self.end_line(open.end);
        self.depth += 1;
        self.block_start = true;
        contents(self);
        self.flush_comments(close.start);
        self.depth -= 1;
        self.block_start = false;
        self.start_close_line(close.start);
        self.push("}");
    }

    // ---- machines ----

    /// `over B { ... }`. Returns the span of the closing `}`.
    fn machine_body(&mut self, m: &MachineExpr) -> Span {
        self.push(&format!("over {} ", m.alphabet.text));
        self.machine_items(m.alphabet.span.end, &m.items)
    }

    fn machine_items(&mut self, open_after: usize, items: &[MachineItem]) -> Span {
        let open = self.find(Tok::LBrace, open_after);
        // Where the last item ends; a nested `for` ends at its own `}`.
        let mut end = open.end;
        let mut ends = Vec::new();
        for item in items {
            end = self.item_end(item, end);
            ends.push(end);
        }
        let close = self.find(Tok::RBrace, end);

        // Line up the transitions of this block: pad the source states so
        // every `--` is in one column, and the labels so every `-->` is.
        let transitions = items.iter().filter_map(|item| match item {
            MachineItem::Transition { from, action, .. } => Some((self.state(from), action_text(action))),
            _ => None,
        });
        let (mut from_width, mut label_width) = (0, 0);
        for (from, labels) in transitions {
            from_width = from_width.max(from.chars().count());
            label_width = label_width.max(labels.chars().count());
        }

        self.braces(open, close, |f| {
            for (item, &end) in items.iter().zip(&ends) {
                f.start_line(item_start(item));
                match item {
                    MachineItem::Start(state) => {
                        let state = f.state(state);
                        f.push(&format!("start  {state}"));
                    }
                    MachineItem::Accept(states) => {
                        let states: Vec<String> = states.iter().map(|s| f.state(s)).collect();
                        f.push(&format!("accept {}", states.join(", ")));
                    }
                    MachineItem::Reject(states) => {
                        let states: Vec<String> = states.iter().map(|s| f.state(s)).collect();
                        f.push(&format!("reject {}", states.join(", ")));
                    }
                    MachineItem::Transition { from, action, to, .. } => {
                        let (from, to) = (f.state(from), f.state(to));
                        f.push(&format!(
                            "{} --{}--> {to}",
                            pad(&from, from_width),
                            pad(&action_text(action), label_width)
                        ));
                    }
                    MachineItem::For { var, items: range, body } => {
                        f.push(&format!("for {} in ", var.text));
                        f.expr(range);
                        f.push(" ");
                        f.machine_items(range.span.end, body);
                    }
                }
                f.end_line(end);
            }
        });
        close
    }

    /// `over B { ... }` for a grammar: one rule per line, with the `->`
    /// of every rule in the same column. Returns the span of the `}`.
    fn grammar_body(&mut self, g: &GrammarExpr) -> Span {
        self.push(&format!("over {} ", g.alphabet.text));
        let open = self.find(Tok::LBrace, g.alphabet.span.end);
        let close = self.find(Tok::RBrace, g.rules.last().map_or(open.end, |r| r.span.end));
        let width = g.rules.iter().map(|r| r.head.text.chars().count()).max().unwrap_or(0);
        self.braces(open, close, |f| {
            for rule in &g.rules {
                f.start_line(rule.span.start);
                let alternatives: Vec<String> = rule.alternatives.iter().map(|alt| alternative_text(alt)).collect();
                f.push(&format!("{} -> {}", pad(&rule.head.text, width), alternatives.join(" | ")));
                f.end_line(rule.span.end);
            }
        });
        close
    }

    /// Where `item` ends in the source. `after` is where the previous item
    /// ended, used to find the `}` of a nested `for`.
    fn item_end(&self, item: &MachineItem, after: usize) -> usize {
        match item {
            MachineItem::Start(state) => state.span.end,
            MachineItem::Accept(states) | MachineItem::Reject(states) => states.last().map_or(after, |s| s.span.end),
            MachineItem::Transition { span, .. } => span.end,
            MachineItem::For { items, body, .. } => {
                let open = self.find(Tok::LBrace, items.span.end);
                let mut end = open.end;
                for inner in body {
                    end = self.item_end(inner, end);
                }
                self.find(Tok::RBrace, end).end
            }
        }
    }

    /// `q0`, or `q[i + 1]`.
    fn state(&mut self, state: &StateRef) -> String {
        match &state.index {
            None => state.name.clone(),
            Some(index) => format!("{}[{}]", state.name, self.expr_text(index)),
        }
    }

    // ---- expressions ----

    /// An expression as a string, for places that need one (state indices).
    fn expr_text(&mut self, expr: &Expr) -> String {
        let saved = self.cur.replace(String::new());
        self.expr(expr);
        std::mem::replace(&mut self.cur, saved).unwrap_or_default()
    }

    fn list(&mut self, exprs: &[Expr]) {
        for (i, expr) in exprs.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            self.expr(expr);
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            // Literals are copied exactly, so `"a\"b"` and `/a | b/` keep
            // the way they were written.
            ExprKind::Int(_) | ExprKind::Str(_) | ExprKind::Bool(_) | ExprKind::Regex(_) => {
                let text = self.text(expr.span).to_string();
                self.push(&text);
            }
            ExprKind::Name(name) => self.push(name),
            ExprKind::Paren(inner) => {
                self.push("(");
                self.expr(inner);
                self.push(")");
            }
            ExprKind::Unary(op, inner) => {
                self.push(match op {
                    UnOp::Neg => "-",
                    UnOp::Not => "not ",
                });
                self.expr(inner);
            }
            ExprKind::Binary(op, left, right) => {
                self.expr(left);
                match op {
                    BinOp::Range => self.push(".."),
                    _ => self.push(&format!(" {} ", op.symbol())),
                }
                self.expr(right);
            }
            ExprKind::Call { name, args } => {
                self.push(&name.text);
                self.push("(");
                self.list(args);
                self.push(")");
            }
            ExprKind::Set(items) => {
                self.push("{");
                self.list(items);
                self.push("}");
            }
            ExprKind::Machine(m) => {
                self.push(m.kind.keyword());
                self.push(" ");
                self.machine_body(m);
            }
            ExprKind::Grammar(g) => {
                self.push("grammar ");
                self.grammar_body(g);
            }
        }
    }
}

/// Where an item starts, for blank lines and comments before it. (`start`
/// and `accept` have no span of their own; their first state is close enough,
/// since nothing can come between the keyword and the state.)
fn item_start(item: &MachineItem) -> usize {
    match item {
        MachineItem::Start(state) => state.span.start,
        MachineItem::Accept(states) | MachineItem::Reject(states) => states.first().map_or(0, |s| s.span.start),
        MachineItem::Transition { span, .. } => span.start,
        MachineItem::For { var, .. } => var.span.start,
    }
}

/// What goes between `--` and `-->`: `0,1`; in a PDA `a, Z/AZ`; in a TM `0/1,R`.
fn action_text(action: &Action) -> String {
    let label = |l: &Label| l.symbol.map_or("eps".to_string(), |c| c.to_string());
    match action {
        Action::Symbols(labels) => labels.iter().map(label).collect::<Vec<_>>().join(","),
        Action::Stack { read, pop, push, .. } => {
            let push: String = if push.is_empty() { "eps".into() } else { push.iter().collect() };
            format!("{}, {}/{push}", label(read), label(pop))
        }
        Action::Tape { read, write, dir, .. } => format!("{}/{},{}", label(read), label(write), dir.letter()),
    }
}

/// One alternative of a grammar rule: `a S b`, or `eps` when empty.
fn alternative_text(symbols: &[GSym]) -> String {
    if symbols.is_empty() {
        return "eps".to_string();
    }
    let parts: Vec<String> = symbols
        .iter()
        .map(|s| match &s.kind {
            GSymKind::Variable(name) => name.clone(),
            GSymKind::Terminal(c) => c.to_string(),
        })
        .collect();
    parts.join(" ")
}

fn pad(text: &str, width: usize) -> String {
    format!("{text:width$}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(src: &str) -> String {
        format(src).unwrap_or_else(|d| panic!("{}", d.render("t.kstar", src)))
    }

    #[test]
    fn aligns_machine_blocks() {
        let src = "alphabet B={0,1}\ndfa M over B{\nstart a\naccept b\na--0-->a\na --1--> bb\nbb--0,1-->bb}\n";
        assert_eq!(
            fmt(src),
            "alphabet B = {0, 1}\n\
             dfa M over B {\n    start  a\n    accept b\n    a  --0  --> a\n    a  --1  --> bb\n    bb --0,1--> bb\n}\n"
        );
    }

    #[test]
    fn spaces_operators_and_keeps_parentheses() {
        assert_eq!(fmt("let x=(1+2)*-3%4\nprint x,0..x+1"), "let x = (1 + 2) * -3 % 4\nprint x, 0..x + 1\n");
        assert_eq!(fmt("test not(/a/ accepts \"a\\\"b\")"), "test not (/a/ accepts \"a\\\"b\")\n");
    }

    #[test]
    fn indents_blocks_and_chains_else_if() {
        let src = "fn f(n:Int)->Int{if n<1{return 0}else if n<2{return 1}else{\nreturn f(n-1)}}";
        assert_eq!(
            fmt(src),
            "fn f(n: Int) -> Int {\n    if n < 1 {\n        return 0\n    } else if n < 2 {\n        return 1\n    } else {\n        return f(n - 1)\n    }\n}\n"
        );
        // `else { if ... }` is not turned into `else if`.
        assert_eq!(
            fmt("if true {print 1} else {if false {print 2}}"),
            "if true {\n    print 1\n} else {\n    if false {\n        print 2\n    }\n}\n"
        );
    }

    #[test]
    fn keeps_comments_and_single_blank_lines() {
        let src = "# head\n\n\n\nlet x = 1   # one\n# before two\nlet y = 2\n\n\n# tail\n";
        assert_eq!(fmt(src), "# head\n\nlet x = 1  # one\n# before two\nlet y = 2\n\n# tail\n");
        let src = "for i in 0..2 {  # loop\n\n    # inside\n    print i\n\n}\n";
        assert_eq!(fmt(src), "for i in 0..2 {  # loop\n    # inside\n    print i\n}\n");
        let src = "print 1 # one\nprint 100 # hundred\n\nprint 1000 # alone\n";
        assert_eq!(fmt(src), "print 1    # one\nprint 100  # hundred\n\nprint 1000  # alone\n");
    }

    #[test]
    fn machine_values_and_loops() {
        let src = "fn d(n: Int) -> Dfa {\nreturn dfa over B {\nstart q[0]\naccept q[0]\nfor i in 0..n {\nq[i] --0--> q[(2*i)%n]\nq[i]--1-->q[(2*i+1)%n]\n}\n}\n}\n";
        assert_eq!(
            fmt(src),
            "fn d(n: Int) -> Dfa {\n    return dfa over B {\n        start  q[0]\n        accept q[0]\n        for i in 0..n {\n            q[i] --0--> q[(2 * i) % n]\n            q[i] --1--> q[(2 * i + 1) % n]\n        }\n    }\n}\n"
        );
    }

    #[test]
    fn empty_blocks_and_files() {
        assert_eq!(fmt(""), "");
        assert_eq!(fmt("if true {}"), "if true {}\n");
        assert_eq!(fmt("let s = {}\nprint {1,2,}"), "let s = {}\nprint {1, 2}\n");
    }

    #[test]
    fn is_idempotent() {
        let src = "alphabet B = {0, 1}\nnfa N over B {start a # s\n accept c\n a --0,1--> a\n a --0--> b\n\n\n b --1--> c }\nprint N";
        let once = fmt(src);
        assert_eq!(fmt(&once), once);
    }

    #[test]
    fn reports_syntax_errors() {
        assert!(format("let = 1").is_err());
    }
}

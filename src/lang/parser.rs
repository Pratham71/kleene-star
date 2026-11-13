//! The parser: turns tokens into a syntax tree.
//!
//! This is a recursive-descent parser: there is one method per grammar rule,
//! and each method calls the methods for the rules it is made of. The grammar
//! is written out in `docs/reference.md`; the comment above each method quotes
//! the rule it implements.

use super::ast::*;
use super::codes;
use super::error::{Diag, ErrorKind};
use super::token::{Span, Tok, Token};
use crate::automata::regex;
use std::rc::Rc;

pub struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    src: &'a str,
    /// In the REPL a bare expression is allowed and means "print this".
    repl: bool,
    /// How many `{ ... }` blocks we are inside; functions must be at depth 0.
    depth: usize,
    /// Errors inside a machine that were recovered from: the parser skipped
    /// the bad line and went on with the next one.
    recovered: Vec<Diag>,
    /// How deeply the expression or block being parsed is nested.
    nesting: usize,
}

type Parse<T> = Result<T, Diag>;

/// How deeply expressions and blocks may nest. Every later stage walks the
/// syntax tree recursively, so without a limit a pathological program (a
/// thousand opening parentheses, or a sum of ten thousand terms) would
/// overflow the stack and crash instead of reporting an error. A chain such
/// as `a + b + c` counts one level per operator, since it nests too.
pub const MAX_NESTING: usize = 200;

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token>, src: &'a str, repl: bool) -> Parser<'a> {
        Parser { tokens, pos: 0, src, repl, depth: 0, recovered: Vec::new(), nesting: 0 }
    }

    // program := statement*
    //
    // Stops at the first error. `program_recovering` carries on instead.
    pub fn program(&mut self) -> Parse<Vec<Stmt>> {
        let (stmts, mut errors) = self.program_recovering();
        if errors.is_empty() {
            Ok(stmts)
        } else {
            Err(errors.swap_remove(0))
        }
    }

    /// Parses the whole program, carrying on after a syntax error so that
    /// one run reports all of them. Returns the statements that parsed and
    /// the errors, in order.
    ///
    /// After an error the parser skips ahead to the next line that starts a
    /// statement, and starts again from there. Only the first error in each
    /// statement is reported: anything later in the same statement is likely
    /// to be a consequence of the first.
    pub fn program_recovering(&mut self) -> (Vec<Stmt>, Vec<Diag>) {
        let mut stmts = Vec::new();
        let mut errors = Vec::new();
        while *self.peek() != Tok::Eof {
            let start = self.pos;
            match self.statement() {
                Ok(stmt) => stmts.push(stmt),
                Err(error) => {
                    errors.push(error);
                    self.depth = 0;
                    self.nesting = 0;
                    self.skip_to_next_statement(start);
                }
            }
        }
        errors.append(&mut self.recovered);
        errors.sort_by_key(|e| e.span.start);
        (stmts, errors)
    }

    /// After a statement that began at token `start` failed, moves to the
    /// first token that plausibly begins the next statement: one that starts
    /// a line, is a keyword that begins statements, and is not inside a
    /// `{ ... }` that the failed statement opened. If a brace was left open,
    /// a statement written at the very start of a line (no indent) also
    /// counts, so one missing `}` does not hide the rest of the file.
    fn skip_to_next_statement(&mut self, start: usize) {
        let mut depth: i64 = 0;
        for i in start..self.tokens.len() {
            let token = &self.tokens[i];
            if i > start && self.starts_line(i) {
                let begins = matches!(
                    token.tok,
                    Tok::Alphabet | Tok::Dfa | Tok::Nfa | Tok::Pda | Tok::Tm | Tok::Grammar | Tok::Let | Tok::Fn
                        | Tok::Print | Tok::Test | Tok::Draw | Tok::Trace | Tok::If | Tok::For | Tok::Return
                        | Tok::Import | Tok::From
                );
                let unindented = self.column(i) == 0;
                if i >= self.pos && i > start && begins && (depth <= 0 || unindented) || token.tok == Tok::Eof {
                    self.pos = i;
                    return;
                }
            }
            match token.tok {
                Tok::LBrace => depth += 1,
                Tok::RBrace => depth -= 1,
                Tok::Eof => {
                    self.pos = i;
                    return;
                }
                _ => {}
            }
        }
        self.pos = self.tokens.len() - 1;
    }

    /// Goes one level deeper, failing past `MAX_NESTING`.
    fn nest(&mut self) -> Parse<()> {
        self.nesting += 1;
        if self.nesting > MAX_NESTING {
            return Err(self
                .error(format!("this is nested too deeply (more than {MAX_NESTING} levels)"))
                .code(codes::TOO_DEEP)
                .hint("split it into smaller pieces with `let` or a function"));
        }
        Ok(())
    }

    /// Fails unless every token has been used, for callers that parse a
    /// single expression.
    pub fn finish(&self) -> Parse<()> {
        match self.peek() {
            Tok::Eof => Ok(()),
            other => Err(self.error(format!("expected the end of the expression, found {}", other.describe()))),
        }
    }

    /// True if token `i` is the first on its line.
    fn starts_line(&self, i: usize) -> bool {
        i == 0 || self.tokens[i - 1].span.line < self.tokens[i].span.line
    }

    /// How far into its line token `i` starts, in bytes.
    fn column(&self, i: usize) -> usize {
        let start = self.tokens[i].span.start.min(self.src.len());
        start - self.src[..start].rfind('\n').map_or(0, |n| n + 1)
    }

    // ---- token helpers ----

    fn peek(&self) -> &Tok {
        &self.tokens[self.pos].tok
    }

    fn peek_next(&self) -> &Tok {
        &self.tokens[(self.pos + 1).min(self.tokens.len() - 1)].tok
    }

    fn span(&self) -> Span {
        self.tokens[self.pos].span
    }

    /// The span of the token just consumed.
    fn prev(&self) -> Span {
        self.tokens[self.pos.saturating_sub(1)].span
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        token
    }

    fn eat(&mut self, tok: Tok) -> bool {
        if *self.peek() == tok {
            self.advance();
            true
        } else {
            false
        }
    }

    fn error(&self, message: impl Into<String>) -> Diag {
        Diag::new(ErrorKind::Syntax, self.span(), message)
    }

    fn expect(&mut self, tok: Tok, what: &str) -> Parse<Span> {
        if *self.peek() == tok {
            Ok(self.advance().span)
        } else {
            Err(self.error(format!("expected {what}, found {}", self.peek().describe())))
        }
    }

    fn name(&mut self, what: &str) -> Parse<Name> {
        match self.peek().clone() {
            Tok::Ident(text) => Ok(Name { text, span: self.advance().span }),
            other => {
                let mut err = self.error(format!("expected {what}, found {}", other.describe()));
                if Tok::keyword(self.text(self.span())).is_some() {
                    err = err.code(codes::KEYWORD_AS_NAME).hint(format!(
                        "`{}` is a keyword and cannot be used as a name",
                        self.text(self.span())
                    ));
                }
                Err(err)
            }
        }
    }

    fn text(&self, span: Span) -> &'a str {
        &self.src[span.start..span.end]
    }

    // ---- statements ----

    // statement := alphabet | machine-def | let | fn | return | if | for
    //            | print | test | draw | trace | import | from-import
    fn statement(&mut self) -> Parse<Stmt> {
        let start = self.span();
        let kind = match self.peek() {
            Tok::Alphabet => self.alphabet()?,
            Tok::Dfa | Tok::Nfa | Tok::Pda | Tok::Tm => {
                // machine-def := ('dfa' | 'nfa' | 'pda' | 'tm') NAME 'over' NAME '{' item* '}'
                let kind = machine_kind(&self.advance().tok);
                let word = kind.keyword();
                if *self.peek() == Tok::Over {
                    return Err(self.error(format!("a `{word}` statement needs a name")).code(codes::NEEDS_NAME).hint(
                        format!("write `{word} Name over B {{ ... }}`, or `let m = {word} over B {{ ... }}`"),
                    ));
                }
                let name = self.name("a name for the machine")?;
                let machine = self.machine_body(kind, start)?;
                StmtKind::MachineDef { name, machine }
            }
            Tok::Grammar => {
                // grammar-def := 'grammar' NAME grammar-body
                self.advance();
                if *self.peek() == Tok::Over {
                    return Err(self.error("a `grammar` statement needs a name").code(codes::NEEDS_NAME).hint(
                        "write `grammar G over B { ... }`, or `let g = grammar over B { ... }`",
                    ));
                }
                let name = self.name("a name for the grammar")?;
                let grammar = self.grammar_body(start)?;
                StmtKind::GrammarDef { name, grammar }
            }
            Tok::Let => {
                // let := 'let' NAME '=' expr
                self.advance();
                let name = self.name("a name after `let`")?;
                self.expect(Tok::Assign, "`=`")?;
                StmtKind::Let { name, value: self.expr()? }
            }
            Tok::Fn => self.function()?,
            Tok::Return => {
                self.advance();
                StmtKind::Return(self.expr()?)
            }
            Tok::If => self.if_statement()?,
            Tok::For => {
                // for := 'for' NAME 'in' expr block
                self.advance();
                let var = self.name("a loop variable")?;
                self.expect(Tok::In, "`in`")?;
                let items = self.expr()?;
                StmtKind::For { var, items, body: self.block()? }
            }
            Tok::Print => {
                // print := 'print' expr (',' expr)*
                self.advance();
                let mut values = vec![self.expr()?];
                while self.eat(Tok::Comma) {
                    values.push(self.expr()?);
                }
                StmtKind::Print(values)
            }
            Tok::Test => {
                self.advance();
                StmtKind::Test(self.expr()?)
            }
            Tok::Draw => {
                self.advance();
                StmtKind::Draw(self.expr()?)
            }
            Tok::Trace => self.trace()?,
            Tok::Import | Tok::From => self.import()?,
            Tok::Ident(_) if *self.peek_next() == Tok::Assign => {
                return Err(self.error("assignment needs `let`").code(codes::NEEDS_LET).hint(
                    "write `let x = ...`; values cannot be changed once defined, so pick a new name",
                ));
            }
            _ if self.repl => StmtKind::Print(vec![self.expr()?]),
            other => {
                return Err(self
                    .error(format!("expected a statement, found {}", other.describe()))
                    .hint("a statement starts with one of: alphabet, dfa, nfa, pda, tm, grammar, let, fn, return, if, for, print, test, draw, trace, import, from"));
            }
        };
        Ok(Stmt { kind, span: start.to(self.prev()) })
    }

    // alphabet := 'alphabet' NAME '=' '{' symbol (',' symbol)* '}'
    fn alphabet(&mut self) -> Parse<StmtKind> {
        self.advance();
        let name = self.name("a name for the alphabet")?;
        self.expect(Tok::Assign, "`=`")?;
        self.expect(Tok::LBrace, "`{`")?;
        let mut symbols = Vec::new();
        while *self.peek() != Tok::RBrace {
            let span = self.span();
            match self.label()?.symbol {
                Some(c) => symbols.push((c, span)),
                None => {
                    return Err(Diag::new(ErrorKind::Syntax, span, "`eps` is not a symbol")
                        .code(codes::BAD_EPS)
                        .hint("eps is the empty string; it cannot be in an alphabet"));
                }
            }
            if !self.eat(Tok::Comma) {
                break;
            }
        }
        self.expect(Tok::RBrace, "`,` or `}`")?;
        Ok(StmtKind::Alphabet { name, symbols })
    }

    // trace := 'trace' range 'on' expr
    //
    // The machine is parsed as a `range`, one level below comparisons, so
    // that a mistaken `trace M accepts "01"` stops at `accepts` and can be
    // reported clearly instead of being read as one Bool expression.
    fn trace(&mut self) -> Parse<StmtKind> {
        self.advance();
        let machine = self.range()?;
        if *self.peek() != Tok::On {
            return Err(self
                .error(format!(
                    "expected `on` after the machine to trace, found {}",
                    self.peek().describe()
                ))
                .hint("write `trace M on \"0110\"`"));
        }
        self.advance();
        Ok(StmtKind::Trace { machine, word: self.expr()? })
    }

    // import      := 'import' lib ('as' NAME)?
    // from-import := 'from' lib 'import' NAME ('as' NAME)? (',' NAME ('as' NAME)?)*
    //
    // Only the syntax is read here. Finding the file is the loader's job
    // (`module.rs`), and checking the names is the checker's.
    fn import(&mut self) -> Parse<StmtKind> {
        let keyword = self.advance();
        let word = if keyword.tok == Tok::Import { "import" } else { "from" };
        if self.depth > 0 {
            return Err(Diag::new(ErrorKind::Syntax, keyword.span, format!("`{word}` can only be used at the top level"))
                .code(codes::IMPORT_NOT_TOP)
                .hint("move the import to the top of the file; its names can then be used everywhere below it"));
        }
        let lib = self.lib()?;
        if keyword.tok == Tok::Import {
            // `import "x.kstar" import name`: probably meant `from`.
            if *self.peek() == Tok::Import && self.span().line == lib.span.line {
                return Err(self
                    .error("expected `as` or the end of the line, found `import`")
                    .hint(format!("to take names out of a module, write `from {} import name`", lib.shown())));
            }
            let alias = if self.eat(Tok::As) { Some(self.name("a name for the module after `as`")?) } else { None };
            // Without `as`, the module is named after its file, which must
            // then be usable as a name.
            let default = lib.default_name();
            if alias.is_none() && !is_name(&default) {
                let mut suggestion: String = default.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
                if !suggestion.starts_with(char::is_alphabetic) {
                    suggestion = format!("m_{suggestion}");
                }
                return Err(Diag::new(
                    ErrorKind::Syntax,
                    lib.span,
                    format!("this module needs a name: `{default}` cannot be used as one"),
                )
                .code(codes::BAD_IMPORT_PATH)
                .hint(format!("name it with `as`: `import {} as {suggestion}`", lib.shown())));
            }
            return Ok(StmtKind::Import { lib, alias, target: None });
        }
        if *self.peek() != Tok::Import {
            return Err(self
                .error(format!("expected `import` after the module, found {}", self.peek().describe()))
                .hint(format!("write `from {} import name, other`", lib.shown())));
        }
        self.advance();
        if *self.peek() == Tok::Star {
            return Err(self
                .error("`import *` is not allowed")
                .hint("list the names you want, so a reader can see where each one comes from: `from ... import a, b`"));
        }
        let mut names = Vec::new();
        loop {
            let name = self.name("a name to import")?;
            let alias = if self.eat(Tok::As) { Some(self.name("a new name after `as`")?) } else { None };
            names.push((name, alias));
            if !self.eat(Tok::Comma) {
                break;
            }
        }
        Ok(StmtKind::FromImport { lib, names, target: None })
    }

    // lib := STRING | 'std' '.' NAME
    fn lib(&mut self) -> Parse<Lib> {
        let span = self.span();
        match self.peek().clone() {
            Tok::Str(path) => {
                self.advance();
                let bad_path = |message: String, hint: &str| {
                    Diag::new(ErrorKind::Syntax, span, message).code(codes::BAD_IMPORT_PATH).hint(hint.to_string())
                };
                if !path.ends_with(".kstar") {
                    return Err(bad_path(
                        format!("an imported file must be a `.kstar` file, but this is `{path}`"),
                        "write the whole file name, such as `import \"utils.kstar\"`",
                    ));
                }
                if path.starts_with(['/', '\\']) || std::path::Path::new(&path).is_absolute() {
                    return Err(bad_path(
                        format!("`{path}` is an absolute path"),
                        "write the path relative to the file that imports it, so the files can be moved together",
                    ));
                }
                Ok(Lib { kind: LibKind::File(path), span })
            }
            Tok::Ident(word) if word == "std" => {
                let name = self.qualified_name("a module")?;
                let parts = name.parts();
                match &parts[..] {
                    [_, module] => Ok(Lib { kind: LibKind::Std(module.text.clone()), span: name.span }),
                    [_] => Err(Diag::new(ErrorKind::Syntax, name.span, "expected a standard-library module after `std`")
                        .hint("write `import std.math`, for example")),
                    _ => Err(Diag::new(ErrorKind::Syntax, parts[2].span, "a module is named by `std` and one more name")
                        .hint(format!("import the module, `import std.{}`, then use its names", parts[1].text))),
                }
            }
            other => Err(self
                .error(format!("expected a file name in quotes or a standard-library module, found {}", other.describe()))
                .hint("write `import \"utils.kstar\"`, or `import std.math`")),
        }
    }

    // qualified-name := NAME ('.' NAME)*        with no spaces around the dots
    //
    // `utils.EvenZeros` and `std.math.gcd` are read as one `Name` whose text
    // keeps the dots. The checker works out what each part means.
    fn qualified_name(&mut self, what: &str) -> Parse<Name> {
        let mut name = self.name(what)?;
        while *self.peek() == Tok::Dot {
            let dot = self.span();
            let next = self.tokens[(self.pos + 1).min(self.tokens.len() - 1)].clone();
            let Tok::Ident(part) = &next.tok else {
                self.advance();
                let mut err = self.error(format!("expected a name after `.`, found {}", next.tok.describe()));
                if Tok::keyword(self.text(next.span)).is_some() {
                    err = err.code(codes::KEYWORD_AS_NAME).hint(format!(
                        "`{}` is a keyword, so no module can define it",
                        self.text(next.span)
                    ));
                }
                return Err(err);
            };
            if dot.start != name.span.end || next.span.start != dot.end {
                return Err(Diag::new(ErrorKind::Syntax, dot, "a qualified name is written without spaces around `.`")
                    .hint(format!("write `{}.{part}`", name.text)));
            }
            self.advance();
            self.advance();
            name = Name { text: format!("{}.{part}", name.text), span: name.span.to(next.span) };
        }
        Ok(name)
    }

    // fn := 'fn' NAME '(' (NAME ':' type (',' NAME ':' type)*)? ')' '->' type block
    fn function(&mut self) -> Parse<StmtKind> {
        if self.depth > 0 {
            return Err(self
                .error("functions can only be defined at the top level")
                .code(codes::NESTED_FUNCTION)
                .hint("move this `fn` out of the block; a function can be called from anywhere in the file"));
        }
        self.advance();
        let name = self.name("a function name")?;
        self.expect(Tok::LParen, "`(`")?;
        let mut params = Vec::new();
        while *self.peek() != Tok::RParen {
            let param = self.name("a parameter name")?;
            if *self.peek() != Tok::Colon {
                return Err(self
                    .error(format!("parameter `{}` needs a type", param.text))
                    .hint(format!("write `{}: Int`, for example", param.text)));
            }
            self.advance();
            params.push((param, self.type_name()?));
            if !self.eat(Tok::Comma) {
                break;
            }
        }
        self.expect(Tok::RParen, "`,` or `)`")?;
        if *self.peek() != Tok::Arrow {
            return Err(self
                .error(format!("function `{}` needs a return type", name.text))
                .hint("write `-> Type` after the parameters; every function returns a value"));
        }
        self.advance();
        let ret = self.type_name()?;
        let body = self.block()?;
        Ok(StmtKind::Fn(Rc::new(FnDef { name, params, ret, body })))
    }

    // type := 'Int' | 'Bool' | 'Str' | 'Alphabet' | 'Dfa' | 'Nfa' | 'Pda' | 'Tm' | 'Regex' | 'Grammar'
    //       | 'Set' '[' type ']'
    fn type_name(&mut self) -> Parse<Type> {
        let name = self.name("a type").map_err(|e| {
            e.hint("types are: Int, Bool, Str, Alphabet, Dfa, Nfa, Pda, Tm, Regex, Grammar, Set[T]")
        })?;
        Ok(match name.text.as_str() {
            "Int" => Type::Int,
            "Bool" => Type::Bool,
            "Str" => Type::Str,
            "Alphabet" => Type::Alphabet,
            "Dfa" => Type::Dfa,
            "Nfa" => Type::Nfa,
            "Regex" => Type::Regex,
            "Grammar" => Type::Grammar,
            "Pda" => Type::Pda,
            "Tm" => Type::Tm,
            "Set" => {
                self.expect(Tok::LBracket, "`[` after `Set`")?;
                let inner = self.type_name()?;
                self.expect(Tok::RBracket, "`]`")?;
                Type::Set(Box::new(inner))
            }
            other => {
                let err = Diag::new(ErrorKind::Type, name.span, format!("unknown type `{other}`"))
                    .code(codes::UNKNOWN_TYPE);
                let known = ["Int", "Bool", "Str", "Alphabet", "Dfa", "Nfa", "Pda", "Tm", "Regex", "Grammar", "Set"];
                let lower = other.to_ascii_lowercase();
                if let Some(t) = known.iter().find(|t| t.to_ascii_lowercase() == lower) {
                    return Err(err.hint(format!("types start with a capital letter: `{t}`")));
                }
                if let Some(close) = super::error::did_you_mean(other, known.into_iter()) {
                    return Err(err.hint(format!("did you mean `{close}`?")));
                }
                return Err(err.hint("types are: Int, Bool, Str, Alphabet, Dfa, Nfa, Pda, Tm, Regex, Grammar, Set[T]"));
            }
        })
    }

    // if := 'if' expr block ('else' (if | block))?
    fn if_statement(&mut self) -> Parse<StmtKind> {
        self.advance();
        let cond = self.expr()?;
        let then = self.block()?;
        let otherwise = if self.eat(Tok::Else) {
            if *self.peek() == Tok::If {
                let start = self.span();
                self.nest()?;
                let kind = self.if_statement()?;
                self.nesting -= 1;
                Some(vec![Stmt { kind, span: start.to(self.prev()) }])
            } else {
                Some(self.block()?)
            }
        } else {
            None
        };
        Ok(StmtKind::If { cond, then, otherwise })
    }

    // block := '{' statement* '}'
    fn block(&mut self) -> Parse<Vec<Stmt>> {
        let open = self.expect(Tok::LBrace, "`{`")?;
        self.nest()?;
        self.depth += 1;
        let mut stmts = Vec::new();
        while *self.peek() != Tok::RBrace {
            if *self.peek() == Tok::Eof {
                return Err(unclosed(open));
            }
            stmts.push(self.statement()?);
        }
        self.advance();
        self.depth -= 1;
        self.nesting -= 1;
        Ok(stmts)
    }

    // ---- machines ----

    // machine-body := 'over' qualified-name '{' item* '}'
    fn machine_body(&mut self, kind: MachineKind, start: Span) -> Parse<MachineExpr> {
        self.expect(Tok::Over, "`over` and an alphabet")?;
        let alphabet = self.qualified_name("the name of an alphabet")?;
        let header = start.to(self.prev());
        let open = self.span();
        let errors_before = self.recovered.len();
        let items = self.machine_items(kind)?;
        let body = open.to(self.prev());
        let incomplete = self.recovered.len() > errors_before;
        Ok(MachineExpr { kind, alphabet, items, header, body, incomplete })
    }

    fn machine_items(&mut self, kind: MachineKind) -> Parse<Vec<MachineItem>> {
        let open = self.expect(Tok::LBrace, "`{`")?;
        self.nest()?;
        let mut items = Vec::new();
        while *self.peek() != Tok::RBrace {
            if *self.peek() == Tok::Eof {
                return Err(unclosed(open));
            }
            let start = self.pos;
            match self.machine_item(kind) {
                Ok(item) => items.push(item),
                // Items are one per line, so a bad one can be skipped and the
                // rest of the machine still checked for mistakes.
                Err(error) => {
                    // Nested too deeply: skipping would only meet the same
                    // problem again, so give up on the whole machine.
                    if error.code == codes::TOO_DEEP {
                        return Err(error);
                    }
                    self.recovered.push(error);
                    self.skip_to_next_item(start);
                }
            }
        }
        self.advance();
        self.nesting -= 1;
        Ok(items)
    }

    /// After a machine item that began at token `start` failed, moves to
    /// the next line, or to the `}` that ends the machine.
    fn skip_to_next_item(&mut self, start: usize) {
        let mut depth: i64 = 0;
        let mut i = start;
        loop {
            let tok = &self.tokens[i].tok;
            let past_error = i > self.pos || (i == self.pos && i > start);
            if *tok == Tok::Eof || (depth == 0 && *tok == Tok::RBrace && i > start)
                || (past_error && depth == 0 && self.starts_line(i))
            {
                self.pos = i;
                return;
            }
            match tok {
                Tok::LBrace => depth += 1,
                Tok::RBrace => depth -= 1,
                _ => {}
            }
            i += 1;
        }
    }

    // item := 'start' state
    //       | 'accept' state (',' state)*
    //       | 'reject' state (',' state)*                  (tm only)
    //       | 'for' NAME 'in' expr '{' item* '}'
    //       | state '--' action '-->' state
    fn machine_item(&mut self, kind: MachineKind) -> Parse<MachineItem> {
        match self.peek() {
            Tok::Start => {
                self.advance();
                Ok(MachineItem::Start(self.state()?))
            }
            Tok::Accept => {
                self.advance();
                let mut states = vec![self.state()?];
                while self.eat(Tok::Comma) {
                    states.push(self.state()?);
                }
                Ok(MachineItem::Accept(states))
            }
            Tok::Reject => {
                if kind != MachineKind::Tm {
                    return Err(self
                        .error(format!("only a Turing machine has `reject` states, and this is {} `{}`", if kind == MachineKind::Nfa { "an" } else { "a" }, kind.keyword()))
                        .code(codes::WRONG_MACHINE_KIND)
                        .hint("a finite automaton rejects every word that does not end in an accepting state"));
                }
                self.advance();
                let mut states = vec![self.state()?];
                while self.eat(Tok::Comma) {
                    states.push(self.state()?);
                }
                Ok(MachineItem::Reject(states))
            }
            Tok::For => {
                self.advance();
                let var = self.name("a loop variable")?;
                self.expect(Tok::In, "`in`")?;
                let items = self.expr()?;
                Ok(MachineItem::For { var, items, body: self.machine_items(kind)? })
            }
            Tok::Ident(_) => {
                let from = self.state()?;
                if *self.peek() != Tok::DashDash {
                    return Err(self
                        .error(format!(
                            "expected `--` to begin a transition, found {}",
                            self.peek().describe()
                        ))
                        .hint("a transition is written `q0 --a--> q1`"));
                }
                self.advance();
                let action = match kind {
                    MachineKind::Pda => self.stack_action()?,
                    MachineKind::Tm => self.tape_action()?,
                    _ => {
                        let mut labels = vec![self.label()?];
                        while self.eat(Tok::Comma) {
                            labels.push(self.label()?);
                        }
                        Action::Symbols(labels)
                    }
                };
                if *self.peek() != Tok::LongArrow {
                    return Err(self
                        .error(format!(
                            "expected `-->` to finish the transition, found {}",
                            self.peek().describe()
                        ))
                        .hint("a transition is written `q0 --a--> q1`; several symbols as `q0 --a,b--> q1`"));
                }
                self.advance();
                let to = self.state()?;
                let span = from.span.to(to.span);
                Ok(MachineItem::Transition { from, action, to, span })
            }
            other => Err(self
                .error(format!("expected a machine item, found {}", other.describe()))
                .hint("inside a machine write `start q0`, `accept q1`, or a transition `q0 --a--> q1`")),
        }
    }

    // action := label (',' label)*               in a DFA or NFA
    //         | label ',' label '/' push          in a PDA
    // push   := 'eps' | stack symbols written together, top first: `AZ`
    fn stack_action(&mut self) -> Parse<Action> {
        let hint = "a PDA move is written `q0 --a, Z/AZ--> q1`: read `a`, pop `Z`, push `AZ` (top first); any part may be `eps`";
        let read = self.label().map_err(|e| e.hint(hint))?;
        if !self.eat(Tok::Comma) {
            return Err(self
                .error(format!("expected `,` and the symbol to pop, found {}", self.peek().describe()))
                .hint(hint));
        }
        let pop = self.label().map_err(|e| e.hint(hint))?;
        if !self.eat(Tok::Slash) {
            return Err(self
                .error(format!("expected `/` and the symbols to push, found {}", self.peek().describe()))
                .hint(hint));
        }
        let push_span = self.span();
        match self.peek() {
            Tok::Eps => {
                self.advance();
                Ok(Action::Stack { read, pop, push: Vec::new(), push_span })
            }
            Tok::Ident(_) | Tok::Int(_) => {
                // `AZ` is one token; `0A` is two tokens that touch. Either
                // way the symbols are the characters written, top first.
                let mut push = Vec::new();
                let mut end = push_span;
                while matches!(self.peek(), Tok::Ident(_) | Tok::Int(_))
                    && (push.is_empty() || self.span().start == end.end)
                {
                    end = self.advance().span;
                    push.extend(self.text(end).chars());
                }
                let push_span = push_span.to(end);
                if push.contains(&'_') {
                    return Err(Diag::new(ErrorKind::Syntax, push_span, "`_` cannot be a stack symbol")
                        .code(codes::BAD_SYMBOL)
                        .hint("stack symbols are letters and digits"));
                }
                Ok(Action::Stack { read, pop, push, push_span })
            }
            other => Err(self.error(format!("expected the symbols to push, found {}", other.describe())).hint(hint)),
        }
    }

    // tape-action := tape-symbol '/' tape-symbol ',' ('L' | 'R' | 'S')     in a TM
    // tape-symbol := symbol | '_'
    fn tape_action(&mut self) -> Parse<Action> {
        let hint = "a Turing machine move is written `q0 --0/1,R--> q1`: read `0`, write `1`, move right (L, R, or S to stay); `_` is the blank";
        let read = self.tape_symbol(hint)?;
        if !self.eat(Tok::Slash) {
            return Err(self
                .error(format!("expected `/` and the symbol to write, found {}", self.peek().describe()))
                .hint(hint));
        }
        let write = self.tape_symbol(hint)?;
        if !self.eat(Tok::Comma) {
            return Err(self
                .error(format!("expected `,` and a direction (L, R or S), found {}", self.peek().describe()))
                .hint(hint));
        }
        let dir_span = self.span();
        let dir = match self.peek() {
            Tok::Ident(d) if d == "L" => Dir::Left,
            Tok::Ident(d) if d == "R" => Dir::Right,
            Tok::Ident(d) if d == "S" => Dir::Stay,
            other => {
                return Err(self
                    .error(format!("expected a direction (L, R or S), found {}", other.describe()))
                    .hint(hint));
            }
        };
        self.advance();
        Ok(Action::Tape { read, write, dir, dir_span })
    }

    /// One tape symbol: a letter or digit, or `_` for the blank. (A Turing
    /// machine always reads and writes something, so `eps` is not allowed.)
    fn tape_symbol(&mut self, hint: &str) -> Parse<Label> {
        if *self.peek() == Tok::Eps {
            return Err(self
                .error("a Turing machine always reads and writes a symbol, so `eps` cannot be used here")
                .code(codes::BAD_EPS)
                .hint("write `_` for the blank cell"));
        }
        self.label().map_err(|e| e.hint(hint))
    }

    // grammar-body := 'over' qualified-name '{' rule* '}'
    // rule         := NAME '->' alternative ('|' alternative)*
    // alternative  := 'eps' | gsymbol gsymbol*
    // gsymbol      := NAME starting with a capital letter (a variable)
    //               | symbol                            (a terminal)
    //
    // Line breaks mean nothing, so a rule ends where the next one begins:
    // at a name followed by `->`, or at the closing `}`.
    fn grammar_body(&mut self, start: Span) -> Parse<GrammarExpr> {
        self.expect(Tok::Over, "`over` and an alphabet")?;
        let alphabet = self.qualified_name("the name of an alphabet")?;
        let header = start.to(self.prev());
        let open = self.expect(Tok::LBrace, "`{`")?;
        let mut rules = Vec::new();
        while *self.peek() != Tok::RBrace {
            if *self.peek() == Tok::Eof {
                return Err(unclosed(open));
            }
            rules.push(self.rule()?);
        }
        self.advance();
        let body = open.to(self.prev());
        Ok(GrammarExpr { alphabet, rules, header, body })
    }

    fn rule(&mut self) -> Parse<RuleDef> {
        let rule_hint = "a rule is written `S -> a S b | eps`; variables start with a capital letter";
        let head = self.name("a variable to define").map_err(|e| e.hint(rule_hint))?;
        if !head.text.starts_with(char::is_uppercase) {
            return Err(Diag::new(
                ErrorKind::Syntax,
                head.span,
                format!("`{}` cannot have rules: variables start with a capital letter", head.text),
            )
            .code(codes::BAD_SYMBOL)
            .hint(rule_hint));
        }
        if *self.peek() != Tok::Arrow {
            return Err(self
                .error(format!("expected `->` after `{}`, found {}", head.text, self.peek().describe()))
                .hint(rule_hint));
        }
        self.advance();
        let mut alternatives = vec![self.alternative()?];
        while self.eat(Tok::Bar) {
            alternatives.push(self.alternative()?);
        }
        Ok(RuleDef { span: head.span.to(self.prev()), head, alternatives })
    }

    /// True where an alternative ends: at `|`, `}`, or the next rule.
    fn alternative_ends(&self) -> bool {
        matches!(self.peek(), Tok::RBrace | Tok::Bar | Tok::Eof)
            || (matches!(self.peek(), Tok::Ident(_)) && *self.peek_next() == Tok::Arrow)
    }

    fn alternative(&mut self) -> Parse<Vec<GSym>> {
        let eps_hint = "`eps` is the empty string; write it as a whole alternative: `S -> a S | eps`";
        if *self.peek() == Tok::Eps {
            let eps = self.advance().span;
            if !self.alternative_ends() {
                return Err(Diag::new(ErrorKind::Syntax, eps, "`eps` must stand alone").code(codes::BAD_EPS).hint(eps_hint));
            }
            return Ok(Vec::new());
        }
        let mut symbols = Vec::new();
        while !self.alternative_ends() {
            let span = self.span();
            let text = self.text(span);
            let kind = match self.peek() {
                Tok::Ident(name) if name.starts_with(char::is_uppercase) => GSymKind::Variable(name.clone()),
                Tok::Ident(_) | Tok::Int(_) if text.chars().count() == 1 => {
                    GSymKind::Terminal(text.chars().next().unwrap_or_default())
                }
                Tok::Ident(_) | Tok::Int(_) => {
                    let spaced: Vec<String> = text.chars().map(String::from).collect();
                    return Err(self.error(format!("`{text}` is not one symbol")).code(codes::BAD_SYMBOL).hint(format!(
                        "write terminals one at a time with spaces between: `{}`; variables start with a capital letter",
                        spaced.join(" ")
                    )));
                }
                Tok::Eps => return Err(self.error("`eps` must stand alone").code(codes::BAD_EPS).hint(eps_hint)),
                other => {
                    return Err(self
                        .error(format!("expected a symbol or a variable, found {}", other.describe()))
                        .hint("a rule is written `S -> a S b | eps`"));
                }
            };
            self.advance();
            symbols.push(GSym { kind, span });
        }
        if symbols.is_empty() {
            return Err(self
                .error(format!("expected a symbol, a variable or `eps`, found {}", self.peek().describe()))
                .hint("write `eps` for the empty string: `S -> a S | eps`"));
        }
        Ok(symbols)
    }

    // state := NAME ('[' expr ']')?
    fn state(&mut self) -> Parse<StateRef> {
        let name = self.name("a state name")?;
        let mut span = name.span;
        let index = if self.eat(Tok::LBracket) {
            let index = self.expr()?;
            span = span.to(self.expect(Tok::RBracket, "`]`")?);
            Some(index)
        } else {
            None
        };
        Ok(StateRef { name: name.text, index, span })
    }

    // label := 'eps' | symbol        (a symbol is one letter or one digit)
    fn label(&mut self) -> Parse<Label> {
        let span = self.span();
        let symbol = match self.peek() {
            Tok::Eps => None,
            Tok::Int(_) | Tok::Ident(_) => {
                let mut chars = self.text(span).chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => Some(c),
                    _ => {
                        return Err(self
                            .error(format!("`{}` is not a single symbol", self.text(span)))
                            .hint("a symbol is one letter or digit; separate several with commas: `0, 1`"));
                    }
                }
            }
            other => {
                return Err(self
                    .error(format!("expected a symbol, found {}", other.describe()))
                    .hint("a symbol is one letter or digit, or `eps` for the empty string"));
            }
        };
        self.advance();
        Ok(Label { symbol, span })
    }

    // ---- expressions, loosest-binding rule first ----

    // expr := or
    pub fn expr(&mut self) -> Parse<Expr> {
        self.nest()?;
        let expr = self.or()?;
        self.nesting -= 1;
        Ok(expr)
    }

    fn binary(op: BinOp, left: Expr, right: Expr) -> Expr {
        let span = left.span.to(right.span);
        Expr { kind: ExprKind::Binary(op, Box::new(left), Box::new(right)), span }
    }

    // or := and ('or' and)*
    fn or(&mut self) -> Parse<Expr> {
        let mut left = self.and()?;
        let outer = self.nesting;
        while self.eat(Tok::Or) {
            self.nest()?;
            left = Self::binary(BinOp::Or, left, self.and()?);
        }
        self.nesting = outer;
        Ok(left)
    }

    // and := not ('and' not)*
    fn and(&mut self) -> Parse<Expr> {
        let mut left = self.not()?;
        let outer = self.nesting;
        while self.eat(Tok::And) {
            self.nest()?;
            left = Self::binary(BinOp::And, left, self.not()?);
        }
        self.nesting = outer;
        Ok(left)
    }

    // not := 'not' not | comparison
    fn not(&mut self) -> Parse<Expr> {
        if *self.peek() == Tok::Not {
            let start = self.advance().span;
            self.nest()?;
            let inner = self.not()?;
            self.nesting -= 1;
            let span = start.to(inner.span);
            return Ok(Expr { kind: ExprKind::Unary(UnOp::Not, Box::new(inner)), span });
        }
        self.comparison()
    }

    fn comparison_op(&self) -> Option<BinOp> {
        Some(match self.peek() {
            Tok::EqEq => BinOp::Eq,
            Tok::NotEq => BinOp::NotEq,
            Tok::Lt => BinOp::Lt,
            Tok::Le => BinOp::Le,
            Tok::Gt => BinOp::Gt,
            Tok::Ge => BinOp::Ge,
            Tok::Accepts => BinOp::Accepts,
            _ => return None,
        })
    }

    // comparison := range (('==' | '!=' | '<' | '<=' | '>' | '>=' | 'accepts') range)?
    fn comparison(&mut self) -> Parse<Expr> {
        let left = self.range()?;
        let Some(op) = self.comparison_op() else { return Ok(left) };
        self.advance();
        let result = Self::binary(op, left, self.range()?);
        if self.comparison_op().is_some() {
            return Err(self
                .error("comparisons cannot be chained")
                .hint("join two comparisons with `and`: `a < b and b < c`"));
        }
        Ok(result)
    }

    // range := sum ('..' sum)?
    fn range(&mut self) -> Parse<Expr> {
        let left = self.sum()?;
        if self.eat(Tok::DotDot) {
            return Ok(Self::binary(BinOp::Range, left, self.sum()?));
        }
        Ok(left)
    }

    // sum := product (('+' | '-') product)*
    fn sum(&mut self) -> Parse<Expr> {
        let mut left = self.product()?;
        let outer = self.nesting;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            self.nest()?;
            left = Self::binary(op, left, self.product()?);
        }
        self.nesting = outer;
        Ok(left)
    }

    // product := unary (('*' | '%') unary)*
    fn product(&mut self) -> Parse<Expr> {
        let mut left = self.unary()?;
        let outer = self.nesting;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            self.nest()?;
            left = Self::binary(op, left, self.unary()?);
        }
        self.nesting = outer;
        Ok(left)
    }

    // unary := '-' unary | primary
    fn unary(&mut self) -> Parse<Expr> {
        if *self.peek() == Tok::Minus {
            let start = self.advance().span;
            self.nest()?;
            let inner = self.unary()?;
            self.nesting -= 1;
            let span = start.to(inner.span);
            return Ok(Expr { kind: ExprKind::Unary(UnOp::Neg, Box::new(inner)), span });
        }
        self.primary()
    }

    // primary := INT | STRING | REGEX | 'true' | 'false'
    //          | qualified-name | qualified-name '(' (expr (',' expr)*)? ')'
    //          | '(' expr ')' | '{' (expr (',' expr)*)? '}'
    //          | ('dfa' | 'nfa') machine-body
    fn primary(&mut self) -> Parse<Expr> {
        let start = self.span();
        let kind = match self.peek().clone() {
            Tok::Int(n) => {
                self.advance();
                ExprKind::Int(n)
            }
            Tok::Str(s) => {
                self.advance();
                ExprKind::Str(s)
            }
            Tok::True | Tok::False => ExprKind::Bool(self.advance().tok == Tok::True),
            Tok::Regex(text) => {
                self.advance();
                let parsed = regex::parse(&text).map_err(|(offset, message)| {
                    let at = start.start + 1 + offset; // +1 skips the opening slash
                    let width = self.src[at..].chars().next().map_or(1, char::len_utf8);
                    let span = Span { start: at, end: at + width, ..start };
                    Diag::new(ErrorKind::Syntax, span, format!("in regex: {message}"))
                        .code(codes::BAD_REGEX)
                        .hint("a regex is made of symbols, `|` for or, `*` for repeat, and `( )` for grouping, as in /(0|1)*01/")
                })?;
                ExprKind::Regex(parsed)
            }
            Tok::Ident(_) => {
                let name = self.qualified_name("a name")?;
                if self.eat(Tok::LParen) {
                    let mut args = Vec::new();
                    while *self.peek() != Tok::RParen {
                        args.push(self.expr()?);
                        if !self.eat(Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(Tok::RParen, "`,` or `)`")?;
                    ExprKind::Call { name, args }
                } else {
                    ExprKind::Name(name.text)
                }
            }
            Tok::LParen => {
                self.advance();
                let inner = self.expr()?;
                self.expect(Tok::RParen, "`)`")?;
                ExprKind::Paren(Box::new(inner))
            }
            Tok::LBrace => {
                self.advance();
                let mut items = Vec::new();
                while *self.peek() != Tok::RBrace {
                    items.push(self.expr()?);
                    if !self.eat(Tok::Comma) {
                        break;
                    }
                }
                self.expect(Tok::RBrace, "`,` or `}`")?;
                ExprKind::Set(items)
            }
            Tok::Dfa | Tok::Nfa | Tok::Pda | Tok::Tm => {
                let kind = machine_kind(&self.advance().tok);
                if let Tok::Ident(name) = self.peek() {
                    return Err(self
                        .error(format!("a machine used as a value cannot have a name (`{name}`)"))
                        .hint(format!("write `{} over B {{ ... }}` here", kind.keyword())));
                }
                ExprKind::Machine(Box::new(self.machine_body(kind, start)?))
            }
            Tok::Grammar => {
                self.advance();
                if let Tok::Ident(name) = self.peek() {
                    return Err(self
                        .error(format!("a grammar used as a value cannot have a name (`{name}`)"))
                        .hint("write `grammar over B { ... }` here"));
                }
                ExprKind::Grammar(Box::new(self.grammar_body(start)?))
            }
            other => {
                // `let x = 2 +` followed by a new statement: the mistake is
                // the unfinished line, so point there.
                if self.pos > 0 && self.span().line > self.prev().line {
                    let after = self.text(self.prev());
                    let next = match other {
                        Tok::Eof => "the file ends here".to_string(),
                        _ => format!("the next line starts with {}", other.describe()),
                    };
                    return Err(Diag::new(
                        ErrorKind::Syntax,
                        self.prev(),
                        format!("expected a value after `{after}`, but {next}"),
                    )
                    .hint("finish the expression on this line"));
                }
                let mut err = self.error(format!("expected a value, found {}", other.describe()));
                if other == Tok::Assign {
                    err = err.hint("to compare two values, write `==`");
                }
                return Err(err);
            }
        };
        Ok(Expr { kind, span: start.to(self.prev()) })
    }
}

/// True if `text` can be used as a name: letters, digits and `_`, not
/// starting with a digit, and not a keyword.
fn is_name(text: &str) -> bool {
    text.starts_with(|c: char| c.is_alphabetic() || c == '_')
        && text.chars().all(|c| c.is_alphanumeric() || c == '_')
        && Tok::keyword(text).is_none()
}

/// The error for a `{` with no matching `}`.
fn unclosed(open: Span) -> Diag {
    Diag::new(ErrorKind::Syntax, open, "this `{` is never closed")
        .code(codes::UNCLOSED)
        .hint("add the missing `}`; the block that starts here runs to the end of the file")
}

/// The kind of machine a `dfa`, `nfa`, `pda` or `tm` keyword introduces.
fn machine_kind(tok: &Tok) -> MachineKind {
    match tok {
        Tok::Dfa => MachineKind::Dfa,
        Tok::Nfa => MachineKind::Nfa,
        Tok::Pda => MachineKind::Pda,
        _ => MachineKind::Tm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::lexer::lex;

    fn parse(src: &str) -> Parse<Vec<Stmt>> {
        Parser::new(lex(src, 0, 1)?, src, false).program()
    }

    fn parse_expr(src: &str) -> Expr {
        match parse(&format!("let x = {src}")).unwrap().remove(0).kind {
            StmtKind::Let { value, .. } => value,
            _ => unreachable!(),
        }
    }

    /// Prints an expression fully parenthesised, to check precedence.
    fn show(e: &Expr) -> String {
        match &e.kind {
            ExprKind::Int(n) => n.to_string(),
            ExprKind::Name(n) => n.clone(),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Str(s) => format!("{s:?}"),
            ExprKind::Paren(inner) => show(inner),
            ExprKind::Unary(UnOp::Neg, a) => format!("(-{})", show(a)),
            ExprKind::Unary(UnOp::Not, a) => format!("(not {})", show(a)),
            ExprKind::Binary(op, a, b) => format!("({} {} {})", show(a), op.symbol(), show(b)),
            ExprKind::Call { name, args } => {
                format!("{}({})", name.text, args.iter().map(show).collect::<Vec<_>>().join(", "))
            }
            _ => "?".into(),
        }
    }

    #[test]
    fn precedence() {
        assert_eq!(show(&parse_expr("1 + 2 * 3")), "(1 + (2 * 3))");
        assert_eq!(show(&parse_expr("(2 * i + 1) % n")), "(((2 * i) + 1) % n)");
        assert_eq!(show(&parse_expr("0..n - 1")), "(0 .. (n - 1))");
        assert_eq!(show(&parse_expr("not a and b or c")), "(((not a) and b) or c)");
        assert_eq!(show(&parse_expr("d accepts \"01\" and x == 1")), "((d accepts \"01\") and (x == 1))");
        assert_eq!(show(&parse_expr("f(1, g(2)) - -3")), "(f(1, g(2)) - (-3))");
    }

    #[test]
    fn parses_a_machine() {
        let src = "dfa M over B {\n start q0\n accept q0, q1\n q0 --0,1--> q1\n q1 --eps--> q[i + 1]\n}";
        let stmts = parse(src).unwrap();
        let StmtKind::MachineDef { name, machine } = &stmts[0].kind else { panic!() };
        assert_eq!(name.text, "M");
        assert_eq!(machine.kind, MachineKind::Dfa);
        assert_eq!(machine.alphabet.text, "B");
        assert_eq!(machine.items.len(), 4);
        let MachineItem::Transition { action: Action::Symbols(labels), to, .. } = &machine.items[3] else {
            panic!()
        };
        assert_eq!(labels[0].symbol, None);
        assert!(to.index.is_some());
        assert_eq!(&src[machine.header.start..machine.header.end], "dfa M over B");
    }

    #[test]
    fn parses_functions_and_control_flow() {
        let src = "fn f(n: Int, s: Set[Str]) -> Bool {\n if n < 1 { return true } else if n < 2 { return false } else { return f(n - 1, s) }\n}\nfor w in {\"a\", \"b\"} { print w, 1 }";
        let stmts = parse(src).unwrap();
        let StmtKind::Fn(f) = &stmts[0].kind else { panic!() };
        assert_eq!(f.params[1].1, Type::Set(Box::new(Type::Str)));
        assert!(matches!(stmts[1].kind, StmtKind::For { .. }));
    }

    #[test]
    fn helpful_syntax_errors() {
        let hint = |src: &str| parse(src).unwrap_err().hint.unwrap_or_default();
        assert!(hint("x = 1").contains("let"));
        assert!(hint("dfa M over B { q0 -> q1 }").contains("q0 --a--> q1"));
        assert!(hint("dfa M over B { q0 --ab--> q1 }").contains("one letter or digit"));
        assert!(hint("fn f(n) -> Int { return n }").contains("n: Int"));
        assert!(hint("fn f(n: Int) { return n }").contains("-> Type"));
        assert!(hint("let x = 1 < 2 < 3").contains("and"));
        assert!(hint("let if = 1").contains("keyword"));
        assert!(parse("if true { fn f() -> Int { return 1 } }").is_err());
        assert!(parse("let r = /(ab/").unwrap_err().message.contains("never closed"));
        assert!(hint("trace M accepts \"01\"").contains("trace M on"));
    }

    #[test]
    fn parses_pda_moves() {
        let src = "pda P over B { start p  p --a, eps/AZ--> p  p --eps, Z/eps--> q  p --0, A/0A--> p }";
        let stmts = parse(src).unwrap();
        let StmtKind::MachineDef { machine, .. } = &stmts[0].kind else { panic!() };
        assert_eq!(machine.kind, MachineKind::Pda);
        let moves: Vec<(Option<char>, Option<char>, String)> = machine.items[1..]
            .iter()
            .map(|item| match item {
                MachineItem::Transition { action: Action::Stack { read, pop, push, .. }, .. } => {
                    (read.symbol, pop.symbol, push.iter().collect())
                }
                _ => panic!(),
            })
            .collect();
        assert_eq!(
            moves,
            [(Some('a'), None, "AZ".into()), (None, Some('Z'), "".into()), (Some('0'), Some('A'), "0A".into())]
        );
        let hint = |src: &str| parse(src).unwrap_err().hint.unwrap_or_default();
        assert!(hint("pda P over B { p --a--> q }").contains("read `a`, pop `Z`"));
        assert!(hint("pda P over B { p --a, Z--> q }").contains("read `a`, pop `Z`"));
    }

    #[test]
    fn parses_turing_machines() {
        let src = "tm M over B { start s  accept y  reject n  s --0/_,R--> s  s --_/1,L--> y }";
        let stmts = parse(src).unwrap();
        let StmtKind::MachineDef { machine, .. } = &stmts[0].kind else { panic!() };
        assert_eq!(machine.kind, MachineKind::Tm);
        assert!(matches!(machine.items[2], MachineItem::Reject(_)));
        let MachineItem::Transition { action: Action::Tape { read, write, dir, .. }, .. } = &machine.items[3] else {
            panic!()
        };
        assert_eq!((read.symbol, write.symbol, *dir), (Some('0'), Some('_'), Dir::Right));
        let hint = |src: &str| parse(src).unwrap_err().hint.unwrap_or_default();
        assert!(hint("tm M over B { s --0/1--> s }").contains("move right"));
        assert!(hint("tm M over B { s --0/1,X--> s }").contains("move right"));
        assert!(hint("tm M over B { s --eps/1,R--> s }").contains("`_`"));
        assert!(hint("dfa M over B { reject s }").contains("finite automaton"));
    }

    #[test]
    fn parses_grammars() {
        let src = "grammar G over B { S -> a S b | eps  T -> S S\n | a }";
        let stmts = parse(src).unwrap();
        let StmtKind::GrammarDef { grammar, .. } = &stmts[0].kind else { panic!() };
        assert_eq!(grammar.rules.len(), 2);
        let s = &grammar.rules[0];
        assert_eq!(s.alternatives.len(), 2);
        assert_eq!(s.alternatives[0][0].kind, GSymKind::Terminal('a'));
        assert_eq!(s.alternatives[0][1].kind, GSymKind::Variable("S".into()));
        assert!(s.alternatives[1].is_empty());
        assert_eq!(grammar.rules[1].alternatives.len(), 2);
        let hint = |src: &str| parse(src).unwrap_err().hint.unwrap_or_default();
        assert!(hint("grammar G over B { S -> aSb }").contains("`a S b`"));
        assert!(hint("grammar G over B { S -> a eps }").contains("whole alternative"));
        assert!(hint("grammar G over B { s -> a }").contains("capital letter"));
        assert!(hint("grammar G over B { S -> }").contains("eps"));
    }

    #[test]
    fn parses_trace() {
        let stmts = parse("trace minimize(M) on \"01\" + \"1\"").unwrap();
        let StmtKind::Trace { machine, word } = &stmts[0].kind else { panic!() };
        assert_eq!(show(machine), "minimize(M)");
        assert_eq!(show(word), "(\"01\" + \"1\")");
    }

    #[test]
    fn repl_allows_bare_expressions() {
        let src = "1 + 2";
        let stmts = Parser::new(lex(src, 0, 1).unwrap(), src, true).program().unwrap();
        assert!(matches!(stmts[0].kind, StmtKind::Print(_)));
        assert!(parse(src).is_err());
    }

    fn errors(src: &str) -> Vec<(u32, String)> {
        let (_, errors) = Parser::new(lex(src, 0, 1).unwrap(), src, false).program_recovering();
        errors.into_iter().map(|e| (e.span.line, e.message)).collect()
    }

    #[test]
    fn recovers_at_the_next_statement() {
        let found = errors("let a = 1 +\nlet b = = 2\nprint a\nlet c = )\n");
        let lines: Vec<u32> = found.iter().map(|(line, _)| *line).collect();
        assert_eq!(lines, [1, 2, 4], "{found:?}");
        assert!(found[0].1.contains("after `+`"), "{found:?}");
    }

    #[test]
    fn recovers_inside_a_machine_and_after_a_missing_brace() {
        let src = "dfa M over B {\n  start q0\n  q0 -> q1\n  q1 --0-> q1\n  accept q1\n}\nfn f( -> Int {\nlet ok = 1\nlet bad = )\n";
        let lines: Vec<u32> = errors(src).iter().map(|(line, _)| *line).collect();
        assert_eq!(lines, [3, 4, 7, 9]);
        // The good items of the machine are kept, and it is marked incomplete.
        let (program, _) = Parser::new(lex(src, 0, 1).unwrap(), src, false).program_recovering();
        let StmtKind::MachineDef { machine, .. } = &program[0].kind else { panic!("not a machine") };
        assert_eq!(machine.items.len(), 2);
        assert!(machine.incomplete);
    }

    #[test]
    fn an_unclosed_block_does_not_hide_later_definitions() {
        let src = "fn f() -> Int {\n    return 1\n\nlet x = )\nfn g() -> Int { return ) }\n";
        let found = errors(src);
        assert!(found.iter().any(|(line, _)| *line == 5), "{found:?}");
    }

    #[test]
    fn errors_have_codes() {
        let code = |src: &str| parse(src).unwrap_err().code;
        assert_eq!(code("x = 1"), codes::NEEDS_LET);
        assert_eq!(code("let if = 1"), codes::KEYWORD_AS_NAME);
        assert_eq!(code("let r = /(ab/"), codes::BAD_REGEX);
        assert_eq!(code("fn f(n: int) -> Int { return n }"), codes::UNKNOWN_TYPE);
        assert_eq!(code("print"), codes::SYNTAX);
        assert_eq!(code("if true {"), codes::UNCLOSED);
    }

    // Regression: deep nesting overflowed the stack instead of being an error.
    #[test]
    fn deep_nesting_is_an_error() {
        // Programs near the limit need the big stack every caller provides.
        crate::with_big_stack(deep_nesting_cases);
    }

    fn deep_nesting_cases() {
        let too_deep = [
            format!("print {}1{}", "(".repeat(MAX_NESTING + 1), ")".repeat(MAX_NESTING + 1)),
            format!("print 1{}", " + 1".repeat(MAX_NESTING + 1)),
            format!("print {}true", "not ".repeat(MAX_NESTING + 1)),
            format!("{}print 1{}", "if true { ".repeat(MAX_NESTING + 1), " }".repeat(MAX_NESTING + 1)),
            format!("if true {{ print 1 }}{}", " else if true { print 1 }".repeat(MAX_NESTING + 1)),
        ];
        for src in &too_deep {
            assert_eq!(parse(src).unwrap_err().code, codes::TOO_DEEP, "{}", &src[..40]);
        }
        let fine = format!("print {}1{}", "(".repeat(MAX_NESTING - 5), ")".repeat(MAX_NESTING - 5));
        assert!(parse(&fine).is_ok());
    }
}

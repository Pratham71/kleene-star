//! The parser: turns tokens into a syntax tree.
//!
//! This is a recursive-descent parser: there is one method per grammar rule,
//! and each method calls the methods for the rules it is made of. The grammar
//! is written out in `docs/reference.md`; the comment above each method quotes
//! the rule it implements.

use super::ast::*;
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
}

type Parse<T> = Result<T, Diag>;

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token>, src: &'a str, repl: bool) -> Parser<'a> {
        Parser { tokens, pos: 0, src, repl, depth: 0 }
    }

    // program := statement*
    pub fn program(&mut self) -> Parse<Vec<Stmt>> {
        let mut stmts = Vec::new();
        while *self.peek() != Tok::Eof {
            stmts.push(self.statement()?);
        }
        Ok(stmts)
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
                    err = err.hint(format!(
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
    //            | print | test | draw
    fn statement(&mut self) -> Parse<Stmt> {
        let start = self.span();
        let kind = match self.peek() {
            Tok::Alphabet => self.alphabet()?,
            Tok::Dfa | Tok::Nfa => {
                // machine-def := ('dfa' | 'nfa') NAME 'over' NAME '{' item* '}'
                let is_dfa = self.advance().tok == Tok::Dfa;
                let word = if is_dfa { "dfa" } else { "nfa" };
                if *self.peek() == Tok::Over {
                    return Err(self.error(format!("a `{word}` statement needs a name")).hint(
                        format!("write `{word} Name over B {{ ... }}`, or `let m = {word} over B {{ ... }}`"),
                    ));
                }
                let name = self.name("a name for the machine")?;
                let machine = self.machine_body(is_dfa, start)?;
                StmtKind::MachineDef { name, machine }
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
            Tok::Ident(_) if *self.peek_next() == Tok::Assign => {
                return Err(self.error("assignment needs `let`").hint(
                    "write `let x = ...`; values cannot be changed once defined, so pick a new name",
                ));
            }
            _ if self.repl => StmtKind::Print(vec![self.expr()?]),
            other => {
                return Err(self
                    .error(format!("expected a statement, found {}", other.describe()))
                    .hint("a statement starts with one of: alphabet, dfa, nfa, let, fn, return, if, for, print, test, draw"));
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

    // fn := 'fn' NAME '(' (NAME ':' type (',' NAME ':' type)*)? ')' '->' type block
    fn function(&mut self) -> Parse<StmtKind> {
        if self.depth > 0 {
            return Err(self.error("functions can only be defined at the top level"));
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

    // type := 'Int' | 'Bool' | 'Str' | 'Alphabet' | 'Dfa' | 'Nfa' | 'Regex' | 'Set' '[' type ']'
    fn type_name(&mut self) -> Parse<Type> {
        let name = self.name("a type").map_err(|e| {
            e.hint("types are: Int, Bool, Str, Alphabet, Dfa, Nfa, Regex, Set[T]")
        })?;
        Ok(match name.text.as_str() {
            "Int" => Type::Int,
            "Bool" => Type::Bool,
            "Str" => Type::Str,
            "Alphabet" => Type::Alphabet,
            "Dfa" => Type::Dfa,
            "Nfa" => Type::Nfa,
            "Regex" => Type::Regex,
            "Set" => {
                self.expect(Tok::LBracket, "`[` after `Set`")?;
                let inner = self.type_name()?;
                self.expect(Tok::RBracket, "`]`")?;
                Type::Set(Box::new(inner))
            }
            other => {
                return Err(Diag::new(
                    ErrorKind::Type,
                    name.span,
                    format!("unknown type `{other}`"),
                )
                .hint("types are: Int, Bool, Str, Alphabet, Dfa, Nfa, Regex, Set[T]"));
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
                let kind = self.if_statement()?;
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
        self.depth += 1;
        let mut stmts = Vec::new();
        while *self.peek() != Tok::RBrace {
            if *self.peek() == Tok::Eof {
                return Err(Diag::new(ErrorKind::Syntax, open, "this `{` is never closed"));
            }
            stmts.push(self.statement()?);
        }
        self.advance();
        self.depth -= 1;
        Ok(stmts)
    }

    // ---- machines ----

    // machine-body := 'over' NAME '{' item* '}'
    fn machine_body(&mut self, is_dfa: bool, start: Span) -> Parse<MachineExpr> {
        self.expect(Tok::Over, "`over` and an alphabet")?;
        let alphabet = self.name("the name of an alphabet")?;
        let header = start.to(self.prev());
        let items = self.machine_items()?;
        Ok(MachineExpr { is_dfa, alphabet, items, header })
    }

    fn machine_items(&mut self) -> Parse<Vec<MachineItem>> {
        let open = self.expect(Tok::LBrace, "`{`")?;
        let mut items = Vec::new();
        while *self.peek() != Tok::RBrace {
            if *self.peek() == Tok::Eof {
                return Err(Diag::new(ErrorKind::Syntax, open, "this `{` is never closed"));
            }
            items.push(self.machine_item()?);
        }
        self.advance();
        Ok(items)
    }

    // item := 'start' state
    //       | 'accept' state (',' state)*
    //       | 'for' NAME 'in' expr '{' item* '}'
    //       | state '--' label (',' label)* '-->' state
    fn machine_item(&mut self) -> Parse<MachineItem> {
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
            Tok::For => {
                self.advance();
                let var = self.name("a loop variable")?;
                self.expect(Tok::In, "`in`")?;
                let items = self.expr()?;
                Ok(MachineItem::For { var, items, body: self.machine_items()? })
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
                let mut labels = vec![self.label()?];
                while self.eat(Tok::Comma) {
                    labels.push(self.label()?);
                }
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
                Ok(MachineItem::Transition { from, labels, to, span })
            }
            other => Err(self
                .error(format!("expected a machine item, found {}", other.describe()))
                .hint("inside a machine write `start q0`, `accept q1`, or a transition `q0 --a--> q1`")),
        }
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
        self.or()
    }

    fn binary(op: BinOp, left: Expr, right: Expr) -> Expr {
        let span = left.span.to(right.span);
        Expr { kind: ExprKind::Binary(op, Box::new(left), Box::new(right)), span }
    }

    // or := and ('or' and)*
    fn or(&mut self) -> Parse<Expr> {
        let mut left = self.and()?;
        while self.eat(Tok::Or) {
            left = Self::binary(BinOp::Or, left, self.and()?);
        }
        Ok(left)
    }

    // and := not ('and' not)*
    fn and(&mut self) -> Parse<Expr> {
        let mut left = self.not()?;
        while self.eat(Tok::And) {
            left = Self::binary(BinOp::And, left, self.not()?);
        }
        Ok(left)
    }

    // not := 'not' not | comparison
    fn not(&mut self) -> Parse<Expr> {
        if *self.peek() == Tok::Not {
            let start = self.advance().span;
            let inner = self.not()?;
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
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => return Ok(left),
            };
            self.advance();
            left = Self::binary(op, left, self.product()?);
        }
    }

    // product := unary (('*' | '%') unary)*
    fn product(&mut self) -> Parse<Expr> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Percent => BinOp::Mod,
                _ => return Ok(left),
            };
            self.advance();
            left = Self::binary(op, left, self.unary()?);
        }
    }

    // unary := '-' unary | primary
    fn unary(&mut self) -> Parse<Expr> {
        if *self.peek() == Tok::Minus {
            let start = self.advance().span;
            let inner = self.unary()?;
            let span = start.to(inner.span);
            return Ok(Expr { kind: ExprKind::Unary(UnOp::Neg, Box::new(inner)), span });
        }
        self.primary()
    }

    // primary := INT | STRING | REGEX | 'true' | 'false'
    //          | NAME | NAME '(' (expr (',' expr)*)? ')'
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
                    let span = Span { start: at, end: at + 1, line: start.line };
                    Diag::new(ErrorKind::Syntax, span, format!("in regex: {message}"))
                })?;
                ExprKind::Regex(parsed)
            }
            Tok::Ident(text) => {
                let name = Name { text, span: self.advance().span };
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
                return Ok(Expr { kind: inner.kind, span: start.to(self.prev()) });
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
            Tok::Dfa | Tok::Nfa => {
                let is_dfa = self.advance().tok == Tok::Dfa;
                if let Tok::Ident(name) = self.peek() {
                    let word = if is_dfa { "dfa" } else { "nfa" };
                    return Err(self
                        .error(format!("a machine used as a value cannot have a name (`{name}`)"))
                        .hint(format!("write `{word} over B {{ ... }}` here")));
                }
                ExprKind::Machine(Box::new(self.machine_body(is_dfa, start)?))
            }
            other => {
                return Err(self.error(format!("expected a value, found {}", other.describe())));
            }
        };
        Ok(Expr { kind, span: start.to(self.prev()) })
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
        assert!(machine.is_dfa);
        assert_eq!(machine.alphabet.text, "B");
        assert_eq!(machine.items.len(), 4);
        let MachineItem::Transition { labels, to, .. } = &machine.items[3] else { panic!() };
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
    }

    #[test]
    fn repl_allows_bare_expressions() {
        let src = "1 + 2";
        let stmts = Parser::new(lex(src, 0, 1).unwrap(), src, true).program().unwrap();
        assert!(matches!(stmts[0].kind, StmtKind::Print(_)));
        assert!(parse(src).is_err());
    }
}

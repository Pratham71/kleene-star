//! The checker: finds mistakes before the program runs.
//!
//! It walks the syntax tree once, working out the type of every expression
//! and keeping a table of the names in scope. Anything it accepts cannot
//! fail later with a type or name error, which is what lets the interpreter
//! stay simple.

use super::ast::*;
use super::builtins::{self, article, is_context_free, is_lang};
use super::error::{did_you_mean, Diag, ErrorKind};
use super::index::{Body, Call, Def, DefKind, Index};
use super::machine::{self, GrammarSpec, MachineSpec, Transition};
use super::token::Span;
use std::collections::HashMap;

#[derive(Clone)]
struct Var {
    ty: Type,
    /// For alphabets declared with `alphabet`, the symbols, so machines that
    /// use them can be validated before running.
    alphabet: Option<Vec<char>>,
    /// Its entry in the index.
    def: usize,
}

#[derive(Clone)]
struct Signature {
    params: Vec<Type>,
    ret: Type,
    def: usize,
}

#[derive(Clone)]
pub struct Checker {
    /// Innermost scope last. `scopes[0]` holds the globals.
    scopes: Vec<HashMap<String, Var>>,
    funcs: HashMap<String, Signature>,
    /// The return type of the function being checked, if we are inside one.
    ret: Option<Type>,
    /// The REPL lets you redefine top-level names.
    repl: bool,
    /// Where the innermost block being checked ends, so the index knows how
    /// far a name defined in it can be seen.
    block_end: usize,
    /// Every definition, use and type found, for editor tools.
    pub index: Index,
}

type Check<T> = Result<T, Diag>;

fn type_error(span: Span, message: impl Into<String>) -> Diag {
    Diag::new(ErrorKind::Type, span, message)
}

impl Checker {
    pub fn new(repl: bool) -> Checker {
        Checker {
            scopes: vec![HashMap::new()],
            funcs: HashMap::new(),
            ret: None,
            repl,
            block_end: usize::MAX,
            index: Index::default(),
        }
    }

    /// Checks a whole program (or, in the REPL, one more piece of it).
    pub fn check(&mut self, program: &[Stmt]) -> Check<()> {
        // Functions are declared first so they can call each other in any order.
        for stmt in program {
            if let StmtKind::Fn(f) = &stmt.kind {
                if builtins::NAMES.contains(&f.name.text.as_str()) {
                    return Err(Diag::new(
                        ErrorKind::Name,
                        f.name.span,
                        format!("`{}` is a built-in function and cannot be redefined", f.name.text),
                    ));
                }
                let params: Vec<String> = f.params.iter().map(|(n, ty)| format!("{}: {ty}", n.text)).collect();
                let def = self.index.add_def(Def {
                    name: f.name.text.clone(),
                    kind: DefKind::Function,
                    ty: None,
                    span: f.name.span,
                    visible: (0, usize::MAX),
                    detail: format!("fn {}({}) -> {}", f.name.text, params.join(", "), f.ret),
                });
                let signature = Signature {
                    params: f.params.iter().map(|(_, ty)| ty.clone()).collect(),
                    ret: f.ret.clone(),
                    def,
                };
                if self.funcs.insert(f.name.text.clone(), signature).is_some() && !self.repl {
                    return Err(Diag::new(
                        ErrorKind::Name,
                        f.name.span,
                        format!("function `{}` is already defined", f.name.text),
                    ));
                }
            }
        }
        self.statements(program)
    }

    // ---- names ----

    /// Defines `name` in the innermost scope. `kind` and `detail` are only
    /// for the index.
    fn define(
        &mut self,
        name: &Name,
        ty: Type,
        alphabet: Option<Vec<char>>,
        kind: DefKind,
        detail: String,
    ) -> Check<()> {
        let top_level = self.scopes.len() == 1;
        if self.scopes.last().unwrap().contains_key(&name.text) && !(self.repl && top_level) {
            return Err(Diag::new(
                ErrorKind::Name,
                name.span,
                format!("`{}` is already defined", name.text),
            )
            .hint("values cannot be changed once defined; choose a new name"));
        }
        let def = self.index.add_def(Def {
            name: name.text.clone(),
            kind,
            ty: Some(ty.clone()),
            span: name.span,
            visible: (name.span.end, self.block_end),
            detail,
        });
        self.scopes.last_mut().unwrap().insert(name.text.clone(), Var { ty, alphabet, def });
        Ok(())
    }

    /// Defines a `let`, a parameter or a loop variable: the detail is just
    /// its name and type.
    fn define_value(&mut self, name: &Name, ty: Type, kind: DefKind, alphabet: Option<Vec<char>>) -> Check<()> {
        let detail = match kind {
            DefKind::Parameter => format!("{}: {ty} (parameter)", name.text),
            DefKind::LoopVariable => format!("for {}: {ty}", name.text),
            _ => format!("let {}: {ty}", name.text),
        };
        self.define(name, ty, alphabet, kind, detail)
    }

    fn lookup(&mut self, name: &str, span: Span) -> Check<Var> {
        if let Some(var) = self.scopes.iter().rev().find_map(|scope| scope.get(name)) {
            let var = var.clone();
            self.index.add_ref(span, var.def);
            return Ok(var);
        }
        let mut err = Diag::new(ErrorKind::Name, span, format!("`{name}` is not defined"));
        if self.funcs.contains_key(name) || builtins::NAMES.contains(&name) {
            err = err.hint(format!("`{name}` is a function; call it as `{name}(...)`"));
        } else {
            let visible = self.scopes.iter().flat_map(|scope| scope.keys().map(String::as_str));
            if let Some(close) = did_you_mean(name, visible) {
                err = err.hint(format!("did you mean `{close}`?"));
            }
        }
        Err(err)
    }

    /// Runs `body` with a fresh inner scope, for a block that ends at byte
    /// `end` of the source.
    fn scoped<T>(&mut self, end: usize, body: impl FnOnce(&mut Checker) -> Check<T>) -> Check<T> {
        self.scopes.push(HashMap::new());
        let outer_end = std::mem::replace(&mut self.block_end, end);
        let result = body(self);
        self.block_end = outer_end;
        self.scopes.pop();
        result
    }

    // ---- statements ----

    fn statements(&mut self, stmts: &[Stmt]) -> Check<()> {
        stmts.iter().try_for_each(|stmt| self.statement(stmt))
    }

    fn statement(&mut self, stmt: &Stmt) -> Check<()> {
        match &stmt.kind {
            StmtKind::Alphabet { name, symbols } => {
                let mut seen = Vec::new();
                for &(c, span) in symbols {
                    if seen.contains(&c) {
                        return Err(Diag::new(
                            ErrorKind::Alphabet,
                            span,
                            format!("symbol `{c}` is listed twice"),
                        ));
                    }
                    seen.push(c);
                }
                seen.sort();
                let detail = format!("alphabet {} = {}", name.text, crate::automata::fmt_alphabet(&seen));
                self.define(name, Type::Alphabet, Some(seen), DefKind::Alphabet, detail)
            }
            StmtKind::MachineDef { name, machine } => {
                let ty = self.machine(machine, Some(&name.text))?;
                let detail = format!("{} {} over {}", machine.kind.keyword(), name.text, machine.alphabet.text);
                self.define(name, ty, None, DefKind::Machine, detail)
            }
            StmtKind::GrammarDef { name, grammar } => {
                let ty = self.grammar(grammar, Some(&name.text))?;
                let detail = format!("grammar {} over {}", name.text, grammar.alphabet.text);
                self.define(name, ty, None, DefKind::Grammar, detail)
            }
            StmtKind::Let { name, value } => {
                let ty = self.expr(value)?;
                // `let A = B` keeps what we know about alphabet B.
                let alphabet = match &value.kind {
                    ExprKind::Name(other) => self.lookup(other, value.span)?.alphabet,
                    _ => None,
                };
                self.define_value(name, ty, DefKind::Variable, alphabet)
            }
            StmtKind::Fn(f) => self.function(f, stmt.span.end),
            StmtKind::Return(value) => {
                let Some(expected) = self.ret.clone() else {
                    return Err(Diag::new(
                        ErrorKind::Syntax,
                        stmt.span,
                        "`return` can only be used inside a function",
                    ));
                };
                let actual = self.expr(value)?;
                if !fits(&expected, &actual) {
                    return Err(type_error(
                        value.span,
                        format!(
                            "this function returns {}, but this value is {}",
                            article(&expected),
                            article(&actual)
                        ),
                    ));
                }
                Ok(())
            }
            StmtKind::If { cond, then, otherwise } => {
                self.expect(cond, &Type::Bool, "the condition of `if`")?;
                self.scoped(stmt.span.end, |c| c.statements(then))?;
                match otherwise {
                    Some(block) => self.scoped(stmt.span.end, |c| c.statements(block)),
                    None => Ok(()),
                }
            }
            StmtKind::For { var, items, body } => {
                let element = self.element_type(items)?;
                self.scoped(stmt.span.end, |c| {
                    c.define_value(var, element, DefKind::LoopVariable, None)?;
                    c.statements(body)
                })
            }
            StmtKind::Print(values) => values.iter().try_for_each(|v| self.expr(v).map(drop)),
            StmtKind::Test(cond) => self.expect(cond, &Type::Bool, "a `test`"),
            StmtKind::Draw(value) => {
                let ty = self.expr(value)?;
                if !is_lang(&ty) && !matches!(ty, Type::Pda | Type::Tm) {
                    let mut err = type_error(
                        value.span,
                        format!("`draw` needs a machine or regex, but this is {}", article(&ty)),
                    );
                    if ty == Type::Grammar {
                        err = err.hint("a grammar has no states to draw; `draw to_pda(g)` draws its PDA");
                    }
                    return Err(err);
                }
                Ok(())
            }
            StmtKind::Trace { machine, word } => {
                let ty = self.expr(machine)?;
                if !matches!(ty, Type::Dfa | Type::Nfa | Type::Pda | Type::Tm) {
                    let mut err = type_error(
                        machine.span,
                        format!("`trace` needs a machine (Dfa, Nfa, Pda or Tm), but this is {}", article(&ty)),
                    );
                    if is_lang(&ty) {
                        err = err.hint("a regex or set has no states to show; convert it first, for example `trace to_dfa(r) on \"01\"`");
                    } else if ty == Type::Grammar {
                        err = err.hint("to see how a grammar makes a word, use `print derive(g, \"...\")`; or trace its PDA: `trace to_pda(g) on \"...\"`");
                    }
                    return Err(err);
                }
                let word_ty = self.expr(word)?;
                if word_ty != Type::Str {
                    return Err(type_error(
                        word.span,
                        format!("the word to trace must be a Str, but this is {}", article(&word_ty)),
                    )
                    .hint("write the word in quotes: `trace M on \"0101\"`"));
                }
                Ok(())
            }
        }
    }

    fn function(&mut self, f: &FnDef, end: usize) -> Check<()> {
        // A function body sees its parameters and the globals defined so far,
        // but not the local variables of whoever calls it.
        let globals = self.scopes[0].clone();
        let outer = std::mem::replace(&mut self.scopes, vec![globals, HashMap::new()]);
        let outer_end = std::mem::replace(&mut self.block_end, end);
        self.ret = Some(f.ret.clone());
        if let Some(signature) = self.funcs.get(&f.name.text) {
            self.index.add_ref(f.name.span, signature.def);
        }
        let result = f
            .params
            .iter()
            .try_for_each(|(name, ty)| self.define_value(name, ty.clone(), DefKind::Parameter, None))
            .and_then(|()| self.statements(&f.body));
        self.ret = None;
        self.block_end = outer_end;
        self.scopes = outer;
        result?;
        if !always_returns(&f.body) {
            return Err(type_error(
                f.name.span,
                format!("function `{}` can reach its end without returning a value", f.name.text),
            )
            .hint(format!("every path must end in `return`; it should return {}", article(&f.ret))));
        }
        Ok(())
    }

    // ---- machines ----

    fn machine(&mut self, m: &MachineExpr, name: Option<&str>) -> Check<Type> {
        let alphabet = self.alphabet_of(&m.alphabet)?;
        self.index_states(m, name);
        self.index.bodies.push(Body { span: m.body, symbols: alphabet.clone().unwrap_or_default() });
        let literal = self.scoped(m.body.end, |c| c.machine_items(&m.items))?;
        // A machine written out in full can be validated right now. One built
        // with loops or `q[i]` has to wait until it runs.
        if let (true, Some(symbols)) = (literal, alphabet) {
            let mut spec = MachineSpec {
                kind: m.kind,
                name: name.map(str::to_string),
                alphabet: symbols,
                alphabet_name: m.alphabet.text.clone(),
                header: m.header,
                starts: Vec::new(),
                accepts: Vec::new(),
                rejects: Vec::new(),
                transitions: Vec::new(),
            };
            for item in &m.items {
                match item {
                    MachineItem::Start(s) => spec.starts.push((s.name.clone(), s.span)),
                    MachineItem::Accept(states) => {
                        spec.accepts.extend(states.iter().map(|s| (s.name.clone(), s.span)))
                    }
                    MachineItem::Reject(states) => {
                        spec.rejects.extend(states.iter().map(|s| (s.name.clone(), s.span)))
                    }
                    MachineItem::Transition { from, action, to, span } => {
                        spec.transitions.push(Transition {
                            from: from.name.clone(),
                            action: action.clone(),
                            to: to.name.clone(),
                            span: *span,
                        })
                    }
                    MachineItem::For { .. } => unreachable!("not literal"),
                }
            }
            machine::build(&spec)?;
        }
        Ok(m.kind.ty())
    }

    /// Records the states of a machine in the index: the first mention of
    /// each state is its definition, later mentions are uses. States are only
    /// visible inside the machine's body. (`q[i]` names a family of states
    /// that only exists when the machine runs, so it is left out.)
    fn index_states(&mut self, m: &MachineExpr, name: Option<&str>) {
        fn mentions<'a>(items: &'a [MachineItem], out: &mut Vec<(&'a StateRef, &'static str)>) {
            for item in items {
                match item {
                    MachineItem::Start(s) => out.push((s, "start")),
                    MachineItem::Accept(states) => out.extend(states.iter().map(|s| (s, "accepting"))),
                    MachineItem::Reject(states) => out.extend(states.iter().map(|s| (s, "rejecting"))),
                    MachineItem::Transition { from, to, .. } => out.extend([(from, ""), (to, "")]),
                    MachineItem::For { body, .. } => mentions(body, out),
                }
            }
        }
        let mut all = Vec::new();
        mentions(&m.items, &mut all);
        let owner = match name {
            Some(name) => format!("{} `{name}`", m.kind.title()),
            None => format!("this {}", m.kind.title()),
        };
        let mut defs: Vec<(String, usize, Vec<&str>)> = Vec::new();
        for (state, role) in all.into_iter().filter(|(s, _)| s.index.is_none()) {
            match defs.iter_mut().find(|(n, _, _)| *n == state.name) {
                Some((_, def, roles)) => {
                    self.index.add_ref(state.span, *def);
                    if !role.is_empty() && !roles.contains(&role) {
                        roles.push(role);
                    }
                }
                None => {
                    let def = self.index.add_def(Def {
                        name: state.name.clone(),
                        kind: DefKind::State,
                        ty: None,
                        span: state.span,
                        visible: (m.body.start, m.body.end),
                        detail: String::new(),
                    });
                    defs.push((state.name.clone(), def, if role.is_empty() { vec![] } else { vec![role] }));
                }
            }
        }
        for (state, def, roles) in defs {
            let roles = if roles.is_empty() { String::new() } else { format!(" ({})", roles.join(", ")) };
            self.index.defs[def].detail = format!("state {state} of {owner}{roles}");
        }
    }

    /// Records a grammar's variables in the index, like `index_states`.
    fn index_variables(&mut self, g: &GrammarExpr, name: Option<&str>) {
        let owner = name.map_or("this grammar".to_string(), |n| format!("grammar `{n}`"));
        let mut defs: Vec<(String, usize)> = Vec::new();
        for (i, rule) in g.rules.iter().enumerate() {
            match defs.iter().find(|(n, _)| *n == rule.head.text) {
                Some(&(_, def)) => self.index.add_ref(rule.head.span, def),
                None => {
                    let start = if i == 0 { " (the start variable)" } else { "" };
                    let def = self.index.add_def(Def {
                        name: rule.head.text.clone(),
                        kind: DefKind::GrammarVariable,
                        ty: None,
                        span: rule.head.span,
                        visible: (g.body.start, g.body.end),
                        detail: format!("variable {} of {owner}{start}", rule.head.text),
                    });
                    defs.push((rule.head.text.clone(), def));
                }
            }
        }
        for rule in &g.rules {
            for sym in rule.alternatives.iter().flatten() {
                if let GSymKind::Variable(v) = &sym.kind {
                    if let Some(&(_, def)) = defs.iter().find(|(n, _)| n == v) {
                        self.index.add_ref(sym.span, def);
                    }
                }
            }
        }
    }

    /// Checks a grammar. Like a machine, it is validated now if its
    /// alphabet is known; otherwise when it is built while running.
    fn grammar(&mut self, g: &GrammarExpr, name: Option<&str>) -> Check<Type> {
        let alphabet = self.alphabet_of(&g.alphabet)?;
        self.index_variables(g, name);
        self.index.bodies.push(Body { span: g.body, symbols: alphabet.clone().unwrap_or_default() });
        if let Some(symbols) = alphabet {
            machine::build_grammar(&GrammarSpec {
                name: name.map(str::to_string),
                alphabet: symbols,
                alphabet_name: g.alphabet.text.clone(),
                header: g.header,
                rules: &g.rules,
            })?;
        }
        Ok(Type::Grammar)
    }

    /// Checks that `name` (after `over`) is an alphabet, and returns its
    /// symbols if they are known before running.
    fn alphabet_of(&mut self, name: &Name) -> Check<Option<Vec<char>>> {
        let alphabet = self.lookup(&name.text, name.span)?;
        if alphabet.ty != Type::Alphabet {
            return Err(type_error(
                name.span,
                format!("`{}` is {}, not an Alphabet", name.text, article(&alphabet.ty)),
            )
            .hint("declare one with `alphabet B = {0, 1}`"));
        }
        Ok(alphabet.alphabet)
    }

    /// Checks the items and reports whether they are all written out
    /// literally (no loops, no computed state names).
    fn machine_items(&mut self, items: &[MachineItem]) -> Check<bool> {
        let mut literal = true;
        for item in items {
            match item {
                MachineItem::Start(state) => literal &= self.state(state)?,
                MachineItem::Accept(states) | MachineItem::Reject(states) => {
                    for state in states {
                        literal &= self.state(state)?;
                    }
                }
                MachineItem::Transition { from, to, .. } => {
                    literal &= self.state(from)?;
                    literal &= self.state(to)?;
                }
                MachineItem::For { var, items, body } => {
                    let element = self.element_type(items)?;
                    let end = self.block_end;
                    self.scoped(end, |c| {
                        c.define_value(var, element, DefKind::LoopVariable, None)?;
                        c.machine_items(body)
                    })?;
                    literal = false;
                }
            }
        }
        Ok(literal)
    }

    fn state(&mut self, state: &StateRef) -> Check<bool> {
        match &state.index {
            None => Ok(true),
            Some(index) => {
                self.expect(index, &Type::Int, "a state index")?;
                Ok(false)
            }
        }
    }

    // ---- expressions ----

    /// Checks that `expr` has type `wanted`; `what` names it in the message.
    fn expect(&mut self, expr: &Expr, wanted: &Type, what: &str) -> Check<()> {
        let actual = self.expr(expr)?;
        if !fits(wanted, &actual) {
            return Err(type_error(
                expr.span,
                format!("{what} must be {}, but this is {}", article(wanted), article(&actual)),
            ));
        }
        Ok(())
    }

    /// The type of the values a `for` loop over `items` produces.
    fn element_type(&mut self, items: &Expr) -> Check<Type> {
        match self.expr(items)? {
            Type::Set(inner) => Ok(*inner),
            Type::Str | Type::Alphabet | Type::EmptySet => Ok(Type::Str),
            other => Err(type_error(
                items.span,
                format!("cannot loop over {}", article(&other)),
            )
            .hint("`for` works on a Set, a range like `0..n`, a Str, or an Alphabet")),
        }
    }

    /// The type of `expr`, which is also recorded in the index.
    fn expr(&mut self, expr: &Expr) -> Check<Type> {
        let ty = self.expr_type(expr)?;
        self.index.exprs.push((expr.span, ty.clone()));
        Ok(ty)
    }

    fn expr_type(&mut self, expr: &Expr) -> Check<Type> {
        match &expr.kind {
            ExprKind::Int(_) => Ok(Type::Int),
            ExprKind::Str(_) => Ok(Type::Str),
            ExprKind::Bool(_) => Ok(Type::Bool),
            ExprKind::Regex(_) => Ok(Type::Regex),
            ExprKind::Name(name) => Ok(self.lookup(name, expr.span)?.ty),
            ExprKind::Paren(inner) => self.expr(inner),
            ExprKind::Machine(m) => self.machine(m, None),
            ExprKind::Grammar(g) => self.grammar(g, None),
            ExprKind::Set(items) => {
                let Some(first) = items.first() else { return Ok(Type::EmptySet) };
                let element = self.expr(first)?;
                if !matches!(element, Type::Int | Type::Str | Type::Bool) {
                    return Err(type_error(
                        first.span,
                        format!("a set can hold Int, Str or Bool values, not {}", article(&element)),
                    ));
                }
                for item in &items[1..] {
                    let ty = self.expr(item)?;
                    if ty != element {
                        return Err(type_error(
                            item.span,
                            format!(
                                "every item of a set must have the same type: the first is {}, but this is {}",
                                article(&element),
                                article(&ty)
                            ),
                        ));
                    }
                }
                Ok(Type::Set(Box::new(element)))
            }
            ExprKind::Unary(op, inner) => {
                let (wanted, what) = match op {
                    UnOp::Neg => (Type::Int, "the operand of `-`"),
                    UnOp::Not => (Type::Bool, "the operand of `not`"),
                };
                self.expect(inner, &wanted, what)?;
                Ok(wanted)
            }
            ExprKind::Binary(op, left, right) => self.binary(*op, left, right, expr.span),
            ExprKind::Call { name, args } => self.call(name, args, expr.span),
        }
    }

    fn binary(&mut self, op: BinOp, left: &Expr, right: &Expr, span: Span) -> Check<Type> {
        let (l, r) = (self.expr(left)?, self.expr(right)?);
        let mismatch = |wanted: &str| {
            type_error(
                span,
                format!(
                    "`{}` needs {wanted}, but the left side is {} and the right side is {}",
                    op.symbol(),
                    article(&l),
                    article(&r)
                ),
            )
        };
        match op {
            BinOp::Add => match (&l, &r) {
                (Type::Int, Type::Int) => Ok(Type::Int),
                (Type::Str, Type::Str) => Ok(Type::Str),
                _ => Err(mismatch("two Ints or two Strs")),
            },
            BinOp::Sub | BinOp::Mul | BinOp::Mod => match (&l, &r) {
                (Type::Int, Type::Int) => Ok(Type::Int),
                _ => Err(mismatch("two Ints")),
            },
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => match (&l, &r) {
                (Type::Int, Type::Int) => Ok(Type::Bool),
                _ => Err(mismatch("two Ints")),
            },
            BinOp::And | BinOp::Or => match (&l, &r) {
                (Type::Bool, Type::Bool) => Ok(Type::Bool),
                _ => Err(mismatch("two Bools")),
            },
            BinOp::Range => match (&l, &r) {
                (Type::Int, Type::Int) => Ok(Type::Set(Box::new(Type::Int))),
                _ => Err(mismatch("two Ints")),
            },
            BinOp::Eq | BinOp::NotEq => {
                if matches!(l, Type::Dfa | Type::Nfa | Type::Pda | Type::Tm | Type::Regex | Type::Grammar) {
                    return Err(type_error(
                        span,
                        format!("`{}` cannot compare machines or regexes", op.symbol()),
                    )
                    .hint("to ask whether two of them describe the same language, use `equivalent(a, b)`"));
                }
                if fits(&l, &r) || fits(&r, &l) {
                    Ok(Type::Bool)
                } else {
                    Err(mismatch("two values of the same type"))
                }
            }
            BinOp::Accepts => {
                if !is_lang(&l) && !is_context_free(&l) && l != Type::Tm {
                    return Err(type_error(
                        left.span,
                        format!(
                            "the left side of `accepts` must be a language (Dfa, Nfa, Pda, Tm, Regex, Grammar or Set[Str]), but this is {}",
                            article(&l)
                        ),
                    ));
                }
                if r != Type::Str {
                    return Err(type_error(
                        right.span,
                        format!("the right side of `accepts` must be a Str, but this is {}", article(&r)),
                    )
                    .hint("write the word in quotes: `m accepts \"0101\"`"));
                }
                Ok(Type::Bool)
            }
        }
    }

    fn call(&mut self, name: &Name, args: &[Expr], span: Span) -> Check<Type> {
        self.index.calls.push(Call { name: name.text.clone(), name_span: name.span, span });
        if let Some(signature) = self.funcs.get(&name.text) {
            self.index.add_ref(name.span, signature.def);
        }
        let types = args.iter().map(|arg| self.expr(arg)).collect::<Check<Vec<Type>>>()?;

        if builtins::NAMES.contains(&name.text.as_str()) {
            return builtins::result_type(&name.text, &types).map_err(|e| {
                let err = type_error(e.arg.map_or(span, |i| args[i].span), e.message);
                match e.hint {
                    Some(hint) => err.hint(hint),
                    None => err,
                }
            });
        }

        let Some(signature) = self.funcs.get(&name.text) else {
            let mut err = Diag::new(
                ErrorKind::Name,
                name.span,
                format!("there is no function called `{}`", name.text),
            );
            let known = builtins::NAMES.iter().copied().chain(self.funcs.keys().map(String::as_str));
            if self.scopes.iter().any(|scope| scope.contains_key(&name.text)) {
                err = err.hint(format!("`{}` is a value, not a function", name.text));
            } else if let Some(close) = did_you_mean(&name.text, known) {
                err = err.hint(format!("did you mean `{close}`?"));
            }
            return Err(err);
        };
        if types.len() != signature.params.len() {
            let n = signature.params.len();
            return Err(type_error(
                span,
                format!(
                    "`{}` takes {n} argument{}, but {} {} given",
                    name.text,
                    if n == 1 { "" } else { "s" },
                    types.len(),
                    if types.len() == 1 { "was" } else { "were" }
                ),
            ));
        }
        for (i, (wanted, actual)) in signature.params.iter().zip(&types).enumerate() {
            if !fits(wanted, actual) {
                return Err(type_error(
                    args[i].span,
                    format!(
                        "argument {} of `{}` must be {}, but this is {}",
                        i + 1,
                        name.text,
                        article(wanted),
                        article(actual)
                    ),
                ));
            }
        }
        Ok(signature.ret.clone())
    }
}

/// Can a value of type `actual` be used where `wanted` is required?
fn fits(wanted: &Type, actual: &Type) -> bool {
    wanted == actual || (*actual == Type::EmptySet && matches!(wanted, Type::Set(_)))
}

/// True if running these statements always ends in a `return`.
fn always_returns(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|stmt| match &stmt.kind {
        StmtKind::Return(_) => true,
        StmtKind::If { then, otherwise: Some(otherwise), .. } => {
            always_returns(then) && always_returns(otherwise)
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::lexer::lex;
    use crate::lang::parser::Parser;

    fn check(src: &str) -> Check<()> {
        let program = Parser::new(lex(src, 0, 1)?, src, false).program()?;
        Checker::new(false).check(&program)
    }

    /// The error for `src`, as "Kind: message".
    fn error(src: &str) -> String {
        let d = check(src).expect_err("expected an error");
        format!("{}: {}", d.kind.name(), d.message)
    }

    const B: &str = "alphabet B = {0, 1}\n";

    #[test]
    fn accepts_a_good_program() {
        check(
            "alphabet B = {0, 1}
             dfa Even over B { start a  accept a  a --0--> b  a --1--> a  b --0--> a  b --1--> b }
             fn twice(n: Int) -> Int { return n * 2 }
             fn pick(flag: Bool) -> Dfa { if flag { return Even } else { return minimize(/0*/) } }
             let s = {\"0\", \"00\"}
             for w in s { test Even accepts w or len(w) == twice(1) }
             test equivalent(pick(true), Even) and not is_empty(s)
             print states(Even), 0..3, {} == s",
        )
        .unwrap();
    }

    #[test]
    fn rejects_bad_machines_before_running() {
        let dfa = |body: &str| error(&format!("{B}dfa M over B {{ {body} }}"));
        assert!(dfa("start a  a --0--> a").contains("not total: no transition from `a` on `1`"));
        assert!(dfa("start a  a --0,1--> a  a --0--> b  b --0,1--> b").contains("already goes to `a` on `0`"));
        assert!(dfa("start a  a --2--> a").starts_with("AlphabetError: symbol `2`"));
        assert!(dfa("start a  a --eps--> a").contains("cannot have eps"));
        assert!(dfa("a --0,1--> a").contains("no start state"));
        assert!(dfa("start a  start b  a --0,1--> b  b --0,1--> a").contains("already has the start state"));
        // The same body is a perfectly good NFA.
        check(&format!("{B}nfa M over B {{ start a  a --0--> a  a --0--> b  a --eps--> b }}")).unwrap();
    }

    #[test]
    fn type_errors() {
        assert!(error("let x = 1 + \"a\"").starts_with("TypeError: `+` needs"));
        assert!(error("let x = {1, \"a\"}").contains("same type"));
        assert!(error("test 1").contains("must be a Bool"));
        assert!(error("let x = /a/ == /a/").contains("cannot compare"));
        assert!(error("let x = 3 accepts \"a\"").contains("left side of `accepts`"));
        assert!(error("let x = /a/ accepts 3").contains("right side of `accepts`"));
        assert!(error("let x = minimize(3)").contains("argument 1 of `minimize`"));
        assert!(error("let x = union(/a/)").contains("takes 2 arguments, but 1 was given"));
        assert!(error("fn f(n: Int) -> Int { return n }\nlet x = f(true)").contains("argument 1 of `f` must be an Int"));
        assert!(error("fn f() -> Int { return true }").contains("returns an Int"));
        assert!(error("fn f(n: Int) -> Int { if n > 0 { return 1 } }").contains("without returning"));
        assert!(error("for x in 3 { print x }").contains("cannot loop over an Int"));
        assert!(error("let a = 1\ndfa M over a { start q }").contains("not an Alphabet"));
        assert!(error("trace /a/ on \"a\"").contains("`trace` needs a machine (Dfa, Nfa, Pda or Tm), but this is a Regex"));
        assert!(error("trace 1 on \"a\"").contains("but this is an Int"));
        assert!(error(&format!("{B}nfa M over B {{ start a }}\ntrace M on 1")).contains("word to trace must be a Str"));
        check(&format!("{B}nfa M over B {{ start a }}\ntrace M on \"01\"")).unwrap();
    }

    #[test]
    fn grammars_and_pdas() {
        const G: &str = "alphabet B = {0, 1}\ngrammar G over B { S -> 0 S 1 | eps }\npda P over B { start p  accept p  p --0, eps/A--> p }\n";
        let ok = |rest: &str| check(&format!("{G}{rest}")).unwrap();
        ok("test G accepts \"01\" and P accepts \"0\"");
        ok("let c = to_cnf(G)\nlet p = to_pda(c)\nprint states(p), is_empty(G), derive(G, \"01\")");
        ok("let g = to_grammar(/0*1/)\ntrace P on \"00\"\ndraw P");
        ok("fn f(g: Grammar) -> Pda { return to_pda(g) }\nprint f(grammar over B { S -> 0 })");
        let err = |rest: &str| error(&format!("{G}{rest}"));
        assert!(err("print equivalent(G, /0*/)").contains("cannot compare context-free languages"));
        assert!(err("print equivalent(/0*/, P)").contains("this is a Pda"));
        assert!(err("print minimize(G)").contains("must be a regular language"));
        assert!(err("print to_pda(P)").contains("must be a Grammar"));
        assert!(err("draw G").contains("`draw` needs a machine or regex"));
        assert!(err("trace G on \"0\"").contains("needs a machine (Dfa, Nfa, Pda or Tm)"));
        assert!(err("print G == G").contains("cannot compare machines"));
        assert!(error("alphabet B = {0}\ngrammar G over B { S -> T }").starts_with("NameError: variable `T` has no rules"));
        assert!(error("alphabet B = {0}\ngrammar G over B { S -> 1 }").starts_with("AlphabetError"));
        assert!(error("alphabet B = {0}\npda P over B { start p  p --1, Z/Z--> p }").starts_with("AlphabetError"));
        assert!(error("alphabet B = {0}\npda P over B { p --0, Z/Z--> p }").contains("PDA `P` has no start state"));
        let hint = check("alphabet B = {0}\nlet x = equivalent(grammar over B { S -> 0 }, /0/)").unwrap_err().hint.unwrap();
        assert!(hint.contains("undecidable"));
    }

    #[test]
    fn turing_machines() {
        const M: &str = "alphabet B = {0, 1}\ntm M over B { start s  accept y  s --0/0,R--> s  s --_/_,S--> y }\n";
        let ok = |rest: &str| check(&format!("{M}{rest}")).unwrap();
        ok("test M accepts \"00\"\nprint run(M, \"0\"), states(M)\ntrace M on \"0\"\ndraw M");
        ok("fn f(m: Tm) -> Str { return run(m, \"\") }\nprint f(M)");
        let err = |rest: &str| error(&format!("{M}{rest}"));
        assert!(err("print equivalent(M, /0*/)").contains("cannot compare Turing machines"));
        assert!(err("print run(M, 0)").contains("argument 2 of `run` must be a Str"));
        assert!(err("print run(/0/, \"0\")").contains("must be a Turing machine"));
        assert!(err("print minimize(M)").contains("must be a regular language"));
        assert!(error("alphabet B = {0}\ntm M over B { start s  s --0/0,R--> s  s --0/1,R--> s }").contains("already has a move"));
        assert!(error("alphabet B = {0}\ntm M over B { start s  accept s  reject s }").contains("both accept and reject"));
    }

    #[test]
    fn name_errors() {
        assert!(error("print y").contains("`y` is not defined"));
        assert!(error("let x = 1\nlet x = 2").contains("already defined"));
        assert!(error("let x = minimise(/a/)").contains("no function called"));
        assert!(error("return 1").contains("inside a function"));
        assert!(error("fn len(n: Int) -> Int { return n }").contains("built-in"));
        let hint = |src: &str| check(src).unwrap_err().hint.unwrap();
        assert_eq!(hint("let x = minimise(/a/)"), "did you mean `minimize`?");
        assert_eq!(hint("let count = 1\nprint cout"), "did you mean `count`?");
        assert_eq!(hint("print len"), "`len` is a function; call it as `len(...)`");
    }

    #[test]
    fn scoping() {
        // Inner blocks may reuse a name; a function cannot see a caller's locals.
        check("let x = 1\nif true { let x = 2  print x }").unwrap();
        assert!(error("fn f() -> Int { return y }\nif true { let y = 1  print f() }").contains("`y` is not defined"));
        // Functions may call functions defined later.
        check("fn a() -> Int { return b() }\nfn b() -> Int { return 1 }").unwrap();
    }
}

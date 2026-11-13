//! The interpreter: runs a checked program by walking its syntax tree.
//!
//! `exec` runs a statement, `eval` computes an expression. The call stack is
//! an explicit list of frames, which is also what a traceback is printed from.
//!
//! Every file has its own globals and functions (`ModuleState`), and every
//! frame remembers which file's code it is running, so a function always
//! sees the globals of the file it was written in. An imported module runs
//! *lazily*: its top level is run the first time one of its names is used,
//! in a frame of its own called `<module>`, so an error there shows the line
//! that first used it. Running a module is *quiet*: its top-level `print`,
//! `test`, `draw` and `trace` are skipped, since they are there for when the
//! file is run by itself.

use super::ast::*;
use super::codes;
use super::error::{Diag, ErrorKind, TraceFrame};
use super::machine::{self, Built, MachineSpec, Transition};
use super::source::SourceMap;
use super::token::{FileId, Span};
use super::value::Value;
use super::trace as show_trace;
use super::machine::GrammarSpec;
use crate::automata::grammar::{DeriveError, Grammar};
use crate::automata::pda::{self, Outcome, Pda};
use crate::automata::tm::{self, Halt, Tm};
use crate::automata::{convert, dot, fmt_alphabet, minimize, ops, trace, Dfa, Nfa};
use std::collections::{BTreeSet, HashMap};
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;

/// How deeply functions may call each other before we stop with an error.
const MAX_DEPTH: usize = 500;
/// How many sentential forms `derive` looks at before giving up.
const DERIVE_LIMIT: usize = 200_000;
/// The largest range `a..b` we are willing to build.
const MAX_RANGE: i64 = 1_000_000;
/// The largest regex `to_regex` builds, in nodes of its syntax tree.
const MAX_REGEX_SIZE: usize = 100_000;

/// One function call in progress, or the top level of a file.
struct Frame {
    func: String,
    /// Where, in the caller, this call was made.
    call_site: Span,
    /// The file whose code this is, which decides the globals it sees.
    module: FileId,
    /// Innermost scope last. Empty at the top level of a file, whose
    /// definitions go into the file's globals.
    scopes: Vec<HashMap<String, Value>>,
    /// The top level of an imported module: `print`, `test`, `draw` and
    /// `trace` are skipped.
    quiet: bool,
}

/// How far an imported module has run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Progress {
    /// Not yet: none of its names has been used.
    NotRun,
    Running,
    Done,
}

/// What one file has defined while running.
struct ModuleState {
    globals: HashMap<String, Value>,
    funcs: HashMap<String, Rc<FnDef>>,
    /// The modules its `import` statements bound, by name.
    aliases: HashMap<String, ModuleRef>,
    /// The names its `from ... import` statements took: the module and the
    /// name there.
    taken: HashMap<String, (ModuleRef, String)>,
    /// Its statements; the main file's arrive through `run` instead.
    program: Option<Rc<Vec<Stmt>>>,
    progress: Progress,
}

impl ModuleState {
    fn new(program: Option<Rc<Vec<Stmt>>>) -> ModuleState {
        let progress = if program.is_some() { Progress::NotRun } else { Progress::Done };
        ModuleState {
            globals: HashMap::new(),
            funcs: HashMap::new(),
            aliases: HashMap::new(),
            taken: HashMap::new(),
            program,
            progress,
        }
    }
}

/// A function about to be called, and the file it was written in.
struct Callee {
    module: FileId,
    def: Rc<FnDef>,
}

/// What happened when a statement ran.
enum Flow {
    Normal,
    Return(Value),
}

pub struct Interp {
    /// What each file has defined; the main file is `FileId::MAIN`.
    modules: HashMap<FileId, ModuleState>,
    /// `frames[0]` is the top level of the main file.
    frames: Vec<Frame>,
    /// Run each imported module at its `import` statement, instead of when
    /// one of its names is first used.
    pub eager_imports: bool,
    out: Box<dyn Write>,
    /// Where `draw` writes its files.
    out_dir: PathBuf,
    /// Whether `draw` should also try to make an SVG with Graphviz.
    pub render_svg: bool,
    /// Print only ASCII (`-` instead of an em dash) for consoles that
    /// cannot show anything else.
    pub ascii: bool,
    /// Print a line for every function call and return (`--trace-calls`).
    pub trace_calls: bool,
    pub passed: usize,
    pub failed: usize,
    drawings: usize,
    /// The program's files, only needed to quote a failing `test` line.
    sources: SourceMap,
}

type Run<T> = Result<T, Diag>;

impl Interp {
    pub fn new(out: Box<dyn Write>, out_dir: PathBuf) -> Interp {
        Interp {
            modules: HashMap::from([(FileId::MAIN, ModuleState::new(None))]),
            frames: vec![Frame {
                func: "<main>".into(),
                call_site: Span::new(0, 0, 1),
                module: FileId::MAIN,
                scopes: Vec::new(),
                quiet: false,
            }],
            eager_imports: false,
            out,
            out_dir,
            render_svg: false,
            ascii: false,
            trace_calls: false,
            passed: 0,
            failed: 0,
            drawings: 0,
            sources: SourceMap::default(),
        }
    }

    /// Sends what the program prints somewhere else from now on.
    pub fn set_output(&mut self, out: Box<dyn Write>) {
        self.out = out;
    }

    /// Makes a module that the loader has checked ready to run, the first
    /// time it is seen. It runs when one of its names is first used.
    pub fn add_module(&mut self, file: FileId, program: Rc<Vec<Stmt>>) {
        self.modules.entry(file).or_insert_with(|| ModuleState::new(Some(program)));
    }

    /// Runs a program (or, in the REPL, one more piece of it). `sources`
    /// holds the text of its files.
    pub fn run(&mut self, program: &[Stmt], sources: &SourceMap) -> Run<()> {
        self.sources = sources.clone();
        let result = self.block(program);
        // After an error, forget any calls and blocks that were in progress.
        self.frames.truncate(1);
        self.frames[0].scopes.clear();
        result.map(drop)
    }

    // ---- errors ----

    /// Builds an error at `span`, with the traceback of the calls in progress.
    fn error(&self, kind: ErrorKind, span: Span, message: impl Into<String>) -> Diag {
        self.located(Diag::new(kind, span, message))
    }

    /// Adds the traceback to an error. Frame `i` is shown at the place where
    /// it called frame `i + 1`; the innermost frame is shown at the error.
    fn located(&self, mut diag: Diag) -> Diag {
        let places = self.frames.iter().skip(1).map(|f| f.call_site).chain([diag.span]);
        diag.trace = self
            .frames
            .iter()
            .zip(places)
            .map(|(frame, span)| TraceFrame { func: frame.func.clone(), span })
            .collect();
        diag
    }

    // ---- variables ----

    fn scopes(&mut self) -> &mut Vec<HashMap<String, Value>> {
        &mut self.frames.last_mut().unwrap().scopes
    }

    /// The file whose code is running.
    fn current(&self) -> FileId {
        self.frames.last().unwrap().module
    }

    fn module(&mut self, file: FileId) -> &mut ModuleState {
        self.modules.get_mut(&file).expect("the loader registered every module")
    }

    /// Defines a name in the innermost scope, or, at the top level of a
    /// file, in that file's globals.
    fn define(&mut self, name: &str, value: Value) {
        let file = self.current();
        match self.scopes().last_mut() {
            Some(scope) => scope.insert(name.to_string(), value),
            None => self.module(file).globals.insert(name.to_string(), value),
        };
    }

    /// Looks in the current function's scopes, then in its file's globals,
    /// then among the names its file imported with `from`.
    fn lookup(&mut self, name: &str, span: Span) -> Run<Value> {
        if name.contains('.') {
            let (target, member) = self.qualified(name);
            return self.member(&target, &member, span);
        }
        let frame = self.frames.last().unwrap();
        let module = &self.modules[&frame.module];
        if let Some(value) = frame.scopes.iter().rev().find_map(|s| s.get(name)).or_else(|| module.globals.get(name)) {
            return Ok(value.clone());
        }
        if let Some((target, member)) = module.taken.get(name).cloned() {
            return self.member(&target, &member, span);
        }
        // The checker allows a function to use a global defined above it,
        // but the function may have been *called* before that line ran.
        Err(self
            .error(ErrorKind::Name, span, format!("`{name}` is used before it is defined"))
            .code(codes::USED_TOO_EARLY)
            .hint(format!("move the definition of `{name}` above the call that needs it")))
    }

    // ---- modules ----

    /// The module and the name inside it that a qualified name such as
    /// `utils.B` or `std.math.gcd` refers to. The checker has made sure
    /// that it exists.
    fn qualified(&self, name: &str) -> (ModuleRef, String) {
        let parts: Vec<&str> = name.split('.').collect();
        match self.modules[&self.current()].aliases.get(parts[0]) {
            Some(target) => (target.clone(), parts[1].to_string()),
            None => (ModuleRef::Std(parts[1].to_string()), parts[2].to_string()),
        }
    }

    /// The value called `member` in module `target`, running the module
    /// first if this is the first use of any of its names.
    fn member(&mut self, target: &ModuleRef, member: &str, span: Span) -> Run<Value> {
        let ModuleRef::File(file) = target else {
            unreachable!("the checker allows only functions from the standard library")
        };
        self.run_module(*file, span)?;
        match self.modules[file].globals.get(member) {
            Some(value) => Ok(value.clone()),
            None => Err(self
                .error(ErrorKind::Name, span, format!("`{member}` is used before it is defined"))
                .code(codes::USED_TOO_EARLY)
                .hint(format!("in its module, move the definition of `{member}` above the line that needs it"))),
        }
    }

    /// Runs the top level of a module, if it has not run yet. `use_site` is
    /// where it is needed: the line of first use, or with `--eager-imports`
    /// the import itself. A module runs at most once; if it fails, it is
    /// left as if it had never run (so the REPL can try again).
    fn run_module(&mut self, file: FileId, use_site: Span) -> Run<()> {
        let module = self.module(file);
        if module.progress != Progress::NotRun {
            return Ok(());
        }
        let program = module.program.clone().expect("a module has its statements");
        let depth = self.frames.len();
        // Modules whose top levels use each other in a long chain nest like
        // calls do, so they share the limit.
        if depth >= MAX_DEPTH {
            return Err(self
                .error(ErrorKind::Recursion, use_site, format!("more than {MAX_DEPTH} calls and modules are running at once"))
                .hint("a long chain of modules, each needing the next to run first; shorten the chain"));
        }
        self.module(file).progress = Progress::Running;
        self.frames.push(Frame { func: "<module>".into(), call_site: use_site, module: file, scopes: Vec::new(), quiet: true });
        let result = self.block(&program);
        self.frames.truncate(depth);
        let module = self.module(file);
        match result {
            Ok(_) => {
                module.progress = Progress::Done;
                Ok(())
            }
            Err(error) => {
                *module = ModuleState::new(module.program.clone());
                Err(error)
            }
        }
    }

    /// True at the top level of an imported module, where output is skipped.
    fn quiet(&self) -> bool {
        self.frames.last().unwrap().quiet
    }

    // ---- statements ----

    /// Runs statements in order, stopping early at a `return`.
    fn block(&mut self, stmts: &[Stmt]) -> Run<Flow> {
        for stmt in stmts {
            if let Flow::Return(value) = self.exec(stmt)? {
                return Ok(Flow::Return(value));
            }
        }
        Ok(Flow::Normal)
    }

    /// Runs statements inside a new scope with `bindings` defined in it.
    fn scoped_block(&mut self, stmts: &[Stmt], bindings: HashMap<String, Value>) -> Run<Flow> {
        self.scopes().push(bindings);
        let flow = self.block(stmts)?;
        self.scopes().pop();
        Ok(flow)
    }

    fn exec(&mut self, stmt: &Stmt) -> Run<Flow> {
        match &stmt.kind {
            StmtKind::Alphabet { name, symbols } => {
                let set: BTreeSet<char> = symbols.iter().map(|&(c, _)| c).collect();
                let value = Value::Alphabet(Rc::new(set.into_iter().collect()));
                self.define(&name.text, value);
            }
            StmtKind::MachineDef { name, machine } => {
                let value = self.machine(machine, Some(&name.text))?;
                self.define(&name.text, value);
            }
            StmtKind::GrammarDef { name, grammar } => {
                let value = self.grammar(grammar, Some(&name.text))?;
                self.define(&name.text, value);
            }
            StmtKind::Let { name, value } => {
                let value = self.eval(value)?;
                self.define(&name.text, value);
            }
            StmtKind::Fn(f) => {
                let file = self.current();
                self.module(file).funcs.insert(f.name.text.clone(), f.clone());
            }
            StmtKind::Import { lib, alias, target } => {
                let target = target.clone().expect("the loader found every module before running");
                let name = alias.as_ref().map_or_else(|| lib.default_name(), |a| a.text.clone());
                if let (ModuleRef::File(module), true) = (&target, self.eager_imports) {
                    self.run_module(*module, lib.span)?;
                }
                let file = self.current();
                self.module(file).aliases.insert(name, target);
            }
            StmtKind::FromImport { lib, names, target } => {
                let target = target.clone().expect("the loader found every module before running");
                if let (ModuleRef::File(module), true) = (&target, self.eager_imports) {
                    self.run_module(*module, lib.span)?;
                }
                let file = self.current();
                for (name, alias) in names {
                    let local = alias.as_ref().unwrap_or(name);
                    self.module(file).taken.insert(local.text.clone(), (target.clone(), name.text.clone()));
                }
            }
            StmtKind::Print(_) | StmtKind::Test(_) | StmtKind::Draw(_) | StmtKind::Trace { .. } if self.quiet() => {}
            StmtKind::Return(value) => return Ok(Flow::Return(self.eval(value)?)),
            StmtKind::If { cond, then, otherwise } => {
                if self.eval_bool(cond)? {
                    return self.scoped_block(then, HashMap::new());
                } else if let Some(otherwise) = otherwise {
                    return self.scoped_block(otherwise, HashMap::new());
                }
            }
            StmtKind::For { var, items, body } => {
                for item in self.items_of(items)? {
                    let bindings = HashMap::from([(var.text.clone(), item)]);
                    if let Flow::Return(value) = self.scoped_block(body, bindings)? {
                        return Ok(Flow::Return(value));
                    }
                }
            }
            StmtKind::Print(values) => {
                let mut parts = Vec::new();
                for value in values {
                    parts.push(self.eval(value)?.to_string());
                }
                let _ = writeln!(self.out, "{}", parts.join(" "));
            }
            StmtKind::Test(cond) => {
                if self.eval_bool(cond)? {
                    self.passed += 1;
                } else {
                    self.failed += 1;
                    let file = stmt.span.file;
                    let text = self.sources.text(file).get(stmt.span.start..stmt.span.end).unwrap_or("");
                    let first_line = text.lines().next().unwrap_or("");
                    let _ = writeln!(
                        self.out,
                        "test failed: {}, line {}\n    {}",
                        self.sources.name(file),
                        stmt.span.line,
                        first_line
                    );
                }
            }
            StmtKind::Draw(value) => self.draw(value)?,
            StmtKind::Trace { machine, word } => self.trace(machine, word)?,
        }
        Ok(Flow::Normal)
    }

    /// The values a `for` loop steps through.
    fn items_of(&mut self, items: &Expr) -> Run<Vec<Value>> {
        Ok(match self.eval(items)? {
            Value::Set(set) => set.iter().cloned().collect(),
            Value::Str(text) => text.chars().map(|c| Value::str(&c.to_string())).collect(),
            Value::Alphabet(symbols) => {
                symbols.iter().map(|c| Value::str(&c.to_string())).collect()
            }
            other => unreachable!("checker allowed a loop over {other:?}"),
        })
    }

    fn draw(&mut self, expr: &Expr) -> Run<()> {
        let dot = match self.eval(expr)? {
            Value::Dfa(dfa) => dot::dfa_to_dot(&dfa),
            Value::Nfa(nfa) => dot::nfa_to_dot(&nfa),
            Value::Pda(pda) => dot::pda_to_dot(&pda),
            Value::Tm(m) => dot::tm_to_dot(&m),
            other => dot::nfa_to_dot(&to_nfa(&other, &symbols_of(&other))),
        };
        // `draw d` writes d.dot (and `draw utils.d` too); anything more
        // complex gets a numbered name.
        let base = match &expr.kind {
            ExprKind::Name(name) => name.rsplit('.').next().unwrap_or(name).to_string(),
            _ => {
                self.drawings += 1;
                format!("machine{}", self.drawings)
            }
        };
        let path = self.out_dir.join(format!("{base}.dot"));
        std::fs::write(&path, dot).map_err(|e| {
            self.error(ErrorKind::Io, expr.span, format!("cannot write {}: {e}", path.display()))
        })?;
        let _ = writeln!(self.out, "wrote {base}.dot");
        if self.render_svg {
            let svg = self.out_dir.join(format!("{base}.svg"));
            let done = std::process::Command::new("dot")
                .arg("-Tsvg")
                .arg(&path)
                .arg("-o")
                .arg(&svg)
                .status()
                .is_ok_and(|status| status.success());
            if done {
                let _ = writeln!(self.out, "wrote {base}.svg");
            }
        }
        Ok(())
    }

    /// `trace machine on word`: runs the machine and prints every step.
    fn trace(&mut self, machine: &Expr, word: &Expr) -> Run<()> {
        let machine = self.eval(machine)?;
        let Value::Str(word) = self.eval(word)? else {
            unreachable!("checker ensures the word is a Str")
        };
        // Step 0 has no symbol and no "from" state. An em dash marks those
        // cells, unless the console cannot show one.
        let dash = if self.ascii { "-" } else { "\u{2014}" };
        let shown = match &machine {
            Value::Dfa(dfa) => show_trace::render_dfa(dfa, &word, &trace::trace_dfa(dfa, &word), dash),
            Value::Nfa(nfa) => show_trace::render_nfa(nfa, &word, &trace::trace_nfa(nfa, &word), dash),
            Value::Pda(pda) => show_trace::render_pda(pda, &word, &pda.search(&word, pda::STEP_LIMIT), dash),
            Value::Tm(m) => show_trace::render_tm(m, &word, &m.run(&word, show_trace::TM_TRACE_LIMIT, true), dash),
            other => unreachable!("checker allowed tracing {other:?}"),
        };
        // A blank line after each trace keeps several traces apart.
        let _ = writeln!(self.out, "{shown}\n");
        Ok(())
    }

    // ---- machines ----

    fn machine(&mut self, m: &MachineExpr, name: Option<&str>) -> Run<Value> {
        let Value::Alphabet(alphabet) = self.lookup(&m.alphabet.text, m.alphabet.span)? else {
            unreachable!("checker ensures `over` names an alphabet")
        };
        let mut spec = MachineSpec {
            kind: m.kind,
            name: name.map(str::to_string),
            alphabet: alphabet.to_vec(),
            alphabet_name: m.alphabet.text.clone(),
            header: m.header,
            starts: Vec::new(),
            accepts: Vec::new(),
            rejects: Vec::new(),
            transitions: Vec::new(),
        };
        self.machine_items(&m.items, &mut spec)?;
        Ok(match machine::build(&spec).map_err(|diag| self.located(diag))? {
            Built::Dfa(dfa) => Value::dfa(dfa),
            Built::Nfa(nfa) => Value::nfa(nfa),
            Built::Pda(pda) => Value::Pda(Rc::new(pda)),
            Built::Tm(m) => Value::Tm(Rc::new(m)),
        })
    }

    fn grammar(&mut self, g: &GrammarExpr, name: Option<&str>) -> Run<Value> {
        let Value::Alphabet(alphabet) = self.lookup(&g.alphabet.text, g.alphabet.span)? else {
            unreachable!("checker ensures `over` names an alphabet")
        };
        let spec = GrammarSpec {
            name: name.map(str::to_string),
            alphabet: alphabet.to_vec(),
            alphabet_name: g.alphabet.text.clone(),
            header: g.header,
            rules: &g.rules,
        };
        let grammar = machine::build_grammar(&spec).map_err(|diag| self.located(diag))?;
        Ok(Value::Grammar(Rc::new(grammar)))
    }

    /// Collects the items of a machine body into `spec`, running any loops.
    fn machine_items(&mut self, items: &[MachineItem], spec: &mut MachineSpec) -> Run<()> {
        for item in items {
            match item {
                MachineItem::Start(state) => spec.starts.push((self.state_name(state)?, state.span)),
                MachineItem::Accept(states) => {
                    for state in states {
                        spec.accepts.push((self.state_name(state)?, state.span));
                    }
                }
                MachineItem::Reject(states) => {
                    for state in states {
                        spec.rejects.push((self.state_name(state)?, state.span));
                    }
                }
                MachineItem::Transition { from, action, to, span } => {
                    spec.transitions.push(Transition {
                        from: self.state_name(from)?,
                        action: action.clone(),
                        to: self.state_name(to)?,
                        span: *span,
                        from_span: from.span,
                        to_span: to.span,
                    })
                }
                MachineItem::For { var, items, body } => {
                    for item in self.items_of(items)? {
                        self.scopes().push(HashMap::from([(var.text.clone(), item)]));
                        self.machine_items(body, spec)?;
                        self.scopes().pop();
                    }
                }
            }
        }
        Ok(())
    }

    /// `q0` is the state "q0"; `q[i]` is "q" followed by the value of `i`.
    fn state_name(&mut self, state: &StateRef) -> Run<String> {
        let Some(index) = &state.index else { return Ok(state.name.clone()) };
        match self.eval_int(index)? {
            i if i >= 0 => Ok(format!("{}{i}", state.name)),
            i => Err(self
                .error(ErrorKind::Machine, index.span, format!("state index is negative ({i})"))
                .code(codes::NEGATIVE_INDEX)
                .hint("`%` always gives a result from 0 upwards, so `(i - 1) % n` wraps around safely")),
        }
    }

    // ---- expressions ----

    fn eval_bool(&mut self, expr: &Expr) -> Run<bool> {
        match self.eval(expr)? {
            Value::Bool(b) => Ok(b),
            other => unreachable!("checker allowed {other:?} as a Bool"),
        }
    }

    fn eval_int(&mut self, expr: &Expr) -> Run<i64> {
        match self.eval(expr)? {
            Value::Int(n) => Ok(n),
            other => unreachable!("checker allowed {other:?} as an Int"),
        }
    }

    fn eval(&mut self, expr: &Expr) -> Run<Value> {
        Ok(match &expr.kind {
            ExprKind::Int(n) => Value::Int(*n),
            ExprKind::Str(s) => Value::str(s),
            ExprKind::Bool(b) => Value::Bool(*b),
            ExprKind::Regex(r) => Value::Regex(Rc::new(r.clone())),
            ExprKind::Name(name) => self.lookup(name, expr.span)?,
            ExprKind::Paren(inner) => self.eval(inner)?,
            ExprKind::Machine(m) => self.machine(m, None)?,
            ExprKind::Grammar(g) => self.grammar(g, None)?,
            ExprKind::Set(items) => {
                let mut set = BTreeSet::new();
                for item in items {
                    set.insert(self.eval(item)?);
                }
                Value::Set(Rc::new(set))
            }
            ExprKind::Unary(UnOp::Not, inner) => Value::Bool(!self.eval_bool(inner)?),
            ExprKind::Unary(UnOp::Neg, inner) => {
                let n = self.eval_int(inner)?;
                Value::Int(n.checked_neg().ok_or_else(|| self.overflow(expr.span))?)
            }
            ExprKind::Binary(op, left, right) => self.binary(*op, left, right, expr.span)?,
            ExprKind::Call { name, args } => {
                let mut values = Vec::new();
                for arg in args {
                    values.push(self.eval(arg)?);
                }
                if super::builtins::NAMES.contains(&name.text.as_str()) {
                    let spans: Vec<Span> = args.iter().map(|arg| arg.span).collect();
                    self.builtin(&name.text, &values, &spans)?
                } else {
                    self.call(name, values, expr.span)?
                }
            }
        })
    }

    fn overflow(&self, span: Span) -> Diag {
        self.error(ErrorKind::Math, span, "the result is too large for an Int")
            .hint("Ints are 64-bit: about plus or minus 9 * 10^18")
    }

    fn binary(&mut self, op: BinOp, left: &Expr, right: &Expr, span: Span) -> Run<Value> {
        // `and` / `or` only evaluate the right side when it matters.
        match op {
            BinOp::And => return Ok(Value::Bool(self.eval_bool(left)? && self.eval_bool(right)?)),
            BinOp::Or => return Ok(Value::Bool(self.eval_bool(left)? || self.eval_bool(right)?)),
            _ => {}
        }
        let (l, r) = (self.eval(left)?, self.eval(right)?);
        Ok(match (op, &l, &r) {
            (BinOp::Eq, _, _) => Value::Bool(l == r),
            (BinOp::NotEq, _, _) => Value::Bool(l != r),
            (BinOp::Add, Value::Str(a), Value::Str(b)) => Value::str(&format!("{a}{b}")),
            (BinOp::Accepts, _, Value::Str(word)) => Value::Bool(self.accepts(&l, word, right.span)?),
            (_, Value::Int(a), Value::Int(b)) => {
                let (a, b) = (*a, *b);
                let checked = |result: Option<i64>| result.ok_or_else(|| self.overflow(span));
                match op {
                    BinOp::Add => Value::Int(checked(a.checked_add(b))?),
                    BinOp::Sub => Value::Int(checked(a.checked_sub(b))?),
                    BinOp::Mul => Value::Int(checked(a.checked_mul(b))?),
                    BinOp::Mod => {
                        if b == 0 {
                            return Err(self
                                .error(ErrorKind::Math, span, "modulo by zero")
                                .code(codes::MODULO_ZERO)
                                .hint("the right-hand side of `%` is 0 here"));
                        }
                        // The result is never negative, like Python's % for a
                        // positive divisor, so it is always a valid state index.
                        Value::Int(checked(a.checked_rem_euclid(b))?)
                    }
                    BinOp::Lt => Value::Bool(a < b),
                    BinOp::Le => Value::Bool(a <= b),
                    BinOp::Gt => Value::Bool(a > b),
                    BinOp::Ge => Value::Bool(a >= b),
                    BinOp::Range => {
                        if b.saturating_sub(a) > MAX_RANGE {
                            return Err(self.error(
                                ErrorKind::Math,
                                span,
                                format!("the range {a}..{b} is too large (limit {MAX_RANGE} items)"),
                            )
                            .code(codes::RANGE_TOO_LARGE)
                            .hint("loop over a smaller range, or work with the language directly instead of listing its words"));
                        }
                        Value::Set(Rc::new((a..b).map(Value::Int).collect()))
                    }
                    _ => unreachable!("checker allowed {op:?} on Ints"),
                }
            }
            _ => unreachable!("checker allowed {op:?} on {l:?} and {r:?}"),
        })
    }

    /// Does the language `lang` contain `word`?
    fn accepts(&self, lang: &Value, word: &str, word_span: Span) -> Run<bool> {
        let result = match lang {
            Value::Dfa(dfa) => dfa.accepts(word).map_err(|c| (c, &dfa.alphabet)),
            Value::Nfa(nfa) => nfa.accepts(word).map_err(|c| (c, &nfa.alphabet)),
            Value::Set(words) => return Ok(words.contains(&Value::str(word))),
            Value::Grammar(g) => g.accepts(word).map_err(|c| (c, &g.alphabet)),
            Value::Pda(p) => {
                return match p.search(word, pda::STEP_LIMIT).outcome {
                    Outcome::Accepted => Ok(true),
                    Outcome::Rejected => Ok(false),
                    Outcome::Foreign { symbol, .. } => Err(self.foreign_word(symbol, &p.alphabet, word_span)),
                    Outcome::GaveUp => Err(self.pda_gave_up(word_span)),
                };
            }
            Value::Tm(m) => {
                let run = self.run_tm(m, word, word_span)?;
                return Ok(run.halt == Halt::Accepted);
            }
            // A regex has no fixed alphabet, so a foreign symbol just fails to match.
            regex => {
                let mut alphabet = symbols_of(regex);
                alphabet.extend(word.chars());
                alphabet.sort();
                alphabet.dedup();
                return Ok(to_nfa(regex, &alphabet).accepts(word) == Ok(true));
            }
        };
        result.map_err(|(c, alphabet)| self.foreign_word(c, alphabet, word_span))
    }

    fn foreign_word(&self, c: char, alphabet: &[char], word_span: Span) -> Diag {
        self.error(
            ErrorKind::Alphabet,
            word_span,
            format!("the word contains `{c}`, which is not in the machine's alphabet"),
        )
        .code(codes::FOREIGN_WORD)
        .hint(format!("the machine is over {}", fmt_alphabet(alphabet)))
    }

    /// Runs a Turing machine to the end, or reports why it could not.
    fn run_tm(&self, m: &Tm, word: &str, span: Span) -> Run<tm::Run> {
        let run = m.run(word, tm::STEP_LIMIT, false);
        match run.halt {
            Halt::Foreign { symbol, .. } => Err(self.foreign_word(symbol, &m.alphabet, span)),
            Halt::GaveUp => Err(self
                .error(
                    ErrorKind::Limit,
                    span,
                    format!("the Turing machine is still running after {} steps", tm::STEP_LIMIT),
                )
                .code(codes::STEP_LIMIT)
                .hint("it may never halt. By the halting problem, no program can decide in general whether a Turing machine halts, so Kleene Star stops here instead of waiting forever")),
            _ => Ok(run),
        }
    }

    fn pda_gave_up(&self, span: Span) -> Diag {
        self.error(
            ErrorKind::Limit,
            span,
            format!(
                "gave up after looking at {} configurations of the PDA without finding an accepting run",
                pda::STEP_LIMIT
            ),
        )
        .hint("eps moves that push without reading can let a PDA run forever; the answer is unknown, not \"no\"")
    }

    /// The function that `name` calls: one of the current file's, one taken
    /// from a module with `from`, or `module.name`. A module's functions are
    /// only there once the module has run, so this may run it.
    fn callee(&mut self, name: &Name) -> Run<Callee> {
        let current = self.current();
        let (target, member) = if name.is_qualified() {
            self.qualified(&name.text)
        } else {
            let module = &self.modules[&current];
            if let Some(def) = module.funcs.get(&name.text) {
                return Ok(Callee { module: current, def: def.clone() });
            }
            match module.taken.get(&name.text) {
                Some(taken) => taken.clone(),
                None => (ModuleRef::File(current), name.text.clone()),
            }
        };
        let ModuleRef::File(file) = target else {
            unreachable!("the standard library has no functions yet")
        };
        self.run_module(file, name.span)?;
        match self.modules[&file].funcs.get(&member) {
            Some(def) => Ok(Callee { module: file, def: def.clone() }),
            None => Err(self
                .error(
                    ErrorKind::Name,
                    name.span,
                    format!("function `{}` is called before it is defined", name.text),
                )
                .code(codes::USED_TOO_EARLY)
                .hint(format!("move `fn {member}` above this line"))),
        }
    }

    /// Calls a user-defined function.
    fn call(&mut self, name: &Name, args: Vec<Value>, call_site: Span) -> Run<Value> {
        let Callee { module, def: f } = self.callee(name)?;
        if self.frames.len() >= MAX_DEPTH {
            return Err(self
                .error(
                    ErrorKind::Recursion,
                    call_site,
                    format!("more than {MAX_DEPTH} calls are in progress at once"),
                )
                .hint(format!("does `{}` call itself without ever reaching a base case?", name.text)));
        }
        let indent = "  ".repeat(self.frames.len() - 1);
        if self.trace_calls {
            let shown: Vec<String> = f.params.iter().zip(&args).map(|((p, _), v)| format!("{}={}", p.text, v.brief())).collect();
            let _ = writeln!(self.out, "{indent}-> {}({})   line {}", name.text, shown.join(", "), call_site.line);
        }
        let params = f.params.iter().map(|(param, _)| param.text.clone()).zip(args).collect();
        self.frames.push(Frame { func: f.name.text.clone(), call_site, module, scopes: vec![params], quiet: false });
        let flow = self.block(&f.body)?;
        self.frames.pop();
        match flow {
            Flow::Return(value) => {
                if self.trace_calls {
                    let _ = writeln!(self.out, "{indent}<- {} returned {}", name.text, value.brief());
                }
                Ok(value)
            }
            Flow::Normal => unreachable!("checker ensures every function returns"),
        }
    }

    // ---- built-in functions ----

    fn builtin(&mut self, name: &str, args: &[Value], spans: &[Span]) -> Run<Value> {
        // The few built-ins that are not about languages.
        match (name, &args[0]) {
            ("len", Value::Str(s)) => return Ok(Value::Int(s.chars().count() as i64)),
            ("len", Value::Set(s)) => return Ok(Value::Int(s.len() as i64)),
            ("len", Value::Alphabet(a)) => return Ok(Value::Int(a.len() as i64)),
            ("states", Value::Dfa(d)) => return Ok(Value::Int(d.states.len() as i64)),
            ("states", Value::Nfa(n)) => return Ok(Value::Int(n.states.len() as i64)),
            ("rename", Value::Dfa(d)) => return Ok(Value::dfa(d.renamed())),
            ("rename", Value::Nfa(n)) => return Ok(Value::nfa(n.renamed())),
            ("states", Value::Pda(p)) => return Ok(Value::Int(p.states.len() as i64)),
            ("states", Value::Tm(m)) => return Ok(Value::Int(m.states.len() as i64)),
            ("run", Value::Tm(m)) => {
                let Value::Str(word) = &args[1] else { unreachable!("checker ensures a Str") };
                let run = self.run_tm(m, word, spans[1])?;
                return Ok(Value::str(&run.last().tape.contents()));
            }
            ("is_empty", Value::Grammar(g)) => return Ok(Value::Bool(g.is_empty())),
            ("to_cnf", Value::Grammar(g)) => return Ok(Value::Grammar(Rc::new(g.to_cnf()))),
            ("to_pda", Value::Grammar(g)) => return Ok(Value::Pda(Rc::new(Pda::from_grammar(g)))),
            ("derive", Value::Grammar(g)) => {
                let Value::Str(word) = &args[1] else { unreachable!("checker ensures a Str") };
                return self.derive(g, word, spans[1]);
            }
            _ => {}
        }

        // Everything else works on languages. First agree on an alphabet,
        // then convert each argument to the kind of machine the algorithm needs.
        let (langs, explicit) = match args {
            [lang, Value::Alphabet(alphabet)] => (std::slice::from_ref(lang), Some(alphabet)),
            _ => (args, None),
        };
        let alphabet = self.common_alphabet(langs, spans, explicit.map(|a| a.as_slice()))?;
        let dfa = |i: usize| to_dfa(&args[i], &alphabet);
        let nfa = |i: usize| to_nfa(&args[i], &alphabet);
        Ok(match name {
            "to_dfa" => Value::dfa(dfa(0)),
            "to_nfa" => Value::nfa(nfa(0)),
            "to_regex" => match &args[0] {
                Value::Regex(_) => args[0].clone(),
                _ => match convert::dfa_to_regex_within(&dfa(0), MAX_REGEX_SIZE) {
                    Some(regex) => Value::Regex(Rc::new(regex)),
                    None => {
                        return Err(self
                            .error(
                                ErrorKind::Limit,
                                spans[0],
                                format!("the regex for this language would be too large (over {MAX_REGEX_SIZE} parts)"),
                            )
                            .code(codes::TOO_LARGE)
                            .hint("some languages only have very long regexes; keep working with the machine instead"));
                    }
                },
            },
            "minimize" => Value::dfa(minimize::minimize(&dfa(0))),
            "reduce" => Value::nfa(minimize::reduce(&nfa(0))),
            "remove_eps" => Value::nfa(convert::remove_eps(&nfa(0))),
            "union" => Value::dfa(ops::union(&dfa(0), &dfa(1))),
            "intersect" => Value::dfa(ops::intersect(&dfa(0), &dfa(1))),
            "difference" => Value::dfa(ops::difference(&dfa(0), &dfa(1))),
            "complement" => Value::dfa(ops::complement(&dfa(0))),
            "concat" => Value::nfa(ops::concat(&nfa(0), &nfa(1))),
            "star" => Value::nfa(ops::star(&nfa(0))),
            "reverse" => Value::nfa(ops::reverse(&nfa(0))),
            "equivalent" => Value::Bool(ops::equivalent(&dfa(0), &dfa(1))),
            "is_empty" => Value::Bool(ops::is_empty(&dfa(0))),
            "to_grammar" => Value::Grammar(Rc::new(Grammar::from_dfa(&minimize::minimize(&dfa(0))))),
            other => unreachable!("checker allowed unknown built-in {other}"),
        })
    }

    /// `derive(g, word)`: a leftmost derivation, one sentential form per line.
    fn derive(&self, g: &Grammar, word: &str, span: Span) -> Run<Value> {
        if let Some(c) = word.chars().find(|c| !g.alphabet.contains(c)) {
            return Err(self.foreign_word(c, &g.alphabet, span));
        }
        match g.derive(word, DERIVE_LIMIT) {
            Ok(steps) => {
                let lines: Vec<String> = steps
                    .iter()
                    .enumerate()
                    .map(|(i, form)| format!("{}{}", if i == 0 { "" } else { "=> " }, g.show(form)))
                    .collect();
                Ok(Value::str(&lines.join("\n")))
            }
            Err(DeriveError::NotInLanguage) => {
                let shown = if word.is_empty() { "the empty string".to_string() } else { format!("\"{word}\"") };
                Err(self
                    .error(ErrorKind::Value, span, format!("the grammar cannot derive {shown}"))
                    .hint("check first with `g accepts \"...\"`; only words in the language have a derivation"))
            }
            Err(DeriveError::TooManySteps(n)) => Err(self
                .error(ErrorKind::Limit, span, format!("gave up looking for a derivation after {n} sentential forms"))
                .code(codes::GAVE_UP)
                .hint("the word is in the language, but its derivation is too long to find this way")),
        }
    }

    /// Picks the alphabet an operation works over.
    ///
    /// Machines have a fixed alphabet, and all machines involved must agree.
    /// Regexes and finite sets are flexible: they adopt the machines' alphabet
    /// (or the explicit one), or failing that, the symbols they mention.
    fn common_alphabet(
        &self,
        langs: &[Value],
        spans: &[Span],
        explicit: Option<&[char]>,
    ) -> Run<Vec<char>> {
        let mut fixed: Option<Vec<char>> = explicit.map(<[char]>::to_vec);
        for (lang, &span) in langs.iter().zip(spans) {
            let own = match lang {
                Value::Dfa(dfa) => &dfa.alphabet,
                Value::Nfa(nfa) => &nfa.alphabet,
                _ => continue,
            };
            match &fixed {
                Some(other) if other != own => {
                    return Err(self
                        .error(
                            ErrorKind::Alphabet,
                            span,
                            format!(
                                "this machine is over {}, but {} is needed here",
                                fmt_alphabet(own),
                                fmt_alphabet(other)
                            ),
                        )
                        .code(codes::ALPHABET_MISMATCH)
                        .hint("languages can only be combined when they share an alphabet; build both machines `over` the same one"));
                }
                _ => fixed = Some(own.clone()),
            }
        }
        let alphabet = fixed.clone().unwrap_or_else(|| {
            let all: BTreeSet<char> = langs.iter().flat_map(symbols_of).collect();
            all.into_iter().collect()
        });
        for (lang, &span) in langs.iter().zip(spans) {
            if matches!(lang, Value::Dfa(_) | Value::Nfa(_)) {
                continue;
            }
            if let Some(c) = symbols_of(lang).into_iter().find(|c| !alphabet.contains(c)) {
                return Err(self
                    .error(
                        ErrorKind::Alphabet,
                        span,
                        format!("this uses the symbol `{c}`, which is not in the alphabet {}", fmt_alphabet(&alphabet)),
                    )
                    .hint("every symbol must belong to the alphabet being used"));
            }
        }
        Ok(alphabet)
    }
}

/// The symbols a regex or a finite set of words mentions, sorted.
fn symbols_of(lang: &Value) -> Vec<char> {
    let mut symbols = BTreeSet::new();
    match lang {
        Value::Regex(regex) => regex.symbols(&mut symbols),
        Value::Set(words) => {
            for word in words.iter() {
                if let Value::Str(word) = word {
                    symbols.extend(word.chars());
                }
            }
        }
        Value::Dfa(dfa) => symbols.extend(&dfa.alphabet),
        Value::Nfa(nfa) => symbols.extend(&nfa.alphabet),
        _ => {}
    }
    symbols.into_iter().collect()
}

/// Any language value as an NFA over `alphabet`.
fn to_nfa(lang: &Value, alphabet: &[char]) -> Nfa {
    match lang {
        Value::Nfa(nfa) => (**nfa).clone(),
        Value::Dfa(dfa) => convert::dfa_to_nfa(dfa),
        Value::Regex(regex) => convert::regex_to_nfa(regex, alphabet),
        Value::Set(words) => {
            let words = words.iter().filter_map(|w| match w {
                Value::Str(word) => Some(&**word),
                _ => None,
            });
            convert::regex_to_nfa(&convert::finite_to_regex(words), alphabet)
        }
        other => unreachable!("checker allowed {other:?} as a language"),
    }
}

/// Any language value as a DFA over `alphabet`.
fn to_dfa(lang: &Value, alphabet: &[char]) -> Dfa {
    match lang {
        Value::Dfa(dfa) => (**dfa).clone(),
        other => convert::nfa_to_dfa(&to_nfa(other, alphabet)),
    }
}

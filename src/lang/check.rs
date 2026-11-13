//! The checker: finds mistakes before the program runs.
//!
//! It walks the syntax tree once, working out the type of every expression
//! and keeping a table of the names in scope. Anything it accepts cannot
//! fail later with a type or name error, which is what lets the interpreter
//! stay simple.

use super::ast::*;
use super::builtins::{self, article, is_context_free, is_lang};
use super::codes;
use super::error::{did_you_mean, Diag, ErrorKind};
use super::index::{Body, Call, Def, DefKind, Index, Namespace};
use super::machine::{self, GrammarSpec, MachineSpec, Transition};
use super::module::{ExportedFn, ExportedValue, Exports};
use super::token::{FileId, Span};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

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

/// A module bound to a name in this file by `import`.
#[derive(Clone)]
struct Alias {
    target: ModuleRef,
    /// Its entry in the index.
    def: usize,
}

/// Something a module offers.
enum Member {
    Value(ExportedValue),
    Function(ExportedFn),
}

impl Member {
    fn def(&self) -> &Def {
        match self {
            Member::Value(v) => &v.def,
            Member::Function(f) => &f.def,
        }
    }
}

#[derive(Clone)]
pub struct Checker {
    /// The file being checked: the main file, or a module being loaded.
    file: FileId,
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
    /// The errors found so far. Checking goes on after an error, one
    /// statement at a time, so that a single run reports them all.
    errors: Vec<Diag>,
    /// Names whose definition had an error. Using one is not reported
    /// again: the real mistake is in the definition.
    broken: HashSet<String>,
    /// Warnings about machines, found while checking them.
    machine_warnings: Vec<Diag>,
    /// What every module loaded so far offers.
    modules: HashMap<ModuleRef, Rc<Exports>>,
    /// The modules this file binds to a name with `import`.
    aliases: HashMap<String, Alias>,
    /// The names this file took from modules with `from ... import`, each
    /// with the statement that imports it. They are not passed on to files
    /// that import this one.
    imported: HashMap<String, String>,
    /// Definitions in other files that this file's index has a copy of, by
    /// where they are, so each is copied once.
    mirrored: HashMap<(FileId, usize, String), usize>,
}

type Check<T> = Result<T, Diag>;

/// The code of an error that only follows from an earlier one, such as using
/// a name whose definition failed. Such errors are dropped, not reported.
const CONSEQUENCE: &str = "consequence";

fn type_error(span: Span, message: impl Into<String>) -> Diag {
    Diag::new(ErrorKind::Type, span, message)
}

impl Checker {
    pub fn new(repl: bool) -> Checker {
        Checker {
            file: FileId::MAIN,
            scopes: vec![HashMap::new()],
            funcs: HashMap::new(),
            ret: None,
            repl,
            block_end: usize::MAX,
            index: Index::default(),
            errors: Vec::new(),
            broken: HashSet::new(),
            machine_warnings: Vec::new(),
            modules: HashMap::new(),
            aliases: HashMap::new(),
            imported: HashMap::new(),
            mirrored: HashMap::new(),
        }
    }

    /// A checker for a module being loaded, which is the file `file` and
    /// can use the modules in `modules`.
    pub fn for_module(file: FileId, modules: HashMap<ModuleRef, Rc<Exports>>) -> Checker {
        Checker { file, modules, ..Checker::new(false) }
    }

    /// Tells the checker what every module loaded so far offers.
    pub fn set_modules(&mut self, modules: HashMap<ModuleRef, Rc<Exports>>) {
        self.modules = modules;
    }

    /// What this file offers to files that import it: every name defined at
    /// its top level, except the ones it imported itself.
    pub fn exports(&self) -> Exports {
        let mut exports = Exports::default();
        for (name, var) in &self.scopes[0] {
            if !self.imported.contains_key(name) {
                let def = self.index.defs[var.def].clone();
                exports.values.insert(name.clone(), ExportedValue { ty: var.ty.clone(), alphabet: var.alphabet.clone(), def });
            }
        }
        for (name, sig) in &self.funcs {
            if !self.imported.contains_key(name) {
                let def = self.index.defs[sig.def].clone();
                exports.functions.insert(name.clone(), ExportedFn { params: sig.params.clone(), ret: sig.ret.clone(), def });
            }
        }
        exports.imported = self.imported.clone();
        exports.broken = self.broken.clone();
        exports
    }

    /// Checks a whole program (or, in the REPL, one more piece of it) and
    /// returns the first error, if any.
    pub fn check(&mut self, program: &[Stmt]) -> Check<()> {
        match self.check_all(program).into_iter().next() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Checks a whole program and returns every error found, in the order
    /// they appear in the source.
    pub fn check_all(&mut self, program: &[Stmt]) -> Vec<Diag> {
        self.declare_functions(program);
        self.statements(program);
        let mut errors: Vec<Diag> = std::mem::take(&mut self.errors);
        errors.retain(|e| e.code != CONSEQUENCE);
        errors.sort_by_key(|e| e.span.start);
        errors.dedup_by(|a, b| a.span == b.span && a.message == b.message);
        errors
    }

    /// The warnings for a program that `check_all` found no errors in.
    pub fn warnings(&mut self, program: &[Stmt]) -> Vec<Diag> {
        let mut warnings = std::mem::take(&mut self.machine_warnings);
        warnings.extend(super::lint::program(program, &self.index, self.repl, self.file));
        warnings.sort_by_key(|w| w.span.start);
        warnings
    }

    /// The type of an expression, as the REPL's `:type` shows it.
    pub fn type_of(&mut self, expr: &Expr) -> Check<Type> {
        let result = self.expr(expr);
        self.errors.clear();
        result
    }

    /// Every name defined at the top level so far, values and functions.
    pub fn global_names(&self) -> Vec<String> {
        let mut names: Vec<String> =
            self.scopes[0].keys().chain(self.funcs.keys()).chain(self.aliases.keys()).cloned().collect();
        names.sort();
        names
    }

    /// Functions are declared first so they can call each other in any order.
    fn declare_functions(&mut self, program: &[Stmt]) {
        for stmt in program {
            if let StmtKind::Fn(f) = &stmt.kind {
                if f.name.text == "std" {
                    self.errors.push(std_reserved(f.name.span));
                    continue;
                }
                if builtins::NAMES.contains(&f.name.text.as_str()) {
                    self.errors.push(
                        Diag::new(
                            ErrorKind::Name,
                            f.name.span,
                            format!("`{}` is a built-in function and cannot be redefined", f.name.text),
                        )
                        .code(codes::BUILTIN_REDEFINED)
                        .hint(format!("choose another name, such as `my_{}`", f.name.text)),
                    );
                    continue;
                }
                let params: Vec<String> = f.params.iter().map(|(n, ty)| format!("{}: {ty}", n.text)).collect();
                let def = self.index.add_def(Def {
                    name: f.name.text.clone(),
                    kind: DefKind::Function,
                    ty: None,
                    span: f.name.span,
                    visible: (0, usize::MAX),
                    detail: format!("fn {}({}) -> {}", f.name.text, params.join(", "), f.ret),
                    origin: None,
                    doc: None,
                });
                let signature = Signature {
                    params: f.params.iter().map(|(_, ty)| ty.clone()).collect(),
                    ret: f.ret.clone(),
                    def,
                };
                if self.funcs.insert(f.name.text.clone(), signature).is_some() && !self.repl {
                    self.errors.push(
                        Diag::new(
                            ErrorKind::Name,
                            f.name.span,
                            format!("function `{}` is already defined", f.name.text),
                        )
                        .code(codes::ALREADY_DEFINED)
                        .hint("two functions cannot share a name; rename one of them"),
                    );
                }
            }
        }
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
        if name.text == "std" {
            return Err(std_reserved(name.span));
        }
        if top_level && !self.repl && self.aliases.contains_key(&name.text) {
            return Err(Diag::new(ErrorKind::Name, name.span, format!("`{}` is already the name of a module", name.text))
                .code(codes::ALREADY_DEFINED)
                .hint("choose another name, or import the module under another name with `as`"));
        }
        if self.scopes.last().unwrap().contains_key(&name.text) && !(self.repl && top_level) {
            return Err(Diag::new(
                ErrorKind::Name,
                name.span,
                format!("`{}` is already defined", name.text),
            )
            .code(codes::ALREADY_DEFINED)
            .hint("values cannot be changed once defined; choose a new name"));
        }
        let def = self.index.add_def(Def {
            name: name.text.clone(),
            kind,
            ty: Some(ty.clone()),
            span: name.span,
            visible: (name.span.end, self.block_end),
            detail,
            origin: None,
                    doc: None,
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
        if name.contains('.') {
            return match self.resolve_path(name, span)? {
                (Member::Value(value), def) => Ok(Var { ty: value.ty, alphabet: value.alphabet, def }),
                (Member::Function(_), _) => Err(Diag::new(ErrorKind::Name, span, format!("`{name}` is not defined"))
                    .hint(format!("`{name}` is a function; call it as `{name}(...)`"))),
            };
        }
        if let Some(var) = self.scopes.iter().rev().find_map(|scope| scope.get(name)) {
            let var = var.clone();
            self.index.add_ref(span, var.def);
            return Ok(var);
        }
        if self.broken.contains(name) {
            return Err(Diag::new(ErrorKind::Name, span, "").code(CONSEQUENCE));
        }
        if let Some(alias) = self.aliases.get(name) {
            self.index.add_ref(span, alias.def);
            return Err(module_as_value(name, span));
        }
        if name == "std" {
            return Err(module_as_value(name, span));
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

    // ---- modules ----

    /// `import lib as alias`: binds the module to a name.
    fn import(&mut self, lib: &Lib, alias: Option<&Name>, target: Option<&ModuleRef>, end: usize) -> Check<()> {
        let name = match alias {
            Some(alias) => alias.clone(),
            None => Name { text: lib.default_name(), span: lib.span },
        };
        // When the module could not be loaded, the loader said why; uses of
        // the name are not reported again.
        let Some(target) = target else {
            self.broken.insert(name.text);
            return Ok(());
        };
        self.check_binding(&name)?;
        let exports = self.modules.get(target).cloned().unwrap_or_default();
        let visible = (end, self.block_end);
        let def = self.index.add_def(Def {
            name: name.text.clone(),
            kind: DefKind::Module,
            ty: None,
            span: name.span,
            visible,
            detail: format!("import {}", lib.shown()),
            origin: None,
                    doc: None,
        });
        let mut members: Vec<Def> = exports.values.values().map(|v| v.def.clone()).collect();
        members.extend(exports.functions.values().map(|f| f.def.clone()));
        members.sort_by(|a, b| a.name.cmp(&b.name));
        self.index.namespaces.push(Namespace { name: name.text.clone(), visible, members });
        self.imported.insert(name.text.clone(), format!("import {}", lib.shown()));
        self.aliases.insert(name.text, Alias { target: target.clone(), def });
        Ok(())
    }

    /// `from lib import a, b as c`: defines each name in this file. A
    /// mistake with one name is reported and the others are still imported.
    fn import_names(&mut self, lib: &Lib, names: &[(Name, Option<Name>)], target: Option<&ModuleRef>, end: usize) {
        for (name, alias) in names {
            let local = alias.as_ref().unwrap_or(name);
            let Some(target) = target else {
                self.broken.insert(local.text.clone());
                continue;
            };
            let exports = self.modules.get(target).cloned().unwrap_or_default();
            let module = match &lib.kind {
                LibKind::Std(_) => format!("`{}`", lib.shown()),
                LibKind::File(_) => lib.shown(),
            };
            let result = self.member(&exports, &module, name).and_then(|member| {
                self.check_binding(local)?;
                let def = self.index.add_def(Def {
                    name: local.text.clone(),
                    kind: DefKind::Imported,
                    ty: match &member {
                        Member::Value(v) => Some(v.ty.clone()),
                        Member::Function(_) => None,
                    },
                    span: local.span,
                    visible: (end, self.block_end),
                    detail: member.def().detail.clone(),
                    origin: Some(member.def().span).filter(|s| s.file != FileId::STD),
                    doc: member.def().doc.clone(),
                });
                match member {
                    Member::Value(v) => {
                        self.scopes[0].insert(local.text.clone(), Var { ty: v.ty, alphabet: v.alphabet, def });
                    }
                    Member::Function(f) => {
                        self.funcs.insert(local.text.clone(), Signature { params: f.params, ret: f.ret, def });
                    }
                }
                self.imported.insert(local.text.clone(), format!("from {} import {}", lib.shown(), name.text));
                Ok(())
            });
            if let Err(error) = result {
                self.broken.insert(local.text.clone());
                self.errors.push(error);
            }
        }
    }

    /// Checks that a module, or a name taken from one, can be called `name`
    /// in this file.
    fn check_binding(&self, name: &Name) -> Check<()> {
        if name.text == "std" {
            return Err(std_reserved(name.span));
        }
        let taken = self.scopes[0].contains_key(&name.text)
            || self.funcs.contains_key(&name.text)
            || self.aliases.contains_key(&name.text);
        if taken && !self.repl {
            return Err(Diag::new(ErrorKind::Name, name.span, format!("`{}` is already defined in this file", name.text))
                .code(codes::IMPORT_CLASH)
                .hint(format!("import it under another name with `as`: `... as {}2`, or rename the other one", name.text)));
        }
        Ok(())
    }

    /// The member `name` of a module, described as `module` in messages,
    /// and a reference to its definition in the index.
    fn member(&mut self, exports: &Exports, module: &str, name: &Name) -> Check<Member> {
        let member = if let Some(value) = exports.values.get(&name.text) {
            Member::Value(value.clone())
        } else if let Some(function) = exports.functions.get(&name.text) {
            Member::Function(function.clone())
        } else if exports.broken.contains(&name.text) {
            return Err(Diag::new(ErrorKind::Name, name.span, "").code(CONSEQUENCE));
        } else if let Some(how) = exports.imported.get(&name.text) {
            return Err(Diag::new(
                ErrorKind::Name,
                name.span,
                format!("`{}` is not defined in {module}, only imported there", name.text),
            )
            .code(codes::REEXPORT)
            .hint(format!("a module passes on only what it defines; import it from where it comes from: `{how}`")));
        } else {
            let mut err = Diag::new(ErrorKind::Name, name.span, format!("{module} has no `{}`", name.text))
                .code(codes::UNKNOWN_MEMBER);
            if let Some(close) = did_you_mean(&name.text, exports.names()) {
                err = err.hint(format!("did you mean `{close}`?"));
            } else if exports.values.is_empty() && exports.functions.is_empty() {
                err = err.hint("it defines nothing at its top level");
            }
            return Err(err);
        };
        let def = self.mirror(member.def());
        self.index.add_ref(name.span, def);
        Ok(member)
    }

    /// The index entry for a definition in another file, made the first
    /// time it is needed. Editors use it to show and find that definition.
    fn mirror(&mut self, def: &Def) -> usize {
        let key = (def.span.file, def.span.start, def.name.clone());
        if let Some(&index) = self.mirrored.get(&key) {
            return index;
        }
        // Not visible anywhere in this file by its bare name.
        let copy = Def { visible: (usize::MAX, usize::MAX), ..def.clone() };
        let index = self.index.add_def(copy);
        self.mirrored.insert(key, index);
        index
    }

    /// What a qualified name like `utils.EvenZeros` or `std.math.gcd`
    /// refers to, and its definition's index entry.
    fn resolve_path(&mut self, text: &str, span: Span) -> Check<(Member, usize)> {
        let parts = parts_of(text, span);
        let (exports, module, used) = self.namespace(&parts)?;
        match &parts[used..] {
            [] => Err(module_as_value(text, span)),
            [member] => {
                let member = self.member(&exports, &module, member)?;
                let def = self.mirror(member.def());
                Ok((member, def))
            }
            [first, ..] => {
                if let Some(how) = exports.imported.get(&first.text) {
                    return Err(Diag::new(
                        ErrorKind::Name,
                        first.span,
                        format!("`{}` is a module that {module} imports, and imports are not passed on", first.text),
                    )
                    .code(codes::REEXPORT)
                    .hint(format!("import it here yourself: `{how}`")));
                }
                let member = self.member(&exports, &module, first)?;
                let what = match &member {
                    Member::Value(v) => article(&v.ty),
                    Member::Function(_) => "a function".to_string(),
                };
                Err(Diag::new(ErrorKind::Name, parts[used + 1].span, format!("`{}` is {what}, which has no names inside it", first.text))
                    .code(codes::UNKNOWN_MEMBER)
                    .hint("only a module has names written after a `.`"))
            }
        }
    }

    /// The module that the first part (or two, for `std.math`) of a
    /// qualified name refers to: its exports, how to describe it, and how
    /// many parts named it.
    fn namespace(&mut self, parts: &[Name]) -> Check<(Rc<Exports>, String, usize)> {
        let first = &parts[0];
        if let Some(alias) = self.aliases.get(&first.text).cloned() {
            self.index.add_ref(first.span, alias.def);
            let exports = self.modules.get(&alias.target).cloned().unwrap_or_default();
            return Ok((exports, format!("module `{}`", first.text), 1));
        }
        if first.text == "std" {
            let module = &parts[1];
            let target = ModuleRef::Std(module.text.clone());
            if let Some(exports) = self.modules.get(&target) {
                return Ok((exports.clone(), format!("`std.{}`", module.text), 2));
            }
            let Some(exports) = super::stdlib::exports(&module.text) else {
                let mut err = Diag::new(
                    ErrorKind::Name,
                    module.span,
                    format!("the standard library has no module called `{}`", module.text),
                )
                .code(codes::UNKNOWN_MEMBER);
                err = match did_you_mean(&module.text, super::stdlib::MODULES.iter().map(|m| m.name)) {
                    Some(close) => err.hint(format!("did you mean `std.{close}`?")),
                    None => err.hint(super::stdlib::modules_hint()),
                };
                return Err(err);
            };
            let exports = Rc::new(exports);
            self.modules.insert(target, exports.clone());
            return Ok((exports, format!("`std.{}`", module.text), 2));
        }
        let what = if let Some(var) = self.scopes.iter().rev().find_map(|scope| scope.get(&first.text)) {
            Some(article(&var.ty))
        } else {
            self.funcs.contains_key(&first.text).then(|| "a function".to_string())
        };
        if what.is_none() && self.broken.contains(&first.text) {
            return Err(Diag::new(ErrorKind::Name, first.span, "").code(CONSEQUENCE));
        }
        if let Some(what) = what {
            return Err(Diag::new(ErrorKind::Name, first.span, format!("`{}` is {what}, not a module", first.text))
                .code(codes::UNKNOWN_MEMBER)
                .hint("only a module has names written after a `.`; import one with `import \"file.kstar\"`"));
        }
        let mut err = Diag::new(ErrorKind::Name, first.span, format!("`{}` is not defined", first.text));
        if let Some(close) = did_you_mean(&first.text, self.aliases.keys().map(String::as_str)) {
            err = err.hint(format!("did you mean `{close}`?"));
        } else {
            err = err.hint(format!("to use a module, import it first: `import \"{}.kstar\"`", first.text));
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

    /// Checks each statement, recording errors and carrying on. When a
    /// definition fails, its name is remembered as broken, so its uses do
    /// not cause a second, misleading error.
    fn statements(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            if let Err(error) = self.statement(stmt) {
                if let Some(name) = defined_name(stmt) {
                    if !self.scopes.last().unwrap().contains_key(name) {
                        self.broken.insert(name.to_string());
                    }
                }
                self.errors.push(error);
            }
        }
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
                        )
                        .code(codes::DUPLICATE_SYMBOL)
                        .hint("an alphabet is a set: list each symbol once"));
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
                    )
                    .code(codes::RETURN_OUTSIDE)
                    .hint("at the top level, use `print` to show a value"));
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
                // A bad condition is reported, and the blocks are still checked.
                if let Err(error) = self.expect(cond, &Type::Bool, "the condition of `if`") {
                    self.errors.push(error);
                }
                self.scoped(stmt.span.end, |c| {
                    c.statements(then);
                    Ok(())
                })?;
                if let Some(block) = otherwise {
                    self.scoped(stmt.span.end, |c| {
                        c.statements(block);
                        Ok(())
                    })?;
                }
                Ok(())
            }
            StmtKind::For { var, items, body } => {
                let element = self.element_type(items)?;
                self.scoped(stmt.span.end, |c| {
                    c.define_value(var, element, DefKind::LoopVariable, None)?;
                    c.statements(body);
                    Ok(())
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
            StmtKind::Import { lib, alias, target } => self.import(lib, alias.as_ref(), target.as_ref(), stmt.span.end),
            StmtKind::FromImport { lib, names, target } => {
                self.import_names(lib, names, target.as_ref(), stmt.span.end);
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
            .map(|()| self.statements(&f.body));
        self.ret = None;
        self.block_end = outer_end;
        self.scopes = outer;
        result?;
        if !always_returns(&f.body) {
            return Err(type_error(
                f.name.span,
                format!("function `{}` can reach its end without returning a value", f.name.text),
            )
            .code(codes::MISSING_RETURN)
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
        if let (true, Some(symbols), false) = (literal, alphabet, m.incomplete) {
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
                            from_span: from.span,
                            to_span: to.span,
                        })
                    }
                    MachineItem::For { .. } => unreachable!("not literal"),
                }
            }
            machine::build(&spec)?;
            self.machine_warnings.extend(super::lint::machine(&spec));
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
                        origin: None,
                    doc: None,
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
                        origin: None,
                    doc: None,
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
                    )
                    .code(codes::BAD_SET)
                    .hint("machines and regexes cannot go in a set; to combine languages use `union(a, b)`"));
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
                        )
                        .code(codes::BAD_SET));
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
            .code(codes::OPERATOR_TYPES)
        };
        match op {
            BinOp::Add => match (&l, &r) {
                (Type::Int, Type::Int) => Ok(Type::Int),
                (Type::Str, Type::Str) => Ok(Type::Str),
                (Type::Str, Type::Int) | (Type::Int, Type::Str) => Err(mismatch("two Ints or two Strs")
                    .hint("an Int is not turned into a Str by itself; `print` can show both: `print \"n =\", n`")),
                _ if is_lang(&l) && is_lang(&r) => Err(mismatch("two Ints or two Strs")
                    .hint("for the union of two languages use `union(a, b)`; for concatenation `concat(a, b)`")),
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
                    .code(codes::NOT_COMPARABLE)
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
                    )
                    .code(codes::OPERATOR_TYPES)
                    .hint("write the machine first and the word second: `M accepts \"0101\"`"));
                }
                if r != Type::Str {
                    return Err(type_error(
                        right.span,
                        format!("the right side of `accepts` must be a Str, but this is {}", article(&r)),
                    )
                    .code(codes::OPERATOR_TYPES)
                    .hint("write the word in quotes: `m accepts \"0101\"`"));
                }
                Ok(Type::Bool)
            }
        }
    }

    fn call(&mut self, name: &Name, args: &[Expr], span: Span) -> Check<Type> {
        self.index.calls.push(Call { name: name.text.clone(), name_span: name.span, span });
        if name.is_qualified() {
            let signature = match self.resolve_path(&name.text, name.span)? {
                (Member::Function(f), def) => Signature { params: f.params, ret: f.ret, def },
                (Member::Value(v), _) => {
                    return Err(Diag::new(
                        ErrorKind::Name,
                        name.span,
                        format!("`{}` is {}, not a function", name.text, article(&v.ty)),
                    )
                    .code(codes::UNKNOWN_FUNCTION));
                }
            };
            let types = args.iter().map(|arg| self.expr(arg)).collect::<Check<Vec<Type>>>()?;
            return check_arguments(&name.text, &signature, &types, args, span);
        }
        if let Some(signature) = self.funcs.get(&name.text) {
            self.index.add_ref(name.span, signature.def);
        }
        // An unknown function is reported before anything wrong with its
        // arguments: it is the bigger mistake.
        let builtin = builtins::NAMES.contains(&name.text.as_str());
        if !builtin && !self.funcs.contains_key(&name.text) {
            let mut err = Diag::new(
                ErrorKind::Name,
                name.span,
                format!("there is no function called `{}`", name.text),
            )
            .code(codes::UNKNOWN_FUNCTION);
            let known = builtins::NAMES.iter().copied().chain(self.funcs.keys().map(String::as_str));
            if self.scopes.iter().any(|scope| scope.contains_key(&name.text)) {
                err = err.hint(format!("`{}` is a value, not a function", name.text));
            } else if let Some(close) = did_you_mean(&name.text, known) {
                err = err.hint(format!("did you mean `{close}`?"));
            }
            return Err(err);
        }
        let types = args.iter().map(|arg| self.expr(arg)).collect::<Check<Vec<Type>>>()?;

        if builtin {
            let ty = builtins::result_type(&name.text, &types).map_err(|e| {
                let err = type_error(e.arg.map_or(span, |i| args[i].span), e.message).code(e.code);
                match e.hint {
                    Some(hint) => err.hint(hint),
                    None => err,
                }
            })?;
            // A function name written out can be checked now; one that is
            // computed is checked when the program runs.
            if let ("to_rust", Some(Expr { kind: ExprKind::Str(fn_name), span })) = (name.text.as_str(), args.get(1)) {
                crate::automata::codegen::check_fn_name(fn_name).map_err(|message| bad_rust_name(*span, message))?;
            }
            return Ok(ty);
        }

        check_arguments(&name.text, &self.funcs[&name.text], &types, args, span)
    }
}

/// Checks the arguments of a call to a user-defined (or imported) function
/// and returns the type of its result.
fn check_arguments(name: &str, signature: &Signature, types: &[Type], args: &[Expr], span: Span) -> Check<Type> {
    if types.len() != signature.params.len() {
        let n = signature.params.len();
        return Err(type_error(
            span,
            format!(
                "`{}` takes {n} argument{}, but {} {} given",
                name,
                if n == 1 { "" } else { "s" },
                types.len(),
                if types.len() == 1 { "was" } else { "were" }
            ),
        )
        .code(codes::ARGUMENT_COUNT));
    }
    for (i, (wanted, actual)) in signature.params.iter().zip(types).enumerate() {
        if !fits(wanted, actual) {
            return Err(type_error(
                args[i].span,
                format!(
                    "argument {} of `{}` must be {}, but this is {}",
                    i + 1,
                    name,
                    article(wanted),
                    article(actual)
                ),
            ));
        }
    }
    Ok(signature.ret.clone())
}

/// The error for a function name that Rust would not accept, in `to_rust`.
pub fn bad_rust_name(span: Span, message: String) -> Diag {
    Diag::new(ErrorKind::Value, span, message)
        .code(codes::BAD_RUST_NAME)
        .hint("a Rust function name is usually written in snake_case, like `even_zeros`")
}

/// The error for using `std`, or a module, where a value is needed.
fn module_as_value(name: &str, span: Span) -> Diag {
    let example = if name == "std" { "std.math.gcd(12, 18)".to_string() } else { format!("{name}.Name") };
    Diag::new(ErrorKind::Name, span, format!("`{name}` is a module, not a value"))
        .code(codes::MODULE_AS_VALUE)
        .hint(format!("use one of the names inside it, like `{example}`"))
}

/// The error for defining `std`, which always means the standard library.
fn std_reserved(span: Span) -> Diag {
    Diag::new(ErrorKind::Name, span, "`std` is the name of the standard library and cannot be defined")
        .code(codes::IMPORT_CLASH)
        .hint("choose another name")
}

/// The name a definition statement defines, if it is one.
fn defined_name(stmt: &Stmt) -> Option<&str> {
    match &stmt.kind {
        StmtKind::Alphabet { name, .. }
        | StmtKind::MachineDef { name, .. }
        | StmtKind::GrammarDef { name, .. }
        | StmtKind::Let { name, .. } => Some(&name.text),
        _ => None,
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

    fn all_errors(src: &str) -> Vec<String> {
        let program = Parser::new(lex(src, 0, 1).unwrap(), src, false).program().unwrap();
        Checker::new(false).check_all(&program).into_iter().map(|d| d.message).collect()
    }

    #[test]
    fn reports_every_error() {
        let found = all_errors("let a = 1 + true\nlet b = len(5)\nprint nope()\nlet c = 1");
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found[2].contains("no function called `nope`"));
    }

    #[test]
    fn a_broken_definition_is_reported_once() {
        // `m` and `d` fail; their uses do not add more errors.
        let src = format!("{B}let m = minimize(1)\nprint m\ndfa D over B {{ start q }}\nprint D accepts \"0\"");
        let found = all_errors(&src);
        assert_eq!(found.len(), 2, "{found:?}");
    }

    #[test]
    fn keeps_checking_inside_blocks() {
        let found = all_errors("fn f(n: Int) -> Int {\n  let a = n + \"x\"\n  return true\n}\nif 1 { print 1 + true }");
        assert_eq!(found.len(), 4, "{found:?}");
    }

    #[test]
    fn errors_have_codes() {
        let code = |src: &str| check(src).unwrap_err().code;
        assert_eq!(code("print x"), codes::UNDEFINED);
        assert_eq!(code("let x = 1\nlet x = 2"), codes::ALREADY_DEFINED);
        assert_eq!(code("print mininize(1)"), codes::UNKNOWN_FUNCTION);
        assert_eq!(code("print 1 + true"), codes::OPERATOR_TYPES);
        assert_eq!(code("print len(1, 2)"), codes::ARGUMENT_COUNT);
        assert_eq!(code("return 1"), codes::RETURN_OUTSIDE);
        assert_eq!(code(&format!("{B}dfa M over B {{ start a  a --0--> a }}")), codes::NOT_TOTAL);
    }
}

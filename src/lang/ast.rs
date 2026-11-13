//! The syntax tree: the shape of a program after parsing.
//!
//! A program is a list of statements. Statements contain expressions.
//! Every node remembers its `Span` so later stages can point at it.

use super::token::{FileId, Span};
use crate::automata::Regex;

#[derive(Debug, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    /// `alphabet B = {0, 1}`
    Alphabet { name: Name, symbols: Vec<(char, Span)> },
    /// `dfa Name over B { ... }`, and the same for `nfa`, `pda` and `tm`
    MachineDef { name: Name, machine: MachineExpr },
    /// `grammar Name over B { S -> a S b | eps }`
    GrammarDef { name: Name, grammar: GrammarExpr },
    /// `let x = value`
    Let { name: Name, value: Expr },
    /// `fn name(params) -> Type { ... }`
    Fn(std::rc::Rc<FnDef>),
    /// `return value`
    Return(Expr),
    /// `if cond { ... } else { ... }`
    If { cond: Expr, then: Vec<Stmt>, otherwise: Option<Vec<Stmt>> },
    /// `for x in items { ... }`
    For { var: Name, items: Expr, body: Vec<Stmt> },
    /// `print a, b`
    Print(Vec<Expr>),
    /// `test condition`
    Test(Expr),
    /// `draw machine`
    Draw(Expr),
    /// `trace machine on word`: run the machine step by step, printing each step.
    Trace { machine: Expr, word: Expr },
    /// `import "utils.kstar" as u`, or `import std.math`: binds a module
    /// as a namespace, so its names are written `u.Name`.
    Import { lib: Lib, alias: Option<Name>, target: Option<ModuleRef> },
    /// `from "utils.kstar" import a, b as c`: puts names of a module into
    /// this file's scope. Each pair is a name and its optional new name.
    FromImport { lib: Lib, names: Vec<(Name, Option<Name>)>, target: Option<ModuleRef> },
}

/// What an import statement names: a file, or a standard-library module.
#[derive(Debug, Clone)]
pub struct Lib {
    pub kind: LibKind,
    /// The quoted path, or `std.math`.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibKind {
    /// `"machines/parity.kstar"`: a path relative to the importing file.
    File(String),
    /// `std.math`: the module's name, `math`.
    Std(String),
}

impl Lib {
    /// The name the module is bound to when `import` has no `as`: the file
    /// name without `.kstar` (`parity` for `"machines/parity.kstar"`), or the
    /// standard-library module's own name. It may not be a valid name, which
    /// the checker reports.
    pub fn default_name(&self) -> String {
        match &self.kind {
            LibKind::File(path) => {
                let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
                file.strip_suffix(".kstar").unwrap_or(file).to_string()
            }
            LibKind::Std(module) => module.clone(),
        }
    }

    /// How the module is written in messages: `"utils.kstar"` or `std.math`.
    pub fn shown(&self) -> String {
        match &self.kind {
            LibKind::File(path) => format!("\"{path}\""),
            LibKind::Std(module) => format!("std.{module}"),
        }
    }
}

/// Which module an import refers to, once the loader has found it: a file
/// (by its id in the source map) or a standard-library module (by name).
/// The parser leaves `target` empty; `module::Loader` fills it in.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModuleRef {
    File(FileId),
    Std(String),
}

/// A name together with where it was written.
///
/// A name that refers to something in a module is *qualified*: its text
/// holds the parts joined by dots, as written, like `utils.EvenZeros` or
/// `std.math.gcd`. The parser only accepts dots with no spaces around them,
/// so each part's place in the source follows from the text.
#[derive(Debug, Clone)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

impl Name {
    /// True for `utils.B`, false for `B`.
    pub fn is_qualified(&self) -> bool {
        self.text.contains('.')
    }

    /// The parts of a qualified name, each with its own span.
    pub fn parts(&self) -> Vec<Name> {
        parts_of(&self.text, self.span)
    }
}

/// The dot-separated parts of `text`, which was written at `span`.
pub fn parts_of(text: &str, span: Span) -> Vec<Name> {
    let mut start = span.start;
    text.split('.')
        .map(|part| {
            let name = Name { text: part.to_string(), span: Span { start, end: start + part.len(), ..span } };
            start += part.len() + 1;
            name
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: Name,
    pub params: Vec<(Name, Type)>,
    pub ret: Type,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Int(i64),
    Str(String),
    Bool(bool),
    Regex(Regex),
    /// A variable.
    Name(String),
    /// `(expr)`. The parentheses change nothing when running, but they are
    /// kept so that `kstar fmt` can write them back exactly as they were.
    Paren(Box<Expr>),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `name(args)`
    Call { name: Name, args: Vec<Expr> },
    /// `{a, b, c}`
    Set(Vec<Expr>),
    /// `dfa over B { ... }` used as a value.
    Machine(Box<MachineExpr>),
    /// `grammar over B { ... }` used as a value.
    Grammar(Box<GrammarExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Mod,
    Eq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Accepts,
    Range,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Mod => "%",
            BinOp::Eq => "==",
            BinOp::NotEq => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "and",
            BinOp::Or => "or",
            BinOp::Accepts => "accepts",
            BinOp::Range => "..",
        }
    }
}

/// The kinds of machine written with arrow transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineKind {
    Dfa,
    Nfa,
    Pda,
    Tm,
}

impl MachineKind {
    /// The keyword that introduces this kind of machine.
    pub fn keyword(self) -> &'static str {
        match self {
            MachineKind::Dfa => "dfa",
            MachineKind::Nfa => "nfa",
            MachineKind::Pda => "pda",
            MachineKind::Tm => "tm",
        }
    }

    /// How the kind is named in messages: "DFA".
    pub fn title(self) -> &'static str {
        match self {
            MachineKind::Dfa => "DFA",
            MachineKind::Nfa => "NFA",
            MachineKind::Pda => "PDA",
            MachineKind::Tm => "TM",
        }
    }

    /// The type of a machine of this kind.
    pub fn ty(self) -> Type {
        match self {
            MachineKind::Dfa => Type::Dfa,
            MachineKind::Nfa => Type::Nfa,
            MachineKind::Pda => Type::Pda,
            MachineKind::Tm => Type::Tm,
        }
    }
}

/// The body of a `dfa`, `nfa`, `pda` or `tm` definition.
#[derive(Debug, Clone)]
pub struct MachineExpr {
    pub kind: MachineKind,
    /// The alphabet named after `over`.
    pub alphabet: Name,
    pub items: Vec<MachineItem>,
    /// The `dfa ... over B` header, used when an error is about the whole machine.
    pub header: Span,
    /// From `{` to `}`.
    pub body: Span,
    /// Some items had syntax errors and were left out, so the machine is
    /// not checked further: its other errors would be misleading.
    pub incomplete: bool,
}

// A transition is much bigger than `start q0`, but the syntax tree is built
// once and holds few items, so boxing it would only add noise.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum MachineItem {
    /// `start q0`
    Start(StateRef),
    /// `accept q1, q2`
    Accept(Vec<StateRef>),
    /// `reject q3` (Turing machines only)
    Reject(Vec<StateRef>),
    /// `q0 --a,b--> q1`, or in a PDA `q0 --a, Z/AZ--> q1`
    Transition { from: StateRef, action: Action, to: StateRef, span: Span },
    /// `for i in 0..n { ... }` generating more items
    For { var: Name, items: Expr, body: Vec<MachineItem> },
}

/// A state: `q0`, or `q[i]` which names the state `q` followed by the value of `i`.
#[derive(Debug, Clone)]
pub struct StateRef {
    pub name: String,
    pub index: Option<Expr>,
    pub span: Span,
}

/// A transition label: a symbol, or `None` for `eps`.
#[derive(Debug, Clone)]
pub struct Label {
    pub symbol: Option<char>,
    pub span: Span,
}

/// What a transition does, written between `--` and `-->`.
#[derive(Debug, Clone)]
pub enum Action {
    /// DFA and NFA: the symbols it reads, `a,b` (or `eps`).
    Symbols(Vec<Label>),
    /// PDA: `read, pop/push`. `read` and `pop` may be `eps`; `push` is a
    /// string of stack symbols, top first, empty for `eps`.
    Stack { read: Label, pop: Label, push: Vec<char>, push_span: Span },
    /// Turing machine: `read/write,move`, for example `0/1,R`. The blank
    /// is `_`; `move` is `L`, `R` or `S` (stay).
    Tape { read: Label, write: Label, dir: Dir, dir_span: Span },
}

/// Which way a Turing machine's head moves.
pub use crate::automata::tm::Dir;

/// The body of a `grammar` definition.
#[derive(Debug, Clone)]
pub struct GrammarExpr {
    /// The alphabet named after `over`; its symbols are the terminals.
    pub alphabet: Name,
    pub rules: Vec<RuleDef>,
    /// The `grammar ... over B` header.
    pub header: Span,
    /// From `{` to `}`.
    pub body: Span,
}

/// `S -> a S b | eps`: a variable and its alternatives.
#[derive(Debug, Clone)]
pub struct RuleDef {
    pub head: Name,
    /// Each alternative is a list of symbols; `eps` is an empty list.
    pub alternatives: Vec<Vec<GSym>>,
    pub span: Span,
}

/// One symbol on the right of `->`.
#[derive(Debug, Clone)]
pub struct GSym {
    pub kind: GSymKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GSymKind {
    /// A name starting with a capital letter.
    Variable(String),
    /// A symbol of the alphabet.
    Terminal(char),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Bool,
    Str,
    Alphabet,
    Dfa,
    Nfa,
    Regex,
    Grammar,
    Pda,
    Tm,
    Set(Box<Type>),
    /// The type of `{}`, which fits any `Set[T]`.
    EmptySet,
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Type::Int => write!(f, "Int"),
            Type::Bool => write!(f, "Bool"),
            Type::Str => write!(f, "Str"),
            Type::Alphabet => write!(f, "Alphabet"),
            Type::Dfa => write!(f, "Dfa"),
            Type::Nfa => write!(f, "Nfa"),
            Type::Regex => write!(f, "Regex"),
            Type::Grammar => write!(f, "Grammar"),
            Type::Pda => write!(f, "Pda"),
            Type::Tm => write!(f, "Tm"),
            Type::Set(inner) => write!(f, "Set[{inner}]"),
            Type::EmptySet => write!(f, "Set[?]"),
        }
    }
}

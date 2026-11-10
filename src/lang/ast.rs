//! The syntax tree: the shape of a program after parsing.
//!
//! A program is a list of statements. Statements contain expressions.
//! Every node remembers its `Span` so later stages can point at it.

use super::token::Span;
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
    /// `dfa Name over B { ... }` or `nfa Name over B { ... }`
    MachineDef { name: Name, machine: MachineExpr },
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
}

/// A name together with where it was written.
#[derive(Debug, Clone)]
pub struct Name {
    pub text: String,
    pub span: Span,
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
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `name(args)`
    Call { name: Name, args: Vec<Expr> },
    /// `{a, b, c}`
    Set(Vec<Expr>),
    /// `dfa over B { ... }` used as a value.
    Machine(Box<MachineExpr>),
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

/// The body of a `dfa` or `nfa` definition.
#[derive(Debug, Clone)]
pub struct MachineExpr {
    pub is_dfa: bool,
    /// The alphabet named after `over`.
    pub alphabet: Name,
    pub items: Vec<MachineItem>,
    /// The `dfa ... over B` header, used when an error is about the whole machine.
    pub header: Span,
}

#[derive(Debug, Clone)]
pub enum MachineItem {
    /// `start q0`
    Start(StateRef),
    /// `accept q1, q2`
    Accept(Vec<StateRef>),
    /// `q0 --a,b--> q1`
    Transition { from: StateRef, labels: Vec<Label>, to: StateRef, span: Span },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Bool,
    Str,
    Alphabet,
    Dfa,
    Nfa,
    Regex,
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
            Type::Set(inner) => write!(f, "Set[{inner}]"),
            Type::EmptySet => write!(f, "Set[?]"),
        }
    }
}

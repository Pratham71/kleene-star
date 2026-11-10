//! What the checker learns about names, kept for editor tools.
//!
//! While checking, the checker already resolves every name to its
//! definition and works out the type of every expression. The `Index`
//! simply writes that down, so the language server (`crate::ide`) can answer
//! "what is this?", "where is it defined?" and "where is it used?" without a
//! second copy of the scoping rules.

use super::ast::Type;
use super::token::Span;

/// What kind of thing a definition introduces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
    Alphabet,
    /// A `dfa`, `nfa`, `pda` or `tm` with a name.
    Machine,
    Grammar,
    /// `let`.
    Variable,
    /// A function's parameter.
    Parameter,
    /// The variable of a `for` loop.
    LoopVariable,
    Function,
    /// A state, inside one machine.
    State,
    /// A variable of a grammar, inside one grammar.
    GrammarVariable,
}

/// One definition: a name and everything known about it.
#[derive(Debug, Clone)]
pub struct Def {
    pub name: String,
    pub kind: DefKind,
    /// The type of the value, for names that hold one.
    pub ty: Option<Type>,
    /// Where the name is written in its definition.
    pub span: Span,
    /// The byte range where the name can be used: from just after its
    /// definition to the end of the block it belongs to (or, for states and
    /// grammar variables, the body of their machine or grammar).
    pub visible: (usize, usize),
    /// A one-line description, like `fn twice(n: Int) -> Int`.
    pub detail: String,
}

/// A call `name(args)`.
#[derive(Debug, Clone)]
pub struct Call {
    pub name: String,
    pub name_span: Span,
    pub span: Span,
}

/// The `{ ... }` body of a machine or grammar, and its alphabet's symbols,
/// for completing symbols inside it.
#[derive(Debug, Clone)]
pub struct Body {
    pub span: Span,
    pub symbols: Vec<char>,
}

#[derive(Debug, Clone, Default)]
pub struct Index {
    pub defs: Vec<Def>,
    /// Every use of a name, with the definition it refers to.
    pub refs: Vec<(Span, usize)>,
    /// The type of every expression that was checked.
    pub exprs: Vec<(Span, Type)>,
    pub calls: Vec<Call>,
    pub bodies: Vec<Body>,
}

impl Index {
    pub fn add_def(&mut self, def: Def) -> usize {
        self.defs.push(def);
        self.defs.len() - 1
    }

    pub fn add_ref(&mut self, span: Span, def: usize) {
        if !self.refs.iter().any(|&(s, d)| s == span && d == def) {
            self.refs.push((span, def));
        }
    }
}

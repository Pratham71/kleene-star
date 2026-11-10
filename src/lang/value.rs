//! Runtime values: what an expression evaluates to.

use crate::automata::grammar::Grammar;
use crate::automata::pda::Pda;
use crate::automata::tm::Tm;
use crate::automata::{fmt_alphabet, Dfa, Nfa, Regex};
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

/// Values are immutable, so the large ones sit behind `Rc` and copying a
/// value never copies a machine.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(Rc<str>),
    Alphabet(Rc<Vec<char>>),
    Dfa(Rc<Dfa>),
    Nfa(Rc<Nfa>),
    Regex(Rc<Regex>),
    Grammar(Rc<Grammar>),
    Pda(Rc<Pda>),
    Tm(Rc<Tm>),
    Set(Rc<BTreeSet<Value>>),
}

impl Value {
    pub fn str(text: &str) -> Value {
        Value::Str(Rc::from(text))
    }

    pub fn dfa(dfa: Dfa) -> Value {
        Value::Dfa(Rc::new(dfa))
    }

    pub fn nfa(nfa: Nfa) -> Value {
        Value::Nfa(Rc::new(nfa))
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Alphabet(a) => write!(f, "{}", fmt_alphabet(a)),
            Value::Dfa(d) => write!(f, "{d}"),
            Value::Nfa(n) => write!(f, "{n}"),
            Value::Regex(r) => write!(f, "/{r}/"),
            Value::Grammar(g) => write!(f, "{g}"),
            Value::Pda(p) => write!(f, "{p}"),
            Value::Tm(m) => write!(f, "{m}"),
            Value::Set(items) => {
                // Inside a set, strings are quoted so "" and "a b" stay readable.
                let parts: Vec<String> = items
                    .iter()
                    .map(|v| match v {
                        Value::Str(s) => format!("{s:?}"),
                        other => other.to_string(),
                    })
                    .collect();
                write!(f, "{{{}}}", parts.join(", "))
            }
        }
    }
}

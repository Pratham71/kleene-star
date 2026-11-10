//! Turning a machine or grammar definition into the real thing, and
//! rejecting definitions that break the rules.
//!
//! Both the checker (before the program runs) and the interpreter (for
//! machines built by loops) describe a machine as a `MachineSpec` and hand it
//! to `build`, so the rules live in exactly one place. Grammars work the same
//! way with `GrammarSpec` and `build_grammar`.

use super::ast::{Action, GSymKind, MachineKind, RuleDef};
use super::error::{Diag, ErrorKind};
use super::token::Span;
use crate::automata::grammar::{Grammar, Rule, Sym};
use crate::automata::pda::{self, Pda};
use crate::automata::{fmt_alphabet, Dfa, Nfa};
use std::collections::BTreeSet;

pub struct Transition {
    pub from: String,
    pub action: Action,
    pub to: String,
    pub span: Span,
}

pub struct MachineSpec {
    pub kind: MachineKind,
    /// The machine's name, if it has one, for error messages.
    pub name: Option<String>,
    pub alphabet: Vec<char>,
    pub alphabet_name: String,
    /// Where the `dfa Name over B` header is.
    pub header: Span,
    pub starts: Vec<(String, Span)>,
    pub accepts: Vec<(String, Span)>,
    pub transitions: Vec<Transition>,
}

pub enum Built {
    Dfa(Dfa),
    Nfa(Nfa),
    Pda(Pda),
}

/// An error for a symbol that is not in the alphabet.
fn foreign_symbol(c: char, span: Span, spec_alphabet: &[char], alphabet_name: &str) -> Diag {
    Diag::new(ErrorKind::Alphabet, span, format!("symbol `{c}` is not in alphabet `{alphabet_name}`"))
        .hint(format!("{alphabet_name} = {}", fmt_alphabet(spec_alphabet)))
}

pub fn build(spec: &MachineSpec) -> Result<Built, Diag> {
    let kind = spec.kind.title();
    let title = match &spec.name {
        Some(name) => format!("{kind} `{name}`"),
        None => format!("this {kind}"),
    };
    let machine_error = |span, message: String| Diag::new(ErrorKind::Machine, span, message);

    // Exactly one start state.
    let Some((start, _)) = spec.starts.first() else {
        return Err(machine_error(spec.header, format!("{title} has no start state"))
            .hint("add a line such as `start q0`"));
    };
    if let Some((other, span)) = spec.starts.iter().find(|(name, _)| name != start) {
        return Err(machine_error(
            *span,
            format!("{title} already has the start state `{start}`, so `{other}` cannot be one too"),
        )
        .hint("a machine has exactly one start state; an NFA can branch from it with eps moves"));
    }

    // States are numbered in order of first appearance, start state first.
    let mut states: Vec<String> = Vec::new();
    let mut id = |name: &String| match states.iter().position(|s| s == name) {
        Some(i) => i,
        None => {
            states.push(name.clone());
            states.len() - 1
        }
    };
    let start = id(start);
    let mut edges = Vec::new();
    let mut moves = Vec::new();
    for t in &spec.transitions {
        let (from, to) = (id(&t.from), id(&t.to));
        match &t.action {
            Action::Symbols(labels) => {
                for label in labels {
                    if let Some(c) = label.symbol {
                        if !spec.alphabet.contains(&c) {
                            return Err(foreign_symbol(c, label.span, &spec.alphabet, &spec.alphabet_name));
                        }
                    } else if spec.kind == MachineKind::Dfa {
                        return Err(machine_error(label.span, "a DFA cannot have eps transitions".into())
                            .hint("eps moves need nondeterminism; declare the machine with `nfa` instead"));
                    }
                    edges.push((from, label.symbol, to, t));
                }
            }
            Action::Stack { read, pop, push, .. } => {
                if let Some(c) = read.symbol {
                    if !spec.alphabet.contains(&c) {
                        return Err(foreign_symbol(c, read.span, &spec.alphabet, &spec.alphabet_name));
                    }
                }
                moves.push(pda::Move {
                    from,
                    read: read.symbol,
                    pop: pop.symbol.map(String::from),
                    push: push.iter().map(|c| c.to_string()).collect(),
                    to,
                });
            }
        }
    }
    let accept: BTreeSet<usize> = spec.accepts.iter().map(|(name, _)| id(name)).collect();

    match spec.kind {
        MachineKind::Pda => {
            return Ok(Built::Pda(Pda {
                alphabet: spec.alphabet.clone(),
                states,
                start,
                accept,
                moves,
                bottom: "Z".to_string(),
            }))
        }
        MachineKind::Nfa => {
            let mut nfa = Nfa::new(spec.alphabet.clone());
            for name in &states {
                nfa.add_state(name.clone());
            }
            for (from, label, to, _) in edges {
                nfa.add_edge(from, label, to);
            }
            nfa.start = start;
            nfa.accept = accept;
            return Ok(Built::Nfa(nfa));
        }
        MachineKind::Dfa => {}
    }

    // A DFA needs exactly one transition per state and symbol.
    let mut delta: Vec<Vec<Option<usize>>> = vec![vec![None; spec.alphabet.len()]; states.len()];
    for (from, label, to, t) in edges {
        let symbol = label.expect("eps was rejected above");
        let column = spec.alphabet.iter().position(|&c| c == symbol).expect("checked above");
        match delta[from][column] {
            Some(existing) if existing != to => {
                return Err(machine_error(
                    t.span,
                    format!(
                        "state `{}` already goes to `{}` on `{symbol}`",
                        states[from], states[existing]
                    ),
                )
                .hint("a DFA has one transition per state and symbol; use `nfa` if you want a choice"));
            }
            _ => delta[from][column] = Some(to),
        }
    }
    let missing: Vec<String> = (0..states.len())
        .flat_map(|s| (0..spec.alphabet.len()).map(move |c| (s, c)))
        .filter(|&(s, c)| delta[s][c].is_none())
        .map(|(s, c)| format!("`{}` on `{}`", states[s], spec.alphabet[c]))
        .collect();
    if !missing.is_empty() {
        let shown = missing.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
        let more = match missing.len() {
            0..=3 => String::new(),
            n => format!(" and {} more", n - 3),
        };
        return Err(machine_error(
            spec.header,
            format!("{title} is not total: no transition from {shown}{more}"),
        )
        .hint("a DFA needs a transition from every state on every symbol; add the missing ones (often to a dead state), or use `nfa`"));
    }
    Ok(Built::Dfa(Dfa {
        alphabet: spec.alphabet.clone(),
        states,
        start,
        accept,
        delta: delta.into_iter().map(|row| row.into_iter().map(Option::unwrap).collect()).collect(),
    }))
}

pub struct GrammarSpec<'a> {
    pub name: Option<String>,
    pub alphabet: Vec<char>,
    pub alphabet_name: String,
    pub header: Span,
    pub rules: &'a [RuleDef],
}

/// Builds a grammar. The rules: there is at least one rule, every variable
/// used on a right-hand side has rules of its own, and every terminal is in
/// the alphabet. The first rule's variable is the start variable.
pub fn build_grammar(spec: &GrammarSpec) -> Result<Grammar, Diag> {
    if spec.rules.is_empty() {
        let title = spec.name.as_ref().map_or("this grammar".to_string(), |n| format!("grammar `{n}`"));
        return Err(Diag::new(ErrorKind::Machine, spec.header, format!("{title} has no rules"))
            .hint("add a rule such as `S -> a S b | eps`; the first rule's variable is the start variable"));
    }
    // Variables in order of their first rule, so the start variable is 0.
    let mut variables: Vec<String> = Vec::new();
    for rule in spec.rules {
        if !variables.contains(&rule.head.text) {
            variables.push(rule.head.text.clone());
        }
    }
    let mut rules = Vec::new();
    for rule in spec.rules {
        let head = variables.iter().position(|v| *v == rule.head.text).expect("added above");
        for alternative in &rule.alternatives {
            let mut body = Vec::new();
            for sym in alternative {
                body.push(match &sym.kind {
                    GSymKind::Terminal(c) => {
                        if !spec.alphabet.contains(c) {
                            return Err(foreign_symbol(*c, sym.span, &spec.alphabet, &spec.alphabet_name));
                        }
                        Sym::T(*c)
                    }
                    GSymKind::Variable(name) => match variables.iter().position(|v| v == name) {
                        Some(v) => Sym::V(v),
                        None => {
                            let mut hint = format!("add a rule such as `{name} -> ...`");
                            if name.chars().count() == 1 && spec.alphabet.contains(&name.chars().next().unwrap_or_default()) {
                                hint = format!(
                                    "`{name}` is in the alphabet, but a name starting with a capital letter is always a variable in a grammar; {hint}"
                                );
                            }
                            return Err(Diag::new(
                                ErrorKind::Name,
                                sym.span,
                                format!("variable `{name}` has no rules"),
                            )
                            .hint(hint));
                        }
                    },
                });
            }
            rules.push(Rule { head, body });
        }
    }
    Ok(Grammar { alphabet: spec.alphabet.clone(), variables, start: 0, rules })
}

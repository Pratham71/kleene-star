//! Turning a machine definition into a real `Dfa` or `Nfa`, and rejecting
//! definitions that are not valid machines.
//!
//! Both the checker (before the program runs) and the interpreter (for
//! machines built by loops) describe a machine as a `MachineSpec` and hand it
//! to `build`, so the rules live in exactly one place.

use super::error::{Diag, ErrorKind};
use super::token::Span;
use crate::automata::{fmt_alphabet, Dfa, Nfa};
use std::collections::BTreeSet;

pub struct Transition {
    pub from: String,
    pub labels: Vec<(Option<char>, Span)>,
    pub to: String,
    pub span: Span,
}

pub struct MachineSpec {
    pub is_dfa: bool,
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
}

pub fn build(spec: &MachineSpec) -> Result<Built, Diag> {
    let kind = if spec.is_dfa { "DFA" } else { "NFA" };
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
    for t in &spec.transitions {
        let (from, to) = (id(&t.from), id(&t.to));
        for &(label, span) in &t.labels {
            if let Some(c) = label {
                if !spec.alphabet.contains(&c) {
                    return Err(Diag::new(
                        ErrorKind::Alphabet,
                        span,
                        format!("symbol `{c}` is not in alphabet `{}`", spec.alphabet_name),
                    )
                    .hint(format!("{} = {}", spec.alphabet_name, fmt_alphabet(&spec.alphabet))));
                }
            } else if spec.is_dfa {
                return Err(machine_error(span, "a DFA cannot have eps transitions".into())
                    .hint("eps moves need nondeterminism; declare the machine with `nfa` instead"));
            }
            edges.push((from, label, to, t));
        }
    }
    let accept: BTreeSet<usize> = spec.accepts.iter().map(|(name, _)| id(name)).collect();

    if !spec.is_dfa {
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

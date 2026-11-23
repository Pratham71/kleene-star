//! A machine written as its formal definition, the tuple of the usual
//! textbook definition, with the transition function δ as a table:
//!
//! * DFA and NFA: (Q, Σ, δ, q0, F);
//! * PDA: (Q, Σ, Γ, δ, q0, Z0, F);
//! * Turing machine: (Q, Σ, Γ, δ, q0, q_accept, q_reject), with sets of
//!   accepting and rejecting states, since a machine here may have several.

use super::pda::Pda;
use super::tm::{Tm, BLANK};
use super::{pdas, render_table, tms, Dfa, Nfa, State};
use std::collections::{BTreeMap, BTreeSet};

fn set<'a>(items: impl IntoIterator<Item = &'a str>) -> String {
    let items: Vec<&str> = items.into_iter().collect();
    if items.is_empty() {
        "∅".to_string()
    } else {
        format!("{{{}}}", items.join(", "))
    }
}

fn names<'a>(states: &'a [String], set_of: &BTreeSet<State>) -> Vec<&'a str> {
    set_of.iter().map(|&s| states[s].as_str()).collect()
}

fn symbols(alphabet: &[char]) -> String {
    let shown: Vec<String> = alphabet.iter().map(char::to_string).collect();
    set(shown.iter().map(String::as_str))
}

/// The lines every tuple starts with, then δ as a table, indented.
fn layout(title: &str, parts: &[(&str, String)], delta: &[Vec<String>]) -> String {
    let mut lines = vec![title.to_string()];
    let width = parts.iter().map(|(name, _)| name.chars().count()).max().unwrap_or(0);
    for (name, value) in parts {
        if *name == "δ" {
            lines.push("δ:".to_string());
            lines.extend(render_table(delta).lines().map(|l| format!("  {l}")));
        } else {
            lines.push(format!("{name:width$} = {value}"));
        }
    }
    lines.join("\n")
}

pub fn dfa(d: &Dfa) -> String {
    let mut rows = vec![std::iter::once("δ".to_string()).chain(d.alphabet.iter().map(char::to_string)).collect::<Vec<_>>()];
    for (s, row) in d.delta.iter().enumerate() {
        rows.push(std::iter::once(d.states[s].clone()).chain(row.iter().map(|&t| d.states[t].clone())).collect());
    }
    layout(
        "DFA M = (Q, Σ, δ, q0, F)",
        &[
            ("Q", set(d.states.iter().map(String::as_str))),
            ("Σ", symbols(&d.alphabet)),
            ("δ", String::new()),
            ("q0", d.states[d.start].clone()),
            ("F", set(names(&d.states, &d.accept))),
        ],
        &rows,
    )
}

pub fn nfa(n: &Nfa) -> String {
    let mut labels: Vec<Option<char>> = n.alphabet.iter().map(|&c| Some(c)).collect();
    if n.has_eps() {
        labels.push(None);
    }
    let header = labels.iter().map(|l| l.map_or("eps".to_string(), |c| c.to_string()));
    let mut rows = vec![std::iter::once("δ".to_string()).chain(header).collect::<Vec<_>>()];
    for (s, edges) in n.delta.iter().enumerate() {
        let mut row = vec![n.states[s].clone()];
        for label in &labels {
            row.push(set(edges.get(label).map(|t| names(&n.states, t)).unwrap_or_default()));
        }
        rows.push(row);
    }
    layout(
        if n.has_eps() { "eps-NFA M = (Q, Σ, δ, q0, F)" } else { "NFA M = (Q, Σ, δ, q0, F)" },
        &[
            ("Q", set(n.states.iter().map(String::as_str))),
            ("Σ", symbols(&n.alphabet)),
            ("δ", String::new()),
            ("q0", n.states[n.start].clone()),
            ("F", set(names(&n.states, &n.accept))),
        ],
        &rows,
    )
}

/// δ(q, a, X) = {(p, γ), ...}: one row for each state, input (or eps)
/// and popped symbol (or eps) that has a move.
pub fn pda(p: &Pda) -> String {
    let mut delta: BTreeMap<(State, Option<char>, Option<String>), Vec<String>> = BTreeMap::new();
    for m in &p.moves {
        let pair = format!("({}, {})", p.states[m.to], p.show_stack(&m.push));
        delta.entry((m.from, m.read, m.pop.clone())).or_default().push(pair);
    }
    let mut rows = vec![vec!["δ(q, a, X)".to_string(), "moves".to_string()]];
    for ((q, a, x), moves) in &delta {
        rows.push(vec![
            format!("δ({}, {}, {})", p.states[*q], a.map_or("eps".to_string(), |c| c.to_string()), x.as_deref().unwrap_or("eps")),
            format!("{{{}}}", moves.join(", ")),
        ]);
    }
    let gamma: Vec<String> = pdas::stack_symbols(p).into_iter().collect();
    layout(
        "PDA M = (Q, Σ, Γ, δ, q0, Z0, F)",
        &[
            ("Q", set(p.states.iter().map(String::as_str))),
            ("Σ", symbols(&p.alphabet)),
            ("Γ", set(gamma.iter().map(String::as_str))),
            ("δ", String::new()),
            ("q0", p.states[p.start].clone()),
            ("Z0", p.bottom.clone().unwrap_or_else(|| "none: the stack starts empty".to_string())),
            ("F", if p.empty_stack { "∅: it accepts by empty stack".to_string() } else { set(names(&p.states, &p.accept)) }),
        ],
        &rows,
    )
}

/// δ(q, X) = (p, Y, D) for each state and tape symbol, `-` where the
/// machine has no move (and so halts).
pub fn tm(m: &Tm) -> String {
    let gamma: Vec<char> = tms::tape_symbols(m).into_iter().collect();
    let mut rows = vec![std::iter::once("δ".to_string()).chain(gamma.iter().map(char::to_string)).collect::<Vec<_>>()];
    for (s, name) in m.states.iter().enumerate() {
        let mut row = vec![name.clone()];
        for &c in &gamma {
            row.push(match m.move_for(s, c) {
                Some(i) => {
                    let mv = &m.moves[i];
                    format!("({}, {}, {})", m.states[mv.to], mv.write, mv.dir.letter())
                }
                None => "-".to_string(),
            });
        }
        rows.push(row);
    }
    let shown: Vec<String> = gamma.iter().map(char::to_string).collect();
    layout(
        "TM M = (Q, Σ, Γ, δ, q0, q_accept, q_reject)",
        &[
            ("Q", set(m.states.iter().map(String::as_str))),
            ("Σ", symbols(&m.alphabet)),
            ("Γ", format!("{} (the blank is {BLANK})", set(shown.iter().map(String::as_str)))),
            ("δ", String::new()),
            ("q0", m.states[m.start].clone()),
            ("q_accept", set(names(&m.states, &m.accept))),
            ("q_reject", set(names(&m.states, &m.reject))),
        ],
        &rows,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::dfa::tests::even_zeros;
    use crate::automata::nfa::tests::ends_in_01;

    #[test]
    fn a_dfa_as_a_five_tuple() {
        let text = dfa(&even_zeros());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "DFA M = (Q, Σ, δ, q0, F)");
        assert!(lines[1].starts_with("Q  = {"), "{text}");
        assert!(text.contains("δ:\n  δ "), "{text}");
        assert!(lines.last().unwrap().starts_with("F  = {"), "{text}");
    }

    #[test]
    fn an_nfa_shows_sets_and_empty_sets() {
        let text = nfa(&ends_in_01());
        assert!(text.contains("∅"), "{text}");
        assert!(text.starts_with("NFA M = (Q, Σ, δ, q0, F)"), "{text}");
    }

    #[test]
    fn a_pda_and_a_tm() {
        let p = pdas::anbn('a', 'b');
        let text = pda(&p);
        assert!(text.contains("Z0 = Z"), "{text}");
        assert!(text.contains("δ(ready, a, Z) | {(as, XZ)}"), "{text}");
        let m = tms::palindromes(&['a', 'b']);
        let text = tm(&m);
        assert!(text.contains("(the blank is _)"), "{text}");
        assert_eq!(text.lines().filter(|l| l.starts_with("  ")).count(), m.states.len() + 1, "{text}");
    }
}

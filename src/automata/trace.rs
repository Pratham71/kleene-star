//! Running a machine one symbol at a time and keeping a record of every step.
//!
//! `accepts` only answers yes or no. A trace also shows *how* the machine got
//! there: which state (or, for an NFA, which set of states) it was in before
//! and after each symbol. The language prints a trace as a table; this file
//! only produces the steps and knows nothing about how they are shown.

use super::{Dfa, Nfa, State};
use std::collections::BTreeSet;

/// One symbol read: where the machine was, and where it went.
///
/// `S` is what the machine is "in": one `State` for a DFA, a set of states
/// for an NFA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step<S> {
    pub symbol: char,
    pub from: S,
    pub to: S,
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// Every symbol was read and the machine accepts.
    Accepted,
    /// Every symbol was read and the machine does not accept.
    Rejected,
    /// The run stopped at a symbol outside the alphabet. `position` counts
    /// symbols from 1, so it matches what a person reading the word would say.
    Foreign { position: usize, symbol: char },
}

/// The complete record of one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace<S> {
    /// Where the machine is before reading anything.
    pub start: S,
    pub steps: Vec<Step<S>>,
    pub end: End,
}

impl<S: Clone> Trace<S> {
    /// Where the machine was when the run ended.
    pub fn last(&self) -> &S {
        self.steps.last().map_or(&self.start, |step| &step.to)
    }
}

/// Runs a DFA on `word`, recording each move.
pub fn trace_dfa(dfa: &Dfa, word: &str) -> Trace<State> {
    let mut trace = Trace { start: dfa.start, steps: Vec::new(), end: End::Rejected };
    let mut state = dfa.start;
    for (i, symbol) in word.chars().enumerate() {
        let Some(column) = dfa.symbol_index(symbol) else {
            trace.end = End::Foreign { position: i + 1, symbol };
            return trace;
        };
        let next = dfa.delta[state][column];
        trace.steps.push(Step { symbol, from: state, to: next });
        state = next;
    }
    if dfa.accept.contains(&state) {
        trace.end = End::Accepted;
    }
    trace
}

/// Runs an NFA on `word`, recording the set of states it could be in before
/// and after each symbol. Every set already includes the states reachable by
/// eps moves, so the sets are exactly what the subset construction would build.
pub fn trace_nfa(nfa: &Nfa, word: &str) -> Trace<BTreeSet<State>> {
    let start = nfa.eps_closure(&BTreeSet::from([nfa.start]));
    let mut trace = Trace { start: start.clone(), steps: Vec::new(), end: End::Rejected };
    let mut current = start;
    for (i, symbol) in word.chars().enumerate() {
        if !nfa.alphabet.contains(&symbol) {
            trace.end = End::Foreign { position: i + 1, symbol };
            return trace;
        }
        let next = nfa.step(&current, symbol);
        trace.steps.push(Step { symbol, from: current, to: next.clone() });
        current = next;
    }
    if current.iter().any(|s| nfa.accept.contains(s)) {
        trace.end = End::Accepted;
    }
    trace
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::dfa::tests::even_zeros;
    use crate::automata::nfa::tests::ends_in_01;

    #[test]
    fn dfa_trace_records_every_move() {
        // even_zeros: q0 --0--> q1, q0 --1--> q0, q1 --0--> q0, q1 --1--> q1
        let trace = trace_dfa(&even_zeros(), "010");
        assert_eq!(trace.start, 0);
        assert_eq!(
            trace.steps,
            vec![
                Step { symbol: '0', from: 0, to: 1 },
                Step { symbol: '1', from: 1, to: 1 },
                Step { symbol: '0', from: 1, to: 0 },
            ]
        );
        assert_eq!(trace.end, End::Accepted);
        assert_eq!(*trace.last(), 0);
    }

    #[test]
    fn dfa_trace_of_empty_word_stays_at_start() {
        let trace = trace_dfa(&even_zeros(), "");
        assert!(trace.steps.is_empty());
        assert_eq!(trace.end, End::Accepted);
        assert_eq!(trace_dfa(&even_zeros(), "0").end, End::Rejected);
    }

    #[test]
    fn dfa_trace_stops_at_a_foreign_symbol() {
        let trace = trace_dfa(&even_zeros(), "01x0");
        assert_eq!(trace.steps.len(), 2);
        assert_eq!(trace.end, End::Foreign { position: 3, symbol: 'x' });
    }

    #[test]
    fn nfa_trace_tracks_sets_of_states() {
        // ends_in_01: a --0,1--> a, a --0--> b, b --1--> c; c accepts
        let trace = trace_nfa(&ends_in_01(), "01");
        let set = |s: &[State]| s.iter().copied().collect::<BTreeSet<State>>();
        assert_eq!(trace.start, set(&[0]));
        assert_eq!(trace.steps[0].to, set(&[0, 1]));
        assert_eq!(trace.steps[1].to, set(&[0, 2]));
        assert_eq!(trace.end, End::Accepted);
        assert_eq!(trace_nfa(&ends_in_01(), "10").end, End::Rejected);
        assert_eq!(trace_nfa(&ends_in_01(), "2").end, End::Foreign { position: 1, symbol: '2' });
    }

    #[test]
    fn nfa_trace_includes_eps_closure_from_the_start() {
        let mut n = Nfa::new(vec!['1']);
        let (a, b) = (n.add_state("a"), n.add_state("b"));
        n.add_edge(a, None, b);
        n.accept.insert(b);
        let trace = trace_nfa(&n, "");
        assert_eq!(trace.start, BTreeSet::from([a, b]));
        assert_eq!(trace.end, End::Accepted);
        // Reading `1` leaves no possible state at all.
        let stuck = trace_nfa(&n, "1");
        assert!(stuck.last().is_empty());
        assert_eq!(stuck.end, End::Rejected);
    }
}

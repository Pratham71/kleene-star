//! Pushdown automata: finite automata with a stack.
//!
//! Each move reads one input symbol (or nothing, for an eps move), pops one
//! symbol off the stack (or nothing), pushes a string of symbols, and changes
//! state. Like an NFA, a PDA may have several moves to choose from; it
//! accepts a word if *some* sequence of moves reads all of it and ends in an
//! accepting state. (This is acceptance by final state. What is left on the
//! stack does not matter.)
//!
//! The stack starts holding one bottom symbol, `Z` unless chosen otherwise,
//! so a PDA can tell when its stack is "empty".
//!
//! Running a PDA means searching through configurations (state, how much
//! input is read, stack contents). The search is breadth-first, so it finds a
//! shortest accepting run. Because eps moves can push forever, the search
//! stops after a fixed number of configurations and says so.

use super::grammar::{Grammar, Sym};
use super::{fmt_alphabet, fresh_name, render_table, State};
use std::collections::{BTreeSet, HashSet, VecDeque};
use std::fmt;

/// How many configurations `accepts` and `trace` look at before giving up.
pub const STEP_LIMIT: usize = 50_000;
/// The tallest stack the search allows. Runs that would push past it are
/// cut off, and an answer that depended on them is reported as unknown.
pub const MAX_STACK: usize = 1_000;

/// One move: `from --read, pop/push--> to`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Move {
    pub from: State,
    /// The input symbol read, or `None` for an eps move.
    pub read: Option<char>,
    /// The stack symbol popped, or `None` to pop nothing.
    pub pop: Option<String>,
    /// The symbols pushed, top of the stack first. Empty pushes nothing.
    pub push: Vec<String>,
    pub to: State,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pda {
    /// The input alphabet. Sorted, without duplicates.
    pub alphabet: Vec<char>,
    pub states: Vec<String>,
    pub start: State,
    pub accept: BTreeSet<State>,
    pub moves: Vec<Move>,
    /// The symbol the stack holds at the start.
    pub bottom: String,
}

/// A snapshot of a run: the state, how many input symbols have been read,
/// and the stack (bottom first, so the top is the last element).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Config {
    pub state: State,
    pub read: usize,
    pub stack: Vec<String>,
}

/// The result of searching for an accepting run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Search {
    /// The configurations of the run found, from the start. For an accepted
    /// word this is a shortest accepting run; otherwise it is a run that read
    /// as much of the input as any run could.
    pub path: Vec<Config>,
    /// The move taken into each configuration after the first
    /// (an index into `Pda::moves`).
    pub moves: Vec<usize>,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Accepted,
    /// Every possible run was tried; none accepts.
    Rejected,
    /// The search stopped after `STEP_LIMIT` configurations without finding
    /// an accepting run, so the answer is unknown.
    GaveUp,
    /// A symbol outside the alphabet, at this position (counting from 1).
    Foreign { position: usize, symbol: char },
}

impl Pda {
    /// Shows a stack symbol string: `AZ` when every symbol is one character,
    /// `S0 Z` (with spaces) otherwise, `eps` when empty.
    pub fn show_symbols(symbols: &[String]) -> String {
        if symbols.is_empty() {
            "eps".to_string()
        } else if symbols.iter().all(|s| s.chars().count() == 1) {
            symbols.concat()
        } else {
            symbols.join(" ")
        }
    }

    /// The label of a move, as written in source: `a, Z/AZ`.
    pub fn label(&self, m: &Move) -> String {
        format!(
            "{}, {}/{}",
            m.read.map_or("eps".to_string(), |c| c.to_string()),
            m.pop.as_deref().unwrap_or("eps"),
            Self::show_symbols(&m.push)
        )
    }

    /// Searches breadth-first for an accepting run on `word`, looking at no
    /// more than `limit` configurations and never letting the stack grow
    /// past `MAX_STACK` symbols.
    pub fn search(&self, word: &str, limit: usize) -> Search {
        let word: Vec<char> = word.chars().collect();
        if let Some(position) = word.iter().position(|c| !self.alphabet.contains(c)) {
            let start = Config { state: self.start, read: 0, stack: vec![self.bottom.clone()] };
            let outcome = Outcome::Foreign { position: position + 1, symbol: word[position] };
            return Search { path: vec![start], moves: Vec::new(), outcome };
        }

        // Inside the search, stack symbols are small numbers: copying and
        // comparing stacks of numbers is much cheaper than of strings.
        let mut symbols: Vec<String> = vec![self.bottom.clone()];
        let mut id = |s: &String| match symbols.iter().position(|t| t == s) {
            Some(i) => i as u32,
            None => {
                symbols.push(s.clone());
                (symbols.len() - 1) as u32
            }
        };
        // Each move as (read, pop, symbols to push in push order).
        let compiled: Vec<(Option<char>, Option<u32>, Vec<u32>)> = self
            .moves
            .iter()
            .map(|m| (m.read, m.pop.as_ref().map(&mut id), m.push.iter().rev().map(&mut id).collect()))
            .collect();

        // Every configuration found, with the index of the one it came from
        // and the move that led to it, so a run can be rebuilt at the end.
        type Found = (State, usize, Vec<u32>);
        let start: Found = (self.start, 0, vec![0]);
        let mut found: Vec<(Found, Option<(usize, usize)>)> = vec![(start.clone(), None)];
        let mut seen = HashSet::from([start]);
        let mut queue = VecDeque::from([0]);
        let mut furthest = 0;
        let mut outcome = Outcome::Rejected;
        let mut last = 0;
        'search: while let Some(i) = queue.pop_front() {
            let (state, read, stack) = found[i].0.clone();
            if read > found[furthest].0 .1 {
                furthest = i;
            }
            if read == word.len() && self.accept.contains(&state) {
                outcome = Outcome::Accepted;
                last = i;
                break;
            }
            for (m, mv) in self.moves.iter().enumerate().filter(|(_, mv)| mv.from == state) {
                let (read_symbol, pop, push) = &compiled[m];
                let mut next_read = read;
                if let Some(c) = read_symbol {
                    if word.get(read) != Some(c) {
                        continue;
                    }
                    next_read += 1;
                }
                let mut next_stack = stack.clone();
                if let Some(pop) = pop {
                    if next_stack.last() != Some(pop) {
                        continue;
                    }
                    next_stack.pop();
                }
                next_stack.extend(push);
                if next_stack.len() > MAX_STACK {
                    // Cut off: the answer can no longer be a sure "no".
                    outcome = Outcome::GaveUp;
                    continue;
                }
                let next = (mv.to, next_read, next_stack);
                if seen.insert(next.clone()) {
                    if found.len() >= limit {
                        outcome = Outcome::GaveUp;
                        break 'search;
                    }
                    found.push((next, Some((i, m))));
                    queue.push_back(found.len() - 1);
                }
            }
        }
        if outcome != Outcome::Accepted {
            last = furthest;
        }

        let mut path = Vec::new();
        let mut moves = Vec::new();
        let mut at = Some(last);
        while let Some(i) = at {
            let (state, read, stack) = &found[i].0;
            let stack = stack.iter().map(|&s| symbols[s as usize].clone()).collect();
            path.push(Config { state: *state, read: *read, stack });
            at = found[i].1.map(|(parent, m)| {
                moves.push(m);
                parent
            });
        }
        path.reverse();
        moves.reverse();
        Search { path, moves, outcome }
    }

    /// The standard PDA for a grammar. It keeps the part of a leftmost
    /// derivation that is still to be matched on its stack:
    ///
    /// * `start --eps, Z/S Z--> loop` puts the start variable on the stack;
    /// * `loop --eps, A/w--> loop` for each rule `A -> w` expands a variable;
    /// * `loop --a, a/eps--> loop` matches a terminal with the input;
    /// * `loop --eps, Z/Z--> accept` finishes once only the bottom is left.
    pub fn from_grammar(g: &Grammar) -> Pda {
        let bottom = fresh_name(&g.variables, "Z");
        let sym = |s: &Sym| match s {
            Sym::T(c) => c.to_string(),
            Sym::V(v) => g.variables[*v].clone(),
        };
        let (start, looping, accept) = (0, 1, 2);
        let mut moves = vec![Move {
            from: start,
            read: None,
            pop: Some(bottom.clone()),
            push: vec![g.variables[g.start].clone(), bottom.clone()],
            to: looping,
        }];
        for rule in &g.rules {
            moves.push(Move {
                from: looping,
                read: None,
                pop: Some(g.variables[rule.head].clone()),
                push: rule.body.iter().map(sym).collect(),
                to: looping,
            });
        }
        for &c in &g.alphabet {
            moves.push(Move { from: looping, read: Some(c), pop: Some(c.to_string()), push: Vec::new(), to: looping });
        }
        moves.push(Move { from: looping, read: None, pop: Some(bottom.clone()), push: vec![bottom.clone()], to: accept });
        Pda {
            alphabet: g.alphabet.clone(),
            states: vec!["start".into(), "loop".into(), "accept".into()],
            start,
            accept: BTreeSet::from([accept]),
            moves,
            bottom,
        }
    }
}

impl fmt::Display for Pda {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let n = self.states.len();
        let accept: Vec<&str> = self.accept.iter().map(|&s| self.states[s].as_str()).collect();
        writeln!(
            f,
            "PDA with {} state{} over {}, stack starts as {}",
            n,
            if n == 1 { "" } else { "s" },
            fmt_alphabet(&self.alphabet),
            self.bottom
        )?;
        write!(
            f,
            "start {}, accept {}",
            self.states[self.start],
            if accept.is_empty() { "nothing".to_string() } else { accept.join(", ") }
        )?;
        if self.moves.is_empty() {
            return Ok(());
        }
        let mut rows = vec![["From", "Read", "Pop", "Push", "To"].map(String::from).to_vec()];
        for m in &self.moves {
            rows.push(vec![
                self.states[m.from].clone(),
                m.read.map_or("eps".into(), |c| c.to_string()),
                m.pop.clone().unwrap_or_else(|| "eps".into()),
                Self::show_symbols(&m.push),
                self.states[m.to].clone(),
            ]);
        }
        write!(f, "\n{}", render_table(&rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::grammar::tests::anbn;

    /// a^n b^n: push an A for each a, pop one for each b.
    fn anbn_pda() -> Pda {
        let s = |x: &str| x.to_string();
        Pda {
            alphabet: vec!['a', 'b'],
            states: vec![s("p"), s("q"), s("f")],
            start: 0,
            accept: BTreeSet::from([2]),
            bottom: s("Z"),
            moves: vec![
                Move { from: 0, read: Some('a'), pop: None, push: vec![s("A")], to: 0 },
                Move { from: 0, read: None, pop: None, push: vec![], to: 1 },
                Move { from: 1, read: Some('b'), pop: Some(s("A")), push: vec![], to: 1 },
                Move { from: 1, read: None, pop: Some(s("Z")), push: vec![s("Z")], to: 2 },
            ],
        }
    }

    #[test]
    fn accepts_by_final_state() {
        let p = anbn_pda();
        for (word, expected) in [("", true), ("ab", true), ("aabb", true), ("aab", false), ("abab", false)] {
            let found = p.search(word, STEP_LIMIT).outcome == Outcome::Accepted;
            assert_eq!(found, expected, "{word}");
        }
        assert_eq!(p.search("ac", STEP_LIMIT).outcome, Outcome::Foreign { position: 2, symbol: 'c' });
    }

    #[test]
    fn search_returns_the_run() {
        let run = anbn_pda().search("ab", STEP_LIMIT);
        let states: Vec<usize> = run.path.iter().map(|c| c.state).collect();
        assert_eq!(states, [0, 0, 1, 1, 2]);
        assert_eq!(run.moves, [0, 1, 2, 3]);
        assert_eq!(run.path[1].stack, ["Z", "A"]);
        // Rejected: the run shown read as much as possible ("ab" of "abb").
        let run = anbn_pda().search("abb", STEP_LIMIT);
        assert_eq!(run.outcome, Outcome::Rejected);
        assert_eq!(run.path.last().unwrap().read, 2);
    }

    #[test]
    fn gives_up_on_endless_pushing() {
        let mut p = anbn_pda();
        // An eps move that pushes forever, and no way to accept.
        p.moves = vec![Move { from: 0, read: None, pop: None, push: vec!["A".into()], to: 0 }];
        assert_eq!(p.search("a", 1000).outcome, Outcome::GaveUp);
    }

    #[test]
    fn pda_from_grammar_accepts_the_same_words() {
        let g = anbn();
        let p = Pda::from_grammar(&g);
        for word in ["", "ab", "aabb", "aaabbb", "a", "ba", "aabbb"] {
            let accepted = p.search(word, STEP_LIMIT).outcome == Outcome::Accepted;
            assert_eq!(Ok(accepted), g.accepts(word), "{word}");
        }
        assert!(p.to_string().contains("loop  | eps  | S   | aSb  | loop"), "{p}");
    }
}

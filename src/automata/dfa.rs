//! Deterministic finite automata.

use super::{fmt_alphabet, render_table, row_label, State};
use std::collections::{BTreeSet, VecDeque};
use std::fmt;

/// A complete DFA: every state has exactly one transition per symbol.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Dfa {
    /// Sorted, without duplicates.
    pub alphabet: Vec<char>,
    /// State names; a state is identified by its index in this list.
    pub states: Vec<String>,
    pub start: State,
    pub accept: BTreeSet<State>,
    /// `delta[state][i]` is the target on `alphabet[i]`.
    pub delta: Vec<Vec<State>>,
}

impl Dfa {
    pub fn symbol_index(&self, symbol: char) -> Option<usize> {
        self.alphabet.iter().position(|&c| c == symbol)
    }

    /// Runs the machine on `word`. `Err(c)` means `c` is not in the alphabet.
    pub fn accepts(&self, word: &str) -> Result<bool, char> {
        let mut state = self.start;
        for c in word.chars() {
            let i = self.symbol_index(c).ok_or(c)?;
            state = self.delta[state][i];
        }
        Ok(self.accept.contains(&state))
    }

    /// States reachable from the start, in breadth-first order.
    pub fn reachable(&self) -> Vec<State> {
        let mut seen = vec![false; self.states.len()];
        let mut order = Vec::new();
        let mut queue = VecDeque::from([self.start]);
        seen[self.start] = true;
        while let Some(s) = queue.pop_front() {
            order.push(s);
            for &t in &self.delta[s] {
                if !seen[t] {
                    seen[t] = true;
                    queue.push_back(t);
                }
            }
        }
        order
    }

    /// Builds a copy containing only the states in `order` (in that order),
    /// with the given new names. `order` must be closed under transitions.
    pub fn remap(&self, order: &[State], names: Vec<String>) -> Dfa {
        let mut new_index = vec![usize::MAX; self.states.len()];
        for (new, &old) in order.iter().enumerate() {
            new_index[old] = new;
        }
        Dfa {
            alphabet: self.alphabet.clone(),
            states: names,
            start: new_index[self.start],
            accept: order
                .iter()
                .filter(|s| self.accept.contains(s))
                .map(|&s| new_index[s])
                .collect(),
            delta: order
                .iter()
                .map(|&s| self.delta[s].iter().map(|&t| new_index[t]).collect())
                .collect(),
        }
    }

    /// Renames states to `q0, q1, ...` in breadth-first order from the start.
    /// Unreachable states are kept and numbered last.
    pub fn renamed(&self) -> Dfa {
        let mut order = self.reachable();
        for s in 0..self.states.len() {
            if !order.contains(&s) {
                order.push(s);
            }
        }
        let names = (0..order.len()).map(|i| format!("q{i}")).collect();
        self.remap(&order, names)
    }
}

impl fmt::Display for Dfa {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let n = self.states.len();
        writeln!(
            f,
            "DFA with {} state{} over {}",
            n,
            if n == 1 { "" } else { "s" },
            fmt_alphabet(&self.alphabet)
        )?;
        let mut rows = vec![std::iter::once(String::new())
            .chain(self.alphabet.iter().map(|c| c.to_string()))
            .collect::<Vec<_>>()];
        for (s, name) in self.states.iter().enumerate() {
            let mut row = vec![row_label(name, s == self.start, self.accept.contains(&s))];
            row.extend(self.delta[s].iter().map(|&t| self.states[t].clone()));
            rows.push(row);
        }
        write!(f, "{}", render_table(&rows))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Strings over {0,1} with an even number of 0s.
    pub fn even_zeros() -> Dfa {
        Dfa {
            alphabet: vec!['0', '1'],
            states: vec!["q0".into(), "q1".into()],
            start: 0,
            accept: BTreeSet::from([0]),
            delta: vec![vec![1, 0], vec![0, 1]],
        }
    }

    #[test]
    fn accepts_even_zeros() {
        let d = even_zeros();
        assert_eq!(d.accepts(""), Ok(true));
        assert_eq!(d.accepts("0011"), Ok(true));
        assert_eq!(d.accepts("0"), Ok(false));
        assert_eq!(d.accepts("10101"), Ok(true));
    }

    #[test]
    fn rejects_foreign_symbol() {
        assert_eq!(even_zeros().accepts("02"), Err('2'));
    }

    #[test]
    fn renamed_keeps_language() {
        let d = even_zeros().renamed();
        assert_eq!(d.states, vec!["q0", "q1"]);
        assert_eq!(d.accepts("00"), Ok(true));
    }

    #[test]
    fn prints_a_table() {
        let text = even_zeros().to_string();
        assert!(text.contains("-> *q0 | q1 | q0"), "{text}");
    }
}

//! Nondeterministic finite automata, with or without epsilon moves.

use super::{fmt_alphabet, render_table, row_label, State};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

/// An NFA. A transition label is `Some(symbol)` or `None` for an epsilon move,
/// so an "epsilon-NFA" is simply an `Nfa` that has some `None` labels.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Nfa {
    /// Sorted, without duplicates.
    pub alphabet: Vec<char>,
    pub states: Vec<String>,
    pub start: State,
    pub accept: BTreeSet<State>,
    /// `delta[state][label]` is the set of targets.
    pub delta: Vec<BTreeMap<Option<char>, BTreeSet<State>>>,
}

impl Nfa {
    /// An NFA with no states yet. Add at least one state before using it.
    pub fn new(alphabet: Vec<char>) -> Nfa {
        Nfa { alphabet, states: Vec::new(), start: 0, accept: BTreeSet::new(), delta: Vec::new() }
    }

    pub fn add_state(&mut self, name: impl Into<String>) -> State {
        self.states.push(name.into());
        self.delta.push(BTreeMap::new());
        self.states.len() - 1
    }

    pub fn add_edge(&mut self, from: State, label: Option<char>, to: State) {
        self.delta[from].entry(label).or_default().insert(to);
    }

    pub fn has_eps(&self) -> bool {
        self.delta.iter().any(|edges| edges.contains_key(&None))
    }

    /// Everything reachable from `states` using only epsilon moves.
    pub fn eps_closure(&self, states: &BTreeSet<State>) -> BTreeSet<State> {
        let mut closure = states.clone();
        let mut stack: Vec<State> = states.iter().copied().collect();
        while let Some(s) = stack.pop() {
            if let Some(targets) = self.delta[s].get(&None) {
                for &t in targets {
                    if closure.insert(t) {
                        stack.push(t);
                    }
                }
            }
        }
        closure
    }

    /// One step on `symbol` from a set of states, followed by epsilon closure.
    pub fn step(&self, states: &BTreeSet<State>, symbol: char) -> BTreeSet<State> {
        let mut moved = BTreeSet::new();
        for &s in states {
            if let Some(targets) = self.delta[s].get(&Some(symbol)) {
                moved.extend(targets);
            }
        }
        self.eps_closure(&moved)
    }

    /// Runs the machine on `word` by tracking the set of states it could be in.
    /// `Err(c)` means `c` is not in the alphabet.
    pub fn accepts(&self, word: &str) -> Result<bool, char> {
        let mut current = self.eps_closure(&BTreeSet::from([self.start]));
        for c in word.chars() {
            if !self.alphabet.contains(&c) {
                return Err(c);
            }
            current = self.step(&current, c);
        }
        Ok(current.iter().any(|s| self.accept.contains(s)))
    }

    /// States reachable from the start (over any label), in breadth-first order.
    pub fn reachable(&self) -> Vec<State> {
        let mut seen = vec![false; self.states.len()];
        let mut order = Vec::new();
        let mut queue = VecDeque::from([self.start]);
        seen[self.start] = true;
        while let Some(s) = queue.pop_front() {
            order.push(s);
            for targets in self.delta[s].values() {
                for &t in targets {
                    if !seen[t] {
                        seen[t] = true;
                        queue.push_back(t);
                    }
                }
            }
        }
        order
    }

    /// Builds a copy containing only the states in `order` (in that order) with
    /// new names. Edges to dropped states are dropped too.
    pub fn remap(&self, order: &[State], names: Vec<String>) -> Nfa {
        let mut new_index = vec![usize::MAX; self.states.len()];
        for (new, &old) in order.iter().enumerate() {
            new_index[old] = new;
        }
        let mut out = Nfa::new(self.alphabet.clone());
        for name in names {
            out.add_state(name);
        }
        for (new, &old) in order.iter().enumerate() {
            for (label, targets) in &self.delta[old] {
                for &t in targets {
                    if new_index[t] != usize::MAX {
                        out.add_edge(new, *label, new_index[t]);
                    }
                }
            }
            if self.accept.contains(&old) {
                out.accept.insert(new);
            }
        }
        out.start = new_index[self.start];
        out
    }

    /// Renames states to `q0, q1, ...` in breadth-first order from the start.
    pub fn renamed(&self) -> Nfa {
        let mut order = self.reachable();
        // Then the unreachable states. (A `seen` table, not `order.contains`,
        // which made renaming a big machine take quadratic time.)
        let mut seen = vec![false; self.states.len()];
        for &s in &order {
            seen[s] = true;
        }
        order.extend((0..self.states.len()).filter(|&s| !seen[s]));
        let names = (0..order.len()).map(|i| format!("q{i}")).collect();
        self.remap(&order, names)
    }
}

impl fmt::Display for Nfa {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let n = self.states.len();
        let eps = self.has_eps();
        writeln!(
            f,
            "{} with {} state{} over {}",
            if eps { "eps-NFA" } else { "NFA" },
            n,
            if n == 1 { "" } else { "s" },
            fmt_alphabet(&self.alphabet)
        )?;
        let mut labels: Vec<Option<char>> = self.alphabet.iter().map(|&c| Some(c)).collect();
        if eps {
            labels.push(None);
        }
        let mut header = vec![String::new()];
        header.extend(labels.iter().map(|l| l.map_or("eps".to_string(), |c| c.to_string())));
        let mut rows = vec![header];
        for (s, name) in self.states.iter().enumerate() {
            let mut row = vec![row_label(name, s == self.start, self.accept.contains(&s))];
            for label in &labels {
                row.push(match self.delta[s].get(label) {
                    Some(targets) if !targets.is_empty() => {
                        let names: Vec<&str> =
                            targets.iter().map(|&t| self.states[t].as_str()).collect();
                        format!("{{{}}}", names.join(", "))
                    }
                    _ => "-".to_string(),
                });
            }
            rows.push(row);
        }
        write!(f, "{}", render_table(&rows))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Strings over {0,1} that end in 01.
    pub fn ends_in_01() -> Nfa {
        let mut n = Nfa::new(vec!['0', '1']);
        let (a, b, c) = (n.add_state("a"), n.add_state("b"), n.add_state("c"));
        n.add_edge(a, Some('0'), a);
        n.add_edge(a, Some('1'), a);
        n.add_edge(a, Some('0'), b);
        n.add_edge(b, Some('1'), c);
        n.accept.insert(c);
        n
    }

    #[test]
    fn accepts_ends_in_01() {
        let n = ends_in_01();
        assert_eq!(n.accepts("01"), Ok(true));
        assert_eq!(n.accepts("11001"), Ok(true));
        assert_eq!(n.accepts("010"), Ok(false));
        assert_eq!(n.accepts(""), Ok(false));
        assert_eq!(n.accepts("2"), Err('2'));
    }

    #[test]
    fn follows_eps_moves() {
        // a --eps--> b --1--> c, and c --eps--> a
        let mut n = Nfa::new(vec!['1']);
        let (a, b, c) = (n.add_state("a"), n.add_state("b"), n.add_state("c"));
        n.add_edge(a, None, b);
        n.add_edge(b, Some('1'), c);
        n.add_edge(c, None, a);
        n.accept.insert(c);
        assert_eq!(n.eps_closure(&BTreeSet::from([a])), BTreeSet::from([a, b]));
        assert_eq!(n.accepts("111"), Ok(true));
        assert_eq!(n.accepts(""), Ok(false));
        assert!(n.has_eps());
    }
}

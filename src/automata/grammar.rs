//! Context-free grammars.
//!
//! A grammar has variables (written with a capital first letter, like `S` or
//! `Expr`), terminals (the symbols of the alphabet), and rules such as
//! `S -> a S b`. A word is in the grammar's language if the start variable
//! can be rewritten, one rule at a time, into exactly that word.
//!
//! This file holds the grammar type and the algorithms on it:
//!
//! * `to_cnf`: an equivalent grammar in Chomsky normal form;
//! * `accepts`: membership, by the CYK algorithm on the CNF copy;
//! * `is_empty`: does the grammar generate any word at all?
//! * `derive`: a leftmost derivation of a word;
//! * `from_dfa`: the right-linear grammar of a regular language.

use super::{fmt_alphabet, fresh_name, Dfa};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt;

/// One symbol on the right-hand side of a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sym {
    /// A terminal: a symbol of the alphabet.
    T(char),
    /// A variable, by its index in `Grammar::variables`.
    V(usize),
}

/// A rule `head -> body`. An empty body is the rule `head -> eps`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rule {
    pub head: usize,
    pub body: Vec<Sym>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Grammar {
    /// Sorted, without duplicates.
    pub alphabet: Vec<char>,
    /// Variable names; a variable is identified by its index in this list.
    pub variables: Vec<String>,
    pub start: usize,
    /// In the order they were written.
    pub rules: Vec<Rule>,
}

/// Why `derive` could not produce a derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeriveError {
    /// The word is not in the language.
    NotInLanguage,
    /// The word is in the language, but the search for a derivation of it
    /// gave up after looking at this many sentential forms.
    TooManySteps(usize),
}

impl Grammar {
    /// The rules for variable `v`, in the order they were written.
    pub fn rules_for(&self, v: usize) -> impl Iterator<Item = &Rule> {
        self.rules.iter().filter(move |r| r.head == v)
    }

    /// Shows a sequence of symbols the way rules are written: `a S b`.
    /// The empty sequence is `eps`.
    pub fn show(&self, symbols: &[Sym]) -> String {
        if symbols.is_empty() {
            return "eps".to_string();
        }
        let parts: Vec<String> = symbols
            .iter()
            .map(|s| match s {
                Sym::T(c) => c.to_string(),
                Sym::V(v) => self.variables[*v].clone(),
            })
            .collect();
        parts.join(" ")
    }

    /// The variables that can be rewritten to the empty word.
    pub fn nullable(&self) -> Vec<bool> {
        let mut nullable = vec![false; self.variables.len()];
        loop {
            let mut changed = false;
            for rule in &self.rules {
                if !nullable[rule.head] && rule.body.iter().all(|s| matches!(s, Sym::V(v) if nullable[*v])) {
                    nullable[rule.head] = true;
                    changed = true;
                }
            }
            if !changed {
                return nullable;
            }
        }
    }

    /// The variables that can be rewritten into some word of terminals.
    pub fn generating(&self) -> Vec<bool> {
        let mut generating = vec![false; self.variables.len()];
        loop {
            let mut changed = false;
            for rule in &self.rules {
                if !generating[rule.head]
                    && rule.body.iter().all(|s| match s {
                        Sym::T(_) => true,
                        Sym::V(v) => generating[*v],
                    })
                {
                    generating[rule.head] = true;
                    changed = true;
                }
            }
            if !changed {
                return generating;
            }
        }
    }

    /// True if the grammar generates no word at all, which happens exactly
    /// when the start variable cannot be rewritten into terminals.
    pub fn is_empty(&self) -> bool {
        !self.generating()[self.start]
    }

    /// Is `word` in the language? `Err(c)` means `c` is not in the alphabet.
    pub fn accepts(&self, word: &str) -> Result<bool, char> {
        if let Some(c) = word.chars().find(|c| !self.alphabet.contains(c)) {
            return Err(c);
        }
        Ok(cyk(&self.to_cnf(), &word.chars().collect::<Vec<_>>()))
    }

    // ---- Chomsky normal form ----

    /// An equivalent grammar in Chomsky normal form: every rule is `A -> B C`
    /// (two variables) or `A -> a` (one terminal), plus `S -> eps` for the
    /// start variable if the language contains the empty word. The start
    /// variable never appears on a right-hand side.
    ///
    /// The steps, each keeping the language the same:
    ///
    /// 1. START: add a new start variable `S0 -> S`.
    /// 2. TERM: in bodies longer than one, replace each terminal `a` by a
    ///    new variable `T_a` with the rule `T_a -> a`.
    /// 3. BIN: split long bodies `A -> B C D` into `A -> B X1`, `X1 -> C D`.
    /// 4. DEL: remove eps rules, adding copies of rules with nullable
    ///    variables left out.
    /// 5. UNIT: replace unit rules `A -> B` by B's rules.
    /// 6. Remove variables that generate nothing or cannot be reached.
    pub fn to_cnf(&self) -> Grammar {
        let mut g = self.clone();

        // 1. START
        let old_start = g.start;
        let name = fresh_name(&g.variables, &format!("{}0", g.variables[old_start]));
        g.variables.push(name);
        g.start = g.variables.len() - 1;
        g.rules.insert(0, Rule { head: g.start, body: vec![Sym::V(old_start)] });

        // 2. TERM
        let mut term_var: HashMap<char, usize> = HashMap::new();
        let mut new_rules = Vec::new();
        for i in 0..g.rules.len() {
            if g.rules[i].body.len() < 2 {
                continue;
            }
            for j in 0..g.rules[i].body.len() {
                if let Sym::T(c) = g.rules[i].body[j] {
                    let v = *term_var.entry(c).or_insert_with(|| {
                        let name = fresh_name(&g.variables, &format!("T_{c}"));
                        g.variables.push(name);
                        new_rules.push(Rule { head: g.variables.len() - 1, body: vec![Sym::T(c)] });
                        g.variables.len() - 1
                    });
                    g.rules[i].body[j] = Sym::V(v);
                }
            }
        }
        g.rules.extend(new_rules);

        // 3. BIN
        let mut rules = Vec::new();
        let mut counter = 0;
        for rule in std::mem::take(&mut g.rules) {
            let mut head = rule.head;
            let mut body = rule.body;
            while body.len() > 2 {
                counter += 1;
                let name = fresh_name(&g.variables, &format!("X{counter}"));
                g.variables.push(name);
                let rest = g.variables.len() - 1;
                rules.push(Rule { head, body: vec![body[0], Sym::V(rest)] });
                head = rest;
                body.remove(0);
            }
            rules.push(Rule { head, body });
        }
        g.rules = rules;

        // 4. DEL
        let nullable = g.nullable();
        let mut rules: BTreeSet<Rule> = BTreeSet::new();
        let mut ordered = Vec::new();
        for rule in &g.rules {
            // Every way of leaving out nullable variables (bodies have at
            // most two symbols now, so at most four combinations).
            let optional: Vec<usize> = (0..rule.body.len())
                .filter(|&i| matches!(rule.body[i], Sym::V(v) if nullable[v]))
                .collect();
            for mask in 0..(1usize << optional.len()) {
                let body: Vec<Sym> = rule
                    .body
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| match optional.iter().position(|o| o == i) {
                        Some(bit) => mask & (1 << bit) == 0,
                        None => true,
                    })
                    .map(|(_, s)| *s)
                    .collect();
                if body.is_empty() && rule.head != g.start {
                    continue;
                }
                let new = Rule { head: rule.head, body };
                if rules.insert(new.clone()) {
                    ordered.push(new);
                }
            }
        }
        g.rules = ordered;

        // 5. UNIT: for each variable, the variables it reaches by unit rules.
        let n = g.variables.len();
        let mut unit_reach: Vec<BTreeSet<usize>> = (0..n).map(|v| BTreeSet::from([v])).collect();
        loop {
            let mut changed = false;
            for rule in &g.rules {
                if let [Sym::V(b)] = rule.body[..] {
                    let from_b = unit_reach[b].clone();
                    for reach in unit_reach.iter_mut().filter(|reach| reach.contains(&rule.head)) {
                        for &c in &from_b {
                            changed |= reach.insert(c);
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let mut seen = HashSet::new();
        let mut rules = Vec::new();
        for (a, reach) in unit_reach.iter().enumerate() {
            for &b in reach {
                for rule in g.rules.iter().filter(|r| r.head == b) {
                    if matches!(rule.body[..], [Sym::V(_)]) {
                        continue;
                    }
                    let new = Rule { head: a, body: rule.body.clone() };
                    if seen.insert(new.clone()) {
                        rules.push(new);
                    }
                }
            }
        }
        // Keep the start variable's rules first, then the rest in the order
        // of the original rules, so the result reads naturally.
        rules.sort_by_key(|r| (r.head != g.start, r.head));
        g.rules = rules;

        // 6. Remove useless variables.
        g.trimmed()
    }

    /// A copy without variables that generate nothing or cannot be reached
    /// from the start, renumbered in order of first use from the start.
    pub fn trimmed(&self) -> Grammar {
        let generating = self.generating();
        let useful_rules: Vec<&Rule> = self
            .rules
            .iter()
            .filter(|r| generating[r.head] && r.body.iter().all(|s| !matches!(s, Sym::V(v) if !generating[*v])))
            .collect();
        // Reachable variables, in breadth-first order from the start.
        let mut order = vec![self.start];
        let mut queue = VecDeque::from([self.start]);
        while let Some(v) = queue.pop_front() {
            for rule in useful_rules.iter().filter(|r| r.head == v) {
                for s in &rule.body {
                    if let Sym::V(w) = s {
                        if !order.contains(w) {
                            order.push(*w);
                            queue.push_back(*w);
                        }
                    }
                }
            }
        }
        let index = |v: usize| order.iter().position(|&w| w == v);
        let mut rules: Vec<Rule> = useful_rules
            .iter()
            .filter(|r| index(r.head).is_some())
            .map(|r| Rule {
                head: index(r.head).unwrap(),
                body: r
                    .body
                    .iter()
                    .map(|s| match s {
                        Sym::V(v) => Sym::V(index(*v).unwrap()),
                        t => *t,
                    })
                    .collect(),
            })
            .collect();
        rules.sort_by_key(|r| r.head); // stable: keeps each variable's rules in order
        Grammar {
            alphabet: self.alphabet.clone(),
            variables: order.iter().map(|&v| self.variables[v].clone()).collect(),
            start: 0,
            rules,
        }
    }

    // ---- derivations ----

    /// A leftmost derivation of `word`: the sentential forms from the start
    /// variable to the word, each made by rewriting the leftmost variable.
    ///
    /// The search is breadth-first, so it finds a shortest derivation. It
    /// throws away forms that cannot lead to the word: ones whose terminals
    /// before the first variable do not match the start of the word, and
    /// ones that already need more symbols than the word has.
    pub fn derive(&self, word: &str, limit: usize) -> Result<Vec<Vec<Sym>>, DeriveError> {
        let word: Vec<char> = word.chars().collect();
        if word.iter().any(|c| !self.alphabet.contains(c)) || !cyk(&self.to_cnf(), &word) {
            return Err(DeriveError::NotInLanguage);
        }
        let nullable = self.nullable();
        // The fewest terminals a form can still produce.
        let min_len = |form: &[Sym]| {
            form.iter().filter(|s| matches!(s, Sym::T(_)) || matches!(s, Sym::V(v) if !nullable[*v])).count()
        };
        let start = vec![Sym::V(self.start)];
        let mut parent: HashMap<Vec<Sym>, Vec<Sym>> = HashMap::new();
        let mut queue = VecDeque::from([start.clone()]);
        let mut seen = HashSet::from([start]);
        while let Some(form) = queue.pop_front() {
            if seen.len() > limit {
                return Err(DeriveError::TooManySteps(limit));
            }
            let Some(at) = form.iter().position(|s| matches!(s, Sym::V(_))) else {
                if form.iter().map(|s| if let Sym::T(c) = s { *c } else { ' ' }).eq(word.iter().copied()) {
                    let mut steps = vec![form.clone()];
                    let mut cur = form;
                    while let Some(prev) = parent.get(&cur) {
                        steps.push(prev.clone());
                        cur = prev.clone();
                    }
                    steps.reverse();
                    return Ok(steps);
                }
                continue;
            };
            let Sym::V(v) = form[at] else { unreachable!() };
            for rule in self.rules_for(v) {
                let mut next = form[..at].to_vec();
                next.extend(&rule.body);
                next.extend(&form[at + 1..]);
                // The terminals before the first variable are final, so they
                // must match the start of the word.
                let prefix_ok = next
                    .iter()
                    .take_while(|s| matches!(s, Sym::T(_)))
                    .enumerate()
                    .all(|(i, s)| matches!(s, Sym::T(c) if word.get(i) == Some(c)));
                if prefix_ok && min_len(&next) <= word.len() && seen.insert(next.clone()) {
                    parent.insert(next.clone(), form.clone());
                    queue.push_back(next);
                }
            }
        }
        // CYK said the word is in the language, so the search cannot run dry.
        Err(DeriveError::NotInLanguage)
    }

    // ---- from regular languages ----

    /// The right-linear grammar of a DFA's language: one variable per useful
    /// state, a rule `A -> a B` for each transition from A to B on `a`, and
    /// `A -> eps` for each accepting state. States that cannot reach an
    /// accepting state are left out.
    pub fn from_dfa(dfa: &Dfa) -> Grammar {
        // Which states can reach an accepting state?
        let n = dfa.states.len();
        let mut live: Vec<bool> = (0..n).map(|s| dfa.accept.contains(&s)).collect();
        loop {
            let mut changed = false;
            for s in 0..n {
                if !live[s] && dfa.delta[s].iter().any(|&t| live[t]) {
                    live[s] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        // Variables in breadth-first order from the start, named S, A, B, ...
        let order: Vec<usize> = dfa.reachable().into_iter().filter(|&s| live[s] || s == dfa.start).collect();
        let index = |s: usize| order.iter().position(|&t| t == s).unwrap();
        let mut letters = ('A'..='Z').filter(|&c| c != 'S').map(|c| c.to_string());
        let variables: Vec<String> = (0..order.len())
            .map(|i| if i == 0 { "S".to_string() } else { letters.next().unwrap_or_else(|| format!("Q{i}")) })
            .collect();
        let mut rules = Vec::new();
        for &s in &order {
            for (i, &t) in dfa.delta[s].iter().enumerate() {
                if live[t] {
                    rules.push(Rule { head: index(s), body: vec![Sym::T(dfa.alphabet[i]), Sym::V(index(t))] });
                }
            }
            if dfa.accept.contains(&s) {
                rules.push(Rule { head: index(s), body: Vec::new() });
            }
        }
        Grammar { alphabet: dfa.alphabet.clone(), variables, start: 0, rules }
    }
}

/// The CYK algorithm: can CNF grammar `g` generate `word`?
///
/// `table[i][j]` holds the variables that generate the `j + 1` symbols
/// starting at position `i`. Single symbols come from rules `A -> a`; longer
/// pieces are split in every possible place and combined with `A -> B C`.
fn cyk(g: &Grammar, word: &[char]) -> bool {
    let n = word.len();
    if n == 0 {
        return g.rules_for(g.start).any(|r| r.body.is_empty());
    }
    let vars = g.variables.len();
    // The rules `A -> B C`, found by their B, so that each step only looks
    // at the rules that could apply instead of at every rule.
    let mut by_left: Vec<Vec<(usize, usize)>> = vec![Vec::new(); vars];
    for rule in &g.rules {
        if let [Sym::V(b), Sym::V(c)] = rule.body[..] {
            by_left[b].push((c, rule.head));
        }
    }
    // `has[i][len - 1]` marks the variables that derive the `len` symbols
    // from position `i`; `list` holds the same as a list, to loop over.
    let mut has = vec![vec![vec![false; vars]; n]; n];
    let mut list: Vec<Vec<Vec<usize>>> = vec![vec![Vec::new(); n]; n];
    for (i, &c) in word.iter().enumerate() {
        for rule in &g.rules {
            if rule.body == [Sym::T(c)] && !has[i][0][rule.head] {
                has[i][0][rule.head] = true;
                list[i][0].push(rule.head);
            }
        }
    }
    for len in 2..=n {
        for i in 0..=n - len {
            let mut found = Vec::new();
            for split in 1..len {
                let (left, right) = (i, i + split);
                for &b in &list[left][split - 1] {
                    for &(c, head) in &by_left[b] {
                        if has[right][len - split - 1][c] && !has[i][len - 1][head] {
                            has[i][len - 1][head] = true;
                            found.push(head);
                        }
                    }
                }
            }
            list[i][len - 1] = found;
        }
    }
    has[0][n - 1][g.start]
}

impl fmt::Display for Grammar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let n = self.variables.len();
        write!(
            f,
            "Grammar with {} variable{} over {}, start {}",
            n,
            if n == 1 { "" } else { "s" },
            fmt_alphabet(&self.alphabet),
            self.variables[self.start]
        )?;
        let width = self.variables.iter().map(|v| v.chars().count()).max().unwrap_or(0);
        // Variables in order of their first rule, start variable first.
        let mut heads: Vec<usize> = vec![self.start];
        for rule in &self.rules {
            if !heads.contains(&rule.head) {
                heads.push(rule.head);
            }
        }
        for v in heads {
            let bodies: Vec<String> = self.rules_for(v).map(|r| self.show(&r.body)).collect();
            if bodies.is_empty() {
                continue;
            }
            write!(f, "\n{:width$} -> {}", self.variables[v], bodies.join(" | "))?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// `S -> a S b | eps`: the language a^n b^n.
    pub fn anbn() -> Grammar {
        Grammar {
            alphabet: vec!['a', 'b'],
            variables: vec!["S".into()],
            start: 0,
            rules: vec![
                Rule { head: 0, body: vec![Sym::T('a'), Sym::V(0), Sym::T('b')] },
                Rule { head: 0, body: vec![] },
            ],
        }
    }

    /// Balanced parentheses over {(, )} written with `l` and `r`:
    /// `S -> S S | l S r | eps`.
    fn balanced() -> Grammar {
        Grammar {
            alphabet: vec!['l', 'r'],
            variables: vec!["S".into()],
            start: 0,
            rules: vec![
                Rule { head: 0, body: vec![Sym::V(0), Sym::V(0)] },
                Rule { head: 0, body: vec![Sym::T('l'), Sym::V(0), Sym::T('r')] },
                Rule { head: 0, body: vec![] },
            ],
        }
    }

    fn is_cnf(g: &Grammar) -> bool {
        g.rules.iter().all(|r| match r.body[..] {
            [] => r.head == g.start,
            [Sym::T(_)] => true,
            [Sym::V(b), Sym::V(c)] => b != g.start && c != g.start,
            _ => false,
        })
    }

    #[test]
    fn prints_like_source() {
        assert_eq!(anbn().to_string(), "Grammar with 1 variable over {a, b}, start S\nS -> a S b | eps");
    }

    #[test]
    fn cyk_decides_membership() {
        let g = anbn();
        for (word, expected) in [("", true), ("ab", true), ("aabb", true), ("aab", false), ("ba", false)] {
            assert_eq!(g.accepts(word), Ok(expected), "{word}");
        }
        assert_eq!(g.accepts("abc"), Err('c'));
        let b = balanced();
        assert_eq!(b.accepts("llrlrr"), Ok(true));
        assert_eq!(b.accepts("lrrl"), Ok(false));
    }

    #[test]
    fn cnf_has_the_right_shape_and_language() {
        for g in [anbn(), balanced()] {
            let cnf = g.to_cnf();
            assert!(is_cnf(&cnf), "{cnf}");
            // Same answers as a brute-force derivation search on all short words.
            for len in 0..=6 {
                for bits in 0..(1u32 << len) {
                    let word: String = (0..len).map(|i| g.alphabet[(bits >> i) as usize & 1]).collect();
                    let derived = g.derive(&word, 100_000).is_ok();
                    assert_eq!(cyk(&cnf, &word.chars().collect::<Vec<_>>()), derived, "{word}");
                }
            }
        }
    }

    #[test]
    fn cnf_handles_unit_and_useless_rules() {
        // S -> A | b B,  A -> a,  B -> B b (generates nothing),  C -> c (unreachable)
        let g = Grammar {
            alphabet: vec!['a', 'b', 'c'],
            variables: vec!["S".into(), "A".into(), "B".into(), "C".into()],
            start: 0,
            rules: vec![
                Rule { head: 0, body: vec![Sym::V(1)] },
                Rule { head: 0, body: vec![Sym::T('b'), Sym::V(2)] },
                Rule { head: 1, body: vec![Sym::T('a')] },
                Rule { head: 2, body: vec![Sym::V(2), Sym::T('b')] },
                Rule { head: 3, body: vec![Sym::T('c')] },
            ],
        };
        let cnf = g.to_cnf();
        assert_eq!(cnf.to_string(), "Grammar with 1 variable over {a, b, c}, start S0\nS0 -> a");
    }

    #[test]
    fn emptiness() {
        assert!(!anbn().is_empty());
        let g = Grammar {
            alphabet: vec!['a'],
            variables: vec!["S".into()],
            start: 0,
            rules: vec![Rule { head: 0, body: vec![Sym::T('a'), Sym::V(0)] }],
        };
        assert!(g.is_empty());
    }

    #[test]
    fn leftmost_derivation() {
        let g = anbn();
        let steps = g.derive("aabb", 1000).unwrap();
        let shown: Vec<String> = steps.iter().map(|s| g.show(s)).collect();
        assert_eq!(shown, ["S", "a S b", "a a S b b", "a a b b"]);
        assert_eq!(g.derive("aab", 1000), Err(DeriveError::NotInLanguage));
        assert_eq!(g.show(&g.derive("", 10).unwrap()[1]), "eps");
    }

    #[test]
    fn grammar_from_a_dfa() {
        let dfa = crate::automata::dfa::tests::even_zeros();
        let g = Grammar::from_dfa(&dfa);
        assert_eq!(g.to_string(), "Grammar with 2 variables over {0, 1}, start S\nS -> 0 A | 1 S | eps\nA -> 0 S | 1 A");
        for word in ["", "0", "00", "0101", "111"] {
            assert_eq!(g.accepts(word), dfa.accepts(word), "{word}");
        }
    }
}

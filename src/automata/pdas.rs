//! More about pushdown automata: the grammar of a PDA (the converse of
//! `Pda::from_grammar`), whether a PDA is deterministic, and a few PDAs
//! that come up again and again.
//!
//! The PDA type and its search for accepting runs are in `pda`.

use super::grammar::{Grammar, Rule, Sym};
use super::pda::{Move, Pda};
use super::{fresh_name, State};
use std::collections::{BTreeSet, HashMap};

/// The grammar of a PDA, by the "triple" construction: a variable
/// `[p X q]` derives exactly the words that take the PDA from state p to
/// state q while popping X off the top of the stack (and leaving what is
/// below it alone). `None` if the grammar would have more than `max_rules`
/// rules before useless ones are removed.
///
/// The construction needs a PDA that accepts by emptying its stack and
/// whose every move pops one symbol, so the PDA is first changed into one,
/// without changing its language:
///
/// 1. a new start state, `begin`, pushes the old bottom symbol onto a new one, `$`,
///    which only the end can pop;
/// 2. a move that pops nothing becomes one move per stack symbol X that
///    pops X and pushes it back under what it pushed;
/// 3. a new state `end` is reached by an eps move from each accepting
///    state, and pops everything, `$` included.
///
/// Then each move `p --a, X/Y1...Yk--> r` gives, for every choice of
/// states q1, ..., qk, the rule `[p X qk] -> a [r Y1 q1] [q1 Y2 q2] ...
/// [qk-1 Yk qk]` (just `[p X r] -> a` when k = 0). The start variable `S`
/// has `S -> [begin $ end]`. The result is trimmed, so only variables that
/// matter are left.
pub fn to_grammar(pda: &Pda, max_rules: usize) -> Option<Grammar> {
    // The stack symbols, the bottom first.
    let mut stack: Vec<String> = vec![pda.bottom.clone()];
    for m in &pda.moves {
        for s in m.pop.iter().chain(&m.push) {
            if !stack.contains(s) {
                stack.push(s.clone());
            }
        }
    }
    let floor = fresh_name(&stack, "$");
    stack.push(floor.clone());

    // Steps 1 to 3: the states and moves of the changed PDA. Its states are
    // the old ones, then the new start, `begin`, and `end`.
    let n = pda.states.len();
    let (start, end) = (n, n + 1);
    let mut moves: Vec<Move> = Vec::new();
    moves.push(Move { from: start, read: None, pop: Some(floor.clone()), push: vec![pda.bottom.clone(), floor.clone()], to: pda.start });
    for m in &pda.moves {
        match &m.pop {
            Some(_) => moves.push(m.clone()),
            None => {
                for x in &stack {
                    let mut push = m.push.clone();
                    push.push(x.clone());
                    moves.push(Move { from: m.from, read: m.read, pop: Some(x.clone()), push, to: m.to });
                }
            }
        }
    }
    for &f in &pda.accept {
        for x in &stack {
            moves.push(Move { from: f, read: None, pop: Some(x.clone()), push: vec![x.clone()], to: end });
        }
    }
    for x in &stack {
        moves.push(Move { from: end, read: None, pop: Some(x.clone()), push: Vec::new(), to: end });
    }
    let states = n + 2;
    // The two new states are named so that they cannot be confused with
    // the PDA's own: `begin` and `end`, with primes added if taken.
    let mut names = pda.states.clone();
    names.push(fresh_name(&pda.states, "begin"));
    names.push(fresh_name(&names, "end"));
    let name = |s: State| -> &str { &names[s] };

    // Count the rules first, so a PDA with long pushes cannot make the
    // construction run away: each move gives states^k rules.
    let mut total: usize = 1;
    for m in &moves {
        let k = u32::try_from(m.push.len()).ok()?;
        total = total.checked_add(states.checked_pow(k)?)?;
        if total > max_rules {
            return None;
        }
    }

    // Variables, numbered as they are first needed; `S` is variable 0.
    let mut variables = vec!["S".to_string()];
    let mut index: HashMap<(State, usize, State), usize> = HashMap::new();
    let stack_index = |x: &String| stack.iter().position(|s| s == x).expect("every stack symbol was collected");
    let mut var = |p: State, x: usize, q: State, variables: &mut Vec<String>| -> usize {
        *index.entry((p, x, q)).or_insert_with(|| {
            variables.push(format!("[{} {} {}]", name(p), stack[x], name(q)));
            variables.len() - 1
        })
    };
    let floor_index = stack.len() - 1;
    let mut rules = vec![Rule { head: 0, body: vec![Sym::V(var(start, floor_index, end, &mut variables))] }];
    for m in &moves {
        let pop = stack_index(m.pop.as_ref().expect("every move pops one symbol now"));
        let read: Vec<Sym> = m.read.map(Sym::T).into_iter().collect();
        let k = m.push.len();
        if k == 0 {
            let head = var(m.from, pop, m.to, &mut variables);
            rules.push(Rule { head, body: read });
            continue;
        }
        let pushed: Vec<usize> = m.push.iter().map(stack_index).collect();
        // Every choice of q1, ..., qk, counted like a k-digit number in
        // base `states`.
        let mut qs = vec![0; k];
        loop {
            let mut body = read.clone();
            let mut from = m.to;
            for (y, &q) in pushed.iter().zip(&qs) {
                body.push(Sym::V(var(from, *y, q, &mut variables)));
                from = q;
            }
            let head = var(m.from, pop, qs[k - 1], &mut variables);
            rules.push(Rule { head, body });
            let Some(i) = qs.iter().rposition(|&q| q + 1 < states) else { break };
            qs[i] += 1;
            qs[i + 1..].iter_mut().for_each(|q| *q = 0);
        }
    }
    let g = Grammar { alphabet: pda.alphabet.clone(), variables, start: 0, rules };
    Some(g.trimmed())
}

/// Is the PDA deterministic: in every configuration, at most one move can
/// be taken? Two moves from the same state clash when they could both
/// apply at once: their reads are the same or one is eps, and their pops
/// are the same or one pops nothing (which works whatever is on top).
pub fn is_deterministic(pda: &Pda) -> bool {
    for (i, m) in pda.moves.iter().enumerate() {
        for other in &pda.moves[i + 1..] {
            let reads = m.read.is_none() || other.read.is_none() || m.read == other.read;
            let pops = m.pop.is_none() || other.pop.is_none() || m.pop == other.pop;
            if m.from == other.from && reads && pops {
                return false;
            }
        }
    }
    true
}

/// The stack symbols a PDA uses, its bottom symbol included.
pub fn stack_symbols(pda: &Pda) -> BTreeSet<String> {
    let mut symbols = BTreeSet::from([pda.bottom.clone()]);
    for m in &pda.moves {
        symbols.extend(m.pop.iter().cloned());
        symbols.extend(m.push.iter().cloned());
    }
    symbols
}

// ---- ready-made PDAs ----

/// A move written out for `build`: `(from, read, pop, push, to)`, with
/// `None` for an eps read and `""` for popping nothing.
type Written<'a> = (&'a str, Option<char>, &'a str, Vec<String>, &'a str);

/// Builds a PDA from its state names (the first is the start), accepting
/// states and moves, with the bottom `Z`.
fn build(alphabet: &[char], states: &[&str], accept: &[&str], moves: &[Written]) -> Pda {
    let mut alphabet = alphabet.to_vec();
    alphabet.sort_unstable();
    alphabet.dedup();
    let at = |name: &str| states.iter().position(|s| *s == name).expect("a state of this PDA");
    Pda {
        alphabet,
        states: states.iter().map(|s| s.to_string()).collect(),
        start: 0,
        accept: accept.iter().map(|s| at(s)).collect(),
        moves: moves
            .iter()
            .map(|(from, read, pop, push, to)| Move {
                from: at(from),
                read: *read,
                pop: if pop.is_empty() { None } else { Some(pop.to_string()) },
                push: push.clone(),
                to: at(to),
            })
            .collect(),
        bottom: "Z".into(),
    }
}

fn push(symbols: &[&str]) -> Vec<String> {
    symbols.iter().map(|s| s.to_string()).collect()
}

/// a^n b^n, deterministically: one `X` per `a`, popped by each `b`.
///
/// `start` accepts the empty word; `as` reads a's, `bs` reads b's, and
/// `done` is reached once the stack is back to `Z`.
pub fn anbn(a: char, b: char) -> Pda {
    build(
        &[a, b],
        &["start", "as", "bs", "done"],
        &["start", "done"],
        &[
            ("start", Some(a), "Z", push(&["X", "Z"]), "as"),
            ("as", Some(a), "X", push(&["X", "X"]), "as"),
            ("as", Some(b), "X", push(&[]), "bs"),
            ("bs", Some(b), "X", push(&[]), "bs"),
            ("bs", None, "Z", push(&["Z"]), "done"),
        ],
    )
}

/// Balanced brackets, deterministically: an `X` for each open bracket that
/// is not closed yet. `even` (accepting) means every bracket so far is
/// closed; after a close bracket, `closed` looks at the stack to see
/// whether that is so.
pub fn balanced(open: char, close: char) -> Pda {
    build(
        &[open, close],
        &["even", "open", "closed"],
        &["even"],
        &[
            ("even", Some(open), "Z", push(&["X", "Z"]), "open"),
            ("open", Some(open), "X", push(&["X", "X"]), "open"),
            ("open", Some(close), "X", push(&[]), "closed"),
            ("closed", None, "X", push(&["X"]), "open"),
            ("closed", None, "Z", push(&["Z"]), "even"),
        ],
    )
}

/// The palindromes over an alphabet. The PDA pushes what it reads, guesses
/// where the middle is (skipping one symbol there for odd lengths), then
/// pops one matching symbol per symbol read. It must guess, and no PDA
/// for this language can do without.
pub fn palindromes(alphabet: &[char]) -> Pda {
    let mut alphabet = alphabet.to_vec();
    alphabet.sort_unstable();
    alphabet.dedup();
    // The symbols read are pushed as themselves, so the bottom must be
    // something else.
    let taken: Vec<String> = alphabet.iter().map(|c| c.to_string()).collect();
    let bottom = fresh_name(&taken, "Z");
    let (first, second, done) = (0, 1, 2);
    let mut moves = Vec::new();
    for &c in &alphabet {
        let s = c.to_string();
        moves.push(Move { from: first, read: Some(c), pop: None, push: vec![s.clone()], to: first });
        moves.push(Move { from: first, read: Some(c), pop: None, push: Vec::new(), to: second });
        moves.push(Move { from: second, read: Some(c), pop: Some(s), push: Vec::new(), to: second });
    }
    moves.push(Move { from: first, read: None, pop: None, push: Vec::new(), to: second });
    moves.push(Move { from: second, read: None, pop: Some(bottom.clone()), push: vec![bottom.clone()], to: done });
    Pda {
        alphabet,
        states: ["first", "second", "done"].map(String::from).to_vec(),
        start: first,
        accept: BTreeSet::from([done]),
        moves,
        bottom,
    }
}

/// The words over {a, b} with as many a's as b's. The stack holds the
/// surplus: `A`s while there have been more a's, `B`s while more b's. The
/// PDA accepts when it guesses the stack is back to `Z` at the end.
pub fn equal_count(a: char, b: char) -> Pda {
    build(
        &[a, b],
        &["count", "done"],
        &["done"],
        &[
            ("count", Some(a), "Z", push(&["A", "Z"]), "count"),
            ("count", Some(a), "A", push(&["A", "A"]), "count"),
            ("count", Some(a), "B", push(&[]), "count"),
            ("count", Some(b), "Z", push(&["B", "Z"]), "count"),
            ("count", Some(b), "B", push(&["B", "B"]), "count"),
            ("count", Some(b), "A", push(&[]), "count"),
            ("count", None, "Z", push(&["Z"]), "done"),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::cfg;
    use crate::automata::pda::{Outcome, STEP_LIMIT};

    /// Every word over the alphabet of length at most `n`.
    fn all_words(alphabet: &[char], n: usize) -> Vec<String> {
        let mut all = vec![String::new()];
        let mut layer = vec![String::new()];
        for _ in 0..n {
            layer = layer.iter().flat_map(|w| alphabet.iter().map(move |c| format!("{w}{c}"))).collect();
            all.extend(layer.iter().cloned());
        }
        all
    }

    fn accepted(pda: &Pda, n: usize) -> BTreeSet<String> {
        all_words(&pda.alphabet, n)
            .into_iter()
            .filter(|w| match pda.search(w, STEP_LIMIT).outcome {
                Outcome::Accepted => true,
                Outcome::Rejected => false,
                other => panic!("{w:?}: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn ready_made_pdas_have_their_languages() {
        let same = |pda: &Pda, g: &Grammar| assert_eq!(accepted(pda, 6), cfg::words_up_to(g, 6, 1 << 20).unwrap(), "{pda}");
        same(&anbn('a', 'b'), &cfg::anbn('a', 'b'));
        same(&balanced('l', 'r'), &cfg::balanced('l', 'r'));
        same(&palindromes(&['a', 'b']), &cfg::palindromes(&['a', 'b']));
        same(&palindromes(&['Z', 'a']), &cfg::palindromes(&['Z', 'a']));
        same(&equal_count('a', 'b'), &cfg::equal_count('a', 'b'));
    }

    #[test]
    fn determinism_is_recognised() {
        assert!(is_deterministic(&anbn('a', 'b')));
        assert!(is_deterministic(&balanced('l', 'r')));
        assert!(!is_deterministic(&palindromes(&['a', 'b'])));
        assert!(!is_deterministic(&equal_count('a', 'b')));
        assert!(!is_deterministic(&Pda::from_grammar(&cfg::anbn('a', 'b'))));
    }

    #[test]
    fn the_grammar_of_a_pda_has_its_language() {
        for pda in [anbn('a', 'b'), balanced('l', 'r'), palindromes(&['a', 'b']), equal_count('a', 'b')] {
            let g = to_grammar(&pda, 1 << 20).unwrap();
            assert_eq!(cfg::words_up_to(&g, 6, 1 << 20).unwrap(), accepted(&pda, 6), "{pda}\n{g}");
        }
        // Back and forth: grammar, PDA, grammar.
        let g = cfg::equal_count('a', 'b');
        let back = to_grammar(&Pda::from_grammar(&g), 1 << 20).unwrap();
        assert_eq!(cfg::words_up_to(&back, 6, 1 << 20), cfg::words_up_to(&g, 6, 1 << 20));
    }

    #[test]
    fn the_rule_count_is_limited() {
        assert!(to_grammar(&palindromes(&['a', 'b']), 10).is_none());
    }

    #[test]
    fn stack_symbols_include_the_bottom() {
        let names: Vec<String> = stack_symbols(&anbn('a', 'b')).into_iter().collect();
        assert_eq!(names, ["X", "Z"]);
    }
}

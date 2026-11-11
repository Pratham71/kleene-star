//! Properties of the algorithms, checked on many random machines, regexes
//! and grammars. Each property is a fact from the theory, such as "the
//! subset construction does not change the language", tested by comparing
//! the two sides on every word up to a fixed length.
//!
//! The inputs come from a seeded generator: set `KSTAR_SEED` to try others.

mod common;
use common::{seed, Rng};
use kstar::automata::grammar::{Grammar, Rule, Sym};
use kstar::automata::pda::{Outcome, Pda};
use kstar::automata::tm::{self, Dir, Halt, Tm};
use kstar::automata::{convert, minimize, ops, regex, Dfa, Nfa, Regex};
use std::collections::BTreeSet;

const ALPHABET: [char; 2] = ['a', 'b'];
/// How many random inputs each property is checked on.
const CASES: usize = 200;

/// Every word over {a, b} of length at most `max`, shortest first.
fn words(max: usize) -> Vec<String> {
    let mut all = vec![String::new()];
    let mut layer = vec![String::new()];
    for _ in 0..max {
        layer = layer.iter().flat_map(|w| ALPHABET.map(|c| format!("{w}{c}"))).collect();
        all.extend(layer.iter().cloned());
    }
    all
}

fn random_dfa(rng: &mut Rng) -> Dfa {
    let n = 1 + rng.below(5);
    Dfa {
        alphabet: ALPHABET.to_vec(),
        states: (0..n).map(|i| format!("s{i}")).collect(),
        start: 0,
        accept: (0..n).filter(|_| rng.chance(40)).collect(),
        delta: (0..n).map(|_| ALPHABET.iter().map(|_| rng.below(n)).collect()).collect(),
    }
}

fn random_nfa(rng: &mut Rng) -> Nfa {
    let n = 1 + rng.below(5);
    let mut nfa = Nfa::new(ALPHABET.to_vec());
    for i in 0..n {
        nfa.add_state(format!("s{i}"));
    }
    for _ in 0..rng.below(3 * n + 1) {
        let label = match rng.below(5) {
            0 => None,
            i => Some(ALPHABET[i % 2]),
        };
        nfa.add_edge(rng.below(n), label, rng.below(n));
    }
    nfa.accept = (0..n).filter(|_| rng.chance(35)).collect();
    nfa
}

fn random_regex(rng: &mut Rng, depth: usize) -> Regex {
    if depth == 0 || rng.chance(25) {
        return match rng.below(10) {
            0 => Regex::Eps,
            1 => Regex::Empty,
            2 => Regex::Any,
            i => Regex::Sym(ALPHABET[i % 2]),
        };
    }
    match rng.below(3) {
        0 => Regex::Concat(Box::new(random_regex(rng, depth - 1)), Box::new(random_regex(rng, depth - 1))),
        1 => Regex::Alt(Box::new(random_regex(rng, depth - 1)), Box::new(random_regex(rng, depth - 1))),
        _ => Regex::Star(Box::new(random_regex(rng, depth - 1))),
    }
}

fn random_grammar(rng: &mut Rng) -> Grammar {
    let variables = 1 + rng.below(3);
    let mut rules = Vec::new();
    for head in 0..variables {
        for _ in 0..1 + rng.below(3) {
            let body = (0..rng.below(4))
                .map(|_| if rng.chance(55) { Sym::T(*rng.pick(&ALPHABET)) } else { Sym::V(rng.below(variables)) })
                .collect();
            rules.push(Rule { head, body });
        }
    }
    Grammar {
        alphabet: ALPHABET.to_vec(),
        variables: ["S", "A", "B"][..variables].iter().map(|s| s.to_string()).collect(),
        start: 0,
        rules,
    }
}

fn dfa(regex: &Regex) -> Dfa {
    convert::nfa_to_dfa(&convert::regex_to_nfa(regex, &ALPHABET))
}

/// Asserts that two acceptors agree on every word up to length 7.
fn same_language(what: &str, a: impl Fn(&str) -> bool, b: impl Fn(&str) -> bool) {
    for word in words(7) {
        assert_eq!(a(&word), b(&word), "{what}: they disagree on {word:?}");
    }
}

#[test]
fn subset_construction_keeps_the_language() {
    let mut rng = Rng::new(seed());
    for case in 0..CASES {
        let nfa = random_nfa(&mut rng);
        let d = convert::nfa_to_dfa(&nfa);
        let what = format!("case {case}: {nfa:?}");
        same_language(&what, |w| nfa.accepts(w).unwrap(), |w| d.accepts(w).unwrap());
        let no_eps = convert::remove_eps(&nfa);
        assert!(!no_eps.has_eps());
        same_language(&what, |w| nfa.accepts(w).unwrap(), |w| no_eps.accepts(w).unwrap());
        let reduced = minimize::reduce(&nfa);
        assert!(reduced.states.len() <= nfa.states.len().max(1), "{what}: reduce added states");
        same_language(&what, |w| nfa.accepts(w).unwrap(), |w| reduced.accepts(w).unwrap());
        let back = convert::dfa_to_nfa(&d);
        same_language(&what, |w| d.accepts(w).unwrap(), |w| back.accepts(w).unwrap());
    }
}

#[test]
fn minimization_is_idempotent_and_keeps_the_language() {
    let mut rng = Rng::new(seed());
    for case in 0..CASES {
        let d = random_dfa(&mut rng);
        let m = minimize::minimize(&d);
        let what = format!("case {case}: {d:?}");
        same_language(&what, |w| d.accepts(w).unwrap(), |w| m.accepts(w).unwrap());
        assert!(m.states.len() <= d.reachable().len(), "{what}");
        let again = minimize::minimize(&m);
        assert_eq!(again.states.len(), m.states.len(), "{what}: minimizing twice shrank it again");
        assert!(ops::equivalent(&m, &d), "{what}");
        // Minimal DFAs of the same language have the same size: here, the
        // DFA of the NFA built from it.
        let other = minimize::minimize(&convert::nfa_to_dfa(&convert::dfa_to_nfa(&d)));
        assert_eq!(other.states.len(), m.states.len(), "{what}: two minimal DFAs differ in size");
    }
}

#[test]
fn regex_conversions_round_trip() {
    let mut rng = Rng::new(seed());
    for case in 0..CASES {
        let r = random_regex(&mut rng, 4);
        let d = dfa(&r);
        let what = format!("case {case}: /{r}/");
        // DFA -> regex -> DFA gives the same language.
        let back = convert::dfa_to_regex(&d);
        assert!(ops::equivalent(&d, &dfa(&back)), "{what} came back as /{back}/");
        // Printing a regex and reading it back gives the same language.
        let printed = r.to_string();
        let reparsed = regex::parse(&printed).unwrap_or_else(|e| panic!("{what} printed as /{printed}/: {e:?}"));
        assert!(ops::equivalent(&d, &dfa(&reparsed)), "{what} printed as /{printed}/");
    }
}

#[test]
fn closure_operations_follow_the_laws() {
    let mut rng = Rng::new(seed());
    for case in 0..CASES {
        let (a, b) = (random_dfa(&mut rng), random_dfa(&mut rng));
        let what = format!("case {case}: {a:?} and {b:?}");
        // De Morgan: not (A or B) = (not A) and (not B), and the other way.
        let left = ops::complement(&ops::union(&a, &b));
        let right = ops::intersect(&ops::complement(&a), &ops::complement(&b));
        assert!(ops::equivalent(&left, &right), "{what}");
        let left = ops::complement(&ops::intersect(&a, &b));
        let right = ops::union(&ops::complement(&a), &ops::complement(&b));
        assert!(ops::equivalent(&left, &right), "{what}");
        // The operations mean what they say, word by word.
        let (na, nb) = (convert::dfa_to_nfa(&a), convert::dfa_to_nfa(&b));
        let has = |d: &Dfa, w: &str| d.accepts(w).unwrap();
        let union = ops::union(&a, &b);
        same_language(&what, |w| has(&union, w), |w| has(&a, w) || has(&b, w));
        let difference = ops::difference(&a, &b);
        same_language(&what, |w| has(&difference, w), |w| has(&a, w) && !has(&b, w));
        let concat = ops::concat(&na, &nb);
        same_language(
            &what,
            |w| concat.accepts(w).unwrap(),
            |w| (0..=w.len()).any(|i| has(&a, &w[..i]) && has(&b, &w[i..])),
        );
        // reverse(reverse(L)) = L, and reverse really reverses.
        let reversed = ops::reverse(&na);
        same_language(&what, |w| reversed.accepts(w).unwrap(), |w| has(&a, &w.chars().rev().collect::<String>()));
        let twice = ops::reverse(&reversed);
        same_language(&what, |w| twice.accepts(w).unwrap(), |w| has(&a, w));
        // A star language contains eps and is closed under concatenation.
        let star = ops::star(&na);
        assert!(star.accepts("").unwrap(), "{what}");
        // equivalent and distinguishing_word agree.
        match ops::distinguishing_word(&a, &b) {
            None => assert!(ops::equivalent(&a, &b), "{what}"),
            Some(w) => assert_ne!(has(&a, &w), has(&b, &w), "{what}: {w:?} does not distinguish them"),
        }
        assert_eq!(ops::is_empty(&a), words(a.states.len()).iter().all(|w| !has(&a, w)), "{what}");
    }
}

#[test]
fn grammars_agree_with_their_pdas() {
    let mut rng = Rng::new(seed());
    let mut decided = 0;
    // The PDA search is slow when it has to give up, so fewer cases here.
    let cases = CASES / 4;
    for case in 0..cases {
        let g = random_grammar(&mut rng);
        let pda = Pda::from_grammar(&g);
        let what = format!("case {case}: {g}");
        for word in words(4) {
            let cyk = g.accepts(&word).unwrap();
            match pda.search(&word, 2_000).outcome {
                Outcome::Accepted => assert!(cyk, "{what}: the PDA accepts {word:?} but CYK does not"),
                Outcome::Rejected => assert!(!cyk, "{what}: CYK accepts {word:?} but the PDA rejects it"),
                // The PDA search may give up (left recursion makes it
                // endless), but it may never be wrong.
                Outcome::GaveUp => continue,
                Outcome::Foreign { .. } => panic!("{what}: {word:?} is over the alphabet"),
            }
            decided += 1;
        }
        // CNF keeps the language.
        let cnf = g.to_cnf();
        for word in words(5) {
            assert_eq!(cnf.accepts(&word).unwrap(), g.accepts(&word).unwrap(), "{what}: {word:?}");
        }
        if g.is_empty() {
            assert!(words(6).iter().all(|w| !g.accepts(w).unwrap()), "{what}: empty, yet accepts a word");
        }
    }
    assert!(decided > cases * 15, "the PDA gave up too often: only {decided} answers");
}

#[test]
fn regular_grammars_match_their_dfa() {
    let mut rng = Rng::new(seed());
    for case in 0..CASES / 2 {
        let d = random_dfa(&mut rng);
        let g = Grammar::from_dfa(&d);
        let what = format!("case {case}: {d:?}");
        for word in words(5) {
            assert_eq!(g.accepts(&word).unwrap(), d.accepts(&word).unwrap(), "{what}: {word:?}");
        }
    }
}

#[test]
fn turing_machines_always_stop_within_the_limit() {
    let mut rng = Rng::new(seed());
    let tape = ['a', 'b', tm::BLANK];
    for case in 0..CASES {
        let n = 2 + rng.below(4);
        let mut moves = Vec::new();
        for from in 0..n {
            for &read in &tape {
                if rng.chance(80) {
                    moves.push(tm::Move {
                        from,
                        read,
                        write: *rng.pick(&tape),
                        dir: *rng.pick(&[Dir::Left, Dir::Right, Dir::Stay]),
                        to: rng.below(n + 2),
                    });
                }
            }
        }
        let machine = Tm {
            alphabet: ALPHABET.to_vec(),
            states: (0..n + 2).map(|i| format!("s{i}")).collect(),
            start: 0,
            accept: BTreeSet::from([n]),
            reject: BTreeSet::from([n + 1]),
            moves,
        };
        for word in ["", "a", "abba", "babababa"] {
            let limit = 2_000;
            let run = machine.run(word, limit, case % 4 == 0);
            assert!(run.steps <= limit, "case {case}: ran {} steps", run.steps);
            if run.halt == Halt::GaveUp {
                assert_eq!(run.steps, limit, "case {case}: gave up early");
            }
            // The same run with the default limit agrees whenever it halted.
            if run.halt != Halt::GaveUp {
                assert_eq!(machine.run(word, tm::STEP_LIMIT, false).halt, run.halt, "case {case}");
            }
        }
    }
}

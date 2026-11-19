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

/// A random word over {a, b} of length at most `max`.
fn random_word(rng: &mut Rng, max: usize) -> Vec<char> {
    (0..rng.below(max + 1)).map(|_| *rng.pick(&ALPHABET)).collect()
}

/// Each pattern machine of `std.automata` accepts exactly the words its
/// definition describes (checked against the plain definition on every word
/// up to length 7), and is already as small as a DFA for it can be.
#[test]
fn pattern_machines_match_their_definitions() {
    use kstar::automata::patterns;
    let minimal = |what: &str, d: &Dfa| {
        assert_eq!(d.states.len(), minimize::minimize(d).states.len(), "{what}: not minimal: {d}");
    };
    let mut rng = Rng::new(seed());
    for case in 0..CASES {
        let w = random_word(&mut rng, 4);
        let text: String = w.iter().collect();
        let what = format!("case {case}, pattern {text:?}");
        let d = patterns::starts_with(&ALPHABET, &w);
        same_language(&what, |x| d.accepts(x).unwrap(), |x| x.starts_with(&text));
        minimal(&what, &d);
        let d = patterns::ends_with(&ALPHABET, &w);
        same_language(&what, |x| d.accepts(x).unwrap(), |x| x.ends_with(&text));
        minimal(&what, &d);
        let d = patterns::contains(&ALPHABET, &w);
        same_language(&what, |x| d.accepts(x).unwrap(), |x| x.contains(&text));
        minimal(&what, &d);

        let symbol = *rng.pick(&ALPHABET);
        let n = rng.below(5);
        let count = |x: &str| x.chars().filter(|&c| c == symbol).count();
        let what = format!("case {case}, symbol {symbol}, n = {n}");
        for (d, holds) in [
            (patterns::count_exactly(&ALPHABET, symbol, n), Box::new(|x: &str| count(x) == n) as Box<dyn Fn(&str) -> bool>),
            (patterns::count_at_least(&ALPHABET, symbol, n), Box::new(|x: &str| count(x) >= n)),
            (patterns::count_at_most(&ALPHABET, symbol, n), Box::new(|x: &str| count(x) <= n)),
        ] {
            same_language(&what, |x| d.accepts(x).unwrap(), &holds);
            minimal(&what, &d);
        }
        let m = 1 + rng.below(4);
        let r = rng.below(m);
        let what = format!("case {case}, symbol {symbol}, mod {m} = {r}");
        let d = patterns::count_mod(&ALPHABET, symbol, m, r);
        same_language(&what, |x| d.accepts(x).unwrap(), |x| count(x) % m == r);
        minimal(&what, &d);
        let d = patterns::length_mod(&ALPHABET, m, r);
        same_language(&what, |x| d.accepts(x).unwrap(), |x| x.chars().count() % m == r);
        minimal(&what, &d);
        let (lo, hi) = (rng.below(4), rng.below(4));
        let (lo, hi) = (lo.min(hi), lo.max(hi));
        let what = format!("case {case}, length {lo} to {hi}");
        let d = patterns::length_between(&ALPHABET, lo, hi);
        same_language(&what, |x| d.accepts(x).unwrap(), |x| (lo..=hi).contains(&x.chars().count()));
        minimal(&what, &d);
    }
}

/// The pattern machines over alphabets with no symbol or one symbol: no
/// state that cannot be reached, and still correct.
#[test]
fn pattern_machines_over_tiny_alphabets() {
    use kstar::automata::patterns;
    let d = patterns::starts_with(&[], &[]);
    assert_eq!((d.states.len(), d.accepts("")), (1, Ok(true)));
    let d = patterns::length_between(&[], 1, 3);
    assert_eq!((d.states.len(), d.accepts("")), (1, Ok(false)));
    let d = patterns::starts_with(&['a'], &['a', 'a']);
    assert_eq!(d.states.len(), 3, "no dead state when every symbol matches: {d}");
    assert_eq!(d.accepts("a"), Ok(false));
    assert_eq!(d.accepts("aaa"), Ok(true));
}

/// The questions of `std.automata`, against brute force over every word up
/// to length 7 (random machines have at most 5 states, so every answer
/// shows up by then).
#[test]
fn language_questions_agree_with_brute_force() {
    use kstar::automata::questions;
    let mut rng = Rng::new(seed());
    let all = words(7);
    for case in 0..CASES {
        let d = random_dfa(&mut rng);
        let what = format!("case {case}: {d:?}");
        let accepted: Vec<&String> = all.iter().filter(|w| d.accepts(w).unwrap()).collect();
        // `words` lists shortest first, then in alphabet order.
        assert_eq!(questions::shortest_word(&d).as_ref(), accepted.first().copied(), "{what}");
        for n in 0..=7 {
            let by_hand = accepted.iter().filter(|w| w.chars().count() == n).count() as i64;
            assert_eq!(questions::count_words(&d, n), Some(by_hand), "{what}, n = {n}");
        }
        let listed = questions::words_up_to(&d, 5, 1_000_000).expect("small enough");
        let by_hand: Vec<String> = accepted.iter().filter(|w| w.chars().count() <= 5).map(|w| w.to_string()).collect();
        assert_eq!(listed, by_hand, "{what}");
        // A DFA with n states has an infinite language exactly when it
        // accepts some word of length n to 2n - 1 (the pumping lemma).
        let n = d.states.len();
        let infinite = (n..2 * n).any(|len| questions::count_words(&d, len).unwrap() > 0);
        assert_eq!(questions::is_finite(&d), !infinite, "{what}");
    }
}

#[test]
fn listing_words_stops_at_its_budget() {
    use kstar::automata::{patterns, questions};
    let everything = patterns::length_mod(&ALPHABET, 1, 0);
    assert_eq!(questions::words_up_to(&everything, 30, 10_000), None);
    assert_eq!(questions::words_up_to(&everything, 2, 10_000).map(|w| w.len()), Some(7));
    assert_eq!(questions::count_words(&everything, 70), None, "2^70 does not fit in an Int");
    assert_eq!(questions::count_words(&everything, 62), Some(1 << 62));
}

/// The regex builders of `std.regex` describe exactly the words their
/// definitions say, checked on every word up to length 7.
#[test]
fn regex_builders_match_their_definitions() {
    let mut rng = Rng::new(seed());
    let accepts = |r: &Regex, w: &str| dfa(r).accepts(w).unwrap();
    for case in 0..CASES {
        let word: String = random_word(&mut rng, 3).into_iter().collect();
        let lit = regex::literal(&word);
        same_language(&format!("case {case}: literal {word:?}"), |w| accepts(&lit, w), |w| w == word);

        let r = random_regex(&mut rng, 2);
        let rd = dfa(&r);
        let n = rng.below(4);
        let (lo, hi) = (rng.below(3), rng.below(3));
        let (lo, hi) = (lo.min(hi), lo.max(hi));
        // `w` is in r^k when it splits into k pieces that are each in r.
        let in_power = |w: &str, k: usize| -> bool {
            let chars: Vec<char> = w.chars().collect();
            // reach[i]: the first i symbols split into the pieces so far.
            let mut reach = vec![false; chars.len() + 1];
            reach[0] = true;
            for _ in 0..k {
                let mut next = vec![false; chars.len() + 1];
                for i in (0..=chars.len()).filter(|&i| reach[i]) {
                    for j in i..=chars.len() {
                        let piece: String = chars[i..j].iter().collect();
                        if rd.accepts(&piece).unwrap() {
                            next[j] = true;
                        }
                    }
                }
                reach = next;
            }
            reach[chars.len()]
        };
        let what = format!("case {case}: /{r}/, n = {n}, {lo} to {hi}");
        let p = regex::power(&r, n);
        same_language(&what, |w| accepts(&p, w), |w| in_power(w, n));
        let between = regex::repeat_between(&r, lo, hi);
        same_language(&what, |w| accepts(&between, w), |w| (lo..=hi).any(|k| in_power(w, k)));
        // At least n: n copies, then any number more (7 symbols need at
        // most 7 more non-empty pieces).
        let least = regex::at_least(&r, n);
        same_language(&what, |w| accepts(&least, w), |w| (n..=n + 7).any(|k| in_power(w, k)));
    }
    let both = regex::any_of(&ALPHABET);
    same_language("any_of", |w| accepts(&both, w), |w| w.chars().count() == 1);
    assert_eq!(regex::any_of(&[]), Regex::Empty);
}

/// Every bit string of length at most `max`, shortest first.
fn bit_strings(max: usize) -> Vec<String> {
    let mut all = vec![String::new()];
    let mut layer = vec![String::new()];
    for _ in 0..max {
        layer = layer.iter().flat_map(|w| ['0', '1'].map(|c| format!("{w}{c}"))).collect();
        all.extend(layer.iter().cloned());
    }
    all
}

/// The machines of `std.binary` against arithmetic and plain definitions on
/// every bit string up to 10 bits.
#[test]
fn binary_machines_match_their_definitions() {
    use kstar::automata::binary;
    let all = bit_strings(10);
    // Read most significant bit first; leading zeros do not count, and the
    // empty string is 0.
    let value = |w: &str| w.chars().fold(0u64, |v, c| 2 * v + (c == '1') as u64);
    for n in 1..=12usize {
        for r in 0..n {
            let d = binary::remainder(n, r);
            assert_eq!(d.states.len(), n, "one state per remainder");
            for w in &all {
                assert_eq!(d.accepts(w).unwrap(), value(w) % n as u64 == r as u64, "{w} mod {n} = {r}?");
            }
        }
    }
    let d = binary::no_leading_zeros();
    for w in &all {
        let expected = w == "0" || w.starts_with('1');
        assert_eq!(d.accepts(w).unwrap(), expected, "{w:?}");
    }
    for bit in ['0', '1'] {
        for k in 0..4 {
            let d = binary::run_at_most(bit, k);
            for w in &all {
                let longest = w.split(|c| c != bit).map(str::len).max().unwrap_or(0);
                assert_eq!(d.accepts(w).unwrap(), longest <= k, "{w:?}: run of {bit} at most {k}?");
            }
            assert_eq!(d.states.len(), minimize::minimize(&d).states.len(), "run_at_most({bit}, {k}) is minimal");
        }
    }
}

/// Writing a number in binary and reading it back gives the number.
#[test]
fn binary_numbers_round_trip() {
    use kstar::automata::binary::{from_binary, to_binary};
    let mut rng = Rng::new(seed());
    for _ in 0..CASES {
        let n = (rng.below(1 << 30) as i64) << rng.below(33);
        let bits = to_binary(n as u64);
        assert!(bits == "0" || bits.starts_with('1'), "{n} written as {bits}");
        assert_eq!(from_binary(&bits), Some(n as u64));
        assert_eq!(from_binary(&format!("000{bits}")), Some(n as u64), "leading zeros do not count");
    }
    assert_eq!(to_binary(0), "0");
    assert_eq!(from_binary(""), Some(0));
    assert_eq!(from_binary(&to_binary(u64::MAX)), Some(u64::MAX));
    assert_eq!(from_binary(&format!("1{}", to_binary(u64::MAX))), None, "65 bits do not fit");
}

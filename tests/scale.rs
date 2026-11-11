//! Large inputs: big machines, long words, big grammars, long traces. Each
//! test has a time budget, generous enough for a debug build on a slow
//! machine but small enough to catch an algorithm that has gone quadratic.
//!
//! The heaviest cases are marked `#[ignore]`, so a plain `cargo test` stays
//! quick. Run them, in a release build, with
//!
//! ```text
//! cargo test --release --test scale -- --ignored
//! ```

mod common;
use kstar::automata::grammar::{Grammar, Rule, Sym};
use kstar::automata::{convert, minimize, ops, Dfa, Nfa};
use std::time::{Duration, Instant};

/// Runs `work` and fails if it takes longer than `budget`.
fn within<T>(what: &str, budget: Duration, work: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = work();
    let took = start.elapsed();
    assert!(took <= budget, "{what} took {took:?}, over its budget of {budget:?}");
    result
}

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

/// The classic blow-up: "the k-th symbol from the end is 1". The NFA has
/// k + 1 states; the smallest DFA has 2^k, one for each possible window of
/// the last k symbols.
fn kth_from_end(k: usize) -> Nfa {
    let mut nfa = Nfa::new(vec!['0', '1']);
    for i in 0..=k {
        nfa.add_state(format!("q{i}"));
    }
    nfa.add_edge(0, Some('0'), 0);
    nfa.add_edge(0, Some('1'), 0);
    nfa.add_edge(0, Some('1'), 1);
    for i in 1..k {
        nfa.add_edge(i, Some('0'), i + 1);
        nfa.add_edge(i, Some('1'), i + 1);
    }
    nfa.accept.insert(k);
    nfa
}

fn subset_construction_blow_up(k: usize, budget: Duration) {
    let nfa = kth_from_end(k);
    let dfa = within(&format!("to_dfa with k = {k}"), budget, || convert::nfa_to_dfa(&nfa));
    assert_eq!(dfa.states.len(), 1 << k);
    let min = within(&format!("minimize with k = {k}"), budget, || minimize::minimize(&dfa));
    assert_eq!(min.states.len(), 1 << k, "the 2^k states are all distinguishable");
    within(&format!("equivalent with k = {k}"), budget, || assert!(ops::equivalent(&dfa, &min)));
    // Spot checks: a word with 1 exactly k from the end, and one without.
    let yes = format!("{}1{}", "01".repeat(5), "0".repeat(k - 1));
    let no = format!("{}0{}", "01".repeat(5), "1".repeat(k - 1));
    assert!(min.accepts(&yes).unwrap() && !min.accepts(&no).unwrap());
}

#[test]
fn subset_construction_with_thousands_of_states() {
    subset_construction_blow_up(10, secs(20));
}

#[test]
#[ignore]
fn subset_construction_with_tens_of_thousands_of_states() {
    subset_construction_blow_up(15, secs(120));
}

fn long_word(length: usize) -> String {
    (0..length).map(|i| if (i * 7 + i / 3) % 5 < 2 { '1' } else { '0' }).collect()
}

#[test]
fn a_million_symbols() {
    let word = long_word(1_000_000);
    let nfa = kth_from_end(8);
    let dfa = convert::nfa_to_dfa(&nfa);
    let expected = word.as_bytes()[word.len() - 8] == b'1';
    let by_dfa = within("a DFA on a million symbols", secs(10), || dfa.accepts(&word).unwrap());
    assert_eq!(by_dfa, expected);
    let by_nfa = within("an NFA on a million symbols", secs(60), || nfa.accepts(&word).unwrap());
    assert_eq!(by_nfa, expected);
}

/// The same through the language: a million-symbol word built by doubling,
/// then given to a machine.
#[test]
fn a_million_symbols_in_a_program() {
    let program = r#"
alphabet B = {0, 1}
fn double(s: Str, times: Int) -> Str {
    if times == 0 {
        return s
    }
    return double(s + s, times - 1)
}
dfa EndsIn1 over B {
    start  q0
    accept q1
    q0 --0--> q0
    q0 --1--> q1
    q1 --0--> q0
    q1 --1--> q1
}
let word = double("0110", 18)
print len(word)
test EndsIn1 accepts word + "1"
test not (EndsIn1 accepts word)
test to_dfa(/(0|1)*1/) accepts word + "1"
"#;
    let outcome = within("a program on a million symbols", secs(60), || common::run_program("big.kstar", program));
    assert!(outcome.ok, "{}", outcome.output);
    assert!(outcome.output.starts_with("1048576\n"), "{}", outcome.output);
}

#[test]
fn long_traces() {
    let word: String = long_word(5_000);
    let program = format!(
        "alphabet B = {{0, 1}}\ndfa M over B {{ start a  accept b  a --0--> a  a --1--> b  b --0,1--> b }}\nnfa N over B {{ start a  accept b  a --0,1--> a  a --1--> b }}\ntrace M on \"{word}\"\ntrace N on \"{word}\"\n"
    );
    let outcome = within("tracing 5000 steps", secs(30), || common::run_program("trace.kstar", &program));
    assert!(outcome.ok, "{}", &outcome.output[..outcome.output.len().min(2000)]);
    assert!(outcome.output.lines().count() > 10_000);
}

/// A grammar with many variables and rules: a chain `V0 -> a V1 | b V1`, ...
/// ending in a balanced-parentheses part, so CYK has real work to do.
fn big_grammar(variables: usize) -> Grammar {
    let mut rules = Vec::new();
    for v in 0..variables {
        let next = Sym::V(v + 1);
        rules.push(Rule { head: v, body: vec![Sym::T('a'), next] });
        rules.push(Rule { head: v, body: vec![Sym::T('b'), next] });
        rules.push(Rule { head: v, body: vec![next] });
    }
    let last = variables;
    rules.push(Rule { head: last, body: vec![Sym::T('a'), Sym::V(last), Sym::T('b'), Sym::V(last)] });
    rules.push(Rule { head: last, body: vec![] });
    Grammar {
        alphabet: vec!['a', 'b'],
        variables: (0..=variables).map(|v| format!("V{v}")).collect(),
        start: 0,
        rules,
    }
}

#[test]
fn big_grammars() {
    let g = big_grammar(60);
    let cnf = within("to_cnf of a 60-variable grammar", secs(20), || g.to_cnf());
    assert!(!cnf.rules.is_empty());
    let word = format!("{}{}", "ab".repeat(15), "aabb".repeat(5));
    let accepted = within("CYK on a 50-symbol word", secs(30), || g.accepts(&word).unwrap());
    assert!(accepted);
    // The chain absorbs at most 60 symbols; what follows must be balanced.
    let unbalanced = "b".repeat(64);
    assert!(!within("CYK again", secs(30), || g.accepts(&unbalanced).unwrap()));
}

#[test]
#[ignore]
fn bigger_grammars() {
    let g = big_grammar(300);
    let word = format!("{}{}", "ab".repeat(60), "aabb".repeat(30));
    assert!(within("CYK on a 240-symbol word", secs(120), || g.accepts(&word).unwrap()));
    assert!(!within("CYK again", secs(120), || g.accepts(&"b".repeat(304)).unwrap()));
}

/// State elimination can produce long regexes; it must still finish, and
/// the regex must describe the same language. (Regression: before the
/// elimination order was chosen by cost, this took over five minutes.)
#[test]
fn regex_from_a_bigger_dfa() {
    let nfa = kth_from_end(5);
    let dfa: Dfa = convert::nfa_to_dfa(&nfa);
    let regex = within("to_regex of a 32-state DFA", secs(30), || convert::dfa_to_regex(&dfa));
    let back = within("and back", secs(30), || convert::nfa_to_dfa(&convert::regex_to_nfa(&regex, &dfa.alphabet)));
    assert!(ops::equivalent(&dfa, &back));
}

#[test]
fn turing_machine_step_limit_is_fast() {
    let program = "alphabet B = {0, 1}\ntm Loop over B {\n    start  s\n    accept never\n    s --0/0,R--> s\n    s --1/1,L--> s\n    s --_/_,R--> s\n}\ntest not (Loop accepts \"01\")\n";
    let outcome = within("a Turing machine hitting its step limit", secs(30), || common::run_program("tm.kstar", program));
    assert!(outcome.output.contains("E0604"), "{}", outcome.output);
}

/// Regression: renaming a machine's states searched a list for every state,
/// which made Thompson's construction quadratic in the size of the regex.
#[test]
fn renaming_big_machines_is_linear() {
    let mut nfa = Nfa::new(vec!['0', '1']);
    let n = 100_000;
    for i in 0..n {
        nfa.add_state(format!("s{i}"));
    }
    // A chain through the first half; the other half is unreachable, which
    // is the case that was slow.
    for i in 0..n / 2 - 1 {
        nfa.add_edge(i, Some('0'), i + 1);
    }
    let renamed = within("renaming 100000 states", secs(10), || nfa.renamed());
    assert_eq!(renamed.states.len(), n);
}

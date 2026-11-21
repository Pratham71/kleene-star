//! More about context-free grammars: the questions a parser writer asks
//! (FIRST and FOLLOW sets, is the grammar LL(1)?), checking Chomsky normal
//! form, listing the short words of a language, and a few grammars that
//! come up again and again.
//!
//! The grammar type and the transformations that change a grammar
//! (`to_cnf`, `remove_eps`, `remove_units`, `trimmed`) are in `grammar`.

use super::grammar::{Grammar, Rule, Sym};
use std::collections::BTreeSet;

/// FIRST of every variable: the terminals that can begin a word it
/// derives. Whether it can also derive the empty word is `nullable`.
///
/// The usual fixed point: `FIRST(A)` gets `FIRST` of each body of A, which
/// is the first symbols' sets up to and including the first symbol that is
/// not nullable.
pub fn first_sets(g: &Grammar) -> Vec<BTreeSet<char>> {
    let nullable = g.nullable();
    let mut first = vec![BTreeSet::new(); g.variables.len()];
    loop {
        let mut changed = false;
        for rule in &g.rules {
            let (add, _) = first_of(&rule.body, &first, &nullable);
            for c in add {
                changed |= first[rule.head].insert(c);
            }
        }
        if !changed {
            return first;
        }
    }
}

/// FIRST of a sequence of symbols, and whether the whole sequence is
/// nullable, given FIRST and nullability of the variables.
pub fn first_of(symbols: &[Sym], first: &[BTreeSet<char>], nullable: &[bool]) -> (BTreeSet<char>, bool) {
    let mut out = BTreeSet::new();
    for s in symbols {
        match *s {
            Sym::T(c) => {
                out.insert(c);
                return (out, false);
            }
            Sym::V(v) => {
                out.extend(first[v].iter().copied());
                if !nullable[v] {
                    return (out, false);
                }
            }
        }
    }
    (out, true)
}

/// FOLLOW of every variable: the terminals that can come right after it
/// in a sentential form derived from the start, and whether it can come
/// last (the end of the input, written `$` in many books).
pub fn follow_sets(g: &Grammar) -> Vec<(BTreeSet<char>, bool)> {
    let nullable = g.nullable();
    let first = first_sets(g);
    let mut follow = vec![(BTreeSet::new(), false); g.variables.len()];
    follow[g.start].1 = true;
    loop {
        let mut changed = false;
        for rule in &g.rules {
            for (i, s) in rule.body.iter().enumerate() {
                let Sym::V(b) = *s else { continue };
                // What follows B here: FIRST of the rest of the body, and if
                // the rest can vanish, whatever follows the head.
                let (add, rest_nullable) = first_of(&rule.body[i + 1..], &first, &nullable);
                for c in add {
                    changed |= follow[b].0.insert(c);
                }
                if rest_nullable && b != rule.head {
                    let (head_set, head_end) = follow[rule.head].clone();
                    for c in head_set {
                        changed |= follow[b].0.insert(c);
                    }
                    if head_end && !follow[b].1 {
                        follow[b].1 = true;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            return follow;
        }
    }
}

/// Is the grammar LL(1), so a parser can always pick the rule to use by
/// looking at the next symbol only? For each variable A and any two of
/// its rules `A -> x` and `A -> y`:
///
/// * FIRST(x) and FIRST(y) have no terminal in common;
/// * at most one of x and y is nullable;
/// * if x is nullable, FIRST(y) has no terminal in FOLLOW(A).
///
/// Variables that cannot be reached do not matter to a parser, so they are
/// not checked.
pub fn is_ll1(g: &Grammar) -> bool {
    let nullable = g.nullable();
    let first = first_sets(g);
    let follow = follow_sets(g);
    let reachable = g.reachable();
    for (a, _) in g.variables.iter().enumerate().filter(|(a, _)| reachable[*a]) {
        let bodies: Vec<(BTreeSet<char>, bool)> = g.rules_for(a).map(|r| first_of(&r.body, &first, &nullable)).collect();
        for (i, (fx, nx)) in bodies.iter().enumerate() {
            for (fy, ny) in &bodies[i + 1..] {
                if !fx.is_disjoint(fy) || (*nx && *ny) {
                    return false;
                }
                if (*nx && !fy.is_disjoint(&follow[a].0)) || (*ny && !fx.is_disjoint(&follow[a].0)) {
                    return false;
                }
            }
        }
    }
    true
}

/// Is the grammar in Chomsky normal form? Every rule is `A -> B C` (two
/// variables) or `A -> a` (one terminal); the start variable may also have
/// `S -> eps`, and then it must not appear on any right-hand side.
pub fn is_cnf(g: &Grammar) -> bool {
    let start_eps = g.rules.iter().any(|r| r.head == g.start && r.body.is_empty());
    g.rules.iter().all(|r| match r.body[..] {
        [] => r.head == g.start,
        [Sym::T(_)] => true,
        [Sym::V(b), Sym::V(c)] => !(start_eps && (b == g.start || c == g.start)),
        _ => false,
    })
}

/// The words of the language of length at most `n`, or `None` if listing
/// them would take more than `budget` characters of work (each word built
/// costs its length, whether or not it is new).
///
/// Works on the CNF copy, building for every variable and every length
/// the words it derives: those of length 1 come from rules `A -> a`, and
/// those of length k from rules `A -> B C`, a word of B of length i
/// followed by a word of C of length k - i. This is CYK run forwards.
pub fn words_up_to(g: &Grammar, n: usize, budget: usize) -> Option<BTreeSet<String>> {
    let cnf = g.to_cnf();
    let vars = cnf.variables.len();
    // `words[v][k]`: the words of length k that variable v derives, and
    // `lengths[v]`: the k for which there are any, so that the splits
    // tried are only those that can give something.
    let mut words: Vec<Vec<BTreeSet<String>>> = vec![vec![BTreeSet::new(); n + 1]; vars];
    let mut lengths: Vec<Vec<usize>> = vec![Vec::new(); vars];
    let mut spent = 0usize;
    for rule in &cnf.rules {
        match rule.body[..] {
            [] => {
                words[rule.head][0].insert(String::new());
            }
            [Sym::T(c)] if n >= 1 => {
                spent += 1;
                words[rule.head][1].insert(c.to_string());
            }
            _ => {}
        }
    }
    for (v, by_length) in words.iter().enumerate() {
        if n >= 1 && !by_length[1].is_empty() {
            lengths[v].push(1);
        }
    }
    let pairs: Vec<(usize, usize, usize)> = cnf
        .rules
        .iter()
        .filter_map(|r| match r.body[..] {
            [Sym::V(b), Sym::V(c)] => Some((r.head, b, c)),
            _ => None,
        })
        .collect();
    for k in 2..=n {
        for &(head, b, c) in &pairs {
            let mut new = Vec::new();
            for &i in lengths[b].iter().filter(|&&i| i < k) {
                for left in &words[b][i] {
                    for right in &words[c][k - i] {
                        spent += k;
                        if spent > budget {
                            return None;
                        }
                        new.push(format!("{left}{right}"));
                    }
                }
            }
            words[head][k].extend(new);
        }
        for (v, by_length) in words.iter().enumerate() {
            if !by_length[k].is_empty() {
                lengths[v].push(k);
            }
        }
    }
    Some(words[cnf.start].iter().flatten().cloned().collect())
}

// ---- ready-made grammars ----

/// A grammar with the one variable `S` and these bodies.
fn one_variable(alphabet: &[char], bodies: Vec<Vec<Sym>>) -> Grammar {
    let mut alphabet = alphabet.to_vec();
    alphabet.sort_unstable();
    alphabet.dedup();
    Grammar {
        alphabet,
        variables: vec!["S".into()],
        start: 0,
        rules: bodies.into_iter().map(|body| Rule { head: 0, body }).collect(),
    }
}

const S: Sym = Sym::V(0);

/// Balanced brackets: `S -> open S close S | eps`. Every word in which the
/// brackets pair up, like `(()())`. This form (rather than `S -> S S |
/// ...`) is unambiguous and LL(1).
pub fn balanced(open: char, close: char) -> Grammar {
    one_variable(&[open, close], vec![vec![Sym::T(open), S, Sym::T(close), S], vec![]])
}

/// `S -> a S b | eps`: a^n b^n, the standard language that is not regular.
pub fn anbn(a: char, b: char) -> Grammar {
    one_variable(&[a, b], vec![vec![Sym::T(a), S, Sym::T(b)], vec![]])
}

/// The palindromes over an alphabet: `S -> eps | c | c S c` for each
/// symbol c.
pub fn palindromes(alphabet: &[char]) -> Grammar {
    let mut bodies = vec![vec![]];
    for &c in alphabet {
        bodies.push(vec![Sym::T(c)]);
        bodies.push(vec![Sym::T(c), S, Sym::T(c)]);
    }
    one_variable(alphabet, bodies)
}

/// The words over {a, b} with as many a's as b's:
/// `S -> a S b S | b S a S | eps`. (A word with equal counts either is
/// empty or splits after the first point where the counts are equal.)
pub fn equal_count(a: char, b: char) -> Grammar {
    one_variable(&[a, b], vec![vec![Sym::T(a), S, Sym::T(b), S], vec![Sym::T(b), S, Sym::T(a), S], vec![]])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Brute force: every word over the alphabet of length at most `n`
    /// that the grammar accepts (by CYK).
    fn by_cyk(g: &Grammar, n: usize) -> BTreeSet<String> {
        let mut all = vec![String::new()];
        let mut out = BTreeSet::new();
        for _ in 0..=n {
            let mut next = Vec::new();
            for w in &all {
                if g.accepts(w).unwrap() {
                    out.insert(w.clone());
                }
                for &c in &g.alphabet {
                    next.push(format!("{w}{c}"));
                }
            }
            all = next;
        }
        out
    }

    fn set(words: &[&str]) -> BTreeSet<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    /// The textbook expression grammar, made LL(1):
    /// E -> T E', E' -> + T E' | eps, T -> F T', T' -> * F T' | eps,
    /// F -> ( E ) | i
    fn expressions() -> Grammar {
        let (e, e2, t, t2, f) = (Sym::V(0), Sym::V(1), Sym::V(2), Sym::V(3), Sym::V(4));
        let rule = |head, body: Vec<Sym>| Rule { head, body };
        Grammar {
            alphabet: vec!['(', ')', '*', '+', 'i'],
            variables: ["E", "E'", "T", "T'", "F"].iter().map(|s| s.to_string()).collect(),
            start: 0,
            rules: vec![
                rule(0, vec![t, e2]),
                rule(1, vec![Sym::T('+'), t, e2]),
                rule(1, vec![]),
                rule(2, vec![f, t2]),
                rule(3, vec![Sym::T('*'), f, t2]),
                rule(3, vec![]),
                rule(4, vec![Sym::T('('), e, Sym::T(')')]),
                rule(4, vec![Sym::T('i')]),
            ],
        }
    }

    #[test]
    fn first_and_follow_of_the_expression_grammar() {
        let g = expressions();
        let first = first_sets(&g);
        let follow = follow_sets(&g);
        let chars = |s: &str| s.chars().collect::<BTreeSet<char>>();
        assert_eq!(first[0], chars("(i"));
        assert_eq!(first[1], chars("+"));
        assert_eq!(first[3], chars("*"));
        assert_eq!(follow[0], (chars(")"), true));
        assert_eq!(follow[1], (chars(")"), true));
        assert_eq!(follow[2], (chars("+)"), true));
        assert_eq!(follow[3], (chars("+)"), true));
        assert_eq!(follow[4], (chars("*+)"), true));
        assert!(is_ll1(&g));
    }

    #[test]
    fn ambiguous_and_left_recursive_grammars_are_not_ll1() {
        // S -> S S | ( S ) | eps
        let s = Sym::V(0);
        let g = one_variable(&['(', ')'], vec![vec![s, s], vec![Sym::T('('), s, Sym::T(')')], vec![]]);
        assert!(!is_ll1(&g));
        assert!(is_ll1(&balanced('l', 'r')));
        assert!(is_ll1(&anbn('a', 'b')));
        assert!(!is_ll1(&palindromes(&['a', 'b'])));
    }

    #[test]
    fn cnf_is_recognised() {
        assert!(!is_cnf(&anbn('a', 'b')));
        assert!(is_cnf(&anbn('a', 'b').to_cnf()));
        assert!(is_cnf(&palindromes(&['a', 'b']).to_cnf()));
        // S -> S S | a is CNF; adding S -> eps breaks it (S is on the right).
        let s = Sym::V(0);
        assert!(is_cnf(&one_variable(&['a'], vec![vec![s, s], vec![Sym::T('a')]])));
        assert!(!is_cnf(&one_variable(&['a'], vec![vec![s, s], vec![Sym::T('a')], vec![]])));
    }

    #[test]
    fn ready_made_grammars_have_their_languages() {
        assert_eq!(words_up_to(&anbn('a', 'b'), 4, 1000).unwrap(), set(&["", "ab", "aabb"]));
        assert_eq!(words_up_to(&balanced('l', 'r'), 4, 1000).unwrap(), set(&["", "lr", "llrr", "lrlr"]));
        assert_eq!(
            words_up_to(&palindromes(&['a', 'b']), 3, 1000).unwrap(),
            set(&["", "a", "b", "aa", "bb", "aaa", "aba", "bab", "bbb"])
        );
        let equal = words_up_to(&equal_count('a', 'b'), 4, 1000).unwrap();
        assert_eq!(equal, set(&["", "ab", "ba", "aabb", "abab", "abba", "baab", "baba", "bbaa"]));
    }

    #[test]
    fn listing_words_agrees_with_cyk_and_respects_the_budget() {
        for g in [expressions(), equal_count('a', 'b'), palindromes(&['a', 'b', 'c'])] {
            assert_eq!(words_up_to(&g, 5, 100_000).unwrap(), by_cyk(&g, 5));
        }
        assert_eq!(words_up_to(&palindromes(&['a', 'b']), 12, 500), None);
    }

    #[test]
    fn removing_eps_and_unit_rules_keeps_the_language() {
        // S -> A B | a, A -> B | eps, B -> b | eps
        let rule = |head, body: Vec<Sym>| Rule { head, body };
        let g = Grammar {
            alphabet: vec!['a', 'b'],
            variables: ["S", "A", "B"].iter().map(|s| s.to_string()).collect(),
            start: 0,
            rules: vec![
                rule(0, vec![Sym::V(1), Sym::V(2)]),
                rule(0, vec![Sym::T('a')]),
                rule(1, vec![Sym::V(2)]),
                rule(1, vec![]),
                rule(2, vec![Sym::T('b')]),
                rule(2, vec![]),
            ],
        };
        let no_eps = g.remove_eps(1000).unwrap();
        assert!(no_eps.rules.iter().all(|r| !r.body.is_empty() || r.head == no_eps.start));
        let no_units = g.remove_units();
        assert!(no_units.rules.iter().all(|r| !matches!(r.body[..], [Sym::V(_)])));
        for h in [&no_eps, &no_units] {
            assert_eq!(by_cyk(h, 4), by_cyk(&g, 4));
        }
        assert!(g.remove_eps(3).is_none());
    }
}

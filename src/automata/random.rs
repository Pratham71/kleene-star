//! Seeded randomness: random words, machines and grammars that are the
//! same every time for the same seed, on every computer.
//!
//! Nothing here asks the operating system for randomness. The generator is
//! SplitMix64 (Steele, Lea and Flood, 2014): a 64-bit counter run through a
//! mixing function. It is small, fast, and passes the usual statistical
//! tests; it is *not* for cryptography. Every function takes the generator
//! it draws from, so a caller decides where the seed comes from.

use super::grammar::{Grammar, Rule, Sym};
use super::{Dfa, Nfa, State};
use std::collections::BTreeSet;

/// A SplitMix64 generator.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// The generator for `seed`. Any seed works, 0 included.
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n`, every one equally likely (`n` must not be 0).
    ///
    /// Plain `next % n` would favour small numbers when `n` does not divide
    /// 2^64, so draws from the uneven top end are thrown away and redrawn.
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "below(0)");
        let zone = u64::MAX - u64::MAX % n;
        loop {
            let x = self.next_u64();
            if x < zone {
                return x % n;
            }
        }
    }

    /// An index into a list of `n` things.
    pub fn index(&mut self, n: usize) -> usize {
        self.below(n as u64) as usize
    }

    /// True with probability `percent` in 100.
    pub fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    /// A number in `[0, 1)`, from the top 53 bits.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A word of length `n` over `alphabet`, each symbol drawn independently.
pub fn word(rng: &mut Rng, alphabet: &[char], n: usize) -> String {
    (0..n).map(|_| alphabet[rng.index(alphabet.len())]).collect()
}

/// A word of length exactly `n` in the language of `dfa`, every such word
/// equally likely (up to rounding), or `None` if there is none.
///
/// `ways[k][s]` counts the words of length k that lead from s to an
/// accepting state; each symbol is then chosen with probability
/// proportional to the words that remain after it. The counts grow like
/// |alphabet|^k, so each row is scaled down by its largest entry: only
/// their ratios matter.
pub fn word_of(rng: &mut Rng, dfa: &Dfa, n: usize) -> Option<String> {
    let states = dfa.states.len();
    // `has[k][s]`: is there such a word at all? Kept exactly, apart from
    // the scaled counts, which may round a tiny share down to 0.
    let mut has: Vec<Vec<bool>> = vec![(0..states).map(|s| dfa.accept.contains(&s)).collect()];
    let mut ways: Vec<Vec<f64>> = vec![has[0].iter().map(|&h| if h { 1.0 } else { 0.0 }).collect()];
    for k in 1..=n {
        has.push((0..states).map(|s| dfa.delta[s].iter().any(|&t| has[k - 1][t])).collect());
        let mut row: Vec<f64> = (0..states).map(|s| dfa.delta[s].iter().map(|&t| ways[k - 1][t]).sum()).collect();
        let max = row.iter().cloned().fold(0.0, f64::max);
        if max > 0.0 {
            row.iter_mut().for_each(|x| *x /= max);
        }
        ways.push(row);
    }
    if !has[n][dfa.start] {
        return None;
    }
    let mut out = String::new();
    let mut s = dfa.start;
    for left in (1..=n).rev() {
        let open: Vec<bool> = dfa.delta[s].iter().map(|&t| has[left - 1][t]).collect();
        let mut weights: Vec<f64> = dfa.delta[s].iter().map(|&t| ways[left - 1][t]).collect();
        if weights.iter().sum::<f64>() == 0.0 {
            // Every share rounded to 0: the ways on are equally negligible.
            weights = open.iter().map(|&o| if o { 1.0 } else { 0.0 }).collect();
        }
        let total: f64 = weights.iter().sum();
        let mut x = rng.unit() * total;
        // The last open symbol, in case rounding leaves `x` past the end.
        let mut choice = weights.iter().rposition(|&w| w > 0.0).expect("a state with words has a way on");
        for (i, &w) in weights.iter().enumerate() {
            if w > 0.0 && x < w {
                choice = i;
                break;
            }
            x -= w;
        }
        out.push(dfa.alphabet[choice]);
        s = dfa.delta[s][choice];
    }
    Some(out)
}

/// A random total DFA with states `q0`, `q1`, ...: each move goes to any
/// state with equal chance, and each state accepts with chance 1/2.
pub fn dfa(rng: &mut Rng, alphabet: &[char], states: usize) -> Dfa {
    Dfa {
        alphabet: alphabet.to_vec(),
        states: (0..states).map(|i| format!("q{i}")).collect(),
        start: 0,
        accept: (0..states).filter(|_| rng.chance(50)).collect(),
        delta: (0..states).map(|_| alphabet.iter().map(|_| rng.index(states)).collect()).collect(),
    }
}

/// A random NFA with states `q0`, `q1`, ...: on each symbol a state has
/// no, one or two moves (each as likely), it has an eps move to another
/// state with chance 1/4, and it accepts with chance 1/3.
pub fn nfa(rng: &mut Rng, alphabet: &[char], states: usize) -> Nfa {
    let mut nfa = Nfa::new(alphabet.to_vec());
    for i in 0..states {
        nfa.add_state(format!("q{i}"));
    }
    for s in 0..states {
        for &c in alphabet {
            for _ in 0..rng.index(3) {
                let to: State = rng.index(states);
                nfa.add_edge(s, Some(c), to);
            }
        }
        // An eps move to the state itself would change nothing.
        if states > 1 && rng.chance(25) {
            let to = (s + 1 + rng.index(states - 1)) % states;
            nfa.add_edge(s, None, to);
        }
    }
    nfa.accept = (0..states).filter(|_| rng.chance(33)).collect::<BTreeSet<State>>();
    nfa
}

/// The variable names `random_grammar` uses: `S`, then `A`, `B`, ... (no
/// second `S`), at most 26.
pub const VARIABLE_NAMES: &str = "SABCDEFGHIJKLMNOPQRTUVWXYZ";

/// A random grammar with `variables` variables (1 to 26) named from
/// `VARIABLE_NAMES`, start `S`. Each variable has one to three rules, and
/// each body has zero to three symbols, a terminal with chance 1/2.
pub fn grammar(rng: &mut Rng, alphabet: &[char], variables: usize) -> Grammar {
    let mut rules = Vec::new();
    for head in 0..variables {
        for _ in 0..1 + rng.index(3) {
            let body = (0..rng.index(4))
                .map(|_| {
                    if rng.chance(50) {
                        Sym::T(alphabet[rng.index(alphabet.len())])
                    } else {
                        Sym::V(rng.index(variables))
                    }
                })
                .collect();
            rules.push(Rule { head, body });
        }
    }
    Grammar {
        alphabet: alphabet.to_vec(),
        variables: VARIABLE_NAMES.chars().take(variables).map(|c| c.to_string()).collect(),
        start: 0,
        rules,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_numbers() {
        let a: Vec<u64> = { let mut r = Rng::new(42); (0..5).map(|_| r.next_u64()).collect() };
        let b: Vec<u64> = { let mut r = Rng::new(42); (0..5).map(|_| r.next_u64()).collect() };
        assert_eq!(a, b);
        assert_ne!(a, { let mut r = Rng::new(43); (0..5).map(|_| r.next_u64()).collect::<Vec<_>>() });
        // SplitMix64's published first output for seed 0, so the generator
        // is the real one and stays the same between versions.
        assert_eq!(Rng::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);
    }

    #[test]
    fn below_stays_in_range_and_hits_everything() {
        let mut r = Rng::new(7);
        let mut seen = [0usize; 6];
        for _ in 0..6000 {
            seen[r.below(6) as usize] += 1;
        }
        assert!(seen.iter().all(|&n| (800..1200).contains(&n)), "{seen:?}");
    }

    #[test]
    fn words_of_a_language_are_in_it_and_spread_out() {
        // The words over {a, b} that end in b: half of all words.
        let dfa = Dfa {
            alphabet: vec!['a', 'b'],
            states: vec!["x".into(), "y".into()],
            start: 0,
            accept: BTreeSet::from([1]),
            delta: vec![vec![0, 1], vec![0, 1]],
        };
        let mut r = Rng::new(1);
        let mut seen = std::collections::BTreeMap::new();
        for _ in 0..4000 {
            let w = word_of(&mut r, &dfa, 3).unwrap();
            assert!(w.ends_with('b') && w.len() == 3);
            *seen.entry(w).or_insert(0) += 1;
        }
        assert_eq!(seen.len(), 4, "all four words appear");
        assert!(seen.values().all(|&n| (850..1150).contains(&n)), "{seen:?}");
        // No word of length 0 ends in b.
        assert_eq!(word_of(&mut r, &dfa, 0), None);
        // Long words do not overflow the counts.
        assert_eq!(word_of(&mut r, &dfa, 5000).unwrap().len(), 5000);
    }
}

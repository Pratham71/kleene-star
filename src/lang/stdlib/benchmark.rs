//! `std.benchmark`: how much work the algorithms do, to watch it grow.
//!
//! Two kinds of measure:
//!
//! * **Counts** that are the same on every computer and every run: the
//!   rounds of refinement minimisation takes, the size of the regex state
//!   elimination builds, and the configurations a PDA's search looks at.
//!   Together with `states`, they show how the work grows with the input,
//!   for example how the subset construction blows up on "the n-th symbol
//!   from the end is 1".
//! * **Times**, in microseconds, which depend on the computer and change
//!   from run to run. Each is the median of five runs of the operation,
//!   after the input has been turned into the machine the operation needs.
//!
//! So that a program's output can be checked (in a `test`, or as the
//! expected output of a test file), the times can be fixed: with
//! `kstar run --fixed-clock`, and always in Kleene Star's own tests, every
//! time is 0. The interpreter says which, for each call (`with_clock`).

use super::{str_arg, Failure, Function, Module, Ty};
use crate::automata::{convert, minimize, ops, pda};
use crate::lang::codes;
use crate::lang::langs;
use crate::lang::value::Value;
use std::cell::Cell;
use std::time::Instant;

/// How many times each operation is run for a time; the median is kept.
pub const RUNS: usize = 5;

/// The most parts of a regex `regex_size` builds, as `to_regex` does.
const MAX_REGEX_SIZE: usize = 100_000;

/// The operations `time_us` can time.
pub const OPERATIONS: &[&str] = &["complement", "minimize", "remove_eps", "reverse", "to_dfa", "to_regex"];

pub const MODULE: Module = Module {
    name: "benchmark",
    doc: "How much work the algorithms do: counts that are the same on every computer, and times in microseconds.",
    functions: &[
        Function { name: "equivalence_rounds", params: &[("m", Ty::Lang)], ret: Ty::Int, doc: "The rounds of refinement minimising `m`'s DFA takes: the smallest k for which k-equivalence equals (k+1)-equivalence.", run: equivalence_rounds },
        Function { name: "pda_configs", params: &[("p", Ty::Pda), ("w", Ty::Str)], ret: Ty::Int, doc: "How many configurations the search for an accepting run of `p` on `w` looks at.", run: pda_configs },
        Function { name: "regex_size", params: &[("m", Ty::Lang)], ret: Ty::Int, doc: "The number of symbols and operators in the regex state elimination builds for `m`, as `to_regex` gives it.", run: regex_size },
        Function { name: "time_accepts_us", params: &[("m", Ty::Acceptor), ("w", Ty::Str)], ret: Ty::Int, doc: "Microseconds to decide whether `m` accepts `w`, the median of 5 runs; 0 with a fixed clock.", run: time_accepts_us },
        Function { name: "time_us", params: &[("op", Ty::Str), ("m", Ty::Lang)], ret: Ty::Int, doc: "Microseconds the operation `op` (`to_dfa`, `minimize`, `to_regex`, `remove_eps`, `complement` or `reverse`) takes on `m`, the median of 5 runs; 0 with a fixed clock.", run: time_us },
    ],
    explain: None,
};

thread_local! {
    /// Whether the run calling a `std.benchmark` function has a fixed clock.
    static FIXED: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f`, a call of a `std.benchmark` function, with a fixed clock or not.
pub fn with_clock<T>(fixed: bool, f: impl FnOnce() -> T) -> T {
    let before = FIXED.with(|c| c.replace(fixed));
    let result = f();
    FIXED.with(|c| c.set(before));
    result
}

/// The median time of `RUNS` runs of `work`, in microseconds; 0 with a
/// fixed clock (the work still runs, so a failure still shows).
fn median_us(mut work: impl FnMut()) -> i64 {
    let mut times: Vec<u128> = (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            work();
            start.elapsed().as_micros()
        })
        .collect();
    if FIXED.with(Cell::get) {
        return 0;
    }
    times.sort_unstable();
    i64::try_from(times[RUNS / 2]).unwrap_or(i64::MAX)
}

fn dfa_of(m: &Value) -> crate::automata::Dfa {
    langs::to_dfa(m, &langs::symbols_of(m))
}

fn nfa_of(m: &Value) -> crate::automata::Nfa {
    langs::to_nfa(m, &langs::symbols_of(m))
}

fn equivalence_rounds(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Int(minimize::equivalence_rounds(&dfa_of(&args[0])) as i64))
}

fn regex_size(args: &[Value]) -> Result<Value, Failure> {
    let size = match &args[0] {
        Value::Regex(r) => r.size(),
        m => convert::dfa_to_regex_within(&dfa_of(m), MAX_REGEX_SIZE)
            .ok_or_else(|| Failure {
                code: codes::TOO_LARGE,
                arg: Some(0),
                message: format!("the regex for this language would be too large (over {MAX_REGEX_SIZE} parts)"),
                hint: "some languages only have very long regexes; count the states of its DFA instead".into(),
            })?
            .size(),
    };
    Ok(Value::Int(size as i64))
}

fn pda_configs(args: &[Value]) -> Result<Value, Failure> {
    let Value::Pda(p) = &args[0] else { unreachable!("the checker allowed {:?} as a Pda", args[0]) };
    let search = p.search(str_arg(args, 1), pda::STEP_LIMIT);
    if let pda::Outcome::Foreign { position, symbol } = search.outcome {
        return Err(Failure {
            code: codes::FOREIGN_WORD,
            arg: Some(1),
            message: format!("the word has `{symbol}` at position {position}, which is not in the alphabet"),
            hint: format!("the alphabet is {}", crate::automata::fmt_alphabet(&p.alphabet)),
        });
    }
    Ok(Value::Int(search.explored as i64))
}

fn time_us(args: &[Value]) -> Result<Value, Failure> {
    let op = str_arg(args, 0);
    let m = &args[1];
    let micros = match op {
        "to_dfa" => {
            let n = nfa_of(m);
            median_us(|| drop(convert::nfa_to_dfa(&n)))
        }
        "remove_eps" => {
            let n = nfa_of(m);
            median_us(|| drop(convert::remove_eps(&n)))
        }
        "reverse" => {
            let n = nfa_of(m);
            median_us(|| drop(ops::reverse(&n)))
        }
        "minimize" => {
            let d = dfa_of(m);
            median_us(|| drop(minimize::minimize(&d)))
        }
        "complement" => {
            let d = dfa_of(m);
            median_us(|| drop(ops::complement(&d)))
        }
        "to_regex" => {
            let d = dfa_of(m);
            if convert::dfa_to_regex_within(&d, MAX_REGEX_SIZE).is_none() {
                return regex_size(&args[1..]);
            }
            median_us(|| drop(convert::dfa_to_regex_within(&d, MAX_REGEX_SIZE)))
        }
        _ => {
            let names: Vec<String> = OPERATIONS.iter().map(|o| format!("\"{o}\"")).collect();
            return Err(Failure {
                code: codes::UNKNOWN_OPERATION,
                arg: Some(0),
                message: format!("`std.benchmark.time_us` cannot time \"{op}\""),
                hint: format!("it times {}", names.join(", ")),
            });
        }
    };
    Ok(Value::Int(micros))
}

fn time_accepts_us(args: &[Value]) -> Result<Value, Failure> {
    let (m, w) = (&args[0], str_arg(args, 1));
    // Once first, so a word the machine refuses is an error, not a time.
    super::testing::accepts(args, 0, w, Some(1))?;
    Ok(Value::Int(median_us(|| drop(langs::accepts(m, w)))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::dfa::tests::even_zeros;

    fn call(name: &str, args: &[Value]) -> Result<Value, Failure> {
        (super::super::function("benchmark", name).unwrap().run)(args)
    }

    #[test]
    fn a_fixed_clock_gives_zero() {
        let m = Value::dfa(even_zeros());
        for op in OPERATIONS {
            assert_eq!(with_clock(true, || call("time_us", &[Value::str(op), m.clone()])), Ok(Value::Int(0)), "{op}");
        }
        assert_eq!(with_clock(true, || call("time_accepts_us", &[m.clone(), Value::str("0110")])), Ok(Value::Int(0)));
        let real = with_clock(false, || call("time_us", &[Value::str("minimize"), m]));
        assert!(matches!(real, Ok(Value::Int(n)) if n >= 0));
    }

    #[test]
    fn counts() {
        let m = Value::dfa(even_zeros());
        assert_eq!(call("equivalence_rounds", std::slice::from_ref(&m)), Ok(Value::Int(0)));
        assert!(matches!(call("regex_size", &[m]), Ok(Value::Int(n)) if n > 3));
    }

    #[test]
    fn an_unknown_operation_names_the_known_ones() {
        let failure = call("time_us", &[Value::str("sort"), Value::dfa(even_zeros())]).unwrap_err();
        assert_eq!(failure.code, codes::UNKNOWN_OPERATION);
        assert!(failure.hint.contains("\"to_dfa\""), "{}", failure.hint);
    }
}

//! `std.tm`: ready-made Turing machines and questions about runs, building
//! on the built-ins `run`, `accepts`, `trace` and `states`.
//!
//! The machines are in `automata::tms`, each with its tape convention
//! (what the input must be, what the tape holds at the end). The questions
//! run the machine with `Tm::run` and the same step limit as `run` and
//! `accepts`: 100 000 steps, then a `LimitError`, because by the halting
//! problem no program can wait for every machine to halt. `halts_within`
//! is the one question that answers past the limit, with "not yet".

use super::grammar::symbol;
use super::{alphabet_arg, int_arg as int, str_arg as text, Failure, Function, Module, Ty};
use crate::automata::tm::{self, Halt, Tm};
use crate::automata::{fmt_alphabet, tms};
use crate::lang::codes;
use crate::lang::value::Value;
use std::collections::BTreeSet;
use std::rc::Rc;

pub const MODULE: Module = Module {
    name: "tm",
    doc: "Ready-made Turing machines (binary increment, unary addition and copying, deciders for palindromes and a^n b^n c^n), and the steps, space and final state of a run.",
    functions: &[
        Function { name: "increment", params: &[], ret: Ty::Tm, doc: "A machine that adds one to a binary number, leaving the sum on the tape.", run: increment },
        Function { name: "unary_add", params: &[], ret: Ty::Tm, doc: "A machine that turns `m` ones, a 0 and `n` ones into `m + n` ones.", run: unary_add },
        Function { name: "unary_copy", params: &[], ret: Ty::Tm, doc: "A machine that turns `n` ones into `n` ones, a 0 and `n` ones again.", run: unary_copy },
        Function { name: "palindromes", params: &[("alphabet", Ty::Alphabet)], ret: Ty::Tm, doc: "A machine that decides the palindromes over `alphabet`, halting in `yes` or `no`.", run: palindromes },
        Function { name: "anbncn", params: &[("a", Ty::Str), ("b", Ty::Str), ("c", Ty::Str)], ret: Ty::Tm, doc: "A machine that decides `n` copies of `a`, then of `b`, then of `c`, which no PDA can.", run: anbncn },
        Function { name: "steps", params: &[("m", Ty::Tm), ("w", Ty::Str)], ret: Ty::Int, doc: "How many steps the machine takes on `w` before it halts.", run: steps },
        Function { name: "space", params: &[("m", Ty::Tm), ("w", Ty::Str)], ret: Ty::Int, doc: "How many tape cells the head visits on `w` before the machine halts.", run: space },
        Function { name: "final_state", params: &[("m", Ty::Tm), ("w", Ty::Str)], ret: Ty::Str, doc: "The state the machine is in when it halts on `w`.", run: final_state },
        Function { name: "halts_within", params: &[("m", Ty::Tm), ("w", Ty::Str), ("n", Ty::Int)], ret: Ty::Bool, doc: "Whether the machine halts on `w` within `n` steps, at most 100000.", run: halts_within },
        Function { name: "tape_symbols", params: &[("m", Ty::Tm)], ret: Ty::Set(&Ty::Str), doc: "The symbols that can be on the tape: the alphabet, the blank `_`, and every symbol the machine writes.", run: tape_symbols },
        Function { name: "ids", params: &[("m", Ty::Tm), ("w", Ty::Str)], ret: Ty::Str, doc: "The run of `m` on `w` as instantaneous descriptions `u q v` (the head on the first symbol of `v`), one per line after `⊢`, then how it halted; at most 100 steps.", run: ids },
    ],
    explain: None,
};

fn machine(args: &[Value]) -> &Tm {
    match &args[0] {
        Value::Tm(m) => m,
        other => unreachable!("the checker allowed {other:?} as a Tm"),
    }
}

fn value(m: Tm) -> Value {
    Value::Tm(Rc::new(m))
}

/// Runs argument 0 on argument 1 for at most `limit` steps, refusing an
/// input outside the alphabet.
fn run_with(args: &[Value], limit: usize) -> Result<tm::Run, Failure> {
    let m = machine(args);
    let run = m.run(text(args, 1), limit, false);
    if let Halt::Foreign { symbol, .. } = run.halt {
        return Err(Failure {
            code: codes::FOREIGN_WORD,
            arg: Some(1),
            message: format!("the input contains `{symbol}`, which is not in the machine's alphabet"),
            hint: format!("the alphabet is {}", fmt_alphabet(&m.alphabet)),
        });
    }
    Ok(run)
}

/// The run of argument 0 on argument 1 to its end, or a `LimitError` if it
/// is still going after the step limit, as for the built-in `run`.
fn halted(args: &[Value]) -> Result<tm::Run, Failure> {
    let run = run_with(args, tm::STEP_LIMIT)?;
    if run.halt == Halt::GaveUp {
        return Err(Failure {
            code: codes::STEP_LIMIT,
            arg: None,
            message: format!("the Turing machine is still running after {} steps", tm::STEP_LIMIT),
            hint: "it may never halt; `std.tm.halts_within` asks whether it halts within a number of steps".into(),
        });
    }
    Ok(run)
}

fn increment(_: &[Value]) -> Result<Value, Failure> {
    Ok(value(tms::increment()))
}

fn unary_add(_: &[Value]) -> Result<Value, Failure> {
    Ok(value(tms::unary_add()))
}

fn unary_copy(_: &[Value]) -> Result<Value, Failure> {
    Ok(value(tms::unary_copy()))
}

fn palindromes(args: &[Value]) -> Result<Value, Failure> {
    Ok(value(tms::palindromes(alphabet_arg(args, 0))))
}

fn anbncn(args: &[Value]) -> Result<Value, Failure> {
    let symbols = [symbol(args, 0)?, symbol(args, 1)?, symbol(args, 2)?];
    for (i, &s) in symbols.iter().enumerate() {
        if symbols[..i].contains(&s) {
            return Err(Failure {
                code: codes::DUPLICATE_SYMBOL,
                arg: Some(i),
                message: format!("`{s}` is given twice"),
                hint: "the three symbols must be different".into(),
            });
        }
        if tms::MARKS.contains(&s) {
            return Err(Failure {
                code: codes::DUPLICATE_SYMBOL,
                arg: Some(i),
                message: format!("`{s}` is one of the marks the machine writes"),
                hint: "the machine marks the symbols it has matched with x, y and z, so use other letters".into(),
            });
        }
    }
    let [a, b, c] = symbols;
    Ok(value(tms::anbncn(a, b, c)))
}

fn steps(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Int(halted(args)?.steps as i64))
}

fn space(args: &[Value]) -> Result<Value, Failure> {
    Ok(Value::Int(halted(args)?.cells as i64))
}

fn final_state(args: &[Value]) -> Result<Value, Failure> {
    let run = halted(args)?;
    Ok(Value::str(&machine(args).states[run.last().state]))
}

fn halts_within(args: &[Value]) -> Result<Value, Failure> {
    let n = int(args, 2);
    if n < 0 {
        return Err(Failure {
            code: codes::STRING_RANGE,
            arg: Some(2),
            message: format!("a number of steps cannot be negative, but this is {n}"),
            hint: "use 0 or more".into(),
        });
    }
    if n as u64 > tm::STEP_LIMIT as u64 {
        return Err(Failure {
            code: codes::RESULT_TOO_LARGE,
            arg: Some(2),
            message: format!("a number of steps can be at most {}, but this is {n}", tm::STEP_LIMIT),
            hint: format!("use {} or less, the step limit of every Turing machine run", tm::STEP_LIMIT),
        });
    }
    Ok(Value::Bool(run_with(args, n as usize)?.halt != Halt::GaveUp))
}

/// The run as instantaneous descriptions, as `trace` shows it but one
/// line per configuration: `X q1 011`, then `⊢ ...` for each step.
fn ids(args: &[Value]) -> Result<Value, Failure> {
    let m = machine(args);
    let limit = crate::lang::trace::TM_TRACE_LIMIT;
    let run = m.run(text(args, 1), limit, true);
    if let Halt::Foreign { symbol, .. } = run.halt {
        return Err(Failure {
            code: codes::FOREIGN_WORD,
            arg: Some(1),
            message: format!("the input contains `{symbol}`, which is not in the machine's alphabet"),
            hint: format!("the alphabet is {}", fmt_alphabet(&m.alphabet)),
        });
    }
    let mut lines: Vec<String> = Vec::new();
    for (i, c) in run.configs.iter().enumerate() {
        let id = c.tape.id(&m.states[c.state], c.head);
        lines.push(if i == 0 { id } else { format!("⊢ {id}") });
    }
    let last = run.last();
    let state = &m.states[last.state];
    lines.push(match run.halt {
        Halt::Accepted => format!("Accepted: halted in accepting state {state}."),
        Halt::Rejected => format!("Rejected: halted in rejecting state {state}."),
        Halt::Stuck { symbol } => format!("Rejected: state {state} has no move for `{symbol}`, so the machine halts."),
        Halt::GaveUp => format!("Still running after {limit} steps; `run` and `accepts` go on for up to {} steps.", tm::STEP_LIMIT),
        Halt::Foreign { .. } => unreachable!("refused above"),
    });
    Ok(Value::str(&lines.join("
")))
}

fn tape_symbols(args: &[Value]) -> Result<Value, Failure> {
    let symbols: BTreeSet<Value> = tms::tape_symbols(machine(args)).iter().map(|c| Value::str(&c.to_string())).collect();
    Ok(Value::Set(Rc::new(symbols)))
}

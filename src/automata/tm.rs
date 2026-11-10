//! Turing machines: a finite control with an unbounded tape.
//!
//! The machine starts in its start state with the input written on the tape
//! and the head on the first symbol. Every other cell holds the blank `_`.
//! Each step reads the symbol under the head, writes a symbol in its place,
//! moves the head left (`L`), right (`R`) or not at all (`S`), and changes
//! state. The machine halts when it enters an accepting or rejecting state,
//! or when no move applies (which counts as rejecting).
//!
//! Machines here are deterministic: at most one move per state and symbol.
//!
//! Unlike every other machine in this library, a Turing machine may run
//! forever, and by the halting problem no program can tell in general whether
//! it will. So running one takes a step limit, and running out of steps is
//! its own outcome, distinct from accepting or rejecting.

use super::{fmt_alphabet, render_table, State};
use std::collections::BTreeSet;
use std::fmt;

/// The blank symbol that fills every cell not holding input.
pub const BLANK: char = '_';
/// How many steps `accepts` and `run` take before giving up.
pub const STEP_LIMIT: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dir {
    Left,
    Right,
    Stay,
}

impl Dir {
    pub fn letter(self) -> char {
        match self {
            Dir::Left => 'L',
            Dir::Right => 'R',
            Dir::Stay => 'S',
        }
    }
}

/// One move: in state `from` reading `read`, write `write`, move, go to `to`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Move {
    pub from: State,
    pub read: char,
    pub write: char,
    pub dir: Dir,
    pub to: State,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tm {
    /// The input alphabet. Sorted, without duplicates, never containing `_`.
    pub alphabet: Vec<char>,
    pub states: Vec<String>,
    pub start: State,
    pub accept: BTreeSet<State>,
    pub reject: BTreeSet<State>,
    pub moves: Vec<Move>,
}

/// The tape: cells from `offset` (which may be negative) rightwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tape {
    cells: Vec<char>,
    /// The position of `cells[0]`.
    offset: i64,
}

impl Tape {
    fn new(input: &str) -> Tape {
        Tape { cells: input.chars().collect(), offset: 0 }
    }

    fn get(&self, at: i64) -> char {
        usize::try_from(at - self.offset).ok().and_then(|i| self.cells.get(i).copied()).unwrap_or(BLANK)
    }

    fn set(&mut self, at: i64, symbol: char) {
        while at < self.offset {
            self.cells.insert(0, BLANK);
            self.offset -= 1;
        }
        let i = (at - self.offset) as usize;
        if i >= self.cells.len() {
            self.cells.resize(i + 1, BLANK);
        }
        self.cells[i] = symbol;
    }

    /// Positions of the first and last non-blank cells, if any.
    fn used(&self) -> Option<(i64, i64)> {
        let first = self.cells.iter().position(|&c| c != BLANK)?;
        let last = self.cells.iter().rposition(|&c| c != BLANK)?;
        Some((self.offset + first as i64, self.offset + last as i64))
    }

    /// The tape's contents without the blanks at either end: what `run`
    /// returns.
    pub fn contents(&self) -> String {
        match self.used() {
            Some((first, last)) => (first..=last).map(|i| self.get(i)).collect(),
            None => String::new(),
        }
    }

    /// The used part of the tape with the head's cell in brackets:
    /// `10[1]1`. The window always includes the head.
    pub fn show(&self, head: i64) -> String {
        let (first, last) = match self.used() {
            Some((first, last)) => (first.min(head), last.max(head)),
            None => (head, head),
        };
        (first..=last)
            .map(|i| if i == head { format!("[{}]", self.get(i)) } else { self.get(i).to_string() })
            .collect()
    }
}

/// A snapshot of a run: the state, the tape and the head position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub state: State,
    pub tape: Tape,
    pub head: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Halt {
    /// Entered an accepting state.
    Accepted,
    /// Entered a rejecting state.
    Rejected,
    /// No move for this state and symbol, which also rejects.
    Stuck { symbol: char },
    /// Still running after the step limit.
    GaveUp,
    /// A symbol outside the alphabet (or the blank) in the input, at this
    /// position counting from 1. The machine never started.
    Foreign { position: usize, symbol: char },
}

/// A finished (or abandoned) run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// Every configuration from the start, if the run was recorded;
    /// otherwise only the last one.
    pub configs: Vec<Config>,
    /// The move taken before each configuration after the first, if recorded.
    pub moves: Vec<usize>,
    pub halt: Halt,
    /// How many steps were taken.
    pub steps: usize,
}

impl Run {
    pub fn last(&self) -> &Config {
        self.configs.last().expect("a run has at least its start")
    }
}

impl Tm {
    /// The move for `state` reading `symbol`, if there is one.
    pub fn move_for(&self, state: State, symbol: char) -> Option<usize> {
        self.moves.iter().position(|m| m.from == state && m.read == symbol)
    }

    /// The label of a move, as written in source: `0/1,R`.
    pub fn label(&self, m: &Move) -> String {
        format!("{}/{},{}", m.read, m.write, m.dir.letter())
    }

    /// Runs the machine on `input` for at most `limit` steps. With `record`,
    /// every configuration is kept (for `trace`); otherwise only the last.
    pub fn run(&self, input: &str, limit: usize, record: bool) -> Run {
        let mut config = Config { state: self.start, tape: Tape::new(input), head: 0 };
        if let Some(position) = input.chars().position(|c| !self.alphabet.contains(&c)) {
            let symbol = input.chars().nth(position).unwrap_or(BLANK);
            let halt = Halt::Foreign { position: position + 1, symbol };
            return Run { configs: vec![config], moves: Vec::new(), halt, steps: 0 };
        }
        let mut configs = vec![config.clone()];
        let mut moves = Vec::new();
        let mut steps = 0;
        let halt = loop {
            if self.accept.contains(&config.state) {
                break Halt::Accepted;
            }
            if self.reject.contains(&config.state) {
                break Halt::Rejected;
            }
            let symbol = config.tape.get(config.head);
            let Some(m) = self.move_for(config.state, symbol) else {
                break Halt::Stuck { symbol };
            };
            if steps == limit {
                break Halt::GaveUp;
            }
            let mv = &self.moves[m];
            config.tape.set(config.head, mv.write);
            config.head += match mv.dir {
                Dir::Left => -1,
                Dir::Right => 1,
                Dir::Stay => 0,
            };
            config.state = mv.to;
            steps += 1;
            if record {
                configs.push(config.clone());
                moves.push(m);
            }
        };
        if !record {
            configs = vec![config];
        }
        Run { configs, moves, halt, steps }
    }
}

impl fmt::Display for Tm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let n = self.states.len();
        let names = |set: &BTreeSet<State>| {
            let names: Vec<&str> = set.iter().map(|&s| self.states[s].as_str()).collect();
            if names.is_empty() { "nothing".to_string() } else { names.join(", ") }
        };
        writeln!(
            f,
            "TM with {} state{} over {}, blank {BLANK}",
            n,
            if n == 1 { "" } else { "s" },
            fmt_alphabet(&self.alphabet)
        )?;
        write!(
            f,
            "start {}, accept {}, reject {}",
            self.states[self.start],
            names(&self.accept),
            names(&self.reject)
        )?;
        if self.moves.is_empty() {
            return Ok(());
        }
        let mut rows = vec![["From", "Read", "Write", "Move", "To"].map(String::from).to_vec()];
        for m in &self.moves {
            rows.push(vec![
                self.states[m.from].clone(),
                m.read.to_string(),
                m.write.to_string(),
                m.dir.letter().to_string(),
                self.states[m.to].clone(),
            ]);
        }
        write!(f, "\n{}", render_table(&rows))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn tm(states: &[&str], accept: &[usize], reject: &[usize], moves: &[(usize, char, char, Dir, usize)]) -> Tm {
        Tm {
            alphabet: vec!['0', '1'],
            states: states.iter().map(|s| s.to_string()).collect(),
            start: 0,
            accept: accept.iter().copied().collect(),
            reject: reject.iter().copied().collect(),
            moves: moves.iter().map(|&(from, read, write, dir, to)| Move { from, read, write, dir, to }).collect(),
        }
    }

    /// Adds one to a binary number: go to the right end, then carry leftwards.
    pub fn increment() -> Tm {
        use Dir::*;
        tm(
            &["right", "carry", "done"],
            &[2],
            &[],
            &[
                (0, '0', '0', Right, 0),
                (0, '1', '1', Right, 0),
                (0, BLANK, BLANK, Left, 1),
                (1, '1', '0', Left, 1),
                (1, '0', '1', Stay, 2),
                (1, BLANK, '1', Stay, 2),
            ],
        )
    }

    #[test]
    fn binary_increment() {
        let m = increment();
        for (input, output) in [("0", "1"), ("1", "10"), ("1011", "1100"), ("111", "1000"), ("", "1")] {
            let run = m.run(input, STEP_LIMIT, false);
            assert_eq!(run.halt, Halt::Accepted, "{input}");
            assert_eq!(run.last().tape.contents(), output, "{input}");
        }
    }

    #[test]
    fn records_a_trace() {
        let run = increment().run("1", STEP_LIMIT, true);
        let shown: Vec<String> = run.configs.iter().map(|c| c.tape.show(c.head)).collect();
        assert_eq!(shown, ["[1]", "1[_]", "[1]", "[_]0", "[1]0"]);
        assert_eq!(run.moves, [1, 2, 3, 5]);
        assert_eq!(run.steps, 4);
    }

    #[test]
    fn halts_by_rejecting_or_getting_stuck() {
        use Dir::*;
        // Accepts words starting with 1; rejects those starting with 0;
        // has no move on a blank.
        let m = tm(&["s", "yes", "no"], &[1], &[2], &[(0, '1', '1', Stay, 1), (0, '0', '0', Stay, 2)]);
        assert_eq!(m.run("10", 100, false).halt, Halt::Accepted);
        assert_eq!(m.run("01", 100, false).halt, Halt::Rejected);
        assert_eq!(m.run("", 100, false).halt, Halt::Stuck { symbol: BLANK });
        assert_eq!(m.run("2", 100, false).halt, Halt::Foreign { position: 1, symbol: '2' });
    }

    #[test]
    fn gives_up_on_a_machine_that_never_halts() {
        let m = tm(&["s"], &[], &[], &[(0, BLANK, BLANK, Dir::Right, 0)]);
        let run = m.run("", 1000, false);
        assert_eq!(run.halt, Halt::GaveUp);
        assert_eq!(run.steps, 1000);
    }

    #[test]
    fn tape_grows_to_the_left() {
        let mut tape = Tape::new("1");
        tape.set(-2, 'x');
        assert_eq!(tape.contents(), "x_1");
        assert_eq!(tape.show(-3), "[_]x_1");
    }
}

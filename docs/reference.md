# Kleene Star reference

Everything in the language on one page. For a gentler introduction, start with
[the tutorial](tutorial/01-dfa.md).

## Program structure

A program is a list of statements, run from top to bottom. There are no
semicolons, and line breaks and indentation are not significant. `#` starts a
comment that runs to the end of the line.

## Statements

| Statement | Meaning |
|---|---|
| `alphabet B = {0, 1}` | Defines an alphabet. Symbols are single letters or digits. |
| `dfa Name over B { ... }` | Defines a DFA. See [machines](#machines). |
| `nfa Name over B { ... }` | Defines an NFA, with or without eps moves. |
| `pda Name over B { ... }` | Defines a pushdown automaton. See [pushdown automata](#pushdown-automata). |
| `tm Name over B { ... }` | Defines a Turing machine. See [Turing machines](#turing-machines). |
| `grammar Name over B { ... }` | Defines a context-free grammar. See [grammars](#grammars). |
| `let x = value` | Names a value. |
| `fn name(a: T, b: U) -> R { ... }` | Defines a function. Top level only. |
| `return value` | Returns from a function. |
| `if cond { ... } else { ... }` | Chooses. `else` is optional; `else if` chains. |
| `for x in items { ... }` | Repeats for each item of a set, range, string or alphabet. |
| `print a, b, c` | Prints the values on one line, separated by spaces. |
| `test cond` | Checks a condition. Failures are reported and counted. |
| `draw machine` | Writes a Graphviz `.dot` file (and `.svg` if Graphviz is installed). |
| `draw machine on word` | Writes an `.html` page that plays the run of the machine on the word step by step (see [Playing a run](#playing-a-run)). |
| `trace machine on word` | Runs a `Dfa`, `Nfa`, `Pda` or `Tm` on a `Str` and prints every step. See [tracing](#tracing). |
| `import "file.kstar" as m` | Binds another file as a module; its names are written `m.Name`. See [modules](#modules). |
| `from "file.kstar" import a, b as c` | Puts names of another file into this one. |

### Names and scope

- A name cannot be redefined in the same block. Values never change.
- A block `{ ... }` may reuse a name from outside; the inner one hides the
  outer one until the block ends.
- A function sees its parameters and the top-level names defined above it. It
  does not see the local names of whoever calls it.
- Functions may call each other in any order, but a function must be defined
  above the top-level line that first calls it.

```kstar
let limit = 3

fn below_limit(n: Int) -> Bool {
    return n < limit
}

if below_limit(2) {
    let limit = 10          # a new, inner `limit`
    print limit
}
print limit
```

```text
10
3
```

## Machines

```text
dfa Name over Alphabet {
    start  state
    accept state, state, ...
    state --symbol, symbol, ...--> state
    for i in items { ... }
}
```

- Exactly one `start`. Any number of `accept` lines; none means the machine
  accepts nothing.
- A label is a symbol of the alphabet, or `eps` (NFA only) for a move that
  reads nothing.
- A state is a name, or `name[expr]` for a numbered state: `q[2]` is `q2`.
- `for` inside a machine repeats the items in its braces.
- Without a name, a machine is a value: `let m = nfa over B { ... }`. The same
  goes for `pda` and `grammar`.

Rules checked for a `dfa`:

- every state has a transition on every symbol of the alphabet,
- no state has two different transitions on the same symbol,
- no eps moves.

An `nfa` has none of these restrictions. Machines written out in full are
checked before the program runs; machines built with `for` or `name[expr]` are
checked when they are built.

## Pushdown automata

```text
pda Name over Alphabet {
    start  state
    accept state, ...
    state --read, pop/push--> state
}
```

- `read` is an input symbol or `eps`; `pop` is one stack symbol or `eps`;
  `push` is a string of stack symbols written together, top first (`AZ`
  leaves `A` on top), or `eps`.
- Stack symbols are letters and digits; they need not be declared. The stack
  starts holding one `Z`.
- Acceptance is **by final state**: a word is accepted if some run reads all
  of it and ends in an accepting state, whatever is left on the stack.
- A PDA may be nondeterministic. Runs are searched breadth first; the search
  gives up after 50 000 configurations, or when the stack would pass 1 000
  symbols, with a `LimitError` (an unknown answer is never reported as
  "no").
- `for` loops and `name[expr]` states work as in other machines.

## Turing machines

```text
tm Name over Alphabet {
    start  state
    accept state, ...
    reject state, ...
    state --read/write,move--> state
}
```

- `read` and `write` are tape symbols: letters, digits, or `_` for the
  blank. Symbols the machine writes need not be in the alphabet; the alphabet
  says only what the input may contain, and it cannot contain `_`.
- `move` is `L` (left), `R` (right) or `S` (stay).
- The input starts on the tape with the head on its first symbol.
- The machine halts on entering an `accept` or `reject` state, or when no
  move applies (which rejects). A state cannot both accept and reject.
- At most one move per state and symbol: machines are deterministic.
- `accepts` and `run` stop after 100 000 steps with a `LimitError`, since a
  Turing machine may never halt (the halting problem); `trace` shows at most
  100 steps and then reports `UNKNOWN`.

## Grammars

```text
grammar Name over Alphabet {
    Variable -> alternative | alternative | ...
    ...
}
```

- A **variable** is a name starting with a capital letter. A **terminal** is
  one symbol of the alphabet. Symbols in an alternative are separated by
  spaces: `a S b`. `eps` alone is the empty alternative.
- The variable of the first rule is the start variable. A variable may have
  several rules; they add up.
- Checked before running: every variable used has rules, and every terminal
  is in the alphabet.
- A capital letter is always a variable in a grammar, even if the alphabet
  contains it.

## Tracing

`trace M on "0110"` runs `M` (a `Dfa`, `Nfa`, `Pda` or `Tm`) on the word and prints a
table with one row per symbol, then the result and the reason for it:

```kstar
alphabet B = {0, 1}
dfa Ones over B {
    start  even
    accept odd
    even --0--> even
    even --1--> odd
    odd  --0--> odd
    odd  --1--> even
}
trace Ones on "011"
```

```text
Input: 011

Step  Symbol  From  To
0     —       —     even
1     0       even  even
2     1       even  odd
3     1       odd   even
Result: REJECTED
Reason: Ended in non-accepting state even
```

- Step 0 is the start, before anything is read; its empty cells show `—`
  (or `-` on a Windows console whose code page cannot show `—`).
- For an `Nfa`, `From` and `To` are sets of states, written `{a,b}`, and
  already include the states reachable by eps moves. The reason names an
  accepting state in the final set, or says there is none.
- A symbol outside the alphabet ends the trace with `Result: REJECTED` and a
  reason naming the symbol and its position. It is not an error: the program
  carries on.
- For a `Pda` the columns are `Step  Move  State  Input  Stack`: the move
  taken (`a, Z/AZ`), the state after it, the input still to read, and the
  stack, top first. An accepted word shows a shortest accepting run; a
  rejected one shows a run that read as much as possible. If the search gives
  up, the result is `UNKNOWN`.
- For a `Tm` the columns are `Step  Move  State  Tape`, with the head's cell
  in brackets (`10[1]1`), and a last line `Tape:` with the final contents.
- A blank line follows each trace.

## Playing a run

`draw M on "0110"` writes `M.html` (into the folder `kstar run --out`
names, or the current one): a page that plays the run of `M` on the word
step by step, like an animated `trace`. Open it in any browser.

- It shows the machine's diagram, with the states the machine is in and
  the move it just took highlighted, the input with the symbols read so
  far, a PDA's stack (top first) or a Turing machine's tape with its head,
  and at the end the result and the reason, in the same words as `trace`.
- Play and pause with the button or Space, step with the arrow buttons or
  keys, jump with the slider or Home and End, and pick a speed.
- `M` may be a `Dfa`, `Nfa`, `Pda` or `Tm`, as for `trace`. An `Nfa` shows
  the whole set of states it can be in. A Turing machine is played for at
  most 1000 steps.
- The page is one file with nothing to download: no Graphviz, no network.
  The states are laid out in columns by their distance from the start
  state. It follows the browser's light or dark theme.
- The layout is for machines of at most 300 states; past that it is a
  `LimitError` (E0612). `minimize` the machine, or use `trace`.
- `std.visual.player(M, "w")` gives the same page as a string.

## Modules

A program can be split over several files. Any `.kstar` file can be
imported by another; it is then called a **module**.

| Form | Binds |
|---|---|
| `import "lib/parity.kstar"` | the module as `parity` (the file name without `.kstar`) |
| `import "lib/parity.kstar" as p` | the module as `p` |
| `import std.math`, `import std.math as m` | a standard-library module as `math`, or `m` |
| `from "parity.kstar" import EvenZeros, ends_in as ending` | `EvenZeros` and `ending` |
| `from std.math import gcd` | `gcd` |

- `import` and `from` are written at the top level of a file. The path is
  in quotes, relative to the folder of the importing file, and ends in
  `.kstar`. Without `as`, the file name must be a valid name.
- A module's names are its top-level alphabets, machines, grammars, `let`
  values and functions. All of them are public. Names it imported itself
  are not passed on: `a.b.X`, where `b` is a module `a` imports, is an
  error (E0210). Import `b` directly.
- A **qualified name** `m.Name` works wherever a name does: in expressions
  and calls, after `over`, and in `trace` and `draw`. It is written without
  spaces around the dot.
- An imported module, a name from `from`, and every other top-level name of
  the file must all be different. `std` always means the standard library,
  so nothing can be called `std`.
- Every imported file is read, parsed and checked before the program runs,
  so its errors are reported first, each in its own file, even if none of
  its names is used.
- **Quiet:** when a file is loaded as a module, its top-level `print`,
  `test`, `draw` and `trace` are checked but not run, and its tests are not
  counted. Its functions behave normally when called.
- **Lazy:** a module's top level runs the first time one of its names is
  used, and a run-time error there shows that line in the traceback, then
  the module's line `in <module>`. `kstar run --eager-imports` runs each
  module at its `import` instead.
- A module runs at most once per program, however many files import it, and
  keeps its own globals: a function sees the names of the file it is written
  in.
- Files that import each other in a circle are an error showing the chain:
  `a.kstar -> b.kstar -> a.kstar`.
- An import that is never used is a warning (W0202).

## The standard library

The standard library comes with Kleene Star. A module of it is reached by
its full name with no import (`std.math.gcd(12, 18)`), or imported like a
file: `import std.math` (bound as `math`), `import std.math as m`, or
`from std.math import gcd`. The name `std` always means the standard
library. Hovering over a library function in an editor shows the same
descriptions as below.

### `std.automata`

Ready-made DFAs for common patterns (prefixes, suffixes, substrings, counting, parity, lengths), and questions about regular languages: a shortest word, a word that tells two languages apart, finiteness and counting words.

| Function | What it does |
|---|---|
| `starts_with(alphabet: Alphabet, prefix: Str) -> Dfa` | The smallest DFA for the strings over `alphabet` that begin with `prefix`. |
| `ends_with(alphabet: Alphabet, suffix: Str) -> Dfa` | The smallest DFA for the strings over `alphabet` that end with `suffix`. |
| `contains_substring(alphabet: Alphabet, part: Str) -> Dfa` | The smallest DFA for the strings over `alphabet` that contain `part` somewhere. |
| `exactly(alphabet: Alphabet, symbol: Str, n: Int) -> Dfa` | The smallest DFA for the strings with exactly `n` copies of `symbol`. |
| `at_least(alphabet: Alphabet, symbol: Str, n: Int) -> Dfa` | The smallest DFA for the strings with at least `n` copies of `symbol`. |
| `at_most(alphabet: Alphabet, symbol: Str, n: Int) -> Dfa` | The smallest DFA for the strings with at most `n` copies of `symbol`. |
| `count_mod(alphabet: Alphabet, symbol: Str, m: Int, r: Int) -> Dfa` | The smallest DFA for the strings whose number of `symbol`s leaves remainder `r` when divided by `m`; `m = 2` gives parity. |
| `length_mod(alphabet: Alphabet, m: Int, r: Int) -> Dfa` | The smallest DFA for the strings whose length leaves remainder `r` when divided by `m`. |
| `length_between(alphabet: Alphabet, min: Int, max: Int) -> Dfa` | The smallest DFA for the strings of length `min` to `max`, both included. |
| `shortest_word(lang: Lang) -> Str` | A shortest word of the language (the first in alphabet order if there are several); the language must not be empty. |
| `distinguishing_word(a: Lang, b: Lang) -> Str` | A shortest word in one language but not the other, showing why they are not `equivalent`; they must differ. |
| `is_finite(lang: Lang) -> Bool` | Whether the language has only finitely many words. |
| `count_words(lang: Lang, n: Int) -> Int` | How many words of length exactly `n` the language has. |
| `words_up_to(lang: Lang, n: Int) -> Set[Str]` | The words of the language of length at most `n`, as a set; there may be at most 100000 of them. |

**Pattern machines.** The first nine functions build a `Dfa` over the
alphabet they are given. It is already minimal, so `minimize` leaves it the
same size, and its states are named after what they remember:

- `starts_with`: `p0`, `p1`, ... for how much of the prefix has been read,
  and `dead` after a wrong symbol;
- `ends_with`, `contains_substring`: `m0`, `m1`, ... for the longest end of
  the input that begins the pattern (the Knuth-Morris-Pratt automaton);
- `exactly`, `at_least`, `at_most`: `c0`, `c1`, ... for how many copies so
  far, and `more` for more than `n`;
- `count_mod`, `length_mod`: `r0`, `r1`, ... for the remainder so far;
- `length_between`: `l0`, `l1`, ... for the length so far, and `long` for
  longer than `max`.

A state that cannot be reached is left out: over the alphabet `{a}`,
`starts_with(A, "aa")` has no `dead` state. The result is an ordinary
machine, so it combines with the built-ins:
`intersect(std.automata.ends_with(B, "0"), std.automata.count_mod(B, "1", 2, 0))`
is "ends in 0 and has an even number of 1s".

A `symbol` is a string of one symbol of the alphabet (E0614 if it is longer
or empty, E0500 if the alphabet does not have it). Every symbol of a pattern
must be in the alphabet (E0501). Counts and lengths cannot be negative
(E0610); a modulus `m` must be at least 1 and the remainder `r` from 0 to
`m - 1`, and `min` must not be larger than `max` (E0609). A machine needing
more than 10000 states is a `LimitError` (E0612).

**Questions.** In the signatures, `Lang` means any regular language: a
`Dfa`, `Nfa`, `Regex` or `Set[Str]`, the types the built-ins on languages
take. The alphabet is chosen the same way as for the built-ins (see
[alphabets in operations](#alphabets-in-operations)), so the two languages
of `distinguishing_word` must share it. A grammar, PDA or Turing machine is
refused when the program is checked. The questions extend the built-ins:

- `equivalent(a, b)` says whether two languages differ;
  `distinguishing_word(a, b)` gives the shortest word that shows it, and is
  a `ValueError` (E0615) if they do not differ.
- `is_empty(lang)` says whether there is a word; `shortest_word(lang)` gives
  one, and is E0615 for the empty language.
- `is_finite` uses the idea behind the pumping lemma: a language is infinite
  exactly when its DFA has a loop on the way to an accepting state.
- `count_words(lang, n)` counts the words of one length without listing
  them, for lengths up to 10000 (a `MathError`, E0600, past the largest
  Int); `words_up_to(lang, n)` lists them, and is E0612 when they are too
  many.

```kstar
alphabet B = {0, 1}
let EndsIn01 = std.automata.ends_with(B, "01")
print EndsIn01
let EvenOnes = std.automata.count_mod(B, "1", 2, 0)
print states(EvenOnes), EvenOnes accepts "0110", EvenOnes accepts "010"
let Both = intersect(EndsIn01, EvenOnes)
print std.automata.shortest_word(Both)
print std.automata.distinguishing_word(EndsIn01, /(0|1)*1/)
print std.automata.is_finite(std.automata.length_between(B, 2, 3)), std.automata.is_finite(EvenOnes)
print std.automata.count_words(EvenOnes, 4), std.automata.words_up_to(Both, 4)
```

```text
DFA with 3 states over {0, 1}
       | 0  | 1
->  m0 | m1 | m0
    m1 | m1 | m2
   *m2 | m1 | m0
2 true false
101
1
true false
8 {"0101", "1001", "101"}
```

### `std.benchmark`

How much work the algorithms do: counts that are the same on every computer, and times in microseconds.

| Function | What it does |
|---|---|
| `equivalence_rounds(m: Lang) -> Int` | The rounds of refinement minimising `m`'s DFA takes: the smallest k for which k-equivalence equals (k+1)-equivalence. |
| `pda_configs(p: Pda, w: Str) -> Int` | How many configurations the search for an accepting run of `p` on `w` looks at. |
| `regex_size(m: Lang) -> Int` | The number of symbols and operators in the regex state elimination builds for `m`, as `to_regex` gives it. |
| `time_accepts_us(m: Acceptor, w: Str) -> Int` | Microseconds to decide whether `m` accepts `w`, the median of 5 runs; 0 with a fixed clock. |
| `time_us(op: Str, m: Lang) -> Int` | Microseconds the operation `op` (`to_dfa`, `minimize`, `to_regex`, `remove_eps`, `complement` or `reverse`) takes on `m`, the median of 5 runs; 0 with a fixed clock. |

**Counts and times.** The counts are the same on every computer and every
run, so they can go in a `test` or a table of how the work grows: with
`states`, they show the subset construction doubling the states of "the
k-th symbol from the end is 1" for each k, and state elimination growing
faster still. Times depend on the computer and change from run to run;
each is the median of 5 runs of the operation alone, after its input has
been turned into the machine the operation works on (an NFA for `to_dfa`,
`remove_eps` and `reverse`, a DFA for the others).

**Output that does not change.** `kstar run --fixed-clock` makes every
time 0, so a program's output is the same on every run; Kleene Star's own
tests, and the samples on this page, always run that way.

- `equivalence_rounds` counts as in the usual textbook working:
  0-equivalence splits accepting from non-accepting states, each round
  splits the groups by where their states go, and the answer is the first
  `k` whose split is the same as the next one's.
- `regex_size` counts what `to_regex` builds, and like it stops past
  100000 parts (E0608).
- `pda_configs` counts the configurations (state, input read, stack) the
  breadth-first search for an accepting run finds.
- `time_us` with any other operation is E0621.

```kstar
alphabet B = {0, 1}
let N = nfa over B {
    start  a
    accept c
    a --0,1--> a
    a --1--> b
    b --0,1--> c
}
let D = to_dfa(N)
print states(N), states(D), std.benchmark.equivalence_rounds(D)
print std.benchmark.time_us("to_dfa", N)
```

```text
3 4 1
0
```

### `std.binary`

Binary numbers and bit strings over {0, 1}: divisibility and remainders, numerals without leading zeros, fixed widths, parity, runs of equal bits, and converting between Ints and bit strings.

| Function | What it does |
|---|---|
| `divisible_by(n: Int) -> Dfa` | The bit strings whose value is a multiple of `n`, as the machine with one state per remainder. |
| `remainder(n: Int, r: Int) -> Dfa` | The bit strings whose value leaves remainder `r` when divided by `n`. |
| `no_leading_zeros() -> Dfa` | Numerals written without leading zeros: `0`, or a `1` followed by any bits. |
| `fixed_width(n: Int) -> Dfa` | The bit strings of exactly `n` bits. |
| `even_parity() -> Dfa` | The bit strings with an even number of 1s. |
| `odd_parity() -> Dfa` | The bit strings with an odd number of 1s. |
| `run_at_most(bit: Str, k: Int) -> Dfa` | The bit strings in which `bit` never appears more than `k` times in a row. |
| `to_binary(n: Int) -> Str` | `n` written in binary without leading zeros; `n` must not be negative, and 0 is `"0"`. |
| `from_binary(bits: Str) -> Int` | The value of a bit string, most significant bit first; leading zeros do not count and `""` is 0. |

Every machine here is over the alphabet `{0, 1}`, so none of the functions
takes an alphabet; combine them with machines `over` an alphabet `{0, 1}`.

**How a bit string is a number.** It is read most significant bit first,
the way numbers are written: `110` is 6. Leading zeros do not change the
value, so `0110` is 6 too, and the empty string counts as 0. That is the
reading under which the machines are correct, and it means
`divisible_by(3)` accepts `""`, `"0"` and `"0011"`. To accept only
numerals written the usual way, intersect with `no_leading_zeros()`:
`intersect(std.binary.divisible_by(3), std.binary.no_leading_zeros())`.

**The remainder machine.** `divisible_by(n)` and `remainder(n, r)` have
one state per remainder, `r0` to `r{n-1}`. Reading a bit `b` after a
number `x` makes the number `2x + b`, so the remainder `i` becomes
`(2i + b) mod n`: the machine only ever needs to remember the remainder.
This is the machine a textbook proof builds, and it is minimal when `n` is
odd; for an even `n`, `minimize` makes it smaller. `run_at_most` names its
states `run0`, `run1`, ... for the length of the current run, and `dead`.

**Relation to other modules.** `fixed_width(n)` is
`std.automata.length_between(B, n, n)`, and the parity machines are
`std.automata.count_mod(B, "1", 2, 0)` and `(..., 1)`, over `{0, 1}`.

`to_binary` and `from_binary` connect machines to arithmetic, which makes
proof-style checks short: `for n in 0..64 { test (D accepts to_binary(n)) == (n % 3 == 0) }`.
A divisor below 1 or a remainder outside 0 to `n - 1` is a `MathError`
(E0609), as is a negative number for `to_binary`; a negative width or run
length is E0610; `bit` must be `"0"` or `"1"` (E0614, E0500); a machine of
more than 10000 states is E0612. `from_binary` refuses anything but 0s and
1s (E0611) and values past the largest Int (E0600).

```kstar
alphabet B = {0, 1}
let Div3 = std.binary.divisible_by(3)
print Div3
for n in 0..64 {
    test (Div3 accepts std.binary.to_binary(n)) == (n % 3 == 0)
}
print Div3 accepts "", Div3 accepts "0110", std.binary.from_binary("0110")
let Numerals = intersect(Div3, std.binary.no_leading_zeros())
print Numerals accepts "0110", Numerals accepts "110"
print states(std.binary.divisible_by(4)), states(minimize(std.binary.divisible_by(4)))
test equivalent(std.binary.divisible_by(6), intersect(std.binary.divisible_by(2), std.binary.divisible_by(3)))
print std.automata.count_words(std.binary.run_at_most("1", 1), 10)
```

```text
DFA with 3 states over {0, 1}
       | 0  | 1
-> *r0 | r0 | r1
    r1 | r2 | r0
    r2 | r1 | r2
true true 6
false true
4 3
144
tests: 65 passed, 0 failed
```

### `std.collections`

Sets: membership, subsets, union, intersection, difference, size, smallest, largest and sum, and every word of a given length over an alphabet.

| Function | What it does |
|---|---|
| `contains(s: Set[T], x: T) -> Bool` | Whether `x` is a member of the set `s`. |
| `difference(a: Set[T], b: Set[T]) -> Set[T]` | The members of the set `a` that are not in `b`. |
| `intersect(a: Set[T], b: Set[T]) -> Set[T]` | The members that the sets `a` and `b` have in common. |
| `is_subset(a: Set[T], b: Set[T]) -> Bool` | Whether every member of the set `a` is also in `b`; the empty set is a subset of every set. |
| `max(s: Set[Int]) -> Int` | The largest member of a set of Ints; the set must not be empty. |
| `min(s: Set[Int]) -> Int` | The smallest member of a set of Ints; the set must not be empty. |
| `size(s: Set[T]) -> Int` | The number of members of the set `s`; the same as `len(s)`. |
| `sum(s: Set[Int]) -> Int` | The members of a set of Ints added up; the sum of the empty set is 0. |
| `union(a: Set[T], b: Set[T]) -> Set[T]` | The members that are in the set `a`, in `b`, or in both. |
| `words(alphabet: Alphabet, n: Int) -> Set[Str]` | Every string of length `n` over `alphabet`, as a set; `n` must not be negative, and there may be at most 100000 of them. |

A set holds `Int`, `Str` or `Bool` values. In the signatures, `T` stands for
that element type: `contains(s, x)` works for a set of Ints with an Int and
for a set of Strs with a Str, and `union(a, b)` needs both sets to hold the
same kind of value. `{}` fits any `Set[T]`. `min`, `max` and `sum` need a set
of Ints. `min` and `max` of the empty set are a `ValueError` (E0613) with a
hint, and a sum that does not fit in an Int is a `MathError` (E0600).

`words(alphabet, n)` lists all `size^n` strings of length `n`, so
`words(B, 3)` over `{0, 1}` has 8 words, and `words(B, 0)` is `{""}`. When
the result would have more than 100000 words, it is a `LimitError` (E0612)
whose message names that limit, instead of a program that never finishes;
a negative `n` is E0610.

**Names that also belong to the built-ins.** The built-ins `union`,
`intersect` and `difference` combine *languages* into a machine:
`union({"a"}, {"b"})` is a `Dfa`. The functions here are different: they
combine the *members of sets* and give a set back. Call them by their full
names (`std.collections.union(a, b)`) or import them under other names
(`from std.collections import union as set_union`). Taking one under its own
name, as in `from std.collections import union`, is an error (E0203), because
that name already means the built-in. `size` is the same as the built-in
`len` on a set.

**What is not here.** There is no power set and no set of pairs (a product
of two sets), because both are sets of sets or sets of tuples, and a set can
only hold `Int`, `Str` or `Bool` values.

```kstar
let a = {1, 2, 3, 4}
let b = {3, 4, 5}
print std.collections.union(a, b), std.collections.intersect(a, b)
print std.collections.difference(a, b), std.collections.is_subset({3, 4}, a)
print std.collections.size(a), std.collections.min(a), std.collections.max(a), std.collections.sum(a)
print std.collections.contains({"", "ab"}, "ab"), std.collections.contains(a, 9)
alphabet B = {0, 1}
print std.collections.words(B, 2), std.collections.size(std.collections.words(B, 10))
```

```text
{1, 2, 3, 4, 5} {3, 4}
{1, 2} true
4 1 4 10
true false
{"00", "01", "10", "11"} 1024
```

### `std.grammar`

Helpers for context-free grammars: the steps of the CNF construction one at a time, FIRST and FOLLOW sets, LL(1) and CNF checks, listing short words, and ready-made grammars.

| Function | What it does |
|---|---|
| `nullable(g: Grammar) -> Set[Str]` | The variables that can derive the empty word. |
| `generating(g: Grammar) -> Set[Str]` | The variables that can derive some word of terminals. |
| `reachable(g: Grammar) -> Set[Str]` | The variables that can appear in a derivation from the start variable. |
| `remove_useless(g: Grammar) -> Grammar` | The same grammar without the variables that generate nothing or cannot be reached. |
| `remove_eps(g: Grammar) -> Grammar` | An equivalent grammar whose only eps rule, if any, is for a start variable that is on no right-hand side. |
| `remove_units(g: Grammar) -> Grammar` | An equivalent grammar without unit rules such as `A -> B`. |
| `is_cnf(g: Grammar) -> Bool` | Whether the grammar is in Chomsky normal form, as `to_cnf` makes it. |
| `first(g: Grammar, variable: Str) -> Set[Str]` | The terminals that can begin a word `variable` derives, and `""` if it can derive the empty word. |
| `follow(g: Grammar, variable: Str) -> Set[Str]` | The terminals that can come right after `variable`, and `""` if it can come last. |
| `is_ll1(g: Grammar) -> Bool` | Whether a parser can always choose the rule to use by looking at the next symbol only. |
| `words_up_to(g: Grammar, n: Int) -> Set[Str]` | The words of the grammar's language of length at most `n`, as a set. |
| `balanced(open: Str, close: Str) -> Grammar` | Balanced brackets: `S -> open S close S` or eps. |
| `anbn(a: Str, b: Str) -> Grammar` | `n` copies of `a` then `n` copies of `b`: `S -> a S b` or eps. |
| `palindromes(alphabet: Alphabet) -> Grammar` | The words over `alphabet` that read the same backwards. |
| `equal_count(a: Str, b: Str) -> Grammar` | The words over `a` and `b` with as many of one as of the other. |

`grammar` is a keyword (it begins a grammar), so the module has no name of
its own: write `std.grammar.first(G, "S")`, `from std.grammar import first`,
or `import std.grammar as grammars`. Plain `import std.grammar` is an error
(E0115) that suggests the `as`. The same goes for `std.pda` and `std.tm`.

**Cleaning up.** The built-in `to_cnf` does all the steps of the Chomsky
normal form construction at once. These functions do one step each, so a
grammar can be shown after every step:

- `remove_useless` drops the variables that are not `generating` (derive no
  word of terminals) and then those not `reachable` from the start, with
  every rule that uses them. The language stays the same.
- `remove_eps` copies each rule once for every way of leaving out its
  `nullable` variables, then drops the eps rules. If the language has the
  empty word, the start variable keeps `S -> eps`; when the start variable
  also appears on a right-hand side, a new start `S0 -> S | eps` comes
  first, so the language stays the same. A rule with k nullable variables
  gives up to 2^k copies; more than 100000 rules in all is a `LimitError`
  (E0612).
- `remove_units` gives each variable the rules of every variable it reaches
  through unit rules, and drops the unit rules.

The last two also drop variables that can no longer be reached. `is_cnf`
says whether every rule is `A -> B C` or `A -> a`, apart from `S -> eps` for
a start variable that is on no right-hand side. The result of `to_cnf` always
passes.

**Parsing questions.** `first` and `follow` are the sets a predictive parser
is built from. A terminal is a one-symbol string; `""` in `first(g, "A")`
means A can derive the empty word, and in `follow(g, "A")` that A can come
at the end of the input (many books write `$`). A name that is not a variable
of the grammar is a `ValueError` (E0617). `is_ll1` checks the rules of every
reachable variable two by two: their FIRST sets must not share a terminal,
at most one may derive the empty word, and if one does, the other's FIRST
must not meet the variable's FOLLOW. Ambiguous and left-recursive grammars
are never LL(1).

`words_up_to(g, n)` lists the words of length up to `n` (at most 10000),
building them from the CNF copy, shortest first. There may be at most 100000
of them, and building them may take at most a million characters in all; a
larger listing is a `LimitError` (E0612). Combined with `std.testing`, it
checks a grammar against a description: `std.testing.accepts_all(G,
words_up_to(G, 6))` holds by definition, but the same set checked against a
machine you wrote by hand does not.

**Ready-made grammars.** Each has the one variable `S`:

- `balanced("l", "r")`: `S -> l S r S | eps`, the words in which each `l`
  (a left bracket) has a matching `r` (a right bracket) after it. This form,
  rather than `S -> S S | l S r | eps`, is unambiguous and LL(1).
- `anbn("a", "b")`: `S -> a S b | eps`.
- `palindromes(B)`: `S -> eps`, and `S -> c` and `S -> c S c` for each
  symbol c of `B`.
- `equal_count("a", "b")`: `S -> a S b S | b S a S | eps`.

A symbol is a string of one letter or digit, as in an alphabet (E0614), and the two symbols must differ
(an `AlphabetError`, E0503).

```kstar
alphabet AB = {a, b}
grammar G over AB {
    S -> A B | a C | a
    A -> B | eps
    B -> b | eps
    C -> a C
}
print std.grammar.nullable(G), std.grammar.generating(G), std.grammar.reachable(G)
print std.grammar.remove_useless(G)
print std.grammar.remove_eps(G)
print std.grammar.remove_units(std.grammar.remove_eps(G))
print std.grammar.is_cnf(G), std.grammar.is_cnf(to_cnf(G))
let Brackets = std.grammar.balanced("l", "r")
print std.grammar.first(Brackets, "S"), std.grammar.follow(Brackets, "S"), std.grammar.is_ll1(Brackets)
print std.grammar.words_up_to(Brackets, 4)
print std.grammar.words_up_to(std.grammar.equal_count("a", "b"), 2)
```

```text
{"A", "B", "S"} {"A", "B", "S"} {"A", "B", "C", "S"}
Grammar with 3 variables over {a, b}, start S
S -> A B | a
A -> B | eps
B -> b | eps
Grammar with 4 variables over {a, b}, start S
S -> A B | B | A | eps | a C | a
A -> B
B -> b
C -> a C
Grammar with 4 variables over {a, b}, start S
S -> A B | eps | a C | a | b
A -> b
B -> b
C -> a C
false true
{"", "l"} {"", "r"} true
{"", "llrr", "lr", "lrlr"}
{"", "ab", "ba"}
```

### `std.io`

Reading and writing text files in the program's folder, such as word lists and machines saved as JSON.

| Function | What it does |
|---|---|
| `append_text(path: Str, text: Str) -> Int` | Adds `text` to the end of the file at `path`, creating it if needed, and gives the number of bytes written. |
| `exists(path: Str) -> Bool` | Whether there is a file at `path`; a path outside the allowed folders is an error, not `false`. |
| `files(folder: Str) -> Set[Str]` | The paths of the files directly in `folder` (`"."` for the program's own), leaving out hidden ones. |
| `read_lines(path: Str) -> Set[Str]` | The lines of the file at `path` that are not empty, as a set. |
| `read_text(path: Str) -> Str` | The whole file at `path`, which must be UTF-8 text of at most 1 MiB; line ends become `\n`. |
| `read_words(path: Str) -> Set[Str]` | The words of the file at `path`, separated by spaces or new lines, as a set; a `#` starts a comment to the end of its line. |
| `write_lines(path: Str, lines: Set[Str]) -> Int` | Writes the members of `lines` to the file at `path`, one per line in the set's order, and gives the number of bytes written. |
| `write_text(path: Str, text: Str) -> Int` | Creates or replaces the file at `path` with `text`, and gives the number of bytes written. |

A program may only use files in **its own folder** and the folders under
it, so running a program someone sent you cannot read or change the rest
of your files. Paths are relative to that folder, wherever you run the
program from (in the REPL, to the current folder). To let a program use
another folder too, run it with `kstar run --allow-path FOLDER`; the flag
can be given several times. On every call:

- the path is resolved on the disk, following `..` and symbolic links,
  and must end up inside an allowed folder, or the call fails with E0618;
- no part of it below that folder may be hidden (start with `.`), so
  `.git` and `.env` are never read or written; names with `:` and Windows
  device names such as `CON` are refused too;
- writing creates or replaces only text files ending in `.txt`, `.json`,
  `.csv`, `.tsv`, `.md`, `.log`, `.dot`, `.svg` or `.html`, never a
  program or a script, and creates the folders on the way;
- files are UTF-8 text (a leading byte order mark is dropped, and Windows
  line ends become `\n`). A file read may be at most 1 MiB, one write at
  most 1 MiB, and one run may write at most 16 MiB in at most 1000 calls;
  past that is E0619.

A code block in these docs is not a file, so it cannot use `std.io`; see
`examples/22_io.kstar` for a program that reads a word list and saves a
machine. A machine goes through a file as JSON, with the built-ins:

```text
let saved = std.io.write_text("out/m.json", to_json(M, "M"))
let Again = from_json(Dfa, std.io.read_text("out/m.json"))
```

Every statement that is not a declaration needs a value, so the functions
that write give the number of bytes written: use them with `let`, `print`
or `test`.

### `std.visual`

Drawings and traces as strings: DOT diagrams, a run drawn on its diagram, the trace table, and the page that plays a run.

| Function | What it does |
|---|---|
| `dot(m: Acceptor) -> Str` | The Graphviz DOT text that `draw m` writes; a regex or set is drawn as its NFA. |
| `dot_run(m: Acceptor, w: Str) -> Str` | The DOT text of machine `m` with its run on `w` drawn on it: the states and moves used in bold, the states it ends in filled. |
| `player(m: Acceptor, w: Str) -> Str` | The self-contained HTML page that `draw m on w` writes, which plays the run step by step. |
| `trace_text(m: Acceptor, w: Str) -> Str` | The table `trace m on w` prints, as a string, with `-` in the empty cells. |

These give, as strings, what the statements `draw`, `draw M on "w"` and
`trace` write or print, so a program can print them, test them, or save
them with `std.io`. `m` may be a `Dfa`, `Nfa`, `Pda` or `Tm`; `dot` also
takes a regex or set (drawn as its NFA). A run needs states, so
`dot_run`, `trace_text` and `player` refuse a regex, a set or a grammar
with E0620; convert it first, for example `to_dfa(r)`.

```kstar
alphabet B = {0, 1}
dfa Ones over B {
    start  even
    accept odd
    even --0--> even
    even --1--> odd
    odd  --0--> odd
    odd  --1--> even
}
let run = std.visual.dot_run(Ones, "01")
print std.string.contains(run, "fillcolor"), std.string.length(std.visual.player(Ones, "01")) > 1000
print std.visual.trace_text(Ones, "1")
```

```text
true true
Input: 1

Step  Symbol  From  To
0     -       -     even
1     1       even  odd
Result: ACCEPTED
Reason: Ended in accepting state odd
```

### `std.math`

Whole-number arithmetic: division, powers, roots, divisors and primes.

| Function | What it does |
|---|---|
| `abs(n: Int) -> Int` | The size of `n`, without its sign. |
| `div(a: Int, b: Int) -> Int` | `a` divided by `b` as a whole number: the `q` with `a == b * q + a % b`. |
| `factorial(n: Int) -> Int` | `n! = 1 * 2 * ... * n`, for `n` from 0 to 20. |
| `gcd(a: Int, b: Int) -> Int` | The greatest common divisor of `a` and `b`; never negative, and `gcd(0, 0)` is 0. |
| `is_prime(n: Int) -> Bool` | Whether `n` is a prime number: greater than 1, with no divisors but 1 and itself. |
| `lcm(a: Int, b: Int) -> Int` | The least common multiple of `a` and `b`; never negative, and 0 if either is 0. |
| `log2(n: Int) -> Int` | The base-2 logarithm of `n`, rounded down; `n` must be positive. |
| `max(a: Int, b: Int) -> Int` | The larger of `a` and `b`. |
| `min(a: Int, b: Int) -> Int` | The smaller of `a` and `b`. |
| `pow(base: Int, exp: Int) -> Int` | `base` to the power `exp`; `exp` must not be negative. |
| `sqrt(n: Int) -> Int` | The square root of `n`, rounded down; `n` must not be negative. |

Every function works on `Int`s. A result too large for an Int is a
`MathError` (E0600), and an argument a function does not accept, such as a
negative number for `sqrt`, is a `MathError` with code E0609 and a hint.
`div` goes with `%`: since `a % b` is never negative, `div` rounds down
when `b` is positive (`div(-17, 5)` is -4, with remainder 3), and
`a == b * div(a, b) + a % b` holds for every `a` and every `b` that is not 0.

```kstar
print std.math.div(17, 5), 17 % 5
print std.math.div(-17, 5), -17 % 5
print std.math.gcd(84, 36), std.math.lcm(4, 6)
print std.math.pow(2, 10), std.math.sqrt(99), std.math.log2(1000)
print std.math.factorial(10), std.math.is_prime(1000000007)
print std.math.abs(-3), std.math.min(3, -2), std.math.max(3, -2)
```

```text
3 2
-4 3
12 12
1024 9 9
3628800 true
3 -2 3
```

### `std.pda`

Ready-made pushdown automata, the grammar of a PDA, whether a PDA is deterministic, and membership and word lists that never give up.

| Function | What it does |
|---|---|
| `anbn(a: Str, b: Str) -> Pda` | A deterministic PDA for `n` copies of `a` then `n` copies of `b`. |
| `balanced(open: Str, close: Str) -> Pda` | A deterministic PDA for balanced brackets, with `open` and `close` as the brackets. |
| `palindromes(alphabet: Alphabet) -> Pda` | A PDA for the words over `alphabet` that read the same backwards; it guesses where the middle is. |
| `equal_count(a: Str, b: Str) -> Pda` | A PDA for the words over `a` and `b` with as many of one as of the other. |
| `to_grammar(p: Pda) -> Grammar` | A grammar with the same language as the PDA, by the triple construction. |
| `is_deterministic(p: Pda) -> Bool` | Whether at most one move can be taken in every configuration. |
| `stack_symbols(p: Pda) -> Set[Str]` | The symbols the PDA uses on its stack, the bottom symbol included. |
| `member(p: Pda, w: Str) -> Bool` | Whether the PDA accepts `w`, decided through its grammar, so it never gives up as `accepts` can. |
| `words_up_to(p: Pda, n: Int) -> Set[Str]` | The words the PDA accepts of length at most `n`, as a set. |

`pda` is a keyword, so write `std.pda.member(P, w)`, `from std.pda import
member`, or `import std.pda as pdas` (see `std.grammar`).

**Ready-made PDAs.** They have the languages of the grammars with the same
names in `std.grammar`, and states named after what they do:

- `anbn("a", "b")`: `ready` (accepting, for the empty word), `as` pushes an
  `X` for each a, `bs` pops one for each b, and `done` (accepting) is
  reached when only `Z` is left.
- `balanced("l", "r")`: `even` (accepting) when every bracket so far is
  closed, `open` while some are not; after a close bracket, `closed`
  looks at the stack to choose between them.
- `palindromes(B)`: `first` pushes each symbol read, then guesses the
  middle (skipping one symbol there for odd lengths) and goes to `second`,
  which pops a matching symbol for each one read; `done` accepts when the
  stack is back to its bottom. No PDA for the palindromes can do without
  guessing, so this one is not deterministic.
- `equal_count("a", "b")`: one state, `count`, keeps the surplus on the
  stack (`A`s while there have been more a's, `B`s while more b's), and
  guesses the end with an eps move to `done` when only `Z` is left.

The first two are deterministic. Symbols follow the rules of `std.grammar`
(one letter or digit, E0614; two different ones, E0503).

**The grammar of a PDA.** `to_grammar` is the converse of the built-in
`to_pda` (which builds a PDA from a grammar), so between them every PDA and
every grammar can be turned into the other. It uses the triple construction.
A variable `[p X q]` derives the words that take the PDA from state `p` to
state `q` while popping `X`. It first makes the PDA accept by emptying its
stack: a new start state `begin` puts a new bottom `$` under the old one,
and a new state `end`, reached from each accepting state, pops everything.
A move that pushes k symbols gives a rule for every choice of k states, so
the grammar has at most 100000 rules before the useless ones are dropped
(a `LimitError`, E0612, past that). Only the variables that matter are kept.
The built-in `to_grammar` takes a regular language instead, so this one is
called by its full name or imported under another (`from std.pda import
to_grammar as grammar_of`); `from std.pda import to_grammar` is E0203.

**Questions that always answer.** The `accepts` operator searches through
the PDA's runs, and a PDA whose eps moves push forever makes it give up
(E0605). `member(p, w)` decides the same question through `to_grammar(p)`
and the CYK algorithm, so it always answers. A symbol outside the alphabet
is E0501. `words_up_to(p, n)` lists the accepted words the same way, with
the limits of `std.grammar.words_up_to`.

`is_deterministic` checks every two moves from the same state: they clash
when their reads are the same or one is eps, and their pops are the same or
one pops nothing. `to_pda(g)` chooses a rule with an eps move, so it is
deterministic only when each variable has just one rule; `std.pda.anbn` is
deterministic.

```kstar
alphabet AB = {a, b}
let P = std.pda.anbn("a", "b")
print P
print std.pda.is_deterministic(P), std.pda.stack_symbols(P)
print std.pda.to_grammar(P)
print std.pda.words_up_to(std.pda.palindromes(AB), 3)
pda Forever over AB {
    start  q
    accept f
    q --eps, eps/A--> q
    q --a, eps/eps--> f
}
print std.pda.member(Forever, "a"), std.pda.member(Forever, "b")
```

```text
PDA with 4 states over {a, b}, stack starts as Z
start ready, accept ready, done
From  | Read | Pop | Push | To
ready | a    | Z   | XZ   | as
as    | a    | X   | XX   | as
as    | b    | X   | eps  | bs
bs    | b    | X   | eps  | bs
bs    | eps  | Z   | Z    | done
true {"X", "Z"}
Grammar with 9 variables over {a, b}, start S
S             -> [begin $ end]
[begin $ end] -> [ready Z end] [end $ end]
[ready Z end] -> a [as X bs] [bs Z end] | [end Z end]
[end $ end]   -> eps
[as X bs]     -> a [as X bs] [bs X bs] | b
[bs Z end]    -> [done Z end]
[end Z end]   -> eps
[bs X bs]     -> b
[done Z end]  -> [end Z end]
{"", "a", "aa", "aaa", "aba", "b", "bab", "bb", "bbb"}
true false
```

### `std.random`

Random Ints, words, DFAs, NFAs and grammars from a seed: the same seed always gives the same result.

| Function | What it does |
|---|---|
| `int(seed: Int, low: Int, high: Int) -> Int` | An Int from `low` to `high`, both included, each as likely. |
| `pick(seed: Int, s: Set[T]) -> T` | A member of the set `s`, each as likely; the set must not be empty. |
| `word(seed: Int, alphabet: Alphabet, n: Int) -> Str` | A word of length `n` over `alphabet`, each symbol drawn on its own. |
| `words(seed: Int, alphabet: Alphabet, count: Int, max: Int) -> Set[Str]` | `count` random words of length 0 to `max`, as a set, so fewer if some are drawn twice. |
| `word_in(seed: Int, lang: Lang, n: Int) -> Str` | A word of length `n` in the language, each such word as likely; there must be one. |
| `random_dfa(seed: Int, alphabet: Alphabet, states: Int) -> Dfa` | A random DFA with `states` states named `q0`, `q1`, ... |
| `random_nfa(seed: Int, alphabet: Alphabet, states: Int) -> Nfa` | A random NFA with `states` states named `q0`, `q1`, ..., which may have eps moves. |
| `random_grammar(seed: Int, alphabet: Alphabet, variables: Int) -> Grammar` | A random grammar with `variables` variables, from 1 to 26, named `S`, `A`, `B`, ... |

**Seeds, not luck.** Every function takes a seed, any `Int`, as its first
argument, and its result depends only on its arguments. The same call gives
the same result every time and on every computer, so a program that uses
`std.random` still has one output, and a `test` built on it cannot fail by
chance. To get different results, pass different seeds: the counter of a
`for` loop is the usual one. Kleene Star never asks the operating system for
randomness. The numbers come from SplitMix64, a small, well-tested generator
(not one for passwords or keys), started from the seed for each call.

**What each draw means.**

- `int` and `pick` give every possible answer the same chance. `pick` of
  the empty set is a `ValueError` (E0613), and `int` with `low` larger than
  `high` is a `MathError` (E0609).
- `word` draws each symbol on its own; `words` draws `count` words, each of
  a length from 0 to `max` with every length as likely.
- `word_in` gives every word of length `n` in the language the same chance
  (up to rounding), so `word_in(seed, L, 8)` is a fair sample of L's words
  of length 8, however rare they are among all words. A language with no
  word of that length is a `ValueError` (E0615);
  `std.automata.count_words` says how many there are.
- `random_dfa` is total: each move goes to any state with the same chance,
  and each state accepts with chance 1/2. `random_nfa` gives each state
  zero, one or two moves on each symbol, an eps move to another state with
  chance 1/4, and
  makes it accepting with chance 1/3. The start state is `q0`.
- `random_grammar` gives each variable one to three rules, each with up to
  three symbols, every symbol a terminal or a variable with the same chance.
  Its language may well be empty: random grammars are for testing that two
  ways of doing something agree, such as a grammar and its `to_cnf`.

Lengths may be up to 10000 and machines up to 10000 states (larger is a
`LimitError`, E0612); a negative length or count, or fewer than one state or
variable, is E0610. `words` may draw at most 100000 words and `count` times
`max` may be at most a million. An alphabet with no symbols is E0613.

The names `random_dfa`, `random_nfa` and `random_grammar` repeat the module's
name because `dfa`, `nfa` and `grammar` are keywords.

```kstar
alphabet B = {0, 1}
print std.random.int(1, 1, 6), std.random.int(1, 1, 6), std.random.int(2, 1, 6)
print std.random.pick(7, {"red", "green", "blue"})
print std.random.word(3, B, 8), std.random.words(3, B, 4, 3)
print std.random.word_in(5, std.binary.divisible_by(5), 6)
let D = std.random.random_dfa(11, B, 3)
print D
for seed in 0..20 {
    let M = std.random.random_dfa(seed, B, 4)
    test equivalent(M, minimize(M))
}
```

```text
6 6 5
blue
11110100 {"00", "1", "10"}
010100
DFA with 3 states over {0, 1}
       | 0  | 1
-> *q0 | q2 | q2
   *q1 | q1 | q0
    q2 | q2 | q2
tests: 20 passed, 0 failed
```

### `std.regex`

Regular expressions built at run time and put together from pieces: from text, from a word or an alphabet, by choice, sequence and repetition, and the common prefix, suffix and substring forms.

| Function | What it does |
|---|---|
| `parse(text: Str) -> Regex` | The regex written in `text`, in the same syntax as between slashes; text that is not a regex is an error. |
| `literal(word: Str) -> Regex` | The regex that matches exactly `word` and nothing else. |
| `any_of(alphabet: Alphabet) -> Regex` | The regex for any one symbol of `alphabet`, written out as a choice between them. |
| `alt(a: Regex, b: Regex) -> Regex` | The choice of `a` or `b`: the words of either one. |
| `seq(a: Regex, b: Regex) -> Regex` | `ab`: a word of `a` followed by a word of `b`. |
| `zero_or_more(r: Regex) -> Regex` | `r*`: any number of words of `r` one after another, including none. |
| `one_or_more(r: Regex) -> Regex` | `r+`: one or more words of `r` one after another. |
| `optional(r: Regex) -> Regex` | `r?`: a word of `r`, or the empty string. |
| `power(r: Regex, n: Int) -> Regex` | Exactly `n` words of `r` one after another; `power(r, 0)` matches only the empty string. |
| `repeat_between(r: Regex, min: Int, max: Int) -> Regex` | From `min` to `max` words of `r` one after another, both included. |
| `at_least(r: Regex, n: Int) -> Regex` | `n` or more words of `r` one after another. |
| `starts_with(alphabet: Alphabet, prefix: Str) -> Regex` | The regex for the strings over `alphabet` that begin with `prefix`. |
| `ends_with(alphabet: Alphabet, suffix: Str) -> Regex` | The regex for the strings over `alphabet` that end with `suffix`. |
| `contains(alphabet: Alphabet, part: Str) -> Regex` | The regex for the strings over `alphabet` that contain `part` somewhere. |
| `size(r: Regex) -> Int` | The number of parts of the regex: its symbols, operators and joins. |
| `text(r: Regex) -> Str` | The regex written out as a string, without the slashes. |

**Why a library for regexes.** The built-ins already combine languages,
but they hand back machines: `concat(/a/, /b/)` and `star(/a/)` are `Nfa`s.
These functions keep a `Regex` a `Regex`, so it still prints as a regex and
can be compared with what a student wrote. `parse` reads a regex from a
string made while the program runs, which a `/.../` literal cannot do. To go
the other way, from a machine to a regex, use the built-in `to_regex`.

The results are simplified as they are built, with the identities a
textbook uses: `[]` disappears from a choice and swallows a sequence, `()`
disappears from a sequence, and `(r*)*` is `r*`. So `power(r, 0)` is `()`,
and `repeat_between(/a/, 1, 3)` prints as `a(aa?)?`: the optional copies
are nested so that each word splits into copies in only one way.

`starts_with(B, "01")` is `01(0|1)*`, with the alphabet written out; it is
the regex counterpart of `std.automata.starts_with`, which builds the
smallest DFA instead.

`parse` reports a bad regex as a `ValueError` (E0105) with the parser's
message. A negative number of copies is E0610, `min` larger than `max` is
E0609, a word with a symbol outside the alphabet is E0501, and a result of
more than 20000 parts (the same limit as for a regex the parser reads) is a
`LimitError` (E0612), checked before anything is built.

```kstar
alphabet B = {0, 1}
let digit = std.regex.any_of(B)
let byte = std.regex.power(digit, 8)
print std.regex.text(std.regex.power(digit, 3)), std.regex.size(byte)
print byte accepts "01100001", byte accepts "0110"
let typed = std.regex.parse("1(0|1)*")
let no_leading_zero = std.regex.alt(std.regex.literal("0"), typed)
print no_leading_zero
print std.regex.repeat_between(std.regex.literal("ab"), 1, 3), std.regex.at_least(/a/, 2)
print std.regex.contains(B, "11"), equivalent(std.regex.ends_with(B, "01"), std.automata.ends_with(B, "01"))
```

```text
(0|1)(0|1)(0|1) 31
true false
/0|1(0|1)*/
/ab(ab(ab)?)?/ /aaa*/
/(0|1)*11(0|1)*/ true
```

### `std.steps`

The working of the conversions, step by step: eps-closures, the subset construction table, the k-equivalence partitions and state elimination.

| Function | What it does |
|---|---|
| `elimination(m: Lang) -> Str` | State elimination, as `to_regex` does it: the new start and final states, then each state removed and the edges that replace the paths through it, then the regex. |
| `eps_closures(m: Lang) -> Str` | A table of each state of `m`'s NFA and its eps-closure, the states it reaches by eps moves alone. |
| `partitions(m: Lang) -> Str` | Minimisation of `m`'s DFA by k-equivalence: the partitions P0, P1, ... until one is the same as the one before. |
| `subset_table(m: Lang) -> Str` | The subset construction for `m`'s NFA as a table: each set of states found, in order, and where each symbol leads, with the empty set written Ø. |

Each function gives a string to `print`, laid out the way the usual
textbook working writes it by hand. It runs the same code as the
conversion it explains, so the working always ends where `to_dfa`,
`minimize` or `to_regex` does. `m` may be any regular language: a `Dfa`,
`Nfa`, `Regex` or `Set[Str]`. A regex or set is first turned into its NFA
(Thompson's construction), and `partitions` and `elimination` work on the
DFA, as `minimize` and `to_regex` do.

- `eps_closures` marks the start state `->` and accepting states `*`, as a
  transition table does.
- `subset_table` starts from the eps-closure of the start state, and lists
  the sets in the order they are found. A set is named as `to_dfa` names
  it, `{a,b}`, except the empty set, written `Ø`: the dead state, which
  every symbol leads back to.
- `partitions` first drops the states that cannot be reached, as
  `minimize` does. P0 splits the accepting states from the rest; each next
  partition splits a group when its states go to different groups on some
  symbol. It stops at the first partition equal to the one before, whose
  groups are the states of the minimal DFA (named `q1+q3`, as `minimize`
  names them). `std.benchmark.equivalence_rounds` counts the rounds.
- `elimination` minimises the DFA, adds a new start state `S` and a new
  final state `F` (primed if those names are taken), then removes one
  state at a time, in the order `to_regex` does, listing the edges each
  removal makes or changes: a path `i --a--> k --b--> j` through `k` with a
  loop `c` becomes `i --ac*b--> j`, joined with `|` to any edge already
  there. An `eps` label is the empty string. The regex at the end is the
  one `to_regex` gives. When the working would build a regex of over 5000
  parts it is a `LimitError` (E0608).

```kstar
alphabet B = {0, 1}
nfa N over B {
    start  p
    accept r
    p --1--> q
    p --1--> r
    q --0,1--> q
    q --1--> r
}
print std.steps.subset_table(N)
dfa M over B {
    start  q0
    accept q2
    q0 --0--> q1
    q0 --1--> q3
    q1 --0--> q2
    q1 --1--> q0
    q2 --0,1--> q2
    q3 --0--> q2
    q3 --1--> q0
}
print std.steps.partitions(M)
print std.steps.elimination(M)
```

```text
Start: {p}
          | 0   | 1
->  {p}   | Ø   | {q,r}
    Ø     | Ø   | Ø
   *{q,r} | {q} | {q,r}
    {q}   | {q} | {q,r}
The DFA has 4 states; Ø is the dead state: every symbol leads from it back to it.
P0: {q0, q1, q3} {q2}
P1: {q0} {q1, q3} {q2}
P2: {q0} {q1, q3} {q2}
P2 = P1, so the minimal DFA has 3 states.
The minimal DFA has 3 states: q0, q1+q3, q2.
Add a new start state S and a new final state F, joined by eps moves:
  S --eps--> q0
  q0 --0|1--> q1+q3
  q1+q3 --1--> q0
  q1+q3 --0--> q2
  q2 --0|1--> q2
  q2 --eps--> F
Remove q2, which loops on 0|1:
  q1+q3 --0(0|1)*--> F
Remove q0:
  q1+q3 --1(0|1)--> q1+q3
  S --0|1--> q1+q3
Remove q1+q3, which loops on 1(0|1):
  S --(0|1)(1(0|1))*0(0|1)*--> F
Regex: (0|1)(1(0|1))*0(0|1)*
```

### `std.string`

Words as data: searching, slicing, repeating, and the prefixes, suffixes and substrings of a word.

| Function | What it does |
|---|---|
| `char_at(s: Str, i: Int) -> Str` | The character of `s` at index `i`, counting from 0, as a string of length 1. |
| `contains(s: Str, part: Str) -> Bool` | Whether `part` occurs anywhere inside `s`; the empty string occurs in every string. |
| `count(s: Str, part: Str) -> Int` | How many times `part` occurs in `s`, not overlapping: `count("aaaa", "aa")` is 2; `part` must not be empty. |
| `ends_with(s: Str, suffix: Str) -> Bool` | Whether `s` ends with `suffix`. |
| `from_int(n: Int) -> Str` | The digits of `n` as a string, with a leading `-` if it is negative. |
| `index_of(s: Str, part: Str) -> Int` | The index of the first occurrence of `part` in `s`, or -1 if there is none. |
| `is_palindrome(s: Str) -> Bool` | Whether `s` reads the same backwards; the empty string does. |
| `length(s: Str) -> Int` | The number of characters in `s`; the same as `len(s)`. |
| `prefixes(s: Str) -> Set[Str]` | Every prefix of `s`, including the empty string and `s` itself. |
| `repeat(s: Str, n: Int) -> Str` | `s` written `n` times in a row; `n` must not be negative, and the result at most 1000000 characters. |
| `reverse(s: Str) -> Str` | The characters of `s` in the opposite order. |
| `starts_with(s: Str, prefix: Str) -> Bool` | Whether `s` starts with `prefix`. |
| `substring(s: Str, start: Int, end: Int) -> Str` | The characters of `s` from index `start` up to, but not including, index `end`; needs `0 <= start <= end <= length(s)`. |
| `substrings(s: Str) -> Set[Str]` | Every substring of `s` (a run of neighbouring characters), including the empty string and `s` itself. |
| `suffixes(s: Str) -> Set[Str]` | Every suffix of `s`, including the empty string and `s` itself. |
| `symbols(s: Str) -> Set[Str]` | The distinct characters of `s`, each as a string of length 1; the empty string is not one of them. |
| `to_int(s: Str) -> Int` | The number written in `s`: digits with an optional leading `-` or `+`, and nothing else. |

Indices count characters, not bytes, from 0, and `substring(s, start, end)`
leaves out the character at `end`. `length(s)` is the same as the built-in
`len(s)`, which also measures sets and alphabets. The built-in `reverse`
reverses a *language*; `std.string.reverse` reverses one *word*, so it has to
be written in full or imported under another name
(`from std.string import reverse as reverse_word`): importing it as plain
`reverse` is an error, because that name belongs to the built-in.

`prefixes`, `suffixes` and `substrings` return sets that always include the
empty string (and the word itself). `symbols` does not include the empty
string: it is the word's distinct characters, each as a one-character
string. `index_of` gives -1 when there is no match, and `count` does not
count overlapping matches (`count("aaaa", "aa")` is 2).

A mistake in an argument is a `ValueError`: an index or count out of range
is E0610, text `to_int` cannot read is E0611, and a number too large for an
Int is E0600. A result of more than 1000000 characters (for a set, all its
members added up) is a `LimitError`, E0612, instead of a hang.

```kstar
let w = "abracadabra"
print std.string.length(w), std.string.count(w, "abra"), std.string.index_of(w, "cad")
print std.string.substring(w, 4, 7), std.string.char_at(w, 0), std.string.reverse(w)
print std.string.is_palindrome("level"), std.string.repeat("01", 3)
print std.string.to_int("-42") + 1, std.string.from_int(7) + "!"
print std.string.prefixes("ab"), std.string.symbols("abba")
```

```text
11 2 4
cad a arbadacarba
true 010101
-41 7!
{"", "a", "ab"} {"a", "b"}
```

### `std.testing`

Checks for `test` statements that explain a failure: equality with the difference shown, sets of words a machine must accept or reject, equal languages with a word that tells them apart, and two acceptors that must agree on sample words.

| Function | What it does |
|---|---|
| `equal(expected: T, actual: T) -> Bool` | Whether `actual` is `expected`; a failing test shows both, with the first difference of two strings marked. |
| `equal_sets(expected: Set[T], actual: Set[T]) -> Bool` | Whether two sets are equal; a failing test lists the members that are missing and those that should not be there. |
| `accepts_all(machine: Acceptor, words: Set[Str]) -> Bool` | Whether `machine` accepts every word of `words`; a failing test lists the words it rejects. |
| `rejects_all(machine: Acceptor, words: Set[Str]) -> Bool` | Whether `machine` rejects every word of `words`; a failing test lists the words it accepts. |
| `same_language(expected: Lang, actual: Lang) -> Bool` | Whether two regular languages are `equivalent`; a failing test shows a shortest word that is in one but not the other. |
| `agree_on(a: Acceptor, b: Acceptor, words: Set[Str]) -> Bool` | Whether `a` and `b` accept the same words of `words`; a failing test lists the words only one of them accepts. |
| `agree_up_to(a: Acceptor, b: Acceptor, n: Int) -> Bool` | Whether `a` and `b` accept the same words of length up to `n` over their symbols, at most 10000 words; a failing test lists the words only one accepts. |

**Use them in `test`.** Each function is an ordinary check that returns a
`Bool`, so it can go anywhere a condition can. Its point is what happens
when it is the condition of a `test` that fails: under the usual two lines,
the test prints *why*.

```kstar-error
alphabet B = {0, 1}
let EndsIn1 = std.automata.ends_with(B, "1")
test std.testing.equal("10110", std.binary.to_binary(21))
test std.testing.accepts_all(EndsIn1, {"1", "01", "10"})
test std.testing.same_language(EndsIn1, /(0|1)*1(0|1)?/)
```

```text
test failed: demo.kstar, line 3
    test std.testing.equal("10110", std.binary.to_binary(21))
    expected: "10110"
    actual:   "10101"
                  ^ first difference, at index 3
test failed: demo.kstar, line 4
    test std.testing.accepts_all(EndsIn1, {"1", "01", "10"})
    should accept, but rejects: "10"
test failed: demo.kstar, line 5
    test std.testing.same_language(EndsIn1, /(0|1)*1(0|1)?/)
    "10" is in the actual language but not in the expected one
tests: 0 passed, 3 failed
```

The explanation comes from every `std.testing` check in the condition that
returned `false`, so `test equal(a, b) or equal(a, c)` explains both. A
check outside `test` (in `print`, `if` or `let`) explains nothing; it is
just a `Bool`.

**In imported modules, nothing changes.** `test` statements of a file that
is imported are skipped (see [modules](#modules)), so a module's own checks
stay quiet when another program uses it, and run when the module is run by
itself.

**`Acceptor`** in the signatures means anything the `accepts` operator
works on: a `Dfa`, `Nfa`, `Regex`, `Set[Str]`, `Grammar`, `Pda` or `Tm`.
A word with a symbol outside a machine's alphabet is an `AlphabetError`
(E0501), as with `accepts`; a PDA that gives up (E0605) or a Turing
machine that does not halt (E0604) is a `LimitError`.

**How they relate to the built-ins.** `same_language` is `equivalent` plus a
shortest word that shows the difference (`std.automata.distinguishing_word`).
`equivalent` refuses grammars, PDAs and Turing machines because their
equivalence is undecidable; for them, `agree_on` compares the answers on
chosen words, and `agree_up_to` on every word up to a length, over the
symbols of both. That cannot prove two of them equal, but it finds a
difference whenever one is that short. `agree_up_to` tries at most 10000
words (E0612 otherwise; a negative length is E0610).

### `std.tm`

Ready-made Turing machines (binary increment, unary addition and copying, deciders for palindromes and a^n b^n c^n), and the steps, space and final state of a run.

| Function | What it does |
|---|---|
| `increment() -> Tm` | A machine that adds one to a binary number, leaving the sum on the tape. |
| `unary_add() -> Tm` | A machine that turns `m` ones, a 0 and `n` ones into `m + n` ones. |
| `unary_copy() -> Tm` | A machine that turns `n` ones into `n` ones, a 0 and `n` ones again. |
| `palindromes(alphabet: Alphabet) -> Tm` | A machine that decides the palindromes over `alphabet`, halting in `yes` or `no`. |
| `anbncn(a: Str, b: Str, c: Str) -> Tm` | A machine that decides `n` copies of `a`, then of `b`, then of `c`, which no PDA can. |
| `steps(m: Tm, w: Str) -> Int` | How many steps the machine takes on `w` before it halts. |
| `space(m: Tm, w: Str) -> Int` | How many tape cells the head visits on `w` before the machine halts. |
| `final_state(m: Tm, w: Str) -> Str` | The state the machine is in when it halts on `w`. |
| `halts_within(m: Tm, w: Str, n: Int) -> Bool` | Whether the machine halts on `w` within `n` steps, at most 100000. |
| `tape_symbols(m: Tm) -> Set[Str]` | The symbols that can be on the tape: the alphabet, the blank `_`, and every symbol the machine writes. |

`tm` is a keyword, so write `std.tm.steps(M, w)`, `from std.tm import
steps`, or `import std.tm as tms` (see `std.grammar`).

**Ready-made machines and their tape conventions.** The machines that
compute leave their answer on the tape, where the built-in `run` reads it.
The machines that decide end in `yes` or `no`.

- `increment()`, over `{0, 1}`: the input is a binary number, most
  significant bit first (`""` counts as 0). It halts in `done` with the
  number plus one: `run(M, "1011")` is `"1100"`.
- `unary_add()`, over `{0, 1}`: the input is `m` ones, a `0` and `n` ones,
  so `"11011"` is 2 + 2. It halts in `done` with `m + n` ones; an input
  without exactly one `0` halts in `no`.
- `unary_copy()`, over `{1}`: the input is `n` ones. It halts in `done` with
  `n` ones, a `0` and `n` ones: `"111"` becomes `"1110111"`.
- `palindromes(B)`: any word over `B`. It halts in `yes` for a palindrome,
  otherwise in `no`.
- `anbncn("a", "b", "c")`: any word over the three symbols. It halts in
  `yes` for `n` a's, then `n` b's, then `n` c's, otherwise in `no`.

The deciders never get stuck: every state has a move for every symbol it
can meet, so a word they reject ends in `no` and `final_state` says so.
`unary_copy` and `anbncn` mark cells as they go (`x`, and `x`, `y`, `z`),
so the symbols of `anbncn` must be three different letters or digits other
than `x`, `y` and `z` (an `AlphabetError`, E0503). `palindromes` erases the
first and the last symbol and checks they match, so it takes about n^2/2
steps; `steps` shows the growth.

**Questions about a run.** `steps`, `space` and `final_state` run the
machine until it halts, as `run` does. Past 100 000 steps they give the
same `LimitError` (E0604), because by the halting problem no program can
wait for every machine. `space` counts the cells from the leftmost to the
rightmost the head reached, because the head moves one cell at a time.
`halts_within(m, w, n)` is the question that answers either way: `true`
if the machine halts within `n` steps, `false` if it is still running. `n`
may be up to 100 000 (E0612 past that, E0610 if negative). An input with a
symbol outside the alphabet is E0501.

```kstar
let Inc = std.tm.increment()
print run(Inc, "1011"), std.tm.steps(Inc, "1011"), std.tm.space(Inc, "1011"), std.tm.final_state(Inc, "1011")
print run(std.tm.unary_add(), "11011"), run(std.tm.unary_copy(), "111")
alphabet AB = {a, b}
let Pal = std.tm.palindromes(AB)
print Pal accepts "abba", std.tm.final_state(Pal, "ab"), std.tm.steps(Pal, "abba")
let ABC = std.tm.anbncn("a", "b", "c")
print ABC accepts "aabbcc", ABC accepts "aabbc", std.tm.tape_symbols(ABC)
print std.tm.halts_within(Inc, "111", 5), std.tm.halts_within(Inc, "111", 8)
```

```text
1100 8 5 done
1111 1110111
true no 15
true false {"_", "a", "b", "c", "x", "y", "z"}
false true
```

## Types

| Type | Values | Written as |
|---|---|---|
| `Int` | whole numbers (64-bit) | `0`, `42` |
| `Bool` | truth values | `true`, `false` |
| `Str` | strings | `"0110"`, `""` |
| `Alphabet` | a set of symbols | made by `alphabet` |
| `Dfa` | a complete deterministic machine | `dfa over B { ... }` |
| `Nfa` | a nondeterministic machine | `nfa over B { ... }` |
| `Regex` | a regular expression | `/(0\|1)*01/` |
| `Pda` | a pushdown automaton | `pda over B { ... }` |
| `Tm` | a Turing machine | `tm over B { ... }` |
| `Grammar` | a context-free grammar | `grammar over B { ... }` |
| `Set[T]` | a set of `Int`, `Str` or `Bool` | `{1, 2, 3}`, `{"a", "ab"}`, `0..5` |

Types are written only in function signatures; everywhere else they are worked
out for you. All type errors are found before the program runs.

A **regular language** is described by a `Dfa`, an `Nfa`, a `Regex`, or a
`Set[Str]` (a finite language). Wherever this page says "language", any of the
four is accepted. A `Pda` or a `Grammar` describes a **context-free language**,
which may not be regular; they work with `accepts`, `trace` (PDA), `draw`
(PDA) and the grammar built-ins below, but not with the operations for
regular languages.

In a string, `\"` and `\\` are the only escapes.

## Operators

From loosest to tightest binding:

| Operators | Works on | Result |
|---|---|---|
| `or` | `Bool` | `Bool` |
| `and` | `Bool` | `Bool` |
| `not` | `Bool` | `Bool` |
| `==` `!=` | two values of the same type (not machines, regexes or grammars) | `Bool` |
| `<` `<=` `>` `>=` | `Int` | `Bool` |
| `accepts` | language, `Pda`, `Tm` or `Grammar`, then `Str` | `Bool` |
| `..` | `Int` | `Set[Int]`: `a..b` is `a` up to but not including `b` |
| `+` `-` | `Int`; `+` also joins two `Str` | `Int` or `Str` |
| `*` `%` | `Int` | `Int` |
| `-` (prefix) | `Int` | `Int` |

Comparisons do not chain: write `a < b and b < c`. `and` and `or` skip the
right side when the left side already decides the answer. `%` is the remainder
and is never negative for a positive divisor. There is no division operator.

```kstar
print 7 % 3, -1 % 3, 2 + 3 * 4
print "ab" + "c", 1..4, {2, 1} == {1, 2}
print /a*b/ accepts "aab", {"x", "y"} accepts "z"
```

```text
1 2 14
abc {1, 2, 3} true
true false
```

## Regular expressions

Written between slashes. Spaces are ignored.

| Syntax | Meaning |
|---|---|
| `a` | a symbol: any letter or digit |
| `rs` | `r` then `s` |
| `r\|s` | `r` or `s` |
| `r*` `r+` `r?` | zero or more, one or more, zero or one |
| `(r)` | grouping |
| `.` | any one symbol of the alphabet |
| `()` | the empty string |
| `[]` | the empty language |
| `\x` | the character `x` literally, for example `\/` |

Postfix operators bind tightest, then concatenation, then `|`.

## Built-in functions

### Converting

| Function | Returns | What it does |
|---|---|---|
| `to_nfa(lang)` | `Nfa` | A regex becomes an eps-NFA by Thompson's construction. |
| `to_dfa(lang)` | `Dfa` | Subset construction. States are named after sets, e.g. `{a,b}`. |
| `to_nfa(lang, B)`, `to_dfa(lang, B)` | | The same, over alphabet `B`. |
| `to_regex(lang)` | `Regex` | State elimination. |
| `remove_eps(lang)` | `Nfa` | The same language without eps moves. |

To see the working of a conversion step by step, use
[`std.steps`](#stdsteps).

### Simplifying

| Function | Returns | What it does |
|---|---|---|
| `minimize(lang)` | `Dfa` | The unique smallest DFA. Merged states are named `q1+q2`. |
| `reduce(lang)` | `Nfa` | Removes useless states and merges equivalent ones. Smaller, but not guaranteed smallest. |
| `rename(machine)` | same type | Renames states `q0, q1, ...` outward from the start. |

### Combining

| Function | Returns | Language |
|---|---|---|
| `union(a, b)` | `Dfa` | strings in `a` or `b` |
| `intersect(a, b)` | `Dfa` | strings in both |
| `difference(a, b)` | `Dfa` | strings in `a` but not `b` |
| `complement(a)` | `Dfa` | strings over the alphabet not in `a` |
| `concat(a, b)` | `Nfa` | a string of `a` followed by a string of `b` |
| `star(a)` | `Nfa` | zero or more strings of `a` joined together |
| `reverse(a)` | `Nfa` | the strings of `a` written backwards |

### Asking

| Function | Returns | Meaning |
|---|---|---|
| `equivalent(a, b)` | `Bool` | Do `a` and `b` contain exactly the same strings? |
| `is_empty(a)` | `Bool` | Does `a` contain no strings? Also works on a `Grammar`. |
| `states(machine)` | `Int` | The number of states of a `Dfa`, `Nfa`, `Pda` or `Tm`. |
| `len(x)` | `Int` | Length of a `Str`, or size of a `Set` or `Alphabet`. |

### Grammars and pushdown automata

| Function | Returns | What it does |
|---|---|---|
| `to_cnf(g)` | `Grammar` | An equivalent grammar in Chomsky normal form: rules `A -> B C` and `A -> a`, plus `S0 -> eps` if needed. |
| `to_pda(g)` | `Pda` | The three-state PDA that simulates leftmost derivations of `g`. |
| `to_grammar(lang)` | `Grammar` | A right-linear grammar for a regular language, one variable per state of its minimal DFA. |
| `derive(g, w)` | `Str` | A shortest leftmost derivation of `w`, one sentential form per line. A `ValueError` if `g` cannot derive `w`. |

`g accepts w` uses the CYK algorithm on `to_cnf(g)`. `equivalent` refuses
grammars and PDAs before running: equivalence of context-free languages is
undecidable. Operations for regular languages (`minimize`, `union`,
`complement`, ...) refuse them too.

### Turing machines

| Function | Returns | What it does |
|---|---|---|
| `run(m, w)` | `Str` | Runs Turing machine `m` on `w` until it halts and returns the tape, without the blanks at either end. A `LimitError` after 100 000 steps. |

`equivalent` refuses Turing machines: whether two of them accept the same
language is undecidable.

### Generating Rust

| Function | Returns | What it does |
|---|---|---|
| `to_rust(lang, name)` | `Str` | Rust source for `pub fn name(input: &str) -> bool`, which accepts exactly the words of `lang`. |

`lang` is any regular language; it is converted to a DFA first. The code is
one dependency-free function, a `match` on the state number and the symbol
with the original state names as comments, followed by a `#[cfg(test)]`
module that checks a few of the shortest accepted and rejected words. A
symbol outside the alphabet makes the function return `false`. `name` must
be a valid Rust function name and not a Rust keyword (E0308). Grammars,
PDAs and Turing machines are refused when the program is checked. The
command `kstar export` does the same from the command line.

### Machines as JSON

| Function | Returns | What it does |
|---|---|---|
| `to_json(m)`, `to_json(m, name)` | `Str` | `m` (a `Dfa`, `Nfa`, `Regex`, `Grammar`, `Pda` or `Tm`) in [Kleene Star's JSON format](#the-json-format), with `name` if given. |
| `from_json(Kind, text)` | `Kind` | The machine, regex or grammar in `text`. `Kind` is the type expected, written as a type name: `Dfa`, `Nfa`, `Regex`, `Grammar`, `Pda` or `Tm`. |

`from_json`'s first argument is a type name, not a value, so the checker
knows what the call returns: `from_json(Dfa, text)` is a `Dfa`. If `text` is
not valid JSON, not in the format, or holds another kind, the program stops
with a `ValueError` (E0616) that says where the problem is. A string cannot
span lines, so JSON written into a program goes on one line, which JSON
allows. The command `kstar export --format json` writes a file.

### Alphabets in operations

- A `Dfa` or `Nfa` has a fixed alphabet. When an operation takes two machines,
  their alphabets must be the same.
- A `Regex` or `Set[Str]` is flexible. Combined with a machine, it uses the
  machine's alphabet. On its own, it uses the symbols it mentions.
- `machine accepts word` is an error if `word` has a symbol outside the
  machine's alphabet. `regex accepts word` is simply false in that case.

## The JSON format

`to_json` and `kstar export --format json` write, and `from_json` reads, one
JSON object per machine. Version 1 of the format looks like this for a DFA:

```kstar
alphabet B = {0, 1}
dfa EvenZeros over B {
    start  even
    accept even
    even --0--> odd
    even --1--> even
    odd  --0--> even
    odd  --1--> odd
}
print to_json(EvenZeros, "EvenZeros")
```

```text
{
  "format": "kleene-star",
  "version": 1,
  "kind": "dfa",
  "name": "EvenZeros",
  "alphabet": ["0", "1"],
  "states": ["even", "odd"],
  "start": "even",
  "accept": ["even"],
  "transitions": [
    {"from": "even", "symbol": "0", "to": "odd"},
    {"from": "even", "symbol": "1", "to": "even"},
    {"from": "odd", "symbol": "0", "to": "even"},
    {"from": "odd", "symbol": "1", "to": "odd"}
  ]
}
```

Every file has these fields first:

| Field | Value |
|---|---|
| `format` | always `"kleene-star"` |
| `version` | `1`; a reader refuses a version newer than its own |
| `kind` | `"dfa"`, `"nfa"`, `"regex"`, `"grammar"`, `"pda"` or `"tm"` |
| `name` | optional: the machine's name |

Then, by kind (states, variables and symbols are always written by name,
so the names survive a round trip):

| Kind | Fields |
|---|---|
| `dfa` | `alphabet`: symbols; `states`: names, in order; `start`; `accept`: states; `transitions`: `{"from", "symbol", "to"}`, exactly one for each state and symbol |
| `nfa` | the same, with any number of transitions; `"symbol": null` is an eps move |
| `regex` | `regex`: the regex in the language's syntax, without the slashes |
| `grammar` | `alphabet`; `variables`: names; `start`: a variable; `rules`: `{"head", "body"}`, where `body` is a list of `{"terminal": "0"}` and `{"variable": "S"}` (an empty list is eps) |
| `pda` | `alphabet`; `states`; `start`; `accept`; `bottom`: the stack symbol the stack starts with; `moves`: `{"from", "read", "pop", "push", "to"}`, where `read` and `pop` may be `null` (eps) and `push` lists stack symbols, top first |
| `tm` | `alphabet`; `states`; `start`; `accept`; `reject`; `moves`: `{"from", "read", "write", "move", "to"}`, where `move` is `"L"`, `"R"` or `"S"` and `"_"` is the blank |

A symbol is a string of one letter or digit. A grammar body says which items
are terminals and which are variables because they can look alike: over the
alphabet `{A, B}`, `to_grammar` makes both a terminal `A` and a variable `A`.

**Reading is strict**, because a file may come from anywhere:

- the text may be at most 1 MiB, and JSON nesting is limited (to 128 levels;
  a valid file never goes deeper than 4);
- at most 10000 states, variables or symbols, at most 100000 transitions,
  rules or moves, at most 1000 symbols in a rule body or a push;
- names have 1 to 200 characters and no control characters, and no state
  or variable is declared twice;
- every state, variable and symbol that is used must be declared, and every
  symbol must be in the alphabet;
- a DFA must be total and deterministic, a Turing machine deterministic, and
  no state both accepts and rejects;
- a field the format does not have is an error, so a misspelt `"acept"` is
  reported instead of meaning "accepts nothing".

An error names the place as a path, such as `transitions[3].to`. The round trip
keeps everything: `from_json(Dfa, to_json(D))` has the same states, with the
same names, in the same order, and the same transitions as `D`. A regex
comes back as an equivalent regex (its text is read again).

## Errors

Errors found before running show the place and the problem. Errors while
running also show the chain of calls, most recent last. Every error has a
code in brackets, like `NameError[E0200]`; `kstar explain E0200` explains
it at length, and [the list of errors and warnings](errors.md) has every
code with an example.

The first two digits of a code give its kind: `E01xx` syntax, `E02xx`
names, `E03xx` types, `E04xx` machines, `E05xx` alphabets, `E06xx` errors
while running (maths, recursion, limits, values and files), and `W0xxx`
warnings.

`kstar run` and `kstar check` report every error in a file, not only the
first: the parser skips to the next statement after a syntax error, and the
checker carries on after each statement that fails. A name whose definition
failed is not reported again where it is used.

| Error | Cause |
|---|---|
| `SyntaxError` | The text is not a valid program, or nests more than 200 levels deep (500 in a regex). |
| `NameError` | A name is unknown, defined twice, or used before its definition has run; a grammar variable with no rules. |
| `TypeError` | A value of the wrong type, a wrong number of arguments, a function that might not return, or `trace` on something that is not a `Dfa` or `Nfa`. |
| `MachineError` | A machine breaks a rule: no start state, a DFA that is not total or not deterministic, an eps move in a DFA, a grammar with no rules, a Turing machine with two moves for one state and symbol or a blank in its alphabet. |
| `AlphabetError` | A symbol outside the alphabet, or two machines over different alphabets. |
| `MathError` | Modulo by zero, a result too large for an `Int`, or a range that is too large. |
| `RecursionError` | More than 500 function calls in progress at once. |
| `LimitError` | A search gave up: a PDA run (50 000 configurations or a stack of 1 000), a derivation (200 000 sentential forms), or a Turing machine still running after 100 000 steps; or a result too large to build, such as a regex from `to_regex` of more than 100 000 parts. |
| `ValueError` | A value of the right type that cannot be used, such as a word a grammar cannot derive. |
| `IOError` | `draw` could not write its file. |

One example of each of the common ones:

```kstar-error
let total = 1
let total = 2
```

```text
Error in demo.kstar, line 2:
    let total = 2
        ^^^^^
NameError[E0201]: `total` is already defined
  hint: values cannot be changed once defined; choose a new name
```

```kstar-error
print len(42)
```

```text
Error in demo.kstar, line 1:
    print len(42)
              ^^
TypeError[E0300]: argument 1 of `len` must be a Str, Set or Alphabet, but this is an Int
```

```kstar-error
alphabet B = {0, 1}
nfa M over B {
    start a
    a --x--> a
}
```

```text
Error in demo.kstar, line 4:
    a --x--> a
        ^
AlphabetError[E0500]: symbol `x` is not in alphabet `B`
  hint: B = {0, 1}
```

```kstar-error
alphabet B = {0, 1}
dfa M over B {
    start a
    a --0,1--> a
    a --0--> b
    b --0,1--> b
}
```

```text
Error in demo.kstar, line 5:
    a --0--> b
    ^^^^^^^^^^
MachineError[E0405]: state `a` already goes to `a` on `0`
  hint: a DFA has one transition per state and symbol; use `nfa` if you want a choice
```

```kstar-error
alphabet B = {0, 1}
dfa Any over B {
    start  q
    accept q
    q --0,1--> q
}
trace Any accepts "01"
```

```text
Error in demo.kstar, line 7:
    trace Any accepts "01"
              ^^^^^^^
SyntaxError[E0100]: expected `on` after the machine to trace, found `accepts`
  hint: write `trace M on "0110"`
```

```kstar-error
fn forever(n: Int) -> Int {
    return forever(n + 1)
}
print forever(0)
```

## Grammar

In the notation below, `'x'` is literal text, `*` means zero or more, `?` means
optional, and `|` separates alternatives.

```text
program     := statement*

statement   := 'alphabet' NAME '=' '{' (symbol (',' symbol)*)? '}'
             | ('dfa' | 'nfa' | 'pda' | 'tm') NAME machine
             | 'grammar' NAME grammar
             | 'let' NAME '=' expr
             | 'fn' NAME '(' (param (',' param)*)? ')' '->' type block
             | 'return' expr
             | 'if' expr block ('else' (if-statement | block))?
             | 'for' NAME 'in' expr block
             | 'print' expr (',' expr)*
             | 'test' expr
             | 'draw' expr ( 'on' expr )?
             | 'trace' range 'on' expr
             | 'import' lib ('as' NAME)?                     (top level only)
             | 'from' lib 'import' NAME ('as' NAME)? (',' NAME ('as' NAME)?)*

lib         := STRING                       a path ending in `.kstar`
             | 'std' '.' NAME
qualified   := NAME ('.' NAME)*             no spaces around the dots

block       := '{' statement* '}'
param       := NAME ':' type
type        := 'Int' | 'Bool' | 'Str' | 'Alphabet' | 'Dfa' | 'Nfa' | 'Pda' | 'Tm'
             | 'Regex' | 'Grammar' | 'Set' '[' type ']'

machine     := 'over' qualified '{' item* '}'
item        := 'start' state
             | 'accept' state (',' state)*
             | 'reject' state (',' state)*                  (tm)
             | state '--' action '-->' state
             | 'for' NAME 'in' expr '{' item* '}'
state       := NAME ('[' expr ']')?
action      := label (',' label)*                  (dfa, nfa)
             | label ',' label '/' push             (pda)
             | tape '/' tape ',' ('L' | 'R' | 'S')  (tm)
tape        := symbol | '_'
push        := 'eps' | letters and digits written together, top first
label       := symbol | 'eps'
symbol      := one letter or digit

grammar     := 'over' qualified '{' rule* '}'
rule        := VARIABLE '->' alternative ('|' alternative)*
alternative := 'eps' | (VARIABLE | symbol)+
VARIABLE    := a NAME starting with a capital letter

expr        := or
or          := and ('or' and)*
and         := not ('and' not)*
not         := 'not' not | comparison
comparison  := range (('==' | '!=' | '<' | '<=' | '>' | '>=' | 'accepts') range)?
range       := sum ('..' sum)?
sum         := product (('+' | '-') product)*
product     := unary (('*' | '%') unary)*
unary       := '-' unary | primary
primary     := INT | STRING | REGEX | 'true' | 'false'
             | qualified
             | qualified '(' (expr (',' expr)*)? ')'
             | '(' expr ')'
             | '{' (expr (',' expr)*)? '}'
             | ('dfa' | 'nfa' | 'pda' | 'tm') machine
             | 'grammar' grammar
```

### Warnings

Warnings are reported before a program runs, but do not stop it, and are
only looked for when there are no errors:

| Code | Warning |
|---|---|
| `W0100` | A state no path from the start state reaches. |
| `W0101` | In a DFA or NFA, a state from which no accepting state can be reached (except a DFA's dead state that loops to itself). |
| `W0200` | A `let` value that is never used. |
| `W0201` | A function that is never called. |
| `W0300` | A `test` that uses no names, calls or regexes, so its answer never changes. |

A name starting with `_` is never reported as unused, and a machine that
can never accept is not warned about at all.

Keywords, which cannot be used as names: `accept` `accepts` `alphabet` `and`
`as` `dfa` `draw` `else` `eps` `false` `fn` `for` `from` `grammar` `if`
`import` `in` `let` `nfa` `not` `on` `or` `over` `pda` `print` `reject`
`return` `start` `test` `tm` `trace` `true`. `std`, the standard library,
cannot be defined either (E0209).

## The `kstar` command

| Command | What it does |
|---|---|
| `kstar run <files>...` | Runs programs. A folder means every `.kstar` file in it. `kstar file.kstar` is short for `kstar run file.kstar`. |
| `kstar run --out dir ...` | Writes drawings into `dir`. |
| `kstar run --watch ...` | Runs again whenever a file changes; Ctrl-C stops. |
| `kstar run --trace-calls ...` | Prints a line for every function call (with its arguments) and return (with its result). |
| `kstar run --fixed-clock ...` | Makes every time from `std.benchmark` 0, so the output is the same on every run. |
| `kstar run --allow-path dir ...` | Lets `std.io` also read and write files in `dir`; without it a program may only use files in its own folder. |
| `kstar run --eager-imports ...` | Runs each imported module at its `import` line, instead of when one of its names is first used. |
| `kstar check <files>...` | Reports every error and warning without running. |
| `kstar fmt <files>...` | Rewrites files (or every file in a folder) in the standard style. See [formatting](#formatting). |
| `kstar fmt --check <files>...` | Lists the files that are not formatted, changing nothing. |
| `kstar fmt -` | Formats standard input to standard output, for editors and scripts. |
| `kstar new <name>` | Writes `<name>.kstar`, a small working program to start from. Never overwrites. |
| `kstar trace <file> <M> <word>` | Runs the file without showing its output, then traces machine `M` (any expression) on `word`. |
| `kstar export <file> <M> [--fn name] [--minimize]` | Runs the file without showing its output, then writes the regular language `M` as a standalone Rust function (see [`to_rust`](#generating-rust)) to `M.rs` next to the file, and prints `Wrote <full path> (Rust, N lines)`. It never replaces a file unless given `--force`. |
| `kstar export ... -o <path>` | Writes to that file, or into that folder. `-o -` prints the code to standard output instead. |
| `kstar export --format python <file> <M>` | The same, as `M.py`: a dependency-free `def m(word: str) -> bool` that checks itself on sample words when run as a script. |
| `kstar export --format json <file> <M>` | The same, as `M.json`: `M` (any `Dfa`, `Nfa`, `Regex`, `Grammar`, `Pda` or `Tm`) in [the JSON format](#the-json-format), with its name. |
| `kstar explain <code>` | Explains an error or warning code; `--list` lists them all. |
| `kstar repl` | Interactive prompt. A bare expression is printed; names can be redefined. |
| `kstar help [command]`, `kstar <command> --help` | Help, for one command or all. |
| `kstar version` | The version, the commit it was built from, the build date and the target platform. |
| `kstar update --check` | Asks GitHub whether a newer release exists (through the `gh` command, or `curl` with `GITHUB_TOKEN`), and whether the VS Code extension matches this `kstar`. |
| `kstar update` | The same, and if there is a newer release, shows both versions and what the release is about, then asks `Update to vX.Y.Z? [Y/n]`. On yes it installs the release into the folder `kstar` is in, VS Code extension included; on no it changes nothing. If `kstar` is up to date but the VS Code extension is missing or from another release, it installs the matching `.vsix`. `--no-vscode` or `KSTAR_NO_VSCODE=1` leaves VS Code alone. |
| `kstar update --yes` | Updates without asking. Without a terminal to ask on (a script or CI), `kstar update` refuses unless given `--yes`. |

`--color auto|always|never` works with `run`, `check` and `repl`. By default
errors, warnings and test results are coloured only when the output is a
terminal and the `NO_COLOR` environment variable is not set.

With several files, `kstar run` and `kstar check` name each file before its
output and end with a count of the files that passed and failed.

The exit code is 0 on success, 1 if there was an error or a failed test (or,
for `kstar fmt --check`, a file that would change), and 2 if the command itself
was wrong. Warnings do not change the exit code.

In the REPL, these commands are available besides statements and expressions:

| Command | What it does |
|---|---|
| `:type <expr>` | Shows the type of an expression without running it. |
| `:trace M <word>` | Short for `trace M on "<word>"`. |
| `:load <file>` | Runs a file in the REPL, so its definitions can be used. |
| `:reset` | Forgets everything defined so far. |
| `:help`, `:quit` | Help, and leave (Ctrl-D also leaves). |

Tab completes keywords, built-in functions and the names defined so far.
Earlier entries are kept in `~/.kstar_history`, so the up arrow reaches
entries from previous sessions too.

## Formatting

`kstar fmt` rewrites a program in one standard style:

| Rule | Example |
|---|---|
| Four spaces per level of `{ ... }`; one statement per line | |
| Single spaces around operators and after commas | `let x = (a + 1) * 2`, `print a, b` |
| No spaces around `..` | `for i in 0..n {` |
| `start` and `accept` line up | `start  q0` / `accept q1` |
| Transitions padded so `--` and `-->` line up | `a --0,1--> a` / `a --1  --> b` |
| Blank lines: at most one in a row, none just inside `{` or `}` | |
| Comments stay in place; trailing comments on neighbouring lines line up | |
| Parentheses, numbers, strings and regexes are kept exactly as written | |

Formatting is idempotent (formatting twice changes nothing more) and never
changes meaning: `kstar fmt` compares the tokens and comments before and after
and refuses to write if they differ. The file must parse; type errors do not
matter. Windows line endings are kept. A comment inside a statement that
spans several lines moves to just after that statement.

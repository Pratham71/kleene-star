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
             | 'draw' expr
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
| `kstar run --eager-imports ...` | Runs each imported module at its `import` line, instead of when one of its names is first used. |
| `kstar check <files>...` | Reports every error and warning without running. |
| `kstar fmt <files>...` | Rewrites files (or every file in a folder) in the standard style. See [formatting](#formatting). |
| `kstar fmt --check <files>...` | Lists the files that are not formatted, changing nothing. |
| `kstar fmt -` | Formats standard input to standard output, for editors and scripts. |
| `kstar new <name>` | Writes `<name>.kstar`, a small working program to start from. Never overwrites. |
| `kstar trace <file> <M> <word>` | Runs the file without showing its output, then traces machine `M` (any expression) on `word`. |
| `kstar export <file> <M> [--fn name] [-o out.rs] [--minimize]` | Runs the file without showing its output, then writes the regular language `M` as a standalone Rust function (see [`to_rust`](#generating-rust)). |
| `kstar export --format json <file> <M> [-o out.json]` | The same, but writes `M` (any `Dfa`, `Nfa`, `Regex`, `Grammar`, `Pda` or `Tm`) in [the JSON format](#the-json-format), with its name. |
| `kstar explain <code>` | Explains an error or warning code; `--list` lists them all. |
| `kstar repl` | Interactive prompt. A bare expression is printed; names can be redefined. |
| `kstar help [command]`, `kstar <command> --help` | Help, for one command or all. |
| `kstar version` | The version, the commit it was built from, the build date and the target platform. |
| `kstar update --check` | Asks GitHub whether a newer release exists (through the `gh` command, or `curl` with `GITHUB_TOKEN`). |
| `kstar update` | The same, and if there is one, prints the command that installs it. It does not replace itself. |

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

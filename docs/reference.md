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

### Alphabets in operations

- A `Dfa` or `Nfa` has a fixed alphabet. When an operation takes two machines,
  their alphabets must be the same.
- A `Regex` or `Set[Str]` is flexible. Combined with a machine, it uses the
  machine's alphabet. On its own, it uses the symbols it mentions.
- `machine accepts word` is an error if `word` has a symbol outside the
  machine's alphabet. `regex accepts word` is simply false in that case.

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
`dfa` `draw` `else` `eps` `false` `fn` `for` `grammar` `if` `in` `let` `nfa`
`not` `on` `or` `over` `pda` `print` `reject` `return` `start` `test` `tm`
`trace` `true`.

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

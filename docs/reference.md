# TOC-Lang reference

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
| `let x = value` | Names a value. |
| `fn name(a: T, b: U) -> R { ... }` | Defines a function. Top level only. |
| `return value` | Returns from a function. |
| `if cond { ... } else { ... }` | Chooses. `else` is optional; `else if` chains. |
| `for x in items { ... }` | Repeats for each item of a set, range, string or alphabet. |
| `print a, b, c` | Prints the values on one line, separated by spaces. |
| `test cond` | Checks a condition. Failures are reported and counted. |
| `draw machine` | Writes a Graphviz `.dot` file (and `.svg` if Graphviz is installed). |
| `trace machine on word` | Runs a `Dfa` or `Nfa` on a `Str` and prints every step. See [tracing](#tracing). |

### Names and scope

- A name cannot be redefined in the same block. Values never change.
- A block `{ ... }` may reuse a name from outside; the inner one hides the
  outer one until the block ends.
- A function sees its parameters and the top-level names defined above it. It
  does not see the local names of whoever calls it.
- Functions may call each other in any order, but a function must be defined
  above the top-level line that first calls it.

```toc
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
- Without a name, a machine is a value: `let m = nfa over B { ... }`.

Rules checked for a `dfa`:

- every state has a transition on every symbol of the alphabet,
- no state has two different transitions on the same symbol,
- no eps moves.

An `nfa` has none of these restrictions. Machines written out in full are
checked before the program runs; machines built with `for` or `name[expr]` are
checked when they are built.

## Tracing

`trace M on "0110"` runs `M` (a `Dfa` or `Nfa`) on the word and prints a
table with one row per symbol, then the result and the reason for it:

```toc
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
- A blank line follows each trace.

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
| `Set[T]` | a set of `Int`, `Str` or `Bool` | `{1, 2, 3}`, `{"a", "ab"}`, `0..5` |

Types are written only in function signatures; everywhere else they are worked
out for you. All type errors are found before the program runs.

A **language** is anything that describes a set of strings: a `Dfa`, an `Nfa`,
a `Regex`, or a `Set[Str]` (a finite language). Wherever this page says
"language", any of the four is accepted.

In a string, `\"` and `\\` are the only escapes.

## Operators

From loosest to tightest binding:

| Operators | Works on | Result |
|---|---|---|
| `or` | `Bool` | `Bool` |
| `and` | `Bool` | `Bool` |
| `not` | `Bool` | `Bool` |
| `==` `!=` | two values of the same type (not machines or regexes) | `Bool` |
| `<` `<=` `>` `>=` | `Int` | `Bool` |
| `accepts` | language, then `Str` | `Bool` |
| `..` | `Int` | `Set[Int]`: `a..b` is `a` up to but not including `b` |
| `+` `-` | `Int`; `+` also joins two `Str` | `Int` or `Str` |
| `*` `%` | `Int` | `Int` |
| `-` (prefix) | `Int` | `Int` |

Comparisons do not chain: write `a < b and b < c`. `and` and `or` skip the
right side when the left side already decides the answer. `%` is the remainder
and is never negative for a positive divisor. There is no division operator.

```toc
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
| `is_empty(a)` | `Bool` | Does `a` contain no strings? |
| `states(machine)` | `Int` | The number of states. |
| `len(x)` | `Int` | Length of a `Str`, or size of a `Set` or `Alphabet`. |

### Alphabets in operations

- A `Dfa` or `Nfa` has a fixed alphabet. When an operation takes two machines,
  their alphabets must be the same.
- A `Regex` or `Set[Str]` is flexible. Combined with a machine, it uses the
  machine's alphabet. On its own, it uses the symbols it mentions.
- `machine accepts word` is an error if `word` has a symbol outside the
  machine's alphabet. `regex accepts word` is simply false in that case.

## Errors

Errors found before running show the place and the problem. Errors while
running also show the chain of calls, most recent last.

| Error | Cause |
|---|---|
| `SyntaxError` | The text is not a valid program. |
| `NameError` | A name is unknown, defined twice, or used before its definition has run. |
| `TypeError` | A value of the wrong type, a wrong number of arguments, a function that might not return, or `trace` on something that is not a `Dfa` or `Nfa`. |
| `MachineError` | A machine breaks a rule: no start state, a DFA that is not total or not deterministic, an eps move in a DFA. |
| `AlphabetError` | A symbol outside the alphabet, or two machines over different alphabets. |
| `MathError` | Modulo by zero, a result too large for an `Int`, or a range that is too large. |
| `RecursionError` | More than 500 function calls in progress at once. |
| `IOError` | `draw` could not write its file. |

One example of each of the common ones:

```toc-error
let total = 1
let total = 2
```

```text
Error in demo.toc, line 2:
    let total = 2
        ^^^^^
NameError: `total` is already defined
  hint: values cannot be changed once defined; choose a new name
```

```toc-error
print len(42)
```

```text
Error in demo.toc, line 1:
    print len(42)
              ^^
TypeError: argument 1 of `len` must be a Str, Set or Alphabet, but this is an Int
```

```toc-error
alphabet B = {0, 1}
nfa M over B {
    start a
    a --x--> a
}
```

```text
Error in demo.toc, line 4:
    a --x--> a
        ^
AlphabetError: symbol `x` is not in alphabet `B`
  hint: B = {0, 1}
```

```toc-error
alphabet B = {0, 1}
dfa M over B {
    start a
    a --0,1--> a
    a --0--> b
    b --0,1--> b
}
```

```text
Error in demo.toc, line 5:
    a --0--> b
    ^^^^^^^^^^
MachineError: state `a` already goes to `a` on `0`
  hint: a DFA has one transition per state and symbol; use `nfa` if you want a choice
```

```toc-error
alphabet B = {0, 1}
dfa Any over B {
    start  q
    accept q
    q --0,1--> q
}
trace Any accepts "01"
```

```text
Error in demo.toc, line 7:
    trace Any accepts "01"
              ^^^^^^^
SyntaxError: expected `on` after the machine to trace, found `accepts`
  hint: write `trace M on "0110"`
```

```toc-error
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
             | ('dfa' | 'nfa') NAME machine
             | 'let' NAME '=' expr
             | 'fn' NAME '(' (param (',' param)*)? ')' '->' type block
             | 'return' expr
             | 'if' expr block ('else' (if-statement | block))?
             | 'for' NAME 'in' expr block
             | 'print' expr (',' expr)*
             | 'test' expr
             | 'draw' expr
             | 'trace' range 'on' expr

block       := '{' statement* '}'
param       := NAME ':' type
type        := 'Int' | 'Bool' | 'Str' | 'Alphabet' | 'Dfa' | 'Nfa' | 'Regex'
             | 'Set' '[' type ']'

machine     := 'over' NAME '{' item* '}'
item        := 'start' state
             | 'accept' state (',' state)*
             | state '--' label (',' label)* '-->' state
             | 'for' NAME 'in' expr '{' item* '}'
state       := NAME ('[' expr ']')?
label       := symbol | 'eps'
symbol      := one letter or digit

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
             | NAME
             | NAME '(' (expr (',' expr)*)? ')'
             | '(' expr ')'
             | '{' (expr (',' expr)*)? '}'
             | ('dfa' | 'nfa') machine
```

Keywords, which cannot be used as names: `accept` `accepts` `alphabet` `and`
`dfa` `draw` `else` `eps` `false` `fn` `for` `if` `in` `let` `nfa` `not` `on`
`or` `over` `print` `return` `start` `test` `trace` `true`.

## The `toc` command

| Command | What it does |
|---|---|
| `toc run file.toc` | Runs a program. `toc file.toc` is the same. |
| `toc run file.toc --out dir` | Runs it, writing drawings into `dir`. |
| `toc check file.toc` | Reports errors without running. |
| `toc fmt file.toc ...` | Rewrites files in the standard style. See [formatting](#formatting). |
| `toc fmt --check file.toc ...` | Lists the files that are not formatted, changing nothing. |
| `toc repl` | Interactive prompt. A bare expression is printed; names can be redefined. |
| `toc help`, `toc version` | Help and version. |

The exit code is 0 on success, 1 if there was an error or a failed test (or,
for `toc fmt --check`, a file that would change), and 2 if the command itself
was wrong.

## Formatting

`toc fmt` rewrites a program in one standard style:

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
changes meaning: `toc fmt` compares the tokens and comments before and after
and refuses to write if they differ. The file must parse; type errors do not
matter. Windows line endings are kept. A comment inside a statement that
spans several lines moves to just after that statement.

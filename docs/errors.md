# Errors and warnings

Every problem Kleene Star reports has a code, shown in brackets after its
kind:

```text
NameError[E0200]: `alphabet_sise` is not defined
```

The codes never change meaning, so you can search for them. `kstar explain
E0200` prints the section below about that code, and `kstar explain --list`
lists them all. Codes starting with `E` are errors, which stop the program;
codes starting with `W` are warnings, which do not.

`kstar run` and `kstar check` report every error in a file at once, not
only the first. After a syntax error, "not defined" errors are left out,
because the statement that did not parse may be the one that defines the
name.

Warnings are only looked for when a program has no errors. They point at
things that are allowed but probably mistakes; each section says how to
silence the warning when the code is intended.

The numbers are grouped: `E01..` syntax, `E02..` names, `E03..` types,
`E04..` machines and grammars, `E05..` alphabets, `E06..` problems while
running, and `W..` warnings.

## E0100: Unexpected text

The parser expected one thing and found another. The message says what it
expected; the caret points at what it found instead.

```kstar-error
alphabet B = {0, 1}
print B accepts
```

```text
Error in demo.kstar, line 2:
    print B accepts
            ^^^^^^^
SyntaxError[E0100]: expected a value after `accepts`, but the file ends here
  hint: finish the expression on this line
```

## E0101: Something is never closed

A `{` without its `}`, or a string or regex that reaches the end of the
line without its closing `"` or `/`. Strings and regexes cannot span lines.
For a `{`, the error points at the brace that is still open, which may be
far above where the file ends.

```kstar-error
alphabet B = {0, 1}
dfa M over B {
    start q0
    accept q0
    q0 --0,1--> q0
```

```text
Error in demo.kstar, line 2:
    dfa M over B {
                 ^
SyntaxError[E0101]: this `{` is never closed
  hint: add the missing `}`; the block that starts here runs to the end of the file
```

## E0102: Assignment needs `let`

Values are defined once with `let` and never changed, so there is no plain
`x = ...`. To get a different value, define a new name.

```kstar-error
x = 5
```

```text
Error in demo.kstar, line 1:
    x = 5
    ^
SyntaxError[E0102]: assignment needs `let`
  hint: write `let x = ...`; values cannot be changed once defined, so pick a new name
```

## E0103: A keyword used as a name

Words such as `start`, `accept`, `test` and `over` belong to the language
and cannot name a value, machine or state. Pick another name; for states,
`q0`, `s`, `go` and `done` are common choices.

```kstar-error
let test = 1
```

```text
Error in demo.kstar, line 1:
    let test = 1
        ^^^^
SyntaxError[E0103]: expected a name after `let`, found `test`
  hint: `test` is a keyword and cannot be used as a name
```

## E0104: Text that is not part of the language

A character the language does not use, such as `;` or `&`, a number too
large for an Int, or an unknown escape in a string. Kleene Star has no
semicolons; `not`, `and` and `or` are words.

```kstar-error
let x = 1;
```

```text
Error in demo.kstar, line 1:
    let x = 1;
             ^
SyntaxError[E0104]: unexpected `;`
  hint: Kleene Star has no semicolons; just end the line
```

## E0105: A regex that cannot be read

A regex literal between slashes must be a well-formed regular expression:
symbols, `|` for "or", `*` for "any number of times", and parentheses for
grouping. `eps` is written inside a regex as `()`.

```kstar-error
let r = /(0|1*/
```

```text
Error in demo.kstar, line 1:
    let r = /(0|1*/
             ^
SyntaxError[E0105]: in regex: this `(` is never closed
  hint: a regex is made of symbols, `|` for or, `*` for repeat, and `( )` for grouping, as in /(0|1)*01/
```

## E0106: A function inside a block

Functions can only be defined at the top level of a file, not inside an
`if`, a `for` or another function. A function can be called from anywhere,
so move it out of the block.

```kstar-error
if true {
    fn one() -> Int {
        return 1
    }
}
```

```text
Error in demo.kstar, line 2:
    fn one() -> Int {
    ^^
SyntaxError[E0106]: functions can only be defined at the top level
  hint: move this `fn` out of the block; a function can be called from anywhere in the file
```

## E0107: `eps` in the wrong place

`eps` is the empty string. It can label a move of an NFA or PDA, or stand
alone as a grammar alternative, but it is not a symbol: it cannot be in an
alphabet, be mixed with symbols in a grammar alternative, or be read or
written by a Turing machine.

```kstar-error
alphabet B = {0, eps}
```

```text
Error in demo.kstar, line 1:
    alphabet B = {0, eps}
                     ^^^
SyntaxError[E0107]: `eps` is not a symbol
  hint: eps is the empty string; it cannot be in an alphabet
```

## E0108: A symbol written the wrong way

Symbols are single characters. In a grammar, terminals are written one at a
time with spaces between (`a S b`, not `aSb`), and only names starting with
a capital letter are variables that can have rules. In a PDA, `_` cannot be
a stack symbol.

```kstar-error
alphabet AB = {a, b}
grammar G over AB {
    S -> aSb | eps
}
```

```text
Error in demo.kstar, line 3:
    S -> aSb | eps
         ^^^
SyntaxError[E0108]: `aSb` is not one symbol
  hint: write terminals one at a time with spaces between: `a S b`; variables start with a capital letter
```

## E0109: A definition without a name

`dfa`, `nfa`, `pda`, `tm` and `grammar` statements give the thing they
define a name. Without one, write it as a value instead:
`let m = dfa over B { ... }`.

```kstar-error
alphabet B = {0, 1}
dfa over B {
    start q0
    accept q0
    q0 --0,1--> q0
}
```

```text
Error in demo.kstar, line 2:
    dfa over B {
        ^^^^
SyntaxError[E0109]: a `dfa` statement needs a name
  hint: write `dfa Name over B { ... }`, or `let m = dfa over B { ... }`
```

## E0110: `return` outside a function

`return` ends a function and gives its result, so it only makes sense
inside one. At the top level, use `print` to show a value.

```kstar-error
return 1
```

```text
Error in demo.kstar, line 1:
    return 1
    ^^^^^^^^
SyntaxError[E0110]: `return` can only be used inside a function
  hint: at the top level, use `print` to show a value
```

## E0111: A part only another kind of machine has

Each kind of machine is written a little differently: only a Turing machine
has `reject` states; a PDA's moves read, pop and push (`a, Z/AZ`); a Turing
machine's moves read, write and move (`0/1,R`).

```kstar-error
alphabet B = {0, 1}
dfa M over B {
    start s
    reject s
    s --0,1--> s
}
```

```text
Error in demo.kstar, line 4:
    reject s
    ^^^^^^
SyntaxError[E0111]: only a Turing machine has `reject` states, and this is a `dfa`
  hint: a finite automaton rejects every word that does not end in an accepting state
```

## E0112: The formatter would change the program

`kstar fmt` checks its own work: it formats the file, reads the result back,
and refuses to save anything that would mean something different. Seeing
this error means `kstar fmt` has a bug. The file is left alone; please
report the bug together with the file.

## E0200: A name that is not defined

The name is used but never defined, or defined further down the file.
Names must be defined before the line that uses them. A close match, which
is usually a typo, is suggested.

```kstar-error
alphabet B = {0, 1}
print len(alphabet_b)
```

```text
Error in demo.kstar, line 2:
    print len(alphabet_b)
              ^^^^^^^^^^
NameError[E0200]: `alphabet_b` is not defined
```

## E0201: A name defined twice

Every name is defined once. Values never change, so instead of redefining a
name, choose a new one. (The REPL is the exception: there, redefining a
top-level name replaces it.)

```kstar-error
let x = 1
let x = 2
```

```text
Error in demo.kstar, line 2:
    let x = 2
        ^
NameError[E0201]: `x` is already defined
  hint: values cannot be changed once defined; choose a new name
```

## E0202: A call to a function that does not exist

The name before `(` is not a built-in function or one defined with `fn`. A
close match is suggested; the built-ins use the American spelling, as in
`minimize`.

```kstar-error
alphabet B = {0, 1}
print minimise(/(0|1)*/)
```

```text
Error in demo.kstar, line 2:
    print minimise(/(0|1)*/)
          ^^^^^^^^
NameError[E0202]: there is no function called `minimise`
  hint: did you mean `minimize`?
```

## E0203: A function with the name of a built-in

Built-in functions such as `union` and `len` cannot be redefined. Give the
new function a name of its own.

```kstar-error
fn len(s: Str) -> Int {
    return 0
}
```

```text
Error in demo.kstar, line 1:
    fn len(s: Str) -> Int {
       ^^^
NameError[E0203]: `len` is a built-in function and cannot be redefined
  hint: choose another name, such as `my_len`
```

## E0204: Used before it is defined, while running

A function may use a global value or call another function defined further
down the file, but only once that definition has run. This error appears
when the function is called too early: move the call below the definition.

```kstar-error
alphabet B = {0, 1}
print size()
fn size() -> Int {
    return len(B)
}
```

```text
Traceback (most recent call last):
  demo.kstar, line 2, in <main>
    print size()
          ^^^^
NameError[E0204]: function `size` is called before it is defined
  hint: move `fn size` above this line
```

## E0205: A grammar variable without rules

Every variable used on the right of `->` needs at least one rule of its own;
otherwise it cannot produce anything.

```kstar-error
alphabet AB = {a, b}
grammar G over AB {
    S -> a T b | eps
}
```

```text
Error in demo.kstar, line 3:
    S -> a T b | eps
           ^
NameError[E0205]: variable `T` has no rules
  hint: add a rule such as `T -> ...`
```

## E0300: A value of the wrong type

A value is used where another type is needed: an argument, the condition of
an `if`, a `test`, a returned value, or a name after `over`. The message
says which type was needed and which was found. Types are never converted
automatically.

```kstar-error
fn double(n: Int) -> Int {
    return 2 * n
}
print double("4")
```

```text
Error in demo.kstar, line 4:
    print double("4")
                 ^^^
TypeError[E0300]: argument 1 of `double` must be an Int, but this is a Str
```

## E0301: The wrong number of arguments

A function was called with more or fewer arguments than it takes. The
reference lists every built-in with its arguments.

```kstar-error
alphabet B = {0, 1}
print union(/0*/)
```

```text
Error in demo.kstar, line 2:
    print union(/0*/)
          ^^^^^^^^^^^
TypeError[E0301]: `union` takes 2 arguments, but 1 was given
```

## E0302: An operator used on the wrong types

`+` adds Ints or joins Strs, `-`, `*`, `%`, `<` and `..` need Ints, `and`
and `or` need Bools, and `accepts` needs a language on the left and a Str
on the right. Languages are combined with functions such as `union`, not
with `+`.

```kstar-error
let n = 3
print "n = " + n
```

```text
Error in demo.kstar, line 2:
    print "n = " + n
          ^^^^^^^^^^
TypeError[E0302]: `+` needs two Ints or two Strs, but the left side is a Str and the right side is an Int
  hint: an Int is not turned into a Str by itself; `print` can show both: `print "n =", n`
```

## E0303: A type that does not exist

The types are `Int`, `Bool`, `Str`, `Alphabet`, `Dfa`, `Nfa`, `Pda`, `Tm`,
`Regex`, `Grammar` and `Set[T]`, all starting with a capital letter.

```kstar-error
fn twice(n: int) -> Int {
    return 2 * n
}
```

```text
Error in demo.kstar, line 1:
    fn twice(n: int) -> Int {
                ^^^
TypeError[E0303]: unknown type `int`
  hint: types start with a capital letter: `Int`
```

## E0304: A function that may not return

Every path through a function must end in `return`. Usually an `if` has a
`return` but its `else` does not, or there is no `else`.

```kstar-error
fn sign(n: Int) -> Int {
    if n > 0 {
        return 1
    }
}
```

```text
Error in demo.kstar, line 1:
    fn sign(n: Int) -> Int {
       ^^^^
TypeError[E0304]: function `sign` can reach its end without returning a value
  hint: every path must end in `return`; it should return an Int
```

## E0305: A set that cannot be built

A set holds Ints, Strs or Bools, all of one type. Machines and regexes
cannot go in a set; to combine languages, use `union`.

```kstar-error
print {1, "two"}
```

```text
Error in demo.kstar, line 1:
    print {1, "two"}
              ^^^^^
TypeError[E0305]: every item of a set must have the same type: the first is an Int, but this is a Str
```

## E0306: A question no program can answer

Some questions about languages are undecidable: no algorithm answers them
for every input. Whether two grammars, PDAs or Turing machines describe the
same language is one of them, so `equivalent` refuses them. Compare them on
sample words with `accepts` instead.

```kstar-error
alphabet AB = {a, b}
grammar G over AB {
    S -> a S b | eps
}
print equivalent(G, G)
```

```text
Error in demo.kstar, line 5:
    print equivalent(G, G)
                     ^
TypeError[E0306]: `equivalent` cannot compare context-free languages, and this is a Grammar
  hint: whether two grammars or PDAs describe the same language is undecidable: no algorithm can answer it for every pair. Compare them on sample words with `accepts` instead
```

## E0307: Comparing machines with `==`

`==` compares Ints, Strs, Bools and sets. Two machines or regexes can be
different and still describe the same language, so to ask whether they do,
use `equivalent(a, b)`.

```kstar-error
print /0*/ == /0*/
```

```text
Error in demo.kstar, line 1:
    print /0*/ == /0*/
          ^^^^^^^^^^^^
TypeError[E0307]: `==` cannot compare machines or regexes
  hint: to ask whether two of them describe the same language, use `equivalent(a, b)`
```

## E0400: A machine that breaks the rules

A machine definition is checked before the program runs. The codes below
cover the common cases; this one covers the rest.

## E0401: A machine without a start state

Every machine needs exactly one `start` line.

```kstar-error
alphabet B = {0, 1}
dfa M over B {
    accept q0
    q0 --0,1--> q0
}
```

```text
Error in demo.kstar, line 2:
    dfa M over B {
    ^^^^^^^^^^^^
MachineError[E0401]: DFA `M` has no start state
  hint: add a line such as `start q0`
```

## E0402: Two start states

A machine has exactly one start state. An NFA that should begin in several
states can start in a new state with eps moves to each of them.

```kstar-error
alphabet B = {0, 1}
nfa M over B {
    start a
    start b
    accept b
    a --0--> b
}
```

```text
Error in demo.kstar, line 4:
    start b
          ^
MachineError[E0402]: NFA `M` already has the start state `a`, so `b` cannot be one too
  hint: a machine has exactly one start state; an NFA can branch from it with eps moves
```

## E0403: An eps move in a DFA

A DFA reads one symbol on every move, so it has no eps moves. Declare the
machine with `nfa` instead, and convert it with `to_dfa` if you need a DFA.

```kstar-error
alphabet B = {0, 1}
dfa M over B {
    start a
    accept b
    a --eps--> b
    a --0,1--> a
    b --0,1--> b
}
```

```text
Error in demo.kstar, line 5:
    a --eps--> b
        ^^^
MachineError[E0403]: a DFA cannot have eps transitions
  hint: eps moves need nondeterminism; declare the machine with `nfa` instead
```

## E0404: A DFA without a move for every symbol

A DFA must have a move from every state on every symbol of its alphabet.
Words that should be rejected usually go to a dead state that loops to
itself. Or declare the machine with `nfa`, where missing moves simply
reject.

```kstar-error
alphabet B = {0, 1}
dfa M over B {
    start q0
    accept q0
    q0 --0--> q0
}
```

```text
Error in demo.kstar, line 2:
    dfa M over B {
    ^^^^^^^^^^^^
MachineError[E0404]: DFA `M` is not total: no transition from `q0` on `1`
  hint: a DFA needs a transition from every state on every symbol; add the missing ones (often to a dead state), or use `nfa`
```

## E0405: Two different moves for the same state and symbol

A DFA, and a Turing machine here, are deterministic: at most one move per
state and symbol. Use an NFA when a machine should have a choice.

```kstar-error
alphabet B = {0, 1}
dfa M over B {
    start q0
    accept q1
    q0 --0--> q0
    q0 --0--> q1
    q0 --1--> q0
    q1 --0,1--> q1
}
```

```text
Error in demo.kstar, line 6:
    q0 --0--> q1
    ^^^^^^^^^^^^
MachineError[E0405]: state `q0` already goes to `q0` on `0`
  hint: a DFA has one transition per state and symbol; use `nfa` if you want a choice
```

## E0406: The blank in a Turing machine's alphabet

`_` is the blank that fills the rest of a Turing machine's tape, so it
cannot also be an input symbol. Leave it out of the alphabet; the machine
can still read and write it.

```kstar-error
alphabet B = {0, _}
tm M over B {
    start s
    accept s
}
```

```text
Error in demo.kstar, line 2:
    tm M over B {
    ^^^^^^^^^^^
MachineError[E0406]: TM `M` is over an alphabet containing `_`, which is the blank
  hint: the input cannot contain blanks; leave `_` out of the alphabet
```

## E0407: A state that both accepts and rejects

A Turing machine halts as soon as it enters an accepting or a rejecting
state, so no state can be both.

```kstar-error
alphabet B = {0, 1}
tm M over B {
    start s
    accept s
    reject s
}
```

```text
Error in demo.kstar, line 5:
    reject s
           ^
MachineError[E0407]: state `s` cannot both accept and reject
  hint: a Turing machine halts in an accepting or a rejecting state; pick one for this state
```

## E0408: A grammar with no rules

A grammar needs at least one rule. The variable of the first rule is the
start variable.

```kstar-error
alphabet AB = {a, b}
grammar G over AB {
}
```

```text
Error in demo.kstar, line 2:
    grammar G over AB {
    ^^^^^^^^^^^^^^^^^
MachineError[E0408]: grammar `G` has no rules
  hint: add a rule such as `S -> a S b | eps`; the first rule's variable is the start variable
```

## E0409: A negative state index

In a machine built with loops, `q[i]` names the state `q` followed by the
value of `i`, which must not be negative. `%` always gives a result from 0
upwards, which makes it the safe way to wrap around.

```kstar-error
alphabet B = {0, 1}
fn ring(n: Int) -> Dfa {
    return dfa over B {
        start q[0]
        accept q[0]
        for i in 0..n {
            q[i] --0--> q[i - 1]
            q[i] --1--> q[i]
        }
    }
}
print ring(3)
```

```text
Traceback (most recent call last):
  demo.kstar, line 12, in <main>
    print ring(3)
  demo.kstar, line 7, in ring
    q[i] --0--> q[i - 1]
                  ^^^^^
MachineError[E0409]: state index is negative (-1)
  hint: `%` always gives a result from 0 upwards, so `(i - 1) % n` wraps around safely
```

## E0500: A symbol that is not in the alphabet

A machine or grammar uses a symbol that its alphabet (the one after `over`)
does not contain. Either the symbol is a typo or the alphabet is missing it.

```kstar-error
alphabet B = {0, 1}
dfa M over B {
    start q0
    accept q0
    q0 --0,2--> q0
    q0 --1--> q0
}
```

```text
Error in demo.kstar, line 5:
    q0 --0,2--> q0
           ^
AlphabetError[E0500]: symbol `2` is not in alphabet `B`
  hint: B = {0, 1}
```

## E0501: A word with symbols the machine does not know

The word given to `accepts`, `trace` or `run` contains a symbol outside the
machine's alphabet. A machine cannot read such a symbol, so instead of
answering "no", this is an error.

```kstar-error
alphabet B = {0, 1}
dfa Even over B {
    start q0
    accept q0
    q0 --0--> q0
    q0 --1--> q1
    q1 --0--> q1
    q1 --1--> q0
}
print Even accepts "102"
```

```text
Traceback (most recent call last):
  demo.kstar, line 10, in <main>
    print Even accepts "102"
                       ^^^^^
AlphabetError[E0501]: the word contains `2`, which is not in the machine's alphabet
  hint: the machine is over {0, 1}
```

## E0502: Languages over different alphabets

Operations such as `union`, `intersect` and `equivalent` combine two
languages, which must share an alphabet. Build both machines `over` the
same alphabet.

```kstar-error
alphabet B = {0, 1}
alphabet L = {a, b}
print union(to_dfa(/0*/, B), to_dfa(/a*/, L))
```

```text
Traceback (most recent call last):
  demo.kstar, line 3, in <main>
    print union(to_dfa(/0*/, B), to_dfa(/a*/, L))
                                 ^^^^^^^^^^^^^^^
AlphabetError[E0502]: this machine is over {a, b}, but {0, 1} is needed here
  hint: languages can only be combined when they share an alphabet; build both machines `over` the same one
```

## E0503: A symbol listed twice

An alphabet is a set of symbols, so each one is listed once.

```kstar-error
alphabet B = {0, 1, 0}
```

```text
Error in demo.kstar, line 1:
    alphabet B = {0, 1, 0}
                        ^
AlphabetError[E0503]: symbol `0` is listed twice
  hint: an alphabet is a set: list each symbol once
```

## E0600: A number too large

Ints are 64-bit: they hold whole numbers from about -9 * 10^18 to
9 * 10^18. A calculation that goes past that stops with this error rather
than giving a wrong answer.

```kstar-error
let big = 9223372036854775807
print big + 1
```

```text
Traceback (most recent call last):
  demo.kstar, line 2, in <main>
    print big + 1
          ^^^^^^^
MathError[E0600]: the result is too large for an Int
  hint: Ints are 64-bit: about plus or minus 9 * 10^18
```

## E0601: Modulo by zero

`a % b` is the remainder after dividing `a` by `b`, which has no meaning
when `b` is 0.

```kstar-error
fn rest(n: Int) -> Int {
    return 10 % n
}
print rest(0)
```

```text
Traceback (most recent call last):
  demo.kstar, line 4, in <main>
    print rest(0)
  demo.kstar, line 2, in rest
    return 10 % n
           ^^^^^^
MathError[E0601]: modulo by zero
  hint: the right-hand side of `%` is 0 here
```

## E0602: A range too large

`a..b` is the set of Ints from `a` up to (not including) `b`. To keep
programs from running out of memory, a range holds at most 1000000 numbers.

```kstar-error
for i in 0..5000000 {
    print i
}
```

```text
Traceback (most recent call last):
  demo.kstar, line 1, in <main>
    for i in 0..5000000 {
             ^^^^^^^^^^
MathError[E0602]: the range 0..5000000 is too large (limit 1000000 items)
  hint: loop over a smaller range, or work with the language directly instead of listing its words
```

## E0603: Too many calls at once

More than 500 function calls were in progress at the same time. Usually a
recursive function never reaches the case that stops it.

```kstar-error
fn down(n: Int) -> Int {
    return down(n - 1)
}
print down(3)
```

```text
Traceback (most recent call last):
  demo.kstar, line 4, in <main>
    print down(3)
  demo.kstar, line 2, in down
    return down(n - 1)
  demo.kstar, line 2, in down
    return down(n - 1)
  demo.kstar, line 2, in down
    return down(n - 1)
  demo.kstar, line 2, in down
    return down(n - 1)
  ... 490 more calls ...
  demo.kstar, line 2, in down
    return down(n - 1)
  demo.kstar, line 2, in down
    return down(n - 1)
  demo.kstar, line 2, in down
    return down(n - 1)
  demo.kstar, line 2, in down
    return down(n - 1)
  demo.kstar, line 2, in down
    return down(n - 1)
           ^^^^^^^^^^^
RecursionError[E0603]: more than 500 calls are in progress at once
  hint: does `down` call itself without ever reaching a base case?
```

## E0604: A Turing machine that does not halt

`accepts` and `run` stop a Turing machine after 100000 steps. It may never
halt, and by the halting problem no program can decide in general whether
it will, so this is reported instead of waiting forever. `trace` shows the
first steps, which usually make a loop easy to see.

```kstar-error
alphabet B = {0, 1}
tm Forever over B {
    start walk
    accept done
    walk --0/0,R--> walk
    walk --1/1,R--> walk
    walk --_/_,R--> walk
}
print Forever accepts "01"
```

```text
Traceback (most recent call last):
  demo.kstar, line 9, in <main>
    print Forever accepts "01"
                          ^^^^
LimitError[E0604]: the Turing machine is still running after 100000 steps
  hint: it may never halt. By the halting problem, no program can decide in general whether a Turing machine halts, so Kleene Star stops here instead of waiting forever
```

## E0605: A search that gave up

Some questions are answered by searching: whether a PDA has an accepting
run, or how a grammar derives a word. When a search grows too large it
stops with this error. The answer is unknown, not "no". A PDA that pushes
without reading can make the search endless; for grammars, `accepts` uses a
faster method than `derive`.

```kstar-error
alphabet AB = {a, b}
pda Forever over AB {
    start p
    accept q
    p --eps, eps/A--> p
    p --a, A/eps--> q
}
print Forever accepts "ab"
```

```text
Traceback (most recent call last):
  demo.kstar, line 8, in <main>
    print Forever accepts "ab"
                          ^^^^
LimitError[E0605]: gave up after looking at 50000 configurations of the PDA without finding an accepting run
  hint: eps moves that push without reading can let a PDA run forever; the answer is unknown, not "no"
```

## E0606: A word the grammar cannot derive

`derive(g, w)` shows how the grammar produces `w`, so `w` must be in the
grammar's language. Check first with `g accepts w`.

```kstar-error
alphabet AB = {a, b}
grammar G over AB {
    S -> a S b | eps
}
print derive(G, "aab")
```

```text
Traceback (most recent call last):
  demo.kstar, line 5, in <main>
    print derive(G, "aab")
                    ^^^^^
ValueError[E0606]: the grammar cannot derive "aab"
  hint: check first with `g accepts "..."`; only words in the language have a derivation
```

## E0607: A file that cannot be written

`draw` writes a `.dot` file (and, with Graphviz installed, an `.svg`) into
the current folder or the one given with `kstar run --out`. The folder must
exist and be writable.

## W0100: A state that can never be reached

No sequence of moves from the start state leads to this state, so it does
not change what the machine accepts. Usually a transition into it is
missing. A machine that can never accept at all is left alone, since that
is usually deliberate.

```kstar
alphabet B = {0, 1}
dfa M over B {
    start q0
    accept q0
    q0 --0,1--> q0
    lost --0,1--> q0
}
print M accepts "01"
```

```text
Warning in demo.kstar, line 6:
    lost --0,1--> q0
    ^^^^
Warning[W0100]: state `lost` can never be reached from the start state `q0`
  hint: add a transition into it, or remove it; it does not change the language

true
```

## W0101: A state from which nothing is accepted

Every word that reaches this state is rejected. A DFA needs one such
*dead state* to send rejected words to, so a state that loops to itself on
every symbol is not reported. Any other state like this is usually a
mistake: a missing `accept`, or a transition to the wrong state.

```kstar
alphabet B = {0, 1}
nfa M over B {
    start a
    accept b
    a --0--> b
    a --1--> stuck
    stuck --0--> stuck
}
print M accepts "0"
```

```text
Warning in demo.kstar, line 6:
    a --1--> stuck
             ^^^^^
Warning[W0101]: no accepting state can be reached from state `stuck`
  hint: every path through this state is rejected; an NFA does not need a dead state, so this one can go

true
```

## W0200: A value that is never used

A `let` defines a value that nothing uses. Remove it, or if you want to
keep it, start its name with `_`, which tells Kleene Star it is unused on
purpose.

```kstar
let answer = 42
let _draft = 7
print "done"
```

```text
Warning in demo.kstar, line 1:
    let answer = 42
        ^^^^^^
Warning[W0200]: value `answer` is never used
  hint: remove it, or start its name with `_` (`_answer`) to keep it quietly

done
```

## W0201: A function that is never called

The function is defined but never called. Remove it, or start its name with
`_` to keep it quietly.

```kstar
fn helper(n: Int) -> Int {
    return n + 1
}
print "done"
```

```text
Warning in demo.kstar, line 1:
    fn helper(n: Int) -> Int {
       ^^^^^^
Warning[W0201]: function `helper` is never used
  hint: remove it, or start its name with `_` (`_helper`) to keep it quietly

done
```

## W0300: A test that always gives the same answer

The condition of this `test` uses only literal numbers, strings and
booleans, so it passes or fails no matter what the rest of the program
does. A test is for checking something that could go wrong, such as a
machine on a word.

```kstar
test 2 + 2 == 4
```

```text
Warning in demo.kstar, line 1:
    test 2 + 2 == 4
         ^^^^^^^^^^
Warning[W0300]: this test always gives the same answer: it uses no names, calls or regexes
  hint: a test should check something that could go wrong, like `test M accepts "0110"`

tests: 1 passed, 0 failed
```

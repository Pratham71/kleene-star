# TOC-Lang

A small language for Theory of Computation. You describe machines the way you
draw them on paper, then ask questions about them.

```toc
alphabet B = {0, 1}

nfa EndsIn01 over B {
    start  a
    accept c
    a --0,1--> a
    a --0-->   b
    b --1-->   c
}

let d = minimize(EndsIn01)

print d
test d accepts "1101"
test equivalent(d, /(0|1)*01/)
```

```text
DFA with 3 states over {0, 1}
          | 0     | 1
->  {a}   | {a,b} | {a}
    {a,b} | {a,b} | {a,c}
   *{a,c} | {a,b} | {a}
tests: 2 passed, 0 failed
```

## What it can do

- **Define** DFAs, NFAs, eps-NFAs, regular expressions and finite languages.
- **Test** which strings a language accepts.
- **Convert** between them: regex to eps-NFA to NFA to DFA to minimal DFA, and back to a regex.
- **Combine** languages: union, intersection, difference, complement, concatenation, star, reversal.
- **Compare** languages: equivalence and emptiness.
- **Draw** state diagrams as Graphviz files.
- **Catch mistakes early**: a DFA that is missing a transition, a symbol outside
  the alphabet, or a type mix-up is reported before the program runs, with the
  exact place underlined and a hint.

## Getting started

You need [Rust](https://rustup.rs). Then, in this folder:

```bash
cargo build --release
```

```bash
cargo run -- run examples/tour.toc
```

```bash
cargo run -- repl
```

`toc run file.toc` runs a program, `toc check file.toc` only looks for errors,
`toc fmt file.toc` tidies its layout, and `toc repl` gives you an interactive
prompt. The exit code is 0 when there
were no errors and no failed tests.

## Learning the language

1. [Deterministic machines](docs/tutorial/01-dfa.md)
2. [Nondeterminism and eps moves](docs/tutorial/02-nfa.md)
3. [Regular expressions](docs/tutorial/03-regex.md)
4. [Combining and comparing languages](docs/tutorial/04-closure.md)
5. [Functions and machine families](docs/tutorial/05-functions.md)
6. [Watching a machine run](docs/tutorial/06-trace.md)

Appendix: [tools](docs/tutorial/tools.md), including `toc fmt`.

Then keep [the reference](docs/reference.md) open: every statement, type,
built-in function and error on one page.

If you want to know how the interpreter itself works, read
[internals](docs/internals.md). The design decisions are in
[design](docs/design.md).

## When something goes wrong

Errors point at the problem and usually suggest a fix:

```toc-error
alphabet B = {0, 1}

dfa Broken over B {
    start  q0
    accept q1
    q0 --0--> q1
    q1 --0,1--> q1
}
```

```text
Error in demo.toc, line 3:
    dfa Broken over B {
    ^^^^^^^^^^^^^^^^^
MachineError: DFA `Broken` is not total: no transition from `q0` on `1`
  hint: a DFA needs a transition from every state on every symbol; add the missing ones (often to a dead state), or use `nfa`
```

Errors that happen while running show the chain of calls, most recent last:

```text
Traceback (most recent call last):
  demo.toc, line 20, in <main>
    test smallest(0) accepts "110"
  demo.toc, line 16, in smallest
    return minimize(div_by(n))
  demo.toc, line 9, in div_by
    q[i] --0--> q[(2 * i) % n]
                  ^^^^^^^^^^^
MathError: modulo by zero
  hint: the right-hand side of `%` is 0 here
```

## Project layout

```text
src/automata/   the machines and algorithms, as a plain Rust library
src/lang/       the language: lexer, parser, checker, interpreter
src/main.rs     the `toc` command
examples/       programs to read and run
tests/          golden tests for output and error messages
docs/           tutorial, reference, internals, design
```

Run the tests with `cargo test`. They include every example and every code
sample in the documentation, so the docs cannot fall out of date unnoticed.

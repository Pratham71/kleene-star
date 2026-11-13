# Kleene Star: a small, well-designed language for Theory of Computation

> Status: version 0.2.5-beta (2026-11-13), MIT licensed. Stages 0 to 9 below are built, and so is most of
> the later list: `kstar fmt`, grammars and PDAs, Turing machines, a language server with
> a VS Code extension, and release builds with installers for Windows, Linux and macOS.
> Version 0.3 (in progress) adds imports between files, `export` to Rust and a standard
> library. All three are on `main` and in the 0.2.5-beta pre-release; `std.math` is the first library module, and
> `std.string` and `std.collections` come next. This is the design as agreed on 2026-11-10,
> with the layout, error format and test sections updated to match the code.

## Context

A for-fun project: a language for describing regular languages and their machines and asking
questions about them. The project folder is empty and no earlier notes exist (local memory,
other projects' memory, past session transcripts and Notion were all searched), so the
remembered "compiles down to Rust" idea is treated as a leaning, not a decision.

What the user asked for: simple to understand, good programming-language fundamentals
(readability, clear syntax structure), inspired by other languages, Python-like error traces,
fast and safe like Rust, good docs. Start with regular languages; add grammars, PDAs and
Turing machines later.

## Decisions

Decided by the user:
- A program can **define and test** machines, **transform** them, **draw** them, and
  **compare** languages.
- **Simple first, both goals**: a working tool first, clean enough to add a compile backend later.
- **Arrow syntax** for machines.
- **Functions and types**: user-defined functions, static types with inference, `if/else`,
  `for` loops. No structs or pattern matching. (Modules were left out at first; v0.3 adds
  them, see below.)
- **Python-like tracebacks**, **Rust-like speed and safety**, **good docs**.
- Claude writes the code in small readable stages; the user guides and learns along.

My calls (open to change):
- **Execution model: tree-walking interpreter**, written in Rust. Scripts are glue; the heavy
  work (subset construction, minimisation, equivalence) is native Rust, so it runs at Rust
  speed either way. "Compile to Rust" comes later as an `export` stage that turns a DFA into a
  standalone Rust matcher function.
- One Cargo package, hand-written lexer and recursive-descent parser (the learning value).
  Only dependency: `rustyline` for the REPL.
- **"Safe" means caught before running**: a type checker runs over the whole program first.
  No null, values immutable (`let` only, no reassignment), a DFA must be total and
  deterministic over its alphabet, every symbol must belong to the alphabet. The interpreter
  itself never panics; every failure is a reported error.
- **Integers and indexed states** (`q[i]`) exist so functions can build machine families
  (e.g. "divisible by n").
- **Drawing** writes Graphviz DOT (`name.dot`), rendered to SVG only if Graphviz is installed.
  `print` of a machine shows a transition table.
- File extension `.kstar`; binary `kstar`. (Until v0.2 the project was called TOC-Lang, with
  `.toc` files and a `toc` binary; it is now Kleene Star, after the star in `a*`.)
- **"min-NFA"**: true NFA minimisation is PSPACE-hard, so `reduce(nfa)` merges equivalent
  states (smaller, not guaranteed smallest). `minimize` is exact and DFA-only.
- **Finite languages** are a set of strings, `{"a", "ab"}`, usable wherever a language is expected.

### Modules (v0.3, agreed 2026-11-13)

Decided by the user:
- `import "file.kstar" [as name]` binds a module as a namespace; `from "file.kstar" import a,
  b as c` takes single names. No `import *`. Paths are relative to the importing file.
- Qualified names (`utils.B`) work wherever a name does, including `over`, `trace` and `draw`.
- Everything a module defines at its top level is public; there is no re-exporting.
- Loading is **safe**: a module's top-level `print`, `test`, `draw` and `trace` are checked
  but not run. It is **lazy**: checked up front, run at the first use of one of its names
  (`--eager-imports` runs it at the import). Each module runs once and keeps its own globals.
- Circular imports are an error showing the chain.
- `std` is the standard library, always reachable by its full name (`std.math.gcd`) and
  importable like a file. No list type yet.

My calls:
- A qualified name is one `Name` whose text keeps the dots, rather than a new kind of syntax
  node, so every place that takes a name takes a qualified one for free; the parser requires
  the dots to touch their neighbours, so each part's position follows from the text.
- The loader fills each import statement's `target` before checking, so the checker and the
  interpreter never see a path. A module's interface is a table of types (`Exports`) read off
  its checker; the standard library supplies the same table.
- `import std.math` with no `as` binds `math`, the module's own name, the way a file is bound
  by its file name.
- Errors inside an imported file are reported with the file; warnings about it are not (they
  appear when that file is checked by itself).
- The standard library is a table of functions (`src/lang/stdlib/`), each with its types, a
  one-line description and its code; everything else (checking, hover, completion, the
  reference test) reads the table. `std.math` came first. Its `div` goes with `%` (Euclid's
  division), so `a == b * div(a, b) + a % b` always holds.

## Design principles and where they come from

| Principle | Borrowed from |
|---|---|
| Reads top to bottom like pseudocode; few symbols, no semicolons | Python |
| Traceback with call chain, most recent call last | Python |
| `let`, `fn name(x: T) -> T`, immutability by default, no null | Rust |
| Errors point at the source with carets and give a `hint:` | Rust, Elm |
| Types are inferred for `let`; written only on function signatures | Rust, Swift |
| One way to do each thing; small keyword list | Go |
| Notation matches the textbook (arrows, `eps`, `/regex/`, set braces) | Sipser, Hopcroft-Ullman |
| One binary: `run`, `check`, `fmt`, `repl` | Go, Cargo |

## The language (v1)

```
alphabet B = {0, 1}

dfa EvenZeros over B {
    start  q0
    accept q0
    q0 --0--> q1
    q0 --1--> q0
    q1 --0--> q0
    q1 --1--> q1
}

nfa EndsIn01 over B {              # eps moves: a --eps--> b
    start  a
    accept c
    a --0,1--> a
    a --0-->   b
    b --1-->   c
}

# Binary numbers divisible by n
fn div_by(n: Int) -> Dfa {
    return dfa over B {
        start  q[0]
        accept q[0]
        for i in 0..n {
            q[i] --0--> q[(2 * i) % n]
            q[i] --1--> q[(2 * i + 1) % n]
        }
    }
}

let r = /(0|1)*01/
let d = minimize(to_dfa(EndsIn01))

print EvenZeros accepts "0011"
print equivalent(d, r)
test  div_by(3) accepts "110"      # fails the run if false

for w in {"", "01", "1101"} {
    if d accepts w { print w }
}
draw d
```

- Statements: `alphabet`, `dfa`, `nfa`, `fn`, `let`, `return`, `if/else`, `for`, `print`,
  `test`, `draw`.
- Types: `Int`, `Bool`, `Str`, `Alphabet`, `Dfa`, `Nfa`, `Regex`, `Set[T]`.
- Operators: `+ - * %`, comparisons, `and or not`, `accepts`, ranges `a..b`.
- Built-ins: `to_nfa`, `to_dfa`, `to_regex`, `remove_eps`, `minimize`, `reduce`, `rename`,
  `union`, `intersect`, `difference`, `complement`, `concat`, `star`, `reverse`,
  `equivalent`, `is_empty`, `states`, `len`.
- A regex or NFA converts automatically where a DFA is needed; no other implicit conversions.

Error output:

```
Traceback (most recent call last):
  examples/div.kstar, line 28, in <main>
    test div_by(0) accepts "110"
  examples/div.kstar, line 17, in div_by
    q[i] --0--> q[(2 * i) % n]
                  ^^^^^^^^^^^
MathError: modulo by zero
  hint: the right-hand side of `%` is 0 here
```

Check-time errors (types, non-total DFA, unknown symbol, undefined name) use the same
source-line-and-caret format without a call chain, and stop the program before it runs.

## Layout

```
Cargo.toml  build.rs           build.rs records the commit and date, and puts the icon in kstar.exe
.cargo/config.toml             links the C runtime into the Windows programs (no VC++ Redistributable)
src/
  lib.rs             Session: feeds text through the stages; shared by CLI, REPL, tests and the LSP
  main.rs            CLI: run, check, fmt, trace, export, new, explain, repl, version, update
  repl.rs  update.rs the REPL; `kstar version` and `kstar update --check`
  ide.rs             analysis for editors: problems, hover, definitions, completion, outline
  bin/kstar-lsp.rs   the language server (tower-lsp-server)
  automata/          pure library, no knowledge of the language
    dfa.rs  nfa.rs  regex.rs  convert.rs  minimize.rs  ops.rs  dot.rs
    grammar.rs  pda.rs  tm.rs  trace.rs
    codegen.rs       a DFA as a standalone Rust function (`kstar export`, `to_rust`)
  lang/
    token.rs  lexer.rs  ast.rs  parser.rs     with error recovery, so every error is reported
    check.rs  machine.rs         type checker; the rules for a valid DFA/NFA/PDA/TM
    lint.rs                      warnings
    codes.rs                     error and warning codes, explained by docs/errors.md
    builtins.rs                  built-in function names and their types
    value.rs  interp.rs  trace.rs   runtime values, evaluator, call stack, step-by-step traces
    format.rs  index.rs          the formatter; names and their places, for the editor
    module.rs  stdlib/           the loader: finds, reads and checks imported files; std
    error.rs  source.rs          error kinds, traceback and caret rendering; file names and texts
examples/            *.kstar sample programs
tests/
  golden/  format/   programs with their exact expected output
  programs.rs        runs every example and every code sample in the docs
  properties.rs  robustness.rs  scale.rs  cli.rs  ide.rs
docs/
  tutorial/          follows a Theory of Computation course, DFA to Turing machines, plus tools
  reference.md       grammar (EBNF), types, built-ins, errors, the kstar command
  errors.md          every error and warning code, with an example
  internals.md       how the interpreter works, stage by stage, and how releases are made
editors/vscode/      the VS Code extension
installer/           the Windows installer (Inno Setup); install.ps1 and install.sh at the root
.github/workflows/   ci.yml tests every push and pull request; release.yml publishes releases
```

`automata/` stays separate from `lang/` so the export stage and any future VM reuse it.
Every source position is carried from token to AST to runtime, which is what makes the
tracebacks possible; this is built in from stage 2, not added afterwards.

## Build stages

Each stage ends runnable and tested, with its section of `docs/` written alongside and a
short walkthrough of what was built and why.

0. **Setup**: install Rust via rustup (not currently installed; needs the user's go-ahead),
   `git init`, `cargo new`, hello-world CLI.
1. **Automata core**: `Dfa`, `Nfa`, `accepts`, eps-closure. Unit tests on textbook machines.
2. **Lexer + parser**: tokens with spans, AST, recursive descent for machine definitions,
   `let`, `print`, `test`; caret-style syntax errors.
3. **Checker + interpreter**: types, machine validation, environment, first end-to-end program.
4. **Conversions**: regex parser, Thompson construction, subset construction, minimisation,
   `reduce`.
5. **Language operations**: product construction (union, intersect), complement, concat, star,
   reverse, `is_empty`, `equivalent`, finite-language sets.
6. **Functions and control flow**: `fn`, `return`, `if/else`, `for`, `Int` arithmetic, indexed
   states, call stack and full tracebacks.
7. **Output**: transition-table printing, DOT export, `draw`.
8. **REPL** with `rustyline`.
9. **DFA -> regex** (state elimination); finish tutorial and reference.

Later, each as its own design: `export` to Rust source, `kstar fmt`, context-free grammars and
PDAs, Turing machines. All are built (`export` in v0.3), along with the language server
(`kstar-lsp`), the VS Code extension and the release workflow (version 0.2.0).

## Verification

- `cargo test`: unit tests per algorithm with known answers, plus round-trip checks such as
  `equivalent(m, minimize(to_dfa(m)))` and regex -> DFA -> regex.
- Every program in `examples/` runs with no errors and no failed `test` lines.
- Golden tests: each program in `tests/golden/` must produce exactly its saved output
  (non-total DFA, unknown symbol, type mismatch, undefined name, modulo by zero inside a
  nested call to confirm the traceback chain, runaway recursion, and more).
- Every code sample in the docs is run by the test suite and compared with the output
  shown beneath it, so the docs cannot drift.
- DOT output checked by rendering one example (Graphviz, or pasted into an online viewer).

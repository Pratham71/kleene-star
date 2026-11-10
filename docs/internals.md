# How TOC-Lang works inside

This page is a guided tour of the source code, in the order a program travels
through it. Each stage is one file, and each file starts with a comment saying
what it is for.

```text
              src/lang/                                        src/automata/
 text ──► lexer ──► parser ──► checker ──► interpreter ──────► machines and
          tokens    syntax     (types,     (runs it)           algorithms
                    tree       machines)
```

The project has two halves that know very little about each other:

- `src/automata/` is an ordinary Rust library. It has no idea a language exists.
- `src/lang/` is the language. It calls the library to do the real work.

That split is why TOC-Lang is an **interpreter** and not a compiler. A TOC
program is a short script that says *which* machines to build and *what* to ask.
The expensive part, such as a subset construction over thousands of states,
happens inside the library, which is compiled Rust. Compiling the script itself
would save almost nothing and would cost the interactive prompt.

## 1. Lexer: text to tokens

`src/lang/lexer.rs`, `src/lang/token.rs`

The lexer walks through the characters once and groups them into **tokens**:
`let`, a name, `=`, a number, and so on. It throws away spaces and comments
(except for the formatter, which asks `lex_with_comments` to keep the comments).

Things worth noticing:

- Every token records a `Span`: where it starts and ends in the source, and on
  which line. Spans are carried through every later stage. They are the reason
  an error can underline the exact characters at fault.
- The lexer tries the longest match first, so `-->` is one token, not `--`
  followed by `>`.
- Because the language has no division operator, a `/` can only start a regex.
  That keeps the lexer free of guesswork.

## 2. Parser: tokens to a syntax tree

`src/lang/parser.rs`, `src/lang/ast.rs`

The parser turns the flat list of tokens into a tree that shows the structure:
this `for` contains that `if`, which contains this call.

It is a **recursive-descent** parser, the most direct way to write one by hand:
there is one function per grammar rule, and a rule that mentions another rule
calls that rule's function. The grammar in [the reference](reference.md#grammar)
and the functions in `parser.rs` line up one to one; each function has its rule
in a comment above it.

Operator precedence falls out of the call order. `sum` calls `product` for its
operands, so `*` groups before `+`:

```text
sum     := product (('+' | '-') product)*
product := unary (('*' | '%') unary)*
```

The language is designed so that the parser never needs to look more than one
token ahead, and never needs line breaks to tell statements apart: every
statement starts with a keyword.

## 3. Checker: finding mistakes before running

`src/lang/check.rs`, `src/lang/machine.rs`, `src/lang/builtins.rs`

The checker walks the tree once and works out the type of every expression. It
keeps a stack of **scopes**, each a table from names to types; entering a block
pushes a table and leaving pops it.

What it guarantees: if the checker accepts a program, then while running there
will be no type errors, no unknown names, and no function that forgets to
return. The interpreter relies on this. Where it would otherwise need an error
check, it says `unreachable!` instead.

Machines get special treatment. A machine written out in full is turned into a
`MachineSpec` and validated on the spot, so a DFA with a missing transition is
reported before anything runs. A machine built with `for` or `q[i]` cannot be
known in advance, so the interpreter builds the same `MachineSpec` later and
calls the same `build` function. The rules live in one place, `machine.rs`.

## 4. Interpreter: running the tree

`src/lang/interp.rs`, `src/lang/value.rs`

This is a **tree-walking interpreter**. `exec` runs a statement; `eval`
computes an expression. Both are a `match` on the kind of node, and both call
themselves on the parts of the node.

- A `Value` is what an expression evaluates to. Machines sit behind `Rc`, a
  shared pointer, so passing one to a function never copies it. That is safe
  because values cannot change.
- The **call stack** is an explicit list of `Frame`s. Calling a function pushes
  a frame holding its parameters; returning pops it.
- `return` is handled by having `exec` report a `Flow`: either `Normal` or
  `Return(value)`. Loops and blocks pass a `Return` upward until it reaches the
  call.

## 5. Errors and tracebacks

`src/lang/error.rs`

Every error is a `Diag`: a kind, a message, a span, an optional hint, and a
trace. Rendering one means finding the source line that contains the span and
drawing carets under it.

A traceback costs almost nothing to produce, because the interpreter already
has the list of frames. Each frame remembers the span of the call that created
it, so the trace is that list of call sites, followed by the span where the
error happened.

Suggestions such as "did you mean `minimize`?" come from comparing the unknown
name with every known name by edit distance: the number of single-character
changes needed to turn one into the other.

## 6. The automata library

`src/automata/`

| File | Contents |
|---|---|
| `dfa.rs`, `nfa.rs` | The two machine types, running them on a word, printing them as tables. |
| `regex.rs` | The regex tree, its own small parser, and a printer that adds as few parentheses as possible. |
| `convert.rs` | Thompson's construction, eps removal, the subset construction, state elimination. |
| `minimize.rs` | DFA minimisation and NFA reduction, both by partition refinement. |
| `ops.rs` | The product construction, complement, concatenation, star, reversal, emptiness, equivalence. |
| `dot.rs` | Graphviz output. |
| `trace.rs` | Running a machine step by step and recording each move. |
| `grammar.rs` | Context-free grammars: CNF, CYK, emptiness, derivations, grammars from DFAs. |
| `pda.rs` | Pushdown automata: the configuration search, PDAs from grammars. |

A state is just a number: its position in the machine's list of state names.
A DFA's transitions are a table indexed by state and symbol, so a DFA is
complete by construction. An NFA's transitions map a label, where `None` means
eps, to a set of states.

Each algorithm has a comment explaining the idea, and unit tests beside it that
use small machines with known answers.

## 7. Tracing a run

`src/automata/trace.rs`, `src/lang/trace.rs`

A `trace` statement is split along the same line as everything else:

- `automata/trace.rs` runs the machine and records a `Trace`: the starting
  point, one `Step` (symbol, from, to) per symbol read, and how the run
  `End`ed: accepted, rejected, or stopped at a foreign symbol. `Trace<S>` is
  generic over what the machine "is in": a `State` for a DFA, a
  `BTreeSet<State>` for an NFA. The NFA version uses the same `eps_closure`
  and `step` as `accepts`, so a trace can never disagree with `accepts`.
- `lang/trace.rs` only formats: it names the states, lays the cells out in
  columns as wide as their widest cell, and writes the result and reason.

The parser reads the machine with `range`, one level below comparisons, so a
mistaken `trace M accepts "01"` stops at `accepts` and gets a clear message
instead of being read as one Bool.

Step 0 marks its empty cells with an em dash. On Windows, `main.rs` asks
whether that will survive: a console window always shows it (Rust writes
UTF-16 to it), but output piped into another program is decoded with the
console's code page, so unless that is UTF-8 (65001) the session is switched
to ASCII and prints `-` instead.

## 8. Grammars and pushdown automata

`src/automata/grammar.rs`, `src/automata/pda.rs`, `src/lang/machine.rs`

Both are plain library types first, with the language on top.

**Grammars.** A `Grammar` is a list of variable names, the alphabet, and
`Rule`s whose bodies are `Sym::T(char)` (terminal) or `Sym::V(index)`
(variable). The algorithms:

- `to_cnf` follows the textbook steps in order: a new start variable, a
  variable per terminal (`T_a`), splitting long bodies (`X1`, `X2`, ...),
  removing eps rules (adding a copy of each rule with every combination of
  nullable variables left out), removing unit rules (through the set of
  variables each one reaches by unit rules), and finally dropping variables
  that generate nothing or cannot be reached.
- `accepts` runs CYK on the CNF copy: `table[i][len]` holds the variables that
  generate the `len` symbols starting at `i`.
- `is_empty` computes the generating variables, growing the set until it
  stops changing.
- `derive` is a breadth-first search over sentential forms, rewriting the
  leftmost variable. It first confirms membership with CYK, then prunes forms
  whose leading terminals do not match the word or that already need more
  symbols than the word has. A limit stops runaway searches.
- `from_dfa` makes one variable per useful DFA state.

**PDAs.** A `Pda` is a list of `Move`s (`from`, `read`, `pop`, `push`, `to`)
and a bottom-of-stack symbol. `search` is a breadth-first search over
configurations (state, symbols read, stack). It remembers every configuration
it has seen, so loops of eps moves cannot trap it, and it keeps a parent link
for each, so it can return the run itself, which is what `trace` prints.
Inside the search, stack symbols are numbered, so stacks are cheap to copy
and hash. Pushing can still go on forever, so the search stops at a limit on
configurations and on stack height, and then reports `GaveUp` rather than
"rejected".

**In the language.** Grammars have their own syntax node (`GrammarExpr`)
because they are not machines. PDAs reuse the machine syntax, with a
`MachineKind` saying which kind of machine it is; a transition's `Action` is
either `Symbols` (DFA, NFA) or `Stack` (PDA). Validity rules for all of them
live in `machine.rs`.

Two lexer details make the PDA syntax work. `|` is a token for grammar
alternatives. And `/` normally opens a regex, but in `a, Z/AZ` it follows the
name `Z`. A regex can never directly follow a name, number, `eps` or closing
bracket, so after one of those the lexer reads `/` as a plain slash.

`equivalent` on a grammar or PDA is refused by the checker, in
`builtins::result_type`, with an explanation that the question is
undecidable. That is a deliberate difference from regular languages, where
the product construction always answers it.

## 9. The formatter

`src/lang/format.rs`

`toc fmt` parses the program as usual and prints the syntax tree back out in
the standard style. The tree was built for running programs, not for
printing them, so a few things come from elsewhere:

- **Comments.** The parser never sees them. `lex_with_comments` returns them
  alongside the tokens, and the formatter writes out every comment that
  starts before the next thing it prints. A comment on the same source line
  as the code just printed stays at the end of that line.
- **Parentheses.** The parser keeps `(expr)` as an `ExprKind::Paren` node.
  The checker and interpreter just look through it; the formatter writes the
  parentheses back, so the user's grouping is never changed.
- **Literals.** Numbers, strings and regexes are copied from the source text
  by their spans, so escapes and spacing inside a regex survive.
- **Blank lines** are worked out from line numbers: if the next thing starts
  more than one line after the last thing printed, the source had a blank
  line there, and one is kept.
- **Braces and `else`** are not in the tree. Their positions are looked up in
  the token list, which is also how `else if` is told apart from
  `else { if ... }`.

Machine blocks are printed in two passes: first measure the widest source
state and the widest label list among the transitions, then pad every
transition to those widths so the arrows line up.

The safety net: before returning, `format` lexes its own output and compares
it with the input's tokens and comments. If they differ, that is a formatter
bug, and the file is left alone with an error instead of being changed. The
tests check, in addition, that every example formats to itself and that
formatting never changes what a program prints.

## 10. Tying it together

`src/lib.rs`, `src/main.rs`

A `Session` owns one checker and one interpreter and feeds text through the
stages. A file is one piece of text. The interactive prompt feeds one entry at
a time into the same session, which is why names defined earlier are still
there. All the text is kept in one growing string so that an error can quote a
line typed several entries ago.

The interpreter runs on a thread with a large stack, because a tree-walking
interpreter uses the Rust stack for every nested TOC call.

## Testing

```bash
cargo test
```

- **Unit tests** sit at the bottom of each source file.
- **Golden tests** (`tests/golden/`) run a program and compare everything it
  prints with a saved `.out` file. Most of them are deliberately broken
  programs, so the wording and layout of every error message is pinned down.
- **Formatting** (`tests/format.rs`): every `tests/format/*.toc` must format
  to its `.out` file, every example must already be formatted, and
  formatting must never change what a program prints.
- **Examples and docs** (`tests/programs.rs`): every file in `examples/` must
  run cleanly, and every code sample in the documentation is run and compared
  with the output shown beneath it.

After changing an error message on purpose, regenerate the saved outputs and
read the diff before keeping it:

```bash
UPDATE_GOLDEN=1 cargo test --test golden --test format
```

## Where to go next

Ideas that fit the current design, roughly in order of size:

- `export`: write a DFA out as a standalone Rust function. This is where a
  compile step makes sense, since the result would run without TOC-Lang.
- Turing machines.

New machine types belong in `src/automata/` as plain Rust first, with tests,
and get syntax in `src/lang/` second.

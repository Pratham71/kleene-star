# How Kleene Star works inside

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

That split is why Kleene Star is an **interpreter** and not a compiler. A Kleene Star
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

- Every token records a `Span`: which file it is in, where it starts and ends
  in that file's text, and on which line. Spans are carried through every
  later stage. They are the reason an error can underline the exact
  characters at fault.
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

A span does not hold a file name or any text, only a `FileId`: a small
number. The **source map** (`src/lang/source.rs`) is the table that turns
that number back into a name and a text, and rendering is the only place
that needs it. So the lexer, parser, checker and interpreter just pass spans
along, and a traceback whose frames are in different files simply looks up
each frame's file as it is printed. The file being run is always
`FileId::MAIN`. Keeping the id and the line number as `u32`s keeps a span at
three machine words, which matters because every error carries several.

A traceback costs almost nothing to produce, because the interpreter already
has the list of frames. Each frame remembers the span of the call that created
it, so the trace is that list of call sites, followed by the span where the
error happened.

Suggestions such as "did you mean `minimize`?" come from comparing the unknown
name with every known name by edit distance: the number of single-character
changes needed to turn one into the other.

### Codes

Every `Diag` also carries a code from `src/lang/codes.rs`. Each kind of error
has a default code, and the places that report a more specific mistake set
their own. The long explanations are the sections of `docs/errors.md`,
compiled into the binary with `include_str!`, so `kstar explain` and the
documentation can never disagree. Tests check that every code has a
section, and that each section's example really produces its code.

### Reporting every error

A compiler that stops at the first error makes you fix one thing, run again,
fix the next. Each stage of Kleene Star carries on instead, which is called
*error recovery*:

- The **lexer** records an error and skips the bad character, or turns an
  unclosed string into a string anyway, so the parser sees sensible tokens.
- The **parser** gives up on the statement it was in, skips forward to a
  token that starts a line and starts a statement (`let`, `dfa`, `print`, ...),
  and carries on. It counts braces while skipping so it does not stop inside
  a `{ ... }` that the broken statement opened; a statement at the very start
  of a line ends the skipping anyway, so one missing `}` does not hide the
  rest of the file. Inside a machine it recovers line by line, and marks the
  machine *incomplete* so that it is not checked for totality and so on:
  with a line missing, those errors would be wrong.
- The **checker** checks one statement at a time. When a statement fails,
  its error is recorded, and if it was a definition the name is marked
  *broken*. Using a broken name later produces an error that is silently
  dropped: the real mistake has already been reported.

After a syntax error, the checker also drops "not defined" errors, since the
definition may be in a statement that did not parse. `lang::check_source`
runs the three stages and sorts what they found by position.

### Warnings

`src/lang/lint.rs`

Warnings are found only when there are no errors. The checker passes each
machine that is written out in full to `lint::machine`, which computes the
states reachable from the start state and the states from which an
accepting state can be reached (the same search, run backwards over the
transitions), and warns about the rest. Unused names come from the index
(section 10): a definition with no references is unused. A constant `test`
is one whose expression contains nothing but literals.

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
| `codegen.rs` | Writing a DFA as a standalone Rust function. |
| `grammar.rs` | Context-free grammars: CNF, CYK, emptiness, derivations, grammars from DFAs. |
| `pda.rs` | Pushdown automata: the configuration search, PDAs from grammars. |
| `tm.rs` | Turing machines: the tape, running with a step limit. |

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

## 9. Turing machines

`src/automata/tm.rs`

A `Tm` has `Move`s (`from`, `read`, `write`, `dir`, `to`), accepting and
rejecting states, and the input alphabet. The `Tape` is a `Vec<char>` with an
offset, so it can grow to the left as well as the right; any cell outside it
reads as the blank `_`.

`run` steps the machine until it enters an accepting or rejecting state, has
no move, or reaches the step limit. Each of those is a different `Halt`, so
the language can tell "rejected" apart from "still running": `accepts` and
`run` turn `GaveUp` into a `LimitError` whose hint explains the halting
problem, while `trace` (which records every configuration, and so uses a
smaller limit) reports `UNKNOWN`.

In the language, a Turing machine is one more `MachineKind`, with its own
transition `Action::Tape` and a `reject` item. Determinism and the other rules
are checked in `machine.rs`, like every other machine.

## 10. The language server

`src/lang/index.rs`, `src/ide.rs`, `src/bin/kstar-lsp.rs`

While checking, the checker already knows what every name refers to. It
writes this down in an `Index`: every definition (with its kind, type,
visible range and a one-line description), every use mapped to its
definition, the type of every expression, every call, and the bodies of
machines and grammars. States and grammar variables are indexed per machine
or grammar. The checker stops at the first error, but the index keeps what
it learned up to there.

`ide.rs` answers editor questions from the index using byte offsets:
`hover`, `definition`, `references`, `completions` and `signature_help`.
Signature help reads the tokens before the cursor instead of the syntax
tree, so it works while a call is half typed; when the text does not parse,
`with_fallback` keeps the previous index for completion. All of this is
tested directly, without an editor.

`kstar-lsp` is a thin `tower-lsp-server` shell: it keeps one `Analysis` per
open file, converts between LSP positions (UTF-16 columns) and offsets, and
publishes the first error as a diagnostic.

### The VS Code extension

`editors/vscode` is a thin client. `extension.js` finds `kstar-lsp` and
`kstar` (a setting, the PATH, or the usual install folders), starts the
server with `vscode-languageclient`, and runs `kstar run`, `check` or
`trace` for its commands, showing the output in a panel. Highlighting is a
TextMate grammar (`syntaxes/kstar.tmLanguage.json`); machines and grammars
are regions from their header to the closing `}`, so states, labels, rule
heads and terminals can be coloured differently from ordinary code. Its
tests (`npm test`) tokenize code with VS Code's own TextMate engine, check
every snippet with `kstar check`, and drive the real server over the
protocol.

## 11. The formatter

`src/lang/format.rs`

`kstar fmt` parses the program as usual and prints the syntax tree back out in
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

## 12. Tying it together

`src/lib.rs`, `src/main.rs`

A `Session` owns one checker, one interpreter and the source map, and feeds
text through the stages. Warnings are rendered and kept for the caller to
show; errors are rendered, joined, and counted. The interactive prompt feeds
one entry at a time into the same session, which is why names defined
earlier are still there. Each entry is appended to the text of the main file
in the source map, so an error can quote a line typed several entries ago.

The interpreter runs on a thread with a large stack, because a tree-walking
interpreter uses the Rust stack for every nested Kleene Star call.

## 13. Modules

`src/lang/module.rs`, `src/lang/check.rs`, `src/lang/interp.rs`

An `import` statement goes through the same stages as everything else, with
one stage added between parsing and checking: the **loader**.

**Parsing.** `import` and `from` are statements; the parser checks that
they are at the top level and that a path ends in `.kstar`, and leaves the
statement's `target` empty. A qualified name such as `utils.EvenZeros` is
read as one `Name` whose text keeps the dot. The dots must touch the names
on both sides, so the position of each part can be worked out from the
text, which is how an error can point at just `EvenZeros`.

**Loading.** Before a file is checked, `Loader::resolve` goes through its
import statements. For each one it joins the path to the importing file's
folder, tidies it (`a/../b` becomes `b`, without asking the disk), and:

- if the file is already loaded, uses it again: every file is loaded once;
- if the file is in the middle of being loaded, the imports go round in a
  circle, and the chain of files being loaded is the error message;
- otherwise reads it, gives it the next `FileId` in the source map, parses
  it, resolves *its* imports (recursion), and checks it with a checker of
  its own.

Either way the statement's `target` is filled in with a `ModuleRef`: a file
id, or the name of a standard-library module. From then on nobody deals
with paths. Reading goes through a function the loader is given, so tests
and the language server can supply files that are not on disk.

**Interfaces.** What a checked module offers is its `Exports`: the type of
each top-level value, the parameter and result types of each function, and
each one's definition for the editor. The checker that checked the module
produces it, leaving out names the module imported itself (so there is no
re-exporting). The importing file's checker gets the table of every
module's exports and needs nothing else, which is why a module's mistakes
cannot leak into the importing file's checking: either a name is in the
table with a type, or it is not there. The standard library (`stdlib/`)
produces the same kind of table.

**Checking.** The checker keeps the modules bound by `import` apart from
ordinary names (`aliases`). `resolve_path` reads a qualified name part by
part: the first part (or two, for `std.math`) names a module, the next
must be one of its exports, and anything after that is an error, with a
special message when it is a module that module imported. A name taken
with `from` becomes an ordinary name of the file. For the editor, each
definition from another file is copied into the index once (`mirror`), so
hovering, completing and "go to definition" work across files.

**Running.** The interpreter keeps a `ModuleState` per file: its globals,
its functions, and the modules and names it imported. Every frame records
which file's code it runs, so a function finds the globals of the file it
was written in. A module's top level runs the first time one of its names
is needed (`run_module`), in a frame called `<module>` whose call site is
that first use. That frame is *quiet*: `print`, `test`, `draw` and `trace`
are skipped in it. If the module fails, it is reset to "not run", so the
REPL can try again after a fix. `--eager-imports` simply runs the module
when its `import` statement executes.

**Errors across files.** Every span carries its file id (section 5), so an
error in a module is shown with the module's name and line, and a
traceback whose frames are in different files shows each frame's file.

## 14. Generating Rust

`src/automata/codegen.rs`, `to_rust` in `src/lang/interp.rs`, `kstar export`
in `src/main.rs`

This is the one place where a compile step pays: a DFA is a fixed table, and
written out as code it runs at full speed with nothing else installed. The
generator is part of the automata library and knows nothing about the
language: a `Dfa` goes in, a `String` of Rust comes out.

- States are numbered in breadth-first order from the start state
  (`Dfa::reachable`), so the start state is always `0` and unreachable
  states are left out. Their names survive in a doc comment and in a
  comment on every arm.
- The function is a loop over `input.chars()` with
  `state = match (state, symbol) { ... }`. The transitions of one state that
  go to the same target are joined into one arm (`'0' | '1'`), and a final
  `_ => return false` arm rejects symbols outside the alphabet.
  Characters and strings are written with Rust's own `{:?}` escaping, so any
  symbol is a valid literal.
- The accepting states become `matches!(state, 0 | 2)`.
- `sample_words` walks the words breadth first, shortest first, and keeps
  the first few that are accepted and rejected, plus one word with a symbol
  outside the alphabet. They become the generated `#[cfg(test)]` module.

`to_rust` converts its argument to a DFA like every other built-in on
languages (`to_dfa`), and checks the function name against Rust's rules and
keywords: at check time when the name is written as a string, otherwise
when it runs. `kstar export` runs the file quietly and then evaluates
`print to_rust(M, "name")`, like `kstar trace` does with `trace`.

`tests/export.rs` compiles the generated code with `rustc` for several
machines (one of them built from a regex, one empty), runs it on random
words, compares every answer with the interpreter's, and runs the generated
tests with `rustc --test`. Without `rustc` it prints that it was skipped.

## 15. The standard library

`src/lang/stdlib/`

The standard library is a table, so that adding to it never means touching
the checker, the interpreter or the editor support. `stdlib/mod.rs` lists
the modules (`MODULES`); each module file (`math.rs`) holds one `Module`
with a list of `Function`s. A function is one entry: its name, its
parameters and result as `Ty`s (a version of `Type` that a constant table
can write down), a one-line description, and the Rust function that runs
it, which gets values the checker has already typed and returns a value or
a `Failure`.

From that table, `stdlib::exports` builds the same `Exports` that a checked
file produces, with the description in each definition's `doc`. So the
loader, the checker (`resolve_path`, `check_arguments`) and the editor
(hover, completion after `std.` and `std.math.`, signature help) treat a
library function exactly like a function imported from a file. The
interpreter's `callee` finds the entry with `stdlib::function` and calls its
`run`; a `Failure` becomes a `MathError` pointing at the argument it names.

A test (`the_reference_matches_the_table`) checks that `docs/reference.md`
has a section for each module with one row per function, giving its exact
signature and description, so the reference cannot drift from the code.

### Adding a standard-library module

Say the new module is `std.string`.

1. **Write the module.** Create `src/lang/stdlib/string.rs` with
   `pub const MODULE: Module = Module { name: "string", doc: "...", functions: &[...] };`,
   one `Function` per function. Copy the shape of `math.rs`: a one-line
   `doc` that reads as a sentence, parameter names that say what they are
   for, and `Ty`s for the types (`Ty::Set(&Ty::Str)` for a `Set[Str]`).
2. **Implement each function** as `fn name(args: &[Value]) -> Result<Value, Failure>`.
   The checker has already made sure each argument has its declared type,
   so take them apart with `match` and `unreachable!` for the impossible
   case. Never panic and never return a wrong answer: an argument the
   function cannot handle is a `Failure` with a code from `codes.rs`, the
   index of the argument (`arg: Some(i)`), a message that quotes the value,
   and a hint that says what would work. Reuse codes where they fit
   (`OVERFLOW`, `MATH_DOMAIN`); a new kind of mistake gets a new code, in
   `codes.rs` and `docs/errors.md`, with an example.
3. **Register it.** Add `pub mod string;` to `stdlib/mod.rs` and
   `string::MODULE` to `MODULES`, keeping the list in alphabetical order.
4. **Unit-test it** at the bottom of `string.rs`: ordinary answers, the
   edge cases (empty input, the largest and smallest values), and every
   `Failure`, checking its code.
5. **Document it** in `docs/reference.md`: a `### \`std.string\`` section
   under "The standard library", with the module's `doc` as its first
   paragraph and a table with one row per function,
   `` | `name(a: T) -> R` | <doc> | ``, in the same order as the code. Then
   `cargo test lang::stdlib` tells you any row that does not match. Add a
   runnable example under the table, then fill in its output with
   `UPDATE_DOCS=1 cargo test --test programs`.
6. **Show it in use**: a golden test in `tests/golden/` with ordinary calls
   and each error, and, if it suits the tutorial, a few lines in chapter 9.
   Regenerate with `UPDATE_GOLDEN=1 cargo test --test golden` and read the
   diff.
7. Run `cargo test` and `cargo clippy --all-targets -- -D warnings`.

Nothing else changes: hover, completion, signature help, `from std.string
import ...` and the checker's errors all come from the table.

## Testing

```bash
cargo test
```

- **Unit tests** sit at the bottom of each source file.
- **Golden tests** (`tests/golden/`) run a program and compare everything it
  prints with a saved `.out` file. Most of them are deliberately broken
  programs, so the wording and layout of every error message is pinned down.
- **Formatting** (`tests/format.rs`): every `tests/format/*.kstar` must format
  to its `.out` file, every example must already be formatted, and
  formatting must never change what a program prints.
- **Examples and docs** (`tests/programs.rs`): every file in `examples/` must
  run cleanly, and every code sample in the documentation is run and compared
  with the output shown beneath it. `UPDATE_DOCS=1 cargo test --test programs`
  rewrites those outputs after a deliberate change.
- **Modules** (`tests/modules.rs`, and the golden tests named `import_*`)
  import files that exist only in the test, or in `tests/golden/modules/`:
  quiet and lazy loading, eager loading, per-file globals, the REPL.
- **The command line** (`tests/cli.rs`) runs the real `kstar` binary in a
  scratch folder: several files, folders, `fmt -`, `new`, `explain`, colour,
  `--trace-calls`, and exit codes.
- **Robustness** (`tests/robustness.rs`): Kleene Star must never crash. A
  seeded generator produces random token sequences, truncated and mutated
  copies of every program in the repository, unbalanced brackets, programs
  nested a hundred thousand deep, huge literals and odd Unicode. Each goes
  through the lexer, parser, checker, editor queries and formatter, and must
  end in a result or a clean error; the formatter must also be idempotent.
- **Properties** (`tests/properties.rs`): facts from the theory, checked on
  random machines, regexes and grammars by comparing both sides on every
  word up to a fixed length. The subset construction, eps removal and
  `reduce` keep the language; minimization is idempotent and unique in size;
  DFA to regex to DFA, and printing a regex and reading it back, keep the
  language; De Morgan's laws, `reverse(reverse(L)) = L`, and the meaning of
  union, difference and concatenation hold; CYK agrees with the PDA built by
  `to_pda`; and Turing machines always stop within their step limit.
- **Scale** (`tests/scale.rs`): big inputs with time budgets. The k-th
  symbol from the end has an NFA of k + 1 states and a DFA of 2^k; a word of
  a million symbols, in Rust and through a program; long traces; a grammar
  of sixty variables; `to_regex` of a 32-state DFA.

The random tests use a fixed seed, so they are repeatable. To try other
inputs, set `KSTAR_SEED`. The heaviest cases are marked `#[ignore]` so that
`cargo test` stays under a minute; run them in a release build:

```bash
cargo test --release -- --ignored
```

```bash
KSTAR_SEED=12345 cargo test --release --test robustness -- --ignored
```

### Limits

Every stage walks the program recursively, so the parser refuses nesting
deeper than 200 levels (`MAX_NESTING`), and the regex parser 500
(`regex::MAX_DEPTH`), with error E0113 instead of a stack overflow. In a
debug build each level costs about 20 KB of stack, more than a default
thread can give, so every entry point (`kstar run`, `check`, `fmt`, the
REPL and each language-server thread) runs with a 256 MB stack. Only the
part that is used is ever committed.

A few results can grow exponentially, and are capped rather than allowed to
run out of memory: `+` in a regex copies what it repeats (so a regex is
limited to 20 000 nodes, and `(a+)+` is simplified to `a+`), and `to_regex`
stops past 100 000 nodes with error E0608. State elimination removes the
state whose removal creates the least, rather than going in a fixed order;
for a 32-state DFA that took the time from minutes to milliseconds.

After changing an error message on purpose, regenerate the saved outputs and
read the diff before keeping it:

```bash
UPDATE_GOLDEN=1 cargo test --test golden --test format
```

## Where to go next

Ideas that fit the current design, roughly in order of size:

- `kstar update` that replaces the installed program itself. Today it only
  says whether a newer release exists and prints the install command; doing
  the replacement safely means renaming the running `.exe` on Windows and
  knowing how this copy was installed (installer, script or `cargo install`).
- Publishing the VS Code extension on the marketplace and the crate on
  crates.io.

## Continuous integration

`.github/workflows/ci.yml` runs on every push to `main` and every pull
request, and can be started by hand. On Windows, Linux and macOS it runs
`cargo test`, the doc tests and clippy, and checks that the examples are
formatted; then it builds `kstar`, runs the VS Code extension's tests and
packages the `.vsix`.

## Releases

`.github/workflows/release.yml` runs when a `v*` tag is pushed. Each platform
runs `cargo test`, then builds `kstar` and `kstar-lsp` in release mode and
packages them with `.github/scripts/package.sh`. The Windows installer is
built from `installer/kstar.iss` with Inno Setup. It carries the `.vsix`
and, when VS Code is installed, installs the extension through VS Code's
`code` command (and removes it on uninstall). It is tested by
`installer/test-installer.ps1` on a runner with VS Code installed (install,
check, uninstall, check again);
`install.ps1` and `install.sh` are tested against the freshly built archives
through `KSTAR_INSTALL_FROM`. A tag must match the version in `Cargo.toml`.
A plain `vX.Y.Z` tag publishes a release; a tag with a suffix, such as
`v0.2.4-beta`, publishes a pre-release, which GitHub does not count as the
latest (so `kstar update` and the install scripts keep offering the last
stable one). Running the workflow by hand is a dry run whose files stay as
the run's artifacts. Apple Silicon is cross-compiled on the Intel macOS
runner, so a shortage of arm64 runners cannot hold the release up; a
separate job runs the tests on a real arm64 machine without the release
waiting for it. Each archive and the installer carry the MIT `LICENSE`.

Third-party actions are pinned to commit SHAs and every job asks only for
the permissions it needs; Dependabot proposes updates to the pins, the Rust
crates and the extension's npm packages.

`build.rs` records the commit, build date and target that `kstar version`
prints, and on Windows embeds the K* icon in the executables.
`.cargo/config.toml` links the C runtime statically into the Windows
programs (`crt-static`), so they need no Visual C++ Redistributable; a test
in `tests/cli.rs` fails if either program imports `VCRUNTIME140.dll`.

To release: bump the version in `Cargo.toml` and
`editors/vscode/package.json`, merge, then `git tag vX.Y.Z` (or
`vX.Y.Z-beta` for a pre-release) and push the tag.

New machine types belong in `src/automata/` as plain Rust first, with tests,
and get syntax in `src/lang/` second.

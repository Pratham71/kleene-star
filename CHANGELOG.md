# Changelog

Every release of Kleene Star, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the version
numbers follow [Semantic Versioning](https://semver.org/). A version with a
suffix, such as `0.2.5-beta`, is a pre-release: it is on the
[releases page](https://github.com/Pratham71/kleene-star/releases) but is
not what the install scripts and `kstar update` treat as the latest.

The release workflow copies each version's section below into the notes of
its GitHub release, so write it for people who use Kleene Star.

## [Unreleased]

### Added

- `std.grammar`: the steps of the Chomsky normal form construction one at a
  time (`remove_useless`, `remove_eps`, `remove_units`, and the sets they use:
  `nullable`, `generating`, `reachable`), `first` and `follow` sets, `is_ll1`
  and `is_cnf`, `words_up_to` to list a grammar's short words, and four
  ready-made grammars: `balanced`, `anbn`, `palindromes` and `equal_count`.
  Example: `examples/18_grammar.kstar`.
- Standard-library modules may be named by a keyword (`std.grammar`). Such a
  module is used by its full name, with `from ... import`, or imported with
  `as`; plain `import std.grammar` asks for a name.
- Error code E0617, for a variable name a grammar does not have.
- `std.random`: random Ints, words, DFAs, NFAs and grammars from a seed. The
  same seed always gives the same result, on every computer, so programs and
  tests that use it stay repeatable: `int`, `pick`, `word`, `words`,
  `word_in` (a word of a regular language, each as likely),
  `random_dfa`, `random_nfa` and `random_grammar`. Example:
  `examples/19_random.kstar`.
- `std.pda`: ready-made PDAs (`anbn`, `balanced`, `palindromes`,
  `equal_count`), `to_grammar` (a PDA's grammar, by the triple construction,
  the converse of `to_pda`), `is_deterministic`, `stack_symbols`, and
  `member` and `words_up_to`, which go through the grammar and so always
  answer where `accepts` would give up. Example: `examples/20_pda.kstar`.

### Changed

- A standard-library error about two symbols that must differ is an
  `AlphabetError` (E0503), like the same error in an alphabet.

## [0.4.0] - 2026-11-19

Four new standard-library modules for the subject itself, and a way to move
machines to and from other tools as JSON. `std.automata` builds the machines
that come up again and again and answers questions the built-ins only answer
with yes or no; `std.regex` keeps regexes as regexes; `std.binary` does binary
numbers; `std.testing` makes a failing `test` say why.

### Upgrading from 0.3.0

Programs that ran under 0.3.0 run the same, with one exception: `to_json` and
`from_json` are now built-in functions, so a program that defines a function
with either name is an error (E0203). Rename the function. Nothing else needs
changing; the new modules are reached by their full names (`std.automata...`)
or imported, like the 0.3 ones.

### Added

- **`std.automata`**: ready-made minimal DFAs for common patterns
  (`starts_with`, `ends_with`, `contains_substring`, `exactly`, `at_least`,
  `at_most`, `count_mod` for parity and remainders, `length_mod`,
  `length_between`), with states named after what they remember, and
  questions the built-ins only answer with yes or no: `shortest_word`,
  `distinguishing_word` (the shortest word that shows two languages are not
  `equivalent`), `is_finite`, `count_words` and `words_up_to`. The questions
  take any regular language, like the built-ins do.
- **`std.regex`**: regexes built at run time and put together from pieces,
  staying regexes (the built-ins `concat` and `star` turn them into
  machines): `parse` (a regex from a string), `literal`, `any_of`, `alt`,
  `seq`, `zero_or_more`, `one_or_more`, `optional`, `power`,
  `repeat_between`, `at_least`, the prefix, suffix and substring forms
  `starts_with`, `ends_with`, `contains`, and `size` and `text`.
- **`std.binary`**: machines over `{0, 1}` for binary numbers and bit
  strings (`divisible_by`, `remainder`, `no_leading_zeros`, `fixed_width`,
  `even_parity`, `odd_parity`, `run_at_most`) and the conversions
  `to_binary` and `from_binary`. A bit string is read most significant bit
  first; leading zeros do not count, and the empty string is 0.
- **`std.testing`**: checks for `test` statements that say why they
  failed: `equal` (with the first difference of two strings marked),
  `equal_sets` (what is missing and what should not be there),
  `accepts_all` and `rejects_all` (the words that went wrong),
  `same_language` (a shortest word that is in one language but not the
  other), and `agree_on` and `agree_up_to` to compare grammars, PDAs and
  Turing machines on sample words. A failing `test` prints the explanation
  under its line; `test` statements in imported files are still skipped.
- **Machines as JSON**: a documented, versioned format (version 1) for
  DFAs, NFAs (with eps moves), regexes, grammars, PDAs and Turing machines.
  `to_json(m)` writes it, `from_json(Dfa, text)` reads it back (the first
  argument names the kind expected), and `kstar export --format json` writes
  a file. State names survive the round trip. Reading is strict and limited
  (1 MiB, 10000 states, unknown fields refused), with errors that say where
  the problem is (E0616).
- Three new error codes: E0614 (a string that should be one symbol), E0615
  (no word with the asked-for property) and E0616 (text that is not a
  machine in the JSON format).

### Changed

- A library argument whose element type is not decided yet now says it
  must be "an Int, a Str or a Bool" instead of "a T".
- An error from a standard-library function about a symbol or an alphabet
  is an `AlphabetError`, like the same error from a built-in.

## [0.3.0] - 2026-11-19

The first stable release since 0.2.3. It brings together everything from
the two 0.2.x pre-releases and finishes the standard library's first three
modules. Nothing that worked in 0.2.3 stops working, with one small
exception: a program can no longer import a name that is also a built-in
function (see Changed below).

### Upgrading from 0.2.3

What 0.3 adds since 0.2.3:

- **Imports.** `import "utils.kstar"`, `import "utils.kstar" as u` and
  `from "utils.kstar" import name`, with qualified names such as `utils.B`.
- **`kstar export`** and `to_rust`, which write a regular language as a
  standalone Rust function with its tests.
- **A standard library**, reached as `std.<module>.<name>` with no import,
  or imported like a file: `std.math` (arithmetic), `std.string` (words)
  and `std.collections` (sets, and every word of a length over an
  alphabet). Hovering, completion and signature help know all of it.
- New error codes E0114, E0115, E0206 to E0211, E0308 and E0609 to E0613,
  and the warning W0202; each is explained by `kstar explain`.
- The Windows programs run without the Visual C++ Redistributable.

To upgrade, run the install command in
[the installation guide](docs/install.md) again, or `kstar update`. Check
your programs for a function you defined called `reverse`, `union` and so
on: that was always an error, and `from ... import` of such a name now is
too. Version 0.2.4-beta and 0.2.5-beta were pre-releases of this one; their
entries are below.

### Added

- `std.collections`, the third standard-library module: set helpers
  `contains`, `is_subset`, `union`, `intersect`, `difference`, `size`, `min`,
  `max`, `sum`, and `words(alphabet, n)`, every string of length `n` over an
  alphabet (at most 100000, or a clear error naming the limit). The
  functions are generic over the element type of a set (Int, Str or Bool);
  `min` and `max` of an empty set are an error with a hint (new code
  E0613). Its `union`, `intersect` and `difference` act on set members, not
  on languages like the built-ins of those names, so
  `from std.collections import union` is an error that points to `as`. There
  is no power set or set of pairs, because sets of sets do not exist yet.
- `std.string`, the second standard-library module: `length`, `reverse`,
  `repeat`, `contains`, `starts_with`, `ends_with`, `index_of` (-1 when
  absent), `count` (non-overlapping), `substring`, `char_at`, `to_int`,
  `from_int`, `is_palindrome`, and the set-valued `symbols`, `prefixes`,
  `suffixes` and `substrings` (each but `symbols` includes the empty
  string). Indices count characters, not bytes. Bad indices, a negative
  repeat count and text that is not a number are errors with hints (new
  codes E0610 and E0611); a result over 1000000 characters is an error
  (E0612), not a hang. `std.string.length` is the same as `len` on a Str.
- An animation at the top of the README shows a regular expression becoming
  an eps-NFA, a DFA and a minimal DFA, drawn from real `kstar` output.
- Two more animations, in the README and the tutorial: a DFA written a line
  at a time with its diagram growing beside it, ending with `draw`; and
  `trace` following a word through it step by step.
- [docs/install.md](docs/install.md), a full installation guide: every way
  to install, how to check a download, update, choose between stable and
  beta, uninstall, and fix common problems.
- This changelog.

### Changed

- Taking a built-in's name with `import ... as` or `from ... import`, as in
  `from std.string import reverse`, is now an error (E0203) with a hint to
  use `as` or the full name. Library errors that are not arithmetic are now
  `ValueError` or `LimitError` instead of `MathError`.
- The notes of each GitHub release now come from this file.

### Fixed

- The reference's list of keywords now includes `import`, `from` and `as`.
- The README showed `from utils import f`, which is not valid; a `from`
  import names a file in quotes: `from "utils.kstar" import f`.
- `kstar help update`, `kstar update --help`, `kstar help version` and
  `kstar version --help` show help for that command, not the general help.
- `kstar update` suggests the same install command as the README, which
  needs only PowerShell or curl, instead of one that needed the GitHub CLI.
- The VS Code extension uses minimatch 11.0.8, fixing a denial-of-service
  advisory in the version it bundled before.

## [0.2.5-beta] - 2026-11-13

A pre-release: see
[choosing stable or beta](docs/install.md#choosing-stable-or-beta).

### Changed

- When a module has no name like the one you asked for (E0208), the error
  now lists what the module does define, such as "it defines `B`,
  `EvenZeros` and `div_by`", instead of saying nothing more. It shows the
  first six names in alphabetical order and how many more there are.
- The Windows programs no longer need the Visual C++ Redistributable: the C
  runtime is built into `kstar.exe` and `kstar-lsp.exe`, so they run on a
  fresh Windows installation.

## [0.2.4-beta] - 2026-11-13

A pre-release with the first features of 0.3: imports, a standard library
and exporting machines to Rust. See
[choosing stable or beta](docs/install.md#choosing-stable-or-beta).

### Added

- **Imports.** A program can be split over several files:
  `import "utils.kstar"`, `import "utils.kstar" as u`, and
  `from "utils.kstar" import EvenZeros, double as twice`. Qualified names
  such as `utils.B` work wherever a name does, including after `over` and in
  `trace` and `draw`. Each file is loaded once, circular imports are
  reported with their chain, and errors and tracebacks name the file they
  are in. See [the tutorial chapter](docs/tutorial/09-modules.md).
- An imported file runs only when one of its names is first used, and
  quietly: its `print`, `test`, `draw` and `trace` lines are skipped.
  `kstar run --eager-imports` runs each one at its `import` line instead.
- **The standard library**, available without an import as `std`. Its first
  module, `std.math`, has `abs`, `min`, `max`, `pow`, `div`, `gcd`, `lcm`,
  `sqrt`, `log2`, `factorial` and `is_prime`. It can also be imported:
  `import std.math as m` or `from std.math import gcd`.
- **`kstar export`** and the built-in `to_rust` write any regular language
  as one standalone Rust function, `pub fn name(input: &str) -> bool`, with
  its own tests: `kstar export file.kstar M --fn name -o out.rs`, optionally
  with `--minimize`.
- New errors and a warning, each explained by `kstar explain`: E0114,
  E0115, E0206 to E0211, E0308 and E0609, and W0202 for an import that is
  never used.
- In the editor: go to definition across files, hover, completion and
  signature help for imported names and for `std`, and errors in an
  imported file shown at its `import` line.

### Changed

- A version tag with a suffix (such as `v0.2.4-beta`) now publishes a
  GitHub pre-release instead of nothing.

### Upgrading

- `import`, `from` and `as` are now keywords, and `std` is the name of the
  standard library. A program that used any of them as the name of a value,
  function, machine or alphabet stops with an error (E0103 or E0209) that
  points at it; rename it.

## [0.2.3] - 2026-11-12

### Added

- The Windows installer can install the VS Code extension too. The option
  is ticked by default and only shown when VS Code is installed. The
  uninstaller removes the extension again, if the installer put it there.

## [0.2.2] - 2026-11-12

### Added

- In VS Code, Run File and Trace a Machine now run `kstar` in a terminal
  named "Kleene Star", which behaves exactly like the command line, colours
  included. The new setting `kstar.runIn` set to `output` brings back the
  output panel.
- F5 and Ctrl+F5 (Cmd+F5 on macOS) run the current `.kstar` file.
- The extension's README explains how to run Kleene Star files with Code
  Runner, and how to show the K* icon with icon themes such as Material
  Icon Theme.

### Fixed

- `.kstar` files now show the K* icon in VS Code. The icon was missing from
  the extension in 0.2.0 and 0.2.1.

## [0.2.1] - 2026-11-12

The first release from the public repository,
[Pratham71/kleene-star](https://github.com/Pratham71/kleene-star).

### Added

- Builds for Apple Silicon Macs (`kstar-aarch64-apple-darwin.tar.gz`) in
  every release, and `install.sh` picks them automatically.
- Kleene Star is released under the MIT licence. The licence is in every
  archive, the installer and the VS Code extension.
- Contributor documentation: [CONTRIBUTING.md](CONTRIBUTING.md), a
  [code of conduct](CODE_OF_CONDUCT.md), a [security policy](SECURITY.md),
  and templates for bug reports and language proposals.

### Changed

- The install scripts download from the public repository with a plain
  download; signing in to GitHub is no longer needed.

## [0.2.0] - 2026-11-11

The first release with downloadable builds, and the first under the name
Kleene Star. It was published only in the project's earlier, private
repository; 0.2.1 has the same features and more.

### Added

- **Releases** for Windows x64, Linux x64 and macOS x64, each with
  `kstar` and `kstar-lsp`, a SHA-256 checksum per file and a `SHA256SUMS`
  list.
- **The Windows installer**, `kstar-setup-x64.exe`: installs for the
  current user without administrator rights, adds `kstar` to the PATH,
  associates `.kstar` files with the K* icon, adds a Start Menu entry for
  the REPL, and has an uninstaller that removes all of it.
- **Install scripts**, `install.ps1` for Windows and `install.sh` for Linux
  and macOS, which check the checksum before installing.
- `kstar version` shows the version, commit, build date and target, and
  `kstar update --check` says whether a newer release exists.
- **Every error at once.** A run reports every error in the program, not
  only the first, without inventing follow-on errors.
- **Error codes.** Every error and warning has a code, such as
  `NameError[E0200]`, and `kstar explain E0200` describes it, with an
  example. All of them are in [docs/errors.md](docs/errors.md).
- **Warnings**, shown before a program runs: unreachable states, states
  that can never reach acceptance, unused values and functions, and tests
  whose answer never changes.
- A better command line: `run` and `check` take several files and folders,
  `kstar new`, `kstar explain`, `kstar trace <file> <M> <word>`,
  `run --watch`, `run --trace-calls`, `--color`, `NO_COLOR` and `--help`
  for each command. The REPL has `:type`, `:load`, `:reset`, `:trace`,
  history and tab completion.
- A finished VS Code extension: highlighting for every kind of block,
  snippets, folding, an outline, formatting, the K* icon, and commands to
  run, check, trace and format the current file.
- The K* logo.

### Changed

- The project is now called Kleene Star, after the star in `a*`.

### Fixed

- Bugs found by new stress tests: regexes such as `/a+++/` that grew
  without end, very deep nesting that crashed instead of reporting an
  error (E0113), crashes on non-ASCII text in editors and error messages,
  `to_regex` taking minutes on a 32-state DFA (it now also stops past
  100000 nodes, E0608), and slow renaming of states and CYK parsing on big
  inputs.

### Upgrading

- The command `toc` is now `kstar`, the language server `toc-lsp` is now
  `kstar-lsp`, and programs end in `.kstar` instead of `.toc`. The
  language itself did not change: rename your files and they run as
  before.

## [0.1.0] - 2026-11-10

The first version, under the name TOC-Lang, with a command called `toc`.
It was never published as a release.

### Added

- DFAs, NFAs with eps moves, regular expressions and finite languages,
  with the conversions between them (Thompson's construction, the subset
  construction, eps removal, minimization, state elimination back to a
  regex) and the closure operations (union, intersection, difference,
  complement, concatenation, star, reversal), equivalence and emptiness.
- A language with types checked before the program runs, functions,
  loops, tests and tracebacks, and the commands `run`, `check` and `repl`.
- `draw`, which writes a Graphviz state diagram.
- `trace`, which runs a machine step by step and shows each state.
- `toc fmt`, which rewrites programs in one standard style.
- Context-free grammars (Chomsky normal form, CYK, derivations) and
  pushdown automata.
- Turing machines, with a step limit whose error explains the halting
  problem.
- A language server and a first VS Code extension.
- A tutorial, a reference and an internals guide whose examples are run by
  the tests.

[Unreleased]: https://github.com/Pratham71/kleene-star/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/Pratham71/kleene-star/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/Pratham71/kleene-star/compare/v0.2.5-beta...v0.3.0
[0.2.5-beta]: https://github.com/Pratham71/kleene-star/compare/v0.2.4-beta...v0.2.5-beta
[0.2.4-beta]: https://github.com/Pratham71/kleene-star/compare/v0.2.3...v0.2.4-beta
[0.2.3]: https://github.com/Pratham71/kleene-star/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/Pratham71/kleene-star/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/Pratham71/kleene-star/releases/tag/v0.2.1
[0.2.0]: https://github.com/Pratham71/kleene-star/tree/v0.2.0
[0.1.0]: https://github.com/Pratham71/kleene-star/tree/2f991b4

# Contributing to Kleene Star

Thanks for wanting to help. Kleene Star is a small language for Theory of
Computation; contributions that make it clearer, more correct or better
documented are all welcome. Please read the
[code of conduct](CODE_OF_CONDUCT.md) first.

## How this project was built

The code, tests and documentation of Kleene Star were written with
[Claude Code](https://claude.com/claude-code), an AI coding assistant. The
language design and every decision about it were made by a human, and a
human reviewer was present at each step and approved each change.

You may use AI tools for your contributions too, on the same terms: a human
reviews everything before it is submitted, understands it, and takes
responsibility for it.

## Building and testing

You need [Rust](https://rustup.rs) (stable). Then:

```bash
cargo build
```

```bash
cargo test
```

```bash
cargo clippy --all-targets -- -D warnings
```

Both `cargo test` and clippy must be clean before a pull request is merged.

The tests include golden files: programs in `tests/golden/` and
`tests/format/` with their exact expected output. When you change an error
message or the formatter on purpose, regenerate them and check the diff:

```bash
UPDATE_GOLDEN=1 cargo test --test golden --test format
```

Every example in `examples/` and every code sample in `docs/` and the README
is run by `tests/programs.rs`, so the documentation cannot drift from the
language. `UPDATE_DOCS=1 cargo test --test programs` rewrites the expected
outputs shown in the docs.

The fences say what to do with a block:

- ```` ```kstar ```` must run cleanly, and ```` ```kstar-error ```` must fail.
  A ```` ```text ```` block straight after either is its exact output.
- ```` ```kstar file=utils.kstar ```` is not run. It is a file that every
  later block on the same page can import, as if it sat next to it on disk
  (in `docs/errors.md`, every later block in the same section). A later
  block with the same file name replaces it. Use it to show imports:

  ````markdown
  ```kstar file=utils.kstar
  let Size = 2
  ```

  ```kstar
  import "utils.kstar"
  print utils.Size
  ```
  ````

Golden programs can import too: put the imported files in
`tests/golden/modules/`, which the golden test does not run by themselves.

The stress tests are too slow for every run and are marked `#[ignore]`; run
them in a release build, optionally with a fixed seed:

```bash
cargo test --release -- --ignored
```

```bash
KSTAR_SEED=12345 cargo test --release --test robustness -- --ignored
```

The VS Code extension has its own tests: `npm ci` and `npm test` in
`editors/vscode`, after `cargo build`.

## Where things are

```text
src/automata/   the machines and algorithms, as a plain Rust library
src/lang/       the language: lexer, parser, checker, interpreter, formatter, lints
src/ide.rs      analysis shared by the language server
src/main.rs     the kstar command; src/bin/kstar-lsp.rs is the language server
tests/          golden, property, robustness, scale, CLI and IDE tests
docs/           tutorial, reference, error catalogue, internals, design
editors/vscode/ the VS Code extension
installer/      the Windows installer (Inno Setup)
```

[docs/internals.md](docs/internals.md) walks through the source in the
order a program travels through it.

## House rules

- **Library first.** A new machine type or algorithm goes into
  `src/automata/` as plain Rust with its own unit tests, and only then gets
  syntax in `src/lang/`.
- **Every feature ships with its documentation, an example and golden
  tests**: a tutorial section or reference entry, a program in `examples/`
  if it is a whole feature, and golden tests for its output and its errors.
- **Every error has a code** in `src/lang/codes.rs` and a section in
  `docs/errors.md` (which `kstar explain` shows), with a hint where one helps.
- **Code samples in the docs are executed**, so write them as real programs.
- Keep the language small. A new statement or built-in needs a reason that
  a student of the subject would recognise.

## Branches, commits and pull requests

- Work on a branch named for the kind of change: `feat/...`, `fix/...`,
  `docs/...`, `test/...` or `chore/...`. `main` only changes through pull
  requests, which are squash-merged.
- Write commit messages as a short imperative summary ("Add Turing
  machines", "Fix the caret under a tab"), then a blank line and a few
  sentences on what changed and why, if it is not obvious.
- A pull request says what it changes, why, and how it was tested. Keep it
  to one topic; split unrelated changes.

## The changelog

[CHANGELOG.md](CHANGELOG.md) lists what changed in each version, for people
who use Kleene Star. A pull request that changes what users see (a feature,
a fix, a new error or warning, different output) adds a line under
`## [Unreleased]`, in the group that fits: **Added**, **Changed** or
**Fixed**. If existing programs or setups need changing, say how under
**Upgrading**. Write it plainly: what a user notices, not how the code
changed.

## Making a release

A release is a version tag pushed from `main`. The
[release workflow](.github/workflows/release.yml) then tests and builds
everything and publishes the release, with the version's section of the
changelog as its notes.

1. **Write the changelog.** In `CHANGELOG.md`, turn `## [Unreleased]` into
   `## [X.Y.Z] - YYYY-MM-DD`, add a new empty `## [Unreleased]` above it,
   and add a link for the new version at the end of the file. Read the
   section as a user would.
2. **Bump the version** in `Cargo.toml` (then `cargo build` updates
   `Cargo.lock`) and in the extension: in `editors/vscode`, run
   `npm version X.Y.Z --no-git-tag-version`, which updates `package.json`
   and `package-lock.json`.
3. **Update the version where the docs show it:** the status table in the
   README, the examples in `docs/install.md`, the `.vsix` name in
   `editors/vscode/README.md`, and the status line in `docs/design.md`.
4. **Check the notes** the release will get:

   ```bash
   .github/scripts/release-notes.sh vX.Y.Z
   ```

5. **Open a pull request** named "Release X.Y.Z: ..." with these changes,
   wait for CI (it fails if the changelog has no section for the version in
   `Cargo.toml`) and merge it.
6. **Tag the merge on `main`** and push the tag:

   ```bash
   git switch main && git pull
   ```

   ```bash
   git tag vX.Y.Z && git push origin vX.Y.Z
   ```

The tag must match `Cargo.toml`. A plain tag such as `v0.3.0` publishes a
release that GitHub marks as the latest, which is what the install scripts
and `kstar update` offer. A tag with a suffix such as `v0.3.0-beta`
publishes a pre-release, which they skip unless asked for it. Running the
workflow by hand from the Actions tab is a dry run that publishes nothing
and keeps the files as artifacts.

To correct the notes of a published release, fix `CHANGELOG.md` through a
pull request, then:

```bash
.github/scripts/release-notes.sh vX.Y.Z > notes.md && gh release edit vX.Y.Z --notes-file notes.md
```

## Reporting a bug or proposing a change

- **Bugs:** open an issue with the bug report template. Include the
  smallest program that shows the problem, what `kstar` printed, what you
  expected, and the output of `kstar version`.
- **Language changes:** open an issue with the language proposal template
  before writing code. Describe the problem, show the syntax you have in
  mind with an example, and say how it fits with what already exists.
- **Security problems:** do not open a public issue; see
  [SECURITY.md](SECURITY.md).

## Licence

Kleene Star is released under the [MIT licence](LICENSE). By contributing,
you agree that your contributions are licensed under it too.

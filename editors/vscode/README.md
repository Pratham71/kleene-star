# Kleene Star for VS Code

Support for Kleene Star (`.kstar`) files:

- **Highlighting** for machines (`dfa`, `nfa`, `pda`, `tm`), their states and
  transition labels (`0,1`, `a, Z/AZ`, `0/1,R`), grammars (rule heads,
  variables, terminals), regex literals, functions, types and built-ins.
- **Errors and warnings as you type**, with their codes (`kstar explain
  E0200` tells you more), hover (types, signatures, built-in documentation,
  which states start or accept), go to definition, find all references,
  completion, signature help, and the **outline** (machines with their
  states, grammars with their variables, functions, values).
- **Formatting**: Format Document (Shift+Alt+F) gives the same result as
  `kstar fmt`.
- **Snippets** for each kind of block: `alphabet`, `dfa`, `nfa`, `pda`, `tm`,
  `grammar`, `fn`, `fnmachine` (a family of machines), `if`, `for`, `test`,
  `trace`, and `--` for a transition.
- **Run with F5** (or Ctrl+F5): saves the file and runs `kstar run` on it
  in a terminal, exactly as on the command line, with colours.
- **Commands** (Command Palette, or the run button at the top right of a
  `.kstar` file): *Kleene Star: Run File*, *Check File*, *Trace a
  Machine...* (pick a machine, type a word, see every step), *Format File*,
  and *Restart Language Server*. Run and Trace use the terminal (or the
  Kleene Star output panel, with `kstar.runIn`); Check uses the panel.
  If you also use the Code Runner extension, its run button is not this
  one; see [Code Runner](#code-runner).
- The K\* icon for `.kstar` files (see [file icons](#file-icons) if you use
  an icon theme such as Material Icon Theme).

The extension is not on the Marketplace; install it from a `.vsix` file.

## Install

On Windows, the Kleene Star installer (`kstar-setup-x64.exe`) does all of
this for you: it installs `kstar` and `kstar-lsp`, and if VS Code is
installed, it installs this extension too. Otherwise:

1. **Install Kleene Star itself**, which provides the `kstar` command and the
   `kstar-lsp` language server: with the Windows installer or an install
   script from the [main README](../../README.md#installing), or from the
   repository root with:

   ```bash
   cargo install --path .
   ```

   The extension looks on your PATH, in `~/.cargo/bin`, in
   `~/.kstar/bin`, and (on Windows) in
   `%LOCALAPPDATA%\Programs\Kleene Star\bin`. If they are somewhere else,
   set `kstar.path` and `kstar.serverPath` to their full paths.

2. **Get the `.vsix`**: download `kleene-star-<version>.vsix` from a release,
   or build it here:

   ```bash
   cd editors/vscode
   npm ci
   cp ../../LICENSE LICENSE
   npx vsce package
   ```

3. **Install it**:

   ```bash
   code --install-extension kleene-star-0.2.5-beta.vsix
   ```

   or, in VS Code, *Extensions* → `...` → *Install from VSIX...*.

Open any `.kstar` file to start. If `kstar-lsp` cannot be found, a message
says so, with buttons to open the setting or this page.

## File icons

The extension gives `.kstar` files the K\* icon, which VS Code shows with
its built-in icon themes. Icon themes that draw their own icons, such as
Material Icon Theme, ignore it. To get it with Material Icon Theme, copy
`images/kstar-file.svg` to `~/.vscode/extensions/icons/kstar.svg` and add
this to your settings:

```json
"material-icon-theme.files.associations": {
  "*.kstar": "../../icons/kstar"
}
```

## Settings

| Setting | Default | Meaning |
|---|---|---|
| `kstar.serverPath` | (empty) | Full path to `kstar-lsp`. Empty: search the PATH and the install folders. |
| `kstar.path` | (empty) | Full path to `kstar`, used by Run, Check and Trace. Empty: search as above. |
| `kstar.runIn` | `terminal` | Where Run and Trace run `kstar`: `terminal`, or `output` for the Kleene Star output panel. |

## Code Runner

The Code Runner extension adds its own run button, which does not know
Kleene Star. Use F5 or the Kleene Star button instead, or teach Code Runner
the command in your settings:

```json
"code-runner.executorMap": {
  "kstar": "kstar run"
}
```

(Add the `"kstar"` line to your existing `code-runner.executorMap` if you
have one.)

## Developing

`npm test` checks the highlighting with VS Code's own TextMate engine,
checks that every snippet expands to a valid program, and talks to the
language server over the protocol (the last two use the `kstar` and
`kstar-lsp` from `cargo build`). To try changes, open this folder in VS Code
and press F5, which starts a second VS Code window with the extension
loaded.

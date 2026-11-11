# Kleene Star for VS Code

Syntax highlighting for `.kstar` files, plus everything the `kstar-lsp` language
server offers: errors as you type (with hints), hover (types, signatures,
built-in docs, state info), go to definition, find all references,
completion, and signature help.

This extension is not published anywhere; install it from this folder.

## Install

1. Build and install the language server (from the repository root):

   ```bash
   cargo install --path .
   ```

   This puts `kstar` and `kstar-lsp` in `~/.cargo/bin`, which is on your PATH.
   Otherwise, set the `kstar.serverPath` setting to the full path of `kstar-lsp`.

2. Install the extension's one dependency:

   ```bash
   cd editors/vscode && npm install
   ```

3. Link this folder into VS Code's extensions folder, then restart VS Code:

   ```bash
   ln -s "$PWD" ~/.vscode/extensions/kleene-star
   ```

   On Windows (PowerShell): `New-Item -ItemType Junction -Path "$env:USERPROFILE\.vscode\extensions\kleene-star" -Target (Get-Location)`.

Open any `.kstar` file to start the server.

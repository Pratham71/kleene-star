# TOC-Lang for VS Code

Syntax highlighting for `.toc` files, plus everything the `toc-lsp` language
server offers: errors as you type (with hints), hover (types, signatures,
built-in docs, state info), go to definition, find all references,
completion, and signature help.

This extension is not published anywhere; install it from this folder.

## Install

1. Build and install the language server (from the repository root):

   ```bash
   cargo install --path .
   ```

   This puts `toc` and `toc-lsp` in `~/.cargo/bin`, which is on your PATH.
   Otherwise, set the `toc.serverPath` setting to the full path of `toc-lsp`.

2. Install the extension's one dependency:

   ```bash
   cd editors/vscode && npm install
   ```

3. Link this folder into VS Code's extensions folder, then restart VS Code:

   ```bash
   ln -s "$PWD" ~/.vscode/extensions/toc-lang
   ```

   On Windows (PowerShell): `New-Item -ItemType Junction -Path "$env:USERPROFILE\.vscode\extensions\toc-lang" -Target (Get-Location)`.

Open any `.toc` file to start the server.

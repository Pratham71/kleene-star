// The TOC-Lang extension: starts the `toc-lsp` language server for .toc
// files. Everything clever happens in the server; this file only connects
// VS Code to it.

const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");

let client;

function activate(context) {
  const command = vscode.workspace.getConfiguration("toc").get("serverPath") || "toc-lsp";
  const server = { command, args: [] };
  client = new LanguageClient(
    "toc",
    "TOC-Lang",
    { run: server, debug: server },
    { documentSelector: [{ scheme: "file", language: "toc" }] }
  );
  client.start().catch((error) => {
    vscode.window.showErrorMessage(
      `TOC-Lang: could not start "${command}" (${error.message}). ` +
        "Install toc-lsp and put it on your PATH, or set toc.serverPath."
    );
  });
  context.subscriptions.push({ dispose: () => client && client.stop() });
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };

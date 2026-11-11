// The Kleene Star extension: starts the `kstar-lsp` language server for .kstar
// files. Everything clever happens in the server; this file only connects
// VS Code to it.

const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");

let client;

function activate(context) {
  const command = vscode.workspace.getConfiguration("kstar").get("serverPath") || "kstar-lsp";
  const server = { command, args: [] };
  client = new LanguageClient(
    "kstar",
    "Kleene Star",
    { run: server, debug: server },
    { documentSelector: [{ scheme: "file", language: "kstar" }] }
  );
  client.start().catch((error) => {
    vscode.window.showErrorMessage(
      `Kleene Star: could not start "${command}" (${error.message}). ` +
        "Install kstar-lsp and put it on your PATH, or set kstar.serverPath."
    );
  });
  context.subscriptions.push({ dispose: () => client && client.stop() });
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };

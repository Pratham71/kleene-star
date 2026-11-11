// Talks to the real `kstar-lsp` over the Language Server Protocol, the way
// VS Code does: diagnostics (errors and warnings with codes), the outline,
// and formatting. Uses the server built by `cargo build`, or the one named
// by KSTAR_LSP; without either, the test is skipped.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");
const rpc = require("vscode-jsonrpc/node");

const exe = process.platform === "win32" ? "kstar-lsp.exe" : "kstar-lsp";
const server = process.env.KSTAR_LSP || path.join(__dirname, "..", "..", "..", "target", "debug", exe);

test("diagnostics, outline and formatting", { skip: !fs.existsSync(server) && `no ${server}` }, async () => {
  const child = spawn(server, [], { stdio: ["pipe", "pipe", "inherit"] });
  const connection = rpc.createMessageConnection(
    new rpc.StreamMessageReader(child.stdout),
    new rpc.StreamMessageWriter(child.stdin)
  );
  const diagnostics = new Promise((resolve) =>
    connection.onNotification("textDocument/publishDiagnostics", (params) => resolve(params.diagnostics))
  );
  connection.listen();
  try {
    const init = await connection.sendRequest("initialize", { processId: null, rootUri: null, capabilities: {} });
    assert.ok(init.capabilities.documentFormattingProvider);
    assert.ok(init.capabilities.documentSymbolProvider);
    connection.sendNotification("initialized", {});

    const uri = "file:///demo.kstar";
    const text = "alphabet B = {0, 1}\nlet spare=1\ndfa M over B {\n start q0\n accept q0\n q0 --0,1--> q0\n}\nprint M\n";
    connection.sendNotification("textDocument/didOpen", {
      textDocument: { uri, languageId: "kstar", version: 1, text },
    });

    const found = await diagnostics;
    assert.strictEqual(found.length, 1, JSON.stringify(found));
    assert.strictEqual(found[0].severity, 2, "a warning");
    assert.strictEqual(found[0].code, "W0200");
    assert.strictEqual(found[0].range.start.line, 1);

    const symbols = await connection.sendRequest("textDocument/documentSymbol", { textDocument: { uri } });
    assert.deepStrictEqual(symbols.map((s) => s.name), ["B", "spare", "M"]);
    assert.deepStrictEqual(symbols[2].children.map((s) => s.name), ["q0"]);
    assert.strictEqual(symbols[2].detail, "dfa over B");

    const edits = await connection.sendRequest("textDocument/formatting", {
      textDocument: { uri },
      options: { tabSize: 4, insertSpaces: true },
    });
    assert.strictEqual(edits.length, 1);
    assert.strictEqual(
      edits[0].newText,
      "alphabet B = {0, 1}\nlet spare = 1\ndfa M over B {\n    start  q0\n    accept q0\n    q0 --0,1--> q0\n}\nprint M\n"
    );

    await connection.sendRequest("shutdown");
  } finally {
    // Stop listening before the server goes away, so nothing is written
    // to a closed pipe.
    connection.dispose();
    const exited = new Promise((resolve) => child.on("exit", resolve));
    child.kill();
    await exited;
  }
});

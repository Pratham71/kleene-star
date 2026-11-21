// Every language feature the README promises, asked of the real
// `kstar-lsp` the way VS Code asks: hover, go to definition, find
// references, completion (plain, after `std.`, after `std.<module>.` and
// after an import's name), and signature help. `server.test.js` covers
// diagnostics, the outline and formatting. Skipped without a built server.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { pathToFileURL } = require("url");
const { spawn } = require("child_process");
const rpc = require("vscode-jsonrpc/node");

const exe = process.platform === "win32" ? "kstar-lsp.exe" : "kstar-lsp";
const server = process.env.KSTAR_LSP || path.join(__dirname, "..", "..", "..", "target", "debug", exe);

/// Starts the server, opens `text` as a file in a folder that also holds
/// `lib.kstar`, and runs `body` with a function that sends a request at a
/// position given by a marker string: the position is where `marker`
/// starts, plus `shift` characters.
async function withServer(text, body) {
  const folder = fs.mkdtempSync(path.join(os.tmpdir(), "kstar-features-"));
  fs.writeFileSync(path.join(folder, "lib.kstar"), "alphabet B = {0, 1}\nfn double(n: Int) -> Int {\n    return 2 * n\n}\n");
  const file = path.join(folder, "main.kstar");
  fs.writeFileSync(file, text);
  const uri = pathToFileURL(file).href;
  const child = spawn(server, [], { stdio: ["pipe", "pipe", "inherit"] });
  const connection = rpc.createMessageConnection(new rpc.StreamMessageReader(child.stdout), new rpc.StreamMessageWriter(child.stdin));
  const opened = new Promise((resolve) => connection.onNotification("textDocument/publishDiagnostics", resolve));
  connection.listen();
  try {
    const init = await connection.sendRequest("initialize", { processId: null, rootUri: null, capabilities: {} });
    connection.sendNotification("initialized", {});
    connection.sendNotification("textDocument/didOpen", { textDocument: { uri, languageId: "kstar", version: 1, text } });
    await opened;
    const at = (marker, shift = 0) => {
      const index = text.indexOf(marker);
      assert.ok(index >= 0, `no ${marker} in the text`);
      const before = text.slice(0, index + shift).split("\n");
      return { line: before.length - 1, character: before[before.length - 1].length };
    };
    const ask = (method, marker, shift, extra = {}) =>
      connection.sendRequest(method, { textDocument: { uri }, position: at(marker, shift), ...extra });
    await body({ init, ask, at, uri });
    await connection.sendRequest("shutdown");
  } finally {
    connection.dispose();
    const exited = new Promise((resolve) => child.on("exit", resolve));
    child.kill();
    await exited;
    fs.rmSync(folder, { recursive: true, force: true });
  }
}

const skip = !fs.existsSync(server) && `no ${server}`;

const program = [
  'import "lib.kstar" as lib',
  "alphabet B = {0, 1}",
  "dfa Even over B {",
  "    start  e",
  "    accept e",
  "    e --0--> e",
  "    e --1--> o",
  "    o --0--> o",
  "    o --1--> e",
  "}",
  'print Even accepts "11"',
  "print lib.double(2)",
  "print std.math.gcd(12, 18)",
  'let G = std.grammar.anbn("a", "b")',
  'print std.grammar.first(G, "S")',
  "",
].join("\n");

test("the server says which features it has", { skip }, async () => {
  await withServer(program, async ({ init }) => {
    const c = init.capabilities;
    for (const feature of ["hoverProvider", "definitionProvider", "referencesProvider", "documentSymbolProvider", "documentFormattingProvider"]) {
      assert.ok(c[feature], `no ${feature}`);
    }
    assert.ok(c.completionProvider, "no completion");
    // Without `.` as a trigger, VS Code shows members of `std.` or of an
    // imported module only after Ctrl+Space.
    assert.ok((c.completionProvider.triggerCharacters || []).includes("."), "completion is not triggered by `.`");
    assert.deepStrictEqual(c.signatureHelpProvider.triggerCharacters, ["(", ","]);
    assert.ok(init.serverInfo && /^\d+\.\d+\.\d+/.test(init.serverInfo.version), "the server reports its version");
  });
});

test("hover, definition and references", { skip }, async () => {
  await withServer(program, async ({ ask }) => {
    const hover = await ask("textDocument/hover", "Even accepts", 1);
    assert.match(JSON.stringify(hover.contents), /dfa/i);
    const builtin = await ask("textDocument/hover", "gcd(", 1);
    assert.match(JSON.stringify(builtin.contents), /gcd/);
    const keywordModule = await ask("textDocument/hover", "first(", 1);
    assert.match(JSON.stringify(keywordModule.contents), /first/, "hover on a std.grammar function");

    const definition = await ask("textDocument/definition", "Even accepts", 1);
    const target = Array.isArray(definition) ? definition[0] : definition;
    assert.strictEqual(target.range.start.line, 2, "Even is defined on line 3");

    const references = await ask("textDocument/references", "Even over", 1, { context: { includeDeclaration: true } });
    assert.strictEqual(references.length, 2);
  });
});

test("completion, also after `std.`, `std.<module>.` and an import's name", { skip }, async () => {
  await withServer(program, async ({ ask }) => {
    const labels = (result) => (Array.isArray(result) ? result : result.items).map((item) => item.label);

    const plain = labels(await ask("textDocument/completion", "print lib", 0));
    for (const expected of ["Even", "B", "lib", "std", "to_dfa", "to_json", "print"]) {
      assert.ok(plain.includes(expected), `${expected} not offered: ${plain.join(", ")}`);
    }
    const modules = labels(await ask("textDocument/completion", "std.math", 4));
    for (const expected of ["math", "automata", "grammar", "pda", "tm", "random", "testing"]) {
      assert.ok(modules.includes(expected), `std.${expected} not offered: ${modules.join(", ")}`);
    }
    const math = labels(await ask("textDocument/completion", "std.math.gcd", 9));
    assert.ok(math.includes("gcd") && math.includes("is_prime"), math.join(", "));
    const grammar = labels(await ask("textDocument/completion", "std.grammar.first", 12));
    assert.ok(grammar.includes("first") && grammar.includes("follow"), grammar.join(", "));
    const lib = labels(await ask("textDocument/completion", "lib.double", 4));
    assert.ok(lib.includes("double") && lib.includes("B"), lib.join(", "));
  });
});

test("signature help", { skip }, async () => {
  await withServer(program, async ({ ask }) => {
    const help = await ask("textDocument/signatureHelp", "gcd(12", 4);
    assert.match(help.signatures[0].label, /gcd\(a: Int, b: Int\)/);
    const library = await ask("textDocument/signatureHelp", 'first(G, "S")', 9);
    assert.match(library.signatures[0].label, /first\(g: Grammar, variable: Str\)/);
    assert.strictEqual(library.activeParameter, 1);
  });
});

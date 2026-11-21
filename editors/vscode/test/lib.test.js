// The extension's logic that needs no VS Code: which outline entries the
// Trace command offers, and the warning when the language server and the
// extension come from different releases.

const { test } = require("node:test");
const assert = require("node:assert");
const { traceable, parseVersion, versionMismatch } = require("../lib.js");

// VS Code's SymbolKind numbers.
const CLASS = 4;
const VARIABLE = 12;
const STRUCT = 22;

test("Trace offers machines, also those made by an expression", () => {
  const outline = [
    { name: "B", kind: 9, detail: "{0, 1}" },
    { name: "Even", kind: CLASS, detail: "dfa over B" },
    { name: "G", kind: STRUCT, detail: "grammar over B" },
    { name: "Small", kind: VARIABLE, detail: "Dfa" },
    { name: "P", kind: VARIABLE, detail: "Pda" },
    { name: "count", kind: VARIABLE, detail: "Int" },
    { name: "r", kind: VARIABLE, detail: "Regex" },
  ];
  assert.deepStrictEqual(traceable(outline, CLASS, VARIABLE), ["Even", "Small", "P"]);
  assert.deepStrictEqual(traceable(undefined, CLASS, VARIABLE), []);
});

test("versions are read the way releases write them", () => {
  assert.deepStrictEqual(parseVersion("0.5.0"), [0, 5, 0]);
  assert.deepStrictEqual(parseVersion("v0.10.2"), [0, 10, 2]);
  assert.deepStrictEqual(parseVersion("0.2.5-beta"), [0, 2, 5]);
  assert.strictEqual(parseVersion("dev"), undefined);
});

test("a server and an extension from different releases are warned about", () => {
  assert.strictEqual(versionMismatch("0.5.1", "0.5.1"), undefined);
  assert.strictEqual(versionMismatch("0.5.1", undefined), undefined);
  // The user's case: an old extension with a new server.
  const old = versionMismatch("0.2.3", "0.5.0");
  assert.match(old, /server is 0\.5\.0 but this extension is 0\.2\.3/);
  assert.match(old, /kstar update/);
  assert.match(old, /kleene-star-0\.5\.0\.vsix/);
  // The other way round: the extension is newer.
  const newer = versionMismatch("0.6.0", "0.5.1");
  assert.match(newer, /Update Kleene Star itself to 0\.6\.0/);
  assert.match(versionMismatch("0.5.10", "0.5.9"), /Update Kleene Star itself/);
});

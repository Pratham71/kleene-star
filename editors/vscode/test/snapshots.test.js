// Snapshot tests of the highlighting: every program in examples/ is
// tokenized with VS Code's own TextMate engine, and the scopes of every
// token are compared with test/snapshots/<file>.snap. A change to the
// grammar that changes how any example is coloured fails here, and so does
// a token with no scope at all (shown as `(none)`).
//
// After a deliberate change, run `UPDATE_SNAPSHOTS=1 npm test` and review
// the diff of test/snapshots/.
//
// The same file checks that every scope name the grammar gives is one of
// the standard TextMate names, which colour themes know, so that the
// colours show in any theme and not only in the default one.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const textmate = require("vscode-textmate");
const oniguruma = require("vscode-oniguruma");

const root = path.join(__dirname, "..");
const examples = path.join(root, "..", "..", "examples");
const snapshots = path.join(__dirname, "snapshots");
const grammarFile = path.join(root, "syntaxes", "kstar.tmLanguage.json");

let grammarPromise;
function loadGrammar() {
  grammarPromise ??= (async () => {
    const wasm = fs.readFileSync(require.resolve("vscode-oniguruma/release/onig.wasm")).buffer;
    await oniguruma.loadWASM(wasm);
    const registry = new textmate.Registry({
      onigLib: Promise.resolve({
        createOnigScanner: (patterns) => new oniguruma.OnigScanner(patterns),
        createOnigString: (s) => new oniguruma.OnigString(s),
      }),
      loadGrammar: async () => textmate.parseRawGrammar(fs.readFileSync(grammarFile, "utf8"), "kstar.tmLanguage.json"),
    });
    return registry.loadGrammar("source.kstar");
  })();
  return grammarPromise;
}

/// Every .kstar file under `dir`, as paths relative to it, sorted.
function programs(dir, prefix = "") {
  const found = [];
  for (const name of fs.readdirSync(dir).sort()) {
    const full = path.join(dir, name);
    if (fs.statSync(full).isDirectory()) found.push(...programs(full, prefix + name + "/"));
    else if (name.endsWith(".kstar")) found.push(prefix + name);
  }
  return found;
}

/// The snapshot of `source`: each line, then one line per token that is not
/// blank, with its column, its text and its scopes (without `source.kstar`).
async function snapshot(source) {
  const grammar = await loadGrammar();
  let stack = textmate.INITIAL;
  const out = [];
  for (const line of source.replace(/\r\n/g, "\n").split("\n")) {
    const result = grammar.tokenizeLine(line, stack);
    out.push(`> ${line}`);
    for (const t of result.tokens) {
      const text = line.slice(t.startIndex, t.endIndex);
      if (!text.trim()) continue;
      const scopes = t.scopes.slice(1);
      out.push(`  ${String(t.startIndex).padStart(3)} ${JSON.stringify(text)} ${scopes.length ? scopes.join(" ") : "(none)"}`);
    }
    stack = result.ruleStack;
  }
  return out.join("\n") + "\n";
}

const update = !!process.env.UPDATE_SNAPSHOTS;

// The examples, and test/fixtures/, which holds what the examples do not
// show (escapes, every keyword, blanks on a tape).
const fixtures = path.join(__dirname, "fixtures");
const sources = [
  ...programs(examples).map((p) => ({ shown: `examples/${p}`, file: path.join(examples, p), snap: p.replace(/\//g, "__") })),
  ...programs(fixtures).map((p) => ({ shown: `test/fixtures/${p}`, file: path.join(fixtures, p), snap: "fixture__" + p.replace(/\//g, "__") })),
];

for (const { shown, file: source, snap } of sources) {
  const program = shown;
  test(`highlighting of ${shown}`, async () => {
    const actual = await snapshot(fs.readFileSync(source, "utf8"));
    const unscoped = actual.split("\n").filter((l) => l.endsWith(" (none)"));
    assert.deepStrictEqual(unscoped, [], `tokens with no scope in ${program}`);
    const file = path.join(snapshots, snap + ".snap");
    if (update) {
      fs.mkdirSync(snapshots, { recursive: true });
      fs.writeFileSync(file, actual);
      return;
    }
    assert.ok(fs.existsSync(file), `no snapshot for ${program}; run UPDATE_SNAPSHOTS=1 npm test`);
    const expected = fs.readFileSync(file, "utf8").replace(/\r\n/g, "\n");
    assert.strictEqual(actual, expected, `the highlighting of ${program} changed; if that is intended, run UPDATE_SNAPSHOTS=1 npm test`);
  });
}

test("every snapshot belongs to an example", () => {
  if (update || !fs.existsSync(snapshots)) return;
  const wanted = new Set(sources.map((s) => s.snap + ".snap"));
  const stale = fs.readdirSync(snapshots).filter((f) => !wanted.has(f));
  assert.deepStrictEqual(stale, [], "snapshots without an example; delete them");
});

// The standard TextMate scope names: the first part, and for most of them
// the second, are the ones themes give colours to. See the TextMate manual,
// "Naming Conventions".
const STANDARD = {
  comment: ["line", "block"],
  constant: ["numeric", "character", "language", "other"],
  entity: ["name", "other"],
  invalid: ["illegal", "deprecated"],
  keyword: ["control", "operator", "other"],
  meta: null,
  punctuation: null,
  storage: ["type", "modifier"],
  string: ["quoted", "unquoted", "interpolated", "regexp", "other"],
  support: ["function", "class", "type", "constant", "variable", "other"],
  variable: ["parameter", "language", "other"],
};
const ENTITY_NAMES = ["function", "type", "tag", "section", "namespace", "class"];

/// Every scope name in the grammar, from `name`, `contentName` and captures.
function scopeNames(node, found = new Set()) {
  if (Array.isArray(node)) node.forEach((n) => scopeNames(n, found));
  else if (node && typeof node === "object") {
    for (const [key, value] of Object.entries(node)) {
      if ((key === "name" || key === "contentName") && typeof value === "string") found.add(value);
      else scopeNames(value, found);
    }
  }
  return found;
}

test("every scope name is a standard one, so any theme colours it", () => {
  const grammar = JSON.parse(fs.readFileSync(grammarFile, "utf8"));
  const names = [...scopeNames(grammar.repository), ...scopeNames(grammar.patterns)].filter((n) => n !== grammar.name);
  assert.ok(names.length > 30, `only ${names.length} scope names found`);
  for (const name of names) {
    const parts = name.split(".");
    assert.strictEqual(parts[parts.length - 1], "kstar", `${name} should end in .kstar`);
    assert.ok(parts[0] in STANDARD, `${name}: ${parts[0]} is not a standard scope`);
    const seconds = STANDARD[parts[0]];
    if (seconds) assert.ok(seconds.includes(parts[1]), `${name}: ${parts[0]}.${parts[1]} is not a standard scope`);
    if (parts[0] === "entity" && parts[1] === "name") {
      assert.ok(ENTITY_NAMES.includes(parts[2]), `${name}: entity.name.${parts[2]} is not a standard scope`);
    }
  }
});

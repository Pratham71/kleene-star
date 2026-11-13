// Tests of the syntax highlighting, using the same TextMate engine as VS
// Code. Each case tokenizes a few lines and checks the scope given to some
// pieces of text. Run with `npm test`.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const textmate = require("vscode-textmate");
const oniguruma = require("vscode-oniguruma");

const root = path.join(__dirname, "..");

async function loadGrammar() {
  const wasm = fs.readFileSync(require.resolve("vscode-oniguruma/release/onig.wasm")).buffer;
  await oniguruma.loadWASM(wasm);
  const registry = new textmate.Registry({
    onigLib: Promise.resolve({
      createOnigScanner: (patterns) => new oniguruma.OnigScanner(patterns),
      createOnigString: (s) => new oniguruma.OnigString(s),
    }),
    loadGrammar: async () =>
      textmate.parseRawGrammar(
        fs.readFileSync(path.join(root, "syntaxes", "kstar.tmLanguage.json"), "utf8"),
        "kstar.tmLanguage.json"
      ),
  });
  return registry.loadGrammar("source.kstar");
}

/// Every token of `source`, as [text, innermost scope, all scopes].
async function tokens(source) {
  const grammar = await loadGrammar();
  let stack = textmate.INITIAL;
  const out = [];
  for (const line of source.split("\n")) {
    const result = grammar.tokenizeLine(line, stack);
    for (const t of result.tokens) {
      const text = line.slice(t.startIndex, t.endIndex);
      out.push([text, t.scopes[t.scopes.length - 1], t.scopes]);
    }
    stack = result.ruleStack;
  }
  return out;
}

/// The innermost scope of the `nth` token whose text is `text`.
function scopeOf(all, text, nth = 0) {
  const found = all.filter(([t]) => t.trim() === text);
  assert.ok(found.length > nth, `no token ${JSON.stringify(text)} (number ${nth}) in ${JSON.stringify(all.map((t) => t[0]))}`);
  return found[nth][1];
}

test("machines: header, roles, states and labels", async () => {
  const all = await tokens(`dfa Even over B {
    start  even
    accept even
    even --0,1--> odd   # comment
}`);
  assert.strictEqual(scopeOf(all, "dfa"), "storage.type.machine.kstar");
  assert.strictEqual(scopeOf(all, "Even"), "entity.name.type.machine.kstar");
  assert.strictEqual(scopeOf(all, "B"), "entity.name.type.alphabet.kstar");
  assert.strictEqual(scopeOf(all, "start"), "keyword.control.state-role.kstar");
  assert.strictEqual(scopeOf(all, "even"), "variable.other.state.kstar");
  assert.strictEqual(scopeOf(all, "--"), "keyword.operator.transition.begin.kstar");
  assert.strictEqual(scopeOf(all, "0"), "constant.character.symbol.kstar");
  assert.strictEqual(scopeOf(all, "-->"), "keyword.operator.transition.end.kstar");
  assert.strictEqual(scopeOf(all, "odd"), "variable.other.state.kstar");
  assert.strictEqual(scopeOf(all, "# comment"), "comment.line.number-sign.kstar");
});

test("PDA and Turing machine labels", async () => {
  const all = await tokens(`pda P over AB {
    p --a, Z/AZ--> p
    p --eps, eps/A--> q
}
tm T over B {
    s --0/1,R--> s
    s --_/_,S--> t
}`);
  assert.strictEqual(scopeOf(all, "/"), "keyword.operator.stack.kstar");
  assert.strictEqual(scopeOf(all, "eps"), "constant.language.eps.kstar");
  assert.strictEqual(scopeOf(all, "R"), "keyword.other.direction.kstar");
  assert.strictEqual(scopeOf(all, "_"), "constant.character.blank.kstar");
  assert.strictEqual(scopeOf(all, "tm"), "storage.type.machine.kstar");
});

test("machine families: computed states and loops", async () => {
  const all = await tokens(`fn div(n: Int) -> Dfa {
    return dfa over B {
        start q[0]
        for i in 0..n {
            q[i] --0--> q[(2 * i) % n]
        }
        accept q[0]
    }
}
print div(3)`);
  assert.strictEqual(scopeOf(all, "div"), "entity.name.function.kstar");
  assert.strictEqual(scopeOf(all, "n"), "variable.parameter.kstar");
  assert.strictEqual(scopeOf(all, "Int"), "support.type.kstar");
  assert.strictEqual(scopeOf(all, "Dfa"), "support.type.kstar");
  assert.strictEqual(scopeOf(all, "q"), "variable.other.state.kstar");
  assert.strictEqual(scopeOf(all, "for"), "keyword.control.kstar");
  // After the inner loop's `}` the machine continues: `accept` is still a role.
  assert.strictEqual(scopeOf(all, "accept"), "keyword.control.state-role.kstar");
  // And after the machine, ordinary code: `print` is a keyword again.
  assert.strictEqual(scopeOf(all, "print"), "keyword.control.kstar");
});

test("grammars: rule heads, variables and terminals", async () => {
  const all = await tokens(`grammar G over AB {
    S -> a S b | eps
}`);
  assert.strictEqual(scopeOf(all, "G"), "entity.name.type.grammar.kstar");
  assert.strictEqual(scopeOf(all, "S"), "entity.name.function.rule.kstar");
  assert.strictEqual(scopeOf(all, "S", 1), "variable.other.nonterminal.kstar");
  assert.strictEqual(scopeOf(all, "a"), "constant.character.terminal.kstar");
  assert.strictEqual(scopeOf(all, "|"), "keyword.operator.alternative.kstar");
  assert.strictEqual(scopeOf(all, "eps"), "constant.language.eps.kstar");
});

test("regex literals, strings, calls and operators", async () => {
  const all = await tokens(`alphabet B = {0, 1}
let r = /(0|1)*01/
test minimize(r) accepts "0101" and not equivalent(r, /0*/)
print len("a\\"b") + 1`);
  assert.strictEqual(scopeOf(all, "alphabet"), "storage.type.alphabet.kstar");
  assert.strictEqual(scopeOf(all, "0"), "constant.character.symbol.kstar");
  assert.ok(all.some(([t, s]) => t === "*" && s === "keyword.operator.quantifier.regexp.kstar"));
  assert.ok(all.some(([t, , scopes]) => t === "01" && scopes.includes("string.regexp.kstar")));
  assert.strictEqual(scopeOf(all, "minimize"), "support.function.builtin.kstar");
  assert.strictEqual(scopeOf(all, "accepts"), "keyword.operator.word.kstar");
  assert.strictEqual(scopeOf(all, '\\"'), "constant.character.escape.kstar");
  assert.strictEqual(scopeOf(all, "+"), "keyword.operator.arithmetic.kstar");
  const ones = all.filter(([t]) => t.trim() === "1");
  assert.strictEqual(ones[ones.length - 1][1], "constant.numeric.integer.kstar");
});

test("imports and qualified names", async () => {
  const all = await tokens(`import "lib/parity.kstar" as p
from std.math import gcd
dfa D over p.B {
    start q
}
print p.EvenZeros accepts "0", gcd(4, 6)`);
  assert.strictEqual(scopeOf(all, "import"), "keyword.control.import.kstar");
  assert.strictEqual(scopeOf(all, "as"), "keyword.control.import.kstar");
  assert.strictEqual(scopeOf(all, "from"), "keyword.control.import.kstar");
  assert.strictEqual(scopeOf(all, "std"), "entity.name.namespace.kstar");
  assert.strictEqual(scopeOf(all, "p.B"), "entity.name.type.alphabet.kstar");
  assert.strictEqual(scopeOf(all, "p", 1), "entity.name.namespace.kstar");
  assert.strictEqual(scopeOf(all, "start"), "keyword.control.state-role.kstar");
  assert.strictEqual(scopeOf(all, "gcd"), "entity.name.function.call.kstar");
});

test("every example tokenizes, and no machine or grammar is left open", async () => {
  const grammar = await loadGrammar();
  const examples = path.join(root, "..", "..", "examples");
  for (const name of fs.readdirSync(examples).filter((f) => f.endsWith(".kstar"))) {
    let stack = textmate.INITIAL;
    for (const line of fs.readFileSync(path.join(examples, name), "utf8").replace(/\r\n/g, "\n").split("\n")) {
      stack = grammar.tokenizeLine(line, stack).ruleStack;
    }
    // At the end of a file everything is closed again.
    const last = grammar.tokenizeLine("print 1", stack).tokens[0].scopes;
    assert.deepStrictEqual(last, ["source.kstar", "keyword.control.kstar"], `${name} leaves ${last.join(" ")} open`);
  }
});

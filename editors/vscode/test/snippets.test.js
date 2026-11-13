// Every snippet, expanded with its default values, must be a valid Kleene
// Star program (with a little context around it). This uses the `kstar`
// built by `cargo build`, or the one named by the KSTAR environment
// variable; without either, the test is skipped.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { execFileSync } = require("child_process");

const root = path.join(__dirname, "..");
const exe = process.platform === "win32" ? "kstar.exe" : "kstar";
const kstar = process.env.KSTAR || path.join(root, "..", "..", "target", "debug", exe);

/// What VS Code inserts when every placeholder keeps its default: `${1:x}`
/// becomes `x`, a choice `${1|a,b|}` its first option, and `$0` nothing.
function expand(body) {
  let text = body.join("\n");
  let previous;
  do {
    previous = text;
    text = text
      .replace(/\$\{\d+\|([^,|]*)[^}]*\|\}/g, "$1")
      .replace(/\$\{\d+:((?:[^}\\]|\\.)*)\}/g, "$1")
      .replace(/\$\d+/g, "");
  } while (text !== previous);
  return text.replace(/\\([}$\\])/g, "$1");
}

const prelude = "alphabet B = {0, 1}\nalphabet AB = {a, b}\n";
const machine = "dfa M over B { start q0  accept q0  q0 --0,1--> q0 }\n";
/// Snippets that only make sense inside something else, or about a machine.
const contexts = {
  Transition: (s) => `nfa X over B {\n    start q0\n    accept q1\n    ${s}\n}`,
  Test: (s) => machine + s,
  Trace: (s) => machine + s,
};

test("every snippet expands to a valid program", { skip: !fs.existsSync(kstar) && `no ${kstar}` }, () => {
  const snippets = JSON.parse(fs.readFileSync(path.join(root, "snippets", "kstar.json"), "utf8"));
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "kstar-snippets-"));
  // The import snippets import this file.
  fs.writeFileSync(path.join(dir, "utils.kstar"), "let Size = 1\n");
  for (const [name, snippet] of Object.entries(snippets)) {
    const body = expand(snippet.body);
    const wrapped = (contexts[name] || ((s) => s))(body);
    const program = name === "Alphabet" ? wrapped : prelude + wrapped + "\n";
    const file = path.join(dir, "snippet.kstar");
    fs.writeFileSync(file, program);
    try {
      execFileSync(kstar, ["check", "--color", "never", file], { stdio: "pipe" });
    } catch (error) {
      assert.fail(`snippet "${name}" is not valid:\n${program}\n${error.stderr}`);
    }
  }
});

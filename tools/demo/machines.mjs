// Runs the programs behind the animations with the real `kstar` and turns
// what they print into machines.js, which the animations read:
//
//   demo.kstar  ->  window.MACHINES: the eps-NFA, DFA and minimal DFA (demo.gif)
//   div3.kstar  ->  window.DIV3: its source, its DFA, its trace and what
//                   `draw` printed (draw.gif and trace.gif)
//
// Nothing in the animations is typed in by hand except where each state is
// placed on screen.
//
//   node machines.mjs            uses the debug build: cargo run from the repo root
//   KSTAR=path/to/kstar node machines.mjs

import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");

// The output of `kstar run` on one program here. Files that `draw` writes go
// to a temporary folder, not into the repository.
export function runKstar(file) {
  const program = join(here, file);
  const out = mkdtempSync(join(tmpdir(), "kstar-demo-"));
  const args = ["run", "--out", out, program];
  const [cmd, all] = process.env.KSTAR
    ? [process.env.KSTAR, args]
    : ["cargo", ["run", "--quiet", "--bin", "kstar", "--", ...args]];
  const output = execFileSync(cmd, all, { cwd: root, encoding: "utf8" });
  const tests = output.split(/\r?\n/).find((l) => l.startsWith("tests:"));
  if (!tests || !/ 0 failed/.test(tests)) throw new Error(`${file}'s tests did not all pass:\n${output}`);
  return output;
}

// One printed machine: a header line, a column header, then one row per state:
//   "-> *{q0,q1,q2} | {q3,q4} | {}"
function parseTable(lines) {
  const header = lines[0].trim();
  const symbols = lines[1].split("|").slice(1).map((s) => s.trim());
  const states = [];
  let start = null;
  const accept = [];
  const edges = []; // { from, to, label }
  for (const row of lines.slice(2)) {
    const cells = row.split("|");
    let name = cells[0].trim();
    const isStart = name.startsWith("->");
    if (isStart) name = name.slice(2).trim();
    if (name.startsWith("*")) { name = name.slice(1); accept.push(name); }
    if (isStart) start = name;
    states.push(name);
    cells.slice(1).forEach((cell, i) => {
      const value = cell.trim();
      if (value === "-") return;
      // NFA cells are sets "{q1, q2}"; DFA cells are single states, which may
      // themselves be named after sets ("{q3,q4}", "{}").
      const targets = /, /.test(value) || header.startsWith("eps-NFA")
        ? value.replace(/^\{|\}$/g, "").split(", ").filter(Boolean)
        : [value];
      for (const to of targets) {
        const label = symbols[i] === "eps" ? "ε" : symbols[i];
        const same = edges.find((e) => e.from === name && e.to === to);
        if (same) same.label += ", " + label; else edges.push({ from: name, to, label });
      }
    });
  }
  return { header, states, start, accept, edges };
}

// Every machine the output prints, in order.
export function parseMachines(output) {
  const machines = [];
  let current = null;
  for (const line of output.split(/\r?\n/)) {
    if (/^(eps-NFA|NFA|DFA) with \d+ states/.test(line)) {
      current = [line];
      machines.push(current);
    } else if (current && line.includes("|")) {
      current.push(line);
    } else {
      current = null;
    }
  }
  return machines.map(parseTable);
}

// A printed trace:
//   Input: 1001
//   Step  Symbol  From  To
//   0     —       —     r0
//   1     1       r0    r1
//   Result: ACCEPTED
//   Reason: Ended in accepting state r0
export function parseTrace(output) {
  const lines = output.split(/\r?\n/);
  const first = lines.findIndex((l) => l.startsWith("Input: "));
  if (first < 0) throw new Error(`no trace in the output:\n${output}`);
  const input = lines[first].slice("Input: ".length);
  const header = lines.find((l, i) => i > first && l.startsWith("Step"));
  const rows = [];
  let result = null, reason = null;
  for (const line of lines.slice(lines.indexOf(header) + 1)) {
    if (line.startsWith("Result: ")) result = line.slice("Result: ".length);
    else if (line.startsWith("Reason: ")) { reason = line.slice("Reason: ".length); break; }
    else if (/^\d/.test(line)) {
      // Step 0 has no symbol and no "from" state. kstar marks them with an
      // em dash, or with "-" when its output goes through a Windows pipe;
      // show the em dash, as on a terminal, whichever was captured.
      const [step, symbol, from, to] = line.trim().split(/\s+/).map((cell) => (cell === "-" ? "—" : cell));
      rows.push({ step: Number(step), symbol, from, to });
    }
  }
  if (!result || !reason) throw new Error(`the trace has no result or reason:\n${output}`);
  return { input, header, rows, result, reason };
}

export function writeMachines() {
  const demo = runKstar("demo.kstar");
  const machines = parseMachines(demo);
  if (machines.length !== 3) throw new Error(`expected 3 machines from demo.kstar, found ${machines.length}:\n${demo}`);
  const [nfa, dfa, min] = machines;

  const output = runKstar("div3.kstar");
  const [machine] = parseMachines(output);
  const wrote = output.split(/\r?\n/).find((l) => l.startsWith("wrote "));
  if (!machine || !wrote) throw new Error(`div3.kstar printed no machine or no "wrote" line:\n${output}`);
  const source = readFileSync(join(here, "div3.kstar"), "utf8").split(/\r?\n/);
  const div3 = { source, machine, trace: parseTrace(output), wrote };

  writeFileSync(join(here, "machines.js"),
    "// Generated by machines.mjs from the output of demo.kstar and div3.kstar. Do not edit.\n" +
    `window.MACHINES = ${JSON.stringify({ nfa, dfa, min }, null, 2)};\n` +
    `window.DIV3 = ${JSON.stringify(div3, null, 2)};\n`);
  console.log(`kstar: ${nfa.header} / ${dfa.header} / ${min.header}`);
  console.log(`kstar: ${machine.header}; trace on ${div3.trace.input}: ${div3.trace.result}; ${wrote}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) writeMachines();

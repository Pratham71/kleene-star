// The trace animation: `trace Div3 on "1001"`. For each symbol of the word,
// the symbol lights up, a marker travels along the edge it takes, the state
// it reaches lights up, and the row kstar printed for that step appears.
// The result and the reason end it, as in kstar's output.
//
// The rows, result and reason are what kstar printed for div3.kstar
// (window.DIV3.trace); the page refuses to run if a step is not an edge of
// the DFA kstar printed.

"use strict";

const { clamp, p, ease, pop, leave, el, set, drawMachine, R } = window.Kit;
const { machine, pos, bendOf, PANEL, MONO_ADV, frame, codeLine } = window.Div3Scene;
const { source, trace } = window.DIV3;

const STEPS = trace.rows.slice(1); // row 0 is the start, before any symbol
for (const row of STEPS) {
  if (!machine.edges.some((e) => e.from === row.from && e.to === row.to && e.label.split(", ").includes(row.symbol))) {
    throw new Error(`the trace goes ${row.from} --${row.symbol}--> ${row.to}, which is not an edge of the DFA`);
  }
}
if (STEPS.map((r) => r.symbol).join("") !== trace.input) throw new Error("the trace's symbols do not spell its input");

// ---------------------------------------------------------------- timing --

const STEP_MS = 1150;
const T = {
  start: 450,                                   // row 0: the start state lights up
  step: (k) => 1350 + k * STEP_MS,              // step k (from 0) reads symbol k
  result: 1350 + STEPS.length * STEP_MS + 200,  // "Result:" and "Reason:"
};
T.fadeOut = T.result + 3000;
T.total = T.fadeOut + 450;
window.DEMO_TOTAL = T.total;
window.DEMO_STILL = T.fadeOut - 50;
const TRAVEL = 520; // the marker's trip along an edge
const arrives = (k) => T.step(k) + 160 + TRAVEL;

// The state the machine is in at time t, and since when.
function current(t) {
  let state = trace.rows[0].to, since = T.start;
  STEPS.forEach((row, k) => { if (t >= arrives(k)) { state = row.to; since = arrives(k); } });
  return { state, since };
}

// -------------------------------------------------------------------- dom --

const SIZE = 14, ADV = SIZE * MONO_ADV, X = PANEL.x + 22, Y = PANEL.y + 36;
const { panel, diagram, view } = frame("trace");

const command = source.find((l) => l.startsWith("trace "));
codeLine(panel, command, X, Y, 15, "command").show(command.length);
el("rect", { x: PANEL.x + 12, y: Y - 15, width: 3, height: 20, rx: 1.5, fill: "var(--accent)" }, panel);
el("line", { x1: X, x2: PANEL.x + PANEL.w - 22, y1: Y + 20, y2: Y + 20, stroke: "var(--rule)" }, panel);

// kstar's output, laid out in its own columns: each column starts where its
// heading does in the header line kstar printed.
const text = (x, y, s, fill, size = SIZE) => {
  const node = el("text", { class: "mono code", x, y, "font-size": size, fill, style: "white-space: pre" }, panel);
  node.textContent = s;
  return node;
};
const input = text(X, Y + 50, `Input: ${trace.input}`, "var(--dim)");
const header = text(X, Y + 82, trace.header, "var(--faint)");
const columns = ["Step", "Symbol", "From", "To"].map((h) => X + trace.header.indexOf(h) * ADV);
const rows = trace.rows.map((row, i) => {
  const g = el("g", {}, panel);
  const y = Y + 108 + i * 24;
  [[String(row.step), "var(--dim)"], [row.symbol, "var(--accent)"], [row.from, "var(--ink)"], [row.to, "var(--ink)"]]
    .forEach(([s, fill], c) => {
      el("text", { class: "mono code", x: columns[c], y, "font-size": SIZE, fill }, g).textContent = s;
    });
  return g;
});
const resultY = Y + 108 + trace.rows.length * 24 + 14;
const result = el("g", {}, panel);
el("text", { class: "mono code", x: X, y: resultY, "font-size": SIZE, fill: "var(--dim)" }, result).textContent = "Result: ";
el("text", { class: "mono code", x: X + 8 * ADV, y: resultY, "font-size": SIZE, "font-weight": 700, fill: "var(--accent)" }, result).textContent = trace.result;
const reason = text(X, resultY + 24, `Reason: ${trace.reason}`, "var(--dim)");

// The word, above the machine, a symbol at a time.
const WORD_SIZE = 40, WORD_ADV = WORD_SIZE * MONO_ADV;
const wordX = 688 - (trace.input.length * WORD_ADV) / 2;
const word = [...trace.input].map((ch, i) => {
  const g = el("g", { transform: `translate(${wordX + (i + 0.5) * WORD_ADV} 142)` }, diagram);
  const t = el("text", { class: "mono", "text-anchor": "middle", "font-size": WORD_SIZE, "font-weight": 500 }, g);
  t.textContent = ch;
  const mark = el("rect", { x: -WORD_ADV / 2 + 3, y: 10, width: WORD_ADV - 6, height: 3, rx: 1.5, fill: "var(--accent)" }, g);
  return { g, t, mark };
});

// The marker that travels along an edge.
const marker = el("circle", { r: 7, fill: "var(--accent)", stroke: "var(--bg)", "stroke-width": 3 }, diagram);

// ------------------------------------------------------------------ frame --

window.render = (time) => {
  const t = ((time % T.total) + T.total) % T.total;
  const gone = leave(t, T.fadeOut, 350);
  const now = current(t);
  const done = t >= T.result;

  // The output, a row per step, each rising into place as its step lands.
  const rise = (node, at) => {
    const u = ease.out(p(t, at, 240));
    set(node, { opacity: u * gone, transform: `translate(0 ${(1 - u) * 6})` });
  };
  rise(input, 150);
  rise(header, 150);
  rows.forEach((row, i) => rise(row, i === 0 ? T.start : arrives(i - 1)));
  rise(result, T.result);
  rise(reason, T.result + 220);

  // The word: read symbols fade back, the one being read is lit and underlined.
  word.forEach((w, i) => {
    const reading = t >= T.step(i) && t < (i + 1 < STEPS.length ? T.step(i + 1) : T.result) && t < T.fadeOut;
    const read = t >= T.step(i) && t < T.result; // once the result is in, the whole word reads again
    const lift = pop(t, T.step(i), 300) * (reading ? 1 : 0);
    set(w.t, { fill: reading ? "var(--accent)" : read ? "var(--faint)" : "var(--ink)" });
    set(w.g, { transform: `translate(${wordX + (i + 0.5) * WORD_ADV} ${142 - 4 * lift})` });
    set(w.mark, { opacity: reading ? clamp(lift) : 0 });
  });

  // The machine: the current state is lit; the accepting state it ends in
  // pops when the result appears.
  const geometry = drawMachine(
    view,
    pos,
    (s) => {
      const here = s === now.state && t >= T.start && t < T.fadeOut;
      const lit = here ? ease.out(p(t, now.since, 220)) : 0;
      const success = done && here ? Math.sin(Math.PI * p(t, T.result, 420)) : 0;
      return { scale: 1 + 0.14 * success, opacity: 1, halo: 0.22 * lit + 0.12 * success, accent: lit > 0.3 };
    },
    (e) => {
      const k = STEPS.findIndex((row, i) => row.from === e.from && row.to === e.to && t >= T.step(i) + 100 && t < arrives(i) + 260);
      return { draw: 1, accent: k >= 0 };
    },
    bendOf,
  );

  // The marker, on its way along this step's edge.
  const k = STEPS.findIndex((_, i) => t >= T.step(i) + 160 && t < arrives(i));
  if (k >= 0) {
    const geo = geometry[`${STEPS[k].from}>${STEPS[k].to}`];
    const [x, y] = geo.at(ease.inOut(p(t, T.step(k) + 160, TRAVEL)));
    set(marker, { cx: x, cy: y, opacity: 1 });
  } else {
    set(marker, { opacity: 0 });
  }
};

window.READY = document.fonts.ready.then(() => {
  window.render(0);
  return true;
});
window.READY.then(() => Kit.play(window.render));

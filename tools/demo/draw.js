// The draw animation: Div3 is written a line at a time, and each line adds
// its piece to the diagram as it is typed: `start r0` brings in the start
// state, `accept r0` makes it a double circle, each `a --s--> b` draws its
// edge. Then `draw Div3` prints what kstar printed: "wrote Div3.dot".
//
// The lines are div3.kstar's own lines, and the states and edges are the DFA
// kstar printed for it (window.DIV3); the page refuses to run if a line's
// edge is not in that DFA, or the DFA has an edge no line wrote.

"use strict";

const { clamp, p, ease, pop, leave, el, set, drawMachine } = window.Kit;
const { machine, pos, bendOf, PANEL, MONO_ADV, frame, codeLine } = window.Div3Scene;
const { source, wrote } = window.DIV3;

// The program as shown: the alphabet, the dfa block, then `draw`.
const first = source.findIndex((l) => l.startsWith("dfa "));
const last = source.indexOf("}", first);
const LINES = [
  source.find((l) => l.startsWith("alphabet ")),
  ...source.slice(first, last + 1),
  "",
  source.find((l) => l.startsWith("draw ")),
];

// What each line does to the diagram.
const parse = (line) => {
  let m;
  if ((m = /^\s*start\s+(\w+)/.exec(line))) return { start: m[1] };
  if ((m = /^\s*accept\s+(\w+)/.exec(line))) return { accept: m[1] };
  if ((m = /^\s*(\w+)\s+--(\S+)-->\s+(\w+)/.exec(line))) return { edge: { from: m[1], label: m[2], to: m[3] } };
  if (/^draw /.test(line)) return { draw: true };
  return {};
};
const ACTIONS = LINES.map(parse);
for (const e of machine.edges) {
  if (!ACTIONS.some((a) => a.edge && a.edge.from === e.from && a.edge.to === e.to && a.edge.label === e.label)) {
    throw new Error(`kstar printed an edge ${e.from} --${e.label}--> ${e.to} that no line writes`);
  }
}
for (const a of ACTIONS) {
  if (a.edge && !machine.edges.some((e) => e.from === a.edge.from && e.to === a.edge.to && e.label === a.edge.label)) {
    throw new Error(`the line for ${a.edge.from} --${a.edge.label}--> ${a.edge.to} is not an edge kstar printed`);
  }
}

// ---------------------------------------------------------------- timing --

const SIZE = 15, ADV = SIZE * MONO_ADV, LH = 22, X = PANEL.x + 22, Y = PANEL.y + 36;
const TYPE_MS = 22;   // per character
const PAUSE = 240;    // after a line that changes nothing on screen
const BEAT = 520;     // after a line that adds to the diagram, so it lands first

// When each line starts typing, and when its effect starts.
const schedule = [];
let clock = 250;
for (const [i, line] of LINES.entries()) {
  const typed = clock + line.length * TYPE_MS;
  const effect = typed + 80;
  schedule.push({ start: clock, typed, effect });
  const a = ACTIONS[i];
  clock = typed + (a.start || a.accept || a.edge || a.draw ? BEAT : line ? PAUSE : 0);
}
const DRAW = schedule[LINES.length - 1].effect;
const T = { fadeOut: DRAW + 2900, total: DRAW + 3400 };
window.DEMO_TOTAL = T.total;
window.DEMO_STILL = T.fadeOut - 50;

// A state appears with the first line that names it; an edge with its line.
const stateAt = {};
const edgeAt = {};
ACTIONS.forEach((a, i) => {
  const at = schedule[i].effect;
  if (a.start && !(a.start in stateAt)) stateAt[a.start] = at;
  if (a.edge) {
    // A new target state pops in first, then the edge reaches it.
    for (const s of [a.edge.from, a.edge.to]) if (!(s in stateAt)) stateAt[s] = at;
    edgeAt[`${a.edge.from}>${a.edge.to}`] = at + 140;
  }
});
const acceptAt = Object.fromEntries(ACTIONS.filter((a) => a.accept).map((a) => [a.accept, schedule[ACTIONS.indexOf(a)].effect]));

// -------------------------------------------------------------------- dom --

const { panel, view } = frame("draw");
const lines = LINES.map((line, i) => codeLine(panel, line, X, Y + i * LH, SIZE, `line${i}`));
const bar = el("rect", { x: PANEL.x + 12, width: 3, height: 20, rx: 1.5, fill: "var(--accent)" }, panel);
const caret = el("rect", { width: 2, height: 18, fill: "var(--ink)" }, panel);
const ruleY = Y + (LINES.length - 1) * LH + 22;
el("line", { x1: X, x2: PANEL.x + PANEL.w - 22, y1: ruleY, y2: ruleY, stroke: "var(--rule)" }, panel);
const output = el("text", { class: "mono", x: X, y: ruleY + 28, "font-size": 14, fill: "var(--dim)" }, panel);
output.textContent = wrote;

// ------------------------------------------------------------------ frame --

window.render = (time) => {
  const t = ((time % T.total) + T.total) % T.total;
  const gone = leave(t, T.fadeOut, 350);

  // The program, typed a line at a time; the current line is bright.
  let current = 0;
  lines.forEach((line, i) => {
    const { start } = schedule[i];
    const typed = clamp(Math.floor((t - start) / TYPE_MS), 0, line.chars);
    line.show(typed);
    if (t >= start) current = i;
  });
  lines.forEach((line, i) => set(line.node, { opacity: (i === current ? 1 : 0.55) * gone }));
  const cur = schedule[current];
  const typed = clamp(Math.floor((t - cur.start) / TYPE_MS), 0, lines[current].chars);
  const typing = t < cur.typed;
  const blink = typing || Math.floor((t - cur.typed) / 530) % 2 === 0;
  set(caret, { x: X + typed * ADV + 1, y: Y + current * LH - 14, opacity: (blink ? 0.9 : 0) * gone });
  set(bar, { y: Y + current * LH - 15, opacity: gone });
  // kstar's answer to `draw`, rising into place.
  const said = ease.out(p(t, DRAW + 120, 260));
  set(output, { opacity: said * gone, transform: `translate(0 ${(1 - said) * 6})` });

  // The diagram, built by the lines above.
  drawMachine(
    view,
    pos,
    (s) => {
      const at = stateAt[s];
      const enter = at === undefined ? 0 : pop(t, at);
      // `draw` lights the states one after another: this is the machine it writes.
      const lit = Math.sin(Math.PI * p(t, DRAW + 80 * machine.states.indexOf(s), 420));
      return {
        scale: (0.4 + 0.6 * enter) * (1 + 0.08 * lit),
        opacity: Math.min(enter, 1) * gone,
        inner: s in acceptAt ? pop(t, acceptAt[s]) : 1,
        halo: 0.25 * lit,
        accent: lit > 0.2,
        arrow: s === machine.start ? ease.out(p(t, stateAt[s] + 120, 260)) : 1,
      };
    },
    (e) => ({ draw: ease.out(p(t, edgeAt[`${e.from}>${e.to}`], 380)), opacity: gone }),
    bendOf,
  );
};

window.READY = document.fonts.ready.then(() => {
  window.render(0);
  return true;
});
window.READY.then(() => Kit.play(window.render));

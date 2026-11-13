// The demo animation: /(ab)*/ becomes an eps-NFA (Thompson's construction),
// a DFA (subset construction) and a minimal DFA, in four stages.
//
// Everything on screen is a pure function of the time t (milliseconds):
// window.render(t) draws the frame at t. render.mjs steps t frame by frame
// and screenshots each one, so the GIF is the same on every run. In a normal
// browser tab the page plays itself in a loop.
//
// The machines (states, start, accepting states, transitions, the headers
// kstar prints) come from machines.js. Only where states sit on screen and
// how much an edge curves are chosen here, in LAYOUT and BEND.

"use strict";

const { R, clamp, lerp, p, ease, pop, leave, el, set, buildMachine, drawMachine, checkLayout, brand } = window.Kit;

brand(document.getElementById("brand"));
const { nfa: NFA, dfa: DFA, min: MIN } = window.MACHINES;

// ---------------------------------------------------------------- layout --

const LAYOUT = {
  nfa: { q0: [453, 248], q1: [556, 248], q3: [659, 248], q4: [762, 248], q5: [865, 248], q2: [659, 368] },
  dfa: { "{q0,q1,q2}": [480, 160], "{q3,q4}": [680, 262], "{}": [866, 262], "{q1,q2,q5}": [480, 364] },
  min: { "{q0,q1,q2}+{q1,q2,q5}": [480, 262], "{q3,q4}": [680, 262], "{}": [866, 262] },
};

// Where a state's name goes when it does not fit inside the circle.
const LABEL_SIDE = { "{q0,q1,q2}": "above", "{q3,q4}": "above", "{}": "below", "{q1,q2,q5}": "below", "{q0,q1,q2}+{q1,q2,q5}": "below" };

// How far an edge bows away from the straight line, as a fraction of its
// length; positive bows to the left of the direction of travel.
const BEND = {
  "q5>q1": 0.42, "q0>q2": 0.24, "q5>q2": -0.24,
  "{q0,q1,q2}>{}": -0.3, "{q1,q2,q5}>{}": 0.3,
  "{q3,q4}>{q1,q2,q5}": -0.18, "{q1,q2,q5}>{q3,q4}": -0.18,
  "{q0,q1,q2}+{q1,q2,q5}>{q3,q4}": 0.2, "{q3,q4}>{q0,q1,q2}+{q1,q2,q5}": 0.2,
  "{q0,q1,q2}+{q1,q2,q5}>{}": -0.36,
};

// Check the hand-placed layout against the real machines, both ways.
for (const [key, machine] of [["nfa", NFA], ["dfa", DFA], ["min", MIN]]) checkLayout(`LAYOUT.${key}`, LAYOUT[key], machine);

// Minimization merges DFA states; kstar names the result "X+Y".
const MERGED_INTO = {};
for (const s of MIN.states) for (const part of s.split("+")) MERGED_INTO[part] = s;
for (const s of DFA.states) if (!MERGED_INTO[s]) throw new Error(`DFA state ${s} is missing from the minimal DFA`);
const MERGED = MIN.states.find((s) => s.includes("+"));
const MERGING = MERGED.split("+");

// ---------------------------------------------------------------- timing --

// Playful personality (motion-design skill): 200-350 ms moves, ease-out-back
// entrances with about 10-15 % overshoot, ease-in exits that are shorter than
// entrances, staggers well inside a 500 ms budget.
const T = {
  regexIn: 150,   // stage 1: the regex pops in
  nfa: 1800,      // stage 2: Thompson's construction
  dfa: 4500,      // stage 3: subset construction
  min: 7500,      // stage 4: minimization
  slide: 7900,    //   the two equivalent states slide together
  land: 8400,     //   ... and land as one
  fadeOut: 11250, // the hold ends; everything resets for the loop
  total: 11700,
};
window.DEMO_TOTAL = T.total;
window.DEMO_STILL = T.fadeOut - 50;

// ---------------------------------------------------------------- stepper --

const STEPS = ["regex", "eps-NFA", "DFA", "minimal DFA"];
let stepper = null; // built once the font has loaded, since it measures text

function buildStepper() {
  const g = document.getElementById("stepper");
  g.replaceChildren();
  const pill = el("rect", { y: 34, height: 30, rx: 15, fill: "var(--accent)" }, g);
  const labels = STEPS.map((s) => {
    const l = el("text", { class: "sans", y: 54, "font-size": 14, "font-weight": 550, fill: "var(--dim)" }, g);
    l.textContent = s;
    return l;
  });
  // Right-aligned with the edge of the diagram.
  let x = 932;
  const boxes = [];
  for (let i = STEPS.length - 1; i >= 0; i--) {
    const w = labels[i].getComputedTextLength() + 28;
    x -= w;
    boxes[i] = { x, w };
    set(labels[i], { x: x + 14 });
    x -= 6;
  }
  return { pill, labels, boxes };
}

function drawStepper(t) {
  // Which step, as a float, so the pill can travel between them.
  const moves = [[T.nfa], [T.dfa], [T.min]];
  let at = 0;
  for (const [start] of moves) at += ease.back(p(t, start, 340), 1.4);
  at -= ease.inOut(p(t, T.fadeOut, 400)) * 3;
  const i = clamp(Math.floor(at), 0, 3), j = clamp(i + 1, 0, 3), f = at - i;
  const a = stepper.boxes[i], b = stepper.boxes[j];
  const x = lerp(a.x, b.x, f), w = lerp(a.w, b.w, f);
  set(stepper.pill, { x, width: w });
  stepper.labels.forEach((l, k) => {
    const c = stepper.boxes[k].x + stepper.boxes[k].w / 2;
    set(l, { fill: c > x && c < x + w ? "var(--on-accent)" : "var(--dim)" });
  });
}

// ------------------------------------------------------------------ code --

const ADV = 0.6; // JetBrains Mono advance, in em
const CODE = [
  { start: -1, parts: [["let ", "var(--dim)"], ["r", "var(--ink)"], [" = ", "var(--dim)"], ["/(ab)*/", "var(--accent)"]] },
  { start: T.nfa, parts: [["let ", "var(--dim)"], ["n", "var(--ink)"], [" = ", "var(--dim)"], ["to_nfa", "var(--ink)"], ["(r)", "var(--dim)"]] },
  { start: T.dfa, parts: [["let ", "var(--dim)"], ["d", "var(--ink)"], [" = ", "var(--dim)"], ["to_dfa", "var(--ink)"], ["(r)", "var(--dim)"]] },
  { start: T.min, parts: [["let ", "var(--dim)"], ["m", "var(--ink)"], [" = ", "var(--dim)"], ["minimize", "var(--ink)"], ["(d)", "var(--dim)"]] },
];
const CODE_SIZE = 17, CODE_X = 54, CODE_Y = 150, CODE_LH = 38, TYPE_MS = 26;
const code = (() => {
  const g = document.getElementById("code");
  const defs = el("defs", {}, g);
  const bar = el("rect", { x: 40, width: 3, height: 24, rx: 1.5, fill: "var(--accent)" }, g);
  const lines = CODE.map((line, i) => {
    const clip = el("clipPath", { id: `type${i}` }, defs);
    const clipRect = el("rect", { x: CODE_X - 2, y: CODE_Y + i * CODE_LH - 20, height: 28, width: 0 }, clip);
    const text = el("text", { class: "mono", x: CODE_X, y: CODE_Y + i * CODE_LH, "font-size": CODE_SIZE, "clip-path": `url(#type${i})` }, g);
    for (const [s, fill] of line.parts) el("tspan", { fill }, text).textContent = s;
    const chars = line.parts.reduce((n, [s]) => n + s.length, 0);
    return { text, clipRect, chars };
  });
  const caret = el("rect", { width: 2, height: 20, fill: "var(--ink)" }, g);
  return { bar, lines, caret };
})();

function drawCode(t) {
  const adv = CODE_SIZE * ADV;
  const reset = leave(t, T.fadeOut, 300);
  let active = 0;
  code.lines.forEach((line, i) => {
    const start = CODE[i].start;
    const typed = start < 0 ? line.chars : clamp(Math.floor((t - start) / TYPE_MS), 0, line.chars);
    if (t >= start) active = i;
    set(line.clipRect, { width: typed ? typed * adv + 4 : 0 });
    // After the hold, line 1 is current again, as in the first frame.
    const current = t < T.fadeOut ? i === active : i === 0;
    const opacity = (start < 0 ? 1 : reset) * (current ? 1 : 0.5);
    set(line.text, { opacity });
    if (current) {
      // The caret sits after the typed text and blinks once typing stops.
      const done = start < 0 || t > start + line.chars * TYPE_MS;
      const since = start < 0 ? t : t - (start + line.chars * TYPE_MS);
      const on = !done || Math.floor(since / 530) % 2 === 0;
      set(code.caret, { x: CODE_X + typed * adv + 2, y: CODE_Y + i * CODE_LH - 15, opacity: on ? 0.9 : 0 });
    }
  });
  if (t >= T.fadeOut) set(code.caret, { x: CODE_X + code.lines[0].chars * adv + 2, y: CODE_Y - 15, opacity: 0.9 });
  // The marker slides to the current line with a little overshoot.
  let y = CODE_Y - 18;
  for (let i = 1; i < CODE.length; i++) y += ease.back(p(t, CODE[i].start, 300), 1.6) * CODE_LH;
  y -= ease.inOut(p(t, T.fadeOut, 400)) * CODE_LH * 3;
  set(code.bar, { y });
}

// --------------------------------------------------------------- caption --

// A title for the stage and the first line kstar prints for it.
const CAPTIONS = [
  { at: -1, title: "A regular expression", out: "" },
  { at: T.nfa, title: "Thompson's construction", out: NFA.header },
  { at: T.dfa, title: "Subset construction", out: DFA.header },
  { at: T.min + 900, title: "Minimization", out: MIN.header },
];
const caption = (() => {
  const g = document.getElementById("caption");
  return CAPTIONS.map((c) => {
    const group = el("g", {}, g);
    el("text", { class: "sans", x: 50, y: 344, "font-size": 19, "font-weight": 600, fill: "var(--ink)" }, group).textContent = c.title;
    el("text", { class: "mono", x: 50, y: 374, "font-size": 13.5, fill: "var(--dim)" }, group).textContent = c.out;
    return group;
  });
})();
// The minimization caption appears with the merge, not before it.
CAPTIONS[3].titleAt = T.min;

function drawCaption(t) {
  // Text swap: the old caption lifts away, the new one rises into place.
  CAPTIONS.forEach((c, i) => {
    const next = CAPTIONS[i + 1];
    // The new caption waits for the old one to leave, so they never overlap.
    const inStart = i === 0 ? T.fadeOut + 150 : c.at + 130;
    const enter = i === 0 && t < T.fadeOut ? 1 : ease.out(p(t, inStart, 220));
    const exit = t >= T.fadeOut ? (i === 0 ? 0 : 1) : next ? ease.in(p(t, next.at, 150)) : 0;
    const opacity = enter * (1 - exit);
    const dy = (1 - enter) * 6 - exit * 6;
    set(caption[i], { opacity, transform: `translate(0 ${dy})` });
  });
}

// --------------------------------------------------------------- diagrams --

const diagram = document.getElementById("diagram");

const views = {
  nfa: buildMachine(NFA, diagram, { namesInside: true }),
  dfa: buildMachine(DFA, diagram, { namesInside: false, labelSide: LABEL_SIDE }),
};

// The regex of stage 1, a character at a time.
const REGEX = "/(ab)*/";
const regex = (() => {
  const g = el("g", {}, diagram);
  const size = 66, adv = size * ADV, x0 = 657 - (REGEX.length * adv) / 2;
  return [...REGEX].map((ch, i) => {
    const cg = el("g", {}, g);
    const fill = ch === "/" ? "var(--faint)" : "ab".includes(ch) ? "var(--ink)" : "var(--accent)";
    el("text", { class: "mono", x: 0, y: 0, "font-size": size, "font-weight": 500, fill, "text-anchor": "middle" }, cg).textContent = ch;
    return { g: cg, x: x0 + adv * (i + 0.5), y: 274 };
  });
})();

function drawRegex(t) {
  regex.forEach((c, i) => {
    const order = i; // left to right
    const enter = pop(t, T.regexIn + order * 45, 320);
    const exit = 1 - leave(t, T.nfa + 60 + order * 25, 170);
    // Gone by the end, so the last frame matches the first one of the loop.
    const shown = t >= T.fadeOut ? 0 : enter * (1 - exit);
    const s = 0.6 + 0.4 * shown;
    const dy = (1 - Math.min(shown, 1)) * 14 - exit * 10;
    set(c.g, { transform: `translate(${c.x} ${c.y + dy}) scale(${s})`, opacity: clamp(shown) });
  });
}

const bendKey = (e) => `${e.from}>${e.to}`;

// Stage 2 and 3: the order states appear in, and when.
const NFA_ORDER = ["q0", "q1", "q3", "q4", "q5", "q2"];
const DFA_ORDER = [DFA.start, "{q3,q4}", "{q1,q2,q5}", "{}"];
for (const s of [...NFA.states, ...DFA.states]) {
  if (!NFA_ORDER.includes(s) && !DFA_ORDER.includes(s)) throw new Error(`no entry time for ${s}`);
}
const NFA_IN = T.nfa + 320, NFA_STAGGER = 75;
const DFA_IN = T.dfa + 880, DFA_STAGGER = 110;
const nodeIn = (order, base, stagger) => (s) => base + order.indexOf(s) * stagger;
const nfaAt = nodeIn(NFA_ORDER, NFA_IN, NFA_STAGGER);
const dfaAt = nodeIn(DFA_ORDER, DFA_IN, DFA_STAGGER);
const edgeAt = (at) => (e) => Math.max(at(e.from), at(e.to)) + 180;

// The eps-closure of the NFA's start state is the DFA's start state.
const CLOSURE = DFA.start.replace(/[{}]/g, "").split(",");

function drawNfa(t) {
  const view = views.nfa;
  const out = T.dfa + 420; // the NFA leaves after the closure is shown
  const order = [...NFA_ORDER].reverse();
  view.g.setAttribute("visibility", t < T.nfa || t > out + 600 ? "hidden" : "visible");
  drawMachine(
    view,
    (s) => LAYOUT.nfa[s],
    (s) => {
      const enter = pop(t, nfaAt(s));
      const exit = 1 - leave(t, out + order.indexOf(s) * 30, 180);
      // Secondary beat: q0's eps-closure lights up before the NFA leaves.
      const lit = CLOSURE.includes(s) ? Math.sin(Math.PI * p(t, T.dfa + 40, 380)) : 0;
      return {
        scale: (0.4 + 0.6 * enter) * (1 - 0.35 * exit) * (1 + 0.1 * lit),
        opacity: Math.min(enter, 1) * (1 - exit),
        inner: 0.5 + 0.5 * pop(t, nfaAt(s) + 90),
        halo: 0.22 * lit,
        accent: lit > 0.15,
        arrow: ease.out(p(t, NFA_IN - 120, 220)),
      };
    },
    (e) => ({
      draw: ease.out(p(t, edgeAt(nfaAt)(e), 260)),
      opacity: 1 - ease.in(p(t, out, 160)),
    }),
    (e) => BEND[bendKey(e)] || 0,
  );
}

function drawDfa(t) {
  const view = views.dfa;
  view.g.setAttribute("visibility", t < T.dfa || t > T.total ? "hidden" : "visible");
  const slide = ease.inOut(p(t, T.slide, T.land - T.slide));
  const landed = t >= T.land;
  const gone = leave(t, T.fadeOut, 300);
  drawMachine(
    view,
    (s) => lerp2(LAYOUT.dfa[s], LAYOUT.min[MERGED_INTO[s]], slide),
    (s) => {
      const enter = pop(t, dfaAt(s));
      const merging = MERGING.includes(s);
      // Anticipation: the two equivalent states light up and draw in a little.
      const cue = merging ? ease.out(p(t, T.min + 280, 160)) - ease.out(p(t, T.land, 400)) : 0;
      const squash = merging ? -0.08 * ease.out(p(t, T.min + 280, 160)) * (1 - slide) : 0;
      // The merged state lands with an overshoot (the second one hides under the first).
      const landPop = merging ? 0.22 * Math.sin(Math.PI * p(t, T.land, 360)) * (1 - p(t, T.land, 360)) : 0;
      const hideSecond = merging && s === MERGING[1] && landed ? 0 : 1;
      return {
        scale: (0.4 + 0.6 * enter) * (1 + squash + landPop),
        opacity: Math.min(enter, 1) * hideSecond * gone,
        inner: 0.5 + 0.5 * pop(t, dfaAt(s) + 90),
        halo: 0.2 * cue,
        accent: cue > 0.1,
        name: merging ? ease.out(p(t, dfaAt(s) + 60, 240)) * (1 - ease.in(p(t, T.slide, 150))) : ease.out(p(t, dfaAt(s) + 60, 240)),
        arrow: ease.out(p(t, DFA_IN - 120, 220)),
      };
    },
    (e) => {
      // Edges that become the same edge of the minimal DFA: keep the first.
      const key = `${MERGED_INTO[e.from]}>${MERGED_INTO[e.to]}`;
      const first = DFA.edges.find((f) => `${MERGED_INTO[f.from]}>${MERGED_INTO[f.to]}` === key) === e;
      return {
        draw: ease.out(p(t, edgeAt(dfaAt)(e), 260)),
        opacity: (first ? 1 : 1 - ease.in(p(t, T.land - 60, 120))) * gone,
        // Labels step aside while the states slide, so they do not sweep across others.
        label: 1 - Math.sin(Math.PI * slide),
      };
    },
    (e) => lerp(BEND[bendKey(e)] || 0, BEND[`${MERGED_INTO[e.from]}>${MERGED_INTO[e.to]}`] || 0, slide),
  );
  // The merged state's name, once it has landed.
  mergedName.setAttribute("opacity", ease.out(p(t, T.land + 120, 280)) * gone);
  const [mx, my] = LAYOUT.min[MERGED];
  mergedName.setAttribute("transform", `translate(${mx} ${my + (1 - ease.out(p(t, T.land + 120, 280))) * 6})`);
}
const lerp2 = (a, b, u) => [lerp(a[0], b[0], u), lerp(a[1], b[1], u)];

const mergedName = (() => {
  const g = el("g", { opacity: 0 }, diagram);
  const text = el("text", { class: "mono", "text-anchor": "middle", "font-size": 14, fill: "var(--accent)" }, g);
  MERGING.forEach((part, i) => {
    el("tspan", { x: 0, y: R + 22 + i * 18 }, text).textContent = i < MERGING.length - 1 ? part + "+" : part;
  });
  return g;
})();

// ------------------------------------------------------------------ frame --

window.render = (time) => {
  const t = ((time % T.total) + T.total) % T.total;
  drawStepper(t);
  drawCode(t);
  drawCaption(t);
  drawRegex(t);
  drawNfa(t);
  drawDfa(t);
};

window.READY = document.fonts.ready.then(() => {
  stepper = buildStepper();
  window.render(0);
  return true;
});

window.READY.then(() => Kit.play(window.render));

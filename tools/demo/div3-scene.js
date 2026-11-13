// What draw.js and trace.js share: the frame (the K* mark, a label for the
// command, the program panel), where Div3's states sit, and colouring a line
// of kstar source. The machine itself comes from window.DIV3, which
// machines.mjs writes from the real output of div3.kstar.

"use strict";

window.Div3Scene = (() => {
  const { el, set, buildMachine, checkLayout, brand } = window.Kit;
  const { machine } = window.DIV3;

  const LAYOUT = { r0: [508, 270], r1: [688, 270], r2: [868, 270] };
  // Each pair of opposite edges bows apart, one above and one below.
  const BEND = { "r0>r1": 0.28, "r1>r0": 0.28, "r1>r2": 0.28, "r2>r1": 0.28 };
  checkLayout("Div3Scene's LAYOUT", LAYOUT, machine);
  const pos = (s) => LAYOUT[s];
  const bendOf = (e) => BEND[`${e.from}>${e.to}`] || 0;

  const PANEL = { x: 28, y: 92, w: 364, h: 360 };
  const MONO_ADV = 0.6; // JetBrains Mono advance, in em

  // The K* mark, the command this animation is about as a pill on the
  // right, the program panel and the diagram's group.
  function frame(command) {
    const svg = document.querySelector("svg");
    el("rect", { width: svg.getAttribute("width"), height: svg.getAttribute("height"), fill: "var(--bg)" }, svg);
    brand(el("g", {}, svg));
    const pill = el("g", {}, svg);
    const label = el("text", { class: "mono", y: 54, "font-size": 15, "font-weight": 600, fill: "var(--on-accent)", "text-anchor": "middle" });
    label.textContent = command;
    const w = command.length * 15 * MONO_ADV + 32;
    el("rect", { x: 932 - w, y: 34, width: w, height: 30, rx: 15, fill: "var(--accent)" }, pill);
    set(label, { x: 932 - w / 2 });
    pill.appendChild(label);
    el("rect", { x: PANEL.x, y: PANEL.y, width: PANEL.w, height: PANEL.h, rx: 14, fill: "var(--panel)" }, svg);
    const panel = el("g", {}, svg);
    const diagram = el("g", {}, svg);
    const view = buildMachine(machine, diagram, { namesInside: true });
    return { svg, panel, diagram, view };
  }

  // A line of kstar as coloured pieces: keywords dim, names bright, symbols
  // and strings in the accent, punctuation faint.
  const KEYWORDS = new Set(["alphabet", "dfa", "over", "start", "accept", "draw", "trace", "on"]);
  function colour(line) {
    const pieces = [];
    const re = /(\s+)|--([^-\s]+)-->|("[^"]*")|([A-Za-z_]\w*)|(\S)/g;
    let m;
    while ((m = re.exec(line))) {
      if (m[1]) pieces.push([m[1], "var(--ink)"]);
      else if (m[2]) pieces.push(["--", "var(--faint)"], [m[2], "var(--accent)"], ["-->", "var(--faint)"]);
      else if (m[3]) pieces.push([m[3], "var(--accent)"]);
      else if (m[4]) pieces.push([m[4], KEYWORDS.has(m[4]) ? "var(--dim)" : "var(--ink)"]);
      else if (/[0-9]/.test(m[5])) pieces.push([m[5], "var(--accent)"]);
      else pieces.push([m[5], "var(--faint)"]);
    }
    return pieces;
  }

  // One line of source in the panel, revealed a character at a time through
  // a clip, so typing never changes the text's layout.
  function codeLine(parent, text, x, y, size, id) {
    const defs = el("defs", {}, parent);
    const clip = el("clipPath", { id }, defs);
    const clipRect = el("rect", { x: x - 2, y: y - size - 4, height: size + 10, width: 0 }, clip);
    const node = el("text", { class: "mono code", x, y, "font-size": size, "clip-path": `url(#${id})`, style: "white-space: pre" }, parent);
    for (const [s, fill] of colour(text)) el("tspan", { fill }, node).textContent = s;
    return {
      node,
      chars: text.length,
      // Shows the first n characters.
      show(n) { set(clipRect, { width: n ? n * size * MONO_ADV + 4 : 0 }); },
    };
  }

  return { machine, LAYOUT, pos, bendOf, PANEL, MONO_ADV, frame, colour, codeLine };
})();

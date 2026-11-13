// What every animation here shares: easing, SVG helpers, and drawing a
// state machine (circles, double circles, the start arrow, labelled edges).
// Each animation is a pure function of the time t, so nothing here keeps
// state between frames.

"use strict";

window.Kit = (() => {
  const SVG = "http://www.w3.org/2000/svg";
  const R = 23; // state radius

  const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
  const lerp = (a, b, u) => a + (b - a) * u;
  // Progress of a move that starts at `start` and lasts `dur`, from 0 to 1.
  const p = (t, start, dur) => clamp((t - start) / dur);
  // Playful personality (motion-design skill): ease-out-back entrances with
  // about 10-15 % overshoot, ease-in exits, ease-in-out for moves on screen.
  const ease = {
    out: (u) => 1 - Math.pow(1 - u, 3),
    in: (u) => u * u * u,
    inOut: (u) => (u < 0.5 ? 4 * u * u * u : 1 - Math.pow(-2 * u + 2, 3) / 2),
    back: (u, s = 1.9) => (u <= 0 ? 0 : 1 + (s + 1) * Math.pow(u - 1, 3) + s * Math.pow(u - 1, 2)),
  };
  // A pop-in: 0 before start, overshoots, settles at 1.
  const pop = (t, start, dur = 320) => ease.back(p(t, start, dur));
  // A quick exit: 1 before start, 0 after.
  const leave = (t, start, dur = 180) => 1 - ease.in(p(t, start, dur));

  function el(name, attrs = {}, parent) {
    const node = document.createElementNS(SVG, name);
    for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, v);
    if (parent) parent.appendChild(node);
    return node;
  }
  const set = (node, attrs) => { for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, v); };

  // A quadratic edge from a to b, trimmed to the two circles. `at(u)` gives
  // points along the visible part, for things that travel along the edge.
  function edgeGeometry(a, b, bend) {
    const dx = b[0] - a[0], dy = b[1] - a[1];
    const len = Math.hypot(dx, dy);
    const nx = -dy / len, ny = dx / len;
    const c = [(a[0] + b[0]) / 2 + nx * bend * len, (a[1] + b[1]) / 2 + ny * bend * len];
    const curve = (u) => [
      (1 - u) * (1 - u) * a[0] + 2 * (1 - u) * u * c[0] + u * u * b[0],
      (1 - u) * (1 - u) * a[1] + 2 * (1 - u) * u * c[1] + u * u * b[1],
    ];
    const solve = (from, inward) => {
      // The parameter where the curve crosses the circle around `from`.
      let lo = inward ? 0 : 0.5, hi = inward ? 0.5 : 1;
      for (let k = 0; k < 30; k++) {
        const mid = (lo + hi) / 2;
        const q = curve(mid), d = Math.hypot(q[0] - from[0], q[1] - from[1]);
        if ((d < R) === inward) lo = mid; else hi = mid;
      }
      return (lo + hi) / 2;
    };
    const t0 = solve(a, true), t1 = solve(b, false);
    // The control point of the piece between t0 and t1 (blossoming).
    const ctl = [0, 1].map((k) => (1 - t0) * (1 - t1) * a[k] + ((1 - t0) * t1 + t0 * (1 - t1)) * c[k] + t0 * t1 * b[k]);
    const s = curve(t0), e = curve(t1);
    const tan = [2 * (1 - t1) * (c[0] - a[0]) + 2 * t1 * (b[0] - c[0]), 2 * (1 - t1) * (c[1] - a[1]) + 2 * t1 * (b[1] - c[1])];
    const mid = curve(0.5);
    // Labels go on the outside of the bow; straight edges get theirs above.
    let side = Math.sign(bend) || (ny < 0 || (ny === 0 && nx < 0) ? 1 : -1);
    if (!bend && Math.abs(ny) < 1e-9) side = nx > 0 ? -1 : 1;
    const label = [mid[0] + nx * side * 14, mid[1] + ny * side * 14 + 5];
    return { d: `M${s[0]},${s[1]} Q${ctl[0]},${ctl[1]} ${e[0]},${e[1]}`, tip: e, tan, label, at: (u) => curve(lerp(t0, t1, u)) };
  }

  // A loop above a state, with the same shape as edgeGeometry's result.
  function loopGeometry([x, y]) {
    const a = [x - 10, y - R + 2], b = [x + 10, y - R + 2];
    const c1 = [x - 34, y - 78], c2 = [x + 34, y - 78];
    const at = (u) => [0, 1].map((k) =>
      Math.pow(1 - u, 3) * a[k] + 3 * Math.pow(1 - u, 2) * u * c1[k] + 3 * (1 - u) * u * u * c2[k] + u * u * u * b[k]);
    return { d: `M${a[0]},${a[1]} C${c1[0]},${c1[1]} ${c2[0]},${c2[1]} ${b[0]},${b[1]}`, tip: b, tan: [-4, 12], label: [x, y - 72], at };
  }

  function arrowPoints(tip, tan) {
    const len = Math.hypot(tan[0], tan[1]);
    const ux = tan[0] / len, uy = tan[1] / len;
    const bx = tip[0] - ux * 10, by = tip[1] - uy * 10;
    return `${tip[0]},${tip[1]} ${bx - uy * 5},${by + ux * 5} ${bx + uy * 5},${by - ux * 5}`;
  }

  // The SVG for one machine. Short names go inside the circles; long ones
  // (sets of states) go above or below, as `labelSide` says, a part per line.
  function buildMachine(machine, parent, { namesInside = true, labelSide = {} } = {}) {
    const g = el("g", {}, parent);
    const edges = el("g", {}, g);
    const nodes = el("g", {}, g);
    const start = {
      line: el("line", { stroke: "var(--line)", "stroke-width": 2 }, edges),
      head: el("polygon", { fill: "var(--line)" }, edges),
    };
    const edgeViews = machine.edges.map((e) => {
      const eg = el("g", {}, edges);
      const path = el("path", { fill: "none", stroke: "var(--line)", "stroke-width": 2, pathLength: 1, "stroke-dasharray": "1 1" }, eg);
      const head = el("polygon", { fill: "var(--line)" }, eg);
      const label = el("text", { class: "mono halo", "font-size": 15, "text-anchor": "middle", fill: "var(--ink)" }, eg);
      label.textContent = e.label;
      return { e, g: eg, path, head, label };
    });
    const nodeViews = {};
    for (const s of machine.states) {
      const ng = el("g", {}, nodes);
      const halo = el("circle", { r: R + 9, fill: "var(--accent)", opacity: 0 }, ng);
      const ring = el("circle", { r: R, fill: "var(--node)", stroke: "var(--line)", "stroke-width": 2 }, ng);
      const inner = machine.accept.includes(s) ? el("circle", { r: R - 5, fill: "none", stroke: "var(--line)", "stroke-width": 2 }, ng) : null;
      const name = el("text", { class: "mono", "text-anchor": "middle", "font-size": namesInside ? 15 : 14, fill: namesInside ? "var(--ink)" : "var(--dim)" }, ng);
      const parts = namesInside ? [s] : s.split("+").map((x, i, all) => (i < all.length - 1 ? x + "+" : x));
      parts.forEach((part, i) => {
        const above = labelSide[s] === "above";
        const y = namesInside ? 5 : above ? -R - 12 - (parts.length - 1 - i) * 18 : R + 22 + i * 18;
        el("tspan", { x: 0, y }, name).textContent = part;
      });
      nodeViews[s] = { g: ng, halo, ring, inner, name };
    }
    return { g, start, edges: edgeViews, nodes: nodeViews, machine };
  }

  // Positions a machine for one frame. `pos(s)` places state s; `look(s)`
  // gives its scale, opacity and highlights; `edgeLook(e)` how far an edge is
  // drawn and whether it is lit; `bendOf(e)` its curve. Returns each edge's
  // geometry, keyed by "from>to", for things that move along edges.
  function drawMachine(view, pos, look, edgeLook, bendOf) {
    const { machine } = view;
    for (const s of machine.states) {
      const n = view.nodes[s];
      const [x, y] = pos(s);
      const l = look(s);
      set(n.g, { transform: `translate(${x} ${y}) scale(${l.scale})`, opacity: clamp(l.opacity) });
      set(n.halo, { opacity: l.halo || 0 });
      const stroke = l.accent ? "var(--accent)" : "var(--line)";
      set(n.ring, { stroke });
      if (n.inner) set(n.inner, { stroke, transform: `scale(${l.inner ?? 1})` });
      set(n.name, { opacity: l.name ?? 1 });
    }
    // The start arrow follows the start state.
    const [sx, sy] = pos(machine.start);
    const sl = look(machine.start);
    const reach = clamp(sl.arrow ?? 1);
    set(view.start.line, { x1: sx - R - 42, y1: sy, x2: sx - R - 42 + 40 * reach, y2: sy, opacity: clamp(sl.opacity) });
    set(view.start.head, { points: arrowPoints([sx - R - 1, sy], [1, 0]), opacity: reach > 0.9 ? clamp(sl.opacity) : 0 });
    const geometry = {};
    for (const v of view.edges) {
      const { e } = v;
      const l = edgeLook(e);
      const geo = e.from === e.to ? loopGeometry(pos(e.from)) : edgeGeometry(pos(e.from), pos(e.to), bendOf(e));
      geometry[`${e.from}>${e.to}`] = geo;
      const color = l.accent ? "var(--accent)" : "var(--line)";
      set(v.path, { d: geo.d, stroke: color, "stroke-width": l.accent ? 3 : 2, "stroke-dashoffset": 1 - clamp(l.draw) });
      set(v.head, { points: arrowPoints(geo.tip, geo.tan), fill: color, opacity: l.draw > 0.92 ? 1 : 0 });
      set(v.label, { x: geo.label[0], y: geo.label[1], fill: l.accent ? "var(--accent)" : "var(--ink)", opacity: clamp((l.draw - 0.4) / 0.6) * (l.label ?? 1) });
      set(v.g, { opacity: clamp(l.opacity ?? 1) });
    }
    return geometry;
  }

  // Throws unless `layout` places exactly the states kstar printed.
  function checkLayout(name, layout, machine) {
    const placed = Object.keys(layout).sort().join(" ");
    const real = [...machine.states].sort().join(" ");
    if (placed !== real) throw new Error(`${name} places [${placed}] but kstar printed [${real}]`);
  }

  // The K* mark from assets/logo.svg and the name, top left.
  function brand(parent) {
    const mark = el("g", { transform: "translate(28 30) scale(0.125)" }, parent);
    el("rect", { width: 256, height: 256, rx: 52, fill: "#2b2d7c" }, mark);
    const k = el("g", { fill: "#ffffff" }, mark);
    el("rect", { x: 42, y: 96, width: 44, height: 124 }, k);
    el("polygon", { points: "80,160 140,96 196,96 118,178 86,192" }, k);
    el("polygon", { points: "106,158 144,140 204,220 148,220" }, k);
    const star = el("g", { stroke: "#ffffff", "stroke-width": 18, "stroke-linecap": "round", transform: "translate(206 52)" }, mark);
    el("line", { x1: 0, y1: -27, x2: 0, y2: 27 }, star);
    el("line", { x1: -23.4, y1: -13.5, x2: 23.4, y2: 13.5 }, star);
    el("line", { x1: -23.4, y1: 13.5, x2: 23.4, y2: -13.5 }, star);
    el("text", { class: "sans", x: 70, y: 53, fill: "var(--ink)", "font-size": 19, "font-weight": 600, "letter-spacing": -0.2 }, parent).textContent = "Kleene Star";
  }

  // Plays the animation in a normal browser tab; render.mjs drives the clock
  // itself instead (navigator.webdriver is true there).
  function play(render) {
    if (navigator.webdriver) return;
    const t0 = performance.now();
    const tick = () => { render(performance.now() - t0); requestAnimationFrame(tick); };
    requestAnimationFrame(tick);
  }

  return { R, clamp, lerp, p, ease, pop, leave, el, set, edgeGeometry, loopGeometry, arrowPoints, buildMachine, drawMachine, checkLayout, brand, play };
})();

# The demo animation

`assets/demo.gif` shows the regular expression `/(ab)*/` becoming a machine
in four stages: the regex, the eps-NFA from Thompson's construction, the DFA
from the subset construction, and the minimal DFA. `assets/demo.png` is its
last frame, for places that cannot show animation.

The machines are not drawn by hand. [`demo.kstar`](demo.kstar) builds them
with `to_nfa`, `to_dfa` and `minimize`, and prints them. The build runs it
with the real `kstar`, reads the printed tables and animates exactly those
states and transitions. Only where each state sits on screen is chosen by
hand, in `LAYOUT` in `demo.js`, and the page refuses to draw if that layout
and the printed machines disagree.

## Regenerating it

You need Node.js and Google Chrome or Microsoft Edge (nothing is downloaded;
set `BROWSER` to the browser's path if it is not found). From this folder:

```bash
npm ci
```

```bash
npm run build
```

That runs `demo.kstar` with `cargo run` (set `KSTAR` to use an installed
`kstar` instead), writes `machines.js`, plays `demo.html` in a headless
browser one frame at a time and writes `assets/demo.gif` and
`assets/demo.png`. To check frames by eye, `node render.mjs --stills DIR`
also saves every fifth frame as a PNG.

To watch the animation live, open `demo.html` in a browser after `npm ci`.

## Files

```text
demo.kstar    the program whose output is animated
machines.mjs  runs it and turns the printed tables into machines.js
demo.html     the page: layout, colours, fonts
demo.js       the animation, a pure function of time (window.render(t))
render.mjs    captures the frames and encodes the GIF with gifenc
```

The design: one deep-ink background from the logo's indigo, so the GIF reads
as a framed card on both GitHub's light and dark themes, one light-indigo
accent, Geist for text and JetBrains Mono for code (it has the Greek `ε`).
The motion is playful but short: states pop in with a small overshoot,
edges draw from their source, and in the last stage the two equivalent
accepting states light up, slide together and land as one.

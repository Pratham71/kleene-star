# The animations

Three GIFs in `assets/`, each with a still of its last frame (`.png`) for
places that cannot show animation:

| GIF | What it shows | Program |
|---|---|---|
| `demo.gif` | `/(ab)*/` becoming an eps-NFA (Thompson's construction), a DFA (subset construction) and a minimal DFA | [`demo.kstar`](demo.kstar) |
| `draw.gif` | The DFA `Div3` written a line at a time, its diagram growing with each line, then `draw Div3` | [`div3.kstar`](div3.kstar) |
| `trace.gif` | `trace Div3 on "1001"`, one symbol at a time, with the table `trace` prints | [`div3.kstar`](div3.kstar) |

The machines are not drawn by hand. The build runs each program with the
real `kstar` and reads what it prints: the machine tables, the trace table,
the line `draw` prints, and the program's own lines. The animations show
exactly those states, edges, rows and lines. Only where each state sits on
screen is chosen by hand (`LAYOUT` in `demo.js` and `div3-scene.js`), and a
page refuses to draw if its layout, the lines it shows or the steps it
animates disagree with the machine `kstar` printed.

## Regenerating them

You need Node.js and Google Chrome or Microsoft Edge (nothing is downloaded;
set `BROWSER` to the browser's path if it is not found). From this folder:

```bash
npm ci
```

```bash
npm run build
```

That runs the programs with `cargo run` (set `KSTAR` to use an installed
`kstar` instead), writes `machines.js`, plays each page in a headless
browser one frame at a time and writes the GIFs and stills into `assets/`.
`node render.mjs draw` rebuilds only one, and `node render.mjs --stills DIR`
also saves every fifth frame as a PNG, to check frames by eye. The output is
the same on every run, so an unchanged animation gives an unchanged file.

To watch an animation live, open its page (`demo.html`, `draw.html`,
`trace.html`) in a browser after `npm ci`.

## Files

```text
demo.kstar     the program behind demo.gif
div3.kstar     the program behind draw.gif and trace.gif
machines.mjs   runs them and turns what they print into machines.js
kit.js         shared drawing: easing, the K* mark, state machines and edges
style.css      shared fonts and colours
demo.js        demo.gif's animation (demo.html is its page)
div3-scene.js  the frame and layout that draw.js and trace.js share
draw.js        draw.gif's animation (draw.html)
trace.js       trace.gif's animation (trace.html)
render.mjs     captures the frames and encodes the GIFs with gifenc
```

Each animation is a pure function of time, `window.render(t)`, so the
capture sets the clock for each frame instead of waiting for it.

The design: one deep-ink background from the logo's indigo, so the GIFs read
as framed cards on both GitHub's light and dark themes, one light-indigo
accent, Geist for text and JetBrains Mono for code (it has the Greek `ε`;
its arrow ligatures are turned off in source code, so `-->` shows as
typed). The motion is playful but short: things pop in with a small
overshoot, edges draw from their source, and each change on screen follows
the line or step that causes it.

// Captures the animations into GIFs, with a still of each one's last frame:
//
//   demo.html   ->  assets/demo.gif,  assets/demo.png   a regex becomes a minimal DFA
//   draw.html   ->  assets/draw.gif,  assets/draw.png   a DFA written, then drawn
//   trace.html  ->  assets/trace.gif, assets/trace.png  a DFA reading a word
//
//   npm run build                      machines.js from kstar, then every GIF
//   node render.mjs draw trace         only these
//   node render.mjs --stills DIR ...   also every fifth frame, as PNGs in DIR/<name>/
//
// It drives a browser that is already installed (Chrome or Edge; set
// BROWSER=path to choose one) through puppeteer-core, so nothing is
// downloaded. Each animation is a pure function of time, so a frame is
// rendered by setting the clock, not by waiting, and the GIFs come out the
// same on every run.

import { mkdirSync, writeFileSync, existsSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import puppeteer from "puppeteer-core";
import { PNG } from "pngjs";
import gifenc from "gifenc";
import { writeMachines } from "./machines.mjs";

const { GIFEncoder, quantize, applyPalette } = gifenc;
const here = dirname(fileURLToPath(import.meta.url));
const assets = join(here, "..", "..", "assets");
const SCENES = ["demo", "draw", "trace"];
const FPS = 25;

function findBrowser() {
  const candidates = [
    process.env.BROWSER,
    // Chrome first: Edge's startup boost can make a headless Edge exit at once.
    "C:/Program Files/Google/Chrome/Application/chrome.exe",
    "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/usr/bin/google-chrome", "/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/microsoft-edge",
  ].filter(Boolean);
  const found = candidates.find((p) => existsSync(p));
  if (!found) throw new Error("no Chrome or Edge found; set BROWSER to the path of one");
  return found;
}

const args = process.argv.slice(2);
const stillsAt = args.indexOf("--stills");
const stillsDir = stillsAt >= 0 ? args.splice(stillsAt, 2)[1] : null;
const scenes = args.length ? args : SCENES;
for (const name of scenes) if (!SCENES.includes(name)) throw new Error(`no animation called ${name}; there are ${SCENES.join(", ")}`);

// 1. The machines, from the real kstar.
writeMachines();

// 2. The frames, then the GIF, for each animation.
const browser = await puppeteer.launch({ executablePath: findBrowser(), headless: true, args: ["--force-color-profile=srgb", "--font-render-hinting=none"] });
try {
  for (const name of scenes) await capture(name);
} finally {
  await browser.close();
}

async function capture(name) {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(pathToFileURL(join(here, `${name}.html`)).href);
  await page.evaluate(() => window.READY);
  if (errors.length) throw new Error(`${name}.html failed: ${errors.join("; ")}`);
  const { width, height, total, still } = await page.evaluate(() => {
    const svg = document.querySelector("svg");
    return { width: Number(svg.getAttribute("width")), height: Number(svg.getAttribute("height")), total: window.DEMO_TOTAL, still: window.DEMO_STILL };
  });
  await page.setViewport({ width, height, deviceScaleFactor: 1 });

  const frameAt = async (t) => {
    await page.evaluate((t) => window.render(t), t);
    return PNG.sync.read(Buffer.from(await page.screenshot({ type: "png", clip: { x: 0, y: 0, width, height } })));
  };
  const dir = stillsDir && join(stillsDir, name);
  if (dir) mkdirSync(dir, { recursive: true });
  const step = 1000 / FPS;
  const frames = [];
  for (let i = 0; i * step < total; i++) {
    const png = await frameAt(i * step);
    frames.push(png.data);
    if (dir && i % 5 === 0) writeFileSync(join(dir, `t${String(Math.round(i * step)).padStart(5, "0")}.png`), PNG.sync.write(png));
  }
  writeFileSync(join(assets, `${name}.png`), PNG.sync.write(await frameAt(still)));
  await page.close();
  if (errors.length) throw new Error(`${name}.html failed: ${errors.join("; ")}`);

  writeFileSync(join(assets, `${name}.gif`), encode(frames, width, height, step));
  const size = statSync(join(assets, `${name}.gif`)).size;
  console.log(`wrote assets/${name}.gif: ${width}x${height}, ${frames.length} frames at ${FPS} fps, ${(total / 1000).toFixed(1)} s, ${(size / 1024 / 1024).toFixed(2)} MB; ${name}.png is the frame at ${still} ms`);
}

// One palette for every frame (no flicker), identical frames merged into one
// longer frame, and only the pixels that changed written: the rest is
// transparent and shows the frame before.
function encode(frames, width, height, step) {
  // Sample pixels from every frame, so colours that appear briefly are kept.
  const all = new Uint8Array(frames.length * 4096 * 4);
  frames.forEach((f, i) => {
    for (let k = 0; k < 4096; k++) {
      const o = ((k * 7919) % (width * height)) * 4;
      all.set(f.subarray(o, o + 4), (i * 4096 + k) * 4);
    }
  });
  const palette = quantize(all, 255, { format: "rgb565" });
  const TRANSPARENT = palette.length; // one spare index for "unchanged"
  palette.push([255, 0, 255]);

  const gif = GIFEncoder();
  let previous = null;
  let pending = null; // { index, delay, diff }
  let first = true;
  const flush = () => {
    if (!pending) return;
    gif.writeFrame(pending.index, width, height, {
      palette: first ? palette : undefined,
      delay: pending.delay,
      repeat: 0,
      transparent: pending.diff,
      transparentIndex: TRANSPARENT,
      dispose: 1,
    });
    first = false;
    pending = null;
  };
  for (const data of frames) {
    const index = applyPalette(data, palette, "rgb565");
    if (previous) {
      let changed = false;
      const diff = new Uint8Array(index.length);
      for (let k = 0; k < index.length; k++) {
        if (index[k] === previous[k]) diff[k] = TRANSPARENT; else { diff[k] = index[k]; changed = true; }
      }
      if (!changed) { pending.delay += step; continue; }
      flush();
      pending = { index: diff, delay: step, diff: true };
    } else {
      pending = { index, delay: step, diff: false };
    }
    previous = index;
  }
  flush();
  gif.finish();
  return gif.bytes();
}

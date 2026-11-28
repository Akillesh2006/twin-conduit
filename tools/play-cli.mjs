// Playable Node front end.
//
// Runs the exact same Rust game (compiled to tools/harness.wasm) inside Node's
// terminal. This exists because some Windows machines block newly compiled
// native executables via Smart App Control; Node itself is already trusted, so
// this is a fully playable build without touching that setting.
//
//   node tools/play-cli.mjs
//
// Keys: P1 WASD, P2 arrow keys, Enter/Space start, P pause, R restart, Esc back.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const { instance } = await WebAssembly.instantiate(readFileSync(join(here, "harness.wasm")), {});
const x = instance.exports;
const mem = () => new Uint8Array(x.memory.buffer);
const take = (ptr, len) => mem().slice(ptr, ptr + len);

const TICK_MS = 60;

// Harness key ids.
const KEY = {
  W: 0, A: 1, S: 2, D: 3,
  UP: 4, DOWN: 5, LEFT: 6, RIGHT: 7,
  ENTER: 8, SPACE: 9, ESC: 10, P: 11, R: 12, Q: 13,
  N1: 14, N2: 15, N3: 16, N4: 17,
};
const MOVE = new Set([KEY.W, KEY.A, KEY.S, KEY.D, KEY.UP, KEY.DOWN, KEY.LEFT, KEY.RIGHT]);

if (!process.stdout.isTTY || !process.stdin.isTTY) {
  console.error("CONDUIT needs a real terminal (run it directly, not piped).");
  process.exit(1);
}

const cols = () => process.stdout.columns || 100;
const rows = () => process.stdout.rows || 32;

process.stdout.write("\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H");
process.stdin.setRawMode(true);
process.stdin.resume();
process.stdin.setEncoding("utf8");

x.init(cols(), rows());
if (typeof x.set_has_release === "function") x.set_has_release(0);

let alive = true;

function cleanup() {
  if (!alive) return;
  alive = false;
  process.stdout.write("\x1b[0m\x1b[?25h\x1b[?1049l");
  try {
    process.stdin.setRawMode(false);
  } catch {}
  process.stdin.pause();
}
process.on("exit", cleanup);
process.on("SIGINT", () => {
  cleanup();
  process.exit(0);
});

function press(id) {
  // Send raw keystroke once: single tap steps 1 cell cleanly; OS repeat while held triggers steady stride.
  x.key_event(id, 0);
}

function feed(text) {
  let i = 0;
  while (i < text.length) {
    const ch = text[i];
    if (ch === "\x1b") {
      const seq = text.slice(i, i + 3);
      if (seq === "\x1b[A") { press(KEY.UP); i += 3; continue; }
      if (seq === "\x1b[B") { press(KEY.DOWN); i += 3; continue; }
      if (seq === "\x1b[C") { press(KEY.RIGHT); i += 3; continue; }
      if (seq === "\x1b[D") { press(KEY.LEFT); i += 3; continue; }
      press(KEY.ESC);
      i += 1;
      continue;
    }
    i += 1;
    switch (ch) {
      case "\x03": cleanup(); process.exit(0); break;
      case "\r":
      case "\n": press(KEY.ENTER); break;
      case " ": press(KEY.SPACE); break;
      case "w": case "W": press(KEY.W); break;
      case "a": case "A": press(KEY.A); break;
      case "s": case "S": press(KEY.S); break;
      case "d": case "D": press(KEY.D); break;
      case "p": case "P": press(KEY.P); break;
      case "r": case "R": press(KEY.R); break;
      case "q": case "Q": press(KEY.Q); break;
      case "1": press(KEY.N1); break;
      case "2": press(KEY.N2); break;
      case "3": press(KEY.N3); break;
      case "4": press(KEY.N4); break;
      default: break;
    }
  }
}

process.stdin.on("data", feed);

process.stdout.on("resize", () => {
  x.resize(cols(), rows());
  x.draw_now();
  process.stdout.write(take(x.render_ansi(), x.out_len()));
});

function frame() {
  if (!alive) return;
  if (x.tick() === 1) {
    cleanup();
    process.exit(0);
  }
  process.stdout.write(take(x.render_ansi(), x.out_len()));
}

x.draw_now();
process.stdout.write(take(x.render_ansi(), x.out_len()));
setInterval(frame, TICK_MS);

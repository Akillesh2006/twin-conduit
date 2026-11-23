// Full playthrough smoke test.
//
// Drives the real game (compiled to wasm) with a bot that anchors player one on
// one side of the nearest enemy and mirrors player two on the other, so the live
// conduit sweeps the swarm. It huddles whenever charge runs low.
//
//   node tools/play.mjs
//
// Exits non-zero if the bot fails to reach and clear wave 12.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const { instance } = await WebAssembly.instantiate(readFileSync(join(here, "harness.wasm")), {});
const x = instance.exports;
const mem = () => new Uint8Array(x.memory.buffer);
const str = (p, n) => new TextDecoder().decode(mem().slice(p, p + n));
const state = () => JSON.parse(str(x.state(), x.out_len()));

const D = { up: 0, down: 1, left: 2, right: 3 };
function step(cur, target) {
  const dx = target.x - cur.x;
  const dy = target.y - cur.y;
  if (dx !== 0) return dx > 0 ? D.right : D.left;
  if (dy !== 0) return dy > 0 ? D.down : D.up;
  return -1;
}
const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));

x.init(120, 36);
x.key_event(8, 0); // Enter
x.step(-1, -1, 0);

let reached12 = false;
let lastWave = 0;

for (let tick = 0; tick < 30000; tick++) {
  const w = state();
  if (w.mode !== "run") break;

  let p1;
  let p2;
  const low = w.charge < 28;
  const down = w.php === 0 || w.qhp === 0;

  if (low || down) {
    // Huddle in the middle: recharge, or revive a downed partner.
    p1 = { x: 56, y: 12 };
    p2 = { x: 59, y: 12 };
  } else if (w.foes.length === 0) {
    p1 = { x: 45, y: 10 };
    p2 = { x: 70, y: 16 };
  } else {
    // Target the enemy nearest player one; put the beam across it.
    let best = null;
    let bd = Infinity;
    for (const [ex, ey] of w.foes) {
      const d = Math.abs(ex - w.px) + Math.abs(ey - w.py);
      if (d < bd) {
        bd = d;
        best = [ex, ey];
      }
    }
    const [ex, ey] = best;
    const dir = ex >= 60 ? -1 : 1;
    p1 = { x: clamp(ex - 7 * dir, 1, 82), y: clamp(ey - 2, 1, 22) };
    p2 = { x: clamp(ex + 7 * dir, 1, 82), y: clamp(ey + 2, 1, 22) };
  }

  x.step(step({ x: w.px, y: w.py }, p1), step({ x: w.qx, y: w.qy }, p2), 0);

  const n = state();
  if (n.wave !== lastWave) {
    console.log(
      `wave ${String(n.wave).padStart(2)}  t=${String(tick).padStart(5)}  ` +
        `kills=${String(n.kills).padStart(3)}  score=${String(n.score).padStart(6)}  ` +
        `charge=${n.charge.toFixed(0).padStart(3)}`
    );
    lastWave = n.wave;
    if (n.wave >= 12) reached12 = true;
  }
}

const f = state();
console.log("final:", JSON.stringify({
  wave: f.wave,
  kills: f.kills,
  score: f.score,
  phase: f.phase,
  charge: Number(f.charge.toFixed(1)),
}));

const won = f.phase === "won";
if (!won || !reached12) {
  console.log("FAILED: the bot did not clear all 12 waves");
  process.exit(1);
}
console.log("PASSED: all 12 waves cleared");

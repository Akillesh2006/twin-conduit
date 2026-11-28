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
x.set_difficulty(0); // Easy mode for automated smoke playthrough
x.key_event(8, 0); // Enter
x.step(-1, -1, 0);

let reached12 = false;
let lastWave = 0;

function inWall(x, y, walls) {
  for (const [wx, wy, len, vert] of walls || []) {
    if (vert ? (x === wx && y >= wy && y < wy + len) : (y === wy && x >= wx && x < wx + len)) {
      return true;
    }
  }
  return false;
}

function safeTarget(cur, want, w) {
  const opts = [
    { d: -1, x: cur.x, y: cur.y },
    { d: D.up, x: cur.x, y: Math.max(0, cur.y - 1) },
    { d: D.down, x: cur.x, y: Math.min(w.fh - 1, cur.y + 1) },
    { d: D.left, x: Math.max(0, cur.x - 1), y: cur.y },
    { d: D.right, x: Math.min(w.fw - 1, cur.x + 1), y: cur.y },
  ];
  let bestMove = step(cur, want);
  let bestScore = -1e9;
  for (const o of opts) {
    let sc = -Math.hypot(want.x - o.x, want.y - o.y);
    if (inWall(o.x, o.y, w.walls)) sc -= 80;
    for (const [hx, hy, hw, hh, active] of w.hazards || []) {
      if (o.x >= hx && o.x < hx + hw && o.y >= hy && o.y < hy + hh) {
        sc -= active ? 55 : 22;
      }
    }
    for (const [bx, by] of w.bullets || []) {
      const dist = Math.hypot(bx - o.x, by - o.y);
      if (dist < 2.5) sc -= (2.8 - dist) * 25;
    }
    for (const [cx, cy, r] of w.blasts || []) {
      if (Math.max(Math.abs(cx - o.x), Math.abs(cy - o.y)) <= r + 1) sc -= 40;
    }
    for (const [x0, y0, x1, y1, fuse] of w.lasers || []) {
      const weight = fuse <= 6 ? 60 : 18;
      if (x0 === x1 && Math.abs(o.x - x0) <= 1) sc -= weight;
      if (y0 === y1 && Math.abs(o.y - y0) <= 1) sc -= weight;
    }
    for (const [fx, fy] of w.foes) {
      if (fx === o.x && fy === o.y) sc -= 50;
    }
    if (sc > bestScore) {
      bestScore = sc;
      bestMove = o.d;
    }
  }
  return bestMove;
}

let recharging = false;
const upPriority = ["HALO", "COIL", "SHIELD", "CHAIN", "REACTOR", "MAGNET", "OVERCLK", "RELINK", "REACH"];

for (let tick = 0; tick < 30000; tick++) {
  const w = state();
  if (w.mode === "upgrade") {
    const choices = w.upgrade_choices || [];
    let bestIdx = 0;
    let bestRank = 99;
    for (let i = 0; i < choices.length; i++) {
      const r = upPriority.indexOf(choices[i]);
      if (r !== -1 && r < bestRank) {
        bestRank = r;
        bestIdx = i;
      }
    }
    x.key_event(14 + bestIdx, 0);
    x.step(-1, -1, 0);
    continue;
  }
  if (w.mode !== "run") break;

  if (w.charge < 32) recharging = true;
  if (w.charge >= 78) recharging = false;

  let p1;
  let p2;
  if (!w.p1alive) {
    p1 = { x: w.px, y: w.py };
    p2 = { x: w.px, y: w.py };
  } else if (!w.p2alive) {
    p1 = { x: w.qx, y: w.qy };
    p2 = { x: w.qx, y: w.qy };
  } else if (recharging) {
    const mx = Math.round((w.px + w.qx) / 2);
    const my = Math.round((w.py + w.qy) / 2);
    p1 = { x: clamp(mx - 1, 1, w.fw - 2), y: my };
    p2 = { x: clamp(mx + 1, 1, w.fw - 2), y: my };
  } else if (w.foes.length === 0) {
    p1 = { x: Math.round(w.fw * 0.4), y: Math.round(w.fh * 0.4) };
    p2 = { x: Math.round(w.fw * 0.6), y: Math.round(w.fh * 0.6) };
  } else {
    // Pick nearest enemy to player 1 and orient a long (>=18) beam straight across it.
    let best = w.foes[0];
    let bd = Infinity;
    for (const [ex, ey] of w.foes) {
      const d = Math.abs(ex - w.px) + Math.abs(ey - w.py);
      if (d < bd) {
        bd = d;
        best = [ex, ey];
      }
    }
    const [ex, ey] = best;
    const horiz = ey <= 2 || ey >= w.fh - 3;
    const vert = ex <= 2 || ex >= w.fw - 3;
    if (horiz && vert) {
      // Corner foe: anchor P1 adjacent (halo hits corner) and stretch P2 inward.
      const sx = ex < 3 ? 1 : -1;
      const sy = ey < 3 ? 1 : -1;
      p1 = { x: clamp(ex + sx, 0, w.fw - 1), y: clamp(ey + sy, 0, w.fh - 1) };
      p2 = { x: clamp(ex + sx * 17, 0, w.fw - 1), y: clamp(ey + sy * 6, 0, w.fh - 1) };
    } else if (horiz) {
      p1 = { x: clamp(ex - 9, 0, w.fw - 1), y: ey };
      p2 = { x: clamp(ex + 9, 0, w.fw - 1), y: ey };
    } else if (vert || w.blocked) {
      const sideX = inWall(ex, ey, w.walls) ? clamp(ex + 1, 0, w.fw - 1) : ex;
      p1 = { x: sideX, y: clamp(ey - 8, 0, w.fh - 1) };
      p2 = { x: sideX, y: clamp(ey + 8, 0, w.fh - 1) };
    } else {
      const sway = (tick % 40) < 20 ? 2 : -2;
      p1 = { x: clamp(ex - 9, 0, w.fw - 1), y: clamp(ey - sway, 0, w.fh - 1) };
      p2 = { x: clamp(ex + 9, 0, w.fw - 1), y: clamp(ey + sway, 0, w.fh - 1) };
    }
  }

  x.step(safeTarget({ x: w.px, y: w.py }, p1, w), safeTarget({ x: w.qx, y: w.qy }, p2, w), 0);

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

// Diagnostic: print the simulation every N ticks so balance can be inspected.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const { instance } = await WebAssembly.instantiate(readFileSync(join(here, "harness.wasm")), {});
const x = instance.exports;
const mem = () => new Uint8Array(x.memory.buffer);
const str = (p, n) => new TextDecoder().decode(mem().slice(p, p + n));
const state = () => JSON.parse(str(x.state(), x.out_len()));
const DIR = { up: 0, down: 1, left: 2, right: 3 };

function toward(cur, t) {
  const dx = t.x - cur.x;
  const dy = t.y - cur.y;
  if (dx !== 0) return dx > 0 ? DIR.right : DIR.left;
  if (dy !== 0) return dy > 0 ? DIR.down : DIR.up;
  return -1;
}

x.init(120, 36);
x.key_event(8, 0);
x.step(-1, -1, 0);

for (let tick = 0; tick < 800; tick++) {
  const w = state();
  if (w.mode !== "run") {
    console.log(`t=${tick} ENDED mode=${w.mode} phase=${w.phase}`);
    break;
  }
  const cyc = 260;
  const p = tick % cyc;
  const stretch = p < 168;
  const sway = Math.sin(tick / 55) * 9;
  const p1 = stretch ? { x: 45, y: Math.round(13 + sway) } : { x: 50, y: 13 };
  const p2 = stretch ? { x: 75, y: Math.round(13 - sway) } : { x: 53, y: 13 };
  x.step(toward({ x: w.px, y: w.py }, p1), toward({ x: w.qx, y: w.qy }, p2), 0);

  if (tick % 20 === 0) {
    const n = state();
    console.log(
      `t=${String(tick).padStart(3)} d=${String(n.dist).padStart(2)} hot=${n.hot ? 1 : 0} ` +
        `chg=${n.charge.toFixed(0).padStart(3)} kills=${n.kills} foes=${JSON.stringify(n.foes)} ` +
        `p=(${n.px},${n.py}) q=(${n.qx},${n.qy})`
    );
  }
}

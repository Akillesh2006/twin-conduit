// Headless CONDUIT harness runner.
//
// Loads tools/harness.wasm (the real game code compiled to wasm), drives it with
// a bot, and checks the simulation behaves. Prints frames as text so the
// renderer can be eyeballed without running a native binary.
//
//   node tools/run.mjs            # play + report
//   node tools/run.mjs --frames    # also dump gameplay frames

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const wasm = readFileSync(join(here, "harness.wasm"));
const { instance } = await WebAssembly.instantiate(wasm, {});
const x = instance.exports;
const mem = () => new Uint8Array(x.memory.buffer);

function str(ptr, len) {
  return new TextDecoder().decode(mem().slice(ptr, ptr + len));
}

function render() {
  return str(x.render(), x.out_len());
}
function renderAnsi() {
  return str(x.render_ansi(), x.out_len());
}
function state() {
  return JSON.parse(str(x.state(), x.out_len()));
}

const W = 120;
const H = 36;
x.init(W, H);

const showFrames = process.argv.includes("--frames");
const failures = [];
function ok(cond, label) {
  if (cond) {
    console.log(`  PASS  ${label}`);
  } else {
    console.log(`  FAIL  ${label}`);
    failures.push(label);
  }
}

// ---------------------------------------------------------------- title
x.draw_now();
console.log("\n=== TITLE SCREEN (plain) ===\n");
console.log(render());

const ansi = renderAnsi();
console.log(`ANSI frame: ${ansi.length} bytes, sync markers: ${ansi.includes("\x1b[?2026h")}`);

// Start a run: press Enter (id 8), then a step consumes the edge flag.
x.key_event(8, 0);
x.step(-1, -1, 0);
let s = state();
ok(s.mode === "run", "Enter starts a run");
ok(s.wave === 1, "run begins on wave 1");
ok(s.enemies >= 0, "enemy list initialised");

// ---------------------------------------------------------------- bot
// Keep the pair ~30 cells apart (conduit hot, no snap) and rotate the beam by
// sweeping their heights in antiphase. That is the actual intended play pattern.
const DIR = { up: 0, down: 1, left: 2, right: 3 };
const NONE = -1;

function stepToward(cur, target) {
  const dx = target.x - cur.x;
  const dy = target.y - cur.y;
  if (dx !== 0) return dx > 0 ? DIR.right : DIR.left;
  if (dy !== 0) return dy > 0 ? DIR.down : DIR.up;
  return NONE;
}

const frames = [];
let killsSeen = 0;
let maxEnemies = 0;
let sawHot = false;
let sawCold = false;
let minCharge = 100;
let comboSeen = 0;
let recharging = false;

for (let tick = 0; tick < 3000; tick++) {
  const w = state();
  if (w.mode === "upgrade") {
    x.key_event(14, 0);
    x.step(NONE, NONE, 0);
    continue;
  }
  if (w.mode !== "run") break;

  if (w.charge < 42) recharging = true;
  if (w.charge >= 82) recharging = false;

  const focus = w.foes && w.foes.length > 0
    ? { x: Math.max(12, Math.min(72, w.foes[0][0])), y: Math.max(6, Math.min(20, w.foes[0][1])) }
    : { x: 42, y: 13 };
  const sway = Math.round(Math.sin(tick / 18) * 6);
  const p1 = recharging
    ? { x: focus.x, y: focus.y }
    : { x: Math.max(4, focus.x - 8), y: Math.max(3, Math.min(22, focus.y + sway)) };
  const p2 = recharging
    ? { x: focus.x + 2, y: focus.y }
    : { x: Math.min(80, focus.x + 8), y: Math.max(3, Math.min(22, focus.y - sway)) };

  const d0 = stepToward({ x: w.px, y: w.py }, p1);
  const d1 = stepToward({ x: w.qx, y: w.qy }, p2);
  x.step(d0, d1, 0);

  const n = state();
  killsSeen = Math.max(killsSeen, n.kills);
  maxEnemies = Math.max(maxEnemies, n.enemies);
  if (n.hot) sawHot = true;
  if (!n.hot) sawCold = true;
  minCharge = Math.min(minCharge, n.charge);
  comboSeen = Math.max(comboSeen, n.best_combo);
  if (showFrames && tick % 400 === 0) frames.push({ tick, frame: render(), state: n });
}

const final = state();
if (showFrames) {
  console.log("\n=== GAMEPLAY FRAMES ===\n");
  for (const f of frames) {
    console.log(`--- tick ${f.tick} | wave ${f.state.wave} | kills ${f.state.kills} | charge ${f.state.charge.toFixed(1)} | dist ${f.state.dist} | hot ${f.state.hot} ---`);
    console.log(f.frame);
  }
}

console.log("=== SIMULATION ===\n");
console.log(`  final: ${JSON.stringify(final)}`);
ok(final.kills > 0, `bot killed enemies (kills=${final.kills})`);
ok(maxEnemies > 0, `enemies spawn and accumulate (max ${maxEnemies})`);
ok(sawHot, "conduit reached the hot state when stretched");
ok(sawCold || final.dist < 10, "conduit distance responds to input");
ok(final.arc > 1, `arc has length (${final.arc} cells)`);
ok(final.charge >= 0 && final.charge <= 100, "charge stays in range");
ok(final.wave >= 1, `wave advanced/progressed (wave ${final.wave})`);

// ------------------------------------------------------------ lose path
console.log("\n=== STATE TRANSITIONS ===\n");
x.set_charge(0.0);
x.step(NONE, NONE, 0);
s = state();
ok(s.phase === "lost" || s.mode === "over", `zero charge ends the run (phase=${s.phase}, mode=${s.mode})`);

x.idle(40);
s = state();
ok(s.mode === "over", "loss moves to the results screen");
console.log("\n=== RESULTS SCREEN ===\n");
console.log(render());

// ------------------------------------------------------------ win path
x.step(NONE, NONE, 1); // confirm -> restart
s = state();
ok(s.mode === "run", "Enter restarts from results");

x.set_wave(12);
s = state();
ok(s.wave === 12, "jumped to the boss wave");
x.idle(3);
x.draw_now();
console.log("\n=== BOSS WAVE FRAME ===\n");
console.log(render());

x.clear_wave();
x.idle(3);
s = state();
ok(s.phase === "won" || s.mode === "over", `clearing the last wave wins (phase=${s.phase}, mode=${s.mode})`);
ok(s.won === true, "win flag set");
x.draw_now();
console.log("\n=== VICTORY SCREEN ===\n");
console.log(render());

// ------------------------------------------------------------ pause
x.idle(30); // results screen has a short input debounce
x.step(NONE, NONE, 1);
s = state();
ok(s.mode === "run", "Enter restarts from victory");
x.step(NONE, NONE, 4); // pause
s = state();
ok(s.mode === "pause", "P pauses");
console.log("\n=== PAUSE ===\n");
console.log(render());
x.step(NONE, NONE, 4);
ok(state().mode === "run", "P resumes");
const chgBeforeBlur = state().charge;
x.key_event(-1, 3); // FocusLost (tab minimize / blur)
x.idle(5);
const blurState = state();
ok(blurState.mode === "pause" && Math.abs(blurState.charge - chgBeforeBlur) < 0.01, "focus loss auto-pauses run without draining charge");
x.key_event(-1, 4); // FocusGained (reopen tab)
x.step(NONE, NONE, 1); // Enter resumes cleanly
ok(state().mode === "run", "reopening tab can resume with Enter");

// ------------------------------------------------------------ resize
x.init(120, 36);
x.step(NONE, NONE, 0);
x.step(NONE, NONE, 1); // start a fresh run (init resets to title)

console.log("\n=== WAVE PROGRESSION ===\n");
let progressed = true;
let sawDraft = false;
for (let want = 1; want <= 12; want++) {
  const st = state();
  if (st.wave !== want) {
    console.log(`  wave expected ${want}, got ${st.wave}`);
    progressed = false;
    break;
  }
  x.set_charge(100);
  x.clear_wave();
  x.idle(5);
  if (want < 12 && state().mode === "upgrade") {
    sawDraft = true;
    x.key_event(14, 0); // choose upgrade card 1
    x.step(NONE, NONE, 0);
  }
  x.idle(65);
}
const afterWaves = state();
ok(progressed, "waves advance one after another through all 12");
ok(sawDraft && (afterWaves.upgrades || []).length === 11, "upgrade draft occurs after clearing waves 1-11");
ok(afterWaves.phase === "won" || afterWaves.mode === "over", `clearing wave 12 wins (phase=${afterWaves.phase})`);

// ------------------------------------------------------------ resize
x.init(72, 26);
x.step(NONE, NONE, 0);
s = state();
ok(s.mode === "title", "re-init resets to title");
console.log("\n=== MINIMUM SIZE (72x26) ===\n");
console.log(render());

x.init(60, 20);
x.step(NONE, NONE, 0);
const small = render();
ok(small.includes("TERMINAL TOO SMALL"), "too-small guard renders");

// ------------------------------------------------------------ tap precision via tick()
x.init(100, 32);
x.set_has_release(0);
x.key_event(8, 0); // Enter
x.tick();
const startPos = state();
x.key_event(0, 0); // Single tap W (no key-up sent)
for (let k = 0; k < 6; k++) x.tick();
const afterTap = state();
ok(
  startPos.py - afterTap.py === 1,
  `TTY single tap moves exactly 1 cell (moved=${startPos.py - afterTap.py})`
);

console.log("\n=========================================");
if (failures.length) {
  console.log(`${failures.length} FAILURE(S):`);
  for (const f of failures) console.log(`  - ${f}`);
  process.exit(1);
} else {
  console.log("ALL CHECKS PASSED");
}

# ⚡ CONDUIT (`twin-conduit`)

[![Live Playable Web Demo](https://img.shields.io/badge/Play_Online-GitHub_Pages-00f0ff?style=for-the-badge&logo=googlechrome&logoColor=white)](https://akillesh2006.github.io/twin-conduit/)
[![GitHub Repo](https://img.shields.io/badge/GitHub-twin--conduit-181717?style=for-the-badge&logo=github)](https://github.com/Akillesh2006/twin-conduit)
[![Rust](https://img.shields.io/badge/Engine-Rust_&_WebAssembly-orange?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)

> **🎮 Play Instantly in Your Browser (GitHub Pages):**  
> 👉 [**https://akillesh2006.github.io/twin-conduit/**](https://akillesh2006.github.io/twin-conduit/) 👈

A two-player, one-keyboard cooperative terminal arcade game in pure Rust & WebAssembly.

Two operators are tethered together by a shared energy conduit. **The conduit beam is your only weapon, and it only burns when the pair is stretched apart.**

```text
   GET CLOSE (dist <= 3)  ->  conduit goes cold  ->  shared CHARGE recharges (+0.52/tick base)
   NEUTRAL   (dist 4..5)  ->  slack tether       ->  neither burns nor recharges
   GET FAR   (dist >= 6)  ->  conduit ignites    ->  enemies on beam burn, CHARGE drains
   TOO FAR   (dist > 34)  ->  CONDUIT SNAPS      ->  yanked together (-14.0 CHARGE)
```

Nothing can be done alone: the straight line (`arc`) between Player 1 and Player 2 is the weapon, so slicing swarms demands constant spatial teamwork without draining shared `CHARGE` to `0%` (`SIGNAL LOST — the cell went dark`).

* **4 Difficulty Modes**: Select **Easy**, **Normal** (default), **Hard**, or **Nightmare** on the title screen (`1`–`4` or `A`/`D`).
* **3 Multi-Phase Bosses Across 12 Waves**: Fight **THE WARDEN** (Wave 4, 2 phases), **THE HYDRA** (Wave 8, 2 phases w/ Tether-Cutter beam), and **THE NULL** (Wave 12, 3 phases w/ spirals, Void Meteors & cross-arena lasers).
* **9 Shared Upgrades & 5 Arena Pickups**: Draft 1 of 3 shared upgrades between waves (including synergies and risk/reward trade-offs) and grab rare `¤` **Slow Time**, `★` **Nova**, and `≡` **Twin Beam** pickups.
* **Dynamic Obstacles, Hazards & Wave Events**: Navigate patrolling beam-blocking walls, telegraphed floor vents, projectiles, blasts, and wave modifiers (`Meteor Shower`, `Blackout`, `Enemy Surge`, `Charge Drought`).
* **Persistent High Scores & Zero External Crates**: Pure Rust (`serde`-free Win32/POSIX terminal + double-buffered 256-colour ANSI canvas at `60 ms/tick`), saving Top-5 scores to `.conduit_scores.txt` on native or `localStorage` in the web front end.

---

## 🚀 How to Play

### 🌐 Play Online (Browser)
Open [**https://akillesh2006.github.io/twin-conduit/**](https://akillesh2006.github.io/twin-conduit/) (supports desktop keyboards, touch D-pads, tab-minimize auto-pause, and persistent `localStorage` Top-5 scores).

### 💻 Play Locally in Terminal

#### Windows (One-Click / PowerShell)
Double-click [`play.bat`](play.bat), or run in PowerShell:

```powershell
# Build & launch CONDUIT (uses MSVC cargo targeting x86_64-pc-windows-gnu, falls back to Node/WASM)
.\run.ps1

# Run all 46 built-in verification checks
.\run.ps1 --test
```

#### Standard Cargo (macOS / Linux / Unrestricted Windows)
```sh
cargo run --release
cargo run --release -- --test
```

#### Node + WebAssembly Front End
If Windows Smart App Control blocks unsigned native `.exe` execution, run the identical Rust simulation via WASM in Node:

```sh
node tools/play-cli.mjs
```

> Minimum terminal size is **72x26** cells (`TERMINAL TOO SMALL` renders if smaller).

---

## 🎮 Controls

| Action | Player 1 (`◉` Cyan) | Player 2 (`◉` Magenta) |
|---|---|---|
| **Move** | `W` `A` `S` `D` | `Arrow keys` (`↑` `↓` `←` `→`) |
| **Select Difficulty / Upgrade** | `1` `2` `3` `4` or `A`/`D` | `Left`/`Right` |
| **Start / Confirm / Retry** | `Enter` or `Space` | `Enter` or `Space` |
| **Pause / Resume** | `P` | `P` |
| **Restart Run** | `R` | `R` |
| **Back to Title / Quit** | `Esc` | `Esc` |

Both operators move simultaneously: **a single tap steps 1 exact cell** (`200 ms` DAS grace window), while holding glides coherently at `115 ms/cell` (`~8.7 cells/sec`). Moving in **Coherent Stride (`SYNC`)** simultaneously reduces live beam drain by **25%**.

Switching or minimizing the browser tab (or losing terminal focus) automatically **pauses** the run and freezes simulation updates until you reopen and resume (`P`, `Enter`, or `Space`).

---

## 🎚️ Difficulty Modes

| Mode | Starting Hearts | Stretch Drain | Hug Recharge | Enemy HP | Enemy Speed | Score Mult |
|---|---:|---:|---:|---:|---:|---:|
| **1: EASY** | 4 ♥ | 0.52× | 1.45× | 0.85× | Slower (`1.25×` CD) | 0.80× |
| **2: NORMAL** *(default)* | 3 ♥ | 1.00× | 1.00× | 1.00× | Standard (`1.00×`) | 1.00× |
| **3: HARD** | 3 ♥ | 1.25× | 0.82× | 1.25× | Faster (`0.84×` CD) | 1.35× |
| **4: NIGHTMARE** | 2 ♥ | 1.50× | 0.68× | 1.50× | Relentless (`0.70×` CD) | 1.80× |

---

## ⚡ Mechanics, Pickups & Obstacles

1. **Hug (`dist <= 3`)**: Beam shifts to `COLD · RECHARGING`, restoring `+0.52 × diff × upgrades` CHARGE per tick up to `100%`.
2. **Stretch (`dist >= 6`)**: Beam ignites (`CONDUIT LIVE · REACH`), burning targets on the line (`±1` cell halo) while draining `(0.28 + (dist - 6) * 0.022) × diff × upgrades` CHARGE per tick.
3. **Snap Limit (`dist > 34`, `40` with Elastic Tether)**: Exceeding the limit triggers `CONDUIT SNAP`, pulling both operators inward (`-14.0` CHARGE, 20-tick snap cooldown).
4. **Revives (`<= 2` cells, `45` ticks)**: While an operator is `DOWN` (`0` HP), the beam cannot ignite; the partner must stand within `2` cells for `45` ticks (`24` ticks with Fast Relink) to re-link them.
5. **Patrolling Obstacles & Hazards**: Starting Wave 3, patrolling solid walls (`█`) block movement, absorb bullets, and **cut off the conduit beam past the impact point (`#`)**. Wave 5+ spawns telegraphed floor plasma vents (`░` -> `▓`).
6. **Random Wave Events**: Non-boss waves can trigger `METEOR SHOWER` (periodic `3×3` impacts), `BLACKOUT` (`12`-cell lantern radius), `ENEMY SURGE` (+3 inbound enemies), or `CHARGE DROUGHT` (`-35%` hug recharge).
7. **Arena Pickups (`Orbs`)**:
   * `○` **Charge Orb**: Restores `+26` CHARGE.
   * `✚` **Mend Orb**: Restores `+1` heart (`♥`; heals partner if collector is full).
   * `¤` **Slow Time** *(rare)*: Slows enemies & halves projectile speed for `120` ticks.
   * `★` **Nova** *(rare)*: Clears all active enemy projectiles/blasts and deals `2` damage arena-wide.
   * `≡` **Twin Beam** *(rare)*: Projects a parallel second plasma beam offset by `2` cells for `150` ticks.

---

## 👾 Threats & Bosses

| Glyph | Name | Base HP | Points | Kill Charge | Behaviour |
|---|---|---:|---:|---:|---|
| `×` | **Drifter** | 1 | 10 | +4.5% | Drifts across arena edges and migrates inward |
| `▼` | **Hunter** | 1 | 15 | +5.5% | Tracks straight toward the nearest alive operator |
| `♦` | **Shooter** | 2 | 25 | +6.0% | Maintains medium range (`8..=17`), telegraphs `12` ticks, fires dodgeable bullets |
| `»` | **Dasher** | 2 | 28 | +6.5% | Telegraphs a dashed line (`╌`, `17` ticks), then surges at high speed |
| `✹` | **Splitter** | 2 | 20 | +6.5% | Splits into two `Drifter`s when destroyed |
| `◎` | **Shielded** | 3 | 35 | +8.5% | Front armor (`│`/`─`) blocks short frontal beams — flank or stretch `dist >= 16` |
| `Ψ` | **Leech** | 3 | 32 | +8.0% | Hunts conduit midpoint and latches on, draining CHARGE until burned off |
| `☼` | **Bomber** | 2 | 30 | +7.0% | Detonates (`5×5` area) on contact or after a `10`-tick fuse upon beam death |
| `■` | **Bulwark** | 3 | 35 | +10.0% | Heavy slow juggernaut scaling in HP and speed across waves |
| `▣` | **THE WARDEN** | 15 | 150 | +25.0% | **Wave 4 Boss (2 Phases)**: aimed 3-way spreads + blasts (P1), 6-way rotating rings + Drifter summons (P2) |
| `✦` | **THE HYDRA** | 21 | 220 | +30.0% | **Wave 8 Boss (2 Phases)**: dual-target volleys + Leech/Bomber spawns (P1), telegraphed Tether-Cutter sweep + Splitters (P2) |
| `◈` | **THE NULL** | 30 | 350 | Drops orbs | **Wave 12 Boss (3 Phases)**: 8-way spirals + Hunters (P1), 5-way fans + Void Meteors + Leech/Shooter (P2), 10-way spirals + cross-arena Tether-Cutter lasers + Dasher (P3) |

---

## 🛠️ Between-Wave Upgrades (Choose 1 of 3)

| Upgrade | Effect | Notes |
|---|---|---|
| **PLASMA HALO** | `+1` beam halo radius while stretched `dist >= 10` | Wide sweep clearing |
| **INSULATED COIL** | `-22%` stretch charge drain *(stacks)* | Synergizes with Overclock Arc |
| **RAPID REACTOR** | `+28%` hug recharge rate *(stacks)* | Synergizes with tight drain waves |
| **OVERCLOCK ARC** | `2×` beam damage per tick | **Trade-off**: `+25%` stretch drain |
| **FAST RELINK** | Revives complete in `24` ticks with `2` HP | Co-op safety net |
| **ORB MAGNET** | Pulls nearby orbs within `6` cells toward players | Great during swarms |
| **PHASE SHIELD** | Blocks `1` hit per wave for each operator | Refreshes every wave |
| **CHAIN ARC** | Kills arc `1` damage to up to `2` enemies within `4` cells | Synergizes with Bomber/Splitter packs |
| **ELASTIC TETHER** | `+6` max snap limit (`40` cells) | **Trade-off**: `-12%` hug recharge |

---

## 🧪 Verification & WASM Build

Run all native, WASM, and bot verification suites:

```sh
# 46 built-in Rust requirement checks (native binary if permitted, or WASM fallback via run.ps1)
cargo run --release -- --test
.\run.ps1 --test

# Rebuild tools/harness.wasm and root harness.wasm (requires wasm32-unknown-unknown target)
rustc --edition 2021 --target wasm32-unknown-unknown --crate-type cdylib -O tools/harness.rs -o tools/harness.wasm
cp tools/harness.wasm harness.wasm

# Headless WASM assertions + rendered ASCII screens (Title, Boss, Pause, Victory, 72x26 min size)
node tools/run.mjs

# Automated Easy-difficulty playthrough bot clearing all 12 waves
node tools/play.mjs
```

---

## 📁 Project Structure

```text
index.html         WebAssembly browser front end (xterm.js, touch controls, tab-minimize pause & localStorage scores)
harness.wasm       Root copy of compiled WASM module for GitHub Pages
.nojekyll          Ensures GitHub Pages serves static assets without Jekyll
play.bat           One-click Windows terminal launcher
run.ps1            Build & launch script (MSVC Cargo -> GNU target + WASM fallback)
setup.ps1          Portable MinGW-w64 + Rust toolchain bootstrapper
Cargo.toml         Zero-dependency crate manifest (`conduit` v1.0.0)
src/
  main.rs          Terminal setup, 60 ms loop, `--test` runner
  lib.rs           Library exports for shared verification checks
  checks.rs        46 automated requirement & regression checks
  sys.rs           Zero-dependency Win32 Console / POSIX platform layer
  app.rs           Title / Run / Upgrade / Pause / Over state machine + auto-pause & score saving
  game.rs          Simulation: difficulties, tether, sync stride, enemies, bosses, upgrades, walls, hazards, events
  scene.rs         Arena rendering: warnings, beam/twin-beam, walls, hazards, blackout lighting
  ui.rs            CONDUIT wordmark, CHARGE meter, boss HP bar, upgrade selector, run summary
  scores.rs        Top-5 high-score sorting and native file persistence (`.conduit_scores.txt`)
  canvas.rs        Double-buffered 256-colour ANSI canvas with diff & sync markers
  input.rs         Simultaneous WASD + Arrow + 1-4 hotkeys, 1-cell tap precision & DAS/ARR hold timing
  keys.rs          Key identifiers and event types
  fx.rs            Particles, surge bursts, floating combat text, screen shake
  core.rs          Vec2 / Vec2f / Dir / Field / Bresenham line primitives
  rng.rs           Deterministic xorshift PRNG
tools/
  harness.rs       WASM bridge exposing simulation, state JSON & verification runner
  harness.wasm     Precompiled WebAssembly module
  run.mjs          Automated assertions & ASCII screen checks
  play.mjs         Automated Easy-mode playthrough bot clearing all 12 waves
  debug.mjs        Tick-by-tick simulation diagnostic script
  play-cli.mjs     Interactive Node/WASM terminal front end
```

---

## 📜 License

MIT License

# ⚡ CONDUIT (`twin-conduit`)

[![Live Playable Web Demo](https://img.shields.io/badge/Play_Online-GitHub_Pages-00f0ff?style=for-the-badge&logo=googlechrome&logoColor=white)](https://akillesh2006.github.io/twin-conduit/)
[![GitHub Repo](https://img.shields.io/badge/GitHub-twin--conduit-181717?style=for-the-badge&logo=github)](https://github.com/Akillesh2006/twin-conduit)
[![Rust](https://img.shields.io/badge/Engine-Rust_&_WebAssembly-orange?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)

> **🎮 Play Instantly in Your Browser (GitHub Pages):**  
> 👉 [**https://akillesh2006.github.io/twin-conduit/**](https://akillesh2006.github.io/twin-conduit/) 👈

A two-player, one-keyboard cooperative terminal arcade game in Rust & WebAssembly.

Two operators are tethered together by a shared energy conduit. **The conduit beam is your only weapon, and it only burns when the pair is stretched apart.**

```text
   GET CLOSE (dist <= 3)  ->  conduit goes cold  ->  shared CHARGE recharges (+0.86/tick)
   NEUTRAL   (dist 4..5)  ->  slack tether       ->  neither burns nor recharges
   GET FAR   (dist >= 6)  ->  conduit ignites    ->  enemies on beam burn, CHARGE drains
   TOO FAR   (dist > 34)  ->  CONDUIT SNAPS      ->  yanked together (-10 CHARGE)
```

Nothing can be done alone: the weapon is the straight line (`arc`) between Player 1 and Player 2, so sweeping enemies requires constant spatial coordination without letting the shared `CHARGE` meter hit `0%` (`SIGNAL LOST — the cell went dark`).

* **12 Waves + Boss (`THE NULL`)**: Progress through escalating waves culminating in **Wave 12: `THE NULL`** (`◈`, 26 HP, breeds `Hunter`s).
* **Shared Tether & Cooperative Revives**: Each operator has `3` hearts (`♥`). Losing all 3 sets the operator `DOWN` — the beam cannot ignite while a player is down, and the partner must stand within **2 cells** for **45 ticks** (`RE-LINKED`) to revive them with 1 heart.
* **Zero External Crates**: Pure Rust using direct Win32 Console / POSIX `stty` I/O and a double-buffered 256-colour ANSI renderer wrapped in `ESC[?2026h` / `ESC[?2026l` synchronized updates at 60 ms/tick.
* **Cross-Platform + Browser Build**: Runs natively in Windows/Unix terminals, through Node (`tools/play-cli.mjs`), or directly in any browser via WebAssembly (`index.html` + `harness.wasm`).

---

## 🚀 How to Play

### 🌐 Play Online (Browser)
Open [**https://akillesh2006.github.io/twin-conduit/**](https://akillesh2006.github.io/twin-conduit/) (supports desktop keyboards and on-screen touch D-pads).

### 💻 Play Locally in Terminal

#### Windows (One-Click / PowerShell)
Double-click [`play.bat`](play.bat), or run in PowerShell:

```powershell
# Build & launch CONDUIT (uses MSVC cargo targeting x86_64-pc-windows-gnu, falls back to Node/WASM)
.\run.ps1

# Run the 19 built-in cooperative requirement checks
.\run.ps1 --test
```

#### Standard Cargo (macOS / Linux / Unrestricted Windows)
```sh
cargo run --release
cargo run --release -- --test
```

#### Node + WebAssembly Front End
If Windows Smart App Control blocks executing unsigned `.exe` builds, play the compiled WASM build directly via Node:

```sh
node tools/play-cli.mjs     # interactive terminal game over tools/harness.wasm
```

> Minimum terminal size is **72x26** cells (resizing smaller shows `TERMINAL TOO SMALL`).

---

## 🎮 Controls

| Action | Player 1 (`◉` Cyan) | Player 2 (`◉` Magenta) |
|---|---|---|
| **Move** | `W` `A` `S` `D` | `Arrow keys` (`↑` `↓` `←` `→`) |
| **Start / Confirm / Retry** | `Enter` or `Space` | `Enter` or `Space` |
| **Pause / Resume** | `P` | `P` |
| **Restart Run** | `R` | `R` |
| **Back to Title / Quit** | `Esc` | `Esc` |

Both operators move simultaneously at 1 cell per tick (`60 ms`).

---

## ⚡ Mechanics, Pickups & Scoring

1. **Hug (`dist <= 3`)**: Beam shifts to `COLD · RECHARGING`, restoring `+0.86` CHARGE per tick up to `100%`.
2. **Stretch (`dist >= 6`)**: Beam ignites (`CONDUIT LIVE · REACH`), dealing damage per tick across the beam (`±1` cell plasma halo) while draining `0.13 + (dist - 6) * 0.011` CHARGE per tick.
3. **Snap Limit (`dist > 34`)**: Exceeding `34` cells triggers `CONDUIT SNAP`, pulling both players inward (`-10` CHARGE, 20-tick snap cooldown).
4. **Combos & Wave Bonuses**: Rapid kills build a combo multiplier up to `4x` (`1 + combo.min(9) / 3`), and clearing each wave grants `wave * 100 * mult` bonus points.
5. **Arena Pickups (`Orbs`)**:
   * `○` **Charge Orb**: Restores `+26` CHARGE.
   * `✚` **Mend Orb**: Restores `+1` heart (`♥`, up to `3` max; heals partner if collector is already full).

---

## 👾 Threats & Wave 12 Boss

| Glyph | Name | HP | Points | Kill Charge | Behaviour |
|---|---|---:|---:|---:|---|
| `×` | **Drifter** | 1 | 10 | +10% | Drifts across the arena, bounces off walls, migrates inward |
| `▼` | **Hunter** | 1 | 15 | +11% | Tracks straight toward the nearest alive operator |
| `✹` | **Splitter** | 2 | 20 | +13% | Slower chaser; splits into two `Drifter`s upon destruction |
| `■` | **Bulwark** | 3 | 35 | +20% | Heavy armoured unit; slow and relentless |
| `◈` | **THE NULL** | 26 | 250 | Drops orbs | **Wave 12 Boss**; breeds `Hunter`s until severed by the beam |

---

## 🧪 Verification & WASM Build

Run the verification suites (native + headless WASM):

```sh
# 19 built-in Rust unit/requirement checks
.\target\x86_64-pc-windows-gnu\release\conduit.exe --test

# Rebuild tools/harness.wasm and root harness.wasm (requires wasm32-unknown-unknown target)
rustc --edition 2021 --target wasm32-unknown-unknown --crate-type cdylib -O tools/harness.rs -o tools/harness.wasm
cp tools/harness.wasm harness.wasm

# Headless WASM assertions + rendered screen checks + bot playthrough to Wave 12 victory
node tools/run.mjs
node tools/play.mjs
node tools/debug.mjs
```

---

## 🌐 Deploying via GitHub Pages

1. Go to repository **Settings -> Pages** -> **Deploy from a branch** -> select **`main`** / **`/ (root)`** -> **Save**.
2. Once live at `https://akillesh2006.github.io/twin-conduit/`, add that URL under **About -> Website** on the repository main page.

---

## 📁 Project Structure

```text
index.html         WebAssembly browser front end (xterm.js + touch controls)
harness.wasm       Root copy of compiled WASM module for GitHub Pages
.nojekyll          Ensures GitHub Pages serves static assets without Jekyll
play.bat           One-click Windows terminal launcher
run.ps1            Build & launch script (MSVC Cargo -> GNU target + Node fallback)
setup.ps1          Portable MinGW-w64 + Rust toolchain bootstrapper
Cargo.toml         Zero-dependency crate manifest (`conduit` v1.0.0)
src/
  main.rs          Terminal setup, 60 ms loop, `--test` verification suite
  sys.rs           Zero-dependency Win32 Console / POSIX platform layer
  app.rs           Title / Run / Pause / Over screen state machine
  game.rs          CONDUIT simulation: tether, charge, snap, revive, 12 waves
  scene.rs         Arena rendering: floor, conduit beam states, orbs, foes, players
  ui.rs            CONDUIT 5x7 wordmark, CHARGE meter, HUD, pause & results
  canvas.rs        Double-buffered 256-colour ANSI canvas with diff & sync markers
  input.rs         Simultaneous WASD + Arrow held-key tracking
  keys.rs          Key identifiers and event types
  fx.rs            Particles, floating combat text, screen shake
  core.rs          Vec2 / Dir / Field / Bresenham line primitives
  rng.rs           Deterministic xorshift PRNG
tools/
  harness.rs       WASM bridge exposing simulation & ANSI rendering
  harness.wasm     Precompiled WebAssembly module
  run.mjs          Automated assertions & ASCII screen checks
  play.mjs         Automated beam-aiming bot playthrough to Wave 12 victory
  debug.mjs        Tick-by-tick simulation diagnostic script
  play-cli.mjs     Interactive Node/WASM terminal front end
```

---

## 📜 License

MIT License

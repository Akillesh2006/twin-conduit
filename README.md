# ⚡ TWIN CONDUIT

[![Live Playable Web Demo](https://img.shields.io/badge/Play_Online-GitHub_Pages-00f0ff?style=for-the-badge&logo=googlechrome&logoColor=white)](https://akillesh2006.github.io/twin-conduit/)
[![GitHub Repo](https://img.shields.io/badge/GitHub-twin--conduit-181717?style=for-the-badge&logo=github)](https://github.com/Akillesh2006/twin-conduit)
[![Rust](https://img.shields.io/badge/Engine-Rust_&_WebAssembly-orange?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg?style=for-the-badge)](LICENSE)

> **🎮 Play Instantly in Your Browser:**  
> 👉 [**https://akillesh2006.github.io/twin-conduit/**](https://akillesh2006.github.io/twin-conduit/) 👈  
> *(No installation needed — runs 100% in-browser powered by Rust + WebAssembly!)*

---

A fast-paced, two-player, one-keyboard cooperative terminal bullet-hell game built in Rust and playable natively in the terminal or on the web via WebAssembly.

Two operators defend together against relentless waves of monsters. Both carry automatic cannons that acquire and fire at incoming threats, leaving players free to focus entirely on **movement, positioning, dodging bullet patterns, reviving each other, collecting powerups, and surviving**.

* **Two Players, One Keyboard**: Player 1 moves with `W A S D`, Player 2 moves with the `Arrow keys`. Both move simultaneously.
* **Cooperative Defense**: Auto-firing cannons acquire the closest monsters. If a partner falls (`DOWN`), stand beside them to re-link and revive them!
* **A Boss Every 5th Wave**:
  * **Wave 5**: **THE WARDEN** (`◈`) - 45 HP. Sweeping ring patterns with telegraphed safe dodge lanes.
  * **Wave 10**: **HYDRA** (`▣`) - 100 HP. Aimed multi-spread volleys and swarm reinforcements.
  * **Wave 15**: **OVERMIND** (`◉`) - 180 HP. Massive barrage patterns and ultimate encounter.
* **Weapon Evolution System**: Collect Evolution Crystals (`▲`) to upgrade your cannons:
  * Tier 0: Single Auto-Cannon
  * Tier 1: **Dual Cannons**
  * Tier 2: **Tri-Spread Cannons**
  * Tier 3: **Hyper Plasma Cannons**!
* **Combat Drones / Companion Pets**: Collect Pet Cores (`★`) to deploy companion drones that orbit each operator! Pets auto-fire at monsters, level up to **Sentinel** (actively zaps incoming enemy bullets!) and **Apex Overdrive** (rapid twin-plasma firepower).
* **Monster Drops & Tougher Waves**: Normal waves feature larger hordes with Weavers, Spitters, Swarms, and Bulwarks. Defeated monsters drop powerups to fuel your progression.
* **Dynamic Pressure (DDA)**: The director monitors dodging habits and dynamically adapts bullet velocity and cadence.
* **Zero External Dependencies**: Pure Rust and WebAssembly (`wasm32-unknown-unknown`). Direct Win32 console integration locally, and xterm.js ANSI rendering in the browser.

---

## 🚀 How to Play

### 🌐 Play in Browser (GitHub Pages)

Open the live web deployment directly:
👉 [**https://akillesh2006.github.io/twin-conduit/**](https://akillesh2006.github.io/twin-conduit/)

---

### 💻 Play Locally in Terminal

#### Option 1: Double-Click (Windows)
Double-click `play.bat` in the repository folder.

#### Option 2: PowerShell
```powershell
# From the project folder:
.\run.ps1

# Run the 25-check automated verification suite:
.\run.ps1 --test
```

#### Option 3: Cargo (Native Rust)
```sh
# Build and run with Cargo:
cargo run --release

# Run automated tests:
cargo run --release -- --test
```

> [!NOTE]
> When running in a terminal, a window size of **72x26** cells or larger (such as Windows Terminal) is recommended.

---

## 🎮 Controls

| Action | Player 1 (Cyan) | Player 2 (Magenta) |
|---|---|---|
| **Move** | `W` `A` `S` `D` | `Arrow keys` (Up / Down / Left / Right) |
| **Pause / Resume** | `P` | `P` |
| **Restart Run** | `R` | `R` |
| **Start / Retry** | `Enter` or `Space` | `Enter` or `Space` |
| **Quit / Title** | `Esc` | `Esc` |

*Both players can move and dodge simultaneously at 60 FPS.*

---

## ⚡ Powerups, Evolution & Combat Pets

| Glyph | Powerup | Effect |
|---|---|---|
| `▲` | **Evolution Crystal** | Evolves cannons: **Dual Cannons** (Tier 1) -> **Tri-Spread** (Tier 2) -> **Hyper Plasma** (Tier 3) |
| `★` | **Combat Drone Pet** | Deploys an orbital drone that attacks enemies. Evolves to **Sentinel** (zaps enemy bullets!) and **Apex** (twin plasma) |
| `✚` | **Heal** | Restores +1 heart to the most wounded operator (up to 5 HP max) |
| `◇` | **Shield** | Barrier absorbing 1 instance of damage |
| `✦` | **Rapid Fire** | Halves cannon cooldown for rapid-fire devastation |

> [!TIP]
> Both normal monsters and bosses drop powerups when defeated!

---

## 👾 Threats & Monsters

| Glyph | Name | Behavior |
|---|---|---|
| `×` | Grunt | Pursues players directly |
| `▼` | Spitter | Keeps distance and fires aimed pellets |
| `✹` | Weaver | Orbits operators and fires 3-way dart spreads; vents ring on death |
| `■` | Bulwark | Armored (7 HP), fires heavy damaging shells (50% drop rate) |
| `∙` | Swarm | Fast-moving chasers in large packs |
| `◈` | **THE WARDEN** | **Wave 5 Boss** (45 HP). Sweeping ring patterns with telegraphed dodge gaps |
| `▣` | **HYDRA** | **Wave 10 Boss** (100 HP). Multi-spread volleys and swarm seeds |
| `◉` | **OVERMIND** | **Wave 15 Boss** (180 HP). Multi-vector spiral barrages |

---

## 🛠️ Deploying to GitHub Pages

To activate the browser version on your own fork / repository:
1. Go to your repository on GitHub: `https://github.com/<your-username>/twin-conduit`
2. Click **Settings** (top tab) -> **Pages** (left sidebar under *Code and automation*).
3. Under **Build and deployment**:
   * **Source**: Select `Deploy from a branch`
   * **Branch**: Select `main` and folder `/ (root)`
   * Click **Save**.
4. In ~1 minute, your game will be live at `https://<your-username>.github.io/twin-conduit/`!
5. Add the link to your repository's **About** section on the main page (click the gear icon ⚙ beside "About" and paste the URL into the **Website** field).

---

## 📁 Project Structure

```
├── index.html       WebAssembly browser frontend (xterm.js + CRT arcade styling)
├── harness.wasm     Compiled WebAssembly game engine
├── play.bat         One-click launcher for Windows
├── run.ps1          PowerShell build and launch script
├── Cargo.toml       Rust package configuration
├── src/
│   ├── main.rs      Entry point, frame loop, terminal management, verification suite
│   ├── game.rs      Simulation core: 2 players, auto-cannons, monsters, bosses, pets, evolution
│   ├── sys.rs       Zero-dependency platform layer (Win32 Console API & POSIX)
│   ├── app.rs       Game state machine (Title, Run, Pause, Over)
│   ├── canvas.rs    Double-buffered ANSI console renderer with diff-based flushing
│   ├── ui.rs        HUD layout, boss health bar, player status, evolution/pet indicators
│   ├── scene.rs     Arena, pet orbits, and entity rendering
│   ├── input.rs     Simultaneous input tracking for WASD and Arrow keys
│   ├── keys.rs      Key definitions and event parsing
│   ├── fx.rs        Particle effects, screen shake, and floating combat text
│   ├── core.rs      Vector arithmetic, field boundaries, and coordinates
│   └── rng.rs       Deterministic fast PRNG
└── tools/
    ├── harness.rs   WASM bridge exposing tick, key_event, and ANSI rendering
    └── harness.wasm Precompiled WebAssembly module
```

---

## 📜 License

MIT License - feel free to build, modify, and distribute!

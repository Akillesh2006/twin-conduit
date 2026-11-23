# OVERDRIVE

A two-player, one-keyboard terminal bullet-hell game in Rust.

Two operators defend together against relentless waves of monsters. Both carry automatic cannons that acquire and fire at incoming threats, leaving players free to focus entirely on **movement, positioning, dodging bullet patterns, reviving each other, collecting powerups, and surviving**.

* **Two Players, One Keyboard**: Player 1 moves with `W A S D`, Player 2 moves with the `Arrow keys`. Both move simultaneously.
* **Cooperative Defense**: Auto-firing cannons acquire the closest monsters. If a partner falls (`DOWN`), stand beside them to re-link and revive them!
* **A Boss Every 5th Wave**:
  * **Wave 5**: **THE WARDEN** (`◈`) - 45 HP. Sweeping ring patterns with telegraphed safe dodge lanes.
  * **Wave 10**: **HYDRA** (`▣`) - 100 HP. Aimed multi-spread volleys and swarm reinforcements.
  * **Wave 15**: **OVERMIND** (`◉`) - 180 HP. Massive barrage patterns and ultimate encounter.
* **Weapon Evolution System**: Collect Evolution Crystals (`▲`) to upgrade your cannons from Single -> **Dual Cannons** -> **Tri-Spread Cannons** -> **Hyper Plasma Cannons**!
* **Combat Drones / Pets**: Collect Pet Cores (`★`) to deploy companion drones that orbit each operator! Pets auto-fire at monsters, level up to **Sentinel** (actively zaps incoming enemy bullets!) and **Apex Overdrive** (rapid twin-plasma firepower).
* **Monster Drops & Tougher Waves**: Normal waves feature larger hordes with Weavers, Spitters, Swarms, and Bulwarks. Defeated monsters drop powerups to fuel your progression.
* **Dynamic Pressure (DDA)**: The director monitors dodging habits and dynamically adapts bullet velocity and cadence.
* **Zero Dependencies**: Builds with pure `rustc` / Cargo. Direct Win32 console (or POSIX termios) integration.

---

## Quick Start / How to Run

### Option 1: Using PowerShell Script (Recommended on Windows)

```powershell
# Play the game (in a terminal 72x26 or larger)
.\run.ps1

# Run the automated verification suite
.\run.ps1 --test
```

### Option 2: Standard Cargo

```sh
# Build and run the game
cargo run --release

# Run automated verification tests
cargo run --release -- --test
```

### Option 3: Direct Native Binary

After compiling (`cargo build --release` or `.\run.ps1`):

```powershell
.\target\release\overdrive.exe
.\target\release\overdrive.exe --test
```

> [!NOTE]
> OVERDRIVE requires a real terminal window (such as Windows Terminal or conhost). Minimum size is **72x26** cells.

---

## Controls

| Action | Player 1 (Blue) | Player 2 (Red) |
|---|---|---|
| Move | `W` `A` `S` `D` | `Arrow keys` (Up / Down / Left / Right) |
| Pause / Resume | `P` | `P` |
| Restart Run | `R` | `R` |
| Start / Retry | `Enter` or `Space` | `Enter` or `Space` |
| Quit / Back | `Esc` | `Esc` |

Both players can hold keys and move at the exact same time.

---

## Powerups, Evolution & Combat Pets

| Glyph | Powerup | Effect |
|---|---|---|
| `▲` | **Evolution Crystal** | Evolves cannons: **Dual Cannons** (Tier 1) -> **Tri-Spread** (Tier 2) -> **Hyper Plasma** (Tier 3) |
| `★` | **Combat Drone Pet** | Deploys an orbital drone that attacks enemies. Evolves to **Sentinel** (zaps incoming bullets!) and **Apex** (double plasma fire) |
| `✚` | **Heal** | Restores +1 heart to the most wounded operator (up to 5 HP max) |
| `◇` | **Shield** | Barrier absorbing 1 instance of damage |
| `✦` | **Rapid Fire** | Halves cannon cooldown for rapid-fire devastation |

> [!TIP]
> Both normal monsters and bosses have a chance to drop powerup items upon defeat!

---

## Threats & Monsters

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

## Project Structure

```
src/
  main.rs     Entry point, frame loop, terminal management, verification suite
  game.rs     Simulation core: 2 players, auto-cannons, monsters, bosses, pets, evolution
  sys.rs      Zero-dependency platform layer (Win32 Console API & POSIX)
  app.rs      Game state machine (Title, Run, Pause, Over)
  canvas.rs   Double-buffered ANSI console renderer with diff-based flushing
  ui.rs       HUD layout, boss health bar, player status, evolution/pet indicators
  scene.rs    Arena, pet orbits, and entity rendering
  input.rs    Simultaneous input tracking for WASD and Arrow keys
  keys.rs     Key definitions and event parsing
  fx.rs       Particle effects, screen shake, and floating combat text
  core.rs     Vector arithmetic, field boundaries, and coordinates
  rng.rs      Deterministic fast PRNG
run.ps1       Build and launch helper script
```

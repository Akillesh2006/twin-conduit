# RESEARCH — Local 2P Terminal Mini-Game

Compiled 2026-11-19. Sources: docs.rs/crossterm 0.29.0, crossterm CHANGELOG, Devolver/Le Cartel press material, Wikipedia (Heave Ho, Pico Park), TECOPARK IDC2023 + CEDEC2023 talks, Ghostty/Wintty synchronized-output docs, microsoft/terminal #8331.

---

## 1. Simultaneous 2P Couch-Co-op Mechanics (shared object / resource / tether)

**PICO PARK — "Go Together" rope world** (TECOPARK, 2021; PICO PARK 2, Aug 2024)
Every player is physically roped to every other player. Because the rope is stretchy and rigid, any one player's movement applies force to all others, so the group moves as a single damped mass. There is no "your turn": progress comes from two players agreeing on a direction a fraction of a second apart, and a single player over-extending drags the pair off a ledge.

**PICO PARK — shared gauge / "move and stop" stages** (TECOPARK; taxonomy from Miyake's IDC2023 talk)
A single global gauge drains whenever *any* player is moving, and the stage only refills it when *both* stand still. This is "sharing play" — one HP/energy pool owned by the team, so a fast player is actively harming a careful one. The failure is collective, which the designer explicitly prefers: a solo mistake reads as blame, a shared mistake reads as comedy.

**Heave Ho** (Le Cartel, 2019; prototyped at a game jam)
Each player is a head with two stretchy arms, gripping with L/R. Players grab *each other's hands* and form a dangling chain, converting one player's anchor into the other's reach. The optional coin is the sharpest idea: carrying it consumes one of your two hands, so co-op pressure rises exactly when the objective is present.

**Bread & Fred** (SandCastles Studio, 2022; Switch 2024)
Two penguins are joined by a rope. The lighter player can anchor on a ledge so the other can pendulum across a gap, and a mistimed jump yanks the anchor off. Momentum is shared and conserved — the rope transmits both position and velocity, so the pair is one under-actuated body with two inputs.

**Phogs!** (Bit Loom)
Two players each control one head of a single two-headed dog with one shared stretchy midsection. One head can bite and anchor an object while the other head reaches somewhere unreachable alone. The body length is a literal shared resource: stretching to help your partner puts you in a worse position.

**Biped** (NEXT Studios)
Each analog stick drives one leg; each player's stick drives their own robot. Walking requires the two legs to alternate in a rhythm, so traversal itself is the cooperation. Two players standing next to each other naturally fall into an unintentional marching cadence.

**Tentacle Tango** (recent shareable-keyboard 2P title, Windows/Switch)
Two players hold opposite ends of elastic tentacles and swing each other across gaps while evading a chef. The tentacle is simultaneously the tool, the tether and the failure state — releasing at the wrong phase is the entire skill.

**Design taxonomy worth stealing** (Miyake, IDC2023): formation play (stack into shapes), role play (asymmetric jobs), multi-person drive (object too heavy for one), sharing play (common HP/energy), aggregation play (sum of individual results judged together), chain play (A's output is B's input), timing play (counted "3-2-1-now"), jump-rope play (any one mistake resets everyone).

---

## 2. Terminal / TUI Rendering Techniques (Windows Terminal)

**Sub-cell pixels via half blocks.** Render each cell as `▀` (U+2580) with `SetForegroundColor` = the top pixel and `SetBackgroundColor` = the bottom pixel. That doubles vertical resolution (80×24 → 80×48 logical pixels) with no extra rows. Include `▄` (lower half) and `█` for solid fills; `▘▝▖▗▚▞` quadrant blocks give 4 sub-pixels when you need finer edges. Avoid braille/emoji: they break 1-cell width assumptions on Windows Terminal.

**Aspect ratio.** Terminal cells are ~1:2 (h:w). Two half-block pixels per cell are therefore roughly square, so a 2× vertical scale is the only distortion-free mapping. Never scale X and Y by the same integer.

**ANSI 256 vs truecolor.** `ESC[38;5;Nm` / `ESC[48;5;Nm` for 256-color; `ESC[38;2;r;g;bm` for truecolor. Windows Terminal handles truecolor fine, but a 256-color indexed palette is cheaper per cell and forces a coherent art direction. Honour `NO_COLOR` (crossterm 0.27+ does this for its colour commands).

**Double buffering by cell diff.** Keep `prev: Vec<Cell>` of `{ch, fg, bg}` and build `next: Vec<Cell>`. Compare, group changed cells into contiguous horizontal runs per row, then emit one `MoveTo(x,y)` + one styled string per run. A 120×40 frame of half blocks is ~4800 cells; full redraws are ~90 KB of escape codes per frame and will visibly lag, while a diff is typically a few hundred bytes. Batch all writes into one `String`/`Vec<u8>` and `flush()` exactly once per frame.

**Synchronized output (the single biggest polish win).** Wrap each frame in `ESC[?2026h` … `ESC[?2026l` so the emulator swaps the frame atomically. Ghostty/Wintty/Windows Terminal (v1.23+) all support it; without it, fast emulators tear and flicker even when your diffing is correct. crossterm exposes this as `SynchronizedUpdate` / `synchronized_update!` (0.26.1+).

**Never `Clear(All)` per frame.** It causes flicker, destroys the renderer's ability to diff, and on some emulators scrolls the alt screen. Clear once on entry; afterwards paint every cell of the diff and pad short rows with `ESC[K` (erase to end of line).

**Frame pacing.** Fixed-step accumulator: `while acc >= 1/60 { sim.step(); acc -= 1/60 }`, rendering once per loop. Use `event::poll(Duration::from_millis(1..4))` as your sleep — it returns early on input, so you get responsive keys without a busy loop, and you avoid `thread::sleep` drift (Windows rounds sleeps up to ~15 ms). Clamp `acc` to avoid a spiral of death after a stall.

**Cursor + screen hygiene.** `cursor::Hide` on entry, `Show` on exit; `EnterAlternateScreen` / `LeaveAlternateScreen` to preserve scrollback; `DisableLineWrap` so a wide row cannot wrap and shear the whole frame; `SetTitle` for polish. Re-query `terminal::size()` on `Event::Resize` and rebuild both buffers.

**Key press / release / hold on Windows.** Raw mode is mandatory (`enable_raw_mode`) or you get line buffering, echo, and Ctrl-C delivered as a signal instead of a key. With crossterm ≥0.26 Windows *does* emit `KeyEventKind::Release` from the console input record, but treat it as best-effort: alt-tab, focus loss, and some ConPTY paths swallow the key-up, leaving a key logically stuck. Robust pattern: `Press` sets held, `Release` clears held, and on `Event::FocusLost` clear all held keys. For continuous movement prefer "held set" semantics over edge events, and use `KeyEventKind::Repeat` only as confirmation of the OS auto-repeat, not as the movement driver.

**Known pitfalls.**
- **Panic-safe teardown is non-negotiable.** Install `std::panic::set_hook` that pops enhancement flags, disables raw mode, leaves the alt screen and shows the cursor *before* the panic message prints. Also use a `Drop` guard. Otherwise a panic leaves the user's shell in raw mode with no echo.
- Enable VT processing: crossterm does this for you on Windows 10+, but if stdout is redirected the ANSI path silently fails — gate the game on `std::io::stdout().is_terminal()`.
- Lock stdout **once** (`let mut out = stdout().lock()`) and hold it for the whole frame; per-command `stdout()` re-locks and murders throughput.
- Cheap membrane keyboards ghost/block above 2–4 simultaneous keys. Design each player's control set to ~2 held keys and pick keys in different matrix regions (e.g. `WASD` vs `IJKL`, not `ASDF` vs `GHJK`).
- Keyboard `release` for modifier combinations is unreliable; avoid chords as core verbs.

---

## 3. `crossterm` — Version and Exact API (docs.rs, verified 2026-11-19)

**Latest stable: `0.29.0`** (published 2026-11-08). Pin `crossterm = "0.29"`. `master`/unreleased 0.30 is breaking: `Event` loses `Copy`, `Event`/`KeyCode`/`KeyEvent`/`KeyModifiers` become `#[non_exhaustive]`, `IsTty` removed, MSRV 1.85.

```rust
// terminal
crossterm::terminal::enable_raw_mode()  -> io::Result<()>
crossterm::terminal::disable_raw_mode() -> io::Result<()>
crossterm::terminal::EnterAlternateScreen   // unit struct, use via execute!()
crossterm::terminal::LeaveAlternateScreen   // unit struct
crossterm::terminal::size() -> io::Result<(u16, u16)>   // (columns, rows)

// cursor
crossterm::cursor::Hide     // unit struct
crossterm::cursor::Show     // unit struct
crossterm::cursor::MoveTo(x: u16, y: u16)   // tuple struct, 0-based

// style
crossterm::style::SetForegroundColor(color: Color)   // tuple struct
crossterm::style::ResetColor                         // unit struct
crossterm::style::Color::{Reset, Black, DarkGrey, White,
    AnsiValue(u8), Rgb { r: u8, g: u8, b: u8 }, /* + named variants */}

// events
crossterm::event::poll(timeout: std::time::Duration) -> io::Result<bool>
crossterm::event::read() -> io::Result<Event>

pub enum Event { Key(KeyEvent), Mouse(MouseEvent), Resize(u16, u16),
                 FocusGained, FocusLost, Paste(String) }

pub struct KeyEvent { pub code: KeyCode, pub modifiers: KeyModifiers,
                      pub kind: KeyEventKind, pub state: KeyEventState }

pub enum KeyCode { Char(char), Enter, Esc, Tab, BackTab, Backspace, Delete,
                   Insert, Left, Right, Up, Down, Home, End, PageUp, PageDown,
                   F(u8), Null, CapsLock, ScrollLock, NumLock, PrintScreen,
                   Pause, Menu, KeypadBegin, Media(..), Modifier(..) }

pub enum KeyEventKind { Press, Repeat, Release }
// const fns: is_press(), is_repeat(), is_release()

// KeyModifiers is a bitflags type: SHIFT, CONTROL, ALT, SUPER, HYPER, META, NONE
if key.modifiers.contains(KeyModifiers::CONTROL) { .. }

// convenience macros / traits
queue!(out, MoveTo(0,0), SetForegroundColor(Color::AnsiValue(46)))?;
execute!(out, EnterAlternateScreen, Hide)?;
QueueableCommand / ExecutableCommand / SynchronizedUpdate + synchronized_update! { .. }
crossterm::event::{PushKeyboardEnhancementFlags, PopKeyboardEnhancementFlags}
crossterm::event::query_keyboard_enhancement_flags()   // 0.29
```

**`MoveTo` is 0-based; relative moves (`MoveUp`/`MoveDown`/…) are 1-based** (CHANGELOG 0.24.0). `terminal::size()` returns an error rather than `(0,0)` when unknown (0.24.0). All fallible APIs return `std::io::Result` since 0.27.

**`KeyEventKind` history.** Introduced in **0.25.0** alongside the kitty keyboard protocol (`PushKeyboardEnhancementFlags`), which is what made `Press`/`Repeat`/`Release` distinguishable on Unix. **Windows key-release events were added in 0.26.0** (CHANGELOG: "Add key release event for windows. (#745)"). Therefore:
- On Windows with crossterm **≥ 0.26**, `KeyEventKind::Release` *is* emitted, sourced from the Win32 `KEY_EVENT` `bKeyDown == FALSE` records — no enhancement flags required for basic press/release.
- On crossterm **≤ 0.25**, Windows emitted only `Press`; any code doing "if kind == Press" is portable, any code gating on `Release` was silently broken.
- Still do not *depend* on `Release`: focus loss, alt-tab, ConPTY, and some remote/SSH paths drop key-up events. Always clear held keys on `Event::FocusLost`.
- `Repeat` on Windows is OS auto-repeat, not crossterm synthesis; its rate is user-configurable, so never derive movement speed from it.

---

## DESIGN RECOMMENDATION

Three candidates, ranked for a terminal game where both players sit at one keyboard and act simultaneously.

### 1. COUPLED — shared power tether  ★ recommended
Two drones are joined by a visible energy cable rendered as `─ │ ╱ ╲` glyphs. The cable is a *shared object*: it carries power, so the pair's total energy income is proportional to how much slack the cable has (closer = more power, less reach). Stretching the cable past its limit drains the shared cell and emits a warning shimmer; letting it go fully slack kills income. Hazards sever the cable, and re-docking is a two-player action. Why it wins: it fuses the three proven patterns — tether (Bread & Fred / Heave Ho), shared resource (PICO PARK sharing play), and continuous position coupling — into one readable visual, and it works on a coarse character grid because only *distance*, not rope physics, matters. It also produces the readability the terminal needs: one glyph line between two sprites says everything about state. Keyboard load is 2 held keys each (`WASD` / arrow cluster), which survives cheap-keyboard rollover limits.

### 2. BEAM RELAY — emitter and reflector
Player A emits a beam from a fixed mount they rotate; player B drags a mirror/prism to route it. The beam is the shared object and the only light source, so the map is drawn only where the beam touches — an information mechanic and a puzzle in one. Both act every frame with no turn structure. Slightly lower rank: it needs a raycast per frame (cheap, but grid-exact), and the two roles are asymmetric, which is fine for a 3-minute jam game but less funny than symmetric failure.

### 3. OVERHEAT REACTOR — shared throughput gauge
Both players shovel fuel into one reactor; combined input rate fills a heat bar, and the bar must be kept inside a narrow band. Too slow and the timer kills you, too fast and it melts down. This is PICO PARK's "move and stop" with a continuous dial instead of a binary one, and it is the cheapest of the three to implement. It ranks last because the optimal strategy collapses into "alternate on a metronome", which is cooperative but not *conversational* — there is little for the players to say to each other beyond "slower".

**Recommendation:** build **COUPLED**. It maximises the ratio of emergent comedy to implementation cost, needs no sub-cell physics, degrades gracefully on a coarse grid, and gives the renderer one continuous, high-contrast element (the cable) to show off half-block shading and per-frame diffs.

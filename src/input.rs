//! Simultaneous two-player keyboard state with tap-precision & DAS/ARR hold pacing.
//!
//! Player one drives `WASD`, player two drives the arrow cluster, and both act at
//! the same time. Key design properties:
//!
//! * **Exact 1-cell taps:** Every fresh key press queues one immediate step (`tap_pending`).
//!   Ultra-short taps (`Release` before the next tick) still execute that single step
//!   via `released`, and never overshoot.
//! * **Delayed Auto Shift (DAS) + Auto Repeat Rate (ARR):** Holding a key waits `DAS_MS`
//!   before repeating steps at `ARR_MS`, giving players controllable coherent pacing.
//! * **Release-capable vs TTY backends:** Windows console & browser (`has_release = true`)
//!   track explicit key-up events. POSIX/Node TTYs (`has_release = false`) expire stale
//!   strokes at `HOLD_MS` and only repeat when OS repeat bytes arrive.

use crate::core::Dir;
use crate::keys::{self, Key, KeyEv, Mods};

/// Timeout before an un-released keystroke expires on terminals without key-up.
pub const HOLD_MS: u64 = 160;
/// Grace delay between the initial 1-cell tap and continuous hold movement.
pub const DAS_MS: u64 = 200;
/// Step cadence while continuously holding a direction (~8.7 cells/sec).
pub const ARR_MS: u64 = 115;

#[derive(Default, Clone, Copy, Debug)]
pub struct Flags {
    pub quit: bool,
    pub confirm: bool,
    pub pause: bool,
    pub restart: bool,
    /// Horizontal menu navigation (-1 left, +1 right) triggered on press.
    pub nav: i32,
    /// Direct choice key (1..=4) triggered on press.
    pub choice: Option<u8>,
}

#[derive(Clone, Copy, Debug)]
struct Held {
    dir: Dir,
    since: u64,
    tap_pending: bool,
    released: bool,
    from_repeat: bool,
    next_repeat_at: u64,
}

#[derive(Debug)]
pub struct Input {
    /// Newest-first per player, so releasing key B falls back to key A.
    held: [Vec<Held>; 2],
    flags: Flags,
    now: u64,
    focus: bool,
    has_release: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl Input {
    pub fn new() -> Self {
        Self {
            held: [Vec::with_capacity(4), Vec::with_capacity(4)],
            flags: Flags::default(),
            now: 0,
            focus: true,
            has_release: cfg!(any(windows, target_arch = "wasm32")),
        }
    }

    pub fn set_has_release(&mut self, has_release: bool) {
        self.has_release = has_release;
    }

    /// Advance the internal clock and drop stale holds.
    pub fn expire(&mut self, now_ms: u64) {
        self.now = now_ms;
        // Release-capable backends still keep a long safety cap in case focus drops an event,
        // while keeping unconsumed taps alive until the next tick processes them.
        let cap = if self.has_release { 4000 } else { HOLD_MS };
        for p in 0..2 {
            self.held[p].retain(|h| h.tap_pending || now_ms.saturating_sub(h.since) <= cap);
        }
    }

    /// Drop every held direction (focus loss, pause, reset).
    pub fn release_all(&mut self) {
        self.held[0].clear();
        self.held[1].clear();
    }

    pub fn feed(&mut self, ev: KeyEv) {
        match ev {
            KeyEv::FocusLost => {
                self.focus = false;
                self.release_all();
            }
            KeyEv::FocusGained => {
                self.focus = true;
                self.release_all();
            }
            KeyEv::Resize(..) => {}
            KeyEv::Release(key) => self.release(key),
            KeyEv::Repeat(key, _) => self.hold(key, true),
            KeyEv::Press(key, mods) => {
                self.hold(key, false);
                self.edge(key, mods);
            }
        }
    }

    fn hold(&mut self, key: Key, is_repeat: bool) {
        let Some((p, dir)) = keys::movement(key) else {
            return;
        };
        let list = &mut self.held[p];
        if let Some(pos) = list.iter().position(|h| h.dir == dir) {
            let mut h = list.remove(pos);
            h.since = self.now;
            if h.released && !is_repeat {
                // Re-tapped while previous tap hadn't been flushed yet.
                h.released = false;
                h.tap_pending = true;
            } else {
                h.from_repeat = true;
            }
            list.insert(0, h);
        } else if !is_repeat {
            list.insert(
                0,
                Held {
                    dir,
                    since: self.now,
                    tap_pending: true,
                    released: false,
                    from_repeat: false,
                    next_repeat_at: self.now + DAS_MS,
                },
            );
            if list.len() > 4 {
                list.truncate(4);
            }
        }
    }

    fn release(&mut self, key: Key) {
        let Some((p, dir)) = keys::movement(key) else {
            return;
        };
        for h in &mut self.held[p] {
            if h.dir == dir && h.tap_pending {
                // Keep ultra-short taps buffered so they still execute 1 cell on the next tick.
                h.released = true;
                return;
            }
        }
        self.held[p].retain(|h| h.dir != dir);
    }

    fn edge(&mut self, key: Key, mods: Mods) {
        let ctrl_only = mods.ctrl && !mods.alt;
        match key {
            Key::Esc => self.flags.quit = true,
            Key::C if ctrl_only => self.flags.quit = true,
            Key::Q if !mods.ctrl && !mods.alt => self.flags.quit = true,
            Key::R if !mods.ctrl && !mods.alt => self.flags.restart = true,
            Key::P if !mods.ctrl && !mods.alt => self.flags.pause = true,
            Key::Enter | Key::Space => self.flags.confirm = true,
            Key::A | Key::Left => self.flags.nav -= 1,
            Key::D | Key::Right => self.flags.nav += 1,
            Key::Num1 => self.flags.choice = Some(1),
            Key::Num2 => self.flags.choice = Some(2),
            Key::Num3 => self.flags.choice = Some(3),
            Key::Num4 => self.flags.choice = Some(4),
            _ => {}
        }
    }

    /// Raw currently held direction per player (peek without consuming DAS/tap state).
    pub fn dirs(&self) -> [Option<Dir>; 2] {
        [
            self.held[0].first().map(|h| h.dir),
            self.held[1].first().map(|h| h.dir),
        ]
    }

    /// Consume one simulation tick of movement with 1-cell tap precision & DAS/ARR hold cadence.
    pub fn step_dirs(&mut self) -> [Option<Dir>; 2] {
        let mut out = [None, None];
        for p in 0..2 {
            let list = &mut self.held[p];
            let Some(h) = list.first_mut() else {
                continue;
            };
            if h.tap_pending {
                h.tap_pending = false;
                let dir = h.dir;
                let done = h.released;
                if done {
                    list.remove(0);
                }
                out[p] = Some(dir);
            } else if self.has_release {
                if self.now >= h.next_repeat_at {
                    h.next_repeat_at = self.now + ARR_MS;
                    out[p] = Some(h.dir);
                }
            } else if h.from_repeat && self.now >= h.next_repeat_at {
                h.next_repeat_at = self.now + ARR_MS;
                out[p] = Some(h.dir);
            }
        }
        out
    }

    pub fn take_flags(&mut self) -> Flags {
        std::mem::take(&mut self.flags)
    }

    pub fn any_held(&self) -> bool {
        !self.held[0].is_empty() || !self.held[1].is_empty()
    }

    pub fn focused(&self) -> bool {
        self.focus
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(i: &mut Input, k: Key) {
        i.feed(KeyEv::press(k));
    }

    #[test]
    fn both_players_move_at_once() {
        let mut i = Input::new();
        press(&mut i, Key::W);
        press(&mut i, Key::Right);
        assert_eq!(i.dirs(), [Some(Dir::Up), Some(Dir::Right)]);
    }

    #[test]
    fn newest_direction_wins_and_falls_back() {
        let mut i = Input::new();
        press(&mut i, Key::A);
        press(&mut i, Key::W);
        assert_eq!(i.dirs()[0], Some(Dir::Up));
        i.expire(HOLD_MS);
        i.step_dirs();
        i.feed(KeyEv::release(Key::W));
        assert_eq!(i.dirs()[0], Some(Dir::Left));
    }

    #[test]
    fn release_clears() {
        let mut i = Input::new();
        press(&mut i, Key::Left);
        i.step_dirs();
        i.feed(KeyEv::release(Key::Left));
        assert_eq!(i.dirs()[1], None);
    }

    #[test]
    fn stale_holds_expire() {
        let mut i = Input::new();
        i.set_has_release(false);
        press(&mut i, Key::D);
        i.step_dirs();
        i.expire(HOLD_MS + 1);
        assert_eq!(i.dirs()[0], None);
    }

    #[test]
    fn repeat_refreshes_hold() {
        let mut i = Input::new();
        i.set_has_release(false);
        press(&mut i, Key::D);
        i.expire(HOLD_MS - 50);
        i.feed(KeyEv::Repeat(Key::D, Mods::plain()));
        i.expire(HOLD_MS + 100);
        assert_eq!(i.dirs()[0], Some(Dir::Right));
    }

    #[test]
    fn focus_loss_clears_everything() {
        let mut i = Input::new();
        press(&mut i, Key::W);
        press(&mut i, Key::Up);
        i.feed(KeyEv::FocusLost);
        assert_eq!(i.dirs(), [None, None]);
    }

    #[test]
    fn flags_are_edge_triggered_once() {
        let mut i = Input::new();
        press(&mut i, Key::Enter);
        press(&mut i, Key::Enter);
        assert!(i.take_flags().confirm);
        assert!(!i.take_flags().confirm);
    }

    #[test]
    fn ctrl_c_quits_but_plain_c_does_not() {
        let mut i = Input::new();
        press(&mut i, Key::C);
        assert!(!i.take_flags().quit);
        i.feed(KeyEv::Press(Key::C, Mods { ctrl: true, ..Mods::default() }));
        assert!(i.take_flags().quit);
    }
}

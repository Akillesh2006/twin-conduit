//! Simultaneous two-player keyboard state.
//!
//! Player one drives `WASD`, player two drives the arrow cluster, and both act at
//! the same time. The design constraints that shaped this file:
//!
//! * Terminals that emit key *release* (Windows console does) get exact
//!   press/release tracking.
//! * Terminals that only emit presses (most Unix terminals) never send a release,
//!   so every held direction has a timestamp and silently expires. A press that
//!   is still being reported refreshes the timestamp, so real holds never expire.
//! * Losing focus must clear everything: alt-tab swallows the key-up that would
//!   otherwise leave a player permanently running into a wall.

use crate::core::Dir;
use crate::keys::{self, Key, KeyEv, Mods};

/// A direction counts as held for this long after its last event if the terminal
/// never tells us the key came back up.
pub const HOLD_MS: u64 = 400;

#[derive(Default, Clone, Copy, Debug)]
pub struct Flags {
    pub quit: bool,
    pub confirm: bool,
    pub pause: bool,
    pub restart: bool,
}

#[derive(Clone, Copy, Debug)]
struct Held {
    dir: Dir,
    since: u64,
}

#[derive(Debug)]
pub struct Input {
    /// Newest-first per player, so releasing key B falls back to key A.
    held: [Vec<Held>; 2],
    flags: Flags,
    now: u64,
    focus: bool,
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
        }
    }

    /// Advance the internal clock and drop stale holds.
    pub fn expire(&mut self, now_ms: u64) {
        self.now = now_ms;
        for p in 0..2 {
            self.held[p].retain(|h| now_ms.saturating_sub(h.since) <= HOLD_MS);
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
            KeyEv::Repeat(key, _) => self.hold(key),
            KeyEv::Press(key, mods) => {
                self.hold(key);
                self.edge(key, mods);
            }
        }
    }

    fn hold(&mut self, key: Key) {
        if let Some((p, dir)) = keys::movement(key) {
            let list = &mut self.held[p];
            if let Some(slot) = list.iter_mut().find(|h| h.dir == dir) {
                slot.since = self.now;
                let h = *slot;
                list.retain(|x| x.dir != dir);
                list.insert(0, h);
            } else {
                list.insert(0, Held {
                    dir,
                    since: self.now,
                });
                if list.len() > 4 {
                    list.truncate(4);
                }
            }
        }
    }

    fn release(&mut self, key: Key) {
        if let Some((p, dir)) = keys::movement(key) {
            self.held[p].retain(|h| h.dir != dir);
        }
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
            _ => {}
        }
    }

    pub fn dirs(&self) -> [Option<Dir>; 2] {
        [
            self.held[0].first().map(|h| h.dir),
            self.held[1].first().map(|h| h.dir),
        ]
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
        i.feed(KeyEv::release(Key::W));
        assert_eq!(i.dirs()[0], Some(Dir::Left));
    }

    #[test]
    fn release_clears() {
        let mut i = Input::new();
        press(&mut i, Key::Left);
        i.feed(KeyEv::release(Key::Left));
        assert_eq!(i.dirs()[1], None);
    }

    #[test]
    fn stale_holds_expire() {
        let mut i = Input::new();
        press(&mut i, Key::D);
        i.expire(HOLD_MS + 1);
        assert_eq!(i.dirs()[0], None);
    }

    #[test]
    fn repeat_refreshes_hold() {
        let mut i = Input::new();
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

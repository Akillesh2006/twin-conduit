//! Key identifiers and platform-neutral key events.
//!
//! The platform layer (`sys`) translates raw terminal records into these, so the
//! input state machine can be exercised headlessly (see `tests/` and the wasm
//! harness) without a real console.

use crate::core::Dir;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    W,
    A,
    S,
    D,
    Enter,
    Space,
    Esc,
    P,
    R,
    Q,
    C,
    Tab,
    Num1,
    Num2,
    Num3,
    Num4,
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Mods {
    pub fn plain() -> Self {
        Mods::default()
    }
    pub fn none(self) -> bool {
        !self.ctrl && !self.alt && !self.shift
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyEv {
    Press(Key, Mods),
    Repeat(Key, Mods),
    Release(Key),
    FocusLost,
    FocusGained,
    Resize(i32, i32),
}

impl KeyEv {
    pub fn press(k: Key) -> Self {
        KeyEv::Press(k, Mods::plain())
    }
    pub fn release(k: Key) -> Self {
        KeyEv::Release(k)
    }
}

/// Which player owns a movement key, and in which direction.
pub fn movement(key: Key) -> Option<(usize, Dir)> {
    Some(match key {
        Key::W => (0, Dir::Up),
        Key::S => (0, Dir::Down),
        Key::A => (0, Dir::Left),
        Key::D => (0, Dir::Right),
        Key::Up => (1, Dir::Up),
        Key::Down => (1, Dir::Down),
        Key::Left => (1, Dir::Left),
        Key::Right => (1, Dir::Right),
        _ => return None,
    })
}

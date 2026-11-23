//! Double-buffered ANSI canvas.
//!
//! Rendering strategy (see docs/RESEARCH.md §2):
//!  * one `Cell` per terminal cell, compared against the previous frame;
//!  * only changed horizontal runs are emitted, each preceded by one `MoveTo`;
//!  * the whole frame is wrapped in a synchronized-update block so emulators
//!    swap it atomically (no tearing, no flicker);
//!  * colours are raw 256-colour SGR codes written straight into a `String`.

use crate::core::Vec2;
use std::io::{self, Write};

/// 256-colour palette index.
pub type Col = u8;

/// Shared art-direction palette. Everything the game draws pulls from here.
pub mod pal {
    use super::Col;

    pub const BG: Col = 234;
    pub const PANEL: Col = 235;
    pub const GRID: Col = 236;
    pub const GRID_HI: Col = 238;

    pub const TEXT: Col = 252;
    pub const MUTED: Col = 244;
    pub const DIM: Col = 240;
    pub const BLACK: Col = 16;
    pub const WHITE: Col = 231;

    // Player one: electric cyan.
    pub const P1: Col = 51;
    pub const P1_HI: Col = 159;
    pub const P1_LO: Col = 31;
    // Player two: hot magenta.
    pub const P2: Col = 213;
    pub const P2_HI: Col = 225;
    pub const P2_LO: Col = 127;

    // The beam.
    pub const ARC_HOT: Col = 231;
    pub const ARC_MID: Col = 87;
    pub const ARC_COLD: Col = 24;
    pub const ARC_SPARK: Col = 195;

    // Enemies.
    pub const FOE: Col = 168;
    pub const FOE_HI: Col = 217;
    pub const FOE_ARMOR: Col = 137;
    pub const NULL: Col = 99;
    pub const NULL_HI: Col = 177;

    // Pickups / status.
    pub const CHARGE: Col = 45;
    pub const MEND: Col = 114;
    pub const WARN: Col = 203;
    pub const GOLD: Col = 220;
    pub const GOOD: Col = 84;
    pub const BAD: Col = 160;

    // Enemy fire. Hot enough to read instantly against the dark floor.
    pub const HAZ: Col = 204;
    pub const HAZ_HI: Col = 196;
    pub const HAZ_HEAVY: Col = 208;

    // Bosses get their own purple/gold register.
    pub const BOSS: Col = 128;
    pub const BOSS_HI: Col = 171;

    // Buffs.
    pub const SHIELD: Col = 117;
    pub const RAPID: Col = 226;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub ch: char,
    pub fg: Col,
    pub bg: Col,
}

impl Default for Cell {
    fn default() -> Self {
        Cell {
            ch: ' ',
            fg: pal::TEXT,
            bg: pal::BG,
        }
    }
}

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub color: bool,
    cur: Vec<Cell>,
    prev: Vec<Cell>,
    buf: String,
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Self {
        let w = w.max(1);
        let h = h.max(1);
        Self {
            w,
            h,
            color: std::env::var_os("NO_COLOR").is_none(),
            cur: vec![Cell::default(); (w * h) as usize],
            prev: vec![Cell {
                ch: '\0',
                fg: 0,
                bg: 0,
            }; (w * h) as usize],
            buf: String::with_capacity(16 * 1024),
        }
    }

    pub fn resize(&mut self, w: i32, h: i32) {
        *self = Canvas::new(w, h);
    }

    #[inline]
    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            None
        } else {
            Some((y * self.w + x) as usize)
        }
    }

    pub fn clear(&mut self) {
        for c in self.cur.iter_mut() {
            *c = Cell::default();
        }
    }

    #[inline]
    pub fn put(&mut self, x: i32, y: i32, ch: char, fg: Col) {
        if let Some(i) = self.idx(x, y) {
            let bg = self.cur[i].bg;
            self.cur[i] = Cell { ch, fg, bg };
        }
    }

    #[inline]
    pub fn put2(&mut self, x: i32, y: i32, ch: char, fg: Col, bg: Col) {
        if let Some(i) = self.idx(x, y) {
            self.cur[i] = Cell { ch, fg, bg };
        }
    }

    #[inline]
    pub fn putv(&mut self, p: Vec2, ch: char, fg: Col) {
        self.put(p.x, p.y, ch, fg);
    }

    #[inline]
    pub fn bg(&mut self, x: i32, y: i32, bg: Col) {
        if let Some(i) = self.idx(x, y) {
            self.cur[i].bg = bg;
        }
    }

    /// Paint a background only where the cell is still blank.
    pub fn shade(&mut self, x: i32, y: i32, bg: Col) {
        if let Some(i) = self.idx(x, y) {
            if self.cur[i].ch == ' ' {
                self.cur[i].bg = bg;
            }
        }
    }

    pub fn text(&mut self, x: i32, y: i32, s: &str, fg: Col) {
        let mut cx = x;
        for ch in s.chars() {
            self.put(cx, y, ch, fg);
            cx += 1;
        }
    }

    pub fn text_bg(&mut self, x: i32, y: i32, s: &str, fg: Col, bg: Col) {
        let mut cx = x;
        for ch in s.chars() {
            self.put2(cx, y, ch, fg, bg);
            cx += 1;
        }
    }

    /// Centred text inside `[x, x+w)`.
    pub fn text_center(&mut self, x: i32, y: i32, w: i32, s: &str, fg: Col) {
        let len = s.chars().count() as i32;
        self.text(x + (w - len).max(0) / 2, y, s, fg);
    }

    pub fn hline(&mut self, x: i32, y: i32, len: i32, ch: char, fg: Col) {
        for i in 0..len {
            self.put(x + i, y, ch, fg);
        }
    }

    pub fn vline(&mut self, x: i32, y: i32, len: i32, ch: char, fg: Col) {
        for i in 0..len {
            self.put(x, y + i, ch, fg);
        }
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, ch: char, fg: Col) {
        for yy in y..y + h {
            self.hline(x, yy, w, ch, fg);
        }
    }

    /// Solid fill including background - used for overlays that must occlude.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, ch: char, fg: Col, bg: Col) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put2(xx, yy, ch, fg, bg);
            }
        }
    }

    pub fn fill_bg(&mut self, x: i32, y: i32, w: i32, h: i32, bg: Col) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.bg(xx, yy, bg);
            }
        }
    }

    /// Single-line box border using light box drawing.
    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, fg: Col) {
        if w < 2 || h < 2 {
            return;
        }
        self.put(x, y, '\u{250c}', fg);
        self.put(x + w - 1, y, '\u{2510}', fg);
        self.put(x, y + h - 1, '\u{2514}', fg);
        self.put(x + w - 1, y + h - 1, '\u{2518}', fg);
        for i in 1..w - 1 {
            self.put(x + i, y, '\u{2500}', fg);
            self.put(x + i, y + h - 1, '\u{2500}', fg);
        }
        for i in 1..h - 1 {
            self.put(x, y + i, '\u{2502}', fg);
            self.put(x + w - 1, y + i, '\u{2502}', fg);
        }
    }

    pub fn line(&mut self, a: Vec2, b: Vec2, ch: char, fg: Col) {
        for p in crate::core::line(a, b) {
            self.put(p.x, p.y, ch, fg);
        }
    }

    /// Force a full repaint on the next flush.
    pub fn invalidate(&mut self) {
        for c in self.prev.iter_mut() {
            c.ch = '\0';
        }
    }

    /// Plain-text dump of the frame currently being built (no colours).
    /// Used by the headless harness and tests.
    pub fn snapshot(&self) -> String {
        let mut s = String::with_capacity((self.w * (self.h + 1)) as usize);
        for y in 0..self.h {
            for x in 0..self.w {
                s.push(self.cur[(y * self.w + x) as usize].ch);
            }
            s.push('\n');
        }
        s
    }

    pub fn cell_count(&self) -> usize {
        self.cur.len()
    }

    /// The frame currently built, as the ANSI byte stream `flush` would emit.
    pub fn ansi_frame(&mut self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::with_capacity(4096);
        let _ = self.flush(&mut out);
        out
    }

    fn push_colors(&mut self, fg: Col, bg: Col) {
        use std::fmt::Write as _;
        let _ = write!(self.buf, "\x1b[38;5;{}m\x1b[48;5;{}m", fg, bg);
    }

    fn push_move(&mut self, x: i32, y: i32) {
        use std::fmt::Write as _;
        let _ = write!(self.buf, "\x1b[{};{}H", y + 1, x + 1);
    }

    /// Diff against the previous frame and write one atomic update.
    pub fn flush(&mut self, out: &mut impl Write) -> io::Result<()> {
        self.buf.clear();
        self.buf.push_str("\x1b[?2026h");

        for y in 0..self.h {
            let base = (y * self.w) as usize;
            let mut run: Option<i32> = None;
            for x in 0..=self.w {
                let changed = if x < self.w {
                    self.cur[base + x as usize] != self.prev[base + x as usize]
                } else {
                    false
                };
                match (changed, run) {
                    (true, None) => run = Some(x),
                    (false, Some(sx)) => {
                        self.push_move(sx, y);
                        let mut last: Option<(Col, Col)> = None;
                        for cx in sx..x {
                            let c = self.cur[base + cx as usize];
                            if self.color && last != Some((c.fg, c.bg)) {
                                self.push_colors(c.fg, c.bg);
                                last = Some((c.fg, c.bg));
                            }
                            self.buf.push(c.ch);
                        }
                        run = None;
                    }
                    _ => {}
                }
            }
        }

        if self.color {
            self.buf.push_str("\x1b[0m");
        }
        self.buf.push_str("\x1b[?2026l");

        let bytes = self.buf.as_bytes();
        out.write_all(bytes)?;
        out.flush()?;

        std::mem::swap(&mut self.cur, &mut self.prev);
        // The buffer we just painted becomes the baseline; start the next frame
        // from a blank slate but keep a copy in `prev` for diffing.
        for c in self.cur.iter_mut() {
            *c = Cell::default();
        }
        Ok(())
    }
}

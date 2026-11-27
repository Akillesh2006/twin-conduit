//! Screen chrome: layout, HUD, title, pause, results.

use crate::canvas::{pal, Canvas, Col};
use crate::core::Field;
use crate::game::{World, MAX_HP, REST_DIST, TOTAL_WAVES};

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub term_w: i32,
    pub term_h: i32,
    pub ox: i32,
    pub oy: i32,
    pub field_w: i32,
    pub field_h: i32,
}

impl Layout {
    pub fn field(&self) -> Field {
        Field::new(self.field_w, self.field_h)
    }
    pub fn right(&self) -> i32 {
        self.ox + self.field_w - 1
    }
    pub fn bottom(&self) -> i32 {
        self.oy + self.field_h - 1
    }
    pub fn band(&self) -> (i32, i32) {
        (self.ox - 1, self.field_w + 2)
    }
}

pub const MIN_W: i32 = 72;
pub const MIN_H: i32 = 26;

pub fn compute_layout(term_w: i32, term_h: i32) -> Layout {
    let field_w = (term_w - 4).clamp(24, 84);
    let field_h = (term_h - 12).clamp(10, 24);
    let ox = (term_w - field_w) / 2;
    Layout {
        term_w,
        term_h,
        ox,
        oy: 5,
        field_w,
        field_h,
    }
}

// ------------------------------------------------------------------ helpers

fn right_text(cv: &mut Canvas, right_edge: i32, y: i32, s: &str, fg: Col) {
    let len = s.chars().count() as i32;
    cv.text(right_edge - len + 1, y, s, fg);
}

fn hearts(cv: &mut Canvas, x: i32, y: i32, hp: i32, full: Col, empty: Col) {
    for i in 0..MAX_HP {
        let ch = if i < hp { '\u{2665}' } else { '\u{2661}' };
        let col = if i < hp { full } else { empty };
        cv.put(x + i * 2, y, ch, col);
    }
}

pub fn charge_col(pct: f32) -> Col {
    if pct > 0.60 {
        pal::CHARGE
    } else if pct > 0.25 {
        pal::GOLD
    } else {
        pal::WARN
    }
}

fn meter(cv: &mut Canvas, x: i32, y: i32, w: i32, frac: f32, label: &str, col: Col) {
    let fill = ((frac.clamp(0.0, 1.0)) * w as f32).round() as i32;
    for i in 0..w {
        let (ch, c) = if i < fill {
            ('\u{2588}', col)
        } else {
            ('\u{2591}', pal::GRID)
        };
        cv.put(x + i, y, ch, c);
    }
    let lx = x + (w - label.chars().count() as i32) / 2;
    cv.text_bg(lx, y, label, pal::WHITE, pal::PANEL);
}

// ---------------------------------------------------------------------- HUD

pub fn draw_hud(cv: &mut Canvas, w: &World, l: &Layout, _t: u64) {
    let (bx, bw) = l.band();
    let right = bx + bw - 1;

    // --- row 0 : Player 1, Wave counter, Score
    let (c1, c1h) = (pal::P1, pal::P1_HI);
    cv.put(bx, 0, '\u{25c9}', c1h);
    cv.text(bx + 2, 0, "P1", c1);
    hearts(cv, bx + 5, 0, w.players[0].hp, c1, pal::DIM);
    if !w.players[0].alive {
        cv.text(bx + 12, 0, "DOWN", pal::BAD);
    }

    let wave_str = format!("WAVE {:02}/{:02}", w.wave.min(TOTAL_WAVES), TOTAL_WAVES);
    let wcol = if w.wave == TOTAL_WAVES {
        pal::NULL_HI
    } else {
        pal::MUTED
    };
    cv.text_center(bx, 0, bw, &wave_str, wcol);

    let score_str = format!("SCORE {:>7}", w.score);
    right_text(cv, right, 0, &score_str, pal::TEXT);

    // --- row 1 : Shared CHARGE meter
    let pct = (w.charge / 100.0).clamp(0.0, 1.0);
    let status = if w.charge <= 0.0 {
        "CELL DARK"
    } else if !w.players[0].alive || !w.players[1].alive {
        "REVIVE YOUR PARTNER"
    } else if w.dist <= REST_DIST {
        "COLD \u{00b7} RECHARGING"
    } else if w.hot {
        "CONDUIT LIVE \u{00b7} REACH"
    } else {
        "SLACK TETHER"
    };
    let label = format!("CHARGE {:>3}% \u{00b7} {}", (pct * 100.0).round() as i32, status);
    meter(cv, bx, 1, bw, pct, &label, charge_col(pct));

    // --- row 2 : Player 2, Boss progress / Distance, Combo
    let (c2, c2h) = (pal::P2, pal::P2_HI);
    cv.put(bx, 2, '\u{25c9}', c2h);
    cv.text(bx + 2, 2, "P2", c2);
    hearts(cv, bx + 5, 2, w.players[1].hp, c2, pal::DIM);
    if !w.players[1].alive {
        cv.text(bx + 12, 2, "DOWN", pal::BAD);
    }

    if let Some((frac, name)) = w.boss_progress() {
        let b_lbl = format!("{}  {:>3}%", name, (frac * 100.0).round() as i32);
        cv.text_center(bx, 2, bw, &b_lbl, pal::NULL_HI);
    } else {
        let dist_lbl = format!("DIST {:>2} / 34", w.dist);
        let dcol = if w.dist > 30 { pal::WARN } else { pal::MUTED };
        cv.text_center(bx, 2, bw, &dist_lbl, dcol);
    }

    if w.combo >= 2 {
        let combo_str = format!("x{}", w.combo);
        right_text(cv, right, 2, &combo_str, pal::GOLD);
    }

    // --- row 3 : Objective or banner guidance
    let (obj_msg, obj_col) = if !w.players[0].alive || !w.players[1].alive {
        ("STAND BESIDE YOUR PARTNER TO RE-LINK", pal::GOOD)
    } else if w.boss().is_some() {
        ("SEVER THE NULL WITH THE BEAM", pal::NULL_HI)
    } else if w.intermission > 0 {
        ("WAVE CLEARED \u{00b7} RECHARGE", pal::GOOD)
    } else if w.dist > 30 {
        ("SNAP WARNING \u{00b7} PULL CLOSER!", pal::WARN)
    } else {
        ("GET CLOSE TO RECHARGE \u{00b7} GET FAR TO BURN", pal::DIM)
    };
    cv.text_center(bx, 3, bw, obj_msg, obj_col);

    // --- below arena : Controls
    let y = l.bottom() + 3;
    if y < l.term_h {
        let left = "WASD \u{25c9}P1   \u{00b7}   ARROWS \u{25c9}P2";
        cv.text(bx, y, left, pal::DIM);
        right_text(cv, right, y, "P PAUSE   R RESTART   ESC QUIT", pal::DIM);
    }

    // --- floating banner
    if let Some((text, ttl)) = &w.banner {
        let by = l.oy + l.field_h / 2 - 1;
        let len = text.chars().count() as i32 + 4;
        let bx2 = l.ox + (l.field_w - len) / 2;
        let fade = if *ttl > 12 { pal::WHITE } else { pal::MUTED };
        cv.fill_rect(bx2 - 1, by, len + 2, 3, ' ', pal::TEXT, pal::PANEL);
        cv.text_bg(bx2, by + 1, &format!("  {}  ", text), fade, pal::PANEL);
    }
}

// -------------------------------------------------------------------- title

/// 5x7 bitmap glyphs for CONDUIT: C O N D U I T
const FONT: [[&str; 7]; 7] = [
    // C
    [" ### ", "#   #", "#    ", "#    ", "#    ", "#   #", " ### "],
    // O
    [" ### ", "#   #", "#   #", "#   #", "#   #", "#   #", " ### "],
    // N
    ["#   #", "##  #", "# # #", "#  ##", "#   #", "#   #", "#   #"],
    // D
    ["#### ", "#   #", "#   #", "#   #", "#   #", "#   #", "#### "],
    // U
    ["#   #", "#   #", "#   #", "#   #", "#   #", "#   #", " ### "],
    // I
    ["#####", "  #  ", "  #  ", "  #  ", "  #  ", "  #  ", "#####"],
    // T
    ["#####", "  #  ", "  #  ", "  #  ", "  #  ", "  #  ", "  #  "],
];

const LOGO: [usize; 7] = [0, 1, 2, 3, 4, 5, 6];
const LOGO_COLS: [Col; 7] = [
    pal::P1_HI,
    pal::P1,
    pal::ARC_MID,
    pal::WHITE,
    pal::ARC_MID,
    pal::P2,
    pal::P2_HI,
];

pub fn draw_logo(cv: &mut Canvas, ox: i32, oy: i32) {
    let mut cx = ox;
    for (i, &idx) in LOGO.iter().enumerate() {
        let glyph = FONT[idx];
        let col = LOGO_COLS[i];
        for (r, row) in glyph.iter().enumerate() {
            for (c, ch) in row.chars().enumerate() {
                if ch == '#' {
                    cv.put(cx + c as i32, oy + r as i32, '\u{2588}', col);
                }
            }
        }
        cx += 6;
    }
}

pub fn logo_width() -> i32 {
    7 * 6 - 1
}

pub fn draw_title(cv: &mut Canvas, l: &Layout, _t: u64) {
    let lw = logo_width();
    let lx = (l.term_w - lw) / 2;
    let ly = (l.term_h / 2) - 8;

    draw_logo(cv, lx, ly);

    let sub = "TWO OPERATORS \u{00b7} ONE CONDUIT \u{00b7} ONE KEYBOARD";
    cv.text_center(0, ly + 9, l.term_w, sub, pal::TEXT);

    let rule = "GET CLOSE TO RECHARGE \u{00b7} GET FAR TO BURN";
    cv.text_center(0, ly + 11, l.term_w, rule, pal::GOLD);

    let info = "the beam between you is the only weapon";
    cv.text_center(0, ly + 12, l.term_w, info, pal::MUTED);

    let keys = "P1: W A S D     \u{00b7}     P2: ARROW KEYS";
    cv.text_center(0, ly + 15, l.term_w, keys, pal::P1_HI);

    let prompt = "ENTER to start \u{00b7} ESC to quit";
    cv.text_center(0, ly + 18, l.term_w, prompt, pal::WHITE);
}

// -------------------------------------------------------------------- results

pub fn draw_pause(cv: &mut Canvas, l: &Layout) {
    let (bx, bw) = l.band();
    let py = l.oy + l.field_h / 2 - 2;
    cv.fill_rect(bx + 4, py, bw - 8, 5, ' ', pal::TEXT, pal::PANEL);
    cv.frame(bx + 4, py, bw - 8, 5, pal::GOLD);
    cv.text_center(bx + 4, py + 1, bw - 8, "PAUSED", pal::WHITE);
    cv.text_center(bx + 4, py + 3, bw - 8, "P / ENTER to resume", pal::MUTED);
}

pub fn draw_gameover(cv: &mut Canvas, w: &World, l: &Layout, won: bool) {
    let (bx, bw) = l.band();
    let py = l.oy + l.field_h / 2 - 5;
    cv.fill_rect(bx + 4, py, bw - 8, 12, ' ', pal::TEXT, pal::PANEL);
    cv.frame(bx + 4, py, bw - 8, 12, if won { pal::GOOD } else { pal::WARN });

    let (title, sub, col) = if won {
        ("SIGNAL STABILISED", "the conduit held", pal::GOOD)
    } else {
        ("SIGNAL LOST", "the cell went dark", pal::WARN)
    };

    cv.text_center(bx + 4, py + 1, bw - 8, title, col);
    cv.text_center(bx + 4, py + 2, bw - 8, sub, pal::MUTED);

    let stats = [
        format!("SCORE        {:>7}", w.score),
        format!("WAVE         {:>7}", w.wave),
        format!("KILLS        {:>7}", w.kills),
        format!("BEST COMBO   {:>7}", w.best_combo),
    ];
    for (i, s) in stats.iter().enumerate() {
        cv.text_center(bx + 4, py + 4 + i as i32, bw - 8, s, pal::TEXT);
    }

    cv.text_center(bx + 4, py + 9, bw - 8, "ENTER  run it again", pal::WHITE);
    cv.text_center(bx + 4, py + 10, bw - 8, "ESC    back to title", pal::MUTED);
}

pub fn draw_too_small(cv: &mut Canvas, tw: i32, th: i32) {
    cv.clear();
    let msg = "TERMINAL TOO SMALL";
    let req = format!("need at least {}x{} (current: {}x{})", MIN_W, MIN_H, tw, th);
    cv.text_center(0, th / 2 - 1, tw, msg, pal::WARN);
    cv.text_center(0, th / 2 + 1, tw, &req, pal::MUTED);
}

//! Screen chrome: layout, HUD, title, pause, results.

use crate::canvas::{pal, Canvas, Col};
use crate::core::Field;
use crate::game::{Player, World, MAX_HP, TOTAL_WAVES};

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
    /// Full-width row available to chrome (frame included).
    fn band(&self) -> (i32, i32) {
        (self.ox - 1, self.field_w + 2)
    }
}

pub const MIN_W: i32 = 72;
pub const MIN_H: i32 = 26;

pub fn compute_layout(term_w: i32, term_h: i32) -> Layout {
    // Keep the play space dense: a wide field spreads enemy fire thin, and the
    // auto-cannons want targets inside a screen's reach.
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

fn meter(cv: &mut Canvas, x: i32, y: i32, w: i32, frac: f32, col: Col) {
    let fill = ((frac.clamp(0.0, 1.0)) * w as f32).round() as i32;
    for i in 0..w {
        let (ch, c) = if i < fill {
            ('\u{2588}', col)
        } else {
            ('\u{2591}', pal::GRID)
        };
        cv.put(x + i, y, ch, c);
    }
    let pct = (frac.clamp(0.0, 1.0) * 100.0).round() as i32;
    let label = format!("THREAT {:>3}%", pct);
    let lx = x + (w - label.chars().count() as i32) / 2;
    cv.text_bg(lx, y, &label, pal::WHITE, pal::PANEL);
}

fn threat_col(p: f32) -> Col {
    if p > 0.72 {
        pal::WARN
    } else if p > 0.48 {
        pal::GOLD
    } else {
        pal::GOOD
    }
}

fn buffs(p: &Player, pet: Option<&crate::game::Pet>) -> String {
    let mut s = String::new();
    if p.rapid > 0 {
        s.push_str("RAPID ");
    }
    if p.shield > 0 {
        s.push_str("SHIELD ");
    }
    if p.evolution > 0 {
        match p.evolution {
            1 => s.push_str("EVO:DUAL "),
            2 => s.push_str("EVO:TRI "),
            _ => s.push_str("EVO:MAX "),
        }
    }
    if let Some(pet) = pet {
        s.push_str(&format!("PET:{} ", pet.name()));
    }
    s.trim_end().to_string()
}

// ---------------------------------------------------------------------- HUD

pub fn draw_hud(cv: &mut Canvas, w: &World, l: &Layout, t: u64) {
    let (bx, bw) = l.band();
    let right = bx + bw - 1;

    // --- row 0 : player one, wave, score
    let (c1, c1h) = (pal::P1, pal::P1_HI);
    cv.put(bx, 0, '\u{25c9}', c1h);
    cv.text(bx + 2, 0, "P1", c1);
    hearts(cv, bx + 5, 0, w.players[0].hp, c1, pal::DIM);
    if !w.players[0].alive {
        cv.text(bx + 16, 0, "DOWN", pal::BAD);
    }

    let wave = format!("WAVE {:02}/{:02}", w.wave.min(TOTAL_WAVES), TOTAL_WAVES);
    let wcol = if w.wave_is_boss(w.wave) {
        pal::BOSS_HI
    } else {
        pal::MUTED
    };
    cv.text_center(bx, 0, bw, &wave, wcol);

    let score = format!("SCORE {:>7}", w.score);
    right_text(cv, right, 0, &score, pal::TEXT);

    // --- row 1 : director pressure
    let pressure = w.adaptive.pressure;
    meter(cv, bx, 1, bw, pressure, threat_col(pressure));

    // --- row 2 : player two, boss health / accuracy, combo
    let (c2, c2h) = (pal::P2, pal::P2_HI);
    cv.put(bx, 2, '\u{25c9}', c2h);
    cv.text(bx + 2, 2, "P2", c2);
    hearts(cv, bx + 5, 2, w.players[1].hp, c2, pal::DIM);
    if !w.players[1].alive {
        cv.text(bx + 16, 2, "DOWN", pal::BAD);
    }

    if let Some((frac, name)) = w.boss_progress() {
        let label = format!("{}  {:>3}%", name, (frac * 100.0).round() as i32);
        let col = if frac < 0.34 { pal::HAZ_HI } else { pal::BOSS_HI };
        cv.text_center(bx, 2, bw, &label, col);
    } else {
        let acc = format!("ACC {:>3}%", w.accuracy().round() as i32);
        cv.text_center(bx, 2, bw, &acc, pal::MUTED);
    }

    if w.combo >= 2 {
        let s = format!("x{} ", w.combo);
        right_text(cv, right, 2, &s, pal::GOLD);
    }

    // --- row 3 : buffs and the current objective
    let pet1 = w.pets.iter().find(|p| p.side == crate::core::Side::One);
    let pet2 = w.pets.iter().find(|p| p.side == crate::core::Side::Two);
    let b1 = buffs(&w.players[0], pet1);
    if !b1.is_empty() {
        cv.text(bx, 3, &b1, pal::GOLD);
    }
    let b2 = buffs(&w.players[1], pet2);
    if !b2.is_empty() {
        right_text(cv, right, 3, &b2, pal::GOLD);
    }

    let (msg, mcol) = if w.players.iter().any(|p| !p.alive) {
        ("REVIVE YOUR PARTNER", pal::GOOD)
    } else if w.boss().is_some() {
        ("BOSS ENGAGED", pal::BOSS_HI)
    } else if w.intermission > 0 {
        ("WAVE CLEAR", pal::GOOD)
    } else {
        ("AUTO-CANNONS ENGAGED", pal::DIM)
    };
    cv.text_center(bx, 3, bw, msg, mcol);

    // --- below the arena : legend / controls
    // Leave two rows of slack under the frame: shake can push it down a cell.
    let y = l.bottom() + 3;
    if y < l.term_h {
        let left = format!(
            "WASD \u{25c9}P1   {}   ARROWS \u{25c9}P2",
            if w.players[0].alive && w.players[1].alive {
                "\u{00b7}"
            } else {
                "!"
            }
        );
        cv.text(bx, y, &left, pal::DIM);
        right_text(cv, right, y, "P PAUSE   R RESTART   ESC QUIT", pal::DIM);
    }

    // --- banner
    if let Some((text, ttl)) = &w.banner {
        let by = l.oy + l.field_h / 2 - 1;
        let len = text.chars().count() as i32 + 4;
        let bx2 = l.ox + (l.field_w - len) / 2;
        let fade = if *ttl > 12 { pal::WHITE } else { pal::MUTED };
        cv.fill_rect(bx2 - 1, by, len + 2, 3, ' ', pal::TEXT, pal::PANEL);
        cv.text_bg(bx2, by + 1, &format!("  {}  ", text), fade, pal::PANEL);
    }

    let _ = t;
}

// -------------------------------------------------------------------- title

/// 5x7 bitmap glyphs for the wordmark. `#` is a solid pixel.
const FONT: [[&str; 7]; 6] = [
    // O
    [
        " ### ", "#   #", "#   #", "#   #", "#   #", "#   #", " ### ",
    ],
    // V
    [
        "#   #", "#   #", "#   #", "#   #", "#   #", " # # ", "  #  ",
    ],
    // E
    [
        "#####", "#    ", "#    ", "#### ", "#    ", "#    ", "#####",
    ],
    // R
    [
        "#### ", "#   #", "#   #", "#### ", "# #  ", "#  # ", "#   #",
    ],
    // D
    [
        "#### ", "#   #", "#   #", "#   #", "#   #", "#   #", "#### ",
    ],
    // I
    [
        "#####", "  #  ", "  #  ", "  #  ", "  #  ", "  #  ", "#####",
    ],
];

/// OVERDRIVE = O V E R D R I V E
const LOGO: [usize; 9] = [0, 1, 2, 3, 4, 3, 5, 1, 2];

const LOGO_COLS: [Col; 9] = [
    pal::P1_HI,
    pal::P1,
    pal::P1_LO,
    pal::P2_LO,
    pal::P2,
    pal::P2_HI,
    pal::GOLD,
    pal::P1_HI,
    pal::P2_HI,
];

fn draw_logo(cv: &mut Canvas, x: i32, y: i32, scale: i32) {
    for row in 0..7 {
        let mut cx = x;
        for (li, glyph) in LOGO.iter().enumerate() {
            for px in FONT[*glyph][row].chars() {
                if px == '#' {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            cv.put(
                                cx + sx,
                                y + row as i32 * scale + sy,
                                '\u{2588}',
                                LOGO_COLS[li],
                            );
                        }
                    }
                }
                cx += scale;
            }
            cx += scale; // gap between letters
        }
    }
}

fn logo_width(scale: i32) -> i32 {
    // 9 glyphs of 5px each plus 8 inter-letter gaps.
    (9 * 5 + 8) * scale
}

pub fn draw_title(cv: &mut Canvas, t: u64, best: u64) {
    let tw = cv.w;
    let th = cv.h;
    let cx = tw / 2;

    let scale = if tw >= 116 { 2 } else { 1 };
    let logo_h = 7 * scale;
    let top = ((th - (logo_h + 13)) / 2).max(1);
    draw_logo(cv, cx - logo_width(scale) / 2, top, scale);

    let y = top + logo_h + 1;
    cv.text_center(
        0,
        y,
        tw,
        "TWO OPERATORS \u{00b7} AUTO-CANNONS \u{00b7} ONE KEYBOARD",
        pal::MUTED,
    );

    // Animated demo: two operators hosing a drifting monster with tracer fire.
    let span = (tw - 30).clamp(24, 74);
    let x0 = cx - span / 2;
    let by = y + 2;
    let sway = ((t as f32) * 0.05).sin();
    let ex = (x0 + span / 2 + (sway * span as f32 * 0.30) as i32)
        .clamp(x0 + 3, x0 + span - 4);
    let et = t as i32;
    cv.put(x0, by, '\u{25c9}', pal::P1_HI);
    cv.put(x0 + span - 1, by, '\u{25c9}', pal::P2_HI);
    cv.put(ex, by, '\u{00d7}', pal::FOE);
    for x in x0 + 2..ex {
        if (x + et / 2) % 2 == 0 {
            cv.put(x, by, '\u{00b7}', pal::P1);
        }
    }
    for x in ex + 1..x0 + span - 1 {
        if (x + et / 2) % 2 == 0 {
            cv.put(x, by, '\u{00b7}', pal::P2);
        }
    }

    let y = by + 2;
    let mid = cx - 20;
    cv.text(mid, y, "\u{25c9} P1", pal::P1);
    cv.text(mid + 7, y, "W A S D", pal::TEXT);
    cv.text(mid + 24, y, "\u{25c9} P2", pal::P2);
    cv.text(mid + 31, y, "ARROW KEYS", pal::TEXT);

    let y = y + 2;
    cv.text_center(
        0,
        y,
        tw,
        "MOVE TO DODGE  \u{00b7}  THE GUNS FIRE FOR YOU",
        pal::GOLD,
    );
    cv.text_center(
        0,
        y + 1,
        tw,
        "survive the swarm \u{00b7} a boss every five waves",
        pal::MUTED,
    );

    let y = y + 3;
    let start_col = if (t / 18) % 2 == 0 {
        pal::WHITE
    } else {
        pal::GOOD
    };
    cv.text_center(0, y, tw, "[ ENTER ]  START", start_col);
    if best > 0 {
        cv.text_center(0, y + 1, tw, &format!("BEST {:>7}", best), pal::DIM);
    }
    let foot = th - 2;
    if foot > y + 1 {
        cv.text_center(
            0,
            foot,
            tw,
            "P PAUSE  \u{00b7}  R RESTART  \u{00b7}  ESC QUIT",
            pal::DIM,
        );
    }
}

// ------------------------------------------------------------------ overlays

fn panel(cv: &mut Canvas, x: i32, y: i32, w: i32, h: i32, border: Col) {
    cv.fill_rect(x, y, w, h, ' ', pal::TEXT, pal::PANEL);
    cv.frame(x, y, w, h, border);
}

pub fn draw_pause(cv: &mut Canvas, l: &Layout) {
    let w = 34;
    let h = 5;
    let x = l.ox + (l.field_w - w) / 2;
    let y = l.oy + (l.field_h - h) / 2;
    panel(cv, x, y, w, h, pal::BOSS_HI);
    cv.text_center(x, y + 1, w, "PAUSED", pal::WHITE);
    cv.text_center(x, y + 2, w, "P / ENTER to resume", pal::MUTED);
}

pub fn draw_gameover(cv: &mut Canvas, w: &World, l: &Layout, won: bool, t: u64, best: u64) {
    let (title, col) = if won {
        ("ARENA CLEARED", pal::GOOD)
    } else {
        ("SYSTEM DOWN", pal::BAD)
    };

    let pw = 46.min(l.field_w);
    let ph = 13;
    let x = l.ox + (l.field_w - pw) / 2;
    let y = l.oy + (l.field_h - ph) / 2;
    panel(cv, x, y, pw, ph, col);

    cv.text_center(x, y + 1, pw, title, col);
    cv.text_center(
        x,
        y + 2,
        pw,
        if won {
            "the swarm is scrap"
        } else {
            "the operators went dark"
        },
        pal::MUTED,
    );

    let rows = [
        ("SCORE", format!("{}", w.score), pal::WHITE),
        ("WAVE", format!("{} / {}", w.wave.min(TOTAL_WAVES), TOTAL_WAVES), pal::TEXT),
        ("KILLS", format!("{}", w.kills), pal::TEXT),
        ("BEST COMBO", format!("x{}", w.best_combo), pal::GOLD),
        ("ACCURACY", format!("{:.0}%", w.accuracy()), pal::TEXT),
        ("BEST EVER", format!("{}", best.max(w.score)), pal::DIM),
    ];
    for (i, (k, v, c)) in rows.iter().enumerate() {
        let yy = y + 4 + i as i32;
        cv.text(x + 4, yy, k, pal::MUTED);
        let vl = v.chars().count() as i32;
        cv.text(x + pw - 4 - vl, yy, v, *c);
    }

    if (t / 18) % 2 == 0 {
        cv.text_center(x, y + ph - 2, pw, "ENTER  run it again", pal::WHITE);
    } else {
        cv.text_center(x, y + ph - 2, pw, "ESC    back to title", pal::DIM);
    }
}

pub fn draw_too_small(cv: &mut Canvas, term_w: i32, term_h: i32) {
    let msg = "TERMINAL TOO SMALL";
    let sub = format!("need at least {}x{}  \u{00b7}  now {}x{}", MIN_W, MIN_H, term_w, term_h);
    cv.text_center(0, cv.h / 2 - 1, cv.w, msg, pal::WARN);
    cv.text_center(0, cv.h / 2 + 1, cv.w, &sub, pal::MUTED);
}

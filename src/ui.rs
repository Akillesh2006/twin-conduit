//! Screen chrome: layout, HUD, title, pause, results.

use crate::canvas::{pal, Canvas, Col};
use crate::core::Field;
use crate::game::{Difficulty, World, REST_DIST, TOTAL_WAVES};
use crate::scores::ScoreEntry;

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

fn hearts(cv: &mut Canvas, x: i32, y: i32, hp: i32, max_hp: i32, full: Col, empty: Col) {
    for i in 0..max_hp {
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
    hearts(cv, bx + 5, 0, w.players[0].hp, w.max_hp, c1, pal::DIM);
    if !w.players[0].alive {
        cv.text(bx + 5 + w.max_hp * 2 + 1, 0, "DOWN", pal::BAD);
    }

    let wave_str = format!(
        "WAVE {:02}/{:02} \u{00b7} {}",
        w.wave.min(TOTAL_WAVES),
        TOTAL_WAVES,
        w.difficulty.spec().short
    );
    let wcol = if w.wave_is_boss(w.wave) {
        pal::NULL_HI
    } else {
        pal::MUTED
    };
    cv.text_center(bx, 0, bw, &wave_str, wcol);

    let score_str = format!("SCORE {:>7}", w.score);
    right_text(cv, right, 0, &score_str, pal::TEXT);

    // --- row 1 : Shared CHARGE meter
    let pct = (w.charge / 100.0).clamp(0.0, 1.0);
    let max_snap = w.max_snap_dist();
    let near_snap = w.dist >= max_snap - 4;
    let low_charge = w.charge < 25.0;
    let status = if w.charge <= 0.0 {
        "CELL DARK"
    } else if !w.players[0].alive || !w.players[1].alive {
        "REVIVE YOUR PARTNER"
    } else if low_charge && ( _t % 4 < 2 ) {
        "!! LOW CHARGE !!"
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
    hearts(cv, bx + 5, 2, w.players[1].hp, w.max_hp, c2, pal::DIM);
    if !w.players[1].alive {
        cv.text(bx + 5 + w.max_hp * 2 + 1, 2, "DOWN", pal::BAD);
    }

    if let Some((frac, name, cur_p, tot_p)) = w.boss_info() {
        let bar_w = (bw - 46).clamp(8, 22) as usize;
        let fill = ((frac * bar_w as f32).round() as usize).min(bar_w);
        let mut bar = String::with_capacity(bar_w);
        for i in 0..bar_w {
            bar.push(if i < fill { '\u{2588}' } else { '\u{2591}' });
        }
        let b_lbl = format!(
            "{} [{}] P{}/{} {:>3}%",
            name,
            bar,
            cur_p,
            tot_p,
            (frac * 100.0).round() as i32
        );
        cv.text_center(bx, 2, bw, &b_lbl, pal::NULL_HI);
    } else {
        let dist_lbl = if near_snap {
            format!("SNAP! {:>2} / {}", w.dist, max_snap)
        } else {
            format!("DIST {:>2} / {}", w.dist, max_snap)
        };
        let dcol = if near_snap { pal::WARN } else { pal::MUTED };
        cv.text_center(bx, 2, bw, &dist_lbl, dcol);
    }

    if w.combo >= 2 {
        let combo_str = format!("x{}", w.combo);
        right_text(cv, right, 2, &combo_str, pal::GOLD);
    }

    // --- row 3 : Objective, critical alerts, active buffs, wave event, or guidance
    let mut buffs = Vec::new();
    if w.synced && w.hot {
        buffs.push("SYNC -25% DRAIN");
    }
    if w.slow_t > 0 {
        buffs.push("SLOW");
    }
    if w.twin_t > 0 {
        buffs.push("TWIN");
    }
    if w.players.iter().any(|p| p.alive && p.barrier > 0) {
        buffs.push("SHIELD");
    }
    if w.event != crate::game::WaveEvent::None {
        buffs.push(w.event.label());
    }
    if let Some((obj_msg, obj_col)) = if !w.players[0].alive || !w.players[1].alive {
        Some(("STAND BESIDE YOUR PARTNER TO RE-LINK", pal::GOOD))
    } else if low_charge && w.charge > 0.0 {
        Some(("LOW CHARGE (<25%) \u{00b7} HUG TO RECHARGE!", pal::WARN))
    } else if near_snap {
        Some(("SNAP IMMINENT \u{00b7} PULL CLOSER!", pal::WARN))
    } else if !buffs.is_empty() {
        None
    } else if let Some((_, name, _, _)) = w.boss_info() {
        let msg = match name {
            "THE WARDEN" => "SEVER THE WARDEN \u{00b7} DODGE THE VOLLEYS",
            "THE HYDRA" => "SEVER THE HYDRA \u{00b7} AVOID TETHER CUTTER",
            _ => "SEVER THE NULL \u{00b7} SURVIVE ALL 3 PHASES",
        };
        Some((msg, pal::NULL_HI))
    } else if w.blocked_at.is_some() {
        Some(("OBSTACLE BLOCKING BEAM \u{00b7} FLANK AROUND", pal::WARN))
    } else if w.intermission > 0 {
        Some(("WAVE CLEARED \u{00b7} RECHARGE", pal::GOOD))
    } else {
        Some(("GET CLOSE TO RECHARGE \u{00b7} GET FAR TO BURN", pal::DIM))
    } {
        cv.text_center(bx, 3, bw, obj_msg, obj_col);
    } else {
        let bstr = format!("ACTIVE: {}", buffs.join(" \u{00b7} "));
        let bcol = if w.event != crate::game::WaveEvent::None {
            pal::GOLD
        } else {
            pal::P1_HI
        };
        cv.text_center(bx, 3, bw, &bstr, bcol);
    }

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

pub fn draw_title(cv: &mut Canvas, l: &Layout, diff: Difficulty, scores: &[ScoreEntry], _t: u64) {
    let lw = logo_width();
    let lx = (l.term_w - lw) / 2;
    let ly = ((l.term_h / 2) - 10).max(1);

    draw_logo(cv, lx, ly);

    let sub = "TWO OPERATORS \u{00b7} ONE CONDUIT \u{00b7} ONE KEYBOARD";
    cv.text_center(0, ly + 8, l.term_w, sub, pal::TEXT);

    let rule = "GET CLOSE TO RECHARGE \u{00b7} GET FAR TO BURN";
    cv.text_center(0, ly + 10, l.term_w, rule, pal::GOLD);

    let info = "the beam between you is the only weapon";
    cv.text_center(0, ly + 11, l.term_w, info, pal::MUTED);

    // Difficulty bar: clickable/navigable via A/D, Left/Right, or 1-4
    let mut bar = String::from("< ");
    for (i, d) in Difficulty::ALL.iter().enumerate() {
        if *d == diff {
            bar.push_str(&format!("[{}:{}] ", i + 1, d.spec().name));
        } else {
            bar.push_str(&format!(" {}:{}  ", i + 1, d.spec().name));
        }
    }
    bar.push('>');
    let dcol = match diff {
        Difficulty::Easy => pal::GOOD,
        Difficulty::Normal => pal::P1_HI,
        Difficulty::Hard => pal::GOLD,
        Difficulty::Nightmare => pal::WARN,
    };
    cv.text_center(0, ly + 13, l.term_w, &bar, dcol);
    cv.text_center(0, ly + 14, l.term_w, diff.spec().blurb, pal::MUTED);

    let keys = "P1: W A S D     \u{00b7}     P2: ARROW KEYS";
    cv.text_center(0, ly + 16, l.term_w, keys, pal::P1_HI);

    let prompt = "A/D or 1-4 DIFFICULTY \u{00b7} ENTER START \u{00b7} ESC QUIT";
    cv.text_center(0, ly + 18, l.term_w, prompt, pal::WHITE);

    if let Some(top) = scores.first() {
        let best_line = format!(
            "HIGH SCORE: {} ({} \u{00b7} W{:02}{})",
            top.score,
            top.diff,
            top.wave,
            if top.won { " WIN" } else { "" }
        );
        if ly + 20 < l.term_h {
            cv.text_center(0, ly + 20, l.term_w, &best_line, pal::GOLD);
        }
    }
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

pub fn draw_upgrade(cv: &mut Canvas, w: &World, l: &Layout, cursor: usize) {
    let (bx, bw) = l.band();
    let mw = (bw - 4).clamp(64, 78);
    let mx = bx + (bw - mw) / 2;
    let my = (l.oy + l.field_h / 2 - 7).max(4);
    let mh = 15;
    cv.fill_rect(mx, my, mw, mh, ' ', pal::TEXT, pal::PANEL);
    cv.frame(mx, my, mw, mh, pal::P1_HI);

    let title = format!("WAVE {:02} CLEARED \u{00b7} SELECT AN UPGRADE", w.wave.saturating_sub(1));
    cv.text_center(mx, my + 1, mw, &title, pal::GOLD);
    let held = format!("INSTALLED MODS: {}", w.upgrades.len());
    cv.text_center(mx, my + 2, mw, &held, pal::MUTED);

    for (i, up) in w.upgrade_choices.iter().enumerate() {
        let sel = i == cursor;
        let ry = my + 4 + (i as i32) * 3;
        let col = if sel { pal::P1_HI } else { pal::TEXT };
        let marker = if sel { ">" } else { " " };
        let hdr = format!("{} [{}] {}", marker, i + 1, up.name());
        cv.text_bg(mx + 3, ry, &hdr, col, pal::PANEL);
        cv.text_bg(mx + 7, ry + 1, up.desc(), if sel { pal::WHITE } else { pal::MUTED }, pal::PANEL);
    }

    cv.text_center(mx, my + mh - 2, mw, "1-3 CHOOSE \u{00b7} A/D + ENTER CONFIRM", pal::WHITE);
}

pub fn draw_gameover(cv: &mut Canvas, w: &World, l: &Layout, won: bool, best: u64) {
    let (bx, bw) = l.band();
    let py = (l.oy + l.field_h / 2 - 7).max(l.oy);
    cv.fill_rect(bx + 4, py, bw - 8, 14, ' ', pal::TEXT, pal::PANEL);
    cv.frame(bx + 4, py, bw - 8, 14, if won { pal::GOOD } else { pal::WARN });

    let (title, sub, col) = if won {
        ("SIGNAL STABILISED", "the conduit held", pal::GOOD)
    } else {
        ("SIGNAL LOST", "the cell went dark", pal::WARN)
    };

    cv.text_center(bx + 4, py + 1, bw - 8, title, col);
    cv.text_center(bx + 4, py + 2, bw - 8, sub, pal::MUTED);

    let secs = w.elapsed_secs();
    let stats = [
        format!("DIFFICULTY  {:<7}   TIME   {:02}:{:02}", w.difficulty.spec().name, secs / 60, secs % 60),
        format!("SCORE       {:>7}   BEST {:>7}", w.score, best.max(w.score)),
        format!("WAVE        {:>7}   KILLS  {:>5}", w.wave, w.kills),
        format!("BEST COMBO  {:>7}   MODS   {:>5}", w.best_combo, w.upgrades.len()),
    ];
    for (i, s) in stats.iter().enumerate() {
        cv.text_center(bx + 4, py + 4 + i as i32, bw - 8, s, pal::TEXT);
    }

    let mods_line = if w.upgrades.is_empty() {
        "UPGRADES: none installed".to_string()
    } else {
        let list: Vec<&str> = w.upgrades.iter().map(|u| u.short()).collect();
        let joined = format!("MODS: {}", list.join(" \u{00b7} "));
        let max_len = (bw - 12).max(20) as usize;
        joined.chars().take(max_len).collect()
    };
    cv.text_center(bx + 4, py + 9, bw - 8, &mods_line, pal::GOLD);

    cv.text_center(bx + 4, py + 12, bw - 8, "ENTER  run it again   ESC  title", pal::WHITE);
}

pub fn draw_too_small(cv: &mut Canvas, tw: i32, th: i32) {
    cv.clear();
    let msg = "TERMINAL TOO SMALL";
    let req = format!("need at least {}x{} (current: {}x{})", MIN_W, MIN_H, tw, th);
    cv.text_center(0, th / 2 - 1, tw, msg, pal::WARN);
    cv.text_center(0, th / 2 + 1, tw, &req, pal::MUTED);
}

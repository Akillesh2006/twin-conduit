//! Arena rendering: floor, border, conduit beam, pickups, enemies, players.

use crate::canvas::{pal, Canvas};
use crate::core::Side;
use crate::fx::Fx;
use crate::game::{EnemyKind, OrbKind, Phase, World, REST_DIST, REVIVE_TICKS};

pub fn draw_arena(cv: &mut Canvas, world: &World, fx: &Fx, ox: i32, oy: i32, t: u64) {
    let _ = fx;
    let f = world.field;
    if f.w <= 0 || f.h <= 0 {
        return;
    }
    let t = if world.phase == Phase::Lost { 0 } else { t };

    // 1. Floor
    cv.fill(ox, oy, f.w, f.h, ' ', pal::BG);
    for y in (2..f.h).step_by(4) {
        for x in (2..f.w).step_by(4) {
            if (x / 4 + y / 4) % 2 == 0 {
                cv.put(ox + x, oy + y, '\u{00b7}', pal::GRID);
            }
        }
    }

    // 2. Frame
    let (frame_col, corner_col) = if world.boss().is_some() {
        if (t / 3) % 2 == 0 {
            (pal::NULL_HI, pal::WHITE)
        } else {
            (pal::NULL, pal::NULL_HI)
        }
    } else if world.intermission > 0 {
        (pal::GOOD, pal::P1_HI)
    } else if world.hot {
        (pal::ARC_MID, pal::ARC_HOT)
    } else {
        (pal::GRID_HI, pal::MUTED)
    };

    cv.frame(ox - 1, oy - 1, f.w + 2, f.h + 2, frame_col);
    let (fx0, fy0) = (ox - 1, oy - 1);
    let (fx1, fy1) = (ox + f.w, oy + f.h);
    cv.put(fx0, fy0, '\u{250c}', corner_col);
    cv.put(fx1, fy0, '\u{2510}', corner_col);
    cv.put(fx0, fy1, '\u{2514}', corner_col);
    cv.put(fx1, fy1, '\u{2518}', corner_col);

    // 3. Conduit Beam
    let p0 = world.players[0].pos;
    let p1 = world.players[1].pos;
    let any_down = !world.players[0].alive || !world.players[1].alive;

    for (step_idx, p) in world.arc.iter().enumerate() {
        if *p == p0 || *p == p1 {
            continue;
        }
        if p.x < 0 || p.y < 0 || p.x >= f.w || p.y >= f.h {
            continue;
        }

        if any_down {
            // Downed: flickering, broken gray tether
            if (step_idx as u64 + t) % 3 != 0 {
                cv.put(ox + p.x, oy + p.y, '\u{00b7}', pal::DIM);
            }
        } else if world.dist <= REST_DIST {
            // Recharging: pulsing electric cable
            let ch = match (step_idx as u64 + t / 2) % 4 {
                0 => '=',
                1 => '~',
                2 => '-',
                _ => '~',
            };
            let col = if (step_idx as u64 + t) % 2 == 0 {
                pal::P1_HI
            } else {
                pal::GOLD
            };
            cv.put(ox + p.x, oy + p.y, ch, col);
        } else if world.hot {
            // Live blazing laser beam
            let ch = match (step_idx as u64 + t) % 4 {
                0 => '\u{2261}', // ≡
                1 => '#',
                2 => '*',
                _ => '\u{2261}',
            };
            let col = match (step_idx as u64 + t) % 3 {
                0 => pal::ARC_HOT,
                1 => pal::ARC_MID,
                _ => pal::ARC_SPARK,
            };
            cv.put(ox + p.x, oy + p.y, ch, col);
        } else {
            // Cold slack tether
            let ch = if (step_idx % 2) == 0 { '\u{00b7}' } else { '-' };
            cv.put(ox + p.x, oy + p.y, ch, pal::ARC_COLD);
        }
    }

    // 4. Orbs / Pickups
    for o in &world.orbs {
        if o.ttl < 60 && (t % 2 == 1) {
            continue;
        }
        let col = match o.kind {
            OrbKind::Charge => pal::CHARGE,
            OrbKind::Mend => pal::MEND,
        };
        cv.put(ox + o.pos.x, oy + o.pos.y, o.kind.glyph(), col);
    }

    // 5. Enemies
    for e in &world.enemies {
        let col = if e.flash > 0 {
            pal::WHITE
        } else {
            match e.kind {
                EnemyKind::Drifter => pal::FOE,
                EnemyKind::Hunter => pal::FOE_HI,
                EnemyKind::Splitter => pal::GOLD,
                EnemyKind::Bulwark => pal::FOE_ARMOR,
                EnemyKind::Null => pal::NULL_HI,
            }
        };

        cv.put(ox + e.pos.x, oy + e.pos.y, e.kind.glyph(), col);

        // Armor pips for Bulwark or Null
        if e.max_hp > 1 && e.hp < e.max_hp && e.pos.y > 0 {
            let pips = ((e.hp as f32 / e.max_hp as f32) * 3.0).ceil() as i32;
            for k in 0..pips.max(1) {
                let nx = e.pos.x - 1 + k;
                if nx >= 0 && nx < f.w {
                    cv.put(ox + nx, oy + e.pos.y - 1, '\u{2022}', pal::FOE_ARMOR);
                }
            }
        }
    }

    // 6. Players
    for p in &world.players {
        let (glyph, col) = if !p.alive {
            ('X', pal::BAD)
        } else if p.invuln > 0 && t % 2 == 1 {
            ('\u{25c9}', pal::WHITE)
        } else {
            match p.side {
                Side::One => ('\u{25c9}', pal::P1_HI),
                Side::Two => ('\u{25c9}', pal::P2_HI),
            }
        };

        cv.put(ox + p.pos.x, oy + p.pos.y, glyph, col);

        // Reviving indicator
        if !p.alive && p.revive > 0 {
            let frac = p.revive as f32 / REVIVE_TICKS as f32;
            let bar_w = (frac * 4.0).ceil() as i32;
            for bx in 0..bar_w {
                if p.pos.y > 0 && p.pos.x - 1 + bx >= 0 && p.pos.x - 1 + bx < f.w {
                    cv.put(ox + p.pos.x - 1 + bx, oy + p.pos.y - 1, '\u{2588}', pal::GOOD);
                }
            }
        }
    }
}

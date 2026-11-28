//! Arena rendering: floor, border, conduit beam, pickups, enemies, players.

use crate::canvas::{pal, Canvas};
use crate::core::{Side, Vec2};
use crate::fx::Fx;
use crate::game::{EnemyKind, OrbKind, Phase, WaveEvent, World, REST_DIST, REVIVE_TICKS};

fn blackout_hidden(world: &World, p: Vec2) -> bool {
    world.event == WaveEvent::Blackout
        && world
            .players
            .iter()
            .filter(|pl| pl.alive)
            .all(|pl| pl.pos.cheb(p) > 12)
}

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
            if (x / 4 + y / 4) % 2 == 0 && !blackout_hidden(world, Vec2::new(x, y)) {
                cv.put(ox + x, oy + y, '\u{00b7}', pal::GRID);
            }
        }
    }

    // 2. Frame (strobe warning when CHARGE is critical or tether is near snapping)
    let near_snap = world.players[0].alive
        && world.players[1].alive
        && world.dist >= world.max_snap_dist() - 4;
    let low_charge = world.charge < 25.0;
    let (frame_col, corner_col) = if (low_charge || near_snap) && (t % 2 == 0) {
        (pal::WARN, pal::WHITE)
    } else if world.boss().is_some() {
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

    // 2b. Hazard floor vents, blast warnings & Dasher telegraph lines (drawn under actors)
    for hz in &world.hazards {
        let is_active = hz.warn == 0;
        let ch = if is_active {
            if t % 2 == 0 { '\u{2593}' } else { '~' }
        } else {
            '\u{2591}'
        };
        let col = if is_active {
            pal::HAZ_HI
        } else if t % 2 == 0 {
            pal::HAZ
        } else {
            pal::GOLD
        };
        for dy in 0..hz.h {
            for dx in 0..hz.w {
                let p = Vec2::new(hz.pos.x + dx, hz.pos.y + dy);
                if p.within(f) {
                    cv.put(ox + p.x, oy + p.y, ch, col);
                }
            }
        }
    }
    for bl in &world.blasts {
        let urgent = bl.fuse <= 4 && (t % 2 == 0);
        let fill_ch = if urgent { '\u{2593}' } else { '\u{2591}' };
        let col = if urgent { pal::HAZ_HI } else { pal::HAZ };
        for dy in -bl.radius..=bl.radius {
            for dx in -bl.radius..=bl.radius {
                let cx = bl.center.x + dx;
                let cy = bl.center.y + dy;
                if cx >= 0 && cy >= 0 && cx < f.w && cy < f.h {
                    if dx == 0 && dy == 0 {
                        let digit = char::from_digit(bl.fuse.min(9), 10).unwrap_or('*');
                        cv.put(ox + cx, oy + cy, digit, pal::WHITE);
                    } else if dx.abs().max(dy.abs()) == bl.radius || urgent {
                        cv.put(ox + cx, oy + cy, fill_ch, col);
                    }
                }
            }
        }
    }
    for e in &world.enemies {
        if e.kind == EnemyKind::Dasher && e.telegraph > 0 {
            let line = crate::core::line(e.pos, e.aim);
            let col = if t % 2 == 0 { pal::WARN } else { pal::GOLD };
            for pt in line {
                if pt != e.pos && pt.within(f) {
                    cv.put(ox + pt.x, oy + pt.y, '\u{254c}', col); // ╌
                }
            }
        }
    }

    // 2c. Boss tether-cutter laser telegraphs
    for bh in &world.beam_hazards {
        let urgent = bh.fuse <= 5;
        let ch = if urgent {
            if (bh.p0.y - bh.p1.y).abs() <= 1 { '\u{2500}' } else { '\u{2502}' }
        } else {
            '\u{254c}' // ╌
        };
        let col = if urgent {
            if t % 2 == 0 { pal::WHITE } else { pal::HAZ_HI }
        } else if t % 2 == 0 {
            pal::HAZ
        } else {
            pal::BOSS_HI
        };
        for &pt in &bh.pts {
            if pt.within(f) {
                cv.put(ox + pt.x, oy + pt.y, ch, col);
            }
        }
    }

    // 2d. Patrolling obstacle walls
    for wall in &world.walls {
        for k in 0..wall.len {
            let wp = if wall.vertical {
                Vec2::new(wall.pos.x, wall.pos.y + k)
            } else {
                Vec2::new(wall.pos.x + k, wall.pos.y)
            };
            if wp.within(f) {
                let snagged = world.blocked_at == Some(wp);
                let (ch, col) = if snagged {
                    ('\u{2593}', if t % 2 == 0 { pal::WARN } else { pal::WHITE })
                } else {
                    ('\u{2588}', pal::MUTED)
                };
                cv.put(ox + wp.x, oy + wp.y, ch, col);
            }
        }
    }

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
            // Live blazing laser beam (amber-red near snap tension, golden shimmer when synced)
            let ch = match (step_idx as u64 + t) % 4 {
                0 => '\u{2261}', // ≡
                1 => '#',
                2 => '*',
                _ => '\u{2261}',
            };
            let tension = world.dist >= world.max_snap_dist() - 4;
            let col = if tension {
                match (step_idx as u64 + t) % 3 {
                    0 => pal::WARN,
                    1 => pal::HAZ_HEAVY,
                    _ => pal::WHITE,
                }
            } else if world.synced {
                match (step_idx as u64 + t) % 3 {
                    0 => pal::GOLD,
                    1 => pal::ARC_HOT,
                    _ => pal::ARC_MID,
                }
            } else {
                match (step_idx as u64 + t) % 3 {
                    0 => pal::ARC_HOT,
                    1 => pal::ARC_MID,
                    _ => pal::ARC_SPARK,
                }
            };
            cv.put(ox + p.x, oy + p.y, ch, col);
        } else {
            // Cold slack tether
            let ch = if (step_idx % 2) == 0 { '\u{00b7}' } else { '-' };
            cv.put(ox + p.x, oy + p.y, ch, pal::ARC_COLD);
        }
    }

    // Blocked beam tail & wall snag spark
    if let Some(hit) = world.blocked_at {
        if hit.within(f) {
            cv.put(ox + hit.x, oy + hit.y, '#', if t % 2 == 0 { pal::WARN } else { pal::GOLD });
        }
        for p in crate::core::line(hit, p1) {
            if p != hit && p != p1 && p.within(f) && !world.wall_at(p) {
                cv.put(ox + p.x, oy + p.y, '\u{00b7}', pal::DIM);
            }
        }
    }

    // Twin-beam special pickup parallel arc
    for (step_idx, p) in world.twin_arc().iter().enumerate() {
        if !p.within(f) {
            continue;
        }
        let ch = if (step_idx as u64 + t) % 2 == 0 { '\u{2261}' } else { '=' };
        let col = if (step_idx as u64 + t) % 2 == 0 { pal::WHITE } else { pal::P1_HI };
        cv.put(ox + p.x, oy + p.y, ch, col);
    }

    // 4. Orbs / Pickups
    for o in &world.orbs {
        if blackout_hidden(world, o.pos) && !o.kind.is_special() {
            continue;
        }
        if o.ttl < 60 && (t % 2 == 1) {
            continue;
        }
        let col = match o.kind {
            OrbKind::Charge => pal::CHARGE,
            OrbKind::Mend => pal::MEND,
            OrbKind::SlowTime => pal::ARC_MID,
            OrbKind::Nova => pal::GOLD,
            OrbKind::TwinBeam => pal::WHITE,
        };
        cv.put(ox + o.pos.x, oy + o.pos.y, o.kind.glyph(), col);
    }

    // 4b. Projectiles
    for b in &world.bullets {
        let c = b.pos.cell();
        if c.within(f) {
            let ch = if b.heavy { '\u{25cf}' } else { '*' };
            let col = if b.from_boss { pal::BOSS_HI } else { pal::HAZ_HI };
            cv.put(ox + c.x, oy + c.y, ch, col);
        }
    }

    // 5. Enemies
    for e in &world.enemies {
        if blackout_hidden(world, e.pos) && !e.kind.is_boss() && e.telegraph == 0 {
            continue;
        }
        // Directional front shield plate indicator for Shielded units
        if e.kind == EnemyKind::Shielded && world.dist < 16 {
            let d = e.dir.delta();
            let sx = e.pos.x + d.x;
            let sy = e.pos.y + d.y;
            if sx >= 0 && sy >= 0 && sx < f.w && sy < f.h {
                let plate = match e.dir {
                    crate::core::Dir::Left | crate::core::Dir::Right => '\u{2502}',
                    crate::core::Dir::Up | crate::core::Dir::Down => '\u{2500}',
                };
                let scol = if e.shield_cd > 0 { pal::WHITE } else { pal::SHIELD };
                cv.put(ox + sx, oy + sy, plate, scol);
            }
        }

        let col = if e.flash > 0 {
            pal::WHITE
        } else if e.telegraph > 0 && t % 2 == 0 {
            pal::WARN
        } else {
            match e.kind {
                EnemyKind::Drifter => pal::FOE,
                EnemyKind::Shooter => pal::HAZ,
                EnemyKind::Hunter => pal::FOE_HI,
                EnemyKind::Dasher => pal::RAPID,
                EnemyKind::Splitter => pal::GOLD,
                EnemyKind::Shielded => pal::SHIELD,
                EnemyKind::Leech => pal::BOSS_HI,
                EnemyKind::Bomber => pal::HAZ_HEAVY,
                EnemyKind::Bulwark => pal::FOE_ARMOR,
                EnemyKind::Warden => pal::GOLD,
                EnemyKind::Hydra => pal::BOSS_HI,
                EnemyKind::Null => pal::NULL_HI,
            }
        };

        cv.put(ox + e.pos.x, oy + e.pos.y, e.kind.glyph(), col);

        // Armor pips for multi-HP units
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

    // 6. Players (2x2 doubled icon so both operators stand out clearly in the arena)
    for p in &world.players {
        let col = if !p.alive {
            pal::BAD
        } else if p.invuln > 0 && t % 2 == 1 {
            pal::WHITE
        } else {
            match p.side {
                Side::One => pal::P1_HI,
                Side::Two => pal::P2_HI,
            }
        };

        let (ax, ay) = if f.w >= 2 && f.h >= 2 {
            let ax = p.pos.x.clamp(0, f.w - 2);
            let ay = p.pos.y.clamp(0, f.h - 2);
            let cells = if !p.alive {
                [('\u{2572}', '\u{2571}'), ('\u{2571}', '\u{2572}')] // ╲╱ / ╱╲
            } else {
                [('\u{259b}', '\u{259c}'), ('\u{2599}', '\u{259f}')] // ▛▜ / ▙▟
            };
            cv.put(ox + ax, oy + ay, cells[0].0, col);
            cv.put(ox + ax + 1, oy + ay, cells[0].1, col);
            cv.put(ox + ax, oy + ay + 1, cells[1].0, col);
            cv.put(ox + ax + 1, oy + ay + 1, cells[1].1, col);
            (ax, ay)
        } else {
            let glyph = if !p.alive { 'X' } else { '\u{25c9}' };
            cv.put(ox + p.pos.x, oy + p.pos.y, glyph, col);
            (p.pos.x, p.pos.y)
        };

        if p.alive && p.barrier > 0 && ay > 0 {
            cv.put(ox + ax, oy + ay - 1, '\u{25e0}', pal::SHIELD); // ◠ barrier crown
            if f.w >= 2 {
                cv.put(ox + ax + 1, oy + ay - 1, '\u{25e0}', pal::SHIELD);
            }
        }

        // Reviving indicator
        if !p.alive && p.revive > 0 && ay > 0 {
            let frac = p.revive as f32 / REVIVE_TICKS as f32;
            let bar_w = (frac * 4.0).ceil() as i32;
            for bx in 0..bar_w {
                let rx = ax - 1 + bx;
                if rx >= 0 && rx < f.w {
                    cv.put(ox + rx, oy + ay - 1, '\u{2588}', pal::GOOD);
                }
            }
        }
    }
}

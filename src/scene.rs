//! Arena rendering: floor, frame, pickups, monsters, players, tracers and enemy
//! fire.
//!
//! Art direction: the floor stays almost black so bullets pop. Enemy fire is
//! drawn *last*, on top of the players, because in a bullet hell the threat has
//! to be readable even when it overlaps you. The frame owns the mood: quiet
//! grid while a wave trickles in, boss purple when a giant is on the field, and
//! a hard strobe when that boss is enraged.

use crate::canvas::{pal, Canvas, Col};
use crate::core::Side;
use crate::fx::Fx;
use crate::game::{EnemyKind, Phase, World};

/// Mirrors `game::REVIVE_TICKS`; the ring fills over this many ticks.
const REVIVE_TICKS: u32 = 80;

pub fn draw_arena(cv: &mut Canvas, world: &World, fx: &Fx, ox: i32, oy: i32, t: u64) {
    let _ = fx;
    let f = world.field;
    if f.w <= 0 || f.h <= 0 {
        return;
    }
    // Freeze the animation clock when the run is over; `ui` dims the rest.
    let t = if world.phase == Phase::Lost { 0 } else { t };

    // ---------------------------------------------------------- 1. floor
    cv.fill(ox, oy, f.w, f.h, ' ', pal::BG);
    for y in (2..f.h).step_by(4) {
        for x in (2..f.w).step_by(4) {
            if (x / 4 + y / 4) % 2 == 0 {
                cv.put(ox + x, oy + y, '.', pal::GRID);
            }
        }
    }

    // ---------------------------------------------------------- 2. frame
    let (frame_col, corner_col) = if let Some(b) = world.boss() {
        let enraged = b.hp * 3 < b.max_hp;
        if enraged && (t / 3) % 2 == 0 {
            (pal::HAZ_HI, pal::WHITE)
        } else {
            (pal::BOSS, pal::BOSS_HI)
        }
    } else if world.intermission > 0 {
        (pal::GOOD, pal::P1_HI)
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

    // ---------------------------------------------------------- 3. pickups
    for o in &world.pickups {
        let c = o.pos.cell_in(f);
        if o.ttl < 120 && t % 2 == 1 {
            continue;
        }
        let col = match o.kind {
            crate::game::PickupKind::Heal => pal::MEND,
            crate::game::PickupKind::Rapid => pal::RAPID,
            crate::game::PickupKind::Shield => pal::SHIELD,
            crate::game::PickupKind::Evolve => pal::GOLD,
            crate::game::PickupKind::Pet => pal::GOOD,
        };
        let ch = if o.ph % 12 < 3 { '\u{00b7}' } else { o.kind.glyph() };
        cv.put(ox + c.x, oy + c.y, ch, col);
    }

    // ---------------------------------------------------------- 4. monsters
    for e in &world.enemies {
        let c = e.pos.cell_in(f);
        if e.born > 0 {
            draw_spawn_ring(cv, f, ox, oy, c.x, c.y, e.kind.is_boss(), e.born, t);
            continue;
        }
        let col = if e.flash > 0 {
            pal::WHITE
        } else {
            enemy_col(e.kind)
        };
        if e.kind.is_boss() {
            draw_boss(cv, f, ox, oy, c.x, c.y, e.kind, col, e.hp, e.max_hp, t);
            continue;
        }
        cv.put(ox + c.x, oy + c.y, e.kind.glyph(), col);
        // A row of pips over the head of anything sturdier than fodder.
        if e.max_hp > 3 && e.hp < e.max_hp && c.y > 0 {
            let pips = ((e.hp as f32 / e.max_hp as f32) * 3.0).ceil() as i32;
            for k in 0..pips.max(1) {
                let nx = c.x - 1 + k;
                if nx >= 0 && nx < f.w {
                    cv.put(ox + nx, oy + c.y - 1, '\u{2022}', pal::FOE_ARMOR);
                }
            }
        }
    }

    // ---------------------------------------------------------- 5. players
    for p in &world.players {
        let c = p.pos.cell_in(f);
        let (base, hi, lo) = match p.side {
            Side::One => (pal::P1, pal::P1_HI, pal::P1_LO),
            Side::Two => (pal::P2, pal::P2_HI, pal::P2_LO),
        };
        let (x, y) = (ox + c.x, oy + c.y);

        if !p.alive {
            cv.put2(x, y, '\u{00d7}', lo, pal::BG);
            // Revive ring: fills clockwise from above as the partner works.
            let marks = (p.revive * 4 + REVIVE_TICKS - 1) / REVIVE_TICKS;
            let ring = [(0, -1), (1, 0), (0, 1), (-1, 0)];
            for (k, (dx, dy)) in ring.iter().enumerate() {
                if k as u32 >= marks.min(4) {
                    break;
                }
                let nx = c.x + dx;
                let ny = c.y + dy;
                if nx >= 0 && ny >= 0 && nx < f.w && ny < f.h {
                    cv.put(ox + nx, oy + ny, '\u{2022}', pal::GOOD);
                }
            }
            continue;
        }

        if p.invuln > 0 && t % 2 == 1 {
            continue;
        }
        let col = if p.flash > 0 {
            pal::WHITE
        } else if p.moving {
            hi
        } else {
            base
        };
        cv.put2(x, y, '\u{25c9}', col, lo);

        // Muzzle flash: one bright speck in the direction of the last shot.
        if p.muzzle > 0 {
            let ax = c.x + if p.aim.x > 0.35 { 1 } else if p.aim.x < -0.35 { -1 } else { 0 };
            let ay = c.y + if p.aim.y > 0.35 { 1 } else if p.aim.y < -0.35 { -1 } else { 0 };
            if ax >= 0 && ay >= 0 && ax < f.w && ay < f.h {
                cv.put(ox + ax, oy + ay, '\u{2736}', pal::WHITE);
            }
        }

        // Shield bubble: four dim diamonds around the operator.
        if p.shield > 0 {
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = c.x + dx;
                let ny = c.y + dy;
                if nx >= 0 && ny >= 0 && nx < f.w && ny < f.h {
                    cv.put(ox + nx, oy + ny, '\u{00b7}', pal::SHIELD);
                }
            }
        }
    }

    // ---------------------------------------------------------- 5b. combat pets
    for pet in &world.pets {
        let c = pet.pos.cell_in(f);
        let col = match pet.level {
            1 => pal::GOOD,
            2 => pal::GOLD,
            _ => pal::WHITE,
        };
        let glyph = match pet.level {
            1 => '\u{25c7}', // ◇
            2 => '\u{2605}', // ★
            _ => '\u{2736}', // ✶
        };
        cv.put(ox + c.x, oy + c.y, glyph, col);
        // Subtle aura for Sentinel / Apex
        if pet.level >= 2 && (t / 2) % 2 == 0 {
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = c.x + dx;
                let ny = c.y + dy;
                if nx >= 0 && ny >= 0 && nx < f.w && ny < f.h {
                    cv.put(ox + nx, oy + ny, '\u{00b7}', pal::GRID_HI);
                }
            }
        }
    }

    // ---------------------------------------------------------- 6. tracers
    for s in &world.shots {
        let c = s.pos.cell_in(f);
        let col = match s.side {
            Side::One => pal::P1_HI,
            Side::Two => pal::P2_HI,
        };
        let ch = if s.vel.x.abs() > s.vel.y.abs() {
            '\u{2500}'
        } else {
            '\u{2502}'
        };
        cv.put(ox + c.x, oy + c.y, ch, col);
    }

    // ---------------------------------------------------------- 7. threat
    // Enemy fire goes on top of everything so it can never be hidden.
    for h in &world.hazards {
        let c = h.pos.cell_in(f);
        let col = match h.kind {
            crate::game::HazardKind::Pellet => pal::HAZ,
            crate::game::HazardKind::Dart => pal::HAZ_HI,
            crate::game::HazardKind::Heavy => pal::HAZ_HEAVY,
        };
        cv.put(ox + c.x, oy + c.y, h.kind.glyph(), col);
    }
}

// ------------------------------------------------------------------ helpers

fn draw_spawn_ring(
    cv: &mut Canvas,
    f: crate::core::Field,
    ox: i32,
    oy: i32,
    cx: i32,
    cy: i32,
    boss: bool,
    born: u32,
    t: u64,
) {
    let r = if boss {
        1 + (born / 14).min(2) as i32
    } else {
        1
    };
    let col = if boss { pal::BOSS_HI } else { pal::WARN };
    let n = if boss { 12 } else { 6 };
    let spin = (t as f32 * 0.2).sin();
    for i in 0..n {
        let a = std::f32::consts::TAU * i as f32 / n as f32 + spin;
        let nx = cx + (a.cos() * r as f32).round() as i32;
        let ny = cy + (a.sin() * r as f32 * 0.6).round() as i32;
        if nx >= 0 && ny >= 0 && nx < f.w && ny < f.h {
            cv.put(ox + nx, oy + ny, '\u{00b7}', col);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_boss(
    cv: &mut Canvas,
    f: crate::core::Field,
    ox: i32,
    oy: i32,
    cx: i32,
    cy: i32,
    kind: EnemyKind,
    col: Col,
    hp: i32,
    max_hp: i32,
    t: u64,
) {
    let halo = if hp * 3 < max_hp { pal::HAZ } else { pal::BOSS };
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let nx = cx + dx;
        let ny = cy + dy;
        if nx >= 0 && ny >= 0 && nx < f.w && ny < f.h {
            cv.put(ox + nx, oy + ny, '\u{2592}', halo);
        }
    }
    for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
        let nx = cx + dx;
        let ny = cy + dy;
        if nx >= 0 && ny >= 0 && nx < f.w && ny < f.h {
            cv.put(ox + nx, oy + ny, '\u{2591}', halo);
        }
    }
    cv.put2(ox + cx, oy + cy, kind.glyph(), col, pal::BG);
    if hp * 3 < max_hp && (t / 2) % 2 == 0 && cy > 0 {
        cv.put(ox + cx, oy + cy - 1, '!', pal::WHITE);
    }
}

fn enemy_col(kind: EnemyKind) -> Col {
    match kind {
        EnemyKind::Grunt => pal::FOE,
        EnemyKind::Spitter => pal::FOE_HI,
        EnemyKind::Weaver => pal::GOLD,
        EnemyKind::Bulwark => pal::FOE_ARMOR,
        EnemyKind::Swarm => pal::MUTED,
        EnemyKind::Warden => pal::BOSS,
        EnemyKind::Hydra => pal::GOLD,
        EnemyKind::Overmind => pal::BOSS_HI,
    }
}

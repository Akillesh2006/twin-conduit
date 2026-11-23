//! Particles, floating score text and screen shake.
//!
//! Everything here lives in *arena grid* coordinates. `draw` is handed the
//! arena's canvas origin (`ox`, `oy`) and the arena `Field`, so effects are
//! always clipped to the playable cells. Particles keep `f32` positions with
//! sub-cell velocity; only at draw time are they snapped to whole cells.

use crate::canvas::{pal, Canvas, Col};
use crate::core::{Field, Side, Vec2};
use crate::game::{EnemyKind, Ev, PickupKind};

/// Hard ceiling on live particles; the oldest are dropped first.
const MAX_PARTICLES: usize = 600;
/// A handful of decorative glyphs, picked by a position/life hash so a cluster
/// shimmers from frame to frame instead of looking like a solid blob.
const SPARKS: [char; 6] = ['\u{00b7}', '\u{2022}', '\u{2736}', '\u{2219}', '+', '*'];

/// One grain of feedback.
struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: u16,
    max_life: u16,
    /// Bright -> mid -> dim, indexed by remaining-life fraction in `draw`.
    ramp: [Col; 3],
}

/// Drifting text, e.g. `x7` or `+HP`.
struct FloatText {
    x: f32,
    y: f32,
    vy: f32,
    text: String,
    col: Col,
    age: u16,
    total: u16,
}

pub struct Fx {
    parts: Vec<Particle>,
    texts: Vec<FloatText>,
    /// Advances once per `step`; drives shimmer and shake holds.
    frame: u64,
    seed: u32,
}

impl Default for Fx {
    fn default() -> Self {
        Self::new()
    }
}

impl Fx {
    pub fn new() -> Self {
        Self {
            parts: Vec::with_capacity(MAX_PARTICLES),
            texts: Vec::with_capacity(16),
            frame: 0,
            seed: 0x9e37_79b9,
        }
    }

    pub fn clear(&mut self) {
        self.parts.clear();
        self.texts.clear();
    }

    // ------------------------------------------------------------ events

    pub fn push_event(&mut self, ev: &Ev, _ox: i32, _oy: i32) {
        match ev {
            Ev::Kill { at, kind, combo } => {
                let (at, kind, combo) = (*at, *kind, *combo);
                // Per-kind palette + weight: swarms pop, bosses detonate.
                let (col, alt, n, spread) = match kind {
                    EnemyKind::Swarm => (pal::MUTED, pal::FOE, 4, 1),
                    EnemyKind::Grunt => (pal::FOE, pal::FOE_HI, 8, 2),
                    EnemyKind::Spitter => (pal::FOE_HI, pal::FOE, 10, 2),
                    EnemyKind::Weaver => (pal::GOLD, pal::FOE_HI, 12, 3),
                    EnemyKind::Bulwark => (pal::FOE_ARMOR, pal::FOE, 16, 3),
                    EnemyKind::Warden => (pal::BOSS, pal::BOSS_HI, 40, 5),
                    EnemyKind::Hydra => (pal::GOLD, pal::WHITE, 50, 6),
                    EnemyKind::Overmind => (pal::BOSS_HI, pal::WHITE, 60, 7),
                };
                self.burst(at, n, col, spread);
                self.burst(at, n / 3, alt, spread + 1);
                if kind.is_boss() {
                    self.burst(at, 26, pal::WHITE, 8);
                    self.text_pop(at, "BOSS DOWN".to_string(), pal::GOOD);
                } else if combo >= 3 {
                    self.text_pop(at, format!("x{}", combo), pal::GOLD);
                }
            }
            Ev::Hit { side, at } => {
                let (side, at) = (*side, *at);
                self.burst(at, 6, pal::WARN, 2);
                self.burst(at, 3, pal::HAZ_HI, 3);
                // Side-coloured ring to make clear *who* got hit.
                self.inward_ring(at, 4, side_col(side), 1.6);
                self.text_pop(at, "-1".to_string(), pal::WARN);
            }
            Ev::Block { side, at } => {
                let (side, at) = (*side, *at);
                self.burst(at, 10, pal::SHIELD, 3);
                self.inward_ring(at, 5, pal::SHIELD, 2.0);
                self.text_pop(at, "BLOCK".to_string(), pal::SHIELD);
                let _ = side;
            }
            Ev::Down { side, at } => {
                let (side, at) = (*side, *at);
                self.burst(at, 24, pal::WHITE, 5);
                self.burst(at, 16, pal::WARN, 4);
                self.inward_ring(at, 6, side_col(side), 2.0);
                self.text_pop(at, "DOWN".to_string(), pal::WARN);
            }
            Ev::Revive { at, .. } => {
                let at = *at;
                // Rising green motes read as "coming back online".
                self.burst_bias(at, 18, pal::GOOD, 3, -0.20);
                self.inward_ring(at, 5, pal::MEND, 1.8);
                self.text_pop(at, "+LINK".to_string(), pal::GOOD);
            }
            Ev::Shoot { at, side } => {
                // A single hot speck: with two auto-cannons this fires often, so
                // keep the per-shot cost tiny.
                let at = *at;
                self.burst(at, 1, side_col(*side), 1);
            }
            Ev::EnemyShoot { at, heavy } => {
                let (at, heavy) = (*at, *heavy);
                let col = if heavy { pal::HAZ_HEAVY } else { pal::HAZ };
                self.burst(at, if heavy { 3 } else { 1 }, col, 1);
            }
            Ev::Pickup { at, kind } => {
                let (at, kind) = (*at, *kind);
                let (col, label) = match kind {
                    PickupKind::Heal => (pal::MEND, "+HP"),
                    PickupKind::Rapid => (pal::RAPID, "+RAPID"),
                    PickupKind::Shield => (pal::SHIELD, "+SHIELD"),
                    PickupKind::Evolve => (pal::GOLD, "EVOLVE!"),
                    PickupKind::Pet => (pal::GOOD, "+PET!"),
                };
                self.burst_bias(at, 8, col, 2, -0.14);
                self.text_pop(at, label.to_string(), col);
            }
            Ev::Spawn { at, kind } => {
                let at = *at;
                // Imploding ring: dim for fodder, loud for armoured / boss.
                let col = if kind.is_boss() {
                    pal::BOSS_HI
                } else {
                    match kind {
                        EnemyKind::Bulwark => pal::FOE,
                        _ => pal::MUTED,
                    }
                };
                let n = if kind.is_boss() { 12 } else { 4 };
                let r = if kind.is_boss() { 2.6 } else { 1.5 };
                self.inward_ring(at, n, col, r);
            }
            Ev::BossVolley { at, pattern } => {
                let at = *at;
                let n = 4 + (*pattern as i32) * 2;
                self.inward_ring(at, n, pal::BOSS_HI, 2.2);
            }
            // Screen-wide beats have no field here; the HUD/banners own them.
            Ev::WaveClear { .. } | Ev::WaveStart { .. } | Ev::Win | Ev::Lose => {}
        }
    }

    // -------------------------------------------------------------- tick

    pub fn step(&mut self) {
        self.frame = self.frame.wrapping_add(1);

        for p in &mut self.parts {
            p.x += p.vx;
            p.y += p.vy;
            // A little drag + a touch of gravity keeps debris from flying off
            // in straight lines; upward bursts still arc gracefully.
            p.vx *= 0.90;
            p.vy = p.vy * 0.90 + 0.03;
            p.life = p.life.saturating_sub(1);
        }
        self.parts.retain(|p| p.life > 0);

        for t in &mut self.texts {
            t.y += t.vy;
            t.vy *= 0.985;
            t.age += 1;
        }
        self.texts.retain(|t| t.age < t.total);

        self.trim();
    }

    // -------------------------------------------------------------- draw

    pub fn draw(&self, cv: &mut Canvas, ox: i32, oy: i32, field: Field, t: u64) {
        for p in &self.parts {
            let x = p.x.round() as i32;
            let y = p.y.round() as i32;
            if x < 0 || y < 0 || x >= field.w || y >= field.h {
                continue;
            }
            let frac = p.life as f32 / p.max_life.max(1) as f32;
            let col = if frac > 0.66 {
                p.ramp[0]
            } else if frac > 0.30 {
                p.ramp[1]
            } else {
                p.ramp[2]
            };
            // Hash on cell + life + frame so neighbouring grains disagree and
            // the sparkle changes between ticks.
            let h = hash(
                (x as u32).wrapping_mul(73)
                    ^ (y as u32).wrapping_mul(131)
                    ^ (t as u32).wrapping_mul(31)
                    ^ p.life as u32,
            );
            let ch = SPARKS[(h as usize) % SPARKS.len()];
            cv.put(ox + x, oy + y, ch, col);
        }

        for ft in &self.texts {
            let x0 = ft.x.round() as i32;
            let y = ft.y.round() as i32;
            if y < 0 || y >= field.h {
                continue;
            }
            let age = ft.age as f32 / ft.total.max(1) as f32;
            let col = if age < 0.55 {
                ft.col
            } else if age < 0.82 {
                pal::MUTED
            } else {
                pal::DIM
            };
            let mut x = x0;
            for ch in ft.text.chars() {
                if x >= 0 && x < field.w {
                    cv.put(ox + x, oy + y, ch, col);
                }
                x += 1;
            }
        }
    }

    // ------------------------------------------------------------- shake

    /// Subtle camera offset, held for a few frames at a time.
    pub fn shake(&self, amount: f32) -> (i32, i32) {
        if amount < 0.15 {
            return (0, 0);
        }
        let a = amount.min(2.5);
        // Weak impulses mostly round to zero; big ones saturate at one cell.
        let amp = (a - 0.12).clamp(0.0, 1.0);
        let tick = (self.frame / 3) as u32;
        let h = hash(tick ^ ((a * 64.0) as u32).wrapping_mul(0x9e37_79b9) ^ 0x5f35_6c1d);
        let nx = ((h & 0xffff) as f32 / 32767.5) - 1.0;
        let ny = (((h >> 16) & 0xffff) as f32 / 32767.5) - 1.0;
        let sx = (nx * amp * 1.8).round() as i32;
        let sy = (ny * amp * 1.8).round() as i32;
        (sx.clamp(-1, 1), sy.clamp(-1, 1))
    }

    // ----------------------------------------------------------- spawning

    /// Isotropic spark burst.
    pub fn burst(&mut self, at: Vec2, n: i32, col: Col, spread: i32) {
        self.burst_bias(at, n, col, spread, 0.0);
    }

    pub fn text_pop(&mut self, at: Vec2, text: String, col: Col) {
        // Bound the floating labels independently of the particle cap.
        if self.texts.len() >= 24 {
            self.texts.remove(0);
        }
        self.texts.push(FloatText {
            x: at.x as f32,
            y: at.y as f32,
            vy: -0.11,
            text,
            col,
            age: 0,
            total: 34,
        });
    }

    pub fn len(&self) -> usize {
        self.parts.len() + self.texts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // ------------------------------------------------------------ helpers

    fn burst_bias(&mut self, at: Vec2, n: i32, col: Col, spread: i32, bias: f32) {
        let n = n.clamp(0, 160);
        let spread = spread.clamp(1, 12) as f32;
        let ramp = ramp_for(col);
        for _ in 0..n {
            let ang = self.rndf() * std::f32::consts::TAU;
            let spd = (0.16 + self.rndf() * 0.40) * (0.55 + spread * 0.18);
            let life = 10 + (self.rand() % 14) as u16;
            let px = at.x as f32 + (self.rndf() - 0.5) * 0.5;
            let py = at.y as f32 + (self.rndf() - 0.5) * 0.5;
            let vx = ang.cos() * spd;
            let vy = ang.sin() * spd * 0.75 + bias;
            self.parts.push(Particle {
                x: px,
                y: py,
                vx,
                vy,
                life,
                max_life: life,
                ramp,
            });
        }
        self.trim();
    }

    fn inward_ring(&mut self, at: Vec2, n: i32, col: Col, radius: f32) {
        let n = n.clamp(0, 24);
        let ramp = ramp_for(col);
        for i in 0..n {
            let ang = std::f32::consts::TAU * i as f32 / n.max(1) as f32
                + (self.rndf() - 0.5) * 0.5;
            let life = 7 + (self.rand() % 6) as u16;
            self.parts.push(Particle {
                x: at.x as f32 + ang.cos() * radius,
                y: at.y as f32 + ang.sin() * radius * 0.6,
                vx: -ang.cos() * 0.22,
                vy: -ang.sin() * 0.13,
                life,
                max_life: life,
                ramp,
            });
        }
        self.trim();
    }

    fn trim(&mut self) {
        if self.parts.len() > MAX_PARTICLES {
            let excess = self.parts.len() - MAX_PARTICLES;
            self.parts.drain(..excess);
        }
    }

    /// Tiny xorshift so the effects are self-contained and allocation-free.
    fn rand(&mut self) -> u32 {
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        x
    }

    fn rndf(&mut self) -> f32 {
        (self.rand() >> 8) as f32 / 16_777_216.0
    }
}

fn side_col(side: Side) -> Col {
    match side {
        Side::One => pal::P1,
        Side::Two => pal::P2,
    }
}

/// Three-stop brightness ramp anchored on palette colours only.
fn ramp_for(c: Col) -> [Col; 3] {
    match c {
        pal::FOE => [pal::FOE_HI, pal::FOE, pal::DIM],
        pal::FOE_HI => [pal::WHITE, pal::FOE_HI, pal::FOE],
        pal::FOE_ARMOR => [pal::WHITE, pal::FOE_ARMOR, pal::DIM],
        pal::BOSS => [pal::BOSS_HI, pal::BOSS, pal::DIM],
        pal::BOSS_HI => [pal::WHITE, pal::BOSS_HI, pal::BOSS],
        pal::HAZ => [pal::HAZ_HI, pal::HAZ, pal::BAD],
        pal::HAZ_HI => [pal::WHITE, pal::HAZ_HI, pal::HAZ],
        pal::HAZ_HEAVY => [pal::WHITE, pal::HAZ_HEAVY, pal::BAD],
        pal::GOLD => [pal::WHITE, pal::GOLD, pal::MUTED],
        pal::WARN => [pal::WHITE, pal::WARN, pal::BAD],
        pal::WHITE => [pal::WHITE, pal::ARC_SPARK, pal::MUTED],
        pal::ARC_SPARK => [pal::WHITE, pal::ARC_SPARK, pal::ARC_COLD],
        pal::GOOD => [pal::WHITE, pal::GOOD, pal::DIM],
        pal::CHARGE => [pal::WHITE, pal::CHARGE, pal::ARC_COLD],
        pal::MEND => [pal::WHITE, pal::MEND, pal::DIM],
        pal::SHIELD => [pal::WHITE, pal::SHIELD, pal::P1_LO],
        pal::RAPID => [pal::WHITE, pal::RAPID, pal::GOLD],
        pal::P1 => [pal::P1_HI, pal::P1, pal::P1_LO],
        pal::P2 => [pal::P2_HI, pal::P2, pal::P2_LO],
        other => [other, other, pal::DIM],
    }
}

/// 32-bit integer hash (integer fmix-style avalanche). Deterministic, cheap.
fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

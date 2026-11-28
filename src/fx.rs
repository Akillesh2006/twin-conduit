//! Particles, floating score text and screen shake.

use crate::canvas::{pal, Canvas, Col};
use crate::core::{Field, Side, Vec2};
use crate::game::{EnemyKind, Ev, OrbKind};

const MAX_PARTICLES: usize = 600;
const SPARKS: [char; 6] = ['\u{00b7}', '\u{2022}', '\u{2736}', '\u{2219}', '+', '*'];

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: u16,
    max_life: u16,
    ramp: [Col; 3],
}

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
                let (col, alt, n, spread) = match kind {
                    EnemyKind::Drifter => (pal::FOE, pal::FOE_HI, 6, 2),
                    EnemyKind::Shooter => (pal::HAZ, pal::HAZ_HI, 10, 2),
                    EnemyKind::Hunter => (pal::FOE_HI, pal::WARN, 8, 2),
                    EnemyKind::Dasher => (pal::RAPID, pal::WHITE, 11, 3),
                    EnemyKind::Splitter => (pal::GOLD, pal::FOE, 12, 3),
                    EnemyKind::Shielded => (pal::SHIELD, pal::WHITE, 14, 3),
                    EnemyKind::Leech => (pal::BOSS_HI, pal::WARN, 14, 3),
                    EnemyKind::Bomber => (pal::HAZ_HEAVY, pal::WARN, 14, 3),
                    EnemyKind::Bulwark => (pal::FOE_ARMOR, pal::FOE, 16, 3),
                    EnemyKind::Warden => (pal::GOLD, pal::WARN, 32, 5),
                    EnemyKind::Hydra => (pal::BOSS, pal::BOSS_HI, 38, 5),
                    EnemyKind::Null => (pal::NULL, pal::NULL_HI, 45, 6),
                };
                self.burst(at, n, col, spread);
                self.burst(at, n / 3, alt, spread + 1);
                if kind.is_boss() {
                    self.burst(at, 26, pal::WHITE, 8);
                    let msg = match kind {
                        EnemyKind::Warden => "WARDEN SHATTERED",
                        EnemyKind::Hydra => "HYDRA SEVERED",
                        _ => "NULL SEVERED",
                    };
                    self.text_pop(at, msg.to_string(), pal::GOOD);
                } else if combo >= 5 && combo % 5 == 0 {
                    self.burst(at, 12, pal::GOLD, 4);
                    self.text_pop(at, format!("x{} SURGE!", combo), pal::GOLD);
                } else if combo >= 3 {
                    self.text_pop(at, format!("x{}", combo), pal::GOLD);
                }
            }
            Ev::Hit { side, at } => {
                let (side, at) = (*side, *at);
                self.burst(at, 6, pal::WARN, 2);
                self.inward_ring(at, 4, side_col(side), 1.6);
                self.text_pop(at, "-1".to_string(), pal::WARN);
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
                self.burst_bias(at, 18, pal::GOOD, 3, -0.20);
                self.inward_ring(at, 5, pal::MEND, 1.8);
                self.text_pop(at, "+LINK".to_string(), pal::GOOD);
            }
            Ev::Snap { at } => {
                let at = *at;
                self.burst(at, 30, pal::WHITE, 6);
                self.burst(at, 20, pal::WARN, 5);
                self.text_pop(at, "SNAP!".to_string(), pal::WARN);
            }
            Ev::Pickup { at, kind } => {
                let (at, kind) = (*at, *kind);
                let (col, label) = match kind {
                    OrbKind::Charge => (pal::CHARGE, "+CHARGE"),
                    OrbKind::Mend => (pal::MEND, "+HP"),
                    OrbKind::SlowTime => (pal::ARC_MID, "SLOW TIME"),
                    OrbKind::Nova => (pal::GOLD, "NOVA!"),
                    OrbKind::TwinBeam => (pal::P1_HI, "TWIN BEAM"),
                };
                self.burst_bias(at, 10, col, 2, -0.14);
                self.text_pop(at, label.to_string(), col);
            }
            Ev::ChainArc { from, to } => {
                self.burst(*from, 4, pal::P1_HI, 2);
                self.burst(*to, 6, pal::ARC_SPARK, 2);
            }
            Ev::Spawn { at } => {
                self.inward_ring(*at, 4, pal::MUTED, 1.5);
            }
            Ev::Shoot { at } => {
                self.burst(*at, 5, pal::HAZ_HI, 2);
            }
            Ev::Dash { at } => {
                self.burst(*at, 8, pal::RAPID, 3);
                self.text_pop(*at, ">>".to_string(), pal::WARN);
            }
            Ev::Block { at } => {
                self.burst(*at, 6, pal::SHIELD, 2);
                self.text_pop(*at, "SHIELD".to_string(), pal::SHIELD);
            }
            Ev::Explode { at, radius } => {
                self.burst(*at, (*radius as usize) * 14, pal::HAZ_HEAVY, radius + 3);
                self.burst(*at, (*radius as usize) * 8, pal::GOLD, radius + 2);
                self.text_pop(*at, "BOOM!".to_string(), pal::WARN);
            }
            Ev::LeechDrain { at } => {
                self.burst(*at, 3, pal::BOSS_HI, 1);
            }
            Ev::PhaseShift { at, phase, .. } => {
                self.burst(*at, 24, pal::BOSS_HI, 5);
                self.inward_ring(*at, 10, pal::GOLD, 3.0);
                self.text_pop(*at, format!("PHASE {}", phase), pal::GOLD);
            }
            Ev::TetherCut { at } => {
                self.burst(*at, 22, pal::HAZ_HI, 5);
                self.text_pop(*at, "CUT! -10%".to_string(), pal::HAZ_HI);
            }
            Ev::WaveClear { .. } | Ev::WaveStart { .. } | Ev::Win | Ev::Lose => {}
        }
    }

    // -------------------------------------------------------------- tick

    pub fn step(&mut self) {
        self.frame = self.frame.wrapping_add(1);

        for p in &mut self.parts {
            p.x += p.vx;
            p.y += p.vy;
            p.life = p.life.saturating_sub(1);
        }
        self.parts.retain(|p| p.life > 0);

        for t in &mut self.texts {
            t.y += t.vy;
            t.age += 1;
        }
        self.texts.retain(|t| t.age < t.total);
    }

    pub fn shake(&self, shake_ticks: u32) -> (i32, i32) {
        if shake_ticks == 0 {
            return (0, 0);
        }
        let s = self.frame.wrapping_mul(2654435761);
        let dx = match (s >> 3) & 3 {
            0 => -1,
            1 => 1,
            _ => 0,
        };
        let dy = match (s >> 5) & 3 {
            0 => 1,
            _ => 0,
        };
        (dx, dy)
    }

    // -------------------------------------------------------------- draw

    pub fn draw(&self, cv: &mut Canvas, f: Field, ox: i32, oy: i32) {
        for p in &self.parts {
            let px = p.x.round() as i32;
            let py = p.y.round() as i32;
            if px < 0 || py < 0 || px >= f.w || py >= f.h {
                continue;
            }
            let frac = p.life as f32 / p.max_life.max(1) as f32;
            let col = if frac > 0.60 {
                p.ramp[0]
            } else if frac > 0.25 {
                p.ramp[1]
            } else {
                p.ramp[2]
            };
            let glyph_idx = ((p.x as i32 * 7 + p.y as i32 * 13 + p.life as i32) as usize) % SPARKS.len();
            cv.put(ox + px, oy + py, SPARKS[glyph_idx], col);
        }

        for t in &self.texts {
            let tx = t.x.round() as i32;
            let ty = t.y.round() as i32;
            if ty < 0 || ty >= f.h {
                continue;
            }
            cv.text(ox + tx, oy + ty, &t.text, t.col);
        }
    }

    // ------------------------------------------------------------- spawners

    fn rng_f(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.seed >> 8) as f32 / 16777216.0
    }

    pub fn burst(&mut self, at: Vec2, count: usize, col: Col, spread: i32) {
        for _ in 0..count {
            if self.parts.len() >= MAX_PARTICLES {
                break;
            }
            let ang = self.rng_f() * std::f32::consts::TAU;
            let spd = 0.20 + self.rng_f() * (spread as f32 * 0.28);
            let life = 6 + (self.rng_f() * 12.0) as u16;
            self.parts.push(Particle {
                x: at.x as f32,
                y: at.y as f32,
                vx: ang.cos() * spd,
                vy: ang.sin() * spd * 0.5,
                life,
                max_life: life,
                ramp: [pal::WHITE, col, pal::DIM],
            });
        }
    }

    pub fn burst_bias(&mut self, at: Vec2, count: usize, col: Col, spread: i32, vy_bias: f32) {
        for _ in 0..count {
            if self.parts.len() >= MAX_PARTICLES {
                break;
            }
            let ang = self.rng_f() * std::f32::consts::TAU;
            let spd = 0.15 + self.rng_f() * (spread as f32 * 0.22);
            let life = 8 + (self.rng_f() * 14.0) as u16;
            self.parts.push(Particle {
                x: at.x as f32,
                y: at.y as f32,
                vx: ang.cos() * spd,
                vy: ang.sin() * spd * 0.5 + vy_bias,
                life,
                max_life: life,
                ramp: [pal::WHITE, col, pal::DIM],
            });
        }
    }

    pub fn inward_ring(&mut self, at: Vec2, count: usize, col: Col, radius: f32) {
        for i in 0..count {
            if self.parts.len() >= MAX_PARTICLES {
                break;
            }
            let ang = (i as f32 / count as f32) * std::f32::consts::TAU;
            let dist = radius * (0.8 + self.rng_f() * 0.4);
            let life = 8 + (self.rng_f() * 6.0) as u16;
            let sx = at.x as f32 + ang.cos() * dist;
            let sy = at.y as f32 + ang.sin() * dist * 0.5;
            self.parts.push(Particle {
                x: sx,
                y: sy,
                vx: -ang.cos() * (dist / life as f32),
                vy: -ang.sin() * (dist / life as f32) * 0.5,
                life,
                max_life: life,
                ramp: [col, col, pal::DIM],
            });
        }
    }

    pub fn text_pop(&mut self, at: Vec2, text: String, col: Col) {
        let len = text.chars().count() as i32;
        self.texts.push(FloatText {
            x: at.x as f32 - (len / 2) as f32,
            y: at.y as f32 - 1.0,
            vy: -0.16,
            text,
            col,
            age: 0,
            total: 22,
        });
    }
}

fn side_col(side: Side) -> Col {
    match side {
        Side::One => pal::P1_HI,
        Side::Two => pal::P2_HI,
    }
}

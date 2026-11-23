//! OVERDRIVE simulation.
//!
//! Two operators share one keyboard, each carrying an automatic cannon. The
//! cannons find their targets themselves, so the players' whole job is movement:
//! stay alive inside a storm of enemy fire and let the guns do the work.
//!
//! Design pillars:
//!
//! * **Auto-fire** means both hands are free for dodging. The player expresses
//!   skill through positioning, not aiming.
//! * **Fair bullet patterns.** Enemy fire is aimed, fanned or ring-shaped, and
//!   every ring leaves a deliberate gap. Density rises per wave; randomness
//!   never decides whether you live.
//! * **A boss every five waves.** Bosses run a small multi-armed-bandit over
//!   their attack patterns: a pattern that keeps drawing blood gets picked more
//!   often, one that players keep reading gets faded out. This is the lightweight
//!   online-learning layer (see `Adaptive`).
//! * **Dynamic pressure.** The director watches how comfortably the players are
//!   dodging and nudges bullet speed / fire cadence between "learning" and
//!   "overwhelmed", the classic DDA loop from the research in `docs/RESEARCH.md`.

use crate::core::{Dir, Field, Side, Vec2, Vec2f};
use crate::rng::Rng;

pub const TICK_MS: u64 = 60;

pub const TOTAL_WAVES: u32 = 15;
pub const BOSS_EVERY: u32 = 5;
pub const MAX_HP: i32 = 5;

pub const PLAYER_SPEED: f32 = 0.58;
pub const PLAYER_R: f32 = 0.38;
pub const INVULN: u32 = 55;
pub const REVIVE_TICKS: u32 = 80;

pub const GUN_CD: u32 = 10;
pub const GUN_CD_RAPID: u32 = 5;
pub const SHOT_SPEED: f32 = 1.05;
pub const SHOT_TTL: u32 = 70;

pub const MAX_ENEMIES: usize = 42;
pub const MAX_HAZARDS: usize = 520;

/// How far off the arena edge bullets travel before being culled.
const CULL_MARGIN: f32 = 6.0;

// --------------------------------------------------------------------- kinds

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyKind {
    Grunt,
    Spitter,
    Weaver,
    Bulwark,
    Swarm,
    Warden,
    Hydra,
    Overmind,
}

impl EnemyKind {
    pub fn glyph(self) -> char {
        match self {
            EnemyKind::Grunt => '\u{00d7}',   // ×
            EnemyKind::Spitter => '\u{25bc}', // ▼
            EnemyKind::Weaver => '\u{2739}',  // ✹
            EnemyKind::Bulwark => '\u{25a0}', // ■
            EnemyKind::Swarm => '\u{2219}',   // ∙
            EnemyKind::Warden => '\u{25c8}',  // ◈
            EnemyKind::Hydra => '\u{25a3}',   // ▣
            EnemyKind::Overmind => '\u{25c9}',// ◉
        }
    }

    pub fn hp(self) -> i32 {
        match self {
            EnemyKind::Grunt => 2,
            EnemyKind::Spitter => 3,
            EnemyKind::Weaver => 3,
            EnemyKind::Bulwark => 7,
            EnemyKind::Swarm => 1,
            EnemyKind::Warden => 45,
            EnemyKind::Hydra => 100,
            EnemyKind::Overmind => 180,
        }
    }

    pub fn points(self) -> u64 {
        match self {
            EnemyKind::Grunt => 10,
            EnemyKind::Spitter => 16,
            EnemyKind::Weaver => 20,
            EnemyKind::Bulwark => 40,
            EnemyKind::Swarm => 6,
            EnemyKind::Warden => 900,
            EnemyKind::Hydra => 1800,
            EnemyKind::Overmind => 3200,
        }
    }

    pub fn contact(self) -> i32 {
        match self {
            EnemyKind::Bulwark => 2,
            EnemyKind::Warden | EnemyKind::Hydra | EnemyKind::Overmind => 2,
            _ => 1,
        }
    }

    pub fn radius(self) -> f32 {
        match self {
            EnemyKind::Warden | EnemyKind::Hydra | EnemyKind::Overmind => 1.7,
            EnemyKind::Bulwark => 0.55,
            EnemyKind::Swarm => 0.32,
            _ => 0.45,
        }
    }

    pub fn is_boss(self) -> bool {
        matches!(
            self,
            EnemyKind::Warden | EnemyKind::Hydra | EnemyKind::Overmind
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            EnemyKind::Grunt => "GRUNT",
            EnemyKind::Spitter => "SPITTER",
            EnemyKind::Weaver => "WEAVER",
            EnemyKind::Bulwark => "BULWARK",
            EnemyKind::Swarm => "SWARM",
            EnemyKind::Warden => "THE WARDEN",
            EnemyKind::Hydra => "HYDRA",
            EnemyKind::Overmind => "OVERMIND",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HazardKind {
    /// Small aimed pellet.
    Pellet,
    /// Fast dart, harder to read but thinner.
    Dart,
    /// Big slow shell.
    Heavy,
}

impl HazardKind {
    pub fn glyph(self) -> char {
        match self {
            HazardKind::Pellet => '\u{2022}', // •
            HazardKind::Dart => '\u{2217}',   // ∗
            HazardKind::Heavy => '\u{25cf}',  // ●
        }
    }

    pub fn radius(self) -> f32 {
        match self {
            HazardKind::Pellet => 0.24,
            HazardKind::Dart => 0.20,
            HazardKind::Heavy => 0.38,
        }
    }

    pub fn damage(self) -> i32 {
        match self {
            HazardKind::Heavy => 2,
            _ => 1,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickupKind {
    Heal,
    Rapid,
    Shield,
    Evolve,
    Pet,
}

impl PickupKind {
    pub fn glyph(self) -> char {
        match self {
            PickupKind::Heal => '\u{271a}',   // ✚
            PickupKind::Rapid => '\u{2726}',  // ✦
            PickupKind::Shield => '\u{25c7}', // ◇
            PickupKind::Evolve => '\u{25b2}', // ▲
            PickupKind::Pet => '\u{2605}',    // ★
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PickupKind::Heal => "+HP",
            PickupKind::Rapid => "RAPID",
            PickupKind::Shield => "SHIELD",
            PickupKind::Evolve => "EVOLVE",
            PickupKind::Pet => "PET",
        }
    }
}

// -------------------------------------------------------------------- structs

#[derive(Clone, Copy, Debug)]
pub struct Player {
    pub side: Side,
    pub pos: Vec2f,
    pub vel: Vec2f,
    pub alive: bool,
    pub hp: i32,
    pub invuln: u32,
    pub flash: u32,
    pub muzzle: u32,
    pub aim: Vec2f,
    pub gun_cd: u32,
    pub rapid: u32,
    pub shield: u32,
    pub revive: u32,
    pub facing: Dir,
    pub moving: bool,
    pub evolution: u32,
}

impl Player {
    pub fn down(&self) -> bool {
        !self.alive
    }

    pub fn rate(&self) -> u32 {
        if self.rapid > 0 {
            GUN_CD_RAPID
        } else {
            GUN_CD
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pet {
    pub pos: Vec2f,
    pub angle: f32,
    pub level: u32, // 1: Scout, 2: Sentinel, 3: Apex Overdrive
    pub fire_cd: u32,
    pub side: Side,
}

impl Pet {
    pub fn name(&self) -> &'static str {
        match self.level {
            1 => "SCOUT",
            2 => "SENTINEL",
            _ => "APEX",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Enemy {
    pub pos: Vec2f,
    pub kind: EnemyKind,
    pub hp: i32,
    pub max_hp: i32,
    pub flash: u32,
    /// Fire timer (bosses: volley timer).
    pub cd: u32,
    /// Animation / pattern phase.
    pub phase: u32,
    pub born: u32,
    pub strafe: f32,
    pub radius: f32,
    /// How many volleys remain in the boss's current pattern.
    pub aux: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub pos: Vec2f,
    pub vel: Vec2f,
    pub ttl: u32,
    pub side: Side,
}

#[derive(Clone, Copy, Debug)]
pub struct Hazard {
    pub pos: Vec2f,
    pub vel: Vec2f,
    pub ttl: u32,
    pub kind: HazardKind,
    pub r: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Pickup {
    pub pos: Vec2f,
    pub kind: PickupKind,
    pub ttl: u32,
    pub ph: u32,
}

/// Online difficulty director plus the boss's pattern bandit.
///
/// `comfort` rises while the players dodge cleanly and drops when they eat
/// damage. `pressure` follows it and drives enemy cadence, bullet speed and the
/// amount of predictive lead enemy shots use. The three `weights` are the
/// bandit arms for boss patterns.
#[derive(Clone, Debug)]
pub struct Adaptive {
    pub comfort: f32,
    pub pressure: f32,
    pub lead: f32,
    /// -1..1 EMA of how players tend to strafe away from incoming fire.
    pub dodge: f32,
    pub weights: [f32; 3],
    pub pattern: usize,
    pub pattern_hits: u32,
    pub hits_taken: u32,
    pub near_misses: u32,
}

impl Default for Adaptive {
    fn default() -> Self {
        Self {
            comfort: 0.5,
            pressure: 0.5,
            lead: 0.3,
            dodge: 0.0,
            weights: [1.0, 1.0, 1.0],
            pattern: 0,
            pattern_hits: 0,
            hits_taken: 0,
            near_misses: 0,
        }
    }
}

impl Adaptive {
    fn tick(&mut self, wave: u32) {
        // Recovery is slow: about 20 seconds of clean play per comfort point.
        self.comfort = (self.comfort + 0.0006).min(1.0);
        let target = (0.30 + self.comfort * 0.55 + wave as f32 * 0.012).clamp(0.24, 1.0);
        self.pressure += (target - self.pressure) * 0.02;
        self.lead = (0.12 + self.pressure * 0.55).min(0.8);
    }

    fn hit(&mut self) {
        self.comfort = (self.comfort - 0.11).max(0.0);
        self.pressure = (self.pressure * 0.93).max(0.18);
        self.hits_taken += 1;
    }

    fn kill(&mut self) {
        self.comfort = (self.comfort + 0.006).min(1.0);
    }

    fn watch_dodge(&mut self, lateral: f32) {
        self.dodge = (self.dodge * 0.96 + lateral * 0.04).clamp(-1.0, 1.0);
    }

    fn scored(&mut self) {
        self.pattern_hits += 1;
    }

    /// Pick the next boss pattern with probability proportional to its weight.
    fn choose_pattern(&mut self, rng: &mut Rng) -> usize {
        // Settle the arm we just finished: patterns that landed hits get
        // reinforced, patterns the players kept dodging decay toward the floor.
        let i = self.pattern;
        let reward = self.pattern_hits as f32;
        self.weights[i] = (self.weights[i] * 0.84 + 0.16 * (1.0 + reward * 0.55)).clamp(0.35, 4.0);
        self.pattern_hits = 0;

        let total: f32 = self.weights.iter().sum();
        let mut roll = rng.f32() * total;
        let mut pick = 2;
        for (k, w) in self.weights.iter().enumerate() {
            roll -= *w;
            if roll <= 0.0 {
                pick = k;
                break;
            }
        }
        self.pattern = pick;
        pick
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Playing,
    Won,
    Lost,
}

#[derive(Clone, Debug)]
pub enum Ev {
    Kill { at: Vec2, kind: EnemyKind, combo: u32 },
    Hit { side: Side, at: Vec2 },
    Block { side: Side, at: Vec2 },
    Down { side: Side, at: Vec2 },
    Revive { side: Side, at: Vec2 },
    Shoot { at: Vec2, side: Side },
    EnemyShoot { at: Vec2, heavy: bool },
    Pickup { at: Vec2, kind: PickupKind },
    Spawn { at: Vec2, kind: EnemyKind },
    BossVolley { at: Vec2, pattern: usize },
    WaveClear { wave: u32, bonus: u64 },
    WaveStart { wave: u32 },
    Win,
    Lose,
}

pub struct World {
    pub field: Field,
    pub players: [Player; 2],
    pub pets: Vec<Pet>,
    pub enemies: Vec<Enemy>,
    pub shots: Vec<Shot>,
    pub hazards: Vec<Hazard>,
    pub pickups: Vec<Pickup>,
    pub phase: Phase,
    pub wave: u32,
    pub score: u64,
    pub kills: u32,
    pub combo: u32,
    pub combo_t: u32,
    pub best_combo: u32,
    pub queued: Vec<EnemyKind>,
    pub spawn_cd: u32,
    pub intermission: u32,
    pub t: u64,
    pub shake: f32,
    pub banner: Option<(String, u32)>,
    pub adaptive: Adaptive,
    pub shots_fired: u32,
    pub shots_hit: u32,
    pub rng: Rng,
    prev_dir: [Option<Dir>; 2],
}

impl World {
    pub fn new(field: Field) -> Self {
        let field = Field::new(field.w, field.h);
        let c = field.center();
        let p1 = Player {
            side: Side::One,
            pos: Vec2f::new((c.x - 6) as f32, c.y as f32),
            vel: Vec2f::default(),
            alive: true,
            hp: MAX_HP,
            invuln: 70,
            flash: 0,
            muzzle: 0,
            aim: Vec2f::new(1.0, 0.0),
            gun_cd: 12,
            rapid: 0,
            shield: 0,
            revive: 0,
            facing: Dir::Right,
            moving: false,
            evolution: 0,
        };
        let p2 = Player {
            side: Side::Two,
            pos: Vec2f::new((c.x + 6) as f32, c.y as f32),
            aim: Vec2f::new(-1.0, 0.0),
            facing: Dir::Left,
            ..p1
        };
        Self {
            field,
            players: [p1, p2],
            pets: Vec::new(),
            enemies: Vec::new(),
            shots: Vec::new(),
            hazards: Vec::new(),
            pickups: Vec::new(),
            phase: Phase::Playing,
            wave: 1,
            score: 0,
            kills: 0,
            combo: 0,
            combo_t: 0,
            best_combo: 0,
            queued: wave_spec(1),
            spawn_cd: 24,
            intermission: 0,
            t: 0,
            shake: 0.0,
            banner: Some(("WAVE 01 \u{00b7} CONTACT".into(), 120)),
            adaptive: Adaptive::default(),
            shots_fired: 0,
            shots_hit: 0,
            rng: Rng::from_time(),
            prev_dir: [None, None],
        }
    }

    // ----------------------------------------------------------- public info

    pub fn score_mult(&self) -> u64 {
        1 + self.combo.min(9) as u64
    }

    pub fn alive_count(&self) -> usize {
        self.players.iter().filter(|p| p.alive).count()
    }

    pub fn boss(&self) -> Option<&Enemy> {
        self.enemies.iter().find(|e| e.kind.is_boss())
    }

    pub fn boss_progress(&self) -> Option<(f32, &'static str)> {
        self.boss()
            .map(|e| (e.hp as f32 / e.max_hp.max(1) as f32, e.kind.name()))
    }

    pub fn accuracy(&self) -> f32 {
        if self.shots_fired == 0 {
            0.0
        } else {
            (self.shots_hit as f32 / self.shots_fired as f32 * 100.0).min(100.0)
        }
    }

    pub fn wave_is_boss(&self, wave: u32) -> bool {
        wave % BOSS_EVERY == 0
    }

    fn banner(&mut self, s: impl Into<String>, ticks: u32) {
        self.banner = Some((s.into(), ticks));
    }

    // ---------------------------------------------------------------- tick

    pub fn step(&mut self, dirs: [Option<Dir>; 2]) -> Vec<Ev> {
        let mut ev = Vec::new();
        self.t += 1;

        if self.phase != Phase::Playing {
            self.shake *= 0.90;
            if let Some((_, n)) = &mut self.banner {
                *n = n.saturating_sub(1);
            }
            return ev;
        }

        self.shake *= 0.86;
        if self.shake < 0.01 {
            self.shake = 0.0;
        }
        if let Some((_, n)) = &mut self.banner {
            *n = n.saturating_sub(1);
            if *n == 0 {
                self.banner = None;
            }
        }

        self.adaptive.tick(self.wave);

        self.move_players(dirs);
        self.auto_fire(&mut ev);
        self.update_pets(&mut ev);
        self.update_shots(&mut ev);
        self.revive_partners(&mut ev);

        if self.combo_t > 0 {
            self.combo_t -= 1;
            if self.combo_t == 0 {
                self.combo = 0;
            }
        }

        self.update_enemies(&mut ev);
        self.update_hazards(&mut ev);
        self.contact_damage(&mut ev);
        if self.phase != Phase::Playing {
            return ev;
        }
        self.update_pickups(&mut ev);
        self.update_waves(&mut ev);

        ev
    }

    // ------------------------------------------------------------- movement

    fn move_players(&mut self, dirs: [Option<Dir>; 2]) {
        for i in 0..2 {
            let p = &mut self.players[i];
            p.invuln = p.invuln.saturating_sub(1);
            p.flash = p.flash.saturating_sub(1);
            p.muzzle = p.muzzle.saturating_sub(1);
            p.rapid = p.rapid.saturating_sub(1);
            p.shield = p.shield.saturating_sub(1);

            if !p.alive {
                p.moving = false;
                p.vel = Vec2f::default();
                continue;
            }

            let before = p.pos;
            match dirs[i] {
                Some(d) => {
                    p.facing = d;
                    p.moving = true;
                    let delta = Vec2f::new(d.delta().x as f32, d.delta().y as f32);
                    p.pos = (p.pos + delta.scale(PLAYER_SPEED)).clamp_to(self.field);
                }
                None => p.moving = false,
            }
            p.vel = p.pos - before;
            let _ = &self.prev_dir;
            self.prev_dir[i] = dirs[i];
        }
    }

    // ------------------------------------------------------------- shooting

    fn auto_fire(&mut self, ev: &mut Vec<Ev>) {
        for i in 0..2 {
            if !self.players[i].alive {
                continue;
            }
            let p = &mut self.players[i];
            if p.gun_cd > 0 {
                p.gun_cd -= 1;
                continue;
            }

            let from = p.pos;
            let target = self
                .enemies
                .iter()
                .filter(|e| e.born == 0)
                .map(|e| (e.pos, e.radius))
                .min_by(|a, b| {
                    a.0.dist(from)
                        .partial_cmp(&b.0.dist(from))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

            p.aim = match target {
                Some((tp, _)) => (tp - from).norm(),
                None => Vec2f::new(p.facing.delta().x as f32, p.facing.delta().y as f32),
            };

            if let Some((tp, _)) = target {
                let to = tp - from;
                if to.len() > 30.0 {
                    p.gun_cd = 6;
                    continue;
                }
                let evo = p.evolution;
                let count = match evo {
                    0 => 1,
                    1 => 2,
                    _ => 3,
                };
                let shot_spd = if evo >= 3 { SHOT_SPEED * 1.25 } else { SHOT_SPEED };
                for k in 0..count {
                    let angle_offset = match (evo, k) {
                        (1, 0) => -0.06,
                        (1, 1) => 0.06,
                        (2, 0) => -0.14,
                        (2, 1) => 0.0,
                        (2, 2) => 0.14,
                        (3, 0) => -0.10,
                        (3, 1) => 0.0,
                        (3, 2) => 0.10,
                        _ => 0.0,
                    };
                    let jitter = (self.rng.f32() - 0.5) * 0.04;
                    let aim = rotate(p.aim, angle_offset + jitter);
                    let origin = from + aim.scale(0.7);
                    if self.shots.len() < 240 {
                        self.shots.push(Shot {
                            pos: origin,
                            vel: aim.scale(shot_spd),
                            ttl: SHOT_TTL,
                            side: p.side,
                        });
                        self.shots_fired += 1;
                    }
                }
                p.muzzle = 3;
                p.gun_cd = p.rate();
                ev.push(Ev::Shoot {
                    at: from.cell(),
                    side: p.side,
                });
            } else {
                p.gun_cd = 6;
            }
        }
    }

    // ----------------------------------------------------------------- pets

    fn update_pets(&mut self, ev: &mut Vec<Ev>) {
        if self.pets.is_empty() {
            return;
        }
        let nearest_enemy = self
            .enemies
            .iter()
            .filter(|e| e.born == 0)
            .map(|e| (e.pos, e.radius))
            .min_by(|a, b| {
                let da = a.0.dist(self.players[0].pos).min(a.0.dist(self.players[1].pos));
                let db = b.0.dist(self.players[0].pos).min(b.0.dist(self.players[1].pos));
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });

        for i in 0..self.pets.len() {
            let owner_idx = self.pets[i].side.idx();
            let owner_pos = self.players[owner_idx].pos;
            let owner_alive = self.players[owner_idx].alive;

            let pet = &mut self.pets[i];
            pet.angle += if owner_alive { 0.10 } else { 0.16 };
            let r = if owner_alive { 1.8 } else { 0.9 };
            let offset = Vec2f::new(pet.angle.cos() * r, pet.angle.sin() * r);
            pet.pos = (owner_pos + offset).clamp_to(self.field);

            // Sentinel & Apex point defense: zap enemy bullets in range
            if pet.level >= 2 {
                let zap_radius = if pet.level >= 3 { 1.4 } else { 0.95 };
                let pet_pos = pet.pos;
                let mut zapped = false;
                self.hazards.retain(|h| {
                    if h.pos.dist(pet_pos) < zap_radius {
                        zapped = true;
                        false
                    } else {
                        true
                    }
                });
                if zapped {
                    self.shots_hit += 1;
                }
            }

            // Auto-fire at closest monster
            if pet.fire_cd > 0 {
                pet.fire_cd -= 1;
            } else if let Some((target_pos, _)) = nearest_enemy {
                let aim = (target_pos - pet.pos).norm();
                if self.shots.len() < 240 {
                    self.shots.push(Shot {
                        pos: pet.pos,
                        vel: aim.scale(SHOT_SPEED * 1.15),
                        ttl: SHOT_TTL,
                        side: pet.side,
                    });
                    self.shots_fired += 1;

                    if pet.level >= 3 {
                        let aim2 = rotate(aim, 0.12);
                        self.shots.push(Shot {
                            pos: pet.pos,
                            vel: aim2.scale(SHOT_SPEED * 1.15),
                            ttl: SHOT_TTL,
                            side: pet.side,
                        });
                        self.shots_fired += 1;
                    }

                    ev.push(Ev::Shoot {
                        at: pet.pos.cell(),
                        side: pet.side,
                    });
                }
                pet.fire_cd = match pet.level {
                    1 => 16,
                    2 => 11,
                    _ => 7,
                };
            }
        }
    }

    fn update_shots(&mut self, ev: &mut Vec<Ev>) {
        let f = self.field;
        let mut hit = vec![false; self.shots.len()];
        for si in 0..self.shots.len() {
            let pos = self.shots[si].pos;
            let mut target: Option<usize> = None;
            let mut best = 0.32f32;
            for (ei, e) in self.enemies.iter().enumerate() {
                if e.born > 0 {
                    continue;
                }
                let r = e.radius + 0.22;
                let d = pos.dist(e.pos);
                if d < r && d <= best + 0.4 {
                    best = d;
                    target = Some(ei);
                }
            }
            if let Some(ei) = target {
                hit[si] = true;
                self.shots_hit += 1;
                let e = &mut self.enemies[ei];
                e.hp -= 1;
                e.flash = 5;
            }
        }

        let mut alive: Vec<Shot> = Vec::with_capacity(self.shots.len());
        for (si, mut s) in self.shots.drain(..).enumerate() {
            if hit[si] {
                continue;
            }
            s.pos = s.pos + s.vel;
            s.ttl = s.ttl.saturating_sub(1);
            if s.ttl > 0
                && s.pos.x > -CULL_MARGIN
                && s.pos.y > -CULL_MARGIN
                && s.pos.x < f.w as f32 + CULL_MARGIN
                && s.pos.y < f.h as f32 + CULL_MARGIN
            {
                alive.push(s);
            }
        }
        self.shots = alive;

        // Sweep the dead.
        let killed: Vec<Enemy> = self.enemies.iter().copied().filter(|e| e.hp <= 0).collect();
        if !killed.is_empty() {
            self.enemies.retain(|e| e.hp > 0);
            for e in killed {
                self.on_kill(e, ev);
            }
        }
    }

    // --------------------------------------------------------------- revive

    fn revive_partners(&mut self, ev: &mut Vec<Ev>) {
        let downed = self.players.iter().position(|p| !p.alive);
        let Some(di) = downed else { return };
        if self.players.iter().filter(|p| !p.alive).count() > 1 {
            return;
        }
        let oi = 1 - di;
        let near = self.players[oi].alive
            && self.players[oi].pos.dist(self.players[di].pos) <= 1.9;
        if near {
            self.players[di].revive += 1;
            if self.players[di].revive >= REVIVE_TICKS {
                self.players[di].revive = 0;
                self.players[di].alive = true;
                self.players[di].hp = 2;
                self.players[di].invuln = INVULN + 20;
                let at = self.players[di].pos.cell();
                ev.push(Ev::Revive {
                    side: self.players[di].side,
                    at,
                });
                self.banner("RE-LINKED", 50);
            }
        } else {
            self.players[di].revive = 0;
        }
    }

    // -------------------------------------------------------------- enemies

    fn update_enemies(&mut self, ev: &mut Vec<Ev>) {
        let targets: Vec<(Vec2f, Vec2f)> = self
            .players
            .iter()
            .filter(|p| p.alive)
            .map(|p| (p.pos, p.vel))
            .collect();
        let wave = self.wave as f32;
        let pressure = self.adaptive.pressure;
        let field = self.field;
        let mut pending: Vec<Hazard> = Vec::new();

        // --- movement + fire timers
        for e in self.enemies.iter_mut() {
            if e.born > 0 {
                e.born -= 1;
                continue;
            }
            e.flash = e.flash.saturating_sub(1);
            e.cd = e.cd.saturating_sub(1);

            if targets.is_empty() {
                continue;
            }
            let nearest = nearest_target(&targets, e.pos);
            let to = nearest.0 - e.pos;
            let dist = to.len();
            let dir = to.norm();
            let speed = match e.kind {
                EnemyKind::Grunt => 0.032 + wave * 0.0022,
                EnemyKind::Swarm => 0.090 + wave * 0.0035,
                EnemyKind::Spitter => 0.040 + wave * 0.0018,
                EnemyKind::Weaver => 0.058 + wave * 0.0022,
                EnemyKind::Bulwark => 0.024 + wave * 0.0012,
                _ => 0.030,
            }
            .min(0.15);

            match e.kind {
                EnemyKind::Spitter => {
                    // Keep a firing lane: close in, back off, or slide sideways.
                    if dist > 9.5 {
                        e.pos = e.pos + dir.scale(speed);
                    } else if dist < 5.5 {
                        e.pos = e.pos + dir.scale(-speed);
                    } else {
                        e.pos = e.pos + dir.perp().scale(speed * e.strafe);
                    }
                }
                EnemyKind::Weaver => {
                    // Orbit at roughly seven cells.
                    let radial = if dist > 7.5 {
                        1.0
                    } else if dist < 5.5 {
                        -1.0
                    } else {
                        0.0
                    };
                    let tangent = dir.perp().scale(e.strafe);
                    e.pos = e.pos + tangent.scale(speed) + dir.scale(radial * speed * 0.7);
                }
                EnemyKind::Warden | EnemyKind::Hydra | EnemyKind::Overmind => {
                    // Bosses hold the middle of the arena and drift.
                    let mid = if dist > 11.0 {
                        1.0
                    } else if dist < 6.0 {
                        -1.0
                    } else {
                        0.0
                    };
                    let tangent = dir.perp().scale(e.strafe);
                    e.pos = e.pos
                        + tangent.scale(speed * 1.4)
                        + dir.scale(mid * speed * 1.2);
                }
                _ => {
                    e.pos = e.pos + dir.scale(speed);
                }
            }
            e.pos = e.pos.clamp_to(field);

            // Non-boss firing.
            if !e.kind.is_boss() && e.cd == 0 {
                let (fire_rate, shot_speed) = match e.kind {
                    EnemyKind::Spitter => (96.0, 0.36),
                    EnemyKind::Weaver => (118.0, 0.32),
                    EnemyKind::Bulwark => (150.0, 0.28),
                    _ => (0.0, 0.0),
                };
                if fire_rate > 0.0 && dist < 22.0 {
                    let cadence =
                        (fire_rate - wave * 2.0 - pressure * 26.0).clamp(34.0, fire_rate) as u32;
                    e.cd = cadence;
                    let aim = lead_aim(
                        e.pos,
                        nearest.0,
                        nearest.1,
                        shot_speed,
                        self.adaptive.lead,
                    );
                    match e.kind {
                        EnemyKind::Weaver => {
                            for k in -1..=1 {
                                let a = aim_angle(aim) + k as f32 * 0.30;
                                pending.push(hazard_at(
                                    e.pos,
                                    a,
                                    shot_speed * (0.95 + pressure * 0.18),
                                    HazardKind::Dart,
                                ));
                            }
                        }
                        EnemyKind::Bulwark => {
                            pending.push(hazard_at(
                                e.pos,
                                aim_angle(aim),
                                shot_speed * (0.95 + pressure * 0.15),
                                HazardKind::Heavy,
                            ));
                        }
                        _ => {
                            pending.push(hazard_at(
                                e.pos,
                                aim_angle(aim),
                                shot_speed * (0.95 + pressure * 0.20),
                                HazardKind::Pellet,
                            ));
                        }
                    }
                    ev.push(Ev::EnemyShoot {
                        at: e.pos.cell(),
                        heavy: e.kind == EnemyKind::Bulwark,
                    });
                } else if fire_rate > 0.0 {
                    e.cd = 12;
                }
            }
        }

        // --- boss volleys (kept separate so the director can borrow itself)
        let boss_idx = self.enemies.iter().position(|e| e.kind.is_boss());
        if let Some(bi) = boss_idx {
            self.boss_act(bi, &mut pending, ev);
        }

        if self.hazards.len() + pending.len() <= MAX_HAZARDS {
            self.hazards.extend(pending);
        }
    }

    fn boss_act(&mut self, bi: usize, pending: &mut Vec<Hazard>, ev: &mut Vec<Ev>) {
        let kind = self.enemies[bi].kind;
        if self.enemies[bi].born > 0 || self.enemies[bi].cd > 0 {
            return;
        }

        let targets: Vec<(Vec2f, Vec2f)> = self
            .players
            .iter()
            .filter(|p| p.alive)
            .map(|p| (p.pos, p.vel))
            .collect();
        if targets.is_empty() {
            return;
        }
        let pos = self.enemies[bi].pos;
        let nearest = nearest_target(&targets, pos);
        let pressure = self.adaptive.pressure;
        let boss_wave = self.wave as f32;

        // Pattern switch every handful of volleys, chosen by the bandit.
        if self.enemies[bi].aux == 0 {
            self.adaptive.choose_pattern(&mut self.rng);
            self.enemies[bi].aux = 7;
            self.enemies[bi].phase = self.enemies[bi].phase.wrapping_add(1);
        }
        let pat = self.adaptive.pattern;

        let speed = match kind {
            EnemyKind::Warden => (0.20 + pressure * 0.06).min(0.32),
            EnemyKind::Hydra => (0.24 + pressure * 0.08).min(0.40),
            _ => (0.28 + pressure * 0.10 + boss_wave * 0.002).min(0.48),
        };
        match pat {
            0 => {
                // Ring with a guaranteed gap. The gap drifts toward the side the
                // players habitually dodge to, so the "safe lane" moves and the
                // habit stops being a free win.
                let n = if kind == EnemyKind::Warden { 13 } else { 16 };
                let base = aim_angle((nearest.0 - pos).norm());
                let gap = base + self.adaptive.dodge * 0.9;
                let gap_width = if kind == EnemyKind::Warden { 0.72 } else { 0.52 };
                for i in 0..n {
                    let a = std::f32::consts::TAU * i as f32 / n as f32;
                    if angle_diff(a, gap) < gap_width {
                        continue;
                    }
                    pending.push(hazard_at(
                        pos,
                        a,
                        speed,
                        HazardKind::Pellet,
                    ));
                }
            }
            1 => {
                // Aimed fan at both operators, with predictive lead.
                for t in &targets {
                    let aim = lead_aim(pos, t.0, t.1, speed, self.adaptive.lead);
                    let center = aim_angle(aim);
                    let count = if kind == EnemyKind::Warden {
                        3
                    } else if kind == EnemyKind::Overmind {
                        7
                    } else {
                        5
                    };
                    for k in 0..count {
                        let off = (k as f32 - (count - 1) as f32 / 2.0) * 0.16;
                        pending.push(hazard_at(
                            pos,
                            center + off,
                            speed * 1.05,
                            HazardKind::Dart,
                        ));
                    }
                }
            }
            _ => {
                // Twin spirals: the arms rotate every volley.
                let spin = self.enemies[bi].phase as f32 * 0.24;
                let base = spin + (self.enemies[bi].cd as f32).sin();
                for k in 0..2 {
                    pending.push(hazard_at(
                        pos,
                        base + k as f32 * std::f32::consts::PI,
                        speed * 0.92,
                        HazardKind::Pellet,
                    ));
                }
                if kind == EnemyKind::Overmind {
                    // A third, slower arm makes the spiral read as a wall.
                    pending.push(hazard_at(
                        pos,
                        base + std::f32::consts::FRAC_PI_2,
                        speed * 0.62,
                        HazardKind::Heavy,
                    ));
                }
            }
        }

        self.enemies[bi].phase = self.enemies[bi].phase.wrapping_add(1);
        self.enemies[bi].aux = self.enemies[bi].aux.saturating_sub(1);
        self.enemies[bi].flash = 3;
        let enraged = self.enemies[bi].hp * 3 < self.enemies[bi].max_hp;
        let base_gap = match kind {
            EnemyKind::Warden => if enraged { 22.0 } else { 34.0 },
            EnemyKind::Hydra => if enraged { 16.0 } else { 26.0 },
            _ => if enraged { 12.0 } else { 20.0 },
        };
        self.enemies[bi].cd = (base_gap - pressure * 5.0).clamp(10.0, base_gap) as u32;
        ev.push(Ev::BossVolley { at: pos.cell(), pattern: pat });

        // Hydra keeps seeding the arena; Overmind joins in when wounded.
        let adds = if kind == EnemyKind::Hydra {
            self.t % 260 == 0
        } else {
            enraged && self.t % 220 == 0
        };
        if adds && self.enemies.len() < 14 {
            for _ in 0..2 {
                let mut s = self.make_enemy(if kind == EnemyKind::Hydra {
                    EnemyKind::Swarm
                } else {
                    EnemyKind::Spitter
                });
                s.pos = Vec2f::new(pos.x + self.rng_f(-3.0, 3.0), pos.y + self.rng_f(-2.0, 2.0))
                    .clamp_to(self.field);
                s.born = 12;
                ev.push(Ev::Spawn {
                    at: s.pos.cell(),
                    kind: s.kind,
                });
                self.enemies.push(s);
            }
        }
    }

    // -------------------------------------------------------------- bullets

    fn update_hazards(&mut self, ev: &mut Vec<Ev>) {
        let f = self.field;
        let margin = CULL_MARGIN;
        let players = [
            (self.players[0].pos, self.players[0].alive, self.players[0].invuln),
            (self.players[1].pos, self.players[1].alive, self.players[1].invuln),
        ];
        let mut keep: Vec<Hazard> = Vec::with_capacity(self.hazards.len());
        let mut hurt: Vec<(usize, i32, Vec2)> = Vec::new();

        for mut h in self.hazards.drain(..) {
            h.pos = h.pos + h.vel;
            h.ttl = h.ttl.saturating_sub(1);
            if h.ttl == 0
                || h.pos.x < -margin
                || h.pos.y < -margin
                || h.pos.x > f.w as f32 + margin
                || h.pos.y > f.h as f32 + margin
            {
                continue;
            }

            let mut consumed = false;
            for (i, (pp, alive, invuln)) in players.iter().enumerate() {
                if !*alive {
                    continue;
                }
                let d = h.pos.dist(*pp);
                if d < PLAYER_R + h.r {
                    if *invuln == 0 {
                        hurt.push((i, h.kind.damage(), h.pos.cell()));
                    }
                    consumed = true;
                    break;
                }
                // Lightly reward surviving a close call: rewards near-misses so
                // the director learns the players are actually dodging, not
                // simply standing still far away.
                if d < 1.35 && self.players[i].invuln == 0 {
                    self.adaptive.near_misses = self.adaptive.near_misses.saturating_add(1);
                    let hdir = h.vel.norm();
                    let lateral = self.players[i].vel.dot(hdir.perp());
                    self.adaptive.watch_dodge(lateral);
                }
            }
            if !consumed {
                keep.push(h);
            }
        }
        self.hazards = keep;

        for (i, dmg, at) in hurt {
            self.hurt_player(i, dmg, at, ev);
        }
    }

    fn contact_damage(&mut self, ev: &mut Vec<Ev>) {
        for i in 0..2 {
            if !self.players[i].alive || self.players[i].invuln > 0 {
                continue;
            }
            let ppos = self.players[i].pos;
            let hit = self
                .enemies
                .iter()
                .enumerate()
                .filter(|(_, e)| e.born == 0)
                .find(|(_, e)| e.pos.dist(ppos) < e.radius + PLAYER_R)
                .map(|(ei, e)| (ei, e.kind, e.pos));
            if let Some((ei, kind, epos)) = hit {
                let at = ppos.cell();
                let away = (ppos - epos).norm();
                self.players[i].pos =
                    (self.players[i].pos + away.scale(1.0)).clamp_to(self.field);
                self.hurt_player(i, kind.contact(), at, ev);
                if self.phase != Phase::Playing {
                    return;
                }
                // Shove the enemy back too, so contact is not a free hug.
                if !kind.is_boss() {
                    self.enemies[ei].pos =
                        (self.enemies[ei].pos - away.scale(0.5)).clamp_to(self.field);
                }
            }
        }
    }

    fn hurt_player(&mut self, i: usize, dmg: i32, at: Vec2, ev: &mut Vec<Ev>) {
        if !self.players[i].alive || self.players[i].invuln > 0 {
            return;
        }
        if self.players[i].shield > 0 {
            self.players[i].shield = 0;
            self.players[i].invuln = 40;
            self.players[i].flash = 6;
            self.shake = self.shake.max(0.8);
            ev.push(Ev::Block {
                side: self.players[i].side,
                at,
            });
            return;
        }

        self.players[i].hp -= dmg;
        self.players[i].invuln = INVULN;
        self.players[i].flash = 10;
        self.shake = self.shake.max(1.0);
        self.combo = 0;
        self.combo_t = 0;
        self.adaptive.hit();
        self.adaptive.scored();
        let side = self.players[i].side;
        ev.push(Ev::Hit { side, at });

        if self.players[i].hp <= 0 {
            self.players[i].hp = 0;
            self.players[i].alive = false;
            self.players[i].revive = 0;
            self.players[i].invuln = 0;
            ev.push(Ev::Down { side, at });
            self.shake = self.shake.max(1.6);
            if self.alive_count() == 0 {
                self.phase = Phase::Lost;
                self.shake = 2.2;
                ev.push(Ev::Lose);
            }
        }
    }

    // -------------------------------------------------------------- pickups

    fn update_pickups(&mut self, ev: &mut Vec<Ev>) {
        let players = [
            (self.players[0].pos, self.players[0].alive),
            (self.players[1].pos, self.players[1].alive),
        ];
        let mut taken: Vec<usize> = Vec::new();
        for (i, o) in self.pickups.iter_mut().enumerate() {
            o.ttl = o.ttl.saturating_sub(1);
            o.ph = o.ph.wrapping_add(1);
            let got = players
                .iter()
                .any(|(p, alive)| *alive && p.dist(o.pos) < 0.85);
            if got || o.ttl == 0 {
                taken.push(i);
            }
        }
        for i in taken.into_iter().rev() {
            let o = self.pickups.remove(i);
            if o.ttl == 0 {
                continue;
            }
            match o.kind {
                PickupKind::Heal => {
                    // Heal whoever is hurt most, rather than wasting it.
                    let mut best = 0usize;
                    let mut lowest = MAX_HP + 1;
                    for (i, p) in self.players.iter().enumerate() {
                        if p.alive && p.hp < lowest {
                            lowest = p.hp;
                            best = i;
                        }
                    }
                    if lowest <= MAX_HP {
                        self.players[best].hp = (self.players[best].hp + 1).min(MAX_HP);
                    }
                }
                PickupKind::Rapid => {
                    for p in self.players.iter_mut() {
                        p.rapid = 360;
                    }
                }
                PickupKind::Shield => {
                    for p in self.players.iter_mut() {
                        p.shield = 240;
                    }
                }
                PickupKind::Evolve => {
                    for p in self.players.iter_mut() {
                        p.evolution = (p.evolution + 1).min(3);
                    }
                    let evo_name = match self.players[0].evolution {
                        1 => "DUAL CANNONS",
                        2 => "TRI-SPREAD CANNONS",
                        _ => "HYPER PLASMA CANNONS",
                    };
                    self.banner(format!("WEAPON EVOLVED: {}", evo_name), 90);
                }
                PickupKind::Pet => {
                    let mut evolved = false;
                    for side in [Side::One, Side::Two] {
                        if let Some(pet) = self.pets.iter_mut().find(|p| p.side == side) {
                            pet.level = (pet.level + 1).min(3);
                            evolved = true;
                        } else {
                            self.pets.push(Pet {
                                pos: self.players[side.idx()].pos,
                                angle: if side == Side::One { 0.0 } else { std::f32::consts::PI },
                                level: 1,
                                fire_cd: 10,
                                side,
                            });
                        }
                    }
                    let pet_name = if let Some(p) = self.pets.first() {
                        p.name()
                    } else {
                        "SCOUT"
                    };
                    if evolved {
                        self.banner(format!("PET EVOLVED: {}", pet_name), 90);
                    } else {
                        self.banner("COMBAT DRONES DEPLOYED!", 90);
                    }
                }
            }
            ev.push(Ev::Pickup { at: o.pos.cell(), kind: o.kind });
        }

        if self.t % 280 == 0 && self.pickups.len() < 4 {
            let roll = self.rng.f32();
            let kind = if roll < 0.25 {
                PickupKind::Heal
            } else if roll < 0.45 {
                PickupKind::Rapid
            } else if roll < 0.65 {
                PickupKind::Shield
            } else if roll < 0.82 {
                PickupKind::Evolve
            } else {
                PickupKind::Pet
            };
            if let Some(p) = self.free_cell(2.0) {
                let ph = self.rng_f(0.0, 64.0) as u32;
                self.pickups.push(Pickup {
                    pos: p,
                    kind,
                    ttl: 720,
                    ph,
                });
            }
        }
    }

    // ---------------------------------------------------------------- waves

    fn update_waves(&mut self, ev: &mut Vec<Ev>) {
        if self.queued.is_empty() && self.enemies.is_empty() && self.intermission == 0 {
            if self.wave >= TOTAL_WAVES {
                self.phase = Phase::Won;
                self.shake = 1.8;
                ev.push(Ev::Win);
                return;
            }
            let done = self.wave;
            let bonus = 150 * done as u64;
            self.score += bonus;
            ev.push(Ev::WaveClear { wave: done, bonus });
            self.wave += 1;
            self.intermission = 80;
            for p in self.players.iter_mut() {
                if p.alive {
                    p.hp = (p.hp + 1).min(MAX_HP);
                }
            }
        }

        if self.intermission > 0 {
            self.intermission -= 1;
            if self.intermission == 0 {
                self.queued = wave_spec(self.wave);
                self.spawn_cd = 8;
                if self.wave_is_boss(self.wave) {
                    self.banner("BOSS INCOMING", 110);
                } else {
                    self.banner(format!("WAVE {:02}", self.wave), 90);
                }
                ev.push(Ev::WaveStart { wave: self.wave });
            }
        }

        if !self.queued.is_empty() && self.enemies.len() < MAX_ENEMIES {
            if self.spawn_cd > 0 {
                self.spawn_cd -= 1;
            } else {
                let kind = self.queued.remove(0);
                let e = self.make_enemy(kind);
                self.spawn_cd = if kind.is_boss() { 70 } else { 13 };
                if kind.is_boss() {
                    self.banner(format!("{} \u{00b7} ENGAGED", kind.name()), 130);
                }
                ev.push(Ev::Spawn {
                    at: e.pos.cell(),
                    kind,
                });
                self.enemies.push(e);
            }
        }
    }

    // -------------------------------------------------------------- helpers

    fn rng_f(&mut self, a: f32, b: f32) -> f32 {
        a + self.rng.f32() * (b - a)
    }

    fn free_cell(&mut self, clearance: f32) -> Option<Vec2f> {
        for _ in 0..48 {
            let p = Vec2f::new(
                self.rng_f(1.0, (self.field.w - 2) as f32),
                self.rng_f(1.0, (self.field.h - 2) as f32),
            );
            if self.players.iter().all(|pl| pl.pos.dist(p) > clearance + 3.0)
                && self.pickups.iter().all(|o| o.pos.dist(p) > 3.0)
                && self.enemies.iter().all(|e| e.pos.dist(p) > 2.0)
            {
                return Some(p);
            }
        }
        None
    }

    fn spawn_pos(&mut self) -> Vec2f {
        for _ in 0..64 {
            let p = match self.rng.below(4) {
                0 => Vec2f::new(self.rng_f(1.0, (self.field.w - 2) as f32), 0.6),
                1 => Vec2f::new(
                    self.rng_f(1.0, (self.field.w - 2) as f32),
                    (self.field.h - 1) as f32 - 0.6,
                ),
                2 => Vec2f::new(0.6, self.rng_f(1.0, (self.field.h - 2) as f32)),
                _ => Vec2f::new(
                    (self.field.w - 1) as f32 - 0.6,
                    self.rng_f(1.0, (self.field.h - 2) as f32),
                ),
            };
            let clear = self.players.iter().all(|pl| pl.pos.dist(p) > 7.0)
                && self.enemies.iter().all(|e| e.pos.dist(p) > 2.5);
            if clear {
                return p;
            }
        }
        Vec2f::new(1.0, self.field.h as f32 / 2.0)
    }

    fn make_enemy(&mut self, kind: EnemyKind) -> Enemy {
        let pos = if kind.is_boss() {
            Vec2f::new(self.field.w as f32 / 2.0, 3.0)
        } else {
            self.spawn_pos()
        };
        let scale = 1.0 + 0.13 * (self.wave.saturating_sub(1)) as f32;
        let hp = if kind.is_boss() {
            kind.hp()
        } else {
            ((kind.hp() as f32) * scale).round() as i32
        };
        Enemy {
            pos,
            kind,
            hp,
            max_hp: hp,
            flash: 0,
            cd: if kind.is_boss() {
                55
            } else {
                30 + self.rng.below(40) as u32
            },
            phase: 0,
            born: if kind.is_boss() { 45 } else { 16 },
            strafe: if self.rng.chance(0.5) { 1.0 } else { -1.0 },
            radius: kind.radius(),
            aux: 0,
        }
    }

    fn on_kill(&mut self, e: Enemy, ev: &mut Vec<Ev>) {
        self.combo = (self.combo + 1).min(99);
        self.best_combo = self.best_combo.max(self.combo);
        self.combo_t = 150;
        self.score += e.kind.points() * self.score_mult();
        self.kills += 1;
        self.adaptive.kill();
        self.shake = self.shake.max(if e.kind.is_boss() {
            2.2
        } else if e.kind == EnemyKind::Bulwark {
            0.9
        } else {
            0.35
        });
        ev.push(Ev::Kill {
            at: e.pos.cell(),
            kind: e.kind,
            combo: self.combo,
        });

        if e.kind.is_boss() {
            self.banner(format!("{} DOWN", e.kind.name()), 150);
            // Boss death clears the screen of hostile fire.
            self.hazards.clear();
            self.shake = 2.4;
        }

        // Weavers vent a small ring on death: killing them point-blank is risky.
        if e.kind == EnemyKind::Weaver && self.hazards.len() < MAX_HAZARDS - 6 {
            let speed = 0.24 + self.adaptive.pressure * 0.06;
            for k in 0..4 {
                let a = std::f32::consts::FRAC_PI_2 * k as f32 + 0.4;
                self.hazards.push(hazard_at(
                    e.pos,
                    a,
                    speed,
                    HazardKind::Pellet,
                ));
            }
        }

        // Chance to spawn a pickup on monster defeat:
        let drop_chance = if e.kind.is_boss() {
            1.0
        } else if e.kind == EnemyKind::Bulwark {
            0.50
        } else {
            0.14
        };
        if self.rng.f32() < drop_chance && self.pickups.len() < 5 {
            let roll = self.rng.f32();
            let kind = if roll < 0.25 {
                PickupKind::Heal
            } else if roll < 0.45 {
                PickupKind::Rapid
            } else if roll < 0.65 {
                PickupKind::Shield
            } else if roll < 0.82 {
                PickupKind::Evolve
            } else {
                PickupKind::Pet
            };
            let ph = self.rng_f(0.0, 64.0) as u32;
            self.pickups.push(Pickup {
                pos: e.pos,
                kind,
                ttl: 720,
                ph,
            });
        }
    }
}

// -------------------------------------------------------------------- free fns

fn rotate(v: Vec2f, a: f32) -> Vec2f {
    let (s, c) = a.sin_cos();
    Vec2f::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

fn aim_angle(dir: Vec2f) -> f32 {
    dir.y.atan2(dir.x)
}

/// Smallest absolute angle between two angles.
fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (a - b).rem_euclid(std::f32::consts::TAU);
    if d > std::f32::consts::PI {
        d = std::f32::consts::TAU - d;
    }
    d
}

/// Predictive aim: lead the target's current velocity by the time of flight.
fn lead_aim(from: Vec2f, target: Vec2f, tvel: Vec2f, speed: f32, lead: f32) -> Vec2f {
    let d = target.dist(from);
    let t = (d / speed.max(0.05)).clamp(0.0, 2.6);
    (target + tvel.scale(t * lead) - from).norm()
}

fn nearest_target(targets: &[(Vec2f, Vec2f)], from: Vec2f) -> (Vec2f, Vec2f) {
    let mut best = targets[0];
    let mut bd = best.0.dist(from);
    for t in &targets[1..] {
        let d = t.0.dist(from);
        if d < bd {
            bd = d;
            best = *t;
        }
    }
    best
}

fn hazard_at(pos: Vec2f, angle: f32, speed: f32, kind: HazardKind) -> Hazard {
    Hazard {
        pos,
        vel: Vec2f::new(angle.cos() * speed, angle.sin() * speed),
        ttl: 260,
        kind,
        r: kind.radius(),
    }
}

/// Roster for one wave. Boss waves are every `BOSS_EVERY` waves.
pub fn wave_spec(w: u32) -> Vec<EnemyKind> {
    use self::EnemyKind::*;
    let mut v = Vec::new();
    if w % BOSS_EVERY == 0 {
        let boss = match w {
            5 => Warden,
            10 => Hydra,
            _ => Overmind,
        };
        v.push(boss);
        v.extend(std::iter::repeat(Grunt).take(2 + (w / 5) as usize));
        v.extend(std::iter::repeat(Spitter).take((w / 5) as usize));
        return v;
    }
    let grunts = 4 + w;
    let spitters = 1 + w / 2;
    let weavers = if w >= 2 { 1 + (w - 2) / 2 } else { 0 };
    let swarms = if w >= 2 { 2 + w * 2 } else { 0 };
    let bulwarks = if w >= 3 { 1 + (w - 3) / 3 } else { 0 };

    v.extend(std::iter::repeat(Grunt).take(grunts as usize));
    v.extend(std::iter::repeat(Spitter).take(spitters as usize));
    v.extend(std::iter::repeat(Weaver).take(weavers as usize));
    v.extend(std::iter::repeat(Swarm).take(swarms as usize));
    v.extend(std::iter::repeat(Bulwark).take(bulwarks as usize));
    // Interleave so the early part of a wave is mixed, not one solid block.
    let mut out = Vec::with_capacity(v.len());
    let mut i = 0;
    while !v.is_empty() {
        let idx = (i * 7) % v.len();
        out.push(v.remove(idx));
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> World {
        let mut w = World::new(Field::new(64, 22));
        w.queued.clear();
        w.enemies.clear();
        w.pickups.clear();
        w.pets.clear();
        w.spawn_cd = 0;
        w.players[0].invuln = 0;
        w.players[1].invuln = 0;
        w
    }

    #[test]
    fn starts_playing_on_wave_one() {
        let w = world();
        assert_eq!(w.phase, Phase::Playing);
        assert_eq!(w.wave, 1);
        assert_eq!(w.players[0].hp, MAX_HP);
    }

    #[test]
    fn cannons_fire_and_kill() {
        let mut w = world();
        w.players[0].pos = Vec2f::new(10.0, 11.0);
        w.players[1].pos = Vec2f::new(54.0, 11.0);
        w.enemies.push(Enemy {
            pos: Vec2f::new(20.0, 11.0),
            kind: EnemyKind::Grunt,
            hp: 2,
            max_hp: 2,
            flash: 0,
            cd: 0,
            phase: 0,
            born: 0,
            strafe: 1.0,
            radius: 0.45,
            aux: 0,
        });
        let mut shots = 0;
        for _ in 0..80 {
            w.step([None, None]);
            shots = shots.max(w.shots_fired);
            if w.kills > 0 {
                break;
            }
        }
        assert!(shots > 0, "auto-cannon should fire at a nearby enemy");
        assert!(w.kills > 0, "the cannon should eventually kill a grunt");
    }

    #[test]
    fn enemy_fire_creates_hazards() {
        let mut w = world();
        w.players[0].pos = Vec2f::new(10.0, 11.0);
        w.players[1].pos = Vec2f::new(12.0, 11.0);
        w.players[0].invuln = 9999;
        w.players[1].invuln = 9999;
        w.enemies.push(Enemy {
            pos: Vec2f::new(25.0, 11.0),
            kind: EnemyKind::Spitter,
            hp: 99,
            max_hp: 99,
            flash: 0,
            cd: 1,
            phase: 0,
            born: 0,
            strafe: 1.0,
            radius: 0.45,
            aux: 0,
        });
        for _ in 0..30 {
            w.step([None, None]);
        }
        assert!(!w.hazards.is_empty(), "spitters must shoot");
    }

    #[test]
    fn hazards_damage_players() {
        let mut w = world();
        w.intermission = 100;
        w.players[0].invuln = 0;
        w.players[0].pos = Vec2f::new(10.0, 11.0);
        w.players[1].pos = Vec2f::new(50.0, 11.0);
        w.hazards.push(Hazard {
            pos: Vec2f::new(9.6, 11.0),
            vel: Vec2f::new(0.3, 0.0),
            ttl: 30,
            kind: HazardKind::Pellet,
            r: HazardKind::Pellet.radius(),
        });
        w.step([None, None]);
        assert_eq!(w.players[0].hp, MAX_HP - 1);
        assert!(w.adaptive.hits_taken >= 1);
    }

    #[test]
    fn shield_absorbs_one_hit() {
        let mut w = world();
        w.intermission = 100;
        w.players[0].invuln = 0;
        w.players[0].shield = 100;
        w.players[0].pos = Vec2f::new(10.0, 11.0);
        w.hazards.push(Hazard {
            pos: Vec2f::new(10.1, 11.0),
            vel: Vec2f::new(0.0, 0.0),
            ttl: 30,
            kind: HazardKind::Pellet,
            r: HazardKind::Pellet.radius(),
        });
        w.step([None, None]);
        assert_eq!(w.players[0].hp, MAX_HP);
        assert_eq!(w.players[0].shield, 0);
    }

    #[test]
    fn boss_waves_every_five() {
        assert!(wave_spec(5).iter().any(|k| k.is_boss()));
        assert!(wave_spec(10).iter().any(|k| k.is_boss()));
        assert!(wave_spec(15).iter().any(|k| k.is_boss()));
        assert!(!wave_spec(4).iter().any(|k| k.is_boss()));
        assert!(!wave_spec(6).iter().any(|k| k.is_boss()));
    }

    #[test]
    fn later_waves_are_bigger() {
        assert!(wave_spec(8).len() > wave_spec(2).len());
        assert!(wave_spec(14).len() > wave_spec(8).len());
    }

    #[test]
    fn clearing_a_wave_advances_after_intermission() {
        let mut w = world();
        w.step([None, None]);
        assert_eq!(w.wave, 2);
        assert!(w.intermission > 0);
        for _ in 0..w.intermission + 2 {
            w.step([None, None]);
        }
        assert!(!w.queued.is_empty() || !w.enemies.is_empty());
    }

    #[test]
    fn clearing_the_last_wave_wins() {
        let mut w = world();
        w.wave = TOTAL_WAVES;
        w.queued.clear();
        w.enemies.clear();
        w.intermission = 0;
        let ev = w.step([None, None]);
        assert_eq!(w.phase, Phase::Won);
        assert!(ev.iter().any(|e| matches!(e, Ev::Win)));
    }

    #[test]
    fn bandit_reinforces_patterns_that_land_hits() {
        let mut a = Adaptive::default();
        a.pattern = 1;
        a.pattern_hits = 6;
        let mut rng = Rng::new(7);
        a.choose_pattern(&mut rng);
        assert!(
            a.weights[1] > 1.0,
            "a pattern that kept hitting should gain weight: {:?}",
            a.weights
        );
    }

    #[test]
    fn pressure_falls_when_players_get_hit() {
        let mut a = Adaptive::default();
        a.pressure = 0.9;
        a.comfort = 0.9;
        a.hit();
        assert!(a.pressure < 0.9);
        assert!(a.comfort < 0.9);
    }

    #[test]
    fn weapon_evolution_increases_firepower() {
        let mut w = world();
        w.intermission = 100;
        w.players[0].evolution = 2; // Tri-spread
        w.players[0].pos = Vec2f::new(10.0, 11.0);
        w.players[1].pos = Vec2f::new(50.0, 11.0);
        w.enemies.push(Enemy {
            pos: Vec2f::new(20.0, 11.0),
            kind: EnemyKind::Grunt,
            hp: 20,
            max_hp: 20,
            flash: 0,
            cd: 0,
            phase: 0,
            born: 0,
            strafe: 1.0,
            radius: 0.45,
            aux: 0,
        });
        w.players[0].gun_cd = 0;
        w.step([None, None]);
        assert!(w.shots.len() >= 3, "Tri-spread should fire 3 shots at once");
    }

    #[test]
    fn combat_pets_summon_orbit_and_protect() {
        let mut w = world();
        w.intermission = 100;
        w.players[0].pos = Vec2f::new(10.0, 11.0);
        w.pets.push(Pet {
            pos: Vec2f::new(11.8, 11.0),
            angle: -0.10,
            level: 2, // Sentinel
            fire_cd: 0,
            side: Side::One,
        });
        // Place an enemy hazard close to the pet
        w.hazards.push(Hazard {
            pos: Vec2f::new(11.8, 11.0),
            vel: Vec2f::new(0.0, 0.0),
            ttl: 30,
            kind: HazardKind::Pellet,
            r: HazardKind::Pellet.radius(),
        });
        w.step([None, None]);
        // The level 2 sentinel pet should zap the nearby hazard
        assert!(w.hazards.is_empty(), "Sentinel pet must zap incoming hazard");
    }
}

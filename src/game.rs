//! CONDUIT simulation.
//!
//! Two operators are joined by an energy conduit. It is their only weapon and
//! it only burns when the pair is stretched apart. Huddling recharges the shared
//! cell; reaching ignites the beam. Everything in the game is a negotiation.
//!
//! Design:
//! * CLOSE (dist <= REST_DIST) -> conduit cold -> shared charge RECHARGES
//! * FAR (dist >= HOT_DIST)   -> conduit hot  -> anything it touches DIES, charge DRAINS
//! * Beyond SNAP_DIST the conduit snaps: players are yanked together and lose charge.
//! * Zero charge = both operators lose ("the cell went dark").
//! * Downed players (0 hp) must be revived by the partner standing within 2 cells for 45 ticks.
//!   While a player is down, the conduit cannot ignite.
//! * 12 waves ending with boss: THE NULL (26 hp, breeds Hunters).

use crate::core::{Dir, Field, Side, Vec2};
use crate::rng::Rng;

pub const TICK_MS: u64 = 60;
pub const TOTAL_WAVES: u32 = 12;
pub const MAX_HP: i32 = 3;

pub const HOT_DIST: i32 = 6;
pub const REST_DIST: i32 = 3;
pub const SNAP_DIST: i32 = 34;
pub const SNAP_COOLDOWN: u32 = 20;

pub const REVIVE_TICKS: u32 = 45;
pub const REVIVE_DIST: i32 = 2;

pub const CHARGE_MAX: f32 = 100.0;
pub const CHARGE_REST_RATE: f32 = 0.86;
pub const CHARGE_BASE_DRAIN: f32 = 0.13;
pub const CHARGE_DIST_DRAIN: f32 = 0.011;
pub const CHARGE_SNAP_PENALTY: f32 = 10.0;

// --------------------------------------------------------------------- kinds

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyKind {
    Drifter,
    Hunter,
    Splitter,
    Bulwark,
    Null,
}

impl EnemyKind {
    pub fn glyph(self) -> char {
        match self {
            EnemyKind::Drifter => '\u{00d7}',  // ×
            EnemyKind::Hunter => '\u{25bc}',   // ▼
            EnemyKind::Splitter => '\u{2739}', // ✹
            EnemyKind::Bulwark => '\u{25a0}',  // ■
            EnemyKind::Null => '\u{25c8}',     // ◈
        }
    }

    pub fn hp(self) -> i32 {
        match self {
            EnemyKind::Drifter => 1,
            EnemyKind::Hunter => 1,
            EnemyKind::Splitter => 2,
            EnemyKind::Bulwark => 3,
            EnemyKind::Null => 26,
        }
    }

    pub fn points(self) -> u64 {
        match self {
            EnemyKind::Drifter => 10,
            EnemyKind::Hunter => 15,
            EnemyKind::Splitter => 20,
            EnemyKind::Bulwark => 35,
            EnemyKind::Null => 250,
        }
    }

    pub fn charge(self) -> f32 {
        match self {
            EnemyKind::Drifter => 10.0,
            EnemyKind::Hunter => 11.0,
            EnemyKind::Splitter => 13.0,
            EnemyKind::Bulwark => 20.0,
            EnemyKind::Null => 0.0,
        }
    }

    pub fn cd(self) -> u32 {
        match self {
            EnemyKind::Drifter => 4,
            EnemyKind::Hunter => 4,
            EnemyKind::Splitter => 6,
            EnemyKind::Bulwark => 8,
            EnemyKind::Null => 5,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            EnemyKind::Drifter => "Drifter",
            EnemyKind::Hunter => "Hunter",
            EnemyKind::Splitter => "Splitter",
            EnemyKind::Bulwark => "Bulwark",
            EnemyKind::Null => "THE NULL",
        }
    }

    pub fn is_boss(self) -> bool {
        matches!(self, EnemyKind::Null)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OrbKind {
    Charge,
    Mend,
}

impl OrbKind {
    pub fn glyph(self) -> char {
        match self {
            OrbKind::Charge => '\u{25cb}', // ○
            OrbKind::Mend => '\u{271a}',   // ✚
        }
    }
}

// ------------------------------------------------------------------- entities

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Playing,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug)]
pub struct Player {
    pub side: Side,
    pub pos: Vec2,
    pub hp: i32,
    pub alive: bool,
    pub invuln: u32,
    pub step_cd: u32,
    pub facing: Dir,
    pub moving: bool,
    pub revive: u32,
    pub glow: u32,
}

impl Player {
    pub fn new(side: Side, pos: Vec2) -> Self {
        Self {
            side,
            pos,
            hp: MAX_HP,
            alive: true,
            invuln: 0,
            step_cd: 0,
            facing: match side {
                Side::One => Dir::Right,
                Side::Two => Dir::Left,
            },
            moving: false,
            revive: 0,
            glow: 0,
        }
    }

    pub fn down(&self) -> bool {
        !self.alive
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub pos: Vec2,
    pub kind: EnemyKind,
    pub hp: i32,
    pub max_hp: i32,
    pub flash: u32,
    pub cd: u32,
    pub dir: Dir,
    pub born: u64,
}

impl Enemy {
    pub fn new(pos: Vec2, kind: EnemyKind, dir: Dir, born: u64) -> Self {
        let hp = kind.hp();
        Self {
            pos,
            kind,
            hp,
            max_hp: hp,
            flash: 0,
            cd: kind.cd(),
            dir,
            born,
        }
    }

    pub fn noise(&self) -> u32 {
        (self.pos.x as u32)
            .wrapping_mul(31)
            .wrapping_add(self.pos.y as u32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Orb {
    pub pos: Vec2,
    pub kind: OrbKind,
    pub ttl: u32,
}

// -------------------------------------------------------------------- events

#[derive(Clone, Debug)]
pub enum Ev {
    Kill { at: Vec2, kind: EnemyKind, combo: u32 },
    Hit { at: Vec2, side: Side },
    Down { at: Vec2, side: Side },
    Revive { at: Vec2, player: usize },
    Snap { at: Vec2 },
    Pickup { at: Vec2, kind: OrbKind },
    Spawn { at: Vec2 },
    WaveClear { wave: u32, bonus: u64 },
    WaveStart { wave: u32 },
    Win,
    Lose,
}

// --------------------------------------------------------------------- World

pub struct World {
    pub field: Field,
    pub players: [Player; 2],
    pub enemies: Vec<Enemy>,
    pub orbs: Vec<Orb>,
    pub arc: Vec<Vec2>,
    pub hot: bool,
    pub dist: i32,
    pub charge: f32,
    pub phase: Phase,
    pub wave: u32,
    pub score: u64,
    pub kills: u64,
    pub combo: u32,
    pub combo_t: u32,
    pub best_combo: u32,
    pub queued: Vec<EnemyKind>,
    pub spawn_cd: u32,
    pub intermission: u32,
    pub shake: u32,
    pub snap_cd: u32,
    pub banner: Option<(String, u32)>,
    pub null_cd: u32,
    pub pulse: u32,
    pub rng: Rng,
    pub prev_dir: [Option<Dir>; 2],
}

impl World {
    pub fn new(field: Field) -> Self {
        let rng = Rng::from_time();
        let mid = field.center();
        let p1 = Player::new(Side::One, Vec2::new((mid.x - 8).max(1), mid.y));
        let p2 = Player::new(Side::Two, Vec2::new((mid.x + 8).min(field.w - 2), mid.y));
        let dist = p1.pos.cheb(p2.pos);
        let arc = crate::core::line(p1.pos, p2.pos);

        let mut w = Self {
            field,
            players: [p1, p2],
            enemies: Vec::with_capacity(64),
            orbs: Vec::with_capacity(32),
            arc,
            hot: false,
            dist,
            charge: CHARGE_MAX,
            phase: Phase::Playing,
            wave: 1,
            score: 0,
            kills: 0,
            combo: 0,
            combo_t: 0,
            best_combo: 0,
            queued: wave_queue(1),
            spawn_cd: 4,
            intermission: 0,
            shake: 0,
            snap_cd: 0,
            banner: Some(("WAVE 01 \u{00b7} SIGNAL FOUND".into(), 50)),
            null_cd: 6,
            pulse: 0,
            rng,
            prev_dir: [None, None],
        };
        w.update_conduit();
        w
    }

    pub fn score_mult(&self) -> u64 {
        1 + (self.combo.min(9) as u64 / 3)
    }

    pub fn alive_count(&self) -> usize {
        self.players.iter().filter(|p| p.alive).count()
    }

    pub fn alive_players(&self) -> impl Iterator<Item = &Player> {
        self.players.iter().filter(|p| p.alive)
    }

    pub fn boss(&self) -> Option<&Enemy> {
        self.enemies.iter().find(|e| e.kind.is_boss())
    }

    pub fn boss_progress(&self) -> Option<(f32, &'static str)> {
        self.boss().map(|b| (b.hp as f32 / b.max_hp as f32, b.kind.name()))
    }

    pub fn wave_is_boss(&self, wave: u32) -> bool {
        wave == TOTAL_WAVES
    }

    pub fn banner(&mut self, text: impl Into<String>, ttl: u32) {
        self.banner = Some((text.into(), ttl));
    }

    pub fn spawn_pos(&mut self) -> Vec2 {
        let f = self.field;
        let mut best = Vec2::new(1, 1);
        let mut best_dist = 0;

        for _ in 0..12 {
            let edge = self.rng.below(4);
            let p = match edge {
                0 => Vec2::new(self.rng.below(f.w.max(2)), 0),
                1 => Vec2::new(self.rng.below(f.w.max(2)), f.h - 1),
                2 => Vec2::new(0, self.rng.below(f.h.max(2))),
                _ => Vec2::new(f.w - 1, self.rng.below(f.h.max(2))),
            };

            let d = self
                .players
                .iter()
                .filter(|pl| pl.alive)
                .map(|pl| pl.pos.cheb(p))
                .min()
                .unwrap_or(20);

            if d > best_dist {
                best_dist = d;
                best = p;
            }
        }
        best
    }

    pub fn make_enemy(&mut self, kind: EnemyKind) -> Enemy {
        let pos = self.spawn_pos();
        let center = self.field.center();
        let dir = step_toward(pos, center);
        Enemy::new(pos, kind, dir, self.pulse as u64)
    }

    pub fn spawn_orbs(&mut self, num: usize) {
        if self.orbs.len() >= 4 {
            return;
        }
        let f = self.field;
        for _ in 0..num {
            let x = if f.w > 4 {
                self.rng.below(f.w - 4) + 2
            } else {
                f.w / 2
            };
            let y = if f.h > 2 {
                self.rng.below(f.h - 2) + 1
            } else {
                f.h / 2
            };
            let injured = self.players.iter().any(|p| p.alive && p.hp < MAX_HP);
            let mend_chance = if injured { 0.42 } else { 0.22 };
            let kind = if self.rng.chance(mend_chance) {
                OrbKind::Mend
            } else {
                OrbKind::Charge
            };
            self.orbs.push(Orb {
                pos: Vec2::new(x, y),
                kind,
                ttl: 300,
            });
        }
    }

    fn update_conduit(&mut self) {
        let p0 = self.players[0].pos;
        let p1 = self.players[1].pos;
        self.dist = p0.cheb(p1);
        self.arc = crate::core::line(p0, p1);

        let both_alive = self.players[0].alive && self.players[1].alive;
        if !both_alive {
            self.hot = false;
        } else if self.dist >= HOT_DIST {
            self.hot = true;
        } else {
            self.hot = false;
        }
    }

    pub fn step(&mut self, dirs: [Option<Dir>; 2], ev: &mut Vec<Ev>) {
        if self.phase != Phase::Playing {
            return;
        }

        self.pulse = self.pulse.wrapping_add(1);
        self.snap_cd = self.snap_cd.saturating_sub(1);
        self.shake = self.shake.saturating_sub(1);

        if let Some((_, ref mut ttl)) = self.banner {
            if *ttl > 0 {
                *ttl -= 1;
            } else {
                self.banner = None;
            }
        }

        // Combo decay
        if self.combo_t > 0 {
            self.combo_t -= 1;
        } else {
            self.combo = 0;
        }

        // 1. Move players
        for i in 0..2 {
            self.players[i].invuln = self.players[i].invuln.saturating_sub(1);
            self.players[i].glow = self.players[i].glow.saturating_sub(1);

            if !self.players[i].alive {
                continue;
            }

            if let Some(d) = dirs[i] {
                self.players[i].facing = d;
                self.players[i].moving = true;
                let next = self.players[i].pos.offset(d).clamp_to(self.field);
                self.players[i].pos = next;
            } else {
                self.players[i].moving = false;
            }
        }
        self.prev_dir = dirs;

        // 2. Revive check
        if self.players[0].alive != self.players[1].alive {
            let down_idx = if self.players[0].down() { 0 } else { 1 };
            let up_idx = 1 - down_idx;
            let d = self.players[0].pos.cheb(self.players[1].pos);

            if d <= REVIVE_DIST {
                self.players[down_idx].revive += 1;
                self.players[up_idx].glow = 4;
                if self.players[down_idx].revive >= REVIVE_TICKS {
                    self.players[down_idx].alive = true;
                    self.players[down_idx].hp = 1;
                    self.players[down_idx].invuln = 60;
                    self.players[down_idx].revive = 0;
                    ev.push(Ev::Revive {
                        at: self.players[down_idx].pos,
                        player: down_idx,
                    });
                    self.banner("RE-LINKED", 40);
                }
            } else {
                self.players[down_idx].revive = self.players[down_idx].revive.saturating_sub(1);
            }
        }

        // 3. Update conduit state & distance
        self.update_conduit();

        // 4. Snap check
        let both_alive = self.players[0].alive && self.players[1].alive;
        if both_alive && self.dist > SNAP_DIST && self.snap_cd == 0 {
            let mid = Vec2::new(
                (self.players[0].pos.x + self.players[1].pos.x) / 2,
                (self.players[0].pos.y + self.players[1].pos.y) / 2,
            );
            // Yank inward toward midpoint
            for p in &mut self.players {
                let dx = (mid.x - p.pos.x).clamp(-8, 8);
                let dy = (mid.y - p.pos.y).clamp(-4, 4);
                p.pos = Vec2::new(p.pos.x + dx, p.pos.y + dy).clamp_to(self.field);
            }
            self.charge = (self.charge - CHARGE_SNAP_PENALTY).max(0.0);
            self.shake = 8;
            self.snap_cd = SNAP_COOLDOWN;
            self.banner("CONDUIT SNAP", 45);
            ev.push(Ev::Snap { at: mid });

            self.update_conduit();
            if self.charge <= 0.0 {
                self.phase = Phase::Lost;
                ev.push(Ev::Lose);
                return;
            }
        }

        // 5. Charge economy
        if both_alive {
            if self.hot {
                let drain = CHARGE_BASE_DRAIN
                    + (self.dist - HOT_DIST).max(0) as f32 * CHARGE_DIST_DRAIN;
                self.charge = (self.charge - drain).max(0.0);
                if self.charge <= 0.0 {
                    self.hot = false;
                    self.phase = Phase::Lost;
                    ev.push(Ev::Lose);
                    return;
                }
            } else if self.dist <= REST_DIST {
                self.charge = (self.charge + CHARGE_REST_RATE).min(CHARGE_MAX);
            }
        }

        // 6. Conduit cutting damage & kill logic (±1 cell plasma halo prevents diagonal tunneling)
        if self.hot {
            let w = self.field.w;
            let h = self.field.h;
            let mut beam_grid = vec![false; (w * h) as usize];
            for p in &self.arc {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let nx = p.x + dx;
                        let ny = p.y + dy;
                        if nx >= 0 && ny >= 0 && nx < w && ny < h {
                            beam_grid[(ny * w + nx) as usize] = true;
                        }
                    }
                }
            }

            let mut killed_indices = Vec::new();
            for (idx, enemy) in self.enemies.iter_mut().enumerate() {
                if enemy.pos.within(self.field)
                    && beam_grid[(enemy.pos.y * w + enemy.pos.x) as usize]
                {
                    enemy.hp -= 1;
                    enemy.flash = 6;
                    if enemy.hp <= 0 {
                        killed_indices.push(idx);
                    }
                }
            }

            // Process kills
            if !killed_indices.is_empty() {
                let mut spawns = Vec::new();
                for &idx in killed_indices.iter().rev() {
                    let enemy = self.enemies.remove(idx);
                    self.on_kill(&enemy, ev, &mut spawns);
                }
                for s in spawns {
                    self.enemies.push(s);
                }
            }
        }

        // 7. Update enemies
        let mut enemy_spawns = Vec::new();
        let target_poses: Vec<Vec2> = self.alive_players().map(|p| p.pos).collect();
        let center = self.field.center();

        for e in self.enemies.iter_mut() {
            e.flash = e.flash.saturating_sub(1);
            if e.cd > 0 {
                e.cd -= 1;
                continue;
            }
            e.cd = e.kind.cd();

            match e.kind {
                EnemyKind::Drifter => {
                    let next = e.pos.offset(e.dir);
                    if !next.within(self.field) {
                        e.dir = step_toward(e.pos, center);
                    } else {
                        e.pos = next;
                        if self.pulse.wrapping_add(e.noise()) % 10 == 0 {
                            // Nudge inward along whichever axis pulls it into the arena.
                            let dx = center.x - e.pos.x;
                            let dy = center.y - e.pos.y;
                            e.dir = if e.pos.y <= 1 || e.pos.y >= self.field.h - 2 {
                                if dy > 0 { Dir::Down } else { Dir::Up }
                            } else if e.pos.x <= 1 || e.pos.x >= self.field.w - 2 {
                                if dx > 0 { Dir::Right } else { Dir::Left }
                            } else if dy.abs() > 2 && self.rng.chance(0.5) {
                                if dy > 0 { Dir::Down } else { Dir::Up }
                            } else {
                                step_toward(e.pos, center)
                            };
                        }
                    }
                }
                EnemyKind::Hunter | EnemyKind::Splitter | EnemyKind::Bulwark => {
                    if let Some(target) = nearest(e.pos, &target_poses) {
                        e.dir = step_toward(e.pos, target);
                        e.pos = e.pos.offset(e.dir).clamp_to(self.field);
                    }
                }
                EnemyKind::Null => {
                    if let Some(target) = nearest(e.pos, &target_poses) {
                        e.dir = step_toward(e.pos, target);
                        e.pos = e.pos.offset(e.dir).clamp_to(self.field);
                    }
                    if self.null_cd == 0 {
                        self.null_cd = 6;
                        let hp = e.pos;
                        enemy_spawns.push(Enemy::new(
                            hp,
                            EnemyKind::Hunter,
                            Dir::Down,
                            self.pulse as u64,
                        ));
                    } else {
                        self.null_cd -= 1;
                    }
                }
            }
        }
        for s in enemy_spawns {
            self.enemies.push(s);
        }

        // 8. Player-Enemy collision
        for p in self.players.iter_mut() {
            if !p.alive || p.invuln > 0 {
                continue;
            }
            if self.enemies.iter().any(|e| e.pos == p.pos) {
                p.hp -= 1;
                p.invuln = 45;
                ev.push(Ev::Hit {
                    at: p.pos,
                    side: p.side,
                });
                if p.hp <= 0 {
                    p.alive = false;
                    p.revive = 0;
                    ev.push(Ev::Down {
                        at: p.pos,
                        side: p.side,
                    });
                }
            }
        }

        if !self.players[0].alive && !self.players[1].alive {
            self.phase = Phase::Lost;
            ev.push(Ev::Lose);
            return;
        }

        // 9. Orbs
        let mut orb_consumed = Vec::new();
        for (idx, orb) in self.orbs.iter_mut().enumerate() {
            if orb.ttl > 0 {
                orb.ttl -= 1;
            }
            let picked_by = self
                .players
                .iter()
                .position(|p| p.alive && p.pos == orb.pos);
            if let Some(p_idx) = picked_by {
                orb_consumed.push(idx);
                match orb.kind {
                    OrbKind::Charge => {
                        self.charge = (self.charge + 26.0).min(CHARGE_MAX);
                    }
                    OrbKind::Mend => {
                        if self.players[p_idx].hp < MAX_HP {
                            self.players[p_idx].hp += 1;
                        } else {
                            let other = 1 - p_idx;
                            if self.players[other].alive && self.players[other].hp < MAX_HP {
                                self.players[other].hp += 1;
                            }
                        }
                    }
                }
                ev.push(Ev::Pickup {
                    at: orb.pos,
                    kind: orb.kind,
                });
            }
        }
        for &idx in orb_consumed.iter().rev() {
            self.orbs.remove(idx);
        }
        self.orbs.retain(|o| o.ttl > 0);

        // 10. Wave management
        self.update_waves(ev);
    }

    fn on_kill(&mut self, enemy: &Enemy, ev: &mut Vec<Ev>, spawns: &mut Vec<Enemy>) {
        self.kills += 1;
        self.combo += 1;
        self.combo_t = 60;
        self.best_combo = self.best_combo.max(self.combo);
        self.score += enemy.kind.points() * self.score_mult();
        self.charge = (self.charge + enemy.kind.charge()).min(CHARGE_MAX);

        ev.push(Ev::Kill {
            at: enemy.pos,
            kind: enemy.kind,
            combo: self.combo,
        });

        if enemy.kind == EnemyKind::Splitter {
            let d1 = Enemy::new(enemy.pos, EnemyKind::Drifter, Dir::Up, self.pulse as u64);
            let d2 = Enemy::new(enemy.pos, EnemyKind::Drifter, Dir::Down, self.pulse as u64);
            spawns.push(d1);
            spawns.push(d2);
        }

        if enemy.kind == EnemyKind::Null {
            self.banner("NULL SEVERED", 100);
            self.spawn_orbs(2);
        } else if self.rng.chance(0.14) {
            self.spawn_orbs(1);
        }
    }

    fn update_waves(&mut self, ev: &mut Vec<Ev>) {
        // Periodic field orb refresh so operators can detour for charge or hearts.
        if self.pulse % 110 == 0 && self.orbs.len() < 3 {
            self.spawn_orbs(1);
        }

        if self.intermission > 0 {
            self.intermission -= 1;
            return;
        }

        // Spawn queued enemies
        if self.spawn_cd > 0 {
            self.spawn_cd -= 1;
        } else if let Some(kind) = self.queued.pop() {
            let enemy = self.make_enemy(kind);
            ev.push(Ev::Spawn { at: enemy.pos });
            self.enemies.push(enemy);
            self.spawn_cd = 10;
        }

        // Wave cleared check
        if self.queued.is_empty() && self.enemies.is_empty() {
            if self.wave >= TOTAL_WAVES {
                self.phase = Phase::Won;
                ev.push(Ev::Win);
            } else {
                let bonus = self.wave as u64 * 100 * self.score_mult();
                self.score += bonus;
                ev.push(Ev::WaveClear {
                    wave: self.wave,
                    bonus,
                });

                self.spawn_orbs(1);
                self.wave += 1;
                self.queued = wave_queue(self.wave);
                self.intermission = 36;
                self.spawn_cd = 6;

                let title = if self.wave == TOTAL_WAVES {
                    "WAVE 12 · THE NULL ENGAGED".to_string()
                } else {
                    format!("WAVE {:02} · CONTACT", self.wave)
                };
                self.banner(title, 90);
                ev.push(Ev::WaveStart { wave: self.wave });
            }
        }
    }
}

// ------------------------------------------------------------------ helpers

pub fn nearest(from: Vec2, targets: &[Vec2]) -> Option<Vec2> {
    targets
        .iter()
        .copied()
        .min_by_key(|&t| from.cheb(t))
}

pub fn step_toward(cur: Vec2, target: Vec2) -> Dir {
    let dx = target.x - cur.x;
    let dy = target.y - cur.y;
    if dx.abs() >= dy.abs() {
        if dx > 0 {
            Dir::Right
        } else {
            Dir::Left
        }
    } else if dy > 0 {
        Dir::Down
    } else {
        Dir::Up
    }
}

pub fn wave_queue(wave: u32) -> Vec<EnemyKind> {
    use self::EnemyKind::*;
    match wave {
        1 => vec![Drifter, Drifter, Drifter, Drifter, Drifter],
        2 => vec![Drifter, Hunter, Drifter, Drifter, Hunter, Drifter, Hunter],
        3 => vec![Drifter, Hunter, Drifter, Splitter, Hunter, Drifter, Hunter],
        4 => vec![Hunter, Drifter, Splitter, Hunter, Splitter, Hunter, Drifter],
        5 => vec![Drifter, Hunter, Bulwark, Drifter, Hunter, Splitter, Hunter, Drifter],
        6 => vec![Hunter, Splitter, Bulwark, Hunter, Splitter, Drifter, Hunter],
        7 => vec![Hunter, Splitter, Bulwark, Hunter, Splitter, Bulwark, Hunter, Drifter],
        8 => vec![Drifter, Hunter, Splitter, Hunter, Bulwark, Drifter, Hunter, Splitter, Hunter],
        9 => vec![Splitter, Bulwark, Hunter, Splitter, Bulwark, Hunter, Splitter, Hunter],
        10 => vec![Hunter, Bulwark, Splitter, Bulwark, Hunter, Splitter, Bulwark, Hunter],
        11 => vec![Hunter, Splitter, Bulwark, Splitter, Bulwark, Hunter, Splitter, Bulwark, Hunter],
        // Note: `Vec::pop()` pops from the end, so Null spawns first!
        12 => vec![Hunter, Hunter, Bulwark, Hunter, Bulwark, Hunter, Null],
        _ => vec![Hunter, Hunter, Bulwark, Null],
    }
}

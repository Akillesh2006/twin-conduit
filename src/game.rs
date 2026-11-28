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

use crate::core::{Dir, Field, Side, Vec2, Vec2f};
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
pub const CHARGE_REST_RATE: f32 = 0.52;
pub const CHARGE_BASE_DRAIN: f32 = 0.28;
pub const CHARGE_DIST_DRAIN: f32 = 0.022;
pub const CHARGE_SNAP_PENALTY: f32 = 14.0;

// ---------------------------------------------------------------- difficulty

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
    Nightmare,
}

#[derive(Clone, Copy, Debug)]
pub struct DiffSpec {
    pub drain_mult: f32,
    pub recharge_mult: f32,
    pub hp_mult: f32,
    pub cd_mult: f32,
    pub spawn_mult: f32,
    pub proj_cd_mult: f32,
    pub count_mult: f32,
    pub score_mult: f32,
    pub start_hp: i32,
    pub name: &'static str,
    pub short: &'static str,
    pub blurb: &'static str,
}

impl Difficulty {
    pub const ALL: [Difficulty; 4] = [
        Difficulty::Easy,
        Difficulty::Normal,
        Difficulty::Hard,
        Difficulty::Nightmare,
    ];

    pub fn idx(self) -> usize {
        match self {
            Difficulty::Easy => 0,
            Difficulty::Normal => 1,
            Difficulty::Hard => 2,
            Difficulty::Nightmare => 3,
        }
    }

    pub fn from_idx(i: usize) -> Self {
        Self::ALL[i % Self::ALL.len()]
    }

    pub fn cycle(self, delta: i32) -> Self {
        let n = Self::ALL.len() as i32;
        let next = (self.idx() as i32 + delta).rem_euclid(n) as usize;
        Self::ALL[next]
    }

    /// Tuning multipliers for this difficulty preset.
    pub fn spec(self) -> DiffSpec {
        match self {
            Difficulty::Easy => DiffSpec {
                drain_mult: 0.52,
                recharge_mult: 1.45,
                hp_mult: 0.85,
                cd_mult: 1.25,
                spawn_mult: 1.25,
                proj_cd_mult: 1.35,
                count_mult: 0.85,
                score_mult: 0.80,
                start_hp: 4,
                name: "EASY",
                short: "EASY",
                blurb: "Forgiving charge, slower hordes, 4 hearts",
            },
            Difficulty::Normal => DiffSpec {
                drain_mult: 1.00,
                recharge_mult: 1.00,
                hp_mult: 1.00,
                cd_mult: 1.00,
                spawn_mult: 1.00,
                proj_cd_mult: 1.00,
                count_mult: 1.00,
                score_mult: 1.00,
                start_hp: 3,
                name: "NORMAL",
                short: "NORM",
                blurb: "Tight charge economy, aggressive waves, 3 hearts",
            },
            Difficulty::Hard => DiffSpec {
                drain_mult: 1.25,
                recharge_mult: 0.82,
                hp_mult: 1.25,
                cd_mult: 0.84,
                spawn_mult: 0.82,
                proj_cd_mult: 0.80,
                count_mult: 1.25,
                score_mult: 1.35,
                start_hp: 3,
                name: "HARD",
                short: "HARD",
                blurb: "Fast drain, tough elites, dense fire, 3 hearts",
            },
            Difficulty::Nightmare => DiffSpec {
                drain_mult: 1.50,
                recharge_mult: 0.68,
                hp_mult: 1.50,
                cd_mult: 0.70,
                spawn_mult: 0.68,
                proj_cd_mult: 0.65,
                count_mult: 1.45,
                score_mult: 1.80,
                start_hp: 2,
                name: "NIGHTMARE",
                short: "NGHT",
                blurb: "Brutal drain, relentless swarms, 2 hearts",
            },
        }
    }
}

// --------------------------------------------------------------------- kinds

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyKind {
    Drifter,
    Shooter,
    Hunter,
    Dasher,
    Splitter,
    Shielded,
    Leech,
    Bomber,
    Bulwark,
    Warden,
    Hydra,
    Null,
}

impl EnemyKind {
    pub fn glyph(self) -> char {
        match self {
            EnemyKind::Drifter => '\u{00d7}',  // ×
            EnemyKind::Shooter => '\u{2666}',  // ♦
            EnemyKind::Hunter => '\u{25bc}',   // ▼
            EnemyKind::Dasher => '\u{00bb}',   // »
            EnemyKind::Splitter => '\u{2739}', // ✹
            EnemyKind::Shielded => '\u{25ce}', // ◎
            EnemyKind::Leech => '\u{03a8}',    // Ψ
            EnemyKind::Bomber => '\u{263c}',   // ☼
            EnemyKind::Bulwark => '\u{25a0}',  // ■
            EnemyKind::Warden => '\u{25a3}',   // ▣
            EnemyKind::Hydra => '\u{2726}',    // ✦
            EnemyKind::Null => '\u{25c8}',     // ◈
        }
    }

    pub fn hp(self) -> i32 {
        match self {
            EnemyKind::Drifter => 1,
            EnemyKind::Shooter => 2,
            EnemyKind::Hunter => 1,
            EnemyKind::Dasher => 2,
            EnemyKind::Splitter => 2,
            EnemyKind::Shielded => 3,
            EnemyKind::Leech => 3,
            EnemyKind::Bomber => 2,
            EnemyKind::Bulwark => 3,
            EnemyKind::Warden => 15,
            EnemyKind::Hydra => 21,
            EnemyKind::Null => 30,
        }
    }

    pub fn points(self) -> u64 {
        match self {
            EnemyKind::Drifter => 10,
            EnemyKind::Shooter => 25,
            EnemyKind::Hunter => 15,
            EnemyKind::Dasher => 28,
            EnemyKind::Splitter => 20,
            EnemyKind::Shielded => 35,
            EnemyKind::Leech => 32,
            EnemyKind::Bomber => 30,
            EnemyKind::Bulwark => 35,
            EnemyKind::Warden => 150,
            EnemyKind::Hydra => 220,
            EnemyKind::Null => 350,
        }
    }

    pub fn charge(self) -> f32 {
        match self {
            EnemyKind::Drifter => 4.5,
            EnemyKind::Shooter => 6.0,
            EnemyKind::Hunter => 5.5,
            EnemyKind::Dasher => 6.5,
            EnemyKind::Splitter => 6.5,
            EnemyKind::Shielded => 8.5,
            EnemyKind::Leech => 8.0,
            EnemyKind::Bomber => 7.0,
            EnemyKind::Bulwark => 10.0,
            EnemyKind::Warden => 25.0,
            EnemyKind::Hydra => 30.0,
            EnemyKind::Null => 0.0,
        }
    }

    /// Wave and difficulty scaled hit points.
    pub fn scaled_hp(self, wave: u32, diff: Difficulty) -> i32 {
        let base = self.hp() as f32;
        let wave_factor = if self.is_boss() {
            1.0
        } else {
            1.0 + (wave.saturating_sub(1) as f32) * 0.055
        };
        let mult = diff.spec().hp_mult;
        if self.hp() == 1 {
            // Keep basic 1-HP units crisp early, scaling to 2 only at extreme wave*difficulty.
            if wave >= 9 && mult >= 1.25 {
                2
            } else {
                1
            }
        } else {
            (base * wave_factor * mult).round().max(1.0) as i32
        }
    }

    /// Wave and difficulty scaled move cooldown (ticks waited between steps).
    pub fn scaled_cd(self, wave: u32, diff: Difficulty) -> u32 {
        let base = self.cd() as f32;
        let wave_trim = (wave.saturating_sub(1) as f32) * 0.11;
        ((base * diff.spec().cd_mult) - wave_trim).round().max(3.0) as u32
    }

    pub fn cd(self) -> u32 {
        match self {
            EnemyKind::Drifter => 5,
            EnemyKind::Shooter => 6,
            EnemyKind::Hunter => 5,
            EnemyKind::Dasher => 6,
            EnemyKind::Splitter => 7,
            EnemyKind::Shielded => 7,
            EnemyKind::Leech => 5,
            EnemyKind::Bomber => 6,
            EnemyKind::Bulwark => 9,
            EnemyKind::Warden => 7,
            EnemyKind::Hydra => 6,
            EnemyKind::Null => 6,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            EnemyKind::Drifter => "Drifter",
            EnemyKind::Shooter => "Shooter",
            EnemyKind::Hunter => "Hunter",
            EnemyKind::Dasher => "Dasher",
            EnemyKind::Splitter => "Splitter",
            EnemyKind::Shielded => "Shielded",
            EnemyKind::Leech => "Leech",
            EnemyKind::Bomber => "Bomber",
            EnemyKind::Bulwark => "Bulwark",
            EnemyKind::Warden => "THE WARDEN",
            EnemyKind::Hydra => "THE HYDRA",
            EnemyKind::Null => "THE NULL",
        }
    }

    pub fn is_boss(self) -> bool {
        matches!(self, EnemyKind::Warden | EnemyKind::Hydra | EnemyKind::Null)
    }

    pub fn total_phases(self) -> u8 {
        match self {
            EnemyKind::Warden | EnemyKind::Hydra => 2,
            EnemyKind::Null => 3,
            _ => 1,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OrbKind {
    Charge,
    Mend,
    SlowTime,
    Nova,
    TwinBeam,
}

impl OrbKind {
    pub fn glyph(self) -> char {
        match self {
            OrbKind::Charge => '\u{25cb}',   // ○
            OrbKind::Mend => '\u{271a}',     // ✚
            OrbKind::SlowTime => '\u{00a4}', // ¤
            OrbKind::Nova => '\u{2605}',     // ★
            OrbKind::TwinBeam => '\u{2261}', // ≡
        }
    }

    pub fn is_special(self) -> bool {
        matches!(self, OrbKind::SlowTime | OrbKind::Nova | OrbKind::TwinBeam)
    }
}

/// Shared cooperative upgrades drafted between waves (including synergies and trade-offs).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Upgrade {
    WideHalo,
    InsulatedCoil,
    RapidReactor,
    OverclockBeam,
    EmergencyRevive,
    OrbMagnet,
    WaveBarrier,
    ArcChain,
    ElasticTether,
}

impl Upgrade {
    pub const ALL: [Upgrade; 9] = [
        Upgrade::WideHalo,
        Upgrade::InsulatedCoil,
        Upgrade::RapidReactor,
        Upgrade::OverclockBeam,
        Upgrade::EmergencyRevive,
        Upgrade::OrbMagnet,
        Upgrade::WaveBarrier,
        Upgrade::ArcChain,
        Upgrade::ElasticTether,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Upgrade::WideHalo => "PLASMA HALO",
            Upgrade::InsulatedCoil => "INSULATED COIL",
            Upgrade::RapidReactor => "RAPID REACTOR",
            Upgrade::OverclockBeam => "OVERCLOCK ARC",
            Upgrade::EmergencyRevive => "FAST RELINK",
            Upgrade::OrbMagnet => "ORB MAGNET",
            Upgrade::WaveBarrier => "PHASE SHIELD",
            Upgrade::ArcChain => "CHAIN ARC",
            Upgrade::ElasticTether => "ELASTIC TETHER",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Upgrade::WideHalo => "HALO",
            Upgrade::InsulatedCoil => "COIL",
            Upgrade::RapidReactor => "REACTOR",
            Upgrade::OverclockBeam => "OVERCLK",
            Upgrade::EmergencyRevive => "RELINK",
            Upgrade::OrbMagnet => "MAGNET",
            Upgrade::WaveBarrier => "SHIELD",
            Upgrade::ArcChain => "CHAIN",
            Upgrade::ElasticTether => "REACH",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Upgrade::WideHalo => "+1 beam halo radius while stretched >= 10",
            Upgrade::InsulatedCoil => "-22% stretch drain (stacks)",
            Upgrade::RapidReactor => "+28% hug recharge rate (stacks)",
            Upgrade::OverclockBeam => "2x beam damage per tick, +25% drain [trade-off]",
            Upgrade::EmergencyRevive => "Revive in 24 ticks with 2 HP",
            Upgrade::OrbMagnet => "Pulls nearby orbs within 6 cells",
            Upgrade::WaveBarrier => "Blocks 1 hit per wave for each operator",
            Upgrade::ArcChain => "Kills arc 1 dmg to 2 nearby foes (4 cells)",
            Upgrade::ElasticTether => "+6 snap limit (40), -12% recharge [trade-off]",
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
    pub barrier: u32,
}

impl Player {
    pub fn new(side: Side, pos: Vec2, hp: i32) -> Self {
        Self {
            side,
            pos,
            hp,
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
            barrier: 0,
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
    pub move_cd: u32,
    pub dir: Dir,
    pub born: u64,
    pub action_cd: u32,
    pub telegraph: u32,
    pub dash_left: u32,
    pub aim: Vec2,
    pub dash_pos: Vec2f,
    pub latched: bool,
    pub shield_cd: u32,
    pub phase_seen: u8,
    pub spin: f32,
}

impl Enemy {
    pub fn new(pos: Vec2, kind: EnemyKind, dir: Dir, born: u64) -> Self {
        Self::scaled(pos, kind, dir, born, 1, Difficulty::Normal)
    }

    pub fn scaled(pos: Vec2, kind: EnemyKind, dir: Dir, born: u64, wave: u32, diff: Difficulty) -> Self {
        let hp = kind.scaled_hp(wave, diff);
        let move_cd = kind.scaled_cd(wave, diff);
        let proj_cd = scaled_proj_cd(wave, diff);
        let action_cd = match kind {
            EnemyKind::Shooter => (proj_cd / 2).max(10),
            EnemyKind::Dasher => 18,
            EnemyKind::Warden | EnemyKind::Hydra | EnemyKind::Null => 20,
            _ => 0,
        };
        Self {
            pos,
            kind,
            hp,
            max_hp: hp,
            flash: 0,
            cd: move_cd,
            move_cd,
            dir,
            born,
            action_cd,
            telegraph: 0,
            dash_left: 0,
            aim: pos,
            dash_pos: Vec2f::from_cell(pos),
            latched: false,
            shield_cd: 0,
            phase_seen: 1,
            spin: 0.0,
        }
    }

    /// Current boss phase (1..=total_phases) based on remaining HP fraction.
    pub fn boss_phase(&self) -> (u8, u8) {
        let total = self.kind.total_phases();
        if total <= 1 || self.max_hp <= 0 {
            return (1, 1);
        }
        let frac = self.hp as f32 / self.max_hp as f32;
        let cur = if total == 3 {
            if frac > 0.66 {
                1
            } else if frac > 0.33 {
                2
            } else {
                3
            }
        } else if frac > 0.50 {
            1
        } else {
            2
        };
        (cur, total)
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

/// Enemy projectile moving at sub-cell speed so players have time to dodge.
#[derive(Clone, Copy, Debug)]
pub struct Bullet {
    pub pos: Vec2f,
    pub vel: Vec2f,
    pub ttl: u32,
    pub heavy: bool,
    pub from_boss: bool,
}

/// Telegraphed area explosion (from Bomber death/contact or boss ordnance).
#[derive(Clone, Copy, Debug)]
pub struct Blast {
    pub center: Vec2,
    pub radius: i32,
    pub fuse: u32,
    pub max_fuse: u32,
    pub dmg_players: bool,
    pub dmg_enemies: i32,
}

/// Telegraphed arena laser sweep that damages players and can sever the conduit.
#[derive(Clone, Debug)]
pub struct BeamHazard {
    pub p0: Vec2,
    pub p1: Vec2,
    pub pts: Vec<Vec2>,
    pub fuse: u32,
    pub max_fuse: u32,
    pub cuts_tether: bool,
}

/// Patrolling obstacle barrier that blocks movement, shots, and the conduit beam.
#[derive(Clone, Copy, Debug)]
pub struct Wall {
    pub pos: Vec2,
    pub len: i32,
    pub vertical: bool,
    pub vel: i32,
    pub min_c: i32,
    pub max_c: i32,
}

impl Wall {
    pub fn contains(&self, p: Vec2) -> bool {
        if self.vertical {
            p.x == self.pos.x && p.y >= self.pos.y && p.y < self.pos.y + self.len
        } else {
            p.y == self.pos.y && p.x >= self.pos.x && p.x < self.pos.x + self.len
        }
    }

    pub fn step_patrol(&mut self) {
        if self.vel == 0 {
            return;
        }
        if self.vertical {
            let next = self.pos.y + self.vel;
            if next < self.min_c || next + self.len - 1 > self.max_c {
                self.vel = -self.vel;
            } else {
                self.pos.y = next;
            }
        } else {
            let next = self.pos.x + self.vel;
            if next < self.min_c || next + self.len - 1 > self.max_c {
                self.vel = -self.vel;
            } else {
                self.pos.x = next;
            }
        }
    }
}

/// Telegraphed floor hazard zone that heats up and damages lingering operators.
#[derive(Clone, Copy, Debug)]
pub struct HazardZone {
    pub pos: Vec2,
    pub w: i32,
    pub h: i32,
    pub warn: u32,
    pub active: u32,
}

impl HazardZone {
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.pos.x
            && p.x < self.pos.x + self.w
            && p.y >= self.pos.y
            && p.y < self.pos.y + self.h
    }
}

/// Random wave modifier introducing tactical twists across non-boss waves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WaveEvent {
    None,
    MeteorShower,
    Blackout,
    EnemySurge,
    Drought,
}

impl WaveEvent {
    pub fn label(self) -> &'static str {
        match self {
            WaveEvent::None => "",
            WaveEvent::MeteorShower => "METEOR SHOWER",
            WaveEvent::Blackout => "BLACKOUT",
            WaveEvent::EnemySurge => "ENEMY SURGE",
            WaveEvent::Drought => "CHARGE DROUGHT",
        }
    }

    pub fn roll(wave: u32, rng: &mut Rng) -> Self {
        if wave <= 1 || wave == 4 || wave == 8 || wave >= TOTAL_WAVES {
            return WaveEvent::None;
        }
        let chance = if wave == 2 { 0.40 } else { 0.65 };
        if !rng.chance(chance) {
            return WaveEvent::None;
        }
        match rng.below(4) {
            0 => WaveEvent::MeteorShower,
            1 => WaveEvent::Blackout,
            2 => WaveEvent::EnemySurge,
            _ => WaveEvent::Drought,
        }
    }
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
    Shoot { at: Vec2 },
    Dash { at: Vec2 },
    Block { at: Vec2 },
    Explode { at: Vec2, radius: i32 },
    LeechDrain { at: Vec2 },
    PhaseShift { at: Vec2, kind: EnemyKind, phase: u8 },
    TetherCut { at: Vec2 },
    ChainArc { from: Vec2, to: Vec2 },
    WaveClear { wave: u32, bonus: u64 },
    WaveStart { wave: u32 },
    Win,
    Lose,
}

// --------------------------------------------------------------------- World

pub struct World {
    pub field: Field,
    pub difficulty: Difficulty,
    pub max_hp: i32,
    pub players: [Player; 2],
    pub enemies: Vec<Enemy>,
    pub orbs: Vec<Orb>,
    pub bullets: Vec<Bullet>,
    pub blasts: Vec<Blast>,
    pub beam_hazards: Vec<BeamHazard>,
    pub upgrades: Vec<Upgrade>,
    pub upgrade_choices: [Upgrade; 3],
    pub draft_pending: bool,
    pub slow_t: u32,
    pub twin_t: u32,
    pub walls: Vec<Wall>,
    pub hazards: Vec<HazardZone>,
    pub event: WaveEvent,
    pub blocked_at: Option<Vec2>,
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
    pub ticks: u32,
    pub move_tick: [u32; 2],
    pub synced: bool,
    pub rng: Rng,
    pub prev_dir: [Option<Dir>; 2],
}

impl World {
    pub fn new(field: Field) -> Self {
        Self::new_with(field, Difficulty::Normal)
    }

    pub fn new_with(field: Field, difficulty: Difficulty) -> Self {
        let rng = Rng::from_time();
        let mid = field.center();
        let start_hp = difficulty.spec().start_hp;
        let p1 = Player::new(Side::One, Vec2::new((mid.x - 8).max(1), mid.y), start_hp);
        let p2 = Player::new(Side::Two, Vec2::new((mid.x + 8).min(field.w - 2), mid.y), start_hp);
        let dist = p1.pos.cheb(p2.pos);
        let arc = crate::core::line(p1.pos, p2.pos);

        let mut w = Self {
            field,
            difficulty,
            max_hp: start_hp,
            players: [p1, p2],
            enemies: Vec::with_capacity(64),
            orbs: Vec::with_capacity(32),
            bullets: Vec::with_capacity(64),
            blasts: Vec::with_capacity(16),
            beam_hazards: Vec::with_capacity(6),
            upgrades: Vec::with_capacity(12),
            upgrade_choices: [
                Upgrade::InsulatedCoil,
                Upgrade::RapidReactor,
                Upgrade::WideHalo,
            ],
            draft_pending: false,
            slow_t: 0,
            twin_t: 0,
            walls: Vec::new(),
            hazards: Vec::new(),
            event: WaveEvent::None,
            blocked_at: None,
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
            queued: scaled_wave_queue(1, difficulty),
            spawn_cd: 4,
            intermission: 0,
            shake: 0,
            snap_cd: 0,
            banner: Some(("WAVE 01 \u{00b7} FIRST SPARK".into(), 50)),
            null_cd: 6,
            pulse: 0,
            ticks: 0,
            move_tick: [0, 0],
            synced: false,
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

    /// Detailed boss telemetry: `(hp_fraction, name, current_phase, total_phases)`.
    pub fn boss_info(&self) -> Option<(f32, &'static str, u8, u8)> {
        self.boss().map(|b| {
            let (cur, total) = b.boss_phase();
            (
                (b.hp as f32 / b.max_hp as f32).clamp(0.0, 1.0),
                b.kind.name(),
                cur,
                total,
            )
        })
    }

    pub fn wave_is_boss(&self, wave: u32) -> bool {
        wave == 4 || wave == 8 || wave == TOTAL_WAVES
    }

    /// Fire an aimed fan of bullets from `from` toward `target`.
    fn fire_fan(bullets: &mut Vec<Bullet>, from: Vec2, target: Vec2, rays: usize, spread: f32, speed: f32, from_boss: bool) {
        let f = Vec2f::from_cell(from);
        let d = Vec2f::from_cell(target) - f;
        let base_ang = d.y.atan2(d.x);
        let half = (rays.saturating_sub(1) as f32) * 0.5;
        for i in 0..rays {
            let ang = base_ang + ((i as f32) - half) * spread;
            let vel = Vec2f::new(ang.cos() * speed, ang.sin() * speed * 0.72);
            bullets.push(Bullet {
                pos: f + vel,
                vel,
                ttl: 120,
                heavy: from_boss,
                from_boss,
            });
        }
    }

    /// Fire an evenly spaced ring of bullets around `from`.
    fn fire_ring(bullets: &mut Vec<Bullet>, from: Vec2, count: usize, offset: f32, speed: f32) {
        let f = Vec2f::from_cell(from);
        let step = std::f32::consts::TAU / count.max(1) as f32;
        for i in 0..count {
            let ang = offset + step * (i as f32);
            let vel = Vec2f::new(ang.cos() * speed, ang.sin() * speed * 0.72);
            bullets.push(Bullet {
                pos: f + vel,
                vel,
                ttl: 130,
                heavy: true,
                from_boss: true,
            });
        }
    }

    fn make_beam_hazard(&self, p0: Vec2, p1: Vec2, fuse: u32, cuts_tether: bool) -> BeamHazard {
        let pts = crate::core::line(p0, p1);
        BeamHazard {
            p0,
            p1,
            pts,
            fuse,
            max_fuse: fuse,
            cuts_tether,
        }
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
        self.make_enemy_at(kind, pos)
    }

    pub fn make_enemy_at(&mut self, kind: EnemyKind, pos: Vec2) -> Enemy {
        let center = self.field.center();
        let dir = step_toward(pos, center);
        Enemy::scaled(pos, kind, dir, self.pulse as u64, self.wave, self.difficulty)
    }

    /// Opposite/flanking edge position for coordinated wave formations.
    fn formation_pos(&mut self, slot: usize, count: usize, anchor_edge: i32) -> Vec2 {
        if count <= 1 || slot == 0 {
            return self.spawn_pos();
        }
        let f = self.field;
        let edge = (anchor_edge + slot as i32 * 2) % 4;
        for _ in 0..8 {
            let p = match edge {
                0 => Vec2::new(self.rng.below(f.w.max(2)), 0),
                1 => Vec2::new(f.w - 1, self.rng.below(f.h.max(2))),
                2 => Vec2::new(self.rng.below(f.w.max(2)), f.h - 1),
                _ => Vec2::new(0, self.rng.below(f.h.max(2))),
            };
            let min_d = self
                .players
                .iter()
                .filter(|pl| pl.alive)
                .map(|pl| pl.pos.cheb(p))
                .min()
                .unwrap_or(10);
            if min_d >= 7 {
                return p;
            }
        }
        self.spawn_pos()
    }

    /// True if a Shielded enemy's front armor blocks the current beam.
    pub fn shield_blocks(&self, enemy: &Enemy) -> bool {
        if enemy.kind != EnemyKind::Shielded || self.dist >= 16 {
            return false;
        }
        let mid_x = (self.players[0].pos.x + self.players[1].pos.x) / 2;
        let mid_y = (self.players[0].pos.y + self.players[1].pos.y) / 2;
        let vx = mid_x - enemy.pos.x;
        let vy = mid_y - enemy.pos.y;
        let f = enemy.dir.delta();
        (vx * f.x + vy * f.y) > 0
    }

    pub fn elapsed_secs(&self) -> u32 {
        ((self.ticks as u64 * TICK_MS) / 1000) as u32
    }

    /// Count stacks held of a given upgrade.
    pub fn upgrade_count(&self, u: Upgrade) -> usize {
        self.upgrades.iter().filter(|&&x| x == u).count()
    }

    pub fn max_snap_dist(&self) -> i32 {
        SNAP_DIST + (self.upgrade_count(Upgrade::ElasticTether) as i32 * 6)
    }

    pub fn revive_ticks_req(&self) -> u32 {
        if self.upgrade_count(Upgrade::EmergencyRevive) > 0 {
            24
        } else {
            REVIVE_TICKS
        }
    }

    pub fn beam_damage(&self) -> i32 {
        1 + self.upgrade_count(Upgrade::OverclockBeam) as i32
    }

    pub fn drain_upgrade_mult(&self) -> f32 {
        let ins = self.upgrade_count(Upgrade::InsulatedCoil) as i32;
        let ovc = self.upgrade_count(Upgrade::OverclockBeam) as i32;
        0.78_f32.powi(ins) * 1.25_f32.powi(ovc)
    }

    pub fn recharge_upgrade_mult(&self) -> f32 {
        let rap = self.upgrade_count(Upgrade::RapidReactor) as i32;
        let ela = self.upgrade_count(Upgrade::ElasticTether) as i32;
        1.28_f32.powi(rap) * 0.88_f32.powi(ela)
    }

    /// Parallel second beam points when a `TwinBeam` pickup is active (clipped by walls).
    pub fn twin_arc(&self) -> Vec<Vec2> {
        if self.twin_t == 0 || !self.hot {
            return Vec::new();
        }
        let d = self.players[1].pos - self.players[0].pos;
        let off = if d.x.abs() >= d.y.abs() {
            Vec2::new(0, 2)
        } else {
            Vec2::new(2, 0)
        };
        let a = (self.players[0].pos + off).clamp_to(self.field);
        let b = (self.players[1].pos + off).clamp_to(self.field);
        let raw = crate::core::line(a, b);
        if let Some(hit) = raw.iter().position(|&pt| self.wall_at(pt)) {
            raw[..hit].to_vec()
        } else {
            raw
        }
    }

    pub fn wall_at(&self, p: Vec2) -> bool {
        self.walls.iter().any(|w| w.contains(p))
    }

    fn spawn_hazard_zone(&mut self) {
        if self.hazards.len() >= 2 || self.field.w < 12 || self.field.h < 8 {
            return;
        }
        for _ in 0..10 {
            let x = 2 + self.rng.below((self.field.w - 6).max(1));
            let y = 2 + self.rng.below((self.field.h - 5).max(1));
            let hz = HazardZone {
                pos: Vec2::new(x, y),
                w: 3,
                h: 2,
                warn: 26,
                active: 48,
            };
            let clear_players = self
                .players
                .iter()
                .filter(|p| p.alive)
                .all(|p| !hz.contains(p.pos));
            if clear_players && !self.wall_at(hz.pos) {
                self.hazards.push(hz);
                return;
            }
        }
    }

    /// Roll 3 distinct upgrades for the between-wave draft.
    pub fn roll_upgrade_choices(&mut self) -> [Upgrade; 3] {
        let mut pool = Upgrade::ALL.to_vec();
        self.rng.shuffle(&mut pool);
        [pool[0], pool[1], pool[2]]
    }

    /// Commit player upgrade pick (`0..=2`) and activate immediate wave effects.
    pub fn pick_upgrade(&mut self, slot: usize) -> Upgrade {
        let chosen = self.upgrade_choices[slot.min(2)];
        self.upgrades.push(chosen);
        self.draft_pending = false;
        if chosen == Upgrade::WaveBarrier {
            let cap = self.upgrade_count(Upgrade::WaveBarrier).min(2) as u32;
            for p in &mut self.players {
                if p.alive {
                    p.barrier = p.barrier.max(cap);
                }
            }
        }
        chosen
    }

    /// Refresh per-wave absorbing barriers if `WaveBarrier` was drafted.
    fn refresh_wave_barriers(&mut self) {
        let cap = self.upgrade_count(Upgrade::WaveBarrier).min(2) as u32;
        if cap > 0 {
            for p in &mut self.players {
                if p.alive {
                    p.barrier = p.barrier.max(cap);
                }
            }
        }
    }

    /// Apply 1 HP of damage to player `idx` if alive and not invulnerable (barrier absorbs first).
    fn damage_player(&mut self, idx: usize, invuln_ticks: u32, ev: &mut Vec<Ev>) {
        let p = &mut self.players[idx];
        if !p.alive || p.invuln > 0 {
            return;
        }
        if p.barrier > 0 {
            p.barrier -= 1;
            p.invuln = 30;
            ev.push(Ev::Block { at: p.pos });
            return;
        }
        p.hp -= 1;
        p.invuln = invuln_ticks;
        self.shake = self.shake.max(4);
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
            let injured = self.players.iter().any(|p| p.alive && p.hp < self.max_hp);
            let roll = self.rng.f32();
            let kind = if roll < 0.16 {
                match self.rng.below(3) {
                    0 => OrbKind::SlowTime,
                    1 => OrbKind::Nova,
                    _ => OrbKind::TwinBeam,
                }
            } else if roll < if injured { 0.52 } else { 0.34 } {
                OrbKind::Mend
            } else {
                OrbKind::Charge
            };
            let pos = Vec2::new(x, y);
            if self.wall_at(pos) {
                continue;
            }
            self.orbs.push(Orb {
                pos,
                kind,
                ttl: 300,
            });
        }
    }

    fn update_conduit(&mut self) {
        let p0 = self.players[0].pos;
        let p1 = self.players[1].pos;
        self.dist = p0.cheb(p1);
        let raw = crate::core::line(p0, p1);
        if let Some(hit_idx) = raw.iter().position(|&pt| self.wall_at(pt)) {
            self.blocked_at = Some(raw[hit_idx]);
            self.arc = raw[..hit_idx].to_vec();
        } else {
            self.blocked_at = None;
            self.arc = raw;
        }

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
        self.ticks = self.ticks.saturating_add(1);
        self.snap_cd = self.snap_cd.saturating_sub(1);
        self.shake = self.shake.saturating_sub(1);
        self.slow_t = self.slow_t.saturating_sub(1);
        self.twin_t = self.twin_t.saturating_sub(1);

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

        // 0b. Patrol walls & periodically spawn hazard zones / wave meteors
        if self.pulse % 8 == 0 {
            for w in &mut self.walls {
                w.step_patrol();
            }
            for p in &mut self.players {
                if p.alive && self.walls.iter().any(|w| w.contains(p.pos)) {
                    let alt = [Dir::Left, Dir::Right, Dir::Up, Dir::Down]
                        .into_iter()
                        .map(|d| p.pos.offset(d).clamp_to(self.field))
                        .find(|&cand| !self.walls.iter().any(|w| w.contains(cand)));
                    if let Some(pos) = alt {
                        p.pos = pos;
                    }
                }
            }
        }
        if self.wave >= 5 && self.pulse % 135 == 0 {
            self.spawn_hazard_zone();
        }
        if self.event == WaveEvent::MeteorShower && self.intermission == 0 && self.pulse % 36 == 0 {
            let x = 2 + self.rng.below((self.field.w - 4).max(1));
            let y = 2 + self.rng.below((self.field.h - 4).max(1));
            let mp = Vec2::new(x, y);
            if !self.wall_at(mp) {
                self.blasts.push(Blast {
                    center: mp,
                    radius: 1,
                    fuse: 22,
                    max_fuse: 22,
                    dmg_players: true,
                    dmg_enemies: 2,
                });
            }
        }

        // 1. Move players (respect patrolling obstacle walls)
        for i in 0..2 {
            self.players[i].invuln = self.players[i].invuln.saturating_sub(1);
            self.players[i].glow = self.players[i].glow.saturating_sub(1);

            if !self.players[i].alive {
                continue;
            }

            if let Some(d) = dirs[i] {
                self.players[i].facing = d;
                let next = self.players[i].pos.offset(d).clamp_to(self.field);
                if !self.wall_at(next) {
                    self.players[i].moving = true;
                    if next != self.players[i].pos {
                        self.move_tick[i] = self.ticks;
                    }
                    self.players[i].pos = next;
                } else {
                    self.players[i].moving = false;
                }
            } else {
                self.players[i].moving = false;
            }
        }
        self.prev_dir = dirs;
        self.synced = self.players[0].alive
            && self.players[1].alive
            && self.move_tick[0] > 0
            && self.move_tick[1] > 0
            && self.move_tick[0].abs_diff(self.move_tick[1]) <= 3
            && self.ticks.saturating_sub(self.move_tick[0].max(self.move_tick[1])) <= 4;

        // 2. Revive check
        if self.players[0].alive != self.players[1].alive {
            let down_idx = if self.players[0].down() { 0 } else { 1 };
            let up_idx = 1 - down_idx;
            let d = self.players[0].pos.cheb(self.players[1].pos);
            let req = self.revive_ticks_req();

            if d <= REVIVE_DIST {
                self.players[down_idx].revive += 1;
                self.players[up_idx].glow = 4;
                if self.players[down_idx].revive >= req {
                    self.players[down_idx].alive = true;
                    self.players[down_idx].hp = if self.upgrade_count(Upgrade::EmergencyRevive) > 0 {
                        2.min(self.max_hp)
                    } else {
                        1
                    };
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
        let max_snap = self.max_snap_dist();
        if both_alive && self.dist > max_snap && self.snap_cd == 0 {
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

        // 5. Charge economy (including attached Leech drain)
        if both_alive {
            let dspec = self.difficulty.spec();
            let latched_leeches = self
                .enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::Leech && e.latched)
                .count();
            if latched_leeches > 0 {
                let leech_drain = latched_leeches as f32 * 0.45 * dspec.drain_mult;
                self.charge = (self.charge - leech_drain).max(0.0);
                if self.pulse % 6 == 0 {
                    if let Some(l) = self
                        .enemies
                        .iter()
                        .find(|e| e.kind == EnemyKind::Leech && e.latched)
                    {
                        ev.push(Ev::LeechDrain { at: l.pos });
                    }
                }
            }
            if self.hot {
                let sync_mult = if self.synced { 0.75 } else { 1.0 };
                let drain = (CHARGE_BASE_DRAIN
                    + (self.dist - HOT_DIST).max(0) as f32 * CHARGE_DIST_DRAIN)
                    * dspec.drain_mult
                    * self.drain_upgrade_mult()
                    * sync_mult;
                self.charge = (self.charge - drain).max(0.0);
            } else if self.dist <= REST_DIST && latched_leeches == 0 {
                let drought = if self.event == WaveEvent::Drought { 0.65 } else { 1.0 };
                let rest =
                    CHARGE_REST_RATE * dspec.recharge_mult * self.recharge_upgrade_mult() * drought;
                self.charge = (self.charge + rest).min(CHARGE_MAX);
            }
            if self.charge <= 0.0 {
                self.hot = false;
                self.phase = Phase::Lost;
                ev.push(Ev::Lose);
                return;
            }
        }

        // 6. Conduit cutting damage & kill logic (plasma halo + optional WideHalo / TwinBeam)
        if self.hot {
            let w = self.field.w;
            let h = self.field.h;
            let mut beam_grid = vec![false; (w * h) as usize];
            let halo_r = if self.dist >= 10 && self.upgrade_count(Upgrade::WideHalo) > 0 {
                2
            } else {
                1
            };
            for p in &self.arc {
                for dy in -halo_r..=halo_r {
                    for dx in -halo_r..=halo_r {
                        let nx = p.x + dx;
                        let ny = p.y + dy;
                        if nx >= 0 && ny >= 0 && nx < w && ny < h {
                            beam_grid[(ny * w + nx) as usize] = true;
                        }
                    }
                }
            }
            let twin = self.twin_arc();
            for p in &twin {
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

            let dmg = self.beam_damage();
            let mut killed_indices = Vec::new();
            let mid_x = (self.players[0].pos.x + self.players[1].pos.x) / 2;
            let mid_y = (self.players[0].pos.y + self.players[1].pos.y) / 2;
            let long_beam = self.dist >= 16;
            for (idx, enemy) in self.enemies.iter_mut().enumerate() {
                if enemy.pos.within(self.field)
                    && beam_grid[(enemy.pos.y * w + enemy.pos.x) as usize]
                {
                    if enemy.kind == EnemyKind::Shielded && !long_beam {
                        let vx = mid_x - enemy.pos.x;
                        let vy = mid_y - enemy.pos.y;
                        let f = enemy.dir.delta();
                        if (vx * f.x + vy * f.y) > 0 {
                            if enemy.shield_cd == 0 {
                                enemy.shield_cd = 6;
                                ev.push(Ev::Block { at: enemy.pos });
                            }
                            continue;
                        }
                    }
                    enemy.hp -= dmg;
                    enemy.flash = 6;
                    if enemy.hp <= 0 {
                        killed_indices.push(idx);
                    }
                }
            }

            // Process kills cleanly before any chain lightning mutates enemy indices.
            self.harvest_kills(killed_indices, ev, true);
        }

        // 7. Update enemies
        let mut enemy_spawns = Vec::new();
        let mut new_bullets = Vec::new();
        let mut new_blasts = Vec::new();
        let mut new_hazards: Vec<(Vec2, Vec2, u32, bool)> = Vec::new();
        let mut banners: Vec<(String, u32)> = Vec::new();
        let target_poses: Vec<Vec2> = self.alive_players().map(|p| p.pos).collect();
        let center = self.field.center();
        let arc_mid = if !self.arc.is_empty() {
            self.arc[self.arc.len() / 2]
        } else {
            center
        };
        let both_alive_now = self.players[0].alive && self.players[1].alive;
        let dspec = self.difficulty.spec();
        let proj_cd = scaled_proj_cd(self.wave, self.difficulty);
        let boss_proj_cd = (proj_cd as f32 * 0.92).round().max(18.0) as u32;
        let cur_enemy_count = self.enemies.len();
        let pulse_u64 = self.pulse as u64;

        for e in self.enemies.iter_mut() {
            e.flash = e.flash.saturating_sub(1);
            e.shield_cd = e.shield_cd.saturating_sub(1);

            // SlowTime special pickup halves non-boss update rate and slows bosses by 1/3.
            if self.slow_t > 0
                && ((e.kind.is_boss() && self.pulse % 3 == 1)
                    || (!e.kind.is_boss() && self.pulse % 2 == 1))
            {
                continue;
            }

            // Boss phase shift detection & transition escalations
            if e.kind.is_boss() {
                let (cur_p, _) = e.boss_phase();
                if cur_p > e.phase_seen {
                    e.phase_seen = cur_p;
                    e.flash = 10;
                    e.telegraph = 0;
                    e.action_cd = 12;
                    ev.push(Ev::PhaseShift {
                        at: e.pos,
                        kind: e.kind,
                        phase: cur_p,
                    });
                    match (e.kind, cur_p) {
                        (EnemyKind::Warden, 2) => {
                            banners.push(("THE WARDEN \u{00b7} OVERDRIVE".into(), 65));
                            for off in [-2, 2] {
                                let sp = Vec2::new(e.pos.x + off, e.pos.y).clamp_to(self.field);
                                enemy_spawns.push(Enemy::scaled(
                                    sp,
                                    EnemyKind::Drifter,
                                    Dir::Down,
                                    pulse_u64,
                                    self.wave,
                                    self.difficulty,
                                ));
                            }
                        }
                        (EnemyKind::Hydra, 2) => {
                            banners.push(("THE HYDRA \u{00b7} TETHER CUTTER".into(), 65));
                            for off in [-2, 2] {
                                let sp = Vec2::new(e.pos.x, e.pos.y + off).clamp_to(self.field);
                                enemy_spawns.push(Enemy::scaled(
                                    sp,
                                    EnemyKind::Splitter,
                                    Dir::Down,
                                    pulse_u64,
                                    self.wave,
                                    self.difficulty,
                                ));
                            }
                        }
                        (EnemyKind::Null, 2) => {
                            banners.push(("THE NULL \u{00b7} VOID FRACTURE".into(), 65));
                            enemy_spawns.push(Enemy::scaled(
                                Vec2::new(e.pos.x - 2, e.pos.y).clamp_to(self.field),
                                EnemyKind::Leech,
                                Dir::Left,
                                pulse_u64,
                                self.wave,
                                self.difficulty,
                            ));
                            enemy_spawns.push(Enemy::scaled(
                                Vec2::new(e.pos.x + 2, e.pos.y).clamp_to(self.field),
                                EnemyKind::Shooter,
                                Dir::Right,
                                pulse_u64,
                                self.wave,
                                self.difficulty,
                            ));
                        }
                        (EnemyKind::Null, 3) => {
                            banners.push(("THE NULL \u{00b7} EVENT HORIZON".into(), 70));
                            e.move_cd = e.move_cd.saturating_sub(1).max(3);
                            enemy_spawns.push(Enemy::scaled(
                                Vec2::new(e.pos.x, e.pos.y - 2).clamp_to(self.field),
                                EnemyKind::Dasher,
                                Dir::Up,
                                pulse_u64,
                                self.wave,
                                self.difficulty,
                            ));
                        }
                        _ => {}
                    }
                }
            }

            // Handle Leech latching & continuous tether attachment every tick
            if e.kind == EnemyKind::Leech {
                if both_alive_now && !self.arc.is_empty() {
                    let on_arc = self.arc.iter().any(|&p| p.cheb(e.pos) <= 1);
                    if on_arc {
                        e.latched = true;
                    }
                    if e.latched {
                        e.pos = arc_mid;
                        continue;
                    }
                } else {
                    e.latched = false;
                }
            }

            // Handle Dasher active dash & telegraph states every tick
            if e.kind == EnemyKind::Dasher {
                if e.dash_left > 0 {
                    e.dash_left -= 1;
                    let delta = Vec2f::from_cell(e.aim) - e.dash_pos;
                    let step = delta.norm().scale(1.8);
                    e.dash_pos = (e.dash_pos + step).clamp_to(self.field);
                    e.pos = e.dash_pos.cell_in(self.field);
                    e.flash = 2;
                    if e.dash_left == 0 {
                        e.action_cd = (28.0 * dspec.cd_mult).round().max(16.0) as u32;
                    }
                    continue;
                }
                if e.telegraph > 0 {
                    e.telegraph -= 1;
                    if e.telegraph == 0 {
                        e.dash_left = 6;
                        e.dash_pos = Vec2f::from_cell(e.pos);
                        ev.push(Ev::Dash { at: e.pos });
                    }
                    continue;
                }
                if e.action_cd > 0 {
                    e.action_cd -= 1;
                } else if let Some(target) = nearest(e.pos, &target_poses) {
                    if e.pos.cheb(target) <= 15 && e.pos.cheb(target) >= 3 {
                        let raw = Vec2f::from_cell(target) - Vec2f::from_cell(e.pos);
                        let end = Vec2f::from_cell(e.pos) + raw.norm().scale(11.5);
                        e.aim = end.cell_in(self.field);
                        e.dir = step_toward(e.pos, target);
                        e.telegraph = 17;
                        continue;
                    }
                }
            }

            // Handle Shooter & multi-phase Boss attack cycles every tick
            if e.kind == EnemyKind::Shooter {
                if e.telegraph > 0 {
                    e.telegraph -= 1;
                    if e.telegraph == 0 {
                        let spd = 0.34 + (self.wave.min(12) as f32 * 0.011);
                        Self::fire_fan(&mut new_bullets, e.pos, e.aim, 1, 0.0, spd, false);
                        ev.push(Ev::Shoot { at: e.pos });
                        e.action_cd = proj_cd;
                    }
                    continue;
                }
                if e.action_cd > 0 {
                    e.action_cd -= 1;
                } else if let Some(target) = nearest(e.pos, &target_poses) {
                    if e.pos.cheb(target) <= 28 {
                        e.aim = target;
                        e.dir = step_toward(e.pos, target);
                        e.telegraph = 12;
                        continue;
                    }
                }
            } else if e.kind.is_boss() {
                if e.telegraph > 0 {
                    e.telegraph -= 1;
                    if e.telegraph == 0 {
                        let (phase, _) = e.boss_phase();
                        let volley_tick = e.born;
                        e.born = e.born.wrapping_add(1);
                        ev.push(Ev::Shoot { at: e.pos });
                        match e.kind {
                            EnemyKind::Warden => {
                                if phase == 1 {
                                    Self::fire_fan(&mut new_bullets, e.pos, e.aim, 3, 0.28, 0.42, true);
                                    if volley_tick % 2 == 1 {
                                        new_blasts.push(Blast {
                                            center: e.aim,
                                            radius: 1,
                                            fuse: 18,
                                            max_fuse: 18,
                                            dmg_players: true,
                                            dmg_enemies: 0,
                                        });
                                    }
                                } else {
                                    Self::fire_ring(&mut new_bullets, e.pos, 6, e.spin, 0.42);
                                    e.spin += 0.45;
                                }
                            }
                            EnemyKind::Hydra => {
                                if phase == 1 || volley_tick % 2 == 0 {
                                    for &tp in &target_poses {
                                        Self::fire_fan(&mut new_bullets, e.pos, tp, 2, 0.20, 0.41, true);
                                    }
                                    if volley_tick % 2 == 1 && cur_enemy_count + enemy_spawns.len() < 6 {
                                        let add = if volley_tick % 4 == 1 {
                                            EnemyKind::Leech
                                        } else {
                                            EnemyKind::Bomber
                                        };
                                        enemy_spawns.push(Enemy::scaled(
                                            e.pos,
                                            add,
                                            Dir::Down,
                                            pulse_u64,
                                            self.wave,
                                            self.difficulty,
                                        ));
                                    }
                                } else {
                                    // Phase 2 alternating Tether-Cutter horizontal sweep
                                    let p0 = Vec2::new(0, e.aim.y);
                                    let p1 = Vec2::new(self.field.w - 1, e.aim.y);
                                    new_hazards.push((p0, p1, 24, true));
                                }
                            }
                            EnemyKind::Null => match phase {
                                1 => {
                                    Self::fire_ring(&mut new_bullets, e.pos, 8, e.spin, 0.39);
                                    e.spin += 0.35;
                                }
                                2 => {
                                    Self::fire_fan(&mut new_bullets, e.pos, e.aim, 5, 0.23, 0.43, true);
                                    for &tp in &target_poses {
                                        new_blasts.push(Blast {
                                            center: tp,
                                            radius: 1,
                                            fuse: 20,
                                            max_fuse: 20,
                                            dmg_players: true,
                                            dmg_enemies: 0,
                                        });
                                    }
                                }
                                _ => {
                                    if volley_tick % 2 == 0 {
                                        Self::fire_ring(&mut new_bullets, e.pos, 10, e.spin, 0.41);
                                        e.spin += 0.38;
                                    } else {
                                        // Cross-arena Tether-Cutter lasers intersecting target
                                        new_hazards.push((
                                            Vec2::new(0, e.aim.y),
                                            Vec2::new(self.field.w - 1, e.aim.y),
                                            24,
                                            true,
                                        ));
                                        new_hazards.push((
                                            Vec2::new(e.aim.x, 0),
                                            Vec2::new(e.aim.x, self.field.h - 1),
                                            24,
                                            true,
                                        ));
                                    }
                                }
                            },
                            _ => {}
                        }
                        e.action_cd = boss_proj_cd;
                    }
                    continue;
                }
                if e.action_cd > 0 {
                    e.action_cd -= 1;
                } else if let Some(target) = nearest(e.pos, &target_poses) {
                    e.aim = target;
                    e.dir = step_toward(e.pos, target);
                    e.telegraph = 10;
                    continue;
                }
            }

            if e.cd > 0 {
                e.cd -= 1;
                continue;
            }
            e.cd = e.move_cd;

            match e.kind {
                EnemyKind::Drifter => {
                    let next = e.pos.offset(e.dir);
                    if !next.within(self.field) || self.walls.iter().any(|w| w.contains(next)) {
                        e.dir = step_toward(e.pos, center);
                        let sidestep = move_around_walls(&self.walls, self.field, e.pos, e.dir);
                        e.pos = sidestep;
                    } else {
                        e.pos = next;
                        if self.pulse.wrapping_add(e.noise()) % 10 == 0 {
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
                EnemyKind::Shooter => {
                    if let Some(target) = nearest(e.pos, &target_poses) {
                        let d = e.pos.cheb(target);
                        if d < 8 {
                            let away = Vec2::new(
                                e.pos.x + (e.pos.x - target.x).signum(),
                                e.pos.y + (e.pos.y - target.y).signum(),
                            )
                            .clamp_to(self.field);
                            if away != e.pos && !self.walls.iter().any(|w| w.contains(away)) {
                                e.pos = away;
                            } else {
                                e.pos = move_around_walls(
                                    &self.walls,
                                    self.field,
                                    e.pos,
                                    step_toward(e.pos, center),
                                );
                            }
                            e.dir = step_toward(e.pos, target);
                        } else if d > 17 {
                            e.dir = step_toward(e.pos, target);
                            e.pos = move_around_walls(&self.walls, self.field, e.pos, e.dir);
                        } else {
                            let dx = target.x - e.pos.x;
                            let dy = target.y - e.pos.y;
                            let cw = if (e.born % 2) == 0 { 1 } else { -1 };
                            let strafe = if dx.abs() >= dy.abs() {
                                if cw > 0 { Dir::Down } else { Dir::Up }
                            } else if cw > 0 {
                                Dir::Right
                            } else {
                                Dir::Left
                            };
                            e.pos = move_around_walls(&self.walls, self.field, e.pos, strafe);
                            e.dir = step_toward(e.pos, target);
                        }
                    }
                }
                EnemyKind::Leech => {
                    e.dir = step_toward(e.pos, arc_mid);
                    e.pos = e.pos.offset(e.dir).clamp_to(self.field);
                }
                EnemyKind::Hunter
                | EnemyKind::Dasher
                | EnemyKind::Splitter
                | EnemyKind::Shielded
                | EnemyKind::Bomber
                | EnemyKind::Bulwark => {
                    if let Some(target) = nearest(e.pos, &target_poses) {
                        e.dir = step_toward(e.pos, target);
                        e.pos = move_around_walls(&self.walls, self.field, e.pos, e.dir);
                    }
                }
                EnemyKind::Warden | EnemyKind::Hydra => {
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
                        self.null_cd = 7;
                        if cur_enemy_count + enemy_spawns.len() < 7 {
                            let hp = e.pos;
                            enemy_spawns.push(Enemy::scaled(
                                hp,
                                EnemyKind::Hunter,
                                Dir::Down,
                                pulse_u64,
                                self.wave,
                                self.difficulty,
                            ));
                        }
                    } else {
                        self.null_cd -= 1;
                    }
                }
            }
        }
        for s in enemy_spawns {
            self.enemies.push(s);
        }
        for b in new_bullets {
            self.bullets.push(b);
        }
        for bl in new_blasts {
            self.blasts.push(bl);
        }
        for (p0, p1, fuse, cuts) in new_hazards {
            let bh = self.make_beam_hazard(p0, p1, fuse, cuts);
            self.beam_hazards.push(bh);
        }
        for (msg, ttl) in banners {
            self.banner(msg, ttl);
        }

        // 7b. Bullets & telegraphed lasers/blasts
        let mut hi = 0;
        while hi < self.beam_hazards.len() {
            if self.beam_hazards[hi].fuse > 1 {
                self.beam_hazards[hi].fuse -= 1;
                hi += 1;
                continue;
            }
            let bh = self.beam_hazards.remove(hi);
            self.shake = self.shake.max(4);
            let mid_pt = Vec2::new((bh.p0.x + bh.p1.x) / 2, (bh.p0.y + bh.p1.y) / 2);
            ev.push(Ev::Explode { at: mid_pt, radius: 1 });
            for p_idx in 0..2 {
                if self.players[p_idx].alive && bh.pts.contains(&self.players[p_idx].pos) {
                    self.damage_player(p_idx, 40, ev);
                }
            }
            if bh.cuts_tether && (self.hot || self.arc.len() >= 2) {
                if let Some(&cut_pt) = self.arc.iter().find(|pt| bh.pts.contains(pt)) {
                    self.charge = (self.charge - 10.0).max(0.0);
                    self.hot = false;
                    ev.push(Ev::TetherCut { at: cut_pt });
                    self.banner("TETHER CUT! -10%", 35);
                    if self.charge <= 0.0 {
                        self.phase = Phase::Lost;
                        ev.push(Ev::Lose);
                        return;
                    }
                }
            }
        }
        let mut bi = 0;
        while bi < self.bullets.len() {
            let bvel = if self.slow_t > 0 {
                self.bullets[bi].vel.scale(0.5)
            } else {
                self.bullets[bi].vel
            };
            self.bullets[bi].pos = self.bullets[bi].pos + bvel;
            self.bullets[bi].ttl = self.bullets[bi].ttl.saturating_sub(1);
            let b = self.bullets[bi];
            let out = b.pos.x < 0.0
                || b.pos.y < 0.0
                || b.pos.x >= self.field.w as f32
                || b.pos.y >= self.field.h as f32
                || b.ttl == 0
                || self.wall_at(b.pos.cell());
            if out {
                self.bullets.swap_remove(bi);
                continue;
            }
            let hit_p = (0..2).find(|&i| {
                self.players[i].alive
                    && self.players[i].invuln == 0
                    && Vec2f::from_cell(self.players[i].pos).dist(b.pos) <= 0.72
            });
            if let Some(p_idx) = hit_p {
                self.damage_player(p_idx, 38, ev);
                self.bullets.swap_remove(bi);
                continue;
            }
            bi += 1;
        }

        // 7c. Telegraphed hazard floor zones
        let mut hzi = 0;
        while hzi < self.hazards.len() {
            if self.hazards[hzi].warn > 0 {
                self.hazards[hzi].warn -= 1;
                hzi += 1;
                continue;
            }
            if self.hazards[hzi].active == 0 {
                self.hazards.swap_remove(hzi);
                continue;
            }
            self.hazards[hzi].active -= 1;
            let hz = self.hazards[hzi];
            for p_idx in 0..2 {
                if self.players[p_idx].alive && hz.contains(self.players[p_idx].pos) {
                    self.damage_player(p_idx, 40, ev);
                }
            }
            if self.pulse % 18 == 0 {
                let mut dead = Vec::new();
                for (ei, e) in self.enemies.iter_mut().enumerate() {
                    if hz.contains(e.pos) {
                        e.hp -= 1;
                        e.flash = 4;
                        if e.hp <= 0 {
                            dead.push(ei);
                        }
                    }
                }
                self.harvest_kills(dead, ev, true);
            }
            hzi += 1;
        }

        // 8. Player-Enemy collision (Bombers detonate on contact; latched Leeches do not body-hit)
        let mut detonated_bombers = Vec::new();
        for p_idx in 0..2 {
            if !self.players[p_idx].alive || self.players[p_idx].invuln > 0 {
                continue;
            }
            let p_pos = self.players[p_idx].pos;
            let hit = self.enemies.iter().enumerate().find(|(_, e)| {
                e.pos == p_pos && !(e.kind == EnemyKind::Leech && e.latched && self.dist > 0)
            });
            if let Some((e_idx, enemy)) = hit {
                let is_bomber = enemy.kind == EnemyKind::Bomber;
                let e_pos = enemy.pos;
                self.damage_player(p_idx, 45, ev);
                if is_bomber {
                    detonated_bombers.push(e_idx);
                    self.blasts.push(Blast {
                        center: e_pos,
                        radius: 2,
                        fuse: 1,
                        max_fuse: 1,
                        dmg_players: false,
                        dmg_enemies: 2,
                    });
                }
            }
        }
        if !detonated_bombers.is_empty() {
            detonated_bombers.sort_unstable();
            detonated_bombers.dedup();
            for &idx in detonated_bombers.iter().rev() {
                if idx < self.enemies.len() {
                    self.enemies.remove(idx);
                }
            }
        }

        // Process telegraphed area blasts (bomber deaths & contact splashes)
        let mut bl_i = 0;
        while bl_i < self.blasts.len() {
            if self.blasts[bl_i].fuse > 1 {
                self.blasts[bl_i].fuse -= 1;
                bl_i += 1;
                continue;
            }
            let blast = self.blasts.remove(bl_i);
            ev.push(Ev::Explode {
                at: blast.center,
                radius: blast.radius,
            });
            self.shake = self.shake.max(5);
            if blast.dmg_players {
                for p_idx in 0..2 {
                    if self.players[p_idx].alive
                        && self.players[p_idx].pos.cheb(blast.center) <= blast.radius
                    {
                        self.damage_player(p_idx, 45, ev);
                    }
                }
            }
            if blast.dmg_enemies > 0 {
                let mut dead = Vec::new();
                for (ei, e) in self.enemies.iter_mut().enumerate() {
                    if e.pos.cheb(blast.center) <= blast.radius {
                        e.hp -= blast.dmg_enemies;
                        e.flash = 6;
                        if e.hp <= 0 {
                            dead.push(ei);
                        }
                    }
                }
                self.harvest_kills(dead, ev, true);
            }
        }

        if !self.players[0].alive && !self.players[1].alive {
            self.phase = Phase::Lost;
            ev.push(Ev::Lose);
            return;
        }

        // 9. Orbs (with optional OrbMagnet pull + rare special pickups)
        let magnet = self.upgrade_count(Upgrade::OrbMagnet) > 0;
        let player_poses: Vec<Vec2> = self.alive_players().map(|p| p.pos).collect();
        let mut nova_triggered = false;
        let mut orb_banner: Option<&'static str> = None;
        let mut orb_consumed = Vec::new();
        for (idx, orb) in self.orbs.iter_mut().enumerate() {
            if orb.ttl > 0 {
                orb.ttl -= 1;
            }
            if magnet && self.pulse % 3 == 0 {
                if let Some(tp) = nearest(orb.pos, &player_poses) {
                    if orb.pos != tp && orb.pos.cheb(tp) <= 6 {
                        orb.pos = move_around_walls(
                            &self.walls,
                            self.field,
                            orb.pos,
                            step_toward(orb.pos, tp),
                        );
                    }
                }
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
                        let other = 1 - p_idx;
                        if self.players[p_idx].hp < self.max_hp {
                            self.players[p_idx].hp += 1;
                        } else if self.players[other].alive && self.players[other].hp < self.max_hp {
                            self.players[other].hp += 1;
                        }
                    }
                    OrbKind::SlowTime => {
                        self.slow_t = 120;
                        orb_banner = Some("TIME DILATION");
                    }
                    OrbKind::Nova => {
                        nova_triggered = true;
                        orb_banner = Some("NOVA PULSE");
                    }
                    OrbKind::TwinBeam => {
                        self.twin_t = 150;
                        orb_banner = Some("TWIN CONDUIT");
                    }
                }
                ev.push(Ev::Pickup {
                    at: orb.pos,
                    kind: orb.kind,
                });
            }
        }
        if let Some(msg) = orb_banner {
            self.banner(msg, 45);
        }
        for &idx in orb_consumed.iter().rev() {
            self.orbs.remove(idx);
        }
        self.orbs.retain(|o| o.ttl > 0);

        if nova_triggered {
            self.bullets.clear();
            self.blasts.clear();
            self.shake = self.shake.max(6);
            let center = self.field.center();
            ev.push(Ev::Explode { at: center, radius: 3 });
            let mut dead = Vec::new();
            for (ei, e) in self.enemies.iter_mut().enumerate() {
                e.hp -= 2;
                e.flash = 8;
                if e.hp <= 0 {
                    dead.push(ei);
                }
            }
            self.harvest_kills(dead, ev, false);
        }

        // 10. Wave management
        self.update_waves(ev);
    }

    fn harvest_kills(&mut self, mut indices: Vec<usize>, ev: &mut Vec<Ev>, allow_chain: bool) {
        if indices.is_empty() {
            return;
        }
        indices.sort_unstable();
        indices.dedup();
        let mut removed = Vec::with_capacity(indices.len());
        for &idx in indices.iter().rev() {
            removed.push(self.enemies.remove(idx));
        }
        let mut spawns = Vec::new();
        for enemy in &removed {
            self.on_kill(enemy, ev, &mut spawns, allow_chain);
        }
        for s in spawns {
            self.enemies.push(s);
        }
    }

    fn on_kill(&mut self, enemy: &Enemy, ev: &mut Vec<Ev>, spawns: &mut Vec<Enemy>, allow_chain: bool) {
        self.kills += 1;
        self.combo += 1;
        self.combo_t = 60;
        self.best_combo = self.best_combo.max(self.combo);
        self.score +=
            ((enemy.kind.points() * self.score_mult()) as f32 * self.difficulty.spec().score_mult)
                .round() as u64;
        self.charge = (self.charge + enemy.kind.charge()).min(CHARGE_MAX);

        ev.push(Ev::Kill {
            at: enemy.pos,
            kind: enemy.kind,
            combo: self.combo,
        });

        if enemy.kind == EnemyKind::Splitter {
            let mut d1 = Enemy::scaled(enemy.pos, EnemyKind::Drifter, Dir::Up, self.pulse as u64, self.wave, self.difficulty);
            let mut d2 = Enemy::scaled(enemy.pos, EnemyKind::Drifter, Dir::Down, self.pulse as u64, self.wave, self.difficulty);
            d1.born = self.pulse as u64;
            d2.born = self.pulse as u64;
            spawns.push(d1);
            spawns.push(d2);
        }

        if enemy.kind == EnemyKind::Bomber {
            self.blasts.push(Blast {
                center: enemy.pos,
                radius: 2,
                fuse: 10,
                max_fuse: 10,
                dmg_players: true,
                dmg_enemies: 2,
            });
        }

        if allow_chain && self.upgrade_count(Upgrade::ArcChain) > 0 {
            let mut zapped = Vec::new();
            for (ei, e) in self.enemies.iter_mut().enumerate() {
                if zapped.len() >= 2 {
                    break;
                }
                if e.pos.cheb(enemy.pos) <= 4 {
                    e.hp -= 1;
                    e.flash = 6;
                    ev.push(Ev::ChainArc {
                        from: enemy.pos,
                        to: e.pos,
                    });
                    if e.hp <= 0 {
                        zapped.push(ei);
                    }
                }
            }
            for &ei in zapped.iter().rev() {
                let chained = self.enemies.remove(ei);
                self.on_kill(&chained, ev, spawns, false);
            }
        }

        if enemy.kind == EnemyKind::Warden {
            self.banner("WARDEN SHATTERED", 75);
            self.spawn_orbs(2);
        } else if enemy.kind == EnemyKind::Hydra {
            self.banner("HYDRA SEVERED", 80);
            self.spawn_orbs(2);
        } else if enemy.kind == EnemyKind::Null {
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

        // Spawn queued formations (paired pincers / trio squads on later waves)
        if self.spawn_cd > 0 {
            self.spawn_cd -= 1;
        } else if !self.queued.is_empty() {
            let top_is_boss = self.queued.last().is_some_and(|k| k.is_boss());
            let batch_n = if top_is_boss {
                1
            } else if self.wave >= 6 && self.queued.len() >= 3 {
                3
            } else if self.queued.len() >= 2 && (self.wave >= 2 || self.queued.len() >= 4) {
                2
            } else {
                1
            };
            let anchor_edge = self.rng.below(4);
            for slot in 0..batch_n {
                if let Some(kind) = self.queued.pop() {
                    let pos = if kind.is_boss() {
                        self.spawn_pos()
                    } else {
                        self.formation_pos(slot, batch_n, anchor_edge)
                    };
                    let enemy = self.make_enemy_at(kind, pos);
                    ev.push(Ev::Spawn { at: enemy.pos });
                    self.enemies.push(enemy);
                }
            }
            let unit_cd = scaled_spawn_cd(self.wave, self.difficulty);
            self.spawn_cd = (unit_cd * batch_n as u32)
                .saturating_sub(if batch_n > 1 { 2 } else { 0 })
                .max(4);
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

                self.bullets.clear();
                self.blasts.clear();
                self.beam_hazards.clear();
                self.hazards.clear();
                self.spawn_orbs(1);
                self.upgrade_choices = self.roll_upgrade_choices();
                self.draft_pending = true;
                self.wave += 1;
                self.queued = scaled_wave_queue(self.wave, self.difficulty);
                self.walls = spawn_wave_walls(self.field, self.wave);
                self.event = WaveEvent::roll(self.wave, &mut self.rng);
                if self.event == WaveEvent::EnemySurge {
                    for _ in 0..2 {
                        self.queued.insert(0, EnemyKind::Hunter);
                    }
                    self.queued.insert(0, EnemyKind::Drifter);
                }
                self.intermission = 36;
                self.spawn_cd = 6;
                self.refresh_wave_barriers();
                self.update_conduit();

                let title = match self.wave {
                    4 => "WAVE 04 \u{00b7} THE WARDEN".to_string(),
                    8 => "WAVE 08 \u{00b7} THE HYDRA".to_string(),
                    TOTAL_WAVES => "WAVE 12 \u{00b7} THE NULL ENGAGED".to_string(),
                    _ if self.event != WaveEvent::None => {
                        format!("WAVE {:02} \u{00b7} {}", self.wave, self.event.label())
                    }
                    _ => format!("WAVE {:02} \u{00b7} CONTACT", self.wave),
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

/// Move one cell toward `want`, sidestepping orthogonally around patrolling walls.
pub fn move_around_walls(walls: &[Wall], field: Field, cur: Vec2, want: Dir) -> Vec2 {
    let primary = cur.offset(want).clamp_to(field);
    if !walls.iter().any(|w| w.contains(primary)) {
        return primary;
    }
    let alts = match want {
        Dir::Left | Dir::Right => [Dir::Up, Dir::Down],
        Dir::Up | Dir::Down => [Dir::Left, Dir::Right],
    };
    let center = field.center();
    let best = alts
        .into_iter()
        .map(|d| cur.offset(d).clamp_to(field))
        .filter(|&p| p != cur && !walls.iter().any(|w| w.contains(p)))
        .min_by_key(|p| p.cheb(center));
    best.unwrap_or(cur)
}

/// Construct patrolling obstacle barriers for waves 3+.
pub fn spawn_wave_walls(field: Field, wave: u32) -> Vec<Wall> {
    if wave < 3 || field.w < 30 || field.h < 14 {
        return Vec::new();
    }
    let len = 4;
    let min_y = 2;
    let max_y = (field.h - 3).max(min_y + len);
    if wave < 6 {
        vec![Wall {
            pos: Vec2::new(field.w / 2, (field.h / 2 - len / 2).clamp(min_y, max_y - len)),
            len,
            vertical: true,
            vel: 1,
            min_c: min_y,
            max_c: max_y,
        }]
    } else {
        vec![
            Wall {
                pos: Vec2::new(field.w / 3, min_y + 1),
                len,
                vertical: true,
                vel: 1,
                min_c: min_y,
                max_c: max_y,
            },
            Wall {
                pos: Vec2::new((field.w * 2) / 3, (max_y - len - 1).max(min_y)),
                len,
                vertical: true,
                vel: -1,
                min_c: min_y,
                max_c: max_y,
            },
        ]
    }
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

pub fn scaled_spawn_cd(wave: u32, diff: Difficulty) -> u32 {
    let base = (13.0 - (wave as f32 * 0.55)).clamp(5.0, 13.0);
    (base * diff.spec().spawn_mult).round().clamp(3.0, 18.0) as u32
}

/// Shooter / ranged attack cooldown in ticks, scaled by wave and difficulty.
pub fn scaled_proj_cd(wave: u32, diff: Difficulty) -> u32 {
    let base = (44.0 - (wave as f32 * 1.6)).clamp(24.0, 44.0);
    (base * diff.spec().proj_cd_mult).round().clamp(16.0, 64.0) as u32
}

pub fn scaled_wave_queue(wave: u32, diff: Difficulty) -> Vec<EnemyKind> {
    let mut q = wave_queue(wave);
    let extra_mult = diff.spec().count_mult;
    if extra_mult > 1.01 {
        let extra = ((q.len() as f32) * (extra_mult - 1.0)).round() as usize;
        for i in 0..extra {
            let add = match (wave + i as u32) % 6 {
                0 if wave >= 5 => EnemyKind::Shielded,
                1 if wave >= 2 => EnemyKind::Shooter,
                2 if wave >= 3 => EnemyKind::Dasher,
                3 if wave >= 6 => EnemyKind::Bomber,
                4 if wave >= 5 => EnemyKind::Leech,
                _ => EnemyKind::Hunter,
            };
            // Insert near front so boss (at the end of vec) still pops first.
            q.insert(0, add);
        }
    } else if extra_mult < 0.99 && q.len() > 5 {
        let trim = ((q.len() as f32) * (1.0 - extra_mult)).floor() as usize;
        for _ in 0..trim.min(q.len().saturating_sub(4)) {
            if !q[0].is_boss() {
                q.remove(0);
            }
        }
    }
    q
}

pub fn wave_queue(wave: u32) -> Vec<EnemyKind> {
    use self::EnemyKind::*;
    match wave {
        1 => vec![Drifter, Drifter, Drifter, Drifter, Drifter, Drifter],
        2 => vec![Drifter, Hunter, Drifter, Shooter, Drifter, Hunter, Drifter],
        3 => vec![Drifter, Dasher, Drifter, Shooter, Splitter, Hunter, Drifter, Shooter],
        4 => vec![Shooter, Drifter, Dasher, Hunter, Shooter, Warden],
        5 => vec![Leech, Shielded, Shooter, Hunter, Shielded, Drifter, Splitter, Shooter],
        6 => vec![Bomber, Dasher, Shooter, Shielded, Bomber, Bulwark, Hunter, Shooter],
        7 => vec![Leech, Bomber, Shielded, Shooter, Splitter, Leech, Bomber, Dasher, Shooter],
        8 => vec![Bomber, Shooter, Shielded, Dasher, Leech, Hydra],
        9 => vec![Shielded, Bomber, Dasher, Shooter, Leech, Bulwark, Dasher, Bomber, Shooter, Shielded],
        10 => vec![Bulwark, Shielded, Leech, Bomber, Shooter, Dasher, Bulwark, Shielded, Shooter, Bomber, Leech],
        11 => vec![Shielded, Leech, Bomber, Dasher, Shooter, Splitter, Bulwark, Shielded, Bomber, Dasher, Shooter, Leech],
        // Note: `Vec::pop()` pops from the end, so Null spawns first!
        12 => vec![Shooter, Bomber, Leech, Dasher, Shielded, Bulwark, Hunter, Null],
        _ => vec![Hunter, Shooter, Bulwark, Null],
    }
}

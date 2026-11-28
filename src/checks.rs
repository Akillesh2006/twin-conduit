//! Shared self-verification suite executed by `--test` (native) and `verify_all` (WASM).

use crate::core::{Dir, Field, Side, Vec2};
use crate::game::{
    wave_queue, Difficulty, Enemy, EnemyKind, Phase, World, HOT_DIST, MAX_HP, REST_DIST,
    REVIVE_TICKS, SNAP_DIST, TOTAL_WAVES,
};

pub struct CheckReport {
    pub passed: usize,
    pub total: usize,
    pub log: String,
}

pub fn run_all() -> CheckReport {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(4096);
    let mut passed = 0usize;
    let mut total = 0usize;

    macro_rules! line {
        ($($arg:tt)*) => {{
            let _ = writeln!(out, $($arg)*);
        }};
    }
    macro_rules! check {
        ($desc:expr, $cond:expr) => {{
            total += 1;
            if $cond {
                line!("  [PASS] {}", $desc);
                passed += 1;
            } else {
                line!("  [FAIL] {}", $desc);
            }
        }};
    }

    line!("============================================================");
    line!("   CONDUIT - Cooperative Energy Tether Verification Tests   ");
    line!("============================================================\n");

    // 1. Two-player and tether setup
    line!("--- Testing Requirement 1: Two Operators Joined by Energy Beam ---");
    {
        let mut w = World::new(Field::new(64, 24));
        check!("Two players initialized in game world", w.players.len() == 2);
        check!("Player 1 is Side::One (WASD controls)", w.players[0].side == Side::One);
        check!("Player 2 is Side::Two (Arrow key controls)", w.players[1].side == Side::Two);
        check!(
            "Normal difficulty starts players with MAX_HP (3)",
            w.players[0].hp == MAX_HP && w.players[1].hp == MAX_HP
        );
        check!("Both players start alive", w.players[0].alive && w.players[1].alive);
        check!("Conduit line (arc) is calculated between players", !w.arc.is_empty());
        check!(
            "Conduit thresholds configured cleanly",
            REST_DIST < HOT_DIST && HOT_DIST < SNAP_DIST
        );
        check!("Distance between players is calculated", w.dist > 0);

        let p1_start = w.players[0].pos;
        let p2_start = w.players[1].pos;
        let mut ev = Vec::new();
        w.step([Some(Dir::Up), Some(Dir::Right)], &mut ev);
        check!(
            "Simultaneous movement updates both player positions independently",
            w.players[0].pos.y < p1_start.y && w.players[1].pos.x > p2_start.x
        );
    }

    // 2. Beam hot state & burning enemies
    line!("\n--- Testing Requirement 2: Beam Burns When Stretched Apart (dist >= HOT_DIST) ---");
    {
        let mut w = World::new(Field::new(64, 24));
        w.intermission = 100;
        w.queued.clear();
        w.enemies.clear();

        w.players[0].pos = Vec2::new(10, 12);
        w.players[1].pos = Vec2::new(22, 12);

        let mut ev = Vec::new();
        w.step([None, None], &mut ev);
        check!("Conduit is hot when dist >= HOT_DIST", w.hot && w.dist >= HOT_DIST);

        let beam_cell = w.arc[w.arc.len() / 2];
        w.enemies.push(Enemy::new(beam_cell, EnemyKind::Drifter, Dir::Down, 0));

        w.step([None, None], &mut ev);
        check!(
            "Live conduit burns and destroys enemy touching the beam line",
            w.kills > 0 && w.enemies.is_empty()
        );
    }

    // 3. Huddling recharges shared charge
    line!("\n--- Testing Requirement 3: Huddling (dist <= REST_DIST) Recharges Shared Cell ---");
    {
        let mut w = World::new(Field::new(64, 24));
        w.intermission = 100;
        w.queued.clear();
        w.enemies.clear();

        w.charge = 50.0;
        w.players[0].pos = Vec2::new(20, 12);
        w.players[1].pos = Vec2::new(22, 12);

        let mut ev = Vec::new();
        w.step([None, None], &mut ev);
        check!(
            "Conduit is cold when huddling (dist <= REST_DIST)",
            !w.hot && w.dist <= REST_DIST
        );
        check!("Shared charge recharges while huddling", w.charge > 50.0);
    }

    // 4. Zero charge ends the run
    line!("\n--- Testing Requirement 4: Zero Charge Ends Run (Both Lose) ---");
    {
        let mut w = World::new(Field::new(64, 24));
        w.intermission = 100;
        w.charge = 0.05;
        w.players[0].pos = Vec2::new(10, 12);
        w.players[1].pos = Vec2::new(25, 12);

        let mut ev = Vec::new();
        w.step([None, None], &mut ev);
        check!(
            "Zero charge terminates run with Phase::Lost",
            w.charge <= 0.0 && w.phase == Phase::Lost
        );
    }

    // 5. Downed and Reviving
    line!("\n--- Testing Requirement 5: Downed Players & Cooperative Reviving ---");
    {
        let mut w = World::new(Field::new(64, 24));
        w.intermission = 100;
        w.queued.clear();
        w.enemies.clear();

        w.players[0].hp = 0;
        w.players[0].alive = false;
        w.players[0].pos = Vec2::new(20, 12);
        w.players[1].pos = Vec2::new(30, 12);

        let mut ev = Vec::new();
        w.step([None, None], &mut ev);
        check!("Conduit cannot burn while a player is down", !w.hot);

        w.players[1].pos = Vec2::new(21, 12);
        for _ in 0..REVIVE_TICKS + 2 {
            w.step([None, None], &mut ev);
        }
        check!(
            "Standing within 2 cells for 45 ticks revives the downed partner",
            w.players[0].alive && w.players[0].hp > 0
        );
    }

    // 6. 12 Waves and Boss (THE NULL)
    line!("\n--- Testing Requirement 6: 12 Waves and Boss (THE NULL) ---");
    {
        check!("Total waves is 12", TOTAL_WAVES == 12);
        let q12 = wave_queue(12);
        check!(
            "Wave 12 roster contains Boss: THE NULL",
            q12.iter().any(|k| *k == EnemyKind::Null && k.is_boss())
        );

        let mut w = World::new(Field::new(64, 24));
        w.wave = 12;
        w.queued.clear();
        w.enemies.clear();
        w.intermission = 0;

        let mut ev = Vec::new();
        w.step([None, None], &mut ev);
        check!("Clearing wave 12 achieves victory (Phase::Won)", w.phase == Phase::Won);
    }

    // 7. Difficulty modes, tighter charge balance, and wave scaling
    line!("\n--- Testing Requirement 7: Difficulty Modes & Wave Scaling ---");
    {
        let mut easy = World::new_with(Field::new(64, 24), Difficulty::Easy);
        let mut norm = World::new_with(Field::new(64, 24), Difficulty::Normal);
        let mut night = World::new_with(Field::new(64, 24), Difficulty::Nightmare);

        check!(
            "Difficulty affects starting HP (Easy=4, Normal=3, Nightmare=2)",
            easy.players[0].hp == 4 && norm.players[0].hp == 3 && night.players[0].hp == 2
        );

        easy.intermission = 100;
        norm.intermission = 100;
        night.intermission = 100;
        for w in [&mut easy, &mut norm, &mut night] {
            w.queued.clear();
            w.enemies.clear();
            w.players[0].pos = Vec2::new(10, 12);
            w.players[1].pos = Vec2::new(26, 12);
        }
        let mut ev = Vec::new();
        easy.step([None, None], &mut ev);
        norm.step([None, None], &mut ev);
        night.step([None, None], &mut ev);
        check!(
            "Stretch charge drain scales tightly by difficulty (Easy < Normal < Nightmare)",
            easy.charge > norm.charge && norm.charge > night.charge
        );

        let bulwark_w1 = EnemyKind::Bulwark.scaled_hp(1, Difficulty::Normal);
        let bulwark_w11 = EnemyKind::Bulwark.scaled_hp(11, Difficulty::Normal);
        let cd_w1 = EnemyKind::Bulwark.scaled_cd(1, Difficulty::Normal);
        let cd_w11 = EnemyKind::Bulwark.scaled_cd(11, Difficulty::Hard);
        check!(
            "Enemy HP and speed scale cleanly with wave and difficulty",
            bulwark_w11 > bulwark_w1 && cd_w11 < cd_w1
        );
    }

    // 8. Five New Enemy Types (Shooter, Dasher, Shielded, Leech, Bomber)
    line!("\n--- Testing Requirement 8: New Enemy Types & Attacks ---");
    {
        // Shooter fires a dodgeable bullet
        let mut w = World::new(Field::new(64, 24));
        w.queued.clear();
        w.enemies.clear();
        w.intermission = 100;
        w.players[0].pos = Vec2::new(16, 12);
        w.players[1].pos = Vec2::new(18, 12);
        let mut s = Enemy::new(Vec2::new(32, 12), EnemyKind::Shooter, crate::core::Dir::Left, 0);
        s.action_cd = 1;
        w.enemies.push(s);
        let mut ev = Vec::new();
        for _ in 0..16 {
            w.step([None, None], &mut ev);
        }
        check!("Shooter telegraphs and fires projectile", !w.bullets.is_empty());

        // Dasher telegraphs line and dashes rapidly
        let mut w = World::new(Field::new(64, 24));
        w.queued.clear();
        w.enemies.clear();
        w.intermission = 100;
        w.players[0].pos = Vec2::new(10, 12);
        w.players[1].pos = Vec2::new(12, 12);
        let mut d = Enemy::new(Vec2::new(22, 12), EnemyKind::Dasher, crate::core::Dir::Left, 0);
        d.action_cd = 0;
        let start_x = d.pos.x;
        w.enemies.push(d);
        for _ in 0..22 {
            w.step([None, None], &mut ev);
        }
        let moved = start_x - w.enemies[0].pos.x;
        check!("Dasher telegraphs then surges rapid distance", moved >= 7);

        // Shielded blocks close frontal hits, vulnerable to flank or long-range beam (>=16)
        let mut w = World::new(Field::new(64, 24));
        w.queued.clear();
        w.enemies.clear();
        w.intermission = 100;
        w.players[0].pos = Vec2::new(20, 10);
        w.players[1].pos = Vec2::new(20, 14);
        w.enemies.push(Enemy::new(Vec2::new(24, 12), EnemyKind::Shielded, crate::core::Dir::Left, 0));
        w.step([None, None], &mut ev); // Beam at x=20 misses (x=24) — put enemy at x=20 with front facing left midpoint?
        // Actually test frontal block when midpoint is Left of Shielded facing Left:
        w.enemies[0].pos = Vec2::new(20, 12);
        w.players[0].pos = Vec2::new(15, 10);
        w.players[1].pos = Vec2::new(25, 14); // dist = 10 (<16), mid=(20,12) -> put enemy at (21,12) facing Left -> mid at (20,12) is LEFT!
        w.enemies[0].pos = Vec2::new(21, 12);
        w.enemies[0].hp = 3;
        w.step([None, None], &mut ev);
        let blocked_hp = w.enemies[0].hp;
        // Now stretch long range (dist >= 16)
        w.players[0].pos = Vec2::new(10, 12);
        w.players[1].pos = Vec2::new(30, 12);
        w.step([None, None], &mut ev);
        let damaged_hp = w.enemies[0].hp;
        check!(
            "Shielded blocks close front arc yet takes long-range damage",
            blocked_hp == 3 && damaged_hp < 3
        );

        // Leech attaches to arc and drains charge
        let mut w = World::new(Field::new(64, 24));
        w.queued.clear();
        w.enemies.clear();
        w.intermission = 100;
        w.players[0].pos = Vec2::new(18, 12);
        w.players[1].pos = Vec2::new(21, 12); // dist = 3 (cold, recharging normally)
        w.charge = 50.0;
        w.enemies.push(Enemy::new(Vec2::new(20, 12), EnemyKind::Leech, crate::core::Dir::Left, 0));
        for _ in 0..5 {
            w.step([None, None], &mut ev);
        }
        check!(
            "Leech latches onto tether and drains CHARGE while cold",
            w.enemies[0].latched && w.charge < 50.0
        );

        // Bomber explodes on death in area blast
        let mut w = World::new(Field::new(64, 24));
        w.queued.clear();
        w.enemies.clear();
        w.intermission = 100;
        w.players[0].pos = Vec2::new(15, 12);
        w.players[1].pos = Vec2::new(25, 12);
        let mut b = Enemy::new(Vec2::new(20, 12), EnemyKind::Bomber, crate::core::Dir::Left, 0);
        b.hp = 1;
        w.enemies.push(b);
        w.step([None, None], &mut ev);
        check!("Bomber spawns area blast fuse upon death", !w.blasts.is_empty());
    }

    // 9. Multi-Phase Bosses (Waves 4, 8, 12) & Tether Cutter
    line!("\n--- Testing Requirement 9: Multi-Phase Bosses & Tether Cutter ---");
    {
        let mut ev = Vec::new();
        let q4 = wave_queue(4);
        let q8 = wave_queue(8);
        let q12 = wave_queue(12);
        check!(
            "Bosses appear on waves 4 (Warden), 8 (Hydra), and 12 (THE NULL)",
            q4.contains(&EnemyKind::Warden)
                && q8.contains(&EnemyKind::Hydra)
                && q12.contains(&EnemyKind::Null)
        );

        let mut w = World::new(Field::new(64, 24));
        w.wave = 12;
        w.queued.clear();
        w.enemies.clear();
        w.intermission = 100;
        w.players[0].pos = Vec2::new(10, 10);
        w.players[1].pos = Vec2::new(12, 10); // cold, out of arc
        let mut null = Enemy::scaled(Vec2::new(40, 12), EnemyKind::Null, crate::core::Dir::Left, 0, 12, Difficulty::Normal);
        let max_hp = null.max_hp;
        null.hp = (max_hp / 2).max(1);
        w.enemies.push(null);
        w.step([None, None], &mut ev);
        let phase2 = w.enemies[0].boss_phase().0;
        w.enemies[0].hp = (max_hp / 5).max(1);
        w.step([None, None], &mut ev);
        let phase3 = w.enemies[0].boss_phase().0;
        check!(
            "THE NULL transitions cleanly through 3 distinct phases",
            phase2 == 2 && phase3 == 3 && w.enemies[0].phase_seen == 3
        );

        // BeamHazard tether-cutter severs overlapping hot conduit charge
        w.charge = 60.0;
        w.players[0].pos = Vec2::new(20, 6);
        w.players[1].pos = Vec2::new(20, 18);
        w.beam_hazards.push(crate::game::BeamHazard {
            p0: Vec2::new(0, 12),
            p1: Vec2::new(63, 12),
            pts: crate::core::line(Vec2::new(0, 12), Vec2::new(63, 12)),
            fuse: 1,
            max_fuse: 1,
            cuts_tether: true,
        });
        w.step([None, None], &mut ev);
        check!("Tether-cutter hazard beam reduces charge when crossing arc", w.charge < 55.0);
    }

    // 10. Shared Between-Wave Upgrades & Special Orbs
    line!("\n--- Testing Requirement 10: Upgrades & Rare Special Pickups ---");
    {
        let mut ev = Vec::new();
        let mut w = World::new(Field::new(64, 24));
        w.queued.clear();
        w.enemies.clear();
        w.intermission = 0;
        w.step([None, None], &mut ev);
        check!(
            "Clearing a wave queues 3 distinct upgrade choices",
            w.draft_pending
                && w.upgrade_choices[0] != w.upgrade_choices[1]
                && w.upgrade_choices[1] != w.upgrade_choices[2]
        );

        w.upgrade_choices = [
            crate::game::Upgrade::OverclockBeam,
            crate::game::Upgrade::WaveBarrier,
            crate::game::Upgrade::OrbMagnet,
        ];
        w.pick_upgrade(0);
        w.pick_upgrade(1);
        w.pick_upgrade(2);
        check!(
            "Overclock increases beam damage & drain; Barrier grants shield; Magnet pulls orbs",
            w.beam_damage() == 2
                && w.drain_upgrade_mult() > 1.2
                && w.players[0].barrier > 0
                && !w.draft_pending
        );

        // Shield barrier absorbs a hit
        w.players[0].invuln = 0;
        let hp_before = w.players[0].hp;
        w.bullets.push(crate::game::Bullet {
            pos: crate::core::Vec2f::from_cell(w.players[0].pos),
            vel: crate::core::Vec2f::new(0.0, 0.0),
            ttl: 5,
            heavy: false,
            from_boss: false,
        });
        w.step([None, None], &mut ev);
        check!("Phase Shield barrier blocks hit without HP loss", w.players[0].hp == hp_before && w.players[0].barrier == 0);

        // OrbMagnet pulls nearby orbs toward alive operators
        w.orbs.clear();
        w.orbs.push(crate::game::Orb {
            pos: Vec2::new(w.players[0].pos.x + 5, w.players[0].pos.y),
            kind: crate::game::OrbKind::Charge,
            ttl: 60,
        });
        let ox_before = w.orbs[0].pos.x;
        for _ in 0..4 {
            w.step([None, None], &mut ev);
        }
        check!("OrbMagnet pulls nearby orb toward player", w.orbs[0].pos.x < ox_before);

        // Special pickups: Nova clears bullets & damages foes; TwinBeam activates twin arc
        w.bullets.push(crate::game::Bullet {
            pos: crate::core::Vec2f::new(10.0, 10.0),
            vel: crate::core::Vec2f::new(0.2, 0.0),
            ttl: 20,
            heavy: false,
            from_boss: false,
        });
        w.enemies.clear();
        w.enemies.push(Enemy::new(Vec2::new(40, 18), EnemyKind::Bulwark, crate::core::Dir::Left, 0));
        let b_hp = w.enemies[0].hp;
        w.orbs.push(crate::game::Orb {
            pos: w.players[0].pos,
            kind: crate::game::OrbKind::Nova,
            ttl: 50,
        });
        w.orbs.push(crate::game::Orb {
            pos: w.players[1].pos,
            kind: crate::game::OrbKind::TwinBeam,
            ttl: 50,
        });
        w.step([None, None], &mut ev);
        check!(
            "Nova clears bullets/damages foes and TwinBeam activates parallel arc",
            w.bullets.is_empty() && w.enemies[0].hp < b_hp && w.twin_t > 0
        );
    }

    // 11. Arena Obstacles (Beam Blocking), Hazard Zones & Random Events
    line!("\n--- Testing Requirement 11: Obstacles, Hazards & Wave Events ---");
    {
        let mut ev = Vec::new();
        let mut w = World::new(Field::new(64, 24));
        w.wave = 3;
        w.queued.clear();
        w.enemies.clear();
        w.intermission = 100;
        w.walls.push(crate::game::Wall {
            pos: Vec2::new(24, 10),
            len: 5,
            vertical: true,
            vel: 0,
            min_c: 2,
            max_c: 20,
        });
        w.players[0].pos = Vec2::new(16, 12);
        w.players[1].pos = Vec2::new(32, 12);
        // Enemy stands behind the wall (between x=24 wall and P2 at x=32)
        w.enemies.push(Enemy::new(Vec2::new(28, 12), EnemyKind::Drifter, crate::core::Dir::Left, 0));
        w.step([None, None], &mut ev);
        check!(
            "Patrolling obstacle blocks conduit beam past wall and protects far side",
            w.blocked_at == Some(Vec2::new(24, 12)) && w.enemies.len() == 1
        );

        // Active hazard zone damages standing operator
        w.walls.clear();
        w.players[0].pos = Vec2::new(20, 12);
        w.players[0].invuln = 0;
        let hp0 = w.players[0].hp;
        w.hazards.push(crate::game::HazardZone {
            pos: Vec2::new(19, 11),
            w: 3,
            h: 2,
            warn: 0,
            active: 10,
        });
        w.step([None, None], &mut ev);
        check!("Active floor hazard zone damages operator inside", w.players[0].hp < hp0);

        // Drought wave event reduces hug recharge rate
        w.hazards.clear();
        w.players[0].pos = Vec2::new(20, 12);
        w.players[1].pos = Vec2::new(22, 12);
        w.charge = 50.0;
        w.event = crate::game::WaveEvent::None;
        w.step([None, None], &mut ev);
        let gain_normal = w.charge - 50.0;
        w.charge = 50.0;
        w.event = crate::game::WaveEvent::Drought;
        w.step([None, None], &mut ev);
        let gain_drought = w.charge - 50.0;
        check!("Charge Drought event reduces recharge rate", gain_drought < gain_normal);
    }

    // 12. Polish, Time Tracking & High-Score Table
    line!("\n--- Testing Requirement 12: Polish, Time Clock & High Scores ---");
    {
        let mut ev = Vec::new();
        let mut w = World::new(Field::new(64, 24));
        w.intermission = 100;
        for _ in 0..20 {
            w.step([None, None], &mut ev);
        }
        check!("World run clock tracks elapsed simulation ticks", w.ticks == 20 && w.elapsed_secs() == 1);

        let mut list = Vec::new();
        crate::scores::record_entry(&mut list, crate::scores::make_entry(1500, 5, 30, Difficulty::Normal, false, 60));
        crate::scores::record_entry(&mut list, crate::scores::make_entry(4200, 12, 95, Difficulty::Hard, true, 140));
        crate::scores::record_entry(&mut list, crate::scores::make_entry(800, 3, 12, Difficulty::Easy, false, 30));
        crate::scores::record_entry(&mut list, crate::scores::make_entry(2200, 8, 50, Difficulty::Normal, false, 90));
        crate::scores::record_entry(&mut list, crate::scores::make_entry(3100, 10, 70, Difficulty::Normal, false, 110));
        crate::scores::record_entry(&mut list, crate::scores::make_entry(999, 4, 20, Difficulty::Normal, false, 45));
        check!(
            "High-score table sorts descending and caps at top 5",
            list.len() == 5 && list[0].score == 4200 && list[4].score == 999
        );
    }

    // 13. Single-Tap Precision, DAS/ARR Hold Cadence & Coherent Stride
    line!("\n--- Testing Requirement 13: Tap Precision, DAS/ARR & Coherent Stride ---");
    {
        use crate::input::{Input, ARR_MS, DAS_MS};
        use crate::keys::{Key, KeyEv};

        // 1 tap executes 1 exact cell and rests inside DAS window before release
        let mut inp = Input::new();
        inp.set_has_release(true);
        inp.expire(0);
        inp.feed(KeyEv::press(Key::W));
        let t0 = inp.step_dirs();
        inp.expire(DAS_MS - 20);
        let t_mid = inp.step_dirs();
        inp.feed(KeyEv::release(Key::W));
        check!(
            "Single click executes exactly 1 step with no overshoot inside DAS",
            t0 == [Some(Dir::Up), None] && t_mid == [None, None]
        );

        // Lightning tap (<60ms: Press + Release before tick) still steps 1 cell
        let mut fast = Input::new();
        fast.set_has_release(true);
        fast.feed(KeyEv::press(Key::Right));
        fast.feed(KeyEv::release(Key::Right));
        let fast_step = fast.step_dirs();
        let after_step = fast.step_dirs();
        check!(
            "Quick Press+Release before frame still executes 1 guaranteed cell",
            fast_step == [None, Some(Dir::Right)] && after_step == [None, None]
        );

        // Sustained hold past DAS_MS repeats cleanly at ARR_MS cadence
        let mut hold = Input::new();
        hold.set_has_release(true);
        hold.expire(0);
        hold.feed(KeyEv::press(Key::D));
        let _ = hold.step_dirs();
        hold.expire(DAS_MS + 5);
        let r1 = hold.step_dirs();
        hold.expire(DAS_MS + 5 + ARR_MS - 10);
        let r_wait = hold.step_dirs();
        hold.expire(DAS_MS + 5 + ARR_MS);
        let r2 = hold.step_dirs();
        check!(
            "Held key pauses across DAS then steps at ARR cadence",
            r1 == [Some(Dir::Right), None] && r_wait == [None, None] && r2 == [Some(Dir::Right), None]
        );

        // Release-less TTY mode never multi-steps from a single stroke
        let mut tty = Input::new();
        tty.set_has_release(false);
        tty.expire(0);
        tty.feed(KeyEv::press(Key::S));
        let s0 = tty.step_dirs();
        tty.expire(DAS_MS + ARR_MS);
        let s_later = tty.step_dirs();
        check!(
            "Release-less TTY single tap moves 1 cell without ghost repeats",
            s0 == [Some(Dir::Down), None] && s_later == [None, None]
        );

        // Coherent Stride reduces hot drain when both operators move together
        let mut ev = Vec::new();
        let mut idle_w = World::new(Field::new(64, 24));
        idle_w.intermission = 100;
        idle_w.step([None, None], &mut ev);
        let idle_drain = 100.0 - idle_w.charge;

        let mut sync_w = World::new(Field::new(64, 24));
        sync_w.intermission = 100;
        sync_w.step([Some(Dir::Up), Some(Dir::Up)], &mut ev);
        let sync_drain = 100.0 - sync_w.charge;
        check!(
            "Coherent synchronized movement activates SYNC and lowers beam drain",
            sync_w.synced && sync_drain < idle_drain
        );
    }

    // 14. Auto-Pause on Tab Minimize / Focus Loss & Resume
    line!("\n--- Testing Requirement 14: Auto-Pause on Focus Loss & Resume ---");
    {
        use crate::app::{App, Mode};
        use crate::fx::Fx;
        use crate::input::{Flags, Input};
        use crate::keys::{Key, KeyEv};

        let mut app = App::new(100, 32);
        let mut inp = Input::new();
        let mut fx = Fx::new();
        app.update(&mut inp, &mut fx, Flags { confirm: true, ..Flags::default() });
        let chg_before = app.world.charge;
        inp.feed(KeyEv::FocusLost);
        app.update(&mut inp, &mut fx, Flags::default());
        let paused_on_blur = app.mode == Mode::Pause && (app.world.charge - chg_before).abs() < 1e-4;
        inp.feed(KeyEv::FocusGained);
        inp.feed(KeyEv::press(Key::Enter));
        let flags = inp.take_flags();
        app.update(&mut inp, &mut fx, flags);
        check!(
            "Losing window/tab focus pauses run safely and Enter/P resumes cleanly",
            paused_on_blur && app.mode == Mode::Run
        );
    }

    line!("\n============================================================");
    line!("  VERIFICATION RESULTS: {passed}/{total} CHECKS PASSED");
    line!("============================================================");
    if passed == total {
        line!("  Status: ALL REQUIREMENTS VERIFIED SUCCESSFULLY");
    } else {
        line!("  Status: SOME CHECKS FAILED");
    }

    CheckReport {
        passed,
        total,
        log: out,
    }
}


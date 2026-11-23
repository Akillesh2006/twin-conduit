//! OVERDRIVE - entry point and frame loop.
//!
//!   Player 1 : W A S D
//!   Player 2 : arrow keys
//!   P pause   R restart   Esc back / quit

mod sys;

use std::io::{self, IsTerminal};
use std::time::Instant;

use overdrive::app::{App, Mode};
use overdrive::canvas::Canvas;
use overdrive::fx::Fx;
use overdrive::game::TICK_MS;
use overdrive::input::Input;

/// Restore the terminal even if something panics mid-frame.
struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        sys::leave();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--test" || a == "--verify") {
        run_verification();
        return;
    }

    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        sys::leave();
        prev(info);
    }));

    if !io::stdout().is_terminal() {
        eprintln!("OVERDRIVE needs a real terminal (stdout is redirected).");
        eprintln!("To run automated verification tests, pass the --test flag.");
        std::process::exit(1);
    }

    if let Err(e) = run() {
        sys::leave();
        eprintln!("OVERDRIVE stopped: {e}");
        std::process::exit(1);
    }
}

fn run_verification() {
    println!("============================================================");
    println!("  OVERDRIVE - Two-Player Monster Defense Verification Tests ");
    println!("============================================================\n");

    let mut passed = 0;
    let mut total = 0;

    macro_rules! check {
        ($desc:expr, $cond:expr) => {
            total += 1;
            if $cond {
                println!("  [PASS] {}", $desc);
                passed += 1;
            } else {
                println!("  [FAIL] {}", $desc);
            }
        };
    }

    // 1. Two-player defense requirements
    println!("--- Testing Requirement 1: 2 Players Defend Against Monsters ---");
    {
        use overdrive::core::{Dir, Field, Side, Vec2f};
        use overdrive::game::{Enemy, EnemyKind, World, MAX_HP, REVIVE_TICKS};

        let mut w = World::new(Field::new(64, 24));
        check!("Two players initialized in game world", w.players.len() == 2);
        check!("Player 1 is Side::One (WASD controls)", w.players[0].side == Side::One);
        check!("Player 2 is Side::Two (Arrow key controls)", w.players[1].side == Side::Two);
        check!("Both players start with MAX_HP (5)", w.players[0].hp == MAX_HP && w.players[1].hp == MAX_HP);
        check!("Both players start alive", w.players[0].alive && w.players[1].alive);

        // Movement test: both players move independently
        let p1_start = w.players[0].pos;
        let p2_start = w.players[1].pos;
        w.step([Some(Dir::Up), Some(Dir::Right)]);
        check!(
            "Simultaneous movement updates both player positions independently",
            w.players[0].pos.y < p1_start.y && w.players[1].pos.x > p2_start.x
        );

        // Monster defense / Auto-firing test
        let mut w_fight = World::new(Field::new(64, 24));
        w_fight.intermission = 100;
        w_fight.queued.clear();
        w_fight.enemies.clear();
        w_fight.players[0].pos = Vec2f::new(20.0, 12.0);
        w_fight.players[1].pos = Vec2f::new(44.0, 12.0);
        w_fight.enemies.push(Enemy {
            pos: Vec2f::new(26.0, 12.0),
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
        for _ in 0..60 {
            w_fight.step([None, None]);
            if w_fight.kills > 0 {
                break;
            }
        }
        check!(
            "Players' auto-cannons acquire, fire at, and eliminate monsters",
            w_fight.shots_fired > 0 && w_fight.kills > 0
        );

        // Cooperative revival test
        let mut w_rev = World::new(Field::new(64, 24));
        w_rev.intermission = 100;
        w_rev.queued.clear();
        w_rev.enemies.clear();
        w_rev.players[0].hp = 0;
        w_rev.players[0].alive = false;
        w_rev.players[0].pos = Vec2f::new(30.0, 12.0);
        w_rev.players[1].pos = Vec2f::new(31.0, 12.0); // Standing next to partner
        for _ in 0..REVIVE_TICKS + 5 {
            w_rev.step([None, None]);
        }
        check!(
            "Downed player is revived when partner stands nearby to re-link",
            w_rev.players[0].alive && w_rev.players[0].hp > 0
        );
    }

    println!("\n--- Testing Requirement 2: Boss Every 5th Wave ---");
    {
        use overdrive::core::Field;
        use overdrive::game::{wave_spec, EnemyKind, Ev, Phase, World, BOSS_EVERY, TOTAL_WAVES};

        check!("BOSS_EVERY constant is set to 5", BOSS_EVERY == 5);

        // Check wave_is_boss for all waves
        let w = World::new(Field::new(64, 24));
        let mut boss_waves_correct = true;
        for wave in 1..=TOTAL_WAVES {
            let expected = wave % 5 == 0;
            if w.wave_is_boss(wave) != expected {
                boss_waves_correct = false;
                break;
            }
        }
        check!(
            "wave_is_boss() returns true exactly for multiples of 5 (waves 5, 10, 15)",
            boss_waves_correct
        );

        // Check wave rosters
        let w5 = wave_spec(5);
        let w10 = wave_spec(10);
        let w15 = wave_spec(15);
        let w4 = wave_spec(4);
        let w6 = wave_spec(6);

        check!(
            "Wave 5 roster contains Boss (THE WARDEN)",
            w5.iter().any(|k| *k == EnemyKind::Warden && k.is_boss())
        );
        check!(
            "Wave 10 roster contains Boss (HYDRA)",
            w10.iter().any(|k| *k == EnemyKind::Hydra && k.is_boss())
        );
        check!(
            "Wave 15 roster contains Boss (OVERMIND)",
            w15.iter().any(|k| *k == EnemyKind::Overmind && k.is_boss())
        );
        check!(
            "Non-5th waves (e.g. Wave 4, Wave 6) do NOT contain any boss",
            !w4.iter().any(|k| k.is_boss()) && !w6.iter().any(|k| k.is_boss())
        );

        // Check boss stats and scaling
        check!(
            "Bosses have balanced high HP, large collision radius, and heavy contact damage",
            EnemyKind::Warden.hp() >= 40
                && EnemyKind::Hydra.hp() >= 100
                && EnemyKind::Overmind.hp() >= 180
                && EnemyKind::Warden.radius() >= 1.5
                && EnemyKind::Warden.contact() == 2
        );

        // Check Boss tracking & HUD progress during a boss wave
        let mut w_boss = World::new(Field::new(64, 24));
        w_boss.wave = 5;
        w_boss.queued = wave_spec(5);
        w_boss.enemies.clear();
        w_boss.intermission = 0;
        w_boss.spawn_cd = 0;
        w_boss.step([None, None]); // Spawns boss
        let boss_active = w_boss.boss().is_some();
        let (boss_hp_ratio, boss_name) = w_boss.boss_progress().unwrap_or((0.0, ""));
        check!(
            "Boss spawns on Wave 5 and HUD reports boss presence and health",
            boss_active && boss_name == "THE WARDEN" && boss_hp_ratio > 0.99
        );

        // Check Wave 15 completion triggers victory
        let mut w_win = World::new(Field::new(64, 24));
        w_win.wave = TOTAL_WAVES;
        w_win.queued.clear();
        w_win.enemies.clear();
        w_win.intermission = 0;
        let evs = w_win.step([None, None]);
        check!(
            "Clearing the final boss wave (Wave 15) triggers victory (Phase::Won & Ev::Win)",
            w_win.phase == Phase::Won && evs.iter().any(|e| matches!(e, Ev::Win))
        );
    }

    println!("\n--- Testing Requirement 3: Tougher Normal Waves ---");
    {
        use overdrive::game::{wave_spec, EnemyKind};

        let w1 = wave_spec(1);
        let w2 = wave_spec(2);
        let w3 = wave_spec(3);
        let w4 = wave_spec(4);

        check!("Wave 1 features increased monster pack size (>= 6 monsters)", w1.len() >= 6);
        check!("Wave 2 features heavy swarms and ranged attackers (>= 12 monsters)", w2.len() >= 12);
        check!("Wave 4 features high-intensity horde defense (>= 20 monsters)", w4.len() >= 20);
        check!("Normal waves feature monster variety: Grunts, Spitters, Weavers, Swarms, Bulwarks",
            w3.iter().any(|k| *k == EnemyKind::Grunt)
                && w3.iter().any(|k| *k == EnemyKind::Spitter)
                && w3.iter().any(|k| *k == EnemyKind::Weaver)
                && w3.iter().any(|k| *k == EnemyKind::Swarm)
                && w3.iter().any(|k| *k == EnemyKind::Bulwark));
    }

    println!("\n--- Testing Requirement 4: Evolution & Combat Pet Systems ---");
    {
        use overdrive::core::{Field, Side, Vec2f};
        use overdrive::game::{Enemy, EnemyKind, Hazard, HazardKind, Pet, Pickup, PickupKind, World};

        let mut w = World::new(Field::new(64, 24));
        w.intermission = 100;

        // Test Evolution Pickup:
        w.pickups.push(Pickup {
            pos: w.players[0].pos,
            kind: PickupKind::Evolve,
            ttl: 300,
            ph: 0,
        });
        w.step([None, None]);
        check!("Collecting Evolve pickup upgrades player weapon evolution tier", w.players[0].evolution >= 1);

        // Test Evolved Multi-Shot:
        w.players[0].evolution = 2; // Tri-spread
        w.players[0].gun_cd = 0;
        w.enemies.push(Enemy {
            pos: Vec2f::new(w.players[0].pos.x + 8.0, w.players[0].pos.y),
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
        w.step([None, None]);
        check!("Evolved weapons fire multi-shot spread patterns", w.shots.len() >= 3);

        // Test Combat Pet Summoning:
        w.pickups.push(Pickup {
            pos: w.players[0].pos,
            kind: PickupKind::Pet,
            ttl: 300,
            ph: 0,
        });
        w.step([None, None]);
        check!("Collecting Pet pickup summons combat drone pets for the operators", !w.pets.is_empty());

        // Test Pet Bullet Point Defense:
        let mut w_pet = World::new(Field::new(64, 24));
        w_pet.intermission = 100;
        w_pet.players[0].pos = Vec2f::new(20.0, 12.0);
        w_pet.pets.push(Pet {
            pos: Vec2f::new(21.8, 12.0),
            angle: -0.10,
            level: 2, // Sentinel
            fire_cd: 0,
            side: Side::One,
        });
        w_pet.hazards.push(Hazard {
            pos: Vec2f::new(21.8, 12.0),
            vel: Vec2f::new(0.0, 0.0),
            ttl: 30,
            kind: HazardKind::Pellet,
            r: HazardKind::Pellet.radius(),
        });
        w_pet.step([None, None]);
        check!("Sentinel & Apex combat pets zap and neutralize incoming enemy bullets", w_pet.hazards.is_empty());
    }

    println!("\n============================================================");
    println!("  Summary: {} / {} checks PASSED", passed, total);
    println!("============================================================");

    if passed == total {
        println!("\nAll requirements verified successfully!");
        std::process::exit(0);
    } else {
        eprintln!("\nSome requirements failed verification.");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    sys::enter()?;
    let _guard = Guard;

    let (mut tw, mut th) = sys::size();
    let mut canvas = Canvas::new(tw, th);
    canvas.invalidate();
    let mut console = sys::Console::new();

    let mut app = App::new(tw, th);
    let mut input = Input::new();
    let mut fx = Fx::new();

    let start = Instant::now();
    let tick = TICK_MS as f32 / 1000.0;
    let mut acc = 0.0f32;
    let mut last = Instant::now();
    let mut events = Vec::with_capacity(32);

    loop {
        events.clear();
        sys::poll_events(2, &mut events)?;

        let now_ms = start.elapsed().as_millis() as u64;
        input.expire(now_ms);
        for ev in events.drain(..) {
            input.feed(ev);
        }

        let (nw, nh) = sys::size();
        if nw != tw || nh != th {
            tw = nw;
            th = nh;
            canvas.resize(tw, th);
            canvas.invalidate();
            app.relayout(tw, th);
        }

        let now = Instant::now();
        acc += now.duration_since(last).as_secs_f32().min(0.25);
        last = now;

        let mut flags = Some(input.take_flags());
        if flags.unwrap_or_default().quit {
            if app.mode == Mode::Title {
                return Ok(());
            }
            app.to_title();
            input.release_all();
        }

        let mut steps = 0;
        while acc >= tick && steps < 5 {
            app.update(&input, &mut fx, flags.take().unwrap_or_default());
            acc -= tick;
            steps += 1;
        }
        if acc > tick * 4.0 {
            acc = 0.0;
        }

        if steps > 0 {
            app.draw(&mut canvas, &fx);
            canvas.flush(&mut console)?;
        }
    }
}

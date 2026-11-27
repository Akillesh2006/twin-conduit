//! CONDUIT - entry point and frame loop.
//!
//!   Player 1 : W A S D
//!   Player 2 : arrow keys
//!   P pause   R restart   Esc back / quit

mod sys;

use std::io::{self, IsTerminal};
use std::time::Instant;

use conduit::app::{App, Mode};
use conduit::canvas::Canvas;
use conduit::fx::Fx;
use conduit::game::TICK_MS;
use conduit::input::Input;

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
        eprintln!("CONDUIT needs a real terminal (stdout is redirected).");
        eprintln!("To run automated verification tests, pass the --test flag.");
        std::process::exit(1);
    }

    if let Err(e) = run() {
        sys::leave();
        eprintln!("CONDUIT stopped: {e}");
        std::process::exit(1);
    }
}

fn run_verification() {
    println!("============================================================");
    println!("   CONDUIT - Cooperative Energy Tether Verification Tests   ");
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

    // 1. Two-player and tether setup
    println!("--- Testing Requirement 1: Two Operators Joined by Energy Beam ---");
    {
        use conduit::core::{Dir, Field, Side};
        use conduit::game::{World, HOT_DIST, MAX_HP, REST_DIST, SNAP_DIST};

        let mut w = World::new(Field::new(64, 24));
        check!("Two players initialized in game world", w.players.len() == 2);
        check!("Player 1 is Side::One (WASD controls)", w.players[0].side == Side::One);
        check!("Player 2 is Side::Two (Arrow key controls)", w.players[1].side == Side::Two);
        check!("Both players start with MAX_HP (3)", w.players[0].hp == MAX_HP && w.players[1].hp == MAX_HP);
        check!("Both players start alive", w.players[0].alive && w.players[1].alive);
        check!("Conduit line (arc) is calculated between players", !w.arc.is_empty());
        check!("Conduit thresholds configured cleanly", REST_DIST < HOT_DIST && HOT_DIST < SNAP_DIST);
        check!("Distance between players is calculated", w.dist > 0);

        // Movement test: both players move independently
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
    println!("\n--- Testing Requirement 2: Beam Burns When Stretched Apart (dist >= HOT_DIST) ---");
    {
        use conduit::core::{Field, Vec2};
        use conduit::game::{Enemy, EnemyKind, World, HOT_DIST};

        let mut w = World::new(Field::new(64, 24));
        w.intermission = 100;
        w.queued.clear();
        w.enemies.clear();

        // Place players 12 cells apart (HOT_DIST is 6)
        w.players[0].pos = Vec2::new(10, 12);
        w.players[1].pos = Vec2::new(22, 12);

        let mut ev = Vec::new();
        w.step([None, None], &mut ev);
        check!("Conduit is hot when dist >= HOT_DIST", w.hot && w.dist >= HOT_DIST);

        // Place an enemy directly on the conduit beam
        let beam_cell = w.arc[w.arc.len() / 2];
        w.enemies.push(Enemy::new(beam_cell, EnemyKind::Drifter, conduit::core::Dir::Down, 0));

        w.step([None, None], &mut ev);
        check!(
            "Live conduit burns and destroys enemy touching the beam line",
            w.kills > 0 && w.enemies.is_empty()
        );
    }

    // 3. Huddling recharges shared charge
    println!("\n--- Testing Requirement 3: Huddling (dist <= REST_DIST) Recharges Shared Cell ---");
    {
        use conduit::core::{Field, Vec2};
        use conduit::game::{World, REST_DIST};

        let mut w = World::new(Field::new(64, 24));
        w.intermission = 100;
        w.queued.clear();
        w.enemies.clear();

        w.charge = 50.0;
        // Huddle: players within REST_DIST (cheb <= 3)
        w.players[0].pos = Vec2::new(20, 12);
        w.players[1].pos = Vec2::new(22, 12);

        let mut ev = Vec::new();
        w.step([None, None], &mut ev);
        check!("Conduit is cold when huddling (dist <= REST_DIST)", !w.hot && w.dist <= REST_DIST);
        check!("Shared charge recharges while huddling", w.charge > 50.0);
    }

    // 4. Zero charge ends the run
    println!("\n--- Testing Requirement 4: Zero Charge Ends Run (Both Lose) ---");
    {
        use conduit::core::Field;
        use conduit::game::{Phase, World};

        let mut w = World::new(Field::new(64, 24));
        w.intermission = 100;
        w.charge = 0.05;
        // Hot state to drain
        w.players[0].pos = conduit::core::Vec2::new(10, 12);
        w.players[1].pos = conduit::core::Vec2::new(25, 12);

        let mut ev = Vec::new();
        w.step([None, None], &mut ev);
        check!(
            "Zero charge terminates run with Phase::Lost",
            w.charge <= 0.0 && w.phase == Phase::Lost
        );
    }

    // 5. Downed and Reviving
    println!("\n--- Testing Requirement 5: Downed Players & Cooperative Reviving ---");
    {
        use conduit::core::{Field, Vec2};
        use conduit::game::{World, REVIVE_TICKS};

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

        // Partner moves close to revive (within 2 cells)
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
    println!("\n--- Testing Requirement 6: 12 Waves and Boss (THE NULL) ---");
    {
        use conduit::core::Field;
        use conduit::game::{wave_queue, EnemyKind, Phase, World, TOTAL_WAVES};

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
        check!(
            "Clearing wave 12 achieves victory (Phase::Won)",
            w.phase == Phase::Won
        );
    }

    println!("\n============================================================");
    println!("  VERIFICATION RESULTS: {passed}/{total} CHECKS PASSED");
    println!("============================================================");

    if passed == total {
        println!("  Status: ALL REQUIREMENTS VERIFIED SUCCESSFULLY");
    } else {
        println!("  Status: SOME CHECKS FAILED");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let _guard = Guard;
    sys::enter()?;

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

        app.draw(&mut canvas, &fx);
        canvas.flush(&mut console)?;
    }
}

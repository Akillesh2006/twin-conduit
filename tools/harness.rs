//! Headless harness: drives the real `App` / `Canvas` / `World` and exposes a
//! few C functions so it can be run from Node via WASM.
//!
//! This exists because building a native binary on this machine is possible but
//! *running* one is blocked by Smart App Control. Compiling to wasm needs no
//! external linker, and Node executes it, so we can validate the actual
//! rendering and simulation code paths end to end.
//!
//! Build:
//!   rustc --target wasm32-unknown-unknown --crate-type cdylib -O \
//!         tools/harness.rs -o tools/harness.wasm

#![allow(dead_code)]

#[path = "../src/canvas.rs"]
mod canvas;
#[path = "../src/core.rs"]
mod core;
#[path = "../src/fx.rs"]
mod fx;
#[path = "../src/game.rs"]
mod game;
#[path = "../src/input.rs"]
mod input;
#[path = "../src/keys.rs"]
mod keys;
#[path = "../src/rng.rs"]
mod rng;
#[path = "../src/scene.rs"]
mod scene;
#[path = "../src/ui.rs"]
mod ui;
#[path = "../src/app.rs"]
mod app;

use app::{App, Mode};
use canvas::Canvas;
use core::Dir;
use fx::Fx;
use input::{Flags, Input};
use keys::{Key, KeyEv, Mods};

static mut APP: Option<App> = None;
static mut CANVAS: Option<Canvas> = None;
static mut INPUT: Option<Input> = None;
static mut FX: Option<Fx> = None;
static mut NOW: u64 = 0;
static mut OUT: Vec<u8> = Vec::new();

fn app() -> &'static mut App {
    unsafe { APP.as_mut().unwrap() }
}
fn canvas() -> &'static mut Canvas {
    unsafe { CANVAS.as_mut().unwrap() }
}
fn input() -> &'static mut Input {
    unsafe { INPUT.as_mut().unwrap() }
}
fn fx() -> &'static mut Fx {
    unsafe { FX.as_mut().unwrap() }
}

fn dir_of(code: i32) -> Option<Dir> {
    match code {
        0 => Some(Dir::Up),
        1 => Some(Dir::Down),
        2 => Some(Dir::Left),
        3 => Some(Dir::Right),
        _ => None,
    }
}

fn key_of(id: i32) -> Key {
    match id {
        0 => Key::W,
        1 => Key::A,
        2 => Key::S,
        3 => Key::D,
        4 => Key::Up,
        5 => Key::Down,
        6 => Key::Left,
        7 => Key::Right,
        8 => Key::Enter,
        9 => Key::Space,
        10 => Key::Esc,
        11 => Key::P,
        12 => Key::R,
        13 => Key::Q,
        _ => Key::Other,
    }
}

#[no_mangle]
pub extern "C" fn init(tw: i32, th: i32) {
    let mut cv = Canvas::new(tw, th);
    cv.invalidate();
    unsafe {
        CANVAS = Some(cv);
        APP = Some(App::new(tw, th));
        INPUT = Some(Input::new());
        FX = Some(Fx::new());
        NOW = 0;
    }
}

/// Advance one simulation step. `d0`/`d1` are -1 (none) or 0..3 = U/D/L/R.
/// `bits` is a bitmask of Flags: 1 confirm, 2 quit, 4 pause, 8 restart.
#[no_mangle]
pub extern "C" fn step(d0: i32, d1: i32, bits: i32) {
    unsafe {
        NOW += 60;
        let inp = INPUT.as_mut().unwrap();
        inp.expire(NOW);
        inp.release_all();
        set_dir(inp, 0, d0);
        set_dir(inp, 1, d1);
        let mut flags = inp.take_flags();
        flags.confirm |= bits & 1 != 0;
        flags.quit |= bits & 2 != 0;
        flags.pause |= bits & 4 != 0;
        flags.restart |= bits & 8 != 0;
        let a = APP.as_mut().unwrap();
        let f = FX.as_mut().unwrap();
        a.update(inp, f, flags);
        let cv = CANVAS.as_mut().unwrap();
        a.draw(cv, f);
    }
}

fn set_dir(inp: &mut Input, player: usize, code: i32) {
    let Some(dir) = dir_of(code) else { return };
    let key = match (player, dir) {
        (0, Dir::Up) => Key::W,
        (0, Dir::Down) => Key::S,
        (0, Dir::Left) => Key::A,
        (0, Dir::Right) => Key::D,
        (1, Dir::Up) => Key::Up,
        (1, Dir::Down) => Key::Down,
        (1, Dir::Left) => Key::Left,
        (1, Dir::Right) => Key::Right,
        _ => return,
    };
    inp.feed(KeyEv::Press(key, Mods::plain()));
}

/// Synthesise a real key event through the input state machine.
/// `kind`: 0 press, 1 release, 2 repeat, 3 focus lost.
#[no_mangle]
pub extern "C" fn key_event(id: i32, kind: i32) {
    unsafe {
        NOW += 10;
        let inp = INPUT.as_mut().unwrap();
        inp.expire(NOW);
        match kind {
            0 => inp.feed(KeyEv::Press(key_of(id), Mods::plain())),
            1 => inp.feed(KeyEv::Release(key_of(id))),
            2 => inp.feed(KeyEv::Repeat(key_of(id), Mods::plain())),
            _ => inp.feed(KeyEv::FocusLost),
        }
    }
}

/// Draw the current screen without advancing the simulation (for headless runs).
#[no_mangle]
pub extern "C" fn draw_now() {
    unsafe {
        let a = APP.as_mut().unwrap();
        let f = FX.as_mut().unwrap();
        let cv = CANVAS.as_mut().unwrap();
        a.draw(cv, f);
    }
}

/// Advance one tick using the real input state machine (no synthetic dirs).
/// This is the entry point used by the Node front end (`tools/play-cli.mjs`).
/// Returns 1 when the player asked to quit from the title screen.
#[no_mangle]
pub extern "C" fn tick() -> i32 {
    unsafe {
        NOW += 60;
        let inp = INPUT.as_mut().unwrap();
        inp.expire(NOW);
        let mut flags = inp.take_flags();
        let a = APP.as_mut().unwrap();
        if flags.quit {
            if a.mode == Mode::Title {
                return 1;
            }
            a.to_title();
            inp.release_all();
            flags.quit = false;
        }
        let f = FX.as_mut().unwrap();
        a.update(inp, f, flags);
        let cv = CANVAS.as_mut().unwrap();
        a.draw(cv, f);
        0
    }
}

/// Resize the game (terminal was resized).
#[no_mangle]
pub extern "C" fn resize(tw: i32, th: i32) {
    unsafe {
        let cv = CANVAS.as_mut().unwrap();
        cv.resize(tw, th);
        cv.invalidate();
        APP.as_mut().unwrap().relayout(tw, th);
    }
}

/// Plain-text frame.
#[no_mangle]
pub extern "C" fn render() -> i32 {
    unsafe {
        let cv = CANVAS.as_mut().unwrap();
        OUT = cv.snapshot().into_bytes();
        OUT.as_ptr() as i32
    }
}

/// ANSI frame (exactly what the real console would receive).
#[no_mangle]
pub extern "C" fn render_ansi() -> i32 {
    unsafe {
        let cv = CANVAS.as_mut().unwrap();
        OUT = cv.ansi_frame();
        OUT.as_ptr() as i32
    }
}

/// JSON snapshot of the simulation state.
#[no_mangle]
pub extern "C" fn state() -> i32 {
    unsafe {
        let a = APP.as_mut().unwrap();
        let w = &a.world;
        let mode = match a.mode {
            Mode::Title => "title",
            Mode::Run => "run",
            Mode::Pause => "pause",
            Mode::Over => "over",
        };
        let phase = match w.phase {
            game::Phase::Playing => "playing",
            game::Phase::Won => "won",
            game::Phase::Lost => "lost",
        };
        let foes: Vec<String> = w
            .enemies
            .iter()
            .take(12)
            .map(|e| format!("[{:.1},{:.1},{}]", e.pos.x, e.pos.y, e.hp))
            .collect();
        // Nearby threats, so the Node dodge-bot can steer around real bullets.
        let hz: Vec<String> = w
            .hazards
            .iter()
            .take(96)
            .map(|h| format!("[{:.1},{:.1}]", h.pos.x, h.pos.y))
            .collect();
        let (boss_hp, boss_max) = w.boss().map(|b| (b.hp, b.max_hp)).unwrap_or((0, 0));
        let s = format!(
            "{{\"mode\":\"{}\",\"phase\":\"{}\",\"wave\":{},\"score\":{},\"kills\":{},\
             \"enemies\":{},\"hazards\":{},\"shots\":{},\"pickups\":{},\
             \"combo\":{},\"best_combo\":{},\
             \"px\":{:.1},\"py\":{:.1},\"qx\":{:.1},\"qy\":{:.1},\
             \"php\":{},\"qhp\":{},\"p1alive\":{},\"p2alive\":{},\
             \"boss\":{},\"bosshp\":{},\"bossmax\":{},\
             \"pressure\":{:.2},\"accuracy\":{:.1},\"won\":{},\"t\":{},\
             \"foes\":[{}],\"hz\":[{}]}}",
            mode,
            phase,
            w.wave,
            w.score,
            w.kills,
            w.enemies.len(),
            w.hazards.len(),
            w.shots.len(),
            w.pickups.len(),
            w.combo,
            w.best_combo,
            w.players[0].pos.x,
            w.players[0].pos.y,
            w.players[1].pos.x,
            w.players[1].pos.y,
            w.players[0].hp,
            w.players[1].hp,
            w.players[0].alive,
            w.players[1].alive,
            w.boss().is_some(),
            boss_hp,
            boss_max,
            w.adaptive.pressure,
            w.accuracy(),
            a.won,
            a.t,
            foes.join(","),
            hz.join(",")
        );
        OUT = s.into_bytes();
        OUT.as_ptr() as i32
    }
}

#[no_mangle]
pub extern "C" fn out_len() -> i32 {
    unsafe { OUT.len() as i32 }
}

/// Directly set both players' hit points, to exercise the lose path deterministically.
#[no_mangle]
pub extern "C" fn set_hp(v: i32) {
    unsafe {
        let w = &mut APP.as_mut().unwrap().world;
        for p in w.players.iter_mut() {
            p.hp = v;
            p.alive = v > 0;
            p.invuln = 0;
            p.revive = 0;
        }
        if v > 0 {
            w.phase = game::Phase::Playing;
        }
    }
}

/// Override the adaptive director's pressure (and comfort) for testing.
#[no_mangle]
pub extern "C" fn set_pressure(v: f32) {
    unsafe {
        let a = &mut APP.as_mut().unwrap().world.adaptive;
        a.pressure = v.clamp(0.0, 1.0);
        a.comfort = v.clamp(0.0, 1.0);
    }
}

/// Force a wave number (for boss / late-wave testing).
#[no_mangle]
pub extern "C" fn set_wave(w: u32) {
    unsafe {
        let a = APP.as_mut().unwrap();
        a.world.wave = w;
        a.world.queued = game::wave_spec(w);
        a.world.enemies.clear();
        a.world.hazards.clear();
        a.world.shots.clear();
        a.world.pickups.clear();
        a.world.intermission = 0;
        a.world.spawn_cd = 0;
        a.world.phase = game::Phase::Playing;
    }
}

/// Clear the current wave so the game resolves it (used to test victory).
#[no_mangle]
pub extern "C" fn clear_wave() {
    unsafe {
        let a = APP.as_mut().unwrap();
        a.world.queued.clear();
        a.world.enemies.clear();
        a.world.hazards.clear();
        a.world.intermission = 0;
    }
}

/// Run the simulation for `n` ticks without input, no rendering.
#[no_mangle]
pub extern "C" fn idle(n: i32) {
    unsafe {
        let a = APP.as_mut().unwrap();
        let f = FX.as_mut().unwrap();
        let inp = INPUT.as_mut().unwrap();
        for _ in 0..n {
            NOW += 60;
            inp.expire(NOW);
            inp.release_all();
            a.update(inp, f, Flags::default());
        }
    }
}

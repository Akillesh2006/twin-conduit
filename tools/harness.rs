//! Headless harness: drives the real `App` / `Canvas` / `World` and exposes a
//! few C functions so it can be run from Node via WASM.
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

#[no_mangle]
pub extern "C" fn draw_now() {
    unsafe {
        let a = APP.as_mut().unwrap();
        let f = FX.as_mut().unwrap();
        let cv = CANVAS.as_mut().unwrap();
        a.draw(cv, f);
    }
}

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

#[no_mangle]
pub extern "C" fn resize(tw: i32, th: i32) {
    unsafe {
        let cv = CANVAS.as_mut().unwrap();
        cv.resize(tw, th);
        cv.invalidate();
        APP.as_mut().unwrap().relayout(tw, th);
    }
}

#[no_mangle]
pub extern "C" fn render() -> i32 {
    unsafe {
        let cv = CANVAS.as_mut().unwrap();
        OUT = cv.snapshot().into_bytes();
        OUT.as_ptr() as i32
    }
}

#[no_mangle]
pub extern "C" fn render_ansi() -> i32 {
    unsafe {
        let cv = CANVAS.as_mut().unwrap();
        OUT = cv.ansi_frame();
        OUT.as_ptr() as i32
    }
}

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
            .map(|e| format!("[{},{}]", e.pos.x, e.pos.y))
            .collect();

        let s = format!(
            "{{\"mode\":\"{}\",\"phase\":\"{}\",\"wave\":{},\"score\":{},\"kills\":{},\
             \"charge\":{:.1},\"dist\":{},\"hot\":{},\"arc\":{},\
             \"px\":{},\"py\":{},\"qx\":{},\"qy\":{},\
             \"php\":{},\"qhp\":{},\"p1alive\":{},\"p2alive\":{},\
             \"enemies\":{},\"combo\":{},\"best_combo\":{},\
             \"won\":{},\"foes\":[{}]}}",
            mode,
            phase,
            w.wave,
            w.score,
            w.kills,
            w.charge,
            w.dist,
            w.hot,
            w.arc.len(),
            w.players[0].pos.x,
            w.players[0].pos.y,
            w.players[1].pos.x,
            w.players[1].pos.y,
            w.players[0].hp,
            w.players[1].hp,
            w.players[0].alive,
            w.players[1].alive,
            w.enemies.len(),
            w.combo,
            w.best_combo,
            a.won,
            foes.join(",")
        );
        OUT = s.into_bytes();
        OUT.as_ptr() as i32
    }
}

#[no_mangle]
pub extern "C" fn out_len() -> i32 {
    unsafe { OUT.len() as i32 }
}

#[no_mangle]
pub extern "C" fn set_charge(v: f32) {
    unsafe {
        let a = APP.as_mut().unwrap();
        a.world.charge = v;
        if v <= 0.0 {
            a.world.hot = false;
            a.world.phase = game::Phase::Lost;
        }
    }
}

#[no_mangle]
pub extern "C" fn set_wave(w: u32) {
    unsafe {
        let a = APP.as_mut().unwrap();
        a.world.wave = w;
        a.world.queued = game::wave_queue(w);
        a.world.enemies.clear();
        a.world.orbs.clear();
        a.world.intermission = 0;
        a.world.spawn_cd = 0;
        a.world.banner = if w == game::TOTAL_WAVES {
            Some(("WAVE 12 \u{00b7} THE NULL ENGAGED".into(), 60))
        } else {
            None
        };
        a.world.phase = game::Phase::Playing;
    }
}

#[no_mangle]
pub extern "C" fn clear_wave() {
    unsafe {
        let a = APP.as_mut().unwrap();
        a.world.queued.clear();
        a.world.enemies.clear();
        a.world.intermission = 0;
    }
}

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

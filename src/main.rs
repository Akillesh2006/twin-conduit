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
    let report = conduit::checks::run_all();
    print!("{}", report.log);
    if report.passed != report.total {
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
            app.update(&mut input, &mut fx, flags.take().unwrap_or_default());
            acc -= tick;
            steps += 1;
        }

        app.draw(&mut canvas, &fx);
        canvas.flush(&mut console)?;
    }
}

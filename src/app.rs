//! Screen state machine. Platform-free so it can be driven headlessly.

use crate::canvas::Canvas;
use crate::fx::Fx;
use crate::game::{Ev, Phase, World};
use crate::input::{Flags, Input};
use crate::ui::{self, Layout};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Title,
    Run,
    Pause,
    Over,
}

pub struct App {
    pub mode: Mode,
    pub world: World,
    pub layout: Layout,
    pub t: u64,
    pub best: u64,
    pub won: bool,
    over_cd: u32,
}

impl App {
    pub fn new(tw: i32, th: i32) -> Self {
        let layout = ui::compute_layout(tw, th);
        let world = World::new(layout.field());
        Self {
            mode: Mode::Title,
            world,
            layout,
            t: 0,
            best: 0,
            won: false,
            over_cd: 0,
        }
    }

    pub fn restart(&mut self) {
        self.world = World::new(self.layout.field());
        self.won = false;
        self.over_cd = 0;
    }

    pub fn to_title(&mut self) {
        self.mode = Mode::Title;
        self.restart();
    }

    pub fn relayout(&mut self, tw: i32, th: i32) {
        let next = ui::compute_layout(tw, th);
        let changed = next.field_w != self.layout.field_w || next.field_h != self.layout.field_h;
        self.layout = next;
        if changed {
            self.restart();
            self.mode = Mode::Title;
        }
    }

    pub fn update(&mut self, inp: &Input, fx: &mut Fx, f: Flags) {
        self.t += 1;
        match self.mode {
            Mode::Title => {
                if f.confirm || f.restart {
                    self.restart();
                    fx.clear();
                    self.mode = Mode::Run;
                }
            }
            Mode::Run => {
                if f.pause {
                    self.mode = Mode::Pause;
                    return;
                }
                let mut events = Vec::new();
                self.world.step(inp.dirs(), &mut events);
                for e in &events {
                    if matches!(e, Ev::Win) {
                        self.won = true;
                    }
                    fx.push_event(e, self.layout.ox, self.layout.oy);
                }
                if self.world.phase != Phase::Playing {
                    self.best = self.best.max(self.world.score);
                    self.mode = Mode::Over;
                    self.over_cd = 0;
                }
            }
            Mode::Pause => {
                if f.pause || f.confirm {
                    self.mode = Mode::Run;
                }
            }
            Mode::Over => {
                self.over_cd += 1;
                if self.over_cd > 24 && (f.confirm || f.restart) {
                    self.restart();
                    fx.clear();
                    self.mode = Mode::Run;
                }
            }
        }
        fx.step();
    }

    pub fn draw(&self, cv: &mut Canvas, fx: &Fx) {
        cv.clear();
        let l = self.layout;
        if l.term_w < ui::MIN_W || l.term_h < ui::MIN_H {
            ui::draw_too_small(cv, l.term_w, l.term_h);
            return;
        }
        match self.mode {
            Mode::Title => ui::draw_title(cv, &l, self.t),
            Mode::Run | Mode::Pause | Mode::Over => {
                let (sx, sy) = fx.shake(self.world.shake);
                crate::scene::draw_arena(
                    cv,
                    &self.world,
                    fx,
                    l.ox + sx.clamp(-1, 1),
                    l.oy + sy.max(0).min(1),
                    self.t,
                );
                fx.draw(cv, self.world.field, l.ox + sx.clamp(-1, 1), l.oy + sy.max(0).min(1));
                ui::draw_hud(cv, &self.world, &l, self.t);
                match self.mode {
                    Mode::Pause => ui::draw_pause(cv, &l),
                    Mode::Over => ui::draw_gameover(cv, &self.world, &l, self.won),
                    _ => {}
                }
            }
        }
    }
}

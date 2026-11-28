//! Screen state machine. Platform-free so it can be driven headlessly.

use crate::canvas::Canvas;
use crate::fx::Fx;
use crate::game::{Difficulty, Ev, Phase, World};
use crate::input::{Flags, Input};
use crate::scores::{self, ScoreEntry};
use crate::ui::{self, Layout};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Title,
    Run,
    Upgrade,
    Pause,
    Over,
}

pub struct App {
    pub mode: Mode,
    pub difficulty: Difficulty,
    pub world: World,
    pub layout: Layout,
    pub t: u64,
    pub best: u64,
    pub high_scores: Vec<ScoreEntry>,
    pub won: bool,
    pub upgrade_cursor: usize,
    was_focused: bool,
    over_cd: u32,
}

impl App {
    pub fn new(tw: i32, th: i32) -> Self {
        let layout = ui::compute_layout(tw, th);
        let difficulty = Difficulty::Normal;
        let world = World::new_with(layout.field(), difficulty);
        let high_scores = scores::load_scores();
        let best = high_scores.first().map(|e| e.score).unwrap_or(0);
        Self {
            mode: Mode::Title,
            difficulty,
            world,
            layout,
            t: 0,
            best,
            high_scores,
            won: false,
            upgrade_cursor: 0,
            was_focused: true,
            over_cd: 0,
        }
    }

    pub fn restart(&mut self) {
        self.world = World::new_with(self.layout.field(), self.difficulty);
        self.won = false;
        self.upgrade_cursor = 0;
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

    pub fn update(&mut self, inp: &mut Input, fx: &mut Fx, mut f: Flags) {
        self.t += 1;
        let focused = inp.focused();
        if self.was_focused && !focused && self.mode == Mode::Run {
            inp.release_all();
            self.mode = Mode::Pause;
            f = Flags::default();
        }
        self.was_focused = focused;

        match self.mode {
            Mode::Title => {
                if f.nav != 0 {
                    self.difficulty = self.difficulty.cycle(f.nav);
                }
                if let Some(ch) = f.choice {
                    if (1..=4).contains(&ch) {
                        self.difficulty = Difficulty::from_idx((ch - 1) as usize);
                    }
                }
                if f.confirm || f.restart {
                    self.restart();
                    inp.release_all();
                    fx.clear();
                    self.mode = Mode::Run;
                }
            }
            Mode::Run => {
                if f.pause {
                    inp.release_all();
                    self.mode = Mode::Pause;
                    return;
                }
                let dirs = inp.step_dirs();
                let mut events = Vec::new();
                self.world.step(dirs, &mut events);
                for e in &events {
                    if matches!(e, Ev::Win) {
                        self.won = true;
                    }
                    fx.push_event(e, self.layout.ox, self.layout.oy);
                }
                if self.world.phase != Phase::Playing {
                    let entry = scores::make_entry(
                        self.world.score,
                        self.world.wave,
                        self.world.kills,
                        self.world.difficulty,
                        self.won,
                        self.world.elapsed_secs(),
                    );
                    scores::record_entry(&mut self.high_scores, entry);
                    scores::save_scores(&self.high_scores);
                    self.best = self.best.max(self.world.score);
                    self.mode = Mode::Over;
                    self.over_cd = 0;
                } else if self.world.draft_pending {
                    self.upgrade_cursor = 0;
                    self.mode = Mode::Upgrade;
                }
            }
            Mode::Upgrade => {
                if f.nav != 0 {
                    self.upgrade_cursor =
                        (self.upgrade_cursor as i32 + f.nav).rem_euclid(3) as usize;
                }
                let picked = if let Some(ch) = f.choice {
                    if (1..=3).contains(&ch) {
                        Some((ch - 1) as usize)
                    } else {
                        None
                    }
                } else if f.confirm {
                    Some(self.upgrade_cursor)
                } else {
                    None
                };
                if let Some(slot) = picked {
                    let up = self.world.pick_upgrade(slot);
                    inp.release_all();
                    self.world.banner(format!("ONLINE: {}", up.name()), 45);
                    self.mode = Mode::Run;
                }
            }
            Mode::Pause => {
                if f.pause || f.confirm {
                    inp.release_all();
                    self.mode = Mode::Run;
                }
            }
            Mode::Over => {
                self.over_cd += 1;
                if self.over_cd > 24 && (f.confirm || f.restart) {
                    self.restart();
                    inp.release_all();
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
            Mode::Title => ui::draw_title(cv, &l, self.difficulty, &self.high_scores, self.t),
            Mode::Run | Mode::Upgrade | Mode::Pause | Mode::Over => {
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
                    Mode::Upgrade => ui::draw_upgrade(cv, &self.world, &l, self.upgrade_cursor),
                    Mode::Pause => ui::draw_pause(cv, &l),
                    Mode::Over => ui::draw_gameover(cv, &self.world, &l, self.won, self.best),
                    _ => {}
                }
            }
        }
    }
}

//! OVERDRIVE - a two-player, one-keyboard terminal bullet-hell.
//!
//! Both operators carry an automatic cannon. The guns pick their own targets, so
//! every decision the players make is about *movement*: dodge the storm of enemy
//! fire, hold the line, and keep each other alive. Every fifth wave is a boss.
//!
//! The crate has **no dependencies**: the terminal layer talks to the Win32
//! console (or POSIX termios) directly, so it compiles with nothing but `rustc`.

pub mod app;
pub mod canvas;
pub mod core;
pub mod fx;
pub mod game;
pub mod input;
pub mod keys;
pub mod rng;
pub mod scene;
pub mod ui;

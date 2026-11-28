//! CONDUIT - a two-player, one-keyboard terminal game.
//!
//! Two operators are joined by an energy conduit. The conduit is your only weapon
//! and it only burns when the pair is stretched apart.
//!
//! * CLOSE (dist <= REST_DIST) -> conduit cold -> shared charge RECHARGES
//! * FAR (dist >= HOT_DIST)   -> conduit hot  -> anything it touches DIES, charge DRAINS
//! * Beyond SNAP_DIST the conduit snaps.
//!
//! Zero dependencies. Hand-rolled Win32 console & POSIX platform layer.

pub mod app;
pub mod canvas;
pub mod checks;
pub mod core;
pub mod fx;
pub mod game;
pub mod input;
pub mod keys;
pub mod rng;
pub mod scene;
pub mod scores;
pub mod ui;

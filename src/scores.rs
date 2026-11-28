//! Zero-dependency persistent Top-5 high-score table.
//!
//! Stores records locally in `.conduit_scores.txt` on native builds; WASM front ends
//! handle persistence via browser `localStorage` and mirror entries here in memory.

use crate::game::Difficulty;

const SCORE_FILE: &str = ".conduit_scores.txt";
pub const MAX_SCORES: usize = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreEntry {
    pub score: u64,
    pub wave: u32,
    pub kills: u64,
    pub diff: &'static str,
    pub won: bool,
    pub time_secs: u32,
}

/// Insert `entry` into `list`, sort descending by score (then wave), and keep top 5.
pub fn record_entry(list: &mut Vec<ScoreEntry>, entry: ScoreEntry) {
    if entry.score == 0 && !entry.won {
        return;
    }
    list.push(entry);
    list.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.wave.cmp(&a.wave))
            .then_with(|| b.kills.cmp(&a.kills))
    });
    list.truncate(MAX_SCORES);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_scores() -> Vec<ScoreEntry> {
    let Ok(raw) = std::fs::read_to_string(SCORE_FILE) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() != 6 {
            continue;
        }
        let Ok(score) = parts[0].parse::<u64>() else { continue };
        let Ok(wave) = parts[1].parse::<u32>() else { continue };
        let Ok(kills) = parts[2].parse::<u64>() else { continue };
        let diff = match parts[3] {
            "EASY" => "EASY",
            "HARD" => "HARD",
            "NGHT" => "NGHT",
            _ => "NORM",
        };
        let won = parts[4] == "1";
        let time_secs = parts[5].parse::<u32>().unwrap_or(0);
        out.push(ScoreEntry {
            score,
            wave,
            kills,
            diff,
            won,
            time_secs,
        });
    }
    out.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| b.wave.cmp(&a.wave)));
    out.truncate(MAX_SCORES);
    out
}

#[cfg(target_arch = "wasm32")]
pub fn load_scores() -> Vec<ScoreEntry> {
    Vec::new()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_scores(list: &[ScoreEntry]) {
    let mut body = String::new();
    for e in list.iter().take(MAX_SCORES) {
        body.push_str(&format!(
            "{}|{}|{}|{}|{}|{}\n",
            e.score,
            e.wave,
            e.kills,
            e.diff,
            if e.won { 1 } else { 0 },
            e.time_secs
        ));
    }
    let _ = std::fs::write(SCORE_FILE, body);
}

#[cfg(target_arch = "wasm32")]
pub fn save_scores(_list: &[ScoreEntry]) {}

pub fn make_entry(score: u64, wave: u32, kills: u64, diff: Difficulty, won: bool, time_secs: u32) -> ScoreEntry {
    ScoreEntry {
        score,
        wave,
        kills,
        diff: diff.spec().short,
        won,
        time_secs,
    }
}

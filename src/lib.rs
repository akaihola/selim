use crate::score::ScoreNote;
use midly::num::u7;
use std::time::Duration;

pub mod cleanup;
pub mod cmdline;
pub mod device;
#[macro_use]
pub mod score;
pub mod algo01_homophonopedantic;
pub mod playback;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Match {
    pub score_index: usize,
    pub live_index: usize,
    pub stretch_factor: f32,
}

impl Match {
    pub fn new(score_index: usize, live_index: usize, stretch_factor: f32) -> Self {
        Self {
            score_index,
            live_index,
            stretch_factor,
        }
    }
}

/// Finds the next note with given `pitch`, starting from `score[index]`
fn find_next_match_starting_at(score: &[ScoreNote], index: usize, pitch: u7) -> Option<usize> {
    score[index..]
        .iter()
        .position(|note| note.pitch == pitch)
        .map(|i| index + i)
}

/// Calculates the stretch factor from elapsed time in the score and the live performance
///
/// # Arguments
///
/// * elapsed_score - Time elapsed between two notes in the expected score
/// * elapsed_live - Time elapsed between the same notes in the live performance
///
/// # Return value
///
/// The ratio between `elapsed_live` and `elapsed_score`
fn get_stretch_factor(elapsed_score: Duration, elapsed_live: Duration) -> f32 {
    elapsed_live.as_secs_f32() / elapsed_score.as_secs_f32()
}

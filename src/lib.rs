use crate::score::ScoreNote;
use algo01_homophonopedantic::MatchPerScore;
// use algo01_homophonopedantic::MatchPerScore;
use index_vec::{define_index_type, IndexVec};
use midly::num::u7;
use std::{ops::RangeBounds, time::Duration};

pub mod cleanup;
pub mod cmdline;
pub mod device;
#[macro_use]
pub mod score;
pub mod algo01_homophonopedantic;
pub mod algo02_polyphonoflex;
pub mod playback;

define_index_type! { pub struct ScoreNoteIdx = usize; }
pub type ScoreVec = IndexVec<ScoreNoteIdx, ScoreNote>;

define_index_type! { pub struct LiveIdx = usize; }
pub type LiveVec = IndexVec<LiveIdx, ScoreNote>;

define_index_type! { pub struct MatchIdx = usize; }
type MatchVec<T> = IndexVec<MatchIdx, T>;

define_index_type! { pub struct LiveOffsetIdx = usize; }
type LiveOffsetVec = IndexVec<LiveOffsetIdx, LiveIdx>;

pub trait Match {
    fn live_note(&self, live: &LiveVec) -> Result<ScoreNote, &'static str>;
    fn live_time(&self, live: &LiveVec) -> Result<Duration, &'static str>;
    fn live_velocity(&self) -> u7;
    fn stretch_factor(&self) -> f32;
}

pub trait ScoreFollower<M> where M: Match {
    fn follow_score(&mut self, new_live_index: LiveIdx) -> Result<(), &'static str>;
    fn push_live(&mut self, note: ScoreNote);
    fn matches_slice<R>(&self, range: R) -> Vec<MatchPerScore>
    where
        R: RangeBounds<usize>;
    fn match_score_note(&self, m: M) -> Result<ScoreNote, &'static str>;
}

/// Finds the next note with given `pitch`, starting from `score[index]`
fn find_next_match_starting_at(
    score: &ScoreVec,
    index: ScoreNoteIdx,
    pitch: u7,
) -> Option<ScoreNoteIdx> {
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

pub fn stretch(duration: Duration, stretch_factor: f32) -> Duration {
    duration * (1000.0 * stretch_factor) as u32 / 1000
}

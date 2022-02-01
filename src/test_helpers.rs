use std::time::Duration;

#[cfg(test)]
use crate::{
    score::{ScoreNote, ZERO_U7},
    ScoreVec,
};

/// Makes a copy of the score events changing all non-zero velocities to 100
/// and rounding timestamps down to the closest millisecond. This is useful
/// for simplifying tests.
pub fn simplify_score(score: ScoreVec) -> ScoreVec {
    score
        .iter()
        .map(
            |ScoreNote {
                 time,
                 pitch,
                 velocity,
             }| ScoreNote {
                time: Duration::from_millis(time.as_millis() as u64),
                pitch: *pitch,
                velocity: if *velocity == ZERO_U7 {
                    ZERO_U7
                } else {
                    100.into()
                },
            },
        )
        .collect::<ScoreVec>()
}

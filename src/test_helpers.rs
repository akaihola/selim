#[cfg(test)]
use crate::{
    score::{ScoreNote, ZERO_U7},
    ScoreVec,
};

pub fn ignore_vel(score: ScoreVec) -> ScoreVec {
    score
        .iter()
        .map(
            |ScoreNote {
                 time,
                 pitch,
                 velocity,
             }| ScoreNote {
                time: time.clone(),
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

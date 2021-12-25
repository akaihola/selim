#![warn(clippy::all)]

/// A note with a given pitch at a given timestamp in a score or in a live performance
#[derive(Clone, Copy)]
pub struct ScoreNote {
    pub time: u32,
    pub pitch: u8,
}

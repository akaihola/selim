use midly::num::u7;
use crate::score::ScoreNote;

#[macro_use]
pub mod score;
pub mod device;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Match {
    pub score_index: usize,
    pub live_index: usize,
}

impl Match {
    pub fn new(score_index: usize, live_index: usize) -> Self {
        Self {
            score_index,
            live_index,
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

/// Calculates the time difference between notes `score[index1]` and `score[index2]`
fn time_difference(score: &[ScoreNote], index1: usize, index2: usize) -> u64 {
    score[index2].time - score[index1].time
}

/// Finds matches in the score for new notes in the live performance
///
/// # Arguments
///
/// * score - The complete expected musical score with timestamps and pitches
/// * live - The live performance recorded so far, with timestamps and pitches
/// * prev_match_score_index - Index of the last note so far which has been matched
///                            between the live performance and the expected score
/// * new_live_index - Index of the first new note received for the live performance
///                    since the previous round
///
/// # Return value
///
/// A 2-tuple of
/// * newly found matches between the live performance and the expected score
/// * ignored new input notes (as a list of live performance indices)
fn find_new_matches(
    score: &[ScoreNote],
    live: &[ScoreNote],
    prev_match_score_index: Option<usize>,
    new_live_index: usize,
) -> (Vec<Match>, Vec<usize>) {
    let mut score_pointer = match prev_match_score_index {
        Some(i) => i + 1, // continue in the score just after last previous match, or
        None => 0,        // start from beginning of score if nothing matched yet
    };
    let mut matches: Vec<Match> = vec![];
    let mut ignored: Vec<usize> = vec![];
    for (live_index, live_note) in live.iter().enumerate().skip(new_live_index) {
        let matching_index = find_next_match_starting_at(score, score_pointer, live_note.pitch);
        match matching_index {
            Some(score_index) => {
                matches.push(Match::new(score_index, live_index));
                score_pointer = score_index + 1;
            }
            None => ignored.push(live_index),
        };
    }
    (matches, ignored)
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
fn get_stretch_factor(elapsed_score: u64, elapsed_live: u64) -> f32 {
    (elapsed_live as f32) / (elapsed_score as f32)
}

/// Returns the score time in milliseconds corresponding to the latest live note
/// (whether matched or unmatched)
///
/// This can be the exact time index of the latest performed note in the expected score,
/// in case a match was found for that performed note. If no match was found (i.e. it's
/// a wrong or an extra note), an estimate based on the last previous match and the
/// current time stretch factor is returned.
///
/// # Arguments
///
/// * score - The complete expected musical score with timestamps and pitches
/// * live - The live performance recorded so far, with timestamps and pitches
/// * prev_match - The index, in the score and in the live performance, for a matching
///                note.
/// * stretch_factor - The time stretch factor to use
///
/// # Return value
///
/// The estimated current time in the expected score in milliseconds.
fn get_score_time(
    score: &[ScoreNote],
    live: &[ScoreNote],
    prev_match: Option<Match>,
    stretch_factor: f32,
) -> u64 {
    let prev_score_time = score[prev_match.map(|m| m.score_index).unwrap_or(0)].time;
    let elapsed_live = time_difference(
        live,
        prev_match.map(|m| m.live_index).unwrap_or(0),
        live.len() - 1,
    );
    prev_score_time + (elapsed_live as f32 / stretch_factor) as u64
}

/// Matches incoming notes with next notes in the score.
/// This is a super naïve algorithm which
/// * supports only monophony (order of events matters),
/// * ignores unexpected (wrong/extra) notes, and
/// * keeps waiting for the next correct note.
///
/// # Example
///
/// ```text
/// |                        111
/// |        time: 0123456789012
/// |                  v------------ prev_match_score_index == 1 (last matched note)
/// | score index: 0   1 2 3 4 5
/// |       score: C   D E F G A  // expected notes
/// |  live notes:   C D   E   F  // actual played notes
/// |  live index:   0 1   2   3
/// |                  ^   ^   ^----
/// |                  |   `-------- new_live_index == 2 (first newly received note)
/// |                  `------------ prev_match_live_index == 1 (last matched node)
/// ```
///
/// This would return
/// * 8 as the timestamp of the score since that's when the last live note (F) occurs in
///   the score
/// * 2.0 as the time stretch factor since E..F took two time steps in the score but
///   four time steps in the live performance
/// * an empty vector of ignored notes
///                          
/// # Arguments
///
/// * score - The complete expected musical score with timestamps and pitches
/// * live - The live performance recorded so far, with timestamps and pitches
/// * prev_match_score_index - For the last previously matched note between the live
///                            performance and the expected score, this gives the index
///                            of the event in the expected score
/// * prev_match_live_index - For the last previously matched note, this gives the index
///                           of the event in the live performance
/// * new_live_index - Index of the first new note received for the live performance
///                    since the previous call to this function
/// * prev_stretch_factor - The time stretch factor returned by the previous call to
///                         this function
///
/// # Return value
///
/// A 4-tuple of
/// * the timestamp of the score at the last new input note
/// * the time stretch factor at the last new matching input note
/// * for all matched notes, the index in the score and in the live performance
/// * ignored new input notes as a list of live performance indices
pub fn follow_score(
    score: &[ScoreNote],
    live: &[ScoreNote],
    prev_match: Option<Match>,
    new_live_index: usize,
    prev_stretch_factor: f32,
) -> (u64, f32, Vec<Match>, Vec<usize>) {
    let (new_matches, ignored) = find_new_matches(
        score,
        live,
        prev_match.map(|m| m.score_index),
        new_live_index,
    );
    let prev_matches = match prev_match {
        Some(m) => vec![m],
        None => vec![],
    };
    let prev_matches_iter = prev_matches.iter();
    let next_matches_iter = new_matches.iter();
    let matches = prev_matches_iter.chain(next_matches_iter);
    let last_two = matches.rev().take(2).collect::<Vec<&Match>>();
    let stretch_factor = match last_two[..] {
        [last, second_last] => {
            let elapsed_score = time_difference(score, second_last.score_index, last.score_index);
            let elapsed_live = time_difference(live, second_last.live_index, last.live_index);
            get_stretch_factor(elapsed_score, elapsed_live)
        }
        _ => prev_stretch_factor,
    };
    let score_time = get_score_time(score, live, prev_match, stretch_factor);
    (score_time, stretch_factor, new_matches, ignored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_approx_eq::assert_approx_eq;
    use once_cell::sync::Lazy;

    static TEST_SCORE: Lazy<[ScoreNote; 3]> = Lazy::new(|| {
        notes![(1000, 60), (1100, 62), (1200, 64)]
    });

    #[test]
    fn match_the_only_note() {
        let score = notes![(1000, 60)];
        let live = notes![(5, 60)];
        let (time, stretch_factor, new_matches, ignored) =
            follow_score(&score, &live, None, 0, 1.0);
        assert_eq!(time, 1000);
        assert_approx_eq!(stretch_factor, 1.0);
        assert_eq!(new_matches, [Match::new(0, 0)]);
        assert!(ignored.is_empty());
    }

    #[test]
    fn match_first() {
        let live = notes![(5, 60)];
        let (time, stretch_factor, new_matches, ignored) =
            follow_score(&*TEST_SCORE, &live, None, 0, 1.0);
        assert_eq!(time, 1000);
        assert_approx_eq!(stretch_factor, 1.0);
        assert_eq!(new_matches, [Match::new(0, 0)]);
        assert!(ignored.is_empty());
    }

    #[test]
    fn match_second() {
        let live = notes![(5, 60), (55, 62)];
        let (time, stretch_factor, new_matches, ignored) =
            follow_score(&*TEST_SCORE, &live, Some(Match::new(0, 0)), 1, 1.0);
        assert_eq!(time, 1100);
        assert_approx_eq!(stretch_factor, 0.5);
        assert_eq!(new_matches, [Match::new(1, 1)]);
        assert!(ignored.is_empty());
    }

    #[test]
    fn skip_extra_note() {
        let live = notes![(5, 60), (25, 61), (55, 62)];
        let (time, stretch_factor, new_matches, ignored) =
            follow_score(&*TEST_SCORE, &live, Some(Match::new(0, 0)), 1, 1.0);
        assert_eq!(time, 1100);
        assert_approx_eq!(stretch_factor, 0.5);
        assert_eq!(new_matches, [Match::new(1, 2)]);
        assert_eq!(ignored, vec![1]);
    }

    #[test]
    fn skip_missing_note() {
        let live = notes![(5, 60), (55, 64)];
        let (time, stretch_factor, new_matches, ignored) =
            follow_score(&*TEST_SCORE, &live, Some(Match::new(0, 0)), 1, 1.0);
        assert_eq!(time, 1200);
        assert_approx_eq!(stretch_factor, 0.25);
        assert_eq!(new_matches, [Match::new(2, 1)]);
        assert!(ignored.is_empty());
    }

    #[test]
    fn only_wrong_notes() {
        let live = notes![(5, 60), (55, 63), (105, 66)];
        let (time, stretch_factor, new_matches, ignored) =
            follow_score(&*TEST_SCORE, &live, Some(Match::new(0, 0)), 1, 1.0);
        assert_eq!(time, 1100);
        assert_approx_eq!(stretch_factor, 1.0);
        assert!(new_matches.is_empty());
        assert_eq!(ignored, vec![1, 2]);
    }
}

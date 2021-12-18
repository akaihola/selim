/// A note with a given pitch at a given timestamp in a score or in a live performance
#[derive(Clone, Copy)]
pub struct ScoreNote {
    time: u32,
    pitch: u8,
}

fn find_next_match_after(score: &Vec<ScoreNote>, score_index: usize, pitch: u8) -> Option<usize> {
    match score[score_index..]
        .iter()
        .position(|score_note| score_note.pitch == pitch)
    {
        Some(i) => Some(score_index + i),
        None => None,
    }
}

fn time_difference(
    score: &Vec<ScoreNote>,
    index1: Option<usize>,
    index2: Option<usize>,
) -> Option<u32> {
    match (index1, index2) {
        (Some(i1), Some(i2)) => Some(score[i2].time - score[i1].time),
        _ => None,
    }
}

fn get_stretch_factor(
    elapsed_score: Option<u32>,
    elapsed_live: Option<u32>,
    prev_stretch_factor: f32,
) -> f32 {
    match (elapsed_score, elapsed_live) {
        (Some(e_score), Some(e_live)) => (e_live as f32) / (e_score as f32),
        _ => prev_stretch_factor,
    }
}

fn get_next_time(
    score: &Vec<ScoreNote>,
    live: &Vec<ScoreNote>,
    prev_match_score_index: Option<usize>,
    prev_match_live_index: Option<usize>,
    stretch_factor: f32,
) -> u32 {
    let prev_match_score_time = score[prev_match_score_index.unwrap_or(0)].time;
    let elapsed_live = time_difference(
        live,
        prev_match_live_index.or(Some(0)),
        Some(live.len() - 1),
    )
    .unwrap() as f32;
    prev_match_score_time + (elapsed_live / stretch_factor) as u32
}

/// Matches incoming notes with next notes in the score.
/// This is a super naïve algorithm which
/// * supports only monophony (order of events matters),
/// * ignores unexpected (wrong/extra) notes, and
/// * keeps waiting for the next correct note.
///
/// # Example
///
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
/// A 3-tuple of
/// * the timestamp of the score at the last new input note
/// * the time stretch factor at the last new matching input note
/// * the index of the last matched note in the score
/// * the index of the last matched note in the live performance
/// * ignored new input notes as a list of live performance indices
pub fn follow_score(
    score: Vec<ScoreNote>,
    live: Vec<ScoreNote>,
    prev_match_score_index: Option<usize>,
    prev_match_live_index: Option<usize>,
    new_live_index: usize,
    prev_stretch_factor: f32,
) -> (u32, f32, Option<usize>, Option<usize>, Vec<usize>) {
    let mut score_index = match prev_match_score_index {
        None => 0,        // start from beginning of score if nothing matched yet
        Some(i) => i + 1, // continue in the score just after last previous match
    };
    let (mut next_match_score_index, mut next_match_live_index) = (None, None);
    let mut ignored: Vec<usize> = vec![];
    for (live_index, live_note) in live.iter().enumerate().skip(new_live_index) {
        let matching_index = find_next_match_after(&score, score_index, live_note.pitch);
        match matching_index {
            Some(i) => {
                next_match_live_index = Some(live_index);
                next_match_score_index = Some(i);
                score_index = i + 1;
            }
            None => ignored.push(live_index),
        };
    }

    let elapsed_score = time_difference(&score, prev_match_score_index, next_match_score_index);
    let elapsed_live = time_difference(&live, prev_match_live_index, next_match_live_index);
    let next_stretch_factor = get_stretch_factor(elapsed_score, elapsed_live, prev_stretch_factor);
    let next_time = get_next_time(
        &score,
        &live,
        prev_match_score_index,
        prev_match_live_index,
        next_stretch_factor,
    );
    (
        next_time,
        next_stretch_factor,
        next_match_score_index,
        next_match_live_index,
        ignored,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! note {
        ( $t:expr, $p:expr ) => {
            ScoreNote {
                time: $t,
                pitch: $p,
            }
        };
    }

    const TEST_SCORE: [ScoreNote; 3] = [note!(1000, 60), note!(1100, 62), note!(1200, 64)];

    #[test]
    fn match_the_only_note() {
        let score = [note!(1000, 60)];
        let live = [note!(5, 60)];
        let (time, stretch_factor, last_match_score, last_match_live, ignored) =
            follow_score(score.to_vec(), live.to_vec(), None, None, 0, 1.0);
        assert_eq!(time, 1000);
        assert_eq!(stretch_factor, 1.0);
        assert_eq!(last_match_score, Some(0));
        assert_eq!(last_match_live, Some(0));
        assert_eq!(ignored.is_empty(), true);
    }

    #[test]
    fn match_first() {
        let live = [note!(5, 60)];
        let (time, stretch_factor, last_match_score, last_match_live, ignored) =
            follow_score(TEST_SCORE.to_vec(), live.to_vec(), None, None, 0, 1.0);
        assert_eq!(time, 1000);
        assert_eq!(stretch_factor, 1.0);
        assert_eq!(last_match_score, Some(0));
        assert_eq!(last_match_live, Some(0));
        assert_eq!(ignored.is_empty(), true);
    }

    #[test]
    fn match_second() {
        let live = [note!(5, 60), note!(55, 62)];
        let (time, stretch_factor, last_match_score, last_match_live, ignored) =
            follow_score(TEST_SCORE.to_vec(), live.to_vec(), Some(0), Some(0), 1, 1.0);
        assert_eq!(time, 1100);
        assert_eq!(stretch_factor, 0.5);
        assert_eq!(last_match_score, Some(1));
        assert_eq!(last_match_live, Some(1));
        assert_eq!(ignored.is_empty(), true);
    }

    #[test]
    fn skip_extra_note() {
        let live = [note!(5, 60), note!(25, 61), note!(55, 62)];
        let (time, stretch_factor, last_match_score, last_match_live, ignored) =
            follow_score(TEST_SCORE.to_vec(), live.to_vec(), Some(0), Some(0), 1, 1.0);
        assert_eq!(time, 1100);
        assert_eq!(stretch_factor, 0.5);
        assert_eq!(last_match_score, Some(1));
        assert_eq!(last_match_live, Some(2));
        assert_eq!(ignored, vec![1]);
    }

    #[test]
    fn skip_missing_note() {
        let live = [note!(5, 60), note!(55, 64)];
        let (time, stretch_factor, last_match_score, last_match_live, ignored) =
            follow_score(TEST_SCORE.to_vec(), live.to_vec(), Some(0), Some(0), 1, 1.0);
        assert_eq!(time, 1200);
        assert_eq!(stretch_factor, 0.25);
        assert_eq!(last_match_score, Some(2));
        assert_eq!(last_match_live, Some(1));
        assert_eq!(ignored.is_empty(), true);
    }
}

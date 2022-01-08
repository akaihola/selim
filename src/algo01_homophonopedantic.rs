use std::time::Duration;

use crate::{find_next_match_starting_at, get_stretch_factor, score::ScoreNote, Match};

pub trait ScoreFollower {
    fn follow_score(&mut self, new_live_index: usize);
    fn last_match(&self) -> Option<&Match>;
    fn push_live(&mut self, note: ScoreNote);
}

pub struct HomophonoPedantic<'a> {
    score: &'a [ScoreNote],
    pub live: Vec<ScoreNote>,
    pub matches: Vec<Match>,
    pub ignored: Vec<usize>,
}

impl<'a> HomophonoPedantic<'a> {
    pub fn new(score: &'a [ScoreNote]) -> Self {
        Self {
            score,
            live: vec![],
            matches: vec![],
            ignored: vec![],
        }
    }
}

impl<'a> ScoreFollower for HomophonoPedantic<'a> {
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
    fn follow_score(&mut self, new_live_index: usize) {
        let (new_matches, ignored) = self.find_new_matches(new_live_index);
        self.matches.extend(new_matches);
        self.ignored.extend(ignored);
    }

    fn last_match(&self) -> Option<&Match> {
        self.matches.last()
    }

    fn push_live(&mut self, note: ScoreNote) {
        self.live.push(note);
    }
}

impl HomophonoPedantic<'_> {
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
    fn find_new_matches(&self, new_live_index: usize) -> (Vec<Match>, Vec<usize>) {
        let mut score_pointer = match self.last_match().cloned() {
            Some(i) => i.score_index + 1, // continue in the score just after last previous match, or
            None => 0,                    // start from beginning of score if nothing matched yet
        };
        let mut matches: Vec<Match> = vec![];
        let mut ignored: Vec<usize> = vec![];
        for (live_index, live_note) in self.live.iter().enumerate().skip(new_live_index) {
            let matching_index =
                find_next_match_starting_at(self.score, score_pointer, live_note.pitch);
            match matching_index {
                Some(score_index) => {
                    let stretch_factor =
                        self.get_stretch_factor_at_new_match(score_index, live_note.time);
                    let new_match = Match::new(
                        score_index,
                        live_index,
                        stretch_factor,
                        self.score[score_index].velocity.into(),
                        self.live[live_index].velocity.into(),
                    );
                    matches.push(new_match);
                    score_pointer = score_index + 1;
                }
                None => ignored.push(live_index),
            };
        }
        (matches, ignored)
    }

    fn get_stretch_factor_at_new_match(
        &self,
        new_match_score_index: usize,
        new_match_in_live_time: Duration,
    ) -> f32 {
        match self.last_match().cloned() {
            Some(Match {
                score_index: prev_match_score_index,
                live_index,
                stretch_factor: _,
                score_velocity: _,
                live_velocity: _,
            }) => {
                let new_match_in_score = self.score[new_match_score_index];
                let prev_match_in_score = self.score[prev_match_score_index];
                let prev_match_in_live = self.live[live_index];
                get_stretch_factor(
                    new_match_in_score.time - prev_match_in_score.time,
                    new_match_in_live_time - prev_match_in_live.time,
                )
            }
            None => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use midly::num::u7;
    use once_cell::sync::Lazy;

    static TEST_SCORE: Lazy<[ScoreNote; 3]> =
        Lazy::new(|| notes![(1000, 60), (1100, 62), (1200, 64)]);

    #[test]
    fn match_the_only_note() {
        let score = notes![(1000, 60)];
        let mut follower = HomophonoPedantic::new(&score);
        follower.live.extend(notes![(5, 60)]);
        follower.follow_score(0);
        assert_eq!(follower.matches, [Match::new(0, 0, 1.0, 100, 100)]);
        assert!(follower.ignored.is_empty());
    }

    #[test]
    fn match_first() {
        let mut follower = HomophonoPedantic::new(&*TEST_SCORE);
        follower.live.extend(notes![(5, 60)]);
        follower.follow_score(0);
        assert_eq!(follower.matches, [Match::new(0, 0, 1.0, 100, 100)]);
        assert!(follower.ignored.is_empty());
    }

    #[test]
    fn match_second() {
        let mut follower = HomophonoPedantic::new(&*TEST_SCORE);
        follower.live.extend(notes![(5, 60), (55, 62)]);
        follower.matches.push(Match::new(0, 0, 1.0, 100, 100));
        follower.follow_score(1);
        assert_eq!(follower.matches[1..], [Match::new(1, 1, 0.5, 100, 100)]);
        assert!(follower.ignored.is_empty());
    }

    #[test]
    fn skip_extra_note() {
        let mut follower = HomophonoPedantic::new(&*TEST_SCORE);
        follower.live.extend(notes![(5, 60), (25, 61), (55, 62)]);
        follower.matches.push(Match::new(0, 0, 1.0, 100, 100));
        follower.follow_score(1);
        assert_eq!(follower.matches[1..], [Match::new(1, 2, 0.5, 100, 100)]);
        assert_eq!(follower.ignored, vec![1]);
    }

    #[test]
    fn skip_missing_note() {
        let mut follower = HomophonoPedantic::new(&*TEST_SCORE);
        follower.live.extend(notes![(5, 60), (55, 64)]);
        follower.matches.push(Match::new(0, 0, 1.0, 100, 100));
        follower.follow_score(1);
        assert_eq!(follower.matches[1..], [Match::new(2, 1, 0.25, 100, 100)]);
        assert!(follower.ignored.is_empty());
    }

    #[test]
    fn only_wrong_notes() {
        let mut follower = HomophonoPedantic::new(&*TEST_SCORE);
        follower.live.extend(notes![(5, 60), (55, 63), (105, 66)]);
        follower.matches.push(Match::new(0, 0, 1.0, 100, 100));
        follower.follow_score(1);
        assert!(follower.matches[1..].is_empty());
        assert_eq!(follower.ignored, vec![1, 2]);
    }
}

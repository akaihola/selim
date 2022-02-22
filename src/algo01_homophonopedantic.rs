use crate::{
    find_next_match_starting_at, get_stretch_factor, score::ScoreNote, LiveIdx, LiveOffsetVec,
    LiveVec, Match, MatchVec, ScoreFollower, ScoreNoteIdx, ScoreVec,
};
use anyhow::{bail, Result};
use index_vec::index_vec;
use midly::num::u7;
use std::{ops::RangeBounds, time::Duration};

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct MatchPerScore {
    score_index: ScoreNoteIdx,
    live_index: LiveIdx,
    stretch_factor: f32,
    score_velocity: u7,
    live_velocity: u7,
}

impl MatchPerScore {
    pub fn new(
        score_index: ScoreNoteIdx,
        live_index: LiveIdx,
        stretch_factor: f32,
        score_velocity: u8,
        live_velocity: u8,
    ) -> Self {
        Self {
            score_index,
            live_index,
            stretch_factor,
            score_velocity: score_velocity.into(),
            live_velocity: live_velocity.into(),
        }
    }

    pub fn score_index(&self) -> ScoreNoteIdx {
        self.score_index
    }

    pub fn score_note(&self, score: &ScoreVec) -> Result<ScoreNote> {
        if let Some(score_note) = score.get(self.score_index()) {
            Ok(*score_note)
        } else {
            bail!("Match points beyond list of score events")
        }
    }

    pub fn score_time(&self, score: &ScoreVec) -> Result<Duration> {
        Ok(self.score_note(score)?.time)
    }

    pub fn live_index(&self) -> LiveIdx {
        self.live_index
    }

    pub fn live_pitch(&self, live: &LiveVec) -> Result<u7> {
        Ok(self.live_note(live)?.pitch)
    }

    pub fn score_velocity(&self) -> u7 {
        self.score_velocity
    }
}

impl Match for MatchPerScore {
    fn live_note(&self, live: &LiveVec) -> Result<ScoreNote> {
        if let Some(live_note) = live.get(self.live_index) {
            Ok(*live_note)
        } else {
            bail!("Match points beyond list of live events")
        }
    }

    fn live_time(&self, live: &LiveVec) -> Result<Duration> {
        Ok(self.live_note(live)?.time)
    }

    fn live_velocity(&self) -> u7 {
        self.live_velocity
    }

    fn stretch_factor(&self) -> f32 {
        self.stretch_factor
    }
}

pub struct HomophonoPedantic {
    score: ScoreVec,
    pub live: LiveVec,
    pub matches: MatchVec<MatchPerScore>,
    pub ignored: LiveOffsetVec,
}

impl HomophonoPedantic {
    pub fn new(score: ScoreVec) -> Self {
        Self {
            score,
            live: index_vec![],
            matches: index_vec![],
            ignored: index_vec![],
        }
    }
}

impl ScoreFollower<MatchPerScore> for HomophonoPedantic {
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
    fn follow_score(&mut self, new_live_index: LiveIdx) -> Result<()> {
        let (new_matches, ignored) = self.find_new_matches(new_live_index);
        self.matches.extend(new_matches);
        self.ignored.extend(ignored);
        Ok(())
    }

    fn push_live(&mut self, note: ScoreNote) {
        self.live.push(note);
    }

    fn matches_slice<R>(&self, range: R) -> Vec<MatchPerScore>
    where
        R: RangeBounds<usize>,
    {
        // // Once `#![feature(slice_index_methods)]` is in Rust stable, we can do something like this instead:
        // use std::ops::slice::SliceIndex;
        // let slice = (range.start_bound().cloned(), range.end_bound().cloned())
        //     .index(self.matches.as_raw_slice());
        // slice.to_vec()
        let slice = self.matches.iter().enumerate().filter_map(|(idx, &item)| {
            if range.contains(&idx) {
                Some(item)
            } else {
                None
            }
        });
        slice.collect::<Vec<MatchPerScore>>()
    }

    fn match_score_note(&self, m: MatchPerScore) -> Result<ScoreNote> {
        m.score_note(&self.score)
    }
}

impl HomophonoPedantic {
    fn last_match(&self) -> Option<MatchPerScore> {
        self.matches.last().cloned()
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
        &self,
        new_live_index: LiveIdx,
    ) -> (MatchVec<MatchPerScore>, LiveOffsetVec) {
        let mut score_pointer = match self.last_match() {
            Some(i) => i.score_index() + 1, // continue in the score just after last previous
            None => 0.into(), // match, or start from beginning of score if nothing matched yet
        };
        let mut matches: MatchVec<MatchPerScore> = index_vec![];
        let mut ignored: LiveOffsetVec = index_vec![];
        for (i, live_note) in self.live.iter().enumerate().skip(new_live_index.into()) {
            let live_index = LiveIdx::from(i);
            let matching_index =
                find_next_match_starting_at(&self.score, score_pointer, live_note.pitch);
            match matching_index {
                Some(score_index) => {
                    let stretch_factor =
                        self.get_stretch_factor_at_new_match(score_index, live_note.time);
                    let new_match = MatchPerScore::new(
                        score_index,
                        live_index,
                        stretch_factor,
                        self.score[score_index].velocity.into(),
                        self.live[live_index].velocity.into(),
                    );
                    matches.push(new_match);
                    score_pointer = score_index + 1;
                }
                None => {
                    ignored.push(live_index);
                }
            };
        }
        (matches, ignored)
    }

    fn get_stretch_factor_at_new_match(
        &self,
        new_match_score_index: ScoreNoteIdx,
        new_match_in_live_time: Duration,
    ) -> f32 {
        match self.last_match() {
            Some(MatchPerScore {
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

    fn test_score() -> ScoreVec {
        notes![(1000, 60), (1100, 62), (1200, 64)]
    }

    #[test]
    fn match_the_only_note() {
        let score = notes![(1000, 60)];
        let mut follower = HomophonoPedantic::new(score);
        follower.live.extend::<LiveVec>(notes![(5, 60)]);
        follower.follow_score(0.into()).unwrap();
        assert_eq!(
            follower.matches,
            [MatchPerScore::new(0.into(), 0.into(), 1.0, 100, 100)]
        );
        assert!(follower.ignored.is_empty());
    }

    #[test]
    fn match_first() {
        let mut follower = HomophonoPedantic::new(test_score());
        follower.live.extend::<LiveVec>(notes![(5, 60)]);
        follower.follow_score(0.into()).unwrap();
        assert_eq!(
            follower.matches,
            [MatchPerScore::new(0.into(), 0.into(), 1.0, 100, 100)]
        );
        assert!(follower.ignored.is_empty());
    }

    #[test]
    fn match_second() {
        let mut follower = HomophonoPedantic::new(test_score());
        follower.live.extend::<LiveVec>(notes![(5, 60), (55, 62)]);
        follower
            .matches
            .push(MatchPerScore::new(0.into(), 0.into(), 1.0, 100, 100));
        follower.follow_score(1.into()).unwrap();
        assert_eq!(
            follower.matches[1.into()..],
            [MatchPerScore::new(1.into(), 1.into(), 0.5, 100, 100)]
        );
        assert!(follower.ignored.is_empty());
    }

    #[test]
    fn skip_extra_note() {
        let mut follower = HomophonoPedantic::new(test_score());
        follower
            .live
            .extend::<LiveVec>(notes![(5, 60), (25, 61), (55, 62)]);
        follower
            .matches
            .push(MatchPerScore::new(0.into(), 0.into(), 1.0, 100, 100));
        follower.follow_score(1.into()).unwrap();
        assert_eq!(
            follower.matches[1.into()..],
            [MatchPerScore::new(1.into(), 2.into(), 0.5, 100, 100)]
        );
        assert_eq!(follower.ignored, vec![1]);
    }

    #[test]
    fn skip_missing_note() {
        let mut follower = HomophonoPedantic::new(test_score());
        follower.live.extend::<LiveVec>(notes![(5, 60), (55, 64)]);
        follower
            .matches
            .push(MatchPerScore::new(0.into(), 0.into(), 1.0, 100, 100));
        follower.follow_score(1.into()).unwrap();
        assert_eq!(
            follower.matches[1.into()..],
            [MatchPerScore::new(2.into(), 1.into(), 0.25, 100, 100)]
        );
        assert!(follower.ignored.is_empty());
    }

    #[test]
    fn only_wrong_notes() {
        let mut follower = HomophonoPedantic::new(test_score());
        follower
            .live
            .extend::<LiveVec>(notes![(5, 60), (55, 63), (105, 66)]);
        follower
            .matches
            .push(MatchPerScore::new(0.into(), 0.into(), 1.0, 100, 100));
        follower.follow_score(1.into()).unwrap();
        assert!(follower.matches[1.into()..].is_empty());
        assert_eq!(follower.ignored, vec![1, 2]);
    }
}

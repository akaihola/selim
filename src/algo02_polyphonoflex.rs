use crate::{
    algo01_homophonopedantic::MatchPerScore,
    get_stretch_factor,
    score::{pitch_to_name, ScoreNote},
    stretch, LiveIdx, LiveOffsetVec, LiveVec, Match, MatchIdx, MatchVec, ScoreFollower,
    ScoreNoteIdx, ScoreVec,
};
use index_vec::{define_index_type, index_vec, IndexVec};
use midly::num::u7;
use std::{iter::repeat, ops::RangeBounds, time::Duration};

define_index_type! {
    pub struct PitchIdx = u8;
    MAX_INDEX = 128;
    IMPL_RAW_CONVERSIONS = true;
}

define_index_type! { pub struct ScoreOffsetIdx = usize; }
type ScoreOffsetVec = IndexVec<ScoreOffsetIdx, ScoreNoteIdx>;
define_index_type! { pub struct MatchOffsetIdx = usize; }
type MatchOffsetVec = IndexVec<MatchOffsetIdx, MatchIdx>;
type MatchOffsetByPitchVec = IndexVec<PitchIdx, MatchOffsetVec>;
type ScoreByPitchVec = IndexVec<PitchIdx, ScoreOffsetVec>;

define_index_type! { pub struct PAMOIdx = usize; }
type PitchesAndMatchOffsets = IndexVec<PAMOIdx, (PitchIdx, MatchIdx)>;

pub struct PolyphonoFlex<'a> {
    score: &'a ScoreVec,
    score_offsets_by_pitch: ScoreByPitchVec,
    pub live: LiveVec,
    pub matches: MatchVec<MatchPerPitch>,
    match_offsets_by_pitch: MatchOffsetByPitchVec,
    pub ignored: LiveOffsetVec,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchPerPitch {
    score_per_pitch_index: ScoreOffsetIdx,
    live_index: LiveIdx,
    stretch_factor: f32,
    score_velocity: u7,
    live_velocity: u7,
}

impl MatchPerPitch {
    fn new(
        score_per_pitch_index: ScoreOffsetIdx,
        live_index: LiveIdx,
        stretch_factor: f32,
        score_velocity: u8,
        live_velocity: u8,
    ) -> Self {
        Self {
            score_per_pitch_index,
            live_index,
            stretch_factor,
            score_velocity: score_velocity.into(),
            live_velocity: live_velocity.into(),
        }
    }

    fn score_per_pitch_index(&self) -> ScoreOffsetIdx {
        self.score_per_pitch_index
    }

    fn to_match_per_score(
        self,
        score_offsets_by_pitch: &ScoreByPitchVec,
        live: &LiveVec,
    ) -> MatchPerScore {
        let pitch: PitchIdx = PitchIdx::from(live[self.live_index].pitch.as_int());
        let score_per_pitch: &ScoreOffsetVec = &score_offsets_by_pitch[pitch];
        let score_index: ScoreNoteIdx = score_per_pitch[self.score_per_pitch_index];
        MatchPerScore::new(
            score_index,
            self.live_index,
            self.stretch_factor,
            self.score_velocity.into(),
            self.live_velocity.into(),
        )
    }
}

impl Match for MatchPerPitch {
    fn live_note(&self, live: &LiveVec) -> Result<ScoreNote, &'static str> {
        if let Some(live_note) = live.get(self.live_index) {
            Ok(*live_note)
        } else {
            Err("Match points beyond list of live events")
        }
    }

    fn live_time(&self, live: &LiveVec) -> Result<Duration, &'static str> {
        Ok(self.live_note(live)?.time)
    }

    fn live_velocity(&self) -> u7 {
        self.live_velocity
    }

    fn stretch_factor(&self) -> f32 {
        self.stretch_factor
    }
}

fn score_by_pitch(score: &ScoreVec) -> ScoreByPitchVec {
    let mut vecs = repeat(ScoreOffsetVec::new())
        .take(128)
        .collect::<ScoreByPitchVec>();
    for (i, note) in score.iter().enumerate() {
        let score_index = i.into();
        vecs[PitchIdx::from(note.pitch.as_int())].push(score_index);
    }
    vecs
}

impl<'a> PolyphonoFlex<'a> {
    pub fn new(score: &'a ScoreVec) -> Self {
        Self {
            score,
            score_offsets_by_pitch: score_by_pitch(score),
            live: index_vec![],
            matches: index_vec![],
            match_offsets_by_pitch: repeat(MatchOffsetVec::new())
                .take(128)
                .collect::<MatchOffsetByPitchVec>(),
            ignored: index_vec![],
        }
    }
}

impl<'a> ScoreFollower<MatchPerPitch> for PolyphonoFlex<'a> {
    /// Matches incoming notes with next notes in the score.
    /// This is a slightly improved algorithm which
    /// * supports polyphony
    /// * finds the best match for unexpected (wrong/extra) notes
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
    fn follow_score(&mut self, new_live_index: LiveIdx) -> Result<(), &'static str> {
        let (matches, ignored, match_offsets_by_pitch) = self.find_new_matches(new_live_index)?;
        let matches_offset = MatchIdx::from(self.matches.len());
        self.matches.extend(matches);
        self.ignored.extend(ignored);
        for (pitch, i) in match_offsets_by_pitch {
            self.match_offsets_by_pitch[pitch].push(matches_offset + i);
        }
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
        // slice
        //     .iter()
        //     .map(|m| {
        //         m.to_match_per_score(&self.score_offsets_by_pitch, &self.live)
        //             .to_owned()
        //     })
        //     .collect::<Vec<_>>()
        // slice.to_vec()
        let slice = self.matches.iter().enumerate().filter_map(|(idx, &item)| {
            if range.contains(&idx) {
                Some(
                    item.to_match_per_score(&self.score_offsets_by_pitch, &self.live), // .to_owned(), // is this needed?
                )
            } else {
                None
            }
        });
        slice.collect::<Vec<MatchPerScore>>()
    }

    fn match_score_note(&self, m: MatchPerPitch) -> Result<ScoreNote, &'static str> {
        m.to_match_per_score(&self.score_offsets_by_pitch, &self.live)
            .score_note(self.score)
    }
}

impl<'a> PolyphonoFlex<'a> {
    fn last_per_pitch_match(&self) -> Option<&MatchPerPitch> {
        self.matches.last()
    }

    pub fn last_match(&self) -> Option<MatchPerScore> {
        let match_per_pitch = self.last_per_pitch_match()?;
        let match_per_score =
            match_per_pitch.to_match_per_score(&self.score_offsets_by_pitch, &self.live);
        Some(match_per_score)
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
    ) -> Result<
        (
            MatchVec<MatchPerPitch>,
            LiveOffsetVec,
            PitchesAndMatchOffsets,
        ),
        &'static str,
    > {
        let live = &self.live;
        let mut matches: MatchVec<MatchPerPitch> = index_vec![];
        let mut ignored = index_vec![];
        let mut match_offsets_by_pitch: PitchesAndMatchOffsets = index_vec![];
        for (
            i,
            ScoreNote {
                time: live_time,
                pitch,
                velocity: live_velocity,
            },
        ) in live.iter().enumerate().skip(new_live_index.into())
        {
            let live_index = LiveIdx::from(i);
            eprintln!(
                "find_new_matches {:.3} {}(@live:{}) v{}",
                live_time.as_secs_f32(),
                pitch_to_name(*pitch),
                usize::from(live_index),
                live_velocity
            );
            if let Some(new_match) = self
                .find_new_match(*pitch, live_index, *live_time, *live_velocity)
                .unwrap()
            {
                let pitch = PitchIdx::from(self.get_match_pitch(new_match)?.as_int());
                match_offsets_by_pitch.push((pitch, matches.len().into()));
                matches.push(new_match);
            } else {
                ignored.push(live_index);
            }
        }
        Ok((matches, ignored, match_offsets_by_pitch))
    }

    fn find_new_match(
        &self,
        pitch: u7,
        live_index: LiveIdx,
        live_time: Duration,
        live_velocity: u7,
    ) -> Result<Option<MatchPerPitch>, &str> {
        let next_unmatched_offset_for_pitch = self.get_next_unmatched_offset_for_pitch(pitch);
        let pitch = pitch.as_int() as usize;
        let score_for_pitch = &self.score_offsets_by_pitch[pitch];
        let live_time_mapped = self.live_time_mapped(live_time)?;
        let mut min_time_diff = Duration::from_secs(9999);
        let mut best_match_pitch_score_index = None;
        let mut debug_output = None;
        let mut prev_debug_output;
        for (i, &score_note_offset) in score_for_pitch[next_unmatched_offset_for_pitch..]
            .iter()
            .enumerate()
        {
            let live_note_offset = next_unmatched_offset_for_pitch + i;
            let score_note = self.score[score_note_offset];
            let time_diff = absolute_time_difference(score_note.time, live_time_mapped);
            prev_debug_output = debug_output.clone();
            debug_output = Some(format!(
                "|{:?}(@score:{}) - {:?}(@live:{:?})| = {:?}",
                score_note.time,
                usize::from(score_note_offset),
                live_time_mapped,
                live_note_offset,
                time_diff
            ));
            if time_diff < min_time_diff {
                best_match_pitch_score_index = Some(live_note_offset);
                min_time_diff = time_diff;
                if let Some(s) = prev_debug_output {
                    eprintln!("  {}", s)
                }
            } else {
                if let Some(s) = prev_debug_output {
                    eprintln!("* {}", s)
                }
                eprintln!("  {}", debug_output.unwrap());
                break;
            }
        }
        if let Some(index) = best_match_pitch_score_index {
            let best_match_score_offset = score_for_pitch[index];
            let best_match_score_note = self.score[best_match_score_offset];
            let stretch_factor =
                self.get_stretch_factor_at_new_match(best_match_score_note, live_time)?;
            Ok(Some(MatchPerPitch::new(
                index,
                live_index,
                stretch_factor,
                best_match_score_note.velocity.into(),
                live_velocity.into(),
            )))
        } else {
            Ok(None)
        }
    }

    fn live_time_mapped(&self, live_time: Duration) -> Result<Duration, &str> {
        Ok(if let Some(last_match) = self.last_per_pitch_match() {
            stretch(
                live_time - last_match.live_time(&self.live)?,
                1.0 / last_match.stretch_factor(),
            )
        } else {
            Duration::ZERO
        })
    }

    fn get_next_unmatched_offset_for_pitch(&self, pitch: u7) -> ScoreOffsetIdx {
        let pitch: PitchIdx = pitch.as_int().into();
        let match_offsets_for_pitch: &MatchOffsetVec = &self.match_offsets_by_pitch[pitch];
        if let Some(match_index) = match_offsets_for_pitch.last() {
            let last_match_for_pitch = &self.matches[*match_index];
            last_match_for_pitch.score_per_pitch_index + 1 // this points into score_for_pitch
        } else {
            0.into() // this is ok even if pitch has no notes
        }
    }

    fn get_match_pitch(&self, new_match: MatchPerPitch) -> Result<u7, &'static str> {
        Ok(new_match.live_note(&self.live)?.pitch)
    }

    fn get_stretch_factor_at_new_match(
        &self,
        new_match_in_score: ScoreNote,
        new_match_in_live_time: Duration,
    ) -> Result<f32, &'static str> {
        match self.last_per_pitch_match() {
            Some::<&MatchPerPitch>(last_match) => {
                let prev_match_in_live = last_match.live_note(&self.live)?;
                let pitch_idx = usize::from(prev_match_in_live.pitch.as_int());
                // let match_offsets = &self.match_offsets_by_pitch[pitch_idx];
                let score_offset_idx: ScoreOffsetIdx = last_match.score_per_pitch_index();
                let score_for_pitch = &self.score_offsets_by_pitch[pitch_idx];
                let score_note_idx = score_for_pitch[score_offset_idx];
                let prev_match_in_score: ScoreNote = self.score[score_note_idx];
                let stretch_factor = get_stretch_factor(
                    new_match_in_score.time - prev_match_in_score.time,
                    new_match_in_live_time - prev_match_in_live.time,
                );
                eprintln!(
                    "get_stretch_factor({:.3} - {:.3} = {:.3}, {:.3} - {:.3} = {:.3}) = {:.0}%",
                    new_match_in_score.time.as_secs_f32(),
                    prev_match_in_score.time.as_secs_f32(),
                    (new_match_in_score.time - prev_match_in_score.time).as_secs_f32(),
                    new_match_in_live_time.as_secs_f32(),
                    prev_match_in_live.time.as_secs_f32(),
                    (new_match_in_live_time - prev_match_in_live.time).as_secs_f32(),
                    100.0 * stretch_factor,
                );
                Ok(stretch_factor)
            }
            None => Ok(1.0),
        }
    }
}

fn absolute_time_difference(t1: Duration, t2: Duration) -> Duration {
    if t2 < t1 {
        t1 - t2
    } else {
        t2 - t1
    }
}

#[cfg(test)]
mod tests {
    use crate::ScoreVec;

    use super::*;
    use midly::num::u7;

    fn test_score() -> ScoreVec {
        notes![(1000, 60), (1100, 62), (1200, 64)]
    }

    fn make_follower<'a>(
        score: &'a ScoreVec,
        live: LiveVec,
        matches: &'a [(usize, usize, u8)],
    ) -> PolyphonoFlex<'a> {
        let mut follower = PolyphonoFlex::new(score);
        follower.live.extend::<LiveVec>(live);
        for (score_per_pitch_index, live_index, pitch) in matches {
            follower.matches.push(MatchPerPitch::new(
                (*score_per_pitch_index).into(),
                (*live_index).into(),
                1.0,
                127,
                127,
            ));
            follower.match_offsets_by_pitch[PitchIdx::from(*pitch)]
                .push((*score_per_pitch_index).into());
        }
        follower
    }

    #[test]
    fn find_new_matches_the_only_note() {
        let score = &notes![(1000, 60)];
        let follower = make_follower(score, notes![(5, 60)], &[]);
        let (matches, ignored, match_offsets_by_pitch) =
            follower.find_new_matches(0.into()).unwrap();
        assert_eq!(
            matches,
            index_vec![MatchPerPitch::new(0.into(), 0.into(), 1.0, 127, 127)]
        );
        assert_eq!(
            match_offsets_by_pitch,
            index_vec![(PitchIdx::from(60u8), MatchIdx::from(0))]
        );
        assert!(ignored.is_empty());
    }

    #[test]
    fn follow_score_the_only_note() {
        let score = &notes![(1000, 60)];
        let mut follower = make_follower(score, notes![(5, 60)], &[]);
        follower.follow_score(0.into()).unwrap();
        assert_eq!(
            follower.matches,
            index_vec![MatchPerPitch::new(0.into(), 0.into(), 1.0, 127, 127)]
        );
        assert_eq!(follower.match_offsets_by_pitch[60], [0]);
        assert!(follower.ignored.is_empty());
    }

    #[test]
    fn match_first() {
        let score = &test_score();
        let mut follower = make_follower(score, notes![(5, 60)], &[]);
        follower.follow_score(0.into()).unwrap();
        assert_eq!(
            follower.matches,
            [MatchPerPitch::new(0.into(), 0.into(), 1.0, 127, 127)]
        );
        assert_eq!(follower.match_offsets_by_pitch[60], [0]);
        assert!(follower.ignored.is_empty());
    }

    #[test]
    fn match_second() {
        let score = &test_score();
        let mut follower = make_follower(score, notes![(5, 60), (55, 62)], &[(0, 0, 60)]);
        follower.follow_score(1.into()).unwrap();
        assert_eq!(
            follower.matches[1.into()..],
            [MatchPerPitch::new(0.into(), 1.into(), 0.5, 127, 127)]
        );
        assert_eq!(follower.match_offsets_by_pitch[62], [1]);
        assert!(follower.ignored.is_empty());
    }

    #[test]
    fn skip_extra_note() {
        let score = &test_score();
        let mut follower = make_follower(score, notes![(5, 60), (25, 61), (55, 62)], &[(0, 0, 60)]);
        follower.follow_score(1.into()).unwrap();
        assert_eq!(
            follower.matches[1.into()..],
            [MatchPerPitch::new(0.into(), 2.into(), 0.5, 127, 127)]
        );
        assert!(follower.match_offsets_by_pitch[61].is_empty());
        assert_eq!(follower.match_offsets_by_pitch[62], [1]);
        assert_eq!(follower.ignored, vec![1]);
    }

    #[test]
    fn skip_missing_note() {
        let score = &test_score();
        let mut follower = make_follower(score, notes![(5, 60), (55, 64)], &[(0, 0, 60)]);
        follower.follow_score(1.into()).unwrap();
        assert_eq!(
            follower.matches[1.into()..],
            [MatchPerPitch::new(0.into(), 1.into(), 0.25, 127, 127)]
        );
        assert_eq!(follower.match_offsets_by_pitch[64], [1]);
        assert!(follower.ignored.is_empty());
    }

    #[test]
    fn only_wrong_notes() {
        let score = &test_score();
        let mut follower =
            make_follower(score, notes![(5, 60), (55, 63), (105, 66)], &[(0, 0, 60)]);
        follower.follow_score(1.into()).unwrap();
        assert!(follower.matches[1.into()..].is_empty());
        assert!(follower.match_offsets_by_pitch[63].is_empty());
        assert!(follower.match_offsets_by_pitch[66].is_empty());
        assert_eq!(follower.ignored, vec![1, 2]);
    }
}

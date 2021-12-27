use midi_reader_writer::{midly_0_5::merge_tracks, ConvertTicksToMicroseconds};
use midly::{
    num::{u4, u7},
    MidiMessage::NoteOn,
    TrackEventKind::Midi,
};
use once_cell::sync::Lazy;
use std::path::Path;

/// A note with a given pitch at a given timestamp in a score or in a live performance
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScoreNote {
    pub time: u64,
    pub pitch: u7,
}

macro_rules! notes {
    (
        $( ($t: expr, $p: expr) ),+
    ) => {
        [ $( ScoreNote {time: $t, pitch: u7::from($p)} ),+ ]
    }
}

// static TEST_SCORE: Lazy<[ScoreNote; 3]> = Lazy::new(|| {

static ALL_CHANNELS: Lazy<[u4; 16]> = Lazy::new(|| {
    (0..16)
        .map(u4::from)
        .collect::<Vec<_>>()
        .try_into()
        .expect("wrong size iterator")
});

fn make_tracks_and_channels_index<'a>(
    include_tracks_with_channels: &'a [(usize, &[u4])],
    tracks_available: usize,
) -> Vec<&'a [u4]> {
    let mut track_channels: Vec<&[u4]> = vec![&*ALL_CHANNELS; tracks_available];
    if !include_tracks_with_channels.is_empty() {
        let highest_track = include_tracks_with_channels
            .iter()
            .map(|(track_index, _)| *track_index)
            .max()
            .unwrap();
        if highest_track >= track_channels.len() {
            panic!(
                "MIDI file has only {} tracks, track {} requested",
                track_channels.len(),
                highest_track + 1
            );
        }
        track_channels.fill(&[]);
        for (track_num, channel_nums) in include_tracks_with_channels {
            track_channels[*track_num] = channel_nums;
        }
    }
    track_channels
}

pub fn load_midi_file(path: &Path, channels: &[(usize, &[u4])]) -> Vec<ScoreNote> {
    let data = std::fs::read(path).unwrap();
    let smf = midly::Smf::parse(&data).unwrap();
    let mut ticks_to_microseconds = ConvertTicksToMicroseconds::try_from(smf.header).unwrap();
    let track_channels = make_tracks_and_channels_index(channels, smf.tracks.len());
    merge_tracks(&smf.tracks)
        .filter_map(|(ticks, track_index, event)| {
            match (track_channels[track_index].len(), event) {
                (0, _) => None,
                (
                    _,
                    Midi {
                        channel,
                        message: NoteOn { key, vel: _ },
                    },
                ) => {
                    if track_channels[track_index].contains(&channel) {
                        Some(ScoreNote {
                            time: ticks_to_microseconds.convert(ticks, &event),
                            pitch: key,
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn load_midi_file_clementi() {
        let path = AsRef::<Path>::as_ref("test-asset").join("Clementi.mid");
        let score = load_midi_file(&path, &[]);
        assert_eq!(score.len(), 1332);
        assert_eq!(
            score[..5],
            notes![(0, 48), (0, 72), (500000, 72), (500000, 76), (500000, 48)]
        );
    }
}

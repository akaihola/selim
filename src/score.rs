use midi_reader_writer::{midly_0_5::merge_tracks, ConvertTicksToMicroseconds};
use midly::{
    num::{u4, u7},
    MidiMessage::NoteOn,
    TrackEventKind::{self, Midi},
};
use once_cell::sync::Lazy;
use std::{path::Path, time::Duration};

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

pub fn load_raw_midi_file<'a>(
    path: &Path,
    channels: &[(usize, &[u4])],
) -> Vec<(Duration, TrackEventKind<'a>)> {
    let data = std::fs::read(path).unwrap();
    let smf = midly::Smf::parse(&data).unwrap();
    let mut ticks_to_microseconds = ConvertTicksToMicroseconds::try_from(smf.header).unwrap();
    let track_channels = make_tracks_and_channels_index(channels, smf.tracks.len());
    merge_tracks(&smf.tracks)
        .filter_map(|(ticks, track_index, event)| {
            match (track_channels[track_index].len(), event) {
                (0, _) => None,
                (_, Midi { channel, message }) => {
                    if track_channels[track_index].contains(&channel) {
                        Some((
                            Duration::from_micros(ticks_to_microseconds.convert(ticks, &event)),
                            Midi { channel, message },
                        ))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        })
        .collect()
}

pub fn load_midi_file(path: &Path, channels: &[(usize, &[u4])]) -> Vec<ScoreNote> {
    let raw = load_raw_midi_file(path, channels);
    raw.iter().filter_map(|(time, event)| match event {
        Midi {
            channel: _,
            message: NoteOn { key, vel: _ },
        } => Some(ScoreNote {
            time: time.as_micros() as u64,
            pitch: *key,
        }),
        _ => None,
    }).collect()
}

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "B", "H",
];
const NOTE_NAMES_LOWER: [&str; 12] = [
    "c", "c#", "d", "eb", "e", "f", "f#", "g", "ab", "a", "b", "h",
];
const OCTAVES: [(&str, bool); 11] = [
    ("-3", false), //  0
    ("-2", false), // 12
    ("-1", false), // 24
    ("", false),   // 36
    ("", true),    // 48
    ("1", false),  // 60
    ("2", false),  // 72
    ("3", false),  // 84
    ("4", false),  // 96
    ("5", false),  // 108
    ("6", false),  // 120
];

pub fn pitch_to_name(pitch: u7) -> String {
    let pitch_u8 = pitch.as_int();
    let pitch_class = (pitch_u8 % 12) as usize;
    let (octave, lower) = OCTAVES[(pitch_u8 / 12) as usize];
    let pitch_symbol: &str = match lower {
        false => NOTE_NAMES[pitch_class],
        true => NOTE_NAMES_LOWER[pitch_class],
    };
    format!("{}{}", pitch_symbol, octave)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
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

    #[test]
    fn load_midi_file_clementi_track_1_channel_1() {
        let path = AsRef::<Path>::as_ref("test-asset").join("Clementi.mid");
        let score = load_midi_file(&path, &[(1, &[u4::from(0)])]);
        assert_eq!(score.len(), 908);
        assert_eq!(
            score[..5],
            notes![
                (0, 72),
                (500000, 72),
                (500000, 76),
                (750000, 76),
                (750000, 72)
            ]
        );
    }

    #[test]
    fn load_midi_file_clementi_track_1_channel_2() {
        let path = AsRef::<Path>::as_ref("test-asset").join("Clementi.mid");
        let score = load_midi_file(&path, &[(1, &[u4::from(1)])]);
        assert_eq!(score.len(), 0);
    }

    #[test]
    fn load_midi_file_clementi_track_3_channel_2() {
        let path = AsRef::<Path>::as_ref("test-asset").join("Clementi.mid");
        let score = load_midi_file(&path, &[(1, &[u4::from(2)])]);
        assert_eq!(score.len(), 0);
    }

    #[rstest(
        pitch,
        expect,
        case(0, "C-3"),
        case(1, "C#-3"),
        case(2, "D-3"),
        case(3, "Eb-3"),
        case(4, "E-3"),
        case(5, "F-3"),
        case(6, "F#-3"),
        case(7, "G-3"),
        case(8, "Ab-3"),
        case(9, "A-3"),
        case(10, "B-3"),
        case(11, "H-3"),
        case(12, "C-2"),
        case(12 + 1, "C#-2"),
        case(12 + 2, "D-2"),
        case(12 + 3, "Eb-2"),
        case(12 + 4, "E-2"),
        case(12 + 5, "F-2"),
        case(12 + 6, "F#-2"),
        case(12 + 7, "G-2"),
        case(12 + 8, "Ab-2"),
        case(12 + 9, "A-2"),
        case(12 + 10, "B-2"),
        case(12 + 11, "H-2"),
        case(24, "C-1"),
        case(24 + 1, "C#-1"),
        case(24 + 2, "D-1"),
        case(24 + 3, "Eb-1"),
        case(24 + 4, "E-1"),
        case(24 + 5, "F-1"),
        case(24 + 6, "F#-1"),
        case(24 + 7, "G-1"),
        case(24 + 8, "Ab-1"),
        case(24 + 9, "A-1"),
        case(24 + 10, "B-1"),
        case(24 + 11, "H-1"),
        case(36, "C"),
        case(36 + 1, "C#"),
        case(36 + 2, "D"),
        case(36 + 3, "Eb"),
        case(36 + 4, "E"),
        case(36 + 5, "F"),
        case(36 + 6, "F#"),
        case(36 + 7, "G"),
        case(36 + 8, "Ab"),
        case(36 + 9, "A"),
        case(36 + 10, "B"),
        case(36 + 11, "H"),
        case(48, "c"),
        case(48 + 1, "c#"),
        case(48 + 2, "d"),
        case(48 + 3, "eb"),
        case(48 + 4, "e"),
        case(48 + 5, "f"),
        case(48 + 6, "f#"),
        case(48 + 7, "g"),
        case(48 + 8, "ab"),
        case(48 + 9, "a"),
        case(48 + 10, "b"),
        case(48 + 11, "h"),
        case(60, "C1"),
        case(60 + 1, "C#1"),
        case(60 + 2, "D1"),
        case(60 + 3, "Eb1"),
        case(60 + 4, "E1"),
        case(60 + 5, "F1"),
        case(60 + 6, "F#1"),
        case(60 + 7, "G1"),
        case(60 + 8, "Ab1"),
        case(60 + 9, "A1"),
        case(60 + 10, "B1"),
        case(60 + 11, "H1"),
        case(72, "C2"),
        case(72 + 1, "C#2"),
        case(72 + 2, "D2"),
        case(72 + 3, "Eb2"),
        case(72 + 4, "E2"),
        case(72 + 5, "F2"),
        case(72 + 6, "F#2"),
        case(72 + 7, "G2"),
        case(72 + 8, "Ab2"),
        case(72 + 9, "A2"),
        case(72 + 10, "B2"),
        case(72 + 11, "H2"),
        case(84, "C3"),
        case(84 + 1, "C#3"),
        case(84 + 2, "D3"),
        case(84 + 3, "Eb3"),
        case(84 + 4, "E3"),
        case(84 + 5, "F3"),
        case(84 + 6, "F#3"),
        case(84 + 7, "G3"),
        case(84 + 8, "Ab3"),
        case(84 + 9, "A3"),
        case(84 + 10, "B3"),
        case(84 + 11, "H3"),
        case(96, "C4"),
        case(96 + 1, "C#4"),
        case(96 + 2, "D4"),
        case(96 + 3, "Eb4"),
        case(96 + 4, "E4"),
        case(96 + 5, "F4"),
        case(96 + 6, "F#4"),
        case(96 + 7, "G4"),
        case(96 + 8, "Ab4"),
        case(96 + 9, "A4"),
        case(96 + 10, "B4"),
        case(96 + 11, "H4"),
        case(108, "C5"),
        case(108 + 1, "C#5"),
        case(108 + 2, "D5"),
        case(108 + 3, "Eb5"),
        case(108 + 4, "E5"),
        case(108 + 5, "F5"),
        case(108 + 6, "F#5"),
        case(108 + 7, "G5"),
        case(108 + 8, "Ab5"),
        case(108 + 9, "A5"),
        case(108 + 10, "B5"),
        case(108 + 11, "H5"),
        case(120, "C6"),
        case(120 + 1, "C#6"),
        case(120 + 2, "D6"),
        case(120 + 3, "Eb6"),
        case(120 + 4, "E6"),
        case(120 + 5, "F6"),
        case(120 + 6, "F#6"),
        case(120 + 7, "G6"),
    )]
    fn test_pitch_to_name(pitch: u8, expect: &str) {
        let note_name = pitch_to_name(u7::from(pitch));
        assert_eq!(note_name, expect);
    }
}

use midi_reader_writer::{midly_0_5::merge_tracks, ConvertTicksToMicroseconds};
use midly::{
    num::{u4, u7},
    MidiMessage::NoteOn,
    Smf,
    TrackEventKind::{self, Midi},
};
use once_cell::sync::Lazy;
use std::{path::Path, str::FromStr, time::Duration};

use crate::ScoreVec;

/// A note-on with a given pitch at a given timestamp in a score or in a live performance
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScoreNote {
    pub time: Duration,
    pub pitch: u7,
    pub velocity: u7,
}

/// A midly MIDI message at a given timestamp
pub struct ScoreEvent<'a> {
    pub time: Duration,
    pub message: TrackEventKind<'a>,
}

#[derive(Debug, PartialEq)]
pub struct Channels {
    pub track: usize,
    pub midi_channels: Vec<u4>,
}

type ChannelsParseError = String;

impl FromStr for Channels {
    type Err = ChannelsParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.rsplitn(2, ':');
        let midi_channels: Result<Vec<u4>, _> = match parts.next() {
            Some(m) => m
                .split(',')
                .map(|c| match c.trim().parse::<u8>() {
                    Ok(n) => {
                        u4::try_from(n - 1).ok_or(format!("Invalid MIDI channel number '{c}'"))
                    }
                    Err(_) => Err(format!("Invalid MIDI channel number '{c}'")),
                })
                .collect(),
            None => return Err(format!("Can't parse MIDI channels list from '{s}'")),
        };
        let track: Result<usize, _> = match parts.next().map(|p| (p, p.trim().parse::<usize>())) {
            Some((_, Ok(0))) => Err(String::from("Invalid track number '0'")),
            Some((p, t)) => t.map_err(|_| format!("Invalid track number '{p}'")),
            None => Ok(1),
        };
        Ok(Channels {
            track: track? - 1,
            midi_channels: midi_channels?,
        })
    }
}

#[cfg(test)]
macro_rules! notes {
    (
        $( ($t: expr, $p: expr) ),+
    ) => {
        index_vec::index_vec![ $( ScoreNote {time: Duration::from_millis($t), pitch: u7::from($p), velocity: u7::from(100)} ),+ ]
    }
}

static ALL_CHANNELS: Lazy<Vec<u4>> = Lazy::new(|| (0..16).map(u4::from).collect::<Vec<_>>());

fn make_tracks_and_channels_index(
    include_tracks_with_channels: Vec<Channels>,
    tracks_available: usize,
) -> Vec<Vec<u4>> {
    let mut track_channels: Vec<Vec<u4>> = vec![ALL_CHANNELS.clone(); tracks_available];
    if !include_tracks_with_channels.is_empty() {
        let highest_track = include_tracks_with_channels
            .iter()
            .map(|channels| channels.track)
            .max()
            .unwrap();
        if highest_track >= track_channels.len() {
            panic!(
                "MIDI file has only {} tracks, track {} requested",
                track_channels.len(),
                highest_track + 1
            );
        }
        track_channels.fill(vec![]);
        for channels in include_tracks_with_channels {
            track_channels[channels.track] = channels.midi_channels;
        }
    }
    track_channels
}

pub fn smf_to_events<'a>(smf: &Smf, channels: Vec<Channels>) -> Vec<ScoreEvent<'a>> {
    let mut ticks_to_microseconds = ConvertTicksToMicroseconds::try_from(smf.header).unwrap();
    let selected_channels_by_track = make_tracks_and_channels_index(channels, smf.tracks.len());
    merge_tracks(&smf.tracks)
        .filter_map(|(ticks, track_index, event)| {
            let selected_channels = &selected_channels_by_track[track_index];
            match (selected_channels.len(), event) {
                (0, _) => None, // no MIDI channels to include from this track
                (_, Midi { channel, message }) => {
                    // at least one MIDI channel to include from this track, and the event is a MIDI message
                    // -> consider the event
                    if selected_channels.contains(&channel) {
                        // event is on a MIDI channel which should be included or this track
                        // -> include the event
                        Some(ScoreEvent {
                            time: Duration::from_micros(
                                ticks_to_microseconds.convert(ticks, &event),
                            ),
                            // Make a copy of the MIDI message so we don't include references to data in `smf`
                            message: Midi { channel, message },
                        })
                    } else {
                        // event is on a MIDI channel which should be exluded on this track
                        // -> skip the event
                        None
                    }
                }
                // event is not a MIDI message, skip it
                _ => None,
            }
        })
        .collect()
}

/// Loads a MIDI SMF file and joins events on all chosen channels of selected tracks
/// into a single list of MIDI events with timestamps
pub fn load_midi_file<'a>(path: &Path, channels: Vec<Channels>) -> Vec<ScoreEvent<'a>> {
    let data = std::fs::read(path).unwrap();
    let smf = midly::Smf::parse(&data).unwrap();
    smf_to_events(&smf, channels)
}

pub const ZERO_U7: u7 = u7::new(0);

pub fn convert_midi_note_ons(events: Vec<ScoreEvent>) -> ScoreVec {
    events
        .iter()
        .filter_map(|ScoreEvent { time, message }| match message {
            Midi {
                channel: _,
                message:
                    NoteOn {
                        key: _,
                        vel: ZERO_U7,
                    },
            } => None,
            Midi {
                channel: _,
                message: NoteOn { key, vel },
            } => Some(ScoreNote {
                time: *time,
                pitch: *key,
                velocity: *vel,
            }),
            _ => None,
        })
        .collect()
}

/// Loads a MIDI SMF file and joins events on all chosen channels of selected tracks
/// into a single list of MIDI events with timestamps in a Selim `ScoreVec`
pub fn load_midi_file_note_ons(path: &Path, channels: Vec<Channels>) -> ScoreVec {
    let raw = load_midi_file(path, channels);
    convert_midi_note_ons(raw)
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
    format!("{pitch_symbol}{octave}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abc::simplify_score;
    use rstest::rstest;
    use std::path::Path;

    macro_rules! chnls {
        (
            $t: expr, $c: expr
        ) => {
            Channels {
                track: $t,
                midi_channels: $c.into_iter().map(u4::from).collect(),
            }
        };
    }

    #[rstest(
        case::empty("", Err(String::from("Invalid MIDI channel number ''"))),
        case::empty_channels("1:", Err(String::from("Invalid MIDI channel number ''"))),
        case::empty_track(":1", Err(String::from("Invalid track number ''"))),
        case::implicit_track1_ch16("16", Ok(chnls!(0, vec![15]))),
        case::implicit_track1_ch2_3("2,3", Ok(chnls!(0, vec![1, 2]))),
        case::track1_ch2_3("1:2,3", Ok(chnls!(0, vec![1, 2]))),
        case::track16_ch_all("16:1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16", Ok(chnls!(15, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]))),
        case::track0("0:1", Err(String::from("Invalid track number '0'"))),
        case::invalid_track("foo:1", Err(String::from("Invalid track number 'foo'"))),
        case::invalid_channel("1:foo", Err(String::from("Invalid MIDI channel number 'foo'"))),
        case::negative_track("-1:1", Err(String::from("Invalid track number '-1'"))),
        case::negative_channel("1:-1", Err(String::from("Invalid MIDI channel number '-1'"))),
        case::whitespace(" 7 : 1 , 15 ", Ok(chnls!(6, vec![0, 14]))),
        case::channel17("1:17", Err(String::from("Invalid MIDI channel number '17'"))),
    )]
    fn channels_from_str(
        #[case] channels: &str,
        #[case] expect: Result<Channels, ChannelsParseError>,
    ) {
        assert_eq!(Channels::from_str(channels), expect);
    }

    #[test]
    fn load_midi_file_clementi() {
        let path = AsRef::<Path>::as_ref("test-asset").join("Clementi.mid");
        let score = simplify_score(load_midi_file_note_ons(&path, vec![]));
        assert_eq!(score.len(), 666);
        assert_eq!(
            score[..5.into()],
            notes![(0, 48), (0, 72), (500, 76), (750, 72), (1000, 67)][..]
        );
    }

    #[test]
    fn load_midi_file_clementi_track_1_channel_1() {
        let path = AsRef::<Path>::as_ref("test-asset").join("Clementi.mid");
        let score = simplify_score(load_midi_file_note_ons(&path, vec![chnls!(1, vec![0])]));
        assert_eq!(score.len(), 454);
        assert_eq!(
            score[..5.into()],
            notes![(0, 72), (500, 76), (750, 72), (1000, 67), (1500, 67)][..]
        );
    }

    #[test]
    fn load_midi_file_clementi_track_1_channel_2() {
        let path = AsRef::<Path>::as_ref("test-asset").join("Clementi.mid");
        let score = load_midi_file_note_ons(&path, vec![chnls!(1, vec![1])]);
        assert_eq!(score.len(), 0);
    }

    #[test]
    fn load_midi_file_clementi_track_3_channel_2() {
        let path = AsRef::<Path>::as_ref("test-asset").join("Clementi.mid");
        let score = load_midi_file_note_ons(&path, vec![chnls!(1, vec![2])]);
        assert_eq!(score.len(), 0);
    }

    #[rstest(
        pitch,
        expect,
        case::c_3(0, "C-3"),
        case::cs_3(1,  "C#-3"),
        case::d_3(2, "D-3"),
        case::eb_3(3, "Eb-3"),
        case::e_3(4, "E-3"),
        case::f_3(5, "F-3"),
        case::fs_3(6, "F#-3"),
        case::g_3(7, "G-3"),
        case::ab_3(8, "Ab-3"),
        case::a_3(9, "A-3"),
        case::b_3(10, "B-3"),
        case::h_3(11, "H-3"),
        case::c_2(12, "C-2"),
        case::cs_2(12 + 1, "C#-2"),
        case::d_2(12 + 2, "D-2"),
        case::eb_2(12 + 3, "Eb-2"),
        case::e_2(12 + 4, "E-2"),
        case::f_2(12 + 5, "F-2"),
        case::fs_2(12 + 6, "F#-2"),
        case::g_2(12 + 7, "G-2"),
        case::ab_2(12 + 8, "Ab-2"),
        case::a_2(12 + 9, "A-2"),
        case::b_2(12 + 10, "B-2"),
        case::h_2(12 + 11, "H-2"),
        case::c_1(24, "C-1"),
        case::cs_1(24 + 1, "C#-1"),
        case::d_1(24 + 2, "D-1"),
        case::eb_1(24 + 3, "Eb-1"),
        case::e_1(24 + 4, "E-1"),
        case::f_1(24 + 5, "F-1"),
        case::fs_1(24 + 6, "F#-1"),
        case::g_1(24 + 7, "G-1"),
        case::ab_1(24 + 8, "Ab-1"),
        case::a_1(24 + 9, "A-1"),
        case::b_1(24 + 10, "B-1"),
        case::h_1(24 + 11, "H-1"),
        case::c_(36, "C"),
        case::cs_(36 + 1, "C#"),
        case::d_(36 + 2, "D"),
        case::eb_(36 + 3, "Eb"),
        case::e_(36 + 4, "E"),
        case::f_(36 + 5, "F"),
        case::fs_(36 + 6, "F#"),
        case::g_(36 + 7, "G"),
        case::ab_(36 + 8, "Ab"),
        case::a_(36 + 9, "A"),
        case::b_(36 + 10, "B"),
        case::h_(36 + 11, "H"),
        case::c(48, "c"),
        case::cs(48 + 1, "c#"),
        case::d(48 + 2, "d"),
        case::eb(48 + 3, "eb"),
        case::e(48 + 4, "e"),
        case::f(48 + 5, "f"),
        case::fs(48 + 6, "f#"),
        case::g(48 + 7, "g"),
        case::ab(48 + 8, "ab"),
        case::a(48 + 9, "a"),
        case::b(48 + 10, "b"),
        case::h(48 + 11, "h"),
        case::c1(60, "C1"),
        case::cs1(60 + 1, "C#1"),
        case::d1(60 + 2, "D1"),
        case::eb1(60 + 3, "Eb1"),
        case::e1(60 + 4, "E1"),
        case::f1(60 + 5, "F1"),
        case::fs1(60 + 6, "F#1"),
        case::g1(60 + 7, "G1"),
        case::ab1(60 + 8, "Ab1"),
        case::a1(60 + 9, "A1"),
        case::b1(60 + 10, "B1"),
        case::h1(60 + 11, "H1"),
        case::c2(72, "C2"),
        case::cs2(72 + 1, "C#2"),
        case::d2(72 + 2, "D2"),
        case::eb2(72 + 3, "Eb2"),
        case::e2(72 + 4, "E2"),
        case::f2(72 + 5, "F2"),
        case::fs2(72 + 6, "F#2"),
        case::g2(72 + 7, "G2"),
        case::ab2(72 + 8, "Ab2"),
        case::a2(72 + 9, "A2"),
        case::b2(72 + 10, "B2"),
        case::h2(72 + 11, "H2"),
        case::c3(84, "C3"),
        case::cs3(84 + 1, "C#3"),
        case::d3(84 + 2, "D3"),
        case::eb3(84 + 3, "Eb3"),
        case::e3(84 + 4, "E3"),
        case::f3(84 + 5, "F3"),
        case::fs3(84 + 6, "F#3"),
        case::g3(84 + 7, "G3"),
        case::ab3(84 + 8, "Ab3"),
        case::a3(84 + 9, "A3"),
        case::b3(84 + 10, "B3"),
        case::h3(84 + 11, "H3"),
        case::c4(96, "C4"),
        case::cs4(96 + 1, "C#4"),
        case::d4(96 + 2, "D4"),
        case::eb4(96 + 3, "Eb4"),
        case::e4(96 + 4, "E4"),
        case::f4(96 + 5, "F4"),
        case::fs4(96 + 6, "F#4"),
        case::g4(96 + 7, "G4"),
        case::ab4(96 + 8, "Ab4"),
        case::a4(96 + 9, "A4"),
        case::b4(96 + 10, "B4"),
        case::h4(96 + 11, "H4"),
        case::c5(108, "C5"),
        case::cs5(108 + 1, "C#5"),
        case::d5(108 + 2, "D5"),
        case::eb5(108 + 3, "Eb5"),
        case::e5(108 + 4, "E5"),
        case::f5(108 + 5, "F5"),
        case::fs5(108 + 6, "F#5"),
        case::g5(108 + 7, "G5"),
        case::ab5(108 + 8, "Ab5"),
        case::a5(108 + 9, "A5"),
        case::b5(108 + 10, "B5"),
        case::h5(108 + 11, "H5"),
        case::c6(120, "C6"),
        case::cs6(120 + 1, "C#6"),
        case::d6(120 + 2, "D6"),
        case::eb6(120 + 3, "Eb6"),
        case::e6(120 + 4, "E6"),
        case::f6(120 + 5, "F6"),
        case::fs6(120 + 6, "F#6"),
        case::g6(120 + 7, "G6"),
    )]
    fn test_pitch_to_name(pitch: u8, expect: &str) {
        let note_name = pitch_to_name(u7::from(pitch));
        assert_eq!(note_name, expect);
    }
}

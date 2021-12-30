use midir::{Ignore, MidiInput};
use midly::live::{LiveEvent, LiveEvent::Midi};
use midly::num::u4;
use midly::MidiMessage::NoteOn;
use selim::device::{find_port, DeviceSelector};
use selim::score::{load_midi_file, pitch_to_name, ScoreNote};
use selim::{follow_score, Match};
use std::boxed::Box;
use std::error::Error;
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use structopt::StructOpt;

#[derive(StructOpt)]
struct Cli {
    // TODO: `conflicts_with` doesn't seem to work!
    #[structopt(
        short = "r",
        long = "rec-device-num",
        conflicts_with = "rec_device_name"
    )]
    rec_device_num: Option<usize>,
    #[structopt(
        short = "D",
        long = "rec-device-name",
        conflicts_with = "rec_device_num"
    )]
    rec_device_name: Option<String>,
    #[structopt(short = "i", long = "--input-score-file", parse(from_os_str))]
    input_score_file: PathBuf,
    #[structopt(short = "p", long = "--playback-score-file", parse(from_os_str))]
    playback_score_file: PathBuf,
}

fn main() {
    let args = Cli::from_args();
    let device = match (args.rec_device_num, args.rec_device_name) {
        (Some(rec_device_num), None) => DeviceSelector::Number(rec_device_num),
        (None, Some(rec_device_name)) => DeviceSelector::NameSubstring(rec_device_name),
        _ => {
            panic!("-d/--device or -D/--device-name required")
        }
    };
    let input_score = load_midi_file(&args.input_score_file, &[(1, &[u4::from(0)])]);
    let playback_score = load_midi_file(&args.playback_score_file, &[(2, &[u4::from(1)])]);
    assert!(!input_score.is_empty());
    if let Err(err) = run(device, input_score, playback_score) {
        eprintln!("Error: {}", err)
    }
}

fn callback(microsecond: u64, message: &[u8], tx: &mut Sender<ScoreNote>) {
    let event = LiveEvent::parse(message).unwrap();
    if let Midi {
        channel: _,
        message: NoteOn { key, vel: _ },
    } = event
    {
        tx.send(ScoreNote {
            time: microsecond,
            pitch: key,
        })
        .unwrap();
    }
}

fn run(
    device: DeviceSelector,
    input_score: Vec<ScoreNote>,
    _playback_score: Vec<ScoreNote>,
) -> Result<(), Box<dyn Error>> {
    assert!(!input_score.is_empty());
    let mut midi_input = MidiInput::new("selim")?;
    midi_input.ignore(Ignore::All);
    let in_port = find_port(&midi_input, device).unwrap();
    let in_port_name = midi_input.port_name(&in_port);
    // _conn_in needs to be a named parameter, because it needs to be kept alive
    // until the end of the scope
    let (tx, rx) = mpsc::channel::<ScoreNote>();
    let _conn_in = midi_input.connect(&in_port, "selim-live-to-score", callback, tx)?;

    eprintln!(
        "Connection open, reading input from '{}' (press Ctrl-C to exit) ...",
        in_port_name.unwrap()
    );

    let mut live = vec![];
    let mut prev_match = None;
    let mut new_live_index = 0;
    let mut prev_stretch_factor = 1.0;
    let mut matches = vec![];
    loop {
        print_expect(&input_score, prev_match);
        let note = rx.recv().unwrap();
        live.push(note);
        let (score_time, stretch_factor, new_matches, ignored) = follow_score(
            &input_score,
            &live,
            prev_match,
            new_live_index,
            prev_stretch_factor,
        );
        print_got(
            &live,
            note,
            score_time,
            stretch_factor,
            &new_matches,
            &ignored,
        );
        matches.extend(new_matches.iter());
        new_live_index = live.len();
        prev_stretch_factor = stretch_factor;
        prev_match = matches.last().cloned();
    }
}

fn print_expect(input_score: &[ScoreNote], prev_match: Option<Match>) {
    let score_next = match prev_match {
        Some(Match {
            score_index,
            live_index: _,
        }) => score_index + 1,
        _ => 0,
    };
    if score_next < input_score.len() {
        print!(
            "score {:>3} {:>7.3} expect {}",
            score_next,
            input_score[score_next].time as f64 / 1000000.0,
            pitch_to_name(input_score[score_next].pitch),
        );
    } else {
        print!("score ended, expect nothing more");
    }
    stdout().flush().unwrap();
}

fn print_got(
    live: &[ScoreNote],
    note: ScoreNote,
    score_time: u64,
    stretch_factor: f32,
    new_matches: &[Match],
    ignored: &[usize],
) {
    println!(
        ", got {} at live {:>3} {:>7.3} -> {:>7.3} {:>5.1}% {:?} {:?}",
        pitch_to_name(note.pitch),
        live.len() - 1,
        note.time as f64 / 1000000.0,
        score_time as f64 / 100000.0,
        100.0 * stretch_factor,
        new_matches
            .iter()
            .map(|m| {
                format!(
                    "{}->{} {}",
                    m.live_index, m.score_index, live[m.live_index].pitch
                )
            })
            .collect::<Vec<_>>(),
        ignored
    );
}

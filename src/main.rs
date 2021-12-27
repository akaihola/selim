use midir::{Ignore, MidiInput};
use midly::live::{LiveEvent, LiveEvent::Midi};
use midly::MidiMessage::NoteOn;
use selim::device::{find_port, DeviceSelector};
use selim::score::load_midi_file;
use std::boxed::Box;
use std::error::Error;
use std::io::stdin;
use std::path::PathBuf;
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
    let _input_score = load_midi_file(&args.input_score_file, &[(2, &[]), (3, &[])]);
    let _playback_score = load_midi_file(&args.playback_score_file, &[(0, &[]), (1, &[])]);
    if let Err(err) = run(device) {
        eprintln!("Error: {}", err)
    }
}

fn callback<T>(microsecond: u64, message: &[u8], _: &mut T) {
    let event = LiveEvent::parse(message).unwrap();
    if let Midi {
        channel: _,
        message: NoteOn { key, vel: _ },
    } = event
    {
        println!("{};{}", microsecond, key);
    }
}

fn run(device: DeviceSelector) -> Result<(), Box<dyn Error>> {
    let mut midi_input = MidiInput::new("selim")?;
    midi_input.ignore(Ignore::All);
    let in_port = find_port(&midi_input, device).unwrap();
    let in_port_name = midi_input.port_name(&in_port);
    // _conn_in needs to be a named parameter, because it needs to be kept alive
    // until the end of the scope
    let _conn_in = midi_input.connect(&in_port, "selim-live-to-score", callback, ())?;

    eprintln!(
        "Connection open, reading input from '{}' (press enter to exit) ...",
        in_port_name.unwrap()
    );

    println!("time;pitch");

    let mut input = String::new();
    stdin().read_line(&mut input)?; // wait for next enter key press

    eprintln!("Closing connection");
    Ok(())
}

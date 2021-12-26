use midir::{Ignore, MidiInput};
use midly::live::{LiveEvent, LiveEvent::Midi};
use midly::MidiMessage::NoteOn;
use std::boxed::Box;
use std::error::Error;
use std::io::stdin;
use structopt::StructOpt;
use selim::device::{DeviceSelector, find_port};

#[derive(StructOpt)]
struct Cli {
    // TODO: `conflicts_with` doesn't seem to work!
    #[structopt(short = "d", long = "device", conflicts_with = "device_name")]
    device_number: Option<usize>,
    #[structopt(short = "D", long = "device-name", conflicts_with = "device_number")]
    device_name: Option<String>,
}

fn main() {
    let args = Cli::from_args();
    let device = match (args.device_number, args.device_name) {
        (Some(device_number), None) => DeviceSelector::Number(device_number),
        (None, Some(device_name)) => DeviceSelector::NameSubstring(device_name),
        _ => {
            panic!("-d/--device or -D/--device-name required")
        }
    };
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

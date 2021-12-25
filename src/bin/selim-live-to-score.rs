use midir::{Ignore, MidiInput};
use midly::live::{LiveEvent, LiveEvent::Midi};
use midly::MidiMessage::NoteOn;
use std::boxed::Box;
use std::error::Error;
use std::io::{stdin, stdout, Write};
use structopt::StructOpt;

enum DeviceSpec {
    None,
    Number(usize),
    Name(String),
}

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
        (Some(device_number), None) => DeviceSpec::Number(device_number),
        (None, Some(device_name)) => DeviceSpec::Name(device_name),
        _ => DeviceSpec::None,
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

fn run(device: DeviceSpec) -> Result<(), Box<dyn Error>> {
    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::All);

    // Get an input port (read from console if multiple are available)
    let in_ports = midi_in.ports();
    let num_ports = in_ports.len();
    let (device_number, message) = match (num_ports, device) {
        (0, _) => return Err("no input port found".into()),
        (1, DeviceSpec::None) => (0, "Choosing the only available input port"),
        (_, DeviceSpec::Name(device_name)) => {
            let mut items = in_ports.iter().enumerate();
            loop {
                match items.next() {
                    Some((i, p)) => {
                        let port_name = midi_in.port_name(p).unwrap();
                        if port_name.contains(&device_name) {
                            break (i, "Choosing input port {} {}");
                        };
                    }
                    None => {
                        return Err(
                            format!(
                                "None of the {} available devices contain the string '{}' in their names",
                                num_ports,
                                device_name
                            )
                            .into()
                        );
                    }
                }
            }
        }
        (_, DeviceSpec::Number(device_number)) => {
            if device_number >= num_ports {
                return Err(format!(
                    "Invalid device number {}. Only devices 0..{} available",
                    device_number,
                    num_ports - 1
                )
                .into());
            }
            (device_number, "Choosing input port")
        }
        (_, DeviceSpec::None) => {
            eprintln!("\nAvailable input ports:");
            for (i, p) in in_ports.iter().enumerate() {
                eprintln!("{}: {}", i, midi_in.port_name(p).unwrap());
            }
            eprint!("Please select input port: ");
            stdout().flush()?;
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            (input.trim().parse::<usize>()?, "Selecting input port")
        }
    };

    let in_port = &in_ports[device_number];
    let in_port_name = midi_in.port_name(in_port)?;
    eprint!("{} {} {}", message, device_number, in_port_name);
    eprintln!("\nOpening connection");

    // _conn_in needs to be a named parameter, because it needs to be kept alive
    // until the end of the scope
    let _conn_in = midi_in.connect(in_port, "selim-live-to-score", callback, ())?;

    eprintln!(
        "Connection open, reading input from '{}' (press enter to exit) ...",
        in_port_name
    );

    println!("time;pitch");

    let mut input = String::new();
    stdin().read_line(&mut input)?; // wait for next enter key press

    eprintln!("Closing connection");
    Ok(())
}

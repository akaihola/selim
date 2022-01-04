use crate::score::ScoreNote;
use crossbeam_channel::{unbounded, Receiver, Sender};
use midir::{Ignore, MidiIO, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use std::{any::TypeId, error::Error, fmt::Display};

pub enum DeviceSelector {
    Number(usize),
    NameSubstring(String),
}

impl Display for DeviceSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceSelector::Number(n) => f.write_fmt(format_args!("{}", n)),
            DeviceSelector::NameSubstring(data) => f.write_fmt(format_args!("\"{}\"", data)),
        }
    }
}

fn get_midi_io_direction<T>(_midi_io: &T) -> &'static str
where
    T: MidiIO + 'static,
{
    if TypeId::of::<T>() == TypeId::of::<MidiInput>() {
        "input"
    } else {
        "output"
    }
}

pub fn find_port<T>(midi_io: &T, device: DeviceSelector) -> Result<T::Port, String>
where
    T: MidiIO + 'static,
{
    let ports = midi_io.ports();
    let numbered_port_names = ports
        .iter()
        .map(|p| midi_io.port_name(p).unwrap())
        .enumerate();

    let matches = numbered_port_names
        .filter(|(i, name)| match &device {
            DeviceSelector::NameSubstring(name_substring) => name.contains(name_substring),
            DeviceSelector::Number(number) => i == number,
        })
        .collect::<Vec<(usize, String)>>();
    let direction = get_midi_io_direction(midi_io);
    if matches.is_empty() {
        print_ports(ports, midi_io, direction);
        return Err(format!("No MIDI {} port matching {}", direction, device));
    } else if matches.len() > 1 {
        print_ports(ports, midi_io, direction);
        return Err(format!(
            "Multiple MIDI {} ports matching {}",
            direction, device
        ));
    };

    let (device_number, port_name) = matches[0].clone();
    eprintln!(
        "Selecting MIDI {} port {}: {}",
        direction, device_number, port_name
    );
    Ok(ports[device_number].clone())
}

fn print_ports<T>(ports: Vec<<T as MidiIO>::Port>, midi_io: &T, direction: &str)
where
    T: MidiIO,
{
    eprintln!("Found {} ports:", direction);
    for (i, port) in ports.iter().enumerate() {
        eprintln!("{}: {}", i, midi_io.port_name(port).unwrap())
    }
}

pub struct MInput {
    _connection: MidiInputConnection<Sender<ScoreNote>>,
    pub rx: Receiver<ScoreNote>,
}

pub fn open_midi_input<F>(device: DeviceSelector, callback: F) -> Result<MInput, Box<dyn Error>>
where
    F: Fn(u64, &[u8], &mut Sender<ScoreNote>) + std::marker::Send + 'static,
{
    let mut midi_input = MidiInput::new("selim")?;
    midi_input.ignore(Ignore::All);
    let in_port = find_port(&midi_input, device)?;
    let in_port_name = midi_input.port_name(&in_port)?;
    let (tx, rx) = unbounded::<ScoreNote>();
    let _connection = midi_input.connect(&in_port, "selim-live-to-score", callback, tx)?;
    eprintln!(
        "Connection open, reading input from '{}' (press Ctrl-C to exit) ...",
        in_port_name
    );
    // `_connection` needs to be returned, because it needs to be kept alive for `rx` to work
    Ok(MInput { _connection, rx })
}

pub fn open_midi_output(device: DeviceSelector) -> Result<MidiOutputConnection, Box<dyn Error>> {
    let midi_output = MidiOutput::new("selim")?;
    let out_port = find_port(&midi_output, device)?;
    let connection = midi_output.connect(&out_port, "selim-live-to-score")?;
    Ok(connection)
}

#[cfg(test)]
mod tests {
    //! Tests require virtual ports and therefore can't work on Windows or Web MIDI
    #![cfg(not(any(windows, target_arch = "wasm32")))]
    use super::*;

    struct MockMidiIo();

    impl MidiIO for MockMidiIo {
        type Port = String;

        fn ports(&self) -> Vec<Self::Port> {
            vec!["port one (1)".to_string(), "port two (2)".to_string()]
        }

        fn port_count(&self) -> usize {
            todo!()
        }

        fn port_name(&self, port: &Self::Port) -> Result<String, midir::PortInfoError> {
            Ok(port.clone())
        }
    }

    #[test]
    fn format_device_selector_number() {
        let result = format!("{}", DeviceSelector::Number(42));
        assert_eq!(result, "42");
    }

    #[test]
    fn format_device_selector_name_substring() {
        let result = format!("{}", DeviceSelector::NameSubstring(" foo ".to_string()));
        assert_eq!(result, "\" foo \"");
    }

    #[test]
    fn get_midi_io_direction_input() {
        let result = get_midi_io_direction(&MidiInput::new("client_name").unwrap());
        assert_eq!(result, "input");
    }

    #[test]
    fn get_midi_io_direction_output() {
        let result = get_midi_io_direction(&MidiOutput::new("client_name").unwrap());
        assert_eq!(result, "output");
    }

    #[test]
    fn find_port_by_substring() {
        let midi_io = MockMidiIo {};
        let device = DeviceSelector::NameSubstring(" one ".to_string());
        let port = find_port(&midi_io, device);
        assert_eq!(port.unwrap(), "port one (1)");
    }

    #[test]
    fn find_port_by_substring_not_exists() {
        let midi_io = MockMidiIo {};
        let device = DeviceSelector::NameSubstring(" zero ".to_string());
        let port = find_port(&midi_io, device);
        assert_eq!(
            port.err().unwrap(),
            "No MIDI output port matching \" zero \""
        );
    }

    #[test]
    fn find_port_by_substring_multiple_matches() {
        let midi_io = MockMidiIo {};
        let device = DeviceSelector::NameSubstring("port ".to_string());
        let port = find_port(&midi_io, device);
        assert_eq!(
            port.err().unwrap(),
            "Multiple MIDI output ports matching \"port \""
        );
    }

    #[test]
    fn find_port_by_number() {
        let midi_io = MockMidiIo {};
        let device = DeviceSelector::Number(0);
        let port = find_port(&midi_io, device);
        assert_eq!(port.unwrap(), "port one (1)");
    }
}

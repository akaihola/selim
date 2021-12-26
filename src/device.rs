use midir::{MidiInput, MidiInputPort};

pub enum DeviceSelector {
    Number(usize),
    NameSubstring(String),
}

pub fn get_midi_in_port(midi_input: &MidiInput, device: DeviceSelector) -> MidiInputPort {
    // Get an input port (read from console if multiple are available)
    let in_ports = midi_input.ports();
    let numbered_port_names = in_ports
        .iter()
        .map(|p| midi_input.port_name(p).unwrap())
        .enumerate();

    let matches_iter = numbered_port_names.filter(|(i, name)| match &device {
        DeviceSelector::NameSubstring(name_substring) => name.contains(name_substring),
        DeviceSelector::Number(number) => i == number,
    });
    let matches = matches_iter.collect::<Vec<(usize, String)>>();
    if matches.is_empty() {
        panic!("No matching devices")
    } else if matches.len() > 1 {
        panic!("Multiple matching devices")
    };

    let (device_number, in_port_name) = matches[0].clone();
    eprintln!(
        "Selecting MIDI input port {} {}",
        device_number, in_port_name
    );
    in_ports[device_number].clone()
}

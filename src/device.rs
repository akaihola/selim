use midir::MidiIO;

pub enum DeviceSelector {
    Number(usize),
    NameSubstring(String),
}

pub fn find_port<T>(midi_io: &T, device: DeviceSelector) -> Result<T::Port, &'static str>
where
    T: MidiIO,
{
    // Get an input port (read from console if multiple are available)
    let ports = midi_io.ports();
    let numbered_port_names = ports
        .iter()
        .map(|p| midi_io.port_name(p).unwrap())
        .enumerate();

    let matches_iter = numbered_port_names.filter(|(i, name)| match &device {
        DeviceSelector::NameSubstring(name_substring) => name.contains(name_substring),
        DeviceSelector::Number(number) => i == number,
    });
    let matches = matches_iter.collect::<Vec<(usize, String)>>();
    if matches.is_empty() {
        return Err("No matching devices");
    } else if matches.len() > 1 {
        return Err("Multiple matching devices");
    };

    let (device_number, in_port_name) = matches[0].clone();
    eprintln!(
        "Selecting MIDI input port {} {}",
        device_number, in_port_name
    );
    Ok(ports[device_number].clone())
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
        assert_eq!(port.err().unwrap(), "No matching devices");
    }

    #[test]
    fn find_port_by_substring_multiple_matches() {
        let midi_io = MockMidiIo {};
        let device = DeviceSelector::NameSubstring("port ".to_string());
        let port = find_port(&midi_io, device);
        assert_eq!(port.err().unwrap(), "Multiple matching devices");
    }

    #[test]
    fn find_port_by_number() {
        let midi_io = MockMidiIo {};
        let device = DeviceSelector::Number(0);
        let port = find_port(&midi_io, device);
        assert_eq!(port.unwrap(), "port one (1)");
    }
}

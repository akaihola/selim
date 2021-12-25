use midi_reader_writer::{midly_0_5::merge_tracks, ConvertTicksToMicroseconds};
use midly::{MidiMessage::NoteOn, TrackEventKind::Midi};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let midi_file_path = &args[1];
    let data = std::fs::read(midi_file_path).unwrap();
    let smf = midly::Smf::parse(&data).unwrap();
    let mut ticks_to_microseconds = ConvertTicksToMicroseconds::try_from(smf.header).unwrap();

    // Iterate over the events from all tracks:
    println!("time;pitch");
        for (ticks, _track_index, event) in merge_tracks(&smf.tracks) {
        let microseconds = ticks_to_microseconds.convert(ticks, &event);
        if let Midi {
            channel: _,
            message: NoteOn { key, vel: _ },
        } = event
        {
            println!("{};{}", microseconds, key);
        }
    }
}

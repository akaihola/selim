use std::env;
use midly::{TrackEvent, TrackEventKind::Midi, MidiMessage::NoteOn};

fn main() {
    let args: Vec<String> = env::args().collect();
    let mid_file_path = &args[1];

    // Load bytes first
    let data = std::fs::read(mid_file_path).unwrap();

    // Parse the raw bytes
    let mut smf = midly::Smf::parse(&data).unwrap();

    // Use the information
    println!("midi file has {} tracks!", smf.tracks.len());

    let track = &smf.tracks[0];
    println!("first track has {} events!", track.len());

    let musical_events: Vec<&TrackEvent> = track
        .into_iter()
        .filter(|event| match event.kind {
            Midi {
                channel,
                message: NoteOn{key: _, vel: _},
            } => channel == 1,
            _ => false,
        })
        .collect();
    println!("first track has {} 'note on' events on channel 1!", musical_events.len());

    // Modify the file
    smf.header.format = midly::Format::Sequential;

    // Save it back
    smf.save("rewritten.mid").unwrap();
}

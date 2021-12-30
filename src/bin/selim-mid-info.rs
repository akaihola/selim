use midly::{MidiMessage::NoteOn, TrackEventKind::Midi};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mid_file_path = &args[1];

    // Load bytes first
    let data = std::fs::read(mid_file_path).unwrap();

    // Parse the raw bytes
    let mut smf = midly::Smf::parse(&data).unwrap();

    // Use the information
    println!("midi file has {} tracks!", smf.tracks.len());

    for (track_num, track) in smf.tracks.iter().enumerate() {
        println!("track {} has {} events", track_num + 1, track.len());
        for c in 0..16 {
            let musical_events = track.iter().filter(|event| match event.kind {
                Midi {
                    channel,
                    message: NoteOn { key: _, vel: _ },
                } => channel == c,
                _ => false,
            });
            println!(
                "track {} has {} 'note on' events on channel {}",
                track_num + 1,
                musical_events.count(),
                c + 1
            );
        }
    }

    // Modify the file
    smf.header.format = midly::Format::Sequential;

    // Save it back
    smf.save("rewritten.mid").unwrap();
}

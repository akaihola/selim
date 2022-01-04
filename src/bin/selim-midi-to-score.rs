use std::{env, path::Path};

use selim::score::load_midi_file_note_ons;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = Path::new(&args[1]);
    let score = load_midi_file_note_ons(path, vec![]);

    // Iterate over the events from all tracks:
    println!("time;pitch");
    for note in score.iter() {
        println!("{};{}", note.time.as_micros(), note.pitch);
    }
}

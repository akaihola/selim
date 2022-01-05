use std::error::Error;

use midir::MidiOutputConnection;
use midly::TrackEventKind;
use nodi::{Event, MidiEvent};

use crate::score::ScoreEvent;

pub fn play_midi_event(
    event: &ScoreEvent,
    conn_out: &mut MidiOutputConnection,
) -> Result<Option<MidiEvent>, Box<dyn Error>> {
    if let TrackEventKind::Midi { .. } = event.message {
        let ev = Event::try_from(event.message)?;
        if let Event::Midi(midi_event) = ev {
            let mut message = Vec::with_capacity(4);
            let _ = midi_event.write(&mut message);
            conn_out.send(&message)?;
            return Ok(Some(midi_event));
        }
    }
    Ok(None)
}

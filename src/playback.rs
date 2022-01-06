use std::{error::Error, time::Duration};

use midir::MidiOutputConnection;
use midly::MidiMessage::NoteOn;
use midly::TrackEventKind;
use nodi::{Event, MidiEvent};

use crate::score::{pitch_to_name, ScoreEvent};

pub fn play_past_moments(
    score: &[ScoreEvent],
    head: usize,
    score_calculated_moment: Duration,
    conn_out: &mut MidiOutputConnection,
) -> Result<usize, Box<dyn Error>> {
    let moment_to_play = score[head].time;
    let mut head = head;
    if moment_to_play <= score_calculated_moment {
        loop {
            if head >= score.len() {
                break;
            }
            let score_event = &score[head];
            if score_event.time > moment_to_play {
                break;
            }
            if let TrackEventKind::Midi {
                channel: _,
                message: NoteOn { key, vel },
            } = score_event.message
            {
                println!(
                    "Play score {}: {:.3}, {} {}",
                    head,
                    score_event.time.as_secs_f32(),
                    pitch_to_name(key),
                    vel.as_int(),
                );
            }
            play_midi_event(score_event, conn_out)?;
            head += 1;
        }
    }
    Ok(head)
}

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

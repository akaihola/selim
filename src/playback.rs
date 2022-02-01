use crate::score::{pitch_to_name, ScoreEvent, ZERO_U7};
use midly::{num::u7, MidiMessage::NoteOn, TrackEventKind};
use nodi::Event;
use std::{error::Error, time::Duration};

pub type MidiMessages = Vec<Vec<u8>>;

pub fn play_past_moments(
    score: &[ScoreEvent],
    head: usize,
    score_calculated_moment: Duration,
    velocity: u7,
) -> Result<(MidiMessages, usize), Box<dyn Error>> {
    let moment_to_play = score[head].time;
    let mut head = head;
    let mut buf = MidiMessages::new();
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
                    "Play score {head}: {:.3}, {} {}",
                    score_event.time.as_secs_f32(),
                    pitch_to_name(key),
                    vel.as_int(),
                );
            }
            if let Some(midi_data) = encode_midi_event(score_event, velocity)? {
                buf.push(midi_data);
            }
            head += 1;
        }
    }
    Ok((buf, head))
}

pub fn encode_midi_event(
    event: &ScoreEvent,
    velocity: u7,
) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    if let TrackEventKind::Midi { .. } = event.message {
        let ev = Event::try_from(event.message)?;
        if let Event::Midi(midi_event) = ev {
            let rme = match midi_event.message {
                NoteOn {
                    key: _,
                    vel: ZERO_U7,
                } => midi_event,
                NoteOn { key, vel: _ } => {
                    eprintln!("Velocity {}", velocity.as_int());
                    nodi::MidiEvent {
                        channel: midi_event.channel,
                        message: NoteOn { key, vel: velocity },
                    }
                }
                _ => midi_event,
            };
            let mut message = Vec::with_capacity(4);
            let _ = rme.write(&mut message);
            return Ok(Some(message));
        }
    }
    Ok(None)
}

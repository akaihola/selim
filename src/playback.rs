use crate::score::{pitch_to_name, ScoreEvent, ZERO_U7};
use crate::{algo01_homophonopedantic::MatchPerScore, stretch, LiveVec, Match, ScoreVec};
use anyhow::{bail, Error, Result};
use midly::{num::u7, MidiMessage::NoteOn, TrackEventKind};
use nodi::Event;
use std::time::Duration;

pub type MidiMessages = Vec<Vec<u8>>;

pub fn play_past_moments(
    score: &[ScoreEvent],
    head: usize,
    score_calculated_moment: Duration,
    velocity: u7,
) -> Result<(MidiMessages, usize)> {
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

pub fn encode_midi_event(event: &ScoreEvent, velocity: u7) -> Result<Option<Vec<u8>>> {
    if let TrackEventKind::Midi { .. } = event.message {
        let ev = Event::try_from(event.message).map_err(Error::msg)?;
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

pub fn play_next(
    expect_score: &ScoreVec,
    live: &LiveVec,
    playback_score: &[ScoreEvent],
    head: usize, // index of next score note to be played
    matches: &[MatchPerScore],
    t: Duration, // system time since Unix Epoch
    delay: Duration,
) -> Result<(MidiMessages, usize, Duration)> {
    if head >= playback_score.len() {
        // The playback score has reached end. Only react to live notes from now on.
        return Ok((vec![], head, Duration::from_secs(3600)));
    }

    // Calculate the wall clock time for when to play the next moment in the playback score:
    // - PREV = the last successfully matched live input note
    // - t = wall time now
    // - t_prev = wall time of PREV
    // - ts_prev = score time of PREV
    // - k = stretch factor at PREV
    // - dt = elapsed wall time since PREV
    // - dts = estimated score elapsed time since PREV
    // - ts = estimated score time now
    // - ts_next = score time of next upcoming playback note
    // - dt_next = estimated wait time until next upcoming playback note
    let prev_match = matches
        .last()
        .expect("play_next() needs a non-empty list of matches");
    let t_prev = prev_match.live_time(live)?;
    let ts_prev = prev_match.score_time(expect_score)?;
    let k = prev_match.stretch_factor();
    if t < t_prev {
        let live_note = prev_match.live_note(live)?;
        bail!("Current time {t:?} is earlier than time {t_prev:?} for the previous {prev_match:#?} which points to {live_note:?}");
    }
    let dt = t - t_prev;
    let dts = stretch(dt + delay, 1.0 / k);
    let ts = ts_prev + dts;
    let (buf, new_head) = play_past_moments(playback_score, head, ts, prev_match.live_velocity())?;
    let dt_next = if new_head >= playback_score.len() {
        Duration::from_secs(1)
    } else {
        let ts_next = playback_score[new_head].time;
        if ts_next < ts {
            Duration::from_millis(10)
        } else {
            let dts_next = ts_next - ts;
            stretch(dts_next, k)
        }
    };
    Ok((buf, new_head, dt_next))
}

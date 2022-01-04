use crossbeam_channel::{after, select, Sender};
use midir::MidiOutputConnection;
use midly::live::{LiveEvent, LiveEvent::Midi};
use midly::MidiMessage::NoteOn;
use midly::TrackEventKind;
use selim::cmdline::parse_args;
use selim::device::{open_midi_input, open_midi_output, DeviceSelector};
use selim::score::{load_midi_file, load_midi_file_note_ons, pitch_to_name, ScoreEvent, ScoreNote};
use selim::{follow_score, Match};
use std::boxed::Box;
use std::error::Error;
use std::time::{Duration, SystemTime};

fn main() {
    let (args, device, playback_device) = parse_args();
    let input_score = load_midi_file_note_ons(&args.input_score_file, args.input_channels);
    let playback_score = load_midi_file(&args.playback_score_file, args.output_channels);
    assert!(!input_score.is_empty());
    if let Err(err) = run(device, playback_device, input_score, playback_score) {
        eprintln!("Error: {}", err)
    }
}

fn callback(microsecond: u64, message: &[u8], tx: &mut Sender<ScoreNote>) {
    let event = LiveEvent::parse(message).expect("Unparseable MIDI message");
    if let Midi {
        channel: _,
        message: NoteOn { key, vel: _ },
    } = event
    {
        tx.send(ScoreNote {
            time: Duration::from_micros(microsecond),
            pitch: key,
        })
        .expect("Can't pass on a MIDI message in the internal channel");
    }
}

fn run(
    input_device: DeviceSelector,
    playback_device: DeviceSelector,
    input_score: Vec<ScoreNote>,
    playback_score: Vec<ScoreEvent>,
) -> Result<(), Box<dyn Error>> {
    assert!(!input_score.is_empty());

    let midi_input = open_midi_input(input_device, callback)?;
    let mut conn_out = open_midi_output(playback_device)?;

    let mut live = vec![];
    let mut new_live_index = 0;
    let mut matches = vec![];
    let mut playback_head = 0;
    let mut live_start_time = None;
    let mut system_time_at_last_match = None;
    let mut score_wait = Duration::from_secs(1);
    loop {
        print_expect(&input_score, matches.last());
        if let (Some(prev_match), Some(prev_system_time)) =
            (matches.last(), system_time_at_last_match)
        {
            let (_new_playback_head, _score_wait) = play_next(
                &mut conn_out,
                &input_score,
                &playback_score,
                playback_head,
                *prev_match,
                prev_system_time,
                SystemTime::now(),
            )?;
            playback_head = _new_playback_head;
            score_wait = _score_wait;
        } else {
            println!("no new notes to play")
        }
        select! {
            recv(midi_input.rx) -> note_result => {
                let live_time = match live_start_time {
                    None => {
                        live_start_time = Some(SystemTime::now());
                        Duration::new(0, 0)
                    }
                    Some(earlier) => SystemTime::now().duration_since(earlier).unwrap(),
                };
                let note = note_result?;
                live.push(note);
                let (new_matches, ignored) =
                    follow_score(&input_score, &live, matches.last().cloned(), new_live_index, live_time);
                if !new_matches.is_empty() {
                    system_time_at_last_match = Some(SystemTime::now());
                    matches.extend(new_matches.iter());
                }
                print_got(&live, note, &new_matches, &ignored);
                new_live_index = live.len();
            },
            recv(after(score_wait)) -> _ => {}
        };
    }
}

fn play_midi_event(
    event: &ScoreEvent,
    conn_out: &mut MidiOutputConnection,
) -> Result<Option<nodi::MidiEvent>, Box<dyn Error>> {
    if let TrackEventKind::Midi { .. } = event.message {
        let ev = nodi::Event::try_from(event.message)?;
        if let nodi::Event::Midi(midi_event) = ev {
            let mut message = Vec::with_capacity(4);
            let _ = midi_event.write(&mut message);
            conn_out.send(&message)?;
            return Ok(Some(midi_event));
        }
    }
    Ok(None)
}

fn stretch(duration: Duration, stretch_factor: f32) -> Duration {
    duration * (1000.0 * stretch_factor) as u32 / 1000
}

fn play_next(
    conn_out: &mut MidiOutputConnection,
    input_score: &[ScoreNote],
    score: &[ScoreEvent],
    head: usize, // index of next score note to be played
    prev_match: Match,
    prev_system_time: SystemTime,
    now: SystemTime,
) -> Result<(usize, Duration), Box<dyn Error>> {
    if head >= score.len() {
        return Ok((head, Duration::from_secs(1)));
    }
    let prev_match_time = input_score[prev_match.score_index].time;
    let wall_time_since_prev_match = now.duration_since(prev_system_time).unwrap();
    let score_time_since_prev_match =
        stretch(wall_time_since_prev_match, prev_match.stretch_factor);
    let score_calculated_moment = prev_match_time + score_time_since_prev_match;
    let moment_to_play = score[head].time;
    let mut head = head;
    println!(
        " play {:>3} {:>7.3} next. Could play {:.3}s until {:.3}s at {:3.0}% speed. {:.3}s since previous match at {:.3}s.",
        head,
        moment_to_play.as_secs_f32(),
        score_time_since_prev_match.as_secs_f32(),
        score_calculated_moment.as_secs_f32(),
        100.0 * prev_match.stretch_factor,
        wall_time_since_prev_match.as_secs_f32(),
        prev_match_time.as_secs_f32(),
    );
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
    let wait = if head >= score.len() {
        Duration::from_secs(1)
    } else {
        println!(
            "Score @{} {:.3}s should be ahead of {:.3}s",
            head,
            score[head].time.as_secs_f32(),
            moment_to_play.as_secs_f32()
        );
        let time_to_catch = stretch(score[head].time - moment_to_play, prev_match.stretch_factor);
        if time_to_catch > Duration::ZERO {
            time_to_catch.max(Duration::from_millis(10))
        } else {
            Duration::from_secs(1)
        }
    };
    Ok((head, wait))
}

fn print_expect(input_score: &[ScoreNote], prev_match: Option<&Match>) {
    let score_next = match prev_match {
        Some(Match {
            score_index,
            live_index: _,
            stretch_factor: _,
        }) => score_index + 1,
        _ => 0,
    };
    if score_next < input_score.len() {
        println!(
            "score {:>3} {:>7.3} expect {}",
            score_next,
            input_score[score_next].time.as_secs_f32(),
            pitch_to_name(input_score[score_next].pitch),
        );
    }
}

fn print_got(live: &[ScoreNote], note: ScoreNote, new_matches: &[Match], ignored: &[usize]) {
    println!(
        " live {:>3} {:>7.3}    got {} -> {:?} {:?}",
        live.len() - 1,
        note.time.as_secs_f32(),
        pitch_to_name(note.pitch),
        new_matches
            .iter()
            .map(|m| {
                format!(
                    "{}->{} {}",
                    m.live_index, m.score_index, live[m.live_index].pitch
                )
            })
            .collect::<Vec<_>>(),
        ignored
    );
}

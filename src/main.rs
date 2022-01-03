use midir::MidiOutputConnection;
use midly::live::{LiveEvent, LiveEvent::Midi};
use midly::num::{u4, u7};
use midly::MidiMessage::NoteOn;
use midly::TrackEventKind;
use selim::cmdline::parse_args;
use selim::device::{open_midi_input, open_midi_output, DeviceSelector};
use selim::score::{load_midi_file, load_raw_midi_file, pitch_to_name, ScoreEvent, ScoreNote};
use selim::{follow_score, Match};
use std::boxed::Box;
use std::error::Error;
use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    let (args, device, playback_device) = parse_args();
    let input_score = load_midi_file(&args.input_score_file, &[(1, &[u4::from(0)])]);
    let playback_score = load_raw_midi_file(&args.playback_score_file, &[(2, &[u4::from(1)])]);
    assert!(!input_score.is_empty());
    if let Err(err) = run(device, playback_device, input_score, playback_score) {
        eprintln!("Error: {}", err)
    }
}

fn callback(microsecond: u64, message: &[u8], tx: &mut Sender<ScoreNote>) {
    let event = LiveEvent::parse(message).unwrap();
    if let Midi {
        channel: _,
        message: NoteOn { key, vel: _ },
    } = event
    {
        tx.send(ScoreNote {
            time: Duration::from_micros(microsecond),
            pitch: key,
        })
        .unwrap();
    }
}

fn run(
    device: DeviceSelector,
    playback_device: DeviceSelector,
    input_score: Vec<ScoreNote>,
    playback_score: Vec<ScoreEvent>,
) -> Result<(), Box<dyn Error>> {
    assert!(!input_score.is_empty());

    let midi_input = open_midi_input(device, callback)?;
    let mut conn_out = open_midi_output(playback_device)?;

    let mut live = vec![];
    let mut prev_match = None;
    let mut new_live_index = 0;
    let mut matches = vec![];
    let mut playback_head = 0;
    let mut live_start_time = None;
    let mut system_time_at_last_match = None;
    loop {
        print_expect(&input_score, prev_match);
        if let (Some(p), Some(s)) = (prev_match, system_time_at_last_match) {
            let (_new_playback_head, _score_wait) = play_next(
                &mut conn_out,
                &input_score,
                &playback_score,
                playback_head,
                p,
                s,
                SystemTime::now(),
            );
            playback_head = _new_playback_head;
        } else {
            println!("no new notes to play")
        }
        let note = midi_input.rx.recv()?;
        let live_time = match live_start_time {
            None => {
                live_start_time = Some(SystemTime::now());
                Duration::new(0, 0)
            }
            Some(earlier) => SystemTime::now().duration_since(earlier).unwrap(),
        };
        live.push(note);
        let (new_matches, ignored) =
            follow_score(&input_score, &live, prev_match, new_live_index, live_time);
        if !new_matches.is_empty() {
            system_time_at_last_match = Some(SystemTime::now());
        }
        print_got(&live, note, &new_matches, &ignored);
        matches.extend(new_matches.iter());
        new_live_index = live.len();
        prev_match = matches.last().cloned();
    }
}

fn play_note(score_note: &ScoreEvent, connection: &mut MidiOutputConnection) {
    if let TrackEventKind::Midi { .. } = score_note.message {
        let ev = nodi::Event::try_from(score_note.message).unwrap();
        if let nodi::Event::Midi(midi_event) = ev {
            let mut message = Vec::with_capacity(4);
            let _ = midi_event.write(&mut message);
            connection.send(&message).unwrap();
        }
    }
}

fn play_next(
    conn_out: &mut MidiOutputConnection,
    input_score: &[ScoreNote],
    score: &[ScoreEvent],
    head: usize, // index of next score note to be played
    prev_match: Match,
    prev_system_time: SystemTime,
    now: SystemTime,
) -> (usize, Duration) {
    if head >= score.len() {
        return (head, Duration::from_secs(1));
    }
    let prev_match_time = input_score[prev_match.score_index].time;
    let wall_time_since_prev_match = now.duration_since(prev_system_time).unwrap();
    let score_time_since_prev_match =
        (1000.0 * prev_match.stretch_factor) as u32 * wall_time_since_prev_match / 1000;
    let score_now = prev_match_time + score_time_since_prev_match;
    let timestamp = score[head].time;
    let mut head = head;
    println!(
        "Now {:>7.3}, {:.3}s since previous match at {:.3}s. Score can play up to {:.3}s until {:.3}s at {:3.0}% speed. Next {:.3}s.",
        now.duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f32()),
        wall_time_since_prev_match.as_secs_f32(),
        prev_match_time.as_secs_f32(),
        score_time_since_prev_match.as_secs_f32(),
        score_now.as_secs_f32(),
        100.0 * prev_match.stretch_factor,
        timestamp.as_secs_f32(),
    );
    if timestamp <= score_now {
        loop {
            if head >= score.len() {
                break;
            }
            let score_event = &score[head];
            if score_event.time > timestamp {
                break;
            }
            if let TrackEventKind::Midi {
                channel: _,
                message: NoteOn { key, vel: _ },
            } = score_event.message
            {
                println!(
                    "Play score {}: {:>7.3}, {}",
                    head,
                    score_event.time.as_secs_f32(),
                    pitch_to_name(key)
                );
            }
            play_note(score_event, conn_out);
            head += 1;
        }
    }
    let wait = if head >= score.len() {
        Duration::from_secs(1)
    } else {
        println!(
            "Score @{} {:>7.3}s should be ahead of {:>7.3}s",
            head,
            score[head].time.as_secs_f32(),
            timestamp.as_secs_f32()
        );
        (score[head].time - timestamp).min(Duration::from_millis(1)) // time to wait for next event
    };
    (head, wait)
}

fn print_expect(input_score: &[ScoreNote], prev_match: Option<Match>) {
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

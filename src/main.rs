use crossbeam_channel::{after, select, Sender};
use midir::MidiOutputConnection;
use midly::live::{LiveEvent, LiveEvent::Midi};
use midly::MidiMessage::NoteOn;

use selim::cleanup::{attach_ctrl_c_handler, handle_ctrl_c};
use selim::cmdline::parse_args;
use selim::device::{open_midi_input, open_midi_output, DeviceSelector};
use selim::playback::play_next_moment;
use selim::score::{load_midi_file, load_midi_file_note_ons, pitch_to_name, ScoreEvent, ScoreNote};
use selim::{follow_score, Match};
use std::boxed::Box;
use std::error::Error;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

fn main() {
    let caught_ctrl_c = attach_ctrl_c_handler();
    let (args, device, playback_device) = parse_args();
    let input_score = load_midi_file_note_ons(&args.input_score_file, args.input_channels);
    let playback_score = load_midi_file(&args.playback_score_file, args.output_channels);
    assert!(!input_score.is_empty());
    if let Err(err) = run(
        device,
        playback_device,
        input_score,
        playback_score,
        caught_ctrl_c,
    ) {
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
    caught_ctrl_c: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>> {
    assert!(!input_score.is_empty());

    let midi_input = open_midi_input(input_device, callback)?;
    let mut conn_out = open_midi_output(playback_device)?;

    let mut live = vec![];
    let mut new_live_index = 0;
    let mut matches = vec![];
    let mut playback_head = 0;
    let mut live_start_time = None;
    let mut score_wait = Duration::from_secs(1);
    loop {
        if handle_ctrl_c(&caught_ctrl_c, &mut conn_out) {
            return Ok(());
        }
        print_expect(&input_score, &matches.last());
        if let (Some(_), Some(_live_start)) = (matches.last(), live_start_time) {
            let now = match live_start_time {
                None => Duration::ZERO,
                Some(earlier) => SystemTime::now().duration_since(earlier).unwrap(),
            };
            let (_new_playback_head, _score_wait) = play_next(
                &mut conn_out,
                &input_score,
                &live,
                &playback_score,
                playback_head,
                &matches,
                now,
            )?;
            playback_head = _new_playback_head;
            score_wait = _score_wait;
        } else {
            println!("no new notes to play")
        }
        select! {
            recv(midi_input.rx) -> note_result => {
                let note = note_result?;
                let live_time = match live_start_time {
                    None => {
                        live_start_time = Some(SystemTime::now() - note.time);
                        Duration::ZERO
                    }
                    Some(earlier) => SystemTime::now().duration_since(earlier).unwrap(),
                };
                live.push(note);
                let (new_matches, ignored) =
                    follow_score(&input_score, &live, matches.last().cloned(), new_live_index, live_time);
                matches.extend(new_matches.iter());
                print_got(&live, note, &new_matches, &ignored);
                new_live_index = live.len();
            },
            recv(after(score_wait)) -> _ => {}
        };
    }
}

fn stretch(duration: Duration, stretch_factor: f32) -> Duration {
    duration * (1000.0 * stretch_factor) as u32 / 1000
}

fn play_next(
    conn_out: &mut MidiOutputConnection,
    expect_score: &[ScoreNote],
    live: &[ScoreNote],
    playback_score: &[ScoreEvent],
    head: usize, // index of next score note to be played
    matches: &[Match],
    now: Duration, // relative to start time of live
) -> Result<(usize, Duration), Box<dyn Error>> {
    if head >= playback_score.len() {
        return Ok((head, Duration::from_secs(1)));
    }
    let prev_match = matches
        .last()
        .expect("play_next() needs a non-empty list of matches");
    let score_time_at_prev_match = expect_score[prev_match.score_index].time;
    println!(
        "  now = {}, last match = {}",
        now.as_secs_f32(),
        live[prev_match.live_index].time.as_secs_f32()
    );
    let wall_time_since_prev_match = now - live[prev_match.live_index].time;
    let score_time_since_prev_match =
        stretch(wall_time_since_prev_match, prev_match.stretch_factor);
    let score_calculated_moment = score_time_at_prev_match + score_time_since_prev_match;
    let prev_moment = playback_score[head].time;
    println!(
        " play {:>3} {:>7.3} next. Could play {:.3}s until {:.3}s at {:3.0}% speed. {:.3}s since previous match at {:.3}s.",
        head,
        prev_moment.as_secs_f32(),
        score_time_since_prev_match.as_secs_f32(),
        score_calculated_moment.as_secs_f32(),
        100.0 * prev_match.stretch_factor,
        wall_time_since_prev_match.as_secs_f32(),
        score_time_at_prev_match.as_secs_f32(),
    );
    let new_head = play_next_moment(playback_score, head, score_calculated_moment, conn_out)?;
    let wait = if new_head >= playback_score.len() {
        Duration::from_secs(1)
    } else {
        println!(
            "Score @{} {:.3}s should be ahead of {:.3}s",
            new_head,
            playback_score[new_head].time.as_secs_f32(),
            prev_moment.as_secs_f32(),
        );
        let time_to_catch = stretch(
            playback_score[new_head].time - prev_moment,
            prev_match.stretch_factor,
        );
        if time_to_catch > Duration::ZERO {
            time_to_catch.max(Duration::from_millis(10))
        } else {
            Duration::from_secs(1)
        }
    };
    Ok((new_head, wait))
}

fn print_expect(input_score: &[ScoreNote], prev_match: &Option<&Match>) {
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

use crossbeam_channel::{after, select, Sender};
use midly::live::{LiveEvent, LiveEvent::Midi};
use midly::MidiMessage::NoteOn;
use selim::algo01_homophonopedantic::{HomophonoPedantic, ScoreFollower};
use selim::cleanup::{attach_ctrl_c_handler, handle_ctrl_c};
use selim::cmdline::parse_args;
use selim::device::{open_midi_input, open_midi_output, DeviceSelector};
use selim::playback::{play_past_moments, MidiMessages};
use selim::score::{load_midi_file, load_midi_file_note_ons, pitch_to_name, ScoreEvent, ScoreNote};
use selim::Match;
use std::boxed::Box;
use std::error::Error;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
        args.delay,
        caught_ctrl_c,
    ) {
        eprintln!("Error: {}", err)
    }
}

fn duration_since_unix_epoch() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System clock error")
}

fn callback(_microsecond: u64, message: &[u8], tx: &mut Sender<(Duration, [u8; 3])>) {
    let t = duration_since_unix_epoch();
    let event = LiveEvent::parse(message).expect("Unparseable MIDI message");
    if let Midi {
        channel: _,
        message: NoteOn { key: _, vel: _ },
    } = event
    {
        tx.send((
            t,
            message
                .try_into()
                .expect("Can't convert MIDI message to array"),
        ))
        .expect("Can't pass on a MIDI message in the internal channel");
    }
}

fn run(
    input_device: DeviceSelector,
    playback_device: DeviceSelector,
    expect_score: Vec<ScoreNote>,
    playback_score: Vec<ScoreEvent>,
    delay: Duration,
    caught_ctrl_c: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>> {
    assert!(!expect_score.is_empty());

    let midi_input = open_midi_input(input_device, callback)?;
    let mut conn_out = open_midi_output(playback_device)?;

    let mut new_live_index = 0;
    let mut playback_head = 0;
    let mut score_wait = Duration::from_secs(1);
    let mut follower = HomophonoPedantic::new(&expect_score);
    let mut buf = MidiMessages::new();
    let mut play = false;
    let mut quit = false;

    loop {
        if play {
            play = false;
            print_expect(&expect_score, &follower.last_match());
            if follower.last_match().is_some() {
                let t = duration_since_unix_epoch();
                let (midi_data, _new_playback_head, _score_wait) = play_next(
                    &expect_score,
                    &follower.live,
                    &playback_score,
                    playback_head,
                    &follower.matches,
                    t,
                    delay,
                )?;
                buf.extend(midi_data);
                playback_head = _new_playback_head;
                score_wait = _score_wait;
            } else {
                println!("no new notes to play")
            }
        }
        for message in &buf {
            conn_out.send(message)?;
        }
        buf.clear();
        if quit {
            return Ok(());
        }
        select! {
            recv(midi_input.rx) -> msg => {
                if let Ok((t, message)) = msg {
                    let event = LiveEvent::parse(&message).expect("Unparseable MIDI message");
                    if let Midi {
                        channel: _,
                        message: NoteOn { key, vel },
                    } = event {
                        let note = ScoreNote {
                            time: t,
                            pitch: key,
                            velocity: vel,
                        };
                        follower.push_live(note);
                        let new_matches_offset = follower.matches.len();
                        let new_ignored_offset = follower.ignored.len();
                        follower.follow_score(new_live_index);
                        print_got(&follower.live, note, &follower.matches[new_matches_offset..], &follower.ignored[new_ignored_offset..]);
                        new_live_index = follower.live.len();
                        play = true;
                    }
                }
            },
            recv(after(score_wait)) -> _ => {
                play = true;
            },
            recv(after(Duration::from_millis(1000))) -> _ => {
                if let Some(midi_reset) = handle_ctrl_c(&caught_ctrl_c) {
                    buf.extend(midi_reset);
                    quit = true;
                }
            },
        };
    }
}

fn stretch(duration: Duration, stretch_factor: f32) -> Duration {
    duration * (1000.0 * stretch_factor) as u32 / 1000
}

fn play_next(
    expect_score: &[ScoreNote],
    live: &[ScoreNote],
    playback_score: &[ScoreEvent],
    head: usize, // index of next score note to be played
    matches: &[Match],
    t: Duration, // system time since Unix Epoch
    delay: Duration,
) -> Result<(MidiMessages, usize, Duration), Box<dyn Error>> {
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
    let t_prev = live[prev_match.live_index].time;
    let ts_prev = expect_score[prev_match.score_index].time;
    let k = prev_match.stretch_factor;
    let dt = t - t_prev;
    let dts = stretch(dt + delay, 1.0 / k);
    let ts = ts_prev + dts;
    let (buf, new_head) = play_past_moments(playback_score, head, ts, prev_match.live_velocity)?;
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

fn print_expect(expect_score: &[ScoreNote], prev_match: &Option<&Match>) {
    let score_next = match prev_match {
        Some(Match {
            score_index,
            live_index: _,
            stretch_factor: _,
            score_velocity: _,
            live_velocity: _,
        }) => score_index + 1,
        _ => 0,
    };
    if score_next < expect_score.len() {
        println!(
            "score {:>3} {:>7.3} expect {}",
            score_next,
            expect_score[score_next].time.as_secs_f32(),
            pitch_to_name(expect_score[score_next].pitch),
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

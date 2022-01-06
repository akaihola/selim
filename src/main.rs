use crossbeam_channel::{after, select, Sender};
use midir::MidiOutputConnection;
use midly::live::{LiveEvent, LiveEvent::Midi};
use midly::MidiMessage::NoteOn;
use selim::cleanup::{attach_ctrl_c_handler, handle_ctrl_c};
use selim::cmdline::parse_args;
use selim::device::{open_midi_input, open_midi_output, DeviceSelector};
use selim::playback::play_past_moments;
use selim::score::{load_midi_file, load_midi_file_note_ons, pitch_to_name, ScoreEvent, ScoreNote};
use selim::{follow_score, Match};
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

fn callback(_microsecond: u64, message: &[u8], tx: &mut Sender<ScoreNote>) {
    let event = LiveEvent::parse(message).expect("Unparseable MIDI message");
    if let Midi {
        channel: _,
        message: NoteOn { key, vel: _ },
    } = event
    {
        tx.send(ScoreNote {
            time: duration_since_unix_epoch(),
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
    let mut score_wait = Duration::from_secs(1);
    loop {
        if handle_ctrl_c(&caught_ctrl_c, &mut conn_out) {
            return Ok(());
        }
        print_expect(&input_score, &matches.last());
        if matches.last().is_some() {
            let now = duration_since_unix_epoch();
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
                let live_time = duration_since_unix_epoch();
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
    t: Duration, // system time since Unix Epoch
) -> Result<(usize, Duration), Box<dyn Error>> {
    if head >= playback_score.len() {
        // The playback score has reached end. Only react to live notes from now on.
        return Ok((head, Duration::from_secs(3600)));
    }

    // Calculate the wall clock time for when to play the next moment in the playback score:
    // - PREV = the last successfully matched live input note
    // - t = wall time now
    // - t_prev = wall time of PREV
    // - ts_prev = score time of PREV
    // - v = stretch factor at PREV
    // - dt = elapsed wall time since PREV
    // - dts = estimated score elapsed time since PREV
    // - ts_next = score time of next upcoming playback note
    // - ts = estimated score time now
    // - t_next = estimated wall time of next upcoming playback note
    let prev_match = matches
        .last()
        .expect("play_next() needs a non-empty list of matches");
    let t_prev = live[prev_match.live_index].time;
    let ts_prev = expect_score[prev_match.score_index].time;
    let v = prev_match.stretch_factor;
    let dt = t - t_prev;
    let dts = stretch(dt, 1.0 / v);
    let ts_next = playback_score[head].time;
    let ts = ts_prev + dts;
    println!(
        "  now = {:.3}, last match = {:.3}",
        t.as_secs_f32(),
        t_prev.as_secs_f32()
    );
    println!(
        " play {:>3} {:>7.3} next. Could play {:.3}s until {:.3}s at {:3.0}% speed. {:.3}s since previous match at {:.3}s.",
        head,
        ts_next.as_secs_f32(),
        dts.as_secs_f32(),
        ts.as_secs_f32(),
        100.0 * prev_match.stretch_factor,
        dt.as_secs_f32(),
        ts_prev.as_secs_f32(),
    );
    let new_head = play_past_moments(playback_score, head, ts, conn_out)?;
    let dt_next = if new_head >= playback_score.len() {
        Duration::from_secs(1)
    } else {
        let ts_next = playback_score[new_head].time;
        println!(
            "dts_next = ts_next:{:.3}s - ts:{:.3}s",
            ts_next.as_secs_f32(),
            ts.as_secs_f32()
        );
        if ts_next < ts {
            Duration::from_millis(10)
        } else {
            let dts_next = ts_next - ts;
            stretch(dts_next, v)
        }
    };
    println!(
        "Score @{} waiting between {:.3}s and {:.3}s for {:.3}s, stretched {:.0}%.",
        new_head,
        ts_next.as_secs_f32(),
        playback_score[new_head].time.as_secs_f32(),
        dt_next.as_secs_f32(),
        100.0 * prev_match.stretch_factor,
    );
    Ok((new_head, dt_next))
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

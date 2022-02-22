use crossbeam_channel::{after, select, Sender};
use midly::{
    live::{LiveEvent, LiveEvent::Midi},
    MidiMessage::NoteOn,
};
use selim::{
    algo01_homophonopedantic::MatchPerScore,
    algo02_polyphonoflex::PolyphonoFlex,
    cleanup::{attach_ctrl_c_handler, handle_ctrl_c},
    cmdline::parse_args,
    device::{open_midi_input, open_midi_output, DeviceSelector},
    playback::{MidiMessages, play_next},
    score::{load_midi_file, load_midi_file_note_ons, pitch_to_name, ScoreEvent, ScoreNote},
    LiveIdx, LiveVec, Match, ScoreFollower, ScoreNoteIdx, ScoreVec,
};
use std::{
    boxed::Box,
    error::Error,
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

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
        eprintln!("Error: {err}")
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
    expect_score: ScoreVec,
    playback_score: Vec<ScoreEvent>,
    delay: Duration,
    caught_ctrl_c: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>> {
    assert!(!expect_score.is_empty());

    let midi_input = open_midi_input(input_device, callback)?;
    let mut conn_out = open_midi_output(playback_device)?;

    let mut new_live_index = 0.into();
    let mut playback_head = 0;
    let mut score_wait = Duration::from_secs(1);
    // let mut follower = HomophonoPedantic::new(&expect_score);
    let mut follower = PolyphonoFlex::new(&expect_score);
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
                    &follower.matches_slice(..),
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
            recv(after(Duration::from_millis(1000))) -> _ => {
                if let Some(midi_reset) = handle_ctrl_c(&caught_ctrl_c) {
                    buf.extend(midi_reset);
                    quit = true;
                }
            },
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
                        follower.follow_score(new_live_index)?;
                        print_got(&follower.live, note, &follower.matches_slice(new_matches_offset..), follower.ignored[new_ignored_offset.into()..].as_raw_slice());
                        new_live_index = follower.live.len().into();
                        play = true;
                    }
                }
            },
            recv(after(score_wait)) -> _ => {
                play = true;
            },
        };
    }
}

fn print_expect(expect_score: &ScoreVec, prev_match: &Option<MatchPerScore>) {
    let score_next: ScoreNoteIdx = match prev_match {
        Some(m) => m.score_index() + ScoreNoteIdx::from(1),
        _ => 0.into(),
    };
    if score_next < expect_score.len() {
        println!(
            "score {:>3} {:>7.3} expect {}",
            usize::from(score_next),
            expect_score[score_next].time.as_secs_f32(),
            pitch_to_name(expect_score[score_next].pitch),
        );
    }
}

fn print_got(
    live: &LiveVec,
    _note: ScoreNote,
    new_matches: &[MatchPerScore],
    _ignored: &[LiveIdx],
) {
    for (_i, new_match) in new_matches.iter().enumerate() {
        let pitch_name = new_match
            .live_pitch(live)
            .map_or("<Err>".to_string(), pitch_to_name);
        eprintln!(
            " live {}/{:.3} {} vel{} -> {}[{}] vel{}, {:.0}%",
            usize::from(new_match.live_index()),
            new_match.live_time(live).unwrap().as_secs_f32(),
            &pitch_name,
            new_match.live_velocity(),
            &pitch_name,
            usize::from(new_match.score_index()),
            new_match.score_velocity(),
            100.0 * new_match.stretch_factor(),
        );
    }
}

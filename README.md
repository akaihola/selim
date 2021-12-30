Selim – a real-time musical score follower toolkit
==================================================

You can provide Selim with

- a MIDI file
  (or text input with millisecond timestamps and integers for MIDI note-on numbers),
  and
- a human performance of the same music
  (on a real-time MIDI device,
  or as text input via a Unix socket, TCP socket, websocket or standard input)

The software will then do its best to figure out
where the player is going in the given MIDI file.

It will be able to output in real time, based on user choices, e.g.:

- a time index in the MIDI file
- events from the original MIDI file, synchronized to the performance
- events from a second MIDI file, synchronized to the performance
- MIDI events from the human performer as pass-through
- ...whatever exciting we'll come up with

The software should be usable as

- a Rust library
- a Python extension
- a command line utility
  communicating via MIDI devices, Unix sockets, websockets, TCP sockets or standard I/O


Examples
--------

    $ cargo run --bin selim-mid-info piece.mid
       Compiling selim v0.1.0 (/home/kaiant/prg/selim)
        Finished dev [unoptimized + debuginfo] target(s) in 0.33s
         Running `target/debug/selim-mid-info 'piece.mid'
    midi file has 1 tracks!
    first track has 16430 events!
    first track has 446 'note on' events on channel 1!


Status and roadmap
------------------

- [x] choose MIDI parser library
- [x] ensure we can get data from MIDI files using the chosen MIDI parser (midly)
- [x] data type for an in-memory reference score (milliseconds and pitches)
- [x] first naïve stateless score follower algorithm `selim-0.1.0`
  - [x] inputs:
    - [x] complete reference score (ms+pitch)
    - [x] complete live input so far (ms+pitch)
    - [x] position of last matching previous input note in the score
    - [x] position of last matching previous input note in the live input
    - [x] position of first new note in the live input
    - [x] time stretch factor at last matching note
  - [x] outputs:
    - [x] reference time index at last new input note (ms)
    - [x] time stretch factor at last new matching note
    - [x] list of ignored new input notes (ms+pitch)
  - [x] support only monophony (order of events matters)
  - [x] ignore unexpected (wrong/extra) notes
  - [x] keep waiting for next correct note
- [ ] unit tests for `selim-0.1.0`
- [ ] function to turn a MIDI file into an in-memory reference score (ms+pitch)
  - [x] use only "note on" events
  - [x] ignore velocity
  - [x] convert time offsets to microseconds (taking tempo changes into account)
  - [x] tool to output reference score on stdout
  - [x] pick one hard-coded track
  - [x] pick one hard-coded channel
  - [ ] pick tracks and channels specified in arguments
- [x] choose MIDI input library
- [x] real-time tool to convert MIDI input into ms+pitch events on stdout
- [x] real-time tool to test out `selim-0.1.x`
  - [x] inputs:
    - [x] reference MIDI file
    - [x] real-time ms+pitch events on stdin
  - [x] outputs on stdout:
    - [x] reference time index at last new input note (ms)
    - [x] reference time stretch factor at last new input note
    - [x] ignored input notes
- [ ] wrong/missed/extra note tolerant score follower algorithm `selim-0.1.1`
  - [ ] match new input notes with future reference notes within a time window
  - [x] jump directly to first matching note
- [ ] time stretch factor adjustment limit in `selim-0.1.2`
- [ ] refine MIDI file interpretation (ms+pitch+vel+dur)
  - [ ] include velocity
  - [ ] convert "note off" events to durations


Resources
---------

- [Playing back a midi with midly: how to organise the ecosystem][playback-ecosystem] (Rust Audio Discourse group)

[playback-ecosystem]: https://rust-audio.discourse.group/t/playing-back-a-midi-with-midly-how-to-organise-the-ecosystem/423
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

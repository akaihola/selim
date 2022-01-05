use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use midir::MidiOutputConnection;
use midly::{
    live::LiveEvent,
    num::{u4, u7},
};

pub fn attach_ctrl_c_handler() -> Arc<AtomicBool> {
    // Attach interrupt handler to catch ctrl-c
    let caught_ctrl_c = Arc::new(AtomicBool::new(false));
    let caught_ctrl_c_clone_for_handler = caught_ctrl_c.clone();
    ctrlc::set_handler(move || {
        if caught_ctrl_c_clone_for_handler.load(Ordering::SeqCst) {
            eprintln!("Multiple ctrl+c caught, force-exiting...");
            std::process::exit(-1);
        }
        eprintln!("Caught interrupt signal, cleaning up...");
        caught_ctrl_c_clone_for_handler.store(true, Ordering::SeqCst);
    })
    .expect("Unable to attach interrupt signal handler");
    caught_ctrl_c
}

pub fn handle_ctrl_c(caught_ctrl_c: &Arc<AtomicBool>, conn_out: &mut MidiOutputConnection) -> bool {
    if caught_ctrl_c.load(Ordering::SeqCst) {
        let cc = midly::MidiMessage::Controller {
            controller: u7::from(120),
            value: u7::from(0),
        };
        for channel in 0..16 {
            let ev = LiveEvent::Midi {
                channel: u4::from(channel),
                message: cc,
            };
            let mut buf = Vec::new();
            ev.write(&mut buf)
                .expect("Can't create All Sound Off MIDI event");
            conn_out
                .send(&buf)
                .expect("Can't send All Sound Off MIDI event");
        }
        println!("received Ctrl+C!");
        true
    } else {
        false
    }
}

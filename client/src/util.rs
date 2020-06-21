// This is taken from https://github.com/fdehau/tui-rs/blob/master/examples/util/event.rs
// minus the exit_key handling, it's MIT licensed and: Copyright (c) 2016 Florian Dehau
use std::io;
use std::sync::mpsc;
use std::thread;

use termion::event::Key;
use termion::input::TermRead;

pub enum Event<I> {
    Input(I),
}

/// A small event handler that wrap termion input events.
pub struct Events {
    rx: mpsc::Receiver<Event<Key>>,
    input_handle: thread::JoinHandle<()>,
}

impl Events {
    pub fn new() -> Events {
        let (tx, rx) = mpsc::channel();
        let input_handle = {
            let tx = tx.clone();
            thread::spawn(move || {
                let stdin = io::stdin();
                for evt in stdin.keys() {
                    if let Ok(key) = evt {
                        if tx.send(Event::Input(key)).is_err() {
                            return;
                        }
                    }
                }
            })
        };
        Events { rx, input_handle }
    }

    pub fn next(&self) -> Result<Event<Key>, mpsc::TryRecvError> {
        self.rx.try_recv()
    }
}

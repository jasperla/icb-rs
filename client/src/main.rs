#[allow(dead_code)]
mod util;
use util::{Event, Events};

#[macro_use]
extern crate clap;
use chrono::{Local, Timelike};
use clap::App;
use crossbeam_utils::thread;
use icb::{packets, Config};
use std::io::{self, Write};
use std::process::exit;
use std::time::Duration;
use termion::cursor::Goto;
use termion::event::Key;
use termion::input::MouseTerminal;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, List, Paragraph, Text, Widget};
use tui::Terminal;
use unicode_width::UnicodeWidthStr;

struct Ui {
    input: String,
    history: Vec<String>,
}

impl Default for Ui {
    fn default() -> Ui {
        Ui {
            input: String::new(),
            history: Vec::new(),
        }
    }
}

fn main() -> Result<(), failure::Error> {
    let clap_yaml = load_yaml!("clap.yml");
    let matches = App::from_yaml(clap_yaml).get_matches();

    let nickname = matches.value_of("nickname").unwrap().to_string();
    let serverip = matches.value_of("hostname").unwrap().to_string();
    let port = value_t!(matches, "port", u16).unwrap_or(7326);

    let config = Config {
        nickname,
        serverip,
        port,
    };

    let (client, mut server) = icb::init(config).unwrap();

    // Configure the terminal...
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // ...and event handlers...
    let events = Events::new();

    // ...and finally create the default UI state
    let mut ui = Ui::default();

    thread::scope(|s| {
        s.spawn(|_| {
            server.run();
        });

        loop {
            // Handle any communication with the backend before drawing the next screen.
            if let Ok(m) = client.msg_r.try_recv() {
                let now = Local::now();
                let ts = format!("{:02}:{:02}", now.hour(), now.minute());

                let packet_type = m[0].chars().next().unwrap();
                match packet_type {
                    packets::T_OPEN => ui.history.push(format!("{} <{}> {}", ts, m[1], m[2])),
                    packets::T_PERSONAL => ui.history.push(format!("{} **{}** {}", ts, m[1], m[2])),
                    packets::T_PROTOCOL => ui
                        .history
                        .push(format!("==> Connected to {} on {}", m[2], m[1])),
                    packets::T_STATUS => match m[1].as_str() {
                        "Arrive" | "Boot" | "Depart" | "Help" | "Name" | "No-Beep" | "Notify"
                        | "Sign-off" | "Sign-on" | "Status" | "Topic" | "Warning" => {
                            ui.history.push(format!("{}: {} ", ts, m[2]))
                        }
                        _ => ui.history.push(format!(
                            "=> Message '{}' received in unknown category '{}'",
                            m[2], m[1]
                        )),
                    },
                    // XXX: should handle "\x18eNick is already in use\x00" too
                    _ => ui.history.push(format!("msg_r: {} read: {:?}", ts, m)),
                }
            }
            std::thread::sleep(Duration::from_millis(1));

            terminal
                .draw(|mut f| {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .horizontal_margin(1)
                        .constraints(
                            [
                                Constraint::Length(1),
                                Constraint::Min(1),
                                Constraint::Length(3),
                            ]
                            .as_ref(),
                        )
                        .split(f.size());

                    let help_message = format!("Group: 1, Topic: carrots and peas");
                    Paragraph::new([Text::raw(help_message)].iter()).render(&mut f, chunks[0]);
                    Paragraph::new([Text::raw(&ui.input)].iter())
                        .style(Style::default().fg(Color::Yellow))
                        .block(Block::default().borders(Borders::TOP))
                        .render(&mut f, chunks[2]);
                    // XXX: Only the last items that fit onto the screen
                    // XXX: using pageup/pagedown should allow for scrolling through
                    //      the history too.
                    let history = ui.history.iter().map(|i| Text::raw(format!("{}", i)));
                    List::new(history)
                        .block(Block::default().borders(Borders::TOP))
                        .render(&mut f, chunks[1]);
                })
                .expect("Failed to draw UI to terminal");

            let termsize = terminal.size().unwrap();
            // Put the cursor back inside the input box
            write!(
                terminal.backend_mut(),
                "{}",
                Goto(2 + ui.input.width() as u16, termsize.height - 1)
            )
            .expect("Failed to position cursor");
            io::stdout().flush().ok();

            // Now read the user input, these could be control actions such as backspace,
            // commands (starting with '/') or actual messages intended for other users.
            match events.next().expect("Failed to read user input") {
                Event::Input(input) => match input {
                    Key::Backspace => {
                        ui.input.pop();
                    }
                    Key::Ctrl(c) => match c {
                        'w' => ui.input.clear(),
                        'a' => {} // XXX move cursor to beginning
                        'e' => {} // XXX move cursor to end.
                        _ => {}
                    },
                    Key::Char('\n') => {
                        match ui.input.chars().next() {
                            Some(v) if v == '/' => {
                                if ui.input == "/quit" {
                                    // Use a hammer to quit, ICB doesn't provide a clean way to
                                    // disconnect anyway other than terminating the conneciton.
                                    io::stdout().flush().ok();
                                    exit(0);
                                }
                                // XXX: Handle other commands here
                            }
                            _ => ui.history.push(ui.input.drain(..).collect()),
                        }
                    }
                    Key::Char(c) => {
                        ui.input.push(c);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    })
    .unwrap();

    Ok(())
}

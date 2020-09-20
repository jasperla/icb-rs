mod input;
mod tab;
mod tailview;
#[allow(dead_code)]
mod util;
use util::{Event, Events};

#[macro_use]
extern crate clap;
use chrono::{Local, Timelike};
use clap::App;
use crossbeam_utils::thread;
use icb::{packets, Command, Config};
use std::io::{self, Write};
use std::sync::mpsc::TryRecvError;
use std::time::Duration;
use termion::clear;
use termion::cursor::Goto;
use termion::event::Key;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::widgets::{Block, Borders, Paragraph, Text, Widget};
use tui::Terminal;

use input::History;
use tab::{ChatType, Tabs};

struct Ui {
    input: History,
    views: Tabs,
}

impl Default for Ui {
    fn default() -> Ui {
        Ui {
            input: History::new(),
            // Tabs for channels and personal chats
            views: Tabs::new(),
        }
    }
}

/// Create a timestamp for 'now', returned as 'HH:MM'.
fn timestamp() -> String {
    let now = Local::now();
    format!("{:02}:{:02}", now.hour(), now.minute())
}

fn main() -> Result<(), failure::Error> {
    let clap_yaml = load_yaml!("clap.yml");
    let matches = App::from_yaml(clap_yaml).get_matches();

    let nickname = matches.value_of("nickname").unwrap().to_string();
    let serverip = matches.value_of("hostname").unwrap().to_string();
    let port = value_t!(matches, "port", u16).unwrap_or(7326);
    let group = matches.value_of("group").unwrap().to_string();

    let config = Config {
        nickname,
        serverip,
        port,
        group: group.clone(),
    };

    let (mut client, mut server) = icb::init(config).unwrap();

    // Configure the terminal...
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // ...and event handlers...
    let events = Events::new();

    // ...and finally create the default UI state
    let mut ui = Ui::default();

    println!("{}", clear::All);

    thread::scope(|s| {
        let server_handle = s.spawn(|_| {
            server.run();
        });

        let mut done = false;
        let mut termsize = terminal.size().unwrap();
        while !done {
            // Check if the terminal has resized and redraw if needed
            let newtermsize = terminal.size().unwrap();
            let mut redraw = newtermsize != termsize;
            // Capture new terminal size
            termsize = newtermsize;

            // Handle any communication with the backend before drawing the next screen.
            if let Ok(m) = client.msg_r.try_recv() {
                redraw = true;
                let packet_type = m[0].chars().next().unwrap();
                match packet_type {
                    packets::T_OPEN => ui.views.add_message(
                        ChatType::Open(group.clone()),
                        format!("{} <{}> {}", timestamp(), m[1], m[2]),
                    ),
                    packets::T_PERSONAL => ui.views.add_message(
                        ChatType::Personal(m[1].clone()),
                        format!("{} <{}> {}", timestamp(), m[1], m[2]),
                    ),
                    packets::T_PROTOCOL => ui
                        .views
                        .add_status(format!("==> Connected to {} on {}", m[2], m[1])),
                    packets::T_STATUS => match m[1].as_str() {
                        "Arrive" | "Boot" | "Depart" | "Help" | "Name" | "No-Beep" | "Notify"
                        | "Sign-off" | "Sign-on" | "Status" | "Topic" | "Warning" => {
                            ui.views.add_message(
                                ChatType::Open(group.clone()),
                                format!("{}: {} ", timestamp(), m[2]),
                            )
                        }

                        _ => ui.views.add_status(format!(
                            "=> Message '{}' received in unknown category '{}'",
                            m[2], m[1]
                        )),
                    },
                    packets::T_BEEP => ui.views.add_message(
                        ChatType::Personal(m[1].clone()),
                        format!("{} <{}> *beeps you*", timestamp(), m[1]),
                    ),
                    // XXX: should handle "\x18eNick is already in use\x00" too
                    _ => ui
                        .views
                        .add_status(format!("msg_r: {} read: {:?}", timestamp(), m)),
                }
                .ok();
            }

            // Now read the user input, these could be control actions such as backspace,
            // commands (starting with '/') or actual messages intended for other users.
            loop {
                match events.next() {
                    Ok(Event::Input(input)) => {
                        redraw = true;
                        match input {
                            Key::Backspace => {
                                ui.input.backspace();
                            }
                            Key::Delete => {
                                ui.input.delete();
                            }
                            Key::Ctrl(c) => match c {
                                // Backspace over one word
                                'w' => ui.input.backspace_word(),
                                // Move the cursor to the beginning of the line
                                'a' => ui.input.move_to_start(),
                                // Move the cursor to the end of the line
                                'e' => ui.input.move_to_end(),
                                // Cycle through tabs
                                'n' => ui.views.next(),
                                'p' => ui.views.previous(),
                                _ => {}
                            },
                            Key::Up => {
                                // Decrement history
                                ui.input.prev();
                            }
                            Key::Down => {
                                // Increment history
                                ui.input.next();
                            }
                            Key::Left => {
                                ui.input.move_left(1);
                            }
                            Key::Right => {
                                ui.input.move_right(1);
                            }
                            Key::Char('\n') => {
                                let line = ui.input.get_string();
                                ui.input.new_line();
                                match line.chars().next() {
                                    Some(v) if v == '/' => {
                                        let input: Vec<_> = line.split_whitespace().collect();
                                        let cmd = input[0];

                                        if cmd == "/quit" {
                                            io::stdout().flush().ok();
                                            client.cmd_s.send(Command::Bye).unwrap();
                                            done = true;
                                        } else if (cmd == "/msg" || cmd == "/m") && input.len() > 2
                                        {
                                            let recipient = input[1];

                                            // Now take the text the user has entered and remove the first
                                            // occurences of the command and recipient. We explicitly don't
                                            // use `input` as we may lose any duplicate whitespace the sender has
                                            // inserted, but remove the space after the recipient name.
                                            let msg_text = line.replacen(cmd, "", 1).replacen(
                                                format!(" {} ", recipient).as_str(),
                                                "",
                                                1,
                                            );
                                            let msg = Command::Personal(
                                                recipient.to_string().clone(),
                                                msg_text.clone(),
                                            );
                                            client.cmd_s.send(msg).unwrap();

                                            ui.views
                                                .add_message(
                                                    ChatType::Personal(recipient.to_string()),
                                                    format!("{}: {}", timestamp(), msg_text),
                                                )
                                                .ok();

                                            ui.views.switch_to(ChatType::Personal(
                                                recipient.to_string(),
                                            ));
                                        } else if cmd == "/beep" && input.len() == 2 {
                                            let recipient = input[1];

                                            let msg = Command::Beep(recipient.to_string());
                                            client.cmd_s.send(msg).unwrap();

                                            ui.views
                                                .add_message(
                                                    ChatType::Personal(recipient.to_string()),
                                                    format!(
                                                        "{}: *beep beep, {}*",
                                                        timestamp(),
                                                        recipient
                                                    ),
                                                )
                                                .ok();
                                        } else if (cmd == "/name" || cmd == "/nick")
                                            && input.len() == 2
                                        {
                                            let newname = input[1];

                                            let msg = Command::Name(newname.to_string());
                                            client.cmd_s.send(msg).unwrap();
                                            client.nickname = newname.to_string();
                                        }
                                    }
                                    _ => {
                                        let msg_text = line;

                                        client
                                            .cmd_s
                                            .send(ui.views.command_for_current(&msg_text))
                                            .unwrap();

                                        // Send our own messages into the history as well as the server
                                        // won't echo them back to us.
                                        ui.views
                                            .add_current(format!("{}: {}", timestamp(), msg_text))
                                            .ok();
                                    }
                                }
                            }
                            Key::Char(c) => {
                                ui.input.insert(c);
                            }
                            Key::PageUp => {
                                ui.views.scroll_up(termsize);
                            }
                            Key::PageDown => {
                                ui.views.scroll_down(termsize);
                            }
                            _ => {}
                        }
                    }
                    Err(TryRecvError::Disconnected) => {
                        panic!("Input Event channel closed");
                    }
                    Err(TryRecvError::Empty) => {
                        break;
                    }
                }
            }

            // Redraw if we need to
            if redraw {
                let (input_str, input_cursor) = ui.input.view(termsize.width as usize);
                terminal
                    .draw(|mut f| {
                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .horizontal_margin(1)
                            .constraints(
                                [
                                    Constraint::Length(2),
                                    Constraint::Min(1),
                                    Constraint::Length(3),
                                ]
                                .as_ref(),
                            )
                            .split(f.size());

                        // XXX: Keep track of the current group and topic
                        ui.views.draw_titles(&mut f, chunks[0]);
                        ui.views.draw_current(&mut f, chunks[1]);

                        Paragraph::new([Text::raw(input_str)].iter())
                            .block(Block::default().borders(Borders::TOP))
                            .render(&mut f, chunks[2]);
                    })
                    .expect("Failed to draw UI to terminal");

                // Put the cursor back inside the input box
                write!(
                    terminal.backend_mut(),
                    "{}",
                    Goto(2 + input_cursor as u16, termsize.height - 1)
                )
                .expect("Failed to position cursor");
                io::stdout().flush().ok();
            } else {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        server_handle.join().unwrap();
    })
    .unwrap();

    Ok(())
}

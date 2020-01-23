use icb::{packets, Config};

#[macro_use]
extern crate clap;
use clap::App;
use chrono::{Local, Timelike};
use crossbeam_utils::thread;
use std::time::Duration;

fn main() {
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

    thread::scope(|s| {
        s.spawn(|_| {
            server.run();
        });

        s.spawn(|_| loop {
            if let Ok(m) = client.msg_r.try_recv() {
                let now = Local::now();
                let ts = format!("{:02}:{:02}", now.hour(), now.minute());

                let packet_type = m[0].chars().next().unwrap();
                match packet_type {
                    packets::T_OPEN => println!("{} <{}> {}", ts, m[1], m[2]),
                    packets::T_PERSONAL => println!("{} **{}** {}", ts, m[1], m[2]),
                    packets::T_PROTOCOL => println!("==> Connected to {} on {}", m[2], m[1]),
                    packets::T_STATUS => match m[1].as_str() {
                        "Arrive" | "Boot" | "Depart" | "Help" | "Name" | "No-Beep" | "Notify"
                        | "Sign-off" | "Sign-on" | "Status" | "Topic" | "Warning" => {
                            println!("{}: {} ", ts, m[2])
                        }
                        _ => println!(
                            "=> Message '{}' received in unknown category '{}'",
                            m[2], m[1]
                        ),
                    },
                    _ => println!("msg_r: {} read: {:?}", ts, m),
                }
            }

            std::thread::sleep(Duration::from_millis(1));
        });
    })
    .unwrap();
}

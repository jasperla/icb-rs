use icb::{packets, Config};

use chrono::{Local, Timelike};
use crossbeam_utils::thread;
use std::time::Duration;

fn main() {
    let config = Config {
        nickname: String::from("jasper"),
        serverip: "192.168.115.247",
        port: 7326,
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
                    packets::T_STATUS => {
                        match m[1].as_str() {
                            "Status" | "Name" => println!("{}: {} ", ts, m[2]),
                            _ => {
                                // What categories other than "Status" can we expect?
                                println!("=> {}", m[2])
                            }
                        }
                    }
                    _ => println!("msg_r: {} read: {:?}", ts, m),
                }
            }

            std::thread::sleep(Duration::from_millis(1));
        });
    })
    .unwrap();
}

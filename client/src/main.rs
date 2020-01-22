use icb::Config;

use crossbeam_utils::thread;
use std::time::Duration;

fn main() {
    let config = Config {
        nickname: String::from("jasper"),
        serverip: "192.168.115.247",
        port: 7326,
    };

    let (client, mut server) = icb::init(config).unwrap();

    println!("Entering thread loop from client");
    thread::scope(|s| {
        s.spawn(|_| {
            server.run();
        });

        s.spawn(|_| loop {
            if let Ok(m) = client.msg_r.try_recv() {
                println!("msg_r: read: {:?}", m)
            }

            std::thread::sleep(Duration::from_millis(1));

            //println!("cmd_s: Sending Bye");
            //client.cmd_s.send(icb::Command::Bye).unwrap();
        });
    })
    .unwrap();
}

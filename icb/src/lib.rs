use crossbeam_channel::{unbounded, Receiver, Sender};
use crossbeam_utils::thread;
use std::collections::HashMap;
use std::io::prelude::*;
use std::io::ErrorKind;
use std::net::{Shutdown, TcpStream};

use std::time::Duration;

pub mod packets;
pub type Icbmsg = Vec<String>;

// Session parameters provided by client
#[derive(Debug)]
pub struct Config {
    pub serverip: &'static str,
    pub nickname: String,
    pub port: u16,
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Bye,
}

#[derive(Debug)]
pub struct Client {
    pub nickname: String,
    pub cmd_s: Sender<Command>,
    pub msg_r: Receiver<Icbmsg>,
}

#[derive(Debug)]
pub struct Server {
    hostname: String,
    port: u16,
    sock: Option<TcpStream>,
    cmd_r: Receiver<Command>,
    msg_s: Sender<Icbmsg>,
    nickname: String,
}

impl Server {
    fn new(
        hostname: &str,
        port: u16,
        nickname: &str,
        cmd_r: Receiver<Command>,
        msg_s: Sender<Icbmsg>,
    ) -> Server {
        Server {
            hostname: hostname.to_string(),
            port,
            cmd_r,
            msg_s,
            nickname: nickname.to_string(),
            sock: None,
        }
    }

    // Read a buffer's worth of data from the TcpStream and dispatch it to the
    // correct parser.
    // If the caller expects a packet of certain type it is provided through `expected`.
    fn read(&mut self, expected: Option<char>) -> Result<HashMap<&str, String>, std::io::Error> {
        // Allocate a buffer large enough to hold the maximum ICB packet.
        let mut buffer = [0; 254];

        // Read the data from the network
        let nbytes = self.sock.as_ref().unwrap().read(&mut buffer)?;
        if nbytes == 0 {
            return Ok(HashMap::new());
        }

        // Copy as much data from the network buffer into a vector as the server said
        // it would send. This is indicated by the first byte of the packet.
        let packet_len = buffer[0] as usize;
        println!("packet_len: {}", packet_len);
        let mut message = Vec::<u8>::with_capacity(packet_len - 1);

        // Copy all packet_len bytes (including trailing NUL)
        for (idx, v) in buffer.iter().enumerate() {
            if idx == 0 {
                // Skip the first byte (packet length)
                continue;
            } else if idx == packet_len + 1 {
                break;
            }

            message.push(*v);
        }

        println!("message: {:?}", message);

        let packet_type_byte = message[0] as char;

        match expected {
            Some(t) if (packet_type_byte == t) => {
                // Caller was expecting a particular type, let's see if we have that.
                println!("OK! Expected packet of type {} was received", t);
            }
            Some(t) => {
                println!("FAIL! You expected {} but I got {}", t, packet_type_byte);
                return Err(std::io::Error::new(
                    ErrorKind::NotFound,
                    "Packet type not found",
                ));
            }
            _ => {
                println!("OK! Nothing was expected, just carry on. ");
            }
        }

        for packet in &packets::PACKETS {
            println!("Looking for a packet of type: {}", packet_type_byte);
            if packet.packet_type == packet_type_byte {
                println!("Matching packet for {}!", packet_type_byte);
                let data = (packet.parse)(message, packet_len);
                println!("data = {:?}", data);

                return Ok(data);
            }
        }

        Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "Invalid data received from peer of type {}",
                packet_type_byte
            ),
        ))
    }

    pub fn run(&mut self) {
        // Up to this point blocking reads from the network were fine, now we're going to reqeuire
        // non-blocking reads.
        self.sock
            .as_ref()
            .unwrap()
            .set_nonblocking(true)
            .expect("set_nonblocking on socket failed");

        // XXX: thread::scope() really needed here?
        thread::scope(|s| {
            s.spawn(|_| loop {
                // Handle incoming commands sent by the client.
                match self.cmd_r.try_recv() {
                    Ok(m) if m == Command::Bye => {
                        println!("Terminating connection to remote host");
                        self.sock
                            .as_ref()
                            .unwrap()
                            .shutdown(Shutdown::Both)
                            .unwrap();
                        // XXX: Inform client the connection was closed
                        break;
                    }
                    Ok(m) => println!("cmd_r: Received unknown command: {:?}", m),
                    Err(_) => {}
                }

                // Handle incoming ICB packets, based on the type we'll determine
                // how to handle them.
                // For example T_OPEN and T_PERSONAL will be sent to the client.
                if let Ok(v) = self.read(None) {
                    if [packets::T_OPEN, packets::T_PERSONAL]
                        .contains(&v["type"].chars().next().unwrap())
                    {
                        // Use an indirection to prevent mutably borrowing self.msg_s
                        let msg = vec![
                            v["type"].clone(),
                            v["nickname"].clone(),
                            v["message"].clone(),
                        ];
                        self.msg_s.send(msg).unwrap();
                    }
                }

                std::thread::sleep(Duration::from_millis(1));
            });
        })
        .unwrap();
    }

    // Send a login packet with the 'login' command and a default group of '1'.
    // Any other commands are currently not understood by the server implementation.
    // Upon sending the login packet we expect an empty login response.
    // At this point the client and server can start exchanging other types of packets.
    fn login(&mut self) -> std::io::Result<()> {
        let login_packet = (packets::LOGIN.create)(vec![
            self.nickname.as_str(),
            self.nickname.as_str(),
            "1",
            "login",
        ]);

        self.sock
            .as_ref()
            .unwrap()
            .write_all(login_packet.as_bytes())?;

        if self.read(Some(packets::T_LOGIN)).is_err() {
            panic!("Login failed.");
        }

        Ok(())
    }

    pub fn connect(&mut self) -> std::io::Result<()> {
        // TcpStream::connect() returns a Result<TcpStream>; this we can
        // handle with Ok() and Err(). self.sock is defined as an Option<TcpStream>,
        // so we need to wrap the outcome of Ok() with Some() to convert it
        // from a Result<> to an Option<>.
        match TcpStream::connect(format!("{}:{}", &self.hostname, &self.port)) {
            Ok(t) => self.sock = Some(t),
            Err(_) => panic!("Could not connect to {}:{}", &self.hostname, &self.port),
        }

        // At this point we expect a protocol packet.
        if let Ok(v) = self.read(Some(packets::T_PROTOCOL)) {
            println!("protocol packet data: {:?}", v);
            println!(
                "Connected to {}/{}",
                v.get("hostid").unwrap(),
                v.get("clientid").unwrap()
            )
        } else {
            panic!("Expected a protocol packet, which didn't arrive.")
        }

        Ok(())
    }
}

/// Entrypoint for this module; it sets up the `Client` and `Server` structs
/// and establishes a connection to the configured server.
pub fn init(config: Config) -> Result<(Client, Server), std::io::Error> {
    let (msg_s, msg_r) = unbounded();
    let (cmd_s, cmd_r) = unbounded();

    let mut server = Server::new(config.serverip, config.port, &config.nickname, cmd_r, msg_s);
    server.connect()?;
    server.login()?;

    let client = Client {
        nickname: config.nickname,
        cmd_s,
        msg_r,
    };

    Ok((client, server))
}

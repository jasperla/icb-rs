use crossbeam_channel::{unbounded, Receiver, Sender};
use crossbeam_utils::thread;
use std::collections::HashMap;
use std::io::prelude::*;
use std::io::ErrorKind;
use std::net::{Shutdown, TcpStream};
use std::time::Duration;

#[macro_use]
extern crate maplit;

pub mod packets;
mod util;
use util::q;

/// Messages the client needs to format/display to the user.
/// First field is always the packet type (`T_*`), followed
/// by a type-specific order.
pub type Icbmsg = Vec<String>;

/// Session parameters provided by client upon initialization.
#[derive(Debug)]
pub struct Config {
    pub serverip: &'static str,
    pub nickname: String,
    pub port: u16,
}

/// Commands a `Client` can send to the `Server` through the `cmd` channels.
#[derive(Debug, PartialEq)]
pub enum Command {
    /// Terminate the connection to the remote server. ICB doesn't have a way to
    /// perform a clean disconnect other than shutting down the socket.
    Bye,
}

/// Representation of the client/user state.
#[derive(Debug)]
pub struct Client {
    pub nickname: String,
    pub cmd_s: Sender<Command>,
    pub msg_r: Receiver<Icbmsg>,
}

/// Representation of the connection to the remote server.
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

    /// Read a buffer's worth of data from the TcpStream and dispatch it to the
    /// correct parser.
    /// If the caller expects a packet of certain type it is provided through `expected`.
    fn read(&mut self, expected: Option<char>) -> Result<HashMap<&str, String>, std::io::Error> {
        // Allocate a buffer large enough to hold two fully sized maximum ICB packets.
        let mut buffer = [0; 512];

        // Peek at the incoming data; some packets may show up as a single large buffer
        // so we need to look at the size of the packet of the data we received.
        // Then call read_exact() to read that many bytes, parse that data and send it
        // up the stack.
        // We know we won't be reading at the middle of an ICB packet because they are
        // at most 255 bytes in size, our buffer is double that, and we will always start
        // the connection with a valid packet. Therefore a full ICB packet will always
        // fit the buffer wherever it's located.
        let nbytes = self.sock.as_ref().unwrap().peek(&mut buffer)?;
        if nbytes == 0 {
            // Nothing to peek at.
            return Ok(hashmap! {"type" => packets::T_INVALID.to_string()});
        }

        // Look for the beginning of the ICB packet. This is the first non-zero byte in the buffer.
        let mut packet_len = 0;
        for (i, byte) in buffer.iter().enumerate() {
            // Skip over empty bytes; the first byte we encounter is the packet size.
            if *byte != 0 {
                q("Non-zero byte found with position and value", &(i, byte))?;
                packet_len = *byte as usize;
                break;
            }
        }

        // XXX: We need to handle packets of 255 bytes too.
        if packet_len == 0 {
            // Still nothing worthwhile found -- bail out.
            return Ok(hashmap! {"type" => packets::T_INVALID.to_string()});
        }

        // Allocate a new message vector the size of the packet plus the leading size byte
        // (which gets stripped later).
        let mut message = vec![0; packet_len + 1];

        // Now read as much data from the socket as the server has indicated it has sent.
        self.sock.as_ref().unwrap().read_exact(&mut message)?;

        // Remove the packet size which is stored as packet_len already.
        message.remove(0);

        q("received message", &message)?;

        let packet_type_byte = message[0] as char;

        match expected {
            Some(t) if (packet_type_byte == t) => {
                // Caller was expecting a particular type, let's see if we have that.
                q("OK! Received packet of expected type", &t)?;
            }
            Some(t) => {
                q(
                    "FAIL! Mismatch between expectation and result",
                    &(t, packet_type_byte),
                )?;
                return Err(std::io::Error::new(
                    ErrorKind::NotFound,
                    "Packet type not found",
                ));
            }
            _ => {
                q("OK! Nothing was expected, just carry on", &())?;
            }
        }

        q("Looking for a packet of type", &packet_type_byte)?;
        for packet in &packets::PACKETS {
            if packet.packet_type == packet_type_byte {
                let data = (packet.parse)(message, packet_len);
                q("data", &data)?;

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

    /// This is the "main event loop" of the library which starts by setting up the socket as
    /// non-blocking before entering a loop where it looks for incoming commands on `msg_r`
    /// which need to be dealt with. Secondly it looks for any ICB traffic that was received.
    pub fn run(&mut self) {
        // Up to this point blocking reads from the network were fine, now we're going to require
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
                        q("Terminating connection to remote host", &()).unwrap();
                        self.sock
                            .as_ref()
                            .unwrap()
                            .shutdown(Shutdown::Both)
                            .unwrap();
                        // XXX: Inform client the connection was closed
                        break;
                    }
                    Ok(m) => q("cmd_r: Received unknown command: {:?}", &m).unwrap(),
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
                    } else if v["type"].chars().next().unwrap() == packets::T_STATUS {
                        let msg = vec![
                            v["type"].clone(),
                            v["category"].clone(),
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
            q("protocol packet data", &v)?;
            q(
                "connected to",
                &(v.get("hostid").unwrap(), v.get("clientid").unwrap()),
            )?;
            let msg = vec![
                v["type"].clone(),
                v["hostid"].clone(),
                v["clientid"].clone(),
            ];
            self.msg_s.send(msg).unwrap();
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

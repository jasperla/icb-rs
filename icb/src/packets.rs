use std::collections::HashMap;
use std::convert::TryFrom;
use std::str;

use crate::util::q;

/// Not a type indicated by the protocol, but one used in this library.
pub const T_INVALID: char = '0';
pub const T_LOGIN: char = 'a';
pub const T_OPEN: char = 'b';
pub const T_PERSONAL: char = 'c';
pub const T_STATUS: char = 'd';
pub const T_ERROR: char = 'e';
pub const T_COMMAND: char = 'h';
pub const T_PROTOCOL: char = 'j';
pub const T_BEEP: char = 'k';

// Generic packet creator. Should really be trait method...
// That way we can rework all the packets functions below as implementations
// like:
// trait IcbPacket {
//   fn parse();
//   fn create(packet_type: char, fields: Vec<&str>) -> Vec<u8> { etc };
// }
// impl IcbPacket for LoginPacket {
//   pub fn new()
// }
//
// static PACKETS: [&dyn<IcbPacket>] {
//     &LoginPacket,
// }
// What needs to be considered is whether it's a good idea to allocate
// all the different packet types upfront?

fn packet_create(packet_type: char, fields: Vec<&str>) -> Vec<u8> {
    let mut data = fields.join("\x01");
    let dlen = data.len() + 2; // account for the packet type and NUL byte

    // FIXME: Return a Result indicating success? Or an array of packets?
    let plen = match u8::try_from(dlen) {
        Ok(res) => res,
        Err(_) => {
            data.truncate(253); // Leave room for packet type and NUL byte
            255
        }
    };

    let mut v = Vec::<u8>::with_capacity(dlen + 2);
    v.push(plen);
    v.push(packet_type as u8);
    v.extend_from_slice(data.as_bytes());
    v.push(0x00);

    q("Created payload", &v).unwrap();

    v
}

#[allow(unused_variables)]
fn invalid_packet_create(_fields: Vec<&str>) -> Vec<u8> {
    panic!("You're attempting to create a packet you're not allowed to send to a server");
}

#[allow(unused_variables)]
fn invalid_packet_parse(buffer: Vec<u8>, len: usize) -> HashMap<&'static str, String> {
    panic!(
        "You're attempting to parse a packet that is not valid for a server to send to a client"
    );
}

/// A Packet contains an identifier of the packet type and the functions responsible for creating a
/// packet (create) and for parsing one (parse).
pub struct Packet {
    /// Designation of the actual packet type.
    pub packet_type: char,
    /// Parser for a given function, the returned HashMap contains at least one field (`type`)
    /// which is set to the `packet_type`.
    pub parse: fn(Vec<u8>, usize) -> HashMap<&'static str, String>,
    /// Used to create a valid packet with all the provided fields.
    pub create: fn(Vec<&str>) -> Vec<u8>,
}

/// These are all the valid packet types we know of.
pub static PACKETS: [&Packet; 7] = [
    &LOGIN, &PROTOCOL, &STATUS, &OPEN, &PERSONAL, &COMMAND, &BEEP,
];

/// Login packet, used to join the initial channel after connecting
pub static LOGIN: Packet = Packet {
    packet_type: T_LOGIN,
    parse: login_packet_parse,
    create: login_packet_create,
};

fn login_packet_parse(buffer: Vec<u8>, _len: usize) -> HashMap<&'static str, String> {
    // A received login packet should only contain the packet type byte terminated
    // by a NUL.
    assert!(buffer[1] == b'\x00');
    hashmap! { "type" => T_LOGIN.to_string() }
}

fn login_packet_create(fields: Vec<&str>) -> Vec<u8> {
    packet_create(T_LOGIN, fields)
}

/// Protocol packet
pub static PROTOCOL: Packet = Packet {
    packet_type: T_PROTOCOL,
    parse: protocol_packet_parse,
    create: protocol_packet_create,
};

/// Create an iterator over the packet buffer's fields
fn packet_buffer_iter(buffer: &[u8], len: usize) -> impl Iterator<Item = &[u8]> {
    // Create a copy of the message to split at the ^A field separator,
    // note it removes the first byte (packet_type) and the last byte (NUL).
    let message = &buffer[1..len - 1];

    // Split the packet on ^A (Start Of Heading), or ASCII 0x1
    message.split(|sep| *sep == 0x1)
}

fn protocol_packet_parse(buffer: Vec<u8>, len: usize) -> HashMap<&'static str, String> {
    let mut iter = packet_buffer_iter(&buffer, len);

    // Skip the first field (protocol level)
    let _ = iter.next();
    let hostid = str::from_utf8(iter.next().unwrap()).unwrap();
    let clientid = str::from_utf8(iter.next().unwrap()).unwrap();

    hashmap! {
        "type" => T_PROTOCOL.to_string(),
        "hostid" => hostid.to_string(),
        "clientid" => clientid.to_string(),
    }
}

fn protocol_packet_create(fields: Vec<&str>) -> Vec<u8> {
    packet_create(T_PROTOCOL, fields)
}

/// Status packet
pub static STATUS: Packet = Packet {
    packet_type: T_STATUS,
    parse: status_packet_parse,
    create: invalid_packet_create,
};

fn status_packet_parse(buffer: Vec<u8>, len: usize) -> HashMap<&'static str, String> {
    let mut iter = packet_buffer_iter(&buffer, len);

    let category = str::from_utf8(iter.next().unwrap()).unwrap();
    let message = str::from_utf8(iter.next().unwrap()).unwrap();

    hashmap! {
        "type" => T_STATUS.to_string(),
        "category" => category.to_string(),
        "message" => message.to_string(),
    }
}

/// Open packet (normal chats)
pub static OPEN: Packet = Packet {
    packet_type: T_OPEN,
    parse: open_packet_parse,
    create: open_packet_create,
};

fn open_packet_parse(buffer: Vec<u8>, len: usize) -> HashMap<&'static str, String> {
    let mut iter = packet_buffer_iter(&buffer, len);

    let nickname = str::from_utf8(iter.next().unwrap()).unwrap();
    let message = str::from_utf8(iter.next().unwrap()).unwrap();

    hashmap! {
        "type" => T_OPEN.to_string(),
        "nickname" => nickname.to_string(),
        "message" => message.to_string(),
    }
}

fn open_packet_create(fields: Vec<&str>) -> Vec<u8> {
    packet_create(T_OPEN, fields)
}

/// Personal message packet (direct chats)
pub static PERSONAL: Packet = Packet {
    packet_type: T_PERSONAL,
    parse: personal_packet_parse,
    create: invalid_packet_create,
};

fn personal_packet_parse(buffer: Vec<u8>, len: usize) -> HashMap<&'static str, String> {
    let mut iter = packet_buffer_iter(&buffer, len);

    let nickname = str::from_utf8(iter.next().unwrap()).unwrap();
    let message = str::from_utf8(iter.next().unwrap()).unwrap();

    hashmap! {
        "type" => T_PERSONAL.to_string(),
        "nickname" => nickname.to_string(),
        "message" => message.to_string(),
    }
}

/// Command packet
pub static COMMAND: Packet = Packet {
    packet_type: T_COMMAND,
    parse: invalid_packet_parse,
    create: command_packet_create,
};

#[allow(unused_variables)]
/// Create a new command packet. Based on the icbd server implementation the following
/// commands can be issued:
///   "?"      -- help
///   "beep"   -- beep
///   "boot"   -- boot
///   "g"      -- change group
///   "m"      -- personal message
///   "msg"    -- personal message
///   "name"   -- change name
///   "nobeep" -- disable beep
///   "pass"   -- pass moderator
///   "topic"  -- set topic
///   "w"      -- list users
pub const CMD_HELP: &str = "?";
pub const CMD_BEEP: &str = "beep";
pub const CMD_BOOT: &str = "boot";
pub const CMD_G: &str = "g";
pub const CMD_M: &str = "m";
pub const CMD_MSG: &str = "msg";
pub const CMD_NAME: &str = "name";
pub const CMD_NOBEEP: &str = "nobeep";
pub const CMD_PASS: &str = "pass";
pub const CMD_TOPIC: &str = "topic";
pub const CMD_W: &str = "w";

fn command_packet_create(fields: Vec<&str>) -> Vec<u8> {
    let all_cmds = vec![CMD_BEEP, CMD_M, CMD_MSG, CMD_NAME];
    let cmd = fields[0];

    if all_cmds.contains(&cmd) {
        packet_create(T_COMMAND, fields)
    } else {
        panic!("Command {} not support (yet)!", cmd);
    }
}

/// Beep beep
pub static BEEP: Packet = Packet {
    packet_type: T_BEEP,
    parse: beep_packet_parse,
    create: invalid_packet_create,
};

fn beep_packet_parse(buffer: Vec<u8>, len: usize) -> HashMap<&'static str, String> {
    let mut iter = packet_buffer_iter(&buffer, len);

    let nickname = str::from_utf8(iter.next().unwrap()).unwrap();

    hashmap! {
        "type" => T_BEEP.to_string(),
        "nickname" => nickname.to_string(),
    }
}

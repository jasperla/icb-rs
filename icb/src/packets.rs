use std::collections::HashMap;
use std::str;

pub const T_LOGIN: char = 'a';
pub const T_OPEN: char = 'b';
pub const T_PERSONAL: char = 'c';
pub const T_STATUS: char = 'd';
pub const T_ERROR: char = 'e';
pub const T_PROTOCOL: char = 'j';

/// Generic packet creator. Should really be trait method...
/// That way we can rework all the packets functions below as implementations
/// like:
/// trait IcbPacket {
///   fn parse();
///   fn create(packet_type: char, fields: Vec<&str>) -> String { etc };
/// }
/// impl IcbPacket for LoginPacket {
///   pub fn new()
/// }
///
/// static PACKETS: [&dyn<IcbPacket>] {
///     &LoginPacket,
/// }
/// What needs to be considered is whether it's a good idea to allocate
/// all the different packet types upfront?

fn packet_create(packet_type: char, fields: Vec<&str>) -> String {
    let data = fields.join("\x01");
    let dlen = data.len() + 2; // account for the packet type and NUL byte

    assert!(dlen < 255);

    // Rather ugly way to use a variable (dlen) as a raw byte (\x16 or \u{16})
    let payload = format!(
        "{}{}{}\x00",
        str::from_utf8(&[dlen as u8]).unwrap(),
        packet_type,
        data
    );
    println!("Created payload: {:?}", payload);
    payload
}

/// A Packet contains an identifier of the packet type and the functions responsible for creating a
/// packet (create) and for parsing one (parse).
pub struct Packet {
    pub packet_type: char,
    pub parse: fn(Vec<u8>, usize) -> HashMap<&'static str, String>,
    pub create: fn(Vec<&str>) -> String,
}

/// These are all the valid packet types we know of.
pub static PACKETS: [&Packet; 5] = [&LOGIN, &PROTOCOL, &STATUS, &OPEN, &PERSONAL];

pub static LOGIN: Packet = Packet {
    packet_type: T_LOGIN,
    parse: login_packet_parse,
    create: login_packet_create,
};

fn login_packet_parse(buffer: Vec<u8>, _len: usize) -> HashMap<&'static str, String> {
    // A received login packet should only contain the packet type byte terminated
    // by a NUL.
    assert!(buffer[1] == b'\x00');
    let mut packet_data = HashMap::new();
    packet_data.insert("type", T_LOGIN.to_string());
    packet_data
}

fn login_packet_create(fields: Vec<&str>) -> String {
    packet_create(T_LOGIN, fields)
}

/// Protocol packet
static PROTOCOL: Packet = Packet {
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
    let mut packet_data = HashMap::new();

    packet_data.insert("type", T_PROTOCOL.to_string());

    // Skip the first field (protocol level)
    let _ = iter.next();
    let hostid = str::from_utf8(iter.next().unwrap()).unwrap();
    let clientid = str::from_utf8(iter.next().unwrap()).unwrap();

    packet_data.insert("hostid", hostid.to_string());
    packet_data.insert("clientid", clientid.to_string());

    packet_data
}

fn protocol_packet_create(fields: Vec<&str>) -> String {
    packet_create(T_PROTOCOL, fields)
}

/// Status packet
static STATUS: Packet = Packet {
    packet_type: T_STATUS,
    parse: status_packet_parse,
    create: invalid_packet,
};

#[allow(unused_variables)]
fn status_packet_parse(buffer: Vec<u8>, len: usize) -> HashMap<&'static str, String> {
    todo!();
}

fn invalid_packet(_fields: Vec<&str>) -> String {
    panic!("Attempting to create a server-only packet.");
}

/// Open packet (normal chats)
static OPEN: Packet = Packet {
    packet_type: T_OPEN,
    parse: open_packet_parse,
    create: open_packet_create,
};

fn open_packet_parse(buffer: Vec<u8>, len: usize) -> HashMap<&'static str, String> {
    let mut iter = packet_buffer_iter(&buffer, len);
    let mut packet_data = HashMap::new();

    packet_data.insert("type", T_OPEN.to_string());
    packet_data.insert(
        "nickname",
        str::from_utf8(iter.next().unwrap()).unwrap().to_string(),
    );
    packet_data.insert(
        "message",
        str::from_utf8(iter.next().unwrap()).unwrap().to_string(),
    );

    packet_data
}

#[allow(unused_variables)]
fn open_packet_create(fields: Vec<&str>) -> String {
    todo!();
}

/// Personal message packet (direct chats)
static PERSONAL: Packet = Packet {
    packet_type: T_PERSONAL,
    parse: personal_packet_parse,
    create: personal_packet_create,
};

fn personal_packet_parse(buffer: Vec<u8>, len: usize) -> HashMap<&'static str, String> {
    let mut iter = packet_buffer_iter(&buffer, len);
    let mut packet_data = HashMap::new();

    packet_data.insert("type", T_PERSONAL.to_string());
    packet_data.insert(
        "nickname",
        str::from_utf8(iter.next().unwrap()).unwrap().to_string(),
    );
    packet_data.insert(
        "message",
        str::from_utf8(iter.next().unwrap()).unwrap().to_string(),
    );

    packet_data
}

#[allow(unused_variables)]
fn personal_packet_create(fields: Vec<&str>) -> String {
    todo!();
}

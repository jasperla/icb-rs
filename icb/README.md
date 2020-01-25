# icb

[![crates.io](http://meritbadge.herokuapp.com/icb)](https://crates.io/crates/icb)

- [Documentation](https://docs.rs/icb)

Simple and small library for writing ICB clients

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
icb = "0.2"
```

In your `main.rs` or `lib.rs` you'll want to provide the connection parameters through
a `Config` struct and use the `init()` function to create a `Client` and `Server`.

The server-component provides a `run()` function which is its main event loop.
In order for the client to send and receive messages and commands it needs its own loop
and communicate with the server via the `msg_r: Receiver<Icbmsg>` and `cmd_s: Sender<Command>`.
A working example can be found in the [icb-client](https://github.com/jasperla/icb-rs/tree/master/client)
crate.

```rust
use icb::Config;

fn main() {
    let config = Config {
        nickname: String::from("jasper"),
        serverip: "192.168.115.245",
        port: 7326,
	group: "slackers",
    };

    let (client, mut server) = icb::init(config).unwrap();
}
```

Note that the `Server` does not implement an ICB server, it is the component inside the `icb`
library responsible for communicating with the remote server.

## ICB

Protocol documentation for ICB can be found [here](http://www.icb.net/_jrudd/icb/protocol.html).

# icbc, an ICB client

[![crates.io](http://meritbadge.herokuapp.com/icbc)](https://crates.io/crates/icbc)

This is a minimal ICB client with a very basic interface.

## Installation

You can install the latest version with:

```
cargo install icbc
```

or to build from git:

```
cargo install --git https://github.com/jasperla/icb-rs icbc
```

## Usage

```
icbc --hostname server.example.net --group hackers -n ferris
```

## ToDo

There are a lot of things to implement and/or fix before others might consider this usable, such as:
- tab completion on usernames
- support for changing groups, listing users, etc
- highlight on mentions
- and the list goes on..

# 0.x.y (NOT RELEASED YET)

- Do not panic on invalid packets or non-utf8 payloads.

# 0.2.2

- implement sending `beep` commands and receiving `beep` messages
- handle nickname changes through the `name` command
- fix panic when typing messages longer than 127 bytes due to invalid UTF-8 cast of packet length

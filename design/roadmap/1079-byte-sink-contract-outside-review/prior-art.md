# Prior art

An appendix to [the milestone](../1079-byte-sink-contract-outside-review.md).

How the contract sits against what a reader already knows. Figures marked (unverified) come from memory, and this review did not check them.

| system | what a write is | payload per message | end of stream | "reader is gone" versus "never had a destination" | levels in the framing |
|---|---|---|---|---|---|
| Unix `write(2)` on a pipe or socket | a syscall returning bytes written, short writes legal | up to the pipe buffer, 64 KiB on Linux (unverified) | `close(2)` of the last writer; the reader sees a 0-byte read | `EPIPE` plus `SIGPIPE` for a closed reader; `EBADF` for no descriptor | none; syslog(3) and `/dev/kmsg` add `<level>` prefixes in a second protocol |
| Plan 9 | `write` on a file descriptor; a hung-up channel returns an error string | per-channel | `close`; 0-byte read | one error, "Hangup", no signal (unverified) | none |
| seL4 IPC | `seL4_Send` of message registers; the rendezvous is the flow control | `seL4_MsgMaxLength` words, about 120 (unverified) | none; a protocol defines its own | a send to a deleted capability fails silently or returns an error by configuration (unverified) | none |
| Zircon channel and socket | `zx_channel_write`, `zx_socket_write`; `ZX_ERR_PEER_CLOSED` | 64 KiB per channel message (unverified) | `zx_socket_set_disposition` half-close | `PEER_CLOSED` versus `BAD_HANDLE` | none; `debuglog` records carry a severity (§242's table) |
| Fuchsia `LogSink` | a FIDL record per message | 32 KiB (§242's table) | none | FIDL epitaph on channel close | one header word holds type, severity and size (§242's table) |
| Genode LOG session | `write(String)` | 232 bytes (§242's table) | none | the session is a capability; a closed session faults the client | none |
| Rust `std::io::Write`, `embedded-io::Write` | `write(&[u8]) -> Result<usize>`: how many were taken, never a silent drop | caller's buffer | `flush`; no end marker | `ErrorKind::BrokenPipe` versus `Unsupported`; std's `handle_ebadf` swallows only the latter | none |
| `defmt` | a framed log record over any byte transport | frame-dependent | none | transport's | a level tag per record, chosen at compile time (unverified) |
| RFC 5424 syslog | one datagram per message | 2048 bytes minimum (unverified) | none | transport's | PRI holds facility and severity 0 to 7 |

Where this contract differs: the whole message rides in the three words a `SEND` already has, so a pipe costs no frame and no mapping. The opcode for bytes is zero, so a bare length is a valid message. End of stream is an explicit message rather than a close. The writer learns exactly one bit beyond success, whether the sink was ever there. What it costs: 16 bytes per rendezvous, so a pipe is lockstep with no buffer (measured in notes/pipes.md). It also carries no per-write reason. It has no reply, so no negotiation of any kind is possible on the wire, which item 2 and decision A turn on. The
`pack` return of `taken` is `Write::write`'s contract made explicit.

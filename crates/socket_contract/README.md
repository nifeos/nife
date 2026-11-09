# socket_contract

The wire contract between a network stack and its clients, in which each socket is its own
capability: a process holds a front door that can only make sockets, plus one capability per open
socket. `no_std`, no dependencies, no system calls of its own. A network stack and the programs that
talk to it share this one definition, so they cannot drift.

Opening and listening are calls on the front door, answered with the socket's capability. Every
other operation is a call on the socket, with an opcode, one argument and one reply word. The stack
names a socket only by its kernel-stamped badge (`SOCKET_BADGE`), so a closed socket's copies reach
nothing. A port claim is granted when the stack is spawned (`listen_grant`, `udp_bind_grant`) and
never assumed.

## The contract

- `VERSION` (1) is the protocol's version: one number for the operation set and the reply shapes.
  A break bumps it; an addition (a new opcode, an optional field) does not.
- Operations (`OPERATION_*`): attach the shared page frame, open a User Datagram Protocol (UDP)
  socket, open a Transmission Control Protocol (TCP) socket, send a datagram, receive, connect, send, close, listen, accept, and bind a UDP port. The opcode is
  the whole request word.
- Replies: `REP_OK` (0, and receive replies its length instead), `REP_ERR` (bit 32 set, so it is
  never mistaken for a length), `CONNECT_ESTABLISHED` and `CONNECT_REFUSED` for connect, and
  `LISTEN_GRANTED`, `LISTEN_DENIED` and `LISTEN_IN_USE` for a port claim. Denied means the port is
  outside the stack's grant, so do not retry; in use means pick another port.
- Port grants: `listen_grant(lo, hi)` packs an inclusive TCP listen range into the low half of one
  word and `udp_bind_grant(lo, hi)` a UDP range into the high half, and the two compose with `|`.
  `grant_allows` and `udp_grant_allows` are the whole check, and the halves are independent
  authorities. A low bound of 0 or an inverted range grants nothing (`NO_LISTEN_GRANT`).
- The shared frame: a 4 KiB frame with the destination address at `OFF_DST_IP`, the port at
  `OFF_DST_PORT`, the payload length at `OFF_LEN` and up to `DATA_MAXIMUM` payload bytes at
  `OFF_PAYLOAD`.
- `stack_slots` names where a network stack finds the two capabilities it is spawned with.
- `lease` encodes the report a stack sends its spawner when it has an address: the Internet Protocol version 4
  (IPv4) address (`ipv4_word`, `word_ipv4`) and the first nameserver or `NO_NAMESERVER`.
- `ephemeral::Ports` allocates outbound local ports from the private range 49152 to 65535 with a
  cursor that never falls behind a clock advancing 256 ports a second, so a new stack starts past
  ports an earlier one used. `polled_wait::next` decides whether a stack may sleep on a network card
  that raises no interrupt.
- `fixture` holds the fixed addresses and messages of one network stack's test runners. It is not
  part of the contract and a consumer can ignore it.

## EXAMPLES

```rust
use socket_contract::{grant_allows, listen_grant, names_a_socket, udp_bind_grant, udp_grant_allows, SOCKET_BADGE};

// A file server spawned with TCP port 445 and a name responder with UDP port 5353.
let word = listen_grant(445, 445) | udp_bind_grant(5353, 5353);
assert!(grant_allows(word, 445) && udp_grant_allows(word, 5353));
assert!(!grant_allows(word, 5353)); // the same number is not the same authority

assert!(names_a_socket(SOCKET_BADGE | 1));
assert!(!names_a_socket(0)); // an unbadged front door
```

## What is proven

Three Kani harnesses in `src/lib.rs` prove the port grants for every pair of bounds and every port.
A listen grant admits exactly its range and no UDP port, a UDP bind grant admits exactly its range
and no listen port, and each half of a composed grant answers exactly as it does alone. Run them
with `cargo kani`. Tests check the rest of the contract: the opcodes, the reply words and the
frame layout.

Each patch under `falsifications/` names the harness or test it breaks, and applying it with
`patch -p1` from the repository root makes that check fail.

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

The proofs need Kani, which `rustup` does not install. Run `cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step downloads a model
checker, so it needs network access. CI uses version 0.68.0, and another version may verify
differently.

## BUGS

- A grant is the stack's, shared by every holder of its front door; a per-client grant would ride on
  a badged front door, which nothing builds yet.
- A listener's backlog is one connection: a second arriving before `ACCEPT` is refused by TCP.
- `polled_wait` keeps a core busy while a stream flows, and for a tick after it ends.
- No operation carries `VERSION`, so a client and a stack built from different versions are not
  told apart when the client binds.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.

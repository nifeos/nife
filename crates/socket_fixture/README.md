# socket_fixture

Test data for a network stack, in one place so the programs that touch it agree: the port the
inbound gate listens on, the port it probes and must be refused, the two messages, and the
addresses of the QEMU runners' echo peer and package source. `no_std`. Its one dependency is
`socket_contract`, whose grants it is checked against. It stays in nife and is not published.

It is not a wire contract. The host's copy of the inbound gate's facts is spelled again on the host
side on purpose, so the gate proves two independently written sides agree.

## EXAMPLES

```rust
use socket_fixture::{INBOUND_MESSAGE, LISTEN_PORT};

assert_eq!(LISTEN_PORT, 7778);
assert_eq!(INBOUND_MESSAGE, b"nife-in!");
```

## What is proven

Nothing is proven. A test checks that the port the gate probes is one no grant admits.

## BUGS

- None known.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.

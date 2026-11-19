# domain_name_system

[![check](https://github.com/nifeos/domain_name_system/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/domain_name_system/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/domain_name_system/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/domain_name_system/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/domain_name_system#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/domain_name_system/blob/main/Cargo.toml)

The Domain Name System (DNS) wire format a stub resolver needs, as pure computation. Request for Comments (RFC)
1035 defines it, and RFC 7766 defines its use over the Transmission Control Protocol (TCP).
It writes the bytes of a query for one name's Internet Protocol version 4 (IPv4) addresses. It also runs the checks a reply must
pass before the caller believes any address in it. `no_std`, no dependencies, no `unsafe`. There is no socket, no timer and no
policy. Which program sends these bytes, and which names a client may ask about, are the caller's
business.

Every byte parsed here came from a server the caller may not trust, so the parser is total on any
input. `Query::accept` believes as little as it can. It takes only the A records owned by the end of
the canonical name (CNAME) chain that starts at the name asked for, so a reply about `example.com` cannot plant an
address for another name. It reads nothing from the authority and additional sections. `Reject`
lists its nine refusals in the order the checks run.

## EXAMPLES

```rust
use domain_name_system::{Query, Reject};

let query = Query::new(0x4e1f, "packages.example.test").unwrap();
let mut request = [0u8; 512];
let n = query.request(&mut request).unwrap();
assert_eq!(&request[..2], &[0x4e, 0x1f]); // our transaction id

// The reply: our header with QR and RA set, our question back, then one A record.
let mut reply = request[..n].to_vec();
reply[2] |= 0x80; // QR: a response
reply[3] |= 0x80; // RA
reply[7] = 1; // ANCOUNT = 1
reply.extend_from_slice(&[0xc0, 0x0c]); // owner: a pointer to the question's name
reply.extend_from_slice(&[0, 1, 0, 1]); // type A, class IN
reply.extend_from_slice(&300u32.to_be_bytes()); // TTL
reply.extend_from_slice(&[0, 4, 10, 0, 2, 9]); // RDLENGTH 4, then the address

let answer = query.accept(&reply).unwrap();
assert_eq!(answer.ipv4_addresses(), &[[10, 0, 2, 9]]);
assert_eq!(answer.ttl(), 300);

// The same bytes under somebody else's transaction id are not an answer to us.
reply[1] ^= 1;
assert_eq!(query.accept(&reply), Err(Reject::IdMismatch));
```

Over TCP, `Query::request_tcp` writes the two-byte length prefix and `TcpReply` collects a reply from
however many reads it arrives in. Size its buffer for the largest reply you accept, up to
`MAXIMUM_TCP_MESSAGE_BYTES` plus 2. A buffer no larger than `MAXIMUM_UDP_MESSAGE_BYTES` (the User Datagram Protocol limit) cannot hold
the reply that a truncated UDP reply asks for.

## What is proven

Three Kani harnesses in `src/proofs.rs` prove three things:

- `the_name_decoder_is_total_and_in_bounds`: on any bytes and any start offset, the name decoder
  stays inside the message and terminates.
- `the_header_round_trips_on_every_value`: a header written and parsed again is the header.
- `tcp_reassembly_is_total_and_honest`: `TcpReply` is total on any split of its input and refuses a
  reply longer than it announced.

Run them with `cargo kani`. The three take about a minute together on a laptop. Each harness has a
falsification patch in `falsifications/` that applies a defect the harness is there to catch. Apply
one and the harness must fail:

```sh
git apply falsifications/proofs.the_header_round_trips_on_every_value.patch
cargo kani --harness the_header_round_trips_on_every_value    # prints VERIFICATION:- FAILED
git checkout -- src
```

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

The proofs need Kani, which `rustup` does not install. Run `cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step downloads a model checker, so it needs
network access. CI uses version 0.68.0, and another version may verify differently.

The tests in `tests/replies.rs` read three real replies captured from a recursive server. Their
capture command is in `tests/fixtures/README.md`. The crate's own `fuzz/` directory holds a target
that feeds `Query::accept` arbitrary bytes.

## Sizes

`Name` is 256 bytes and `Copy`, and `Reader::record` returns a `Record` by value. So `Query::accept` copies
a name once per record on each of its three passes over the answer section. `Answer` is 296
bytes and `Result<Answer, Reject>` is 300. A caller on a small stack should count them beside its
514-byte buffer.

## BUGS

- IPv4 only: a query asks for address records alone, so it reads no address longer than four bytes.
- No Extension Mechanisms for DNS (EDNS) version 0, so UDP replies stop at `MAXIMUM_UDP_MESSAGE_BYTES` and a larger answer costs a TCP
  retry.
- No DNS Security Extensions (DNSSEC): an on-path attacker can forge the reply, transaction id and all. No "0x20" case
  randomization.
- `Query::accept` keeps at most `MAXIMUM_IPV4_ADDRESSES` addresses from one answer and drops the rest.
- Labels hold only letters, digits, `-` and `_` (`Name::from_dotted`): no escape syntax.

## Minimum Rust version

Rust 1.88, the floor that `as_chunks_mut` sets.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.

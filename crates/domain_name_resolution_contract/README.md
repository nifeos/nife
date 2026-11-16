# domain_name_resolution_contract

The contract between a confined name resolver, the program that spawns it, and the clients that ask
it for Internet Protocol version 4 (IPv4) addresses. The resolver is a program of its own. It holds the one capability that can reach a
name server, and it answers a client only for names inside the zone that client was granted. This
crate holds what the three parties agree on: the words each one sends, the page a name and its
addresses cross in, and the grant table the resolver judges each name against. `no_std`, and its
one dependency is `domain_name_system`, whose `Name` a zone is.

On its own the crate sends nothing and maps nothing. It is useful with a resolver program, a
spawner that grants zones, and a client library, which all link this one definition so they cannot
drift. The client half (`wire.rs`) names nothing outside itself, so a standard library that cannot
depend on this crate can copy that one file verbatim.

## The contract

### The three parties

```text
  spawner ──unbadged capability──┐     (GRANT: badge B may resolve names in zone Z)
                                 ▼
  client ──capability, badge B──► resolver ──socket──► network stack ──► a name server
  (ATTACH a page, then RESOLVE)
```

The spawner holds the one unbadged capability to the resolver's endpoint. It gives each client a
copy carrying that client's badge. The resolver reads only badge 0's messages as grants, so a client
cannot widen its own grant.

### The sequence

1. The spawner grants badge B a zone with `grant_messages(B, zone)`. A zone's text travels eight
   bytes per message, after a `control_word` that names the operation, the offset, the total length
   and the badge. The spawner `CALL`s the last piece, and the reply says whether the grant is in
   force (below).
2. The client delegates one writable page with `OPERATION_ATTACH_PAGE_FRAME`. The resolver maps it
   in the window its badge's grant names, or deletes it if the badge holds no grant.
3. The client writes a host name at `NAME_OFFSET_BYTES` and `CALL`s `request(OPERATION_RESOLVE)`
   with the name's length as the second word.
4. The resolver checks the name against the badge's zone (`Grants::check`). Only then does it ask
   the name server. It writes the addresses into the page and replies.
5. When a client ends, the spawner sends `forget_message(B)`. The badge is then denied every name,
   the resolver gives up its page, and the window is free for the next grant.

### The page

| bytes | field | written by |
|---|---|---|
| `NAME_OFFSET_BYTES` (0) | the host name, dotted ASCII, at most `MAXIMUM_NAME_FIELD_BYTES` (254) bytes, a trailing dot included | the client |
| `IPV4_ADDRESSES_OFFSET_BYTES` (0x100) | up to `MAXIMUM_IPV4_ADDRESSES` (8) IPv4 addresses, four bytes each, in the name server's order | the resolver |

### The reply

A resolve's reply is two words. The first is an `Outcome`: `status | detail << 8 | count << 16`.
The second is the answer's time to live in seconds. Each `status` is a distinct reason,
because "no such name" is a different fact from "you may not ask":

- `OK`: `count` addresses are in the page.
- `DENIED`: the badge holds no grant, or the name is outside its zone. Nothing was sent.
- `NO_SUCH_NAME`, `REFUSED`, `NO_ANSWER`, `SERVER_ERROR`: what the name server said, or did not.
- `BAD_NAME`, `NOT_ATTACHED`, `NO_ENTROPY`, `NETWORK`, `BAD_REQUEST`: what stopped the resolver
  before it could ask. `NETWORK`'s detail byte is a `network_step`.

`Outcome::from_word` refuses a word the resolver never sends, such as a failed `CALL`'s negative
error. A control `CALL` is answered with an `Outcome` too: `OK` when the message was accepted, or
`BAD_REQUEST` with a `refusal` number in the detail byte. `control_answer` decodes it.

### Zone spelling: two tiers

A zone is compared label by label (`Name::is_within`). Granted `example.test`, a client may ask
about `packages.example.test` and not about `evilexample.test`. One host name is an exact grant.
The root zone, `ROOT_ZONE`, is the empty text and grants every name.

What a configuration or a program's declaration may write is the strict tier, `is_canonical_zone`:
lowercase letters, digits and hyphens in labels of 1 to 63 bytes, joined by single dots, with no dot
at either end, and the root as the empty text. So two declarations of one zone are the same bytes.
A configuration that wants a visible root may write `.` and convert it to `ROOT_ZONE` before it
grants.

What the grant table accepts on the wire is the loose tier: any text `Name::from_dotted` accepts,
uppercase, `_` and a trailing dot included, compared without regard to case. So the table grants
some zones a declaration may not spell. That is deliberate: the table judges names, and the strict
tier is for comparing what was asked with what was granted.

## EXAMPLES

```rust
use domain_name_system::Name;
use domain_name_resolution_contract::{Denied, Grants, grant_messages};

// A spawner grants badge 7 the zone `example.test`.
let mut grants = Grants::new();
for (first_word, second_word) in grant_messages(7, b"example.test").unwrap() {
    grants.control(first_word, second_word);
}
let asked = Name::from_dotted("packages.example.test").unwrap();
assert!(grants.check(7, &asked).is_ok());
let elsewhere = Name::from_dotted("evilexample.test").unwrap();
assert_eq!(grants.check(7, &elsewhere), Err(Denied::OutsideZone));
assert_eq!(grants.check(8, &asked), Err(Denied::NoGrant));
```

## What is proven

The host tests check each piece of the grant protocol, each boundary of `is_canonical_zone`, the
round trip of every word, and the table's size. There are no Kani harnesses.

## Building and testing

```sh
cargo test
cargo build --lib --target x86_64-unknown-none   # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
```

## BUGS

- A grant lasts until the spawner sends `forget_message`. At most `MAXIMUM_GRANTS` (8) are held at
  once, and each badge keeps its window for as long as it holds a grant.
- One zone per badge. Two unrelated domains need two badges, or their common parent.
- A grant's zone travels eight bytes per message, because a message with a capability carries two
  words and one is the control word. A 253-byte zone is 32 messages, once per client.
- The grant applies to the name asked, not to the alias chain the answer follows.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.

# Prior art

An appendix to [the milestone](../1087-domain-name-system-outside-review.md).

How the crate sits against what a stranger already knows. Download figures are from the
other-implementations note, not refetched.

| implementation | shape | `no_std` | compression pointers | the same problem solved how | what a user coming from it will miss here |
|---|---|---|---|---|---|
| `hickory-proto` 0.26 (88M downloads, fuzzed) | full message model, `alloc` | no | checked against a per-message pointer budget (unverified) | `Name` with case-insensitive equality, `Header` accessors spelled out (`query_count`, `answer_count`, `response_code`, `truncated`, `recursion_desired`; unverified) | every other record type, EDNS, DNSSEC, `FromStr` and `Display` on names |
| `domain` 0.12 (NLnet Labs, 12.6M) | octets-generic, `no_std` path | yes | `ParsedName` refuses a pointer that does not point backwards (unverified) | the specification's spellings (`qdcount`, `rcode`, `tc`) as methods | the whole record zoo, zone files, a resolver |
| `simple-dns` 0.12 (11M, fuzzed) | packet builder and parser | with `alloc` | loop detection (unverified) | a small API, `Name` type | EDNS, many record types |
| `smoltcp` 0.14 `wire/dns.rs` (read here) | sans-IO stub client, `no_std`, no `alloc` | yes | splits the packet at the pointer so no byte is parsed twice, with the comment that backward pointers are what encoders emit | the same strictly-backwards argument as this crate, inside a socket | a standalone parser; it is tied to smoltcp's socket |
| `dns-parser` 0.8 (abandoned) | parser only | no | iteration bound (unverified) | | maintenance |
| BIND-derived `dn_expand` in the C libraries (glibc, OpenBSD, musl) | buffer in, dotted text out | n/a | glibc and OpenBSD bound the iteration count by message size; musl bounds pointers to the message (both from memory, unverified) | the output is text, not wire form | nothing; this crate keeps wire form, which is what a comparison and a grant check need |

Where this crate differs: it is the stub resolver's half only, with no allocation, no socket and no
policy, and it gives one verdict, `Query::accept`, that an application cannot misuse by reading the
wrong section. The termination argument for the name decoder is a proof, not a budget, and the TCP reassembler has a proof of honesty about when a message is whole. What it costs: A records only, no
EDNS(0), no `Display` or `FromStr` on `Name`, a 256-byte `Name` copied once per record read, and the
eight-address cap. The concrete example of the difference is the fuzz target's property 1: an
address `accept` returns must come from a class-IN A record owned by the canonical name the walk
reached. A reply about one name cannot plant an address for another. None of the crates in the table
state that property; they parse and leave belief to the caller.

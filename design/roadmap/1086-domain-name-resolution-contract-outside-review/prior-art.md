# Prior art

An appendix to [the milestone](../1086-domain-name-resolution-contract-outside-review.md).

How other systems scope what a program may resolve, and how comparable Rust crates shape the same
constants. The tree gives no download figures; nobody measured any.

| system | what a program gets | how it is scoped | what a reader coming from it will look for here |
|---|---|---|---|
| glibc and the BSD libc (`getaddrinfo`) | the ambient resolver from `/etc/resolv.conf` | not scoped: every program resolves every name | a `search` list and `ndots`; this contract has neither, by design |
| FreeBSD `libcasper` `cap_dns` | a resolver capability from the casper service | `cap_dns_type_limit` (name to address, address to name) and `cap_dns_family_limit` (IPv4, IPv6); not by name | a limit by record type and family; here the limit is by zone, and only IPv4 is carried |
| OpenBSD `pledge("dns")` | the right to talk to the resolver | all names or none | a per-program switch; here each badge has its own zone |
| systemd-resolved per-link routing domains | an ambient resolver | `~example.org` routes queries to a link; it is routing, not authority | something that looks like a zone grant and is not one |
| Fuchsia `fuchsia.net.name/Lookup` | a resolver protocol routed to the component | capability routing decides who holds the resolver; no zone inside it | a typed `LookupIp` with options; here one `CALL` with a page |
| `smoltcp` `socket::dns` (names from memory, unverified) | a DNS socket on the stack | the application holds the stack | fixed-size results (`MAX_NAME_SIZE`, an address count limit); the same fixed shape as `MAXIMUM_ADDRESSES`, spelled `MAX_` |
| `hickory-resolver` (hosted, std) | a full stub resolver with cache and search list | none | a cache and a search list; this contract has neither and says so |

Where this crate differs: the grant is a zone, compared label by label before any packet leaves
(`domain_name_system::Name::is_within`), one zone per badge, at most eight badges at once, and the
reply is a status that keeps "you may not ask" apart from "there is no such name". The concrete
example is in the tests (`src/tests.rs` line 36): granted `nife.test`, a client that asks `evilnife.test` gets the refusal `Denied::OutsideZone` and the resolver sends nothing, although the text ends
the same way. No system in the table refuses that case at the resolver. Each either forwards the
question or has no notion of a zone.

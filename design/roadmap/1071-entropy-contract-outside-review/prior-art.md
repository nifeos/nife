# Prior art for the entropy_contract crate

An appendix to [the milestone](../1071-entropy-contract-outside-review.md).

How the contract sits against what a reader already knows. Recent downloads are the last 90 days
at the time of the review.

| system | request | reply | short answer | "no source" | health check at start |
|---|---|---|---|---|---|
| `getrandom` 0.4 (791.8M recent / 2.57G all) | `fill(&mut [u8]) -> Result<(), Error>` | the buffer | never: loops inside | `Error` with a code | none |
| `rand_core` 0.10 (617.4M / 2.17G) | `fill_bytes`, `try_fill_bytes -> Result<(), Error>` | the buffer | never | `Error` | none |
| Linux `getrandom(2)` | buffer and length | byte count | up to 256 bytes never short once the pool is ready (unverified wording) | `errno` | the pool blocks until seeded |
| OpenBSD `getentropy(2)` | buffer up to 256 bytes | fills all | never | `EIO` above 256 bytes | none |
| Zephyr `entropy_get_entropy` | buffer and length | `0` or `-errno` | never | `-errno` | none |
| virtio-rng | a buffer in a queue | bytes written | the device may fill fewer (unverified wording) | no device | none |
| Linux `x86_init_rdrand` | eight `RDRAND` draws at boot | | | | refuses when consecutive draws repeat and disables the instruction (unverified detail) |
| FIPS 140-2 4.9.2, SP 800-90B 4.4.1 | | | | | compare each block with the last; a repeat is a failure |

Where this crate differs: the unit is one `CALL` of one request word and two reply words, so a
reply carries at most eight bytes and every caller loops. The crate tells a count from a failure
by magnitude alone, not by an error type, which the kernel's small negative error numbers make
possible. The caller supplies the `CALL`, so the crate has no dependency and no system call. It
carries a bring-up rule no listed system states: a first draw of all zeros is a dead device. It has
no error enumeration; `fill_with` answers `Option<()>`. That costs three things. The request
builder truncates a count without saying so (item 2). One `None` folds two failures together (item
5). The bring-up rule passes a source stuck on any nonzero value (item 17), the failure
`x86_init_rdrand` targets.

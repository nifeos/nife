# Prior art

An appendix to [the milestone](../1096-interfaces-note-outside-review.md).

This table sets the crate against what a loader author already knows. Recent downloads are the last 90
days at the time of the review. The loader behaviors come from public documentation and were not
reread for this review.

| system | what a binary carries | who checks, how | what this crate does instead |
|---|---|---|---|
| ELF symbol versioning (`.gnu.version_r`) | the version names it needs from each shared object | the dynamic loader refuses a missing version | a numeric id and one version per interface, equality not presence |
| glibc `.note.ABI-tag` (owner `GNU`) | OS and a minimum kernel version | the loader refuses a kernel below it | equality; the ABI never dual-serves |
| FreeBSD `.note.tag`, NetBSD `.note.netbsd.ident`, OpenBSD `.note.openbsd.ident` | one OS release word | the kernel picks the ABI brand | several interfaces per note, each versioned |
| Linux modules: `vermagic` and `__versions` | an exact build string, then (CRC, symbol) records | `init_module` refuses a mismatch | the same shape (records, equality at load) with a proven canonical encoding and a bound of 64 |
| Mach-O `LC_BUILD_VERSION` | platform, minimum OS, SDK, tool versions | dyld refuses by minimum | no tool versions; a `const fn` builds the note, no linker feature |
| `cargo-auditable` (`.dep-v0`), 517k / 1.43M downloads, with `auditable-serde` 5.06M / 15.2M | the whole dependency graph, compressed JSON, embedded by a rustc wrapper | `cargo audit bin` reads it after the fact | a hand-written list of twelve-byte records, read at load; deriving the list at build is this crate's own stated BUGS entry, and this is how someone did it |
| `object` 0.41 (143M / 710M), `goblin` 0.10 (22.9M / 87.9M), `elf` 0.8 (3.99M / 14.2M) | parse any note generically | none validates a vendor descriptor | this crate is only the descriptor layer; the note walk belongs to the reader |

This crate differs from all of them in four ways. It has no dependencies and is `no_std` with no allocation. Its descriptor has exactly one encoding per list (proved for two records, tested at zero, one, three
and sixty-four). A proof shows its decoder never panics on any input up to one byte past two records. It leaves the policy (an absent entry passes) to the reader. That costs two things. The list comes by hand,
and only a check in the consuming tree keeps it honest. The registry of ids lives with the
reader, not with the crate.

# Prior art

An appendix to [the milestone](../1081-capability-outside-review.md).

How the crate sits against the systems a reader of a capability table already knows.

| system | rights | table | delegation | what a reader coming from it will miss here |
|---|---|---|---|---|
| seL4 | per-object rights (`Read`, `Write`, `Grant`, `GrantReply`) on a capability | a tree of CNodes with guard bits; capability derivation tree (CDT) tracks children | `CNode_Mint` and `CNode_Copy` can only mask rights; `Revoke` walks the CDT | revocation of derived children, badges, sparse address space; this crate deletes by object match from every table instead |
| Zircon (Fuchsia) | `ZX_RIGHT_*` bits on a handle, including `DUPLICATE`, `TRANSFER`, `MANAGE_THREAD` | a per-process flat handle table, growable | `zx_handle_duplicate` and `zx_handle_replace` can only reduce rights; `DUPLICATE` gates copying | a `TRANSFER` right distinct from `GRANT` (move versus copy), handle growth, `ZX_RIGHT_SAME_RIGHTS` |
| Capsicum (FreeBSD) | `cap_rights_t`, about 80 named rights on a file descriptor | the Unix descriptor table | `cap_rights_limit` only removes rights; `ENOTCAPABLE` as a distinct error | fine-grained rights; a distinct "not capable" error, where this crate answers `CannotWiden` for both a widening derive and a missing right on lookup |
| Unix | none on the descriptor; mode checked at `open` | flat, per process, lowest free slot first | `dup`, `fork` inherit everything; `FD_CLOEXEC` opts out | nothing structural: the crate's first-free insert is the `dup` rule, and `GRANT` is the inversion of inherit-by-default |
| KeyKOS, EROS, CHERI (unverified from memory of the literature) | rights bits on the key or the hardware capability | key registers and nodes; CHERI's are registers checked by the hardware | monotonic narrowing enforced by the kernel or the hardware | hardware enforcement; persistence |

Rust crates a crates.io user may reach for instead, with recent (90-day) and all-time downloads at
the time of the review:

| crate | downloads | what it is | relation to this crate |
|---|---|---|---|
| `capability` 0.1.0 | 29 / 522 | a skill-tree library for games | **holds the name** this crate would publish under (item D1) |
| `cap-std` 4.0 | 16.3M / 32.5M | a capability-based `std` (filesystem, network) | a user-space discipline over OS handles, not a kernel table |
| `caps` 0.5 | 5.9M / 27.6M | Linux `capabilities(7)` bindings | a different meaning of the word; a reader searching for "capability" meets it first |
| `bitflags` 2 | 581M / 2.2G | bit-set types with the or and and operators and `contains` | what `Rights` is by hand; this crate deliberately exposes only `union`, `intersect`, `allows`, `is_subset_of`, so no operator can widen |
| `enumflags2` 0.7 | 44.7M / 141M | enum-backed bit sets | the same |

Where this crate differs: it is a flat fixed-capacity table whose size is part of its type. It uses no heap and no `unsafe`. A free-slot word makes insert one `trailing_zeros`, and the table keeps its own high-water mark. It carries fourteen machine-checked proofs, each with the patch that makes it fail. Its
rights set has five bits and no operator that can widen. What it costs: no sparse space, no
derivation tree, no revocation of children (the kernel sweeps by object identity instead), a
process-wide `static` for the peak, and the gap of item 3.

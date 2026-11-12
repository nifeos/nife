# Prior art

An appendix to [the milestone](../1106-program-argument-contract-outside-review.md).

How other systems hand a program its arguments, against what this crate does. The nife note on
foreign program arguments read the entries marked "from source". The rest are from memory and
unverified here.

| system | how argv travels | authority | what a reader coming from it meets here |
|---|---|---|---|
| Linux, the BSDs, Plan 9 | NUL-terminated strings on the initial stack, `argc` first, `envp` and `auxv` after; Linux caps one string at 128 KiB and the whole at a quarter of the stack limit | the process's credentials and open files | length-prefixed records, so a NUL is a legal byte; a 4,080-byte ceiling; no environment on the page |
| WASI preview 1 (`args_sizes_get`, `args_get`) | two calls: a count and a total byte size, then NUL-terminated strings packed into a buffer the program supplies | capabilities handed separately | the same count-and-total header, written once into a page instead of answered by two calls |
| Fuchsia `processargs` (from source) | a bootstrap message with `args_off` and `args_num` naming NUL-terminated UTF-8 strings, beside `environ_*` and a handle name table | handles in the same message, different fields | bytes rather than UTF-8; no handle table, because grants travel as capabilities in slots |
| Xous, in `std` itself (from source) | a tagged parameter block passed by pointer in an entry register | separate | a fixed slot and a fixed address, chosen over the entry registers so `caps` can show the page as a row |
| seL4 `sel4runtime` (from source) | `argc`, `argv`, `envp` written onto the child's stack by the parent | capabilities placed separately | a page the loader maps read-only, never the stack |
| Hubris, Tock | no argv; a task's parameters are fixed at build time | n/a | an argv at all |

Where this crate differs from all of them: it has a magic that makes a zeroed or foreign page read
as no arguments. It parses all or nothing, so a program never sees a prefix of its line after
single-field damage. What it costs: four bytes of length per argument where two would hold the
largest record the page can carry. It allows one page of ceiling where Linux allows two mebibytes.
It has no checksum, so a header whose count and total both agree on a shorter line is
indistinguishable from that shorter line (item 9). A concrete example of the shape is the line `rg
pattern src`. It is the magic `nifeargv`, count 3, total 25, then `2 rg`, `7 pattern`, `3 src`, each
length a little-endian `u32`, and 4,055 bytes nothing reads.

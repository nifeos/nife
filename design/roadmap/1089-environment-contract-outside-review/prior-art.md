# Prior art

An appendix to [the milestone](../1089-environment-contract-outside-review.md).

How the page sits against what a stranger already knows. Entries marked recalled were not read
during this review.

| system | shape | who validates, and when | what a reader coming from it meets here |
|---|---|---|---|
| Unix `envp` (Linux, the BSDs) | NUL-terminated `KEY=value` strings on the initial stack; any key, any bytes | nobody at hand-off; `setlocale` and `tzset` validate at use, and glibc's `tzset` falls back to UTC on an unknown `TZ` without a word | three fixed keys, a value refused at assembly, an unknown key impossible, absence instead of a fabricated default |
| Linux `auxv` | fixed `(u64 key, u64 value)` pairs the kernel writes, ended by `AT_NULL`, read by `getauxval` | the kernel decides the closed key set; values are open | the same closed-key, kernel-assembled, read-only idea, with the values closed too |
| CloudABI `argdata` (recalled, unverified) | a typed, read-only tree the launcher assembles from YAML, in place of environment variables | the launcher, before the process exists | the nearest ancestor of §111 (inert configuration is a read-only page, and each declared key is validated against a closed domain)'s stance: configuration is typed data assembled outside the process |
| Fuchsia structured configuration (recalled, unverified) | values declared with a type in the component manifest, delivered in a VMO, decoded by a generated client library that checks a schema checksum | the manifest compiler at build, component manager at launch, the client library at read | validation on both sides; this crate validates on the writer's side only (item 2) |
| Plan 9 `/env` | one file per variable in a per-process namespace | nobody | a namespace answers "who may see it"; §111 also answers "what may be in it" |
| WASI `environ_get` with wasmtime's `--env` allowlist | the host passes an explicit list; values are bytes | the operator, by key, at launch | the allowlist idea at the key, with the value closed as well |
| Rust crates `chrono-tz` and `tzdb` | the whole IANA table compiled in, so any name validates | the crate, from the full database | a curated list of 20 zones; `Europe/Madrid` is refused today, by design, and the list grows on demand |
| `program_argument_contract` in this tree | a `MAGIC`, a count, a total and length-prefixed records in one 4096-byte page; a safe `ArgPage::parse(&[u8; PAGE_BYTES])` reader | every length against what is left, at read | the family's other page has the totality this crate's documentation claims and does not have (item 1); its reader takes a borrowed array where this one takes a raw address (item 11) |

Where this crate differs from all of them: a closed domain per key, so a secret cannot ride a
well-known key. Absence rather than a default when the page is missing, zeroed or foreign. One
`no_std`, dependency-free definition shared verbatim by the assembler and the standard library. What
it costs: three keys only, each list short, and an `unsafe` constructor on the reader.

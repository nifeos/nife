# Decision B: the names the gates refuse, and the units on the ratified limits

An appendix to [the milestone](../1090-filesystem-contract-outside-review.md), which states the
rule in force.

Background. The template runs `spelled_out`, `register_names` and `question_shape` at a ceiling
of 0 on every pull request. Seventeen names fail the first (the module `dir`, `IS_DIR`,
`STORE_DIR`, six `spec` functions, bindings), four the second, one the third. Separately, the
rule that a length or a limit carries its unit postdates the ratification of `MAXIMUM_NAME`
(twice), `MAXIMUM_VALUE`, `TRANSFER_MAXIMUM` and `MAXIMUM_LENGTH_OR_OFFSET`, and the rule that a
count names the things reaches `MAXIMUM_COUNT`. A release review rechecks such names. The verb
names (`OPENDIR`, `READDIR`, `MKDIR`, `RMDIR`, `FSTAT`, `GETXATTR` and the rest) and the errno
names are POSIX terms of art, which the rules keep with an alias and an opening line naming the
source. The module names `blk`, `fs`, `dirent`, `xattr`, `statfs` are abbreviations whose
expansions are spoken, except `fs`, which mirrors `std::fs`, and `statfs`, which is the call's
name.

Recommendation, one table, to rule as a set and apply in one lane after the move:

| today | proposed | rule |
|---|---|---|
| module `dir` | `directory` | spelled out (ruled word) |
| `dirent::IS_DIR`, `nameset::IS_DIR` | `IS_DIRECTORY` | spelled out |
| `xattr::STORE_DIR` | `STORE_DIRECTORY` | spelled out (Decision A rules the value) |
| `grant::spec`, `spec_len`, `spec_rights` | `specification`, `specification_len`, `specification_rights` | spelled out (ruled word) |
| `xattr::spec`, `spec_kind`, `spec_value_len` | `specification`, `specification_kind`, `specification_value_len` | spelled out |
| module `blk` | `block` | an expansion everyone says |
| module `dirent` | `directory_entry` | the same |
| module `xattr` | `extended_attribute` | the same; `#[doc(alias = "xattr")]` |
| module `fs` | keep, `#[doc(alias = "filesystem")]` | mirrors `std::fs` |
| module `statfs` | keep, alias `volume_statistics` | a POSIX call name |
| `GETMTIME`, `SETMTIME`, `SETMTIME_AT` | `GET_MODIFICATION_TIME_UNIX_SECONDS`, `SET_MODIFICATION_TIME`, `SET_MODIFICATION_TIME_AT_UNIX_SECONDS` | `mtime` is a ruled word; provisional today |
| `blk::BLOCK_SIZE` | `BLOCK_BYTES` | a size carries its unit |
| `volume_statistics::LEN` | `RECORD_BYTES` | the same |
| `store::RECORD_HEADER` | `RECORD_HEADER_BYTES` | the same |
| `nameset::BYTES` | `MAXIMUM_BYTES` | a limit opens `MAXIMUM_` |
| `grant::WORDS_NAME` | `MAXIMUM_WORDS_NAME_BYTES` | a limit with its unit |
| `PAGE` | `PAGE_BYTES` | the page-contract spelling in `design/naming.md` |
| `xattr::MAXIMUM_NAME`, `grant::MAXIMUM_NAME` | `MAXIMUM_NAME_BYTES` | ratified before the units rule; reopened at release |
| `xattr::MAXIMUM_VALUE` | `MAXIMUM_VALUE_BYTES` | the same |
| `fs::TRANSFER_MAXIMUM`, `blk::TRANSFER_MAXIMUM` | `MAXIMUM_TRANSFER_BYTES` | the same |
| `fs::MAXIMUM_LENGTH_OR_OFFSET` | `MAXIMUM_LENGTH_BYTES` | the same, and the field carries no offset (item 17) |
| `xattr::MAXIMUM_COUNT` | `MAXIMUM_ATTRIBUTES` | a count names the things |
| `blk::SIZE` | keep, exempt from `register_names` | an opcode, not a size |
| `verb::in_opcode_order` | `is_in_opcode_order` | question shape |
| the POSIX verb and errno names | keep, each with `#[doc(alias)]` and a source line | a specification's term of art |

Prior art. Fuchsia spells `GetAttributes`, `UpdateAttributes` and `rights` in full in FIDL and
keeps POSIX names only at its POSIX compatibility layer. Capsicum keeps the Unix spellings
(`CAP_FUTIMES`) because they name Unix calls, which is the case the verb names are in. The `libc`
crate keeps every errno as the platform spells it, which is the case the errno names are in.

Concrete example. A caretaker that reads `verb::of(operation).takes_name()` today would read
`verb::of(operation).takes_name()` still; one that compares `grant::spec_rights(w)` reads
`grant::specification_rights(w)`; a client that sends `fs::request(fs::GETMTIME, ..)` sends
`fs::request(fs::GET_MODIFICATION_TIME_UNIX_SECONDS, ..)`, and the number 20 does not change.

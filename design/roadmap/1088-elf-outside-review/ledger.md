# The ledger

An appendix to [the milestone](../1088-elf-outside-review.md).

| item | state |
|---|---|
| before the move: citations, fuzz seeds, manifest and files | left: the citation rewrite is M and feeds the README; the rest is the move's |
| `page_range` and a page size as input | left: a `PageBytes` type and a parse that takes the page size are new public names and signatures |
| four `as usize` casts | done: `phoff`, `p_offset`, `filesz` and `memsz` convert with `usize::try_from` (`errors_print_and_a_64_bit_field_is_never_truncated`); the three bounds harnesses keep their signature |
| `Display`, `core::error::Error` and derives | done: both errors print, box and hash, and `Elf` derives `Debug`, `Clone` and `Copy` |
| the public vocabulary | left: decision D3 |
| a README and a shared forge | left: the move's, and a test refactor |
| the proofs' hold | left: the module-doc correction waits for the page-size type |
| an entry in the zero-filled tail | left: a refusal is a published behavior, so the architect's |
| what the parser ignores | left: the README's section |
| `segments()` discards an error | done: `segments` states the invariant in a comment; `exactly_the_maximum_header_count_is_accepted` already counts |
| four timeouts | left: the first weekly run |
| the note padding rule | done: `padding` says which values read as 4-byte padding and where LLVM differs, from the review's recollection of LLVM, not a re-read of its source |

# P2: later

An appendix to [the milestone](../1088-elf-outside-review.md).

## P2: after the first tag

#### An entry point in the zero-filled tail is accepted

Finding: the entry check (`src/parse.rs` lines 168 to 172) uses `memsz`, so an entry in `[vaddr + filesz,
vaddr + memsz)` passes and the program starts by executing zeros, which faults on every supported
architecture rather than running. Linux does not check this either (recalled). One compare refuses
it as `EntryNotExecutable`. Door: one-way at release (a refusal is a published behavior; adding one
later breaks a file that passed, so it is now or never). Effort: XS.

#### What the parser ignores is written nowhere a stranger looks

Finding: `EI_OSABI`, `EI_ABIVERSION`, `e_flags` (which on RISC-V carries the float ABI and
compression), `e_ehsize`, `PT_LOAD`'s `p_align` and the gABI congruence `p_vaddr mod p_align ==
p_offset mod p_align`, `PT_TLS`, `PT_GNU_STACK`, and every section header are not read. For a loader
that copies bytes into fresh pages, as this tree's does, none matters. For one that maps file pages,
the congruence does. Nothing says so.

Change: a "What it does not check" section of the README, one line per field with the reason, and
a BUGS entry that a mapping loader must check the congruence itself. Door: two-way. Effort: XS.

#### `segments()` discards an error it cannot have

Finding: `src/parse.rs` line 227 is `filter_map(|i| self.segment_at(i).ok().flatten())`. After `validate`
no error is reachable, and the comment says so. But an edit to `validate` that dropped a check would
make segments vanish from the iterator rather than fail loudly, and no test would see the count
change unless it counted. Door: two-way. Change: `validate` records nothing new. `segments`
documents the invariant, and `a_good_binary_parses` already asserts the count, so the cheapest fix
is a one-line test that a parse with `phnum` 64 yields 64 segments (it exists:
`exactly_the_maximum_header_count_is_accepted`). Record it as understood in a BUGS line and move on.
Effort: XS.

#### Four mutation timeouts have no triage line

Finding: the census row for `elf` reads 4 timeouts with no classification in
`notes/mutation-testing/survivor-classification.md`. A timeout in a parser is usually a loop bound
mutated into a spin (the `while read < KNOWN_MACHINES.len()` in `foreign_machines` and the
`while at < seg.len()` walk in `find_in_segment` are the candidates), which is a catch, not a hole, but the record should say which. The new repository's `.github/mutants-exempts` starts empty
and its first weekly run will name them.

Change: triage the four in the first weekly run and list each with its reason, as the template's
gate requires. Door: two-way. Effort: XS.

#### The note reader's padding rule differs from LLVM's on one input

Finding: `padding` (`src/note.rs` lines 186 to 191) reads `p_align` 0 and 1 as 4-byte padding for any
segment. LLVM's reader accepts 0 only for an empty segment and refuses 1, as the reviewer recalls
and did not re-read. Milestone 784 (reuse what the tree wrote: which first-party code an existing library should replace)'s table shows `object` agreeing with this crate on `p_align` 1.
No producer in the tree writes either value. Change: a sentence in the doc of
`NoteError::BadAlignment` saying which padding the reader treats each value as, and why, with the LLVM
behavior checked against its source at that time. Door: two-way. Effort: XS.

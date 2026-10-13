# The sweep for harnesses that duplicate the implementation

An appendix to [`notes/falsification.md`](../falsification.md), which states the finding and the
test.

Milestone 213, 2026-09-02. Milestone 211's sweep found this shape beside the one it was looking
for and could not fix it in passing, because the repair is a restructuring of the code rather
than a rewrite of a harness. 211 asks whether a harness states its property *through* the
function under test. This asks a blunter question: is the function under test in the harness at
all?

148 harnesses read, one by one. That is `script/falsifications`' 146, which now follows
`cargo metadata` rather than a directory walk, plus the two in `vendor/redoxfs/src/node.rs`,
which is vendored and in no workspace package. No mechanical narrowing was attempted and none is
offered: 211 measured its own extractor flagging three findings for the wrong reason and missing
an entire family, and the cheapest honest thing here was to read them.

### The discriminator, which is the hard part

Some duplication is the point. 211's own eleven rewrites all converged on restating a property in
raw bits, in the wire format's own shifts, or in architecture bit positions as literals,
*precisely so* the harness cannot repeat the implementation back at itself. A sweep that treated
recomputation as the defect would undo that work. So the question is not whether a harness
recomputes something. It is:

> Which side of the assertion did the crate produce?

An assertion compares a subject with an expectation. The crate under test must produce the
subject, because nothing else in the harness can be broken by breaking the crate. The expectation
should come from somewhere the implementation does not get to choose, which is exactly why it is
so often a recomputation, and that recomputation is the virtue. The defect is the case where the
crate produced **neither** side: no rewrite of the code can then change whether the assertion
holds.

`device_tree_blob::be32_reads_big_endian_when_in_bounds` writes the four shifts out and compares
them against `be32`'s answer: subject from the crate, expectation from the format. `nifefs`'s
harness wrote both sides itself. Same shape at a glance, opposite in what they prove.

A second question, for duplication on the assumption side, where a harness restates a guard in
order to reach the state it wants: which way does drift fail? If the implementation moves and
the harness's assumed set becomes *wider* than the code's accepted set, the harness asserts on
inputs the code now refuses and goes red, which is the safe direction.
`direct_memory_access_validator::an_oversized_batch_is_refused` restates the batch guard's condition
and fails this way. If drift makes the assumed set narrower, or if the assertion is the harness's
own arithmetic too, it goes green and says nothing. That is the `nifefs` case, and it is why the
implication in it was the whole harness rather than a detail of it.

### The finding: one, and it is the one the block named

`nifefs::the_validation_implies_reads_slice_is_in_bounds` called neither `Fs::parse` nor
`Fs::read`. It recomputed `parse`'s acceptance condition as its antecedent and `read`'s slice
arithmetic as its consequent, under two comments claiming each was "exactly" what the code did,
and proved that a copy of `read`'s arithmetic stays inside a copy of `parse`'s bound.

Measured in both directions, as 211 did. `read` was changed to slice from `(start_block +
DIR_BLOCKS) * BLOCK`, a whole directory span past what `parse` accepted, which is a defect the
crate's own module comment invites (`start_block` is absolute, and a reader who "helpfully" added
the directory offset would write exactly this). The old phrasing verified in 0.04 seconds. The
rewritten harness goes red on the same defect, and the patch that restores it is
`crates/nifefs/falsifications/verification.the_validation_implies_reads_slice_is_in_bounds.patch`.

**Honest caveat, because this one does not read the way the other eleven did:** the crate's host
tests *do* catch that defect, seven of them. So it is a blind spot in the proof rather than a hole
in the crate, and the falsification record is evidence about the harness, which is all §134 ever
claimed for one. What the harness was for is the part the tests cannot reach: every image, at
every length, including one a kernel is handed off a disk rather than one `write_image` produced.
Its own doc comment called that "the kernel-facing guarantee, free of any bound on the image
size", and that is the sentence the old phrasing did not deliver.

### The repair earned its keep, and the block was right to make that the first question

213's BUGS warned that factoring a condition out to make it provable adds a function whose only
caller-visible reason is the proof, which §46 refuses. That is not what happened here, and the
reason is worth stating because it is the test to apply next time.

The duplication was in the implementation before it was in the harness. `parse` validated with
three lines and `read` sliced with a copy of the same three lines, and `read`'s unchecked
`image[start..start + len]` was sound only because a reader could see the two matched. So
`Fs::entry_bounds` does not exist for the prover. It has two ordinary callers, it turns an
invariant that lived in a reader's head into one expression, and `read`'s `expect` now names why
the failure cannot fire. The prover benefit is a consequence, which is exactly the standing
`elf::check_segment_bounds` has one crate over.

The test the next lane should apply: would this function be worth extracting if there were no
harness? If the answer is no, §46's refusal stands and the honest outcome is a recorded
limitation rather than a split.

### The negative half: two that have the shape and are not findings

Recording these is not modesty: a sweep that reports only its hits is unfalsifiable prose.

`credential_protocol::a_request_word_round_trips_every_field` is blind on its own, and covered by
its neighbor. It is an encoder round-tripped through its own decoder, which is 211's third
family. Swapping the identity and secret length fields in `req` and in `id_len`/`secret_len`
together leaves it verifying. Both sides move and the round trip is perfect, while a client using
the real wire format has its identity length read as its secret length. It was left alone, with
the measurement written at the harness. The defect is caught next door by
`no_request_word_makes_the_parse_read_outside_the_page`, which 211 rewrote to state the two shifts
as literals, and which goes red on exactly that patch. Restating the shifts twice would be a
second copy of a claim one harness already pins. What that costs is a fact about the suite
rather than the harness, so weakening the sibling silently un-covers the wire format, and the
comment now says so where a reader meets it.

`kernel::every_page_between_the_checked_ends_is_itself_a_user_page` models two call sites, and
the model is faithful today. It restates the guard `page_frame_map` and `MAP_INTO` apply before
their map loops. Both were read on 2026-09-02 and both apply exactly it. This is the shape and
cannot be repaired the way `nifefs` was, because what is duplicated is a *caller's* control flow
rather than a function: there is nothing to extract and call. It stays, with the check and its
expiry recorded at the harness.

Two more were looked at and dismissed on sight, and are named so nobody re-derives them.
`direct_memory_access_validator`'s `walk` and `inter_process_communication`'s `seed` look like
harness-side reimplementations and are not: `walk` calls the real `shadow_one_head` and `seed`
builds its symbolic state through the real `push_back`. `component_plan::declares_by_core_eq` is the
good version in its purest form, an independent implementation of `str_eq` standing on the
expectation side on purpose.

`script/falsifications` reads 36 of 146 after this lane, from 35.

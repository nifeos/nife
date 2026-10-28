---
status: DECIDED
raised: 2026-09-05
decided: 2026-10-28
ratified_by: calef
---

# 146. Archive and compression: which pieces we write, which we take, and which we refuse

Ruled 2026-10-28 (UTC): A, the seven verdicts as written: write tar; write inflate and stored-block
deflate, revisit real deflate on measurement; defer zstd and zip; refuse bzip2, xz/LZMA and 7z.
Unblocks milestone 258; milestone 99's block is corrected.

Raised by calef, 2026-09-05, on being told no milestone covered archive
utilities: *"Mint it. However we should debate write it and vendor it for each piece."*

This is [§46](0046-dependency-rule.md) applied piece by piece, not a new rule.
§46's test is already written and this section's job is to run it honestly on seven candidates and
show the working, including where the answer is genuinely arguable rather than obvious.

Taking a dependency is an architect's call, which is why this is a decision file and not a lane's
choice.

## §46's test, restated so the reasoning below can be checked against it

1. On the verification path? Write it. *"You cannot restructure someone else's crate to make a
   model checker tractable."*
2. **A whole subsystem we would never write?** Take it.
3. Touches the kernel, the ABI, or a capability? Write it.
4. Otherwise, write when the specification is complete and checkable; depend when correctness is
   won by exposure rather than by reading the spec.

And separately, from the 2026-07-31 amendment: the trigger for vendoring is "we must patch it."
Not size, not importance. `smoltcp` is a whole subsystem and an ordinary dependency because nothing
needed changing; RedoxFS is vendored because it needed a divergence patch to build `no_std`.

## Who actually needs any of this, because that is what bounds the list

Milestone 99 (`git` on nife) needs zlib inflate and deflate. Every git object is zlib-compressed
and so are packfiles. That block argues `git` beats Vaultwarden as a first real workload because
*"`init`, `add`, `commit`, `log`, `status`, `diff` are a filesystem, a hash, a compressor, a clock,
and a place to put bytes. Every one of those is something this tree either has or is building."*
The compressor clause is false, and this section is the first thing to say so: there is no
compression code anywhere in this tree, in any form.

Milestone 198 (the package manager) will need a container and probably a compressor, and
explicitly has not chosen: *"It does not decide the format, the activation shape, or the repository
split."* AGENTS.md makes 198 a precondition on the ranking function itself, so this sits underneath
the thing that makes a second customer possible.

*(Dated note, 2026-09-26, decisions-hygiene lane.)* Milestone 198's half of this has since been
answered, and not with a compressor. §197 (a package is one archive file) ruled the container on
2026-09-20: one archive file per package, option C2. The lane that built it on 2026-09-23 wrote its
own format, `crates/package_archive`, rather than taking `tar`, and stores members whole. Its BUGS
section records "No compression" on purpose: a compressor is a second hostile-input parser on the
same path, and the size cost is measured nowhere yet. §197 itself says nothing about compression;
the answer is the built format's. So the package manager is no longer a consumer of anything below,
and milestone 99 (`git`) is the only one left. The zstd entry's "if milestone 198 picks it" and
"What this does not decide" are overtaken the same way.

Nothing needs bzip2, xz, 7z, or zip. That is the whole of the evidence for those four and it is
enough.

## Piece by piece

### tar: **write it**, and this one is not close

Rule 1 and rule 4 agree. A `ustar` header is 512 bytes of fixed-offset fields with an octal
checksum, the specification is complete, and the entire format is a loop over headers. It is pure
logic, so it is host-testable and Kani-reachable in the shape AGENTS.md already prescribes, and it
is exactly the kind of parser this tree writes rather than takes (`gpt`, `elf`, `dtb`, `nifefs`).

The security-relevant parts are not spec-reading, they are policy, which is the strongest reason
not to take someone else's: path traversal through `../`, absolute paths, and symlink escapes are
decisions about what an extraction may reach. In a capability system that answer is structural, and
handing it to a dependency would mean adopting its policy instead of ours.

### DEFLATE and zlib: **the real debate, and I recommend writing inflate**

This is the piece worth arguing, so both sides get stated properly.

For taking it. `miniz_oxide` is pure Rust with no C dependency (read 2026-09-05 at
`docs.rs/miniz_oxide`), does both directions, and carries enormous exposure: it is what the Rust
ecosystem's own tooling leans on. Decompressors are a classic attack surface, and rule 4's
"correctness is won by exposure" is the clause that put crypto on the take side. If that clause
applies here, this is a dependency and the argument is over.

**Against, and why I think rule 4 does not reach this case.** Crypto is on the take side because
correctness there includes resistance to attacks not yet published and side-channel behavior no
specification states. DEFLATE has no secrets. It has no key, no timing channel that matters, and
no cryptanalysis. Its threat model is two things, and both are answerable by construction rather
than by exposure:

- Malformed streams causing out-of-bounds access, which Rust prevents at the language level and
  which is the specific bug class that made C decompressors famous.
- Decompression bombs, which are a *resource* question, answered by an output limit. That limit
  is a policy decision about what an extraction may consume, and it belongs to us for the same
  reason tar's path policy does.

And rule 1 points hard the other way. A decompressor over untrusted input is the single best
Kani target this tree could acquire: *the output never exceeds the declared bound*, *the window is
never read outside itself*, *a truncated stream terminates*. Milestone 191 found no harness here had
ever caught a defect and 193 fixed the reachability; a real adversarial parser is what that
machinery is for. Rule 1 exists precisely because you cannot restructure a dependency to make those
proofs tractable, and `crates/calendar` is the worked example of needing to.

The cost, measured rather than asserted. Inflate is a canonical Huffman decoder plus an LZ77
window, and it is small; the specification is RFC 1951 and it is short. Deflate, the compressing
direction, is the larger and less interesting half, because good compression is heuristics
(match-finding, lazy matching) rather than specification.

There is a way to split that nobody has mentioned and it may be the whole answer. A zlib stream
made entirely of stored (uncompressed) blocks is legal, and any conforming reader accepts it. So
`git` could be made to work with a full inflate and a trivial deflate that only emits stored
blocks: correct, tiny, provable, and interoperable, at the cost of larger objects. Real
compression then becomes a measured optimization with a working system already in place, rather than
a prerequisite.

Recommendation: write inflate, write stored-block deflate, and revisit real deflate on
measurement. If that proves wrong, taking `miniz_oxide` later is cheap, which is the test the
*move fast on what can be undone* tenet actually asks.

### zstd: **defer, and take it if it is ever needed**

Nothing needs it. If milestone 198 picks it as the package format, it goes on the take side without
much argument: FSE and ANS entropy coding is a genuine subsystem, the specification is large, and
this project would gain nothing from owning it. That is rule 2.

### bzip2: **refuse**

No consumer, and none is likely. It is a legacy format displaced by zstd and xz everywhere that had
a choice.

### xz and LZMA: **refuse, and record the second reason**

No consumer. And the xz-utils backdoor of 2024 is the supply-chain case of the decade, which is
worth stating in a tree with a dependency tenet rather than leaving as folklore. **The lesson is not
"never take dependencies"**, which would be the wrong reading and would contradict §46. It is that
the cost of a dependency includes the trust chain behind it, and that a format nothing needs is a
trust chain bought for nothing.

### zip and 7z: **defer zip, refuse 7z**

`zip` is a container plus DEFLATE plus CRC-32, and this tree already has the CRC-32 (`crates/globally_unique_identifier_partition_table`
implements the zlib polynomial). So once inflate exists, zip is close to free and can be revisited
against a consumer. `7z` is LZMA-based, has no consumer, and inherits the xz argument.

## What this does not decide

**It does not pick the package format.** That is milestone 198's, and this section deliberately does
not pre-empt it; what it does is make sure 198 has a container and a decompressor to choose from.

**And it does not vendor anything.** Every "take" above is an ordinary dependency unless and until
something needs patching, per the 2026-07-31 amendment. Nothing here is expected to.

## What is blocked until this is answered

Milestone 258, which is minted against this section and scoped by it. Milestone 99 is blocked in
a quieter way: it is `NOT-STARTED`, and its block currently claims a dependency it does not have.

## BUGS

- **`miniz_oxide`'s `no_std` support was not confirmed.** The crate is pure Rust with no C
  dependency, read on 2026-09-05, and the docs page consulted did not state its `no_std` status or
  list its feature flags. Anyone acting on the "take it" side owes that check first, because this
  tree's user programs are `no_std` and a dependency that needs `std` is a different decision.
- The stored-block deflate idea is untested here. It is correct by the specification and this
  tree has not written a byte of it, so "tiny and provable" is a prediction.
- No measurement supports "inflate is small". It is a judgment from the shape of RFC 1951, not a
  line count. §92 (a caretaker is supervised)'s test applies to it: if the real reason to write it
  turns out to be that it looked fun, that has to be said out loud.
- Nobody has checked what `git` actually requires of the compressor. The claim that stored
  blocks suffice rests on zlib's specification rather than on reading git's object reader.

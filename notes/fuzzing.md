# Fuzzing the parse surface

Milestone 42 (supply chain and fuzzing), second leg. The first two (advisories, licenses, vendored
integrity) ask whether code we did not write is what we think; this one asks whether code we *did*
write survives bytes it did not write.

`script/fuzz` runs coverage-guided fuzz targets over the parsers that read data from outside this
system: four when this note was written, seven parsers now, plus three service targets that
[fuzzing-the-services.md](fuzzing-the-services.md) covers. It found two bugs on its first day, and a
third came out of reading the code while designing a target. All three are fixed and pinned by host
tests.

## What fuzzing finds that Kani does not

This project has over 100 Kani harnesses <!--count-at-least:kani-harnesses-->
across more than 20 crates <!--count-at-least:harness-crates-->, and DECISIONS §14 (the project's
direction) says the proofs are the thesis. Adding a fuzzer only earns its place if it answers a question the proofs
cannot, so that is the first thing to settle.

Kani is exhaustive inside a bound. `kani::any()` is every value at once, the program becomes a
formula, and `UNSATISFIABLE` means no input breaks the property. That is a stronger claim than any
fuzzer can make, for the inputs it covers. Its limit is structural rather than incidental. A solver
reasons completely about *fixed-size* things, so an unbounded loop over an arbitrarily long input
has to be unrolled to a limit, and past a certain unrolling the formula stops being solvable in
useful time. notes/verification.md records those bounds and their justifications.

Fuzzing is unbounded and random. It proves nothing. What it does is execute the real code on
millions of concrete inputs an hour, with coverage feedback steering toward inputs that reach new
branches, and it does not care how many loop iterations that takes.

So the two tools divide on one line:

| | Kani | Fuzzing |
|---|---|---|
| Coverage | every input, inside a bound | some inputs, no bound |
| Answers | "is this property true" | "does anything crash" |
| Loops | must be unrolled | free |
| Cost of a wrong bound | a silent hole | none |
| Verdict on success | a proof | "not yet" |

The place where they meet is the bound. In this tree the proved leaf and the unproved shell of
`crates/elf`, the proved leaf readers and unproved walkers of `crates/device_tree_blob`, and
`crates/nifefs`'s writer and reader agreeing (a property nobody had written down) are the three
worked examples, in [fuzzing/kani-and-fuzzing.md](fuzzing/kani-and-fuzzing.md).

The honest summary: a proof says the property holds where it looked, and a fuzzer looks where the
proof could not. Neither subsumes the other, and the third worked example shows the boundary is not
even always "bounded versus unbounded": sometimes it is that nobody wrote the property down.

## Which parsers, and why those

The rule that picked them: does this code read bytes from outside the trust boundary? A parser
whose input this system produced is a correctness question. A parser whose input somebody else
produced is a security question, and it is the one where a panic has a consequence a user notices.

Seven parser targets, deliberately fewer than one per crate. A fuzz target that finds nothing
because it fuzzes a total function is worse than no target: it costs CI time on every commit and it
implies coverage the project does not have.

| Target | Input comes from | Why it earns a target |
|---|---|---|
| `device_tree_blob_walk` | firmware (QEMU, OpenSBI, a board's ROM) | parsed before anything else exists, on both ISAs, from a pointer in a register; a panic here is a kernel that cannot boot and cannot say why. Kani reaches the leaf readers and not the walkers. |
| `elf_parse` | any binary a user asks to run | the only parser that loads what it parses: its output becomes page-table entries. The whole-parse totality proof is recorded as intractable. |
| `globally_unique_identifier_partition_table` | a disk somebody else formatted | decides which LBA range is a filesystem. Heavily checked already, which is the point: the gap is *combinations* of hostile fields, which neither the proofs nor the single-byte mutation tests can build. |
| `domain_name_system_reply` | a name server, or anyone who can forge its reply | a reply that panics takes name resolution down, and one that slips past `Query::accept` sends a client to the wrong host. Kani covers the name decoder, not `accept`; see notes/name-resolution.md. |
| `http_response_feed` | a package server, over plain HTTP | network bytes, read in the progenitor **before** the digest check. Asserts read splits never change the answer. |
| `nifefs_roundtrip` | (structured) the writer's own output | not a panic hunt. Asserts that what goes in comes out, which is the property a truncation bug and a NUL bug in `write_image` each violated. |
| `package_archive_roundtrip` | (structured) the writer's own output | §197 (a package is one archive file)'s owed target: a written package reads back and verifies. |

### And the ones deliberately not fuzzed

Naming these matters as much as naming the targets, because "we have fuzzers" quietly grows to cover
everything. `Fs::parse` on raw bytes is proved total, so a target there could find nothing. The IPC
protocols are total by construction, and their real hazard, a double fetch, is a concurrent mutation
no byte-string fuzzer can express. `video_terminal` and `grant_plan` were left for stated reasons.
Each is argued in [fuzzing/not-fuzzed.md](fuzzing/not-fuzzed.md).

## Why cargo-fuzz

DECISIONS §46 (thin primitives or whole subsystems) makes taking a dev tool a decision too.
cargo-fuzz (libFuzzer) won over cargo-afl and honggfuzz-rs because it is in-process and
coverage-guided, so a target is a plain Rust function over `&[u8]`, the exact shape of these
`no_std` parsers. It needs nightly and `rust-src`, both already present, and adds three crates
nothing ships. `libfuzzer-sys` carries NCSA, so `deny.toml` holds a `[[licenses.exceptions]]` entry
scoped to that one crate. The comparison and the license reasoning are in
[fuzzing/why-cargo-fuzz.md](fuzzing/why-cargo-fuzz.md).

## The three bugs

Each is fixed and pinned by a host test, and each is told in full in
[fuzzing/three-bugs.md](fuzzing/three-bugs.md).

1. `dtb::Dtb::node_reg` indexed past its 16-entry cell stack on a tree nested 17 deep. Fixed by
refusing to match past the stack, as its sibling already did. It was found by reading the code while
writing the target; ten minutes of fuzzing did not find it. 2. `dtb::Region::end` overflowed on a
hostile memory map, a panic on the boot path. Fixed at the source (`Error::RegionOverflow`) and at
the type (`end()` saturates). The fuzzer found it in about ten minutes. 3. `nifefs::write_image`
accepted a name with a NUL in it, so `"a\0b"` was written and read back as `"a"`. Fixed with
`Error::NameHasNul`. `nifefs_roundtrip` found it in under a minute, as a property violation no
totality proof could see.

The crate and type were named `dtb` and `Dtb` when the first two were found; they are
`device_tree_blob` and `DeviceTreeBlob` since 2026-09-18.

## The CI job, and its budget

`script/fuzz --time 60`, every target `script/fuzz --list` names, in a job of its own on its own
runner. Two decisions worth defending.

Why a time budget at all. Fuzzing has no completion condition. A proof discharges or it does not;
a fuzzer runs until you stop it. So it cannot be a step inside a gate somebody waits on, because the
only knob is how long the wait is, and every second of it is paid by every commit forever. `ci.yml`
already has `cpu-matrix` as precedent for "its own runner, not a slower gate".

Why sixty seconds: measured on a developer machine, the slowest target runs about 19,000 inputs a
second, so a minute buys at least a million inputs per target from a corpus already past every
magic-number check. But the sweep is shallow. With both `device_tree_blob` fixes reverted, sixty
seconds from the committed seeds found nothing, and fifteen minutes found the overflow after 13.1
million executions. What keeps those bugs from coming back is
`crates/device_tree_blob/tests/hostile.rs`, which runs on every `script/test`: the fuzzer finds
things once, and a host test holds them forever. The measurements are in
[fuzzing/ci-budget.md](fuzzing/ci-budget.md).

What the job is for: it is a shallow sweep, not a search
and not a guarantee. A minute per target from the committed seeds re-covers the ground those seeds
reach quickly, which is where a change that breaks a parser outright will show. Finding something
genuinely new is a long run on a developer's machine, and that is deliberately not automated. A
nightly job that finds a crash at 4am with no persisted corpus produces an artifact nobody can
reproduce and an alert nobody dispositions, which is DECISIONS §35's wallpaper failure.

**The corpus is not cached between CI runs, on purpose.** Caching it would make a run's coverage
depend on which runs came before it, so a red run could not be reproduced from the repository alone.
Every run starts from exactly the committed seeds, which is the same starting point a developer
gets.

## Corpus discipline

Three kinds of input, and they are kept apart because they have different reasons to exist.

Seeds are committed, and they are read-only. A fuzzer starting from `[]` spends its first
minutes rediscovering that a device tree begins `d0 0d fe ed`. With a sixty-second budget it would
never get past the magic check. But `fuzz/seeds/` holds one small ELF per machine and nothing else,
because the seeds this project needs already exist in the tree.
`crates/device_tree_blob/tests/fixtures/` holds three real device trees dumped from the boards we
boot, and `crates/globally_unique_identifier_partition_table/tests/fixtures/` holds two real disks
formatted by `sgdisk` and by Apple's Disk Utility. `script/fuzz` passes those directories to
libFuzzer as extra corpus arguments. Copying them under `fuzz/` would create a second copy that can
drift from the first.

The exception is `fuzz/seeds/elf_parse/minimal_rx_<machine>.elf`, 120 bytes each, because nothing
else in the tree is a small ELF (our real binaries are over a megabyte). They are hand-assembled,
and [fuzz/seeds/README.md](../fuzz/seeds/README.md) carries the generator that produces them.

There is one per machine nife runs, because `elf` picks the `e_machine` it accepts at compile time;
`crates/elf/tests/fuzz_seed.rs` holds each build's seed to actually parsing and the directory to
carrying one for every machine in `elf::KNOWN_MACHINES`. A seed that stops parsing is not a seed,
and a fuzz run would not say so. Why, and the seed's measured effect, are in
[fuzzing/elf-seeds.md](fuzzing/elf-seeds.md).

**Dictionaries are committed** (`fuzz/dictionaries/*.dict`), and they are the cheap half of a
grammar. A device tree's structure block is a stream of 32-bit tokens; a fuzzer that has to discover
by bit-flipping that `\x00\x00\x00\x01` opens a node spends its whole budget in the first three
branches. `crates/globally_unique_identifier_partition_table`'s dictionary deliberately does *not*
try to help with the CRC-32, which is what the seeds are for.

**The working corpus is not committed** (`fuzz/corpus/`, gitignored). libFuzzer writes every input
that reaches a new edge, so minutes leave thousands of files and hours leave hundreds of megabytes.
It is machine-generated, machine-specific, and unbounded.

**Crash artifacts are not committed either**, and that is the discipline rather than an omission.
When a target finds a crash, the input becomes a host test in the crate that owns the bug, where
it runs in milliseconds on every `script/test` forever and where a reader meets it next to the code.
`crates/device_tree_blob/tests/hostile.rs` and `nifefs`'s `a_name_with_a_nul_in_it_is_refused` are
the two written this way. A hand-built 200-byte blob with a docstring explaining what it attacks is
worth more than a 7,642-byte artifact named after its SHA-1.

## EXAMPLES

Run everything, the way CI does:

```sh
script/fuzz                     # every target, 60 seconds each
```

See what the targets are:

```sh
script/fuzz --list
```

Hunt properly, on a machine you are not using:

```sh
script/fuzz --time 3600 device_tree_blob_walk   # one hour on one target
script/fuzz --time 0 elf_parse                 # until it finds something, or you press ^C
```

Reproduce a crash. libFuzzer writes the failing input to `fuzz/artifacts/<target>/` and prints
the command. Re-running the target with a *file* argument replays that one input instead of fuzzing:

```sh
cargo fuzz run device_tree_blob_walk fuzz/artifacts/device_tree_blob_walk/crash-b93bbc15...
```

See what the input actually was, which matters for a structured target like
`nifefs_roundtrip` where the bytes are not the data:

```sh
cargo fuzz fmt nifefs_roundtrip fuzz/artifacts/nifefs_roundtrip/crash-0ad4fab2...
# [
#     (
#         "\0",
#         [],
#     ),
# ]
```

Shrink it before writing the test:

```sh
cargo fuzz tmin device_tree_blob_walk fuzz/artifacts/device_tree_blob_walk/crash-b93bbc15...
```

Then throw the artifact away and write the test, for the reason under "Corpus discipline". Delete `fuzz/artifacts/<target>/` when you are done.

Prune a corpus that has grown (fuzzing eats disk; a long run's corpus is not small):

```sh
cargo fuzz cmin device_tree_blob_walk   # keep the smallest input per edge
rm -rf fuzz/corpus fuzz/artifacts
```

Add a target. Four steps, and the first decides whether the other three are worth doing.

1. **Answer "what does this find that a proof does not".** If the crate's Kani harnesses already
   cover the function's totality, do not add a panic-hunting target; add a *property* target, or
   nothing. `nifefs_roundtrip` is the worked example of the first, and the "deliberately not
   fuzzed" list above is the worked example of the second.
2. `fuzz/fuzz_targets/<name>.rs`, plus a `[[bin]]` block in `fuzz/Cargo.toml` (with
   `test = false, doc = false, bench = false`) and the path dependency on the crate under test.
   The header comment is not optional: say what the input is, where it comes from, and what the
   target adds over the proofs. That paragraph is what a reader needs to decide whether to trust it.
3. Give it seeds. Prefer a fixture that already exists, wired into `script/fuzz`'s per-target
   `case`; commit a new one only if nothing in the tree fits, and give it a test that it is still
   valid (`crates/elf/tests/fuzz_seed.rs`). A dictionary if the format has fixed tokens.
4. Add the name to `targets` and to `--list` in `script/fuzz`. The CI job needs no change: it runs
   every target, and its budget is per target, so a fifth one costs a minute.

## BUGS

**Ten minutes of fuzzing did not find the `node_reg` bug, and a reader did.** This is the most
important limitation on this page, because it is the one that contradicts what a fuzzer is assumed
to do. Reaching that panic needs a device tree seventeen levels deep whose *only* prefix match is at
the bottom, and the seed corpus is real device trees, which are three deep. A mutational fuzzer
explores outward from what it has; deep recursive structure is what it is worst at synthesizing. A
grammar-based generator (`arbitrary`-derived structure that emits well-formed trees) would reach it,
and is the obvious next step for `device_tree_blob_walk`.

**A green fuzz run means "not yet", never "correct".** Sixty seconds of no crashes is evidence about
sixty seconds. Nothing here is a proof, and nothing here should be quoted as one; the proofs are in
notes/verification.md and they say what they cover.

**The CI job would not re-find either `device_tree_blob` bug.** Measured, not assumed, under "The
CI job, and its budget" above. The regression guard is `crates/device_tree_blob/tests/hostile.rs`.

Only panics and hangs are caught, plus whatever a target asserts. A parser that returns the
*wrong answer* without panicking is invisible to `device_tree_blob_walk`, `elf_parse` and
`globally_unique_identifier_partition_table`, because those three assert nothing beyond "it
returned". `nifefs_roundtrip` is the one target with a real property, and it is the one that found a
silent-corruption bug. That asymmetry is a hint about where the next targets should go, not a fact
about fuzzing.

The needles in `device_tree_blob_walk` are a fixed list. The kernel's real lookups (`intc`,
`plic`, `pl031`, `virtio_mmio`) plus the empty prefix. A `pub fn` that panics for some *other*
prefix would not be found. Letting the fuzzer choose the needle would mean structuring the input,
which would stop the committed `.dtb` fixtures from working as seeds; that trade is worth revisiting
with a grammar.

`node_prop` matches at any depth and `node_reg` now stops at 16. The two lookups disagree about
how deep a device tree can be, which is a behavioral wart rather than a bug: `node_prop` keeps no
per-depth state, so it has nothing to overflow. Nothing in the tree nests past 4.

Sanitizers are on and buy very little here. The default is AddressSanitizer, and the four crates
under test contain no `unsafe`. It costs throughput and catches nothing these targets can produce.
It stays on because it costs nothing to leave a default alone, and because the first `unsafe` to
appear in one of these crates would be the one that needs it. The checks actually doing the work are
`debug-assertions` and `overflow-checks`, set in `fuzz/Cargo.toml`'s release profile, which is what
turned bug 2 from a silent wrap into a crash.

The `nifefs_roundtrip` target filters its own inputs, skipping name sets the writer refuses.
That keeps the budget on the accepted region, and it means the target cannot notice if a *rejection*
regresses. The crate's host tests cover that instead.

One block size. `globally_unique_identifier_partition_table` fixes 512 bytes because
`GloballyUniqueIdentifierPartitionTable::parse` takes the block size from the header block's length
and the fixtures are 512-byte disks. 4K-native disks exist and take a different path through
`array_blocks`.

`globally_unique_identifier_partition_table` barely reaches `check_backup`. The input is one
contiguous disk prefix, so the harness passes the *primary* header and array as the backup, which
fails on the first field comparison (`my_lba != alternate_lba`) almost every time. Everything past
that comparison, which is seven more fields and a second `check_entry_array`, is effectively
untested by this target. Reaching it wants the input split into a head and a tail, which the
committed `.head`/`.tail` fixture pair is already shaped for and which would cost those fixtures
their status as directly-usable seeds. Worth doing; not done.

## See also

- notes/verification.md, the proofs and their bounds. Read it first; this note is its complement.
- notes/scripts.md, where `script/fuzz` sits in the front door.
- notes/device-tree.md, notes/elf.md, notes/globally-unique-identifier-partition-table.md, notes/nifefs.md for what each parser is for.
- `deny.toml` and `script/supply-chain`, milestone 42's first two legs.

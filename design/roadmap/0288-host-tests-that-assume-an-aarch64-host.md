---
status: BUILT
raised: 2026-09-14
built: 2026-09-14
---
# 288. Host tests that assume the host is aarch64

**`cargo test -p elf` failed 20 of 25 unit tests, both integration tests and the module doc-test on
an `x86_64-unknown-linux-gnu` host.** `script/test` runs the host pass first, so on the commonest
developer machine there is it could not reach a single kernel leg. Every failure was
`Error::WrongMachine`, and the crate was right every time: the tests were the thing that was wrong.

## One sentence, written four times

`crates/elf` picks the ELF machine it accepts at compile time, which is correct and is documented as
correct, and it exports `NATIVE_MACHINE` **for this exact purpose**, under a doc comment that states
the problem before it happens:

> *"A test that forges an ELF header has to write some machine number, and writing the native one is
> what lets the forgery get past the machine check and reach the property actually under test."*

Four places wrote a machine **literal** where the meaning was host-relative. That is the whole
defect, restated four times:

1. `Builder::new()` hardcoded `EM_AARCH64`. Twenty of the twenty-five unit tests forge a header
   to reach some property past the machine check (a bad load address, a writable-executable
   segment), and on a non-aarch64 host all twenty died at the check instead.
2. `a_binary_for_the_other_supported_machine_is_refused` iterated `[EM_RISCV, EM_X86_64]`, under
   a doc comment reading *"these host tests build with `EXPECTED_MACHINE == EM_AARCH64`"*. On an
   x86_64 host that asserts an x86_64 binary is refused, which is the opposite of what this crate
   does and must do. The comment is the bug's own confession: a statement about the architect's
   laptop filed as a statement about the crate.
3. The module doc-test wrote `62u16` as its "foreign" machine. Both proposals said this test
   passed because its *header* uses `NATIVE_MACHINE`; one screen further down it does not, and it
   failed. Recorded because the diagnosis was handed over as complete and was not, and a reader who
   trusts the proposal over the run fixes three sites and is still red.
4. **`crates/elf/tests/fuzz_seed.rs` failed both its tests, and it was not wrong.** The committed
   seed genuinely is an aarch64 ELF, so on an x86_64 host `elf_parse` really did start from an empty
   corpus. That one wanted a seed, not an edit.

Nobody met any of it because every machine that has ever run this suite is aarch64: Apple Silicon
and `ubuntu-24.04-arm`. `EXPECTED_MACHINE` then happens to equal the literal and the whole class is
invisible.

## What was built

Rung one twice, and the ladder position is the point rather than the four edits. A comment is
what this crate already had, in the very doc comment quoted above, and it did not work: three of the
four sites are *within one screen* of the constant that exists to prevent them.

`FOREIGN_MACHINES`, derived rather than listed. `KNOWN_MACHINES` names the three machines nife
runs, in one place, and `FOREIGN_MACHINES` is that list minus `EXPECTED_MACHINE`, computed in a
`const fn`. A test can no longer write out "the other supported machines" by hand, because the
phrase now has a definition, decided by the same `cfg` that decides what is *accepted*. Its length
is `KNOWN_MACHINES.len() - 1`, so a new architecture that reaches `EXPECTED_MACHINE` without
reaching the list **fails the build** on the host that adds it.

`machine_no_nife_build_accepts`, checked at compile time. The "a machine nife does not run at all"
test uses SPARC (2), and that number goes through a `const fn` asserting it is in none of
`KNOWN_MACHINES`, so the next time this happens it is a build error rather than a test that quietly
asserts nothing. **Deliberately stronger than "not this build's machine"**: checking against
`EXPECTED_MACHINE` alone would fail only on an x86_64 build, which is to say only on a machine
nobody working on this had. Checking against all three fails on the aarch64 laptop where the mistake
gets made. A gate that fires only where nobody is standing is not a gate.

Three fuzz seeds, one per machine. `fuzz/seeds/elf_parse/` held `minimal_rx.elf`, aarch64, under
an honest note that a riscv64 build would reject it; the note predated x86_64 being a target and
never grew the third case. One seed cannot be right for three machines. There are now three, 120
bytes each, differing in the two bytes at offset 18 and nowhere else. `script/fuzz` already passed
the *directory* rather than a filename, so it needed no change. Selection is by the `e_machine` in
the seed's own header, **not** a `cfg` chain, because a default arm naming one architecture is the
precise shape of the milestone 161 trap; the choice cannot disagree with the bytes.

The `allow(dead_code)` attributes on the three `EM_` constants are gone, which is the tell that
this got better rather than bigger. They existed because each build could see only its own machine.

`every_machine_nife_runs_has_a_seed` replaced an assertion that *the* seed matched this build.
The old one was true and useful right up until the seed's machine was not yours, at which point it
reported a defect with no available fix. The new one compares the directory against
`elf::KNOWN_MACHINES`, so a fourth architecture with no seed fails on whatever host adds it rather
than on the first stranger who tries to fuzz there.

## The sweep, and it came back empty

`crates/elf` is where this bites first because it is the one crate whose logic is *about* machine
identity, but a `#[cfg(target_arch)]` constant paired with a hardcoded literal is a pattern rather
than an incident, so the whole host pass was run on this x86_64 box with `--no-fail-fast`:

```
cargo test --workspace --no-fail-fast --exclude kernel --exclude components --exclude fixtures \
    --exclude user_rt --exclude swap_proto --exclude virtio --exclude supervision_proto \
    --exclude system_initializer
```

150 test binaries, three failing targets, all three in `crates/elf`. Nothing else in the host
pass carries a host-architecture assumption that an x86_64 machine can see. The three other places
that forge ELF headers were read by hand as well and all three already do it right, each with a
comment saying why: `crates/measured_boot` (*"`elf::NATIVE_MACHINE` exists for exactly this"*),
`uefi_loader/src/image.rs` (*"`NATIVE_MACHINE` rather than a literal 62"*), and
`kernel/src/user/tests.rs`. The constant was used correctly everywhere except in the crate that
defines it.

That is a result worth stating plainly rather than leaving as a gap: the answer to "where else" is
nowhere else the host pass can reach. The other three host facts a test could accidentally
assume were swept for too, and the answer is better than expected rather than merely empty:
`target_pointer_width` and `target_endian` appear nowhere in this tree's Rust at all, and the one
`PAGE_SIZE` in a host-tested crate (`crates/paging`) is a plain `4096` with no `cfg(target_arch)`
near it, because page size here is a *parameter* threaded through the API (`Segment::page_range`
takes it, and `elf`'s fuzz target deliberately drives both 4 KiB and 16 KiB) rather than a fact read
off the host. The machine number was the only host fact any of this code had baked in.

## What was verified, and how

Every mechanism above was falsified before being believed, which is notes/verification.md's rule
applied to a gate rather than to a proof:

| Fault injected | Result |
|---|---|
| SPARC added to `KNOWN_MACHINES` | `error[E0080]: this is one of the machines nife runs, so some build accepts it` |
| `EXPECTED_MACHINE` removed from `KNOWN_MACHINES` | `error[E0080]: EXPECTED_MACHINE is not in KNOWN_MACHINES: ...` |
| `Builder::new()` reverted to `EM_AARCH64` | 19 of 25 red again |
| A fourth machine in `KNOWN_MACHINES` with no `EXPECTED_MACHINE` arm | **compiles, correctly**: that build accepts aarch64 and is right to call the newcomer foreign |

The last row is the one worth reading. The guard does not fire "the other way round", and
`FOREIGN_MACHINES`' doc comment writes the asymmetry out rather than overclaiming. For the second
row, the bound check runs ahead of the write, so the build error names its cause rather than
`index out of bounds`.

The seed generator in `fuzz/seeds/README.md` was re-run and reproduces the aarch64 seed
byte-identically to the file it replaces (`md5 b559ce57`), which is how the other two were
trusted; it is idempotent over all three.

## Where `script/test` gets to on this box, and a control that says why

The host pass is green end to end, `crates/elf` included (25 + 2 + 1), through the vendored
RedoxFS round trip and the `redoxfs_server` sans-IO core. That is the criterion this milestone was
set: the pass now *reaches* the kernel legs, which on this machine it had never done.

`script/test` still exits 1, on the aarch64 kernel leg, at
`a_keystroke_from_a_virtio_keyboard_becomes_a_terminal_byte`, whose own failure message names the
cause: *"the host's `sendkey a` is not reaching the device (is the monitor socket attached?)"*. That
is host-side QEMU interaction in a headless sandbox, and this lane changed no code the kernel runs:
outside `#[cfg(test)]` the whole diff is three removed `allow(dead_code)` attributes and three
additive `const` items, and `grep` finds no caller of any of the three outside `crates/elf`'s own
tests.

The control is better than that argument, because the failure flips with the emulator and nothing
else. This box has apt's QEMU 8.2.2 (no `riscv-iommu-pci`, which `script/qemu-check` reports) and
also a pinned 11.0.2 in `$HOME/.cache/nife-qemu`, built here by the lane that fixed `script/bootstrap`.
Same tree, same binary, same test:

| Emulator | keyboard test | `scanout` checks |
|---|---|---|
| apt 8.2.2 | **FAILS**: `sendkey` never reaches the device | **pass**, pixel for pixel, all three |
| pinned 11.0.2 | **passes** | **FAIL**: *"no screendump was ever taken (did QEMU get a monitor?)"* |

`inbound` and `multicast` fail under both, which is the host networking a sandbox does not have.

That table is a finding rather than a formality. The standing account of this box is that its
host-side referees fail because it is headless, full stop. They do fail, but *which* ones is decided
by the emulator version, and the two versions fail disjoint sets. Anybody reading a red post-run
check here has to know which QEMU was on PATH before the result means anything.

## Parity, and what could not be run here

This lane executed exactly one of the three hosts (`x86_64-unknown-linux-gnu`). DECISIONS §19
cuts both ways here, and the honest statement is that the aarch64 and riscv64 host cases are
*reasoned*, not run:

- **aarch64.** `NATIVE_MACHINE == EM_AARCH64`, so `Builder::new()` writes exactly what it wrote
  before and the twenty tests take the identical path. `FOREIGN_MACHINES` evaluates to
  `[EM_RISCV, EM_X86_64]`, which is the literal pair the old code listed. `seed_for_this_build()`
  finds `minimal_rx_aarch64.elf`, byte-identical to the file that was there. Every aarch64
  behavior is bit-for-bit what it was, which is the strongest form this argument can take, and it
  is why the change is safe to land from a machine that cannot run it.
- **riscv64.** The same derivation yields `[EM_AARCH64, EM_X86_64]` and `minimal_rx_riscv64.elf`,
  which is the case the old code was *documented as failing* and never tested. It should pass for
  the first time. Nothing here has run it; CI is arm64 and this lane's box is x86_64.

## BUGS

- No host in CI is anything but aarch64, so nothing stops this returning. Both proposals raised
  it and it is not closed here: CI is `ubuntu-24.04-arm` and the development machine is Apple
  Silicon, so a fifth site written tomorrow with a literal would pass every check in this
  repository. The two compile-time guards narrow the class (a foreign list cannot be written out by
  hand; a never-a-nife-machine number is checked on every host) but a plain `b.e_machine = 183`
  still compiles anywhere. The proposal below is the mechanism; this entry is what the record says
  in the meantime.
- `machine_no_nife_build_accepts` guards exactly one call site. A general facility with a single
  user is usually a smell. It is kept because that one user is the specimen that already failed once
  and cost a milestone, and because a second user is one `const` away.
- **The riscv64 seed has never been parsed by anything**, and neither had the x86_64 one before this
  lane. `the_fuzz_seed_is_a_valid_executable` only ever runs against the seed for the host running
  it, so on this box the riscv64 seed is asserted to exist and nothing more. The evidence that it is
  good is that it is byte-identical to the aarch64 seed, which *is* parsed and validated on every
  aarch64 run, except at the single byte the generator varies: `cmp` puts the only difference at
  offset 18, in all three directions. That is stronger than an argument and weaker than a run.
- Nothing checks the committed seed bytes against the generator that claims to produce them.
  Recorded in `fuzz/seeds/README.md`'s own `BUGS`, where a reader meets the directory.
- The module doc-test uses `FOREIGN_MACHINES[0]`, an index into a derived array. It means "any
  foreign machine" and any element would do, so the `[0]` is a wart the reader has to translate.
  Iterating would cost three lines of noise in an example whose whole job is to be readable.

## Follow-on

- **Milestone 414.** The
  post-run referees fail different sets under 8.2.2 and 11.0.2 on one machine, and nothing in their
  output says which emulator produced them, so a reader cannot tell an environment failure from a
  regression without re-running. Found by this lane's control table above, which is not what it was
  looking for.
- **Milestone 403.** Run the host pass on an x86_64 runner, so the class this milestone fixed
  cannot come back invisibly. It is a CI-shape question (a second job, a matrix leg, or
  `script/stranger-test` reaching a second host) with a real cost in runner minutes, which is why it
  was filed rather than done here. Numbered on 2026-09-19 by milestone 433's drain of the pile.
- **Recorded.** The five limitations above stay limitations in this block's `BUGS`, and the two a
  reader meets away from this file are written where they meet them: `fuzz/seeds/README.md`'s
  `BUGS` carries the dead-weight seeds and the ungated generator.
- **Done.** `design/roadmap/proposals/elf-host-tests-assume-an-aarch64-host.md`, written
  2026-09-12, is the file this block was promoted from: numbered, `git mv`d up a directory, index
  row added, which is the documented three-step promotion. Milestone 277's `Follow-on` moves from
  `**Proposed.**` naming that file to `**Milestone 288.**` naming this block, the same way 238's did
  when 277 itself was promoted.
- **Recorded.** A second proposal describing this same defect,
  `host-tests-that-assume-the-host-is-aarch64.md`, was written 2026-09-13 by a lane that could not
  see the first and lands with [#847](https://github.com/nifeos/nife/pull/847). It was not
  on this lane's base, so it could not be retired here, and this bullet is the record of that:
  whoever merges second deletes the file. A proposal whose work has landed is a second reading of
  the tree that disagrees with it. That two lanes filed the same proposal eight days apart is itself
  worth seeing, and is what `script/roadmap`'s pile is for.

## Index row

`cargo test -p elf` failed 20
of 25 unit tests, both integration tests and the module doc-test on an x86_64 Linux host, blocking `script/test`'s host pass before a single kernel leg. One defect written four times: a machine
literal where the meaning was host-relative, three of the four within a screen of the `NATIVE_MACHINE` that exists to prevent them. Rung one twice: `FOREIGN_MACHINES` is `KNOWN_MACHINES` minus `EXPECTED_MACHINE`, so the pair cannot be written by hand and a fourth
architecture fails the build; `machine_no_nife_build_accepts` checks a never-a-nife-machine number
in a `const`, on every host rather than only where it bites. Three fuzz seeds replace one that
could only be right for one machine. Whole host pass swept with `--no-fail-fast`: 150 test
binaries, three failing targets, all three in `crates/elf`, nowhere else. Four fault injections,
two of which corrected the change.

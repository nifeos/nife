---
status: BUILT
raised: 2026-09-04
built: 2026-09-04
---
# 255. A quarter of `kernel/src/arch/` has no assembly in it, and none of it is proved

Minted the same day by calef, from `design/fatal-risks/README.md` risk 2's
remaining amber, and built in this branch; notes/kernel-proofs.md is the record.
*(Number provisional until the merge queue lands it.)*

## What was built, and it met the bar as stated

Two properties, over code that lives in `kernel/src/arch/aarch64/iommu.rs` today, proved by
harnesses `script/verify` runs, both falsified before they were believed. Nothing was moved out of
`arch/`, which is what "not option B wearing this milestone's name" required.

The SMMUv3 driver was chosen over `x86_64/machine.rs` for a reason this block did not know and which
is the lane's main finding: `x86_64/machine.rs` cannot be reached from any machine this project
has. `arch/mod.rs` selects its subtree with `#[cfg(target_arch)]`, and under Kani the target is the
**host**, so `cargo kani -p kernel` on the `ubuntu-24.04-arm` runners compiles `arch/aarch64/` and
nothing of `arch/riscv64/` or `arch/x86_64/`. The two largest asm-free files in the table above are
out of reach for a `cfg` rather than for an `asm!` block. Nothing in the tree said so. That is
`design/roadmap/proposals/the-prover-only-ever-sees-one-architecture.md`.

So the target was `aarch64/iommu.rs`, which is 347 asm-free lines of confinement and the one file in
the shortlist that both compiles under the prover and carries risk 7's weight. `attach` built its
two entries inline, mixed with the volatile writes that make it unreachable. The arithmetic is now
three functions in the same file, and `attach` reads better for it (one `table_offset` where the
same stride had been written out four times).

- `the_smmu_is_handed_exactly_the_tables_the_kernel_built`. The context descriptor's TTB0 and the
  stream table entry's `S1ContextPtr` are each a 64-bit physical address split across two 32-bit
  words, and each shares its low word with control bits. For every page-frame `ttb` and every
  64-byte-aligned `ctxptr` below 2^52, both fields read back at the bit positions Arm IHI 0070 gives
  them are exactly the input. V, CONFIG and S1FMT are what they were written as rather than what
  an overlapping address bit made them. The second half is the worse failure. An address bit
  reaching CONFIG sets the stream to bypass, which is translation switched off.
- `no_stream_can_reach_another_streams_tables`. `table_offset` is the whole of the addressing for
  both tables and its `assert!` is the only thing between a device-tree-supplied `StreamID` and a raw
  write. For every pair of ids the bound admits, each entry lies inside the single frame that holds
  the table and no two entries share a byte.

Both falsifications leave every test in the tree green, which is the argument. QEMU's `virt`
board puts RAM at `0x4000_0000`, so bit 31 of a domain root is clear and `ttb >> 31` agrees with
`ttb >> 32` on this board. And the one PCIe disk's requester id is far below 64, so raising
`STRTAB_LOG2` to 7 (the edit this file most invites, and one that writes off the end of a frame)
is addressed by no test. `script/falsifications --sweep kernel` replays both, so this is `replayable`
rather than `attested`.

Cost: 0.11 seconds of solver time on `script/verify`'s budget. Almost all of the wall clock is
the kernel crate's own compile, which milestone 193 already pays.

One claim this block made was too strong and is corrected rather than quietly dropped.
notes/kernel-proofs.md said "all of `kernel/src/arch/`" is unreachable. The boundary is `asm!` and
MMIO, not the directory, and that wording is what let the architecture layer sit outside the prover
without anyone measuring whether it had to. It now says so.

In brief. Fatal risk 2 (the proofs prove trivia, and the real bugs live where Kani cannot reach)
is **AMBER**, and its own text names what keeps it there:

> `kernel/src/arch/`, `user/` and `xtask` are still out of reach, so the amber stands

Milestone 197 closed `user/` and `xtask` on 2026-08-31. `kernel/src/arch/` is what remains, and
milestone 193's `BUGS` says why it matters rather than merely being large:

> `kernel/src/arch/` stays unreachable under every option here, which means the architecture layer,
> where the VisionFive 2's undelivered-wake defect actually lived, is not what this fixes.

## The measurement that makes this a milestone rather than a wish

Counted 2026-09-04 across `kernel/src/arch/*/*.rs`:

| | lines |
|---|---|
| `arch/` total | **16,225** |
| files containing **no** `asm!` at all | **4,004 (24%)** |

A quarter of the architecture layer is ordinary Rust. The blanket claim that `arch/` is
unreachable because Kani cannot model `asm!` is true of `arch/` as a directory and false of a
quarter of its lines. The largest asm-free files:

| file | lines | what it is |
|---|---|---|
| `x86_64/irq.rs` | 903 | interrupt plumbing |
| `x86_64/machine.rs` | 762 | ACPI table walking, the RSDP and XSDT decode |
| `x86_64/iommu.rs` | 368 | VT-d |
| `aarch64/iommu.rs` | 347 | SMMUv3 |
| `riscv64/iommu.rs` | 314 | the ratified RISC-V IOMMU |
| `aarch64/isa.rs` | 299 | feature and granule discovery |

## Why these lines specifically

The three IOMMU drivers are 1,029 lines of confinement, and `design/fatal-risks/README.md` risk 7 (the
confinement claim is false) rests on what they do. They build device tables and page tables, and they
are the code that decides which physical addresses a device may touch. That is table arithmetic over
integers, which is exactly what a bounded model checker is good at and what a boot test is bad at.
A wrong entry that still confines *this* device on *this* machine is invisible to every test in the tree.


`x86_64/machine.rs` walks somebody else's tables. ACPI's RSDP, XSDT and MCFG arrive from firmware
and are untrusted input in the sense that matters: a length field, a checksum, a count. notes/x86-uefi-boot.md
records that under OVMF the kernel read a 118-descriptor memory map and a revision-2 RSDP where PVH
gave 9 and revision 0, so both branches are live and one of them had never executed until milestone
195.

## What this is not

**It is not a proof over the architecture layer**, and a block that promised one would be promising
what milestone 193 already refused: *"nobody should imagine a proof over the kernel"*. It is
reachability: harnesses can be written against these lines, one property at a time, the way they
already are for the crates.

**And it is not a licence to move code.** Milestone 193 chose option A (make the crate reachable with
the arch layer stubbed) over option B (lift pure logic into crates), and milestone 244 later declined
option B for `system_initializer` on measurement. The same discipline applies: if a property can be
proved where the code lives, prove it there.

## The proof that this milestone worked

One property over code in `kernel/src/arch/`, proved by a harness `script/verify` runs, and
falsified before it is believed: a real defect re-introduced and the harness turning red. That is
milestone 193's own bar and the reason its result was credible: milestone 191 found no harness in
this tree had ever caught a defect, and 193 answered by reintroducing milestone 142's real
`(count - 1).wrapping_mul(PAGE_SIZE)` and watching two harnesses fail.

Not a survey of what could be proved, and not a harness over a function moved out of `arch/` first.

## BUGS

- A stub is a hole in a proof, and a proof with an unexamined stub reads as coverage. Whatever
  boundary this lands on needs its stubs enumerated where a reader meets the harness, which is
  milestone 193's own recorded limitation and applies here with more force, since `arch/` is where the
  stubs are.
- **24% is a line count, not a difficulty estimate.** Some of those 4,004 lines are `const` tables and
  some are the hardest reasoning in the tree; the fraction says where to look, not how long it takes.
- `script/verify` already costs about 42 minutes and anything added lands on that budget. The
  `--affected-since` machinery exists because it is expensive.
- **The three IOMMU drivers are structural siblings but not identical**. A property proved of one
  is not thereby proved of the other two, and a harness that appears to cover all three by symmetry
  would be the overclaim this milestone is meant to remove rather than add.
- **Two harnesses over one file is not the architecture layer.** 347 of 16,225 lines, on one of
  three architectures, and the timer re-arm drift the corpus records inside `arch/` is still on the
  far side of the `asm!` boundary. This block promised reachability and delivered reachability; it
  should not be quoted as more.
- The register offsets and the bit positions are unproved and unprovable in this tree. Nothing
  here can check `CR0_SMMUEN`, or that CONFIG really is bits [3:1], against Arm IHI 0070; a harness
  asserting the constant it was given is a tautology. If the document was misread, the code and the
  proof are wrong together, and the boot-time confinement test is what stands against that.
- The other two IOMMU drivers gained nothing, exactly as this block's last BUGS entry warned.
  `riscv64/iommu.rs` writes its device context in 64-bit stores with no split at all, so the property
  proved here has no counterpart in it; `x86_64/iommu.rs` is not compiled under the prover on any
  runner this project has.

## Follow-on

- **Milestone 304.** `cargo kani -p kernel` compiles only the host's `arch/` subtree, so
  `x86_64/irq.rs` (903 lines) and
  `x86_64/machine.rs` (762), the two largest asm-free files this block named, cannot be reached from
  any machine this project has. Reaching them is a second `script/verify` row on an x86_64 runner,
  which needs `aarch64-cpu` behind a target `cfg` first.
- **Recorded.** notes/iommu.md, "What is proved, as against tested". The SMMUv3's register constants
  cannot be proved against Arm IHI 0070 by anything in this tree, so the boot-time confinement test
  in `kernel/src/virtual_io.rs` is not made redundant by these harnesses and neither replaces the other.
- **Recorded.** notes/kernel-proofs.md, "The stub boundary, enumerated" and BUGS. Everything in
  `aarch64/iommu.rs` that touches the SMMU (`init`, `cmd_push`, `attach`, `take_fault`) is
  unreachable to a harness because it dereferences `phys_to_virt`, so what is proved is the
  word-building the hardware then reads and not the writing of it. The same note now records that
  only one architecture's `arch/` is ever compiled.
- **Refused.** Proving the RISC-V IOMMU's device-context construction by symmetry with the SMMUv3's.
  The two drivers rhyme and their entry-building does not: RISC-V writes 64-bit stores with no split
  across words, so there is no counterpart property, and asserting one would be the overclaim this
  milestone was minted to remove. It also could not be run: see the proposal above.

## Index row

two harnesses over the SMMUv3's stream table and context descriptor, both replayably falsified;
and `cargo kani -p kernel` turns out to compile only the host's `arch/`

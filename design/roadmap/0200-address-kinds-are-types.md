---
status: NOT-STARTED
raised: 2026-08-31
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 200. A virtual address and a physical address stop being the same type

The question *"what is wrong with the tree-wide refactor"* has no answer on merit, so the refactor
is a milestone.

Nothing needs deciding. The names are ratified below and the sequencing constraint is
practical rather than a fork.

**In brief.** Physical and virtual addresses are bare `u64` everywhere in this tree: `paging`,
`memory_regions`, `kernel/src/arch/*/mmu.rs`, `kernel/src/user.rs`, `syscall.rs`. Nothing stops one
being passed where the other is meant, and the mistake compiles, runs, and passes tests wherever the
two happen to be equal.

## The motivating defect, which is recorded rather than hypothetical

`notes/higher-half.md` has it. A test dereferenced the device tree's **physical** address, and the
identity map made it work by accident:

> That's a test dereferencing the device tree's **physical** address. Before, the identity map
> made it work by accident. Now, a low address does not exist, and the mistake faults on the
> spot with the offending address printed.
>
> -- notes/higher-half.md

The same note names the general shape and its consequence: *"an identity map that lingers is an
identity map that hides physical/virtual confusion, right up until userspace shows up and the
confusion becomes a security hole."*

**That is the whole case.** The mistake compiled, ran, passed, and was caught by an accident of the
address space going away rather than by anything asking. A type asks.

This is also the shape DECISIONS §134 (a harness carries a machine-replayable falsification record)
and milestone 191 (did the proofs catch the bugs?) have been circling all week: a defect no proof was
positioned to see, in code no harness reaches, prevented instead by making it unsayable. AGENTS.md's
ladder puts that at rung one, above a gate and far above a comment.

## The names, ratified 2026-08-31

| type | what it is |
|---|---|
| `VirtualCpuAddress` | virtual, as the CPU sees it |
| `VirtualDeviceAddress` | virtual, as a device sees it: the IOVA the IOMMU translates |
| `PhysicalMemoryAddress` | the one physical reality both translate into |

Two virtual views, one physical fact. The asymmetry says something rather than being untidy:
physical takes no view qualifier because there is only one physical address space, shared by the CPU
and by devices after translation. `notes/iommu.md`'s identity-mapped domain is that statement made
concrete, since `IOVA == PA` there.

Refusals, kept because they are the half a future proposer needs:

- `VirtualAddress` / `PhysicalAddress`, the terms of art, recommended twice by the maintainer and
  declined twice. They carry the recognition §39's protected class exists to preserve, and calef
  preferred explicitness. The recognition was spent deliberately, which is why this bullet
  exists rather than the choice being left to look inevitable.
- `PhysicalCpuAddress`, incoherent: it asserts a non-CPU physical address exists. It also would
  not have compiled, since `script/lint` runs clippy at `-D warnings` and `upper_case_acronyms`
  rejects `CPU` in a type name.
- `VirtualMemoryAddress`, because an IOVA is a memory address too, so "Memory" fails to
  disambiguate the one kind that needed it.
- `DeviceMemoryAddress`, which parses as *an address of device memory*, meaning MMIO, and hides
  that an IOVA is virtual at all.
- `IoVirtualAddress`, only because it breaks the whose-view parallel and spends an abbreviation.

## Where the types stop, and this is the design work

Syscall arguments are `u64` by ABI, so the wrappers cannot cross the syscall boundary. That edge
has to be designed rather than discovered: the conversion belongs at the entry point, once, where a
reader can see the untyped word becoming a typed address and check the claim being made about it.
Everything above that line should be typed; nothing below it can be.

`VirtualDeviceAddress` has no consumer today, because `IOVA == PA` under the identity map. Introduce
it anyway or leave it for milestone 143 (silicon IOMMU): this block does not decide, and the case for
introducing it now is that the type is what makes the identity mapping's assumption visible instead
of implied.

## Sequencing, which is the only real constraint

The blast radius is `paging`, `memory_regions`, three `arch/*/mmu.rs`, `kernel/src/user.rs` and
`syscall.rs`, and `kernel/src/user/tests.rs` is the merge hotspot AGENTS.md already names. Do this
when `kernel/src` is quiet, not while lanes are open in it.

Kani was checked first, and it is clear. Milestone 193 (put `kernel/src` within reach of the
prover) landed harnesses proving run arithmetic over `syscall.rs` in `u64`, so a newtype that
confused CBMC would cost this milestone those proofs. Measured 2026-08-31 rather than assumed: a
probe crate stated milestone 193's own `run_end_va` property twice, once on bare `u64` and once
through a `repr(transparent)` `VirtualCpuAddress`, both in `u128` so neither harness could repeat its
implementation back at itself.

`VERIFICATION:- SUCCESSFUL`, 2 of 2, 0.068s for both. The wrapper adds no measurable solver cost,
which is the number that mattered against `script/verify`'s ~650 seconds.

Two things the probe settles for whoever builds this:

- `kani::any()` needs no new shape. `VirtualCpuAddress::new(kani::any())` generates the raw
  `u64` symbolically and wraps it, which is what a converted harness would write. No `Arbitrary`
  implementation is required, though deriving one would read better across 145 harnesses.
- No unwind or bound effects. The arithmetic is identical to the solver either way, which is what
  `repr(transparent)` promises about layout and is now observed rather than trusted.

## BUGS

- A newtype protects only while the value stays wrapped. If every consumer immediately unwraps,
  the net protection is close to zero. The milestone is worth doing only if the mapping APIs *take*
  the types, and a version that adds wrappers without changing signatures should be rejected as
  worse than nothing, because it looks like a mechanism and is not one.
- It cannot stop a deliberate wrong unwrap, only the silent interchange.
- Nothing here types MMIO addresses, which `notes/unsafe-obligations.md` records
  `gic.rs::initialize` taking on trust, and which are physical addresses being used as virtual ones
  through a mapping. That is arguably the next instance of the same problem and is not in scope.
- The refactor touches the most delicate code in the project for a benefit that is invisible when
  it works. That is the honest cost, and it is why the sequencing constraint above is not optional.

## Index row

"What is wrong with the tree-wide refactor" has no answer on merit. Physical and virtual addresses
are bare `u64` across `paging`, `memory_regions`, three `arch/*/mmu.rs`, `user.rs` and `syscall.rs`.
The confusion is recorded rather than hypothetical: `notes/higher-half.md` has a test
dereferencing a physical address that the identity map made work by accident, and names the
consequence as a security hole once userspace exists. Names ratified: `VirtualCpuAddress`,
`VirtualDeviceAddress`, `PhysicalMemoryAddress`. Two virtual views, one physical fact. Do it when
`kernel/src` is quiet, and check Kani first.

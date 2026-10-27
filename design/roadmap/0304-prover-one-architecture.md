---
status: BUILT
raised: 2026-09-04
built: 2026-09-16
promoted_from: the-prover-only-ever-sees-one-architecture
---
# 304. `cargo kani -p kernel` only ever compiled one architecture, and it was the runner's

## The premise, re-measured rather than inherited

The proposal's claim was that `arch/mod.rs` selects its subtree with `#[cfg(target_arch = ...)]`,
that Kani compiles for the host, and that therefore the prover saw `arch/aarch64/` and nothing else.
AGENTS.md asks whether the premise is true before anything is built on it, and this tree has carried
a wrong premise through every gate before, so it was checked with a probe rather than by reading.

Two deliberately-failing harnesses, one appended to `arch/riscv64/mod.rs` and one to
`arch/x86_64/mod.rs`, then the suite on the dev Mac (`aarch64-apple-darwin`):

```console
$ cargo kani -p kernel -Z unstable-options --ignore-global-asm --output-format=terse
Manual Harness Summary:
Complete - 4 successfully verified harnesses, 0 failures, 4 total.
```

Four harnesses, zero failures, with two harnesses present that assert `false`. Neither subtree was
compiled at all. The premise is exactly true.

## What it actually took, which was more than a row and a label

The proposal priced the mechanism as "a `script/verify` row and a runner label, both of which
already exist in other shapes", and named one prerequisite: `aarch64-cpu` going behind a target
`cfg`. That prerequisite was already done (2026-08-31, recorded in `kernel/Cargo.toml`'s own
comment), so the proposal was one item lighter than it thought and one item heavier than it knew.

**`arch/x86_64/` did not compile under Kani, and the reason was a toolchain skew rather than
anything about the code.** On cordoba, the project's x86_64 Linux box, with the same Kani 0.67.0:

```
error[E0133]: call to unsafe function `core::arch::x86_64::__cpuid` is unsafe and requires unsafe block
  --> kernel/src/arch/x86_64/isa.rs:49:17
```

Four of those, in `isa.rs` and `mod.rs`. `__cpuid` is a **safe** function on the toolchain this tree
pins, which is why every call site read it bare and said so in a comment. It was an `unsafe fn`
until upstream changed it, and Kani bundles its own rustc, ten months behind ours (`kani-0.67.0`
pins `nightly-2025-11-21`, rustc 1.93.0). The repo's own nightly compiles the same crate for
`x86_64-unknown-linux-gnu` with no errors at all, which is how the skew was attributed rather than
guessed at.

So `arch/x86_64/` was out of the prover's reach for two independent reasons, and the proposal
knew about one. The fix is `isa::cpuid` and `isa::cpuid_count`, two private helpers that wrap the
intrinsic in an `unsafe` block the old toolchain needs and an `#[allow(unused_unsafe)]` the new one
needs. That allow is an **exception and is labeled as one** where a reader meets it, per AGENTS.md's
ladder: it is load-bearing for the prover and a foot gun for anyone who reads it as "this call wants
auditing". It comes out when Kani's pin passes the change.

## What the prover sees now

| host | `arch/` subtree compiled | harnesses run |
|---|---|---|
| aarch64 (dev Mac, `ubuntu-24.04-arm` runners) | `arch/aarch64/` | 4: two in `syscall.rs`, two in `arch/aarch64/iommu.rs` |
| x86_64 (cordoba, the new `ubuntu-24.04` runner) | `arch/x86_64/` | 4: the same two in `syscall.rs`, two new in `arch/x86_64/irq.rs` |
| riscv64 | nothing | nothing here; milestone 589 reaches it |

Six distinct harnesses in `kernel`, up from four. No harness went red on a newly-reachable
architecture, which is the question this lane was sent to answer: the two portable `syscall.rs`
properties pass on x86_64 exactly as they do on aarch64, so nothing there was quietly
architecture-specific. That is a clean result and a small one, because the portable harnesses were
the only pre-existing candidates. The two `arch/aarch64/iommu.rs` harnesses cannot run on an x86_64
host for the same `cfg` reason this milestone is about, and are not expected to.

riscv64 is out of this milestone's reach: GitHub offers no riscv64 image, stock Kani has no
cross-target flag, and `radon` is a lab board, not a runner. Milestone 589 (Kani can prove riscv64
from the hosts we already have) reaches it, because CBMC checks riscv64 goto programs on any host:
a carried Kani patch proves the `kernel` row for riscv64 from the arm64 runner.

## The finding, which is worth more than the plumbing

Pointing a model checker at `arch/x86_64/irq.rs` for the first time produced a red harness, and the
defect is real rather than an artifact of the harness.

`an_owned_gsi_routes_inside_the_io_apic_band` failed, with the counterexample `base = 127`,
`entries = 129`, `gsi = 255`. `redirection_index(255)` returns `Some(128)`, so this IO APIC owns
that global interrupt; `gsi_vector(255)` is `GSI_VECTOR_BASE.wrapping_add(255)` = `0x2f`, which is
inside the local APIC's own vector band. `enable` passes that vector straight to `route_gsi`.

The plausible-hardware version of the same case is worse and needs nothing exotic: a second IO APIC
owning global interrupts from 200, with the 24 redirection entries every real part has, admits GSI
210. `gsi_vector(210)` wraps to vector 2, the NMI.

`MAX_REDIRECTION_ENTRIES`'s own doc claims this cannot happen. It is wrong about which quantity it
bounds: it caps the *entry count*, not the GSI. The thing actually standing between a
firmware-supplied GSI and a bad vector is `route_gsi`'s panic, which is a dead kernel rather than a
guarantee. Nothing has hit it because this kernel records one IO APIC and QEMU's q35 gives it base
zero, where the flat map is exact.

**It is recorded and not fixed, deliberately.** The correct vector is
`GSI_VECTOR_BASE + redirection_index(gsi)` rather than `+ gsi`, and that costs `gsi_vector` its
`const fn`, its total signature, and the "flat and reversible" property three doc comments in
`irq.rs` and one in `exceptions.rs` rest on. That is a public-function signature change and a policy
decision about multi-IO-APIC machines the kernel does not otherwise support, which is an architect's
fork rather than this lane's. It leaves here in the two shapes AGENTS.md allows: a `BUGS` entry in
`irq.rs`'s module header, where a reader meets the feature, and
`design/roadmap/proposals/the-gsi-vector-map-wraps-on-a-second-io-apic.md`.

## The two new harnesses, and what falsified them

Both are in `kernel/src/arch/x86_64/irq.rs`, beside the code, under the stub list
notes/kernel-proofs.md already enumerates. Both are about numbers firmware chose, which is the
strongest case in this tree for a bounded model checker. A vector is an index into the IDT, so
getting one wrong does not misroute an interrupt, it runs the page-fault handler.

- `no_vector_belongs_to_two_bands`. `MSI_VECTOR_BASE`'s doc claims the three vector bands are
  "disjoint by construction rather than by anybody remembering". This is that claim, over every
  entry count an eight-bit version register can report and every state the MSI bump counter can be
  in, asked through the two predicates the trap handler actually calls rather than restated from the
  constants.
- **`an_owned_gsi_routes_inside_the_io_apic_band`.** For every GSI the guard admits, the flat map
  lands inside `GSI_VECTOR_BASE..MSI_VECTOR_BASE`. The same shape as milestone 255's
  `no_stream_can_reach_another_streams_tables` one architecture over: a firmware-supplied
  identifier, one bounds check, a write that cannot be taken back. Its assumption that the IO
  APIC's GSI base is zero is the finding above, written at the harness rather than hidden in it.

Falsified twice, on cordoba, and the asymmetry is the honest half. Raising
`MAX_REDIRECTION_ENTRIES` by one turns *both* red. Loosening `redirection_index`'s own bound from
`index < entries` to `index <= entries` turns only the second red, correctly: the first never calls
the guard.

Both are recorded `attested` rather than `replayable`, and the reason is this milestone's own
subject one level out. `script/falsifications --sweep kernel` runs a named harness on whatever host
it is on, so a `replayable` record for an x86_64 harness would fail the sweep on the dev Mac and on
every CI runner but the new one. That has been true of the `arch.aarch64.iommu` patches since
milestone 255 wrote them, facing the other way, and went unnoticed because every machine here was
aarch64. It is now in `script/falsifications`' `BUGS`.

## Cost, measured

- From the first CI log that carried it, which is the machine this column should be taken on
  (`script/verify`'s own table says so about its dev-Mac numbers). Run 2026-09-16, GitHub
  `ubuntu-24.04`: the whole job is 41 seconds, of which `script/verify --only kernel` is **3**,
  Kani's install 15 and the cache restore 17. The proving itself is 0.52s of solver across four
  harnesses and 1.1s of `kernel`'s own compile. The measurement it replaces was ~60s from cold on
  cordoba, which was the right order and the wrong machine.
- It was checked for the invisible green rather than trusted, because three seconds is fast
  enough to look like a job that proved nothing, and that exact failure is on this file's record
  twice. The log names all four harnesses:
  `arch::x86_64::irq::proofs::no_vector_belongs_to_two_bands`,
  `arch::x86_64::irq::proofs::an_owned_gsi_routes_inside_the_io_apic_band`, and `syscall.rs`'s two.
  A pass here is a pass on the architecture this milestone exists to reach.
- One extra CI runner, in parallel with the two `prove` shards, which are 15 minutes each. It
  adds nothing to the critical path. `VERIFY_JOBS` is left at the script's default rather than
  pinned to 2 like the shards, because the memory kill that forced theirs was four concurrent CBMC
  formulas at `glob`'s size and these are milliseconds.
- What each host actually proves, measured on 2026-09-16. `VERIFY_JOBS=2 script/verify` on the
  dev Mac: exit 0, 25 crate runs, 148 harnesses. `script/verify --only kernel` on cordoba: exit
  0, 4 harnesses. The tree carries 150. No single machine proves all of them, and that is now
  a property of the suite rather than an oversight: the two figures overlap on `syscall.rs`'s two
  and differ on the four architecture-specific ones. A reader who takes "148" as the tree's coverage
  is off by the two this milestone added, which is why notes/verification.md's counted claim now
  says so on the same line as the number.
- **It does not multiply the matrix.** `--only kernel` proves the one row whose meaning depends on
  the machine; re-proving the other eighteen portable crates on a second host would buy a runner's
  time and no coverage.

## What this does not close

- `arch/riscv64/` is zero here, for the reasons above; milestone 589 is what proves it.
- `arch/x86_64/` is four harnesses in one file. The proposal's strongest candidate was
  `x86_64/machine.rs`, the ACPI walk, which is 836 lines of untrusted firmware parsing and is now
  *reachable* for the first time. It is not proved. Its parsing half already lives in
  `crates/machine_discovery`, which carries no harnesses and is in no verify row. The volatile half
  in `machine.rs` reads raw pointers into the direct map, which notes/kernel-proofs.md's stub list
  item 5 says to stub rather than pretend. Finding the seam is its own lane.
- **The `--only` flag's name is provisional**, like everything a lane mints.
- Fatal risk 2 stays AMBER. This narrows it: the architecture layer is no longer one-third
  visible to the prover by accident of a runner label. It does not clear it, and the honest number
  is still that `kernel/src/arch/` is 16,225 lines with six harnesses over two files.

## Follow-on

- **Milestone 308.** Promoted from
  `design/roadmap/proposals/the-gsi-vector-map-wraps-on-a-second-io-apic.md` on 2026-09-16, the day
  it was written, when calef chose option 1 (route by index). The
  defect this milestone's own proof found: `gsi_vector` was a flat `GSI_VECTOR_BASE + gsi` and
  `MAX_REDIRECTION_ENTRIES` bounds the entry count rather than the GSI. An IO APIC whose global
  interrupt base is not zero can route an owned line onto the NMI. The fix costs `gsi_vector` its
  `const fn` and its total signature, which makes it calef's fork rather than a patch.
- **Recorded.** `kernel/src/arch/x86_64/interrupt_controller.rs`, module `BUGS`. The same defect,
  stated where a
  reader meets the feature, with the plausible-hardware case and why the harness assumed it away.
  Milestone 308 fixed the defect and removed the assumption; that entry now records what remains,
  which is that the multi-IO-APIC path ships unexecuted.
- **Recorded.** `script/falsifications`, `BUGS`. `--sweep` replays a harness only on a host whose
  architecture compiles it, so an architecture-specific harness can only be `attested`. True of the
  `arch.aarch64.iommu` patches since milestone 255 and unnoticed because every machine here was
  aarch64. Teaching the sweep to skip what it cannot compile is deliberately refused: a skip is the
  invisible direction, and a per-record host statement is a format change and so calef's.
- **Recorded.** notes/kernel-proofs.md, the stub list (new item 3) and `BUGS`. Also that the
  `kernel` row now means something different on each of two hosts, which no single number in
  `script/verify`'s table can say.
- **Milestone 423.** This
  milestone came from `the-prover-only-ever-sees-one-architecture`, which is now marked promoted and
  so is no longer where untaken work can live. Its strongest target,
  `x86_64/machine.rs`'s ACPI walk, is reachable for the first time and still unproved: 836 lines
  over firmware-supplied lengths, checksums and counts. This item was first written up as milestone
  431, on the reading that the walk's parsing half in `crates/machine_discovery` carried no
  harnesses and no `script/verify` row. Milestone 319 (the crate that parses firmware had no
  proofs) proved that crate on 2026-09-17, so 431 is
  `SUPERSEDED` and keeps the account. What is left is the volatile half's bounded read against
  the direct map, which is milestone 423. Finding that seam wants a lane; it is
  not folded into this block because it is a design question rather than a mechanical one.

## Index row

the prover compiled one architecture's `arch/` and it was whichever the runner happened to be, so
two thirds of the architecture layer was unreachable for a `cfg` rather than for an `asm!` block.
the `kernel` row is now proved on an x86_64 host as well, which took four `unsafe` blocks Kani's
ten-month-old bundled rustc still wants. The first proof ever pointed at `arch/x86_64/` found a
GSI that routes onto the NMI

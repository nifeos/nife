# Machine-checked proofs (Kani)

The companion to the verification thesis (DECISIONS §14 (the project's direction)). That decision says *why* we verify; this
note is *how*, and the record of the experiment that green-lit it. The harness tables, the bounds
and each crate's story are in [`verification/`](verification/README.md), one appendix per topic.

## Tests sample; proofs quantify

The `capability` tests check the cases we thought to write: READ cannot become WRITE, an empty slot is
`NoSuchSlot`, a derived cap names the same object. Good tests, but they say nothing about the inputs
we did not enumerate. A proof harness asks a different question. `kani::any()` is an unconstrained
value, so:

```rust
#[kani::proof]
fn derive_never_widens_rights() {
    let src_rights = Rights(kani::any());   // ALL 2^32 patterns at once
    let requested  = Rights(kani::any());
    let mut cs: CapabilityTable<u8> = CapabilityTable::new(2);
    cs.put(0, Cap { object: 0u8, rights: src_rights }).unwrap();
    if cs.derive(0, 1, requested).is_ok() {
        assert!(cs.get(1).unwrap().rights.is_subset_of(src_rights));
        assert!(requested.is_subset_of(src_rights));
    }
}
```

proves "no reachable state widens rights," not "the states we tried did not." Kani compiles the
function to a logical formula and hands it to a SAT solver; `SUCCESS` means there is no assignment of
the symbolic inputs that trips an assertion or panics.

## How it actually works

The surprising part is that Kani checks "every input" without running every input. It does not loop
over 2^64 values. It reasons about them symbolically.

1. Symbolic input. `kani::any()` is not a random value. It is a placeholder standing for *all*
   values at once, an unknown the tool carries as algebra.
2. The program becomes a formula. Kani traces the harness over that unknown, turning each
   operation and branch into a logical constraint. In `index`, the `(va >> shift) & 0x1ff` becomes an
   expression in the *bits* of `va`, not a number, and the `assert!` becomes a claim about that
   expression.
3. A solver hunts for a counterexample. The claim, negated, goes to a SAT/SMT solver whose one
   job is to answer "is there any assignment of these bits that makes this false?"
   - UNSATISFIABLE = no such assignment exists = the property holds for every input. The proof.
   - SATISFIABLE = here is an exact input that breaks it. A counterexample, printed for you.

That is why `paging` verified in ~12 milliseconds: it is not 2^64 executions, it is one algebra
problem about the bits.

## What "bounded" means, and the one honest limit

A solver reasons completely about *fixed-size* things: a 64-bit integer, a four-level walk, a
two-slot table. What it cannot swallow whole is an *unbounded* loop or an arbitrarily large
structure, which would build an infinite formula. So Kani bounds: it unrolls loops to a limit and
gives structures concrete sizes.

The `paging` and `capability` harnesses have no unbounded loops (the four levels are literally four),
so their proofs are *complete*, not "up to a bound." But consider a harness that reasons over
`map_range` for a symbolic `count`, or the `Mapper` building tables. You either bound it (prove it
for count <= N) or reach for a heavier technique (induction, a tool like Verus). "Bounded model
checking is automatic but only reasons up to the bound" is the whole trade.

## What a green check does and does not mean

A proof is only as good as four things, and each is worth being blunt about:

1. It proves what you *asserted*, not what you *meant*. A wrong assertion verifies happily and
   means nothing. The harness is the specification, so it must be read as carefully as the code it
   checks. This is the main failure mode, not solver bugs.
2. It covers only what the model captures. Kani models Rust's semantics. It does not model the
   hardware, and `unsafe` that breaks Rust's assumptions is outside it. That is exactly why we verify
   the pure-logic crates (`capability`, `paging`'s arithmetic) and not the `arch/` assembly: the
   model is faithful where there is no hardware and no `unsafe`. It is also why §14 promises a
   *small verified TCB with an unverified layer beneath it*, not a proof of the whole machine.

   Concurrency is the sharpest edge of this limit. Every queue and endpoint proof here is
   single-threaded. The wake-before-switch-out race (notes/intrusive-queues.md) lived precisely in
   the SMP interleaving those proofs cannot see. Green harnesses and a real race coexisted; the flaky
   test found it.

   And there is a fourth edge of the same limit that is not about the model at all: the prover
   compiles for the host, so it sees one architecture (milestone 304 (`cargo kani -p kernel` only
   ever compiled one architecture)). `kernel/src/arch/mod.rs` selects its subtree with
   `#[cfg(target_arch = ...)]`. So a Kani run on an aarch64 box compiles `arch/aarch64/` and no line
   of the other two. This is worse than the `asm!` boundary above rather than milder, because it is
   silent. An unsupported construct is reported, but a `cfg`-excluded file produces no diagnostic of
   any kind, and the suite goes green faster. The `kernel` row is proved on aarch64, x86_64 and
   riscv64 for exactly this reason, riscv64 by a patched Kani (milestone 589 (Kani can prove riscv64
   from the hosts we already have)). See notes/kernel-proofs.md for what each run reaches.
3. The harness itself is code, and until milestone 113 (the proofs' own unsafe code) it was code no
   gate read. `cfg(kani)` is set by the model checker and by nothing else. So `script/lint` never
   compiled a single `#[cfg(kani)] mod verification`, and `clippy::undocumented_unsafe_blocks` could
   not fire in one. The harnesses that set up the queue and endpoint proofs are `unsafe`, and
   thirteen of those sites had no SAFETY comment: an unexamined assumption inside the thing that
   exists to examine assumptions. `script/lint` now compiles them all against a shim
   (`helpers/kani-lint-shim/`), which is a lint pass and not a second proof. See
   notes/unsafe-obligations.md.
4. The tool is trusted. Kani, its CBMC backend, and the SAT solver could have bugs. They are
   small and widely used, and the solver emits a checkable certificate, but it is a trust assumption.
   seL4 minimizes even its proof checker; we do not, and that is a stated limit.

## What is proved today

One row per crate this note covers, with its central property. Each appendix has the full harness
table and the reasoning.

| Crate | Harnesses | What it proves | Appendix |
|---|---|---|---|
| `capability` | 8 | derivation and the inheriting mint never widen rights; a deleted capability stays deleted, and deleting touches only its slot | [core](verification/core-harnesses.md) |
| `memory_regions` | 2 | untyped carving stays within budget; a child region never frees to the allocator (no double-free) | [core](verification/core-harnesses.md) |
| `paging` (walk) | 8 | table indices in bounds; distinct pages take distinct paths; the halves are disjoint; descriptors keep address and permissions apart | [core](verification/core-harnesses.md) |
| `paging` (blocks) | 8 | a 2 MiB or 1 GiB leaf is aligned, inside the span and the largest that fits; block encodings on three formats | [blocks](verification/paging-blocks.md) |
| `paging::domain` | 6 | an IOMMU domain maps exactly the granted pages, in both directions | [IOMMU](verification/iommu-domain.md) |
| `page_frames` | 5 | two allocations are distinct; frames are aligned and in range | [core](verification/core-harnesses.md) |
| `device_tree_blob` | 4 | the big-endian leaf readers are total | [core](verification/core-harnesses.md) |
| `inter_process_communication` | 6 | at most one wait queue is non-empty; rendezvous is exact; a collected sender is forgotten | [core](verification/core-harnesses.md) |
| `generational_table` | 3 | a removed name never resolves again | [core](verification/core-harnesses.md) |
| `intrusive_fifo` | 1 | any push/pop interleaving is FIFO and lossless | [core](verification/core-harnesses.md) |
| `address_space_identifier` | 3 | ASID 0 is never allocated; live ASIDs are distinct | [core](verification/core-harnesses.md) |
| `elf` | 4 | segment bounds arithmetic is total and yields in-bounds ranges | [parsers](verification/parser-harnesses.md) |
| `nifefs` | 2 | the initrd parse's acceptance makes reads in-bounds | [parsers](verification/parser-harnesses.md) |
| `pci` | 4 | config-space decode is total on any device's answers | [parsers](verification/parser-harnesses.md) |
| `direct_memory_access_validator` | 7 | no descriptor the device reads escapes the driver's grant | [DMA](verification/direct-memory-access-validator.md) |
| `calendar` | 11 | the date algorithms are mutual inverses over the whole range | [calendar](verification/calendar-proofs.md) |
| `glob` | 6 | the matcher is total on any pattern and name | [glob](verification/glob-proofs.md) |

Three things the table cannot say:

- The kernel runs the proved code, not a copy. `untyped::split`/`destroy`, the user-VA gate, the
  IPC rendezvous and the DMA validator were each extracted into their crate and the kernel calls it,
  so the proved arithmetic is the arithmetic that runs. The one-shot Reply rests on three legs, two
  proved and one structural.
- Some properties were declined rather than skipped, and each records why: the `Mapper` round trip
  and `Elf::parse`'s whole-parse totality hit the same BMC-over-real-memory wall
  ([`verification/elf-parser-wall.md`](verification/elf-parser-wall.md)).
- DMA confinement is proved for descriptors only. Said for a skeptic in one sentence:

> Every address that reaches a device through a virtqueue descriptor is provably confined to the
> driver's grant; addresses that reach a device inside a command payload are confined by the IOMMU
> alone, that confinement is tested rather than proved, and on a board without an IOMMU it does not
> exist.

  [`verification/iommu-domain.md`](verification/iommu-domain.md) has why, and what it means for the
  VisionFive 2.

## Where BMC's cost actually is

The cost of a proof is the shape of the code, not the size of the claim. Four crates measured it:

- A symbolic 64-bit division is the expensive part of a calendar, and a symbolic-length slice costs
  more than the parser it wraps. The calendar was factored along that seam and verifies in about
  seven minutes, the largest single entry ([`verification/calendar-proofs.md`](verification/calendar-proofs.md)).
- A loop Kani bounds too loosely, plus symbolic slice offsets, put whole-parse ELF totality out of
  reach. Proving the loopless leaf arithmetic recovered the panic surface in under a second.
- Two loops became one in `glob`, and an unwind bound derived from a measurement replaced one from
  algebra ([`verification/glob-proofs.md`](verification/glob-proofs.md)).
- A loop over leaf sizes ran past ten minutes; two `if`s and a mask prove in 0.05 s
  ([`verification/paging-blocks.md`](verification/paging-blocks.md)).

When BMC stalls, look at what the harness *touches* rather than at what it is trying to prove.

## Running it

```
script/verify
```

It self-installs Kani on first run (its own nightly toolchain and a CBMC backend), then runs
`cargo kani` over every package carrying harnesses. The harness count is generated weekly into
`notes/project-metrics.md`; the harnesses sit across 37 packages <!--count:harness-crates-->. Not
every host compiles all of them: two in `kernel/src/arch/x86_64/irq.rs` run only on an x86_64 host.

Harnesses within a crate verify in parallel, `-j 4` by default (`VERIFY_JOBS` overrides). In CI the
suite runs as three `prove` shards behind one required check, `verify (Kani proofs)`, which calef
ruled on 2026-10-07 to keep the job near 16 minutes. The packer refuses to prove a subset while
reporting itself as the suite. `script/verify --affected-since <base>` reports `not-needed` when a
diff cannot reach a proof, so a kernel-only pull request skips them.
[`verification/ci-timings.md`](verification/ci-timings.md) has the measurements, the shard table
and the trap a matrix would have set.

Not in `script/bootstrap`, because the kernel build does not need it; same self-install pattern as
`script/coverage`. A new proof crate goes in that script's list, and a new harness in an existing
crate is picked up with no change.

## The rules that keep proofs cheap and honest

- Proofs live behind `#[cfg(kani)]`. An ordinary `cargo build`/`cargo test` never compiles them,
  and the crate needs no dependency on `kani` (its intrinsics are injected only under `cargo kani`).
- Verify pure logic first. The §7 (testing) host crates (`capability`, `paging`, `elf`,
  `page_frames`, the ASID allocator) are the frontier: small, allocation-light, already
  host-compiled. Bounded model checking is happiest there.
- Spread inward from the capability core, the order §14 sets: `capability`, then IPC (rendezvous,
  one-shot reply), then the MMU isolation invariants. All three steps are done (milestone 18 (verify
  the capability core, then spread inward)). Each proves a property the security story would
  otherwise rest on by argument. The frontier now moves with milestone 14 (kernel objects from untyped):
  proving properties *of the kernel* at scale wants a kernel that does not allocate.
- **A harness that needs a huge bound is a design smell.** If a property needs Kani to explore an
  unbounded loop or a giant structure, that is often the code telling you the logic is not as local
  as it should be. Prefer refactoring the logic to shrinking the proof. This applies to *declining* a
  proof too, which milestone 35 (prove the DMA-confinement boundary) learned the hard way. The IOMMU
  domain property was written off as the build-and-translate wall, and the wall was real, but it was
  not where the property lived. Before recording a proof as impossible, check whether a smaller
  target carries it.
- **Falsify a property before believing it.** Break the code the harness guards and confirm the
  harness fails. Every milestone 35 property was falsified this way, and one falsification corrected
  a claim in the code (the load-bearing guard was not the one the comment pointed at). A harness
  that cannot be made to fail is not evidence.

  Since milestone 194 (the falsification record, its lint, and the sweep that replays it) this is
  written down rather than remembered (DECISIONS §134 (a harness carries a machine-replayable falsification record)). Every harness carries a `Falsification:`
  block saying whether the evidence is `replayable` (a patch a script applies, requiring the harness
  to go red), `attested` (a person watched it fail, and nothing can re-check that) or `unfalsified`
  (nobody has). `script/falsifications` reports the ratio, and `script/falsifications --sweep`
  replays the patches weekly. Read notes/falsification.md before writing one, in particular for
  what six real records found: a harness can be green, falsifiable, and still blind to the defect
  its own comment claims to rule out.
- **Guard against vacuity with `kani::cover!`.** Assumptions and bounds can silently empty a
  harness's input set, and a vacuous harness reports `SUCCESSFUL`. A `cover!` fails when a state is
  unreachable, so it is the one check that catches this.
  [`verification/direct-memory-access-validator.md`](verification/direct-memory-access-validator.md) has where it was introduced and
  the rule for when a harness needs one.

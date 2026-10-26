---
status: BUILT
raised: 2026-09-16
built: 2026-09-16
---
# 308. A GSI reaches its vector by redirection index, not by its own number

This changes a public function's signature and a vector-assignment policy four doc comments
rested on, so it was calef's. He chose option 1 on 2026-09-16, route by index, over refusing a nonzero base (option 2) and correcting only the documentation (option 3).

## The defect, and why a cap that bounds the wrong quantity is worse than no cap

`arch::x86_64::irq::gsi_vector` was `GSI_VECTOR_BASE.wrapping_add(gsi as u8)`, a flat map from a
global system interrupt to the vector it is delivered on, and `enable` passed its answer straight
into `route_gsi`, which writes a redirection entry.

`MAX_REDIRECTION_ENTRIES` (144, `MSI_VECTOR_BASE - GSI_VECTOR_BASE`) was **documented as the reason
that map could not wrap onto an exception vector.** It bounds the IO APIC's *entry count*, read from
the version register. It says nothing about the GSI, which the ACPI MADT supplies and which nothing
in this kernel bounds. `redirection_index` admits any `gsi` in `base..base + entries`, where `base`
is that part's `gsi_base`.

So a second IO APIC owning global interrupts from 200, with the 24 redirection entries every real
part has, admits **GSI 210**, and `0x30 + 210` is `0x102`, which a `u8` keeps the low byte of:
vector 2, the NMI. That is legal ACPI and it exists on multi-socket servers. The prover's own
counterexample was smaller and stranger (`base = 127`, `entries = 129`, `gsi = 255`, landing on
`0x2f` inside the local APIC's band); the plausible-hardware version is the one worth remembering.

The false doc is the part to carry forward. Nothing hid this defect except a comment asserting a
guarantee the code did not provide, sitting on the constant a reader would go to in order to check.
The correction is written out at `MAX_REDIRECTION_ENTRIES` in those terms rather than quietly
deleted, because the next reader's question is not "what does this cap do" but "why did four people
believe it did something else".

Found by proof, milestone 304, the first time a model checker compiled `kernel/src/arch/x86_64/`
at all. No test in this tree would ever have found it, and none could: the machine it needs does not
exist here.

## What was built

`gsi_vector` is now `GSI_VECTOR_BASE + redirection_index(gsi)`, which costs it two things the
proposal priced and one it did not.

- It loses `const fn`, because it reads two atomics. Measured rather than assumed: nothing
  in the tree used it in a `const` or `static` context, so the cost was zero. Checked by grep across
  every call site and by the build.
- It loses its total signature and returns `Option<u8>`. A GSI this part does not own has no
  entry and therefore no vector, and the old signature had to invent one. This is the same
  partiality `redirection_index` already had, surfaced instead of papered over.
- It loses the "flat and reversible" property three doc comments in `irq.rs` and one in
  `exceptions.rs` described. The map is still flat, in the *index*; recovering the GSI now needs the
  owning part's base. `exceptions.rs`'s own `BUGS` already recorded the vector-to-intid inversion as
  missing and unneeded, and this milestone re-checked that rather than assuming it: the device-vector
  arm does not call `gsi_vector`, so the new `Option` added no caller there. The entry now names the
  extra step the inversion would have to take.

On every machine this project owns the change is bit-for-bit what happened before, because the
MADT gives the single IO APIC `gsi_base` 0, which makes the index equal the GSI.

### The two call sites, and what each does with a `None`

Neither substitutes a default vector, which would be the silent failure this milestone exists to
remove.

`irq::enable` refuses with a panic, and the panic is better than the one it replaces.
`route_gsi` already panics on exactly this condition, one line later, but it sees only the GSI. The
new message names the `intid` the caller passed *and* the GSI it resolved to. That is not a
formality: `enable`'s own doc records milestone 161's bug surfacing as `gsi 34 is outside the IO
APIC's range` when what the caller said was 34, and the two numbers having been silently swapped is
what made it slow to read.

`main.rs`'s boot transcript moved its call to *after* `arch::irq::enable`. It takes the
`Option` with an `expect` whose reason is that ordering: `enable` has just routed the same GSI and
panics with a named reason if the part does not own it. The transcript's field is now the vector
that was actually programmed rather than one computed alongside it.

## The proof, which is the deliverable as much as the code

`proofs::an_owned_gsi_routes_inside_the_io_apic_band` carried `kani::assume(base == 0)`, and that
assumption **was** the finding: milestone 304 wrote it as this proposal standing in the harness until
the fork was answered.

It is gone, and nothing replaced it. The harness now states the property over every `base` in
`u32`, every entry count an eight-bit version field can report, and every `gsi` in `u32`. The
sixteen-bit bound on the GSI went too, because `record_isa_routing`'s packing is a fact about one
caller rather than a precondition of the map. A proof with no `assume` owes the reader nothing, which
is the state to aim at and rarely the state you get.

It gained an assertion about a disagreement nobody had noticed. `is_device_vector` has always
defined the device band by *index* (`GSI_VECTOR_BASE` up to the entry count) while `gsi_vector`
assigned by *GSI*. On a nonzero base the two disagreed, so the same interrupt was routed as a device
vector by this kernel and then **not counted as one** by its own trap handler. They are now the same
arithmetic, and the harness asserts it directly (`is_device_vector(gsi_vector(gsi))` for every GSI the
part owns) rather than leaving it to a reader comparing two function bodies. A second assertion pins
the guard and the map to the same domain: a GSI has a vector exactly when there is an entry for it.

### Falsified before believed, and filed where a machine can replay it

`kernel/falsifications/arch.x86_64.irq.tests.a_gsi_on_a_second_io_apic_routes_inside_the_band.patch`
restores `wrapping_add`. Measured on patagonia, 2026-09-16: **RED**, on the first assertion in the
test body, `left: 2` and `right: 58`. Those two integers are the whole milestone. 58 is `0x3a`, the
vector entry 10 of a part based at 200 is routed to; 2 is the NMI.

The same patch turns the Kani harness red, attested on cordoba (x86_64 Linux) on both of its
assertions, the band one and the `is_device_vector` agreement one. That second red is the evidence
that the agreement assertion is load-bearing rather than decorative: the two functions really did
disagree. Clean, the same command is 4 of 4 harnesses SUCCESSFUL with 131 checks on that one.

The patch is filed against a kernel test rather than against the harness on purpose. A
`#[kani::proof]` compiles for the host and `kernel/src/arch/mod.rs` selects its subtree by
`#[cfg(target_arch)]`, so a `replayable` record on an x86_64 harness turns `script/falsifications
--sweep kernel` red on every machine in this project but cordoba. `script/falsifications`' `BUGS`
records that constraint and names `attested` as the honest consequence. Milestone 305's kernel-test
half of the sweep is what makes a third option available: a kernel test declares `Architecture:
x86_64` in its patch head and boots QEMU, which works on any host. So the harness stays `attested`
and points at a patch that exists.

The new test, `irq::tests::a_gsi_on_a_second_io_apic_routes_inside_the_band`, is the middle
ground between a proof and a boot: the proposal's plausible part (base 200, 24 entries) written into
the two atomics and put through the real functions on a real x86_64 boot. Both ends of the range are
included, so the guard is exercised rather than assumed. It is not evidence that a second IO APIC
works. It is evidence that the arithmetic one would need is compiled, linked and running, which an
`#[cfg(kani)]` harness on an aarch64 dev machine is not.

## What ships unexecuted, and why that is recorded rather than resolved

No machine this project owns has an IO APIC with a nonzero `gsi_base`, so the correctness of the
path this milestone fixed rests on the proof and on the ACPI spec, not on a boot. Accepted by calef,
2026-09-16, and written into `irq.rs`'s module `BUGS` where a reader meets the feature rather than
closed out by the fix landing.

The larger fact that entry now carries, found while writing it rather than assumed: what is
untested is not the map but everything around it. `arch::x86_64::machine`'s MADT walk keeps only the
**first** IO APIC it finds (`if into.io_apic.is_none()`), so a GSI belonging to a second part is
refused by `route_gsi` rather than misrouted. A machine that needs two IO APICs is not merely
unproven here, it is **unimplemented**. That is a bigger gap than the one this milestone closed, it
is the thing to check first when this kernel meets a two-socket server, and it is its own piece of
work rather than a line in this one.

## Options 2 and 3, refused

Option 2, refuse a nonzero base at boot, would have turned a silent misroute into a named
refusal, which is cheaper and honest. It was refused because it makes the machine unbootable rather
than degraded on hardware nobody here can test either answer against. It was also refused because
option 1 is bit-for-bit identical on every machine that boots today, which makes it about as safe
as a correctness fix gets. The two compose and option 2 remains available; it is not foreclosed.

Option 3, correct the documentation and leave the code, is the state the tree was already in
after milestone 304, so choosing it would have cost nothing and fixed nothing. The documentation
correction happened anyway, as part of this.

Asked AGENTS.md's own test, *would we still choose this if both options cost the same*: yes. Option
1 is the larger of the two changes and is chosen for correctness on hardware, not for being small.

## A name this lane proposes and did not take

`gsi_vector` is a worse name after this change than before it, and it was already marked
provisional (milestone 161). The function now maps a *redirection index* into the vector space; the
GSI is what it takes, not what it adds. The name still describes the question a caller asks, so it is
not wrong, and calef names public items. Proposed, not performed, and said out loud at the function.

## Follow-on

- **Milestone 325.**. The gap
  found while writing this milestone's `BUGS` entry, and the larger of the two: the MADT walk in
  `arch::x86_64::machine` keeps only the first IO APIC, so a machine with two has the second's whole
  range refused by `route_gsi`. Routing by index is a precondition for supporting one; it is not
  support for one.
- **Recorded.** `kernel/src/arch/x86_64/interrupt_controller.rs`, module `BUGS`. The multi-IO-APIC path ships
  unexecuted, and cannot be executed on any machine this project owns, so its correctness rests on
  the proof and on the ACPI spec rather than on a boot. Accepted by calef, 2026-09-16, and recorded
  where a reader meets the feature rather than closed by the fix landing.
- **Recorded.** `kernel/src/arch/x86_64/exceptions.rs`, the device-vector arm's `BUGS`. The
  vector-to-intid inversion is still missing and still needed by nothing, re-checked rather than
  assumed; the entry now names the extra step an inversion would take, which is adding the owning
  part's global interrupt base.
- **Refused.** Renaming `gsi_vector`. It is an architect's call, the lane proposes it at the
  function, and a rename performed on a lane's own initiative is a naming decision with extra steps.
- **Done.** Milestone 304's `kani::assume(base == 0)`, which that block named as this proposal
  standing in the harness until the fork was answered. The assumption is gone and nothing replaced
  it.

## Index row

`gsi_vector` added the GSI to `GSI_VECTOR_BASE`, and `MAX_REDIRECTION_ENTRIES`' own doc named that
cap as the reason it could not wrap onto an exception vector. The cap bounds the entry count, not the
GSI, so a second IO APIC based at global interrupt 200 with the 24 entries every real part has routed
GSI 210 onto vector 2, the NMI. That is legal ACPI, found by proof in milestone 304 (only ever
compiled one architecture), unreachable by any
test in this tree. calef chose option 1 on 2026-09-16, so the vector is now the base plus the
**redirection index**, which is bit-for-bit identical wherever `gsi_base` is zero and correct where it
is not. `gsi_vector` loses its `const fn` (nothing used it in a const context, checked) and becomes
fallible. Both call sites refuse a `None` rather than substituting a vector, and `enable`'s new panic
names the intid the caller passed as well as the GSI it resolved to, which the one it replaces could
not. `an_owned_gsi_routes_inside_the_io_apic_band`'s `kani::assume(base == 0)` is gone with nothing
in its place. It gained the assertion that `is_device_vector` and `gsi_vector` agree. They
disagreed on a nonzero base, so the kernel routed a line as a device vector and then declined to count
it as one. A replayable falsification is filed against a new x86_64 kernel test rather than the
harness, because a Kani record would turn `--sweep kernel` red on every host but cordoba; restoring
`wrapping_add` gives `left: 2, right: 58`. Four doc comments corrected, the false one loudly. The
multi-IO-APIC path ships unexecuted and says so in `BUGS`, and the bigger finding is beside it: the
MADT walk keeps only the first IO APIC, so a second one is not merely unproven here, it is
unimplemented.

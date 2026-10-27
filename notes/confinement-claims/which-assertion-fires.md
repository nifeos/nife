# Which assertion actually fires (milestone 307)

An appendix to [notes/confinement-claims.md](../confinement-claims.md). It holds the sweep of
milestone 307 (which assertion actually fires when a confinement claim is broken), over all 26 rows
the table had then. Its verdicts are dated 2026-09-16.

## The question, and the verdicts

Milestone 305 (the six kernel confinement rows get a falsification a machine can replay) found two
independent cases where the assertion a reader would quote is not the assertion doing the work. It
said a sweep would probably find more. This is that sweep. The question asked of every test and
harness: when the claim is broken, which assertion fires, and is the one a reader would quote
reachable at all?

There are three verdicts, and the first is not padding. Saying plainly that most rows are exactly
what they look like is what makes the rest worth reading.

| Verdict | Rows | Count |
|---|---|---|
| Fires as advertised | 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 16, 18, 19, 21, 22, 23, 25 | 17 |
| The quotable assertion cannot run | 4, 13, 14, 15, 20, 24 | 6 |
| Answered by refusing to look | 12 | 1 |
| Deliberately `unfalsified`, so neither | 17, 26 | 2 |

Two of the seventeen carry an unreachable restatement *below* a headline that does fire, which is
the same shape doing less damage: row 18's `& GRANT == 0`, and row 25's two
`rendezvous_waiting_senders` checks.

Row-by-row notes on the seventeen follow, so a reader can tell which fact is which rather than
inferring it from a count.

- Rows 1, 3, 5, 6, 8, 10, 11 and 16 state their property independently of the code under test. Each
  has a recorded patch that fires on the assertion its prose names.
- Row 2's `from_bits_cannot_forge_a_right` is sound for the claim it makes. It is blind to the
  adjacent hazard its own doc comment names, a wrong `Rights::ALL`. That drops rights rather than
  forging them, and is a different claim.
- Row 7's `kani::assume` narrows *to* the adversarial case rather than away from it.
- Row 9 is stated through both functions it compares. That is the claim (that they agree) rather than
  a defect.
- Row 16's two assumes plus `MAXIMUM_QUEUES = 2` admit exactly one pair, `(0, 1)`. So "any two
  distinct queues" is one concrete case, and its patch says so.
- Rows 19, 21, 22 and 23 are the kernel tests. 23's evidence is about the test rather than the
  kernel, for the reason 305 recorded.

## Row 12's proof could not fail, and the tree had written down that it could

`paging::x86_64::no_vtd_entry_ever_sets_a_reserved_bit` is row 12's whole evidence for the claim of
§20 (IOMMU-backed DMA isolation) that an IOMMU entry sets no bit the hardware treats as reserved. It
stated all three of its assertions, and its `kani::assume`, through `VTD_ADDR_MASK`, `VTD_R` and
`VTD_W`. Those are the three constants `Vtd::leaf_entry` builds its result out of:

```rust
fn leaf_entry(pa: u64, flags: Flags) -> u64 { (pa & VTD_ADDR_MASK) | bits }   // bits ⊆ {VTD_R, VTD_W}
assert_eq!(leaf & !(VTD_ADDR_MASK | VTD_R | VTD_W), 0);
```

`(pa & M) | bits` sets no bit outside `M | VTD_R | VTD_W` for every value of M. The assume admitted
exactly the addresses `M` allowed. So widening the mask moved the encoder, the input space and the
assertion in lockstep. The address half of the claim was a tautology.

It was measured, and this is the part that makes it a finding rather than an argument. The test
widened `VTD_ADDR_MASK` to bits 62:12, a range a VT-d second-level entry really does reserve
(patagonia, 2026-09-16, kani 0.67.0):

| Harness | Result |
|---|---|
| as it stood before 307 | SUCCESSFUL, 0 of 45 failed. A survivor. |
| as it stands after 307 | FAILED, 1 of 68 |
| after 307, honest tree | SUCCESSFUL, 0 of 68 |

The defect is not exotic. `VTD_ADDR_MASK`'s own doc comment says VT-d's real width is
`CAP_REG.MGAW`-defined, and this driver has never narrowed to it. So the mask is a number somebody
could plausibly change. And the consequence is not cosmetic. QEMU's model and real silicon fault a
transaction over a reserved bit rather than ignoring it. So the failure mode is an IOMMU whose every
translation fails, presenting as broken hardware rather than as a bad table.

The tree had already recorded the opposite, in writing, forty lines up. The comment on
`the_leaf_keeps_address_and_permissions_apart` explains this exact trap correctly. Then it says
*"`no_vtd_entry_ever_sets_a_reserved_bit` in this crate already works this way; this is the same
move on the portable leaf."* It did not work that way. A lane that had just avoided the trap cited,
as its precedent, the one harness in the crate still caught in it. That comment is corrected rather
than deleted, because the citation is the interesting half.

Fixed here. The permitted bits are a literal (`VTD_PERMITTED_BITS`, `cfg`-gated to the test
configurations, so no implementation can reach it and reintroduce the coupling). The assume is gone,
because masking the address down is `leaf_entry`'s own job. A claim about what it does with an
arbitrary address may not assume the address is already in range.

The host twin `a_vtd_leaf_sets_no_bit_outside_read_write_and_address` was blind for a second,
independent reason on top of the first. It is worth naming, because a literal alone would not have
fixed it. Its one concrete address `0x10_0000` has no bits above 51. So the encoder's masking was
never exercised, and a wider mask changed nothing it could observe. It now runs three addresses and
catches the same defect in microseconds.

## The 305 shape recurs six more times, in proofs as well as in kernel tests

Milestone 305 promoted "in a test that states its property twice, the readable statement is usually
the unreachable one" from an anecdote about §31 (the foreign-language seam) to a thing to look for.
It looks for well. Eight rows carry an assertion that states the claim in the claim's own vocabulary
and cannot run:

- Row 13. `in_region_is_sound` ended with `assert!(addr >= base && end <= limit)`. It carried the
  comment *"no byte the device would touch lies outside the granted region"*, directly below the two
  assertions it is the conjunction of. Removed in 307; the sentence moved onto the live pair.
- Row 14. `an_accepted_descriptor_is_confined` kept `assert!(!d.is_indirect())` and
  `assert!(is_in_region(base, size, d.addr, d.buf_len()))` *below* the assertions that replaced them,
  from milestone 211 (a harness that states its property through the function under test cannot see
  that function break). Inside `if is_descriptor_allowed(..)` those are the same two calls with the same
  arguments the guard returns false on, so neither can fail. 211 added the working phrasing and left
  the blind one underneath, holding both readable messages. Removed in 307.
- Row 18. The inversion described in [breaking-the-harnesses.md](breaking-the-harnesses.md).
- Row 4. `a_deleted_capability_stays_deleted`'s `get`/`delete` re-use refusals sit below the storage
  check `assert!(cs.slots[slot].is_none())`, which catches the same defect one line earlier. Benign,
  and already named in an in-code comment. Left alone, because the accessors are the claim's
  vocabulary and the storage line is its mechanism, and here that costs nothing.
- Row 15. `an_oversized_batch_is_refused`'s single `assert!(!ok)` is never reached; the panicking
  closures are the mechanism. Its own patch says so. Left alone for the same reason.
- Row 20. §31's headline, and 307 sharpens what 305 recorded. `assert_eq!(v[2], CONFINED)` is not
  simply unreachable. It is reachable only through the two bits that are not the confinement claim.
  A broken `IN_GRANT_WRITE_LANDED` or `FAULT_ADDR_AS_EXPECTED` still faults, still produces a death
  report, still produces a verdict, and fires it. A broken `WITNESS_RO_INTACT` or
  `WITNESS_FAR_INTACT` means the store landed instead of faulting. So no death is reported, and the
  run stalls at `wait_for_report`. The assertion that prints *"read-only witness intact"* can fire
  for everything except a broken witness.
- Row 24. 305 recorded the relative-path crossing as unreachable. It is the same for the
  absolute-path crossing twenty lines further down, and for all four `assert_ne!` lines beside the
  two. Every one of them restates a bit `assert_report` has already checked in one direction or the
  other. Six assertions, all of them the readable half, none of them able to run.
- Row 25. The two `assert_eq!(sched::rendezvous_waiting_senders(..), 0)` lines read *"the write did
  not fault"* and *"a client read a pixel of the screen it holds no mapping of"*. They sit below a
  `wait_for` on the fault counter that catches that defect first. The fault wait fires.

## Two limits the sweep found that are not assertion order

An assertion can be live on one architecture and structurally dead on another, and row 21 is not the
only place. `a_read_only_segment_is_mapped_read_only` asserts `!flags.is_kernel_executable()` on a
user `.rodata` page. On aarch64 that is a live check. `PXN` is a bit independent of `AP_USER`, and a
kernel-executable user page is a real hazard the `Flags::user_code` doc calls out by name. On riscv64
and x86_64 it cannot fail. Both decoders reach `ACCESS_KERNEL_EXEC` only through an `else` branch that
requires the user bit clear (`sv39.rs` `leaf_flags`, `x86_64.rs` `leaf_flags`). And the assertion two
lines up has already established the page is user-accessible.

The sweep first wrote that the decoders are faithful: on those two ISAs the hardware really does make
a user page non-executable in supervisor mode. Half of that sentence was false, and the audit of
milestone 313 (userspace confinement, read adversarially) found it on 2026-09-17. On riscv64 the
hardware does refuse a supervisor fetch from a `U` page, unconditionally. On `x86_64` it does so only
while `CR4.SMEP` is set, and this kernel had never set it. `XD` is the one execute bit and applies at
every ring. So the decoder was reporting a user page as not kernel-executable on a machine where ring
0 could execute it. The bit is set now (`arch::x86_64::initialize`, on every core whose CPUID offers
it). That makes the decoder true on the hardware rather than in principle, and the encoder's own
comment carries the correction. The structural point survives with that caveat. The assertion still
cannot fail on those two ISAs, and what is wrong is reading one portable test as three ISAs' worth
of evidence. It is the same distinction 305 drew for row 21: a gap in the evidence, not in the
capability.

Row 19's attackers cannot tell a refusal from a probe that was never sent. `fs_test_client`'s
`dir_attacker` sets `REACHED_PARENT` and its siblings only on *success*. So a fixture that stopped
attempting the parent open would report a clean verdict, and the test would pass. `OPENED_ITS_OWN`
and `GRANTED_ACCESS_FAILED` guard the other direction (a capability that reaches nothing is trivially
confined), and they do that job well. Nothing guards this one. It is the parent's opening sentence,
one level out: a passing test is consistent with the component being stopped and with the component
never having asked. It is recorded rather than fixed, because the fix is a per-probe "attempted" bit
in a wire-format bitmap two programs agree on.

## And where the three instances did not generalize, which is worth as much

The predicate class that produced 305's survivor was checked on every architecture and found sound
elsewhere. aarch64's `user_can_read` asks the silicon (`AT S1E0R`) and has no half to get wrong.
x86_64 has no such predicate at all. The DMA attackers' descriptors reach `is_in_region`, rather than
being turned away by an earlier check. `is_descriptor_allowed` tries `is_indirect` first. The direct
attacker's descriptor is not indirect. The indirect attacker's *is*, and its own comment says so. The
compositor test's vacuity guard, `neighbour_probe_phys(ATTACKER) == client[VICTIM] + FRAME_SIZE`,
compares two different allocation records. It is a real fact about adjacency rather than a
restatement. `reap_tests::assert_can_only_supervise` walks every slot and checks both directions.
All of these fire as advertised.

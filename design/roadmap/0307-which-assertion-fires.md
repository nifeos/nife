---
status: BUILT
raised: 2026-09-16
built: 2026-09-16
---
# 307. Which assertion actually fires when a confinement claim is broken

## Why this exists

Milestone 305 found two independent cases where **the assertion a reader would quote is not the
assertion doing the work**, and one where a test could not fail at all. It said in its own block
that a sweep would probably find more. This is that sweep, over all 26 rows of
`notes/confinement-claims.md`, which are the confinement claims this project makes in public.

One question, asked of every row: *when the claim is broken, which assertion fires, and is the one a
reader would quote reachable at all?*

The framing matters because `AGENTS.md`'s first principle says an experiment that can only confirm is
not a test, and `design/fatal-risks/README.md`'s risk 7 is the claim that the property the whole system is
built to provide does not hold. Risk 3's mutation census, which measures exactly this property, runs
over host crates and cannot see kernel tests. Nothing the project owns would have found 305's
survivor, and nothing would have found this one.

## The finding: a second proof that could not fail

`page_tables::x86_64::no_vtd_entry_ever_sets_a_reserved_bit` is the whole of row 12's evidence for §20 (IOMMU-backed DMA isolation)'s
claim that an IOMMU entry sets no bit the hardware treats as reserved. It stated all three of its
assertions, and its `kani::assume`, through `VTD_ADDR_MASK`, `VTD_R` and `VTD_W`. Those are the
three constants `Vtd::leaf_entry` builds its result out of, so the assertion read

```rust
fn leaf_entry(pa: u64, flags: Flags) -> u64 { (pa & VTD_ADDR_MASK) | bits }  // bits ⊆ {VTD_R, VTD_W}
kani::assume(pa & !VTD_ADDR_MASK == 0);
assert_eq!(leaf & !(VTD_ADDR_MASK | VTD_R | VTD_W), 0);
```

and `(pa & M) | bits` sets no bit outside `M | VTD_R | VTD_W` for every value of M. The assume
narrowed the inputs by the same constant, so a widened mask admitted exactly the addresses it had
just started letting through. The address half of the claim was a tautology.

Measured both ways on patagonia, 2026-09-16, kani 0.67.0, with `VTD_ADDR_MASK` widened from bits
51:12 to bits 62:12:

| Harness | Result |
|---|---|
| as it stood before this milestone | **SUCCESSFUL, 0 of 45 failed. A survivor.** |
| as it stands now | FAILED, 1 of 68 |
| as it stands now, honest tree | SUCCESSFUL, 0 of 68 |

Neither the defect nor its consequence is exotic. The constant's own doc comment records that VT-d's
real address width is `CAP_REG.MGAW`-defined and this driver has never narrowed to it, so the mask is
a number somebody could plausibly change. And the harness's own doc comment explains why the failure
would be bad: QEMU's model and real silicon fault a transaction over a reserved bit rather than
ignoring it. The symptom is an IOMMU whose every translation fails, which reads as broken hardware
rather than as a bad table.

The surprise is that the tree had already recorded the opposite. Forty lines up, the comment on
`the_leaf_keeps_address_and_permissions_apart` explains this exact trap correctly, in full, and then
says: *"`no_vtd_entry_ever_sets_a_reserved_bit` in this crate already works this way; this is the same
move on the portable leaf."* It did not work that way. A lane that had just avoided the trap reached
for a precedent and picked the one harness in the crate still caught in it, and that sentence then
stood as the tree's only statement about the question. The comment is corrected here rather than
deleted, because the citation is the more interesting half: this is `AGENTS.md`'s ladder failing at
rung four exactly as it says it will. The rung-one fix is that the constant is now a literal that
no implementation can reach.

The host twin `a_vtd_leaf_sets_no_bit_outside_read_write_and_address` was blind for a second,
independent reason, which is worth naming because a literal alone would not have fixed it. Its one
concrete address `0x10_0000` carries no bits above 51. So the encoder's masking was never exercised
and a widened mask changed nothing it could observe. It runs three addresses now and catches the same
defect in microseconds, which is where this tree prefers to catch things.

## The 305 shape recurs, six more times, and in proofs rather than only in kernel tests

305 promoted *"in a test that states its property twice, the readable statement is usually the
unreachable one"* from an anecdote about §31 to a thing to look for. Eight rows carry such an
assertion. Three were removed because they restate the *guard itself* and no defect can separate them
(rows 13, 14, 18). Five are left in place because they are genuine redundancy that costs a line, and
the distinction between redundancy and decoration is the thing worth keeping. Every one is recorded
beside its claim in `notes/confinement-claims.md`.

Row 18 is the one to read, because it inverted. `notes/confinement-claims.md` has a section
crediting `a_plan_never_grants_a_right_the_declaration_did_not_ask_for`'s `& GRANT == 0` line as *the
only explicit assertion that catches the defect*. Milestone 211 then repaired the assertion above it
by writing the expected rights out in literals, and since `READ` is `1 << 0`, `WRITE` is `1 << 1` and
`GRANT` is `1 << 2`, that equality now implies the `& GRANT == 0` line entirely. The rescue became
the decoration, and the note went on describing code that had changed for four weeks. That
paragraph is rewritten rather than patched, because the corrected version teaches more than the
original did.

## Counts

**Fires as advertised: 17 rows. Quotable assertion cannot run: 8. Answered by refusing to look: 1.**

The third number is the one that mattered and the first is not padding: saying plainly that 17 rows
are exactly what they look like is what makes the other nine worth reading.

## What it cost

Reading is what found all nine unreachable assertions and it is nearly free. Breaking is the only
instrument that can find a survivor, and it costs one solver run or one boot per defect: the row 12
measurement above is three `cargo kani` invocations at about two seconds each. The asymmetry is the
practical lesson. Reading predicts. Only breaking decides; and a prediction about a proof is cheap
enough to confirm that there is no excuse for leaving one unconfirmed.

## BUGS

- **This is a manual sweep, not a mechanism, and it is dated.** Nothing gates which assertion a row's
  evidence arrives through. `script/falsifications` already records that it checks a red's *shape*
  rather than the assertion its patch predicts; it has nothing at all to say about an assertion that
  is unreachable while the harness is green, which is the entire subject here. Every verdict in
  `notes/confinement-claims.md`'s new section will rot the first time somebody rewrites one of these
  harnesses, and nothing will report it.
- Twenty-five of the twenty-six verdicts are reasoned from the code rather than measured. Only
  row 12 was broken on purpose, because only there did the reading predict something a run could
  settle. Milestone 305's own headline is the standing warning about what reasoning is worth here:
  `user_can_read` was readable for four weeks and every gate was green throughout.
- **The `--sweep` re-runs confirm the recorded patches still fire, not that they fire through the
  right line.** All three edited packages were swept after the edits (`paging` 8 records,
  `dma_validator` 6, `component_plan` 3; 0 survivors, 0 stale). That proves the patches still apply
  and still turn their harnesses red. Which assertion caught them is still a thing a reader checks by
  hand.
- **Row 19's attackers cannot tell a refusal from a probe that was never sent**, and this milestone
  recorded it rather than fixing it. `fs_test_client`'s `dir_attacker` sets its escape bits only on
  success, so a fixture that stopped attempting the parent open would report a clean verdict. The
  opposite direction *is* guarded (`OPENED_ITS_OWN`, `GRANTED_ACCESS_FAILED`). The fix is a per-probe
  "attempted" bit in a bitmap two programs agree on, which is a wire format and therefore not a
  lane's.
- **No shipping code changed in this milestone.** Every edit is inside `#[cfg(test)]`,
  `#[cfg(kani)]`, or a `cfg`-gated constant, plus prose. That is worth stating because a milestone
  about the quality of evidence should not be mistaken for one that changed the system. And it is
  why the architecture gates below are the relevant ones rather than a three-way boot matrix.

## Follow-on

- **Recorded.** In `notes/confinement-claims.md`'s `BUGS` beside the table: that the verdicts are
  dated and ungated, that 25 of 26 are reasoned rather than measured, and that an unreachable
  assertion is not automatically a deletable one.
- **Recorded.** In `notes/confinement-claims.md`'s new section: that an assertion can be live on one
  architecture and structurally dead on another even where no row's citation says so.
  `assert!(!flags.is_kernel_executable())` on a user page is a real check on aarch64 and cannot fail
  on riscv64 or x86_64, because both decoders reach `CAP_KERNEL_EXEC` only through a branch requiring
  the user bit clear. The decoders are faithful and the hardware really does guarantee it there; what
  is wrong is reading one portable test as three ISAs' worth of evidence. Same distinction 305 drew
  for row 21.
- **Milestone 323.**.
  A `Falsification:` block that names the assertion its patch expects to fire, and a
  `script/falsifications` check that the named line is the one the transcript reports. Four of the tree's patches already state this in
  prose, correctly and usefully, and a reader only meets it by opening a patch file, which is rung
  four by `AGENTS.md`'s own reckoning. It would have caught none of this milestone's findings, and
  that is the honest case against doing it first: an unreachable assertion is invisible to it, since
  the patch's prose and the transcript would simply agree on a different line. What it buys is that
  the *next* 305, where a red arrives through a helper at "the supervision tree could not be built:
  stage 3", is a gate failure rather than something somebody happens to read. It is a format change,
  so the field's spelling is an architect's.
- **Milestone 418.**
  Point `script/mutation` at `crates/page_tables`, `crates/dma_validator`, `crates/component_plan` and
  `crates/capability`, and compare its verdict against this milestone's. Risk 3's census measures whether a change to the code is
  caught; this milestone measured whether a specific assertion can catch anything. Row 12's tautology
  is exactly the shape a mutation of `VTD_ADDR_MASK` would have surfaced as a survivor, in a crate
  the census already covers, which makes "did the census already know" a question worth an answer
  rather than an assumption. If it did, the finding is that nobody read it; if it did not, that is a
  gap in the census worth its own work.
- **Done.** `page_tables::x86_64::no_vtd_entry_ever_sets_a_reserved_bit`, which could not fail on the
  address half of its own claim, and its host twin, which could not fail on it for a second reason.
  Both fixed and measured both ways. The recorded falsification is now the defect the harness was
  blind to rather than the one it always caught, with the earlier defect and its measurement kept
  in the patch's prose.
- **Done.** The stale paragraph in `notes/confinement-claims.md` that credited row 18's `& GRANT == 0`
  assertion as the only live one, four weeks after milestone 211 made it the dead one.

## Index row

Milestone 305 (the six kernel confinement rows get a falsification a machine can replay) found two cases where the assertion a reader would quote is not the assertion doing the work and predicted a sweep would find more. This is that sweep, over all 26 rows of `notes/confinement-claims.md`. **The finding is a second proof that could not fail**, and it is a Kani harness rather than a kernel test, which is where nobody was looking. `no_vtd_entry_ever_sets_a_reserved_bit` stated all three assertions *and* its `assume` through the three constants the encoder builds its entry out of. So `(pa & M) | bits` setting no bit outside `M | R | W` held for every value of `M`. Widening `VTD_ADDR_MASK` over bits a VT-d entry really reserves left it **SUCCESSFUL, 0 of 45 failed**, while every translated DMA would fault as broken hardware. The tree had recorded the opposite forty lines away, citing this harness as the example of avoiding that exact trap. Fixed, measured both ways, and the shape found six more times: 17 rows fire as advertised, 8 have a quotable assertion that cannot run, 1 was answered by refusing to look.

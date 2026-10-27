# Verdict briefs for fatal risks 6 and 7

*Name: provisional, minted 2026-10-02 by lane `maintainer/verdict-briefs-6-7`; naming is an architect's.*

Written 2026-10-02 (UTC) against `main` at `b4a081e02`. Risks 6 and 7 in
[`design/fatal-risks/README.md`](../design/fatal-risks/README.md) read Experiment status RUN and carry
no color (`color: none` in both appendices). The color is the architect's alone
([§216 (fatal-risk facts are correctable, and verdicts are the architect's)](../design/decisions/0216-fatal-risk-facts-are-correctable-verdicts-are-the-architects.md)).
This page gathers the evidence so a ruling takes one reading. It sets no verdict and edits nothing
under `design/`.

One thing to know first. [§211 (what a fatal-risk verdict says)](../design/decisions/0211-what-a-fatal-risk-verdict-says.md)
defines the Experiment status words and defines no color. GREEN, AMBER and RED have no written
definition in the tree. What exists is precedent, and this page applies it. GREEN on risks 1 and 9
(ran, could have come back red, did not, caveats stated). AMBER on risks 2 and 3 (ran, and found a
standing weakness or fell short of the entry's own standard), and no color on risk 4, whose
single-crossing numbers exist while the decisive experiment does not. A PR search (`risk 6`,
`risk 7`, `amber confinement`) and `git log` on the README found no color ever proposed or ruled for
either risk.

## Risk 6: a capability-confined userspace driver cannot drive real hardware at real speed

**Recommended: no verdict yet (leave uncolored).** Not AMBER, because nothing adverse was found.
Not GREEN, because the claim's own words ("behind an IOMMU", "real speed") are not what was
measured.

### 1. The claim, and what red would mean

A driver outside the kernel, confined by capabilities and an input-output memory management unit
(IOMMU), cannot drive a real device at a useful rate. Red would be a real device that a confined
driver cannot operate at all, or only at a cost the thesis cannot pay.

### 2. What was run

All of it on radon (riscv64 VisionFive 2, JH7110). The device is the true random number generator
(TRNG), milestone 159 (a real hardware entropy source: the JH7110's TRNG)'s driver, an EL0 process holding two endpoints and one page of device memory.

| What | Result | Source |
|---|---|---|
| Confined | grant is two endpoints and one `0x1000` page; no interrupt, no DMA page, no `Virtio` capability | `design/roadmap/0159-jh7110-trng-driver.md`; read adversarially, "Nothing to find", `design/audit-reports/2026-09-17-userspace-confinement.md`, "The population, found by reading grants rather than names" |
| Drives the device | draws differ between draws and boots, 2026-09-03/04 | transcripts named in the appendix live under `target/board/`, which is not in git |
| Rate, 2026-09-16 | 988,416, 992,248 and 992,248 bytes/s; bring-up 522, 516 and 522 us | `bench/radon-2026-09-16/jobmix-boot3.log:721`, `jobmix-boot5.log:736`, `jobmix-boot4.log:814` |
| Rate, 2026-09-25 | 973,384 bytes/s over 8 round trips, bring-up 580 us | `bench/radon-2026-09-25/soak-8h.log:208` (the 8 h 09 m soak boot) |
| The README's figure | 955,223 bytes/s, 64 bytes in 67 us | commit `4af8ff980`, `design/roadmap/0306-time-the-hw-entropy-step.md`, "The number" |
| QEMU floor | about 250 us per exchange; radon is 8.4 us, so the floor is not a denominator | appendix, `design/roadmap/0306-time-the-hw-entropy-step.md` |

No xenon NVMe result exists. `bench/xenon-2026-09-17/` holds a first-light tour and two failed NVMe
attempts (a crash and a skip). The Results table in `notes/risk-6-bench-evening.md` is empty.

### 3. Is the premise true, and could it have come back red

Partly.

- Confined and drives real hardware: yes, and these did fail first. The success line was unreachable on any device before 2026-09-04 (it asked for 32 bytes down an 8-byte channel). The block's clocks were gated and its reset asserted (milestone 220 (this kernel drives no clock or reset controller)), and the first silicon boot
  reported ready while holding zeros (PR #724). A red was possible and was seen.
- At real speed: could not have come back red. No threshold was written, and there is no
  comparator. The Linux `hwrng` figure is a different thing (an in-kernel read, no IPC), and the
  honest comparison against `jh7110-trng.c` on the same silicon is unmeasured, as the appendix says.
  The 8.4 us per round trip counts IPC, context switches, a poll loop and the device together.
- The IOMMU half of the claim is untested on silicon. radon has no IOMMU and the TRNG does no
  direct memory access (DMA), so confinement here is capability reach only. No real-silicon datum
  exists for a device behind an IOMMU. The 2026-09-25 correction (#1275, #1297) already says the
  xenon VT-d unit that came up may not be the unit in front of the NVMe function; that is an
  inference from the sibling OptiPlex 7040, unread on xenon.
- Has a confinement test here been seen red? The TRNG grant has no recorded falsification (none
  in `script/falsifications` output). The EL0 NVMe test
  (`system_tests/src/user/non_volatile_memory_express_tests.rs`) asserts the IOMMU was active, but
  carries no falsification record either, and ran only under QEMU, whose one VT-d unit owns the
  whole bus.

### 4. What has not been run, and whether it caps the color

The decisive experiment: one real non-virtio device, confined, at throughput. It is built
(milestones 261 and 594, preflights for DMAR scope and LBA size, `cargo xtask disk-throughput`) and
unrun. radon, riscv64, is the only board with any result. aarch64 (argon) never booted nife; xenon
(x86_64) has a tour and nothing for this risk. The absence caps the color: the entry itself says
"this does not retire the risk", and a TRNG is the smallest real device on the board.

### 5. Recommendation

No verdict yet. The same shape as risk 4: the best-measured part of the experiment is a single easy
case, and the decisive case exists as a script and an empty table. If the architect wants a color
now, the only defensible one is GREEN for the narrowed claim "a confined EL0 process can drive a
non-DMA device on real silicon". That is a different claim from the one the entry states, and
ruling on it would leave the NVMe evening looking optional.

### 6. What would move it

- GREEN: the bench evening reads xenon's DMAR, both preflights pass, three boots of
  `disk_throughput` print `CONFINED-AT-RATE` with `verified N of N`, and a Linux queue-depth-1 4 KiB
  `fio` run on the same disk gives a ratio the architect accepts. The architect also has to write the
  bar for "real speed"; none exists.
- RED: the unit owning the NVMe is refused or absent and nothing translates it. The outcomes
  table in `notes/risk-6-bench-evening.md` routes this. Or the confined rate is a small fraction of
  Linux qd1 for a reason that is confinement and not driver shape.
- Cheapest next experiment: that evening. Cost: one evening of calef's bench time, a Linux live
  stick for the ratio, three boots; no purchase, the disk is already wiped. A cheaper partial step is
  the Linux `jh7110-trng` read on radon to give the existing number a denominator (one boot, but it
  needs a Linux image radon does not have staged; unverified).

### 7. Corrections for the maintainer (§216)

1. The cited transcript lacks the figure. README risk 6 and the appendix cite
   `bench/radon-2026-09-16/tour-083200.log` for 955,223 bytes/s. That file is 188 lines, has no
   `hw entropy` line, and ends in a garbled capability-slot line. No committed log holds 955,223 or
   "in 67 us". Cite the four committed boots above, and say the 955,223 boot's capture was not
   committed intact (commit `4af8ff980` carries only the number).
2. "One boot of radon" is stale. The same measurement now has four committed boots, 973,384 to
   992,248 bytes/s, spread about 4%.
3. The appendix frontmatter `updated: 2026-08-31` predates its own 2026-09-25 corrections.

## Risk 7: the confinement claim is false

**Recommended: AMBER.** Not GREEN, because the README's own BUGS section says risk 7 "cannot return a
clean green". Not RED, because no confined component has been found to escape on its own authority.

### 1. The claim, and what red would mean

A confined component escapes, so the property the system exists to provide does not hold. Red would
be an escape, by a component holding only what it was granted, through a defect class the capability
model cannot close. A fixable defect is not red; a published claim that proves false is a finding.

### 2. What was run

Three kinds of work, all by this project, on QEMU and the host unless stated.

| Date (UTC) | What | Result | Source |
|---|---|---|---|
| 2026-08-31 | enumerate claims, falsify each test (milestone 202 (every confinement test is a ritual until somebody breaks the confinement)) | 26 rows, 25 Kani harnesses with a replayable falsification (up from 6), §31 (the foreign-language seam: C holds no capabilities and makes no syscalls)'s headline assertion never runs | `notes/confinement-claims.md`, PR #614 |
| 2026-09-16 | kernel half (305, 307) | one test could not fail since milestone 41 (dead code: triage the suppressions, and un-blindfold the gate); nine unreachable assertions across the rows | PRs #899 and 307's section |
| 2026-09-17 | audit, userspace lens (313) | fixed 3, minted 3, accepted 1; §12 (call/reply IPC: a one-shot reply capability) false on x86_64 (port grant kept after `CAP_DELETE`); SMEP sentence false | `design/audit-reports/2026-09-17-userspace-confinement.md` |
| 2026-09-21 | adversarial pass | a revoked capability parked in `outgoing_capability` was delivered afterwards; fixed in three sweeps; eight attacks held | PR #1059, `notes/confinement-claims.md` |
| 2026-09-23 | `PortRange::REVOKE` other-core window closed (315) | 12 of 12 two-core suites against 3 of 12 failing before | README appendix, PR #1144 |
| 2026-09-24 | audit, new trust boundaries | fixed 5 (incl. SVE/SME/V state neither saved nor disabled, a window no tested machine can open), no confinement claim false | `design/audit-reports/2026-09-24-new-trust-boundaries.md`, "Is any confinement claim in this tree false as stated?" |
| 2026-09-29 | audit, syscall surface as a whole | fixed 0, minted 0, accepted 3, no confinement claim false | `design/audit-reports/2026-09-29-syscall-surface-whole.md`, "Is any confinement claim false as stated?" |

Current replay evidence, from `script/falsifications` at this branch's base: 155 of 212 Kani
harnesses replayable (all packages, not only confinement ones); 16 of 17 kernel tests carry a
replayable record, and the 17th is row 26, `unfalsified`.

### 3. Is the premise true, and could it have come back red

The premise was that tests written by the builders could be rituals. That was true, three times
over: the `U`-bit test (milestone 41), the six unreachable kernel assertions (307), and the two port
tests that hung instead of failing (299/313). It could have come back red and came back partly red
in the sense that matters: a confinement test was shown unable to fail. It also found two published
sentences false in 313 and a third claim false on 2026-09-21.

Where a test has not been seen red, and it is a short list:

- Row 17 (descriptor changed after validation, `a_descriptor_mutated_after_validation_cannot_reach_the_device`): `unfalsified` by design, TOCTOU has no minimal defect.
- Row 26 (client cannot become server): `unfalsified`; a real break hangs the run, so the claim-shaped red does not exist.
- Row 19's second test, `a_grandchild_is_bounded_by_the_root`: `unfalsified`, although the table's "Falsified" column credits milestone 194 (the falsification record, its lint, and the sweep that replays it) for the row (its other test, `attenuate_never_widens`, is replayable).
- Kernel records are replayed by a full `--sweep` only, on the one architecture each patch names, and nothing runs that per commit.
- MSI/interrupt confinement and kernel-cannot-execute-user-pages have no test at all (milestone 424 (a ring 0 that provably cannot execute ring-3 pages) owns the second).

The strongest case for RED, stated so it can be weighed. The 2026-09-21 find is a capability
that survived revocation and reached a receiver, and `MemoryRegion::DESTROY` exists so none does.
That is the confinement property failing, on a path any boot could take. I read it as AMBER and not red. It needed a sender that parked the capability, and it was fixed in a day with a test red first on three architectures. The defect class reduces to one rule ("sweep every place authority
lives"), so the architecture can close it. That reading is a judgment.

### 4. What has not been run, and whether it caps the color

The adversarial half the entry has always named: an outsider trying to escape. Every pass above was
this project attacking itself, and the appendix says that is the same category of evidence. It is
gated behind milestone 198 (a package manager, and the trivial install that makes a second customer possible) by calef's no-third-parties position. It caps the color at AMBER by the
entry's own rule ("neither can return a clean green"). Also not done: hostile-client fixtures for
userspace caretakers (read, not attacked), and any x86_64 test through a real IOMMU unit that
confines a device (QEMU's one unit owns the bus).

### 5. Recommendation

AMBER. The experiment ran, found real defects in the confinement story and test ritual three times,
and found no escape. Findings per pass went 3 fixed, then 1 more false claim, then 5 fixed with none false, then 0 fixed. That is a flattening over four passes, which is a confidence and not
a verdict, as risk 5's entry says of its own curve. The unrun half is the one the claim is named
for. If the architect wants GREEN to be reachable, the standard for it has to be written first.

### 6. What would move it

- GREEN: an architect-written standard (for example, an attack by someone who did not build the
  system finds no escape and no false claim) and a pass that meets it. Row 26 and row 17 closed or
  explicitly accepted as unfalsifiable would help.
- RED: an escape from a granted-nothing component through a class that needs an architecture
  change; or a fourth round of test rituals discovered at the same rate.
- Cheapest next experiment: a blind adversarial lane, one that is not shown
  `notes/confinement-claims.md` and is told to find a claim nobody made, then a full kernel
  `--sweep` on all three architectures. Cost: one lane, plus a kernel sweep that boots once per record
  (milestone 305 (the six kernel confinement rows get a falsification a machine can replay)'s first sweep of 48 took 2 min 55 s warm). That is a new
  lane, so it needs calef to ask for it. It is still in-house evidence, so it narrows the AMBER and
  does not clear it.

### 7. Corrections for the maintainer (§216)

1. Counts are the 2026-08-31 ones. README says "26 claims enumerated, three of them stated
   nowhere, 25 harnesses". The table has 30 rows (27 to 29 from 313, 30 from 2026-09-21), and the
   note's section on unstated claims is headed "Five" while listing six paragraphs. Dated counts
   need dates or refreshed numbers.
2. The audit is no longer the last word. README: "AUDITED, 2026-09-17: a qualified yes with one
   exception". The appendix records two false published sentences from 313 (§12 on x86_64; the SMEP
   sentence) and a third claim found false on 2026-09-21. The README entry omits the 2026-09-21
   pass and the 2026-09-24 and 2026-09-29 audits (the last two found no false confinement claim).
3. "The adversarial half remains" in the running order is half stale. The in-house adversarial
   pass ran on 2026-09-21 (#1059). The outsider half remains. Owners listed (202, 305, 313) omit 315
   and the unnumbered 2026-09-21 lane.
4. The appendix's "syscall surface and IPC model are the remaining untaken lens" is stale. The
   2026-09-29 audit took it (fixed 0, minted 0, accepted 3, no confinement claim false).
5. Frontmatter `updated: 2026-08-31` in `the-confinement-claims.md` is stale.

## BUGS

- Color definitions are precedent, not text (see the top). If the architect would rather define
  GREEN, AMBER and RED once, the definitions would be a §211 amendment and this page's
  recommendations would be re-read against them.
- The figures here were read from committed logs and from `script/falsifications` at one commit;
  none was re-run. The 955,223 bytes/s boot could not be checked against a transcript because none is
  committed.
- Argon (aarch64 Jetson) has never booted nife, so no risk 6 datum exists on aarch64.

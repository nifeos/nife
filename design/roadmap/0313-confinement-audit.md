---
status: BUILT
raised: 2026-09-17
built: 2026-09-17
promoted_from: the-security-audit-that-has-been-due-since-august
---
# 313. The security audit that was due since August: userspace confinement, read adversarially

The audit milestone 311 (the audit cadence tripwire) found overdue, with the lens the maintainer
chose.

## What it is

The seventh security audit on the record and the first under the userspace-confinement lens, which
`design/audit-reports/README.md` had named as untaken since 2026-08-04. The report is
[design/audit-reports/2026-09-17-userspace-confinement.md](../audit-reports/2026-09-17-userspace-confinement.md);
this block is the account of building it, not a second copy of it.

Scope, bounded on purpose because 112 milestones is more tree than one lens holds. The components
that took device or network authority since 2026-08-17, found by reading every capability mint site
rather than the names (the population was two objects, not forty-five). The six confinement claims
milestone 307 (which assertion actually fires) marked as quotable-but-unreachable, asked what they
actually guarantee. And the two
machine classes that booted real silicon in the window, radon and xenon, followed from firmware-owned
memory and device windows into what a confined component can reach.

## What it found, in one paragraph each

One confinement claim was false as stated, and it is fixed. A thread that `SYS_CAP_DELETE`d its
own `x86_64` `PortRange` capability kept `in`/`out` access to the ports for life, because the grant
the context switch installs is a cached field and the delete cleared the table and not the cache.
DECISIONS §12 (call/reply IPC: a one-shot reply capability) says a consumed capability cannot be used again. For the one object enforced outside
the capability table it could, and `system_initializer` performed exactly that delete on every
`x86_64` boot. Fixed in `sched::delete_current_cap`, with a test and a replayed falsification record.

**The two existing port tests could not fail in the direction they exist for.** A wrongly permitted
`out` was followed by a `SEND` nobody received. So a leaked hand-off or a surviving grant would have
hung the suite rather than turning an assertion red. That is `notes/confinement-claims.md`'s row 26
shape, one object over, found when the new test's first draft did exactly that. The fixtures now exit
after an `out` that is expected to fault, and both tests carry records replayed on `x86_64`.

Ring 0 could execute a confined component's pages on `x86_64`, and the tree said otherwise.
`crates/page_tables`'s decoder reported every user page as not kernel-executable and milestone 307 wrote
that the hardware made it so; x86 does so only with `CR4.SMEP`, which nothing had set. Set now, on
every core whose CPUID offers it, with a console line either way; the sentence and the encoder
comment are corrected rather than deleted.

And three things recorded rather than fixed, each with a proposal. `PortRange::REVOKE` reaches
one core and `x86_64` no longer runs one (a one-tick window, accepted with the reason and a `BUGS`
section at the function). Row 21's `x86_64` leg carries no falsification because the mechanism
admits one architecture per record. And, outside the lens, one `x86_64` language test trips the
boot-stack gate when run alone and not inside the suite.

## What it cost

Reading, about four hours of lane time. Twelve targeted `x86_64` boots: one cold build to prime the
farm, three runs of the port tests as the fixtures changed. One run that hung: the first draft's
escape parking on a `SEND`, killed at its root after ten minutes. Four single-test runs to isolate
the stack-gate excess (two of them on the base commit's own `kernel/` and `crates/`), and three
record replays. Then the full suite on all three architectures, one at a time (`x86_64` in both boot
modes, aarch64, riscv64, every one exit 0). The kernel changes are
`#[cfg(target_arch = "x86_64")]` throughout and the only cross-architecture edit is a comment in
`crates/page_tables`. The other two legs were run because the brief asked for every architecture whose
code was touched, and a comment in a shared crate is code that was touched.

## BUGS

- **One `x86_64` test trips the boot-stack gate when run alone and not inside the suite.**
  `language_tests::a_refusal_and_a_success_report_different_numbers`, filtered to by itself, ends at
  62456 bytes against the 61440 gate, on the base commit, before any of this lane's changes. The
  full suite on this lane's tree ends at 53144 (PVH) and 50272 (UEFI), green. An earlier draft of
  this entry and of the report's finding 7 read the filtered result as this machine disagreeing with
  CI, and the full run corrected it. It matters for `script/falsifications --sweep`, which replays
  every kernel record as a filtered run; no record names that test today.
- The lane's own falsification sweep was green while CI's was red, on the same mechanism, by
  scope alone. The lane ran `script/falsifications --sweep kernel` (16 swept, 0 survivors) and
  CI ran `--affected-since <base>`, which follows the diff into `crates/` and found that the SMEP
  comment added above `Ia32e::attrs`'s `XD` branch had moved the context of
  `crates/page_tables/falsifications/x86_64.verification.no_encoded_leaf_is_both_writable_and_executable.patch`.
  That is the mechanism working as its header says it should ("a patch that no longer applies means
  the covered code moved"). The record was redone against the code as it stands, defect unchanged,
  and confirmed red by hand at the harness's only assertion. The lesson for the next lane is the
  form to run before pushing: `--affected-since <base SHA>`, never a package-scoped `--sweep`, because
  the package a change reaches is not the package the lane was thinking about.
- Finding 3 has no test. `CR4.SMEP` is set and a boot line says so; a falsification would need
  ring 0 to survive its own page fault, which this kernel cannot do. The boot line is rung three.
- **The cross-core port window is accepted, not closed.** One tick at most, cache cleared so it
  cannot reopen, no consumer runs a port holder on two cores; the IPI shootdown is proposed.
- **`design/decisions/0152-port-range-capability.md`'s third BUGS entry is now wrong** ("x86 runs a
  single core today") and this lane may not edit it. The two kernel comments that repeated it are
  corrected; the decision is the maintainer's.
- **`design/fatal-risks/README.md` risk 7 does not yet carry this audit's answer** (one claim false as
  stated, fixed), for the same reason.
- The report reads no `arch/` assembly and none of the IPC model. Its own scope section says
  what it excluded; the syscall surface is the remaining untaken lens.

## Follow-on

- **Milestone 315.** The IPI shootdown for `PortRange::REVOKE`, the shape of the TLB shootdown, now
  that `x86_64` boots secondaries.
- **Milestone 323.**: a kernel
  record per architecture, so a portable confinement test can carry evidence on every leg it runs on.
- **Milestone 424.** The
  test finding 3 could not have, and SMAP with the syscall-path number its `BUGS` asks for.
- **Milestone 425.** The
  boot-stack high-water that is 62456 for one test run alone and 53144 for the suite that contains it.
- **Recorded.** The one-tick cross-core window on a revoked `x86_64` port range, in
  `kernel/src/arch/x86_64/segments.rs`'s `revoke_installed_port_grant` `BUGS` section and in the
  report's finding 4.
- **Recorded.** That row 21 of `notes/confinement-claims.md` has no `x86_64` evidence, in that note
  beside the "on every ISA" paragraph.
- **Recorded.** That the kernel-cannot-execute-user-pages claim is stated nowhere and is a control
  bit on `x86_64`, as the sixth entry of that note's "stated nowhere" section.
- **Done.** `SECURITY.md`'s audit count and list, `design/audit-reports/README.md`'s two index rows,
  and the report, all by this lane. `script/audits --due` exits 0 because a report exists, not
  because a table was edited to make it.

## Index row

The security audit `script/audits` had reported overdue every week since 2026-08-17, run under the
userspace-confinement lens on a bounded scope. The scope was the device and port authority minted in
the window, the six claims milestone 307 marked unreachable, and the two boards that booted real
silicon. It found
one confinement claim false as stated and fixed it. A thread that deleted its own `x86_64`
`PortRange` capability kept the ports. The grant is a cached field the context switch
installs, and `SYS_CAP_DELETE` cleared the table and not the cache, which `system_initializer` did on
every `x86_64` boot. The two existing port tests could not go red for the defects they exist to
catch. A wrongly permitted `out` parked on a `SEND` nobody received and hung the run (row
26's shape, one object over). Their fixtures now exit, and all three port claims carry replayed
falsification records. Ring 0 could execute a user page on `x86_64` and the paging decoder said it
could not; `CR4.SMEP` is now set where CPUID offers it. Recorded and proposed. The one-tick
cross-core window on a revoked port range, now that `x86_64` runs secondaries. The missing `x86_64`
evidence for row 21. A test for the SMEP claim. And an `x86_64` stack gate that fires on a filtered
single-test run and not on the suite that contains the same test.

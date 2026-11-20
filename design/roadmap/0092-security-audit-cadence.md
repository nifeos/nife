---
status: BUILT
raised: 2026-08-03
built: 2026-08-16
---
# 92. Security audits as a mechanism: cadence, docs, and findings that become milestones

Built 2026-08-16. `design/audit-reports/` exists with its index, its §75 (directories under
`design/` and `notes/` carry provenance in their) provenance line, and the
disposition rule where an audit lane will read it. `script/audits` is the tripwire, computing
overdue from the rules of §74 (audits run on change, not on the calendar) against counts it derives
from the tree rather than from prose.
Milestone 93 (documentation audits as a mechanism) reuses all of it by adding one row per kind. That
is why the thresholds live in the
index as data rather than in the script as code. Raised 2026-08-03 by calef. The tree has had one security audit and
milestone 43 (A second security audit, with a different lens) asks for a second with a different
lens. This milestone is the machine that makes
them routine, so that auditing stops depending on someone remembering to ask.

Why a mechanism rather than a habit. The same argument as script/gates: a practice that lives
in memory gets skipped exactly when it matters. And the failure mode of audits specifically is
already named in this tree: DECISIONS §35's wallpaper, a finding nobody dispositions. An audit
that produces a report and no state change is wallpaper with a security label.

The mechanism, in four parts:

1. A cadence and its tripwire. Audits run on a recorded schedule (quarterly is the starting
   proposal; calef sets it) plus event triggers that queue one early. The triggers are a new system call
   method, a new component holding device or network authority, a new dependency class (§46 (thin
   primitives or whole subsystems)), or
   first boot on a new machine class (a board, a cloud). The tripwire follows the toolchain-drift
   pattern, a signal rather than an automation. A scheduled workflow compares the last audit's date (recorded
   in the audit index) against the cadence and goes red when one is overdue. Red means "run the
   audit", not "an automation ran it for you".
2. Each audit is a lane with a lens. Milestone 43's insight generalized. The value of the next audit
   is the lens the last one lacked. The lens rotation is recorded in the audit index
   (capability model, supply chain, network surface, userspace confinement, docs-versus-reality
   truthfulness, ...), and each audit names its lens, its scope, and what it deliberately did not
   look at.
3. Every finding ends in exactly one of three states, and the report is not done until each has one.
   Fixed: trivial, done in the audit lane itself. **Minted as a milestone**: the audit report
   proposes it and the integrator mints the number at merge, severity and rationale in the block.
   That is exactly how milestone 90 (A guard page under the per-CPU secondary stacks) was born from a
   finding of milestone 84 (stack high-water). Or **recorded-accepted** (with the
   reason, in the report and, where a reader would meet the risk,
   in the affected doc's BUGS section). "Noted" is not a state.
4. The docs re-baseline with every audit. SECURITY.md's claims, the confinement scope, and the
   affected notes' BUGS sections are part of each audit's diff. An audit that finds the docs
   overclaiming fixes the docs in the same lane, because the demonstrator's docs are part of the
   deliverable and an overclaim in SECURITY.md is itself a security finding.

**The audit reports live in `design/audit-reports/`** (calef, 2026-08-04): one file per audit.
`README.md` is the index, listing every audit's date, lens, finding count by disposition, and a link
to its report. The overdue tripwire reads the index, which keeps the mechanism honest the same way
the roadmap gate reads the milestone files. The record and the signal cannot drift apart.

The shape is `design/roadmap/`'s and `design/decisions/`', and for the same reason: the index is a
table and a report is a document. Audits arrive slowly (quarterly plus triggers, so a handful a
year). So the count is not what would have outgrown a single file. The reports are, because each
carries its lens, its findings, their dispositions, and what it deliberately did not examine.

`audit-trail` was considered and refused: `design/decisions/0035-scanner-findings.md` already uses
that phrase in its established sense, a chronological record of who did what. That is also what an
operating system means by it (Linux's `auditd`, BSD's audit subsystem). A kernel whose thesis is
confinement is a plausible future home for that feature, and this is not it. `audit-reports` over
bare `audits` because every file in there is literally a report.

## Scope note

Milestone 43 is unchanged and becomes the first audit run *under* this mechanism, inheriting its
"different lens" mandate as lens-rotation's first step. This milestone builds the machine, not an
audit. The deliverable is the index, the tripwire workflow, and the disposition rule written where
audit lanes will read it. SECURITY.md gains a sentence that routine audits exist and where
their reports live. What this cannot promise, said plainly. A mechanism guarantees audits happen and
findings get dispositioned. It does not make any audit good. The lens list is a prompt, not a proof
of coverage. That is the same honest limit the cpu matrix records about five models.

## Follow-on

- **Milestone 43.** The second security audit, which becomes the first run *under* this mechanism
  and inherits its "different lens" mandate as lens rotation's first step.
- **Milestone 93.** The same machine for documentation audits, which reuses the index, the tripwire
  and the disposition rule by adding one row per kind. That is why the thresholds live in the index
  as data rather than in the script as code.
- **Decision.** `design/decisions/0074-audit-cadence.md`, which settled the cadence this block could
  only propose. The block shipped the tripwire computing overdue from those rules, and left the
  numbers to calef.
- **Recorded.** `design/roadmap/0092-security-audit-cadence.md`'s scope note states the limit
  plainly. A mechanism guarantees audits happen and findings get dispositioned. It does not make any
  audit good. The lens list is a prompt, not a proof of coverage.

## Index row

One audit happened and milestone 43 asks for a second; this is the machine that makes them
routine. A recorded cadence with a drift-style overdue tripwire, a lens rotation, and docs
re-baselined in the same lane. Every finding ends fixed, minted as a milestone, or
recorded-accepted; "noted" is not a state (§35's wallpaper rule, applied to audits)

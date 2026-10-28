---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-28
ratified_by: calef
---

# 181. May `script/bootstrap` say "installed everything I could, and this machine is still not good enough" without failing?

Ruled 2026-10-28 (UTC): C, split: `script/bootstrap` provisions and returns zero when it installed
everything it could; QEMU adequacy is its own check where QEMU is invoked (running server, building
the image), its own row in `script/ci-build`'s table; `script/setup` and `script/update` call the
check explicitly so a newcomer on a fresh machine still hears the verdict.

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice-c lane, which found milestone 397 (`script/bootstrap` installs what is missing)'s
`DECISION` gate naming no section. *(Section number provisional until the merge queue lands it.)*

## What is being decided

`script/bootstrap` does two jobs. Its header promises one: *"it checks for what is already present
and installs only what is missing, so it is cheap to re-run and safe to call."* After installing, it
calls `script/qemu-check`, which asks whether the QEMU on `PATH` is new enough to boot this
kernel and exits non-zero when it is not.

Installing what is missing and judging whether what is present is adequate are different
questions that fail in different ways. The decision is whether they split, and if they do, how a
caller asks for one without the other.

## Is the premise true

Checked 2026-09-19 in this worktree. Yes: the header is unchanged, and `script/bootstrap:174` runs
`if ! script/qemu-check; then`, with a second `script/qemu-check` at line 200 after the Linux
source-build fallback.

One figure in milestone 397's block is low, and correcting it strengthens its own case. It says
four callers read this exit code. Measured here, the callers that actually read it are
`script/ci-build:267`, `script/runner-container:139`, `script/setup:18`, `script/update:18`, and
three separate `- run: script/bootstrap` steps in `.github/workflows/ci.yml` (lines 229, 330,
567). `setup` and `update` both run under `set -e`, so a non-zero bootstrap ends them where they
stand.

And the measurement the proposal was built on has weakened, which is worth separating from the
shape. Milestone 287 gave the Linux arm a source-build fallback, so a Linux box with a too-old
packaged QEMU now builds the pinned one and re-checks rather than exiting 1, which was the exact
container failure that prompted this. What survives is the shape: macOS has no fallback and still
exits 1 from inside a provisioning step, and so does any Linux box where `script/ci-qemu` cannot
run.

## What this tree already does in the analogous case

§134 wrote down the `script/` family's own split: what *does* something is a verb (`verify`,
`test`, `fuzz`, `bench`) and what *reports* is a noun (`names`, `citations`, `decisions`, `roadmap`,
`coverage`, `mutation`). `bootstrap` is a verb and `qemu-check` is already its own entry point.
Under that split, an adequacy verdict is not `bootstrap`'s to render; it is a check, and since
milestone 286 the place a check lives is a row in `script/ci-build`'s table.

§97 is the posture one level out: six gates run on every pull request and none of them can stop
one. A verdict that ends the whole local tier before any check has run is the opposite arrangement,
and milestone 286 made it loud rather than silent precisely because a lane could fix that half
without a ruling.

## The options, with what each one costs

| | what | cost |
|---|---|---|
| **A** | leave it; the loud message is enough | zero, and the developer on a stale machine keeps typing three commands by hand |
| **B** | `script/bootstrap --no-verify`, passed by `script/ci-build` | one flag, and the judgment moves into a caller that has no opinion about QEMU versions |
| **C** | split: `bootstrap` provisions and returns zero when it installed everything it could; adequacy becomes its own row in `script/ci-build`'s table | changes what a canonical entry point's exit code means, which seven call sites read |
| **D** | keep both jobs, distinguish the exit codes (1 for "could not install", 2 for "installed, still inadequate") | small; callers that do not look still see non-zero, and one that cares can |

Measured on milestone 286's container: packaged QEMU is 8.2.2, which lacks `riscv-iommu-pci`
(§20, milestone 16b). `script/bootstrap` installed nothing, broke nothing, found everything present
and exited 1 on the adequacy check, while `script/ci-build fmt`, `lint` and `image-permissions` all
pass perfectly well on that machine. The no-argument path runs none of them.

## Recommendation

C, and the reason is §134 rather than tidiness: an adequacy check renders a verdict, it is
cheap, and the table is where a verdict lives now. Most of C is wiring, because `script/qemu-check`
already exists as its own entry point.

**Its real cost, and C must not pay it silently**: `script/setup` and `script/update` get the
adequacy verdict for free today, and the newcomer running `script/setup` on a fresh machine is
exactly the reader that check was written for. C has to keep telling them, which means `setup` and
`update` calling the check explicitly rather than inheriting it.

D is the cheap one and should be priced honestly as cheap. It does not fix the conflation; it
makes it legible to one caller. If the answer is "not worth a split", D is better than A, and
saying that in those words is AGENTS.md's rule about an argument from effort.

B is refused unless calef wants it, because it puts "is this machine good enough" in the caller,
and the caller is a table of checks that has no opinion about QEMU versions.

## How reversible, and who has acted on it

Medium. No wire format and no name a stranger has learned, so the code is an afternoon either
way. What is not free is the exit-code contract: seven call sites read it, three of them in CI, and
a change that makes a previously fatal condition non-fatal cannot be noticed by a caller that was
written before it.

## What is blocked until this is answered

Nothing. Milestone 286 shipped, the failure is loud, and the workaround is three commands a
developer can type. This is a sharpness problem rather than a correctness one, and milestone 397 is
the only thing waiting.

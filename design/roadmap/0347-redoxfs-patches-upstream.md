---
status: REFUSED
raised: 2026-09-03
refused_by: 46, 176
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 347. Offer the two RedoxFS patches upstream

Refused (calef, 2026-10-07 UTC). §46 (Thin primitives or whole subsystems; we write everything in between) decides it: *"Redox is a source we consume and never contribute to ... A patch we need is carried here (§34 (RedoxFS is the primary filesystem), `script/vendor-verify`), never offered upstream."* Redox's own policy refuses AI-generated contributions, and calef ruled on pull request #1806 (2026-10-07) that where a project bans AI work nife sends bug reports only, no code. The two patches stay in `patches/` as carried divergences. Calef ruled on #1815 (2026-10-07) that bug reports to Redox are allowed (disclosed, each approved by calef, security findings privately first) and that no code or patches go. A report about the broken `no_std` build is therefore allowed and is not this milestone. The premise check and text below are as filed.

**Premise checked 2026-09-19, and it held.** `patches/` still holds exactly `redoxfs-no-std-vec-import.patch` and
`redoxfs-no-std-create-uuid.patch` beside its README, which still opens by saying each exists to be
upstreamed and still describes the submission route in the future tense (*"fork on
gitlab.redox-os.org, `git am` this file on a branch, push, open the MR"*). No merge request is
recorded anywhere in the tree.

DECISION because a merge request is a fact that leaves the machine
under this project's name, which AGENTS.md puts in the irreversible column. HARDWARE in its second
sense, the one the roadmap README names: no board is involved, but a person with an account on
gitlab.redox-os.org has to fork, push and open the request, and no lane can do that.

**In brief.** Two patches against RedoxFS are written, applied and documented, and neither has been
offered upstream. `patches/redoxfs-no-std-vec-import.patch` fixes the `no_std` build (four E0425
sites across `filesystem.rs` and `record.rs`) and adds a `--no-default-features` CI job so the
configuration cannot rot again. `patches/redoxfs-no-std-create-uuid.patch` lets a `no_std` caller
create a filesystem by supplying the disk id, the same way `create` already takes `ctime`.
`patches/README.md` names the submission route for both: fork on gitlab.redox-os.org, `git am` the
file on a branch, push, open the merge request.

## Revisit

- **Condition.** Redox's CONTRIBUTING stops refusing LLM-generated contributions, or calef rules that nife sends code to a project that bans it. Until then the patches are carried (§34) and only bug reports go upstream, on the terms calef ruled on #1815: disclosed, each approved by calef, security findings privately first.

## Why this matters

`patches/README.md` opens by saying what the directory is for: *"Each exists to be upstreamed; an
entry leaves this directory when the pin that needed it advances past a release containing the
fix."* Neither entry can ever leave, because nobody has asked. The directory is currently a
permanent home for two patches whose whole design was to be temporary.

The concrete cost is per-bump and recurring. Milestone 203 built the machinery that reports when
upstream moves, so this tree will learn about a new RedoxFS release; each one then re-applies both
divergences by hand and re-checks that they still apply cleanly. `redoxfs-no-std-create-uuid.patch`
is written against the published 0.9.1 rather than master specifically so it applies with zero fuzz
to the pin, and its own README entry says rebasing onto master is the submitter's first step. That
rebase gets more expensive the longer nobody does it.

There is a second cost that is not ours. The `no_std` build of RedoxFS is broken upstream for
everyone, and this tree has the fix sitting in a file.

## What it would take

The mechanical part is small: an account, a fork, `git am`, a push, a merge request per patch. The
`create-uuid` patch wants a rebase onto master first. After that the work is waiting, and answering
whatever upstream asks, which is the part with no schedule.

## Index row

Two patches against RedoxFS are written, applied and documented, and neither has been offered
upstream. `redoxfs-no-std-vec-import.patch` fixes the `no_std` build across four `E0425` sites and
adds a `--no-default-features` CI job. `redoxfs-no-std-create-uuid.patch` lets a `no_std` caller
create a filesystem by supplying the disk id, as `create` already takes `ctime`. The cost recurs at
every bump, since milestone 203 (nothing will ever tell us RedoxFS moved) reports when upstream moves and each report re-applies both
divergences by hand. The `no_std` build of RedoxFS is broken upstream for everyone. This milestone
is refused, so the patches stay carried here.

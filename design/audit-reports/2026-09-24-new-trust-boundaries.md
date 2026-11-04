# Security audit, 2026-09-24: new trust boundaries, read where the change concentrated

**Kind:** security. **Lens:** the trust this tree took on between 2026-09-17 and 2026-09-24,
read adversarially where the week's change piled up. That is three places: the automation that
merges, the bytes a file supplies, and the state a core carries for a thread. **Findings:** fixed
5, minted 1, accepted 5.

**One path to `main` was open to anyone on GitHub, and it is closed.** `helpers/merge-drain.sh`
armed auto-merge, as `nife-smelter[bot]`, on every open non-draft pull request against `main`.
The ruleset on `main` requires zero approving reviews. The repository is public and forkable. So a
stranger whose checks went green was one drain pass from merged, unread, and the merge-group build
of that pull request would have run its own workflow edits with this repository's secrets. No fork
has ever opened a pull request here, so nothing was lost. The path existed for as long as the
drain has run under the App, and the tree's own gates could not see it.

**Nothing in the kernel lets a confined process escape.** The one kernel defect found fails
closed: a thread taking a port range back from everyone else lost its own bitmap on `x86_64`, and
nothing in the tree invokes that take-back yet. Two windows were closed on hardware the tree has
not run on, and one boot-time parser now refuses a length it used to believe.

## Why this lens

`script/audits` reported every trigger fired since the 2026-09-17 audit: 61 milestones, 14
components, 4 ABI constants, 64 external packages. Both uncountable triggers were also yes. Three
components took device authority in the window (`non_volatile_memory_express`,
`framebuffer_driver`, and `installer`, which is handed a whole disk). No new machine class booted;
milestone 578 (an ACPI discovery path for aarch64) is QEMU's `virt` with a different firmware table.

The prior lenses were the whole kernel, the assembly, the shared pages, counterparty input, minted
authority, and userspace confinement. The index names the syscall surface and supply chain as
untaken. Neither is where the week's 62,000 changed lines went. Measured against the last audit's
commit, `.github/`, `scripts/` and `script/` changed 23,700 lines, `crates/` 22,100 (fifteen new
crates, ten of them parsers or producers of a byte format), and `kernel/` 13,700. So the lens
follows the change rather than the list: each of the three places is a new thing the tree trusts,
and each was read by asking who supplies the bytes and what happens if they lie.

The 64 packages were checked first because that trigger fires on any change. Every one is the
`cryptography_provider` graph taken on calef's ruling of 2026-09-20 (`rustls`, `ring`, `rsa`,
RustCrypto), plus `windows-*` for the stick maker's Windows arm. One ruling, one graph, reasoned
in `deny.toml` down to the single advisory it ignores. That graph is a supply-chain audit's whole
subject and was not read here; see below.

### What was deliberately not examined

- The cryptography graph's code. `script/supply-chain` runs cargo-deny on every workspace
  and the ignore for RUSTSEC-2023-0071 carries its reason. Reading `ring` or `rsa` is the supply
  chain lens, still untaken, and it wants a lane with the time to do it properly.
- The syscall surface as a whole. Only the four new ABI constants and the revoke paths were
  read. Still the third untaken lens.
- The Stop hook's configuration. `helpers/handoff-check.py` landed on `main` (#1202) while this
  report was being rebased, so it was read. It takes a transcript path from the harness's own event
  on stdin, regex-searches the assistant's final text, and emits either nothing or a fixed block
  reason. It executes nothing, writes nothing, and echoes none of what it read. The `.claude/`
  settings that wire it are not tracked, so what runs it on a given machine was not seen.
- `machine_discovery`, `board_console`, `stick_maker`, `user_mode_runtime`. Large new crates,
  not on a boundary a stranger's bytes cross: ACPI tables come from firmware, the stick maker runs
  on the host over its own output, the runtime is the syscall stubs.
- The DMA validator, the IOMMU builder, the C seam, RedoxFS, the compositor, the network
  stack. Each has its own record and none moved in the window.
- Timing channels beyond the two recorded below. Nothing was measured.

## The automation that merges

The drain and its consequences are finding 1. The rest of the automation read clean, and the
negatives are worth a line each because a reader will ask. No workflow uses `pull_request_target`.
No `run:` step interpolates a pull request's title, body, branch name or comment; the two that
touch pull request text (`merge-drain.sh`, `queue-hold.sh`) read it through `jq` and pass it to
`gh` as an argument or a `printf` operand, quoted. `coe-architect-label.yml` runs with
`pull-requests: write` on `pull_request`, which a fork's run downgrades to read-only. The App token
is minted per run by `actions/create-github-app-token` in three scheduled workflows and never
written to a file. `script/bootstrap`'s `curl | sh` is rustup's own installer over TLS 1.2 with
`--proto '=https'`, which is what rustup documents and is not a finding.

One hygiene defect was in the tree: a committed `.pyc`, finding 3.

## The bytes a file supplies

Four new parsers read bytes something else wrote: `package_archive` (a package), `device_tree_blob`
(firmware's tree), `firmware_configuration` (QEMU's `fw_cfg`), `portable_executable` (an ELF to
convert), with `file_allocation_table` as a producer beside them. The question for each was the
2026-08-15 audit's: what happens on a length that lies. The evidence per crate, with line numbers,
is in [the appendix](2026-09-24-new-trust-boundaries/evidence.md).

`package_archive` is the one a stranger's bytes will cross first, and it is right. `Package::parse`
bounds the count, the table, and every member's `offset + len` in widened arithmetic before any
accessor runs; the writer refuses what the reader could not distinguish. It has a fuzz target. Its
`BUGS` says nothing installs a package, and that is still true: `install_service` installs a
*stick*, not a package, and the boot file it maps into the installer is the kernel's own, mapped
read-only (`Flags::user_rodata`, no write, no execute).

`device_tree_blob` is total over a slice, and its `from_ptr` was not, finding 6.
`firmware_configuration` is fixed-width and total. `portable_executable` is not total and does
not need to be, finding 7. The installer's own narrowing claim was wider than its mechanism,
finding 8.

## The state a core carries for a thread

Three things arrived here: the FP register file moving on every switch (`kernel/src/fp.rs`), the
two-core `x86_64` default of milestone 315 (a port revoke that reaches every core), and four ABI
constants for `SURVEY`'s new records. Each was read as a confined thread trying to see or keep
what it was not granted.

The FP design is eager, not lazy, and says so against CVE-2018-3665 by name. The unit's enable is a
first-use detector and never decides whose data is in the registers; a switch from a live thread
to a non-live one restores the initial state and then disables. Nothing leaks across a core, a
death or a spawn. What it could not save it also did not disable, finding 5.

The port revoke is synchronous on every architecture the tree builds. `x86_64` broadcasts an NMI
under `IPC_TABLES` and spins for every ack; aarch64's `tlbi ... is; dsb ish` waits in hardware;
riscv64 asks OpenSBI for a remote fence and records that an asynchronous firmware would break it.
The tests that prove cross-core staleness are the two 315 named, not the two revocation tests
added in the window, which walk page tables on one core. The take-back's own core was the gap,
finding 4. The in-flight `MAP` race §13 (capability revocation and untyped reclamation) deferred is still open and still recorded, finding 11.

`SURVEY` takes `ENUMERATE`, not `READ`; refuses an unknown record before the walk; takes any
cursor safely; passes no pointer; zeroes its outputs on `DONE`; and can only see the domain the
capability names, which is Kani-proved. `PLACEMENT` is read-only, and no syscall pins a thread.
What a viewer learns about threads it cannot name is findings 9 and 10.

## Findings

### 1. FIXED: the drain merged any green pull request from anyone, unread

`helpers/merge-drain.sh`'s `queue()` selected on `isDraft == false` and `baseRefName == "main"`
and nothing else. The `main` ruleset (`gh api repos/nifeos/nife/rules/branches/main`) has
`required_approving_review_count: 0`; the repository is public with forking on. A fork's pull
request, once its checks passed, was armed by the App and merged by the queue. Its checks pass
automatically for anyone who is not a first-time contributor, and the drain is what makes a
first-timer's next pull request not first-time. Its merge-group build then runs on a branch in
this repository, with the organization's `AUTOMATION_APP_KEY` in reach of any workflow line the
pull request added. That last step was read from GitHub's documentation and not exercised.

Exposure: none realized. 991 pull requests by calef, 9 by dependabot, zero cross-repository, ever.

Fix: the predicate is now one file, `helpers/queue-eligible.jq`, spliced into both consumers, and
it adds `isCrossRepository == false`. Every lane pushes its branch here, so a foreign head is not a
lane. A missing field is refused, so a consumer that stops asking `gh` for it fails closed.
`helpers/queue-eligible-selftest.sh` is red on the tree before the fix and green after, and
`script/lint` check 11 runs it.

### 2. MINTED: the platform does not require a review, and the App's secrets reach a merge group

Finding 1's fix is a script, which is rung two. The rung above is a repository setting, and every
setting is calef's. The proposal `a-merge-needs-a-review-no-fork-can-supply`, promoted on 2026-09-24 to milestone 588 (a merge needs a review no fork can supply), prices three:
requiring one review the App gives to lanes, requiring a click for every outside contributor's
workflow, and moving the App's secrets into an environment restricted to `main`. It recommends the
second and third now and offers the first with its cost named. calef adopted the second and third the same day: the
fork approval policy now reads `all_external_contributors`, and #1215 moves the secrets. The first
is still open, and the proposal says so.

### 3. FIXED: a compiled `.pyc` was committed past `.gitignore`

`scripts/__pycache__/name_provenance.cpython-314.pyc` landed on 2026-09-18 although both patterns
are ignored; something forced it. Bytecode in a tree is a small supply-chain surface and a hygiene
tell. Removed; `script/lint` check 10 fails on any tracked path that matches `.gitignore`.

### 4. FIXED: a port take-back reset the invoker's own bitmap on `x86_64`

`PortRange::REVOKE` deletes the range from every table but the invoker's
(`sched::delete_port_range_caps_impl`, keeper spared). The arch half then reset the local TSS
unconditionally (`segments::revoke_port_grant_everywhere`). The invoker is the thread on that
core, so a boot-endowed holder that took its range back lost its own bitmap until its next
switch-in and its next `out` faulted it to its supervisor. Fails closed, and latent: no program
invokes the take-back. The sweep now passes `keeper.is_some()` through, and the local reset is
skipped in that case. The test installs the grant, invokes the take-back, and asks the TSS; it went
red at `a_take_back_leaves_the_invokers_own_bitmap_installed` in x86_port_tests.rs before the fix.

### 5. FIXED: SVE, SME and V were neither saved nor disabled

`arch/aarch64/fp.rs` saves `q0-q31` and left `CPACR_EL1.ZEN`/`SMEN` at reset; `arch/riscv64/fp.rs`
saves `f0-f31` and left `sstatus.VS` wherever firmware put it. The header recorded the gap. Every
machine the tree runs on lacks the extensions, so this is a window the emulator cannot open.
`init()` now clears the enables beside the FP one; a thread that executes an SVE or V instruction
takes the same trap and `crate::fp` treats it as a fault. The five FP tests stay green on both
legs; no test can see the closure itself.

### 6. FIXED: a device tree's `totalsize` became a slice of any length

`DeviceTreeBlob::from_ptr` reads the header's length and builds a slice of it before validating
anything; `uefi_loader`'s riscv64 arm copies the leap. A firmware header reading `0xffff_ffff` gave
a 4 GiB slice, the walkers read through the direct map past RAM, and the kernel died in
`memory::init` before any handler could say why. Firmware is the boundary, so this is boot
robustness. Both readers now refuse a `totalsize` over 2 MiB (Linux's `MAX_FDT_SIZE`), with a host
test that builds the lying header inside a buffer that really is that long.

### 7. ACCEPTED: the host-side converter is not hardened, and says so

`portable_executable` runs in `xtask` over the ELF it linked seconds earlier. Its arithmetic can
overflow on a hostile header, a `PT_DYNAMIC` offset is unchecked, `memsz` sizes an allocation, and
`DT_RELAENT = 0` loops. Nothing hostile reaches it. Its `BUGS` now records all four paths and what
would change the verdict. `file_allocation_table::cluster_sector` underflows on clusters 0 and 1;
its precondition is now on the function.

### 8. ACCEPTED: the install survey is narrowed by its role, not its grant

`install_service` said a survey process "cannot write a table anything would read back" because it
has no entropy endpoint. A reader never checks where a table's ids came from, and a `blk` endpoint
is the whole disk with no read-only form. The sentence now says the missing endpoint narrows what
the program would write, not what it can. The bounded `blk` that would make it a grant is already
milestone 421 (the block roster cannot name an NVMe disk)'s wire question.

### 9. ACCEPTED: `PLACEMENT` leaks one noisy bit per spawn about other domains

`pick_spawn_target` samples two cores and takes the shorter run queue, and that queue counts every
domain's threads. A supervisor that spawns and surveys learns which of two random cores was lighter.
The `SURVEY` doc denied any such leak; it now states it. Same reasoning as the 2026-08-17 audit's
counting channel, at lower bandwidth.

### 10. ACCEPTED: `CPU_TIME` differenced against a clock reveals foreign load

Already weighed by §150 (how does a thread's CPU time reach userspace?) for the within-domain case. The cross-domain inference is what a clock
capability already lets a thread measure about itself, so it widens only for an `ENUMERATE` holder
with no clock. Recorded in the ABI doc; nothing to add.

### 11. ACCEPTED: the in-flight `MAP` race §13 deferred is unchanged

`page_frame_map` writes the PTE and then logs it; a revoke sweeps the log. A `MAP` on another core
that passed its lookup before the sweep and logged after it keeps a live PTE with no capability.
Recorded at `revoke.rs` since §13 with seL4's answer named. Not widened by anything in the window;
the port path closes its analogue by holding `IPC_TABLES` across the broadcast, which is the shape
a fix would take.

## Is any confinement claim in this tree false as stated?

No. The claim that was false is not about confinement: "the queue merges what has earned it"
(AGENTS.md's steward) meant "green", and green was never the same as reviewed. `SECURITY.md`
gains this report; nothing else it says needed changing.

## Method, so the negatives can be judged

The change was measured with `git diff --stat 8e4587056 origin/main` per directory. The
automation was read by grepping every `${{ }}` in `.github/workflows/` and every `gh`/`jq` call in
`scripts/`, then asking GitHub for the live ruleset, the fork-approval policy, and every pull
request ever opened. Two parallel reads took the parsers and the kernel state under written briefs
demanding file:line and the triggering input for anything claimed; their evidence is the appendix,
checked against the source before anything here was written. Every fix was proven red on the tree
before it, or the report says a test cannot see it. `script/verify` was not run here: the
capability and paging proofs replay in CI on this pull request.

## What wants a lane of its own

- The supply chain lens, now that 64 packages of cryptography are in the graph under one
  ruling. `deny.toml` and `script/supply-chain` are the gate; nobody has read the code.
- The syscall surface itself, the third untaken lens, three audits running.
- Finding 11's fix, if calef wants the in-flight `MAP` window closed: the port path's shape,
  `IPC_TABLES` held across the sweep, priced against the IPC fastpath.
- The Stop hook's other half, if one is written. Today's script is a regex over the assistant's
  own words, which is safe. A hook that ever acts on a transcript's *content* would be a parser of
  untrusted text, and would want the counterparty-input lens.

## Process notes on the mechanism itself

Both reads by delegated agents produced findings this session would not have reached alone, and
both also produced a wrong claim each that source reading caught (a caching path that does not
exist; a test filter that matched nothing). The discipline that held was the brief's: a claim
without a line number is not a finding. The audit index said reports land in this directory; the
brief said `notes/`. This report follows the index and says so here rather than in a chat window.

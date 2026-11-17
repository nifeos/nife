# Security audit, 2026-11-17: the verifier

**Kind:** security. **Lens:** the verifier, read as code that runs with authority nobody reviewed:
the stock Kani that `.kani-version` pins and the bytes `cargo kani setup` fetches behind it, the
daily job that bumps the pin, the patched Kani the riscv64 row builds, and the falsification records
that say each proof can fail. Then the window's ten new external packages, followed to the graph
each reaches. **Findings:** fixed 0, minted 3, accepted 2.

The finding to carry off is what the pin does not pin. `.kani-version` fixes the version of the
`kani-verifier` wrapper on crates.io, and that wrapper's `setup` then downloads the real prover (the
Kani compiler, CBMC and its solvers) from a GitHub release with no checksum. The release is mutable.
The riscv64 row does the same and also builds Kani from a git tag it never checks against a commit.
So the bytes that decide whether every proof in this tree holds are the bytes a release page served
on the day, and nothing in the tree records which bytes those were. Minted, because the fix is a
digest file per bundle and a commit pin, with a bump path that writes both.

Nothing read lets a pull request change which prover runs without changing a file the queue
re-proves, and every Kani job runs on a GitHub-hosted runner with `contents: read`.

## Why this lens

`script/audits` on `main` at `9e8c189c0` reported both kinds DUE. Security fired on two triggers:
milestones built 489 to 510 (+21, fires at 15) and external packages 199 to 209 (+10, fires at 10).
Documentation fired on milestones (+21, fires at 10). Issue #3672 is the scheduled workflow's repeat
row; its reds printed the same `DUE` line the run on this commit printed, so none was a broken run.
This report closes the security kind. The documentation kind is still due and wants a sweep of its
own.

The README's lens list names three candidates no audit has taken: the graphical drivers' parsers,
the device authority radon's peripherals took, and the verifier's own pin. The 2026-10-26 audit,
which read the compiler, listed Kani, `.kani-version` and the job that bumps it as deliberately not
examined. The compiler and the verifier are the two programs whose verdicts the tree trusts without
reading them, and only one has been read. The package trigger points the same way, since the pin is
an external package whose real payload is not in any lockfile.

### The two uncountable triggers, answered

Has a new component taken device or network authority? **Yes.** Four components arrived in the
window and three hold device authority on radon: `jh7110_clock_and_reset` (the clock and reset
server, carried in the base image), `designware_ethernet_control` (the GMAC's bring-up, moved out of
the kernel) and `power_management_chip_route` (the one process that writes the board's power
controller). This audit did not read them. They are the second candidate on the README's list, and
that list now says they are the most pressing one. The fourth, `boot_self_test`, runs the self-test
checks that need a user program and holds no device.

Has this booted on a new machine class? **No.** The window's board work is on radon and xenon,
both of which booted before it.

## What was read, and what came back clean

**How stock Kani is installed.** Every install goes through `script/kani-install`, which reads
`.kani-version` and runs `cargo install --locked kani-verifier --version "=$pin"`, quoted, so the
wrapper's own dependency graph is the one its published lockfile names. `script/lint` refuses any
other `cargo install ... kani-verifier` outside that script and `script/kani-bump`. An installed
version that differs from the pin is replaced, never kept, and a setup whose toolchain link dangles
is rerun.

**The bump job, `propose-kani` in `toolchain-bump.yml`.** It mints the automation App's write token,
but unlike the nightly's `propose` job, which the 2026-10-26 audit found building with that token
readable, it installs and runs nothing. `script/kani-bump` reads `max_stable_version` from crates.io,
refuses anything that is not three dotted numbers, and writes it to `.kani-version` with `printf
'%s'`. The workflow interpolates the value only into double-quoted strings, so a hostile version
string cannot reach a shell. The commit changes `.kani-version` alone.

**Where a Kani bump is proved.** A diff of `.kani-version` alone is not a harness change, so the
pull request's own prove shards skip (finding 2). In the merge queue,
`script/falsifications --plan-since` treats the root file as unattributable and re-falsifies every
replayable record across seven shards, and the prove shards run. A bump cannot merge on a prover
that has stopped finding the bugs the records plant.

**The jobs that run Kani.** `verify.yml` holds `contents: read` at the top, adds `actions: read` on
one job, and takes no secret and no environment; every job runs on a GitHub-hosted runner.
`kani-reach.yml` is dispatch-only. `falsifications.yml`'s `record-times` job, the one with the App
token, consumes only the TSV artifacts the shards upload and runs this tree's scripts over them.

**The riscv64 row's cache.** Its key covers the patch's hash and `script/verify-riscv64` itself, so
a change to either rebuilds the patched Kani. `KANI_APART` fails on a harness name that no longer
exists rather than skipping it.

**The falsification census.** `script/falsifications --count` reports 308 harnesses, 308 falsified,
0 unfalsifiable. 299 records are replayable and 9 attested; finding 3 is about five of
the nine.

**The window's new packages.** From `830fee89f`, where the last security report landed, to this
commit the external package count went from 199 to 209.

- `jiff`, `jiff-core`, `jiff-static`, `portable-atomic` and `portable-atomic-util` arrive with the
  calendar's replacement (ruled 2026-11-12), with default features off, in `components` and
  `crates/timetable`.
- `rsext4` is host-only, in `tools/ext4_oracle` (ruled 2026-11-16).
- `tock-registers-macros` and `tock-registers-codegen` are proc-macros that arrived with the
  tock-registers 0.12 bump (#3344). That pull request carried a ruling of 2026-11-09 approving both,
  which records that they run on the build host only and that their dependencies were already in the
  lockfile.
- `windows-core`, `windows-implement`, `windows-interface` and `windows-strings` are reached only
  through `loom`'s `generator` dependency, and only on a Windows target. `std_exerciser`'s graph is
  the one that holds them, through `clock_contract`'s loom tests. Nothing in the tree builds for
  Windows.
- `lamfat` and `twox-hash` left.

## Findings

### 1. MINTED: the prover's bytes are not pinned

`.kani-version` pins the `kani-verifier` crate, a wrapper of a few hundred lines. Its `setup`
(`src/setup.rs` in the 0.68.0 crate, read from the registry source) downloads
`kani-<version>-<target>.tar.gz` from the model-checking/kani GitHub release with `curl -sSLf`,
unpacks it with `tar`, and installs whatever Rust toolchain the bundle's `rust-toolchain-version`
names. It checks no digest and no signature. The bundle holds the Kani compiler, CBMC and the
solvers, which is to say everything that decides a verdict.

`gh api` on the 0.68.0 and 0.67.0 releases reports both as `immutable: false`, so an asset can be
replaced under the same name. GitHub publishes a SHA-256 for each asset (the 0.68.0
x86_64-unknown-linux-gnu bundle is `32e2b484d73ede0bbf64a2cf0879c4259422497d8aec4ee67d448de9ae7843d3`),
and nothing in the tree records or compares it.

`script/verify-riscv64` repeats the download for its own 0.67.0 bundle, unchecked, and then runs
`git clone --depth 1 --branch kani-$version`. A tag can be moved. The script deletes `.git` after
the clone, so not even the build records which commit it patched.

The tree's analogous cases check a digest. `cargo-mutants` is compared against
`.cargo-mutants-sha256` in both mutation workflows, and Node against `site/.node-sha256` in the
record site's workflow. The prover is the one downloaded tool the tree trusts most and checks least.

Reach: every Kani job is on a hosted runner with a read-only token, so a replaced bundle steals
nothing in CI. What it can do is report a proof that does not hold, on every pull request and in the
queue, and run as the person on any machine where a lane runs `script/kani-install`. The
falsification replay in the queue catches a prover that stops finding planted bugs. It does not
catch one that finds the planted bugs and passes a real one.

A digest copied at bump time protects against a release changed after the bump, not against a
hostile release the bump copies. Finding 4 is why that second case is accepted. Severity medium.
Proposed as [the prover's bundle is checked against a pinned digest](../roadmap/proposals/the-provers-bundle-is-checked-against-a-pinned-digest.md).

### 2. MINTED: the bump pull request says it is proved where it is not

The `propose-kani` job's commit message and pull request body, its comment, and milestone 795's
block all say the pull request's own `verify` run is the proof on the new Kani. A probe commit that
changes only `.kani-version` gives `harness=false` from `helpers/harness_change.py`, the PR-side
prove shards skip, and the proof runs only in the merge queue, as the 2026-11-16 ruling on heavy
gates intends. The protection holds (see what came back clean); the text tells a person reading the
pull request that a green check there means something it does not. Severity low (records).
Proposed as [the Kani bump says the queue proves it](../roadmap/proposals/the-kani-bump-says-the-queue-proves-it.md).

### 3. MINTED: five harnesses are falsified only by a person's word

Of the 308 falsification records, 9 are attested rather than replayable, all of them on
architecture-specific harnesses, because a Kani harness compiles for the host and the sweep runs on
arm64. Four of the nine carry a patch, and the weekly `sweep-x86_64` job in `falsifications.yml`
replays them with `--sweep-attested`. Five carry none, so no job can show that their proofs can
still fail:

- the three riscv64 MMU harnesses, which only the patched Kani proves:
  `the_live_root_is_the_root_that_was_installed`,
  `the_asid_probe_counts_the_implemented_bits_and_puts_satp_back` and
  `a_switch_sweeps_the_tlb_exactly_when_the_asid_cannot_be_trusted`;
- `every_cpacr_edit_changes_exactly_the_field_it_names` in `kernel/src/arch/aarch64/fp.rs`;
- `no_vector_belongs_to_two_bands` in `kernel/src/arch/x86_64/interrupt_controller.rs`.

So neither a Kani bump nor a change to `patches/kani-0.67.0-riscv64-target.patch` is ever shown not
to have blinded these five. A patched prover is the case where that is most worth showing.
Writing the patches is not enough alone: `--sweep-attested` takes every attested record that has a
patch, whatever its architecture, so an aarch64 or riscv64 patch would turn the x86_64 job red. The
`BUGS` entry in `script/falsifications` names a per-record host statement as a format change and the
architect's; the proposal recommends reading the architecture from the patch's `arch.<arch>.` path,
which the convention already carries, and states that choice as provisional. Severity low. Proposed
as
[attested falsifications replay where they compile](../roadmap/proposals/attested-falsifications-replay-where-they-compile.md).

### 4. ACCEPTED: a Kani bump merges unattended

`propose-kani` arms its own pull request to merge, as the nightly's job does, and the default to
merge was set on 2026-09-24. A hostile `kani-verifier` release on crates.io, or a hostile bundle
behind an honest one, would reach `main` with no person reading it, and a digest copied at bump time
would copy the hostile bundle's digest. The 2026-10-26 audit accepted the same property for the
nightly (its finding 6), and the reason carries: the queue's full re-falsification is the check
that runs, and holding every bump for a person trades a daily review for a threat nobody has seen.
Finding 1's digest still narrows the window to the day of the bump.

### 5. ACCEPTED: the pin's shape check is a glob

`script/lint`, `script/kani-install` and `script/kani-bump` each accept `.kani-version` when it
matches `[0-9]*.[0-9]*.[0-9]*`, which also matches `0.6.8.0.0` or a version with text after it. The
value reaches cargo as one quoted `--version "=$pin"` argument, and cargo refuses anything that is
not a version requirement, so a malformed pin fails the install loudly. Tightening three copies of a
check to say what cargo already says is not worth a lane.

## What was deliberately not examined

- The three device components of the first uncountable trigger, and their grants. They want the
  audit the README already lists.
- The contents of the Kani bundle itself, CBMC's solvers, and kani#4913, the upstream change the
  riscv64 patch stands in for. Only the patch's place in the build was read, not its diff.
- `kani-verifier`'s own dependency graph beyond its use of `curl` and `tar`.
- The Rust toolchain the bundle installs. It is the toolchain the 2026-10-26 audit read the path of,
  installed by a different route, and this audit did not compare the two.
- The harnesses themselves, and whether each proves what its name says. That is a proof review, not
  a security read.
- Nothing ran Kani. The one probe was a local commit, removed with its scratch worktree.

## What wants a lane of its own

The three proposals of findings 1 to 3, finding 1 first. Then the device authority the window gave
radon's peripherals, as the next security lens.

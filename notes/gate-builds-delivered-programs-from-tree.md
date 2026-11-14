# The gate and the programs basalt delivers: build them from the candidate tree, or repin

*Name: provisional, minted 2026-11-14 (UTC) by lane/gate-builds-delivered-programs-from-tree.*

This note answers how a nife change that breaks a wire format a delivered program reads can land.
It measures each option and recommends one. The recommendation is the rule in force (2026-11-14
UTC), and milestone 1115 (the gate builds basalt's delivered programs from the candidate tree)
built it; the last section says where the build departs from the plan below.

## The problem, in the code

`cargo xtask basalt-fetch` (`xtask/src/basalt.rs`, `fetch`) downloads five packages (`PACKAGES`:
ripgrep, ioping, hackbench, sqlite, demos) for three architectures from the basalt Release that
`xtask/basalt-pin` names, and accepts each one only under the digest that basalt's recipe records at
the pinned basalt commit. Four merge-group rows run those programs (`script/ci-build` lines 153 to
156): `swish-check`, `swish-check-x86_64`, `rg-threads` and `rg-threads-x86_64`.

Every one of those programs links nife's patched standard library. The C programs also link nife's
C library (basalt's `helpers/packages.py`, `build_c_shape`, which runs the pinned nife's
`helpers/build-c-program.sh`). So each program carries a copy of 14 nife crates, which the std farm
copies into std verbatim (`xtask/src/farm/copied.rs`, `COPIED_FILES`): `abi`, `allocation_cache`
and 12 contract crates, `clock_contract` and `program_argument_contract` among them. A program built
at an older nife commit reads the older wire format.

Two facts make this worse than the BUGS entry says:

- `xtask/basalt-pin` names basalt commit `155eba5`, whose `pins.toml` names nife commit `77d8888`
  (2026-10-17). The file has changed twice, both on 2026-10-17, and nothing moves it. basalt's own
  nife pin moves daily (basalt main pins nife `a60748a`, 2026-11-14), but nife does not follow it.
  `git diff --stat 77d8888 origin/main` over the std farm's inputs and the C library shows 149 files
  changed. Every program the gate runs is a month older than the kernel it runs on.
- A break does not always fail loudly. Merge-group run 43634364501 (PR #3576, the clock page's
  two-copy layout) failed all four rows. `unmodified_ioping_times_reads_in_a_granted_directory` hit
  its 90 s watchdog "while making progress": ioping's one-copy clock reader spins on the two-copy
  page, because the magic stays `CLOCKv01` and the old reader cannot tell the layouts apart.

The rule that shapes every answer below: a pin never points at an unmerged branch. basalt's pin
names a merged nife commit, so basalt cannot publish a program built from a nife change before that
change is on nife's main.

## The measurements

Everything here is either a GitHub run (cited by id) or a local run on this machine: 80 threads of a
Xeon Platinum 8160, 53 GB of memory, shared with other lanes' QEMU suites while it ran. Each local
run was one `helpers/packages.py build` from basalt's main (`cece88a`) against this worktree's tree
(nife main `3a47249` plus the claim commit), under `systemd-run --user --scope -p MemoryMax=12G` and a
heavy-job slot, from a cold `/tmp/basalt-build`. Peak memory is the scope's `memory.peak`.

| what was built | wall time | peak memory | disk under `/tmp/basalt-build` |
|---|---|---|---|
| ioping, three architectures | 130 s | 3.49 GiB | 2.0 GB |
| ripgrep, three architectures | 165 s | 3.20 GiB | 2.2 GB |
| all five, three architectures | 533 s | 3.59 GiB | 2.7 GB |

Each local run exited 1, after building and writing every package: the packages built from the
current tree do not match the digests basalt's recipes record for nife `a60748a`. That exit is the
digest check working as designed, and it is the first thing a gate build has to account for (below).

basalt's `packages` workflow does the same build on GitHub's hosted `ubuntu-24.04-arm` runner (4
vCPUs, 16 GB of memory for a public repository). In run 43602693510 (2026-11-14), the job "build
the packages at nife's pin" took 371 s: 21 s to provision the toolchain, 26 s to install clang 19,
and 310 s for `helpers/packages.py build`. Run 43449221882 took 374 s. Inside the 310 s, the
build compiles the standard library and the C library once per architecture. The first C recipe,
hackbench, pays for it: about 25 s an architecture. Every later recipe reuses them (ioping: about 4
s an architecture). ripgrep's crate build is 32 to 35 s an architecture.

The four rows, in the ten most recent merge-group runs that ran them (for example 43633512417,
43624671851, 43600353220): `swish-check` 804 to 882 s, `rg-threads` 454 to 502 s,
`rg-threads-x86_64` 504 to 569 s, `swish-check-x86_64` 342 to 488 s. The whole merge group took 908
to 940 s. Each row's setup before `script/ci-build` is under 30 s. `helpers/job-budget.py` holds
every job to 20 minutes (`BUDGET_MINUTES`), 1,200 s.

## 1. What it takes to build the five programs from the candidate tree

The build itself needs no new code. basalt's `helpers/packages.py build <nife
checkout> <repository>` already builds every recipe against whatever nife checkout it receives, with
that checkout's own toolchain (`rust-toolchain.toml`), standard library (`cargo xtask std-src`),
C library and packer (`cargo xtask package`). It writes a repository in exactly the layout
`cargo xtask basalt-fetch` reads, and `NIFE_BASALT_REPOSITORY` (`xtask/src/disk.rs`,
`BASALT_REPOSITORY`) already tells `basalt-fetch` to take its programs from such a repository
instead of the Release (`xtask/src/basalt.rs`, `fetch` and `take_programs`). basalt's own `install`
job runs nife's `swish-check` rows exactly this way, on the repository it just built.

What a gate build needs:

- Toolchain. The nightly `rust-toolchain.toml` names, with `rust-src`. `script/bootstrap`
  already provisions it on every row (16 to 28 s in run 43633512417).
- std overlay. `cargo xtask std-src`, which `helpers/packages.py` runs. No patch beyond the
  tree's own `patches/std-nife/overlay`.
- Patches. One, basalt's `patches/ripgrep-ignore-walk.patch`, applied to ripgrep's `ignore`
  crate by the recipe. The C recipes compile upstream sources untouched.
- A C compiler with the AArch64, RISC-V and x86 backends. `script/bootstrap` installs one when
  none is present (`have_cross_clang`). basalt pins clang 19 (`NIFE_CC=clang-19`) so its digests
  reproduce; a gate build does not need basalt's digests (below), so the bootstrap's clang serves.
- Network for the pinned sources: ioping's and hackbench's C files, SQLite's amalgamation zip,
  ripgrep and its crates from crates.io. The recipe checks each by SHA-256, and cargo by `Cargo.lock`.
- basalt's build script and recipes, at the basalt commit `xtask/basalt-pin` names. The gate
  already reads that commit's recipes over the network (`pinned_index`); reading
  `helpers/packages.py` and `patches/` too is a checkout of the same commit.
- The digest check, set aside. `build` fails when a package differs from its recipe's
  `[package_digest]`, and a candidate tree always differs. `--update-digests` rewrites the recipes
  in the scratch checkout instead, so the build succeeds; the gate discards that checkout. A flag
  that skips the check without rewriting would be cleaner, and is a change to basalt.
- Time. 310 s of build on a 4-vCPU hosted arm64 runner for all five programs and all three
  architectures. About two thirds of that for the two architectures an arm64 row runs, and one
  third for the x86_64 rows, since each architecture's work is independent.
- Disk. 2.0 to 2.2 GB under `/tmp/basalt-build` for one or two recipes; 2.7 GB for all five.
  A hosted runner's 14 GB SSD has room.
- Memory. 3.2 to 3.5 GiB peak for one recipe, 3.59 GiB for all five, against a hosted runner's
  16 GB.
- Runners. The three architectures build on any host, since every target is a cross target. An
  arm64 row (`swish-check`, `rg-threads`, `rg-threads-x86_64`) is a hosted `ubuntu-26.04-arm`.
  `swish-check-x86_64` runs on the owned x86_64 runners in a merge group when the repository sets
  `NIFE_RUNNER_OWNED`. `helpers/packages.py` builds at the fixed path `/tmp/basalt-build` (`BUILD_ROOT`) and removes
  it first, so two builds on one owned host at once would destroy each other's work. A hosted runner
  is one job per machine and has no such collision.

Where to put the build decides what it costs the queue:

- One build job that the four rows need. The build's 371 s join every row's critical path,
  since a row's own setup is under 30 s. `swish-check` would reach 1,175 to 1,253 s against the
  1,200 s budget, and the merge group would grow from about 930 s to about 1,300 s.
- Each row builds the architectures it runs. `swish-check` and `rg-threads` build aarch64 and
  riscv64, about 210 s; the x86_64 rows build x86_64, about 105 s. `swish-check` would reach 1,015
  to 1,090 s: under the budget, with 110 to 185 s to spare. The other rows stay under 700 s. The
  merge group grows by about 210 s, to about 1,150 s. Four rows build, so the queue pays about 630 s
  of runner time per merge group in place of 371 s.

## 2. Building only when a change touches the delivered programs' inputs

The tree already computes the right test. `cargo xtask std-stamp` prints `std_inputs_stamp()`
(`xtask/src/farm/stamp.rs`): one hash over the toolchain's version, the 14 copied crate files and
their out-of-line modules, the three target specifications, every file under
`patches/std-nife/overlay` and the vendored heap. Nothing in CI calls it today. Adding `c_library/`,
`vendor/relibc/`, `helpers/c-library-cc.sh`, `helpers/c-library-cflags.sh` and
`helpers/build-c-program.sh` covers the C programs. A hand-kept list of "crates whose wire formats
the programs read" would duplicate `COPIED_FILES` and drift from it, so the stamp is the list to use.

The comparison has to be against the right commit. Comparing the candidate with the merge group's
base is wrong: once one break lands by building, the next merge group that touches no input would
fetch the old programs and fail. The sound comparison is the candidate against the nife commit the
fetched programs came from, which is `pins.toml`'s nife commit at the basalt commit
`xtask/basalt-pin` names (each package's source archive records it again, as `BUILT-FROM`). Build
when they differ; fetch when they agree. That test heals itself: after a repin the inputs agree
again and the gate fetches.

How often it would build, from `git log --first-parent origin/main` over those paths:

| window | merges to main | touch the std inputs | touch the std or C library inputs |
|---|---|---|---|
| since 2026-10-15 | 1,227 | 105 (8.6 percent) | 111 (9.0 percent) |
| since 2026-11-07 | 224 | 28 (12.5 percent) | 30 (13.4 percent) |

These counts are per change against its base. Against the pinned commit, the narrow test builds on
every merge group today. The pin is a month old, and 149 input files have changed since. The
narrow fix saves anything only when `xtask/basalt-pin` follows basalt's Release. basalt publishes a
Release after each pin bump (`rolling-2026-11-14-cece88a` at 14:22 UTC, 21 minutes after its pin
bump merged). A daily bot pull request that moves `xtask/basalt-pin` would keep the pinned commit
within a day of main. basalt's own `pin-bump.yml` does the same in the other direction. Between a
stamp change and the next repin, every merge group builds, so the saving is the share of merge
groups whose inputs match the pin, not the 87 to 91 percent above.

## 3. The break-and-repin procedure

The rule allows exactly one order: the breaking change merges to nife's main first, then basalt
builds against it, then nife moves its pin. Step by step:

1. The breaking pull request declares the programs it breaks, and the four rows skip those
   programs' lines and tests in its merge group and on main until the repin. The four rows are required
   checks, so without a skip the change cannot merge at all. No such mechanism exists today. It
   would be a file the change edits, read by `basalt-fetch`, naming each package and the merged
   commit that broke it, which the repin then removes.
2. The change merges. From here, main runs no test of the skipped programs.
3. basalt bumps its nife pin to that merge commit or later. `pin-bump.yml` runs daily at 06:17
   UTC, or by `workflow_dispatch` with the commit. basalt's gate builds the packages at the new pin
   and runs nife's own `swish-check` rows against them (`packages.yml`, `install`). Its last five
   bump pull requests merged 24 to 26 minutes after opening, with one at 543 minutes.
4. basalt publishes the Release. About 21 minutes after the bump merges (run 43602693510).
5. nife moves `xtask/basalt-pin` to that basalt commit and Release in one pull request that also
   removes the skip. It goes through nife's queue: one merge group is 908 to 940 s.

The window in which main does not test the broken programs runs from step 2 to step 5's merge. With
someone dispatching each step at once, that is about 24 + 21 + 16 minutes, roughly an hour. Left to
the daily schedule, it is up to a day, plus whatever delays step 5. Every other change that merges
in the window lands untested against those programs, and a second break in the window has nothing
to catch it. Both repositories' queues are on the path, and the procedure has four hand-offs that
each wait on a person or an agent to notice.

## 4. A third option: a version-skew shim, change by change

The writer keeps serving the old format until the readers move: an expand-and-contract migration.
The breaking change becomes two changes. The first adds the new format beside the old one, so the
old programs still work and the change merges green. basalt rebuilds, nife repins, and a second
change removes the old format.

It leaves no untested window, and it is what a system with installed users must do anyway. But
nife's formats rarely admit it. The clock page sits at a fixed address with a frozen identity (the
clock page's interface 8, version 1, magic `CLOCKv01`), so a one-copy page and a two-copy page cannot
both live there. The reader checks the argv page's magic (`program_argument_contract`), so
the shell would have to learn which magic each program expects, which is a negotiation mechanism
the system does not have and would then keep. Each shim is a design of its own that a later change deletes, and it doubles the number of pull requests per break. It suits a format that already
carries a version a reader checks, after nife has users, and not as the general fix.

Another candidate, a gate that skips the rows a change provably breaks and records the debt, is
step 1 of the procedure above. It does not shorten the window; it only makes the procedure
mechanical.

## 5. Recommendation

Build the delivered programs from the candidate tree in the merge group, with basalt's own
`helpers/packages.py` at the basalt commit `xtask/basalt-pin` names. Each of the four rows builds
the architectures it runs and hands them to itself with `NIFE_BASALT_REPOSITORY`.

The reasons:

- It is the only option under which a wire-format break lands in one pull request with every row
  green and every program tested on main the whole time. The procedure leaves a window of about an
  hour to a day; the shim is not available for the two breaks that are waiting.
- The build is already written and already trusted: basalt runs the same script on every pin bump
  and runs nife's rows on its output. nife adds a checkout, one environment variable and the
  `--update-digests` workaround (or a basalt flag that skips the digest check).
- The cost fits. In-row, it adds about 210 s to the slowest row and keeps it 110 to 185 s under the
  20-minute budget. A separate build job would push `swish-check` past the budget.
- The gate gives up nothing basalt does not already check. The gate stops testing the published bytes,
  but basalt's `packages` workflow builds them twice, compares them byte for byte, and runs nife's
  `swish-check` rows on them at every pin bump. Each repository tests what it owns.

Two smaller pieces belong with it: `xtask/basalt-pin` should follow basalt's Release (a daily bot
pull request), and the build should not run two at once on one owned host while `BUILD_ROOT` is a
fixed path.

If all options cost the same, the choice would not change: building from the candidate tree is the
only option with no untested window. What cost changes is the narrow test of section 2. It would
build only when the candidate's inputs differ from the pinned commit, and fetch otherwise, which
saves the 210 s on the merge groups whose inputs match. At equal cost it is not worth its second
mode: two paths through the gate, one of them used rarely, and a fetch path whose only extra
coverage basalt already provides. So the narrow test is a recommendation about effort, to add only
if the queue's wall time becomes the constraint, and only once `xtask/basalt-pin` follows basalt
daily, since before that it would build on every merge group anyway.

## How milestone 1115 built it

`cargo xtask basalt-build <architecture>...` (`xtask/src/basalt.rs`, `build`) is the build of
section 1. Each of the four rows runs it, in a merge group and on a push to `main`, for the
architectures it runs, and hands the repository to its `basalt-fetch` with
`NIFE_BASALT_REPOSITORY`. A pull request's run fetches. Two details differ from the plan above,
both because of what basalt's script does at the pinned commit:

- It does not pass `--update-digests`. At that commit the flag refuses a build of fewer than all
  three architectures. The build runs without it, and a run that exits 1 counts as built when the
  fresh index lists every package asked for: the script writes the index only after every package
  is built and packed, so the one failure left after it is the digest check.
- It does not use `/tmp/basalt-build`. The script takes no option for its build root, so the gate
  imports it and sets `BUILD_ROOT` to a directory of its own for each run. Two builds on one host
  do not collide, and nothing needs a lock.

## What this leaves open

- A `--no-digest-check` flag (provisional name) for basalt's `helpers/packages.py build`, so the
  gate does not infer success from a failed run: a basalt change. `xtask/src/basalt.rs`'s `BUGS`
  section records the inference.
- A daily bot pull request that moves `xtask/basalt-pin` to basalt's newest Release: a milestone,
  proposed as "nife's basalt pin follows basalt's Release". `xtask/src/basalt.rs`'s `BUGS`
  section records that the pin does not follow.
- `BUILD_ROOT` as a fixed path in basalt's `helpers/packages.py`, which the gate sets around: a
  `BUGS` entry there belongs to basalt.

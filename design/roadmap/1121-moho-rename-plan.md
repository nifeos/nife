---
status: NOT-STARTED
raised: 2026-11-21
milestone_dependencies: 1075, 1115
decision_dependencies: 323
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1121. The Moho rename: the order in which the binary interface takes its name

§323 (the binary interface, and the names of the layers) names the capability binary interface
Moho. It gives the interface `target_os`, the ABI crate, the `std` port, the SDK crates and the
in-binary names, and it leaves the order of the rename to a plan. This block is that plan. It
changes no code. The branch is `lane/moho-rename-plan`, and the number is provisional.

Reuse: `script/crate-move --dry-run` for the crate's dependents, the checklist in
[design/naming/rename-what-moves.md](../naming/rename-what-moves.md), `script/names`,
`helpers/format_values_check.py`, the farm stamp (`xtask/src/farm/stamp.rs`), and milestone
1116's daily basalt pin pull request (nife's basalt pin follows basalt's newest Release). No new
tool.

## Goal

Every spelling of the capability binary interface in nife and basalt says Moho, with nothing
renamed twice and no queue run broken. The kernel stays `nife`, and the names inside binaries stay
as they are.

## What moves, and what stays

Five spellings move.

- The crate. `crates/abi` becomes `crates/moho_abi`, and the package and the code name become
  `moho_abi`, as milestone 1075 (`abi`: what an outside maintainer would fix before the move)
  records under decision A.
- `target_os`. The three target specifications say `"os": "nife"`, and `std` takes its
  `cfg(target_os = "nife")` arms from the farm's patcher. Both become `moho`, and so
  `std::env::consts::OS` reads `"moho"`.
- The triples. `aarch64-unknown-nife`, `riscv64-unknown-nife` and `x86_64-unknown-nife` are the
  file names of the target specifications, so they become `*-unknown-moho`.
- The `std` port. `patches/std-nife/` and the overlay's `sys/*/nife` modules become `std-moho` and
  `sys/*/moho`, since upstream `std` names each port's directories after its `target_os`.
- The C side of `target_os`. The macro `__nife__` (from `helpers/c-library-cflags.sh`, tested in the
  generated relibc headers), the C library's platform module and the vendored relibc arms become
  `__moho__` and `moho`.

These stay.

- The names inside binaries, under the wire-literal exemption: the ELF note owner `nife`,
  `.note.nife.manifest`, the magics `NIFESLOT` and `NIFESTK1`, the partition GUID and its crate
  `nife_partition_types`, and `nifefs`.
- The kernel's name. `nife` stays the microkernel's, in its boot log, in `boot_ladder`'s heads and
  in the `uname` system name. Open decision 2 asks about the last one.
- The repository's tool names: the toolchain link `nife-dev-<directory>-<hash>` and the farm
  directory `target/nife-farm`. They name nife's build, not the interface. The maintainer keeps
  them as a provisional two-way ruling.
- `.nifepkg`, basalt's package format. It is a distribution's format, not the binary interface,
  and 1,153 Release assets carry it. Renaming it belongs to a ruling of its own.

## The census

The full count by file kind, with the commands, is in
[the census appendix](1121-moho-rename-plan/census.md). It was measured at `nifeos/nife@97d16ae8b`
and `nifeos/basalt@2c437ca`. In short, 731 nife files carry at least one spelling that moves.

| spelling | lines | files | where most of them are |
|---|---|---|---|
| `abi::` | 2,190 | 450 | 1,741 lines of Rust in 232 files; 325 lines of prose |
| `crates/abi` (the path) | 370 | 128 | 155 lines of prose; 104 lines of `design/module-block-baseline.txt` |
| `target_os = "nife"` | 48 | 24 | 36 lines in 15 files of `vendor/relibc`; none in first-party Rust |
| `"os": "nife"` | 10 | 10 | the three target specifications |
| `unknown-nife` | 160 | 82 | 37 lines of xtask; 70 of prose |
| `std-nife` | 120 | 90 | 87 lines of prose; two workflows |
| `__nife__` | 193 | 14 | the generated relibc headers |

basalt names none of the crate's spellings, and the premise that basalt uses none of these names
holds only for the crate. basalt names the triples on 7 lines in 3 files and the farm directory on
1. Its `helpers/packages.py` reads `targets/<triple>.json` and `target/nife-farm` from the nife tree
it builds against, and since milestone 1115 (the gate builds basalt's delivered programs from the
candidate tree) nife's own queue runs that script. A triple rename in nife alone therefore turns
nife's queue red.

## The order

Effort: S is under half a day for a lane, M a day or two, L more. Every step inside the two
repositories is a two-way door, since nobody outside the project builds for these names. The
outward steps are listed apart below.

1. **basalt accepts both triple spellings.** `helpers/packages.py` maps `*-unknown-moho` and
   `*-unknown-nife` to one architecture and reads whichever target specification exists, and
   `.github/workflows/packages.yml` follows. basalt's Release assets are named by architecture, so
   no published name changes. Effort: S. Lands alone, in basalt.
2. **nife's basalt pin moves to that commit.** Milestone 1116's daily pull request does this once
   basalt cuts a Release. Effort: S. Lands alone. It blocks step 5 only.
3. **The crate rename.** The directory, the package, the 26 dependency entries, every `use abi` and
   `abi::`, the 10 `Cargo.lock` files, and `COPIED_FILES`' package in `xtask/src/farm/copied.rs`
   land in one pull request, because the build breaks on any one of them alone. Three gates key on
   the path and go blind unless they change in the same commit: `helpers/architect-label-rules.py`
   rule 1, `ABI_CRATE` in `helpers/format_values_check.py`, and the module-block baseline's 104
   entries. Each entry follows its file, and the count does not move. The 27 falsification
   patches whose context names the crate are regenerated in the same pull request. Prose moves by
   status, per the rename checklist. The path is fixed everywhere, as navigation. Effort: M.
   Lands alone. `needs-architect` follows from rule 1, and open decision 1 covers its clearing.
4. **`target_os` and the `std` port.** The `os` field of the three specifications, the patcher's
   arms and `env_consts` in `xtask/src/farm/patch.rs` and `source.rs`, the overlay's directories,
   the farm's copy destinations and `TALC_IN_STD`, the stamp's overlay path, the C library's
   platform module, the vendored relibc arms, `__nife__`, the regenerated headers, and the
   `os nife` line `system_tests` expects all land in one pull request. `std` dispatches on the name, so
   a half-done change fails to build `std`. Effort: M. Lands alone, after step 3.
5. **The triples.** The three specification files, the stamp's three paths, every `--target` and
   `.cargo/config.toml` that names one, xtask, helpers, the two workflows and the probe scripts in
   live notes. Effort: M. Lands alone, after steps 2 and 4.
6. **basalt drops the old spellings** and bumps its nife pin to the commit of step 5, citing it as
   `nifeos/nife@<sha>`. Effort: S. Lands alone, in basalt.

Steps 3 and 4 could share a pull request, and the plan keeps them apart. Each one alone already
touches hundreds of files. A queue removal then names one cause.

## The sweeps in flight

Five sweeps have merged: `futex` to `address_wait` (#3874), the mutex spelled out (#3875),
`syscall` to `system_call` (#3855), `virtio` to `virtual_io` (#3854), and the `user_mode_runtime`
names (#3853, #3856, #3866). Three open pull requests share files with step 3.

| pull request | shared files | why |
|---|---|---|
| #3879, riscv64 spelled consistently (100 files) | 13, among them the module-block baseline, `kernel/src/system_call.rs`, `system_tests/src/user.rs`, `xtask/src/archive.rs` and `xtask/src/disk.rs` | both edit the same lines of the baseline and of the kernel's system call entry |
| #3883, `inter_process_communication` names | 3, the baseline and two scheduler files | the baseline |
| #3884, the crate-workflows pin bump | 1, the baseline | the baseline |

The recommendation is to start step 3 after #3879 and #3883 merge, and before `abi` leaves for a
repository of its own. #3879 keeps the triples as they are, so it does not collide with step 5. The
move of `user_mode_runtime` (milestone 1112, `user_mode_runtime`: what an outside maintainer would
fix before it leaves) waits on `abi` leaving, as step 3 of milestone 1017's order requires. A
rename before any move keeps the whole change inside one repository and one queue.

## The outward steps

Each needs its own go from calef and none is part of steps 1 to 6.

- **A repository for `moho_abi`.** It follows the clearing of the crate's review package, the open
  decision of milestone 1075.
- **The crates.io publish.** Publishing is held until every outside-review milestone closes. The
  name `moho_abi` was free on crates.io on 2026-11-21.
- **An upstream rustc target.** No rustc target, and no LLVM `OSType`, says `moho` or `nife`, on
  the pinned nightly-2026-11-20 and on LLVM's main branch. The tier policy warns that a name is
  costly to change once upstream, so this is the step after which the name is one-way.
- **A getrandom or libc backend.** Neither project can match on `target_os = "moho"` before rustc
  knows the value: a crate that does draws `unexpected_cfgs` on stable rustc today (measured). So
  this step follows the upstream target. Hermit's getrandom backend calls `extern "C"` symbols and
  depends on no crate, so such a backend would not itself need `moho_abi` published. An upstream
  `std`, like Hermit's, would: `library/std/Cargo.toml` takes `hermit-abi` from crates.io with
  `rustc-dep-of-std`. Until then nife's custom getrandom backend stays.

## Prior art

Read, with sources, in [the prior-art appendix](1121-moho-rename-plan/prior-art.md). Rust renamed
`wasm32-wasi` to `wasm32-wasip1` over three releases, with a release of warnings between the new
name and the removal. Rust dropped its `hermitkernel` targets once Hermit shipped its own JSON
targets. Hermit renamed its crates.io crate from `hermit-sys` to `hermit`. Redox publishes
`redox_syscall` with the library name `syscall`. Darwin's `uname` reports the kernel while
`target_os` is `macos`. nife differs from all of them in having no outside user, so it renames in
place with no alias and no window of two names, except for basalt's one release in step 1.

## Risks

- **A stale farm.** The stamp hashes each input's name and reads its bytes when the file exists.
  A renamed overlay directory or target specification that the stamp still names by its old path
  changes the hash once, and afterwards drops out of it. Later edits then reuse a stale farm. Steps
  4 and 5 change `stamp.rs` in the same commit. They check it by editing one overlay file after the
  rename and watching `cargo xtask std-src` rebuild.
- **Every worktree rebuilds its farm.** The stamp changes in steps 3, 4 and 5, so each lane's next
  build rebuilds `std`, at minutes a rebuild. That is cost, not breakage.
- **The pinned toolchain.** The farm patches the pinned nightly's `std` source. A toolchain bump
  that lands between steps 3 and 5 rebases the patcher onto new source, and the two must not share
  a queue window. The toolchain workflows (`toolchain-bump.yml`, `toolchain-drift.yml`) name
  `std-nife` and change in step 4.
- **The getrandom backend.** It stays the custom one (`getrandom_backend="custom"`) until the
  outward steps above, so the rename does not touch it.
- **Lanes in flight.** A lane cut before step 3 that names `abi::` breaks on its merge of main. The
  fix is mechanical, and the step's pull request body gives the sed line.

## Open decisions

### 1. How the crate rename's `needs-architect` is cleared

Background. Step 3 moves `crates/abi`, so the label rule for the binary interface's surface puts
`needs-architect` on it, and only calef clears that label. The pull request changes no number,
no layout and no method: the format-values gate pairs the old and new paths through git's rename
detection and finds every pinned number unchanged. Its diff is still over 450 files.

Recommendation: A.

- A. calef clears the label against this plan, the gate's unchanged-numbers verdict and
  `git status` showing the crate as renamed, not added.
- B. calef reads the whole diff before clearing.
- C. The label rule learns to pass a pure rename, which is a change to a gate of its own.

Door: two-way. The rename stays inside the tree, and a later pull request can undo it.

Prior art. The `syscall` to `system_call` sweep (#3855) touched the kernel's entry and merged
through the same queue.

Blocks: step 3.

### 2. What `uname` reports

Background. The C library answers `uname` with the system name `nife`. Programs that branch on it
are rare, but a shell prompt or a bug report prints it. `std::env::consts::OS` follows `target_os`
by `std`'s own definition, so it reads `moho` after step 4, whatever `uname` says.

Recommendation: A.

- A. `uname` keeps reporting the kernel, `nife`. A Moho program on a second capability kernel then
  reports that kernel.
- B. `uname` reports the binary interface, `moho`.

Door: two-way now, and one-way once programs outside the project read it.

Prior art. On macOS, `uname -s` prints `Darwin`, the kernel, while Rust's `target_os` is `macos` (recalled, not read).
Hermit and Redox use one name for the kernel and the interface, so they offer no split.

Blocks: step 4 only in which line the C library's platform module keeps.

## Follow-on

- **Outstanding.** Steps 1 to 6, each a lane of its own, in the order above.
- **Outstanding.** The outward steps, each after its own go.
- **Outstanding.** A ruling on `.nifepkg`, if the distribution's format is to follow the interface.

## BUGS

- The census counts lines, not occurrences, and the prose split by status counts each file once at
  its own status. A block that changes status before step 3 shifts the split, not the total.
- The rename checklist's rule for audit reports keeps 49 lines in 15 files that name the old path,
  and step 3 fixes their paths only. A reader may meet `abi::` there after the rename.
- Effort figures are estimates from the census, not measured lane times.

## Index row

The capability binary interface took the name Moho, and this plan sets the order in which nife and
basalt spell it. basalt first learns both triples, so nife's own queue stays green. The crate, then
`target_os` with the `std` port, then the triples each land in one pull request of their own, after
the riscv64 and inter-process communication sweeps. The publish, the upstream target and any getrandom backend each wait
for a separate go.

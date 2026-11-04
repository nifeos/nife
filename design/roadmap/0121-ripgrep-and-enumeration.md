---
status: BUILT
raised: 2026-08-13
built: 2026-10-19
milestone_dependencies: 205, 206, 595, 809, 810
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 121. `ripgrep` on nife: enumeration as a capability, and what the walk costs

Minted 2026-08-13 by calef.

Checked 2026-10-07 (UTC) by lane `lane/121-ripgrep`. Both blockers the 2026-09-26 check named are
BUILT. Milestone 205 (how a foreign program is told what to do) gives a `std` program an argv
through `grant_plan::argv`. Milestone 206 (a program image has under 896 KiB) moved the ceiling to
496 MiB, so `rg` loads unrelinked. With them, `rg` does the three things this block calls the
point, on all three architectures, through the kernel harness (notes/ripgrep-on-nife.md):

- A real search. `ripgrep_tests::ripgrep_searches_the_tree_it_was_granted` tells `rg` the line
  `rg --threads 1 --no-mmap --stats 'walk entry'` over the priced tree, granted
  `ENUMERATE | READ | DESCEND` behind a caretaker. Its own `--stats` say 138 matches in 141 files
  and 333,984 bytes searched, which the test asserts against the fixture's constants. The 3,813
  bytes it prints are byte for byte what the same `rg` prints on macOS.
- The refusal. `ripgrep_without_enumerate_is_refused_rather_than_empty` withholds `ENUMERATE`. `rg`
  prints "this directory capability does not carry the right that verb needs" for `./`, searches
  zero files and prints no match. The control, `ripgrep_reaches_a_named_file_without_enumerate`,
  still finds the line in a file it is named under the same grant.
- The measurement. `rg_search` in `script/bench --real --release --smp` against
  `bench/host/run_linux_rg.sh` on the same `virt,accel=hvf` machine: 1.24 to 1.33 ms on nife against
  0.82 to 0.87 ms on Linux by ripgrep's own clock, about 1.5x. The caveats are in the note.

Two questions this block left open are answered by measurement. The 256-page heap suffices: every
run retyped 65 pages of it. `--no-mmap` need not be a default: the control runs without it, on a
single named file, which is where `ripgrep` reaches for a map, and the stub fails over to reads.

**Built 2026-10-19 (UTC)** (lane `milestone/121-rg-via-jig`): `rg pattern dir` at the swish
prompt, with `rg` installed by `jig` and never part of the base image (calef, 2026-10-07, on
#1797's fork 4: "With a functional jig, ripgrep should be installed via jig and not part of the
base image. It isn't base."). `script/swish-check`'s second boot types `jig install ripgrep`, which
fetches basalt's package by the index's digest over TLS (milestone 810), then in `/search`, holding
a copy of `crates/`, `caps rg BUGS crates` and `rg BUGS crates` by the bare name. `caps` prints the
whole grant: `/packages/ripgrep/14.1.1/rg` gets `crates` alone, read-only, which is `ENUMERATE |
READ | DESCEND`, and nothing else on the disk. The walk's answer must equal, line for line, what
the host's `rg` 14.1.1 prints over the same copy; on 2026-10-19 that was 322 lines from 177 files,
matched on all three architectures. A boot that types the walk and whose answer is missing fails.
notes/ripgrep-on-nife.md has the faults the walk exposed and the timings.

Re-checked in CI from 2026-10-08 (UTC) (lane `milestone/121-rg-ci`, #1853), the first gap of
calef's amber ruling on fatal risk 1. `script/ci-build`'s `swish-check` rows fetch basalt's `rg`
first, so the walk runs on every pull request, and in CI a missing `rg` fails the leg. Why
basalt's: calef's ruling in milestone 810 (`ripgrep` is packaged in basalt and installed with `jig`).

Threaded 2026-10-12 (UTC) (lane `milestone/121-rg-threads`, risk 1's third gap): at `-smp 4` on
all three, the kernel counted five threads in `rg`'s process, and CI re-checks it
(notes/ripgrep-on-nife.md).

Built 2026-08-31 (lane `milestone/121-ripgrep`; notes/ripgrep-on-nife.md). Unmodified
`ripgrep` 14.1.1 builds for all three `*-unknown-nife` triples with zero source changes, loads,
resolves its own directory through a granted capability, and stops at its own "requires at least one
pattern", because `std::env::args()` yields nothing. It never reaches DECISIONS §105 (threads):
`available_parallelism()` answers 1 and it picks its serial walker. Since milestone 303 (a disk the
x86_64 FS service can find) the 62-byte transcript is identical on all three architectures.

Built 2026-09-26, without `rg` (the same lane, second pass; notes/walk-pricing.md). The two
parts of this block that do not need an argument vector, on all three architectures:

- The refusal. `ripgrep_tests::a_walk_without_enumerate_is_refused_rather_than_empty` grants
  `std_exerciser` the priced tree with `READ | DESCEND` behind a caretaker. `read_dir` is refused at
  the root and one level down, a recursive walker in `walkdir`'s shape returns `PermissionDenied`
  rather than zero entries, and a file named two levels down still opens (the control).
- The price. `ripgrep_tests::a_walk_through_a_confined_grant_is_priced` grants the same tree
  with `ENUMERATE | READ | DESCEND`, asserts the walk visits exactly the fixture's 153 entries and
  366 components, and prints the walk split per component, per entry and per KiB. Under HVF with a
  debug kernel: about 42 ms for the whole walk, 15 to 51 us per component, 43 to 85 us per entry
  listed, against 1.85 ms on macOS natively. Path components are not where the time goes; listing
  is, because the server reads every child's node to set `IS_DIR` (recorded in
  `redoxfs_server`'s `read_dir` BUGS).

The walker is `crates/walk_pricing` (provisional name), plain `std::fs` code that runs unchanged on
a host, so the nife walk and the host walk are the same function. The harness gained
`fs_service::start_std_narrowed` (provisional), a `std` program behind a caretaker.

A third "somebody else's real application"
target beside milestone 66's Vaultwarden and milestone 99's git, chosen for a reason neither of those
has: it is the workload that pushes on enumeration, which is the one authority this system treats
as dangerous.

## Why this workload rather than another

Enumeration is authority, and nothing in the tree currently walks. Milestone 40 (documentation as
a system service) met this a week
ago and wrote it down: its search index is built on the host because there is no `readdir` for a
viewer to call, and adding one "would hand a viewer the power to discover what it was not given". A
program that can list a directory can learn what exists, which is a different and larger power than
being able to read a file it was handed.

That is not a wall, it is a right. `ENUMERATE` is one of §47's six directory rights, it is
implemented in `crates/fs_proto`, and its absence answers `EPERM` rather than an empty listing, which
is milestone 42's truthfulness rule applied to the one refusal most easily faked. `rm -r` already
composes it (`REMOVE_TREE = ENUMERATE | DESCEND | REMOVE`).

This block said `rm -r` was the only thing composing `ENUMERATE`, and that was already false when it
was minted (corrected 2026-08-17 by the status-accuracy sweep; the status itself is right and did not
move). At least four other things compose it, and one of them predates this block by ten days. They
are the shell's glob expander (`expand` in `components/src/swish/navigation.rs`, driving
`crates/grant_plan/src/expand.rs`, landed
2026-08-03 against this block's 2026-08-13), the SMB server
(`descend_rights` in `user/src/smb_server.rs`), the `std` PAL's `read_dir`
(`readdir` in `patches/std-nife/overlay/std/src/sys/fs/nife/path_operations.rs`), and the survey viewer's kernel-level
`Rights::ENUMERATE` (`spawn_service` in `crates/system_initializer/src/spawn_loop.rs`). Things walk, too:
`descend` in `user/src/smb_server.rs` walks to a directory one `OPENDIR` per component, `remove` in `components/src/rm.rs` is
a real recursive stack walk, and `swish` walks components to navigate.

The honest claim is narrower, and it is the one this milestone actually rests on: no recursive walk
is reachable from a `std` program, because the PAL retains no directory handle. That is milestone 122,
this block's gate (the correction below).
Nothing composes `ENUMERATE` to walk a tree it was handed, from somebody else's program, and that
is where the interesting costs and the interesting confinement both live.

## What is already in place, measured rather than assumed

| Piece | State | Where |
|---|---|---|
| the `ENUMERATE` right, and `EPERM` when withheld | built | §47, `crates/fs_proto` |
| rights that cannot be widened by a child | built | §47 |
| `read_dir` bound in the `std` PAL, one level only | built, see the correction below | milestone 64 (enough `std` to run somebody else's crate) |
| `regex`, `walkdir`, `ignore`, `crossbeam-channel`, `memchr` | built with no change | milestone 64's probe of 50 crates |

That last row is the surprising one. Every significant dependency in ripgrep's tree is in 64's
"Built with no change (35)" list, including `ignore`, which is the crate that does the gitignore
parsing and the directory walking and which 64's table annotates "fs plus threads". So this milestone
is closer to a port than to a construction, which is what §46's boundary predicts: the reuse boundary
is the TCB boundary, and userspace should actively prefer porting.

### Correction (2026-08-13), since resolved

This section once said a recursive walk could not work: the `std` PAL held no directory handle, so
a program could list `sub` and not open `sub/name`. `walkdir` and `ignore` compiled and could not
walk. Milestone 122 (a directory handle `std` can hold) closed that on 2026-08-18 on top of
`OPENDIR`, which `fs_proto` already had. The lesson stays: "built with no change" meant compiles,
and only running it found the gap.

## The demonstration

`rg pattern src/`, where the process holds a directory capability over `src/` carrying
`ENUMERATE | READ | DESCEND` and cannot see, name, or read anything outside it. Not by policy and
not by a check the program performs on itself, but because the authority it holds does not reach.
Milestone 48's shell already rebinds what it holds, so the grant is the shell's ordinary act rather
than a special case built for this.

The negative half is the load-bearing one and should be a test, in the shape milestone 108 used: the
same command, run against a directory capability that lacks `ENUMERATE`, must be refused loudly
rather than returning nothing. A search that silently finds no matches because it could not look is
the worst possible failure for this tool, which is why `fs_proto` answers `EPERM`.

This is a better first demonstration than git for one reason worth stating plainly: everybody has
run grep. The confinement claim needs no explanation to anyone who has ever typed a search.

## Exit

1. `rg` searches a tree through a directory capability and finds exactly what is there. Done
   2026-10-07, all three architectures, kernel harness.
2. The same `rg` through a capability lacking `ENUMERATE` is refused loudly, never empty. Done
   2026-10-07, with a named-file control.
3. The search is measured against Linux with `--threads 1 --no-mmap` on both sides. Done
   2026-10-07, aarch64 under HVF (the one tier where both run on matched hardware).
4. `rg pattern dir` at the swish prompt, holding `ENUMERATE | READ | DESCEND` over `dir` and nothing
   else, with `rg` installed by `jig` and never in the base image. Done 2026-10-19, all three
   architectures, `script/swish-check`'s second boot.

## The benchmark, which is the part worth the lane

Every `read_dir` is IPC to the filesystem server. A recursive walk is therefore the workload that
most directly prices this system's central architectural bet, and the roadmap does not currently have
that number. ripgrep is a good instrument for it because it is famous for speed, its performance is
publicly compared against other tools, and the comparison against Linux runs the same Rust logic on
both sides rather than two different programs.

What to measure: wall time and instruction count over a fixed tree. Separate the per-entry cost of
the walk from the per-byte cost of the search, because those two exercise different halves of
the system and a single number hides which one is expensive.

The caveat: `rg` walks in parallel since milestone 812, so both sides are pinned to `--threads 1`.

An honest loss is a result. §14's framing is that a recorded loss is worth more than an overclaimed
win, and a microkernel paying IPC per directory entry may well lose this one. That is worth knowing
precisely, and it is the input to whether the FS contract wants a batched or streaming listing.

## Prior art

The three questions, against `notes/prior-art.md`'s ecosystems.

Code to use: ripgrep and its dependency tree, unmodified. This is the case §46 (thin primitives or whole subsystems) rule 2 and the
prior-art note's TCB boundary both point at, and 64 already measured that the tree builds.

A design to copy: Fuchsia is the closest existing system, because its directory handles carry
rights and enumeration is one of them rather than an ambient consequence of having a path. Worth
reading for how it handles rights reduction when a handle is re-opened, which is the same question
§47 answers with "a child can never exceed its parent".

A mistake to avoid: the general shape of a Unix `readdir`, where the ability to list follows from
being able to name the directory at all. Redox is the neighbor that keeps Unix ergonomics, and the
thing to take from it is the ergonomics without the ambient reach. Getting this wrong looks like
ripgrep working beautifully and confinement being decorative.

## BUGS

- `ReadDir` reads a listing whole rather than streaming it, which milestone 64's PAL documents as
  a deliberate choice. A directory with very many entries therefore costs memory proportional to its
  size, and a recursive walk meets many directories. Unmeasured, and this milestone is what would
  measure it.
- Threaded since milestone 812 (`std::thread::spawn` runs real threads in one address space). A
  published number states its thread count; a comparison pins both sides to it. No threaded
  comparison with Linux exists: `rg_search` and `bench/host/run_linux_rg.sh` both pass
  `--threads 1`.
- ripgrep memory-maps large files by default and this system has no `mmap`, which milestone 99 (`git` on
  nife)'s block also names as a gap. Measured 2026-10-07: it costs nothing to correctness, since
  `memmap2`'s stub fails and `ripgrep` reads instead, so `--no-mmap` is a measurement pin and not a
  requirement. What a map would have bought on a large file is unmeasured, because there is none.
- The benchmark measures this tree, not a class of systems: one microkernel's IPC cost is not
  "microkernels are slow at walks".
- `ignore` building is not `ignore` behaving. 64's probe proved it compiles. Whether its
  metadata-heavy paths and gitignore semantics behave identically here is a separate question that
  only running it answers, as the correction above found the sharp way. Since 2026-10-07 it has run: the walk reaches every file in the fixture and nothing else. The fixture
  holds no ignore files, so gitignore semantics are still untested here.
- A walker's cost is per component, and on 2026-09-26 it stopped being invisible: 15 to 51 us
  per component under HVF with a debug kernel, smaller than a single operation's fixed cost and
  than a listed entry's (notes/walk-pricing.md). `ripgrep` on Linux pays neither.
- The Linux `rg` the search is measured against is not upstream's musl build: it drops `jemalloc`,
  which needs a C cross-compiler this machine lacks, and uses musl's malloc. Upstream added
  `jemalloc` because musl's allocator is slow for `ripgrep`, so the Linux figure is if anything
  pessimistic. `bench/host/run_linux_rg.sh` says so where it does it.
- CI's `test` job fetches no `rg`, so the kernel suite's four `rg` tests skip there; the
  `rg-threads` rows run them.
- `ignore`'s parallel walker carries one patch, basalt's `patches/ripgrep-ignore-walk.patch`, which
  nife maintains and re-checks at every `rg` or `ignore` bump. No upstream issue is filed for it.
- Spawning `rg` at the prompt takes up to 39 s under load, the walk up to 432 s on `x86_64` under
  TCG, and neither is explained; its peak memory there is unmeasured (notes/ripgrep-on-nife.md).

## Follow-on

- **Recorded.** The patch nifeos/basalt carries for `ignore`'s parallel walker, in BUGS.
- **Milestone 205.** The ABI has no argument vector, which is what stops `rg` after it loads and
  resolves its own directory. `design/roadmap/0205-foreign-program-arguments.md` was minted from
  this lane on 2026-08-31 and carries the wire-format fork.
- **Milestone 206.** The 896 KiB image ceiling this lane found became
  `design/roadmap/0206-user-image-ceiling.md`, which also owns the mapping error that names an
  overlap rather than a size.
- **Done.** 2026-10-19: exit clause 4, `rg BUGS crates` by bare name after `jig install ripgrep`,
  in `script/swish-check`'s second boot.
- **Recorded.** A threaded comparison with Linux, in BUGS. Checked 2026-10-12: `rg_search` and `bench/host/run_linux_rg.sh` still run `RG_SEARCH`, which passes `--threads 1`.
- **Done.** 2026-10-07: `rg` searches the priced tree and its counts match the fixture, on all
  three architectures (`ripgrep_tests::ripgrep_searches_the_tree_it_was_granted`).
- **Done.** 2026-10-07: the same refusal with `rg` as the walker, and a named-file control
  (`ripgrep_without_enumerate_is_refused_rather_than_empty`,
  `ripgrep_reaches_a_named_file_without_enumerate`).
- **Done.** 2026-10-07: `rg --threads 1 --no-mmap` against Linux on the same machine, `rg_search`
  and `bench/host/run_linux_rg.sh`; numbers and caveats in notes/ripgrep-on-nife.md.
- **Done.** 2026-09-26: the walk priced, per component, per entry and per KiB: notes/walk-pricing.md.
- **Recorded.** The walk pricing's own gaps (a debug kernel, no matched-tier Linux run, three samples
  per slope) are in notes/walk-pricing.md's BUGS. The per-entry listing cost is in `redoxfs_server`'s
  `read_dir` BUGS.
- **Milestone 303.** The x86_64 run, done 2026-09-16. Milestone 184 closed the `std` half of this
  gap on 2026-09-14 and `ripgrep` built there; 303 gave the FS service a disk it can find on `q35`,
  and the transcript is the same 62 bytes as the other two architectures.
- **Recorded.** Directory reading still reads a listing whole rather than streaming it, and the
  memory cost of a deep walk over large directories is unmeasured. Checked 2026-09-03 against the
  filesystem shim under `patches/std-nife/overlay/std/src/sys/`.

## Index row

Enumeration is authority: `ENUMERATE` is a §47 (a directory capability carries six rights) right,
and `rg pattern src/` that provably cannot see outside its grant is the confinement claim anyone who
has typed a search understands. Unmodified `ripgrep` 14.1.1 searches a granted tree on all three
architectures with zero source changes, finds exactly the fixture's 138 matches in 141 files, and
through a grant lacking `ENUMERATE` says so and searches nothing. On the same HVF machine the search
takes 1.24 to 1.33 ms against Linux's 0.82 to 0.87 ms, single-threaded on both. At the swish
prompt it walks a copy of `crates/`, 661 files in 275 directories, and prints exactly the host's
349 matching lines on all three. There `rg` is the one `jig` installed from basalt's index, never part of the base image.

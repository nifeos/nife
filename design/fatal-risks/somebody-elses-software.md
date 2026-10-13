---
risk: 1
color: green
updated: 2026-10-12
---
# Appendix to risk 1: Only software written for nife runs on nife

*An appendix to [`1-only-software-written-for-nife.md`](1-only-software-written-for-nife.md) (risk 1). That entry is the claim of
record, and it is written so that a reader can decide what to work on next without opening this
file. This one exists to be verified or challenged: it holds the evidence, the dates, the numbers,
the corrections and the refusals behind the verdict, at the length they need rather than the length
the six-pager has. Where a study has its own home in `notes/` this page links it rather than copying
it. Name provisional (`design/fatal-risks/` and this file's stem); naming is an architect's.*

### The claim, stated so it can fail

The platform can run hand-written Rust and nothing else, so every piece of software anyone wants has
to be rewritten.

### Why it is the most dangerous one

It is structural. Optimization cannot fix "nothing runs here", and no amount of kernel work changes
it. A system in this state is a research demonstrator forever, which is a legitimate thing to be and
is not what DECISIONS §14 (a verified-Rust capability microkernel that runs real workloads) claims.

### Evidence today, both directions

For: milestone 27 (Rust `std` on the native ABI) works, and milestone 64 sorted crates.io by build
status. `kilo` runs. Against: DECISIONS §105 (`std::thread::spawn` stays declined) means `rayon`,
`tokio` and `crossbeam` compile and link but cannot spawn; `std::process` refuses everything; there
is no `fork`, no POSIX, no libc tier three.

### The experiment

Milestone 121 (`ripgrep`: enumeration as a capability), chosen because `ripgrep` has a real
dependency tree, walks a filesystem, and uses threads.

### The verdict of record

RUN, 2026-08-31. AMBER since 2026-10-06, because `ripgrep` has not yet searched (the correction
below); GREEN on all three architectures from 2026-09-16. The blocker was not what anyone predicted. notes/ripgrep-on-nife.md has it; PR #600 for the first two, milestone 303 for
x86_64.

- Unmodified `ripgrep` 14.1.1 from crates.io, forty transitive crates, builds for
  `aarch64-unknown-nife`, `riscv64-unknown-nife` and `x86_64-unknown-nife` with zero source changes,
  loads, runs, resolves its working directory through a granted directory capability, and exits
  through `std::process::exit`. Zero patches, and the three transcripts are byte for byte identical
  from three separately built binaries. Everything that differs from a Linux build is on the command
  line.
  Correction, 2026-10-10 (UTC): since milestone 812 (`std::thread::spawn` runs real threads in one
  address space) `rg` walks in parallel, and its `ignore` dependency carries one ten-line
  build-time patch for that walker (calef, #1892). `ripgrep`'s own source is still untouched, and
  the patch is nife's to maintain until upstream fixes the bug
  ([`notes/ripgrep-on-nife.md`](../../notes/ripgrep-on-nife.md)).
- x86_64 took two more milestones and the second was a disk. Milestone 184 (extend the `std` port to
  x86_64) made `std_exerciser` pass there on 2026-09-14 and `ripgrep` build. And a build is not a
  transcript: the run needed a RedoxFS disk the FS service could find, which `q35` could not offer
  because the lookup walked the virtio-mmio bus that machine does not have. Milestone 303 (x86_64's
  FS service has a server and no disk it can find) closed that on 2026-09-16, and the transcript is
  the same 62 bytes. So the honest sentence is now *unmodified third-party software runs on nife*,
  with no architecture qualifier, which is what DECISIONS §19 (architectural parity is a tenet) asks
  before the qualifier comes off. `notes/ripgrep-on-nife.md` has the parity table.
- What stops it is that the ABI has no argument vector. `std::env::args()` compiles std's
  `unsupported` backend and yields nothing, so `ripgrep` parses no arguments and prints its own
  *"requires at least one pattern to execute a search"*. Somebody else's application reached its own
  error path on this kernel, which is a far better result than the build failing.
- §105 was never reached, and that is the finding that reverses the premise. `ripgrep` does not
  assume parallelism, it asks: `available_parallelism()`, to which nife's PAL answers `Ok(1)`
  honestly, so it selects `search_serial` and never calls `thread::spawn`. A platform answering
  `Unsupported` there would have failed this program. The declined threads cost nothing here, and
  answering honestly rather than refusing is what made it work.
- The capability model is visible from inside a stranger's program. Without slot 4 the same binary
  prints `failed to get current working directory: operation not supported on this platform`.

- Correction, 2026-10-06 (UTC). The run proves that a stranger's program builds, loads,
  allocates, finds its directory and exits cleanly, and nothing after argument parsing.
  `ripgrep_tests` still passes no arguments though milestone 205 (how a foreign program is told
  what to do) is BUILT, so the filesystem walk and the threads, two of the three reasons milestone
  121 chose `ripgrep`, have not been exercised by `ripgrep`. The walk was priced by
  `crates/walk_pricing`, an in-tree `std` walker. Milestone 64's survey of crates.io established
  that crates compile, not that they run. Nothing else foreign has run: milestone 66 (Vaultwarden)
  and milestone 99 (git) are NOT-STARTED. And a search may not fit: PR #1777 measured `rg
  --threads 1 --no-mmap` over this tree at 3.0 MiB peak on macOS, about three times a `std`
  program's heap.

### What it changes

The structural fear behind this risk is retired on every architecture this kernel supports: this
system runs software it did not write, unmodified, with a real dependency tree. What remains is an
ABI gap with a name, which is a design question rather than a wall. The parity gap that qualified
this paragraph narrowed on 2026-09-14 from a missing port to a missing disk, and closed on
2026-09-16 when the disk arrived.

This qualifier was written on 2026-08-31 and did not land for two weeks. calef caught the first
draft omitting the architecture the day the result came in. The correction was committed to a
maintainer branch that was never pushed. And survived only because a branch cleanup on 2026-09-14
checked for unique commits before deleting. By then half of it was stale (it said riscv64 was
unbuilt, and riscv64 had been run), which is why it was rewritten here rather than merged. AGENTS.md
already names the failure: *nobody reads branches.*

2026-09-19: milestone 64 (enough `std` to run somebody else's crate) turned BUILT, and it moves this
risk very little. Its last pass bound file times by path (`Metadata::modified` on `GETMTIME`,
`std::fs::set_times` on `SETMTIME_AT`), which was the one item its block still called outstanding.
Nothing above depended on it: `ripgrep` stops at the missing argument vector of milestone 205 (how a
foreign program is told what to do), not at `std::fs`. What it adds is one more std surface that
answers rather than refuses, with a caveat a stranger's program can trip over. A file written on
nife reports an mtime in early 1970, because the FS server stamps a per-mount counter (notes/std.md;
proposed as design/roadmap/0497-a-filesystem-server-that-knows-the-time.md). Written by the milestone
64 lane, which does not normally edit this file; the status check requires the entry to know.

The same published argument that sharpens risk 8 sharpens this one, and it is the same paper: Li et
al.'s case for an incremental path rests on clean-slate kernels having *"significantly fewer
features than Linux ... Impeding adoption"*, which is this entry's claim written by people who do
not work here. `notes/incremental-path.md` has it and the answer.

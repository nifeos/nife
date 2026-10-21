# Measured for the rulings: (d), (g) and (h)

Appendix to [784. Reuse what the tree wrote](../0784-reuse-what-the-tree-wrote.md).

Items (d), (g) and (h) each wait on a §46 (thin primitives or whole subsystems) ruling, and each
ruling follows data. This page is that data, measured on 2026-10-21 (UTC) at base `810666a39` on
patagonia (Apple M3) with the pinned nightly of 2026-10-20. Each section ends with options and a
recommendation. No dependency was added: every probe is generated outside the tree by
[probe.sh](probe.sh), and none of the crates below is in `Cargo.lock`.

Item (c) is not measured here. Its row is a wait on a release (c0), and crates.io still lists
`virtio-drivers` 0.13.0 as the latest. Item (b) is held for milestone 705 (full-screen programs).

## (d) `user_mode_heap` against `talc` 5.1.1

### What each one is

`user_mode_heap` is a first-fit, address-sorted free list with coalescing. It keeps no header on
an allocated block: `dealloc` gets the `Layout` back and recomputes the size on a 16-byte grid.
Allocation walks the free list, so its cost grows with the number of free blocks.

`talc` 5.1.1 is a dlmalloc-style allocator with boundary tags and size-class bins, close to TLSF.
Each allocated chunk carries a one-word tag above it. Its `deallocate` also takes the `Layout` and
finds the tag from `ptr + size`, so `allocation_cache`'s rule still holds: a block is freed under
its class's layout, which the std PAL already does (`class_layout`).

### Throughput and fragmentation

The benchmark in [probe.sh](probe.sh) gives each allocator the same 256 MiB arena and the
same operation stream, 11 runs each. Figures are nanoseconds per operation (an allocation, a free
or a grow), median with the range. Peak extent over live is the highest byte touched over the most
bytes ever live at once: 1.00 would be no fragmentation and no per-chunk overhead.

| case | allocator | median ns/op | range | extent / live |
|---|---|---|---|---|
| a 48-byte box, alloc then free, 200,000 times | ours | 8.9 | 7.0 to 11.1 | n/a |
| | talc | 11.4 | 10.0 to 14.3 | n/a |
| 1,000 vectors doubled from 16 B to 64 KiB | ours | 28.2 | 28.0 to 32.3 | 1.14 |
| | talc | 11.6 | 11.0 to 12.6 | 1.99 |
| random churn, about 500 live, 2M ops | ours | 137.3 | 136.9 to 138.3 | 1.17 |
| | talc | 15.7 | 15.5 to 15.8 | 1.31 |
| random churn, about 5,000 live, 2M ops | ours | 738.3 | 731.2 to 751.8 | 1.12 |
| | talc | 14.1 | 13.9 to 14.3 | 1.27 |
| random churn, about 25,000 live, 1M ops | ours | 6,474.5 | 6,374.9 to 7,396.9 | 1.17 |
| | talc | 13.4 | 13.1 to 15.1 | 1.30 |

The churn's sizes are 70% under 128 bytes, 25% up to 1 KiB, 4% up to 16 KiB and 1% up to 128 KiB,
with one request in twenty 64-aligned. Frees pick a random live object.

What the numbers say:

- `talc`'s cost is flat. Ours grows with the free list: 9 times slower at 500 live objects, 52 times
  at 5,000 and 483 times at 25,000. An earlier run of the same code, under heavier load, gave 7, 65
  and 545.
- Ours wins the 48-byte box, where the block it wants is always the list's head. In std that case
  rarely reaches either heap, because each thread's `allocation_cache` serves blocks up to 256 bytes.
- Ours uses less memory. Its peak extent is 10% to 12% below `talc`'s on churn, because it keeps
  no tag on allocated blocks. `talc` grows in place, which is why it wins the vector case on time.
  Its 1.99 there is the old copies left behind when a neighbor blocked the grow.

What this leaves out. These are host figures, without the PAL's spinlock, the thread cache or the
kernel's page mapping. The machine was shared with other lanes, at a load average of 8 to 14
during this run. Nothing here measures how many free blocks a real
nife program holds. `job_mix` and `std_heap_contention` would put the allocators on target, and
doing that needs `talc` in the tree, which is what the ruling decides.

### Code size

The probe puts `alloc`, `free` and add-a-region behind `extern "C"` in a `no_std` static library for
`aarch64-unknown-none-softfloat`, with LTO, and sums the text symbols those reach.

| | ours | `talc` |
|---|---|---|
| alloc, free, add a region | 484 bytes | 1,944 bytes |
| with grow in place | n/a | 2,364 bytes |
| heap object | 16 bytes | 32 bytes |
| first-heap metadata | none | 1,575 bytes of bins |

Every process carries one copy, so `talc` costs about 1.5 KiB of text and 1.5 KiB of heap more.

### What would be lost, and what proves each

- Kani proves nothing in `user_mode_heap` today, so no proof is lost. It has 8 host tests and the
  weekly Miri run, which found two undefined-behavior bugs in it (milestone 79 (Miri over the host crates)).
- `talc` brings its own checks: its CI runs Miri on two targets, and its repository has `cargo
  fuzz` targets *(read from SFBdragon/talc's justfile and tree, 2026-10-21)*. No RustSec advisory, checked the same day.
- New crates in the graph: `talc`, `allocator-api2`, `lock_api` and `scopeguard`.
- The headerless property goes. Each allocation costs one more word.

### The cost the earlier rows did not name: std embeds the heap

`cargo xtask std-src` copies `crates/user_mode_heap/src/lib.rs` verbatim into std as
`sys/alloc/nife/user_mode_heap.rs`, because std cannot depend on a crates.io crate. `talc` has no
`rustc-dep-of-std` feature, and neither do its two dependencies. The core it would need, `base/`,
`node.rs`, `ptr_utils.rs` and the `Source` trait, uses neither dependency outside docs and tests.
So the generator could copy about 3,360 lines in several files and rewrite their `crate::` paths,
without patching the algorithm. That is still vendoring under §46, and every `talc` release would
be a regeneration and a review.

std programs are where the large heaps are (`ripgrep`, `mdr`), so taking `talc` for the `no_std`
programs alone would leave the slow case where it matters.

### Decision (d), and the options

- **A. Take `talc` 5.1.1 everywhere.** An ordinary dependency of `user_mode_runtime`, and
  generated into std from its `base` module. `user_mode_heap` goes. Faster at every live count
  above a few hundred, 11% to 13% more memory, about 3 KiB more per process, four new crates and a
  vendored copy.
- **B. Keep `user_mode_heap` and fix its search.** Writing: size-segregated free lists in front of
  the address-sorted one, kept headerless. The `Reuse:` reason would be that std embeds the
  allocator and a dependency there is a vendored fork.
- **C. `talc` for `no_std` programs, ours in std.** Two allocators, and the fast one where heaps
  are small. Listed to be refused.

Recommendation: A. The 483-times figure decides it: ours fails on a workload that is ordinary
for a server, and `talc` is maintained, Miri-checked and fuzzed. The memory and size costs are
measured and small. B would be choosing to write what exists, and I would not choose it at equal
effort. The vendoring is a real cost, and the ruling should say whether it is acceptable here.

## (g) `portable_executable` against `object` 0.40.0

### What `object` would replace

`portable_executable` has 386 code lines outside its tests. By section:

| part | code lines | with `object` |
|---|---|---|
| reading the ELF header, program headers, dynamic section | 118 | `object::read::elf` replaces most of it |
| the image and the relocations applied into it | 63 | stays: converter policy |
| one section per `PT_LOAD` | 39 | stays |
| writing the PE headers and `.reloc` | 134 | `object::write::pe::Writer` replaces most of it |

At most about 250 lines retire; the 102 lines of policy stay. That is an estimate by section, not
a prototype.

### What `object` costs

With the narrowest features a converter needs (`read_core`, `elf`, `write_core`, `pe`, no
defaults), `object` resolves to 8 crates. Two are new to `Cargo.lock`, `crc32fast` and `foldhash`,
besides `object` itself. The rest (`memchr`, `indexmap`, `hashbrown`, `equivalent`, `cfg-if`) are
already there at the same versions. `object` is 68,616 lines; `write/pe.rs` is 858 of them. It is
MIT or Apache-2.0, released 2026-08, with no advisory.

### Why the earlier survey is answered already

Milestone 821 (`portable_executable`: an ELF-to-PE converter, proven, then released on its own) holds calef's 2026-10-07
ruling on #1806: this crate is "publish ours", proven by Kani and then published. A crate a model
checker must reach in full cannot have `object` inside it. 821's own `Reuse:` line says so. No
ELF-to-PE converter exists on crates.io; `object` writes PE but converts nothing.

### Decision (g), and the options

- **A. Keep ours, and close (g) into 821.** The crate gets its proofs and its hardening there.
- **B. Take `object` in `xtask` and withdraw 821.** About 250 lines go, two crates arrive, and
  the hostile-ELF holes in `BUGS` become `object`'s parser's problem. 821's publication goes.

Recommendation: A. The only consumer is `xtask` on its own build product, so B buys little
exposure, and it reverses a ruling that has a milestone behind it. I would choose A at equal effort.

## (h) uutils for the `coreutils` programs

### Does it build for nife?

The probe builds each `uu_*` 0.12.0 crate unmodified, with a `main` that calls its `uumain`,
against this worktree's patched std for all three nife targets.

| utility | builds | aarch64 | riscv64 | x86_64 | ours on x86_64 |
|---|---|---|---|---|---|
| `printenv` | yes, all three | 376,496 | 366,880 | 415,440 | 20,656 |
| `date` | yes, all three | 1,522,304 | 1,526,864 | 1,610,232 | 25,768 |
| `wc` | no: `errno` | n/a | n/a | n/a | 20,304 |
| `rm` | no: `errno`, `console` | n/a | n/a | n/a | 21,032 |

Sizes are bytes. The `uu_*` builds are stripped, `opt-level = "z"`, with LTO. Ours are the tree's
release profile, unstripped. On aarch64 and riscv64 our four measure 82 to 101 KB, page-aligned
segments included, so x86_64 is the fairer column. Building is not running: nothing here ran a
`uu_*` program under nife.

### Why two fail

- `errno`: `uucore`'s `fs` and `pipes` features turn on `rustix`, and `rustix` needs either `libc`
  or Linux's raw syscalls. `libc` has no `nife` module (784's std section names that blocker).
  `wc` asks for `pipes` and `rm` for `fs`, so both stop there.
- `console`: `uu_rm` takes `indicatif` for its progress bar, and `console` has a terminal only for
  Unix, Windows and wasm32.

### What it would change

- Weight. `uucore` and its graph come in: 38 crates besides itself for `printenv`, 84 for `date`, `clap` and
  `fluent` in every one. Our programs are each about 20 KiB on x86_64; these are 20 to 62 times that.
- Capability shape. Our `wc` reads only the byte stream on slot 1 and holds no filesystem grant;
  `wc report.txt` works because the shell opens the file. `uu_wc` opens its arguments through
  `std::fs`, so it would need the grant ours refuses.
- Packaging. §297 (software reaches a nife system as a package `jig` installs from basalt's index) ships every
  program that way. A uutils program would be
  one, and program ports wait on the packaging milestones.

### Decision (h), and the options

- **A. Keep ours in base; record uutils as a basalt package candidate.** `printenv` and `date`
  first, once ports are unblocked. `wc` and `rm` wait on a `rustix` that builds for nife.
- **B. Replace the four with uutils now.** Two cannot build. The other two cost 20 to 62 times the
  bytes and take a broader capability shape, for programs under 200 lines each.
- **C. Patch `uucore` so `wc` and `rm` build without `rustix`.** That is a fork of `uucore`, which
  §46 permits only when we must patch it.

Recommendation: A. The demo value is real, so the candidate stays recorded with these numbers. The
capability shape decides it for base: a `wc` that needs a filesystem grant is worse than ours on
the property nife exists to show. At equal effort I would still choose A.

## Rerunning

```sh
cargo xtask std-src
design/roadmap/0784-reuse-what-the-tree-wrote/probe.sh heap
design/roadmap/0784-reuse-what-the-tree-wrote/probe.sh object
design/roadmap/0784-reuse-what-the-tree-wrote/probe.sh uutils
```

Each needs the network. `WORK` names the directory the probes are generated in.

# Where an unsafe obligation is written, and where it is only implied

<!-- The writing-standards exception marked here on 2026-09-26 (UTC) was removed on 2026-10-11 (UTC)
by lane/next-ten-longest-notes. Its reason was the bold density of the whole document. The §212 (a
prose budget) split moved four sections into appendices, and the page now meets every limit. -->

Milestone 82. The tree enforces two lints over `unsafe`, and they are meant to compose:

- `clippy::undocumented_unsafe_blocks` (milestone 68) fires on an `unsafe {}` block with no
  `// SAFETY:` comment above it.
- `unsafe_op_in_unsafe_fn` fires on an unsafe operation inside an `unsafe fn` that is not wrapped in
  an explicit `unsafe {}` block.

Neither is interesting alone. An `unsafe fn` body is one implicit unsafe block, so a function with
three unsafe operations carries three separate invariants under a single signature, and the clippy
lint sees none of them because there is nothing for it to fire on. The second lint removes the
implicitness; the first then charges each resulting block for its comment. What you get is the
property this kernel wants: **every unsafe operation sits next to the written invariant that makes
it sound**, whether or not the enclosing function is unsafe.

Both are in `[workspace.lints]` in the root `Cargo.toml`, which is where lint policy lives and where
the reasoning for each is recorded.

## The survey, and the thing it found instead

The milestone was raised expecting a burn-down: 33 `unsafe fn`s, some number of bare operations
inside them, fix each with an honest SAFETY comment, then turn the lint on.

The count of violations was zero, before anything was changed. Measured by adding the lint and
running `cargo check` over each of the thirteen configurations `script/lint` builds. Those are the
host pass, the three side workspaces, the bare-metal pass, and each of the four kernel boot-mode
features on both ISAs. Every `.rs` file was touched first, so nothing was served from cache. Plus one more that
`script/lint` did not build: `-p user -p user_mode_runtime` for riscv64. The gate compiles those two packages
for aarch64 only, which is worth knowing on its own, and is still true.

(Milestone 113 added a configuration, so `script/lint` now builds fourteen: the thirteen above plus
the clippy pass with `--cfg kani`. The riscv64 `user` gap is unrelated and still open.)

The reason is the edition. Every one of the 49 packages we own is edition 2024, and
`unsafe_op_in_unsafe_fn` is **warn-by-default in that edition**, as part of
`rust_2024_compatibility`. `script/lint` runs `-D warnings`. So the rule has been a hard gate here
since the edition bump, enforced by nothing anybody wrote down.

That is easy to check rather than take on faith. Delete one `unsafe {}` wrapper inside an `unsafe
fn`, with the workspace lint line removed, and rustc says:

```
warning[E0133]: dereference of raw pointer is unsafe and requires unsafe block
   --> crates/intrusive/src/lib.rs:116:9
note: an unsafe function restricts its caller, but its body is safe by default
    = note: `#[warn(unsafe_op_in_unsafe_fn)]` (part of `#[warn(rust_2024_compatibility)]`) on by default
```

The line landed anyway, for two reasons that survive the redundancy. A reader of the lint policy can
see the rule, which was milestone 68's entire argument for putting policy in one place. And a
package at an older edition cannot escape it; the tree already contains one, `vendor/redoxfs` at
edition 2021, and any external crate pulled into the workspace arrives at whatever edition its
author picked.

## The shape of the 33

All 33 `unsafe fn`s are in `kernel/` and `crates/`. The program packages have **two**, `malloc` and
`free` in `fixtures/src/c_shim.rs`, which a search that does not allow `extern "C"` misses
([caller-obligations.md](unsafe-obligations/caller-obligations.md) has the detail). So the milestone
spec's "across `kernel/`, `crates/`, and `user/`" (then `user/`, now `components/` and `fixtures/`)
was right.

Twenty-three have at least one explicit `unsafe {}` in the body. Every one of those blocks has a
SAFETY comment, and clippy reproves it on each run, with the single exception of `inter_process_communication`'s `seed`,
which is `#[cfg(kani)]` and therefore never compiled by the gate (see BUGS below). The other
ten have no unsafe block at all, and since the lint is clean, that means their bodies
contain **no unsafe operation**:

| Site | Why it is `unsafe fn` anyway |
|---|---|
| `crates/clock_contract/src/lib.rs:178` `Clock::new` | takes a VA the caller promises is a mapped clock page |
| `crates/paging/src/lib.rs:323` `assume_no_stale_entry` | the name is the contract: the caller asserts a TLB fact |
| `crates/paging/src/lib.rs:410` `Mapper::new` | the caller promises `root` is a live table |
| `crates/user_mode_runtime/src/heap.rs:193` `GlobalAlloc::alloc` | unsafe because the trait method is |
| `kernel/src/arch/riscv64/mmu.rs:513` `activate_user` | forwards to `write_satp`, which is a safe fn |
| `kernel/src/drivers/gic.rs:149` `init` | takes two MMIO virtual addresses on trust |
| `kernel/src/drivers/ns16550.rs:55` `Ns16550::new` | takes an MMIO base on trust |
| `kernel/src/drivers/pl011.rs:88` `Pl011::new` | takes an MMIO base on trust |
| `kernel/src/drivers/plic.rs:83` `init` | takes an MMIO base and a hart context on trust |
| `kernel/src/sync.rs:263` `force_reset_ranks` | breaks lock-order bookkeeping, which is not a memory operation |

For these ten the lint composition buys nothing, and that is not a defect in them. Their
unsafety is a **contract about meaning**, not a memory operation the compiler can point at: writing
`Mapper::new` trusts that `root` heads a live table, and no operation in its body says so. The invariant lives in the `# Safety` section of the rustdoc and nowhere else, so **for a
third of the tree's `unsafe fn`s the doc comment is the only enforcement there is**. Read them
accordingly when you change one.

## BUGS: three things neither lint can reach

1. A safe fn whose SAFETY comment discharges onto "the caller". DECIDED in milestone 112 (the
SAFETY comments that bind nobody); [caller-obligations.md](unsafe-obligations/caller-obligations.md)
records what each site got and why. The
comment names an obligation the signature imposes on nobody, so any safe code may call the function
and both lints are satisfied.
Four sites in `kernel/`:

| Site | The comment's claim |
|---|---|
| `kernel/src/virtio.rs:233` `pread` | "the caller passes addresses inside a device-mapped BAR or mmio window" |
| `kernel/src/stack.rs:121` `paint` | "the caller hands us a mapped, unused stack region" (`#[cfg(test)]`, so test builds only) |
| `kernel/src/arch/aarch64/mmu.rs:843` `switch_user_root` | "the caller passes either a live `AddressSpace`'s composed value or ..." |
| `kernel/src/arch/riscv64/mmu.rs:48` `write_satp` | "the caller guarantees `satp` names a well-formed Sv39 root" |

The last is also an ISA asymmetry: aarch64's equivalent, `set_ttbr0`, **is** an `unsafe fn`, so the
same register write is a contract on one architecture and an ordinary call on the other. Not fixed
in milestone 82 (`unsafe_op_in_unsafe_fn`), deliberately. Turning these four into `unsafe fn`s puts an unsafe block (and a real
SAFETY comment) at every call site, including the context switch. That is a change to the kernel's
soundness surface, and deserves its own review rather than a ride on a lint milestone.

Not every "caller" in a SAFETY comment is this. `sched.rs`'s `ipc_call` and `user_mode_runtime`'s `cap_delete`
mean the calling *thread* and the calling *process*; `interrupts::enable` says outright that the
operation is sound and only the timing is the caller's problem. The pattern to look for is a safe
fn that would be unsound if the sentence were false.

2. `#[cfg(kani)]` code is invisible to both lints. FIXED in milestone 113 (the proofs' own unsafe
code); [kani-gate.md](unsafe-obligations/kani-gate.md) records what the gate found. `cfg(kani)` is set by the model checker and by nothing else, so
`script/lint` never compiled those modules and neither lint could fire in them. The tree has 14
`unsafe {}` blocks under `#[cfg(kani)]`, in `crates/intrusive_fifo` and `crates/inter_process_communication`. `intrusive_fifo`'s two
both carry SAFETY comments. **Eleven of `inter_process_communication`'s twelve do not**, and the gate had never said so. A
real fix is a gate rather than a pass of comments (a clippy invocation with `--cfg kani`, or
`-D warnings` on the `script/verify` build); adding the comments alone leaves nothing to stop the
next harness from skipping them.

3. Neither lint reads the comment. `undocumented_unsafe_blocks` checks that a comment exists,
not that it is true, which is why DECISIONS §61 carries a BUGS note about a generated pass that
produced a comment false at its first site. Three comments in the tree are verbatim copies of each
other ("this function's own `# Safety` contract is exactly the one this call needs; it forwards, it
does not weaken", in `console.rs`, `sync.rs`, and `aarch64/mmu.rs`). All three are true: each is a
pure forwarding call whose callee's contract is implied by the caller's. Verbatim repetition is a
signal worth checking, not a verdict.

## The gate over `cfg(kani)` (milestone 113)

Two candidates were measured. Clippy with `--cfg kani` found 13 undocumented `unsafe` sites and 13
other warnings in about a second. `-D warnings` on `script/verify` found none, because `cargo kani`
drives a rustc and `undocumented_unsafe_blocks` is a clippy lint. So `script/lint` grew a fourteenth
clippy configuration, compiled against `helpers/kani-lint-shim/`, a crate that provides the five
Kani items the harnesses use and no semantics at all. A clean pass there is not a proof;
`script/verify` still proves.

BUGS item 2's count of eleven was an undercount: the lint also fires on an `unsafe impl`, and there
were two under `#[cfg(kani)]`. All 26 warnings are fixed. The measurement, the shim and how the
eleven comments were written are in [kani-gate.md](unsafe-obligations/kani-gate.md).

## The comments that bound nobody, decided (milestone 112)

BUGS item 1's four safe functions were decided by one question: does anything close the set of
callers? `write_satp`, `switch_user_root` and `stack::paint` became `unsafe fn`s. `virtio`'s
`pread` and `pwrite` stayed safe, because they are private to one module and the compiler closes the
caller set. The test to apply to any SAFETY comment on a safe fn is whether the parameter could have
been produced without meeting it.

`script/lint` now checks that every `unsafe fn` states its contract in a `# Safety` section. Whether
a SAFETY comment binds anybody has no gate and should not get one: it is a review discipline. Taking
`pread`'s comment seriously found a real bug, a doorbell rung through `phys_to_virt(0)`, now refused.
Fourteen more sites of the same shape outside `kernel/` want a lane of their own. The decisions,
the bug, and the survey's two blind spots (passive voice and "as above") are in
[caller-obligations.md](unsafe-obligations/caller-obligations.md).

## The census, and which numbers have a direction (milestone 134)

calef asked on 2026-08-18 how much unsafe code the tree has, and whether it should be driven in a
direction. Every number here is derived by `script/lint` on every build, and none is typed.
notes/register-of-measures.md holds them all.

The count of `unsafe {}` outside `kernel/src/arch/` more than quadrupled while the density per
10,000 lines more than halved, falling at every sample. Only the density is about soundness, so the
gate holds a ratio rather than a count. The density outside `arch/` has a ceiling that only goes
down, and `unsafe impl Send`/`Sync` claims have a ceiling at the tree's exact value with no headroom.
Other unsafe-trait claims are ceilinged the same way, and so are `static mut` declarations,
which must also follow the tree down. `arch/` itself gets no direction, because
driving it down would push against rule 1.

`// SAFETY:` parity is deliberately not a gate, because clippy already enforces it per block and a
count can only disagree. The samples, each ceiling's history and reasons, and what `user/`'s share
(now `components/` and `fixtures/`) is made of are in [census.md](unsafe-obligations/census.md) and
[ceilings.md](unsafe-obligations/ceilings.md).

## Re-running the survey

```sh
# the whole gate, with the lint already in [workspace.lints.rust], and (since milestone 113) the
# fourteenth clippy configuration that compiles the proof harnesses
script/lint

# just the count, over every configuration, cache defeated
find crates kernel user xtask -name '*.rs' -exec touch {} +
cargo check --workspace --exclude kernel --exclude user --exclude user_mode_runtime --all-targets 2>&1 | grep E0133
cargo check -p kernel -p user -p user_mode_runtime --target aarch64-unknown-none-softfloat --all-targets 2>&1 | grep E0133
cargo check -p kernel -p user -p user_mode_runtime --target riscv64imac-unknown-none-elf --all-targets 2>&1 | grep E0133
```

Grep for `E0133`, not for the lint's name: rustc reports the error code and spells the lint
`unsafe-op-in-unsafe-fn` with hyphens in its trailing note, so a grep for the underscored form
finds nothing and looks exactly like a clean tree.

### EXAMPLES: finding a SAFETY comment that binds nobody

The `# Safety` check is part of `script/lint` and needs nothing:

```sh
script/lint 2>&1 | grep -A20 'unsafe fn contracts'
# unsafe fn contracts: 51 declarations (12 trait-impl methods, whose contract is the trait's),
#                      every other one has a `# Safety` section
```

To see it fail, take a section off and put it back:

```sh
# delete the `/// # Safety` line above `pub unsafe fn paint` in kernel/src/stack.rs, then:
script/lint
# lint: unsafe fn with no `# Safety` section in its rustdoc:
#   kernel/src/stack.rs:125  paint
git restore kernel/src/stack.rs
```

The judgment half has no gate, so it is a grep plus reading. This is the pattern that found the
four, with its two blind spots (a comment saying "as above", and one in the passive voice) named so
the next person does not repeat the undercount:

```sh
# Candidates: a SAFETY comment inside a fn that is not an `unsafe fn`, mentioning a caller.
# Expect ~33 hits and expect most of them to be legitimate; this is a reading list, not a verdict.
git grep -n 'SAFETY:.*caller' -- ':!vendor' ':!notes'

# The blind spots. Neither of these says "caller", and both were the real thing:
git grep -n 'SAFETY: as above'          # inherits the defect and none of the matchable text
git grep -nE 'SAFETY: (a|an|the) [a-z]' # passive voice: an obligation with nobody owing it
```

For each hit, the question is not whether it says "caller". It is could this parameter have been
produced without meeting the obligation? `sched.rs`'s `endpoint_of` takes `&Scheduler`, which only
the lock guard can produce, so its sentence restates a guarantee. `switch_user_root(u64)` took
anything at all.

## BUGS (milestone 112)

The `# Safety` check parses Rust with a regex and a brace counter. It matches an `unsafe fn`
declaration at the start of a line and tracks `impl ... for ...` blocks by nesting depth. A
declaration split across lines by `rustfmt` would be missed, and a brace inside a string literal or a
comment miscounts the depth. The tree has neither shape today and the check was verified against the
real declarations, but this is a text scanner, not a parser. The same caveat applies to `script/lint`'s
dead-code and `#[path]` checks, which are built the same way.

It cannot see a contract in the wrong place. A `# Safety` section on the enclosing `impl` block,
or in the module doc, does not count; the check wants it on the item. That is the intent (a reader
meets the function), but it means a legitimate arrangement could be flagged. Nothing in the tree is
arranged that way yet.

Nothing checks that a SAFETY comment is true, relevant, or about the operation it sits over, and
milestone 112 did not change that. `net_transport`'s `w16` carried a comment about capability
invocation over a raw store for as long as the file has existed, and every gate was green on it.
Fixing that one line does not make the next one visible.

The `# Safety` count moves with the tree and must be taken from the merged tree. 51 declarations
and 12 trait-impl methods were measured on milestone 112's branch on 2026-08-04. Two concurrent lanes
adding unsafe code would both report honest numbers that disagree, which is what the Kani harness count did.

The riscv64 `user` gap noted at the top of this file is still open. `script/lint` compiles
`user` and `user_mode_runtime` for aarch64 only, so nine of the fourteen sites in the handoff table in
[caller-obligations.md](unsafe-obligations/caller-obligations.md) are
linted on one ISA.

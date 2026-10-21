# The comments that bound nobody, decided (milestone 112)

An appendix to [notes/unsafe-obligations.md](../unsafe-obligations.md), which summarizes it. Its
BUGS item 1 is the finding this page decides. Milestone 112 (the SAFETY comments that bind nobody)
decided it.

The parent's BUGS item 1 is this milestone. Four safe functions carried a `// SAFETY:` comment that
discharged an obligation onto "the caller" while their signatures imposed it on nobody, and both
lints were satisfied throughout, because there is no `unsafe fn` and no undocumented block for
either to fire on.

Three converted, one did not, and the difference is not how strong the obligation is. It is
whether anything closes the set of callers.

| Site | Decided | Why |
|---|---|---|
| `arch/riscv64/mmu.rs` `write_satp` | `unsafe fn` | aarch64's `set_ttbr0` already was, so the same register write was a contract on one ISA and an ordinary call on the other |
| `arch/{aarch64,riscv64}/mmu.rs` `switch_user_root` | `unsafe fn` | `pub`, called cross-module from `sched.rs`, and no type can carry the obligation (below) |
| `stack.rs` `paint`, and its sibling `high_water` | `unsafe fn` | `pub` in the crate, so any kernel code could have written the pattern over an arbitrary range |
| `virtio.rs` `pread` / `pwrite` | stayed safe fns | private to one module, so the compiler closes the caller set at twenty sites in one `impl` block |

## What makes an obligation binding, which is the whole distinction

`sched.rs`'s `endpoint_of` is the contrast that settles it. Its comment says the access is
"serialized by SCHED, which every caller holds", which reads exactly like the four. It binds,
because the parameter is `&Scheduler` and the only way to obtain one is through the lock guard. The
sentence restates a fact the type already enforces.

`switch_user_root(ttbr: u64)` says something that sounds similar and enforces nothing, because any
`u64` will do. So the question to ask of a SAFETY comment on a safe fn is not "does it mention the
caller" but "could the parameter have been produced without meeting this?" When the answer is no,
the comment is documentation of a type-level guarantee. When it is yes, the comment is the only thing
there, and `unsafe fn` is what puts it in front of somebody.

`virtio::pread` is the third case, and it is why the rule is not "convert everything a type does not
guarantee". Nothing about `phys: u64` enforces the invariant, but `pread` is private and every call
site is in one `impl` block passing a field of `Transport::Pci` that `pci.rs` resolved from a mapped
BAR. A module invariant is a real way to be sound, and the compiler is what makes it one.
Converting would have put twenty `unsafe` blocks in a single file, each restating one sentence. That
is the ritual the milestone block named as the thing to avoid. And it would have made nothing
checkable: an `unsafe fn` whose contract nothing verifies is still a contract nothing verifies.

## Why a newtype does not rescue the context switch

The obvious repair for `switch_user_root` is a `#[repr(transparent)]` newtype that only
`AddressSpace::ttbr0` and `reserved_root` can mint, which would make the function honestly safe
rather than merely honestly documented. It does not work, and the reason is worth keeping:

The dangerous half of the obligation is liveness, and a `Copy` wrapper over a `u64` launders
exactly that. An `AddressSpace` can be dropped and its frames recycled while a copy of its composed
value lives on. A borrow would carry liveness, and the scheduler cannot hold one. `sched::switch`
reads the root out from under the `SCHED` lock on purpose, so the lock is released before the
context switch. A lifetime tied to the `AddressSpace` cannot survive that drop. The obligation
stays a sentence. Both call sites now carry the argument that makes it true (the incoming thread is
`Running` with `on_cpu` set before the lock drops, so nothing can reap it across the gap) rather than
a restatement of the contract.

## Two sites the survey missed, and the pattern that missed them

Milestone 82 found its four by looking for the word "caller". Two more had the identical defect and
did not use the word:

- `virtio::pwrite` says `// SAFETY: as above.` A comment by reference inherits the defect and
  none of the text a grep can match.
- `stack::high_water` says "a mapped stack region", in the passive voice. It names the obligation
  without naming anybody who owes it, which is the same defect stated in a way that reads like a
  fact.

Passive voice and comment-by-reference are the two blind spots of any text search over SAFETY
comments, and they are worth knowing before anyone trusts a count produced that way.

Two counts in the parent's survey are also wrong, from the same cause on the other side:

- "33 `unsafe fn`s" is 33 in `kernel/` and `crates/`, not in the tree. The tree had 46 before
  this milestone and 51 after it: `redoxfs_server/` holds 9, `tools/redoxfs_host/` 2, `user/src/` 2.
- "`user/src/` has none" is wrong. `fixtures/src/c_shim.rs` has two, `malloc` and `free`, and a regex
  that does not allow `extern "C"` between `unsafe` and `fn` misses both. They are the C ABI's
  contract and are correctly documented; only the count was wrong.

## The bug this found, which is the argument in one line

Taking `pread`'s comment seriously found a path that made it false. It claimed every address
reaching it was inside a device-mapped BAR. `Transport::Pci`'s `notify_addr[q]` is zero until
`setup_queue` resolves it, and the `NOTIFY` syscall checked only that the queue number was under
`MAX_QUEUES`. So a userspace driver holding a virtio capability could ring a queue it had never set
up, and the kernel wrote a `u16` through `phys_to_virt(0)`: a kernel store, inside no BAR, at a
moment the driver chose. `virtio::notify` now refuses that queue via `Transport::is_doorbell_ready`,
and a unit test builds the two transport values by hand so it runs on both ISAs and in the mmio-only
configurations that have no PCI function at all.

The mmio transport was never exposed, because it has one fixed notify register and nothing per-queue
to resolve. That is why the defect was invisible from the syscall and only appeared in the PCI arm.

## The worst SAFETY comment in the tree, and it passed every gate

`components/src/virtio_net_transport.rs`'s `w16` carried this, over a `write_volatile` into the DMA
page:

```
// SAFETY: `invoke` traps to the kernel, which validates the capability and the method
// before acting (user_mode_runtime's contract). A caller cannot break an invariant by passing a
// bad slot or method; it gets an error back.
```

There is no `invoke` in the function. The comment was pasted from `mr`/`mw` a few lines below and
describes a different operation, on a different mechanism, with a different contract. Its five
siblings (`r8`, `r16`, `r32`, `w8`, `write_desc`) carry the correct DMA-page sentence, so the defect
is one line in a block of six. `undocumented_unsafe_blocks` was green on it the whole time,
because the property it checks is that a comment exists. DECISIONS §61 already carries a BUGS note
predicting this; this is the in-tree instance.

## What is not mechanically checkable, stated plainly

The milestone's headline property has no gate, and should not be given one. Whether a SAFETY
comment binds anybody is not a syntactic question, and the measurements say so rather than the
intuition:

- The tree has 937 `// SAFETY:` comment blocks. 871 are inside a safe fn, which is the normal
  and correct case: an `unsafe` block in a safe fn whose soundness is discharged locally is what
  most correct Rust looks like.
- 36 of those mention a caller. Three are artifacts of this milestone's own prose quoting the
  string `// SAFETY:`, so 33 are real, and 19 of the 33 are legitimate (the calling *thread*,
  the calling *process*, an IPC caller, or a fact the parameter type already enforces). A gate on
  "SAFETY plus caller in a safe fn" would be wrong more often than right.
- And it would have missed `pwrite` and `high_water`, which are two of the six real ones, for the
  reasons above. A check that is both noisy and incomplete is a nag.

An allowlist ratchet would fix the noise and not the incompleteness, at the cost of 19 entries that
each need a reason written and reviewed. Not worth it against a defect class this small. This one
is a review discipline: when you read a SAFETY comment on a safe fn, ask whether the parameter
could have been produced without meeting it.

## What is mechanically checkable, and shipped

A different property, adjacent to the milestone rather than the milestone itself: every `unsafe fn`
states its contract in a `# Safety` section. `script/lint` gained that check.

It earns its place because of the shape measured in the parent's survey. A third of this tree's
`unsafe fn`s contain no unsafe operation at all, so neither unsafe lint has anything to fire on,
and the rustdoc section is the only enforcement there is. Nothing was checking that it existed.
`clippy::missing_safety_doc` is already on via `-D warnings` and does not cover it: that lint fires
only on an exported function, and the interesting ones here (`set_ttbr0`, `write_satp`) are
private to their module.

It found one violation on its first run, `redoxfs_server`'s `file_page`, whose contract was written
but spelled `SAFETY:` in the doc comment instead of `# Safety`, so rustdoc rendered it as ordinary
prose and no tool recognized it as the contract.

Two things it deliberately does not do. It excludes trait-impl methods, because `GlobalAlloc`'s
`alloc` and RedoxFS's `Disk::read_at` are `unsafe fn` by the trait's declaration and the contract
belongs to the trait. Twelve of the tree's 51 are that case. Without the exclusion the check is
twelve false positives out of thirteen. And it checks that a contract is written, never that it is
true, which is the same limit `undocumented_unsafe_blocks` has one level down. It is a low bar, and
it is the bar that was missing.

## The same defect outside `kernel/`, which is somebody else's lane

The milestone scoped to the four sites in `kernel/`. The survey pattern, run over the whole tree,
finds 13 more of the same shape, and they are listed here so the finding lives somewhere a person
reads rather than in a report:

| Site | The comment's claim |
|---|---|
| `components/src/virtio_net_transport.rs` `r8` `r16` `r32` `w8` `w16` `write_desc` | "callers pass offsets inside it" (the DMA frame) |
| `fixtures/src/fs_test_client/extended_attributes.rs:21` `fill_page` | "the caller keeps within it" |
| `components/src/fs_file_caretaker.rs:77` `get` | "callers clamp `out` to the page" |
| `components/src/fs_nameset_caretaker.rs:107` `get_at` | "every caller clamps `out` and `off` to the page" |
| `fixtures/src/file_source.rs:108` `get` | "callers clamp `i` to the page" |
| `components/src/swish/main.rs:430` `put_page` | "every caller is behind a `dir.is_some()` check" |
| `components/src/line_editor.rs:217` `copy_in` | "offset+len is bounded by PAGE by every caller" |
| `patches/std-nife/overlay/std/src/sys/fs/nife.rs:161` `put` | "callers clamp to it" |

All eight rows are the same clamp-to-a-page obligation, which suggests the answer there is one
shared page-slice type rather than eight conversions. That is a design question and wants its own
lane. The `patches/`
one is in the vendored std overlay, which most gates exclude on purpose.

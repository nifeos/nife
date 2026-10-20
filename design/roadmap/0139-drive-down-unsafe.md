---
status: BUILT
raised: 2026-08-18
built: 2026-10-20
---
# 139. Drive the unsafe count down, and cinch the ratchet behind it

In brief. Reduce the hand-written `unsafe` this tree carries outside `kernel/src/arch/`, and lower
the ceiling after each reduction so the ground gained cannot be given back quietly. The instrument
is milestone 134 (the register of measures)'s census and the ceiling relation in `script/lint`.

## Where it stands

The ceiling is `<!--count-at-most:unsafe-density-outside-arch-->`, in blocks per 10,000 lines
outside `kernel/src/arch/`. It sits at the tree's own value with no headroom, a trade calef ruled for.
Its current number, and the reasoning for each step, are in `notes/unsafe-obligations/ceilings.md`.
The `unsafe impl`s other than `Send`/`Sync` are held at the tree's exact value as
`unsafe-trait-claims` (name provisional). `unsafe fn` has no ceiling: every call of one is already
a block the density counts, because `unsafe_op_in_unsafe_fn` is on, and the `# Safety` contracts
check binds each one.

The shared primitives this milestone's reductions rest on, each holding one invariant once:

| Primitive | What it holds |
|---|---|
| `user_mode_runtime::mapped_window::MappedWindow` | a mapped page or frame, bounds-checked at every access; `as_slice`/`as_mut_slice` hand the whole window to a parser |
| the crate-root `invoke` wrappers and `virtio::read_register` and its siblings | one capability method each, safe to call with any slot naming that method's object type (see BUGS) |
| `initial_ramdisk::bytes(len)` | the initrd mapping every program starts with |
| `crate::memory::alloc_zeroed`, `alloc_contiguous_zeroed` | a fresh frame, zeroed, exclusively the caller's |
| `crate::device_tree` | the firmware's device-tree pointer |
| `revoke::chain` | the revocation log's page walk, with `SPACES` held by a borrow |
| `AddressSpace::while_installed` | installing a space for the length of a closure |
| `intrusive_fifo::Unqueued<T>` | a thread's right to be on one queue (name provisional) |

## Rules in force

The scheduler's queue handoff (calef, #1842): an ownership token, `Unqueued<T>`, which is not
`Clone`, replaces `NonNull<T>` in `intrusive_fifo` and `inter_process_communication`. Only a pop,
a removal or thread creation mints one. The running thread's token lives on `Thread::own_token`,
not in its core's current slot, because a slot written by one core and read by another would need
a new `unsafe impl Sync`, or about 5% of the fastpath's band. Asked "confirm the running thread's
queue token stays on Thread::own_token, not the per-core slot your #1842 ruling named", calef
answered "a" (2026-10-20 UTC).

Names (calef, #1842): the foreign spelling stops at the boundary. `initrd` survives only as the
device tree's `linux,initrd-start` and QEMU's `-initrd`, and the program side says
`initial_ramdisk`. "IRQ" becomes "interrupt", per §154 (the acronym test is whether the phrase is
spoken). Each ratified name carries its `Name: ratified` line at its definition, and
`design/naming.md` states the boundary rule.

`MappedWindow::as_slice` and `as_mut_slice` stay, though they cost raw blocks. calef: "Keep. We
don't want to penalize reuse." One invariant at six declarations beats eighteen call sites.

## Exit criterion

calef, #1842: this block is BUILT when every open item and every Follow-on entry marked
Outstanding is done, split out with a recorded owner, or refused with a reason. No item is open
and no Follow-on entry is Outstanding.

`kernel/src/arch/` is not a target and this block will not accept a reduction there. Rule 1 says
architecture-specific code lives under `arch/`, and unsafe is what architecture is made of. Driving
that number down means either writing the assembly wrong or moving it somewhere it does not belong,
and both are worse than the number.

## What counts as a reduction, which is the whole design of this block

The metric is gameable and the obvious way to game it is invisible. Moving three `unsafe` blocks
into one helper function reduces the count by two and reduces the risk by nothing: the same
invariants are asserted by the same code in a different place. A milestone that rewarded that would
make the tree worse while the graph went the right way, and the graph would be the reason.

**So the test is not the token count, it is the number of distinct invariants asserted by hand.**

§94 (what may live in a userspace library, and what must be per-binary) is the worked example and
it is what a real reduction looks like. The trap instruction was inlined at 48 sites in 7 variants
across 58 panic handlers, each one a hand-written assertion of the same invariant. Lifting it into
`user_mode_runtime::trap()` left one. That is 47 chances to write it differently, removed. §94 states the
general form: *a per-binary item whose body is copied verbatim into every binary is not per-binary;
only its declaration is.* Copying it is asserting the same invariant N times by hand.

And §94 found the cost of the copy by counting the copies: one of the 58 handlers was different.
`terminal_sink_caretaker` called `exit()` and never trapped, so a panic there reported `EVENT_EXIT`
where every other program reports `EVENT_FAULT`. **A supervisor would have been told a panicking
program finished cleanly.** The duplication was not only ugly, it was already wrong in one place
and nobody knew.

So a reduction qualifies when it does one of these:

- Collapses N hand-written assertions of one invariant into one, the §94 shape. Best available,
  and the only one that reliably reduces risk rather than moving it.
- Replaces raw pointer arithmetic with a typed abstraction whose invariant the compiler or Kani
  holds, so the assertion stops being a comment. Rung one on the ladder.
- Deletes unsafe that was never needed, which is the cheapest and rarest.

It does not qualify when it merely relocates unsafe, wraps it in a function whose safety argument is
the same argument, or hides it behind a macro.

## The ratchet

After each reduction, **lower the ceiling to the new count in the same commit.** A ceiling left above
the true number is not a ratchet, it is a budget, and a budget gets spent by whoever arrives next
without knowing it was won.

## Why it matters

DECISIONS §14 (the project's direction) calls this a verified-Rust capability microkernel; `unsafe`
is where that claim is suspended. Every block outside the architecture layer is a place where the
proofs and the type system are standing aside and a person's comment is the whole argument.

## BUGS

- This block sets no target number. A ceiling with no headroom fires on the next honest block
  anywhere outside `arch/`, and `script/lint` has had three checks deleted for only ever rejecting
  legitimate work. `notes/unsafe-obligations/ceilings.md` records what the zero-headroom trade
  costs.
- A reduction can be real and still not show in the count. The slice accessors collapsed eighteen
  independently worded page-sharing invariants into six declarations and raised the raw count by
  12, because a whole-slice hand-off has no runtime check that lets the call site drop `unsafe`.
  Do not let the number alone decide which work is worth doing, in either direction: it
  under-credits that collapse and it would over-credit a Kani-proved block that stays where it is.
- **A wrapper is sound only for a slot naming the object type it was written for.** Method numbers
  are per object type and overlap: `rendezvous::SEND_CAP` and `address_space::UNMAP` are both 2,
  `rendezvous::SEND` and `memory_region::MAP` both 0. The kernel dispatches on what the slot names,
  so `send_capability` on a slot holding the caller's own address space is an `UNMAP`, from safe
  code, of whatever page the first argument names. Nothing in the tree does this; the tell is a
  slot number that came from somewhere other than the endowment that typed it. The fix is a typed
  slot per object type, which is a design question for an architect, proposed as its own
  milestone.
- `AddressSpace::UNMAP` has no safe wrapper on purpose. Every caller promises that nothing it
  still uses lives at that address, which is a fact about the caller's own memory that no wrapper
  can check, so `supervision_protocol::give_up_own_page`, `socket_table` and
  `domain_name_resolver` each argue it at the call.
- `window.rs`'s `ROLE_PROBE_INPUT` calls `invoke` raw, to read the negative `abi::Error` a
  `RECEIVE` on an empty slot returns; `user_rt::receive` discards that code. Its `# Safety` comment
  says why it is the one exception.

## Follow-on

- **Done.** `kernel/src/revoke.rs`'s log-page walk is `revoke::chain`, one block.
- **Done.** `system_tests/src/user/tests.rs` carries seven blocks, each a one-off with its own
  argument at the site.
- **Done.** `crates/abi`'s `START` doc comment says what the kernel does with its three arguments.
- **Done.** The ratchet's scope: `unsafe-trait-claims` gates the other `unsafe impl`s, and
  `unsafe fn` gets no ceiling, for the reason under "Where it stands".
- **Done.** The renames calef ruled on #1842 and #1998 are applied, each with `Name: ratified` at
  its definition.
- **Done.** The scheduler's ownership token, `Unqueued<T>`, is in `crates/intrusive_fifo`, and the
  running thread's token lives on `Thread::own_token`, as calef confirmed on 2026-10-20 (UTC).
- **Done.** Outside `user_mode_runtime`, a shipping crate calls `invoke` raw only where no wrapper
  covers the method or its arguments. `supervision_protocol`, `virtio`, `system_initializer`,
  `printenv`, `socket_test_client`, `free`, `slabtop` and `swapper` use the runtime's wrappers, and
  `system_initializer` imports no `invoke` at all. `memory_region_usage` (name provisional) is the
  USAGE call three of them had copied.
- **Done.** The host tests of `current_cpu_contract`, `user_mode_heap` and `redoxfs_server`'s block
  cache argue each contract once. The first uses a test buffer type. The heap tests use `Lent<'a>`,
  whose borrow and by-value `free` the compiler checks. The cache tests use one safe `read`/`write`
  pair over redoxfs's `Disk`, whose `# Safety` asks nothing of a caller.
- **Done.** The page contracts' tests (`environment_contract`, `counter_frequency_contract`,
  `clock_contract`) lend the page through one `with_page` helper each. The kernel's `fp` tests hand
  over through a `Borrowed` token that only the interrupt-masked body holds. The `paging` host tests
  discharge a TLB flush through one `never_installed` helper per file, and its domain tests build
  and walk through `FramePool`.
- **Recorded.** `allocation_cache`'s tests keep their blocks. Each drives a different part of the
  cache's raw-pointer API (a refused `put` that aliases on purpose, a refill from a heap, a spill
  into `dealloc`), and a wrapper would hide what they exercise.
- **Recorded.** `crates/inter_process_communication` carries no `unsafe` in production code.
- **Recorded.** The NS16550 halves of the console and input programs stay hand-written, because
  the register stride is a runtime fact no register-layout macro can express.
- **Recorded.** MMIO and device-register blocks, the three stack helpers and deliberate-fault test
  programs (`hello.rs`, `flaky.rs`, `outlaw.rs`, `memory_grant_depleter.rs`, `swapper.rs`) are not
  candidates: each block asserts a different fact.
- **Recorded.** No target number, by design. The ceiling stands at the tree's own value with no
  headroom.
- **Refused.** `kernel/src/arch/` is not a target and this block will not accept a reduction there.

## Index row

Collapse hand-written `unsafe` outside `kernel/src/arch/` into one declaration per invariant, and
hold the `unsafe-density-outside-arch` ceiling at the tree's own value.

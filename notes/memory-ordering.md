# Memory ordering, and the fences with no partner

Milestone 116 (the fences with no partner). CLAUDE.md's fourth rule is *assume weak memory
ordering*, and this is the inventory
that says where in the tree that rule is being relied on, who is relying on it, and which
sites were trusting something that is not there.

A release fence with no matching acquire orders nothing while reading as though it does. That is
worse than an absent fence, because the fence is the comment: a reader who meets
`fence(Ordering::Release)` stops asking the question. The mistake was found twice on 2026-08-04 by
two methods that share nothing, and no gate in the tree could see either one. One was the loom
harness of milestone 80 (loom), on the clock page's seqlock. The other was the audit of
milestone 43 (a second security audit), on the compositor.

This note does three things. It counts the population honestly. It adjudicates every site into a
bug, a soundness argument, or dead code. And it answers, with a measurement rather than an opinion,
whether pairing can be checked mechanically. The evidence it has no room for is in
[`memory-ordering/`](memory-ordering/README.md).

## The count

At milestone 116's count the tree held 306 `Ordering::*` sites outside test code. Of those, 243
were `Relaxed` and 63 carried acquire or release semantics. There were 13 `fence` call sites and one
`compiler_fence`, across 45 files. A line-oriented grep gets this wrong, because `rustfmt` splits a
call across lines and prose says "Release" constantly. [`the-count.md`](memory-ordering/the-count.md)
has the full table and the method, including the test-code exclusion that once hid a live fence.

These counts have drifted. On 2026-09-29 `script/lint` counted 19 fences, against the 13, "all
fourteen" and "twelve" this note says in three places. Milestone 622 (the fence inventory is
re-adjudicated) re-derives the count and adjudicates the fences added since; until it lands, take
the number from `script/lint` and not from here.

## Where this kernel's ordering actually lives, which is mostly not here

63 sites is a small number for an SMP kernel on two weakly ordered ISAs, and the reason is
DECISIONS §9 (locking). Almost everything shared is behind a ranked interrupt-safe lock, and
`IrqSafeMutex` wraps `spin::Mutex`, which locks with `compare_exchange(false, true, Acquire,
Relaxed)` and unlocks with `store(false, Release)`. So the overwhelming majority of this kernel's
happens-before edges are supplied by an acquire/release pair inside a dependency, where nothing
in this tree names them and no grep over this tree can find them.

That fact runs through every adjudication below, so it is worth stating as a rule:

> An ordering edge in nife comes from one of four places, and only the first is greppable.
>
> 1. An explicit fence or an ordered atomic, in this tree. 63 sites plus 12 fences.
> 2. The `SCHED` lock, taken by every `SEND`, `RECEIVE`, `CALL` and `REPLY`. A blocking IPC
>    rendezvous therefore orders everything the sender wrote before it against everything the
>    receiver reads after it, at no cost and with nothing written down.
> 3. A spawn. A page written before the process that reads it existed is ordered by the act of
>    creating that process.
> 4. Nothing at all, because the writer and the reader are the same core with interrupts masked.

Point 2 is the one that changes the shape of this inventory. DECISIONS §10 says control by message
and bulk by shared page, and in practice every shared-page publish in this tree except one is
immediately followed by a blocking `CALL` or a `REPLY` to the process that reads it. So the fences
guarding those publishes are, with one exception, redundant rather than load-bearing. They are not
wrong and they are not being removed; the point is that a reader cannot tell which kind a given
fence is, and until this milestone nothing said.

The exception is the clock page, which is the only cross-address-space protocol with no
rendezvous underneath it at all: a process reads the wall clock out of a shared mapping with no
syscall. That is exactly where milestone 80's loom harness found a real bug, and it is not a
coincidence.

## The fences, all fourteen, adjudicated

Each of these now carries a `PAIR:` comment at the site naming where its other half is. The gate in
`script/lint` requires it; see below.

| Site | Side | Partner | Adjudication |
|---|---|---|---|
| `crates/clock_contract` `read` | acquire | the writer's `W_SEQ` release store in `publish` | **Sound as far as it goes, and the writer's opening fence is milestone 80's bug.** The only protocol where both halves must be in this tree |
| `crates/credential_contract` `wipe` | neither | none, and none wanted | **Sound.** A `compiler_fence` emits no instruction and orders nothing between cores. Now says so |
| `kernel/src/arch/aarch64/exceptions.rs` `last_user_fault` | acquire | `USER_FAULTS.fetch_add(1, Release)` in `user_fault` | **Sound, and the model for the tree.** Both halves present, both load-bearing, both explained at the site before this milestone |
| `kernel/src/arch/riscv64/exceptions.rs` `last_user_fault` | acquire | the same pair on the other ISA | **Sound.** Parity holds |
| `kernel/src/user/terminal_print.rs` `term_print` | release | none; the `ipc_call` below it is the edge | **Sound, redundant.** The terminal is blocked in `receive_cap` |
| `components/src/console.rs` `show` | release | none; the `call` below it is the edge | **Sound, redundant.** `term_print`'s case from userspace: `display_terminal` is blocked in `receive_cap` (milestone 400 (shell firmware's)) |
| `kernel/src/user/keyboard_service.rs` `take_typed` | acquire | `ring_publish`'s fence in `components/src/keyboard_driver.rs` | **Sound.** The reader milestone 43 named as getting it right |
| `kernel/src/user/compositor_service.rs` `type_bytes` | release | `drain_input` in `components/src/compositor.rs` | **Sound, redundant** (the doorbell `CALL` follows). **A fourth writer the audit's count of three missed**; see below |
| `components/src/keyboard_driver.rs` `ring_publish` | release | two readers, one fenced and one not | **Sound, redundant.** `call(DOORBELL, ...)` follows immediately |
| `fixtures/src/window.rs` `commit` | release | `serve_frame` in `components/src/compositor.rs` | **The one that is load-bearing.** See below |
| `components/src/display_terminal.rs` `present`, first | release | the display driver's `barrier()`, or `serve_frame` | **Sound, redundant** on the display path |
| `components/src/display_terminal.rs` `present`, second | release | `serve_frame` | **Sound**, by the reply this process is about to send |
| `components/src/compositor.rs` `flush` | release | `barrier()` in `components/src/gpu_driver.rs` | **Sound.** The `CALL` orders the driver's read; the fence covers the driver-to-device leg |
| `kernel/src/drivers/ramfb.rs` `run`, first | release | **none in this tree**: QEMU's `fw_cfg` device model, reading guest memory | **Sound, and the only site whose partner is host code.** The command bytes are normal memory and the register that starts the transfer is device memory; without this the device may read a half-written command. Milestone 243 (a machine with no serial port) |
| `kernel/src/drivers/ramfb.rs` `run`, second | acquire | the release leg five lines above, in the same function | **Sound.** A volatile load of the cleared control word does not order the data buffer's bytes behind it on a weak machine. Milestone 243 |

### The one publish the rendezvous does not cover

Milestone 43's audit called finding 7 reachable "on real weakly ordered hardware", which is true and
which this inventory can now make specific. The path is not the obvious one.

The compositor's doorbell is shared, and `serve_frame` rescans *every* client's control page on a
`COMMIT` from *anyone*. Only the caller is blocked. So when window A commits, the compositor reads
window B's sequence and B's four damage fields while B is running, possibly mid-`commit` on another
core. B's release fence orders B's own stores correctly; nothing orders the compositor's loads, so it
may take the new sequence beside the previous frame's rectangle. There is no rendezvous between B and
the compositor to fall back on.

Every other producer in that subsystem is covered by its own `CALL` or by the reply it is about to
send. That includes the keystroke path in `display_terminal`, where `ring` is deliberately false and
it looks at first reading as though nothing follows. Something does: the compositor is blocked in a
`CALL` *to* the terminal, and the terminal's reply is the edge.

So milestone 43's two acquire fences are the right fix, and the reason they are needed is the shared
doorbell rather than the fence asymmetry that led to them.

### And the audit's count was three, where the tree has four

`fixtures/src/window.rs`, `components/src/display_terminal.rs` and `components/src/keyboard_driver.rs` are the three the audit
named. The fourth is `kernel/src/user/compositor_service.rs`'s `type_bytes`, the kernel playing the
input driver, which publishes into the same ring with the same fence and the same comment. The audit
looked at that file and named its *reader* (`keyboard_service`'s `take_typed`) as the one that gets
it right, so the writer beside it went past.

The fix is unaffected, because `drain_input` is the single reader of both ring producers, and one
acquire fence there covers them. Recorded because the count is wrong in the audit and a later reader
would try to reconcile three against four.

## The 63 ordered atomics, by protocol

| Protocol | Where | Adjudication |
|---|---|---|
| Clock page seqlock | `crates/clock_contract` | Writer's opening fence missing. **Milestone 80's finding**; fixed on its branch, loom-checked |
| Work-steal request slot | `kernel/src/sched/placement.rs`, `crates/work_steal_slot` | Release CAS to claim, acquire swap to take. **Loom-checked** by milestone 80, six harnesses |
| User fault record | `kernel/src/arch/*/exceptions.rs` | Relaxed record, release counter, acquire fence on the read. Correct, on both ISAs |
| Boot roster | `kernel/src/smp.rs` | `ROSTER`, `DESCRIBED`, `ONLINE`, `ONLINE_MASK`: relaxed arrays under a release flag, acquire flag then relaxed arrays. Textbook array publication, single-shot at boot |
| IRQ routing table | `kernel/src/sched/interrupt_route.rs` `IRQ_ROUTES` | Release store, acquire load, same array. Paired |
| One-shot service wiring | `fs_service`, `entropy_service`, `disk_service`, `credential_service` | **Four instances of one correct idiom**: relaxed fields, then a release flag; readers acquire the flag, then read the fields relaxed |
| Spin locks | `crates/user_mode_runtime`, `patches/std-nife` (3), `redoxfs_server/src/bin/second_mount.rs` | Acquire CAS, release store. **The one shape that cannot be one-sided**, because the lock is both halves |
| Benchmark start barrier | `kernel/src/bench.rs` `TP_GO` | Release store, acquire spin. Paired. The `SeqCst` reset is over-strong and has no reader yet, so it orders nothing and costs nothing |
| Secret wipe | `crates/credential_contract` | `compiler_fence`, no cross-core meaning, no partner wanted |

Four sites deserve more than a row, and
[`the-atomics-in-detail.md`](memory-ordering/the-atomics-in-detail.md) has each. `HWID` in
`kernel/src/smp.rs` is an acquire with no release, sound because nothing is published behind it,
and left alone. The compositor's `publish` had a comment crediting store order with what only its
run-once placement guarantees; the comment was corrected, not the code. The window list's write
order is backwards for a reader that does not exist yet. And of fifteen compare-exchanges, the three
relaxed on success publish nothing behind the value.

## Can pairing be checked mechanically? Measured, not argued

Broadly, no. A per-variable check (every `Acquire` needs a `Release` on the same atomic) was built
and run on the 63 sites. It raised seven flags, six on correct code, including the best pair in the
tree, and missed `HWID`, the one genuine finding. A check that fires on the exemplary case and stays
quiet on the defective one is an anti-check. The reason is structural: the partner is often an IPC
rendezvous inside `spin::Mutex`, the halves live in different programs, and the defect is an
absence with no token to match. [`checking-pairing.md`](memory-ordering/checking-pairing.md) has the
measurement.

Narrowly, yes, and it is worth having. Same posture as the SAFETY check of milestone 112 (the SAFETY
comments that bind nobody):

> **Every `fence` outside test code must carry a `PAIR:` comment naming where its other half is.**

`script/lint` enforces it, over the contiguous comment block immediately above the fence, which is
where `SAFETY:` already lives. It checks the bookkeeping, not the ordering, and that limitation is
the design. What it buys is the forcing function: writing the marker means opening the other file,
at which point an absence is visible. It covers fences only, because a `Release` store names its
variable and `fence()` names nothing at all.

## What actually wants a loom harness

`script/interleaving-check` (milestone 80) is the only tool in the tree that can *decide* a specific
protocol, so the useful output of an inventory is a queue for it rather than a gate. In priority
order, with the reason each earns a harness:

1. **The compositor's control page.** The one protocol above with a real reader that is not covered
   by a rendezvous, and the one where a reachable bug was found by reading. It needs the reader and
   two concurrent writers, which is the shape loom is good at. Blocked on nothing except that the
   logic lives in `components/src/compositor.rs`, a `no_std` binary, so `serve_frame`'s page reads have to
   come out into `crates/compositor` first. That is rule 7 pushing in the direction it always pushes.
2. **The input ring**, `crates/compositor`'s `ring`. Two producers (`components/src/keyboard_driver.rs` and the kernel)
   and two consumers (`drain_input` and `take_typed`) over one head/tail contract, with only one of
   the four consumers' sides fenced before milestone 43. A four-party contract with an asymmetry in
   it is worth a model even when the answer is "sound".
3. **The one-shot wiring flag**, the idiom repeated in four services. A single harness over one
   extracted helper would cover all four and would turn "the same four lines, four times" into a
   checked property instead of a pattern someone recognizes.

Not worth one: the spin locks (the lock is both halves), the boot roster (single-shot, single
writer), and `TP_GO` (nothing published behind it). Nor is anything under `arch/`, where rule 1 keeps
the code and lifting it is a larger question than a harness.

## EXAMPLES

Run the gate:

```
$ script/lint
...
==> every fence names its counterpart
fences: 12 outside test code, each naming its counterpart
```

See it fail, which is worth doing once before trusting it. Delete the `PAIR:` line above any fence:

```
$ script/lint
lint: fences with no PAIR: comment naming the other half:
  user/src/kbd.rs:194: core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
```

Take the count yourself, without trusting the table above. The multi-line trap is real, so compare
the two:

```
$ git grep -c 'Ordering::' -- '*.rs' | awk -F: '{n+=$2} END {print n}'      # lines, not sites
$ git grep -o 'Ordering::[A-Za-z]*' -- '*.rs' | wc -l                       # sites
```

Write a marker for a new fence. The form is where, not why, because the "why" is already the comment
above it:

```rust
// The bytes must be visible before the tail that advertises them.
//
// PAIR: `take_typed` in kernel/src/user/keyboard_service.rs has the matching fence.
core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
```

If there is no matching fence, say what the edge is and name it:

```rust
// PAIR: no acquire fence, and none is needed. The terminal is blocked in `receive_cap` and the
// `ipc_call` below is what wakes it, so the kernel's release of the `SCHED` lock and the
// terminal's acquire of it are the pair.
```

## BUGS

- **The gate checks that a comment exists, not that it is true.** A `PAIR:` marker naming a function
  that does not exist, or naming the wrong one, passes. It is bookkeeping with a forcing function
  attached, and the paragraph above says so for the same reason this one does: the failure mode of a
  gate people over-trust is worse than the failure mode of no gate.
- **The 243 `Relaxed` sites are not adjudicated.** Only the compare-exchanges were checked, on the
  grounds that a relaxed claim is the classic form. A relaxed store used as a publication flag with
  data behind it would be the same bug as this milestone's and is not covered here.
- **Nothing here is a proof about aarch64 or riscv64.** Every soundness argument in this note is a
  C11-model argument about happens-before, and so is loom's. `notes/interleaving.md` (milestone 80)
  says the same thing three times and it is worth repeating a fourth: a failure either tool reports
  is real, a clean result is not a proof about the silicon.
- **The rendezvous argument rests on `spin::Mutex`, which is a dependency.** If `IrqSafeMutex` were
  ever reimplemented, roughly half the adjudications in this note would need rereading and nothing
  would fail first. A `spin` bump that weakens the lock's Acquire compare-exchange or Release store
  fails `script/lint` (`helpers/kernel_dependency_unsafe.py`'s pins), but that is a text match, and
  nothing proves the lock gives exclusion. This is the largest unproved assumption here; the
  proposal [the kernel's lock proves what it takes from spin](../design/roadmap/proposals/the-kernels-lock-proves-what-it-takes-from-spin.md)
  tracks it.
- The 63-site figure is a count of *syntax*, not of protocols. Nine protocols cover all of it,
  and a tenth could be added tomorrow in `Relaxed` and never appear in this table.
- Two of the three findings this note describes are fixed elsewhere. The clock seqlock's writer
  fence is milestone 80's, and the compositor's two acquire fences are milestone 43's. This milestone
  changed no ordering at all, which was its scope note and is the honest result: the inventory
  found one wrong comment and one decorative `Acquire`, and no new bug.
- `crates/user_mode_runtime`'s spin lock and the interrupt-routing lottery cannot be modeled today,
  for the reasons milestone 80 recorded: `user_mode_runtime` is aarch64 inline `asm!` and does not compile for the
  host, and the lottery lives under `arch/`.

---

*See `notes/locking.md` for the ranked-lock discipline that makes this population small, and
`notes/deadlock.md` for the other half of that discipline. `notes/interleaving.md` (milestone 80)
has the loom harnesses that can decide a protocol. `notes/shared-page-audit.md` (milestone 43) has
the audit whose finding 7 is half of this milestone's motivation, and `notes/compositor.md` the
subsystem most of the fences are in.*

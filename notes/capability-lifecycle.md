# How authority moves, narrows, and ends

The companion to [ipc-naming.md](ipc-naming.md). That note is about *naming* (IPC names an
endpoint, never the peer). This one is about the *lifecycle* of the capabilities themselves: how
authority is copied, how it narrows, and, at the end, how it is revoked.

## Authority moves by copy-with-narrowing, never by widening

A `Cap<O>` is `Copy` (`crates/capability`). Authority spreads by deriving a copy, and the one rule
is that a derivative's rights are a subset of the source's:

```rust
// CapabilityTable::derive: "copy a capability into another slot, with rights that are no greater."
if !rights.is_subset_of(src.rights) { return Err(NoRights); }
```

`Rights` are five bits <!--count:rights-bits-->: `READ`, `WRITE`, `GRANT`, `ENUMERATE` and `BIND`.
`is_subset_of` is the whole enforcement; there is no code path that widens rights, which is the
point (DECISIONS §10 (process model: capability-based, microkernel)): if delegation could widen authority, the model is theater.

`ENUMERATE` is the newest and the one that shows why the count is worth gating rather than retyping.
It arrived on 2026-08-17 with milestone 126 (the `procps` package), this line still said three bits the next day, and the
2026-08-17 documentation sweep is what found it. It is the right to learn what exists, as distinct
from acting on it, the kernel-level twin of `filesystem_contract`'s directory `ENUMERATE`, and
`endpoint::SURVEY` is its only consumer today. The argument for a right of its own rather than a
corner of `READ` is on `capability::Rights::ENUMERATE`, where a reader meets it. `READ` on a
supervision endpoint is what `RECEIVE` and `REAP` take, so a `ps` granted `READ` could reap a child,
and a domain names its members rather than acting on them.

## `SEND_CAP` is share, not move

Delegating a capability over IPC (`SEND_CAP`, whose body is `sched::ipc_delegate_cap`) reads the
sender's cap and delivers a *new* one to the receiver:

```rust
// sched::Delegation::derive, run under the IPC_TABLES hold that also files the copy
let src = table.get(self.slot)?;            // read; the sender's slot is NOT emptied
if !src.rights.allows(GRANT) { return NotPermitted; }   // may I pass it on at all?
if !self.rights.is_subset_of(src.rights) { return NotPermitted; }  // only narrow
Ok(Cap { object: src.object, rights: self.rights })
```

**The read and the filing are one critical section, and that is a revocation property, not a
tidiness one** (2026-10-04 UTC). Until then the syscall layer read the source with `current_cap`
and handed the copy to `ipc_send_cap`, which filed it under a second hold of the lock. A
`PageFrame::REVOKE` or `MemoryRegion::DESTROY` sweep could run between the two, delete the source,
and pass by before the copy was filed, so the copy survived the revoke. `CAP_INSERT` and `SLICE`
had the same shape. `sched::Delegation` carries the argument and
`system_tests::user::revocation_window_tests` the three tests that drove a sweep into the gap.
A `MAP` had the use-side version of the same gap: it read its frame, then mapped and recorded, and
a sweep that scanned the mapping log in between left the mapping alive. `PageFrame::MAP`,
`AddressSpace::MAP_INTO` and `MemoryRegion::MAP` now read their source under the hold of the mapping
registry that maps and records (`revoke::MappingHold`), and
`system_tests::user::map_revocation_window_tests` drives a sweep into each.

So the sender keeps its capability; the receiver gets a narrowed derivative pointing at the same
object. That is exactly what lets a frame be shared: a producer holding `READ|WRITE|GRANT` keeps its
writable mapping while handing a consumer a read-only view of the same physical page.

## Independent narrowings

Delegation answers separate questions, and they narrow independently:

| Question | Right | Example |
|---|---|---|
| What may the holder **do**? | `READ`, `WRITE` | a `PageFrame` with `READ` alone maps read-only, never writable |
| May the holder **pass it on**? | `GRANT` | a derivative sent *without* `GRANT` is a dead end: the receiver may use it but not re-delegate |
| What may the holder **learn**? | `ENUMERATE` | a `Rendezvous` with `ENUMERATE` can `SURVEY` the domain it supervises; one with `READ` instead can `RECEIVE` and `REAP` there but cannot list it |

This section was headed "Two independent narrowings" and had the first two rows until the
2026-08-17 documentation sweep; `ENUMERATE` made the third question a real one the day before. The
heading no longer counts them, because the count is the part that rots and the independence is the
part that matters.

`SEND_CAP` needs `WRITE` on the *endpoint* (may I send here?) and `GRANT` on the *delegated*
capability (was I trusted to lend it?). Two rights, two objects, two questions.

## Frames, end to end

The frame path shows every piece confining the next:

1. `MemoryRegion::RETYPE` mints the owner a `PageFrame` with `READ|WRITE|GRANT` (`syscall.rs:181`).
2. The owner maps it writable (`PageFrame::MAP` with the writable flag needs `WRITE`).
3. The owner delegates a `READ`-only, no-`GRANT` derivative with `SEND_CAP`.
4. The consumer's `PageFrame::MAP` sees `READ` without `WRITE`, so it is confined to `user_rodata`: it
   maps the same physical page but cannot write it, and cannot pass it on.

The test `a_frame_capability_shares_a_page_and_a_read_only_view_cannot_write_it` pins exactly this.
This is DECISIONS §10's "shared memory carries data," composed by the processes at runtime rather
than wired by the kernel at spawn. Read-only derivatives at send time: yes, and enforced all the way
to the page-table bits.

## What a read-only shared frame does and does NOT promise (the consumer's contract)

This is the part that gets misremembered later, so it is worth stating as a contract rather than
leaving as an emergent property of a tested mechanism.

A read-only derivative is *share, not move* (above): the producer keeps `WRITE` and a writable
mapping of the same physical page. So what the read-only bit actually delivers is narrow:

> **It stops the *consumer* from writing the shared page. It does nothing to stop the *producer*
> from writing it while the consumer reads.**

The property is "the consumer cannot corrupt the shared page," not "the page is stable under
the consumer's feet," and not "the data is trustworthy." A consumer that validates the page and
then acts on it is exposed to the producer mutating it *after* the check (a TOCTOU). So, for a
server reading a buffer shared by an untrusted client:

1. Take structural data from the message, never from the page. A length, offset, count, or
   index that you will compute on must ride the IPC message (registers, immutable once sent), not
   live in the mutable shared page. Otherwise the producer edits it under you.
2. Copy-into-private-then-validate. If you must validate content and then act on the validated
   form, copy it into your own memory first and validate the copy. The shared page can change
   between your check and your use.
3. Bound everything by the frame size yourself. Never trust a producer-supplied count to stay
   within the page.

**The console server is the worked example, and it is safe *because* it follows this** (checked).
The length rides the message (`receive(REQUEST)`), and the shared page holds only bytes to print (a
content TOCTOU just prints different bytes: benign). An over-long length is a *read out of the
server's own mapping* that faults the server, i.e. a crashed driver, not a corrupted kernel
(fixtures/src/hello.rs). A future server that read a length or offset *from the page*, or indexed on
page contents, would not be safe, and the read-only bit would not save it.

## Revocation (milestone 13)

**Built.** As of milestone 13 a `PageFrame` capability can be revoked: `PageFrame::REVOKE` unmaps
the page from every holder and deletes every capability to it, and `memory_region::destroy`
reclaims a whole region safely. See DECISIONS §13 and revoke.rs. What follows is the design that
led there, in the present tense of *before* it existed.

Before milestone 13 a capability, once granted, could not be retracted: there was no
capability-derivation tree, no refcount, no `revoke`. The only trace of the idea was `untyped.rs`:
"revocation of derived objects is the harder seL4 story parked for later."

The crucial thing is *what that does and does not cost*, because the lifetime makes it narrower than
it sounds:

**It is not a memory-safety hole.** Frames come from spend-only untyped: `retype_page` only
advances a watermark and never reclaims (`untyped.rs`). And address-space teardown deliberately does
**not** free a mapped frame's leaf, only the page tables reaching it (`user.rs`: *"the frame is not
recorded for freeing, because we do not own it"*; see [teardown.md](teardown.md)). So a peer that
still maps a shared frame after the granter has exited is mapping valid, non-reused memory. No
use-after-free, no double-free. The safety is structural.

**What it does cost is control and reclamation:**

- You cannot un-share. Hand a peer a read-only view and then distrust it, and you cannot take
  the mapping back. The only lever is the blunt one: destroy the peer (tear down its address space,
  which unmaps everything it holds). There is no fine-grained "revoke just this frame."
- You cannot reclaim. A retyped page is spent from the untyped forever; sharing is one-way until
  the whole untyped region is destroyed.

seL4's answer is a capability-derivation tree plus a recursive `revoke` that walks it, unmapping the
object from every holder. It is expensive (a tree walk) and it is kernel-tracked (every derivation
recorded), which is precisely why it is a first-class object there and parked here. See
[design/open-design-ideas.md](../design/open-design-ideas.md) for the deferral and its trigger.

## Where authority can enter at all

Only three ways, and none is ambient: retype it out of untyped you hold, be handed it
(`SEND_CAP` / spawn-time grant), or derive a narrower copy of one you already have. There is no
`open(path)`, no global name, no way to mint authority from who you are. A thread's capability table is empty
until something puts a capability in it. That is the whole of §10, seen from the object's side.

## The sweeps that delete capabilities from every table

`kernel/src/sched/capabilities.rs` holds the deleters that walk every thread's capability table.
Each holds `IPC_TABLES` for its whole walk and takes each table's lock beneath it.

Every sweep clears the hand-off slot too. A capability sent to a rendezvous nobody is receiving
on yet sits in the sender's `Thread::outgoing_cap`, in no table, until a `RECEIVE_CAP` files it. A
sweep that read tables alone would leave a live capability there, and `MemoryRegion::DESTROY` would
return its pages while it was still on its way (§13 (frame revocation)'s use-after-free).
`delete_reply_caps_naming` applies the same rule to `Reply`. The send is not failed: the data word
still arrives and the receiver sees `NO_CAP`, as it does when its table is full, so no new error
reaches userspace. `kernel::user::revocation_in_flight_tests` is the falsification.

### Two `PageFrame` sweeps, for two questions

`delete_page_frame_caps(phys, count)` is `PageFrame::REVOKE`'s. It deletes every capability naming
exactly that run. A narrowed copy goes, since `derive` never changes the object. A different
sub-range of the same memory stays (§102 (a frame names a run)); whether a revoke should take an
overlapping holder's capability is the open question of §132 (what `PageFrame::REVOKE` owes an
overlapping run).

`delete_page_frame_caps_overlapping(base, size)` is reclamation's. The allocator is about to reuse
the range, so any capability naming any page of it goes. Equality would miss a run capability,
since `PageFrame(base, 311)` equals no `PageFrame(p, 1)` inside it. The range sweep does not consult
the mapping log, so a retyped frame that was never mapped loses its capability too. Its end
saturates, and saturating to `u64::MAX` overlaps everything, which is the safe direction for a sweep
whose failure mode is missing a holder. In both sweeps the caller's own capability goes: a revoke
removes all access.

### Take-back sweeps, and port ranges

`delete_device_frame_caps_from_others` (§41 (an endpoint as broker)) and `PortRange::REVOKE`'s
`delete_port_range_caps_from_others` spare the invoker's own capability. That is the difference
between reclaiming a page and taking a device back to hand on, and
`revoke::revoke_device_from_others` has the reasoning. The kernel mints a port capability once, at
boot. `delete_port_range_caps`, which spares nobody, is for the kernel's tests.

On `x86_64` a port range is enforced by `Thread::port_range_grant` and the TSS bitmap the context
switch installs from it, not by the table. Clearing the cached grant is not enough on more than one
core, because a holder running elsewhere keeps its core's bitmap until its next switch. So the sweep
ends with `segments::revoke_port_grant_everywhere`, inside `IPC_TABLES`. The far end writes a core's
TSS from an NMI, at an arbitrary instruction boundary. Holding the lock across the send means no
core is inside `install_port_grant` then, and none can install a grant the sweep already cleared.
`schedule`'s install runs under the lock for this reason, and `segments::set_port_range_grant_on`
states the rule at the writer. The NMI is forced: it is the only message an x86 core takes while it
spins for this lock with interrupts masked ([notes/x86-tlb-shootdown.md](x86-tlb-shootdown.md)).
With an invoker, its own core is spared, because the bitmap there is the invoker's own grant.

`delete_current_cap` takes only the running thread's table lock, so the one-shot `Reply` every
`REPLY` consumes costs no `IPC_TABLES`. On `x86_64`, deleting the `PortRange` behind the thread's
cached grant also drops the grant and resets this core's bitmap at once. Otherwise the table would
say the authority was gone while the switch kept installing it, and `system_initializer` deletes its
console port capability on every boot. That step takes `IPC_TABLES`, like every TSS bitmap writer.
No broadcast is owed: a core's bitmap permits a range only for the thread running on it, which is
the caller.

### Delegation reads its source in the critical section that files the copy

A `Delegation` names a source slot and the rights the copy keeps, not a capability. A copy is safe
only if its source is read in the same critical section that files it, and that critical section
excludes every sweep. A delegation into another thread holds `IPC_TABLES` across both
(`ipc_delegate_cap`, `thread_control_block_delegate_cap`). A derivation into the running thread's
own table (`grant_derived`) holds that table's lock across both. A sweep then falls wholly before
(the source is gone, and the delegation answers `NoSuchSlot`) or wholly after (the copy is in a
table it walks). A copy read in one critical section and filed in another could be filed after a
sweep passed, and a `PageFrame` has no generation to make it inert.
`system_tests::user::revocation_window_tests` drives a sweep into that window on `SEND_CAP`,
`CAP_INSERT` and `SLICE`. Passing a slot leaves no parameter a stale copy could arrive through.

Objects with generational names do not need this. `Rendezvous::BADGE` and `MemoryRegion::SPLIT`
still read, then grant, and a copy minted after its object died names a dead generation and fails
on use (§16 (object revocation)).

### BUGS

- A port range revoke matches by object equality, not overlap. A capability naming a wider or
  narrower range over the same ports keeps its capability and its cached grant. No syscall reaches
  that, because `PortRange::REVOKE` takes `(base, count)` from the invoker's own capability. A change
  that let a caller name a sub-range would open it (`notes/confinement-outsider-pass-8.md`, near
  miss 1).
- Deleting one of two copies of a `PortRange` disturbs the other. The grant is one `Option` on the
  thread, so a thread holding the range in two slots loses `in`/`out` when it deletes either. §12
  (call/reply IPC)'s "dropping one capability does not disturb the others"
  ([notes/confinement-claims.md](confinement-claims.md) row 5) is false for this object on `x86_64`.
  It fails safe and no consumer holds a range twice; a count or a scan on delete would close it.

---

*Add to this file as new capability-lifecycle questions come up.*

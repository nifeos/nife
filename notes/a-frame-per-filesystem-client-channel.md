# A frame per filesystem client channel: the fork

Written 2026-09-26 (UTC) by lane `milestone/fs-client-page`, for milestone 599 (a frame per
filesystem client channel), whose number is provisional. The block is
[design/roadmap/0599-a-frame-per-filesystem-client-channel.md](../design/roadmap/0599-a-frame-per-filesystem-client-channel.md).
The file name is a lane's coinage and provisional.

What is being decided: how the file server learns which client's frame a request's bytes are
in. Every way to give each client its own frame runs into that question, and every answer the tree
could build either changes the kernel's endpoint semantics or adds a protocol. That makes it
calef's, so the lane stopped here instead of building one.

What is blocked until it is answered: all of milestone 599's wiring, and through it milestone
47 (navigation and naming)'s set grant at the prompt (`rm *.txt`), which names this as its
prerequisite in `notes/a-set-grant-at-the-prompt.md`. Nothing else is blocked. The witness test can
be built before the ruling, because its attacker is the same under every option.

## Why the proposal's one-line fix does not work on its own

The proposal (case A of [shared-page-audit.md](shared-page-audit.md)) says to allocate the frame
"in `spawn_fs_client` / `start_granted*` / `narrow_dir` rather than in `ensure`". That fixes the
client side only. The server is the other holder of every channel, and it:

- receives on one endpoint (`redoxfs_server.rs`, `receive_cap(FILE)`), because this kernel has no
  receive over a set of endpoints (DECISIONS §27 (the filesystem service), "Why a process and not a check inside the FS
  server");
- learns nothing about its caller. `RECEIVE_CAP` hands it two words and a one-shot Reply capability,
  and the serve loop's own comment says so: "endpoint-only naming means we never learn who they
  are, only how to answer";
- maps one window at `FILE_PAGE`, `fs::TRANSFER_PAGES` (16) pages wide, and reads every name and
  every `WRITE` payload from its start.

So if two clients each had a private frame, the server would still read one window, and there is
nothing in a request that could tell it which frame to look in. A per-client frame the server can
read needs one of the mechanisms below.

## The premise, checked

It is true, and it is a correctness bug as well as a confinement one. Honest clients race too:
a client stages its name or data in the page and *then* calls, so its use straddles the call. While
the server is blocked on the block server serving someone else, a second client's staging lands on
top. The tree already designs around this. `notes/pipes/the-file-end.md` is why the shell
backs a `>` redirection itself rather than handing it to a process: "two client processes using it
at once race", and `ls > out.txt` would corrupt both. Today's safety is that the shell keeps one FS
client active at a time. A set grant at the prompt would run a checking caretaker beside the shell's
own `>` writer in one pipeline, which is two active clients by construction.

Not yet verified by a run: that the substitution lands on a real boot with two live clients. That is
the witness test's job, and it is the next piece of this milestone either way.

## What the tree does in the analogous cases

- The compositor gives each client its own page and gets away with one doorbell because its
  messages are content-free: it reads every client's page each frame. That does not transfer, since
  the file server must know which page a request is in.
- The network stack has the same shape of bug and an open proposal for it,
  [every-client-of-a-network-stack-shares-its-socket-numbers](../design/roadmap/0649-every-client-of-a-network-stack-shares-its-socket-numbers.md).
  Its option 1 is badged endpoints. A ruling on option A below would decide that option there too.
- DECISIONS §27 records badged endpoints as the alternative to the caretaker and did not take
  it. §101 (notification objects) names badged capabilities as a later fork, and §148 (a supervisor restarts by asking) refused the
  method name `BADGE` to keep it free for this.

## The options

All costs below are computed from constants in the tree, not measured on a boot, except where a
benchmark note is cited. The per-channel figure is the same under A, C and D: one channel is
`fs::TRANSFER_PAGES` = 16 pages, 64 KiB of physically contiguous memory per concurrent client,
where today the whole boot shares one 64 KiB channel. A directory job's region is
`DIR_JOB_REGION_PAGES` = 96 pages (384 KiB), so carving the channel from the job's own region adds
about 17% to it.

### A. Badged endpoint capabilities (seL4's answer)

A derived endpoint capability carries a badge word the kernel delivers to the receiver on
`RECEIVE_CAP`. The server maps K windows and reads request `b` from `FILE_PAGE + b * TRANSFER_MAX`. The
progenitor holds a pool of K (badged endpoint, frame) pairs and hands one to each caretaker chain,
taking it back at reap.

- `filesystem_contract`'s message layout and opcodes do not change.
- The syscall surface does. A method to mint a badged endpoint, and a fourth return value from
  `RECEIVE_CAP` (it already returns the first word, the Reply slot and the second word).
- Per request: nothing added. No extra rendezvous, no copy.
- Server address space: K windows of 64 KiB. K = 8 is 512 KiB, inside the 8 MiB above `BLK_PAGE` that
  `redoxfs_server.rs` says nothing else in the server maps.
- K is a fixed bound on concurrent FS clients per boot, and the pool is new state in the progenitor.
  A server that could accept windows at run time instead needs an attach verb, which is a wire
  change on top.
- Reuse: the network stack's option 1, and any later server with the same shape.

### B. The kernel moves the server's window to the caller's frame at the rendezvous

An endpoint capability is bound, when it is minted, to the caller's channel frame. At `CALL`, the
kernel remaps the receiver's `FILE_PAGE` window to that frame before the server runs. This is
classic L4's receive window for map items (recalled from memory of the L4 X.2 and Fiasco
specifications, not read for this note), which seL4 did not keep.

- `filesystem_contract` unchanged, the server's code unchanged, one `FILE_PAGE` as today.
- A new kernel semantics on endpoints, outside the model §10 (process model: capability-based, microkernel) established: a capability that
  changes the receiver's address space as a side effect of IPC.
- Per request: 16 page-table entries rewritten and a TLB invalidation for the window, on the IPC
  fastpath. Not measured. The icount tripwire would price it.

### C. No kernel change: private client frames plus a staging token

Each confined program gets a private frame shared only with its caretaker. The caretaker copies a
request into the server's page and the reply out of it. It holds a token (a rendezvous used as a
lock) from before staging until after the copy-out, so no two writers of the server page overlap.

- `filesystem_contract` unchanged, the kernel unchanged. The token convention is new, and under
  rule 7 it is a crate, since every writer of the server page must agree on it.
- Per request: two extra rendezvous (take and return the token), about 0.7 to 1.4 µs at the 337 to
  705 ns round trips `notes/benchmarks/cross-os-primitives.md` and
  `notes/benchmarks/calibration-against-sel4.md` record, plus a copy of up to 64 KiB each way.
- It is cooperative. It holds only while every process that maps the server page takes the
  token. Today that set includes the shell and every kernel-harness client (`spawn_fs_client`,
  `start_std`, the sinks), and each would need converting. A holder that skips the token reopens
  the bug with no error anywhere.
- Would we choose it if A and C cost the same to build? No. It has more moving parts, a copy on
  every byte and a rule nothing enforces. Its case is effort: no kernel change. That is its only
  advantage.

### Refused

- A channel index in the request word. A wire change, and it proves nothing: a client can name
  another client's index. Something must bind the index to the caller, and that is option A.
- A file server per client. One block device and one RedoxFS image; two servers mounting it
  would corrupt it.
- The server copies the name out first thing. It narrows the window to the time before the
  server's receive and does not close it. The audit's finding 2 already took this kind of narrowing
  for the nameset caretaker and recorded the residue.
- Notification objects (§101, milestone 151 (notification objects)). They multiplex one endpoint with async signals.
  They do not let a server receive on several endpoints or learn who called, so on their own they
  answer nothing here.
- Keep serializing by construction (one FS client active at a time, as the shell does). That is
  the status quo, and the set grant is exactly the case it cannot cover.

## Reversibility, and who has acted on it

A and B change the syscall surface, which every future program is written against: the expensive
side of the line. C is reversible in code but adds a cross-program convention that is not. Nobody
has built against any of them. The network-stack proposal is waiting on the same question in A's
form. Per the rule for irreversible forks, this note gives options rather than a recommendation.

## The ruling, and what it left open

calef ruled option A on 2026-09-26, and the mechanism is built (milestone 599's block). The
production progenitor still hands every client window 0, and wiring it raises a second fork.

## The progenitor's pool: the fork

What is being decided: how the progenitor gets each client its own window's frame into the client's
address space. `address_space::MAP_INTO` maps a frame the caller holds a capability to, and it maps
the whole run that capability names. So today the progenitor needs one frame capability per window.

The constraint is the progenitor's capability table. `CAPABILITY_TABLE_SLOTS` is 24 and
`CAPABILITY_TABLE_PEAK_MEASURED` is 23 (milestone 590 (the booted system starts its network stack), 2026-09-24), reached during the login block.
That constant's own doc says the next permanent capability "should buy a slot back rather than spend
the last one". Holding the seven extra windows' capabilities permanently needs seven slots that do
not exist. So every option below must cost the progenitor zero permanent slots.

Transient slots are different: a caretaker is built at the prompt, after the login block's peak,
so per-client transients land below it. That is an estimate from where the peak is recorded;
`script/swish-check` is what would confirm it.

What is blocked: only the production pool, and through it milestone 47 (navigation and naming)'s set grant.

### 1. A wider table

Raise `CAPABILITY_TABLE_SLOTS` from 24 to 32 and hold the seven window capabilities permanently.

- Cost: every thread's table is inline in its control block, at 32 bytes a slot. Eight slots are
  256 bytes a thread, 64 KiB across `MAX_THREADS` (256). Computed from the constants `cap.rs` pins.
- It moves `abi::fault::FAULT_EP_SLOT` (always `CAPABILITY_TABLE_SLOTS - 1`) from 23 to 31, which
  every supervised program agrees on. It has moved three times already (16, 17, 24), each a
  one-number change with the tree rebuilt around it.
- §102 (a Frame names a run of pages) considered exactly this for a different consumer and called it brute force: it "buys room for
  the wrong representation". `cap.rs` says the same of raising the ceiling reactively.
- Reversibility: raised three times in practice, never lowered. Lowering it later breaks any program
  that grew into the room.

### 2. An untyped the progenitor retypes per client

Hand the progenitor an untyped; per client it retypes a fresh 16-page frame run, maps it into the
client, and passes the server a capability to it.

- It cannot use the server's fixed windows. Those are mapped into the server at spawn, and a frame
  retyped later is a different frame. So the server must learn about new frames at run time: a new
  `filesystem_contract` verb that carries a frame capability, which the server maps with
  `PageFrame::MAP` into a window it manages. That is a wire change, and a dynamic window table in a
  server that has none today.
- Cost: 64 KiB per client out of whatever region funds it (about 17% of a 96-page directory job), one
  extra rendezvous per client to attach it, and page tables in the server's own budget. K stops
  being fixed, which is its one advantage.
- §102 does not speak to it.
- Reversibility: a wire format and a verb. Irreversible in the tenet's sense.

### 3. Mapping by physical address

Let the progenitor map a physical address it was told, with no frame capability.

- Cost: zero slots, zero per-client syscalls beyond the map.
- It breaks the property `Object::PageFrame`'s doc states: the only ways to hold a frame are to
  retype it or be handed it. Even for the privileged progenitor, that is a new ambient power over
  physical memory in the syscall surface.
- Reversibility: syscall surface. Irreversible.

### 4. One run capability over the pool, sliced per client (found)

Give the progenitor one `PageFrame` naming all K windows as one run, in the slot `fs_page` holds
today. A new method derives a capability naming one window; the progenitor slices window `w`, maps
it into the client with `MAP_INTO`, and deletes the slice.

**One capability does cover the pool, and the 24-slot problem goes away with it:** net zero
permanent slots. The run half needs nothing new. The kernel already mints run capabilities
(`run_cap`, §102 is built), and calef's 2026-09-26 ruling that `MemoryRegion::RETYPE` takes a
page count (#1373) gives a region the same power. What does not go away is handing a client
only its window. `MAP_INTO` maps a capability's whole run, and the whole pool in a client is every
client's window, the original bug. So one operation is still owed, this slice or option 5's offset,
and neither §102 nor #1373 provides it.

- Cost: one transient slot and two syscalls per spawn, noise against a spawn of about 2.8 µs
  (`notes/benchmarks/calibration-against-sel4.md`). The slice can be one page: the progenitor's
  `fs_page` is a one-page capability today, so real-boot clients map only page 0 of a channel.
- §102 anticipates the shape: a consumer "can hold two capabilities: `Frame(phys, 401)` and
  `Frame(phys + 401 * 4096, 74)`".
- §132 (what `PageFrame::REVOKE` owes an overlapping run) left "two capabilities sharing a base but
  not a length" as one record, and window 0's slice shares the pool's base. Nothing revokes these
  frames, so it is a `BUGS` entry, not a bug.
- Reversibility: a new method on `PageFrame`. Irreversible.

### 5. `MAP_INTO` with an offset into a run (found)

Same single run capability as option 4, but `MAP_INTO` itself takes a page offset and count, packed
into its third argument beside the map kind.

- Cost: zero permanent and zero transient slots, one syscall per spawn.
- It changes an existing method's argument shape, which §102 went out of its way to keep fixed, and
  it is the per-page narrowing within a run that §102 lists under "What this does NOT decide".
- Reversibility: syscall surface. Irreversible.

### 6. A window broker (found)

A small process holds the K window capabilities in its own table (eight fit easily; it holds little
else). The progenitor holds one endpoint to it, in `fs_page`'s slot. Per client it asks for a window,
receives a frame capability with `SEND_CAP`, maps it, and deletes it; at reap it tells the broker the
window is free.

- Cost: net zero permanent slots, one transient, one call round trip per spawn (about 0.9 µs,
  `notes/benchmarks/calibration-against-sel4.md`). One more process, estimated at a caretaker's size
  (a stack page, `CARETAKER_STACK_PAGES` of four more, and its tables), about 32 KiB. Estimated, not
  measured.
- No syscall change and no change to `filesystem_contract`. It adds a small protocol between two
  in-tree programs, which rule 7 puts in a crate.
- §102 does not speak to it.
- Reversibility: a protocol between two programs is on the tenet's expensive list, but both parties
  are in-tree and nothing outside them agrees on it, so it is the cheapest of the six to undo.

### Refused

- Holding the windows permanently at today's size: seven slots against one free.
- Shrinking K until it fits: one free slot allows K = 2, the shell and one caretaker, which is
  exactly the concurrency the set grant needs to exceed.

### Costs side by side

| Option | Permanent slots | Per spawn | Memory | Surface |
|---|---|---|---|---|
| 1. Wider table | +7 (of 8 added) | nothing | 64 KiB, every thread | table size, `FAULT_EP_SLOT` |
| 2. Retype per client | 0 | 1 attach rendezvous | 64 KiB per client | wire verb |
| 3. Map by phys | 0 | nothing | none | syscall |
| 4. Sliced run | 0 | 2 syscalls | none extra | new `PageFrame` method |
| 5. Offset in `MAP_INTO` | 0 | nothing | none extra | method argument shape |
| 6. Broker | 0 | 1 call round trip | about 32 KiB, once | two-program protocol |

### Recommendation, and where it stops

Options 2 to 5 touch the syscall surface or a wire format, so under the rule for irreversible forks
this note gives them as options, not a recommendation. Between the two that do not, 6 over 1: 1
spends 64 KiB and the table's headroom on every thread in the machine to fix one process, which is
the representation §102 already refused.

Would we still choose 6 if 4 cost the same? No. Option 4 has fewer moving parts and is the shape
§102 describes; 6's case is only that it avoids new syscall surface. The question for calef is
therefore one method: a `PageFrame` slice (4) or a `MAP_INTO` offset (5). With either, the pool is
one capability and the slot problem is gone.

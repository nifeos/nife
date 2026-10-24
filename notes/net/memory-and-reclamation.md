# The network stack: the aarch64 memory receipt, and the reclamation that answered it

*An appendix to [`notes/net.md`](../net.md), which is the page to read. This file holds how net
servers exhausted the aarch64 test boot, and what reclaiming them cost and returned. It exists to
verify or challenge the main page. A reader who only needs to use the socket contract should not
have to open it. The directory `notes/net/` and this file's stem are provisional names; naming is
calef's.*

*Records cited below: milestone 64 (enough `std` to run somebody else's crate), §16 (object
revocation) and §32 (a supervisor may collect a corpse).*

## The aarch64 test boot has run out of memory, and this is the receipt

That grant half wanted a test of its own, and could not have one. A second `net_stack` spawn costs a
192-page untyped region that nothing ever reclaims (`NET_SERVER_BUDGET_PAGES`), because the
server blocks in its serve loop forever and no supervisor reaps it. With two extra net servers in the
boot, a later test asking for 128 contiguous pages found 137 free frames and no run that long.
The failure surfaced far away, as `time_tests` reporting "no swish program in the initrd archive, or
no memory to wire one", which reads like a packaging bug and is not one.

The fix taken here was to stop over-provisioning. `NET_SERVER_BUDGET_PAGES` was 192 on the
recorded reasoning that `net_stack` caps its heap at 128 pages "so 192 leaves headroom without being
unbounded", which is a margin nobody measured. It is now 128, with `net_stack`'s `HEAP_MAX`
lowered to 96 so the budget still covers the heap's declared worst case with 32 pages for page
tables and clients' frame mappings. Ten net servers per boot at 64 pages saved each is 640 frames
returned, which is six times what the new gate spends. The suite is what proves 96 is enough.

Three facts worth carrying forward, because milestones 54, 55 and 66 will all want more net tests:

- RAM is pinned at 128 MiB and asserted. `memory_map_came_from_the_device_tree` fails on any
  other size, deliberately, because a wrong number there means a misparsed `reg`. So "give QEMU more
  memory" is a decision with a test attached, not a knob.
- The margin before this milestone was about 315 frames, 1.3 MiB, at the end of the aarch64
  boot. One net test spends ~208 of them permanently (192 for the server, 16 for the client), so the
  suite could afford exactly one more net test and not two.
- It is exhaustion, and it was measured, not inferred. A temporary instrument in
  `untyped::create` printed `free=107, largest_run>=96` at the failing allocation, which settles the
  question a total-free number alone leaves open. Worth repeating if this bites again: the
  instrument is four lines and it turned "somebody's test is flaky" into a number in one run.

The fix is the same one `virtio::MAX_DEVICES` has now asked for eight times: reclaim what a dead or
finished service held. It is one piece of work that would relieve both ceilings, and it is
increasingly what stands between this suite and the next network milestone.

And the margin is now thin enough to fail about one run in three, measured. When milestones 54
and 55 were merged together, the aarch64 leg was run three times on the merged tree. The SMB adapter
and the mDNS half now ride the same spawn as the accept test of milestone 107 (the socket contract learns to accept), which is why neither cost a
net server of its own. One of the three died, and it died exactly the way this section and
notes/swish-language.md both predict. `time_tests::a_shell_with_no_usable_clock_refuses_rather_than_running_it_unmeasured`
printed `refused to load a user program: Unmappable(OutOfFrames)` and the lost-wakeup watchdog fired
sixty seconds later; the other two runs passed all 261 tests with every prober green. Two things
follow, and the second is the one that costs time. A red aarch64 leg on a branch that touches the
net tests is not evidence the branch is wrong, so re-run before you debug. And the failure names
a test that has nothing to do with the change, because the boot's last spawn is the one that pays.
That is what makes this so expensive to diagnose from the failure alone. That is the third distinct
milestone to be misled by it, and it is the argument for reclaim being a milestone rather than a
cleanup.

Postscript, 2026-08-16: that one-in-three did not reproduce on the lane that fixed this, and the
honest reading of that is worth more than a tidier one. Twelve aarch64 runs at the pre-fix commit
under TCG and eight on the physical core (`--hvf`) were all green on one developer's machine. What
*did* reproduce, on every single run, is the margin the paragraph above is really about: 216 free
frames of 29307 and no free run longer than 117. With that little headroom, which test dies is
decided by scheduling. So a rate measured on one machine on one day is a property of the machine as
much as of the tree. Reproduce the margin, not the failure.

## The reclamation landed, 2026-08-16, and here is what it cost and returned

The prediction above was right about the cause and wrong about the size of it: the net services
were the second biggest spender, not the first. Measured across a whole aarch64 boot with the frame
ledger (notes/frames.md), the ten net tests held 2759 frames; the six `spawn_progenitor` tests held
12289. Both are fixed, and the note is corrected rather than quietly updated, because "we knew
which one it was" is exactly the kind of claim this file exists to keep honest.

What a net test now does at the end: `net.release_or_fail("a net test's net_stack")`, which is
`kill_thread` on the server and its clients, then `reclaim_region` on each region, retried. No new
verb, no syscall change.

The one thing that had to change in the kernel, and it is worth understanding here rather than
only in `sched.rs`. `net_stack` blocks in `receive_cap(STACK)` forever. DECISIONS §16's armed kill
is spent by `schedule()`, which a `Blocked` thread never reaches. Killing it did nothing. What ends
it is that reclaiming a region removes the endpoints inside it and aborts whoever is blocked on
them (§32's endpoint reap). That sweep now runs *before* the live-thread refusal instead of
after it. So the server's three endpoints come out of a four-page region of their own, and reclaiming
that region is what wakes it up to die. From `create_endpoint`'s shared kernel chunks there is no such
handle, and there was no way to end a `net_stack` short of rebooting the machine.

The measured result for the boot as a whole: 216 free frames at the end became 15307, and the
longest free run went from 117 to 14080. (15249 on the merged tree, which runs one more process in
the SMB test; notes/frames.md carries the remeasurement. The free run, which is the number that
decides whether a boot lives, is 14080 either way.)

Two things this does not fix, both recorded rather than papered over:

- `virtio::MAX_DEVICES` is untouched. `virtio::register` bumps a counter and never reuses a slot,
  so the receipts in that constant's comment still stand: the boot's ceiling is still "how many
  devices has this boot ever wired". Reusing a slot safely needs a generational name on the device
  table, because a stale `Object::Virtio` capability must not alias a fresh device. That is a
capability-semantics change rather than a counter change. Its own lane. Milestone 64 paid the
  ninth receipt (33, for the listener gate's second `net_stack`) and confirmed the other half of
  the prediction from the other side. The memory ceiling really is gone, so what is left is only the
  counter.
- The DMA page and the shadow ring are deliberately not returned, ~2 frames per net service. The
  NIC still holds whatever receive buffers the dead driver posted, and handing those pages back to the
  allocator would let a live device write into somebody else's memory. Ending that safely means
  resetting the device at teardown, through the transport seam. Also its own lane.

## The server's own limits

`components/src/net_stack.rs` lists these as one line each in its `BUGS`. This is the long form.

**No host fuzz target reaches the dispatch** (proposal #1592 part a, rank 2). The request match and
every helper under it live in the EL0 binary. They take `virtio_net_transport::VirtioNet`, the clock
and two blocking waits directly, so nothing builds for the host. Measured by reading on 2026-10-04
(UTC), a host part would move about 630 lines into a sans-IO crate over smoltcp's `phy::Device`,
with the clock and the waits behind an edge trait. `notes/fuzzing-the-services.md` has the pricing.

**One client can take every socket.** The table is `SOCKETS` (sixteen) entries shared by every
client of one stack, and nothing bounds how many one client holds. A client that opens sixteen and
keeps them starves the rest of the network. A per-client quota wants the client named, which a
badged front door per client would do, and nothing builds that yet. Milestone 649 (every client of
a network stack shares its socket numbers) recorded it, and its confinement test runs the hostile
second client, `socket_squatter`.

**DHCP has no bound.** The lease loop runs until a server answers, and every spawner blocks on the
report. So a link that is up with no DHCP server behind it is a wait with no end. The kernel's
`e1000e_service` and `designware_ethernet_service` refuse a link that never came up
(`Absent::NoLink`, `NotStarted::NoLink`). Every test runner provides slirp's server. Only a live
link to a LAN with no DHCP server reaches the hang.

**A spawn without slot 8 cannot reuse a socket's page.** With no address space to `UNMAP` from, a
closed socket's page stays mapped. Its table entry then refuses every later frame rather than show
the next socket the last one's page. After sixteen sockets with pages, the stack carries no more
bytes. Every spawner in the tree grants slot 8.

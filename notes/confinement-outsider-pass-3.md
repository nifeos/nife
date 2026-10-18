# Confinement, a third outsider pass (milestone 633, provisional name)

Milestone 633 (an outside agent attacks the confinement claim) is the experiment this note records.

This is an outside review of the 33 confinement claims in `notes/confinement-claims.md`'s table.
The reviewer did not read that file's narrative, the audit reports, the fatal-risk notes, the
roadmap, the earlier outsider notes, or any git history. The rule was to assume each claim is false
and look for the case that makes it so. An attack counted only when it booted.

**Result: one booted escape, at the application boundary. The kernel's own confinement held.** The
escape is fixed; see its section. The capability core held, as did the rights gate in
`syscall::invoke`, the paging gates, the DMA shadow ring, the port bitmap and the filesystem subtree
scope. The escape is a confused deputy in the swap demonstrator. A client holding only `WRITE` on a
server's endpoint made the server write a byte into a device register page. The client was never
granted that page. The server's `OPERATION_PUT` handler wrote at a client-chosen offset with no
bound. It booted red on aarch64 and riscv64. It is the server's bug, not a kernel gate failure. It is
still a real reach to an ungranted object, so it is an escape of the end-to-end claim. Beside it are
four near misses. Each is in a demonstrator, or in a test that stays green while its property breaks.

## What I added and booted

`system_tests/src/user/confinement_attack_tests.rs` holds three tests driven through the real
dispatcher (`syscall::invoke`). All three are green on aarch64, riscv64 and x86_64, with zero frames
leaked.

- `a_write_only_rendezvous_holder_cannot_receive_reap_or_survey` drives `RECEIVE`, `RECEIVE_CAP`,
  `REAP` (all `READ`) and `SURVEY` (`ENUMERATE`) against a `WRITE`-only rendezvous capability. A
  sender is already parked, so a gate let open returns a message rather than hanging. It covers
  claim 26's kernel gate and the kernel half of claims 6 to 9.
- `a_read_only_holder_cannot_send_and_an_ungranted_slot_names_nothing` drives `SEND` against a
  `READ`-only capability (refused) and invokes an ungranted slot (`NoSuchSlot`). Claim 2.
- `a_grant_less_budget_mints_a_grant_bearing_frame` retypes a page out of a `WRITE`-only budget. It
  asserts the returned frame carries `Rights::ALL`. That pins claim 3's retype gap so it cannot
  close unnoticed.

The escape's test is `live_swap_tests::a_confined_client_drives_the_server_to_write_past_its_log`.
It drives a new `ROLE_DEPUTY` channel in `components/src/swapper.rs` and a new `ROLE_CONFUSED` role
in `fixtures/src/chatty.rs`. `crates/hot_swap_contract` carries the new constants. It was opt-in and red
when this pass reported; it is a default test now that the fix has landed.

## Independence

I did not open these: `notes/confinement-claims.md` below its claims table, `design/audit-reports/`,
`design/fatal-risks/`, `design/roadmap/` (including `proposals/`), any `notes/*outsider*` or
`notes/*falsif*` file, git log, blame, or GitHub. Source, code comments, `BUGS` sections,
`design/decisions/` and how-to notes were fair game. Three encounters are worth recording, all in
source. `kernel/src/cap.rs:609-619` carries a `BUGS` note from this milestone's second pass (claim
3). `system_tests/src/user/receive_cap_attack_tests.rs` records earlier passes' fixed escapes. Some
source comments name forbidden files by path, and none of those files was opened.

## The claims

Outcome is one of escape, near miss, held, untestable. Grade is the strongest evidence behind it.
`booted <isa>` means observed under QEMU on that ISA this pass. `host` means a host-crate suite ran
green this pass. `read` means source review only.

| # | Claim | Outcome | Grade | Test / evidence |
|---|---|---|---|---|
| 1 | A derive holds no more than its source | held | host | `capability::derive_never_widens_rights` (ran green) |
| 2 | Userspace cannot forge a right from a register | held | booted aarch64/riscv64/x86_64 | `confinement_attack_tests::a_read_only_holder_cannot_send_and_an_ungranted_slot_names_nothing` |
| 3 | A non-delegating budget cannot split a delegating child | near miss | booted aarch64/riscv64/x86_64 | `confinement_attack_tests::a_grant_less_budget_mints_a_grant_bearing_frame`; `split_never_widens_rights` (host) |
| 4 | A consumed capability cannot be reused | held | host | `capability::a_deleted_capability_stays_deleted` (ran green) |
| 5 | Dropping one capability spares the others | held | host | `capability::delete_touches_only_its_slot` (ran green) |
| 6 | A supervisor cannot reap a corpse it does not supervise | held | host, booted aarch64/riscv64/x86_64 | `capability` reap suite (host); supervision suite ran green on every ISA |
| 7 | A refusal about a stranger's thread discloses nothing | held | host, booted aarch64/riscv64/x86_64 | `capability::a_stranger_reveals_nothing_about_its_liveness`; survey/reap suites ran green |
| 8 | A view shows exactly the rendezvous's own children | held | host, booted aarch64/riscv64/x86_64 | `capability::a_survey_shows_exactly_the_endpoints_own_children`; survey suite ran green |
| 9 | What a supervisor may see and reap are one domain | held | host, booted aarch64/riscv64/x86_64 | `capability::the_view_and_the_reap_have_the_same_scope`; suites ran green |
| 10 | A user VA is low-half and page-aligned, every ISA | held | host | `paging` user-VA gate suite (ran green) |
| 11 | No page is both writable and executable | held | host | `paging` W^X suite (ran green) |
| 12 | An IOMMU entry sets no reserved bit | held | host | `paging::x86_64` vtd suite (ran green) |
| 13 | A device cannot touch memory outside its region | held | host | `direct_memory_access_validator` suite (ran green) |
| 14 | A driver cannot send its device to unvalidated descriptors | held | host | `direct_memory_access_validator::an_accepted_descriptor_is_confined` |
| 15 | A driver cannot make the validator walk out or forever | held | host | `direct_memory_access_validator` walk/batch suite |
| 16 | One queue's validation cannot touch another's rings | held | host | `direct_memory_access_validator::distinct_queues_occupy_disjoint_blocks` |
| 17 | A descriptor changed after validation cannot reach the device | near miss | host, read | device reads the shadow (`virtio.rs:956-959`); the test is vacuous, see below |
| 18 | A wiring plan grants no unasked right | held | host | `component_plan` suite (ran green, 31 tests) |
| 19 | A directory capability reaches its subtree and nothing above | held | host, booted aarch64/riscv64 | `filesystem_protocol` (66) and `subtree_scope` (7) ran green; `dir_capability_tests` ran green on aarch64/riscv64 (x86 has no disk); cited proofs cover rights bits only, see below |
| 20 | A memory-unsafe C component faults and changes nothing outside its grant | held | booted aarch64/riscv64 | `c_seam_tests::...` ran green; x86 skips (no C program for `x86_64-unknown-none`) |
| 21 | A user program cannot read a kernel address, every ISA | held | booted aarch64/riscv64/x86_64 | `tests::a_user_program_cannot_read_a_kernel_address`, `..._el0_cannot_read...`, `riscv_virtio_tests::the_page_tables_say_u_mode_cannot_read...` |
| 22 | An ELF cannot load over the kernel or ask for a W+X page | held | booted aarch64/riscv64/x86_64 | `tests::an_elf_that_asks_to_be_loaded_over_the_kernel_is_refused` and sibling, ran green |
| 23 | The progenitor cannot rebuild after dropping construction authority | held | booted aarch64/riscv64/x86_64 | `authority_tests::the_root_supervisor_drops_its_construction_authority_and_cannot_build_again` |
| 24 | Two shells with different roots cannot name each other's files | near miss | booted aarch64/riscv64 (the sequential test); read (the reuse leak) | `shell_navigation_tests::two_shells_...` ran green; a reachable window-reuse leak, see below |
| 25 | A client cannot reach its neighbor's pixels or read the screen | near miss | read | `compositor_tests` suite; harness-policy and a respawn scrub gap, see below |
| 26 | A client of a rendezvous cannot become its server | escape | booted aarch64/riscv64 (x86 skip) | kernel gate held (my gate test, all ISAs); `live_swap_tests::a_confined_client_drives_the_server_to_write_past_its_log` was red; fixed since, see below |
| 27 | A non-holder cannot touch a port; ports do not leak across a switch (x86_64) | held | booted x86_64 | `x86_port_tests::port_holder_transmits_then_a_non_holder_faults` (ran green) |
| 28 | A revoked port holder faults on its next in/out (x86_64) | held | booted x86_64 | `x86_port_tests::a_revoked_holder_faults_on_its_next_port_write` (ran green); the test does not check the fault pc, see below |
| 29 | A thread that deletes its own port cap faults on its next in/out (x86_64) | held | booted x86_64 | `x86_port_tests::a_holder_that_deletes_its_port_capability_faults_on_its_next_port_write` (ran green) |
| 30 | A revocation reaches a capability in flight | held | booted aarch64/riscv64/x86_64 | `revocation_in_flight_tests::a_capability_revoked_while_it_is_in_flight_does_not_reach_the_receiver` |
| 31 | An unvouched child holds no undelegated capability | held | read | `script/swish-check` installed/unvouched census |
| 32 | The boot shell holds no display device | held | read | `script/swish-check` caps census |
| 33 | No WRITE, no x86_64 port I/O | held | booted x86_64 | `x86_port_tests::a_read_only_port_capability_must_not_grant_port_output` (ran green) |

Counts: 1 escape, 4 near miss, 28 held, 0 untestable.

## The escape

### Claim 26: a confined client drives the server to write into a device page it never held

The kernel gate holds. `a_write_only_rendezvous_holder_cannot_receive_reap_or_survey` proves it on
all three ISAs: a `WRITE`-only client is refused `RECEIVE`, `RECEIVE_CAP`, `REAP` and `SURVEY`. So it
cannot become the server. But `swap_protocol::serve` handled `OPERATION_PUT` with
`log_put(log_base + arg, version)`, where `arg` is the client's own request word. `log_put` was an
unbounded `write_volatile` at `LOG_VA + seq`. So a client holding only `WRITE` on the stable endpoint
chose where the server wrote. At offset `DEV_VA - LOG_VA` the server wrote the version byte into the
first register of its device page. The client holds no capability to that page. The server replied
and echoed the offset.

The booted test drives this through the real harness. A new `ROLE_DEPUTY` channel starts one server
holding the device and the log. A new `ROLE_CONFUSED` client, with only the honest client's
capabilities, makes the call. It was red on aarch64 and riscv64: the server echoed tag `0x100100000`
for offset `0x100000`. x86_64 skips, having no device page. The kernel gave the client exactly
`WRITE` on an endpoint; the unbounded write was the server's. This pass did not write the fix.

**Fixed by the maintaining lane, 2026-10-06 (UTC), after this pass reported.** `swap_protocol::log_put`
now refuses an offset outside the one log page. The server answers `PUT_REFUSED` (name
provisional). The test is no longer opt-in, and it asserts the refusal. Its replayable falsification
deletes the bound and went red at the `CONFINEMENT ESCAPE` assertion on aarch64. With the bound it
is green on aarch64 and riscv64. The server also honors `OPERATION_QUIESCE` from any client. That is
a denial of service rather than a reach, and its fix changes the protocol. So it is a `BUGS` line at
the arm in `swap_protocol`, not a change.

## The near misses

### Claim 3: a spend-only budget still mints delegable frames

`memory_region_cap` hands a child a `WRITE`-only untyped, with no `GRANT`. `SPLIT` honors that:
`Cap::mint_child` copies the parent's rights, so the child gets no `GRANT` either. That is claim 3 as
stated, and `split_never_widens_rights` proves it. But `MemoryRegion::RETYPE` needs only `WRITE`. It
returns the new `PageFrame` with `Rights::ALL`, `GRANT` included (`kernel/src/syscall.rs:829-845`).
So a holder of a spend-only budget can retype a page and `SEND_CAP` it onward.
`a_grant_less_budget_mints_a_grant_bearing_frame` asserts exactly this at boot on all three ISAs. It
is not an escape, because the frame comes from the holder's own budget. Whether a spend-only budget
should withhold `GRANT` from its retypes is owed a claim or a ruling.

### Claim 17: the device reads the shadow, but the test proves only that two arrays are two arrays

The property holds in its narrow form. A virtio queue's descriptor table and avail ring point at a
kernel-private shadow (`kernel/src/virtio.rs:956-959`). The device never reads the driver's writable
copy, so changing a descriptor after validation cannot steer it. The regression test does not prove
this. Both the Kani harness and the kernel twin write the driver's frame and read back the shadow
frame. Those are separate allocations, so the final assertion cannot fail. Nothing checks the line
the claim rests on: that the device's queue registers point at the shadow.

There is also a real race, which the crate's own `BUGS` names. The shadow descriptor is published as
two unbarriered stores into a slot the device may still own. A device reading mid-publish could pair
a new address with a stale length and read past the region. QEMU's block completion is synchronous,
so the race is not reachable there and stays read. It wants a lane that barriers the publish or
shadows into an idle slot.

### Claim 24: window 0 is safe, but a reused FS window leaks file replies between concurrent jobs

The headline test runs its two shells one at a time, because harness clients share one window page.
The kernel property it proves still holds: neither shell holds a capability reaching the other's
subtree. Production wiring is the interesting part. Window 0 is shared only among root-authority
clients: the shell, login and the progenitor. The identity provisioner is not concurrent with them.
So no confined client is ever on window 0.

The reachable gap is windows 1 to 6, the pool for directory-granted jobs. `Windows::take` hands them
out round robin and never checks whether the last holder is alive
(`crates/system_initializer/src/lib.rs`, with its own `BUGS`). A new job can get a window that an
older, running job still maps. Reuse zeroes and re-slices the page but does not remove the old
mapping. The server writes a `READ` reply into the window the requester's badge names
(`redoxfs_server/src/dispatch.rs`). So the new job's file bytes land in a page the old job can read,
with no capability to the new job's subtree. The code's `BUGS` says the fix needs a "job reaped"
signal that does not exist yet. This was settled by read. Forcing the reuse needs the production job
pool, a RedoxFS disk and seven concurrent jobs, which is beyond a targeted boot.

### Claim 25: the confinement is a test-harness spawner, and a respawn would show stale pixels

A compositor client holds capabilities for its own control page and surface frames, nothing else.
The screen capability goes only to the capture role, and `compositor_tests` runs green. Three
caveats keep it a near miss. The spawner is called only from `system_tests`, so this is a property
of a kernel-side harness. The tests check a narrow set of addresses and slots. And the spawner maps
a slot's frames without scrubbing them (its own `BUGS`), so a client respawned into a used slot would
see the last client's pixels. No shipped path respawns a compositor client, so the gap is latent.

## What I could not do

- The DMA publish race (claim 17) and the window-reuse leak (claim 24) were settled by read, for
  the reasons in each section.
- The compositor respawn gap (claim 25) is latent. Booting it needs the registration handshake re-run
  on a respawn, which this pass did not build.
- Kani proofs were not re-run, because `script/verify` is a heavy job. Rows that cite milestone 194
  (the falsification record, its lint, and the sweep that replays it) were graded by the host tests
  that ran green beside them.
- Claim 28's test does not assert the fault pc, so a fault anywhere would pass it. The property held
  under source review, so the row stays held.

## Where each finding lives

A note is not a home, so each finding is recorded where a reader meets the code, or with an owner.

- Claim 26's confused deputy: fixed in `swap_protocol::log_put`, with the test and falsification
  above. The quiesce half: `BUGS` at the `OPERATION_QUIESCE` arm of `swap_protocol::serve`.
- Claim 3's retype `GRANT`: `BUGS` on `cap::memory_region_cap`, already there. Whether a spend-only
  budget should withhold `GRANT` from its retypes is an architect's question, carried in the
  milestone 633 block.
- Claim 17's vacuous test: `BUGS` on `the_shadow_ring_is_immune_to_a_descriptor_mutated_after_validation`
  in `kernel/src/virtio.rs`. The publish race: the crate's own `BUGS` and the existing proposal
  `the-shadow-descriptor-is-published-in-two-stores`.
- Claim 24's window reuse: `BUGS` on `system_initializer`'s `Windows`, already there. The fix is the
  "job reaped" signal of milestone 685 (a job is finished when its memory is back), which does not
  yet say it closes a confinement gap; the 633 block asks the maintainer to add that.
- Claim 25's stale pixels: `BUGS` on `compositor_service::Wiring::spawn_client`, already there.
- Claim 28's fault pc: `BUGS` on `a_revoked_holder_faults_on_its_next_port_write`.

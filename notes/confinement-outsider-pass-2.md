# A second outsider pass over the 30 confinement claims

Milestone 633 (an outside agent attacks the confinement claim), lane/633-outsider-2, 2026-10-05
UTC. Name provisional.

An independent attack on the 30 claims in [notes/confinement-claims.md](confinement-claims.md). The
briefing was two things: that file and the source. The reviewer did not read the audit reports, the
earlier falsification notes, the 633 roadmap block, the fatal-risks directory, lane reports or
security-fix commits, so the conclusions here come from the claims and the code, not from a prior
pass. The posture is risk 7's: assume a claim is false and look for the case that makes it so. Where
a case could be run it was run under QEMU; where it could only be read it is marked as read, a
weaker grade.

Nothing here supports "the confinement holds". It supports the narrower sentence: these are the
attacks that were tried, and this is where each landed.

## Counts

- Escape: 1 (row 27's neighbor, port I/O ignores the capability's rights). Booted red on x86_64.
- Escape of an unclaimed surface: AMD-Vi, which has no claim at all.
- Near miss: rows 3, 5, 6, 11, 17, 19, 24, 30.
- Untestable here (needs a hostile-client boot or hardware this project lacks): the userspace halves
  of 19, 20, 23, 24, and the AMD-Vi and VT-d silicon findings.
- Held: 1, 2, 4, 7, 8, 9, 10, 13, 14, 15, 16, 18, 21, 22, 25, 26, 28, 29.

A claim can be Held against the attack that was run and carry a near miss against a path its own test
cannot see; the detail sections say which is which.

## One row per claim

"read" means reasoned from the code, not booted.

| # | Attack | Outcome | Evidence |
|---|---|---|---|
| 1 | Derive a cap that gains a bit its source lacks | Held | `from_bits` masks `& ALL`; `is_subset_of`/`allows` sound; kernel uses `Delegation::derive` (GRANT + subset). The Kani proof targets `CapabilityTable::derive`, which the kernel never calls. |
| 2 | Forge a right from the syscall register | Held | `from_bits(aN as u32)` masks `& ALL`; the `u32` cast drops high bits, which can only remove rights. |
| 3 | No-GRANT budget produces a child that can delegate | Near miss | SPLIT is sound (`mint_child` copies rights). `RETYPE`/`RETYPE_OBJ` need only WRITE and mint the new object with `Rights::ALL`, GRANT included, which SEND_CAP then passes on. The `cap.rs` comment "GRANT never appears anywhere it was not present at the root" overstates this. |
| 4 | Reuse a consumed capability | Held | `delete` empties the slot; a later `get` is `NoSuchSlot`. |
| 5 | Disturb one cap by dropping another | Near miss | On x86 `delete_current_cap` of a `PortRange` clears the thread-wide `port_range_grant` with no sibling check, so deleting one of two copies disables the other. Fails safe. |
| 6 | Reap a corpse you do not supervise | Held, near miss | `reap_supervised` gates `NotSupervised` before `reclaim_region`. But `reclaim_region`'s liveness check is per named tid, so a co-resident thread in the same TCB region is reclaimed before the live-resident bail. Needs several TCBs in one self-owned region, so no cross-component authority. Already recorded in `reclaim_region`'s BUGS. |
| 7 | Learn a stranger's liveness from a refusal | Held | `reap_decision` returns `NotSupervised` for a non-supervised tid regardless of `dead`. |
| 8 | Show a supervisor a thread that is not its child | Held | `survey_includes` is `fault_ep == ep`. A builder may point its own child's `fault_ep` at a victim's rendezvous, but exposes only its own thread and gains nothing. |
| 9 | Make view and reap disagree in scope | Held | Both use `fault_ep == ep`; one decision function. |
| 10 | Admit a high-half or unaligned user VA | Held | `is_user_page_va` is `top == 0 && is_multiple_of(4096)`; `SPLIT_SHIFT` 48/38/47; no boundary off-by-one. |
| 11 | Encode a page writable and executable | Held, near miss | ELF refuses W+X; `map_segments` maps exec as `user_code`. W^X lives only in the `Flags` constructors, not the encoders, and the W+X test skips `write_combining`. Not reachable from userspace today. |
| 12 | Set a reserved bit in an IOMMU entry | Held (VT-d); gap (AMD-Vi) | `VTD_PERMITTED_BITS` proved over every u64. AMD-Vi's DTE has no literal guard and no Kani harness (see its section). |
| 13 | Point a device outside its region | Held | `is_in_region` uses checked adds and `end <= limit`. |
| 14 | Send a device to an unvalidated descriptor | Held | `is_descriptor_allowed` refuses indirect first. |
| 15 | Walk outside the rings, or forever | Held | Batch bounded `<= qsize`; chain walk bounded by `qsize`. |
| 16 | One queue's validation touches another's rings | Held | Distinct shadow blocks. |
| 17 | A post-validation change reaches the device | Near miss | `shadow_one_head` writes the shadow descriptor as two non-atomic `write64`s with no barrier and no idle-slot check. A device reading a republished live slot sees a new `addr` with an old `len`. virtio-mmio has no IOMMU, so the validator is the only boundary. The Kani model treats table and shadow as disjoint and cannot see it. |
| 18 | Grant a right the declaration did not ask for | Held | Literal `Serve => READ`, `Use => WRITE`. |
| 19 | Name a file above a directory cap's subtree | Held (filter); near miss elsewhere | `check_component` is byte-exact and refuses `.`, `..`, `/`, `\`, `:`, NUL and the attribute store. The near misses need a hostile-client boot: a name checked then re-read from the client's window (TOCTOU); a directory handle that dangles after `rmdir` and names whichever directory next takes the freed tree id. |
| 20 | A C component changes memory outside its grant | Held; test-integrity near miss | The confined C holds `REPORT` with WRITE, the channel the verdict arrives on, so it could forge a `CONFINED` verdict. |
| 21 | Read a kernel address from user mode | Held | aarch64 asks `AT S1E0R`; riscv walks the U bit. The claims note records that x86 has no `user_can_read` and SMAP is off. |
| 22 | Load an ELF over the kernel, or a W+X page | Held | ELF refuses W+X and overlap; `check_image_band` plus a `Half::Low` mapper refuse the over-kernel case per page. |
| 23 | Rebuild after dropping construction authority | Held, evidence near miss | Proven on `root_supervisor`, not the shipped `system_initializer`; the verdict channel is forgeable. |
| 24 | Two shells name each other's files | Held (filter); same TOCTOU near miss | `check_component` plus `subtree_scope::walk`, DESCEND per hop. |
| 25 | Reach a neighbor's pixels or the screen | Held | `commit_damage` intersects `win.bounds()`; no client-controlled source index found. |
| 26 | A client of a rendezvous becomes its server | Held | RECEIVE and RECEIVE_CAP both require `Rights::READ`. |
| 27 | A thread with no port cap touches a port | Held; escape of a neighbor | A port cap narrowed to READ still drives the hardware: `thread_control_block_insert_from` installs `port_range_grant` for any `PortRange` object whatever its rights. Failing test and proposed claim below. **Fixed by milestone 768 (provisional) as claim 33** (31 and 32 were taken by then). |
| 28 | A revoked port holder keeps the ports | Held | `delete_port_range_caps_impl` matches `(base,count)`, clears the grant and `outgoing_cap`. |
| 29 | A thread keeps ports after deleting its cap | Held, row-5 near miss | Self-delete clears the grant; the sibling-copy case is row 5. |
| 30 | A revocation misses a cap in flight | Held, near miss | The three sweeps clear `outgoing_cap`. But `depart`, `finish_blocked_resident` and `reap_region_objects` do not, safe only because a running thread holds it `None`. A `PageFrame` slice is a distinct object that survives a revoke of its parent run. That one is deliberate, option B of §132 (what `PageFrame::REVOKE` owes an overlapping run), and recorded in `revoke_page_frame_run`'s BUGS. |

## The escape: port I/O ignores the capability's rights

Rows 27 to 29 are about holding, revoking and deleting a `PortRange` capability. None asks whether
the capability's rights gate the `in`/`out`. The rest of the tree treats WRITE as the right that
drives a port: `build_child`'s comment calls WRITE "the rights a driver gets", and `component_plan`
maps a `Use` component to WRITE and a `Serve` component to READ. A thread handed a READ-only
`PortRange`, the shape a `Serve` component gets, should fault on `out` as a thread with no capability
does.

It does not. In `thread_control_block_insert_from`:

```rust
#[cfg(target_arch = "x86_64")]
if let crate::cap::Object::PortRange(base, count) = cap.object {
    t.port_range_grant = Some((base, count));
}
```

There is no rights check. The grant is keyed on `(base, count)` only, so a READ-only (or rights-0)
`PortRange` opens the TSS I/O bitmap and the hardware permits the `out`. A supervisor that hands a
component a READ view of a port range, to observe and not drive, finds the component drives the
hardware anyway. On x86 a port write is a device command, so this is the authority to act.

The failing test is `x86_port_tests::a_read_only_port_capability_must_not_grant_port_output`,
committed before any fix. It builds a child whose only `PortRange` is READ, runs one `out` through
the new `port_out_then_exit` stub, and asserts the child faults. The stub is used instead of
`port_out` so a wrongly-permitted `out` exits rather than parking on a SEND and hanging the run, the
row-26 hazard. Booted red on x86_64: the supervision message was `[EVENT_EXIT, ..]`, meaning the
`out` was permitted, where the secure behavior is `EVENT_FAULT`. It is opt-in (skips unless named
with `--test`) so the default suite stays green, since whether a non-WRITE `PortRange` should deny
I/O is the x86 port syscall surface and so an architect's call. Run it with
`script/test --arch x86_64 --test a_read_only_port_capability`.

Proposed claim 31: port I/O honors the capability's WRITE right; a thread whose only `PortRange`
capability lacks WRITE faults on its next `in`/`out`, on x86_64. It is x86-only for the reason rows
27 to 29 are: the mechanism is the TSS I/O bitmap.

## Near misses

Row 3, the retype path. A spend-only region (WRITE, no GRANT) is meant to be a budget that cannot
delegate. It can still RETYPE a page into a `PageFrame` with GRANT and SEND_CAP it onward. This is
ordinary capability semantics and likely by design, but the claim's prose and the `cap.rs` comment
do not carry the caveat, and claim 3's test exercises only SPLIT.

Rows 5 and 29, the thread-wide port grant. `port_range_grant` is a single value, not a per-cap
record. Holding two copies and deleting one clears the grant both relied on. It only harms the
holder, so it fails safe, but "dropping one capability does not disturb the others" is false on x86
as stated.

Row 6, reclaim before the liveness bail. Once permitted, `reap_supervised` calls `reclaim_region` on
the whole TCB region, and `reap_region_objects` destroys its rendezvous, notifications and timers and
marks residents killed before noticing a still-live co-resident. No authority crosses a component
boundary, since the region is one component's own budget. A bounded "anything live here?" check
before the first destructive step would close it.

Row 17, the torn shadow descriptor. The strongest pure-logic finding; see the table. The validator
proves a property of its design, and the one that could regress, the atomicity of the shadow publish
against a concurrent device read, is the one the harness cannot model.

Rows 19 and 24, the FS badge window reuse. `subtree_scope::Bindings::bind` permits re-binding a
`Revoked` badge, the intended way to recycle a window. The hazard is one layer up:
`system_initializer::Windows::take` hands out window numbers round robin and never learns when a job
dies, and the old holder's badged endpoint is not severed at the kernel level. A job that stays alive
while the pool wraps finds its badge rebound to a later job's directory root and reads that job's
files. This needs a hostile-client boot to demonstrate.

Row 30, the fields a sweep forgets. The live claim holds; two adjacent limbs are one bug away.
`depart`, `finish_blocked_resident` and `reap_region_objects` never clear `outgoing_cap`, safe only
because a running thread's is `None`. A `PageFrame` slice is a distinct object whose capability and
mapping survive a revoke of its parent run.

## Where each finding lives

A note is not a home. Each finding is recorded where a reader meets the code, or as a proposal:

- Row 27's escape: `design/roadmap/proposals/a-read-only-port-range-still-drives-the-hardware.md`
  (the defect, the failing test, and the design question), plus a `BUGS` line at the grant-install
  site in `sched::thread_control_block_insert_from`.
- AMD-Vi: five entries in `amd_vi.rs`'s module `BUGS` (the devfn fault was already there), a
  pointer in `notes/amd-vi.md`, and
  `design/roadmap/0767-amd-vi-hardening-before-the-first-amd-boot.md` with the exclusion
  range, alias quarantine and read-only IVMD as its acceptance items.
- Row 17: `BUGS` in `direct_memory_access_validator`'s module doc and on `shadow_one_head`, and
  `design/roadmap/0799-the-shadow-descriptor-is-published-in-two-stores.md`.
- Row 3: `BUGS` on `cap::memory_region_cap`. Row 5: `BUGS` on `sched::delete_current_cap`. Row 11:
  `BUGS` on `page_tables::Flags`. Row 30: `BUGS` on `Thread::outgoing_cap`.
- Rows 19 and 24: a `BUGS` section in `redoxfs_server/src/dispatch.rs` for the window TOCTOU.
  Two existing records were corrected rather than added to. `Server::rmdir`'s doc said a dangling
  handle fails with `ENOENT`, and `system_initializer`'s `Windows` `BUGS` said a job whose window
  is reused loses its grant. Both now say what the code does.
- Row 6 was already in `reclaim_region`'s `BUGS`; the slice half of row 30 in
  `revoke_page_frame_run`'s.
- Claims 31 to 33 stay here as proposals; `notes/confinement-claims.md` is not edited.

## Untestable here

The userspace halves of 19, 20, 23 and 24 were read, not driven from a hostile client. Making them
testable is a fixture and a boot: a client that keeps a badged FS endpoint across a window-pool wrap;
a C component that forges a verdict on the WRITE channel it holds; a `system_initializer` instance
rather than `root_supervisor`. These are the "caretakers were read, not attacked" shape the claims
note flags for outside eyes. AMD-Vi and VT-d silicon findings fault a transaction in ways QEMU does
not reproduce, and xenon is not yet exercising the IOMMU.

## AMD-Vi: a whole IOMMU with no claim

`NIFE_IOMMU=amd` selects a second x86 IOMMU driver (`kernel/src/arch/x86_64/amd_vi.rs`, page tables
in `crates/page_tables/src/x86_64.rs` `AmdVi`), chosen when the firmware publishes an IVRS and no DMAR.
Nothing in the claims file covers it. Findings, strongest first, all read, not booted:

1. The firmware exclusion range is never cleared. The driver does not write the Exclusion Base/Limit
   MMIO and preserves firmware Control bits it does not clear. On silicon a firmware exclusion range
   with ExEn, especially with Allow, lets every device reach that range untranslated whatever the DTE
   says. QEMU cannot show it.
2. Alias entries are shared and never quarantined. `attach` writes both the requester id and its
   alias source to one DTE, so a second attach puts the first device's aliased DMA into the second's
   domain, and `quarantine` resets only the requester id.
3. Every DMA mapping is read-write. A firmware IVMD marked read-only becomes writable, and the
   kernel-private virtio shadow page is mapped writable to the device.
4. No revocation in production. `quarantine` is dead code outside tests; a device keeps its domain
   for the whole boot, and re-attach leaks the previous domain's tables.
5. The DTE encoder has no literal reserved-bit guard and no Kani harness; the root address is masked
   silently rather than asserted.
6. Faults are attributed from devfn alone under QEMU, so a device off bus 0 is blamed on the wrong
   requester id and the escape tests prove less than they appear to.

Milestone 767 (AMD-Vi hardening before the first AMD boot) closed items 1, 2, 3 and 5 on
2026-10-05 (UTC), with what QEMU could not show recorded in `amd_vi.rs`'s BUGS; 4 and 6 stand.

Proposed claims:

- Claim 32: an AMD-Vi DTE and I/O page-table entry set no bit the hardware treats as reserved. A
  Kani harness over every u64, with the builders moved into a crate and a literal permitted-bits
  mask, mirrors `no_vtd_entry_ever_sets_a_reserved_bit`.
- Claim 33: a device behind AMD-Vi touches nothing outside its granted region, through its own id or
  any alias, and a device nobody confined reaches nothing. It must cover the alias entries and the
  exclusion registers.
- Revocation, on all IOMMUs: a quarantined device reaches nothing through its id or any alias.

## BUGS

- Most verdicts are read, not run. Only row 27's escape was booted. Every Held and every near miss
  marked "read" is reasoned from the source as it stood on 2026-10-05 and will rot as the code moves.
- The escape's failing test is x86_64 only and opt-in, so the default suite stays green and `--sweep`
  will not replay it. That is deliberate: it pins a live defect whose fix is an architect's call.
- The hostile-client boots were not built. The FS badge reuse, the forgeable verdict channels and the
  shared-window TOCTOU are the findings most likely to be real escapes, and the right size for a
  dedicated fixture-and-boot milestone.
- The compositor, network stack and swap path were not attacked from a hostile client either. Their
  claims are Held on a reading of the enforcement, the weakest grade here.

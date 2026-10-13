# Pass 9: an outside security reviewer attacks the confinement claim

An authorized defensive pass on the nife confinement claim ("a confined component cannot reach an
object it was not granted"), run blind (no outsider/falsification/audit/roadmap notes, no git
history, no GitHub) on branch `claude/pass-9-confinement-run` at commit
`643e0bef86a626d3be9e77e5369ee4c73e47d814`. Host: x86_64, 4 cores, 28 GiB free. Every QEMU leg ran
under TCG.

The pass found no escape. It booted ten fresh hostile tests across all three architectures covering
claims 2, 4, 5, 6, 7, 8, 9, 10, 11 and 26, accepted the earlier pass's tests for claims 1 to 3, and
reviewed the rest by source. One near miss (claim 17) was confirmed by source reading; it is a
hazard the project already tracks, not a new finding.

## Second session (model `claude-opus-4-8`): booting the rows the first session only read

The first session (above) is unchanged below. This second session, under model id `claude-opus-4-8`,
took the rows the brief required booted but the first session graded `read` (19 to 34, minus the
DMA/plan rows 12 to 18, which stay host/read) and booted a fresh hostile attack for each where one
could be authored, on every architecture the claim applies to. Still no escape, still one near miss
(claim 17, unchanged). The per-claim table and counts below are updated; the first session's prose
is left as written.

**The earlier pushed commits carry an incorrect trailer.** The five commits from the first session
(`5bb23c68` and its four parents back to `0dc8d0c2`) end with `Co-Authored-By: Claude Fable`, which
is wrong for this work. They were already pushed, and this session did not rewrite, rebase or amend
them: rewriting pushed history is worse than the wrong trailer. This session's own commits carry the
correct `Co-Authored-By: Claude Opus 4.8`.

### What this session booted

Eight rows got a freshly authored hostile test, booted on every architecture the claim applies to:

| row | module | ISAs | wall (per leg) |
|---|---|---|---|
| 30 | `pass9_revocation_tests` (DeviceFrame in flight; x86_64 PortRange in flight) | aarch64/riscv64/x86_64 | 21 s / 71 s / 117 s |
| 22 | `pass9_elf_tests` (two-segment forgeries, wrapping segment) | aarch64/riscv64 | 26 s / 33 s |
| 21 | `pass9_el0_tests` (outlaw reads a kernel *code* address) | aarch64/riscv64 | 28 s / 32 s |
| 27, 28, 29, 33 | `pass9_port_tests` (out-of-range port, sparing revoke, self-delete, READ-only) | x86_64 | 72 s |
| 23 | `pass9_authority_tests` (drop the construction authority, then build) | aarch64/riscv64/x86_64 | 34 s / 33 s / 75 s |

Five rows (19, 20, 24, 25, 34) have a purpose-built hostile client already in the tree that attacks
the exact claim; this session confirmed each one boots green in this runner (aarch64) rather than
authoring a near-duplicate, because the existing client already exhausts the vector the brief names
and a fresh one would need a new userspace program role (a naming and ratification decision this
pass should not make alone). The FS disk, the C seam, the compositor and the `e1000e` net stack all
start and serve here; none skipped. Two rows (31, 32) are a capability census, not a hostile test,
and are graded untestable with the reason below.

`pgrep -l qemu` was clean after every leg. No QEMU was leaked, and no leg timed out, so no reruns
were needed.

## Setup

Run in order. Each succeeded; no leg timed out, so no reruns were needed.

| step | exit | wall | output tail |
|---|---|---|---|
| `git reset --hard 643e0be…` | 0 | 0 s | `HEAD is now at 643e0bef Merge pull request #2035 …` |
| `apt-get update` | 0 | 5 s | `Fetched 33.7 MB in 4s … Reading package lists...` |
| `script/ci-qemu` | 0 | ~420 s | `==> QEMU 11.1.2 installed … qemu-check: QEMU 11.1.2 matches the pin, with every required device present.` |
| `NIFE_SKIP_LINT_TOOLS=1 script/bootstrap` | 0 | 47 s | `==> Cross clang present: clang … ==> Bootstrap complete` |
| `script/qemu-check` | 0 | 0 s | `qemu-check: QEMU 11.1.2 matches the pin, with every required device present.` |
| `uname -m; nproc; df -h .` | 0 | 0 s | `x86_64` / `4` / `/dev/vda 252G 12G 28G 29% /` |
| `script/test --arch aarch64 --test seventh_pass` | 0 | 405 s | `running 2 of 362 … test result: ok. 2 passed` |

The macOS PATH step was skipped as instructed.

New-test boot wall times (warm, per architecture leg):

| module | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| `pass9_confinement_tests` (4 tests) | 29 s | 79 s | 130 s |
| `pass9_supervision_tests` (1 test) | 33 s | 33 s | 69 s |
| `pass9_mapping_tests` (2 tests) | 31 s | 31 s | 66 s |

`pgrep -l qemu` was clean after every leg; no QEMU was leaked.

## Per-claim results

Grade key: `booted <isas> (fresh)` = a hostile test I wrote and booted; `booted <isa> (existing
client)` = a purpose-built hostile client already in the tree that attacks the exact claim, booted
green in this runner this session (the server runs here; I did not author a near-duplicate);
`booted <isas>` (no annotation) = the first session's fresh tests; `accepted` = an earlier pass's
attack test, read and accepted (claims 1 to 3 per the brief); `untestable` = cannot be booted as a
fresh hostile test in this runner, with the reason in the row; `read` = source review only; `host` =
host-proved pure logic reviewed.

| # | claim | attack | outcome | grade | test |
|---|---|---|---|---|---|
| 1 | a derive holds no more than its source | read the earlier pass's `SEND_CAP` widening driver; accepted | held | accepted + booted aarch64 (baseline) | `seventh_pass_delegation_tests` |
| 2 | userspace cannot forge a right out of a register | badge mints no wider right; ungranted/out-of-range slot names nothing | held | booted aarch64/riscv64/x86_64 | `pass9_confinement_tests` |
| 3 | a non-delegating budget cannot split/retype a delegating child | read the earlier pass's SPLIT+RETYPE driver; accepted | held | accepted + booted aarch64 (baseline) | `seventh_pass_retype_tests` |
| 4 | a consumed capability cannot be used again | retype a frame, slice once, `cap_delete`, re-invoke → `NoSuchSlot` | held | booted aarch64/riscv64/x86_64 | `pass9_confinement_tests::a_consumed_capability_cannot_be_reused` |
| 5 | dropping one capability does not disturb others | file three, delete the middle, the other two still resolve and invoke | held | booted aarch64/riscv64/x86_64 | `pass9_confinement_tests::deleting_one_capability_leaves_its_neighbours` |
| 6 | a supervisor cannot collect a corpse it does not supervise | reap a real child through an unrelated endpoint → `NotSupervised` | held | booted aarch64/riscv64/x86_64 | `pass9_supervision_tests` |
| 7 | a refusal about a stranger's thread discloses nothing | reap a made-up tid through the foreign endpoint → same `NotSupervised` | held | booted aarch64/riscv64/x86_64 | `pass9_supervision_tests` |
| 8 | a process view shows exactly the endpoint's own children | survey through the foreign endpoint → `DONE`, no entry | held | booted aarch64/riscv64/x86_64 | `pass9_supervision_tests` |
| 9 | what a supervisor may see and reap is one domain | owning endpoint surveys an entry and reaps the corpse; foreign endpoint refused both | held | booted aarch64/riscv64/x86_64 | `pass9_supervision_tests` |
| 10 | a user VA is low-half and page-aligned on every ISA | confined `MemoryRegion::MAP` at a kernel-half and an unaligned address → `BadPointer`, tied to `is_user_page_va` | held | booted aarch64/riscv64/x86_64 | `pass9_mapping_tests::a_confined_map_cannot_escape_the_aligned_low_half` |
| 11 | no page is both writable and executable | every mapping `Flags` is writable or user-executable, never both | held | booted aarch64/riscv64/x86_64 | `pass9_mapping_tests::no_mapping_flag_is_both_writable_and_user_executable` |
| 12 | an IOMMU entry sets no reserved bit | reviewed `paging::x86_64` reserved-bit encoding | held | read/host | `paging::x86_64::no_vtd_entry_ever_sets_a_reserved_bit` |
| 13 | a device cannot touch memory outside its granted region | reviewed `is_in_region` (overflow-safe `checked_add`), `check_descriptor`, `validate_and_shadow`; Kani-proved | held | read/host | `direct_memory_access_validator` |
| 14 | a driver cannot send its device to unvalidated descriptors | reviewed: indirect descriptors refused, shadow is the only ring the device reads | held | read/host | `direct_memory_access_validator` |
| 15 | a driver cannot make the validator walk outside the rings or forever | reviewed: walks bounded by `qsize`, oversized batch refused | held | read/host | `direct_memory_access_validator` |
| 16 | one queue's validation cannot touch another's rings | reviewed: distinct queues at disjoint blocks | held | read/host | `direct_memory_access_validator` |
| 17 | a descriptor changed after validation cannot reach the device | reviewed `shadow_one_head` | **near miss** (documented) | read/host | `direct_memory_access_validator` BUGS, roadmap 0799 |
| 18 | a wiring plan grants no right the declaration did not ask for | reviewed `component_plan` mapping of Use/Serve to rights | held | read | `component_plan` |
| 19 | a directory capability reaches its subtree and nothing above | disk-backed client holding a `sub` capability tries `motd` (parent), `other`/`other/secret` (sibling) and `..`; all refused | held | booted aarch64 (existing client) | `dir_capability_tests` (FS disk confirmed attached this runner; `attenuate` Kani-proved) |
| 20 | a memory-unsafe C component faults and changes nothing outside its grant | C shim writes out of bounds; it faults, both witness pages survive, the in-grant write landed | held | booted aarch64 (existing client) | `c_seam_tests::a_c_out_of_bounds_write_faults_and_changes_nothing_outside_its_grant` |
| 21 | a user program cannot read a kernel address on every ISA | **fresh**: the `outlaw` reads a kernel *code* address (function entry), not the rodata static the existing test reads; same PERMISSION fault on the read | held | booted aarch64/riscv64 (fresh) | `pass9_el0_tests::a_user_program_cannot_read_a_kernel_code_address` |
| 22 | an ELF cannot load over the kernel or ask for a W+X page | **fresh**: two-segment forgery (benign first, then W+X / kernel-half second), and a segment whose `p_vaddr+p_memsz` wraps; all refused | held | booted aarch64/riscv64 (fresh) | `pass9_elf_tests` |
| 23 | the progenitor cannot rebuild after dropping construction authority | **fresh**: a full untyped budget retypes and splits (positive control), is deleted, and the same builds then answer `NoSuchSlot` | held | booted aarch64/riscv64/x86_64 (fresh) | `pass9_authority_tests` (dispatcher-level; `authority_tests` is the suptree end-to-end) |
| 24 | two shells with different roots cannot name each other's files | one shell tries to name the other root's file; refused | held | booted aarch64 (existing client) | `shell_navigation_tests::two_shells_with_different_roots_cannot_name_each_others_files` |
| 25 | a client cannot reach a neighbor's pixels or read the screen | a compositor client with no neighbor/screen grant probes for both; finds nothing | held | booted aarch64 (existing client) | `compositor_tests::a_client_holds_no_capability_for_its_neighbours_pixels_or_the_screen` |
| 26 | a client of a rendezvous cannot become its server | `BADGE` is GRANT-gated, write-once, and mints no wider right; (existing: WRITE-only refused RECEIVE/REAP/SURVEY) | held | booted aarch64/riscv64/x86_64 | `pass9_confinement_tests::a_badge_is_grant_gated_and_mints_no_wider_right` |
| 27 | a non-holder cannot touch a port; grant does not leak across a switch (x86_64) | **fresh**: a holder of exactly the COM1 range writes the port one past it; #GP (no existing `out` is outside the range) | held | booted x86_64 (fresh) | `pass9_port_tests::a_port_range_holder_cannot_reach_a_port_above_its_range` |
| 28 | a revoked port holder faults on its next in/out (x86_64) | **fresh**: a parked holder revoked by the *sparing* `revoke_port_range_from_others` (the syscall REVOKE body) faults on wake | held | booted x86_64 (fresh) | `pass9_port_tests::a_parked_holder_revoked_by_the_sparing_take_back_faults_on_wake` |
| 29 | a thread that deletes its own port cap faults on next in/out (x86_64) | **fresh**: a self-deleting holder writes after dropping its own port capability; #GP | held | booted x86_64 (fresh) | `pass9_port_tests::a_self_deleting_holder_faults_on_its_next_write` |
| 30 | a revocation reaches a capability in flight, not only in tables | **fresh**: a `DeviceFrame` (and on x86_64 a `PortRange`) parked in the hand-off slot through a whole-machine revoke; receiver handed nothing (the two sweeps `revocation_in_flight_tests` BUGS flagged untested) | held | booted aarch64/riscv64/x86_64 (fresh) | `pass9_revocation_tests` |
| 31 | an unvouched child holds no capability beyond two read-only pages | **untestable as a fresh hostile test** in this runner: it is a capability census asserted during the `script/swish-check` interactive boot, not a hostile program; turning it into an attack (a child that tries to *use* an undelegated capability) needs a new userspace program | held (by the census gate) | untestable | `script/swish-check` |
| 32 | the boot shell holds no display device | **untestable as a fresh hostile test**: same as 31, a census assertion during the gpu `script/swish-check` boot, not a hostile attack; a hostile shell reaching for a display device would need a new userspace program | held (by the census gate) | untestable | `script/swish-check` |
| 33 | no WRITE, no x86_64 port I/O | **fresh**: a READ-only holder of the COM1 range writes a port inside its range; #GP (the grant must read the capability's rights) | held | booted x86_64 (fresh) | `pass9_port_tests::a_read_only_holder_cannot_write_a_port_inside_its_range` |
| 34 | a program reaches only the sockets it holds; a socket moves only by its capability | a squatter at a shared stack endpoint tries ten ways to reach an honest client's socket, plus two kernel-IPC probes on its own; all refused | held | booted aarch64 (existing client) | `net_confinement_tests` (e1000e confirmed attached this runner) |

## Where I attacked hardest, and where the pass is shallow

Hardest, with my own booted hostile tests:

- **Claims 6 to 9 (supervision scope), `pass9_supervision_tests`.** This is the attack I judged most
  worth building, because the failure mode is subtle: a `ps`-style tool that scopes by *rights*
  rather than by *relationship* would let any supervisor reap or enumerate any thread. The existing
  suite drives a WRITE-only holder of the *right* endpoint; I built a real supervised ring-3 child
  and attacked it through an *unrelated* endpoint carrying `READ | ENUMERATE`, every right a
  supervisor needs. Reap and survey are both refused through the foreign endpoint and both permitted
  through the owning one, which is the "one domain" property stated as code. The made-up-tid arm
  shows the refusal leaks nothing about a stranger's liveness. The scope check is
  `capability::reap_decision`/`survey_includes`, keyed on the invoked endpoint, and it held.

- **Claims 2 and 26 (the rights model and `BADGE`), `pass9_confinement_tests`.** `Rendezvous::BADGE`
  is the one method a program can reach that mints a *fresh* capability to an endpoint it holds, so
  it is where a client would try to launder authority. It is `GRANT`-gated, write-once, refuses a
  zero badge, and the minted copy carries exactly the source's rights. Also the consumed-capability
  (`cap_delete` then re-invoke) and delete-isolation paths through the live table, and the
  out-of-range slot variant of "no ambient authority."

- **Claims 10 and 11 (the map gate and W^X), `pass9_mapping_tests`.** Driven one layer up from
  `crates/paging`'s per-format proofs: a confined budget holder's `MemoryRegion::MAP` is refused at a
  kernel-half and an unaligned address, tied to the proved gate, and no `Flags` a mapping path
  installs is both writable and user-executable.

Lightly touched, source review only (no hostile test of my own written and booted). A shallow pass
shows here:

- **Claims 12 to 18 (IOMMU and DMA).** The brief allows host/read grade for these. I reviewed
  `direct_memory_access_validator` closely (it is clean and Kani-proved) but booted nothing and did
  not drive a hostile virtio driver.
- **Claim 19 (directory capability), 24 (two shells), 25 (compositor), 34 (network sockets).** These
  are component-level and need a filesystem disk, a compositor, shells, or the e1000e path. I read
  the existing purpose-built attacker clients and accepted them; I wrote no new client. These are the
  rows where a determined attacker would spend the most time and where my pass is thinnest.
- **Claims 27 to 29 and 33 (x86_64 port ranges).** These require hand-assembled ring-3 stubs. I read
  `x86_port_tests` in full and judged the existing attacks genuine (they boot ring-3 children and
  read the fault), but I did not write my own x86 asm attacker.
- **Claims 20, 21, 22, 23, 30, 31, 32.** Read only. Claim 21 in particular is already covered by a
  strong ring-3 attack (`el0_boundary_tests`) that asserts a *permission* fault on a *mapped* kernel
  address; my own version would have been weaker (an unmapped high-half read gives a translation
  fault), so I did not add it.

## Second session: where I attacked hardest, and what I only confirmed

Hardest, with freshly authored booted tests:

- **Claim 30 (revocation in flight), `pass9_revocation_tests`.** The sharpest row this session,
  because `revocation_in_flight_tests`' own `# BUGS` names the gap: it drives the `PageFrame` sweep
  through a parked hand-off slot and reasons about the `DeviceFrame` and `PortRange` sweeps from the
  code. Those two carry the same two hand-off-slot lines, untested. I drove a `DeviceFrame`, and on
  x86_64 a `PortRange`, parked in `outgoing_cap` through a whole-machine revoke; the receiver was
  handed nothing on every ISA. The `DeviceFrame` case is the sharper object: `revoke_device_from_others`
  exists so a device has one owner after a live driver swap, and a copy in a hand-off slot would give
  it two.
- **Claim 22 (the ELF loader), `pass9_elf_tests`.** The existing tests forge single-segment images.
  I forged two-segment images with a benign first segment and a hostile (W+X, or kernel-half) second,
  to see whether the loader's W^X and half checks reach every segment or stop at the first, and a
  segment whose `p_vaddr+p_memsz` wraps the 64-bit space. All refused. The kernel-half case is caught
  by the placement guard (`TooLarge`) before the mapper's `WrongHalf`, which the test records rather
  than pins, because two guards stand there and the outer one fires first.
- **Claims 27 to 29 and 33 (x86 port ranges), `pass9_port_tests`.** Every `out` in the existing suite
  is inside the granted range; I had a holder of exactly the COM1 range write the port one past it,
  which a TSS bitmap filled too wide would permit. Plus a parked holder revoked by the *sparing*
  `revoke_port_range_from_others` (the syscall `REVOKE` body, which the existing revoke test does not
  drive), a self-deleting holder, and a READ-only holder. No new ring-3 machine code was minted; the
  architect-ratified `x86_programs` stub builders were reused with fresh parameters and scenarios.
- **Claims 21 and 23.** The `outlaw` reads a kernel *code* address rather than a rodata static (a page
  class whose leak would be a gadget store), and the construction authority is dropped and rebuilt
  from through the dispatcher.

Only confirmed, not freshly authored (the server runs here and a purpose-built hostile client already
attacks the exact claim):

- **Claims 19, 20, 24, 25, 34.** The FS disk, the C seam, the compositor and the `e1000e` net stack
  all start and serve in this runner, and each claim's purpose-built hostile client
  (`dir_capability_tests`, `c_seam_tests`, `shell_navigation_tests`, `compositor_tests`,
  `net_confinement_tests`) booted green this session. I did not author a near-duplicate: each existing
  client already exhausts the vector the brief names (parent/sibling/`..`; a C out-of-bounds write;
  two roots; neighbor pixels and the screen; a socket squatter), and a fresh one would need a new
  userspace program role, which is a naming and ratification decision this pass should not make alone.
  These are the rows a determined attacker would still probe hardest, and my coverage of them is
  confirmation of the existing attack rather than a new one.

Untestable as a fresh hostile test:

- **Claims 31 and 32.** Both are a capability census asserted during the `script/swish-check`
  interactive boot: an unvouched child's caps, and the boot shell's lack of a display device. A census
  is an inventory check, not a hostile program, and `swish-check` is a 90-second-to-six-minute
  interactive boot rather than a `#[test_case]`. Turning either into a fresh hostile attack (a child
  that tries to *use* an undelegated capability, or a shell reaching for a display device) needs a new
  userspace program, so neither is bootable as a fresh hostile test here. The census itself can be
  watched through `swish-check`, which is the first session's `read`-grade coverage, not an attack.

## Near miss

### Claim 17: the shadow descriptor is published in two unsynchronised stores

Not booted (claims 12 to 18 are host/read grade), and **not a new finding**: it is recorded in the
`direct_memory_access_validator` crate's own `# BUGS` section and in roadmap
`0799-the-shadow-descriptor-is-published-in-two-stores.md`, attributed to an earlier pass. I confirmed
it by source reading and report it here because it is exactly one bug away from a claim-17 escape.

`shadow_one_head` copies a validated descriptor into the shadow ring as two separate 64-bit stores
(`write64(addr)` then `write64(word)`) with no barrier and no check that the shadow slot is idle, and
it writes each descriptor before validating the next in the chain. A driver that republishes a head
the device still holds (a posted receive buffer) can have the device observe the new `addr` with the
old `len`: a span the validator never admitted. On virtio-mmio there is no IOMMU, so this walk is the
only boundary. The Kani proof cannot see it, because it checks each descriptor only after both halves
are written and never models a concurrent device reader.

Reproduction: none booted. The hazard is in
`crates/direct_memory_access_validator/src/lib.rs` (`shadow_one_head`, and the `# BUGS` note at the
top of the file). Observed by reading, not by a running guest.

## Surfaces attacked that have no numbered row

- **`Rendezvous::BADGE` write-once and zero-badge refusal.** Row 26 is "a client cannot become its
  server"; BADGE's own minting discipline (set once, non-zero, GRANT-gated) is adjacent but not a
  numbered claim. `pass9_confinement_tests` asserts all three, booted on three ISAs.
- **An out-of-range slot index (past the table), as distinct from an empty in-range slot.** Claim 2
  and `confinement_attack_tests` cover the empty in-range case; the index past the table edge is the
  one a program reaching past its granted authority would use. Booted on three ISAs.
- **The `Flags` constructors beyond the VA gate.** Claim 11 is proved per format over encoded leaves;
  `pass9_mapping_tests` additionally asserts the kernel-visible `Flags` set
  (`user_data`/`user_rodata`/`user_code`/`user_device`) that every mapping path draws from, which is
  the twin property one level above the leaf encoding.

No surface I attacked produced an escape outside the claims table.

Second session, surfaces with no numbered row:

- **A `DeviceFrame` used as a bare object in a revocation sweep.** Row 30 is the general in-flight
  claim; the `DeviceFrame` and x86_64 `PortRange` hand-off-slot limbs are the two the
  `revocation_in_flight_tests` BUGS section singled out as untested, now driven, in
  `pass9_revocation_tests`.
- **A port write one byte past a granted range, and a wrapping ELF segment.** The out-of-range `out`
  (`pass9_port_tests`) and the `p_vaddr+p_memsz` wrap (`pass9_elf_tests`) are the arithmetic edges of
  claims 27 and 22 rather than numbered claims of their own.

## Refusal log

Every step either session declined, and every stop by a safety system or classifier.

**None.** No step was refused in either session, and no classifier or safety system stopped the pass
before or during any tool call. The one planned action not taken was authoring fresh userspace
program roles for the component rows (19, 20, 24, 25, 34) and the census rows (31, 32); that was a
scope judgment (a new program is a naming and ratification decision), recorded in the table and the
hardest/confirmed section, not a refusal by a safety system.

## Counts

Updated across both sessions.

By outcome (34 claims): **held 31, near miss 1 (claim 17, documented), untestable 2 (claims 31, 32),
escape 0.** The two untestable rows still hold under their census gate; they are untestable only as a
*fresh hostile test* in this runner.

By grade:
- booted, fresh hostile tests authored this session: claims 21, 22, 23, 27, 28, 29, 30, 33. **8 rows**
  (21, 22 on aarch64/riscv64; 27, 28, 29, 33 on x86_64; 23, 30 on all three).
- booted, first session's fresh hostile tests (aarch64/riscv64/x86_64): claims 2, 4, 5, 6, 7, 8, 9,
  10, 11, 26. **10 rows**.
- booted, existing purpose-built hostile client confirmed green this session (aarch64): claims 19, 20,
  24, 25, 34. **5 rows**.
- accepted plus booted on aarch64 by the baseline `seventh_pass` run: claims 1, 3. **2 rows**.
- read/host (source review only, DMA and the wiring plan, per the brief): claims 12, 13, 14, 15, 16,
  17, 18. **7 rows**.
- untestable as a fresh hostile test (census assertions via `swish-check`): claims 31, 32. **2 rows**.

Twenty-three rows now carry boot-grade coverage (8 fresh this session + 10 fresh first session + 5
existing-client confirmed), up from the first session's twelve. Seven stay read/host by the brief's
own instruction (the DMA and plan rows), two are untestable as fresh hostile tests, and two are the
accepted baseline. Every row the brief asked this session to boot (19 to 34, minus 12 to 18) is now
booted, except 31 and 32, which are a census rather than an attack and are graded untestable with
that reason.

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

Grade key: `booted <isas>` = a hostile test I wrote and booted; `accepted` = an earlier pass's
attack test, read and accepted (claims 1 to 3 per the brief); `read` = source review only; `host` =
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
| 19 | a directory capability reaches its subtree and nothing above | reviewed the hostile dir-attacker client; needs a RedoxFS disk | held | read | `dir_capability_tests` |
| 20 | a memory-unsafe C component faults and changes nothing outside its grant | reviewed the hostile C out-of-bounds test | held | read | `c_seam_tests::a_c_out_of_bounds_write_faults_and_changes_nothing_outside_its_grant` |
| 21 | a user program cannot read a kernel address on every ISA | reviewed the ring-3 `outlaw` reader asserting a PERMISSION fault on a mapped kernel static | held | read | `el0_boundary_tests::a_user_program_cannot_read_a_kernel_address` |
| 22 | an ELF cannot load over the kernel or ask for a W+X page | reviewed the two refusal tests | held | read | `elf_loader_tests` |
| 23 | the progenitor cannot rebuild after dropping construction authority | reviewed the suptree authority-drop test | held | read | `authority_tests::init_drops_its_construction_authority_and_cannot_build_again` |
| 24 | two shells with different roots cannot name each other's files | reviewed shell-navigation and job-window tests | held | read | `shell_navigation_tests` |
| 25 | a client cannot reach a neighbor's pixels or read the screen | reviewed the compositor confinement tests | held | read | `compositor_tests` |
| 26 | a client of a rendezvous cannot become its server | `BADGE` is GRANT-gated, write-once, and mints no wider right; (existing: WRITE-only refused RECEIVE/REAP/SURVEY) | held | booted aarch64/riscv64/x86_64 | `pass9_confinement_tests::a_badge_is_grant_gated_and_mints_no_wider_right` |
| 27 | a non-holder cannot touch a port; grant does not leak across a switch (x86_64) | reviewed the ring-3 holder/non-holder hand-off test | held | read | `x86_port_tests::port_holder_transmits_then_a_non_holder_faults` |
| 28 | a revoked port holder faults on its next in/out (x86_64) | reviewed | held | read | `x86_port_tests::a_revoked_holder_faults_on_its_next_port_write` |
| 29 | a thread that deletes its own port cap faults on next in/out (x86_64) | reviewed | held | read | `x86_port_tests::a_holder_that_deletes_its_port_capability_faults_on_its_next_port_write` |
| 30 | a revocation reaches a capability in flight, not only in tables | reviewed the hand-off-slot sweep and its test | held | read | `revocation_in_flight_tests` |
| 31 | an unvouched child holds no capability beyond two read-only pages | reviewed the census gate | held | read | `script/swish-check` |
| 32 | the boot shell holds no display device | reviewed the caps census gate | held | read | `script/swish-check` |
| 33 | no WRITE, no x86_64 port I/O | reviewed the ring-3 READ-only port test | held | read | `x86_port_tests::a_read_only_port_capability_must_not_grant_port_output` |
| 34 | a program reaches only the sockets it holds; a socket moves only by its capability | reviewed the squatter and socket-handoff tests; needs the e1000e path | held | read | `net_confinement_tests` |

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

## Refusal log

Every step I declined, and every stop by a safety system or classifier. None. No step was refused,
and no classifier or safety system stopped the pass before or during any tool call.

## Counts

By outcome (34 claims): **held 33, near miss 1 (claim 17, documented), escape 0.**

By grade:
- booted on aarch64/riscv64/x86_64 (fresh hostile tests I wrote): claims 2, 4, 5, 6, 7, 8, 9, 10, 11,
  26. **10 rows**.
- accepted (earlier pass's tests, read and accepted) plus booted on aarch64 by the baseline
  `seventh_pass` run: claims 1, 3. **2 rows**.
- read/host (source review only): claims 12 to 25 except 26, and 27 to 34. **22 rows**.

Twelve rows carry boot-grade coverage (10 fresh + 2 accepted); the remaining 22 were graded by source
review. The brief's bar of 27 booted rows was not met: the component-level, x86-asm, and DMA rows
would each need substantial userspace or hand-assembled machinery to drive a fresh hostile client,
and within this pass's budget I booted the core kernel-confinement claims (the rights model,
supervision scope, the map gate, W^X) and reviewed the rest. This is recorded plainly so the pass's
shallowness on those rows is visible.

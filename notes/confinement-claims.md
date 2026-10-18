# What nife claims a confined component cannot do, and which of those claims is tested

Milestone 202, building `design/fatal-risks/README.md`'s risk 7. The enumeration is the first
deliverable and this note is it. What follows the table is what happened when each claim's test
was broken on purpose.

Risk 7 is *"the confinement claim is false."* The evidence against it is a set of tests this
project wrote about attacks this project chose, and a passing confinement test fits two worlds:
the component was stopped, or it never reached the address and the assertion is decorative. Milestone 194 built the mechanism that tells those apart for a Kani
harness (`Falsification:`, `script/falsifications`, a recorded patch that must turn one harness
red).

Nothing here supports "the confinement holds." It supports a narrower sentence, the one to quote: *these named claims are tested, and each test has been shown to fail
when the claim is broken.*

## The claims

Assembled from DECISIONS §14, §20, §31, §32, `notes/untrusted-input-audit.md`, and the tests
themselves. The last column is this milestone's result.

| # | The claim | Stated in | Tested by | Falsified |
|---|---|---|---|---|
| 1 | A component cannot widen its own rights: a derive holds no more than its source | §14, `crates/capability` | `capability::derive_never_widens_rights` | milestone 194 |
| 2 | Userspace cannot forge a right out of a syscall register | §14 | `capability::from_bits_cannot_forge_a_right` | milestone 194 |
| 3 | A budget that cannot delegate cannot split itself a child that can, nor retype a page or kernel object that can | §16 (object revocation), `Cap::mint_child`; the retype half, milestone 824 (a retype mints no right its budget lacks) | `capability::split_never_widens_rights`; `kernel::user::confinement_attack_tests::a_retype_mints_no_right_its_budget_lacks`, `seventh_pass_retype_tests::a_grant_less_budget_narrows_a_split_child_and_every_retype_alike` | **yes**; the retype half [for a frame](../system_tests/falsifications/user.confinement_attack_tests.a_retype_mints_no_right_its_budget_lacks.patch) and [for an object](../system_tests/falsifications/user.seventh_pass_retype_tests.a_grant_less_budget_narrows_a_split_child_and_every_retype_alike.patch), 2026-10-12 |
| 4 | A consumed capability cannot be used again | §12 | `capability::a_deleted_capability_stays_deleted` | **yes** |
| 5 | Dropping one capability does not disturb the others | §12 | `capability::delete_touches_only_its_slot` | **yes** |
| 6 | A supervisor cannot collect a corpse it does not supervise | §32 | `capability::reap_is_permitted_only_to_the_supervising_rendezvous` | **yes** |
| 7 | A refusal about a stranger's thread discloses nothing about it | §32 | `capability::a_stranger_reveals_nothing_about_its_liveness` | **yes** |
| 8 | A process view shows exactly the rendezvous's own children | milestone 126 | `capability::a_survey_shows_exactly_the_endpoints_own_children` | **yes** |
| 9 | What a supervisor may see and what it may reap are one domain | milestone 126 | `capability::the_view_and_the_reap_have_the_same_scope` | **yes** |
| 10 | A user virtual address is in the low half and page-aligned, on every ISA | §19 | `paging::{aarch64,sv39,x86_64}::the_user_va_gate_admits_only_the_aligned_low_half` | **yes, three times** |
| 11 | No page is both writable and executable | §19 (architectural parity is a tenet) | `paging::{aarch64,sv39,x86_64}::no_encoded_leaf_is_both_writable_and_executable` | **yes, three times** |
| 12 | An IOMMU entry sets no bit the hardware treats as reserved | §20 | `paging::x86_64::no_vtd_entry_ever_sets_a_reserved_bit` | **yes, since 2026-09-16, and see below** |
| 13 | A device cannot touch memory outside its driver's granted region | §20 (IOMMU-backed DMA isolation) | `direct_memory_access_validator::in_region_is_sound`, `an_accepted_descriptor_is_confined`, `validate_and_shadow_confines_every_chain` | **yes, three** |
| 14 | A driver cannot send its device to descriptors nothing validated | §20 | `direct_memory_access_validator::an_accepted_descriptor_is_confined` (the indirect refusal) | **yes** |
| 15 | A driver cannot make the validator walk outside the rings, or forever | §20 | `direct_memory_access_validator::the_outer_walk_stays_inside_the_rings_and_terminates`, `an_oversized_batch_is_refused` | **yes, two** |
| 16 | One queue's validation cannot touch another queue's rings | §20 | `direct_memory_access_validator::distinct_queues_occupy_disjoint_blocks` | **yes** |
| 17 | A descriptor changed after validation cannot reach the device | §20 | `direct_memory_access_validator::a_descriptor_mutated_after_validation_cannot_reach_the_device` | [yes, 2026-10-06](../crates/direct_memory_access_validator/falsifications/verification.a_descriptor_mutated_after_validation_cannot_reach_the_device.patch), and see below |
| 18 | A wiring plan never grants a right the declaration did not ask for | §41 | `component_plan::a_plan_never_grants_a_right_the_declaration_did_not_ask_for` | **yes** |
| 19 | A directory capability reaches its subtree and nothing above it | §50 (namespace composition, not stored paths) | `filesystem_contract::attenuate_never_widens`, `a_grandchild_is_bounded_by_the_root`; `kernel::user::dir_capability_tests` | milestone 194 (the proofs); [one kernel test, 2026-10-06](../system_tests/falsifications/user.dir_capability_tests.a_read_only_directory_capability_reaches_its_subtree_and_nothing_above_it.patch) |
| 20 | A memory-unsafe C component faults on an out-of-bounds write and changes nothing outside its grant | §31 | `kernel::user::c_seam_tests::a_c_out_of_bounds_write_faults_and_changes_nothing_outside_its_grant` | **yes, by hand** |
| 21 | A user program cannot read a kernel address, on every ISA | §19 (architectural parity is a tenet) | `kernel::user::tests::a_user_program_cannot_read_a_kernel_address`, `the_hardware_says_el0_cannot_read_the_kernels_memory`, `riscv_virtio_tests::the_page_tables_say_u_mode_cannot_read_the_kernels_memory` | **yes: one record on aarch64 and x86_64, and riscv64's is the software walk** |
| 22 | An ELF cannot ask to be loaded over the kernel, or for a writable executable page | §15 | `kernel::user::tests::an_elf_that_asks_to_be_loaded_over_the_kernel_is_refused`, `..._for_a_writable_executable_page_is_refused` | **yes, two** |
| 23 | The progenitor cannot rebuild after dropping its construction authority | §26 (the fault endpoint) | `kernel::user::authority_tests::the_root_supervisor_drops_its_construction_authority_and_cannot_build_again` | **yes, and see below** |
| 24 | Two shells with different roots cannot name each other's files | §50 (namespace composition, not stored paths) | `kernel::user::shell_navigation_tests::two_shells_with_different_roots_cannot_name_each_others_files`, `grant_plan::job_windows::tests::take_never_hands_out_a_window_whose_last_holder_is_unreaped`, `job_undertaker_tests::job_undertaker_says_which_job_it_reaped_and_only_then_is_its_window_free` | **yes, and see below**; window reuse [closed](../design/roadmap/0685-a-job-is-finished-when-its-memory-is-back.md) 2026-10-06 |
| 25 | A client cannot reach its neighbor's pixels or read the screen | §33 (the compositor's authority is memory, not messages) | `kernel::user::compositor_tests::a_client_holds_no_capability_for_its_neighbours_pixels_or_the_screen` and five more in [compositor-claim-25.md](compositor-claim-25.md) | **yes, six patches, aarch64** |
| 26 | A client of a rendezvous cannot become its server | §41 (the endpoint is the broker) | `kernel::user::live_swap_tests::a_client_of_the_stable_rendezvous_cannot_become_its_server`; `confinement_attack_tests::a_write_only_rendezvous_holder_cannot_receive_reap_or_survey` | yes, [the first](../system_tests/falsifications/user.live_swap_tests.a_client_of_the_stable_rendezvous_cannot_become_its_server.patch) and [the second](../system_tests/falsifications/user.confinement_attack_tests.a_write_only_rendezvous_holder_cannot_receive_reap_or_survey.patch), see below |
| 27 | A thread holding no port capability cannot touch a port, and a holder's ports do not leak across a context switch (`x86_64`) | §121, milestone 299 | `kernel::user::x86_port_tests::port_holder_transmits_then_a_non_holder_faults` | **yes, milestone 313, and see below** |
| 28 | A revoked port holder faults on its next `in`/`out` (`x86_64`) | §121, milestone 299 | `kernel::user::x86_port_tests::a_revoked_holder_faults_on_its_next_port_write` | **yes, milestone 313** |
| 29 | A thread that deletes its own port capability faults on its next `in`/`out` (`x86_64`) | §12 (call/reply IPC: a one-shot reply capability), milestone 313 | `kernel::user::x86_port_tests::a_holder_that_deletes_its_port_capability_faults_on_its_next_port_write` | **yes, milestone 313** |
| 30 | A revocation reaches a capability in flight, not only the ones sitting in capability tables | `sched::delete_page_frame_caps_where` | `kernel::user::revocation_in_flight_tests::a_capability_revoked_while_it_is_in_flight_does_not_reach_the_receiver` | yes, 2026-09-21 |
| 31 | An unvouched child holds no capability its caller did not delegate, beyond two read-only pages | §219 (how the shell names an installed program to the spawner) | `script/swish-check`: `installed/unvouched` | yes, 2026-10-03, swept weekly ([patch](../xtask/falsifications/swish_check.boot.swish_check_boot.patch)) |
| 32 | The boot shell holds no display device | Milestone 715 (provisional) | `script/swish-check`: the `caps` census on the gpu boots | yes, 2026-10-03, swept weekly ([patch](../xtask/falsifications/swish_check.boot.swish_check_leg.patch)) |
| 33 | No `WRITE`, no x86_64 port I/O | Milestone 768 (provisional) | `kernel::user::x86_port_tests::a_read_only_port_capability_must_not_grant_port_output` | [yes](../system_tests/falsifications/user.x86_port_tests.a_read_only_port_capability_must_not_grant_port_output.patch) |
| 34 | A program reaches only the sockets it holds; a socket moves only by its capability | §255 (each socket is its own capability) | `kernel::user::net_confinement_tests::a_squatter_at_a_shared_stack_endpoint_cannot_capture_the_clients_traffic`, `a_socket_moves_by_its_capability_and_a_closed_one_reaches_nothing` | yes: [the squatter](../system_tests/falsifications/user.net_confinement_tests.a_squatter_at_a_shared_stack_endpoint_cannot_capture_the_clients_traffic.patch), [the hand-off](../system_tests/falsifications/user.net_confinement_tests.a_socket_moves_by_its_capability_and_a_closed_one_reaches_nothing.patch) |

Every row carries a replayable record as of 2026-10-07 (UTC), fatal risk 7's second criterion.

The evidence behind each section below is in [`confinement-claims/`](confinement-claims/README.md).

## Six claims that are stated nowhere, which is what step 1 was for

Enumerating the claims also found what the system does not claim. Each is a limit, written down so
nobody reads a row as covering it. [`confinement-claims/unstated-claims.md`](confinement-claims/unstated-claims.md)
has the measurements and the history of each.

- A confined component's *timing* is not confined. Two threads and a shared word make a 6.8 ns
  clock with no privileged instruction, on all three architectures. The cycle-counter grant of
  DECISIONS 139 buys accountable authority, not timing isolation.
- A confined device's *values* are not confined, only its *reach*. The IOMMU confines placement,
  not values, so row 13 does not cover a device-written index the NVMe driver fails to check.
- A `SURVEY` cursor counts threads the viewer cannot name.
- A confined component's *interrupt target* is not confined, and nothing has ever asked. An MSI is a
  memory write, so DMA remapping alone does not confine it. MSI confinement lives in three different
  places (x86_64 interrupt remapping, the GICv3 ITS, one RISC-V IOMMU mode field) and none is
  exercised. It is latent while every component that can reach a BAR is the kernel. It goes live
  with the first driver that leaves the kernel and wants interrupts (§86 (whether an NVMe driver can
  leave the kernel)).
- The progenitor's bytes are unsigned, which is why the confinement has an unverified component
  inside it.
- The kernel cannot execute a confined component's code: `PXN` on aarch64, the hardware on riscv64,
  and `CR4.SMEP` on x86_64, which nothing set until the audit of milestone 313 (userspace
  confinement, read adversarially) on 2026-09-17. There is still no test.

## What breaking them found

Twenty-five Kani harnesses now carry a recorded patch that turns them red, and
`script/falsifications --sweep` replays them in about 30 seconds. Breaking them on purpose found
that the assertion a reader would quote is often not the one doing the work. The headline
witness check of §31 (the foreign-language seam) is reached only by an escape that faults anyway. A harness stated through the function
under test could not see it break, and when it was repaired the "rescue" assertion became the
decoration. Row 17's harness could not see a double fetch until it read a fresh value per load.
[`confinement-claims/breaking-the-harnesses.md`](confinement-claims/breaking-the-harnesses.md)

## What breaking the kernel tests found (milestone 305)

Ten kernel `#[test_case]`s now carry a record that `--sweep` replays by booting one architecture.
The headline: row 21's RISC-V test could not fail. `user_can_read` walked only the low half, so it
answered "no" about a kernel address by refusing to look, and every gate was green throughout. It
was fixed (`translate_in_either_half`). Row 24's readable crossing assertion cannot run. Row 21's
three "yes"es are three different kinds of evidence. Row 26 needed a reshaped fixture before a real
escape stopped hanging the run. And row 23's record overlaps rows 4 and 5.
[`confinement-claims/breaking-the-kernel-tests.md`](confinement-claims/breaking-the-kernel-tests.md)

## Which assertion actually fires (milestone 307)

A sweep of all 26 rows asked, of every test and harness, which assertion fires when the claim is
broken. Seventeen fire as advertised. In six the quotable assertion cannot run. One (row 12) could
not fail at all: its VT-d reserved-bit proof was stated through the encoder's own constants, so a
widened mask moved the encoder and the assertion in lockstep. It is now stated in literals and fails
under that defect. Two rows are deliberately unfalsified. The sweep also found an assertion live on
aarch64 and dead on the other two ISAs, and row 19's attackers unable to tell a refusal from a probe
never sent. [`confinement-claims/which-assertion-fires.md`](confinement-claims/which-assertion-fires.md)

## What attacking them found (risk 7's adversarial pass, 2026-09-21)

This pass assumed each claim false and went looking. Two findings, both fixed:

- A capability revoked while it was in flight was delivered anyway. `Thread::outgoing_cap`, the
  `SEND_CAP` hand-off slot, was swept by no revocation. Row 30 now tests it, and the three sweeps
  that lacked it drop the parked capability.
- A mapping the kernel wired at boot was invisible to revocation, because `map_physical` never
  recorded it. So the progenitor could revoke a page the FS server kept mapped writable. The record
  is now a required argument.

Eight other attacks held, from recycled TCB slots to FP register leaks.
[`confinement-claims/adversarial-pass-2026-09-21.md`](confinement-claims/adversarial-pass-2026-09-21.md)
has every attack, what each fix does not settle, and what the pass could not reach.

## BUGS

The full list, every entry as written, is [`confinement-claims/limitations.md`](confinement-claims/limitations.md).
The ones a reader of the table most needs:

- This table is a floor, and it cannot list the claim nobody made. Every row came from reading what
  this project already wrote, so it inherits the tests' blind spots. The decisive experiment is
  adversarial and by somebody else.
- A yes on a kernel row is a machine-replayable fact that nothing replays on a schedule. A kernel
  record costs a boot per leg and is re-checked only by a full `--sweep`, on the architectures its
  patch names. A kernel test is one boot of one machine, not a solver over every input.
- A recorded falsification proves the harness catches *that* defect, not the class.
- Nothing gates which assertion a row's evidence comes through. Milestone 307 (which assertion actually fires when a confinement claim is broken) was a manual
  sweep;
  read its verdicts as dated 2026-09-16. Of the 26 rows it judged, 25 are reasoned from the code and
  one was measured.
- "Cannot run" is not "delete it": an assertion unreachable behind a correct guard becomes reachable
  the day the guard is wrong.
- Rows 27 to 29 were added by an audit and are dated 2026-09-17.

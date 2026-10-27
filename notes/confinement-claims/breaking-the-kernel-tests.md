# What breaking the kernel tests found (milestone 305)

An appendix to [notes/confinement-claims.md](../confinement-claims.md). It holds what milestone 305
(the six kernel confinement rows get a falsification a machine can replay) found when rows 21 to 26
were broken on purpose.

Rows 21 to 26 are six claims and nine tests, and a falsification is per test. Eight of the nine now
carry a recorded patch. The ninth is row 26, and it cannot carry one. Five results are worth more
than that count.

## A confinement test that could not fail, on RISC-V, since milestone 41 (dead code: triage the suppressions)

Row 21's RISC-V twin, `the_page_tables_say_u_mode_cannot_read_the_kernels_memory`, is the one place
this milestone found the thing it was looking for. The patch removes the `U`-bit check from
`mmu::user_can_read` outright. That is as complete a break of the test's stated property as can be
written. The first sweep reported SURVIVOR: the test ran, and passed.

The reason was not a weak defect. `user_can_read` walked through `translate_user`, and
`translate_at` builds its `Mapper` with `Half::Low`, always. So the high-half kernel address the test
asks about came back `None` before any leaf was read. The headline assertion,
`assert!(!mmu::user_can_read(kernel_va), "the page tables say U-mode could read the kernel's own memory")`,
answered "no" by refusing to look. It could not fail. The test's own doc comment calls that walk
"the thing under test", and calls the `U` bit RISC-V's single line of defense.

The tree had already written the cause down, and nothing connected it. `is_mapped_in_current_space`,
forty lines away in the same file, exists for exactly this case and says so in its doc comment: *"a
user thread reaching for the kernel's memory names a high-half address ... `translate_user` alone
would say 'not mapped' for it and turn the most interesting case into the wrong answer."*
`user_can_read` went on calling `translate_user`.

Milestone 305 fixed it (`translate_in_either_half`), and the patch is recorded against the fixed
function, so the row is evidence now rather than ritual. Two things follow. A green confinement test
is consistent with the assertion being unable to fail. That is the parent's opening sentence
arriving from a direction nobody had checked. And the only instrument that could find it was a
falsification. Every gate in this tree was green throughout, because a vacuous assertion is a
passing assertion.

## The §31 assertion-order hazard recurs, in row 24

Milestone 202 (every confinement test is a ritual until somebody breaks the confinement and watches
it fail) found that the leading sentence of §31 (the foreign-language seam), the witness-page
equality, is reached only by an escape that faults anyway. Row 24 is the same shape in a different
subsystem, and here it is structural rather than incidental.

`two_shells_with_different_roots_cannot_name_each_others_files` states its property twice. Once is
as the per-shell bitmap equalities in `assert_report`. Once is as the crossing,
`assert_eq!((a & nb::REACHED_SECRET, b & nb::REACHED_INNER), (0, 0), "a shell named a file in the other shell's root")`.
The second is the sentence the milestone makes, and the one a reader would quote. It sits below both
per-shell checks, and it cannot run. Any defect that causes a crossing sets a forbidden bit in one of
the reports, and `assert_report`'s first direction catches that one call earlier. No patch tried in
milestone 305 made the crossing fire, and none can.

Here that costs nothing, because `assert_report`'s messages name the offending or missing bit. So a
reader learns as much as the crossing would have told them. §31's instance cost a 234-second watchdog
timeout reading "livelock". Two instances found the same way promote it from an anecdote about §31
to a thing to look for. In a test that states its property twice, the readable statement is usually
the unreachable one.

And row 24's own record is weaker than the row looks, which the patch says where a reader meets it.
The recorded defect is the caretaker serving the filesystem root instead of its narrowed handle. It
turns the test red through `assert_report`'s *second* direction, the vacuity guard. The shell could
no longer reach its own files, so every refusal it reported would have proved nothing. Nothing
crossed. It proves the test is wired to the real root handle; it does not demonstrate a crossing.

## Row 21's "on every ISA" is not evidenced the same way on every ISA

The aarch64 falsifications map the kernel's own memory EL0-readable, one flag at one call site, and
the tests watch the hardware refuse. That defect cannot be booted on RISC-V. `crates/paging`'s Sv39
encoder turns `ACCESS_USER` into the `U` bit. S-mode access to a `U` page faults unless `sstatus.SUM` is
set, which this kernel sets only inside a test helper. A kernel that marked its own `.rodata`
user-readable there would not be a kernel userspace can read. It would be a kernel that cannot read
itself, dead before the first test. So the RISC-V evidence is against the software walk instead, and
the row's three "yes"es are not three of the same thing. DECISIONS §19 (architectural parity is a
tenet) makes parity a gate for the capability; this is a gap in the evidence.

The `x86_64` leg had no evidence until milestone 323 (the falsification record is incomplete in five
ways). The same one-flag defect works there. `arch/x86_64/mmu.rs` maps `.rodata` with
`Flags::kernel_rodata()`, and `user_rodata()` writes the `U/S` bit while SMAP stays off. It was
replayed 2026-10-03 (UTC), red at the same assertion of `a_user_program_cannot_read_a_kernel_address` on both. The record names
`Architecture: aarch64, x86_64`. Read the row as aarch64 twice, `x86_64` once, riscv64 once.

## Row 26 could not be falsified as written, because a real escape hung the run

`a_client_of_the_stable_rendezvous_cannot_become_its_server` asserts `attack[1] == -NotPermitted`.
The honest defect deletes the kernel's `Rights::READ` check on `RECEIVE_CAP`. Run on 2026-09-16, it
gave a 60-second lost-wakeup watchdog and not one word about impersonation. `RECEIVE_CAP` blocks. So
an attacker the kernel fails to refuse takes the server's message or parks, and the run deadlocks.
That is milestone 202's wrong-reason red, on a third claim. So the row stayed unfalsified, rather
than take an easier defect that only changes which error the refusal returns.

Two fixes followed. Milestone 633 (an outside agent attacks the confinement claim) added
`a_write_only_rendezvous_holder_cannot_receive_reap_or_survey`. It parks a sender first, so a
let-open receive returns. Deleting the check turned it red on aarch64 (2026-10-06 UTC). Milestone 800
(a non-Anthropic model attacks the confinement claim) reshaped `chatty` on 2026-10-07 (UTC): the
operator retires the last receiver, and a plant parks a marker. The first test now fails at its own
assertion under
[its record](../../system_tests/falsifications/user.live_swap_tests.a_client_of_the_stable_rendezvous_cannot_become_its_server.patch).

## One row's falsification proves less than the row looks like it proves

Row 23 is falsified in the kernel rather than in the fixture, by making `CapabilityTable::delete`
not take the capability out of the slot. That is the right place, because the claim is the kernel's.
But rows 4 and 5 are `capability::a_deleted_capability_stays_deleted` and
`capability::delete_touches_only_its_slot`, both already `replayable`, and both catch that same
defect. So the honest reading is narrower. It proves row 23's kernel test is wired to the kernel's
own delete, not that row 23's test is the only thing watching it. Rows 21, 24, 25 and 26 have no
such overlap.

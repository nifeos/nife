---
status: BUILT
built: 2026-10-04
raised: 2026-10-03
promoted_from: trace-the-progenitors-login-block-peak
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 753. Trace the progenitor's login block peak

Promoted from `design/roadmap/proposals/trace-the-progenitors-login-block-peak.md` on 2026-10-04
(UTC) by the lane that measured it. The number 753 is provisional until the queue lands it.
*(Title and slug are drafts.)*

Raised by the lane for milestone 715 (the spawn service holds the display grants, and the shell
holds none).

## Why

Milestone 715 moved the gpu's four and the keyboard's three grants from the shell into the spawn
service for the life of the boot. A gpu-and-keyboard boot then read 31 of 32, and
`kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` went from 30 to 31. That constant's doc says the next
capability held across the peak "buys a slot back or raises `CAPABILITY_TABLE_SLOTS`", and that
which grants sit on the peak "is not traced". The next capability added at boot halts the boot with
no message. This block is the trace, and the options it makes possible.

## What was built

A per-slot trace of the progenitor's table across the three `script/swish-check` boots on aarch64,
and the table it produced: [notes/capability-peak-trace.md](../../notes/capability-peak-trace.md)
(name provisional). The instrument is a patch beside the note, not a feature in the build. Nothing
in the kernel or the progenitor changed.

## What the trace found

1. The peak is two plateaus. On the keyboard boot the table reaches 30 three times while building
   the login stack, 31 for one event inside the login block, and 31 again through the
   `graphical_terminal` launch. The serial boot reaches 28 at both. A change confined to the login
   block leaves a gpu boot's peak exactly where it is.
2. Nineteen of the 31 are held for the life of the boot, and the seven device grants are seven of
   those nineteen. The device grants are the whole difference between the three boot shapes:
   24, plus four for a gpu, plus three for a keyboard.
3. The keyboard boot has a NIC, although `swish_check_boot`'s comment says it does not.
   `xtask/src/host.rs` exports `NIFE_NET=1` into xtask's own environment, and every later boot
   inherits it. The net stack's endpoint is one of the 31; without it the boot reads 30. The
   comment is corrected in this lane.
4. No other table in the system reaches 25 on any of the three boots, so the gauge's number is the
   progenitor's.

## The fork: how to lower the peak

Each option's saving is from replaying the three traces with its holdings removed, except where
marked derived. The table in the note has the per-boot figures.

### A. Raise `CAPABILITY_TABLE_SLOTS` to 64

Peak unchanged; headroom goes from one to 33. The free-slot word widens from `u32` to `u64`. Each
table grows by 1,024 bytes inside a TCB page measured at 1,984, 1,968 and 1,712 of 4,096 bytes used
(aarch64, x86_64, riscv64), so it fits on all three and costs no new memory. `FAULT_EP_SLOT` is
derived and moves from 31 to 63. That is an ABI fact every supervisor and child shares, though
nothing outside the tree is built against it today. The fastpath footprint at 64 was not measured.

### B. Graphics sessions get a builder of their own (the lane's recommendation; ruled, then reversed)

A new process, built at boot before the first plateau, takes the seven device grants and a budget.
It builds each graphical session when the shell asks. The progenitor holds neither the grants nor
the launch. Replayed: keyboard 31 to 24, serial 28 to 24, no gpu unchanged at 24. The progenitor's
peak stops depending on which display devices a boot has. The new process's own peak was not
measured; the launch plateau's composition puts it in the low twenties.

Cost: a new program, which is a name, and a request protocol, which is a wire format. It needs the
session's program images, which the progenitor can copy in as blobs the way it gives `login` its
caretaker. About 480 lines (`graphical_terminal_grants` through
`graphical_terminal_session_children`) would move out of `crates/system_initializer`.

### C. Release the login block's inputs earlier

Delete `login`'s seven inputs when `build_child(login)` returns, build the shell after the block,
and drop `term_out` sooner. No names, no protocol, reversible in a commit. Replayed: no gpu 24 to
23, and no change on either gpu boot, because the launch plateau is untouched. Worth doing beside B
for one slot; not a fix on its own.

### D. Lend the device grants to a holder, and fetch each back for the build that uses it

The progenitor holds one endpoint instead of seven outside the launch, and fetches the gpu's or the
keyboard's grants only around the build that needs them. Derived: keyboard 28, serial 26. Loses to
B: it costs the same new program and protocol, saves less, and leaves the launch in the progenitor.

### Refused

- The shell holds the grants again. That reverses milestone 715's narrowing.
- Build the graphical stack at boot and park it. That reverses calef's 2026-09-30 ruling, recorded in
  milestone 632 (graphics on demand), that graphics is launched from the prompt.
- The kernel grants devices on request. That is a new method on the system call surface for a problem a
  userspace process can solve.

## The seven questions, for B against A

1. *What else was considered?* C, D and the three refusals above, each with its measured or
   derived saving and its reason.
2. *What does the tree already do?* It moved device authority out of a process that did not need it
   twice: milestone 600 (the graphical terminal stack is built in userspace) out of the kernel, milestone 715 out of the shell. The progenitor already
   delegates whole jobs to children it builds (`login`, `net_stack`, the entropy service).
3. *Prior art outside the tree, from memory and marked as such:* seL4's CAmkES and Genode both put a
   device behind the one component that drives it, and the root task hands it over at start.
4. *Is the premise true?* Partly not. The proposal assumed the peak was the login block. It is two
   plateaus, and the launch is the one a login-only fix cannot reach.
5. *Cost, measured:* B saves seven slots on the keyboard boot and four on the serial one, by replay.
   A saves none and adds 33 of headroom at 1,024 bytes a thread in pages already allocated.
6. *Reversibility:* A is reversible until something outside the tree is built against
   `FAULT_EP_SLOT` at 63. B adds a program name and a wire format, both on the expensive list.
7. *Same cost?* If B cost what A costs, B would still be chosen. It shrinks the most powerful
   process's authority, and it stops every new boot device from spending a progenitor slot. A is
   the cheaper answer, and that is an argument about effort.

## Recommendation, and the ruling

The lane recommended B, with C alongside it, which together read 23 on every boot shape.

calef chose A on 2026-10-04 (UTC), on pull request #1608: the table grows to 64 slots. Earlier the
same day he had ruled B + C, and he reversed that ruling before anything was built.

His reason for the reversal: a builder process that exists to move slot accounting is not worth a
new program and a new protocol. Taking the device grants away from the progenitor does not
meaningfully reduce its authority, because the progenitor runs the spawn service and can build any
process with any grant it holds. That overturns question 7's argument above, which rested on B
shrinking the most powerful process's authority. The fixed table is the real constraint.

The build is milestone 754 (the capability table grows to 64 slots), number provisional. Whether
tables should instead grow per process, as seL4's do, is a separate proposal:
`design/roadmap/0778-capability-tables-sized-per-process.md`.

## BUGS

- The instrument is a patch and nothing checks that it still applies. It is documented in the note.
- riscv64 and x86_64 were not traced. The riscv64 runner has no `-semihosting` for the drain.
- B's process's own peak is an estimate from the launch plateau, not a measurement.

## Follow-on

- **Milestone 754.** Milestone 754 (the capability table grows to 64 slots), number provisional:
  option A, as ruled.
- **Milestone 778.** Milestone 778 (capability tables sized per process). `design/roadmap/0778-capability-tables-sized-per-process.md`, the longer-term
  question the raise buys time for.
- **Refused.** Options B and C, ruled 2026-10-04 (UTC) and reversed the same day, for the reason
  under the ruling above.
- **Refused.** A new gate that fails when the peak passes 30 already exists in effect:
  `report_peak` prints "ABOVE the recorded" and `script/swish-check` fails on it. What the trace
  adds is the itemization. Keeping it as an opt-in kernel feature would need a name, so it is left as
  a patch until a second use for it appears.

## Index row

Nobody knew which capabilities held the progenitor's table at 31 of 32. BUILT: a per-slot trace of three boots shows two plateaus (the login block and the graphics launch) and the seven device grants as the difference between boot shapes, with each option's saving replayed.

# The eighth outsider pass's tests

An appendix to [notes/confinement-claims.md](../confinement-claims.md). It holds the design of eight
of the nine `system_tests/src/user/eighth_pass_*_tests.rs` modules;
`eighth_pass_spawn_audit_tests.rs` is not covered here. Their doc comments state what each test
proves; the pass itself is
[notes/confinement-outsider-pass-8.md](../confinement-outsider-pass-8.md).

*Name: provisional, for the stem `the-eighth-pass-tests`, named for the pass whose tests it
describes. calef names files.*

Every module here attacks a claim that already has a cited test, at a shape the cited test leaves
open. Each refused path is built to answer promptly, so a regression is red at an assertion rather
than at the watchdog. Unless a section says otherwise, the code under test is portable kernel code,
and §19 (architectural parity is a tenet) is met by running the same test on every architecture.

## The delegation race (claim 30)

The cited test, `revocation_in_flight_tests`, revokes a capability that already sits parked in the
hand-off slot. Its `BUGS` section names two places as unmeasured, and this module attacks both.

The first is the window between the syscall's start and the filing of the copy. `SEND_CAP` reaches
the test kernel's seam, `delegation_pause::here`, after its rights gate and just before the locked
section that reads the source, derives the copy and files it. A revocation sweep landing in that
window must be seen: the delegation has to re-read the source under the hold and find it gone. A
kernel that read the source once, before the seam, would deliver a capability naming a frame the
revoker already took back. That is the cited test's use-after-free, one step earlier.

The sender arms the seam from inside itself and parks there with no lock held. The test revokes the
frame and asserts that the sweep deleted the source out of the sender's own table, so a revoke that
never reached the sender cannot pass for one that worked. After the release, the delegation must
answer `NoSuchSlot` and park nothing. The control run drives the same seam without the revoke: the
delegation completes, parks, and a `RECEIVE_CAP` collects exactly the frame. That is what makes the
refused run's emptiness the revocation talking. The seam makes the interleaving certain rather than
timed, and every wait is clock-bounded.

The second is the `PortRange` sweep on `x86_64`. It carries the same hand-off-clearing lines as the
`PageFrame` sweep, and nothing else drives it through a parked hand-off. A sender parks a port
capability on a rendezvous, the whole COM1 range is revoked, and the receiver must be handed nothing
that names the range. A delivery would be a revived port grant on the one architecture whose I/O
space it can reach.

## Directory attenuation and mirrored scripts (claims 19 and 24)

The cited tests prove the read-only grant, the full grant and the two-roots crossing with scripts
matched to each root.

For claim 19, the cited read-only grant carries `DESCEND | READ | ENUMERATE` together. Two grants
each drop exactly one of those rights. A `Rights::allows` that checks one right too generously, or
one too narrowly, moves exactly one bit of the verdict, and both directions are asserted.

For claim 24, each shell runs the script written for the other root. Every probe aims at a name
that exists only outside its own subtree, absolute paths included. The whole script must still
complete its root-relative skeleton (`ALWAYS`), so its refusals prove something, while reaching
nothing of the tree it probes. The caretaker, the protocol and the shell are portable programs.

## Forged ELF images (claim 22)

The cited tests refuse a segment that starts in the kernel half and a segment whose flags are
exactly `R|W|X`. This module forges the shapes around those.

- A segment whose `vaddr` plus `memsz` overflows `u64` must be refused as an overflow, not wrapped
  into a small image that loads.
- A segment in band whose `memsz` reaches one page past the kernel half's start is the `.bss`
  trick: sixteen bytes of file and a huge claimed size. No addition overflows, so the refusal must
  come from the image placement or the half check. An `Unmappable` refusal would be an accident of
  a single page, and the test rejects it.
- A two-segment image puts a benign segment first and one naming the kernel second. A loader that
  inspects only the first program header, or stops sizing at the first segment, would pass it.
- `R|W|X` with `PF_MASKOS` bits set. The standard says those bits are ignored, so a check written
  as equality against a known flag set, rather than as membership, would let the combination
  through unchecked.

The band, the halves and `load` are portable over each ISA's `Mapper`.

## A client upgrading itself (claim 26)

The cited tests stop a `WRITE`-only client at the rights gate (`confinement_attack_tests`) and at
the live-swap usurper's `RECEIVE_CAP` (`live_swap_tests`). This module tries the two roads around
that gate a patient client would take next, both through the real dispatcher.

Self-upgrade by delegation: a client holding `WRITE | GRANT` holds every right a client can be
trusted with. It may send, and it may lend what it holds. It asks `SEND_CAP` to mint itself a copy
carrying `READ` too, narrowed and widened in one word. The delegation rule must refuse, because
`READ` is not a subset of the source's rights and no right can be minted out of the `a1` register.
Had it succeeded, the client would receive on the next line and be the server.

Badging its way in: `BADGE` is the one method a `GRANT`-holding client legitimately has. A client
that badges its own endpoint capability and tries `RECEIVE`, `RECEIVE_CAP`, `REAP` and `SURVEY`
through the copy must be refused each one. The badge labels a sender; it is not a right.

## Kernel text and the direct map (claim 21)

The cited test reads one kernel static through its link-time virtual address. This module aims
`kernel_test_subject` at two other views of kernel memory a real attacker would reach for first:
the kernel's own code, and the direct map. The direct map is the kernel's writable alias of
physical memory, where the sentinel's own page appears a second time. The sentinel static is kept
as a third target, so the faults cannot all be about a mapping unique to this file.

On `x86_64` the direct map does not cover every page the kernel image occupies, so the alias leg
runs on aarch64 and riscv64. Near miss 4 of the pass explains why that is deliberate.

A precondition asserts each target is mapped for the kernel (`mmu::translate`), so a refusal to
fault cannot hide behind an unmapped address. The fault is read with up to three spawns, because
`last_user_fault` is one global slot. On a loaded machine an unrelated thread's non-page fault
records itself with address 0 and can overwrite the record between the fault and the read. The
escape signal is different and unambiguous: a subject that never dies, because its read never
faulted. The kind may be `Permission(Read)` or `Translation(Read)`. The page may be present but
supervisor-only, or absent from the low half, and that varies by target and ISA. The claim requires
only that each read was stopped as a read at the exact address handed in.

## Port ranges (claims 27, 28, 29 and 33)

The cited tests drive a non-holder's `out`, a revoked holder's `out`, a self-deleting holder's
`out`, and a `READ`-only holder's `out`. This module attacks the sides those leave, on `x86_64`
only, because port I/O is.

The read side covers claims 27 and 33. A holder transmits first, so its grant was really installed
in some core's TSS bitmap. Non-holders then run `in` against the same port, and each must fault at
the `in` itself; a bitmap left over from the holder would carry them to a clean exit. A `READ`-only
capability stops port I/O at the first instruction. The TSS bitmap cannot split reads from writes,
so without `WRITE` nothing is installed, and the claim's "no output" holds the stronger way. A
full-rights holder running the same stub exits cleanly, which proves the refusals were the rights
gate and not a broken stub.

Revocation precision is claim 28. Two holders of the whole COM1 range park on their own wake
endpoints, so the test picks the instant to revoke. One wakes after its exact range was revoked and
must fault. The other wakes after an unheld range was revoked and must still write; a sweep that
matched too widely would stop it. A sub-range revoke is not a leg. The sweep matches by exact
`(base, count)`, and `PortRange::REVOKE` takes its range from the capability, so no syscall can name
a narrower one. The frame sweep's overlap matching (`delete_page_frame_caps_overlapping`) is the
deliberate contrast. This is near miss 1 of the pass: one design change away from an escape if a
sub-range revoke ever becomes expressible.

The duplicated holder is claim 29. A child holds one range in two slots. Deleting one slot clears
the thread's cached TSS grant wholesale, so the surviving capability's `out` faults although the
table still holds it with every right. That is near miss 2, and claim 5's "dropping one capability
does not disturb the others" fails at this corner in the fails-closed direction. Nothing was reached
that was not granted. The test asserts the behavior as it stands, so a fix turns it red: a delete
that checked whether another capability still named the range would leave the twin working. Its
second leg deletes both slots and expects the fault on the `out` itself.

Every child exits or faults promptly, so a wrongly permitted port is red as `EVENT_EXIT` where
`EVENT_FAULT` was asserted. Parked children wait on a wake endpoint of their own and never compete
with the supervision endpoint the deaths arrive on.

## Domain scope (claims 6, 7, 8 and 9)

The host crate proves the four claims on pure logic:
`reap_is_permitted_only_to_the_supervising_rendezvous`, `a_stranger_reveals_nothing_about_its_liveness`,
`a_survey_shows_exactly_the_endpoints_own_children` and `the_view_and_the_reap_have_the_same_scope`.
The reap and survey tests drive them through the real dispatcher. This module attacks them as one
cross-domain forger would. A fully privileged supervisor of its own domain, holding `READ` and
`ENUMERATE`, reaches sideways at another domain's members, alive and dead, through `REAP` and
`SURVEY`. The observable is whether any answer differs from the same probe at its own members.

The liveness attack matters most. A refusal that said `StillAlive` for a live stranger and
`NotSupervised` for a dead one would leak a stranger's liveness through an endpoint its builder
never named. That breaks claim 7 without a single right being gained, so the two refusals must be
the same error word for word. Every probe is non-blocking or aimed at a settled state.

## Slot lifecycle (claims 4, 5, and 2's slot half)

Claim 4 says a consumed capability cannot be used again. Claim 5 says dropping one capability does
not disturb the others. The host crate proves both on pure logic
(`capability::a_deleted_capability_stays_deleted`, `delete_touches_only_its_slot`). This module
drives the live kernel paths instead: `SYS_CAP_DELETE`'s kernel half, `delete_current_cap`, followed
by invokes through `syscall::invoke`. It tries three angles a host crate cannot reach. One is a
stale slot number, held by a program that watched its capability be deleted. One is an out-of-range
index forged in a register. One is a deleted slot whose number the table later hands out again. The
control is a `READ` capability on the same object, granted after the deletion. It still receives a
message the deleted capability sent, so deletion consumed the one capability, not the object or its
neighbors' access to it. Every capability is deleted and every region reclaimed, so the ledgers end
flat.

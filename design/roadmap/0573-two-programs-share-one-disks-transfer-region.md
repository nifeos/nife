---
status: NOT-STARTED
raised: 2026-09-21
promoted_from: two-programs-share-one-disks-transfer-region
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 573. Two programs share one disk's transfer region, and only an ordering keeps them apart

The number is **provisional**: the integrator mints it at merge. Promoted from the proposal `two-programs-share-one-disks-transfer-region` on 2026-09-22, filed 2026-09-21. Raised by the lane `abboot/confirm-a-trial-boot`, which had to
write a live partition table on a machine whose filesystem server already held the same disk. It
found that nothing but where the call sits stops the two corrupting each other.

The two candidate fixes are both values two programs agree on (a field on the
`blk` wire, or a second shared region in a server's handoff). `AGENTS.md` puts those in the
irreversible category. The mechanism either way is small; the agreement is not.

## The gap, in one sentence

One block server has **one** transfer region shared by every holder of its request endpoint; a
client stages bytes into it *before* it calls, so two clients staging at once corrupt each other
and nothing in the system prevents it.

## Where it bites today, and where it does not

It does not bite anywhere today, and saying why is the point. The kernel boot path is
single-threaded and hands a disk's endpoint to one program at a time. It waits for each one's report
before wiring the next. `install_service`'s survey, its installer, `mkfs`, and now its confirming
write all run in that shape.

The confirming write is the first one that runs while another program already holds the same
endpoint. `user::install_service::confirm` sits between the filesystem server's ready report and
the progenitor's first instruction. What makes it safe is that in that window the filesystem server
is blocked in receive, with no client that could wake it. That is a written record at the call site
rather than a mechanism. It is rung three of `AGENTS.md`'s ladder where rung one is available, and
it is recorded there as a foot gun.

The failure it would produce is the worst kind this tree has. It is a corrupted partition table on
somebody's installed machine, arriving as a disk that no longer boots. There is no crash and no log
line, and it comes days after the change that made it possible.

## The two shapes, neither priced

| | |
|---|---|
| **a transfer region per client** | the server maps a region per endpoint holder instead of one per device. Costs a page per client and a way for the server to tell its clients apart, which `file_system_contract::blk` does not currently give it |
| **a `blk` endpoint bounded to a block range** | a client is handed an endpoint that can only touch blocks `[first, last)`, which is the capability answer and also fixes a second thing: `system_installer`'s `BUGS` records that the filesystem server on an installed machine holds the *whole disk*, bounded only by an extent in the filesystem's own header |

The second is the one this project's shape argues for, and it is not obviously the smaller change.
Neither has been measured, and pricing them is most of the work. That is why this is a proposal
rather than a recommendation.

## What is blocked until this is answered

Nothing that is built, and that is the honest answer. What is blocked is anything that wants to
touch a disk from a program while a filesystem server holds it. That is every shape an upgrader
takes. Writing the spare slot on a running machine is exactly this problem one step larger. An
upgrade is megabytes rather than four blocks, and cannot hide inside a window between two boot
steps.

## BUGS

- This proposal has measured neither option, for the reason above. A lane taking it should price
  both before bringing the fork. It should expect the bounded endpoint to win on being the answer
  to two problems rather than one.

## Index row

One block server has one transfer region shared by every holder of its request endpoint. A client stages bytes into that region before it calls, so two clients staging at once corrupt each other and nothing prevents it. It bites nowhere today only because the boot path is single-threaded and hands a disk to one program at a time. The confirming write is the first that runs while another program holds the same endpoint. What makes it safe is a comment at the call site. Both candidate fixes, a field on the `blk` wire or a second shared region in a server's handoff, are values two programs agree on.

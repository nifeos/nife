---
status: BUILT
raised: 2026-09-26
built: 2026-09-27
promoted_from: a-frame-per-filesystem-client-channel
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 599. A frame per filesystem client channel

Minted 2026-09-26 by lane `milestone/599-fs-client-page`, promoted from the
proposal `a-frame-per-filesystem-client-channel`, which milestone 47 (navigation and naming)'s lane
wrote on `milestone/47-navigation` (#1343). The work was first named as case A of
`notes/shared-page-audit.md` by milestone 43 (a second security audit) and never got a number.
calef ruled option A (badged endpoints) on 2026-09-26; the same lane built the kernel mechanism, the
file server's K windows, the kernel-harness pool, and the witness that proves isolation on all three
architectures. What is left is the production progenitor's pool, which has a design question of its
own and its own gate; see "What is left". *(Number and title provisional: the integrator mints the
number at merge, and the title is a draft until an architect names it.)*

The proposal said the work had no gate, "a wiring change inside the tree". Reading the server said
otherwise: it receives on one endpoint, learns nothing about its caller, and reads every request from
one window. A per-client frame it can read needed badged endpoints, a kernel remap at the
rendezvous, or a token convention among the page's writers, and calef chose badged endpoints. The
second fork, how the progenitor holds the windows, is open; see "What is left". Neither has a section
in `design/decisions/` yet; the integrator mints them from the note.

## The finding, checked 2026-09-26

The file service shares one read-write staging channel (`fs::TRANSFER_PAGES`, 16 pages, 64 KiB)
with every client a boot wires. In the kernel harness it is `FILE_SHARED`, memoised by
`fs_service::ensure`. On the interactive boot it is the progenitor's `BootEndowment::fs_page`,
which the shell maps at `SH_FS_VA` and every `build_caretaker` chain maps at `FS_CLIENT_PAGE_VA`,
caretaker and confined program both (`crates/system_initializer/src/lib.rs`). What keeps clients
apart is which of them is blocked, not what they can map.

It is a correctness bug as well as a confinement one. A client stages its bytes and then calls, so
two honest clients active at once overwrite each other; `notes/pipes/the-file-end.md` is why the
shell writes a `>` file itself rather than letting a second process do it. Today every path keeps
one FS client active at a time. The set grant at the prompt would not.

## What is built (2026-09-26, option A)

calef ruled option A: a server tells its clients apart by a badge the kernel delivers.

- **The kernel mechanism.** A capability's badge rides on `Object::Rendezvous(id, badge)` (no size
  growth; `Cap` stays 32 bytes). `abi::rendezvous::BADGE` (method 7, provisional) mints a badged
  copy of an endpoint, GRANT-gated, set once. `RECV_CAP` returns the sender's badge in x3, carried
  in the delivered mailbox's word 3, which was already stored as a zero on the CALL and SEND_CAP
  fastpaths, so the only added instruction is `set_arg(3)` on the RECV_CAP return.
  `crates/user_mode_runtime` gains `badge` and `recv_cap_badged`.
- **The file server's K windows.** `filesystem_protocol::fs::CLIENT_WINDOWS` (8, provisional) staging
  windows; `redoxfs_server` reads window `badge` per request. Window 0 is the unbadged default, so
  single-client paths are unchanged.
- **The kernel-harness pool.** `fs_service` allocates the K frames once, maps them into the server,
  and hands them out with `claim_window`/`release_window` (release zeroes the frame: the take-back
  half). The witness gives each client its own window and a badged endpoint, so the two map
  different frames; its assertion flipped from substitution to isolation. It exercises both shapes:
  the victim mints its own badge, and the attacker is handed a pre-badged endpoint.

## What is built (2026-09-27, the production pool)

calef ruled option 4 of the pool's fork on #1413 (2026-09-27 15:29Z): one run capability over the
windows, and a new `PageFrame` method that derives a capability naming part of it.
`notes/page-frame-slice.md` is the method's semantics write-up, in the shape of §102
(a Frame names a run of pages).

- `abi::page_frame::SLICE` (method 2, provisional), GRANT-gated, on all three architectures (it is
  portable kernel code). `kernel::system_call::tests::a_slice_maps_only_its_window` proves that a slice
  maps its window and neither neighbor.
- `fs_service` allocates the windows as one contiguous run, and the progenitor's slot 6 names the
  whole run.
- The progenitor gives each job behind a directory grant its own window, sliced for the job and its
  caretaker, with the file service's endpoint badged to match. The window is zeroed before reuse.
  The shell, `login`, the identity provisioner and its own activation calls stay on window 0.
- `script/swish-check` passes on all three architectures and reads 23 of 24 capability slots at
  peak on aarch64 and riscv64, as before (x86_64's gauge reads the hand-over mark, not the peak). The provisioner's slice
  is made after its address space is built, which is what keeps the table's peak where it was.

## What it costs

From constants, not a boot: 64 KiB of contiguous memory per concurrent client channel, where the
whole boot shares one 64 KiB channel today. Carved from a directory job's region
(`DIR_JOB_REGION_PAGES`, 96 pages), that adds about 17% to each job. Per-request cost depends on the
option: nothing added under A, a window remap under B, two rendezvous and a copy under C.

## What it unblocks

Milestone 47 (navigation and naming)'s set grant at the prompt, which names this as its
prerequisite. A ruling on option A would also decide option 1 of the proposal
`every-client-of-a-network-stack-shares-its-socket-numbers`, which is the same question for sockets.

## Follow-on

- **Done.** The kernel badge mechanism, the file server's windows, the kernel-harness pool, the
  witness, and the production pool. See both "What is built" sections.
- **Recorded.** A slice escapes its source's revocation (the shared-base, different-length
  case of §132 (what `PageFrame::REVOKE` owes an overlapping run)); nothing revokes the pool, so it is a limitation, recorded in `notes/page-frame-slice.md`.
- **Recorded.** The progenitor reuses a window after seven more granted jobs whether or not its last
  holder exited, because it is not told when a job dies: `Windows` in `crates/system_initializer`.
- **Decision.** calef ruled option A (badged endpoints) on 2026-09-26 and option 4 (the pool) on
  2026-09-27; the maintainer mints both records under `design/decisions/`.

## Index row

Every file-service client gets a staging window of its own, and the server tells them apart by a
badge on the endpoint; the progenitor slices each client's window out of one pool capability.

# Auditing the shared pages: time of check to time of use

<!-- The writing-standards exception marked here on 2026-09-29 (UTC) was removed on 2026-10-11 (UTC)
by lane/next-ten-longest-notes. Its reason was the bold density of a 6,457-word report. The §212 (a prose budget)
split moved the findings into appendices, and the page now meets every limit outright. -->

Done 2026-08-04, as the tree's second security audit. The first
([arch-audit.md](arch-audit.md)) read the hand-written architecture assembly and found three bugs in
the class "state staged in single-copy hardware registers across more than one instruction." This
one deliberately does not re-read a line of that, because the value of a second audit is the lens
the first one lacked.

## The lens, and why this one

Every service contract in this system moves bulk data through a page shared with the client
(DECISIONS §10: control by message, bulk by shared page). That shape did not exist when the first
audit was written; the compositor's surfaces, the C seam, `std::fs`, `std::net`, the FS service and
the sink contract all arrived afterwards, and the attack surface roughly doubled.

The bug class this audit hunts, stated generally:

> A value that a server checks and a value that a server uses are two different reads of
> memory that a party other than the server can write in between.

That is the double fetch, and it is invisible to every gate we run, because both the check and the
use are individually correct. `script/verify` proves the pure-logic crates and cannot see across an
address-space boundary. `script/test` runs one thing at a time. Nothing in the tree today can fail
because of a window that only a second runnable writer can enter.

The question asked of every request handler was therefore the arch audit's four, transposed:

- (a) The window. Where is the check, where is the use, and what is between them?
- (b) What can land in it. Which processes hold a writable mapping of that frame, and is any of
  them runnable at that moment?
- (c) The corrupted state. What does the server do with the value it did not check?
- (d) Reachable? Is it closed by a mapping, by the blocking-`CALL` rendezvous, or only by the
  fact that nobody has built the wiring that would open it?

(d) is again the interesting one, and again the honest answer for most of the tree is "closed by
something other than the check."

## What was audited

Every place a frame is mapped into more than one address space, and every loop that serves requests
against one.

| Contract | Server | Client(s) | Files |
|---|---|---|---|
| blk IPC | block server | FS server | `crates/filesystem_contract` (`blk`), `components/src/block_driver.rs`, `redoxfs_server/src/bin/redoxfs_server.rs` |
| file IPC | FS server | every FS client | `crates/filesystem_contract` (`fs`, `xattr`), `redoxfs_server/src/bin/redoxfs_server.rs` |
| file IPC, narrowed | the three caretakers | one confined program each | `components/src/fs_file_caretaker.rs`, `fs_subtree_caretaker.rs`, `fs_nameset_caretaker.rs` |
| the sink | `fixtures/src/file_sink.rs`, `fixtures/src/file_source.rs` | a redirected program | `crates/byte_sink_contract` |
| the serial terminal | `components/src/line_editor.rs` | the shell | `crates/line_editor` |
| the console | `components/src/console.rs` | its client | `kernel/src/user/console_service.rs` |
| the display | `components/src/gpu_driver.rs` | painter, terminal, compositor | `crates/graphics_contract` |
| the compositor | `components/src/compositor.rs` | window clients, the input source | `crates/compositor` |
| the display terminal | `components/src/display_terminal.rs` | an application | `crates/video_terminal` |
| credentials | `components/src/credentialer.rs` | provisioner, verifier | `crates/credential_contract` |
| the wall clock | `kernel/src/user/clock_service.rs` | The progenitor, the shell, `date` | `crates/clock_contract` |
| the C seam | `fixtures/src/c_shim.rs` (C) | `fixtures/src/c_confiner.rs` | `crates/c_seam`, `fixtures/c/c_seam.c` |
| the input ring | the compositor | the keyboard driver | `crates/compositor` (`proto::ring`) |
| sockets | `components/src/net_stack.rs` | a client, `std::net`, `network_time_client` | `crates/socket_contract` |
| the virtio DMA regions | four userspace drivers | the **device** | `components/src/virtio_net_transport.rs`, `kbd.rs`, `entropy.rs`, `display.rs` |

The last row is not a process pair and is in the table on purpose. A DMA region is a page one party
writes and another reads. The other party is a device rather than a program, and the question this
audit asks does not care which.

## What was deliberately not examined

Stated because a scope nobody wrote down is a scope nobody can check.

- The arch and assembly layer. That is [arch-audit.md](arch-audit.md)'s, and re-reading it would
  be the failure this milestone exists to avoid.
- Capability lifetime races between revocation and an in-flight use (generational names,
  `MemoryRegion::DESTROY`, `Rendezvous::REAP`). Named in the milestone as a candidate lens and left for a
  later one; it is a different question and mixing it in would have diluted both.
- The census of `unsafe`. A candidate lens, a whole audit of its own.
- The supply chain, the boot trust root, and the DMA/IOMMU descriptor validator. The last has
  its own machine-checked proofs (`crates/dma_validator`, DECISIONS §30) and reading it by hand
  would add nothing a prover has not already said for every input.
- The Kani bounds. Whether a proof's chosen bound is the right bound is
  [verification.md](verification.md)'s question, not this one's.
- Anything that requires already being the progenitor. SECURITY.md puts it out of scope and this audit
  honors that: the progenitor is unverified and privileged by design.

### And three things that moved under this audit

An audit reads a commit, not a project. This one read `main` at `313a055`, and three areas changed
in flight; each is named so the clearance above is not read as covering work it never saw.

- The inbound socket half (`LISTEN`/`ACCEPT` with a spawn-time port grant) is not on `main`.
  `crates/socket_contract` there stops at `OPERATION_CLOSE` and `net_stack.rs` has no listener. What is
  audited here is the outbound contract only. The `net_transport.rs` finding below applies to both,
  the file being identical across them. (It has since landed: `crates/socket_contract` now carries
  `LISTEN`, `ACCEPT` and `LISTEN_DENIED`; noted 2026-10-15 UTC.)
- `crates/credential_contract` and `components/src/credentialer.rs` are being substantially rewritten with an
  NTLM path. The clearance recorded below is of the version on `main` and does not transfer.
- The clock page's seqlock has a live finding of its own from another lane (see finding 7's last
  paragraph). This audit did not re-derive it and does not claim `clock_contract` is clear; it uses that
  page only as the in-tree precedent for the acquire side.

## The structural fact that saves most of the tree

Worth stating before the findings, because it is the reason there are so few.

Lengths, offsets, counts, handles, opcodes and rectangles all travel in the IPC register words,
never in the page. The kernel copies a message's words into its own state at `SEND` time and hands
them to the receiver in registers, so by the time a server sees them they are in memory only that
server can write. `filesystem_contract::fs::req` packs opcode, handle and a 40-bit length into one word;
`graphics_contract` packs a whole rectangle into four 14-bit fields of one word; `credential_contract` packs two
lengths into one word. There is no contract in this tree whose length field lives in the shared
page. That removes the entire classic form of the bug (read a length from the page, bound-check
it, read it again to size the copy) by construction rather than by care.

Three consequences fell out of the sweep and bound the search:

- Every payload length is clamped to the page at the top of its serve loop and the clamp is a
  local, not a re-read. `handle` in `redoxfs_server/src/dispatch.rs`, `let len = fs::req_len(w0).min(BLOCK);` is the pattern,
  and the three caretakers, `line_editor`, `display_terminal` and `sink` all repeat it.
- The one contract whose decode lives in a host-testable crate is the one with a proof.
  `credential_contract::read` takes the page and the register word, checks both lengths against their
  maxima, and returns two subslices; `crates/credential_contract/src/lib.rs` carries a harness asserting the
  returned slices' lengths match the word and stay inside the page. Every other contract's decode is
  inlined into a `no_std` serve loop, where neither a host test nor Kani can reach it. That is rule
  7's argument arriving from a new direction.
- The check-free caretaker is the one with no window. `fs_subtree_caretaker` performs no name
  check at all (its attenuation lives entirely in the handle the FS server minted for it), and it is
  therefore the only caretaker that cannot have this bug. Its own doc comment argues for that design
  on simplicity grounds; this audit is the security argument for the same choice.

## Findings

Seven findings, each worked through the lens's four questions in an appendix. The honest summary
below has the table of all seven and their dispositions.

- [Findings 1 and 2](shared-page-audit/filesystem-findings.md): the FS service's one shared frame is
  mapped read-write into every client a boot wires, and `fs_nameset_caretaker` checked a name and
  forwarded it without re-staging it. They are one bug seen from two sides. Finding 1 became
  reachable on 2026-09-27, when the prompt began building a nameset caretaker on that frame.
- [Findings 3 and 6](shared-page-audit/console-and-virtio-findings.md): the console server took an
  unbounded byte count from its client, and two virtio drivers trusted the buffer index the device
  writes into the used ring. Both are fixed; the second fails closed.
- [Findings 4, 5 and 7](shared-page-audit/compositor-findings.md): unchecked arithmetic in the
  compositor's window client, the compositor reading surfaces whose owners are not blocked, and the
  compositor as the one reader in its subsystem with no acquire fence.

## Candidates cleared

"We looked and it is fine" is the other half of an audit. The FS server's request decode,
`GETXATTR`, `credential_contract::read`, the display driver's `FLUSH`, the keyboard ring, the
compositor's damage handling, `line_editor`, the C seam, the clock page, the nameset grant frame and
the socket contract's decode were each cleared. Each is a place a future change could break
something. One warning is worth carrying here: the socket page's `OFF_LEN` is a length field that
nothing reads, and the first change that honors it adds a double fetch that will look like a
tidy-up. Why each is safe is in
[shared-page-audit/candidates-cleared.md](shared-page-audit/candidates-cleared.md).

## The honest summary

| | What | Disposition |
|---|---|---|
| 1 | One FS shared frame for every client a boot wires, read-write in all of them; the property that keeps them apart is a scheduling argument, not a mapping | **Proposed as a milestone** |
| 2 | `fs_nameset_caretaker` checks a name off the page and forwards without re-staging it, so the FS server reads it a second time | **Fixed**, and the residue named |
| 3 | The console server's byte count is unclamped, and its `SAFETY` comment blames the wrong party | **Fixed**, comment corrected |
| 4 | `window.rs`: a `u32` overflow defeats its own geometry check, and an unclamped shift count | **Fixed**, both |
| 5 | The compositor reads surfaces and damage rectangles of clients that are not blocked | **Recorded-accepted**, in notes/compositor.md's BUGS |
| 6 | `net_transport` and `keyboard_driver` index a DMA buffer with a `u32` the device wrote, unchecked; the DMA validator covers the other direction | **Fixed**, fail-closed, with the hostile-device harness proposed |
| 7 | Three release-side fences in the compositor's subsystem whose acquire side did not exist | **Fixed**, two acquire fences |

**Nothing found was a live privilege escalation reachable from a confined program on `main` on the
day this audit ran (2026-08-04)**; the security audits after it carry the current word.
That sentence has to be read with all three of its qualifiers, because two findings fall outside
them. Finding 6 needs only a device that lies, and finding 7 needs only hardware that reorders. Both
are the parts of the threat model this system has declared and not yet met.

The limit is the same one the first audit recorded and is sharper here:

**This lens finds windows, and a window nobody can enter looks exactly like no window.** Findings 1,
2 and 5 were closed by something other than the check that was supposed to close them. It was a
wiring that happens to have one client, a `CALL` that happens to be blocking, a constant that
happens to bound a slice. Every one of those is true. Not one is written down as a requirement
anywhere the next change would have to read. That is the shape of arch-audit.md's finding 1 (safe by
an unstated invariant, not by construction), and finding 1 here is its direct descendant.

Two shapes recurred often enough to be worth naming as patterns rather than as bugs:

- **A guarantee assumed from the wrong side of a boundary.** The DMA validator confines where a
  device may touch and the driver read that as "the device cannot lie" (6). The blocking `CALL`
  parks the caller and the compositor read that as "no client is writing" (5). The producer's fence
  orders the producer's stores and the reader read that as "these loads are ordered" (7). In all
  three the guarantee is real, and it is a guarantee about somebody else.
- **One of a pair, written by instinct.** Three release fences with no acquire (7); a length clamped
  at four sites in one program and none in its sibling (3); a filter that copies what it checks and
  does not write back what it copied (2). Each is half a discipline, and half a discipline reads
  exactly like the whole one.

The single most useful thing this audit can hand forward is therefore not a bug but a rule, and it
is already followed in two places in the tree:

> **Copy what you check, and check what you use.** A server that validates a value out of a shared
> page and then acts on that page has validated a different thing. `fs_nameset_caretaker` copies its
> grant set out of the read-only page at `_start`; the FS server copies an attribute name to the
> stack before it writes the reply over it. Both do it for local reasons and both are right for this
> one.

## What wants a lane

Written as cases rather than as numbers: minting a milestone is the integrator's, and these are
proposals.

**A. One shared frame per client channel, not per FS service.** The case is finding 1. `ensure()`
memoises a single frame and hands it to every client the boot wires, so the FS service's page is
read-write in up to ten processes at once and what keeps them apart is who happens to be blocked.
The fix is a frame per channel: allocate in `spawn_fs_client` / `start_granted*` / `narrow_dir`
rather than in `ensure`. That also removes the ordering hazard `wait_for_caretaker` was written to
patch, because a caretaker's staging page would no longer be reachable by anything but its own
chain. The witness is the deliverable, not the wiring: two live confined programs on one FS
service, one of them substituting the other's name mid-request, failing before the change and
passing after. Severity is what makes it worth a lane rather than a note. It is a confinement escape
in the exact terms SECURITY.md puts in scope. It moved from latent to live when the shell could
ask the progenitor to build a caretaker, which happened on 2026-09-27 (finding 1(d) in
[filesystem-findings.md](shared-page-audit/filesystem-findings.md); corrected 2026-10-11 UTC, since
this case had said that day was still ahead). Milestone 599 (a frame per filesystem client channel)
is this case's lane, and it built on 2026-09-27. Calef ruled badged endpoints. The witness proves
isolation on all three architectures. What remains is the production progenitor's pool, 599's own
"What is left" (noted 2026-10-15 UTC).

**B. A harness that can make a virtio device misbehave.** The case is finding 6 and the honesty gap
under it. This tree tests DMA confinement by making the *driver* attack (`crates/virtio`'s
`run_attack` and its indirect-descriptor cousin, both proving the kernel refuses), and it has no way
to make the *device* attack. So the direction the IOMMU and the validator were built for is the one
direction with no negative control, and the two fixes committed in this lane are unproven for the
same reason. Something that can write an arbitrary used ring under the driver, whether a fake
transport behind `crates/virtio`'s trait or a QEMU device model, would give every driver a hostile
counterparty and would be reusable by every later one. It is also what a second board makes urgent.

**C. A lint for one-sided fences.** The case is finding 7 plus milestone 80's. Both are the same
mistake and neither was visible to any gate, because a fence with a comment explaining it looks
correct whichever side is missing. A cheap version is mechanical: for each shared-page contract,
require that a documented publish/observe pair name its counterpart, and fail when a `fence` in one
program has no partner in the program on the other end. Even a list in a note that says "these are
the pairs, here is where each half lives" would have caught both, and this audit produced most of
that list on the way past.

---

*See also [arch-audit.md](arch-audit.md) for the first audit and its lens, and
[security.md](security.md) for the review after milestone 11 (untyped memory). [fs-server.md](fs-server.md) and
[glob-grant.md](glob-grant.md) hold the contracts finding 1 and 2 live in, and
[compositor.md](compositor.md) holds finding 5. `SECURITY.md` says what this project claims and
what it does not.*

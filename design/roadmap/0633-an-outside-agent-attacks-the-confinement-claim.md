---
status: BUILT
raised: 2026-10-03
built: 2026-10-06
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 633. An outside agent attacks the confinement claim

Raised 2026-10-03 (UTC) by calef, when he ruled risk 7 (the confinement claim is false) AMBER on
#1495. The number is minted by the maintainer; the title and slug are drafts. calef's ask: task an
agent, likely on Fable (model id `claude-fable-5-1`), to be risk 7's adversarial review.

## Why

Risk 7's amber has one named cause: every test and audit of confinement was written by the people
and the model family that built it. The in-house passes found real defects (tests that could not
fail, three times; claims false in milestone 313 (the security audit that was due since August) and on 2026-09-21) and no escape on a
component's own authority. What nobody has run is an attacker who was not in the room.
`design/fatal-risks/README.md` gates the human outsider behind milestone 198 (a package manager,
and the trivial install that makes a second customer possible). This milestone is the half of
that experiment that needs no stranger.

## The experiment

One reviewer agent, briefed with two things only: the 30 published claims in
`notes/confinement-claims.md` and the repository. Independence is the point, so these are the rules:

- No lane reports, no prior audit conclusions, and no hint of where defects were found before. The
  brief names no file under `design/audit-reports/` and none of the earlier falsification notes.
- A different model from the lanes that built and audited confinement. The first choice is Fable.
- It may write exploit programs and run them under QEMU on all three ISAs (aarch64, riscv64 and
  x86_64), through `helpers/qemu-bounded.sh`.
- It reports each attempt as exactly one of: an escape, a near miss, or a claim it found untestable
  (with the reason).

## Premise check

An agent from the same vendor is a weaker outsider than an independent human red team. Shared
training may mean shared blind spots, so a class of defect the builders could not see may be one the
reviewer cannot see either. That bounds what the result can do.

- A clean result moves nothing by itself. It supports that thirty claims survived a second, differently
  trained attacker, which is evidence of the same kind as the audits, a step further from
  self-review. It does not support green, and a green still needs the human half.
- An escape moves the verdict toward red, and is the more useful outcome.
- Untestable claims are findings: each becomes a rewritten claim or a recorded gap.

Record this as the first outsider pass and not the last. The stronger forms are an external human
review, or a public bounty once a stranger can install nife, which waits on milestone 198.

## Done means

- Every one of the 30 claims has an attack, or a written reason it was not attacked.
- Every escape becomes a failing test before any fix is written, in the shape milestone 202 (every
  confinement test is a ritual until somebody breaks the confinement and watches it fail) set.
- The results go into risk 7's appendix (`design/fatal-risks/the-confinement-claims.md`) through
  the maintainer, who may correct facts under §216 (fatal-risk facts are correctable, and verdicts
  are the architect's). Moving the color stays calef's.

## Cost

An estimate, not a measurement: one long reviewer session per ISA plus the fixes it prompts. The
review itself is a few million tokens at most. The $200 monthly budget is the constraint, so run it
as one lane after the current queue drains and not beside other lanes. QEMU time is small.

## Dependencies and risk link

Nothing blocks it. It cites risk 7 and is the owner of that risk's remaining half in the running
order of `design/fatal-risks/README.md`.

## BUGS

- The reviewer reads the same repository the builders wrote, comments included. Comments that
  say where a check lives are a hint the brief cannot remove.

## Follow-on

- **Recorded.** The human and bounty forms are written in the premise check above, and wait on
  milestone 198.

## What the first outsider pass found (2026-10-03 UTC)

A Fable reviewer attacked the confinement surface by reading the enforcement and, where it could
prove a result cheaply, by a test. It left the three-ISA QEMU sweep undone; the third pass below
ran it. The human-outsider half is milestone 198's, as the premise check above says.

### One escape, found and fixed

**A plain `RECEIVE` that collects a parked `SEND_CAP` sender left the sender's delegation staged, and
the sender's next plain `SEND` delivered it to a `RECEIVE_CAP` on another endpoint.** A capability
granted to one endpoint reached a receiver on a different one, which is a process reaching an object
it was not granted. It is the non-abort sibling of the hazard the 2026-10-03 security-audit follow-up
closed in `set_ipc_aborted`: the teardown path clears `outgoing_cap`, but the successful-collect path
in `sched::ipc_receive` does not go through `set_ipc_aborted`, so the clear was missing there.
Milestone 634 (a plain SEND received by RECEIVE_CAP never hands the receiver a sender-chosen slot)'s
`cap_delivered` guard does not catch it, because that guard covers the
receiver-parks-first order and this leak takes `outgoing_cap` on the sender-parks-first immediate path
of `ipc_receive_cap`.

- Fixed in `sched::ipc_receive`'s `!leave_blocked` branch (one line, same rationale as
  `set_ipc_aborted`; the sender keeps its own table copy).
- Proven by `system_tests::user::receive_cap_attack_tests::a_send_cap_collected_by_a_plain_receive_stages_nothing_for_a_later_plain_send`,
  red before the fix, with a replayable falsification patch.

Milestone 634 merged as #1503 earlier the same day, so there was nothing to sequence against.

### Every claim, attacked, and what each ISA actually evidences

The sweep is gated in CI, not on this machine: the `test` target boots the kernel suite on aarch64,
riscv64 and `x86_64`, and `ci.yml` and `verify.yml` were dispatched on the branch. A host proof is
one artifact for every ISA by construction. A kernel test runs on each ISA the table names, but its
falsification record names one architecture, so "runs on three, recorded on one" is the honest
reading wherever it appears. That gap is milestone 323 (the falsification record is incomplete in
five ways, and each was found by a different lane)'s and is not re-raised here.

| # | Attack this pass ran | ISA evidence | Outcome |
|---|---|---|---|
| 1 | Looked for a derive, put or insert path that stores more than the source holds | host proof, every ISA | held |
| 2 | Traced every kernel site turning a user register into `Rights`: `SEND_CAP` and `CAP_INSERT`, both via `from_bits` then the subset check | host proof plus kernel read, every ISA | held at the boundary, not only in the crate |
| 3 | Looked for a mint site outside `derive` that takes a rights argument; `mint_child` takes none | host proof, every ISA | held |
| 4 | Looked for a slot that answers after `delete` or `delete_matching` | host proof, every ISA | held |
| 5 | Looked for a sweep that touches a bystander slot | host proof and host test, every ISA | held |
| 6 | Looked for a reap authorized by anything but the supervision rendezvous | host proof, every ISA | held |
| 7 | Looked for a liveness-dependent refusal to a stranger | host proof, every ISA | held |
| 8 | Looked for a survey entry outside the invoked rendezvous's domain | host proof, every ISA | held |
| 9 | Looked for a divergence between the view predicate and the reap predicate | host proof, every ISA | held |
| 10 | Checked each encoder's user-VA gate for the low half and alignment | three host proofs, one per ISA | held on all three |
| 11 | Checked which encoder is proved W^X | host proof on `x86_64` only | held where proved; no aarch64 or riscv64 harness states the claim, so proposed, and since built as [no page is both writable and executable, proved on every ISA](0718-no-page-is-both-writable-and-executable-on-every-isa.md) |
| 12 | Read the VT-d entry encoder for reserved bits | host proof, `x86_64` by subject | held |
| 13 | Looked for a descriptor the shadow copies from outside the region | host proofs, every ISA | held |
| 14 | Looked for an indirect descriptor reaching the shadow | host proof, every ISA | held |
| 15 | Looked for an unbounded or out-of-ring walk | host proofs, every ISA | held |
| 16 | Looked for two queues sharing a ring block | host proof, every ISA | held |
| 17 | Asked whether a post-validation write can reach the device: it writes the driver's copy, the device reads the shadow | structural, every ISA | held; `unfalsified` on purpose, the structural disposition `notes/confinement-claims.md` records (two disjoint arrays in the harness), not open work |
| 18 | Looked for a plan whose rights word is not the declared direction's | host proof, every ISA | held |
| 19 | Read `subtree_scope::walk`, `admit` and `Bindings` for a way up or sideways | host proofs; `dir_capability_tests` runs on all three, recorded on aarch64 | held |
| 20 | Read the C seam's grant mapping for an out-of-grant page | kernel test on all three, falsified by hand | held |
| 21 | Read the three tests and what each proves | aarch64 twice, riscv64 once; `x86_64` green with no record | held; evidence gap is milestone 323's |
| 22 | Checked the per-page low-half refusal in `map_segments` | both tests run on all three, recorded on aarch64 | held |
| 23 | Looked for a construction path left after the drop | kernel test on all three, recorded on aarch64 | held |
| 24 | Same read as row 19, at the shell | kernel test on all three, recorded on aarch64 | held |
| 25 | Not attacked: the compositor is a userspace server, the class the 2026-09-21 pass also could not reach and the human outsider behind milestone 198 is for | kernel test on all three, recorded on aarch64 | not attacked; owned by milestone 198 |
| 26 | Not attacked: a real escape hangs the run, and the fix is a timed receive on the syscall surface | kernel test on all three | not attacked; stays unfalsifiable as written, owned by milestone 417 (a usurper that reports instead of hanging) |
| 27 | Read the TSS I/O bitmap, IOPL and the single switch site | `x86_64` by subject | held; every gap fails closed |
| 28 | Read the revoke sweep and the NMI broadcast to other cores | `x86_64` by subject | held |
| 29 | Read `delete_current_cap`'s local bitmap clear | `x86_64` by subject | held |
| 30 | Looked for an in-flight location a sweep misses, and for a completion path that leaves `outgoing_cap` staged | test runs on all three, recorded on aarch64; the new sibling test likewise | sweeps held; the completion path broke, and is fixed above |

### What risk 7's appendix should cite (for the maintainer, under §216)

- The new escape and its fix: a plain `RECEIVE` collecting a `SEND_CAP` sender left `outgoing_cap`
  staged; closed by the `ipc_receive` clear, with the replayable falsification above. The same class as
  #1494 and milestone 634, reached by a third mechanism (the successful-collect path, not an abort and
  not the mailbox slot).
- A clean result on every claim this pass could reach by reading or by a host or kernel test, the
  evidence the premise check scopes. It does not support green; the human half still waits on
  milestone 198.

### Observations that are not escapes

- **`subtree_scope::Bindings::of` fails open for a nonzero badge at or past `B`**: it returns
  `Binding::Unbound` (full caretaker authority) rather than refusing. It is not reachable by a confined
  client. The kernel's `BADGE` method refuses an already-badged source, so a client given a badged
  endpoint cannot mint a badge of its own choosing, and the fail-open then depends only on the
  spawner's badge allocation staying inside `B`. It is tied to the §230 (badged endpoint
  capabilities) window model, so tightening
  it to fail closed for an unknown nonzero badge is a §230 question for an architect rather than a
  lane's fix. Ruled by calef 2026-10-03 (UTC) and built as
  milestone 726 (an unknown badge fails closed in subtree_scope), `design/roadmap/0726-an-unknown-badge-fails-closed-in-subtree-scope.md`.
  A hardening candidate, not an escape.
- **Rows 17 and 26 stay unfalsifiable as written**, for the reasons the notes already give. Row 17
  needs a real device re-read. Row 26 needs a non-blocking or timed receive (the syscall surface, so
  a proposal, already noted against milestone 305 (the six kernel confinement rows get a
  falsification a machine can replay)). This pass confirms both and adds nothing new to
  them.

## The second and third outsider passes (2026-10-05 and 2026-10-06 UTC)

BUILT. Every claim has now been attacked three times by reviewers briefed only with the claims
table and the source, and the third pass booted what it could on all three ISAs. The human half
stays milestone 198's.

### The second pass (#1687): one kernel escape, by read and one boot

Recorded whole in [`notes/confinement-outsider-pass-2.md`](../../notes/confinement-outsider-pass-2.md).
A `PortRange` capability narrowed to `READ` still drove x86 port I/O, because the grant install
checked no rights: booted red on x86_64, fixed by milestone 768 (a read-only port range grants
nothing) and now claim 33. It also read AMD-Vi, which had no claim (milestone 767 (AMD-Vi hardening before the first AMD boot) closed four of its
six findings), and eight near misses, each given a `BUGS` home. Most of its verdicts were read, not
booted, which its own `BUGS` said.

### The third pass: booted, one escape at the application boundary, fixed

Recorded whole in [`notes/confinement-outsider-pass-3.md`](../../notes/confinement-outsider-pass-3.md)
(name provisional). A Fable reviewer, told to count an attack only when it boots, ran the full suite
on each ISA (aarch64 134 passed, riscv64 137, x86_64 216 with 79 skipped) and added three dispatcher
tests in `system_tests::user::confinement_attack_tests`, green on all three. Counts: 1 escape, 4 near
misses, 28 held, 0 untestable. Rows 1, 4, 5 and 10 to 18 are host proofs or host tests, one artifact
for every ISA; rows 31 and 32 are `script/swish-check` censuses graded by read.

- **The escape.** A client holding only `WRITE` on the swap demonstrator's endpoint made the server
  write a byte into the device register page the server holds and the client does not:
  `OPERATION_PUT` wrote at `LOG_VA + log_base + arg` with `arg` unbounded. Booted red on aarch64 and
  riscv64 (x86_64 has no device page). The kernel gate held throughout; this is a confused deputy,
  and it is still a component reaching an object it was not granted. Fixed here in
  `swap_protocol::log_put`, with
  `live_swap_tests::a_confined_client_drives_the_server_to_write_past_its_log` and a replayable
  falsification, red on aarch64 under the patch. `system_initializer` starts none of the
  demonstrator's programs, so the class (a server trusting an offset in a request word) is the
  finding more than this instance.
- **Claim 26's cited test could only fail by hanging**, and drove one of its four gated methods on
  two ISAs. `a_write_only_rendezvous_holder_cannot_receive_reap_or_survey` drives all four through
  the dispatcher on three, with a parked peer so a broken gate fails an assertion.
- **Claim 24 is the finding most likely to be a live escape in a shipped boot**, and two independent
  passes have now read it. A directory-granted job's file-server window is reused round robin while
  its last holder may still run, and the server writes the new job's file bytes into a page the old
  job still maps. Not booted: it needs the production job pool, a disk and seven concurrent jobs.
  Its fix is milestone 685 (a job is finished when its memory is back)'s "job reaped" signal, and
  that block does not yet say it closes a confinement gap; the maintainer should add that and weigh
  685's priority against risk 7.
- Near misses, each with a home. Claim 3: a `GRANT`-less budget retypes a `GRANT`-bearing frame,
  booted as a characterization on all three ISAs, with `BUGS` on `cap::memory_region_cap`. Its retype
  question is ruled below. Claim 17: the regression test cannot fail, with `BUGS` on it in `kernel/src/virtio.rs`. Claim 25: stale
  pixels on a respawn nobody performs, in an existing `BUGS`. Claim 28: the test does not check the fault pc, with `BUGS` on
  it. The unauthenticated `OPERATION_QUIESCE` is a denial of service, with `BUGS` at its arm.

### The retype-GRANT question, ruled (2026-10-07 UTC)

calef ruled option (a) in PR #1798's thread: a retype mints only the budget's rights, so `GRANT` is
delegated, never minted. The fix is milestone 824 (a retype mints no right its budget lacks), which
records the refused options.

### What risk 7's appendix should cite (for the maintainer, under §216)

- Pass 2's port-rights escape and its fix by milestone 768, and the AMD-Vi findings by milestone 767.
- Pass 3: the kernel's confinement held under a booted attack on three ISAs by a third reviewer.
  It found one application-boundary escape (the swap deputy), now fixed with a falsifiable test.
  Claim 24's window reuse is the open gap most likely to be a real escape, owned by milestone 685.
- Proposed verdict, which is calef's to move: **AMBER, unchanged.** Three same-vendor outsider
  passes found three escapes between them, every one fixed, and no kernel escape survived; that is
  the evidence the premise check scopes and it does not support green. An escape that is still open
  (claim 24) argues against moving toward green until 685 lands.

## Index row

An agent on a different model, briefed with only the 30 published confinement claims, attacks each
one under QEMU on three ISAs and reports escapes, near misses and untestable claims. It is risk 7's
first outsider pass, not its last.

# Confinement outsider pass 5 (2026-10-10 UTC)

The fifth outsider pass at risk 7 (the confinement claim is false), milestone 867 (a fifth outsider
pass attacks the confinement claim), whose number is provisional. By Claude (Opus 5.5), so it serves criterion (c)'s
Anthropic half: it can be the first of two consecutive clean passes, and the second must be a
non-Anthropic model or a human.

Informed, the posture milestone 800 (a non-Anthropic model attacks the confinement claim) set: the
whole tree and its git history were in hand. The counting rule, the report format and the refusal
log are milestone 800's, unchanged. An attack counts when it boots. A verdict from reading alone is
graded `read`, the weakest grade. Only a new finding counts; a re-found fixed escape is a
re-discovery, scored apart.

## What this pass attacked, and what it did not

The brief named the newest surfaces first. One was the §255 (each socket is its own capability) socket-capability model. The others were `std` at the prompt, the §252 (a resolver grant is one zone per client badge) resolver grant, and the milestone 801 (packages over the internet) package fetch path if it had merged. It had not when this pass ran: its pull request (#1884) merged later the same day. So the package client was not a shipped path, and it was out of scope for this pass. It is the next pass's ground.

The socket-capability surface is where the effort went. It is the newest shipped code, it is what pass 4's escape and the #1817 fix rewrote, and `std::net` at the prompt rides exactly it. The std net PAL (`patches/std-nife/overlay/std/src/sys/net/connection/nife.rs`) opens every socket as a `CALL` on the front door answered by `REPLY_CAPABILITY`. So a std program holds one capability per socket and no shared id namespace. Attacking the contract attacks std's sockets too.

## The claims, attacked

One row per claim. Grade is `booted <isa>`, `host` or `read`. This pass booted the socket-capability
rows on aarch64 locally; riscv64 and x86_64 run the same kernel suite in CI, dispatched by hand on
this draft. Rows carried by host proofs are one artifact for every ISA by construction.

| # | Attack this pass ran | Grade | Outcome |
|---|---|---|---|
| 1 | Re-read `derive` for a widen; variant of pass 1's row 1 | host, read | held |
| 2 | Re-read every register-to-`Rights` site (`from_bits` then subset) | host, read | held |
| 3 | The retype-GRANT question, ruled (a) 2026-10-07 (milestone 824 (a retype mints no right its budget lacks), NOT-STARTED); read the current characterization | read | held, with the known gap owned by milestone 824 |
| 4 | Re-read the consumed-capability path | host, read | held |
| 5 | Re-read `delete` touches only its slot | host, read | held |
| 6 | Re-read reap authorization | host, read | held |
| 7 | Re-read the stranger-liveness refusal | host, read | held |
| 8 | Re-read the survey domain | host, read | held |
| 9 | Re-read view-equals-reap scope | host, read | held |
| 10 | Re-read the user-VA gate | host | held on all three |
| 11 | W^X, proved on every ISA since milestone 718 (no page is both writable and executable, proved on every ISA) | host | held |
| 12 | Re-read the VT-d reserved-bit encoder | host | held |
| 13-16 | Re-read the DMA validator rows | host | held |
| 17 | Structural disposition, unchanged | host | held (unfalsifiable on purpose) |
| 18 | Re-read the wiring-plan rights | host | held |
| 19 | Re-read the directory-capability subtree bound; TOCTOU variant against `name_resolver` (see below) | read | held |
| 20 | Re-read the C-seam grant | read | held |
| 21 | Re-read the kernel-address refusal | read | held |
| 22 | Re-read the ELF-over-kernel refusal | read | held |
| 23 | Re-read the progenitor-authority drop | read | held |
| 24 | Re-read the file-server window reuse; TOCTOU is milestone 825's booted probe | read | held by reading; the booted race is milestone 825, refusal 1 |
| 25 | Compositor; userspace server behind milestone 198 (a package manager, and the trivial install that makes a second customer possible), respawn scrub refused | read | not attacked (refusal 2) |
| 26 | A client of a rendezvous cannot become its server; re-read the reshaped `chatty`/dispatcher tests | read | held, and now attacked: passes 3 and 4 made its own test fail rather than hang |
| 27-29 | Re-read the x86 port rows | read (x86 by subject) | held |
| 30 | Re-read the in-flight revocation sweep | read | held |
| 31 | Re-read the unvouched-child census | read | held |
| 32 | Re-read the boot-shell display census | read | held |
| 33 | Re-read the read-only port-range refusal (pass 2's escape, fixed) | read | held |
| 34 | New ground: a socket holder's kernel RECEIVE/RECEIVE_CAP on its own socket capability, plus a re-run of the squatter's page-capture and front-door attempts | booted aarch64 (riscv64, x86_64 in CI) | held |

## New ground, and why it holds

A socket holder cannot receive on its socket capability. The stack mints each socket's
capability from `socket_protocol::stack_slots::MINT`, a copy of its own serve endpoint carrying the
socket's badge. That copy is `WRITE | GRANT`, with no `READ`, on purpose. A copy that also carried `READ` would let a socket holder run a kernel plain `RECEIVE` or `RECEIVE_CAP` on it and dequeue the stack's own incoming queue, which holds every other client's request to the stack. That is the capture class milestone 649 closed, reached by IPC rather than by the squatted page pass 4 booted. Pass 4's squatter tried
`OPERATION_RECEIVE` (a contract opcode `CALL`) on the front door; it did not try a kernel plain
`RECEIVE` on a real minted socket capability. This pass does.

The kernel refuses a receive on a `READ`-less endpoint (`kernel/src/system_call.rs`, the `RECEIVE` and
`RECEIVE_CAP` arms both check `cap.rights.allows(Rights::READ)` and return `NotPermitted`), so both
probes return a negative error at once rather than blocking. `socket_squatter` (net_stack's role 8)
now opens a socket of its own and runs both probes on it before closing it, reporting the result in
word 2 of its armed report;
`net_confinement_tests::a_squatter_at_a_shared_stack_endpoint_cannot_capture_the_clients_traffic`
asserts both were refused. Green on aarch64.

**Falsification, confirmed red.** Granting the mint slot `READ`
(`system_tests/falsifications/user.net_confinement_tests.a_squatter_at_a_shared_stack_endpoint_cannot_capture_the_clients_traffic.receive_probe.patch`)
makes the kernel pass the rights check, so the probe blocks on the stack's endpoint queue (no sender
pending at that point) and the squatter never arms. The test then panics at
`next_report(squatter, "squatter")`: `[PANIC] ... the squatter never reported`. Confirmed on
aarch64 on 2026-10-10 (UTC). The hunk is portable kernel wiring, so one patch serves every leg;
riscv64 and x86_64 were not replayed by hand.

## Variants checked against each fixed escape

- Pass 4's socket capture (milestone 649 (every client of a network stack shares its socket numbers), §255). Did the fix close the class or the instance?
  Read as closed at the class, by a capability model rather than a patched id namespace. A socket is named only by a kernel-stamped badge, and the front door mints and does nothing else. `BADGE` refuses an already-badged source, so a holder cannot re-badge its socket into another. `CLOSE` unbinds the badge and unmaps the page. The badge counter never rewinds, so a stale copy never names a later socket. `ATTACH` unmaps the prior page before mapping, so a reused entry leaks nothing.
  The one sibling this pass found untested was the IPC receive on a minted socket capability, now
  booted above and held. The window-scoping rule is tree-wide since §256 (a server that keeps
  windows for many clients scopes each by the caller's badge); the audit of the rest is milestone
  823 (NOT-STARTED), where this pass leaves it.
- Pass 2's read-only port range (claim 33, milestone 768 (a read-only port range grants nothing)). Re-read: the grant install now checks
  `READ` against port output. Held.
- The `outgoing_cap` / RECEIVE-path family (#1494, milestones 634, 633's first pass, §246).
  Re-read the receive and reply paths. The staged-delegation leaks are closed and a plain `RECEIVE`
  takes no capability on either order (§246 (a plain `RECEIVE` never takes a capability)). No new
  sibling found.
- The redoxfs name-window TOCTOU (claims 19, 24). Read, not booted (refusal 1, milestone 825).
  The `name_resolver` is the positive contrast and was checked as a variant. It copies the name out of the client's window into its own buffer before judging it against the badge's zone (`components/src/domain_name_resolver.rs`, `resolve`). So the check-then-use race redoxfs has does not exist there. The resolver's one by-design limit is that it does not follow the CNAME chain (§252
  Fork 2), which is authority riding on the asked name, not an escape.

## Re-discoveries

None. No already-fixed escape was re-found as still open.

## Near misses

None new this pass. The open gaps most likely to be live escapes are already homed: claim 24's
file-server window reuse (milestone 825, the next pass's booted probe) and the retype-GRANT
characterization (milestone 824).

## Counts

New findings: 0 escapes, 0 near misses. Re-discoveries: 0. Outcomes across 34 claims: 33 held, 1
(row 25) not attacked with a written reason. Booted this pass: the socket-capability row on aarch64
(riscv64 and x86_64 in CI). Everything else was read or host-graded.

## Can this pass count toward criterion (c)?

It is a clean pass with no escape found on a shipped path, by Claude. It can be the first of
criterion (c)'s two consecutive clean passes. It does not by itself move the count to met: the
second must be a non-Anthropic model or a human, and the refusal log's shipped-path entries must not
be left unexamined. The two shipped-path refusals below (redoxfs TOCTOU, the resolver CNAME limit)
are each homed, not unexamined. Moving the color is calef's.

## Refusal log

Every step this pass declined or was stopped from taking, one entry each, per the standing rule
(milestone 800, calef 2026-10-06). Each names the claim, says in one sentence what was tried with no
exploit detail, and says whether the refusal was this pass's own or an external safety system's.

1. **Claim 19/24, the redoxfs name-window TOCTOU.** Trying to boot a hostile client that races its
   own name window against the file server's check-then-use. Declined to build it here: a disk
   fixture plus a racing writer is a milestone of its own, and it already has one, milestone 825 (a
   hostile client races the file server's name window). This pass's own refusal; a shipped-path
   target already homed, so it is examined, not left open.
2. **Claim 25, the compositor respawn scrub.** Trying to observe stale pixels by respawning a client
   into a used slot. Declined: no shipped path respawns a compositor client, so no red boot test can
   exist before a reachable respawn path does; the gap's BUGS entry says so. This pass's own refusal;
   not a shipped path today.
3. **MSI confinement.** Trying to aim an MSI at a vector the caller was not granted. Declined:
   nothing in the tree programs an MSI-X table and no boot exercises the question, so there is
   nothing to boot. This pass's own refusal; the question stays with the milestones that own MSI,
   and is not a shipped path today.
4. **The milestone 801 package fetch path.** Trying to attack a confined package client reaching the
   network and the resolver. Declined: milestone 801 is NOT-STARTED and unmerged, so there is no
   shipped path to attack. This pass's own refusal; it is the next pass's new ground once 801 lands.
5. **Kani re-runs over the host-graded rows.** Trying to re-verify the host-cited claims under the
   model checker. Declined: the attack-pass brief forbids `script/verify` and mutation sweeps, so
   those rows are graded `host`/`read` for that reason. An external process rule, not a safety
   system; not a worklist item.

No refusal came from an external safety system. Every refusal on a shipped path (1) is homed in a
milestone; the others name surfaces that are not shipped paths today.

## BUGS

- Coverage this pass was a targeted boot, not the whole suite: the socket-capability row booted on
  aarch64 locally, and the three-ISA evidence for it is the CI dispatch on this draft. Every other
  row was read or host-graded, as the table says.
- Informed, so this pass cannot say whether a fresh mind would find what it found. That was the
  blind passes' question (milestone 633 (an outside agent attacks the confinement claim) answered it three times); this pass's value is variant depth
  on the newest surface, not a fresh-eyes claim.

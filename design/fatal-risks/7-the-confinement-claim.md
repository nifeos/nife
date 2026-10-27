---
experiment_status: RUN
experiment_run: 2026-08-31
---
# 7. The confinement claim is false

*Risk 7 of [the nine](README.md). The status vocabulary, the rule an entry meets and the running order are there.*

The claim: a confined component escapes, and the property the whole system is built to provide does
not hold. What was missing: every test of it was written by the same people who wrote the thing being
tested.

**The experiment:** milestone 202 (every confinement test is a ritual until somebody breaks the
confinement and watches it fail).

AMBER (calef, 2026-10-03, #1495). In-house passes found and
fixed real defects (tests that could not fail, three times; claims false in audit 313 and on
2026-09-21) and found no escape on a component's own authority, and the outsider half is unrun. What
moves it is the adversarial review of milestone 633 (an outside agent attacks the confinement claim). 26 claims
enumerated at that date (the table now has 33 rows, and all 33 carry a replayable falsification, counted 2026-10-06 (UTC) after #1747; it was 31 before),
three of them stated nowhere, and 25 harnesses carried a replayable falsification at that date, up
from 6 ([`notes/confinement-claims.md`](../../notes/confinement-claims.md); PR #614). The finding is
worse than a missing test. A page-table assertion was patched to remove the check it exists for and
still passed, because it answered "U-mode cannot read the kernel" by refusing to look. It had done so
since milestone 41 (dead code: triage the suppressions, and un-blindfold the gate), with every gate
green throughout. A test that cannot come back red is indistinguishable from a test that passes, and
three independent sweeps have each found confinement tests that could not fail.

The adversarial pass is AUDITED, 2026-09-17: a qualified yes with one exception, found and fixed.
Milestone 313 (the security audit that was due since August) found DECISIONS §12 (call/reply IPC: a
one-shot reply capability)'s claim that a consumed capability cannot be used again false on x86_64,
on a path every boot takes.

Dated 2026-10-03 (§216, from #1495): later passes are not in the paragraph above. An in-house adversarial pass on
2026-09-21 (PR #1059) found a revoked capability parked in `outgoing_cap` delivered afterwards, fixed in
three sweeps. The audit of 2026-09-24 (new trust boundaries) fixed 5 and found no confinement claim false
(`design/audit-reports/2026-09-24-new-trust-boundaries.md`). The audit of 2026-09-29 (the syscall surface
as a whole) fixed 0, minted 0, accepted 3 and found no confinement claim false
(`design/audit-reports/2026-09-29-syscall-surface-whole.md`). The audit of 2026-10-03 (eight constants
and thirteen components, with its follow-up) fixed 5, minted 4, accepted 6 and found no kernel
confinement claim false; its three findings that bear here are listed below
(`design/audit-reports/2026-10-03-eight-constants-and-thirteen-components.md`). PR #1494's RECEIVE-consumer audit then
found two confinement defects, both confirmed under QEMU: a plain `SEND` delivers badge 0 whatever
the endpoint capability's badge, so a bound `redoxfs_server` client that `SEND`s is seen as root
(closed by #1494, merged 2026-10-03), and a plain `SEND` received by `RECEIVE_CAP` handed the receiver a
sender-chosen slot on one arrival order, a `net_stack`-shaped escape (fixed by milestone 634 (a plain SEND received by RECEIVE_CAP never hands the receiver a sender-chosen slot)). The
appendix has both. Milestone 633 (An outside agent attacks the confinement claim) found a third route to the same RECEIVE-path escape, by PR #1525: a plain `RECEIVE` collecting a `SEND_CAP` sender left the sender's `outgoing_cap` staged, so the sender's next plain `SEND` delivered the capability to a `RECEIVE_CAP` receiver on another endpoint. Fixed in `sched::ipc_receive` with a test and a replayable falsification. Same class as #1494 and milestone 634. The first outsider pass on 2026-10-03 found nothing else on the claims it reached; rows 25 and 26 were not attacked. Row 11 is proved on aarch64, riscv64 and x86_64 (milestone 718 (No page is both writable and executable, proved on every ISA)). Dated 2026-10-03, afterwards: milestone 719 (Compositor confinement claim 25 is attacked part by part) in PR #1536 attacked row 25 part by part on aarch64; row 26 is still unattacked. This does not support green, and the human-outsider half remains behind milestone 198. Dated 2026-10-03: the same pass recorded that `subtree_scope::Bindings::of` mapped a nonzero badge at or past its table size to the whole endpoint's authority, not reachable by a confined client because `BADGE` refuses an already-badged source; calef ruled it should refuse, and milestone 726 (an unknown badge fails closed in subtree_scope) makes it so, with a Kani harness and a replayable falsification, and the harness fails when the old arm is restored.

Dated 2026-10-03 (§216, milestone 706 (a `CALL` server can tell a Reply from a delegation)): the
2026-10-03 audit's finding 11 is built, on calef's ruling of the same day. A client could `SEND_CAP`
a real capability where a `CALL` server expected a Reply, so the server's `reply` blocked on it or
leaked a slot of 32: a denial of service, not an escape, severity medium. `RECEIVE_CAP` now tags a
`CALL`'s Reply in `x4` (DECISIONS §245 (a `CALL` server tells a Reply from a delegation)), and every
`CALL` server in the tree receives through a runtime helper whose typed Reply is the only thing
`reply` accepts. One test, with a replayable falsification replayed red on aarch64, covers the tag
on both arrival orders. It tests the tag, not the hang. A server that reads `x1` raw, outside the
runtime, is still exposed.

Dated 2026-10-04 (§246 (a plain `RECEIVE` never takes a capability), PROVISIONAL number, PR #1611):
a `SEND_CAP` or `CALL` that found a plain `RECEIVE` already parked installed its capability in the
receiver's table, while the other order did not, so a confined program holding a `GRANT` capability
could fill the table of a server draining its output (found by milestone 752 (a seeded syscall
driver with a shadow model)). calef ruled option A; a plain `RECEIVE` now takes no capability on
either order and a `CALL` reaching one is answered `Gone`. Two kernel tests with replayable
falsifications, replayed red on aarch64.

Dated 2026-10-04: nothing fuzzes what a confined process can reach. The six `cargo-fuzz` targets
of §60 (fuzzing complements the proofs) read firmware, disk and network bytes, not IPC requests
or syscalls. A proposal for both is
[`fuzz-the-surface-a-confined-process-can-reach`](../roadmap/0779-fuzz-the-surface-a-confined-process-can-reach.md).

Dated 2026-10-05 (milestone 762 (a mapping cannot outlive its frame's revoke), PROVISIONAL number,
PR #1644): a revoked frame is unreachable through mappings as well as capabilities. `PageFrame::MAP`,
`AddressSpace::MAP_INTO` and `MemoryRegion::MAP` now read their source under the mapping-registry
hold every unmap pass takes. Before that, a sweep landing between the read and the record left the
mapping live. `map_revocation_window_tests` drives a revoke and a region destroy into each path, and
each path's replayable falsification went red on riscv64, one also on aarch64. The gap that remains
is one level up: a destroyed region's intermediate page tables stay linked into a live space
(reasoned, not driven; `revoke::revoke_region`'s BUGS). A lane now holds it.

Dated 2026-10-06 (§216 (fatal-risk facts are correctable, and verdicts are the architect's), milestone 633 (an outside agent attacks the confinement claim), BUILT): two
more outsider passes ran, each briefed with only the claims table and the source. The second
(#1687, [`notes/confinement-outsider-pass-2.md`](../../notes/confinement-outsider-pass-2.md)) found
that a `PortRange` capability narrowed to `READ` still drove x86 port I/O, booted red on x86_64 and
fixed by milestone 768 (a read-only port range grants nothing) as claim 33, and read AMD-Vi, which
had no claim, into milestone 767 (AMD-Vi hardening before the first AMD boot). The third
([`notes/confinement-outsider-pass-3.md`](../../notes/confinement-outsider-pass-3.md)) counted an
attack only when it booted, ran the suite on all three ISAs, and found the kernel's confinement held.
It booted one escape at the application boundary: a client holding only `WRITE` on the swap
demonstrator's endpoint made the server write into a device page the client was never granted,
because the server trusted an offset in the request word. Fixed in `hot_swap_contract::log_put` with a
replayable falsification, red on aarch64. The open gap most likely to be a real escape in a shipped
boot is claim 24's file-server window reuse, read by two passes and not booted; its fix is milestone
685 (a job is finished when its memory is back). All three passes were one vendor's models, so this
is the evidence the 633 premise check scopes and not the human half.

Dated 2026-10-06 (UTC): calef re-affirmed the verdict ("Agreed Amber.") and agreed what green needs:
(a) no known live escape, so claim 24 closed by milestone 685 (a job is finished when its memory is
back), proven by a host test of the window-reuse rule (`Windows::take` never hands out a window
whose last holder is unreaped) that goes red without the fix, plus a booted test of the reap protocol
on all three ISAs; (b) a replayable
falsification on all 33 rows of `notes/confinement-claims.md`; (c) two consecutive independent
attacks with no escape on a shipped path, at least one by a non-Anthropic model or a human, where a
pass that leaves a refusal on a shipped path unexamined does not count (calef, "Add the refusal
log."); and
(d) the milestone that fuzzes the surface a confined process can reach (numbered 779 in #1734,
not yet merged) run to a set budget with no escape.

Dated 2026-10-06 (UTC), facts only: criterion (a) is met, as amended, by milestone 685 (a job is finished when its memory is back) in #1744, which closed claim 24 with a host test of the window-reuse rule that goes red without the fix and a booted test of the reap protocol. Criterion (b) is met by #1747. Criteria (c) and (d) are not met. The verdict is unchanged.

Dated 2026-10-07 (§216 (fatal-risk facts are correctable, and verdicts are the architect's), milestone 800 (a non-Anthropic model attacks the confinement claim), PR #1798): the fourth outsider pass ran, the first by a non-Anthropic model (GLM 5.3), informed by calef's ruling rather than blind ([`notes/confinement-outsider-pass-4.md`](../../notes/confinement-outsider-pass-4.md)). It booted the socket capture recorded as milestone 649 (every client of a network stack shares its socket numbers) on all three ISAs: a second client of a shared network stack endpoint captured another client's traffic, both directions. calef ruled it an escape on a shipped path, so criterion (c)'s two-consecutive count restarted at zero. Milestone 649 fixed the defect the same day in #1817 (§255 (each socket is its own capability)), and the claims table gained row 34 with two replayable records, so criterion (b) still holds on every row. Criterion (c) stays at zero until a fresh pass with no escape on a shipped path. The verdict is unchanged. [Appendix](the-confinement-claims.md#the-fourth-outsider-pass-2026-10-07-milestone-800).

Dated 2026-10-10 (§216 (fatal-risk facts are correctable, and verdicts are the architect's), milestone 867 (a fifth outsider pass attacks the confinement claim)): the fifth outsider pass ran, by Claude (Opus 5.5), informed, on the shipped surfaces ([`notes/confinement-outsider-pass-5.md`](../../notes/confinement-outsider-pass-5.md)). It found no new escape and no re-discovery. It attacked the §255 (each socket is its own capability) socket-capability model first, the surface pass 4's escape and #1817 rewrote and the one `std::net` at the prompt rides. One new-ground variant was booted: a socket holder cannot run a kernel plain `RECEIVE` or `RECEIVE_CAP` on its own minted socket capability, which would otherwise dequeue the stack's incoming queue, every other client's request. It is refused because the minted capability is `WRITE | GRANT` with no `READ`, and the kernel refuses a receive on a `READ`-less endpoint; `net_confinement_tests::a_squatter_at_a_shared_stack_endpoint_cannot_capture_the_clients_traffic` gained the assertion, green on aarch64, with a replayable falsification (granting the mint `READ`) replayed red on aarch64. The milestone 801 (packages over the internet) package fetch path was out of scope because it is unmerged (not a shipped path). Row 26 is no longer unattacked: passes 3 and 4 made its own test fail rather than hang. The pass carries a refusal log; its one shipped-path refusal (the redoxfs name-window TOCTOU) is homed in milestone 825 (a hostile client races the file server's name window), not left unexamined. By criterion (c) this is a clean pass by an Anthropic model, so it can be the first of the two consecutive clean passes; the count is not yet met, because the second must be a non-Anthropic model or a human. Criteria (c) and (d) are not met. The verdict is unchanged, and moving the color stays calef's.

Dated 2026-10-10 (§216 (fatal-risk facts are correctable, and verdicts are the architect's), milestone 871 (a sixth outsider pass attacks the confinement claim), PR #1901): the sixth outsider pass ran, by GLM 5.3 (non-Anthropic), informed ([`notes/confinement-outsider-pass-6.md`](../../notes/confinement-outsider-pass-6.md)). It booted an escape on the newest shipped surface, the milestone 801 (packages over the internet) package fetch: the client checked one resolution of a listed location's host, then connected by name, which resolved again, and a rebinding name server answered the check public and the connect private, so the client reached a private address Q1 forbids. calef's verdict, 2026-10-10 (UTC): "Yes, it is an escape on a shipped path. It was literally just shipped, but counts." Criterion (c)'s count restarted at zero, so the fifth pass no longer stands as the first of two. Fixed in the same pull request on calef's ruling: the client resolves once, checks those addresses, and dials only them (`package_index::Location::check`), keeping the host name for TLS. The red test is green on all three ISAs and red on all three when connect-by-name is patched back; that falsification is `attested`, because no gate builds the exerciser until milestone 855 (the TLS graph enters the gated build). Criteria (c) and (d) are not met. The verdict is unchanged, and moving the color stays calef's. [Appendix](the-confinement-claims.md#the-sixth-outsider-pass-2026-10-10-milestone-871).

Dated 2026-10-12 (§216 (fatal-risk facts are correctable, and verdicts are the architect's), milestone 903 (a seventh outsider pass attacks the confinement claim)): the seventh outsider pass ran blind, milestone 633's posture, by a fresh Claude agent on Fable briefed with only the claims table and the source ([`notes/confinement-outsider-pass-7.md`](../../notes/confinement-outsider-pass-7.md)). A first reviewer read past the table into earlier findings and was discounted; the second ran the pass. It found no escape. It booted new tests for rows 1 to 3 on all three ISAs and graded the other 31 rows `read`. Its one near miss is the retype gap of milestone 824 (a retype mints no right its budget lacks), ruled 2026-10-07 and unbuilt, now shown to reach every kernel object a budget can retype, not only frames. No refusal on a shipped path. It does not count toward criterion (c): calef ruled on #2010 (2026-10-12 UTC), option (b), that with only 3 of 34 rows booted the count stays 0, and that the next pass's brief sets a minimum of booted rows. That brief is [`briefs/confinement-outsider-pass.md`](../../briefs/confinement-outsider-pass.md), whose floor calef ruled the same day on #2010 ("27 seems right"): every row a boot can reach, 27 of 34, with rows 12 to 18 exempt as host proofs. The second clean pass must still come from a non-Anthropic model or a human. Criteria (c) and (d) are not met. The verdict is unchanged, and moving the color stays calef's. [Appendix](the-confinement-claims.md#the-seventh-outsider-pass-2026-10-12-milestone-903).

Dated 2026-10-14 (§216 (fatal-risk facts are correctable, and verdicts are the architect's), milestone 913 (an eighth outsider pass attacks the confinement claim)): the eighth outsider pass ran, by GLM 5.3 (non-Anthropic), at the commit that brings milestone 824 (a retype mints no right its budget lacks) to main ([`notes/confinement-outsider-pass-8.md`](../../notes/confinement-outsider-pass-8.md)). It found no escape and four near misses, none on a shipped path. It is below the floor of 27 booted rows: 25 by its own table, since rows 10 and 11 were attacked by host tests. Its blind posture was breached: it was started with "Do milestone 913" and read this milestone's block, milestone 914 (a ninth outsider pass attacks the confinement claim)'s block and the launch brief before its prompt. The lane reproduced every booted row. It does not count toward criterion (c): calef ruled on 2026-10-14 (UTC), "pass 8 does not count." The count stays 0. Criteria (c) and (d) are not met. The verdict is unchanged, and moving the color stays calef's. [Appendix](the-confinement-claims.md#the-eighth-outsider-pass-2026-10-14-milestone-913).

Dated 2026-10-15 (§216 (fatal-risk facts are correctable, and verdicts are the architect's), milestone 914 (a ninth outsider pass attacks the confinement claim)): the ninth outsider pass ran blind in Claude's cloud runner at the same commit as the eighth ([`notes/confinement-outsider-pass-9.md`](../../notes/confinement-outsider-pass-9.md)). The session was configured for Claude Fable and served by Claude Opus 4.8. It found no escape and one near miss, already milestone 799 (the shadow descriptor is published in two stores). It booted 20 of the 27 bootable rows the floor requires. The lane reproduced every booted row on all three ISAs. It does not count toward criterion (c): calef ruled on 2026-10-15 (UTC), "Does not count." The count stays 0. Criteria (c) and (d) are not met. The verdict is unchanged, and moving the color stays calef's. [Appendix](the-confinement-claims.md#the-ninth-outsider-pass-2026-10-13-milestone-914).

Dated 2026-10-15 (§216 (fatal-risk facts are correctable, and verdicts are the architect's), milestone 825 (a hostile client races the file server's name window)): milestone 825 is BUILT, closing the one shipped-path refusal the fifth pass homed to it above. `redoxfs_server` now copies each path name out of the client's shared window before it checks it (calef's "Refused names" ruling, 2026-10-15 UTC), so no name-taking arm acts on a name the server's check refuses, even if a second thread rewrites the window after the check. Claims 19 and 24 hold that refused-name invariant on the host dispatch (`redoxfs_server/src/tests/name_window.rs`). The fix is defensive: against RedoxFS a swapped name reaches a refused name inside the grant, never one outside the subtree, since the engine stores no `..` entries and its lookup is literal within one directory. The live race, a second thread rewriting the page between the server's check and its use, is recorded rather than booted, because that is a data race a single-threaded host stand-in cannot stage. The verdict is unchanged, and moving the color stays calef's.

Dated 2026-10-17 (§216 (fatal-risk facts are correctable, and verdicts are the architect's), milestone 719 (compositor confinement claim 25 is attacked part by part)): milestone 719 is BUILT. Each of row 25's four parts has a test that fails when only that part's enforcement is removed, with a replayable falsification, recorded on aarch64 only. Row 11 is proved on aarch64, riscv64 and x86_64. This does not support green, and row 26 is still unattacked.

Dated 2026-10-17 (§216 (fatal-risk facts are correctable, and verdicts are the architect's), milestone 768 (a read-only port range grants nothing)): milestone 768 is BUILT. calef's ruling of 2026-10-05, that a port range grants nothing without `WRITE`, is recorded in §121 (what a device capability is when the device has no page: x86 port I/O). Claim 33 is unchanged. This does not support green.

Dated 2026-10-19 (§216 (fatal-risk facts are correctable, and verdicts are the architect's), milestone 871 (a sixth outsider pass attacks the confinement claim)): milestone 871 is BUILT. The claims table gains row 35, the package fetch path: a listed location never reaches a private or link-local address, and the address dialed is the one the check passed. Its record is replayable and booted red on aarch64, riscv64 and x86_64, so criterion (b) holds on every row. The pass's table sweep was not run; calef ruled on #2432 (2026-10-19 UTC) that it runs as a new GLM 5.3 pass, proposed as the tenth, which can count toward criterion (c). Criterion (c) stays at zero. This does not support green.

The caveat that keeps the gate closed: it was us attacking our own system. A hole we closed ourselves
is the same category of evidence as the audit that found it. The outsider trying to escape is gated
behind milestone 198 (a package manager, and the trivial install that makes a second customer
possible), by calef's no-third-parties position. Nothing here says the confinement holds. What it
supports is that these named claims are tested, and each shown to fail when broken.
[Appendix](the-confinement-claims.md).

Open security findings that bear on it, each a proposal and none yet built:

- [The confinement table lists the unvouched child](../roadmap/0673-the-confinement-table-lists-the-unvouched-child.md):
  a claim tested and falsified by hand three times, with no row in the table. Severity not recorded.
  Dated 2026-10-03: row 31 of the table now states it, tested by the `installed/unvouched` line of
  `script/swish-check` on three ISAs, with a replayable falsification (a patch, replayed by hand on
  aarch64, red with exit 1).
- [Reset unowned PCI functions before the IOMMU enables](../roadmap/0693-reset-unowned-pci-functions-before-iommu-enable.md):
  Bus Master Enable is already set on functions the kernel never owns. Ruled 2026-10-27 (UTC):
  record and do not build. The gap is one of fault visibility, not a confinement escape, because
  default-deny still blocks every address of an unregistered device. The milestone stays open, and
  clearing Bus Master Enable is built only when code needs to read faults reliably.
- [Every client of a network stack shares its socket numbers](../roadmap/0649-every-client-of-a-network-stack-shares-its-socket-numbers.md):
  one holder of the network capability can read and close another's sockets. Milestone 800 (a
  non-Anthropic model attacks the confinement claim)'s fourth pass booted it on a shipped path on
  all three ISAs, and calef ruled on 2026-10-07 (UTC) that it restarts criterion (c)'s count. The
  fix, 2026-10-07: each socket is its own capability (§255 (each socket is its own capability)),
  a contract change with a new `REPLY_CAPABILITY` method on the Reply object. Two default-suite
  tests in `system_tests/src/user/net_confinement_tests.rs`, each with a replayable falsification,
  check it on every ISA. This changes no verdict: criterion (c) needs a fresh clean
  non-Anthropic pass.
- [The sibling RECEIVE_CAP paths get a receiver-first test](../roadmap/0714-the-sibling-receive-cap-paths-get-a-receiver-first-test.md):
  two paths now correct by reading, unmeasured. Severity not recorded. Dated 2026-10-03, afterwards:
  a receiver-first test for each is in PR #1576, with a replayable falsification that turns it red
  on aarch64; riscv64 and x86_64 build it and run it in CI.
- [A graphical terminal session on the no-keyboard arm holds only the raw half of the boot discipline](../roadmap/0709-arm-1-holds-only-the-raw-half-of-the-boot-discipline.md):
  the session's copy of the boot line discipline's endpoint also answers `OPERATION_BYTES`, so a
  compromised session can queue a command line the boot shell runs with its own authority. A
  userspace grant wider than its use, not a kernel escape; read, not demonstrated. Severity
  medium (2026-10-03 audit, finding 2). Dated 2026-10-03, afterwards: PR #1586 gives the session a
  badged copy the discipline answers only `OPERATION_RAWMODE` and `OPERATION_READRAW` on. A badged `OPERATION_BYTES` was
  served before the fix (a system test, red on aarch64) and is refused after it, with a replayable
  falsification; riscv64 and x86_64 run the test in CI. A session can still switch the
  discipline's mode under the shell.
- [The spawn service holds the display grants, and the shell holds none](../roadmap/0715-the-spawn-service-holds-the-display-grants-and-the-shell-holds-none.md):
  the boot shell keeps the seven display and keyboard capabilities with `GRANT` for its whole life,
  and could map the keyboard's DMA page or take an interrupt wake; it does neither. Severity medium
  as a width, low as a reach (2026-10-03 audit, finding 10). Dated 2026-10-03, afterwards: PR
  #1585 keeps the seven in the spawn service, which lends each session's drivers copies, and the
  shell holds none. A `caps` census in `script/swish-check` read slots 22 to 25 held before the fix
  and none after, on aarch64, with a replayable falsification; riscv64 runs it in CI, and x86_64
  has no gpu to hold. The progenitor's capability peak rose from 30 to 31 of 32 on a gpu and
  keyboard boot.

Fact, 2026-10-04: milestone 745 (count the error paths no test reaches), a provisional number, ranked 20 unreached host-crate error paths that release or grant memory or authority. Twelve are in `paging`, and one is `subtree_scope::unbind` refusing a caller that is not the root. All 75 cleanup-after-failure paths it found are in kernel and service code no coverage run reaches ([untested error paths](../../notes/untested-error-paths.md)).

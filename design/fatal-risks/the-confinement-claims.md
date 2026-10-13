---
risk: 7
color: amber
updated: 2026-10-03
---
# Appendix to risk 7: The confinement claim is false

*An appendix to [`7-the-confinement-claim.md`](7-the-confinement-claim.md) (risk 7). That entry is the claim of
record, and it is written so that a reader can decide what to work on next without opening this
file. This one exists to be verified or challenged: it holds the evidence, the dates, the numbers,
the corrections and the refusals behind the verdict, at the length they need rather than the length
the six-pager has. Where a study has its own home in `notes/` this page links it rather than copying
it. Name provisional (`design/fatal-risks/` and this file's stem); naming is an architect's.*

### The claim

A confined component escapes, and the property the whole system is built to provide does not hold.

### Evidence today

DECISIONS §31 (the foreign-language seam) proves a C component faulting on a deliberate
out-of-bounds write, restarted by its supervisor, with two witness pages answering two different
questions. `notes/untrusted-input-audit.md` surveys the attack surface, and there are fuzz targets.

### What is missing

Every one of those is a test written by the same people who wrote the thing being tested.

### The verdict of record

RUN, 2026-08-31, and it found the thing this risk exists to find. notes/confinement-claims.md; PR
#614.

- 26 claims enumerated on 2026-08-31 (the table has 32 rows as of 2026-10-03), each with where it is stated, which test checks it, and whether that test
  has been shown to fail when the claim is broken. Three were stated nowhere, including one the
  system deliberately does *not* make: a confined device's values are not confined, only its reach.
  The IOMMU and the DMA validator constrain placement, never content.
- 25 harnesses now carry a replayable falsification, up from 6. The sweep is 25 swept, 0 survivors,
  and every patch names the assertion it expects to fail.
- DECISIONS §31's headline assertion never runs. Mapping `WITNESS_RO` read/write does turn the C
  seam test red. But `assert_eq!(v[2], CONFINED)`, the line that prints *"read-only witness
  intact"*, is not what catches it: a component that is not confined does not fault, no fault means
  no death report. And the witness check is reachable only by an escape that faults anyway. The
  sentence the seam is quoted for is not the sentence doing the work.
- And the first attempt failed for the wrong reason, which is the hazard this milestone's own block
  warned about, on the day it was written. The break surfaced as a 234-second watchdog timeout
  reading *"a livelock, not a lost wakeup"*, a correct red with nothing in it about confinement.

The adversarial pass: AUDITED, 2026-09-17, and the answer is a qualified yes with one exception
found and fixed. Risk 7's Experiment status is the entry's one status, which is all
`script/fatal-risks` reads; how well the experiment was done belongs here, in prose.

Milestone 313 (the security audit that was due since August: userspace confinement, read
adversarially) read this risk's question adversarially under the userspace-confinement lens, the
first security audit since 2026-08-17. `design/audit-reports/2026-09-17-userspace-confinement.md`
has it; findings fixed 3, minted 3, accepted 1.

One published claim was false as stated, on a path taken every boot. DECISIONS §12 (Call/Reply IPC:
a one-shot reply capability) says *a consumed capability cannot be used again*. On `x86_64` it was
not: `SYS_CAP_DELETE` cleared the capability table and not the cached grant the context switch
installs into the TSS I/O bitmap, so a thread that dropped its `PortRange` kept COM1 for the rest of
its life. `system_initializer` performs exactly that delete on every x86 boot. Fixed in
`sched::delete_current_cap`, with a test and a falsification replayed red.

A second published sentence about the hardware was false and is now true. `crates/paging`'s decoder
reports user pages as not kernel-executable, and milestone 307 (which assertion actually fires when
a confinement claim is broken) wrote that the hardware makes it so; on x86 that holds only with
`CR4.SMEP`, which nothing set. The bit is now set per core where CPUID offers it, and 307's sentence
is struck through with the correction beside it rather than edited away.

The headline claim was not found false anywhere this audit looked. And what it looked at is stated
rather than implied: components that took device or network authority since the last audit, which
reading every capability mint site shrank from 45 counted components to two objects. The six claims
milestone 307 marked quotable-but-unreachable, none of which turned out weaker than its claim. And
the two new machine classes, radon and xenon.

Checked 2026-09-25 (UTC) against pull request #1275, which found that x86_64 brought up the DMAR's
first VT-d unit and called a device confined whenever any unit was on. On xenon's sibling machine
that first unit covers only the integrated graphics. None of this entry's support depends on it. The
audit's xenon reading rested on no component holding a DMA-capable device there, which is still
true. DECISIONS §12's x86_64 defect was a port grant, not DMA. And the x86_64 IOMMU rows in
[`notes/confinement-claims.md`](../../notes/confinement-claims.md) were measured on the host or
under QEMU, whose one unit owns the whole bus. The audit report carries the one sentence the finding
does correct.

### Three things this does not settle

The syscall surface and IPC model were the untaken lens until the 2026-09-29 audit took it
(`design/audit-reports/2026-09-29-syscall-surface-whole.md`: fixed 0, minted 0, accepted 3, no
confinement claim false; corrected 2026-10-03, §216 (fatal-risk facts are correctable, and verdicts are the architect's), from #1495). The
adversarial half this entry has always called for is an outsider trying to escape, rather than us
demonstrating that a planned escape fails. It is now partly built, and still gated behind
milestone 198 (the trivial install that makes a second customer possible). An adversarial pass on 2026-09-21
asked where authority lives outside a cspace and found a claim false: every revocation sweep walked
a thread's capability table and stopped. So a capability parked in `Thread::outgoing_cap` (the
hand-off slot a `SEND_CAP` writes when no receiver waits) survived the sweep and was delivered
afterwards. `MemoryRegion::DESTROY` sweeps capabilities precisely so that no capability still names
a page the allocator is about to hand out, and an in-flight one reopened that. Fixed in the three
sweeps that lacked it, with a test red first on all three architectures;
`notes/confinement-claims.md` carries it and the eight attacks that held. The caveat is the one that
keeps the gate closed: it was us attacking our own system, which is the thing milestone 198 exists
to stop being the only kind of attack this project has seen. And one window was accepted rather than
closed at the time: `PortRange::REVOKE` reached one core, so a revoked holder on another core kept
its bitmap for at most one tick. That window is recorded in §152 (the port-range capability)'s
`BUGS`, corrected the same day, and in [milestone 315](../roadmap/0315-port-revoke-every-core.md),
which the audit raised as finding 4 and calef promoted out of this entry's proposal on 2026-09-17.

### Corrected 2026-09-23: that window is closed

Milestone 315 (a port revoke that reaches every core) is BUILT. The revoke now resets this core and
rides the TLB shootdown's NMI to the rest, and `install_port_grant` moved inside the locked region
so no core can reinstall a grant the sweep just cleared. Proved by 12 of 12 full two-core suites, 36
boots, against 3 of 12 failing before.

What it does to this risk, which is less than it sounds. It removes an accepted hole from the
confinement claim, so the claim is stronger than the audit left it. It does not change the caveat
above, which is the one that matters: the attacking was still us attacking our own system. A window
we closed ourselves, found by our own test, is the same category of evidence as the audit that found
it, and this entry's verdict rests on that category rather than on any single hole.

### Added 2026-10-03 (§216, from #1494 and milestone 634): the RECEIVE_CAP plain-SEND findings

PR #1494's audit of this kernel's `RECEIVE` consumers, run for calef's ruling that a plain `SEND`
carries its capability's badge, found two confinement defects on `main`. Both were confirmed under
QEMU on aarch64 on 2026-10-03 before any fix, which is the category of evidence this entry rests on
and still us attacking our own system.

The first is live on `main` as this is written. `redoxfs_server` reads a client's badge to pick its
window and subtree scope, but the `SEND` syscall carries no badge: a client that `SEND`s on its
per-window badged `FILE` capability instead of `CALL`ing arrives as badge 0, the unbound value,
which `subtree_scope::Bindings::of(0)` reads as `Open`. So a bound client could act outside its own
subtree and with another client's window, and without a reply. A `CALL` on a badged endpoint
delivered badge `0x5a5a`; a plain `SEND` on the same endpoint delivered `0`. It is closed by the
plain-SEND badge, calef's ruling on #1494: once a plain `SEND` carries its capability's badge the
client arrives bound and the scope holds, pinned by a test on all three ISAs
(`a_plain_send_arrives_with_its_capabilitys_badge_on_receive_and_receive_cap`). #1494 merged on
2026-10-03 (07:38Z), so it is closed on `main`; §230 (badged endpoint capabilities) records the
contract in its 2026-10-03 amendment.

The second is the arrival-order half, fixed by milestone 634 (a plain SEND received by RECEIVE_CAP
never hands the receiver a sender-chosen slot). On the receiver-first order a plain `SEND`'s second
word reached `RECEIVE_CAP`'s `x1`, where a `CALL` server reads a reply slot, so a client could hand a
server a slot number of its own choosing; the audit found no consumer that checks the kind of object
in a received slot. It was an ESCAPE: a `net_stack`-shaped server deleted its own capability at the
attacker-chosen slot (`x1` delivered = 7, chosen = 7, the victim did not survive), a near miss on
the sender-first order (`x1 = NO_CAP`). The kernel now writes `NO_CAP` unless a capability was
installed, with a falsification red first on all three ISAs.
[milestone 634](../roadmap/0634-a-plain-send-received-by-receive-cap-never-hands-the-receiver-a-sender-chosen-slot.md)
has the evidence and the two options weighed.

And the audit produced a third instance of this file's recurring shape. Milestone 299 (the serial
console becomes a userspace driver)'s two port tests could not fail in the direction they exist for:
a wrongly permitted `out` was followed by a `SEND` nobody received, so the run hung instead of going
red. That is row 26's shape one object over, found only because a draft of finding 1 hung. After
milestone 305's vacuous `U`-bit test and milestone 307's six unreachable assertions, three
independent sweeps have now each found confinement tests that could not fail. That is the strongest
evidence in this file that the question risk 3 asks is answered differently inside the kernel than
outside it.

### What it does not say

Nothing here says the confinement holds. What it supports is narrower and was the point: these named
claims are tested, and each has been shown to fail when the claim is broken. The adversarial
exercise this entry originally called for is still unbuilt: an outsider trying to escape, rather
than us demonstrating that a planned escape fails. That wants outside eyes and is gated behind
milestone 198 by calef's no-third-parties position.

The six kernel rows got their mechanism on 2026-09-16, and one of them was not testing its own
claim. This entry said until that day that those rows had none. Milestone 305 (the six kernel
confinement rows get a falsification a machine can replay) built it. It sits on milestone 210 (no
kernel test can be run by name)'s `cargo xtask test --test <substring>`, built 2026-08-31, so the
note claiming the mechanism "does not exist" had been stale for sixteen days. Seven of the eight
tests behind rows 21 to 26 now carry a replayable falsification. The sweep is 48 swept, 0 survivors,
2 min 55 s warm.

The finding is the one this risk exists to produce, and it is worse than a missing test.
`the_page_tables_say_u_mode_cannot_read_the_kernels_memory` was patched to remove the `U`-bit check
from `mmu::user_can_read` outright, and the test still passed. `user_can_read` went through
`translate_user`, whose `Mapper` is built with `Half::Low` *always*, so a high-half kernel address
returned `None` before any leaf was read. The assertion answered "U-mode cannot read the kernel" by
refusing to look, and had done so since milestone 41 (dead code: triage the suppressions, and
un-blindfold the gate), with every gate in this tree green throughout. `is_mapped_in_current_space`
exists forty lines away for exactly this case and says so in its own doc comment. Fixed in 305
(`translate_in_either_half`) and measured both ways.

That is the shape of failure this file's rule 1 is about: a test that cannot come back red is
indistinguishable from a test that passes, and nothing but a falsification can tell them apart. Risk
3's mutation census measures the same property over host crates and cannot see kernel tests at all,
so this class was invisible to every instrument the project owned.

Two further things were recorded rather than smoothed over. Row 26 (a client of a rendezvous cannot
become its server) is `unfalsified`, honestly: the complete break produces a 60-second lost-wakeup
watchdog rather than a claim-shaped red, because `RECEIVE_CAP` blocks. So an attacker the kernel fails
to refuse takes the honest server's message instead of reporting an escape. Filling it with an
easier defect would have fired an assertion while leaving the claim untested, which is precisely
what the row above shows costs eight months. And §31's assertion-order hazard has a second
independent instance: row 24's quotable crossing assertion sits below two per-shell bitmap
equalities that catch any crossing one call earlier, so it cannot run. Two instances found the same
way in one sweep is a reason to expect more.

### The first outsider pass (2026-10-03, milestone 633)

Milestone 633 (An outside agent attacks the confinement claim) ran an adversarial review, by PR #1525,
with a different model, attacking the thirty claims in `notes/confinement-claims.md`. One escape
was found and fixed: a plain `RECEIVE` collecting a `SEND_CAP` sender left the sender's `outgoing_cap`
staged, and the sender's next plain `SEND` delivered it to a `RECEIVE_CAP` receiver on another endpoint.
A capability granted to one endpoint reached a receiver on a different one, which is a process
reaching an object it was not granted. Fixed in `sched::ipc_receive` with a test and a replayable
falsification. The same class as #1494 and milestone 634, reached by a third mechanism (the
successful-collect path, not an abort and not the mailbox slot). Every other claim it reached held, by reading and by host or kernel proof where cheap. Two were not attacked in that pass: row 25 (enforced by the compositor, not the kernel; milestone 719 (Compositor confinement claim 25 is attacked part by part) in PR #1536 attacked it part by part on aarch64 afterwards, 2026-10-03) and row 26 (an escape would hang the test rather than fail it, so it waits on milestone 417 (a usurper that reports instead of hanging)). Row 11 was proved on `x86_64` only in that pass; milestone 718 (No page is both writable and executable, proved on every ISA) in PR #1534 has since proved it on aarch64, riscv64 and x86_64, 2026-10-03. This does not support green;
the human-outsider half remains behind milestone 198.

### The fourth outsider pass (2026-10-07, milestone 800)

Added under §216 (fatal-risk facts are correctable, and verdicts are the architect's), from PR
#1798. Milestone 800 (a non-Anthropic model attacks the confinement claim) was the first pass by a
model from another vendor, GLM 5.3, run by calef's opencode. It was informed by calef's ruling: the
reviewer read the whole tree and its history, so a re-discovery scores nothing as a new finding.
The record is [`notes/confinement-outsider-pass-4.md`](../../notes/confinement-outsider-pass-4.md).

It found no new escape. It booted one re-discovery on all three ISAs: milestone 649 (every client of
a network stack shares its socket numbers), recorded by reading on 2026-09-24 with no test. A
second client of a shared `net_stack` endpoint attached its own page at the victim's socket number
first. The victim's attach then failed silently, its request went out as the squatter's bytes, and
its reply landed in the squatter's page. The kernel's gates were never consulted; the confusion was
the server's.

The severity, which this appendix had not stated: since milestone 590 (the booted system starts its
network stack), the stack runs on every booted system, and the progenitor gives every program that declares the network an unbadged copy
of the one stack endpoint. On the shipped boot only vouched `NetworkEchoClient` instances declare
it, so no hostile holder existed, but the job pool runs six jobs at once and two holders were one
prompt away. calef ruled it an escape on a shipped path (2026-10-07 UTC), and criterion (c)'s count
restarted at zero.

Fixed the same day by milestone 649 in #1817: each socket is its own capability (§255 (each socket
is its own capability)), and the pass's test, rewritten, runs in the default suite on all three ISAs
with a replayable falsification. Claim 34 of `notes/confinement-claims.md` states the property. The
pass also made claim 26's test fail rather than hang, red under its recorded patch. Its refusal log
has four entries; the redoxfs name-window race is the next probe, milestone 825 (a hostile client
races the file server's name window). The fix does not count toward (c): that needs a fresh pass.

### The fifth outsider pass (2026-10-10, milestone 867)

Added under §216, from milestone 867 (a fifth outsider pass attacks the confinement claim),
whose number is PROVISIONAL. The fifth pass was by Claude (Opus 5.5), informed, on the shipped surfaces.
The record is [`notes/confinement-outsider-pass-5.md`](../../notes/confinement-outsider-pass-5.md).

It found no new escape and no re-discovery. It went at the §255 (each socket is its own capability)
socket-capability model first, the surface pass 4's escape and #1817 rewrote, and the one `std::net`
at the prompt rides. The variant analysis confirmed the fix closed the capture class and not only
the instance: a socket is named only by a kernel-stamped badge, the front door mints and does
nothing else, `BADGE` refuses an already-badged source, `CLOSE` unbinds the badge and unmaps the
page, the badge counter never rewinds, and `ATTACH` unmaps the prior page before mapping.

The one sibling left untested was a kernel plain `RECEIVE` or `RECEIVE_CAP` on a real minted socket
capability, as opposed to the contract opcode pass 4 tried on the front door. This pass booted it.
The stack mints each socket's capability from a `WRITE | GRANT` copy of its own serve endpoint, with
no `READ`. A copy that carried `READ` would let a socket holder dequeue the stack's own incoming
queue, every other client's request, through a kernel receive. The kernel refuses a receive on a
`READ`-less endpoint (`kernel/src/syscall.rs`), so the omission of `READ` confines it. The squatter
now runs both probes on its own socket and
`net_confinement_tests::a_squatter_at_a_shared_stack_endpoint_cannot_capture_the_clients_traffic`
asserts both refused, green on aarch64, with a replayable falsification (grant the mint `READ`)
replayed red on aarch64.

The milestone 801 (packages over the internet) package fetch path was out of scope, unmerged, so not
a shipped path. Row 26 is no longer unattacked; passes 3 and 4 made its own test fail rather than
hang. The refusal log's one shipped-path entry is the redoxfs name-window TOCTOU, homed in milestone
825. By criterion (c) this is a clean pass by an Anthropic model, so it can be the first of the two
consecutive clean passes; the second must be a non-Anthropic model or a human, so (c) is not yet
met. The verdict is unchanged, and moving the color stays calef's.

### The sixth outsider pass (2026-10-10, milestone 871)

Added under §216, from milestone 871 (a sixth outsider pass attacks the confinement claim), whose
number is PROVISIONAL, and PR #1901. The sixth pass was by GLM 5.3, the non-Anthropic model of pass
4, informed. The record is
[`notes/confinement-outsider-pass-6.md`](../../notes/confinement-outsider-pass-6.md).

It went at the milestone 801 (packages over the internet) package fetch first, shipped the same day
in #1884 and #1890, and booted an escape there. calef's ruling Q1 on #1884 says a location an index
lists may never reach a private or link-local address. The client checked every address a listed
host resolved to, then handed the name to `TcpStream::connect`, which resolved it again. The
resolver keeps no cache, so a rebinding name server answered the check with a public address and
the connect with the private peer. The proof is the refusal the client printed: a certificate
name error, which only a TLS server that answered can produce, and slirp's only TLS server is the
private one. The digest admission held, so no untrusted bytes were taken; the escape is the reach
Q1 forbids.

calef's verdict (2026-10-10, 21:04 UTC, on #1901): "Yes, it is an escape on a shipped path. It was
literally just shipped, but counts." Criterion (c)'s count restarted at zero. On the same ruling the
fix landed with the pass: `package_index::Location::check` takes one resolution and returns a
`CheckedLocation` holding exactly the addresses it passed, the client dials only those, and the
host name is kept for TLS alone (curl's `CURLOPT_RESOLVE` and Go's `net.Dialer.Control` are the
prior art). The check now refuses an IPv6 answer it cannot judge, which it used to pass. The test,
committed red before the fix, is green on aarch64, riscv64 and x86_64, and red on all three with
connect-by-name patched back. That falsification is `attested`: the exerciser is outside every gate
until milestone 855 (the TLS graph enters the gated build), so the claims table gains no row for it
yet.

The 34-row sweep did not run; with the count at zero it could not have made this pass clean. The
verdict is unchanged, and moving the color stays calef's.

### The seventh outsider pass (2026-10-12, milestone 903)

Added under §216, from milestone 903 (a seventh outsider pass attacks the confinement claim), whose
number is PROVISIONAL. The seventh pass was blind, milestone 633's posture: a fresh Claude agent on
Fable, briefed with only the 34-row claims table and the source, and none of the audit reports,
earlier pass notes or history. The record is
[`notes/confinement-outsider-pass-7.md`](../../notes/confinement-outsider-pass-7.md).

A first reviewer read past the table into earlier findings and was discounted; a second, given the
table alone, ran the pass. It found no escape. It booted new hostile tests on aarch64, riscv64 and
x86_64 for rows 1 to 3. `SEND_CAP` refused a widening rights register and a `GRANT`-less source.
`SPLIT` off a `WRITE`-only budget stayed narrow. `RETYPE` and `RETYPE_OBJ` off the same budget
minted `Rights::ALL` for a frame and for kernel objects. That is the retype gap calef ruled on
2026-10-07 and milestone 824 (a retype mints no right its budget lacks) fixes, now shown to reach every kernel object; a near miss, since the
objects are the holder's own. The other 31 rows are graded `read`, about half of them resting on
the row's own cited test. The refusal log holds two external safety-system stops during
orientation and the first reviewer's independence slip, none on a shipped path.

It does not count toward criterion (c). calef ruled on #2010 (2026-10-12 UTC), option (b): with
only 3 of 34 rows booted the count stays 0. The next pass's brief,
[`briefs/confinement-outsider-pass.md`](../../briefs/confinement-outsider-pass.md), sets a floor of
booted rows: every row a boot can reach, 27 of 34, ruled by calef on #2010 ("27 seems right"). The verdict is unchanged, and moving the
color stays calef's.

### Added 2026-10-03: row 27's hand-off was tested on one of the two `x86_64` boots

From 2026-09-23 to 2026-10-03 (UTC), `port_holder_transmits_then_a_non_holder_faults` (row 27 of
`notes/confinement-claims.md`) did not test the port hand-off on the direct `x86_64` boot. That boot
runs the suite on two cores and the real-firmware boot on one. The TSS port bitmap is per core, and
the test did not check that its holder and non-holder ran on the same core. Under the record's
defect, the holder's grant was left on cpu 0 and the non-holder was placed on cpu 1, faulted there,
and the test passed. Only the real-firmware boot went red. Milestone 323 (the falsification record is
incomplete in five ways)'s replay found the green,
and lane `x86-port-falsification-split` instrumented the cause. The test now reads each child's core
from the current-CPU page and runs the non-holder's `out` only on the holder's core. The record is
red on both boots, at `x86_port_tests.rs:270`.

### Added 2026-10-03: rows 28 and 29 replayed on both `x86_64` boots

The records for row 28 (`a_revoked_holder_faults_on_its_next_port_write`) and row 29
(`a_holder_that_deletes_its_port_capability_faults_on_its_next_port_write`) were replayed on 2026-10-03
(UTC) on the direct boot (two cores) and the OVMF boot (one core), and each failed on both at
`system_tests/src/user/x86_port_tests.rs` (`left: 2`, `right: 1`); each passed on both boots with the
patch removed.

### Added 2026-10-03: row 31, the unvouched child

Row 31 of `notes/confinement-claims.md` states the claim milestone 198 (a package manager) rung 3a's
gate D2 lane had falsified by hand (once each for the process domain, entropy and the network, commit
`b555bfb3d`, 2026-09-26) and left without a row. The standing test is the `installed/unvouched` line of
`script/swish-check`, which runs on aarch64, riscv64 and x86_64. Its record is
`xtask/falsifications/swish_check.boot.swish_check_boot.patch`, which makes the progenitor endow the process
domain to an unvouched child. `script/falsifications` sweeps Kani harnesses and kernel `#[test_case]`s
and this line is neither, so it reports the patch as a known gap and the replay is by hand. Replayed
on aarch64 on 2026-10-03 (UTC): without the patch `script/swish-check --arch aarch64` exits 0 (135
lines); with it, exit 1, the child answering `domain: REACHED` and `slots held: 0 1 2 7` where the
line wanted `domain: refused (no capability at slot 7)` and `slots held: 0 1 2`. The record is
aarch64 only: the riscv64 and x86_64 legs were not replayed, and entropy and the network have no
patch.

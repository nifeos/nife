# Confinement, a fourth outsider pass (milestone 800, provisional name)

Milestone 800 (a non-Anthropic model attacks the confinement claim) is the experiment this note
records. It is the first attack pass run by a non-Anthropic model: the
reviewer is GLM 5.3, run by calef's opencode, per the milestone's own brief.

This pass was **informed, by ruling** (calef, 2026-10-06 UTC: "GLM should run informed. That's a
more realistic attack for an open source OS"): the reviewer read the whole tree and its git
history. A re-discovery of a fixed escape is reported separately and scores nothing; only a new
finding counts. This is the mirror of passes two and three, which were blind by request, and the
comparison is the point of both existing.

**Result: one booted re-discovery, on all three ISAs, at a service boundary the kernel's gates
never see.** The escape is the net stack's socket-id namespace. A second client of a shared
`Stack` endpoint captures the first client's traffic, both directions, through nothing but a
capability any client already holds. The kernel gave both programs exactly what they were
granted; the confusion is the server's. The finding itself was already recorded (milestone
649); what this pass adds is the boot. Coverage: two rows booted this pass (claim 26 and the
escape), 31 read. Beside it, claim 26's test can now fail rather than hang, the work the
claims note routed here.

## What I added and booted

`system_tests/src/user/net_confinement_tests.rs` holds the escape's test, and
`components/src/socket_squatter.rs` (a role of the `net_stack` binary) the hostile client. The
test was opt-in and red, which was the finding. #1817 (merged 2026-10-07 UTC) rewrote both for the
capability model; the test is now green in the default suite, with a replayable falsification.

`fixtures/src/swap_boundary_witness.rs` grew `ROLE_PLANT`, and `swapper`'s direct channel retires
the last receiver before the attack and un-parks the plant boundedly. Claim 26's cited test is green
on aarch64 (the whole module, 8 tests) and riscv64; x86_64 skips as always (§121 (what a device
capability is when the device has no page)'s UART-page gap). With the kernel's `RECEIVE_CAP`
check deleted it goes red at its own assertion, under its recorded patch.

## The escape: a shared Stack endpoint's socket ids have no owner

The socket contract keys a socket's shared frame by the socket id, a raw word the client picks
(`socket_protocol::req`), and `ATTACH` is a `SEND_CAP`, which has no reply. The serve loop maps
whatever frame a delegation carried at `socket_va(sid)` into `frame_window[sid]`, with no notion
of which client attached it.

Who can hold the endpoint: any two concurrent holders. On the shipped boot the only holders are
vouched `NetworkEchoClient` instances, `grant_plan`'s one network-declaring manifest, endowed
unbadged by the progenitor's `ChildEndowment::network`; no hostile holder exists today, and any
future declaring program inherits the exposure. The job pool holds six live jobs at once, so two
holders are one prompt away. That is the severity sentence risk 7 lacked for 649.

The attack, booted as `ROLE_SQUAT` (role 8 of the `net_stack` binary, endowed exactly as any stack
client):

1. The squatter delegates its own frame at socket id 0, the honest TFTP client's id, first.
2. It arms a valid read request for the runners' fixture in its own frame, so the victim's
   exchange still completes: an informed attacker wants the traffic to flow, not to wedge it.
3. The victim's `ATTACH` at id 0 fails silently: the map is taken, `ATTACH` cannot answer, and
   `map_page_frame`'s failure reaches nobody.
4. The victim's `OPEN_UDP` binds the socket to `frame_window[0]`, the squatter's window.
5. The victim's `SENDTO` transmits the squatter's bytes. The reply arrives; `sock_receive` writes
   it into the squatter's frame, source endpoint and length included.

Observed on all three ISAs, identical: the squatter read a TFTP DATA packet, opcode 3, block 1,
length 14, body beginning "nife", out of its own frame. The victim's verdict was `0xE043` every
time, because its own frame never saw the reply it was answered for. Both directions are reached,
injection (the squatter's bytes sent as the victim's) and capture (the reply delivered to the
squatter), and the victim's exchange fails visibly rather than silently, so the capture announces
itself to the victim.

Boot evidence, one line per ISA, at commit `c36ab530e`, command
`script/test --arch <isa> --test a_squatter_at_a_shared_stack_endpoint`:
- aarch64, riscv64, x86_64: the same assertion fired, `CONFINEMENT ESCAPE: the squatter's frame
  captured the client's traffic (len/opcode/block ...), first body bytes ... = "nife"`, with the
  client's verdict `0xE043`.

The end-to-end claim it breaks is risk 7's own sentence, *a confined component cannot reach an
object it was not granted*. The object is the victim's socket traffic. The kernel's gates were
never consulted: the reach happens in the server's bookkeeping, with no single wrong write
anywhere.

The finding was already on the books: milestone 649 (every client of a network stack shares
its socket numbers), raised 2026-09-24, NOT-STARTED, records the mechanism by reading with no
test. This pass adds the booted bidirectional capture, pinned red on all three ISAs where 649 had
none. For the counts below it is a re-discovery; for risk 7 it counts as an escape, by the
ruling under *The escape's home*.
The sibling that already does it right is one component over: `name_resolver` keys its windows
by the badge's grant index and refuses a second attach at a granted window. Its refusals are
silent, safe there because the scoping makes every attach failure the caller's own. The first
fix-shape ruling (03:50Z) was that shape; the ruling in force (calef, 2026-10-07 UTC,
15:04Z, PR #1798): each socket is its own capability, built as milestone 649 in #1817 (merged
2026-10-07 UTC). §255 (each socket is its own capability) carries both rulings.

## Re-discoveries

- **Milestone 649's finding, re-derived and booted.** The pass arrived at the capture from the
  contract itself, then found the record. 649 (every client of a network stack shares its socket
  numbers), raised 2026-09-24 from milestone 590 (the booted system starts its network stack)'s
  lane, states the mechanism and the frame-redirect variant by reading. Since then: 649 still
  NOT-STARTED, the contract and serve loop unchanged, and the four other homes still carry it
  (590's block and BUGS, `grant_plan`'s `caps` note, `network_echo_client`'s header, risk 7's
  open-gap list). The additions are the boot and the severity sentence.

## Near misses

Nothing else new reached one bug from an escape. The carried findings, matching the table's
near-miss rows:

- **Row 3, a `GRANT`-less budget still mints a `GRANT`-bearing frame**, 633's carry: read, not
  booted this pass. Ruled (a) since (calef, 2026-10-07, PR #1798), the intersection; the fix is
  milestone 824 (a retype mints no right its budget lacks), and the characterization flips to a
  held assertion.
- **Rows 19 and 24, the name TOCTOU, is still open and still recorded** where a reader meets it
  (`redoxfs_server/src/dispatch.rs`'s head: "A name is checked in the client's own window and then
  used from it"). Settled by read; booting it wants a disk fixture and a racing client.
  `name_resolver` copies the name out before judging it, so the resolver half is closed.
- **Claim 25's respawn scrub gap is latent and unchanged**: no shipped path respawns a compositor
  client. `compositor_service::Wiring::spawn_client`'s BUGS carries it.

## Variants checked against the fixed escapes

Every escape found and fixed before this pass, checked for siblings rather than re-run:

- **The in-flight capability (row 30).** The abort path now clears `outgoing_cap`
  (`set_ipc_aborted`, the 2026-10-03 audit follow-up), closing the sharpest sibling pass two
  named: a parked delegation delivered on a different rendezvous after an aborted send. The
  teardown limbs still do not clear it; a dead thread cannot re-deliver, and the sweeps walk
  live tables. Held.
- The wired mapping record. `map_physical` records through `PageMapSource`; `map_new`'s
  exception still holds (its frames are the space's own and die with it); no `map_physical` call
  site maps a `MemoryRegion` page. Held, unchanged from the claims note's own record.
- **The port-rights fix (claim 33).** The grant install now requires `WRITE`
  (`thread_control_block_insert_from`). The mirror question, whether a WRITE-only capability
  should permit `in`, is answered by ruling in the code: the TSS bitmap cannot split directions,
  so WRITE is the right that opens the ports and READ alone grants nothing. The two-copies
  fail-safe limb remains recorded in `delete_current_cap`'s BUGS. Held.
- **The swap deputy's bound.** `log_put` refuses any offset outside the one log page, the refusal
  is asserted by a default test, and the falsification is recorded. The `OPERATION_QUIESCE` from
  any client remains a recorded BUGS line, a denial of service rather than a reach. Held.
- **The window reuse (row 24's badge half).** Closed by milestone 685 (a job is finished when its
  memory is back)'s work: `take` never hands
  out a window whose last holder is unreaped, with a replayable host record in `grant_plan`. Held.
- Recycled pages. `retype_run`, `retype_page` and `retype_object_page` zero the whole run before
  it is handed out, and `MemoryRegion::MAP` maps only retyped pages, so a page that returns to a
  region cannot carry its previous holder's bytes into a second component. This was this pass's own
  new-ground question, asked cold, and the answer is in the code.
- MSI confinement and SMEP sit where milestones 317, 313 and 424 left them; nothing new touches
  an MSI-X table, and no boot this pass changed the x86 picture. AMD-Vi items 4 and 6 stand as
  pass two left them (no revocation in production, fault attribution from devfn alone), recorded
  in `amd_vi.rs`; neither has a claim row. Untestable here.

## One row per claim

Outcome is one of escape, near miss, held, untestable. Grade is the strongest evidence gathered
this pass: `booted <isa>` means observed under QEMU on that ISA in this pass, `host` means a host
suite ran green this pass, `read` means source review only. This pass ran targeted boots, not the
whole suite, so most rows are `read`; the falsification records each row cites were not re-swept.

| # | Claim | Outcome | Grade | Test / evidence |
|---|---|---|---|---|
| 1 | A derive holds no more than its source | held | read | `derive_never_widens_rights` cited; masking and subset checks read unchanged |
| 2 | No right forged from a syscall register | held | read | `from_bits` masks `& ALL`; the cast drops high bits |
| 3 | A non-delegating budget cannot mint a delegating child | near miss | read | `a_grant_less_budget_mints_a_grant_bearing_frame` still characterizes the retype gap; ruled (a) 2026-10-07, the fix is milestone 824 |
| 4 | A consumed capability cannot be reused | held | read | `delete` empties the slot; unchanged |
| 5 | Dropping one capability spares the others | held, x86 exception | read | the port grant's two-copies limb stays recorded, fails safe |
| 6 | No reaping a corpse you do not supervise | held | read | `reap_supervised` gates before `reclaim_region`; unchanged |
| 7 | A stranger's refusal discloses nothing | held | read | `reap_decision` ignores liveness for strangers |
| 8 | A view shows exactly own children | held | read | `survey_includes` is `fault_ep == ep` |
| 9 | View and reap are one domain | held | read | one decision function |
| 10 | User VAs low half and aligned, every ISA | held | read | `is_user_page_va` unchanged |
| 11 | No page writable and executable | held | read | encoder constructors unchanged |
| 12 | No reserved bit in an IOMMU entry | held | read | VT-d literal guard; AMD-Vi's closed by 767 |
| 13 | A device cannot touch outside its region | held | read | `is_in_region` checked adds; unchanged |
| 14 | No device to unvalidated descriptors | held | read | indirect refused first |
| 15 | No walking outside the rings or forever | held | read | batch and chain bounded by `qsize` |
| 16 | One queue's validation cannot touch another's | held | read | disjoint shadow blocks |
| 17 | A post-validation change cannot reach the device | held | read | device reads the shadow; the publish race stays recorded, milestone 799 (the shadow descriptor is published in two stores)'s lane |
| 18 | No unasked right from a wiring plan | held | read | literal rights in `component_plan` |
| 19 | A directory capability reaches its subtree only | held, near miss | read | `check_component` and the subtree walk; the window TOCTOU is the open near miss above |
| 20 | A C component faults and changes nothing outside its grant | held | read | the seam tests and their records unchanged |
| 21 | No user read of a kernel address, every ISA | held | read | `AT S1E0R` / U-bit walk; evidence parity as the claims note records it |
| 22 | No ELF over the kernel, no writable executable | held | read | per-page refusals through `Half::Low` |
| 23 | No rebuild after dropping construction authority | held | read | the authority test and its record unchanged |
| 24 | Two shells cannot name each other's files | held, near miss | read | window reuse closed (685); the name TOCTOU above is the open half |
| 25 | No reach to a neighbor's pixels or the screen | held, near miss | read | damage intersect; the respawn scrub gap latent |
| 26 | A client of a rendezvous cannot become its server | held | booted aarch64, riscv64; red under its own record | this pass's reshape: the test fails rather than hangs, and carries its patch |
| 27 | No port I/O without a capability (x86_64) | held | read | bitmap and hand-off as milestone 313 (the security audit that was due since August)'s port rows left them |
| 28 | A revoked port holder faults next access (x86_64) | held | read | sweep clears grant and `outgoing_cap` |
| 29 | Self-deletion faults next access (x86_64) | held | read | `delete_current_cap` clears the cached grant |
| 30 | Revocation reaches in-flight capabilities | held | read | the abort follow-up closes the parked-delegation sibling; sweeps unchanged |
| 31 | An unvouched child holds nothing undelegated | held | read | `swish-check` census, swept weekly |
| 32 | The boot shell holds no display device | held | read | `swish-check` census, swept weekly |
| 33 | No WRITE, no x86_64 port I/O | held | read | the grant install requires WRITE; the direction mirror is ruled in the code |

Counts: 0 new findings; 1 re-discovery, booted and pinned red (649's finding, ruled an escape
on a shipped path for risk 7); 4 near miss (3, 19/24, 25); 29 held; 0 untestable-elsewhere; the
rows sum to 33. Claim 26's row is this pass's own booted evidence.

## The escape's home

A BUGS entry in `net_stack.rs` and the red test held it until #1817 (merged 2026-10-07 UTC)
fixed it. §255 records the fix, and claim 34 of `notes/confinement-claims.md` is its row.

**How this pass counts toward risk 7's criterion (c) is ruled: option (b), an escape on a shipped
path (calef, 2026-10-07, PR #1798's thread).** The two-consecutive count restarts at zero until
649's defect is fixed and a fresh pass comes back clean. The stack starts on every booted system,
so any granted program is a client today, and a known, unfixed escape counted as anything else
would flatter the verdict.

The other rulings from this PR's thread are recorded with their dates in this milestone's
roadmap block, beside the maintainer's merge list.

## Refusal log

Every step this pass declined or was stopped from taking, one entry each, per the rule.
Their order is ruled (i) (calef, 2026-10-07, PR #1798): the redoxfs boot is the next probe,
milestone 825 (a hostile client races the file server's name window); the rest stay in their
homes:

1. Claim 19/24's name-window TOCTOU: I was trying to boot a hostile client racing its own window
   against `redoxfs_server`. I declined to build it: a disk fixture plus a racing writer is a
   milestone of its own, and the finding is recorded at the dispatch head. My own refusal; a
   target for a later pass.
2. The compositor respawn scrub (claim 25's latent half): I was trying to observe stale pixels by
   respawning a client into a used slot. I declined to build the respawn handshake re-run, because
   no shipped path respawns a compositor client and the gap's BUGS entry says so; my own refusal.
3. MSI confinement: I was trying to forge an MSI aimed at an ungranted vector. Nothing in the
   tree programs an MSI-X table and no boot exercises the question; my own refusal, unchanged
   target.
4. Kani re-runs: I was trying to re-verify the host-cited rows, and declined: the brief forbids
   `script/verify` in an attack pass, so those rows are graded `read` for that reason.

## BUGS

- Closed by #1817 (2026-10-07 UTC): the opt-in test could not be swept; it is now in the default
  suite with a recorded falsification.
- The pass was informed, so it cannot say whether a fresh mind would have found the net-stack
  escape blind. The finding has been on the books since 2026-09-24 and no blind pass booted it;
  that is a fact about coverage, not a claim about difficulty.
- The chatty reshape's residual: the plant parks without a handshake, so its park is not
  provably complete before the attacker's try. Every interleaving converges, and the one that does
  not (an honest kernel whose plant is slow past the operator's bound) bails on its own stage
  rather than reaping a corpse that never comes; the argument is in the operator's comment. Green
  on aarch64 and riscv64 with the guard, red under the recorded patch before it.

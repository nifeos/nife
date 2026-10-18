# Security audit, 2026-09-29: the syscall surface as a whole, read as a confined process trying to widen

**Kind:** security. **Lens:** the `invoke` surface entire, every object type and method, read from
inside a confined process that holds only what its spawner endowed. The two object types, the
rendezvous method and the wake tag this window added were read line by line, beside the older
seams. **Findings:** fixed 0, minted 0, accepted 3.

Nothing in the window's new surface lets a confined process take authority it was not granted. The
twelve new ABI constants were read as an attacker's shopping list, and each purchase was refused at
the same place the old ones are: a right on a capability the caller must already hold. The two live
risks this read confirmed are both recorded ones, and both are stated where a reader meets them.

## Why this lens

The 2026-09-24 audit named two lenses it had not taken: the syscall surface as a whole, and the
supply chain, the cryptography graph's code. Measured from that audit's landing commit
(`1007edb1`, 2026-09-24) to this one: 748 commits, 1,704 files, +103,176 and -25,422 lines
outside `notes/project-metrics`. `crates/abi/src/lib.rs` took +294 against 15 removed and
`kernel/src/syscall.rs` +429 against 20: two new object types (`NOTIFICATION`, `TIMER`), a new
`rendezvous` method (`BADGE`), and a kernel-written wake tag (`BOUND`), the largest move this
surface has made since the audit mechanism began. The cryptography graph did not move at all: none
of the nine new external packages is a crypto crate, and `deny.toml` is untouched. So the surface
is where the change went, and the lens follows it.

Every count trigger fired (`script/audits`). Milestones 251 to 276 (fires at 15); components 169
to 184 (fires at 8); ABI constants 56 to 68 (fires at 1); external packages 172 to 181 (fires at
1).

### The two uncountable triggers, answered

Has a new component taken device or network authority? Yes, and it is milestone 590 (the booted
system starts its network stack): the real progenitor now builds the network stack at boot.
`network_echo_client`, a fixture spawned by the real spawn service, declares network in its
manifest and reaches a TCP echo peer through it. `terminal_supervisor` holds the line editor's
objects and a two-instance budget, an unprivileged operator in `swapper`'s shape. The negative
control is `unreachable_network_witness`: an empty manifest, real requests, recorded refusals.

Has this booted on a new machine class? No. radon, a class since the 2026-09-17 audit, ran an
eight-hour soak (`bench/radon-2026-09-25/soak-8h.log`). No new board or cloud appears in the
window.

## What was read, and what came back clean

The dispatcher's every arm (`kernel/src/syscall.rs:132-537`) with its right: `WRITE` to signal, arm
and bind; `READ` to wait and poll; `GRANT` to badge and revoke; `ENUMERATE` to survey. `BIND`
additionally demands `WRITE` on the thread it names (`syscall.rs:610-622`), and `Timer::ARM`
demands `WRITE` on the notification capability in the caller's own table (`syscall.rs:644-652`), so
arming is signaling later under the same authority.

The badge cannot be forged. It rides the capability object, is minted only by `BADGE` (which
refuses a zero badge and an already-badged source, `syscall.rs:719-732`), and `SEND_CAP` carries it
unchanged through delegation. A sender's three words land in mailbox words zero to two. The kernel
writes word three from the capability, and word four never (`sched.rs:3388-3390`, `3597`,
`3793`); the death path writes word four as zero (`sched.rs:1938`). So `BOUND` in `x4` is kernel-only on
every path enumerated, which is what makes the bound wake tellable.

The bound wake was read as a receiver robbed of its message. `bound_receiver`'s test is "still
linked and undelivered" under one hold of `IPC_TABLES` (`sched.rs:2827-2838`); the delivery unlinks,
fills and serves in the same hold (`sched.rs:2848-2864`); the state machine beneath is Kani-proved,
its invariant that a non-zero word and a queued waiter never coexist. The interrupt-endpoint
refusal (milestone 603 (an interrupt's endpoint refuses every send)) marks the rendezvous before
the route publishes (`sched.rs:2559-2570`), and no syscall reaches `bind_irq`.

The designation half of milestone 205 (how a foreign program is told what to do) clamps in code,
not prose. The shell's asked rights are intersected with the manifest's declared grant
(`crates/system_initializer/src/lib.rs:3451-3462`), and for bytes nobody vouched for that manifest
is read-only. The set page is copied into a page from the job's region before the caretaker maps
it (`lib.rs:3469-3476`). Milestone 597 (a program carries its manifest in an ELF note) puts the
manifest inside the hashed bytes, and `decode` refuses every second spelling of a version 1
descriptor (`crates/capability_manifest_note/src/lib.rs:55-61`).

The nine new external packages are the TOML and serde stacks, plus `glob`. `toml` is depended on
by `xtask` alone, over recipes that are version-controlled, human-reviewed files; unknown keys are
refused (`xtask/src/package.rs:34-60`). No target-side parser reads TOML. The kernel's new library
split (milestone 609 (the system tests leave the kernel crate)) is feature-gated, and a kernel
binary built with the test feature fails to link on purpose (`kernel/Cargo.toml:200-205`).

## Findings

### 1. ACCEPTED: the nameset grant's filter can be raced at a real prompt, and the recording is accurate

Milestone 205's prompt builds an `fs_nameset_caretaker` that forwards checked names through the
file service's window zero, the unbadged default every legacy path maps
(`kernel/src/user/fs_service.rs:223-226`). The shell writes through the same frame while draining a
redirected job. The fix is milestone 599 (a frame per filesystem client channel)'s window pool, and
it is half-wired. `claim_window`'s callers are the test harness (`fs_service.rs:1500`,
`1504`, `1897`) and `release_window` is dead code awaiting the progenitor's reap
(`fs_service.rs:262-271`), so production still runs everything over the one frame. The race is
recorded at the mechanism (`components/src/swish.rs:2113-2120`), in milestone 205's block and in
`notes/a-set-grant-at-the-prompt.md`; this audit verified each claim against source and adds
nothing. The correction it demanded of `notes/shared-page-audit.md` is the documentation audit's
finding 3, fixed there.

### 2. ACCEPTED: the object registries are machine-wide, and a full one lies about why it refused

`MAX_NOTIFICATIONS` and `MAX_TIMERS`, 256 each (`kernel/src/sched.rs:2748`, `3109`), name every
notification and timer on the machine, and a full registry answers every creator with one flat
`OutOfMemory` (`sched.rs:2776-2778`), so a victim cannot tell a full registry from its own empty
region. One domain holding 256 pages of untyped can deny both objects to every other domain for
the rest of the boot. The shape is the rendezvous registry's (`MAX_RENDEZVOUS`, 512), which one
test suite actually filled, and the recorded stance is to raise the bound when a real workload
refuses. Recorded now in `notes/notification-objects.md`'s BUGS by this audit; a per-domain quota
was weighed and left unproposed, because nothing multi-domain has asked for the objects yet.

### 3. ACCEPTED: a notification word with its top bit set reads as an error

`Notification::WAIT` returns the word as `i64` (`syscall.rs:602`), so a word that lands on an
`Error` code decodes as that error and the waiter cannot tell. Found by milestone 151 (notification
objects) while checking §101 (notification objects: async multiplexing without wait-any); the fix
is a register convention, which every receiver in the tree is written against. The BUGS entries at
`crates/abi`'s `RECV` and `WAIT` and in milestone 151's block are accurate; this read confirms the
wart and the recording, and nothing in the window widened it.

## Is any confinement claim false as stated?

No. The one false claim met was not about code: `notes/shared-page-audit.md` finding 1(d) said the
shared-frame window was unreachable, which stopped being true when the 205 prompt opened it. That
is the documentation audit's finding 3, and it is fixed there.

## What was deliberately not examined

- The cryptography graph's code, the other untaken lens. It did not move in this window, and the
  supply-chain audit that reads `ring` and `rsa` still wants its own lane.
- The userspace programs of milestone 600 (the graphical terminal stack is built in userspace) and
  the TCO watchdog (kernel `arch/x86_64/tco.rs`): neither is syscall surface, and neither mints
  authority a confined process can reach.
- The FP register file, the port revoke and the DMA validator: the 2026-09-24 audit's territory,
  unmoved by this window.
- Timing channels beyond what the survey records: nothing was measured here.

## Method, so the negatives can be judged

The window was measured with `git diff --numstat` per directory from `1007edb1`. The surface was
read from `crates/abi` outward into `kernel/src/syscall.rs` and `kernel/src/sched.rs`, with the
mailbox writes enumerated at every assignment site rather than trusted from doc comments. The
designation clamp, the manifest decoder and the TOML consumer were read at their sites. `script/
verify` was not run; the notification state machine's proofs replay in CI.

## What wants a lane of its own

- The supply-chain lens, unchanged from the 2026-09-24 audit's list, still untaken.
- Milestone 599's outstanding half, the progenitor's window claim and reap, which is what keeps
  finding 1 live.
- The registry quota question of finding 2, if a second trust domain ever asks for notifications.

## Process notes

The delegated reader's failure mode from the 2026-09-24 audits (a claim source reading disproves)
was met once here and caught by reading: this lane first believed `claim_window` served production
clients, from the pool's documentation; the call sites say otherwise. The discipline held: every
claim above names its file and line.

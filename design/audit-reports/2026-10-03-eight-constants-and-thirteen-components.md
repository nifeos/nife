# Security audit, 2026-10-03: eight constants and thirteen components, read as a confined process trying to widen

**Kind:** security. **Lens:** the delta since the 2026-09-29 audit and nothing else: the eight ABI
constants and the one whose value changed, the one wire semantic the window altered, and the
thirteen components, each read for what a confined, hostile process can do with it and for what it
holds against what it uses. **Findings:** fixed 5, minted 4, accepted 6, of which findings 10 to
16 are the follow-up's reconciliation of the component survey.

The kernel's new surface refuses a confined process everything it refused before. The finding to
carry off is a grant in userspace: a graphical terminal session on the no-keyboard arm holds the
boot line discipline's whole endpoint, and the discipline does not ask who is holding it, so a
compromised session can queue the boot shell's next command line (finding 2). Nothing here moves
the verdict on fatal risk 7.

## Why this lens

`script/audits --due` on `main` at `1d33b27fa` fired two triggers against the 2026-09-29 baseline
(`2533877d2`, the commit that landed that report): components 184 to 197 (+13, fires at 8) and ABI
constants 68 to 76 (+8, any change fires). Milestones built went 276 to 288 (under 15) and
external packages stayed at 181. calef approved the audit on 2026-10-03 UTC and scoped it to the
two deltas, so the lens is the delta and not the tree.

### The delta, counted the way `script/audits` counts

ABI constants, by `diff` of the `pub const NAME: u64` lines of `crates/moho_abi/src/lib.rs` between the
two commits. Eight new, all from milestone 126 (the `procps` package), ratified by calef on
2026-09-27 (#1360):

| Constant | Value | What it is |
|---|---|---|
| `memory_region::USAGE` | 5 | a method: how much of this region was spent, and on what |
| `usage::SIZE` | 0 | selector: pages the region holds |
| `usage::COMMITTED` | 1 | selector: the watermark |
| `usage::FRAMES` | 2 | selector: plain pages over the subtree |
| `usage::RENDEZVOUS` | 3 | selector: rendezvous pages over the subtree |
| `usage::ADDRESS_SPACES` | 4 | selector: address-space roots over the subtree |
| `usage::THREADS` | 5 | selector: thread control blocks over the subtree |
| `usage::CHILDREN` | 6 | selector: pages carved into live children |

One changed: `CAPABILITY_TABLE_SLOTS` 24 to 32 (calef, 2026-09-27), which moves the derived
`fault::FAULT_EP_SLOT` from 23 to 31. And one wire semantic changed without a constant: since
`4feaf0dc2` (2026-10-02, milestone 613 (a system log service: the in-memory half)) a plain `SEND`
carries the badge of the capability the
sender invoked, so `RECV` returns it in `x3` where it returned `0` before.

Components, by `diff` of `crates/*/` and the `[[bin]]` names in `components/Cargo.toml` and
`fixtures/Cargo.toml`. Six crates: `free`, `vmstat`, `slabtop`, `machine_statistics_protocol`,
`system_log`, `system_log_protocol`. Six programs added and one retired in `components/`: `free`,
`vmstat`, `slabtop`, `system_log`, `graphical_terminal` and `session` added, `session_reviver`
retired (not renamed: `login` re-derives durable sessions at start-up, `850372dcd`). Two fixtures:
`greeting_two` and `noteless`. That is 6 + 5 + 2 = 13.

### The two uncountable triggers, answered

Has a new component taken device or network authority? **No.** `graphical_terminal` holds no
device: the GPU's four capabilities and the keyboard's three go from the shell's slots 22 to 28 to
`gpu_driver`, `display_terminal` and `keyboard_driver` (`crates/system_initializer/src/lib.rs`,
`build_graphical_terminal_session`), and all three drivers predate this window. What moved is the
shell: the progenitor now places those seven in `swish` with `GRANT` for the shell's whole life,
device authority widening an existing component rather than arriving with a new one, which is the
case the count cannot see and the first draft of this answer missed (finding 10). `system_log` holds
one endpoint, four notifications and up to four reader windows, and nothing starts it at boot yet.
`session` holds a budget, two read-only store caretakers and two pages. `free`, `vmstat` and
`slabtop` hold a read-only page and an `ENUMERATE` view. The fixtures hold their output endpoint.

Has this booted on a new machine class? **No.** `bench/` moved only its fastpath numbers in the
window, and no commit records a first boot.

## What was read, and what came back clean

**`USAGE` and its selectors** (`kernel/src/system_call.rs`, `memory_region_usage`;
`kernel/src/memory_region.rs`, `usage_record`; `crates/memory_regions/src/table.rs`, `spent`).
Confused deputy: the selector arrives in `a0` and nothing is read from user memory. Slot misuse,
the class of milestone 634 (a plain SEND received by RECEIVE_CAP never hands the receiver a
sender-chosen slot): no slot goes in and none comes out; the answer is a page count in `x0`.
Badge: none involved. Bounds: `abi::usage::is_known` (`record <= CHILDREN`) is checked before the
region is looked up, so the kernel's `_ => PageUse::Children` arm is unreachable today; the sum is
over at most `MAX_REGIONS` (256) regions of page counts in a `u64`, and `pages as i64` cannot clip
below 2^63 pages. Revocation: the name resolves generationally under the region lock, and a reaped
region answers `Gone`. Rights: `ENUMERATE` alone, the rule of §114 (`ENUMERATE` extends to the
address-space object), and the agent's survey confirmed
that `WRITE` is still what every spend, split and destroy asks for. Parity: the arm is shared
`system_call.rs`, no `arch::` code, and `#[inline(never)]` keeps it out of the measured `system_call_entry`
footprint (the first run with it inline measured 5.6% on riscv64 and 8.8% on `x86_64`, over the
bound). The one thing it costs is finding 6.

**`CAPABILITY_TABLE_SLOTS` 24 to 32.** `FAULT_EP_SLOT` is derived, and the machine statistics page
now rests at kernel slot 23 for the progenitor. The TCB still fits its page: `kernel/src/thread.rs`
pads `size_of::<Thread>()` to the FP state's alignment, and the window's `sched.rs` diff shows the
one casualty, the idle thread's by-value spawn, which held three `Thread`s in one frame (4,368
bytes at 32 slots) over the guard page and was rebuilt in place. `strand_callers_of`'s victim array
is sized by the constant. No `offset_of!` assertion in `kernel/src/arch/` names `Thread` (they
cover `TrapStash`, `FpState` and `Tss`), and a grep for `24` or `23` as a slot bound in `crates/`,
`components/` and `fixtures/` finds only unrelated literals.

**The plain-`SEND` badge** (`kernel/src/sched.rs`, `wide` and `ipc_send_badged`). The badge is the
kernel's word: the `SEND` arm reads it from `cap.object`'s `Rendezvous(ep, badge)` in the caller's
table, never from a register. `BADGE` still refuses a zero badge and an already-badged source, so a
holder cannot re-badge. The abi doc claims "nothing reads `x3` before checking" the first word;
every `recv_fault` caller in the tree (`session`, `terminal_supervisor`, `swapper`, `swish`,
`timetable`, `job_undertaker`, `c_confiner`) either tests `event` first or only reports the
words. The record of this change is finding 1.

**Milestone 634's fix**, which landed in the window and is the window's own confinement work:
`Thread::cap_delivered` is cleared when a `RECV_CAP` parks, set by the two paths that install a
capability, and the mailbox read returns `NO_CAP` in `x1` otherwise, with the bound-notification
delivery exempt by its kernel-only `x4`. The sibling paths it settles for free were checked: an
interrupt signal now reads `[1, NO_CAP, 0, 0, 0]` on both orders, and a death message reads
`[event, NO_CAP, tid, addr, 0]` on both (finding 7). The spawn service's own
`recv_cap(spawn_ep).1` for the machine page was in 634's class: a hostile session could have named
a spawner slot for `cap_delete` to clear; it is closed by the same receive-side rule.

**The machine statistics page** (`kernel/src/machine_statistics.rs`,
`crates/machine_statistics_contract`). Minted once with `READ | GRANT` for the progenitor; the
shell gets it at `MACHINE_PAGE_SLOT` when `GRANT_MACHINE_PAGE` says the owner allows it, and a
program that declares `machine` gets it mapped `MAP_RO`, which `page_frame_map` enforces by
refusing `MAP_RW` without `WRITE`. Every per-core index is `word::cpu(cpu::id())` with `cpu::id()`
asserted under `MAX_CPUS = CPU_ID_BOUND = 8` on every architecture, and a compile-time assertion
keeps the page under 4,096 bytes. The reader copies the page before decoding and checks only the
magic; the arithmetic downstream of kernel-written counters (`vmstat`'s `busy + idle`, `free`'s
`pages * 4`) is unchecked and wraps in release at values only a kernel bug could write. Not a
finding: the page is the kernel's and read-only.

**The thirteen components**, what each holds and what a compromise of each reaches. Manifests for
archive programs are rows in `crates/grant_plan/src/lib.rs`; none of the thirteen carries an ELF
note.

| Component | Holds | A compromise reaches |
|---|---|---|
| `free` | output, diagnostics, machine page `READ` and mapped `RO`, share view `ENUMERATE` | machine-wide counters; the job pool's usage (finding 5) |
| `vmstat` | output, diagnostics, machine page | machine-wide counters, read only |
| `slabtop` | output, diagnostics, share view `ENUMERATE` | the job pool's usage (finding 5) |
| `machine_statistics_protocol` | a layout, no authority | nothing |
| `system_log`, both crates | intake endpoint `READ`, four notifications `WRITE`, reader windows `RW`; spawned only by its test | every record, every reader's window; no device, filesystem or network (finding 8) |
| `graphical_terminal` | `result_ep`, `write_ep`, `keys_ep`, the output page `RW`; arm 1's `keys_ep` is the boot discipline itself | arm 0: its own session; arm 1: the boot shell's input (finding 2) |
| `session` | a 416-page budget `WRITE\|GRANT`, two read-only store caretakers, the registration page, the durable window page; supervises `timetable` | the user's staged bytes in transit; no filesystem root, which `session_reviver` had held |
| `greeting_two`, `noteless` | output only (the no-note default is `Prog::Uptime`'s manifest); unvouched adds the read-only clock and config pages | their output endpoint |

The no-note path was read as the thing `noteless` exists to prove: an installed package without a
note gets output and nothing else, and an unvouched one is refused unless the run-unvouched
capability is presented (`image_manifest`, `grant_plan`). `greeting` 0.1.0 asks for the clock in
its note and 0.2.0 has none, so the two versions hold different authority, which is the package
manager's rule working rather than a defect.

## Findings

### 1. MINTED: the plain-`SEND` badge is on the wire and not in §230 (badged endpoint capabilities)

`RECV`'s `x3` changed meaning for every program on 2026-10-02. calef ruled it on #1494; milestone
634's block and the risk 7 record cite that ruling; `crates/moho_abi` documents it as "milestone 613's
amendment to §230". `grep -n '613\|plain SEND' design/decisions/0230-*.md design/decisions/0242-*.md`
finds nothing. CLAUDE.md's rule for the surface is that a method's semantics are recorded in
`design/decisions/`, and §230's table still says the badge rides `CALL` and `SEND_CAP` only. A
lane may not edit `design/decisions/`, so the amendment is proposed for the integrator to mint at
merge: `design/roadmap/0711-section-230-records-that-a-plain-send-carries-its-badge.md`.
Severity: low, a record; the risk is the next server written against §230's table.

### 2. MINTED: arm 1 of the graphical terminal holds the whole boot discipline

On the no-keyboard arm the spawn service places the boot line discipline's endpoint `term_ep` at
the session's slot 2 with `WRITE`, keeping `WRITE | GRANT` for exactly this
(`crates/system_initializer/src/lib.rs`, the comment at `cap_delete(term_out)`). The program sends
`OP_RAWMODE` and `OP_READRAW` (`components/src/graphical_terminal.rs`, `KEYS`). The discipline
serves every request alike whoever holds the endpoint (`components/src/line_editor.rs`, `TERM`:
"an `OP_BYTES` CALL looks the same regardless of who is holding the other end"), so the same
capability answers `OP_BYTES`, `OP_READLINE` and `OP_PRINT`. The spawn service's comment names
`OP_READLINE` as the widening it accepted, on the ground that the prompt cannot delegate the
endpoint; that closes the prompt's path and leaves this one.

Repro, by construction: a session turns the discipline cooked (it does so courteously on exit) and
`OP_BYTES` a line before exiting; the shell's next `OP_READLINE` at its prompt reads it and runs it
with the shell's authority: the seven device capabilities at slots 22 to 28 with `GRANT`, the spawn
service, the filesystem. Not demonstrated under QEMU; a red test is the fix lane's first step.
Severity: medium. It needs a compromised session on a GPU-and-UART machine (xenon's shape), and
what is gained is the shell's authority, not the kernel's. The kernel confined the session exactly
as granted; the grant is wider than the use. Recorded in `graphical_terminal.rs`'s BUGS; the fix
(a badged copy of `term_ep` that the discipline answers only the raw requests on, §230's shape one
component over) is `design/roadmap/0709-arm-1-holds-only-the-raw-half-of-the-boot-discipline.md`.

### 3. FIXED: `crates/moho_abi` said milestone 634's escape was still open

The `RECV_CAP` BUGS entry, written by milestone 613's lane on 2026-10-03, said the sender-chosen
`x1` was "not fixed there" and "wants its own lane". The lane came the same day, milestone 634
landed in `sched.rs`, and the entry did not move (`git log -- crates/moho_abi/src/lib.rs` shows no commit
from 634). A BUGS entry that reports a closed escape as open is the overclaim in the other
direction, and the first thing an outside reader of the surface meets. Rewritten to record the fix
and when the record caught up.

### 4. FIXED: `free` said "this prompt's job budget", and the view is the machine's job pool

`crates/free/src/lib.rs` described the `Yours:` line as "this prompt's job budget". The region at
`grant_plan::SHARE_SLOT` is `jobs_ut`, the one region the progenitor splits every job and every
graphical terminal session from (`crates/system_initializer/src/lib.rs`, `memory_region_split(ut,
JOBS_BUDGET_PAGES)` and `split_job(jobs_ut, ...)`), and `USAGE`'s subtree records sum all of them.
Corrected at the three places the crate says it.

### 5. ACCEPTED: the share view is a counting channel across sessions

The consequence of finding 4: `free` and `slabtop` in one session see pages another session spent,
a number and never a name, of the kind the 2026-08-17 audit accepted for `SURVEY`. A per-prompt
budget would be a split the progenitor does not make today, and nothing a holder learns here lets
it act. Recorded in `crates/free/src/lib.rs`'s BUGS.

### 6. ACCEPTED: `USAGE`'s subtree records walk the whole region table with interrupts masked

`spent` sums every region that descends from the named one, and `descends_from` walks each
candidate's parent chain, under `REGIONS`, an `InterruptSafeMutualExclusionLock`. Measured on the host (a throwaway
test, not committed): 8 ns for a root with eight children; 42 µs for a root over the deepest chain
the table can hold (256 regions, each split from the last); 62 µs for that chain's leaf, which
rejects every candidate after a full walk. A holder of `ENUMERATE` on any region can repeat the
call, so this is a bounded interrupt-latency cost a hostile process can impose, of the same shape
as the reclaim scans and the registry bounds the 2026-09-29 audit accepted. The bound is
`MAX_REGIONS`, not the caller's. Recorded in `crates/moho_abi`'s `USAGE` BUGS with the numbers.

### 7. ACCEPTED: a death message read through `RECV_CAP` loses `pc`

The kernel's five words are `(event, tid, pc, addr, 0)`. `RECV_CAP`, having no capability to
deliver, now returns `(event, NO_CAP, tid, addr, 0)` on both arrival orders; before milestone 634
the receiver-first order returned `tid` in `x1`, so the two orders disagreed. No supervisor in the
tree receives deaths this way (every one uses `RECV` through `recv_fault`), and the fix would be a
third shape for one mailbox read. Recorded in `crates/moho_abi`'s `RECV_CAP` BUGS so the next supervisor
written against it learns there.

### 8. ACCEPTED: the system log has no per-writer quota and a writer picks its severity

The ring (64 KiB) evicts oldest-first across every writer, so one badge writing fast enough
pushes every other writer's records out before a reader drains them; severity is the writer's
framing bits. Both are what `syslog(3)` allows a client. What the badge guarantees is `program`
and `user`: the registry is written from badge 0 only, `OP_READ` is answered only for a badge
registered as a reader, and the kernel refuses to re-badge. Nothing starts the service at boot yet.
Recorded in `crates/system_log/src/lib.rs`'s BUGS.

### 9. ACCEPTED, met beside the scope: a program's answer word is its own claim

A child whose slot 0 was not redirected holds `result_ep` with `WRITE`, the endpoint the spawn
service's `SPAWN_FAILED`, `job_undertaker`'s `JOB_FAULTED` and the shell's `RESULT` reads share.
The shell takes three words and tests `w0`, so a child can lie about its own exit status, and no
more: the wait is one job at a time. Older than the window, and met because the survey of `free`'s
slot 0 landed on it. Since milestone 613 the badge in `x3` would let the shell tell the spawn
service from a child at no new authority. Recorded in `components/src/swish.rs`'s BUGS.

## Findings 10 to 16: the follow-up's reconciliation

The maintainer read the component survey behind this report and found seven items the table
above did not carry. Evidence for each is in
[the appendix](2026-10-03-eight-constants-and-thirteen-components/reconciliation.md).

### 10. MINTED: the shell keeps the seven display capabilities for its whole life

With `GRANT` on all seven, `WRITE` on the three pages and `READ` on both interrupts. It can map the
keyboard DMA page and the surface and can receive an interrupt wake meant for a driver; it does
neither, and uses them only to delegate. The spawn service already holds `term_ep` for the same
purpose without lending the shell `GRANT`. Severity: medium as a width, low as a reach, since the
shell is the prompt's own authority. BUGS in `swish.rs`; proposal
`0715-the-spawn-service-holds-the-display-grants-and-the-shell-holds-none.md`.

### 11. MINTED: a real capability passes the `NO_CAP` guard

Milestone 634 closed the sender-chosen number; it recorded that no server checks the kind of
object in `x1` and no method lets one. A client with `GRANT` on any capability `SEND_CAP`s it to a
`CALL` server, `x1` is a real slot, and `reply` runs method `0` on it: `SEND` on the client's own
rendezvous parks the server forever, and every non-Reply delivery leaks a slot of 32. Severity:
medium. A denial of service on `net_stack`, the compositor and the file service by any client. On
the system call surface, so calef's call: BUGS at `RECV_CAP`, proposal
`0706-a-call-server-can-tell-a-reply-from-a-delegation.md`.

### 12. FIXED: a stale `outgoing_cap` survived a rendezvous teardown

`set_ipc_aborted` now drops the staged delegation; before, a `SEND_CAP` aborted by `reclaim_region`
left it for the sender's next plain `SEND` on any other rendezvous to deliver. A delegation made to
one endpoint arriving at another. Test in `recv_cap_attack_tests.rs`.

### 13. FIXED: `caps` under-reported a package's `machine` and `share`

It read the placeholder row's manifest where every other row reads the note's. Host test extended.

### 14. FIXED: milestone 634 moved a `cfg(test)` onto the wrong module

`revocation_in_flight_tests` was declared bare. Both modules carry their own attribute, and
`script/lint` now refuses a bare `mod` in `system_tests/src/user.rs`.

### 15. ACCEPTED: the log's intake reads kernel deliveries as the spawner or a writer

Badge `0` is control and the kernel writes `0` there for a signal or a bound wake. Recorded in
`crates/system_log`'s BUGS as a rule on the intake, before milestone 342 (the kernel and the
`console` server drive one UART from two address spaces) wires the service in.

### 16. Refuted: the terminal's 4 KiB `line` array fits its stack

Every child the progenitor builds gets `CHILD_STACK_PAGES`, twelve pages, and the two arrays are
one per arm. Not a finding.

## Is any confinement claim false as stated?

No kernel claim. The one false statement met was `crates/moho_abi`'s own BUGS entry reporting a fixed
escape as open (finding 3). Finding 2 is not a kernel claim failing: the session holds exactly what
the spawn service granted, and the spawn service's comment shows the width was seen and accepted
for the prompt's path only.

## Fatal-risk implications

Risk 7 (the confinement claim is false) is AMBER as of 2026-10-03, with milestone 634's two escapes
recorded under §216 (fatal-risk facts are correctable, and verdicts are the architect's). Nothing
here moves it. Finding 2 is a least-authority defect in a userspace
grant, which is the risk's territory only if the kernel let a session reach past its table, and it
did not; the record should carry it as a grant to narrow, not a claim to test. Finding 1 is a
missing record of a change the risk's own facts already rely on. Finding 11 is an input to the
risk's record: milestone 634's own scope note said no server can tell what kind of object it
received, and this follow-up read that note as the attack it describes, a denial of service on any
`CALL` server by any client rather than an escape. Whether that moves the verdict is calef's; the
proposal carries the options and their costs. This report does not edit `design/fatal-risks/`.

## What was deliberately not examined

- The rest of the surface: the 2026-09-29 audit read it whole, and only `USAGE` was added since.
- The three graphical drivers (`gpu_driver`, `display_terminal`, `keyboard_driver`) beyond who
  holds what: they predate the window, and the 2026-09-29 audit left the programs of milestone
  600 (the graphical terminal stack is built in userspace)
  unread, so they are still the lens no audit has taken. `display_terminal`'s escape parser is
  what a compromised terminal on either arm reaches.
- The supply-chain lens, untaken since 2026-09-24. External packages did not move.
- Timing beyond finding 6's one measurement. The machine page is a deliberate activity channel
  (§225 (`free` sees the machine and your share)) and was not measured as one.
- A QEMU demonstration of finding 2.

## Method, so the negatives can be judged

The delta was taken by `diff` between `2533877d2` and `1d33b27fa` with the same regular expression
and the same manifests `script/audits` reads, then `git diff` of `crates/moho_abi`, `kernel/src/system_call.rs`
and `kernel/src/sched.rs` over the window was read whole. Two delegated surveys read the thirteen
components for their manifests, grants and reach, with file and line for every claim; every claim
that became a finding was re-read at its site by this lane first. The delegated reader's failure
mode from the two previous audits appeared once and was caught the same way: a survey reported
`session` as `session_reviver` renamed, and `git log` says one was retired and the other added,
with different authority. The `spent` measurement is `cargo test --release` on a
`RegionTable::<256>` in a test file deleted after the run.

## What wants a lane of its own

- Finding 2's fix, with the red test first.
- The three graphical drivers as a lens: device authority, read adversarially, nobody has taken.
- The outstanding half of milestone 599 (a frame per filesystem client channel) and the
  supply-chain lens, both carried from the last two reports unchanged.

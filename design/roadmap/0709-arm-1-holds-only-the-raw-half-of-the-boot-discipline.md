---
status: BUILT
built: 2026-10-03
raised: 2026-10-03
promoted_from: arm-1-holds-only-the-raw-half-of-the-boot-discipline
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 709. A graphical terminal session on the no-keyboard arm holds only the raw half of the boot discipline

*Title and slug are drafts.*
Raised by the 2026-10-03 security audit
(`design/audit-reports/2026-10-03-eight-constants-and-thirteen-components.md`, finding 2), reading
`graphical_terminal`, one of the thirteen components the window added, for what it holds against
what it uses.

## The gap

Milestone 632 (graphics on demand: `graphical_terminal`, launched from the swish prompt) gives a session with no
virtio keyboard its keystrokes from the boot's own line discipline. The spawn service keeps the
discipline's endpoint `term_ep` with `WRITE | GRANT` for exactly this and places it at the session's
slot 2 with `WRITE` (`crates/system_initializer/src/lib.rs`, the `None =>` arm of
`build_graphical_terminal_session`). The program uses two requests on it, `OP_RAWMODE` and
`OP_READRAW` (`components/src/graphical_terminal.rs`, `KEYS`).

The discipline answers every request on that endpoint the same whoever holds the other end
(`components/src/line_editor.rs`, the `TERM` doc: "an `OP_BYTES` CALL looks the same regardless of
who is holding the other end"). So the session's capability also answers `OP_BYTES` (feed
keystrokes into the queue the boot shell reads), `OP_READLINE` (take the shell's next line) and
`OP_PRINT` (write to the UART). The spawn service's own comment at `cap_delete(term_out)` names
`OP_READLINE` as the widening it accepted, because the shell's copy carries no `GRANT` and nothing
at the prompt can hand the discipline to a program of its choosing. That closes the prompt's path
and leaves the spawn service's.

What a hostile session does with it: before exiting, turn the discipline cooked (it already does,
courteously) and `OP_BYTES` a command line. The shell's next `OP_READLINE` at its prompt reads it
and runs it with the shell's authority: the seven device capabilities at slots 22 to 28 with
`GRANT`, the spawn service, the filesystem. The session holds none of those. This is read from the
dispatch and the grant, not demonstrated under QEMU; a demonstration is the first step of the fix
lane, as a red test.

Severity: medium. It needs a compromised session (the stack is the tree's own code, spawned by the
shell) and the no-keyboard arm (a real board with a GPU and a UART console, which is xenon's
shape). What is gained is the shell's own authority, not the kernel's. The kernel confined the session
exactly as granted; the grant is wider than the use.

## What else was considered

- Leave it, on the argument that a session is the tree's own code. Refused: the point of the
  grant table is that a program holds what it uses, and this is the one place in the launch where
  a session holds a server's whole surface.
- A second endpoint object in the discipline, served alongside `TERM`. Works, but costs the
  discipline a second receive loop or a `RECEIVE` on two objects it has no way to wait on at once
  (no wait-any: §101 (notification objects: async multiplexing without wait-any)).
- A badged copy of `term_ep` (chosen). §230 (badged endpoint capabilities) and milestone 613 (a
  system log service: the in-memory half) made the badge the kernel's word on every receive, and
  the discipline already serves with `RECEIVE_CAP`, which returns it in `x3`. The
  spawn service holds `GRANT` on `term_ep` and can mint one badged copy per session
  (`rendezvous::BADGE`); the discipline refuses `OP_BYTES`, `OP_READLINE` and `OP_PRINT` from any
  non-zero badge. One `match` arm in `line_editor.rs`, one mint in the spawn service, and the
  session's slot 2 is a capability that can read keystrokes and nothing else.

## What this tree already does in the analogous case

The file service tells clients apart by badge and scopes each to its binding
(`crates/subtree_scope`, §230). The system log honors control words from badge 0 only
(`crates/system_log/src/lib.rs`, `handle`). This is the same shape one component over.

## What it costs

Two edits and a test. The test is a kernel system test on the discipline: a badged holder's
`OP_BYTES` is refused and its `OP_READRAW` served, red before the `match` arm and green after. No
syscall surface, no wire format, no dependency.

## What is blocked until it lands

Nothing. The finding is recorded in `components/src/graphical_terminal.rs`'s BUGS where a reader
of the arm meets it.

## What is built

The badged copy, as chosen above, with one change to its shape: the discipline serves a badged
holder an **allowlist** (`OP_RAWMODE`, `OP_READRAW`) rather than refusing a list, so `OP_WRITE`,
`OP_INTRCOUNT`, `OP_QUIESCE` and any request added later are refused too.

- `crates/line_editor`: `proto::RAW_ONLY_BADGE` (provisional name). The rule is any non-zero badge,
  so the discipline holds no table; the value only makes the minting site and the test agree.
- `components/src/line_editor.rs`: the guard ahead of the dispatch, reading the badge
  `recv_request` already returned.
- `crates/system_initializer`: arm 1 mints `boot_terminal` badged `RAW_ONLY_BADGE` per launch,
  grants that copy at slot 2 and deletes it once the session holds it. One slot for the build,
  below arm 0's peak. Every holder the boot wires (input driver, shell, `login`) stays unbadged.
- `components/src/graphical_terminal.rs`: its BUGS entry now records what is left (below).

### Measured first, 2026-10-03 (UTC), aarch64

The test went in before the fix. A badged `CALL` (`ipc_call_badged`, the kernel's own delivery of a
badged capability) of `OP_BYTES "ls\r"` was served: red at `raw_mode_tests.rs:445`, "a badged
holder's OP_BYTES was served (r0 = 0)", exit 1. This shows the discipline ignored the badge. It is
not a full hostile session under QEMU: that would need arm 1's GPU launch plus a modified
`graphical_terminal`, and the grant itself was read from the spawn service, not observed.

### The test, and its falsification

`a_badged_copy_of_the_terminal_reads_keystrokes_and_cannot_type_them` in
`system_tests/src/user/raw_mode_tests.rs`, against a real `line_editor` process. The badged holder
types `ls\r` cooked (refused), the shell's `OP_READLINE` then gets exactly the input driver's `ok`,
five other requests are refused, and `OP_RAWMODE` and `OP_READRAW` work through the same badged copy.

| run | tree | result |
|---|---|---|
| before the fix | test only | exit 1, red at `raw_mode_tests.rs:445` |
| after the fix | filter `raw_mode_tests` | 8 of 8 pass, exit 0 |
| replay | fix with the record applied | exit 1, red at `raw_mode_tests.rs:445` |

The record is `system_tests/falsifications/user.raw_mode_tests.a_badged_copy_of_the_terminal_reads_keystrokes_and_cannot_type_them.patch`.

### Architectures

The discipline and the spawn service are one source for every ISA, and the test is in the shared
suite. It was booted here on aarch64 only; riscv64 and x86_64 run it in CI. Arm 1 itself is
launched end to end by `script/swish-check`'s aarch64 and riscv64 legs, which CI runs; x86_64 has no
graphical launch, which is milestone 632's scope and not a gap this milestone adds.

### What is left

**A session can still switch the boot discipline's mode.** `OP_RAWMODE` is in the half it uses, so
a session can flip the discipline under the shell, abandoning a half-typed line and failing a parked
read with `BAD_REQUEST`; the shell's §227 (how Tab reaches the shell) recovery takes raw back. A disruption, not authority.
Recorded in `components/src/graphical_terminal.rs`'s BUGS.

## Follow-on

- **Recorded.** A session can still switch the boot discipline's mode under the shell, in
  `components/src/graphical_terminal.rs`'s BUGS.

## Index row

A `graphical_terminal` session with no keyboard receives the boot discipline's endpoint with `WRITE` but uses only two requests on it. BUILT: it holds a badged copy the discipline answers only `OP_RAWMODE` and `OP_READRAW` on, with a test that goes red without the fix (2026-10-03 security audit, finding 2).

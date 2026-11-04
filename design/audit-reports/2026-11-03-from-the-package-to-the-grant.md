# Security audit, 2026-11-03: from the package to the grant

**Kind:** security. **Lens:** the window since 2026-10-26, read as a hostile package author and as a
confined process trying to widen: the path the window built from a package's bytes to the grants a
program holds (the approval file, a program's asks, the `INTERFACES` note, the system manifest and
the base digest page), then the runtime halves split out of the supervision and hot-swap contracts,
the new survey record, the device-tree patcher and the eight new fixtures. **Findings:** fixed 2,
minted 4, accepted 4.

The finding to carry off is an order. The installer's delegated check, which bounds what a script
holding an installer badge may install, runs after the package's bytes are written to the store,
although its own comment says it runs before anything is placed. The refusal still holds: no table
row and no grant is written, and spawn hashes what it runs. But the bound the owner's approval
claims over the store does not hold, and a refused package can replace the bytes at a path a person
runs by name. Minted, because the fix moves a check on the install path and wants its own test.

Nothing read lets a hostile note, approval file, manifest or digest page reach a grant wider than
the owner approved, and nothing read lets a confined process widen.

## Why this lens

`script/audits` on `main` at `88c730fa9` reported both kinds DUE on the same two triggers:
components 236 to 252 (+16, fires at 8) for security, 235 to 252 (+17, fires at 10) for
documentation, and ABI constants 97 to 99 (+2, any change fires) for both. Issue #3098 is the
scheduled workflow's repeat row; each of its red runs was the tripwire reporting a due audit, not a
broken run. This report closes the security kind. The documentation sweep that closes the other is
[2026-11-03-names-the-window-moved.md](2026-11-03-names-the-window-moved.md), run in the same lane.

The delta, counted the way `script/audits` counts, between `d8255188e` (the commit that landed the
2026-10-26 report) and `88c730fa9`:

| What | Added | Left |
|---|---|---|
| crates | `approval_file`, `base_digest_contract`, `device_tree_patch`, `hot_swap_server`, `initial_ramdisk_writer`, `interfaces_note`, `intrusive_queue`, `periodic_deadline`, `primitive_benchmark_contract`, `process_builder`, `program_asks`, `supervision_contract`, `system_manifest` | `intrusive_fifo` and `supervision_protocol` (renamed), `jh7110_entropy` (moved to its own repository), `usb_keyboard_report` (folded into a driver contract) |
| programs | fixtures `job_sleeper`, `servant`, `seeker`, `seeker_v2`, `map_el0`, `null_syscall`, `sink_consumer`, `sink_producer` | `rmle` (removed) |
| ABI constants | `INTERFACE_ID` and `survey::record::PROCESS` | none; four capability names were spelled out, which moves no count |

Six of the new crates carry the window's largest new trust path: a package's bytes, an owner's
approval and a base image's manifest become what a program is granted. No audit has read that
path, because it did not exist at the last one. It is the lens the last one lacked, so this audit
takes it first and the rest of the delta second. Two readers each took one half, and every finding
below was checked against the code by the audit's author before it was written down.

### The two uncountable triggers, answered

Has a new component taken device or network authority? **No.** The new crates hold no device page,
interrupt or port range. The base digest page is a frame the kernel writes and grants read-only
without `GRANT`. The device-tree patcher runs in the loader, before any of this.

Has this booted on a new machine class? **No.** The window's merges add no board and no cloud.

## What was read, and what came back clean

**The approval file.** One parser serves both the device and the image composer. A duplicate key,
zone, directory or package section, or a second `[base-update]`, is refused; nothing is last-wins,
and an oddly quoted section name only makes a section that matches nothing. Compiled into the
installed-programs approval document, a name the record reader would refuse fails as `BadName`, so
nothing broader is ever stored. An ask is covered only by an exact match (a directory by
`is_within`). `base_update::check` fails closed on every branch and requires a strictly greater
version, refusing leading zeros and overflow.

**A program's asks.** A second manifest note is refused, across segments or within one. Any
decode failure gives no asks, and spawn refuses the same bytes, so no grant can exceed a note. A
zone that is not UTF-8 is dropped, which only narrows.

**The `INTERFACES` note.** At most 64 records, an exact length, ids strictly ascending; the length
arithmetic cannot overflow and no slice can run out of bounds. The ELF note walk uses checked
arithmetic and allows only 4- or 8-byte padding.

**The system manifest and the base digest page.** The manifest has one encoding: lowercase digests,
no carriage return, sorted, one progenitor line. No stem can be `progenitor` or hold `/` or a space.
The digest page's reader rejects a short page, a wrong magic, a wrong version and an all-zero
digest. The progenitor hashes the same slice of the measurement table that it then uses, so there
is no window in which to swap it.

**The delegated rollback and remove.** A rollback checks the grants of every row it restores that
the live generation does not hold. It skips a row the live generation holds by program and digest
without comparing grant lines; that is sound because a row's asks are a function of its digest's
note, so the same digest carries the same asks in both generations. A remove is refused unless the
approval says `remove = true`, before anything is written.

**The process builder.** `elf::parse` checks offset plus file size, address plus memory size, and
memory size against file size with checked adds, and Kani proves the overflow checks. A segment
both writable and executable, or two segments on one page, is refused. The child receives its own
space without `GRANT`, its own thread with `WRITE` only, its own process with exactly `BIND` (the
builder deletes its own copy), and its fault endpoint with `READ` in a reserved slot. The thread
capability handed back to the requester is narrowed to `WRITE` without `GRANT`, and the kernel
refuses the spawn methods on a thread that has started. Compared with comments stripped, the logic
moved out of `supervision_protocol` unchanged, and the supervisor and spawner diffs are renames.

**The hot-swap server.** It moved out of `hot_swap_contract` unchanged. A client can ask any server
to quiesce; the crate's `BUGS` already records that, with the fix (an operator badge). The wedge
path parks forever only on the hung-channel test.

**The survey `PROCESS` record.** It needs `ENUMERATE`, an unknown record is refused before the walk,
and the label counts only threads whose fault endpoint is the caller's domain. It returns no
process id and no address, so the "ordinal, not identity" claim in `crates/abi` holds.

**The rest.** `periodic_deadline::next_after` saturates its adds, subtracts only when `now` is not
before the previous deadline, answers `NEVER` for a zero period, and has Kani proofs. The
device-tree patcher's header offsets are checked against a total size that must fit the buffer, and
its output writer never writes past its buffer. None of the eight fixtures is in any package but
`fixtures`, which `base-image.toml` and `xtask/src/base_image.rs` keep out of release images, and
each one's grant manifest asks for no more than its test needs.

## Findings

### 1. MINTED: a delegated install writes the package before the approval check

`install` in `crates/system_initializer/src/activation.rs` calls `place_program` for every program
in the package, and only then `approval_gate`. The gate's own comment says the check runs "before
anything is placed or written", and the comment beside the placement admits that a refused
package's bytes stay under `packages/`. A holder of a delegated installer badge who sends a package
its approval does not cover, or one marked `update = false` that is already installed, is refused,
but the bytes are written or replaced in place first.

No authority widens: no row and no grant is written, and spawn hashes the bytes. The bound the
approval sets over the store does not hold. Severity medium. Proposed as
[a delegated install is refused before it writes](../roadmap/1029-a-delegated-install-is-refused-before-it-writes.md),
which also has the archive check refuse `.` and `..` as program names. The file server already
refuses both, so that half is depth and not a hole.

### 2. MINTED: the system manifest check accepts a stem held twice

`SystemManifest::check` takes the first held copy of each listed stem and refuses a held stem only
when the manifest lists no digest for it, so a held list naming one stem twice, once at the listed
digest and once at another, passes. Its doc says "nothing else". Unreachable today, because the one
caller passes `render`'s own de-duplicated output; milestone 976 (an OS update through a slot) names
the device updater as the next reader. Severity low. Proposed as
[the system manifest check refuses a stem held twice](../roadmap/1032-the-system-manifest-check-refuses-a-stem-held-twice.md).

### 3. MINTED: image composition checks one reading of the note and records another

`xtask/src/base_image.rs` checks the asks with `program_asks::checked_asked_grants_of`, which reads
an ELF of any machine, and writes the grant lines with `program_asks::asked_grants`, which parses
through `elf::Elf::parse` and its build-host machine check. Composing an image for another
architecture than the host's checks the asks and records none. It only narrows, and the reading was
not confirmed by a cross-architecture run. Severity low (availability). It touches the grant path,
so it is not fixed in the report's lane. Proposed as
[image composition records the grants it checked](../roadmap/1030-image-composition-records-the-grants-it-checked.md).

### 4. MINTED: the device-tree patcher can panic on a node name

`begin_node` in `crates/device_tree_patch` rounds the end of a node name up to four bytes and copies
through it without checking the rounded end against the structure block or the buffer, which
`prop` does check. A device tree laid out for it panics the loader instead of returning
`Error::Truncated`. The reader confirmed it with a host-side probe outside the tree. The bytes
come from firmware, which is already trusted below the loader, so the worst outcome is a boot that
stops. The crate's fuzz target has not reached the case. Severity low. Proposed as
[the device-tree patcher bounds a node name's padding](../roadmap/1031-the-device-tree-patcher-bounds-a-node-name.md).

### 5. FIXED: the `INTERFACES` note's doc said more than the spawn check does

`crates/interfaces_note` said the progenitor refuses a program built against "an interface version
it does not serve". `serves_declared_abi` compares only the syscall ABI's entry
(`abi::INTERFACE_ID` against `abi::ABI_VERSION`); the other ids a note lists are carried and not
compared. The module doc now says exactly that. An overclaim about a refusal is itself a finding
under the index's step 4, so it is fixed here.

### 6. FIXED: a width the benchmark contract promised was five bytes too generous

`primitive_benchmark_contract::format_row` said 96 bytes holds any row name up to 40 bytes. The
longest row is the name plus 61 bytes (`bench-production: `, two 20-digit numbers, two spaces and a
newline), so 96 holds 35. Today's names are at most 15 bytes and fixed at compile time. The doc now
says 35.

### 7. ACCEPTED: a program with no `INTERFACES` note is spawned unchecked

A missing note declares nothing and passes, by design: the check is one of compatibility, not
authority, so leaving the note out gains a hostile package nothing it could otherwise have had.
Recorded in `crates/interfaces_note`'s `BUGS`.

### 8. ACCEPTED: two hot-swap systems share one log page

`log_put` bounds a write to the log page, not to the caller's system's region of it, and the broker
system's log starts at `hot_swap_contract::BROKER_LOG_BASE` in the same page. A client of the other
system can make its server write that server's version byte into the broker's log. It cannot
choose the byte or leave the page, and both systems are fixtures no release image carries. Recorded
in `crates/hot_swap_server`'s `BUGS` at `log_put`.

### 9. ACCEPTED: `intrusive_queue::Queue` is `Send` whatever its node type is

`unsafe impl<T: Node> Send for Queue<T>` has no `T: Send` bound. The code predates the window and
moved with the rename. The kernel is the crate's one user, the crate is not published, and its
contract makes the caller serialize every access, which its safety comment states where a reader
of the impl meets it. Adding the bound wants the kernel's node type to be `Send` first.

### 10. ACCEPTED: the survey `PROCESS` walk is quadratic under the global table lock

Each call walks every thread slot, and for each thread of the domain every earlier slot, under
`IPC_TABLES`: at most 256 by 256 comparisons. It needs `ENUMERATE`, it is bounded by
`MAXIMUM_THREADS`, and its doc in `kernel/src/sched/region_reaping.rs` states the cost. What its
timing reveals about the thread count system-wide is less than the walk's cursor and global thread
ids already reveal.

## What was deliberately not examined

- `capability_manifest_note` in full, `grant_plan`, `installed_programs::with_entries_granting`, and
  how a badge's approval is resolved (`with_approval`), including who may delegate `--within`.
- `xtask/src/base_image.rs` beyond its approval and manifest paths, and basalt itself.
- `crates/initial_ramdisk_writer`, a host tool whose output the loader already parses, and
  `crates/device_tree_blob`, the reader the patcher checks its output with.
- `components/src/swapper.rs` beyond its diff, and why the base image carries `swapper` when its
  servers are fixtures.
- `system_initializer`'s spawn loop, which chooses the ELF that reaches the process builder, and the
  base digest page's slot in the progenitor's layout.
- Nothing ran on hardware or against an external service. The one probe was a host program built
  outside the tree.

## What wants a lane of its own

The four proposals of findings 1 to 4, finding 1 first.

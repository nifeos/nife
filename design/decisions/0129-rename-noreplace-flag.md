---
status: DECIDED
raised: 2026-08-25
decided: 2026-08-25
ratified_by: calef
---

# 129. Whether `filesystem_proto::fs::RENAME` grows a `NOREPLACE` flag, revisiting §42

calef, 2026-08-25, in conversation: "Build it when we have a customer." [DECISIONS
§42](0042-truthful-filesystem.md) declined `renameat2`-shaped `NOREPLACE`/`EXCHANGE` rename flags,
giving two reasons. They are not portable (native support is Linux-only). And emulating `NOREPLACE`
via separate `link`+`unlink` calls is racy. Milestone 55 (Time Machine)'s own SMB work found that
the second reason does not describe `redoxfs_server`'s actual implementation. It named revisiting
§42 (a filesystem declares what it offers) as calef's call rather than a lane's, "however cheap the
change measures." This entry is that revisit, and §42 itself is amended in place to carry the
correction.

## What changed since §42

`redoxfs_server::rename` already looks up the destination inside the same `fs.tx` transaction that
performs the move. The handler's own doc comment states why no lock is needed at all: "the serve
loop runs one request to completion before it receives the next, so inside this server there is no
concurrent observer at all." §42's racy-emulation case is a POSIX host filesystem reached through
separate `link` and `unlink` system calls, with another writer free to run between them. That case is a
fact about a different kind of backend. It does not describe this one. A `replace: bool` check costs
one branch before `tx.rename_node`. That code already resolves the destination on every call to
decide `EISDIR`/`ENOTDIR`.

**The wire format has room, confirmed precisely.** `crates/filesystem_proto/src/lib.rs`'s
`rename_dst(handle, len) = (handle & 0xffff) << 40 | (len & MAX_LEN)`: the handle occupies bits
55:40, the length bits 39:0. Bits 63:56 are genuinely always zero today, unread anywhere. Adding a
flag there is free: no wire growth, no existing assumption broken.

## What has not changed: real necessity is not established

`notes/smb.md`'s own BUGS section, read closely, separates two things it does not treat the same
way. The extended-attributes/streams gap next to this one is explicitly confirmed as "not on the
Time Machine path" (a Time Machine backup is a sparse bundle: directories and band files, no xattrs,
no resource forks). `ReplaceIfExists` (SMB2/3's own name for this flag,
`SMB2_FILE_RENAME_INFORMATION`) is framed as "the real defect next door," but never receives the
same confirmation tying it to Time Machine's actual write pattern. As written, this reads as a
general SMB2/3 protocol-conformance gap rather than a demonstrated blocker on this milestone's
actual customer path. Any client that asks "do not replace" is silently given a replace instead, the
wrong-direction failure a truthful filesystem should not make.

## The choice, stated plainly

Building it is nearly free (the check itself, plus the flag bit) and closes a real protocol lie. A
client that explicitly asks not to clobber a file is currently clobbered anyway with no error. That
is the exact shape of untruthful-filesystem behavior §42's own title argues against in general, even
though §42's specific reasoning for declining this flag does not survive contact with this backend.

**Not building it costs nothing on Time Machine's own path today**, since nothing in the tree ties
`ReplaceIfExists` to a confirmed Time Machine operation. This project's own ranking function
(AGENTS.md, "The ranking function is the shortest path to a system a customer runs") also weighs
customer-path work above general completeness. Building protocol conformance nothing on the customer
path currently needs is exactly the shape of speculative work this project's own
elegance-over-convenience tenet warns against building ahead of a real requirement.

## The decision

**§42's text is corrected to note its racy-emulation reason does not describe `redoxfs_server`,
without reopening its declined conclusion.** The record should not keep citing a reason that turned
out to be factually inapplicable to this backend. That correction is owed regardless of which way
the feature question goes, and §42 is marked `AMENDED` to carry it in place. The feature itself
stays declined, for the corrected reason (not demonstrated as necessary for this milestone's
customer path, not "would be racy to build") rather than the original one.

Build the flag only when a specific client failure is observed on the actual customer path (a real
macOS Time Machine or general SMB client operation that needs it). This matches this tree's own
"wait for the customer" pattern elsewhere (DECISIONS §105 (`std::thread::spawn` stays declined)'s
`std::thread::spawn`, "we will likely do A when there is such a customer"). Recorded here as a
known, named, cheap-to-close gap rather than a silent one. So the next reader who hits it does not
have to re-derive that it is easy.

## What this does not decide

Whether `EXCHANGE` (an atomic swap, the other `renameat2` flag §42 also declined) gets the same
treatment. Nothing in this milestone's own research found a reason to revisit that half. It is left
as still declined for its original, still-applicable reasons (no portability precedent, no
identified need).

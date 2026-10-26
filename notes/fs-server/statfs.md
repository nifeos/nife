# `STATFS`: how much room the store behind a capability has

An appendix to [notes/fs-server.md](../fs-server.md). It holds op 18, from milestone 54 (a network
file service a Mac can actually mount): the wire, three choices worth disagreeing with, the right it
does not demand, and its honest limits.

## Why it exists

macOS sizes a Time Machine sparsebundle against what the volume reports. The SMB share was reporting
a 64 MiB constant, with a `BUGS` entry explaining that nothing in the stack could ask.

## The wire

`fs::req(fs::STATFS, handle, 0)`, with the second word 0. The reply fills the shared page with
`filesystem_contract::statfs`'s record, and `r0` is its length. The record is three little-endian
`u64`s: the allocation unit in bytes, the volume's size in those units, and how many are free. That
is `READDIR`'s and `LISTXATTR`'s existing shape rather than a new one, and it is forced. A reply word
carries one `i64`, and two 64-bit counts do not fit in one however they are packed.

## Three choices worth disagreeing with rather than skipping past

- The allocation unit is a field, not `filesystem_contract::PAGE`. They are the same number today
  (4096), and they are not the same *thing*: one is the filesystem's granularity, the other is a
  page. A client that assumed they were equal would mis-size a volume the day a filesystem with
  64 KiB records is served through this contract. The IPC transfer unit stopped being either of them
  at milestone 138 (close the read gap: a 4 KiB request must stop moving 128 KiB) step 3. It is now
  `filesystem_contract::fs::TRANSFER_MAX`, which is the same distinction arriving a second time and
  proving the field was worth having.
- The record's length is its version. A later field extends the record and raises `LEN`. A client
  written against this version reads its prefix correctly, because it checks `r0 >= LEN` for the
  fields it knows, which is how `statfs::decode` is written. There is deliberately no version word,
  because the length already is one, and a second one is a thing to keep in sync.
- No inode count. POSIX `statfs` carries `f_files`/`f_ffree`, and SMB's volume classes carry neither,
  so nothing above this contract has asked. Adding them extends the record.

## It demands no right

It takes any handle this server minted, file or directory. The handle is the *qualification* rather
than the subject. Holding one is what entitles a caller to ask, and the answer is about the one
image behind all of them, so `EBADF` for a handle nobody minted is the whole check. The server
validates the handle and then does not use it, which looks odd in the code and is the point.
Demanding `dir::READ` would have left a write-only grant unable to answer the one question it has:
whether its next write fits. So the verb table's row is `needs_all: 0`, beside `FSTAT` and `CLOSE`.
`verb::file_grant::POLICY` forwards it. `STATFS` is the one directory-shaped question that means
something through a file capability.

## Honest limits, both recorded at the verb

It answers about the whole image and never about a subtree. So a narrow subtree capability's holder
learns the volume's free space. There are no quotas here, so there is no smaller number that would be
true. Reporting a subtree's usage as a "size" would be the silent degradation DECISIONS §42 (a
filesystem declares what it offers and must be truthful) forbids. And free space is a forecast. The
store's own `ENOSPC` at write time is the authority. That is what `statfs` is everywhere.

## Where the numbers come from

`FileSystem::header.size()` gives the volume's bytes, and `FileSystem::allocator().free()` the free
level-0 blocks, both read outside `fs.tx`. Opening a transaction to answer a question changes nothing,
and would put a commit in the path of a read-only verb. RedoxFS's own `clone` computes free space from
exactly this pair. Nothing races it, because this server runs one request to completion before it
receives the next. That is the same property `RENAME`'s concurrency atomicity rests on.

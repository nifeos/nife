# The FS test client: why its roles are shaped as they are

Name: provisional, minted 2026-10-24 (UTC) by lane `comments/929-fixtures-01`, named for its
content. calef names files.

An appendix to [notes/fs-server.md](../fs-server.md). `fixtures/src/fs_test_client/` is one program
with many roles, chosen by its first argument. Each role's doc comment says what it holds and what it
reports. This page holds the reasons the roles are built the way they are, which apply across them.

## A witness reports a bitmap, not a pass

A client that traps tells the waiting test only that something went wrong. The interesting failure
is which claim broke, so the attackers and witnesses set one bit per attempt or claim and send the
word home. The kernel test asserts the exact expected set. In the extended-attribute witness, a
layer that dropped type codes, a store keyed on a path instead of a node, and a purge that never
ran each fail a different bit.

## An attacker is its own negative control

The file attacker runs twice. Against a read-only grant every bit must be clear. Against a
read/write grant of the same shape, `WROTE` and `TRUNCATED` must be set and everything else clear.
Without the second run the first proves little: a caretaker that refused every request would pass
it, and so would a grant that reached nothing at all. When the write is supposed to succeed, the
attacker reads the bytes back, because "the server accepted my write" and "my write landed" are
different claims.

The directory attacker is told nothing about its own grant. It attempts every verb, and the
specification lives in the test rather than in the program being tested. Its three runs are each
other's controls: a caretaker that refused everything fails the wide run, and one that allowed
everything fails the narrow one.

The two-directory witness is likewise told only the slot ordering. It learns which grant is which
through the labels it was compiled to use. Its `/a/../b` refusal comes from `TwoRoots::resolve`,
the same way a single grant's `..` at its own root is refused. Asking each caretaker for the name
only the other holds witnesses, with two live caretakers, the finding
[notes/dir-capability.md](../dir-capability.md) argues: the endpoint is the boundary.

## Every attempt targets something that exists

A refusal of a name that does not exist proves nothing. The neighbor file, `motd` in the parent
and `other/secret` in a sibling are all on the image, one directory entry from the caretaker, which
could open any of them on any request it liked. Milestone 33's attacker was handed a real
neighboring client's address for the same reason. The write probe targets the one file the attacker
may read, since refusing a write to a file it cannot even name would prove nothing.

The name-set witness asks its naming question with `READ`, where `rm`'s witness already asks it with
`REMOVE`. Teaching `fs_nameset_caretaker` the attribute verbs closed an attribute gap, and opening a
naming one in the same change would have been a poor trade, so this checks it did not.

## The image is shared, so each role leaves it as the gate expects

The suite runs its tests in alphabetical order on one image, and the gate's post-run host check
pins some bytes from outside the guest.

- The writable file attacker is granted `scratch`, never `motd`, which two other tests compare
  byte for byte. Its last write restores `scratch`'s pattern, so a truncated file never shows up
  three steps later as an apparent filesystem bug.
- Its read-only run compares against `motd`, which nothing writes. Its writable run cannot compare
  `scratch`, because it runs before the two clients that write a known pattern there. It checks that
  the file reads back the size `FSTAT` reports, and that its own writes read back.
- The directory attacker never writes `sub/inner` or anything outside `sub`, so the host check can
  catch an escape this program failed to report.
- The attribute witnesses remove what they attach and unlink what they create. The image then
  differs only by the attribute store directory, which the host tool is expected to show.

## The crash driver blocks forever, on purpose

The crash driver reports after its first acknowledged write, then issues a second write into a
server told to die inside it. Blocking IPC has no "your server is gone" reply, so the driver never
returns. That is the open item DECISIONS §27 records. The fault endpoint of §26, wired into a
supervision tree, is the mechanism that would turn it into a message. The blocked thread costs no
CPU, and the kernel test waits on this client for nothing after the first report.

## The shared-frame victim forces the ordering

The property under test is whether a second holder of a frame can change the bytes the server
resolves for this client's call. Whether a race happens to fire is not the question. So the victim
and the attacker use a two-message handshake: the victim stages its name, the attacker rewrites the
page, and only then does the victim call. The server has no way to tell whose bytes it read. The
victim mints its own badged endpoint, which exercises the mint syscall end to end, while the
attacker gets a pre-badged one from the wiring, so the test covers both shapes.

## The throughput role

Each phase moves a constant number of bytes rather than a constant number of transfers. A run at a
one-page transfer unit and a run at sixteen then move the same file, and their MiB/s figures are a
before and an after. Random offsets are page-aligned and inside the file, so a random read never
hits EOF and a random write never extends the file: the random phases measure placement, not
growth. A read loop never copies the bytes out, because a client that shares a page with its server
can use them where they land, one copy fewer than a Unix `read(2)`.
[notes/benchmarks/filesystem-throughput.md](../benchmarks/filesystem-throughput.md) has the numbers
and what a write here promises.

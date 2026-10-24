---
status: AMENDED
raised: 2026-07-29
decided: 2026-07-29
ratified_by: calef
---

# 27. The filesystem service: a capability-shaped contract over a component we did not write (milestone 32 phase 2)

RedoxFS runs confined as a userspace FS-server component, and its interface is **capability-shaped
from birth**. Three processes, wired by the kernel and named by nobody else. A **block server** (a
role of the virtio driver) serves blocks over blk IPC with the DMA confinement unchanged. An FS
server (`fs_server/`, its own workspace because it links the vendored engine) runs the no_std
RedoxFS core behind a `Disk` trait over blk IPC. It allocates from its own untyped budget through
§22's `GlobalAlloc`. A **client** holds only a directory capability. The contract
and both wire protocols live in `crates/fs_proto`, host-tested, the way the terminal contract lives
in `line_editor::proto`. Full design in notes/fs-server.md.

The contract's rules, which milestone 31 will grant against. The endpoint a client holds IS the
directory capability: it is bound, in the server, to one directory node, and every name in an `OPEN`
is resolved under that directory. There is no absolute path, no `..`, no global namespace; a client
without the endpoint can open nothing, and the refusal is "no such capability", not a permission
check. A handle is a server-minted token, validated against the session's table in one place, so
forging one is meaningless. Open-by-path exists only inside the server. None of this adds a syscall.
The kernel routes these words the way it routes any IPC (§10 (process model), §12 (Call/Reply IPC))
and never reads an opcode. So adding a method is a change to `fs_proto` and the note, not to the
surface (the §16 (object revocation) discipline).

The error boundary is mapped exactly once. RedoxFS's error type (`syscall::error::Error`) rides
unmapped through the sans-IO core and the `Disk` impl; the serve loop is the single site that turns
it into the wire's negated errno (`fs_proto::reply_err`). There is no ABI type below the boundary to
leak, which is what makes the rule enforceable rather than aspirational.

The block server moves a whole block per request, and waits on the interrupt. RedoxFS scans a
256-entry header ring at mount, so an open is hundreds of reads. The block server moves a whole
4096-byte block per virtio request (its DMA region's second page IS the FS server's block page, so
the device DMAs straight in, no copy). That is what keeps the mount's request count in the low
hundreds and affordable. It then WAITs on the device's completion interrupt, the milestone-9
driver discipline, and lets `used.idx` decide when a wakeup is really its own.

With the WAIT path the fs_server test passes on both ISAs at the 4-core SMP boot, all of the mount's
interrupt-driven completions landing well inside the 60 s watchdog. QEMU completes virtio-blk
synchronously inside `NOTIFY` (notes/dma.md), so the interrupt is already pending when the server
WAITs, and the pending-signal count of milestone 9 (an interrupt becomes a message) returns that
WAIT at once instead of blocking on an event already over.

The runners also order the two mmio disks with care, because QEMU assigns virtio-mmio slots in
reverse command-line order and the kernel enumerates by ascending slot.

Creation stays host-side, always. The std-gated core APIs are exactly creation (uuid, getrandom);
the server only ever opens an image, so entropy never becomes a userspace dependency. Test images are
made by `tools/redoxfs_host` with the same pinned engine (roadmap §32 item 4).

Proven. The read path is proven end to end on both ISAs (the §19 gate): a host-made image,
mounted by the confined FS server over blk IPC, its `motd` opened through a granted directory
capability and read back byte for byte. There is also a host-tool consistency check after the run.
The sans-IO core is host-tested for read AND write (`fs_server` lib), so the filesystem logic is
proven both ways independently of any device.

**Amendment (2026-07-29). There was never a filesystem bug. The write always succeeded.**

[`the-2026-07-29-amendment.md`](0027-filesystem-service/the-2026-07-29-amendment.md) holds the
account of the missing `TRUNCATE`, its lessons, the hypotheses measured and the errno plumbing.

Driven through `std::fs` (§22 (Rust `std` on the native ABI)'s phase-two amendment), the write
completes on both ISAs. It reads back byte for byte when the host tool reopens the image afterwards
with the pinned engine, which is the half a cache cannot fake. That reopen is part of the gate.

## Amendment (milestone 31 phase 2, 2026-07-30): `CREATE` and `TRUNCATE` exist, and a per-file grant is a caretaker process

The two verbs are built. `CREATE` (opcode 6) resolves a name under the bound directory and makes
it, answering `EEXIST` if it is already there and modifying nothing. Create is create, not
create-or-open, because a caller that wants either has to say which it got. The alternative is
what makes a partly-working write read as a working one. `TRUNCATE` (opcode 7) sets a file's size
in both directions, growing with zeroes and shrinking by discarding. The new size is in the second
word rather than the length field. The length field is clamped to one page, which would silently
cap a truncate at 4096 bytes. Both are host-tested in the sans-IO core and bound in `std::fs`, so
`File::create` and `std::fs::write` work rather than returning `Unsupported` (§22's amendment).

One gap closed while adding them, and it was ours rather than RedoxFS's. RedoxFS's `check_name`
rejects `:`, over-long names, and duplicates; `/`, `.` and `..` pass straight through. Nothing walked
paths, so nothing escaped, which made the "one component, no `..`" rule true by the absence of a
walker rather than by a check. `CREATE` turned that from a latent oddity into something a client
could *write*: `create_file("../escape")` made an entry literally named that. `check_component` now
enforces the rule at our boundary, deliberately there and not patched into the vendored engine,
because it is a rule of this contract and not a bug in a component whose callers may name entries
whatever they like.

A per-file grant is a separate process, and that is the decision. Milestone 31's `run wc
report.txt` must hand over one file; the unit of authority here is a directory. The narrowing is
`components/src/fs_file_caretaker.rs`, a caretaker (Mark Miller's term): it holds the directory
capability, opens the granted name once at startup, and serves the same `fs_proto::fs` contract on
its own endpoint with a namespace of exactly one name. Three rules, each phrased as a fact about
what the holder has rather than as a permission refusal, because there is no policy here to consult:

- `OPEN` of any other name is `ENOENT`. In this scope there is no such name. The holder cannot
  enumerate and cannot learn what else the directory holds.
- `CREATE` is `ENOTDIR`. A file capability is not a directory, so "make a name in it" is not a
  request that means anything.
- `WRITE` and `TRUNCATE` are `EROFS` without the write direction. `EACCES` was rejected on purpose:
  it implies a policy that could have said yes.

**Why a process and not a check inside the FS server.** The server receives on one endpoint. Serving
a second, narrower one would need a receive over a *set* of endpoints, which this kernel does not
offer. Adding it means giving endpoint capabilities a badge (seL4's answer). That is a design
fork and is recorded here as the alternative rather than taken. The caretaker needs nothing new: it
is an ordinary FS client above and an ordinary FS server below. And it is the stronger form of the
claim. The confined program holds an endpoint to the caretaker and nothing that names the FS server,
so "it cannot reach a second file" is a property of its cspace rather than of a branch it is trusted
to take. The boundary is an address space, which is the same reason §31's checker lives outside the
component it checks.

The grant costs no memory. The name and direction ride in the caretaker's three `START` argument
words (`fs_proto::grant`, 16 bytes of name), and the one frame is shared by all three processes.
That is sound because every request on both hops is a blocking `CALL`, so the client is parked
inside its own call for the whole time the caretaker is using the page.

Proven on both ISAs by an attacker, twice, and the second run is what makes the first mean
anything. The attacker reports a bitmap of what got through rather than a pass. Read-only: every
bit clear, against a neighboring file that really exists and that the caretaker really could open.
Read/write, same shape: the two write bits set and everything else clear. A caretaker that
refused every request passes the first test and fails the second. Each accepted write is read
straight back, because "the server accepted my write" and "my write landed" are different claims.

The interactive shell refuses a named file, and that refusal is true rather than pending. A bare
token in a file position designates the file. The boot that starts the shell wires no FS service, so the shell holds no directory to narrow, and `caps`
says so in those words. `grant_plan` carries the whole vocabulary (a `FileSpec` in the manifest, a
`FileGrant` in the endowment, refusals both ways) and the decision is a function of what the shell
*holds*, not of the calendar. Wiring an FS service into the interactive boot is the remaining step, and
nothing in the suite gates that boot, which is why it is recorded here instead of built.

Also settled, by measurement: the FS server's stack. RedoxFS recurses in 8 KiB frames, and a server
that runs out dies mid-request while its client blocks forever on a `CALL` nobody will answer. The
size is measured rather than chosen. The kernel poisons every FS-server stack page and
`fs_service::fs_stack_used` reports the deepest word that is no longer poison. The figure is a
maximum over every FS server a boot starts, so the mount that recovers a crashed disk is measured
too. Milestone 37 (prove RedoxFS's crash consistency) measures 127,408 bytes on aarch64 and 127,536
on riscv64, of a 397,312-byte grant, and a test on both ISAs prints it every run and fails under a
quarter left. notes/fs-server.md carries the incident, including the two instruments it blinded and
why a ceiling failure reports the ceiling and not the cost.

The remaining honest gap: a client of a dead server blocks forever. §26's fault endpoint is the
mechanism that would turn that into a message a supervisor can act on, and wiring the FS service into
a supervision tree belongs to milestone 23.

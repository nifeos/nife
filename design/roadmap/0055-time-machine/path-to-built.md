# Milestone 55 appendix: the path to BUILT, as scoped 2026-08-22

An appendix to [milestone 55 (Time Machine: SMB3 with Apple's extensions, and
mDNS)](../0055-time-machine.md), moved out of that page on 2026-10-12 (UTC) under [§212 (a prose
budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main page is complete
without it. This is the milestone's own scoping pass of 2026-08-22, and a second pass the same day
that found the `ReplaceIfExists` question. The premise was retired eight days later, on 2026-08-30,
so none of the steps below will run.

## The scoping pass

This was the milestone's own scoping lane, run against the six-questions framework AGENTS.md asks a
fork to answer before it reaches calef. The finding is that there was no fork left to bring him.

- Both irreversible decisions the block ever carried were already decided: the SMB3 subset
  (2026-08-15), and the metadata stream-versus-sidecar split (§99 (where Apple's metadata lands:
  stream or sidecar), 2026-08-18).
- The mDNS wire protocol was written rather than taken as a dependency, and is built.
- What remained was one gate: contact with a real Mac.

That gate is not code waiting to be written. It is hardware waiting to exist, and it has a
dependency chain worth making explicit, because nothing in the block named it.

### Complete, and what closed each piece

1. The scoping decision itself, calef, 2026-08-15: the SMB3 subset Time Machine needs, not a
   general server. Gate: NONE.
2. Discovery, pull request #246, 2026-08-16. `crates/mdns_proto`, `crates/mdns_config`, and
   `components/src/mdns_responder.rs` answer `_smb._tcp`, `_adisk._tcp` and `_device-info._tcp`
   against the measured reference, both ISAs. Written, not vendored: it is a from-scratch parser and
   responder over an existing `smoltcp` feature flag, not a new dependency. That is §46 (thin
   primitives or whole subsystems; we write everything in between) and its "write it, it's on the
   verification path", read correctly the first time. See notes/mdns.md.
3. The `AAPL` create context and the Time Machine flag, pull request #292, 2026-08-17: the bit that
   makes macOS accept the share at all.
4. Identity, inherited from milestone 54 (a network file service a Mac can actually mount), pull
   request #274, 2026-08-17: NTLMv2 while the adapter holds no key.
5. The Apple-metadata fork, answered rather than built, §99, 2026-08-18. Option 1: say nothing, let
   the client write `._` sidecars, zero new code. The workload that actually wants named streams was
   split out as its own milestone, milestone 137 (the share as a Mac file server). The load-bearing
   finding survives repeating: a Time Machine backup is a sparse bundle, directories and band files,
   and never touches this surface at all. Milestone 55 carried it by proximity, not by requirement.
6. Durability, pull request #311, 2026-08-18: `FULL_SYNC` became a claim backed by a real device
   `VIRTIO_BLK_T_FLUSH`, witnessed by a flush count that only a real round trip can advance.
7. Throughput, 2026-08-19: the 64 KiB transfer reaches the mount, measured 4.8x write and 2.4x read
   through the host SMB prober. `socket_proto::DATA_MAX` became the binding ceiling. It already
   carries its own promotion trigger in notes/smb.md's BUGS, and step C below is what fires it.

[what-landed.md](what-landed.md) has the detail on items 3, 4, 6 and 7.

### Remaining: one gate, transitively blocked on a milestone that is not this one

Nothing left here is a design decision. The remaining work is real-Mac contact, and three things
stand between here and it, in dependency order. The block's "Nothing here has met a Mac" paragraph
(now in [what-landed.md](what-landed.md)) names the QEMU limitation. It does not name the harder
blocker underneath, which this pass found by reading milestone 53 (the board's own peripherals:
network and storage on real silicon) rather than assuming a board and a cable would do.

#### Step A. A real NIC driver, on some board

Every network driver in the tree is `virtio-net`, which only exists under QEMU; real silicon has
none. Milestone 53 is at Status PARTIAL, Gate HARDWARE, and its own text is direct: "this is where
virtio stops carrying us."

What it still owes:

- the JH7110's Synopsys DesignWare GMAC driver on the VisionFive 2 (the riscv64 board on the desk
  since 2026-08-14);
- the PLDA XpressRICH root-complex work that carries the NVMe driver to the real M.2 slot, which is
  milestone 163 (the JH7110's PCIe root complex), NOT-STARTED.

aarch64 has no board at all yet; notes/aarch64-board-survey.md is still choosing one. So the realistic
near-term path to a real Mac is through riscv64, not aarch64, purely because that is where hardware
already exists. That is a sequencing observation, not a design fork. It costs nothing to reverse if
an aarch64 board arrives first, and it does not change what either driver has to do.

#### Step B. Multicast on a real segment

slirp cannot carry multicast, so nothing about discovery has been proven end to end. The closest this
tree has come is an injected query that accidentally escaped QEMU's NAT and came back from the real
router. That is evidence the segment exists, not evidence the responder works on it. Once Step A
gives a board a real interface, this step is `dns-sd -B _adisk._tcp` from a real Mac against the
running responder, on the real family segment.

#### Step C. A real backup, and a real power cut

This project's own standard for durability claims is "tested by actually cutting power" (milestone
53's storage half). A QEMU flush count cannot stand in for that. It needs milestone 53's NVMe path on
real silicon, not just the network half. So a full validation of this milestone's `FULL_SYNC` claim
waits on more of 53 than discovery alone does.

A narrower first experiment (mount, small write, clean unmount) needs only Step A, and could run
before persistent storage lands. Separating those two experiments is worth doing explicitly, rather
than waiting for all of 53 to call any of it started.

### Adjacent, not blocking: milestone 131

The share this milestone builds still admits guests. A family backup target that admits guests is
not one calef would trust with the only copy. Milestone 131 (configure a service instead of compiling it, and type its secret at a console) is what closes that.

It is not a prerequisite for the protocol-correctness gate above. The first real Mac contact can and
should happen against the guest share this tree already has, the same way every QEMU gate does today.
It is a prerequisite for the day this stops being a lab experiment and becomes the family's actual
backup target. That is a distinct milestone, which this block should stop trying to also be.

### What this scoping pass did not find

It found no irreversible fork inside milestone 55 itself that needs calef's decision before Steps A
through C can start. The lane's brief asked about three questions: the mDNS library, vendor versus
write, and Apple metadata. All three were already answered in the block, most of them months before
this pass began.

The one sequencing call made here (riscv64 before aarch64, because the hardware already exists) is
offered as a recommendation rather than a question. That follows AGENTS.md's own rule that a
reversible fork gets a recommendation, not a lane.

## One finding this pass missed: `ReplaceIfExists`

A second scoping pass, 2026-08-22. The block's BUGS-adjacent record in notes/smb.md read as an open,
software-buildable item: `ReplaceIfExists = 0` is ignored on rename. The note said "the fix is a
`NOREPLACE` question in `fs_proto`".

Checking it against §42 (design/decisions/0042-truthful-filesystem.md) found that §42 already decided
not to offer `renameat2`'s `NOREPLACE`. Its stated ground was that emulating it with link-then-unlink
is racy and backend-specific. Read literally, notes/smb.md's own suggestion contradicted a decided
architecture rule. That is worth recording as its own small finding: a note can go stale exactly the
way a roadmap block can, and this one did.

### §42's premise does not hold for this backend

The premise behind §42's refusal was checked against this backend rather than assumed. It does not
hold here. `fs_server::rename` (fs_server/src/lib.rs) already resolves the destination inside the same
`fs.tx` that performs the move. Its own doc comment gives the reason no lock is needed: "the serve
loop runs one request to completion before it receives the next, so inside this server there is no
concurrent observer at all."

§42's racy case is a POSIX host filesystem reached through separate `link` and `unlink` syscalls,
with another writer free to run between them. This server is neither. A `replace: bool` read at that
existing check point is a few lines, not a redesign. The wire has room already: `fs::rename_dst`'s
second word packs a 16-bit handle and a 40-bit length into 64 bits, leaving bits 63:56 unclaimed for
a flag. So the change costs no growth in an already-shipped word.

### Still not built, decided rather than deferred

This is a wire-format change on `fs_proto::fs::RENAME`, a verb every SMB client, the std PAL and
`fs_server` already agree on. It also revisited a section calef decided (§42). [DECISIONS §129
(whether `RENAME` grows a `NOREPLACE` flag)](../../decisions/0129-rename-noreplace-flag.md) priced
the change: a few lines, and free wire room.

calef's own call was "build it when we have a customer". §42 is amended to correct its racy-emulation
reason (which does not describe `redoxfs_server` specifically), but stays declined on the feature
itself, since nothing ties `ReplaceIfExists` to a confirmed Time Machine operation today. Nothing
else in this milestone depends on the answer either way. So this stayed a small, isolated item
rather than something holding up Steps A through C.

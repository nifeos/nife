---
status: DECIDED
raised: 2026-10-06
decided: 2026-10-06
ratified_by: calef
---

# 251. Restarting the machine is a kernel object the progenitor hands out

calef, 2026-10-06 (UTC), on PR #1766: *"Approved, go with the new reboot object."* That approves
the recommendation of the proposal milestone 805 (`reboot` at the prompt) was promoted from, on all
three of its questions. *(Section number provisional until the merge queue lands it.)* Recorded by
the maintainer session the same day. Nothing is built yet; milestone 805 builds it.

Every name here is provisional: the object, its method, the manifest field and the program
`reboot`. Naming them is a separate ratification under design/naming.md. This section calls them
the reboot object, `REBOOT` and the `reboot` field for want of anything better.

## The ruling

1. A new kernel object kind carries the authority to restart the machine. It has no payload: the
   kernel stores nothing in it, and it names no device. It gets one `abi::objtype` number and one
   method. This is a new method within the capability model of §10 (process model:
   capability-based, microkernel) and §16 (object revocation). It adds no syscall number.
2. The kernel creates exactly one at boot and grants it to the progenitor. The progenitor endows it
   only to a program whose manifest declares the `reboot` field, the way `entropy` is endowed
   (`crates/capability_manifest_note`). A program that does not declare it holds none.
3. The `reboot` program syncs the writable filesystem with `filesystem_protocol::fs::SYNC` and
   waits for the reply, then invokes `REBOOT`. An ordered shutdown of services is deferred to a
   later milestone, which will add its step before the `SYNC`.
4. Power-off is excluded. It is a separate program and a separate object kind, so holding the
   reboot object confers no power to switch the machine off. Neither is in milestone 805.

## The method

`REBOOT` takes no arguments. Holding the capability is the whole authority; no rights bit beyond
what invoking any object needs.

- On success it does not return. The kernel calls `arch::reboot`: PSCI `SYSTEM_RESET` on aarch64,
  SBI SRST reset type 1 on riscv64, and the FADT reset register, then `0xCF9`, then the 8042 on
  x86_64. Those routes exist today behind `--features reboot_soak_test`; milestone 805 gives them
  this second caller and leaves the soak its own feature.
- If the firmware refuses and `arch::reboot` returns, the method returns an error to the caller
  carrying the refusal. The program prints it and exits non-zero.
- A firmware that accepts the call and hangs cannot be told apart from a slow reset from inside
  the machine. radon did exactly this on 2026-09-04, and milestone 592 (radon's cold reboot dies
  in OpenSBI's PMIC write) holds the fix.
- The kernel does not sync anything. It knows no filesystem, and a microkernel that did would be
  the bug. The `SYNC` in clause 3 is the program's step. A holder that invokes `REBOOT` without it
  loses writes the device had not flushed. That is a foot gun, recorded here and in the program's
  `BUGS`; the mitigation is that only one program is endowed.
- Devices are not quiesced first. A DMA transfer in flight is cut off. After `SYNC` the block
  servers are idle, and nothing else writes to persistent storage.

## Why SYNC and no more

RedoxFS is the writable filesystem, and notes/fs-server.md measures it prefix-consistent at every
power-cut point. Milestone 37 (prove RedoxFS's crash consistency) showed a cut loses only a suffix
of the workload. `SYNC` reaches a real device flush in both the virtio and NVMe block servers, which
makes "nothing acknowledged is lost" true on a device with a volatile write cache. No service yet
holds state a stop message would save: the system log of milestone 613 (a system log service: the
in-memory half) lives in memory only.

## What was refused

| Option | Why it lost |
|---|---|
| B. A method on an existing object | No existing object names the machine, and a "may reset" bit on the root `MemoryRegion` would make every memory grant a question about power. |
| C. A user-mode driver holding device authority | PSCI and SBI calls cannot be made from user mode, so it works on x86_64 alone and breaks §19 (architectural parity). |
| D. A new syscall number | Ambient authority with no identity for the kernel to check, and a new number is its own fork. |
| E. An op on milestone 391 (kernel introspection over an endpoint)'s endpoint | §149 (may the kernel answer on an endpoint) left that open, and it would put a harmless read and a destructive act behind one capability. |
| Power-off in the same milestone, or a `--off` flag | One program, one thing (calef, 2026-09-26), and on radon a powered-off board needs a person at smart plug 2. |

At equal cost the answer is the same. The reboot object is the only option that is both a
capability and possible on all three architectures. Fuchsia's `zx_system_powerctl`, which takes a
resource handle, is the same shape; Linux's `reboot(2)` behind `CAP_SYS_BOOT` is D with an identity
check. Both are recalled, not re-read.

## Amended 2026-10-06 (UTC): what the build found, under exit criterion 7

Milestone 805's lane built the object and found three places where this section's wording and the
built thing differ. Milestone 805's block left the first one to the building lane ("How `reboot`
reaches `SYNC` is the building lane's call"); the other two are corrections to wording. calef ruled
all three items on 2026-10-06 (UTC), below. He also ruled the sync names on #1783: "Approve sync
for the file-server request and its capability, and flush for the device cache only." So the
request and its capability are `fs::BIND_SYNC`, `SyncOnly` and `Manifest::sync`, and "flush"
means the block device's own cache command. His other rulings the same day: never abbreviate capability as `cap`, so the
constructor is `reboot_capability`; and `prepare_reset_route`.

1. **`reboot` syncs for itself, through a sync-only capability** (clause 3). For an ordinary
   client, `fs::SYNC` needs a handle carrying `dir::WRITE`, which is also the right to open and
   truncate files by name. So the file server gains one protocol message,
   `filesystem_protocol::fs::BIND_SYNC` (66, provisional), beside `BIND` and `UNBIND` (milestone
   606 (a directory walk costs what it does on Linux), ruling D). It binds a client badge
   sync-only: the server answers `SYNC` on that badge with the block server's flush count and
   refuses every other verb with `EPERM`, before it reads a handle. The badge names no directory
   and reaches no file. `BIND`'s caller rule holds (only an unbound badge may bind), and `UNBIND`
   revokes it like any grant. It is a server message on an existing endpoint, inside the badge
   model of §230 (badged endpoint capabilities), and adds no syscall and no kernel
   method.
   - `reboot` declares `grant_plan::Manifest::sync` beside `reboot`. For that job the spawn
     service takes a client window, as for a directory grant, binds its badge sync-only, places
     the badged endpoint `WRITE`-only at slot 14 and deletes its own copy. The window goes back
     when the job is reaped, so the progenitor holds nothing extra for the life of the boot.
   - The program sends `SYNC`, checks the answer, prints it, and invokes `REBOOT`. A failed sync
     (any errno but `EOPNOTSUPP`) makes it refuse. A boot with no writable filesystem leaves the
     slot empty and there is nothing to sync. A boot whose file server cannot bind one refuses the
     spawn, rather than start `reboot` unable to sync.
   - No image can declare `sync`, and a manifest note cannot spell it.
   - The cost, recorded in the program's `BUGS`: a write another job makes between the `SYNC`
     reply and the reset is not covered.

   This replaces this item's first version, in which the progenitor sent `SYNC` at the spawn and
   passed the answer in a start register. calef sent that back on #1783 (2026-10-06 UTC): "My
   concern is progenitor is turning into a god process."

   Ruled A by calef on #1783, 2026-10-06 (UTC), answering "Yes" to: "Approve A
   (fs::BIND_SYNC sync-only binding), with B through F recorded and D filed as a follow-on?"

   Alternatives considered:

   - A, built: the sync-only binding above. Its gap is a write between the `SYNC` reply and
     the reset.
   - B, the progenitor sends `SYNC`: rejected by calef, the god-process concern.
   - C, give `reboot` an ordinary `dir::WRITE` handle: far too much authority for a sync.
   - D, orderly shutdown: the reboot path tells every stateful server to stop taking writes,
     sync and acknowledge before the reset. It is the only option that closes the window. It
      needs a service manager to own shutdown order, so it is neither `reboot`'s job nor the
      progenitor's. Milestone 853 (orderly shutdown closes the sync window) owns it.
   - E, the kernel calls registered pre-reset endpoints: puts shutdown policy in the kernel,
     against the narrow syscall surface.
   - F, a crash-consistent filesystem, so no sync is needed: still loses recent writes, and
     RedoxFS makes no such promise today.

   Prior art, recalled, not re-read:

   - Linux `reboot(2)` does not sync. `reboot(8)` and systemd call `sync(2)` first, and `sync(2)`
     needs no privilege, which treats a sync as harmless authority and supports A.
   - Capsicum has a separate `CAP_FSYNC` descriptor right: A at the descriptor level.
   - E and KeyKOS facets, and seL4 and CAmkES badge attenuation, are the A pattern.
   - Fuchsia's `component_manager` stops components in dependency order, and `fshost` flushes on
     `fuchsia.process.lifecycle` `Stop` before power control resets: D.
   - systemd stops units in reverse dependency order, remounts read-only, syncs, then resets: D.
2. **No `abi::objtype` number** (clause 1). `objtype` lists what `MemoryRegion::RETYPE_OBJ` can make
   out of memory. Nothing makes a reboot object, and a number there would read as a way to. It has a
   method module, `abi::reboot`, like `Irq`, the other kernel-minted object. Ruled A by calef on
   #1783, 2026-10-06 (UTC): "Yes", answering "Approve item 2 as written (A)?"

   Alternatives considered:

   - A, built: no `objtype` number and its own `abi::reboot` method module, like `Irq` and
     `MemoryRegion`, which have none either.
   - B, a number `RETYPE_OBJ` refuses: `objtype` would then mean both "can be made" and "exists".
   - C, a retypable number: anyone holding memory could mint reboot authority.
   - D, split `objtype` into a retype list and a full object-kind list: worth it only once a
     capability type query exists, and nife has none today.

   Prior art, recalled, not re-read:

   - seL4's `seL4_ObjectType` lists only retypable objects; `IRQControl`, `ASIDControl` and the
     domain capability are internal capability tags only. That is A.
   - Zircon's `zx_obj_type_t` lists every kind, kernel-minted resources included, because
     `zx_object_get_info` reports a handle's type, and creation is a syscall per kind. That is D.
3. **A refusal returns its reason to the caller** ("The method"). Ruled by calef on #1783,
   2026-10-06 (UTC): "Rule item 3 that way", answering "Rule item 3 that way (return the reason to
   the caller), and approve item 2 as written?"
   - `arch::reboot` returns a portable reason, as an `abi::Error`, on all three architectures.
     The four are no mechanism (an aarch64 tree with no usable `/psci`), not supported (PSCI or SBI
     `NOT_SUPPORTED`), denied (PSCI or SBI `DENIED`), or still running (every `x86_64` route
     tried, or any other PSCI or SBI code). The firmware's raw code is still printed on the console.
   - `REBOOT` answers that reason as its own `abi::Error` (`NoResetMechanism`, `ResetNotSupported`,
     `ResetDenied`, `ResetDidNotHappen`, -12 to -15). calef ratified those four and dropped a
     separate reason type under §113 (kernel object plain names), one name per concept: "Yes". `reboot` prints which on
     its second stream, and it does not exit non-zero, because no program here reports an exit
     status.
   - No QEMU machine this tree boots can be made to refuse, so `abi`'s
     `a_reset_refusal_maps_to_its_reason_and_survives_the_wire` pins the mapping on the host.
   - Prior art, recalled, not re-read: Linux `reboot(2)` logs a firmware failure and halts, so the
     caller never learns it; Fuchsia's `zx_system_powerctl` returns a status to its caller.

Two smaller facts the section did not state. The kernel grants the progenitor the object with
`WRITE | GRANT`, at slot 31 (it never invokes it, but delegation only narrows), and the progenitor places it with `WRITE` alone at slot 13; the method
checks no right, as "The method" says. And a manifest note cannot spell `reboot`
(`manifest_note::encode` refuses it at compile time), because `grant_plan::image_can_carry` keeps the
object off every image, so the note's wire format is unchanged.

## Open

- Who at the prompt may reboot is not decided. Any session the progenitor endows can. Restricting
  it to some logins belongs to whichever milestone decides what a login grants.
- The new dispatch arm is cold, and `script/fastpath-footprint` will price it when it exists. No
  number is claimed before then.

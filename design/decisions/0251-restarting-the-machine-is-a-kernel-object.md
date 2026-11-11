---
status: AMENDED
raised: 2026-10-06
decided: 2026-10-06
ratified_by: calef
---

# 251. Restarting the machine is a kernel object the progenitor hands out

calef approved the reboot object on PR #1766 on 2026-10-06 (UTC), taking the recommendation of the
proposal milestone 805 (`reboot` at the prompt) was promoted from on all three of its questions.
The same day, on #1783, calef ruled the three corrections milestone 805's build found (how `reboot`
syncs, no `objtype` number, a refusal returns its reason). On 2026-11-02 (UTC) calef ruled that the
object and the sync-only capability belong to whatever bytes the owner grants them, not to a
program name (clause 2). *(Section number provisional until the merge queue lands it.)* Milestone
805 built the object, the sync-only capability and the program.

The object, its method, the `reboot` field and the program `reboot` carry provisional names under
design/naming.md. The sync names are ratified (2026-10-06 UTC): the file-server request and its
capability are `fs::BIND_SYNC`, `SyncOnly` and `Manifest::sync`, and "flush" means the block
device's own cache command only. So are `reboot_capability` (capability is never abbreviated `cap`)
and the four errors under "A refusal returns its reason".

## The ruling

1. A new kernel object kind carries the authority to restart the machine. It has no payload: the
   kernel stores nothing in it, and it names no device. It has one method and its own method
   module, `abi::reboot`, and no `abi::objtype` number (below). This is a new method within the
   capability model of §10 (process model: capability-based, microkernel) and §16 (object
   revocation). It adds no syscall number.
2. The kernel creates exactly one at boot and grants it to the progenitor. The progenitor endows it
   to a job whose manifest declares the `reboot` field and whose bytes something vouches for: the
   image's measured archive for the image's own programs, or an owner's `grant` line for installed
   bytes (ruled 2026-11-02 UTC). The grant line binds to the bytes' digest, or to the package
   identity where a package vouches for them, and it names no program. Nothing in nife keys the
   object on a program's name, and several programs on one system may hold it. A job that does not
   declare it, or whose declaration nothing vouches for, holds none. The image's `reboot` holds it
   through the measured archive and stays the default. The same rule governs the sync-only
   capability, `Manifest::sync`.
3. The image's `reboot` syncs the writable filesystem through a sync-only capability, waits for the
   reply, then invokes `REBOOT` (below). An ordered shutdown of services is deferred to milestone
   853 (orderly shutdown closes the sync window), which will add its step before the `SYNC`.
4. Power-off is excluded. It is a separate program and a separate object kind, so holding the
   reboot object confers no power to switch the machine off.

The kernel grants the progenitor the object with `WRITE | GRANT` at slot 31. The progenitor never
invokes it, but delegation only narrows. It places the object with `WRITE` alone at slot 13
(`grant_plan::REBOOT_SLOT`).

## The method

`REBOOT` takes no arguments. Holding the capability is the whole authority; the method checks no
rights bit beyond what invoking any object needs.

- On success it does not return. The kernel calls `arch::reboot`: PSCI `SYSTEM_RESET` on aarch64,
  SBI SRST reset type 1 on riscv64, and the FADT reset register, then `0xCF9`, then the 8042 on
  x86_64.
- If the firmware refuses and `arch::reboot` returns, the method returns the refusal's reason to
  the caller (below).
- A firmware that accepts the call and hangs looks, from inside the machine, exactly like a slow
  reset. radon did exactly this on 2026-09-04, and milestone 592 (radon's cold reboot dies in
  OpenSBI's PMIC write) holds the fix.
- The kernel does not sync anything. It knows no filesystem, and a microkernel that did would be
  the bug. The `SYNC` is the program's step. A holder that invokes `REBOOT` without it loses writes
  the device had not flushed. That is a foot gun, recorded here and in the program's `BUGS`. The
  image's `reboot` always syncs first. A holder the owner granted the object may skip the sync, and
  the owner's grant accepts that risk.
- The kernel does not quiesce devices first, so it cuts off a DMA transfer in flight. After `SYNC`
  the block servers are idle, and nothing else writes to persistent storage.

## A board's PMIC route runs in userspace

Ruled 2026-11-10 (UTC). A board that resets through its power controller does so from userspace,
not from the reboot object. On a JH7110 the I2C master and the AXP15060 PMIC write leave the kernel
for a userspace PMIC route: a process that holds the I2C controller's page as a `DeviceFrame` and
answers one request, reset the board.

- The image's `reboot` calls the route after the `SYNC` and before `REBOOT`. A write the PMIC
  honors never returns.
- The route bounds its own wait (ruled 2026-11-11 UTC). After its write it waits two seconds for
  the board to reset, then answers `StillRunning`. If the route is absent, refuses, or answers
  `StillRunning`, `reboot` invokes `REBOOT`, the plain firmware route above.
- `reboot`'s own call to the route has no timeout, so a route that stops answering hangs `reboot`.
  Bounding that call is follow-on work that waits on a timed call, which nife does not have; a
  separate proposal asks for one as its own milestone.
- The reboot object itself stays as clause 1 states it: no payload and no device. `REBOOT` calls
  `arch::reboot` and nothing board-specific.

The route is `components/src/power_management_chip_route.rs`, from milestone 1062 (the device
control planes leave the kernel). Its starter brings I2C5 up at boot through the JH7110 clock and
reset server of §320 (the JH7110 clock and reset wire), and the kernel holds no I2C master. The
route's name and its request are provisional, like the object's.

## How `reboot` syncs: a sync-only capability

For an ordinary client, `fs::SYNC` needs a handle carrying `dir::WRITE`, which is also the right to
open and truncate files by name. So the file server has one more protocol message,
`filesystem_protocol::fs::BIND_SYNC` (66, provisional), beside `BIND` and `UNBIND` (milestone 606
(a directory walk costs what it does on Linux), ruling D). It binds a client badge sync-only: the
server answers `SYNC` on that badge with the block server's flush count and refuses every other verb
with `EPERM`, before it reads a handle. The badge names no directory and reaches no file. `BIND`'s
caller rule holds (only an unbound badge may bind), and `UNBIND` revokes it like any grant. It is a
server message on an existing endpoint, inside the badge model of §230 (badged endpoint
capabilities), and adds no syscall and no kernel method.

- A job that holds `Manifest::sync` (clause 2) gets the capability this way. The spawn service takes
  a client window, as for a directory grant, binds its badge sync-only, places the badged endpoint
  `WRITE`-only at slot 14 and deletes its own copy. The window goes back when the job is reaped, so
  the progenitor holds nothing extra for the life of the boot.
- The program sends `SYNC`, checks the answer, prints it, and invokes `REBOOT`. A failed sync (any
  errno but `EOPNOTSUPP`) makes it refuse. A boot with no writable filesystem leaves the slot empty
  and there is nothing to sync. A boot whose file server cannot bind one refuses the spawn, rather
  than start `reboot` unable to sync.
- The cost, recorded in the program's `BUGS`: the sync does not cover a write another job makes
  between the `SYNC` reply and the reset.

Alternatives considered:

- A, built: the sync-only binding above. Its gap is a write between the `SYNC` reply and the reset.
- B, the progenitor sends `SYNC` at the spawn and passes the answer in a start register: refused,
  because it turns the progenitor into a god process.
- C, give `reboot` an ordinary `dir::WRITE` handle: far too much authority for a sync.
- D, orderly shutdown: the reboot path tells every stateful server to stop taking writes, sync and
  acknowledge before the reset. It is the only option that closes the window. It needs a service
  manager to own shutdown order, so it is neither `reboot`'s job nor the progenitor's. Milestone
  853 owns it.
- E, the kernel calls registered pre-reset endpoints: puts shutdown policy in the kernel, against
  the narrow syscall surface.
- F, a crash-consistent filesystem, so no sync is needed: still loses recent writes, and RedoxFS
  makes no such promise today.

Prior art, recalled, not re-read:

- Linux `reboot(2)` does not sync. `reboot(8)` and systemd call `sync(2)` first, and `sync(2)`
  needs no privilege, which treats a sync as harmless authority and supports A.
- Capsicum has a separate `CAP_FSYNC` descriptor right: A at the descriptor level.
- E and KeyKOS facets, and seL4 and CAmkES badge attenuation, are the A pattern.
- Fuchsia's `component_manager` stops components in dependency order, and `fshost` flushes on
  `fuchsia.process.lifecycle` `Stop` before power control resets: D.
- systemd stops units in reverse dependency order, remounts read-only, syncs, then resets: D.

## The object has no `objtype` number

`objtype` lists what `MemoryRegion::RETYPE_OBJECT` (method 2, its name spelled out by the ruling of
2026-11-11 (UTC)) can make out of memory. Nothing makes a reboot object, and a number there would
read as a way to. It has a method module, `abi::reboot`, like `Irq`, the other kernel-minted object.

Alternatives considered:

- A, built: no `objtype` number and its own `abi::reboot` method module, like `Irq` and
  `MemoryRegion`, which have none either.
- B, a number `RETYPE_OBJECT` refuses: `objtype` would then mean both "can be made" and "exists".
- C, a retypable number: anyone holding memory could mint reboot authority.
- D, split `objtype` into a retype list and a full object-kind list: worth it only once a
  capability type query exists, and nife has none today.

Prior art, recalled, not re-read:

- seL4's `seL4_ObjectType` lists only retypable objects; `IRQControl`, `ASIDControl` and the
  domain capability are internal capability tags only. That is A.
- Zircon's `zx_obj_type_t` lists every kind, kernel-minted resources included, because
  `zx_object_get_info` reports a handle's type, and creation is a syscall per kind. That is D.

## A refusal returns its reason

- `arch::reboot` returns a portable reason, as an `abi::Error`, on all three architectures. The
  four are no mechanism (an aarch64 tree with no usable `/psci`), not supported (PSCI or SBI
  `NOT_SUPPORTED`), denied (PSCI or SBI `DENIED`), or still running (every `x86_64` route tried,
  or any other PSCI or SBI code). The console still prints the firmware's raw code.
- `REBOOT` answers that reason as its own `abi::Error`: `NoResetMechanism`, `ResetNotSupported`,
  `ResetDenied` and `ResetDidNotHappen`, -12 to -15. There is no separate reason type, under §113
  (kernel object plain names), one name per concept. `reboot` prints which on its second stream,
  and it does not exit non-zero, because no program here reports an exit status.
- No QEMU machine this tree boots can refuse a reset, so `abi`'s
  `a_reset_refusal_maps_to_its_reason_and_survives_the_wire` pins the mapping on the host.
- Prior art, recalled, not re-read: Linux `reboot(2)` logs a firmware failure and halts, so the
  caller never learns it; Fuchsia's `zx_system_powerctl` returns a status to its caller.

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
| Power-off in the same milestone, or a `--off` flag | One program, one thing (2026-09-26), and on radon a powered-off board needs a person at smart plug 2. |
| The object bound to the program named `reboot` | Nife would then name every program that may restart the machine, and an owner could not install a replacement from a source they opted into (§195 (a reviewed recipe vouches for a package)). Refused 2026-11-02 (UTC) in favor of the owner's grant in clause 2. |

At equal cost the answer is the same. The reboot object is the only option that is both a
capability and possible on all three architectures. Fuchsia's `zx_system_powerctl`, which takes a
resource handle, is the same shape; Linux's `reboot(2)` behind `CAP_SYS_BOOT` is D with an identity
check. Both are recalled, not re-read.

## Open

- Who at the prompt may reboot is not decided. Any session the progenitor endows can. Restricting
  it to some logins belongs to whichever milestone decides what a login grants.
- Clause 2's owner grant is unbuilt. The note encoder (`capability_manifest_note::encode`) and
  `grant_plan::image_can_carry` still refuse `reboot` and `sync` for every installed image, and the
  `Vouch` the progenitor builds carries only the network. The proposal
  design/roadmap/proposals/leaving-the-central-program-table.md schedules the change.
- The new dispatch arm is cold, and `script/fastpath-footprint` will price it when it exists. No
  number is claimed before then.

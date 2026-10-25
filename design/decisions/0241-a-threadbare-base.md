---
status: AMENDED
raised: 2026-09-27
decided: 2026-09-27
ratified_by: calef
---

# 241. A threadbare base: the boot slot holds the kernel and what boots and repairs, and every other program is a package

*Section number provisional, and the file name a lane's coinage.*

Raised 2026-09-27 (UTC) by calef: *"we need to come back to minimizing
the set of programs in the base so that we can update more without updating the base... However it
seems like maybe the base need not contain the kernel... Really, don't we just want a thread bare
base?"*

*Amended 2026-10-07 (UTC): a slot holds only the kernel, progenitor and root supervisor; the rest of the floor is packages. See §159 (only a new kernel needs a reboot).*

## The ruling

calef ruled both questions on #1421, 2026-09-27 (UTC).

1. **Yes, B2.** The slot holds the kernel plus the floor, and every other program is a package. B3
   (a kernel unit of its own) is refused. A thinner floor, with a minimal recovery shell in the slot
   and `swish` as a package, is noted for later and not ruled; see Follow-on.
2. **The default set is a list in the slot**, not a fifth package kind. §239 is unchanged.

## What is being decided

Which programs a boot slot carries. §159 (only a new kernel needs a reboot) wants almost every update
to skip the reboot. §235 (the OS is built and updated from packages) ruled U4: each slot holds a full
copy of the base, and a slot takes effect only at a boot. So every base program is, today, a program
whose update is a reboot. The smaller the base, the more of §159 holds.

## The premise, checked

Five facts from the tree, each of which changes an answer below.

1. A base program cannot be updated by a package at all. calef ruled that `package install` refuses
   a package whose program name equals a base program's (§229 (a bare name reaches an installed
   program), in pull request #1374). So membership in the base is exactly the set of programs whose
   only update path is a slot.
2. Today no package updates without a slot either. `package install` checks a package's digest
   against the image's catalog, an archive entry the kernel's trust root vouches for
   (`notes/packages.md`). §220 (signed builds) ruled that a trusted key vouches instead, and says
   of itself "Nothing here is built". Until it is, a new version of any package is a new catalog,
   which is a new slot.
3. The archive packs by `[[bin]]`, not by package kind. `xtask/src/archive.rs` packs every binary in
   `components/` and `fixtures/` except `greeting`. So §239 (four package kinds)'s `test` and
   `optional` packages ride in every image, contrary to that section's own table.
4. Under T4 with T2 the kernel embeds no userspace digest. The loader hands over the progenitor's
   digest. The kernel and the progenitor still release together, but because of the boot
   endowment (the capabilities the kernel places in fixed slots 1 to 9), not because of a hash.
5. The kernel itself starts `block_driver` and `redoxfs_server` (`kernel/src/user.rs`,
   `fs_service.rs`). Under T4 it checks only the progenitor, so those spawns move to the progenitor
   in P6 whatever this section decides.

## What is in the base today

Measured from the last riscv64 archive in the main checkout, `target/initrd-riscv.img` of
2026-09-21, grouped by the declarations in `packages/*.package.toml` from milestone 611 (every program and crate belongs to a package). The
archive is stale by six days and lacks a few newer programs (`terminal_supervisor`, `top`); the
shares are an order of magnitude, not a figure to quote. 92 entries, 8.8 MB.

| package (kind) | programs | size | share |
|---|---|---|---|
| fixtures (test) | 40 test programs | 1,559 KB | 18% |
| unowned | `redoxfs_server`, `mkfs`, `std_exerciser`, `audit_sink`, the measurement table | 1,258 KB | 15% |
| network (base) | `net_stack` | 981 KB | 11% |
| init (base) | `progenitor`, `root_supervisor`, `spawner`, `sub_server_supervisor`, `job_undertaker`, `swapper`, `broker`, `session_reviver` | 870 KB | 10% |
| swish (base) | `swish` | 661 KB | 8% |
| display (optional) | `compositor`, `display_terminal`, three drivers | 651 KB | 8% |
| login (base) | `login`, `credentialer`, `identity_provisioner` | 417 KB | 5% |
| timetable (base) | `timetable` | 390 KB | 5% |
| util-linux, coreutils, procps, mdr (base) | 12 command-line tools | 926 KB | 11% |
| drivers, filesystem, terminal, time, entropy (base) | 14 services and drivers | 765 KB | 9% |
| rmle, demos (optional) | two | 119 KB | 1% |

Kernel and loader are outside the archive. The x86_64 boot file of 2026-09-17 was 10,158,080 bytes
with a 4,748,288-byte archive inside, so loader and kernel were about 5.4 MB.

## The floor

The progenitor already draws most of it. `crates/system_initializer`'s `boot` calls `fail()`
when a required program is missing, and treats the rest as optional: "a missing component should
cost a feature, not a prompt". The required set is `console`, `input`, `line_editor` (through
`terminal_supervisor`), `swish` and `job_undertaker`. The floor is that set plus what reaches the
store and what repairs it.

| program | why it is in the slot |
|---|---|
| kernel | Nothing else can roll back a bad kernel; see the next section. |
| `progenitor` | The kernel's one measured program. It reads the activation set and is the installer. |
| `console`, `input`, `line_editor`, `terminal_supervisor` | The owner's console (§221 (the boot prompt is the owner's console)), which is where repair happens. |
| `swish` | The prompt that holds the `package` verbs. |
| `job_undertaker` | Required today: without it the job pool fills and the prompt stops spawning. |
| `block_driver`, `non_volatile_memory_express`, `redoxfs_server` | Reading the store at all. |
| `net_stack` | Repair without a person at the machine. Milestone 525 (a bad upgrade cannot brick the machine)'s premise is an appliance nobody can reach. |
| a store repair tool | Missing today; see Recovery. |

That is 12 programs and about 3.1 MB of the 8.8 MB. Everything else leaves: the command-line tools,
the login stack, `entropy`, time, `timetable`, the filesystem caretakers, the test supervisors, and
the display stack. The fixtures leave every image, as §239 already says they should.

Over the
30 days to 2026-09-27, `crates/system_initializer` (the progenitor's logic) had 59 commits and
`swish` 38. Each such commit would be a reboot on a released machine; Follow-on names two ways to cut that.

## The kernel and the slot

calef asked whether the base need contain the kernel. It need not be in the base's release unit,
and under U4 it already is not in any meaningful sense. It should stay in the slot.

- A kernel-only update under U4 writes the inactive slot with the new kernel and copies every
  package whose digest matches the running slot locally (§235's Fork 1). The download is the
  kernel. The base does not change, and is not rebuilt.
- The reboot is the kernel's, not the slot's. A new kernel reboots under every option here, until
  milestone 509 (live-patching the kernel).
- The slot is the only rollback a bad kernel has. Milestones 525 and 554 give it tries and a
  success mark. A kernel in a unit of its own would need its own tries, and the machine would boot
  kernel and floor combinations nobody tested together.
- The floor depends on the kernel's boot endowment. A kernel beside a floor it was not built with
  can fail in the first instruction of the progenitor.

## Per program: updated without a reboot

Leaving the slot changes the mechanism, a package and an activation generation instead of a slot.
It does not by itself remove the reboot for a long-lived service. Three groups, by how a program
lives.

| group | programs | new bytes take effect | what is missing |
|---|---|---|---|
| Run per use | `date`, `printenv`, `rm`, `wc`, `ps`, `pgrep`, `pmap`, `top`, `uptime`, `uuid`, `mdr`, `disk_surveyor`, `disk_partitioner`, `installer`, `rmle`, `least_authority_demo` | at the next spawn | a built §220 (signed builds), fact 2 above, and #1374's bare names |
| Built per grant or per login | the three filesystem caretakers, `login`'s per-session caretaker | at the next grant or login | the same; old grants keep old bytes until released |
| Live swap built | `line_editor` | on a swap (§232 (the line editor swap contract), `FLAG_RETRY` in `swish` and `rmle`) | the trigger, `design/roadmap/0694-the-installer-asks-the-terminal-to-swap.md`, and §229's refusal, since `line_editor` is in the floor |
| Long-lived, no swap | `net_stack`, `entropy`, `clock`, `network_time_client`, `login`, `login_audit_receiver`, `credentialer`, `terminal_sink_caretaker`, `display_terminal`, `compositor`, the display drivers | at the next boot | §159's restart tier: a supervisor that rebuilds the service from new bytes, and clients that survive a reconnect |
| Not started by a real boot | `root_supervisor`, `spawner`, `sub_server_supervisor`, `swapper`, `broker`, `timetable`, `session_reviver` outside its test | at the next spawn | nothing |

`credentialer` is the hard case: its sealed store cannot be rewritten after boot (§221), so it
reboots until someone designs a provisioning path (see BUGS).

For the long-lived group, leaving the slot still buys two things before the restart tier exists.
A bad version is undone by `package rollback` rather than a slot fallback, and the update cannot
break the floor's ability to boot a prompt.

## Recovery

The two slots protect only what is in a slot. Installed packages are protected by §208 (installing
is granting)'s generations: a generation is never rewritten, `current` is renamed into place, and
`package rollback` points it one generation back. That covers a bad package, not a corrupt store, since the generations live in the store.

Today a corrupt store degrades rather than bricks. When `redoxfs_server` finds no filesystem, the
kernel passes no file service and the progenitor boots a prompt without one, which is the ordinary
diskless case. The floor exists so that this prompt can put the store back. Two things are missing:

- No on-device tool makes a new store. `mkfs` is packed but is not a `grant_plan::Prog`, and
  nothing at the prompt can reach it. Fixing that is P9 below.
- A new store has no packages. The default set has to be reinstalled from a source. The slot's
  catalog names the default set's digests, so the progenitor can refuse anything else, and
  `net_stack` fetches it.

A bad boot service installed from the store is the second recovery case. The progenitor's missing-
component rule means a service that fails costs its feature, not the prompt. A generation that
breaks `net_stack` is why `net_stack` stays in the floor: on a remote machine it is the path to the
rollback.

## Trust

A floor program is checked as today: the progenitor against the slot's manifest digest (T4 with
T2). A program moved out is checked at spawn, by digest, against the live activation generation
(`vouched` in `crates/system_initializer`, §219 (how the shell names an installed program to the spawner)). That digest was pinned at install, against the
catalog today and a trusted key once §220 is built. So everything that runs is still measured; only where the list lives changes.

It matters once Secure Boot (milestone 500 (a stick that boots with Secure Boot on)) protects the
slot. The activation set is on a disk the owner's console may write (§221), and so may anyone with
the disk out of the machine. Moving a program out moves its pin from a place Secure Boot covers to one it
does not. Until milestone 500 lands, neither is covered, and nothing changes. When it lands, the gap
is an offline attacker adding a digest to `activation/`. Two remedies exist, neither needed now:
seal the activation set with a machine key, or check signatures again at boot, which §220 refused
for spawn. Recorded in BUGS.

## Prior art

In [`prior-art.md`](0241-a-threadbare-base/prior-art.md).

## Options

| option | shape | cost | verdict |
|---|---|---|---|
| B1. Status quo, full base | Every `base` package in every slot, as `packages/*.package.toml` says today | Every change to 16 base packages reboots. §159's rule 2 holds for nothing except installed third-party programs. | Refused: it contradicts §159. |
| B2. Threadbare slot | Kernel plus the floor above in the slot. Every other program is a package; a default set is installed with the image. | The default set needs a home (a slot list), the progenitor must build optional services from the store, and §220 must be built. | Ruled. |
| B3. Kernel in a unit of its own | A third updatable unit for the kernel, separate from the floor's slot | Tries for a second unit, and kernel and floor pairs nobody tested together. Saves nothing U4 does not already save. | Refused. |
| B4. Minimal slot with no network | B2 without `net_stack` | 1 MB less per slot. A remote machine whose store breaks needs a person. | Refused, on milestone 525's premise. |
| B5. Floor programs overridable from the store | B2, and a newer store copy of a floor program replaces the slot's after boot | Recovery runs a shell older than the one in daily use, and §229's refusal gets an exception. | Not now. Follow-on, because of the churn numbers above. |

B2 was ruled (The ruling, above), with the floor as listed and the default set a list in the slot
rather than a fifth package kind. §239 says a new kind must name a new destination, and "installed
by default" differs per image (a lab image and a desktop image), so it is a property of an image
rather than of a package. Under B2 the `base` kind shrinks to the floor's packages.

Would we choose B2 if every option cost the same? Yes: fewer moving parts than B3 and fewer
reboots than B1. It is the Sculpt shape, not the least work.

## The seven questions, briefly

Questions 1, 3 and 7 are answered above. The tree's analogous case (2) is the progenitor's own
required and optional split, and §239's `optional` kind. The premise (4) holds in part: the kernel
stays in the slot, and a smaller base removes reboots only for programs run per use until the
restart tier exists. Costs (5) are
measured where a command could; the restart tier is not. Reversibility (6): nobody outside this
machine has installed a slot, and the floor can change in any later release.

## What depends on this

P5 (an OS update is a set of packages through a slot) is not built and is what this shapes: it
needs to know which packages a slot carries. P1 (the image assembled from base packages by digest)
builds whichever set is ruled. Milestone 198 (a package manager)'s exit criterion "a new package version with no
reimage and no reboot" is met for the run-per-use group only under B2 and a built §220.

## What was asked, and what was ruled

Two questions were asked; both were ruled yes (see The ruling, above).

1. B2, with the floor as listed?
2. The default set as a list in the slot, rather than a fifth kind?

P5 and P1 were blocked on the first ruling; both now build against the floor as listed.

## Proposed work, none minted

| id | proposal | waits on |
|---|---|---|
| P8 | The archive packs by package kind: no `test` or `optional` package in a release image | nothing |
| P9 | A store can be made new from the owner's prompt | nothing |
| P10 | The progenitor builds optional boot services from the live generation, falling back to the slot while one exists | nothing (B2 ruled) |
| P11 | §159's restart tier: a supervisor restarts a service under new bytes, starting with `net_stack` | nothing (B2 ruled) |

## Follow-on

- A thinner floor, noted for later in the ruling above and not decided.
- Shrinking the floor's churn. The HTTP fetch can leave the progenitor once milestone 205 (how a
  foreign program is told what to do) lands, as `notes/packages/fetching.md` already says. B5 is
  the other lever.
- The chooser on the ESP (`uefi_loader/src/chooser.rs`) sits outside both slots, so a bad chooser
  update has no rollback. Milestone 525's record should say whether that is intended.


## BUGS

- The sizes are a six-day-old riscv64 build. P8's lane should measure a fresh one per architecture.
- Once milestone 500 protects the slot, programs outside the floor are pinned on an owner-writable
  disk and are not covered by Secure Boot. See Trust.
- `credentialer` cannot be restarted without a way to reprovision its sealed store, so under every
  option it updates at a boot.
- The floor lists `non_volatile_memory_express` for every architecture, as the archive does. A per-architecture floor is P8's to decide.

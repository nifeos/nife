---
status: NOT-STARTED
raised: 2026-10-07
promoted_from: lab-machines-update-themselves
milestone_dependencies: 198, 809, 805, 23, 666, 592, 593, 53, 911
decision_dependencies: unwritten
machine_requirements: x86_64 and riscv64 silicon with a writable boot disk and a hardware watchdog
specific_machine: radon (calef's preferred lab machine, and the one whose chooser, disk writes and watchdog are unbuilt)
needs_person: yes
---
# 858. Lab machines update themselves through packages, and only a new kernel reboots them

*(The title and slug are provisional.)*

calef asked on 2026-10-07 (UTC): *"We should have a milestone that lets our lab machines updated
themselves for runs using our package management solution. Download their updates, apply them, and
restart (if it is the kernel). Sound right? We want to be testing updates early."* Written by lane
`lab-self-update-proposal`, which built nothing and touched no lab hardware. Every name here is
provisional. Appendices: [the lab machines](0858-lab-machines-update-themselves/the-lab.md),
[prior art](0858-lab-machines-update-themselves/prior-art.md), [the forks](0858-lab-machines-update-themselves/forks.md)
and [the TUF clients measured](0858-lab-machines-update-themselves/tuf.md).

## Rulings

calef ruled every fork on #1805 on 2026-10-07 (UTC), relayed by the maintainer in comments between
05:51 and 13:32. Those comments are the record. The forks as he read them are in
[the forks appendix](0858-lab-machines-update-themselves/forks.md).

| fork | ruling | his words |
|---|---|---|
| 0 | the tree correction below is right | "Yes" |
| 1 | generations get tries, as slots have; a boot whose required set fails starts the previous generation. A trial | "Let's try it." |
| 2 | a trusted key vouches (§220, milestone 666) and the package's kernel ABI revision must match the running kernel; a manifest field carries it; §229's refusal narrows to slot programs. A trial | "Let's try it." |
| 3 | apply right away, by kind: next spawn, live swap, or a supervisor restart in dependency order, then a report of what was swapped, restarted or still runs old code | "Apply right away." |
| 4 | a package is good when its service reports ready and answers a real probe; a kernel slot when the shell answers and the machine fetches the lab index again; the run's result is recorded and does not gate | "Yes" |
| 5 | recast. Kernel and base stay separate packages. A signed basalt release manifest, a TUF target, pins a kernel and the base versions tested with it. Only packages whose ABI field needs the new kernel activate with it, in one generation on the boot that confirms the slot. Kernel N-1 compatibility is a candidate milestone | "How hard is it to keep the kernel backwards compatible for a generation?", "If we have to version the base and kernel together, should that be one package?", then "Yes" |
| 6 | a principle wider than the lab: CI is never part of deployment; the owner decides, and automatic updates are an owner setting that pulls from a channel | "CI should not be part of deployment. Owners should decide when and if to update a machine and can enable automated updates if they choose." |
| 7 | `uefi_loader` through U-Boot's `bootefi` on radon and argon, slot state in the GPT as on x86_64, a FAT tries file as the fallback | "Yes" |
| 8 | the lab's index is served from cordoba, advanced on each passing basalt gate, delivered as a spec for calef's homelab agent | "Start on cordoba." |
| 9 | TUF. The progenitor holds a minimal Kani-proven `no_std` verification core (canonical parsing, thresholds, version and expiry), offered to rust-tuf first, else published standalone. `jig` and basalt fetch and mirror on rust-tuf with RustCrypto in place of `ring`, offered upstream | "Should we be considering TUF?", then, asked what he would choose if options cost the same, "Yes" |
| owner sources | owners choose their sources: basalt, their own mirror or server, or local media, air-gapped included. A mirror copies the TUF repository without re-signing. An owner may trust a further repository under their own root key. Freshness is on by default and switchable off per source; rollback protection is always on | "I want to make certain owners can update off of their own media or servers. I'm thinking of air gapped deployments for example.", then "Yes" |

Fork 6 and the owner-source requirement are recorded as calef's ruling in §250 (an image names its
distribution's package index)'s amendment of 2026-10-07. The cordoba spec is
[notes/lab-index-on-cordoba.md](../../notes/lab-index-on-cordoba.md).

An automatic update installs only within an approval file the owner wrote, through an installer
endpoint badged to it: milestone 911 (scripted installs grant only what the owner's approval file
allows), added 2026-10-12 (UTC). Fork 6 said automatic updates are an owner setting; 911 is what
bounds one.

## Only a new kernel reboots

calef, 2026-10-07 (UTC): *"The whole system needs to be updatable through packages. The kernel
will need a reboot for now and that is fine. However should base updates require reboot? Why?"*
Then, to the model below: *"Yes. We have discussed this before so it definitely needs updating in
the tree."*

§159 (only a new kernel needs a reboot), ratified 2026-09-19, is the rule: *"Upgrading a userspace
component does not reboot the machine"*, and §208 (installing is granting) builds on it. §159's
amendment of 2026-10-07 records this ruling. §235 (the OS is built and updated from packages),
§229 (how a bare name reaches an installed program), §241 (a threadbare base) and milestone 198 (a
package manager) point at it.

## The model, ruled

1. A boot slot holds the kernel and the root programs that nothing can restart: the progenitor, and
   the root supervisor once a real boot starts one. They update with a reboot, and a bad slot rolls
   back by its tries (milestones 525 and 554).
2. Every other program, base included, is a package that `jig` (milestone 809 (the package client
   becomes a program)) installs. It takes effect when a supervisor restarts it from the new bytes,
   or by a live swap (milestone 23 (a capability-routed component OS with live replacement)).
3. A base package that needs a new kernel ABI rides the kernel's reboot.
4. A service whose state cannot be handed over is restarted, not rebooted.
5. A bad package rolls back with `jig rollback`, §208's generations. A bad slot rolls back as built.

Proteos, the MINIX 3 rewrite, is the closest precedent: every process updates in a transaction and
only the kernel reboots. Fuchsia is the contrast, where the whole base is an over-the-air update
and a reboot (appendix).

## What the tree has against the model

Checked rather than assumed. Each is a gap the model must close before a lab machine can use it.

1. **No package can update a base program without a new slot today.** Install checks a package's
   digest against the image's catalog (`notes/packages.md`), which the slot carries. §241 says so:
   until §220 (signed builds) is built, a new version of any package is a new catalog, which is a
   new slot. §220's build is milestone 666 (a signed build installs up to its key's ceiling),
   NOT-STARTED. This is the hard dependency.
2. **§229's refusal** is built (`activation_set::Error::ImageName`) and must narrow to slot programs.
3. **The progenitor builds every service from the archive.** `crates/system_initializer` names each
   by archive entry. Starting a base service from the activation set is what §241's B2 required and
   nothing has built.
4. **The kernel itself starts `block_driver` and `redoxfs_server`** (`kernel/src/user.rs`, §241's
   fact 5). Nothing can restart those, so in effect they are slot programs until milestone 811 (the
   boot services leave the kernel) moves the spawns to the progenitor.
5. **No real boot starts `root_supervisor`.** Only `authority_tests` does. Today the progenitor is
   the root, and the slot holds the kernel and the progenitor.
6. **The restart tier is unbuilt.** Live swap exists for `line_editor` alone (§232 (the line editor
   swap contract)). §241's table lists `net_stack`, `entropy`, `clock`, `login` and the display stack
   as "at the next boot". §209 (state handoff is an opaque blob, and optional) gives the handoff a
   shape; nothing a real boot runs uses it.
7. **The manifest note has no ABI revision field.** §235 ruled one; nothing writes it, so "needs a
   new kernel ABI" cannot yet be read off a package.

## The worklist, in order

Each line is one lane. "Proposed" means a new milestone an integrator would number.

1. Milestone 811 (the boot services leave the kernel): the kernel stops starting `block_driver`
   and `redoxfs_server`.
2. Proposed, the slot holds only what nothing can restart: the progenitor starts every base service
   from the live generation, the archive carries the kernel's boot set, and §229's refusal narrows.
3. Proposed, the ABI revision field in the manifest note (§235, Fork 2), and the progenitor's check.
4. Proposed, a proven TUF verifier in the progenitor (Fork 9): the `no_std` core with its Kani
   harnesses, offered to rust-tuf as a separable crate. Milestone 666's signed builds take TUF's
   shape here, a publisher's key as a delegated targets role.
5. Milestone 809 (`jig`): fetching on rust-tuf with a RustCrypto backend, offered upstream; the
   owner's sources, mirrors and local media; freshness per source; rollback always on; automatic
   updates as an owner setting (Fork 6). Today the source is compiled in as QEMU's 10.0.2.9:8080
   (`notes/packages.md`), so no lab machine can fetch anything until this lands.
6. Proposed, generations get tries (Fork 1).
7. Proposed, a supervisor restarts a service from new bytes (Fork 3), §159's restart tier, with the
   report; live swap stays milestone 23's. Milestone 910 (a package declares a service: install
   starts it, boot starts it, update restarts it) builds the mechanism for a package's declared
   service (added 2026-10-12 UTC); this item extends it to base services.
8. The health checks of Fork 4, built with items 6 and 7.
9. basalt (out of tree): pin bump per nife merge; on each passing gate, publish a TUF repository
   whose release manifest pins a kernel and its base (Fork 5).
10. This milestone: the slot writer, the slot confirmation of Fork 4, and the run record.
11. Proposed, `uefi_loader`'s chooser on riscv64 and aarch64 through `bootefi` (Fork 7), after
    calef answers bench item 10 in `notes/visionfive2.md`.
12. Milestones 593 (radon's and argon's watchdog steps), 592 (radon's reset) and 53 (radon's SD
    writes), each a bench step of calef's.
13. calef's homelab agent: the cordoba index (Fork 8), from the spec.

Candidate, not proposed: kernel N-1 compatibility (Fork 5), triggered when the stability chart's
syscall rows stay at zero changed for several weeks, or when a first customer arrives.

Reuse: `crates/boot_slot`, `uefi_loader`'s chooser, `crates/activation_set`, `crates/component_plan`'s
dependency graph, the `reboot` program, `helpers/package-http-peer` and basalt's gate. MINIX 3's
service update, QNX's HAM, Mender, RAUC and SWUpdate were considered for their state machines, not
their code: each assumes an OS or a bootloader nife does not have.

## Safety, for machines nobody is standing at

calef's rules bind every step: plug 3 is never switched, radon's USB hub is never unplugged, and no
lane powers, boots or reaches radon or xenon. So each machine recovers by itself, or stops somewhere
safe to leave.

The model moves most risk off the slot, which is the point. A bad package costs a restart and a
generation rollback, not a boot. Three risks remain.

- A hang after a kernel update is recovered only if something resets the machine. The chooser
  spends a try before handoff, but nobody power-cycles. xenon's TCO watchdog (milestone 593 (a
  wedged kernel resets itself)) is proven only under QEMU. radon has no watchdog step. xenon also
  halts at power-on without a keyboard (milestone 260 (boot xenon over the network)).
- A package can break the path to its own rollback: a bad `net_stack` cannot fetch, and a bad
  `swish` cannot type `jig rollback`. §241 kept those in the slot for that reason. Fork 1 replaces
  that floor.
- radon's microSD card is its only boot medium, and nife cannot yet write it (milestone 53 (the board's own peripherals), PARTIAL).

No machine takes an unattended update until its watchdog is proven on its own silicon, and a doomed
kernel and a doomed package have each been rolled back on it once with calef watching.

## The run records the update

A failed update is a test failure. The run log names the slot build and generation the machine
started on and was offered. The banner prints no commit and the slot header holds a length and a
CRC, so both gain the build's identity. It then gives one outcome per step: `confirmed`, `rolled back
by tries`, `rolled back by the chooser`, `package rolled back` (naming the service that failed its
check), `fetch refused`, or `not offered`. patagonia already captures both machines' serial
(`script/board-console`). The new part is a parser that turns those lines into a verdict, the way
`swish-check` reads the prompt.

## Scope note (rule 5)

The package path is architecture-free. The chooser is x86_64 today; Fork 7 adds riscv64. argon is
the TX1 once it arrives and waits on milestone 803 (argon boots the aarch64 kernel); the board in
hand is a TK1 that nife cannot run. radon's and argon's watchdogs are milestone 593's later steps.

## BUGS

- Nothing here has run on silicon. The x86_64 rollback is proven under OVMF only.
- A service that passes its probe can still be subtly wrong. That is the run's job.

## Index row

Lab machines update themselves through packages, and only a new kernel reboots them: generations get tries, fetches are TUF-verified, a supervisor restarts or live-swaps each service, and a basalt manifest pins a kernel with the base versions tested with it. Every fork is ruled on #1805; the worklist is one lane per item.

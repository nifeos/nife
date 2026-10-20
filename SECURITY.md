# Security policy

nife is a capability microkernel built as a **demonstrator**, which is DECISIONS §14 (the project's
direction): a small machine-checked core that confines unverified workloads. Nobody runs it in
production, including us. It boots under QEMU, under HVF on an Apple Silicon laptop, and **on real
silicon**: a StarFive VisionFive 2 has been running it since 2026-08-14 (notes/visionfive2.md
carries every bench boot, including the ones that failed and why).

That shapes what a security report means here. A bug that lets a confined process escape is
interesting because escaping is the exact thing the project claims cannot happen. A bug that says
"this OS is not ready to face the internet" is not a report; it is the premise.

The response you get is a real person reading your report between other things, not a triage queue.
And one thing about how this code got written is worth knowing before you read it: **most of it was
written by AI agents working in parallel lanes, with one human reviewing architecture and outcomes
rather than every line.** That is the project's second claim and it is stated here rather than only
in AGENTS.md, because it is a fact about the code you are auditing. What stands between that method
and a pile of plausible-looking kernel is the machine-checked proofs, the gates, and the audits at
the end of this file, so the most valuable report you can send is one that finds a place where those
did not hold.

## Reporting

**Use GitHub's private vulnerability reporting**: the [Security
tab](https://github.com/nifeos/nife/security) of this repository, "Report a vulnerability".
That opens a private advisory only you and the maintainer can see, with a place to attach a fix.

If that is unavailable, email **chris@crickertech.com** with `nife security` in the subject.
There is no PGP key; if you need one before you will send details, say so in a first message and we
will sort out a channel.

**Please do not open a public issue for something you believe is exploitable.** Everything else
belongs in a public issue, and most things are everything else.

### What makes a report actionable

- The **commit sha** you were looking at. `main` moves.
- Which **architecture** (aarch64 or riscv64) and how it was run (`script/test`, `script/console`,
  `--hvf`, or on a board). Several boundaries differ between the two ISAs, and one working and the
  other not is itself a finding (DECISIONS §19 (architectural parity is a tenet)). **Say if you were
  on hardware**, because the boundaries are not the same there: see the scope note below.
- Which **boundary** you believe is crossed, in the terms below. "A process read another process's
  memory" is a different claim from "a process panicked the kernel", and both are welcome.
- A **reproduction**, ideally as a test in the existing harness (a `#[test_case]` under
  `kernel/src/`, or a host-crate test). A failing test is the most useful thing you can send, and it
  is how a fix gets proven rather than asserted.

## In scope

The claim this project makes is confinement: a workload it did not write, running unverified, cannot
reach past the boundaries the kernel enforces. Anything that breaks one of these is in scope.

- **Capability forgery or widening.** Minting a capability that was never granted, widening rights
  across `derive`, `Send`, or delegation, naming another process's CapabilityTable or endpoints, or reusing a
  generational name after revocation (DECISIONS §10 (the process model), §13 (capability revocation
  and untyped reclamation), §16 (object revocation); `crates/capability`).
- **MMU escape.** EL0 reading, writing, or executing kernel memory or another address space;
  breaking W^X; a stale TLB entry exposing a dead owner's data; anything reachable from userspace
  that maps physical memory outside the process's own untyped budget (`crates/paging`,
  notes/mmu.md).
- **DMA escape.** A device programmed by a userspace driver to read or write memory outside the
  grant it was given, through the software descriptor validator or past the IOMMU domain (DECISIONS
  §20 (IOMMU-backed DMA isolation), §23 (multi-queue DMA confinement), §30 (the DMA boundary is
  proved for descriptors); `crates/direct_memory_access_validator`, notes/iommu.md).

  **On the VisionFive 2 there is no IOMMU at all**, so on that board the confinement for virtio is
  the software validator and nothing else. That is a property of the silicon rather than a defect,
  and it is recorded in notes/dma.md and notes/framebuffer-contract.md, but it makes the validator
  the single point of failure there rather than the first of two. A validator bypass is the
  highest-value finding in this file. The board's own SD/MMC and Ethernet servers are outside the
  validator: each holds a register page that names DMA addresses, so each is confined by its own
  code (notes/designware-mobile-storage.md, notes/designware-ethernet.md), and the booted system
  starts neither until a bench boot proves it.
- **IPC.** Anything that lets a message reach an endpoint the sender cannot name, a reply
  capability be used twice or by the wrong thread, or a server be confused about which client it is
  answering (DECISIONS §12 (Call/Reply IPC), §26 (the fault endpoint);
  `crates/inter_process_communication`).
- **The syscall surface.** Any `svc`/`ecall` argument, from EL0, that panics the kernel, corrupts
  kernel state, leaks kernel memory, or costs unbounded kernel time. The surface is deliberately
  narrow and every method is meant to validate its own inputs (DECISIONS §4 (kernel shape) rule 3,
  §16 (object revocation)).
- **Time-of-check to time-of-use on shared pages.** Every service contract moves bulk data through a
  page shared with the client, so a value validated by one party and then re-read by another is a
  live double-fetch. notes/shared-page-audit.md is the sweep for this and says what it found, what
  it cleared, and what it deliberately did not look at. Two things it left open are the best places
  to start: the FS service memoises **one** frame for every client a boot wires (so the property
  keeping them apart is who happens to be blocked, not a mapping), and no test in this tree can make
  a virtio **device** misbehave, so the direction the IOMMU exists for has no negative control.
- **The foreign-language and vendored seams.** The C component holds no capabilities and makes no
  syscalls; the vendored RedoxFS engine runs as a confined EL0 server. A way for either to reach
  authority it was not given is exactly the claim under test (DECISIONS §27 (the filesystem
  service), §31 (the foreign-language seam); notes/c-seam.md,
  notes/redoxfs-audit.md).
- **The boot trust root.** Anything that lets an unmeasured or altered progenitor run as though it were
  measured (`crates/measured_boot`, notes/trusted-init.md). This one has been exercised on real
  hardware in the failing direction, which is the useful direction: bench boot 12 was **refused at
  the trust boundary** because the image on the card vouched for the previous archive, and the
  kernel halted rather than hand it to the progenitor. A way to get past that refusal is a report.
- **The supply chain of this repository.** A dependency or vendored tree that is not what the
  manifest says it is. `script/supply-chain` is supposed to make that checkable; a way around it is
  a finding.
- **The install path.** A package becomes authority on the target only when the image's catalog or
  the index copy `jig update` writes vouches for its digest, and it is then endowed with what its
  note asks (notes/packages.md, notes/packages/jig.md). A way to get bytes
  installed as vouched, or endowed past that, is a finding. Until the index is signed, `jig` is
  inside the trusted base for installs, which its `BUGS` says.

## Not in scope

None of these are dismissals of the underlying concern. They are things already known and written
down, which makes them roadmap items rather than reports.

- **"It is not production-ready."** Correct. There is no user authentication, no ASLR, no secure
  boot chain to firmware, no network security beyond a TLS client that trusts one pinned root
  (notes/packages/over-the-internet.md), and the shell is a demo. See
  notes/why-not-general-purpose.md.
- **A hardening feature that is on the roadmap.** design/roadmap/ is what is missing and in what
  order, one file per milestone (`script/roadmap` reads it). A missing feature that has a block
  there is a roadmap item; a *defence that is claimed to exist and does not work* is a
  vulnerability, and that distinction is the whole test.
- **Anything that requires already being the progenitor.** The progenitor is privileged and unverified, and DECISIONS
  §14 says so in the thesis itself. The kernel confines it, and a compromised progenitor cannot break the
  kernel or escape confinement, but the progenitor's authority over the processes it builds is by design.
- **QEMU or HVF escapes.** Report those to QEMU or to Apple. A guest breaking out of the emulator is
  not this kernel's boundary.
- **Board bring-up that has not happened yet.** The VisionFive 2 boots and runs the tour; its
  storage and network drivers exist and the booted system does not start them yet
  (design/roadmap/0053-board-peripherals.md), the UART
  interrupt number is wrong for that board and known, and the ratified-spec IOMMU has no silicon to
  run on. Missing hardware support is a roadmap item. A *driver that is present and confines
  nothing* is a report, and the distinction is the same one the line above draws.
- **Denial of service by a process against itself**, or a process exhausting its own untyped budget.
  That budget is the mechanism, not a bug. A process exhausting a *kernel* resource, or another
  process's, is in scope.
- **Findings in upstream code we vendor**, unless our configuration is what makes them exploitable.
  Send those upstream too; we carry redoxfs at a pinned version (vendor/README.md) and would rather
  the fix land where everyone gets it. Tell us as well, so the pin can move.
- **Reports produced by a scanner with no analysis attached.** DECISIONS §35 (what a scanner is for
  here) is the standing policy:
  every finding gets a disposition and a dismissal is a written argument. That cuts both ways, and a
  list of tool output with no reasoning is not something either of us can disposition.

## What to expect

- **Acknowledgement within about a week.** This is a side project; there is no on-call.
- **No fixed remediation timeline, and no bounty.** What there is instead: a real answer about
  whether the boundary you found is one the project claims to hold, and if it is, a fix with a test
  that would have caught it, plus a `design/decisions/` or `notes/` entry recording what was wrong. That
  is how every previous security finding here was handled (notes/security.md, notes/arch-audit.md).
- **Credit however you want it**, in the fix commit and in the note. Say if you would rather not be
  named.
- **Coordinated disclosure, preferred and not demanded.** If you want to publish, publishing after
  90 days is fine by us whether or not there is a fix; tell us the date and we will not argue. If
  the finding is in vendored upstream code, upstream's timeline should win over ours.

## Supported versions

`main`, and only `main`. There are no releases and no version branches, so there is nothing to
backport to. If a fix matters to you, it is a commit on `main`.

## What has already been looked at

**Audits here are routine rather than occasional**, and the index is
[design/audit-reports/README.md](design/audit-reports/README.md): every audit's date, the lens it
took, its findings by disposition, and a link to the report. `script/audits` says when the next one
is due, from the triggers `design/decisions/0074-audit-cadence.md` decided, and a weekly workflow asks
the same question so that auditing does not depend on anyone remembering to.

**Eleven** <!--count:security-audits--> security audits are on the record, and reading them first will
save you time. Each took a lens the previous one did not, deliberately, because the value of an audit
is the lens the last one lacked. (Documentation audits are in the same index and are not listed here;
they read the tree for claims that had gone false, which is worth knowing if you find prose and code
disagreeing.)

- **notes/security.md**: a four-part review after milestone 11 (untyped memory), with the threat
  model, what held
  up, and four real bugs (a crafted ELF that could panic the kernel, a spawn flood that could, a
  wasted-budget path, and documentation describing defences that had been deleted).
- **notes/arch-audit.md**: a pass over the assembly and architecture layer that found three:
  an `eret`/`sret` privilege-escalation staging race, a stale `tp` corrupting cross-hart per-CPU
  data on RISC-V, and a lock-free read-modify-write in the PLIC driver.
- **notes/shared-page-audit.md**: every page shared between two address spaces, asked whether a
  value checked by one party is re-read by another. Seven findings, five fixed. It also records the
  reason there were not more, which is a real property of the design rather than luck: **no contract
  in this tree carries a length in the shared page**, so the classic form of the bug is absent by
  construction. Its scope note says what it did not read, and that list is where an outside reviewer
  has the most room.
- **notes/untrusted-input-audit.md**: the parsers and drivers that read bytes a hostile counterparty
  supplies in a single message or completion, which is the surface that arrived with `crates/non_volatile_memory_express`
  and `crates/multicast_dns_protocol` after the pass above was written. The second was retired on
  2026-09-15 (milestone 298 (retire the multicast DNS responder), notes/mdns.md), so that audit's
  DNS clearance describes nothing on
  `main` today; its NVMe finding still stands. One finding, recorded and accepted: the
  NVMe driver panics on two completion fields the device writes, and the rule it hands forward is
  that **an IOMMU confines placement, not values**, so a confined device's accounting is as
  untrusted as its reach.
- **design/audit-reports/2026-08-17-newly-minted-authority.md**: the seven ABI constants and the one
  new right that landed in a single night, read adversarially, on the reasoning that a right can be
  correct in isolation and wrong in combination with what was already there. Nothing exploitable;
  the finding to carry off is a **counting channel**, where a viewer holding the narrowest capability
  this system can express learns how many threads exist outside its own domain, though it can never
  name one.
- **design/audit-reports/2026-09-17-userspace-confinement.md**: the device and port authority minted
  since the previous audit, the confinement claims a sweep had marked unreachable, and the two boards
  that booted real silicon, read for claims that are true only by accident. **One was false as
  stated and is fixed**: on `x86_64` a thread that deleted its own port capability kept the ports,
  because the grant is enforced by a cached field the context switch installs rather than by the
  capability table. Also: ring 0 could execute a user page on `x86_64` until `CR4.SMEP` was set,
  and two existing port tests could not go red for the defects they exist to catch because a wrongly
  permitted `out` hung the run instead of failing it.
- **design/audit-reports/2026-09-24-new-trust-boundaries.md**: what the tree began trusting in one
  week, read where the change concentrated: the automation that merges, the bytes a file supplies,
  and the state a core carries for a thread. **The finding to carry off is not in the kernel**: the
  merge drain armed auto-merge on any green pull request from anyone, the ruleset requires no
  review, and a merge-group build runs a pull request's workflow edits with the repository's
  secrets. Closed in the script; the ruleset is a proposal. In the kernel, a port take-back on
  `x86_64` reset the invoker's own bitmap (fails closed, fixed), the SVE/SME and V enables are now
  closed beside the FP one, and a device tree's `totalsize` is bounded before it becomes a slice.
- **design/audit-reports/2026-09-29-syscall-surface-whole.md**: the `invoke` surface entire, read
  from inside a confined process: the two object types, the rendezvous method and the wake tag the
  window added, beside the older seams. Nothing lets a confined process take authority it was not
  granted; three recorded risks confirmed where they are stated.
- **design/audit-reports/2026-10-03-eight-constants-and-thirteen-components.md**: the window's
  eight new ABI constants (`MemoryRegion::USAGE` and its seven selectors), the one that changed
  (the capability table grew to 32) and the thirteen components, each read for what it holds
  against what it uses. **The finding to carry off is a grant, not a kernel defect**: a graphical
  terminal session on the no-keyboard arm holds the boot line discipline's whole endpoint, and the
  discipline does not ask who is holding it, so a compromised session can queue the boot shell's
  next command line. Also: the plain-`SEND` badge is on the wire but not in §230 (badged endpoint
  capabilities), and a `RECEIVE_CAP`
  entry in `crates/abi` still said a fixed escape was open. Its follow-up added two more worth
  knowing: the boot shell keeps the seven display capabilities with `GRANT` for its whole life,
  and a real capability delegated to a `CALL` server passes the `NO_CAP` guard, so any client can
  park `net_stack` or the compositor in `SEND` forever (recorded, proposed, calef's call).
- **design/audit-reports/2026-10-19-supply-chain-read-from-the-package-inward.md**: the nine
  external packages the window added, the gate that should see every graph they enter, and the path
  by which bytes this tree did not build become authority on the target. **The finding to carry off
  is a gate that read a typed list**: cargo-deny never ran over `jig`, the package manager, or
  `c_library`, which failed it. Fixed by discovering every lockfile. Also: on `main` a vouch still
  endows a native package with the network its note asks for, and the index copy is a vouch, so the
  index's host decides; milestone 892 (a std program at the prompt holds the network and a name resolver) refuses it.

The machine-checked half is `script/verify` (Kani harnesses over the capability model, IPC, the MMU
invariants, the DMA validator). notes/verification.md states what each proof covers and, more
usefully here, what it does not: a proof holds inside chosen bounds, and DECISIONS §35 spells out
one gap the tools cannot close.

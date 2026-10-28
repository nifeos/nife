---
status: NOT-STARTED
raised: 2026-09-21
promoted_from: the-disk-an-installer-names-has-no-model
milestone_dependencies: none
decision_dependencies: 193
machine_requirements: none
specific_machine: none
needs_person: no
---
# 569. The disk an installer names has no model, only a size

The number is **provisional**: the integrator mints it at merge. Promoted from the proposal `the-disk-an-installer-names-has-no-model` on 2026-09-22, filed 2026-09-21. Raised by the rung 2a lane of milestone 198 (a package manager, and
the trivial install that makes a second customer possible) while building the install offer. It was
filed rather than built because the half that a *program* needs is a wire shape, and that is not a
lane's to mint.

Only for the second half. The kernel-side half below needs no ruling and could
start today. What waits on calef is whether a process that is not the kernel may be told a
controller's identity. That is the same question milestone 421 (the block roster cannot name an
NVMe disk) is already held on.

## The sentence this is about

`kernel/src/user/install_service.rs` prints this before it wipes a disk:

```text
  install     :   TARGET: the NVMe disk attached to this machine, 1073741824 bytes.
  install     :   EVERYTHING ON THAT DISK WILL BE DESTROYED.
```

That is the single most load-bearing sentence a stranger reads in this system. Everything else
the installer does is recoverable by reinstalling; this one is not. The whole safety property of
the install is that a person read it and agreed. Milestone 515 (a stick that puts itself on the
machine's disk)'s own `BUGS` says the confirmation *"must name the disk by model and size, not by an
ordinal"*. Today it names the size and hedges the rest: "the NVMe disk attached to this
machine", because on a machine with two it could not say which.

## Why it says that and not more

Not for want of the data. NVMe's `IDENTIFY` with `CNS = 1` returns the controller structure. Its
first sixty bytes are ASCII: a 20-byte serial number, a 40-byte model number, and an 8-byte firmware
revision. `crates/non_volatile_memory_express` already declares `CNS_CONTROLLER`, and nothing in
this tree has ever issued that command. The one `Command::identify` call site
(`kernel/src/non_volatile_memory_express.rs`) asks for `CNS_NAMESPACE`, because geometry is all the
driver needed.

The limit that is real, and worth stating precisely because it is easy to overclaim, is on the
data plane. `non_volatile_memory_express::Handoff` is what the admin plane tells the confined
server at spawn. Its own header says why it is shaped as it is: *"Three `u64`s because a spawn
carries three scalars"*, which is `kernel/src/user.rs`'s `Spawn`. All three are spent:

| word | carries |
|---|---|
| 0 | `dstrd`, `blocks_per` and `entries`, packed |
| 1 | `data_plane_phys` |
| 2 | `size_bytes` |

Forty ASCII bytes do not fit in a spawn. So a program cannot be told the model the way it is told
the geometry. That is the sentence the rung 2a lane wrote down, and it is true. What it does not
say is that the kernel does not need the spawn at all.

## Two halves, and only one of them is an architect's

A. The kernel prints the model. Reversible, needs no ruling, and is the whole of the sentence
above. The offer is printed by `install_service`, in the kernel, which is also where the admin
plane lives. It takes a second `IDENTIFY` into the DMA region the admin queue already owns and a
`[u8; 40]` on the kernel-side `Wiring` (a Rust struct, not a wire). Then the sentence names the
drive. Cost: one more admin command at bring-up, roughly thirty lines, and a parser for a
fixed-offset ASCII field that is space-padded rather than NUL-terminated.

**It also does not solve the two-disk case**, and saying so is the point. Naming *a* model is not
naming *which*. A machine with two NVMe controllers needs an identity a person can match against
the thing in their hand. A serial number is that; a model number on its own is not.

B. A program names the disk. This is the wire question and it is an architect's. `disk_surveyor`
is what would list the machine's drives for a person to choose between. It cannot see an NVMe
controller at all: `crates/block_roster` encodes two transport kinds and neither is NVMe. That is
milestone 421 (the block roster cannot name an NVMe disk) exactly, held since 2026-09-19 on
DECISIONS §193 (NVMe in the block roster). Its open question is the transport kind's spelling
**and whether an NVMe entry carries anything a virtio one does not**. A controller identity is that
"anything". So this proposal does not add a fork. It adds a consumer to one already open, and the
consumer is the sentence at the top of this file.

## What it would settle

Whether a stranger who is about to lose a disk is told which disk. Today the answer is "the one in
this machine". That is true on xenon and on every machine this has been run on, and is a guess on
the first machine with two.

## BUGS

- **This proposal does not price option B**, because its cost is whatever §193 rules the roster entry
  to be. A price written before that ruling would be a price for a shape nobody has chosen.
- A model number is not an identity. Two identical drives in one machine report the same model,
  and the serial is what separates them. Option A as written would print the model and could print
  the serial as easily. What it cannot do is let a person *choose*, which is option B.
- Nothing here helps a SATA disk, which has no driver at all (milestone 515's `BUGS`), or an
  NVMe controller hidden behind Intel RST or VMD.

## Index row

The installer prints one sentence before it destroys a disk. It is the most load-bearing sentence a stranger reads in this system: everything else the install does is recoverable by reinstalling. Today it names a size and hedges the rest, calling the target "the NVMe disk attached to this machine", because on a machine with two it could not say which. Milestone 515's own `BUGS` already says the confirmation must name the disk by model and size rather than by an ordinal. Whether a process that is not the kernel may be told a controller's identity is the half that waits on a ruling.

---
status: DECIDED
raised: 2026-10-12
decided: 2026-10-12
ratified_by: calef
---

# 297. Software reaches a nife system as a package `jig` installs from basalt's index, the kernel included

*The title and the file's slug are drafts, and neither is calef's.*

## The ruling

calef, 2026-10-12 (UTC), first on the monit milestone:

> That milestone should require it as a package delivered via jig through basalt.

Then, on every other program:

> They should all be installed via jig through basalt. I missed that for our benchmarking tools, so
> I created debt by prioritizing them first.

Then, on the plan this section, its amendments and its new milestones carry out: *"Launch it."*

And on its scope, the same day:

> §297 is not about foreign software. It is about all software.

And then added the kernel:

> We want to be able to update the kernel through packages.

Asked how the programs that run before `jig` reach a system, he answered the same day:

> The image should be composed from basalts packages.

## What it commits the project to

1. Every program reaches a nife system as a package that `jig` installs from basalt's package
   index. That holds for ported C, for Rust taken from crates.io, for benchmarks and tools, and for
   what this tree writes. It holds for the kernel too.
2. basalt builds and publishes every package. That includes first-party ones: swish, process-tools,
   kilo, and the `tar` milestone 258 writes. The recipe, the pinned source and the published
   archive live in `nifeos/basalt`. That is what basalt is for (§247 (the split begins with basalt
   holding nife, and `procps` moves first)).
3. A `helpers/build-*.sh` script, or an image that packs a program by hand, is interim scaffolding.
   It may keep feeding nife's tests until the program's package exists. No milestone is done on
   one: its exit names the program installed with `jig` from basalt.
4. The kernel is a package too. Installing a new kernel package is how a system updates its kernel,
   and the new kernel takes effect at the next boot. A boot that fails returns to the previous
   kernel.
5. A nife image is composed from basalt's packages. That includes the kernel, the progenitor, the
   shell, `jig` and the network stack. The programs needed before `jig` runs reach the system as
   packages placed in the image, not as binaries this tree built.
6. A milestone that needs a program depends on milestone 809 (the package client becomes a
   program, `jig`, with the verbs an index needs). It also depends on milestone 801 (packages over
   the internet: a host name, HTTPS and the distribution's index). Where a milestone packages that
   program in basalt, it depends on that one too.

## The path that already exists

This does not start from nothing. Three records already plan most of it.

- Milestone 607 (nife is built and updated from packages, and the repository divides along what
  releases together) is the path for the OS itself. Its item 1 assembles the image from base
  packages by digest. Its U4 ruling makes packages the update unit, with a full copy in each slot.
- Milestone 611 (every program and crate belongs to a package, and every package has a home)
  draws the package boundaries in this tree. Under this ruling, basalt is where those packages are
  built and published.
- §208 (installing a package is granting it, and the activation set is versioned) gives the
  generations that `jig rollback` moves between.
- Milestone 858 (lab machines update themselves through packages, and only a new kernel reboots
  them) already plans a kernel package. Its fork 5 keeps kernel and base as separate packages.
  Its correction puts the kernel in a boot slot that updates with a reboot and rolls back as
  built. Its worklist names the slot writer and the riscv64 and aarch64 chooser.
- Milestone 525 (a bad upgrade cannot brick the machine: two boot slots, tries and priority) built
  slots and tries on x86_64. Milestone 554 (a good upgrade sticks: what marks a trial boot
  successful) built the confirmation.
- §159 (lab machines upgrade like user machines, and only a new kernel needs a reboot) and §235 (the
  OS is built and updated from packages, and the tree divides by what releases together) are the
  rulings under all of them.

## What is missing for the kernel

- No milestone owns the whole path under QEMU on three architectures. Milestone 858 is the lab's,
  on silicon with a person present. This section mints milestone 908 (the kernel is a package:
  `jig` installs it, the next boot runs it, and a failed boot rolls back) for it.
- Where the loader finds the active kernel. On x86_64, `uefi_loader` reads slot state from GPT
  attribute bits (525). Nothing writes a slot from an installed package yet.
- Rollback after a failed boot exists only on x86_64. The chooser does not exist on aarch64 or
  riscv64. Milestone 568 (the boot file has nowhere to go on a device-tree machine) blocks it.
- Each architecture's boot path differs. x86_64 boots through UEFI firmware. aarch64 can boot
  through UEFI too. riscv64 boards boot through OpenSBI and U-Boot, which runs `uefi_loader` with
  `bootefi` (858's fork 7).

## The bootstrap, answered

The programs a system needs before `jig` can run reach it in the image. calef ruled that the image
is composed from basalt's packages, so they are packages too, placed rather than installed.

Milestone 607 already planned the shape: its item 1 assembles the image from base packages by
digest, on the host. It did not say who assembles it. Under this ruling basalt does, from the
packages it builds and publishes. Milestone 755 (basalt v0 pins nife and runs its gate) is where
basalt stands today: it pins a nife commit and runs that commit's own image build and tests. No
milestone owned the step between, so this section mints milestone 909 (basalt composes the
bootable image from its packages). After the first boot, `jig` updates those same packages
(milestone 809's amendment of 2026-10-07).

## How it sits with the decisions before it

- §135 (running GPL software is aggregation, the capability boundary is what makes it so, and
  packages are how it arrives) already said a foreign program arrives as a package. This extends
  that to every program and names basalt as the source.
- §262 (nife is measured with the field's standard benchmarks, run unmodified, against Linux)
  requires the upstream source at a pinned version. A basalt recipe is where that pin now lives.
- §195 (a reviewed recipe vouches for a package, and the machine's owner may overrule it) and §197
  (a package is one archive file, named and vouched for by its recipe) are the package's format.
- §151 (the goal of the repository split is independent release and third-party programs) is
  served by it: a program basalt builds can release on its own.
- §46 (thin primitives or whole subsystems; we write everything in between) keeps fetching a
  foreign program out of nife's gates. Under this ruling the fetch moves to basalt.
- §270 (a package manager holds an installer endpoint, not the spawn endpoint) settled what `jig`
  holds, and §250 (an image names its distribution's package index, at `basalt.nifeos.org`, and a
  package's bytes may live anywhere) settled where packages live. Nothing here changes either.

## The debt it names

calef named the cost himself: the benchmark milestones were ranked ahead of the packaging path, so
the helpers they run on are debt. Four milestones pay it:

- milestone 810 (`ripgrep` is packaged in basalt and installed with `jig`);
- milestone 897 (SQLite is packaged in basalt, and its tests run in basalt's package build);
- milestone 906 (ioping is packaged in basalt, and its test installs it with `jig`), minted here;
- milestone 907 (hackbench is packaged in basalt, and its test installs it with `jig`), minted here.

## What it covers today

These blocks were amended on 2026-10-12 (UTC). Each one's `## Done when` now names the program
installed with `jig` from basalt, and each depends on 809 and 801.

The §262 benchmarks:

- milestone 826 (an lmbench subset on nife and Linux, the real programs, not the same idea);
- milestone 827 (hackbench on nife and Linux, as a scheduler benchmark and a multicore soak);
- milestone 828 (iperf3 on nife and Linux, throughput for the network drivers);
- milestone 829 (netperf TCP_RR on nife and Linux, request and response latency);
- milestone 831 (SQLite's speedtest1 on nife and Linux, a real database workload);
- milestone 832 (STREAM on nife and Linux, the memory-bandwidth baseline);
- milestone 833 (the same storage benchmark on nife and Linux, by porting real fio);
- milestone 834 (ioping on nife and Linux, storage latency one request at a time).

Tools and applications:

- milestone 99 (`git` on nife: the tool this project is built with, hosting its own history);
- milestone 171 (`git` core plumbing: version control on a nife host with no new primitive);
- milestone 170 (`nano`: a real, full-featured screen editor on the primitive milestone 169 builds);
- milestone 258 (archive and compression: `tar` and zlib, because `git` cannot read its own
  objects without them), whose `tar` is first-party;
- milestone 710 (porting Firefox to nife);
- milestone 66 (Vaultwarden: somebody else's real application, running here);
- milestone 123 (the demonstration: somebody else's software, running narrow, and a gate that keeps
  it there);
- milestone 149 (a common HPC benchmark suite, in Rust, on nife and on Linux);
- milestone 173 (`rustc`/`cargo`/LLVM natively on nife: full local self-hosting).

The 169 in 170's title is milestone 169 (`kilo`: the smallest real text editor, as the forcing
function for raw terminal input).

Two more fall under this ruling without an amendment here. They are milestone 830 (Wakeup latency
on nife and Linux, from each kernel's own scheduler trace) and milestone 905 (monit is packaged in
basalt, installed with `jig`, and restarts a nife service it watches).

The first-party programs already shipping (swish, process-tools, kilo and the rest) are covered
through milestones 607 and 611 above rather than amended one by one. Milestone 121 (`ripgrep` on
nife: enumeration as a capability, and what the walk costs) is `PARTIAL`, and 810 carries its
packaging.

## BUGS

- Nothing enforces it. A gate that refused a `Done when` naming no `jig install` would read prose,
  and `script/roadmap --check` reads fields. A dependency on 809 is the nearest field, and nothing
  checks it is there.
- basalt builds no package yet. Until milestone 810 lands the first recipe and build, every
  milestone this ruling amended waits on work nobody has finished.
- The number of helpers will grow before it shrinks, because a port's first run still needs one.
- Until milestone 909 lands, every image is still built in this tree from binaries it compiled.

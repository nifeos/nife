---
status: PARTIAL
raised: 2026-10-29
---
# 5. Crates leave nife and are consumed back by version

*The slug is this lane's coinage and provisional; calef names things.*

calef asked for this journey on 2026-10-29 (UTC): *"Lets create that journey. I want to track that
as a unit and use it to direct AI development of many milestones."* The subject was a journey for
the crates, parallel to journey 4's package journey. He scoped it the same day: *"We cannot control
adoption. We can control the crates we externalize."* So this file tracks externalization, and it
holds only what nife controls.
[Milestone 857 (crates released on their own)](../roadmap/0857-crates-released-on-their-own.md)
holds the program: each publish-ours crate publishes when ready (proofs passing, API stable),
through its own milestone at its review (calef, 2026-10-07 (UTC)); one repository per crate under
`nifeos`; and the release order ruled 2026-10-27 (UTC). The story, per crate: nife builds and
proves it in this tree, moves it to a repository of its own, soaks it there, publishes it to
crates.io, and consumes it back by semantic version, as any outside project does: `Cargo.toml`
holds a version requirement, not a git revision, and `Cargo.lock` records the checksum. A crate
nife does not publish, or has not yet, is a git dependency pinned by revision with its version
requirement stated, such as `{ git = "https://github.com/nifeos/<crate>", rev = "<commit>", version
= "0.1" }` (ruled 2026-11-01 (UTC)). The revision is the commit of a tagged release whose manifest
carries that version; the tag is a label and the semver baseline (`--baseline-rev` against the
previous tag), never what Cargo follows. At first publication the line becomes a plain version
requirement. Each publication, and each repository a move creates, waits for the architect's
review of the exact crate, version and name, and a contract crate may never publish. How a
consumer's revision follows a producer's release is a
[proposal](../roadmap/1011-a-producer-release-moves-each-consumers-pin.md). The program serves
[risk 8](../fatal-risks/8-nobody-else-builds-on-nife.md): nife shrinks by pushing its mechanisms
out into the Rust ecosystem (calef, 2026-10-18 (UTC): *"I have no desire to maintain 50 crates.
Rather I desire 50 crates that I would confidently use to build a nife."*). A lane reads the next
unbuilt step from the table, the way journey 4 directs the package work.

The journey covers every crate that something outside nife could consume, by the ruling of
2026-11-01 (UTC) that milestone 857 records; the maintainer confirms this widening of the journey.
The end state, ruled the same day: this repository breaks up into single-purpose repositories and
remains, as `nifeos/nife`, only for its history, and the kernel, programs, tooling and crates all leave, bottom-up.
The kernel leaves last among the code, to `nife-kernel` (provisional), a named part of the system
as Linux is, while the project name stays with the whole. The design record (roadmap, decisions,
journeys, naming, notes) moves to one repository named `nifeos.org`, ruled 2026-11-01 (UTC), which
holds the whole site published at nifeos.org. Its creation still needs the architect's approval.
This journey tracks the crates.
Its steps are of two kinds. A publish-ours crate publishes to crates.io, as the table below lays
out. A contract crate moves to a repository of its own and carries a version whether or not it
publishes. Every move goes bottom-up: a crate that leaves references nothing back in this tree, so
the crates a program links leave before it or with it, and step 9's `jh7110_entropy` is the leaf
pilot of the mechanics. Before each move the architect reads the crate's code by hand and reviews
it (§247 (the split begins with basalt holding nife, and `procps` moves first)'s condition 4, ruled 2026-11-01 (UTC)), initially for every crate. A crate also needs a
tagged release whose commit carries its Cargo version, and no time-based measure remains. The first
crate reviewed is the pilot, `jh7110_entropy`, which moved before the ruling and now carries
the tags `v0.1.0`, `v0.1.1` and `v0.2.0` (step 9). Pull request #3026 rules the contract-crate order (2026-11-01 (UTC)): one
repository per crate, each made with the architect's approval, and a package leaves as soon as its
own crates have left. The other contract crates' steps join the table when their lanes are cut.

| step | milestone | decision | what this step needs |
|---|---|---|---|
| 1 | 887 | | the machinery every crate repository calls from its move onward, built: the shared template's nine ruled checks, the allowlist lint that keeps `publish = false` the default, the `crate-maintainers` team as the crates.io owner, and trusted publishing, with every release waiting in the `crates-io` environment for calef. The template also holds the scoped-token first publish and the semver baseline against the newest release tag. Step 9's exit still owes a release dry run that passes |
| 2 | 813 | | Next: move PR pending tag. `file_allocation_table` has its write half built, stages 6 to 9 (allocation and write, directories and rename, the general FAT12/16/32 format, timestamps and the label), with 31 harnesses. `nifeos/file_allocation_table` exists at version 0.1.0 with `publish = false`, green checks and the current template pin, and carries no tag. This tree keeps its copy and nife depends on it by path; the move pull request removes the copy and pins the tagged commit, so it waits for the tag, which calef has not cleared. Then the soak, the publication (on hold) and nife consuming the release by semantic version. Milestone 140 (mount a drive this system did not create) needs it for its FAT32 stratum |
| 3 | 820 | | Next: tag pending calef. `extensible_host_controller_interface`, released with `usb`, which is published as `universal_serial_bus` (the name `usb` is taken on crates.io, and the crate keeps `usb` as a documentation alias for search). `nifeos/extensible_host_controller_interface` exists at version 0.1.0 with `publish = false` and no tag; nife has left the path form and pins its commit. Its seven harnesses prove (the producer ring, the event ring, the capability walk, the device context index, the slot and endpoint contexts, the port status write and the interrupt interval), and the architect has cleared its review. After the tag: the soak, the publication (on hold) and nife consuming the release |
| 4 | [1033](../roadmap/1033-the-generational-table-crate-released-on-its-own.md) | | Next: tag pending calef. [`generational_table`](../roadmap/1033-the-generational-table-crate-released-on-its-own.md), published as the fixed-capacity, proved slotmap. `nifeos/generational_table` exists at version 0.1.0 with `publish = false` and no tag; nife pins its commit. Its release shape is done (its own README and license files), its five harnesses prove, the architect has cleared its review, and [the record](../../notes/generational-table-and-existing-crates.md) answers that `slotmap` does not cover allocation-free fixed capacity. After the tag: the soak, the publication (on hold) and the consumption |
| 5 | 814 | | Next: tag pending calef. `non_volatile_memory_express`: class A, so no cut, and its process plan already left for `non_volatile_memory_express_contract` (#2736). `nifeos/non_volatile_memory_express` exists at version 0.1.0 with `publish = false` and no tag; nife pins its commit. Its eight harnesses prove, and the architect has cleared its review. After the tag: the soak, the publication (on hold) and the consumption |
| 6 | 815 | | Next: tag pending calef. `e1000e`: its cut is done (#2719) and its three harnesses prove (the handoff round trip is a harness of `driver_process_contract`). `nifeos/e1000e` exists at version 0.1.0 with `publish = false` and no tag; nife pins its commit, and the architect has cleared its review. After the tag: the soak, the publication (on hold) and the consumption |
| 7 | 816 | | Next: repository pending the architect. `designware_ethernet`: its cut is done (#2719), with the walk in `machine_discovery::jh7110`, and its six harnesses prove, the PHY link decode's among them. Every name in the crate carries a ratified block. Its review package holds one open item, item 9 of milestone 1019 (the driver crates carry facts from their sources, not their text), a source claim about which sources name the YT8531 registers. No repository exists yet and the crate is in this tree; the move waits on the architect's review clearance, then the tag, the soak, the publication (on hold) and the consumption |
| 8 | 817 | | Next: tag pending calef. `designware_mobile_storage`: its cut is done (#2719), with `serve` moved to `driver_process_contract`; the MBR parse and the capacity decoders have their harnesses and falsification patches (eight in the crate). Its register, constant and bit-overlap names are ratified and spelled out, and the renames have landed (#3311). `nifeos/designware_mobile_storage` exists at version 0.1.0 with `publish = false` and no tag; nife pins its commit, and the architect has cleared its review. After the tag: the soak, the publication (on hold) and the consumption |
| 9 | 818 | | Next: soak exit pending, a release dry run that passes. `jh7110_entropy`, the soak pilot: it moves and soaks before the other soaks start, rehearsing the mechanics every step here runs, and the rehearsal moves no publication; its four harnesses prove. It lives at `nifeos/jh7110_entropy`, the only crate repository with tags (`v0.1.0`, `v0.1.1` and `v0.2.0`), with the current template pin and green checks on `main`. nife links it in this form: its `components`, `machine_discovery` and `kernel` manifests pin the commit of `v0.2.0` and state `version = "0.2.0"`, with `publish = false` in the crate. The architect cleared its review, the first crate reviewed. Which version its README may say ran on a VisionFive 2 is milestone 1035 (a crate names the version that ran on hardware): the README names a version only once nife, built at the pin that names it, has served TRNG output on the board. Its publication sits with the device set and is on hold |
| 10 | 819 | | Next: repository pending the architect. `jh7110_clock_and_reset`: its cut is done and its name ratified, and it holds nine harnesses, each with a falsification patch (milestone 819 (JH7110 clock and reset logic, proven, then released), #3070). One name, `CLOCK_PARENT_MASK`, waits on the architect, and its review package lists four decisions. No repository exists yet and the crate is in this tree; the move waits on those rulings and the review clearance, then the tag, the soak, the publication (on hold) and the consumption |
| 11 | [1059](../roadmap/1059-block-roster-contract-outside-review.md) | | Next: outside-review findings pending (milestone 1059 (`block_roster_contract`: the outside review's findings land before its first release)). `block_roster_contract` is a contract crate, which never publishes. `nifeos/block_roster_contract` exists at version 0.1.0 with no tag; its identity is frozen (id 5, version 1, magic `blkrosv1`) and the architect has cleared its review. This tree keeps its copy and nife depends on it by path. After the findings land: the tag, which calef has not cleared, then the move pull request that pins the tagged commit |

Two orderings govern the program, and they order different events. The soak order sits in the
per-crate blocks: `jh7110_entropy`, milestone 818 (proven JH7110 TRNG logic, released on its own),
is the pilot and soaks first, because it is the
smallest crate with a real cut and the cut is where refactor lessons show up. The publication order
calef ruled on 2026-10-27 (UTC), in
[milestone 857](../roadmap/0857-crates-released-on-their-own.md) and again as risk 8's first
attempts: `file_allocation_table` first, `extensible_host_controller_interface` with `usb` (published as `universal_serial_bus`) second,
`generational_table` third, the device set after their cuts. The pilot's soak rehearses the
mechanics and moves no publication: a soak that ends does not publish by itself, so each crate's
publication waits for its own soak's five end conditions and its turn in the ruled order. So
`jh7110_entropy`'s publication sits with the device set (step 9), and the first publication is the
FAT crate's.

The publish-ours steps hold only publish dispositions. The survey's contribute rows (paging, the GPT parser,
`elf`, `network_time_protocol`, `device_tree_blob`, `glob`, `domain_name_system`, `calendar`) wait,
by the ruling of 2026-10-27 (UTC), until a published crate has users who ask for them upstream, so
nife controls no upstream step for them today. That wait covers the offer only. Ruled 2026-11-01
(UTC): the four crates among the 22 move in pull request #3026's order, each to a repository of its
own, and a later offer comes from that repository. No crate stays in this tree by default
(ruled 2026-11-01 (UTC); 857's item 7 is withdrawn). calef refused milestone 821 (`portable_executable`: an
ELF-to-PE converter, proven, then released on its own) on 2026-10-21 (UTC): its input is nife's own
build product, so a published proof protects no known user. The crate no longer exists, and the
conversion lives in `xtask` (the survey's corrections, 2026-10-27 (UTC)). No step here expects any
of them. The soak's own fourth condition,
the stranger test (the crate builds and proves from its README alone), is controlled work and
stays; what nobody controls is who, if anyone, depends on the result.

## What "done" looks like

Every step above at `BUILT`. At that point, every crate the ruled disposition calls ours has
proved what its block requires, moved to its own `nifeos` repository, cleared its soak's five end
conditions, and published through trusted publishing with calef's approval; nife consumes each
release from crates.io by semantic version, with its checksum in `Cargo.lock`. Each contract crate's step, once
pull request #3026 adds them, is done when the crate has moved and carries a version. That is the
whole completion bar, and
it holds only work nife controls. Adoption is out of scope on purpose: nife does not control it, so
no step tracks it and it is no criterion here; [risk 8](../fatal-risks/8-nobody-else-builds-on-nife.md)
measures it separately, with its five observations and thresholds, and publishing alone counts as
none of the five.

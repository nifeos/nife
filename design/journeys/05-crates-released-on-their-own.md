# 5. Externalize the publish-ours crates, one release at a time

*The slug is this lane's coinage and provisional; calef names things.*

calef asked for this journey on 2026-10-29 (UTC): *"Lets create that journey. I want to track that
as a unit and use it to direct AI development of many milestones."* The subject was a journey for
the crates, parallel to journey 4's package journey. He scoped it the same day: *"We cannot control
adoption. We can control the crates we externalize."* So this file tracks externalization, and it
holds only what nife controls. [Milestone 857](../roadmap/0857-crates-released-on-their-own.md)
holds the program: each publish-ours crate publishes when ready (proofs passing, API stable),
through its own milestone at its review (calef, 2026-10-07 (UTC)); one repository per crate under
`nifeos`; and the release order ruled 2026-10-27 (UTC). The story, per crate: nife builds and
proves it in this tree, moves it to a repository of its own, soaks it there, publishes it to
crates.io, and consumes it back by checksum in `Cargo.lock`. The program serves
[risk 8](../fatal-risks/8-nobody-else-builds-on-nife.md): nife shrinks by pushing its mechanisms
out into the Rust ecosystem (calef, 2026-10-18 (UTC): *"I have no desire to maintain 50 crates.
Rather I desire 50 crates that I would confidently use to build a nife."*). A lane reads the next
unbuilt step from the table, the way journey 4 directs the package work.

| step | milestone | decision | what this step needs |
|---|---|---|---|
| 1 | 887 | | the machinery every crate repository calls from its move onward: the shared template's nine ruled checks, the allowlist lint that keeps `publish = false` the default, the `crate-maintainers` team as the crates.io owner, and trusted publishing, with every release waiting in the `crates-io` environment for calef. The template's two remaining checks (the scoped-token first publish and the semver baseline) wait at step 9 |
| 2 | 813 | | the first publication in the ruled order: `file_allocation_table` finishes the write half, stages 6 to 9 (allocation and write, directories and rename, the general FAT12/16/32 format, timestamps and the label), then the move, the soak, the 0.1.0 publication, and nife consuming the release by checksum in `Cargo.lock`. Milestone 140's FAT32 stratum depends on it |
| 3 | 820 | | the second publication in the ruled order: `extensible_host_controller_interface`, released with `usb`, once the crate gains the two harnesses its table still marks `none yet` (the event ring, the capability walk), then the move, the soak, the publication and nife consuming the release |
| 4 | 857 | | the third publication in the ruled order, `generational_table`: no milestone owns it yet, and fork 7 mints one at its review with calef, quoting his ruling and its disposition; before release, the record says whether `slotmap` already covers allocation-free fixed capacity |
| 5 | 814 | | `non_volatile_memory_express`: class A, so no cut, and its process plan already left for `non_volatile_memory_express_contract` (#2736); its eight harnesses prove, so the step is the move, the soak, the publication and nife consuming the release |
| 6 | 815 | | `e1000e`: its cut landed 2026-10-25 (#2719) and its four harnesses prove; the step is the move, the soak, the publication and the consumption |
| 7 | 816 | | `designware_ethernet`: its cut landed 2026-10-25 (#2719), with the walk in `machine_discovery::jh7110`, and its two harnesses prove; the PHY link decode still needs its harness row, then the move, the soak, the publication and the consumption |
| 8 | 817 | | `designware_mobile_storage`: its cut landed 2026-10-25 (#2719), with `serve` moved to `driver_process_contract`; the MBR parse and the capacity decoders still need their harness rows, then the move, the soak, the publication and the consumption |
| 9 | 818 | | `jh7110_entropy`, the soak pilot: it moves and soaks before the other soaks start, rehearsing the mechanics every step here runs, and the rehearsal moves no publication; its four harnesses prove, so the step starts at the move, and its exit carries the two template checks step 1 names |
| 10 | 819 | | `jh7110_clock_and_reset`: its cut is done and its name ratified, and it holds no harness yet, so the harnesses and their falsification patches come first; then the move, the soak, the publication and the consumption |

Two orderings govern the program, and they order different events. The soak order sits in the
per-crate blocks: `jh7110_entropy` (milestone 818) is the pilot and soaks first, because it is the
smallest crate with a real cut and the cut is where refactor lessons show up. The publication order
calef ruled on 2026-10-27 (UTC), in
[milestone 857](../roadmap/0857-crates-released-on-their-own.md) and again as risk 8's first
attempts: `file_allocation_table` first, `extensible_host_controller_interface` with `usb` second,
`generational_table` third, the device set after their cuts. The pilot's soak rehearses the
mechanics and moves no publication: a soak that ends does not publish by itself, so each crate's
publication waits for its own soak's five end conditions and its turn in the ruled order. So
`jh7110_entropy`'s publication sits with the device set (step 9), and the first publication is the
FAT crate's.

The path holds only publish dispositions. The survey's contribute rows (paging, the GPT parser,
`elf`, `network_time_protocol`, `device_tree_blob`, `glob`, `domain_name_system`, `calendar`) wait,
by the ruling of 2026-10-27 (UTC), until a published crate has users who ask for them upstream, so
nife controls no externalization step for them today. The internal set stays in this tree, and its
reviews close as one sitting (857's item 7). calef refused `portable_executable` (milestone 821,
ruled 2026-10-21 (UTC): its input is nife's own build product, so a published proof protects no
known user), and it is no longer a crate; the conversion lives in `xtask` (the survey's
corrections, 2026-10-27 (UTC)). No step here expects any of them. The soak's own fourth condition,
the stranger test (the crate builds and proves from its README alone), is controlled work and
stays; what nobody controls is who, if anyone, depends on the result.

`generational_table` has no milestone of its own: fork 7 mints one at its review with calef, so
step 4 reads [milestone 857](../roadmap/0857-crates-released-on-their-own.md) until then, and its
row names the minting, not a number.

## What "done" looks like

Every step above at `BUILT`. At that point, every crate the ruled disposition calls ours has
proved what its block requires, moved to its own `nifeos` repository, cleared its soak's five end
conditions, and published through trusted publishing with calef's approval; nife consumes each
release from crates.io, pinned by checksum in `Cargo.lock`. That is the whole completion bar, and
it holds only work nife controls. Adoption is out of scope on purpose: nife does not control it, so
no step tracks it and it is no criterion here; [risk 8](../fatal-risks/8-nobody-else-builds-on-nife.md)
measures it separately, with its five observations and thresholds, and publishing alone counts as
none of the five.

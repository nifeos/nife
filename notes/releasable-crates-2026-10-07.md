# Crates that could be released on their own, and who else implements them, 2026-10-07 (provisional name)

Measured and looked up 2026-10-07 (UTC) at base `cd2f93de` by lane/releasable-crates. calef asked
it on ruling fork 2 of #1803 (nife writes its own FAT crate and releases it independently): *"I
suspect we have other such crates."* And then: *"assess for each of the releasable ones what the
other implementations are so that we could work with those owners to add the proofs we need rather
than jumping their missions."* The forks are in
[milestone 857 (crates released on their own)](../design/roadmap/0857-crates-released-on-their-own.md). The
appendices are in [releasable-crates-2026-10-07/](releasable-crates-2026-10-07/README.md).

The question, so the data has a reader: which crates in this tree are useful outside nife, and for
each, is there an existing project whose owners we should offer our proofs to instead of
publishing a rival?

## Result

- **30 of the 100 crates under `crates/` could leave**, plus `cryptography_provider` outside it. 23
  are standalone today (class A) and 7 are one small cut away (class B). Together they hold 28,334
  code lines and 135 of the 222 Kani harnesses under `crates/`. The other 70 are bound to nife's ABI, its
  kernel object model or its formats (class C).
- Every A and B crate builds on stable Rust 1.94.1, for the host and for
  `riscv64imac-unknown-none-elf`, measured. Nightly is this tree's need, not theirs. Every one is
  MIT OR Apache-2.0 and `no_std`, and every one has a consumer in this tree, which was milestone 39
  (repository structure)'s condition for publishing.
- **Most of them have a maintained counterpart, and those counterparts mostly carry no proofs.**
  Fuzzing is common; a model checker is rare. Two projects in the survey run Kani. rust-osdev's
  `x86_64` has a `kani-github-action` job in CI, and its paging harnesses took outside
  contributions in 2026. `hifitime` runs one weekly, on a different kind of time than `calendar`'s.
- **Where no maintained counterpart exists, it is device logic**: NVMe queues, the e1000e, the
  DesignWare controllers and the JH7110. The Rust drivers that exist are young, and none is a
  sans-IO layer that a proof can reach. That set, xHCI if rust-osdev declines, the FAT crate and
  `portable_executable` are the whole of "publish ours". The rest is contribute or internal.
- The B cuts are small, and four of them are codebase rule 2. The DesignWare and JH7110
  crates parse the device tree themselves; taking the register region from the caller removes the
  dependency and is what rule 2 (a driver gets what it needs passed in) already asks.
- **Contribution policies are the real constraint, not licenses.** `time` asks that every
  message to its maintainers be written by a person and that no LLM appear as a co-author.
  `jiff` and `globset` ask for human-written comments. rcore-os's `tgoskits` asks commits to carry
  no agent branding. This tree's rule is the opposite: an agent's pull request says so in its first
  line. calef ruled on 2026-10-07 that nife sends those projects no code or proofs, only disclosed bug reports.

## The rulings, 2026-10-07

calef ruled all eight forks of the proposal on 2026-10-07 (UTC); the proposal's "The rulings"
quotes each. For this table, three matter. Fork 7: each crate is reviewed with him one at a time,
FAT first, and the disposition column below is a recommendation until its review. A crate approved
to leave becomes its own milestone then; an internal one gets a line here. Fork 8: lanes send
disclosed code or proofs only where a project's policy admits AI work, and calef writes nothing by
hand. *"I think where AI is banned I just shouldn't contribute."* Its refinement: *"Bug reports are
fine."*, everywhere, disclosed, with security findings sent privately first. And each submission,
bug reports included, still comes to calef one at a time before it is sent: *"Let's start with
b."*

Projects in the survey that fork 8 closes to code and proofs, so bug reports only:

- Redox and embedded-sdmmc ban AI-generated contributions.
- `time` wants every message written by a person. `jiff`, `globset`, `rustls` and systemd want
  human-written comments or contributors who speak for themselves.
- rcore-os's `tgoskits` wants no agent branding in commits, so a disclosed offer cannot meet it.

Rows this changes: `calendar` (its target was `time`) and `redoxfs`, both marked "fork 8" below.
No other row targets one of these projects.

## The disposition, per crate

"Contribute" means offer our harnesses, and fixes they find, to the named project, one approach at
a time with calef's approval. "Publish" means release ours under Fork 1 of the proposal. "Internal"
means it stays a crate of this tree. Kani is the harness count; the survey appendix has the
evidence behind each row.

| crate | class | lines | Kani | disposition | why |
|---|---|---|---|---|---|
| `paging` | A | 2,464 | 37 | contribute: `x86_64` (rust-osdev), `aarch64-paging` (Google) | `x86_64` already runs Kani in CI; our x86_64 4-level harnesses fit it. The riscv Sv39 and Sv48 harnesses have no host |
| `elf` | A | 1,198 | 8 | contribute: `elf` (cole14) | Same scope, `no_std`, fuzzed in CI. Quiet for six months, so the offer may sit |
| `device_tree_blob` | A | 763 | 4 | contribute: `dtoolkit` (Google) | Young, fuzzed, CONTRIBUTING, our license. `fdt` is MPL-2.0 with one author |
| `domain_name_system` | A | 578 | 3 | contribute: `domain` (NLnet Labs) | Most outside merges of the DNS crates and a `no_std` path |
| `network_time_protocol` | A | 684 | 7 | contribute: `ntp-proto` (Pendulum) | Security-minded, fuzzed, takes outside work. `sntpc` is the `no_std` second choice |
| `calendar` | A | 1,091 | 11 | fork 8: no code or proofs to `time`; bug reports only. Revisit in its review | 951M downloads and no proofs, but `time` wants a person's words. `hifitime` runs Kani but is MPL-2.0 and another scope |
| `glob` | A | 650 | 6 | contribute: `wildmatch` | Pure matching, no IO, CONTRIBUTING. The large crates do IO |
| `globally_unique_identifier_partition_table`, `universally_unique_identifier` | A | 1,408 | 10 | contribute: `gpt-disk-rs` (Google), `uuid` (uuid-rs) | Google's crates are I/O-free and take outside work; `uuid` already runs fuzzing, AFL and Miri in CI |
| `non_volatile_memory_express` | A | 733 | 8 | publish | No maintained sans-IO NVMe crate; the drivers that exist are months old |
| `extensible_host_controller_interface` | A | 1,191 | 3 | ask rust-osdev first, else publish | `xhci` is the right scope and dormant since 2024-09 |
| `e1000e` | B | 2,151 | 4 | publish | `eth-intel` is a young driver in a monorepo; `e1000-driver` is GPL-2.0 |
| `designware_ethernet`, `designware_mobile_storage` | B | 5,158 | 6 | publish | One Rust DWMAC crate with 35 downloads; the DW MMC crates sit in `tgoskits` |
| `jh7110_entropy`, `jh7110_clock_and_reset` | B | 1,379 | 4 | publish with the DesignWare pair | The only Rust JH7110 crates are GPL-3.0 register maps |
| `pci` | A | 1,109 | 8 | internal for now | `pci_types` fits and merged nothing in six months |
| `usb` | A | 583 | 3 | internal | The maintained crates are the device side; ours is the host side and small |
| `page_frames`, `generational_table`, `intrusive_fifo` | A | 722 | 10 | internal | Shaped by this kernel; the general crates (`slotmap`, `intrusive-collections`) serve other missions well |
| `http_response` | A | 320 | 0 | internal | No proofs to offer; `httparse` is fuzzed and dominant |
| `address_space_identifier`, `cpu_set`, `bitmap_font`, `coremark` | A | 754 | 3 | internal | Too small or too specific to be worth a stranger's dependency |
| `file_allocation_table`, then the ruled FAT crate | A | 479 | 0 | publish, as ruled | rust-fatfs has not released since 2023-01 and has not answered a release request open since 2023-03. `simple-fatfs` is an alpha with no dependents. Offer rust-fatfs the bugs our oracle runs find, as issues |
| `portable_executable` | A | 778 | 0 | publish, after it carries proofs | No ELF-to-PE crate exists; systemd's `elf2efi.py` is the prior art |
| `video_terminal`, `line_editor` | B | 4,141 | 0 | internal | No proofs to offer. `vte`, `vt100` and `noline` serve the need |
| `cryptography_provider` | A | 610 | 0 | internal | Same mission as `rustls-rustcrypto`, which calef refused as a dependency. A second alpha provider helps nobody; offer it fixes when it moves |
| `redoxfs` (vendored, two patches) | n/a | n/a | n/a | fork 8: internal, patches carried; a bug report is allowed | Redox refuses LLM-generated contributions. §46's 2026-10-04 amendment says nothing is offered; whether a report of the broken `no_std` build goes is its review's question |

## Corrections, 2026-10-27 (UTC)

The survey above is left as taken. Read against the tree on 2026-10-27 (UTC) by lane
maintainer/risk-8-picks-2026-10-27, these rows are stale:

- `portable_executable` is no longer a crate. It is a module of `xtask` (`xtask/src/portable_executable.rs`),
  so it is not a release candidate.
- `universally_unique_identifier` is gone. The GPT crate depends on the external `uuid` crate (default
  features off) and owns only how a GPT stores an identifier. The GPT row no longer counts a second crate.
- `intrusive_fifo` is now `intrusive_queue`.
- `http_response` now depends on `httparse`, so it is no longer free of dependencies.
- `video_terminal` depends on `address_space_map` and `glyph_atlas`, and `line_editor` on
  `capability_plan`, so neither is class B (bound to a sibling crate) any more.
- `coremark` is taken on crates.io (created 2026-10-12, an unrelated port), so that name is not free.

The dispositions of the first three releases are ruled; see milestone 857's release order and
[risk 8](../design/fatal-risks/8-nobody-else-builds-on-nife.md) (2026-10-27 UTC). The ruling also defers
every "contribute" row above until a published crate has users who ask for it upstream, and
drops `non_volatile_memory_express` from the first attempts.

## Two records this overtakes

- Milestone 39 (repository structure) judged a `no_std`, I/O-free GPT parser a real gap, because
  crates.io's `gpt` uses `std::io`. Google's `gpt_disk_types` now fills it, MIT OR Apache-2.0, so
  the case for publishing ours became a case for offering our eight harnesses to Google's crate.
- §176 (offering the RedoxFS patches upstream) is still PROPOSED, and milestone 347 (offer the two
  RedoxFS patches upstream) is NOT-STARTED. §46's amendment of 2026-10-04 already answered both:
  Redox is a source we consume and never contribute to. Redox's own policy refuses LLM-generated
  work. The maintainer owes the record that closes them; a lane may not edit `design/decisions/`.

## Method, and how to re-take it

From the repository root:

```sh
python3 notes/releasable-crates-2026-10-07/inventory.py   # the class table, from cargo metadata
cargo +stable build -p <crate> --lib --target riscv64imac-unknown-none-elf --target-dir /tmp/rc
```

The inventory reads `cargo metadata --no-deps`, takes normal and build dependencies only (a dev
dependency does not ship), and counts what the appendix says it counts. The class is this lane's
judgment and lives in the script, so a re-run reproduces it and a disagreement is a one-line diff.
The stable builds used `rustc 1.94.1` and a target directory outside the tree, so the workspace
build was not touched. The survey was read from the crates.io and GitHub APIs; nothing was built.

## What was measured and what was looked up

Measured: the dependency graph, line, harness and falsification counts, the in-tree consumers, the
stable builds, and crates.io name availability. Looked up and not measured: every figure in the
survey appendix, including activity, licenses and contribution policies. Those age by the week.

## BUGS

- The A, B and C lines are judgment. "Useful outside nife" was decided by reading each crate's
  header, not by asking anyone outside nife.
- The "outside" counts in the survey cannot tell a first-time contributor from a regular, so they
  show whether a door is open, not how wide.
- Whether each counterpart's code is restructurable enough for Kani was not tried. §46 (thin
  primitives or whole subsystems)'s first rule says restructuring for a prover is why this tree
  writes its own. An offer may come back as "only if you rewrite our parser".

# Naming programs, scripts and directories

*An appendix to [`design/naming.md`](../naming.md), which is the rule. This file holds the argument
that a name is a claim, the component and program conventions, shell builtins, `script/` against
`helpers/`, and directories. It exists to verify or challenge the main page, and a reader who only
needs to name, ratify or rename something should not have to open it. The directory `design/naming/`
and this file's stem are provisional names;
naming is an architect's.*

## The rule everything else is a corollary of

A name is a claim, and it is made before a reader sees a line of code. That is the whole argument.
A wrong name is the same defect as a stale comment, except that a comment can be skipped and a name
cannot: every reader of every call site reads it.

Two ways a name can be a false claim, and the tree had both:

- It claims a model we rejected. `netd`, `compd`, `gpud`, `termd`. The `-d` suffix says "Unix
  daemon", and a daemon is defined by what it detaches from: no controlling terminal, inherited
  ambient authority, a pid file, started by a privileged init. This OS has none of those. `netd`
  held five explicit capabilities, could not name its own callers, and was supervised. It could be
  reaped by something that lacked the authority to build it. About as far from a daemon as a
  long-running process gets.
- It claims a reader who does not exist. `linedisc` was the correct Unix term of art. calef did not
  recognise it, and he built the system. That is evidence about the name, not about him. It became
  `lineedit` and then `line_editor`, which someone who has never opened a tty manual understands
  immediately.

So: name a component for what it is, and prefer a word that parses without prior Unix exposure.
`spawner`, `console`, `input`, `painter`, `window` were always right, and were always the majority.
The four `-d` names were the outliers.

The shell is the one exception, and it is deliberate. It is called `swish`, not `shell`, because
shell names are identities rather than descriptions (`bash`, `zsh`, `fish`, `rc`). The argument is
in milestone 63's roadmap block. `capsh` was the obvious candidate and is unavailable: Linux's
libcap ships `capsh(1)`, a capability shell wrapper, so a reader arriving from Linux would assume
ours is that tool.

## Components

A component is the shippable unit: one binary in `components/src/`, one `[[bin]]` in
`components/Cargo.toml`, one entry in the initrd archive. A service is what a component offers. A
contract is the wire protocol it offers it over. "Server" is a fine role word inside a component
(`redoxfs_server` serves the FS service). "Daemon" appears nowhere.

- Lowercase, `snake_case`, no suffix. `net_stack`, `compositor`, `gpu_driver`, `line_editor`,
  `fs_subtree_caretaker`. One word where one word will do, an underscore where the name is a
  qualifier applied to a thing. The 2026-08-01 rule retired the older "no separators" wording.
- Never `-d`. Not `netd`, not a future `logd` or `authd`. Checked.
- `c_` means "written in C", and it spans two unrelated milestones. `c_shim`, `c_seam` and
  `c_confiner` are the foreign-language seam of milestone 36 (a foreign-language component), per §31
  (the foreign-language seam). `c_swappable` is the replacement demo of milestone 23 (live
  replacement), the C half of the `rust_swappable` / `c_swappable` pair. The prefix means the same
  thing in both places and the milestones have nothing to do with each other. Do not read the four
  of them as one family.
- Abbreviate only where the abbreviation is what the field itself calls the thing, not merely a
  shortening that reads as obvious to whoever typed it: `pci`, `elf`, `dtb`, `gpt`, `ipc`, `asid`.
  If you have to expand it in the doc comment to make the file readable, it was not the ordinary
  name.

  *Corrected 2026-09-24: §154 (calef, 2026-09-18) replaced this clause's test with "spelled out
  where the expansion is a phrase people actually say". It deratified `dtb`, `gpt`, `ipc` and
  `asid`, which are now `device_tree_blob`, `globally_unique_identifier_partition_table`,
  `inter_process_communication` and `address_space_identifier`. Only `pci` and `elf` survive from
  that list. The main page carries the current test.*

  This clause used to cite `blk` and `kbd`, and they were renamed on 2026-08-28 for failing it, to
  `block_driver` and `keyboard_driver`. A rule whose own examples got renamed is telling you
  something. The test as first written ("is this the ordinary name?") was answered from inside
  Unix, where `blk` and `kbd` obviously are. The survivors are not shortenings at all. `pci` and
  `elf` are the names of standards, `dtb` and `gpt` name formats, `asid` is an architectural term
  of art. A reader meets each of them outside this project and arrives already knowing it. Nobody
  meets `blk` outside a Unix source tree.

  So the sharper question, and the one to ask of a new name: would a competent stranger who has
  never read this tree recognise it? `capsh`, `uheap` and `vt` fail that and are named in CLAUDE.md
  as the abbreviation failure mode. (Since §155 (the naming conventions move out of the
  constitution) that list lives on the main page of `design/naming.md`, not in `AGENTS.md`.) `pci`
  passes it. Truncating a word you happen to be tired of typing is not abbreviation, it is
  shorthand. Shorthand is what the third principle ("a newcomer must be able to succeed without
  asking anyone") exists to refuse.
- The binary name, the source file name and the archive entry name are the same string. `xtask`'s
  `initial_ramdisk_aarch64` pairs them positionally in a flat array. So a
  mismatch is a runtime "program not found" rather than a compile error, which is exactly the kind
  of thing to keep boring.
- There is no exception. The entry the kernel loads by name is a program of its own, `progenitor`
  (milestone 266 (one progenitor)), rather than a role name packed over two different binaries, so
  the three names agree in every row.

Fixtures and benchmarks (`interrupt_heeder`, `interrupt_ignorer`, `first_attempt_crasher`,
`allocator_exerciser`, `coremark`, `os_primitives_benchmarker`) live in `fixtures/`. Milestone 175
(split `user/`) separated them from the real components there on 2026-09-13. The naming rule is the
same either way: a fixture is a program and takes a program's name. `worker` was on that list by
repetition and is not a fixture. calef ruled it the canonical minimal program on 2026-09-13, so it
is `components/src/least_authority_demo.rs`.

### Two suffixes carry a category

The distinction between them is real (milestone 63 (directory and package names)).

An `_exerciser` puts a capability of the system under load and sees whether it holds, with no
contract being probed from outside. `allocator_exerciser` interleaves allocation and free, then
demands a large allocation fit in pages already committed. `std_exerciser` is an ordinary Rust
program on the native ABI whose three behaviours are chosen by the authority it was granted.

A `_test_client` exercises a service contract from outside, with a server on the other end:
`fs_test_client`, `socket_test_client`, `credentialer_test_client`. The `test` is not noise. The
unqualified names (`fs_client`, `socket_client`, `credentialer_client`) belong to the real clients
milestones 54 and 55 will need, and giving them to test programs squats them.

### A network card transport is `<device family>_transport`

calef, 2026-10-06 UTC, renaming `net_transport` to `virtio_net_transport` beside `e1000e_transport`
once a second card's transport made the old name ambiguous. Qualify the family only when its name
covers more than one kind of device: `designware_ethernet_transport`, because Synopsys DesignWare
also names USB, MMC, I2C and PCIe blocks. No vendor prefix, because vendors rebrand while family
names stay, and Linux likewise names the driver `e1000e` and puts the vendor in the directory. The
vendor goes in the crate and module headers. Refused `intel_e1000e_transport` and
`e1000e_ethernet_transport`, since nothing else is called e1000e.

## Shell builtins

A builtin is a word the shell answers itself. It is the most reader-facing name in the tree after a
program's: it is typed, and nothing but this file records why it is spelled the way it is.
`script/lint` cannot check them, because a builtin is a match arm in `grant_plan::parse` rather than
a file.

The rule is the one the crates already follow with the guard rail intact. A term of art a reader
already knows from outside this project is the best name available. So `cd`, `pwd`, `ls`, `mkdir`,
`echo`, `time` and `xargs` are Unix's and were never candidates for renaming. `caps` is ours.

- `apropos` (milestone 40 (documentation as a system service) phase 2, 2026-08-16). Provisional.
  Search the installed documentation store: `apropos capability` names the pages that mention the
  word. It is Unix's, and it is the same word for the same job in the same architecture. (`man` plus
  `apropos` plus `mandb` is the split this whole milestone borrowed.) So a reader arriving from
  anywhere else already knows what it does before they run it.

  The roadmap block proposed `doc search <term>`, and it was refused for a mechanical reason rather
  than a stylistic one. `doc` is a program and builtins are matched before program names. So a
  builtin whose first word is `doc` would shadow the viewer for every line beginning with it. A
  shell where `doc search` and `doc page.md` take different paths through the parser is one where a
  person has to know which class a command is in before they can type it. That is the thing
  milestone 47 (navigation and naming) deleted the `run` verb to avoid.

  `search` alone was refused as a generic word that could name almost anything in an operating
  system (crates §"generic words", where `compose` and `measure` were caught by the same test). And
  `find` because it is Unix's name for walking a directory tree. That is the one thing this system
  cannot do, and this milestone exists because it cannot.

## Scripts

Two directories, on purpose, and the split is by audience.

- `script/` is the front door:
  [Scripts to Rule Them All](https://github.com/github/scripts-to-rule-them-all) names, one short
  file each, no extension, lowercase, hyphenated if more than one word (`qemu-check`, `ci-qemu`,
  `toolchain-bump`, `vendor-verify`, `supply-chain`). These are what a person types. The canonical
  set (`setup`, `test`, `server`, `console`, ...) keeps its standard names even where a different
  word would be more descriptive. The entire value is that the command is the same in every repo
  that follows the pattern.
- `helpers/` is the helper drawer: `.sh` extension, called by other scripts and by `xtask`, not by
  people (`qemu-bounded.sh`, `qemu-runner-aarch64.sh`, `qemu-runner-riscv64.sh`).

  **calef ratified `helpers` on 2026-09-23**, not `scripts/`, after opening the tree and failing to
  tell `script/` and `scripts/` apart. His reason, verbatim: *"I'm totally disoriented in the
  script directory. Also, why do we have script and scripts?"* The split above was never the
  problem. The names were: one character of difference carried the whole distinction between what a
  person types and what `xtask` calls, `script` and `scripts` sorted next to each other in every
  listing, and neither word said which was which. `helpers` says what the drawer is for, and it says
  it in a word nobody confuses with `script`. Captured transcripts and dated accounts keep
  `scripts/` where they describe the past.

  **Ratified by calef on 2026-09-23 (UTC), replacing `scripts`.** The drawer is outside
  `script/names`' scope, for the reason [provenance-limits.md](provenance-limits.md) prices, so this
  paragraph is where its provenance lives and it is the record a later reader should find first.

Every `script/` entry needs a row in [scripts.md](../../notes/scripts.md). `script/lint` fails
without one, and fails in the other direction too if `README.md` names a script that does not exist.

## Directories (milestone 63)

The tenet covered crates, programs, modules, shell entry points and markdown, and said nothing about
directories, so the tree carried three spellings. Two rules, and neither is a new tier:

- A directory that holds a Rust package is named exactly as the package, so `snake_case`. The
  directory and the package are one thing with one name.
- Any other directory is lowercase, and hyphenated if it needs two words. It is the same convention
  as markdown filenames and `script/` entry points, because a directory is a path element and paths
  are hyphenated in the world outside this repository.

Three directories violated the first rule, and milestone 63 (directory and package names) moved all
three: `fs-server/` (package `fs-server`, `redoxfs_server/` today), `tools/redoxfs-host/` (now
`tools/redoxfs_host/`) and `user-std/`, whose package was called `hellostd` and matched neither
(now `std_exerciser/` twice over).

A hyphenated package name is not wrong in the wider ecosystem. `wasm-bindgen` and
`tracing-subscriber` are ordinary, and Cargo normalises a hyphen to an underscore for `use`, so
nothing was broken. The case was internal consistency, 36 crates against 3, and it should be read
that way rather than as a correctness fix.

Two things look alike and are not: `target/` is gitignored build output and `targets/` is the
tracked custom target JSON (`aarch64-unknown-nife.json` and its siblings). Nothing enforces the
distinction.

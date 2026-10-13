# Naming crates

*An appendix to [`design/naming.md`](../naming.md), which is the rule. This file holds the crate
rule and how far it reaches, nouns over verbs, the acronym history, and what `crates/` holds. It
exists to verify or challenge the main page, and a reader who only needs to name, ratify or rename
something should not have to open it. The directory `design/naming/` and this file's stem are
provisional names; naming is an architect's.*

## Crates

### The one rule, and who applies it (2026-08-01)

`snake_case`, everywhere, with no second tier. Crates already did this (`fs_proto`, `user_rt`).
Programs did not: 0 of 57 carried an underscore, so multiword names were squished. The three worst
were `fsclient`, `sysinit` and `credcli`. They are `fs_test_client`, `system_initializer` and
`credentialer_test_client` now (milestone 63 (directory and package names)).

An earlier draft had two tiers: short names for programs a user types, underscores for programs only
the system spawns. It was rejected, and the reason generalises. The category is not a stable
property of a program. `wc` was internal plumbing and became a prompt-typed pipeline stage inside a
day, and a convention keyed to something that changes produces renames. It is also not how Unix got
its names. The terseness of `ls` is emergent pressure on words people type constantly, not a rule
anyone wrote down. Codifying an emergent property turns it into a classification chore every
contributor has to get right.

So one rule, no branch. A short name for a typed command is a *choice its author makes*, not a
convention to apply; nobody needs a rule to know `wc` beats `word_count`.

An architect names the crates, the programs, and the shared modules. It has the same shape as
`design/decisions/` section numbers: global to the tree, so decided by an architect, who can see the
whole tree. The procedure (ship a provisional name, never rename on your own initiative) is on the
[main page](../naming.md). The reason is that names are what make this OS legible to humans and to
LLMs. In a capability system a name is often the only thing that says what a program can *do*.

#### Why it needed a rule, and how far the rule reaches

The evidence is the tree itself, and every one of these was a locally reasonable choice by whoever
was mid-task. `dwarden` is named for what it holds while its two siblings are named for what they
serve, so a reader who correctly infers the scheme gets it wrong. `conx` has no recorded expansion
anywhere: not in §41 (the endpoint is the broker), not in
[live-replacement.md](../../notes/live-replacement.md), not in the commit that introduced it.
`cseam.rs` sat among 48 programs and was not one; it was a shared module.

Crates came into scope on 2026-08-01, and they are the most reader-facing names in the tree. A
newcomer greps `crates/` before they ever open `components/src/`. A crate name appears in every
`Cargo.toml` that depends on it, in every `use` statement, and in the dependency graph an outsider
reads to understand the shape of the system.

Shared modules came in for a reason of their own. `user/src/` (the directory milestone 175 (split
`user/`) split into `components/src/` and `fixtures/src/`) used to hold 48 `[[bin]]` programs. It
also held a handful of modules compiled into them with `#[path = "..."] mod ...`. Nothing in the
naming distinguished the two, so a reader who tried to run `cseam` was misled by the directory.

`AGENTS.md` rule 7 retired that category the same day. What two binaries share is a crate, and what
remains beside the programs is single-consumer submodules (`virtio_net_transport`,
`socket_test_client`), which are ordinary Rust. `script/lint` counts consumers per `#[path]` target
and fails at two. A shared module's name still has to answer a question a program's name never
raises: *"where does this get compiled into?"* That makes it a naming problem of its own rather than
a smaller version of the program one.

The count in an earlier draft of that paragraph said "three modules" and was wrong. The grep that
produced it matched only single-line includes, and several were two lines. Take a count from the
merged tree, with a pattern you have checked against the real shapes.

Public function and method names came in on 2026-08-23 (milestone 160 (public function names)),
after a crate-naming pass across everything the kernel depends on raised the natural next question.
A function name is more reversible than a crate's: typically fewer call sites, all of them inside
one crate. So the "recommend on reversible forks" latitude applies more freely there than it does
one level up.

#### Nouns, not verbs, and the three crates that were settled by it

A crate, a program or a module is a *thing*, so it takes the name of a thing. A verb names an action
and a namespace is not one. That is audible at the call site: `line_edit::expand_output` reads as an
instruction where `line_editor::expand_output` reads as a location. The exception is a term of art
that happens to be a verb, where the word is the one the field already uses. `bind` (§50 (namespace
composition)) is Plan 9's, and respelling it as a noun would assert novelty where there is none.
That is the standard-terms guard rail rather than a hole in this rule.

Three crates predated the rule and were settled by it the day it was written: `compose` became
`compositor`, `measure` became `measured_boot`, and `dma_validate` became `dma_validator`. Each had
named itself a noun in its own first line while carrying a verb as its name. (`dma_validator` has
since been deratified by the acronym test below, on the other half of its name.)

#### The domain table's own argument

The [main page](../naming.md) carries the table of which form each domain takes (it lived in
`AGENTS.md` when this was written). Two things about its shape were argued.

The table as it stood before the main page shortened its reasons on 2026-09-24:

| Domain | Form | Because |
|---|---|---|
| Crates, programs, modules | `snake_case` | Rust's own convention, and what the tree already does |
| `script/` and `helpers/` entry points | `hyphens` | shell commands are hyphenated everywhere (`apt-get`, `pkg-config`, `docker-compose`); an underscore in a command name reads as a mistake |
| Ordinary markdown (`notes/`, `design/`) | `hyphens` | filenames become URL slugs in every static site generator, and hyphens are word separators in a URL where underscores are joiners |
| Repo-root markdown | `SCREAMING_SNAKE_CASE` | GitHub behaviour, not style. It recognises `README.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md` and links them in its UI; get the name wrong and the Security tab does not find your policy |
| A directory holding a Rust package | named exactly as the package, so `snake_case` | the directory and the package are one thing with one name |
| Any other directory | `hyphens` if it needs two words | a directory is a path element, and paths are hyphenated outside this repository |

The splits are *across* domains on a stable property, not within one on an unstable property. That
is the difference calef identified when he rejected the two-tier rule. `wc` moved from internal
plumbing to prompt-typed pipeline stage inside a day. A file, by contrast, either is a Cargo target
or is an executable in `script/`, and `script/test` will never become a `[[bin]]`.

It is also the standard-terms guard rail applied to form rather than to vocabulary. We do not rename
`elf`, and we should not respell `supply-chain` either: a name whose *shape* a reader already knows
from outside this project costs them nothing.

The directory rows are that same principle one level out rather than a new tier. The three
directories that violated them are in
[Directories](programs-scripts-and-directories.md#directories-milestone-63).

Standard terms are already right and must not be touched. `elf`, `pci`, `paging`, `glob` and
`socket_protocol` are names a reader knows from outside this project, so they cost nothing to learn.
This tenet is a naming authority, not a renaming mandate. Renaming `elf` would destroy the
recognition the whole thing exists to buy.

But an acronym is spelled out unless its expansion teaches nothing (calef, 2026-09-05), and this
paragraph used to protect four names that fail that test. The test is an asymmetry. A reader who
knows the term recognises its expansion instantly, so spelling it out costs the expert nothing and
saves the newcomer a bounce. The acronym only ever helps the reader who was already fine.

*Corrected 2026-09-24: §154 (calef, 2026-09-18) superseded this wording. The test is now whether
the expansion is a phrase people actually say. The main page states the current test; the
2026-09-05 version is kept here as the account of how it was reached.*

It was settled by the architect not knowing one. `mmu::bar_window` was defended on 2026-09-05. The
argument was that `BAR` was the vocabulary the retired `PCI_BAR_PHYS` already used. Then calef asked
what BAR stood for, which is the whole answer: an argument from this project's own history is not an
argument that a reader knows the word. He then named the general form, and deratified `dma` in the
same breath.

*"we want accessible names which means spelling out acronyms except in the most
obvious cases because the knowledgeable reader would recognize the expansion of an acronym they
knew."*

What survives and what does not. `pci` expands to peripheral component interconnect and the reader
is no wiser; `elf`, `paging` and `glob` are the same. `dma`, `dtb`, `gpt`, `ipc` and `asid` all
expand into something more informative than themselves (direct memory access, device tree blob, GUID
partition table, inter-process communication, address space identifier). They are therefore
deratified. `dma_validator` was ratified by calef on 2026-08-01, before the test existed, and this
overturns that rather than quietly dropping it from a list.

The sweep is its own milestone, because `ipc` in particular is load-bearing across the tree. A
rename that size is not a side effect of a rule change.

*Four of the five are renamed, to `device_tree_blob`,
`globally_unique_identifier_partition_table`, `inter_process_communication` and
`address_space_identifier`, and `crates/dma_validator` is `crates/direct_memory_access_validator`
(calef, 2026-09-24, #1229).*

One constraint: `nifefs` caps archive names at `NAME_LEN = 32` bytes, so a program's name is
bounded. Crates are not in the archive and are unbounded.

At 24 it decided names rather than bounding them: two settled names were within four bytes of it
and `os_primitives_benchmarker` exceeded it. Raising it
costs directory entries per block, and nothing else now that `Fs` no longer holds an entry array.
See [nifefs.md](../../notes/nifefs.md) for the numbers. The rule that survives the raise: do not let
the limit pick a name, and do not spend a format change on bytes nothing needs.

32 clears the longest settled name by seven bytes. That is a budget, where three bytes were left
before.

## What `crates/` holds

`crates/` holds four audiences under one directory, and naming does not distinguish them. That is a
known gap rather than a decision.

- Kernel logic, host-tested and Kani-reachable: `capability`, `paging`, `page_frames`,
  `memory_regions`, `generational_table`, `address_space_identifier`, `intrusive_fifo`,
  `inter_process_communication`, `direct_memory_access_validator`, `measured_boot`,
  `user_mode_heap`.
- Wire contracts, spelled `*_protocol` and checked for it by `script/lint`:
  `filesystem_protocol`, `socket_protocol`, `byte_sink_protocol`, `credential_protocol`,
  `clock_protocol`, `entropy_protocol`, `graphics_protocol`, `environment_protocol`,
  `login_protocol`, `network_time_protocol`, `supervision_protocol`, `swap_protocol`,
  `counter_frequency_protocol`, `capability_witness_protocol`. Plus `abi`, which is the syscall
  boundary and predates the suffix.

  The suffix is `_protocol`, not `_proto`, by milestone 265 (calef, 2026-09-05: *"I think `_proto`
  was lazy on my part. It should have been `_protocol` globally to differentiate from
  prototype."*). `proto` is a truncation rather than an abbreviation. It is equally short for
  `prototype`, which this tree uses for a real thing. Wherever a dated passage below spells a crate
  `_proto`, that is what it was called then and the passage is left alone.
- Format and hardware parsers: `elf`, `device_tree_blob`, `pci`,
  `globally_unique_identifier_partition_table`, `nifefs`.
- Userspace libraries: `user_mode_runtime`, `grant_plan`, `virtio`, `video_terminal`,
  `line_editor`, `bitmap_font`, `glob`, `calendar`, `credentialer`, `compositor`, `coremark`,
  `c_seam`.

### Two crates that look like contracts

`compositor` and `line_editor` look like contracts and are not. Both are *logic* crates that happen to contain a protocol module.
`compositor` is the scene, the clipping and damage-rectangle arithmetic, and the composition itself.
`line_editor` is a sans-IO editor with a `line_editor::proto` inside it. Renaming either to
`*_proto` would promise a wire definition and deliver an algorithm, which is exactly the kind of
claim §39 (a component is named for what it is) is about. The `*_proto` check is right to leave them
alone.

### The 2026-08-01 census

What the names actually do, over the 39 directories under `crates/` on 2026-08-01, when this census
was taken. It has not been re-taken (there were 68 on 2026-09-19). The names in it are the ones
those directories had that day.

*The census uses that day's names; `user_rt` and `user_heap` are `user_mode_runtime` and
`user_mode_heap` today.*

- One word where one word will do, which is 21 of the 39: `abi`, `capability`, `compositor`,
  `elf`, `frames`, `ipc`, `paging`, `regions`, `slots`, `virtio`.
- Underscore when the two halves are separate concepts and the name reads as a qualifier applied to
  a thing, which is the other 18. `filesystem_protocol` is the protocol *for* the filesystem,
  `graphics_protocol` the protocol *for* graphics, `dma_validator` the validation *of* DMA.
  `user_rt` is the runtime *for* userspace, `user_heap` the heap *for* userspace, `measured_boot`
  the measurement *of* boot.

There is no third bullet, "run together when the result is one word". It was a real observation
(`capsh`, `lineedit`, `uheap`, `crickerfs`, `bitfont`), and milestone 63 (directory and package
names) removed it because it was the rule that produced every abbreviation a reader had to decode.

Two of the five survive it. `crickerfs` (now `nifefs`, milestone 120 (the OS becomes
`nife`)) stayed with a reason: `procfs` is the shape of a filesystem name outside this project, and
nobody writes `proc_fs`. `bitfont` stayed with none, having never been renamed at all. The
kernel-dependency crate naming review ratified it as `bitmap_font` on 2026-08-23. Three moved, to
`grant_plan`, `line_editor` and `user_heap`. The boundary that remains, between one word and two, is
judgement. The guard rail is that a standard term keeps its standard spelling (see above).

The one place it became a real inconsistency is fixed and checked. The wire contract was spelled
four ways (`fs_proto`, `gfx_proto`, `netproto`, `line_editor::proto`) for one concept. `gfx_proto`
was its name at the time; the 2026-08-23 review made it `graphics_proto`, and milestone 265
(`_proto` is a truncation) made it `graphics_protocol` on 2026-09-14. `*_proto` won for crates,
because it is what the actual crates already were. `socket_proto` has since graduated from a module
inside `net_stack` into a crate (now `socket_protocol`). The suffix itself then lost, three weeks
later and to its own author: see the `_protocol` note above.

### A crate that is a component's engine

A crate that is a component's engine takes the component's name. `line_editor` is the sans-IO
editing crate and `line_editor` the binary that wires it to endpoints; `compositor` and `coremark`
are the same pair. They are the same thing at two layers, and giving the engine a second name is how
`termd`/`linedisc` happened in the first place. Where a note needs to tell them apart it says "the
`line_editor` crate" and "the `line_editor` binary".

One pair deliberately does not share a name. The crate is `video_terminal` and the program that
wires it is `display_terminal`. The crate is named for the protocol it implements (the VT standard,
bytes in and a character grid out). The program is named for its role: the terminal on the display,
next to `gpu_driver`, the virtio-gpu driver it is a client of. Both facts are true and neither name
says the other.

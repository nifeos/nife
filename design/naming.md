# Naming things in nife

What the tree's names mean, which conventions are rules, and which of those a machine checks.

This page states every naming rule in force, per §155 (the naming conventions move out of the
constitution, and the note becomes the rule). Each appendix it links holds the reasons, the full
lists and the prior art for one topic, and states no rule this page does not. Where this page and
`AGENTS.md` disagree, this page is the rule and `AGENTS.md` is the bug. The headline rule is
[§39 (a component is named for what it is)](decisions/0039-component-names.md).

## Who names things

An architect names the crates, the programs, the shared modules and the public functions. A name is
global to the tree, so an architect, who can see the whole tree, decides it. In a capability system
a name is often the only thing that says what a program may do.

- A lane ships a provisional name, marks its `Name:` block `provisional`, and says so in its report.
  The integrator surfaces it.
- Nobody renames on their own initiative; a rename is a naming decision.
- A function name is more reversible than a crate's, so the "recommend on reversible forks" latitude
  applies more freely there.

## A name is a claim

A name is a claim, made before a reader sees a line of code, and every reader of every call site
reads it. Name a thing for what it is, in a word that parses without prior Unix exposure. Name
against three failure modes:

- Abbreviations that need a decoder: `capsh`, `uheap`, `vt`.
- Generic words that could name almost anything in an operating system: `compose`, `measure`,
  `regions`, `slots`, `frames`.
- Against both, standard terms a reader already knows are the best names available: `elf`, `pci`,
  `paging`, `glob`. That covers shape too, so `supply-chain` keeps its hyphen. This is a naming
  authority, not a license to rename everything.

## Acronyms and abbreviations

- An acronym or abbreviation stays whole only where nobody says its expansion as a phrase, per §154
  (the acronym test is whether the phrase is spoken, applied recursively). Ask it again inside the
  expansion: `pcie` becomes `pci_express`, and `pci` stays.
- Every name this tree authors, parameters included, spells these out: `cap`, `op`, `recv`, `rx`,
  `tx`, `phy`, `init`, `irq`, `fifo`, `min`, `max`, `os`, `config`, `spec`, `arg`, `mem`, `dir`,
  `prog`, `pos`, `opt` and `mtime`. The same holds for `va`, `pa`, `ipi`, `nmi`, `ist`, `msr`,
  `dtb`, `ipc`, `asid`, `gpt`, `nvme`, `pcie`, `dma`, `pr`, `initrd`, `proto`, `rgmii` and `mdio`.
  The appendix gives each spelling and the cases that decide it.
- These stay: `pci`, `elf`, `usb`, `syscall`, `cpu`, `xor`, `io`, `bios`, `id`, QEMU, `MAC`,
  `GMAC` and `SD`. Spell `identifier` only where ID is one letter of a compound specification
  acronym (`address_space_identifier`). `SDIO` is not accepted.
- A name that arrives across another project's interface keeps its owner's spelling, and only the
  identifier the owner wrote keeps it. A concept nife names in its own code takes nife's spelling.
  The foreign spelling stops at the boundary: past QEMU's `-initrd`, the tree says
  `initial_ramdisk`.
- A kept foreign name's doc comment opens with its full name and the source, and the item carries
  `#[doc(alias = "<plain-English name>")]`. A specification's term of art that a reader searches for
  keeps its spelling inside a name this tree authors, with the expansion and the alias; any other
  term expands.
- A spelled-out name carries `#[doc(alias)]` for the term a reader searches for (`rgmii`, `irq`).
  Before a crate's first release it carries none for this tree's own earlier spelling.

Lists, exceptions and reasons:
[acronyms-and-abbreviations.md](naming/acronyms-and-abbreviations.md).

## Units

A name that holds a quantity ends with its unit, spelled out: `_HERTZ`, `_MILLISECONDS`,
`_NANOSECONDS`, `_SECONDS`, `_BYTES`, `megabits_per_second`. No SI symbol is valid in a name (`_HZ`,
`_MS`, `_NS`, `_US`, `mbps`), because uppercase erases the case SI symbols depend on: `MHZ` could be
megahertz or millihertz. The IEC binary prefixes (`kib`, `mib`, `gib`) read one way in every casing
and stay.

- A length or a limit carries its unit (`MAXIMUM_NAME_BYTES`, never `MAXIMUM_NAME_LEN`). A count of
  things names the things (`MAXIMUM_COLUMNS`).
- A Rust standard-library or smoltcp method keeps its owner's spelling (`as_nanos`, `total_millis`).
- About 95 existing names use a symbol (`TRANSFER_HZ`, `NS_PER_SECOND`); a separate sweep renames
  them, and a ratified one keeps its spelling until its own name review.

## Registers and constants

- A new or renamed register, field or bit constant copied from a cited source takes the spelled-out
  identifier that says what the thing is. The source's spelling goes in `#[doc(alias)]` and in the
  opening of the doc comment, citing OpenBSD's spelling first, then FreeBSD's, and Linux's only
  where no BSD source names it. A ratified name keeps its spelling until its own name review.
- This covers hardware names only. A program, crate or command name says what the thing does.
- Where no source defines an abbreviation (`MULR`), the name keeps it as written and the
  documentation says no source expands it.
- A register accessor's doc comment names the manual that defines the register and gives no section
  number.
- A register offset ends `_REGISTER`, a mask of several bits `_MASK`, an assertion constant
  `_DO_NOT_OVERLAP` (no leading `THE_`). A limit opens `MAXIMUM_` or `MINIMUM_` and ends with its
  unit.
- An address is `_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS` in every location.

## Vocabulary

- A half implies two: a branch of a split with three or more is an arm, part, piece or leg.
- `cpu` is the unit the scheduler runs a thread on; `core` is the physical core; a GPU is `gpu`.
- An identity is what a client presents; a `principal` is the authenticated actor that results.
- `user` means a person; user mode is `user_mode_`.
- A `keeper` holds something alive and restarts nothing; a `supervisor` restarts what faults; a
  `caretaker` serves narrowed access.
- Name a program for a terminus (`sink`) only if no capability grant could end it.
- A declaration is what a program asks for and a grant what it gets (`FileGrantDeclaration`,
  `FileGrant`). A contract is an agreement between two programs.
- A letter or word after a command is an option, never a flag; a flag is a bit.
- `nife` is lowercase everywhere, prose included.
- Avoid a word whose kernel meaning differs from yours (`schedule`, `swap`, `map`, `tick`, `probe`,
  `frame`, `flag`); qualify a word that names two things; prefer the word the field already uses.
- A suffix a reader knows from another family needs a member of that family behind it.
- Spell every name and word the American way. `helpers/house_style.py` refuses new British forms.

The tests behind each word, the stems kept, and examples: [vocabulary.md](naming/vocabulary.md).

## Shapes

- A crate, a program or a module is a thing, so it takes a noun: `line_editor`, not `line_edit`. A
  term of art that is a verb keeps its word (`bind`, per §50 (namespace composition)).
- A method takes std's spelling and meaning where std has one: `len`, `iter`, `iter_mut`, `get`,
  `get_mut`, `values`, `contains`, `is_empty`, `peek`, `with`, and `_mut` for one that returns
  `&mut`.
- Name the property, not the count (`MountedTree`, not `SecondDir`), and name a stand-in as one
  until the real value lands (`IMAGE_PLACEHOLDER_ROW`).
- A field that is an array of booleans takes the predicate form: `is_directory[i]`.
- Names that follow a renamed thing move with it, across crate boundaries.
- The naming rules apply to a function's parameters as well as to its public items.

## Functions that answer yes or no

A function that answers a question and changes nothing takes `is_` (before an adjective, participle
or noun phrase), `has_` (before a thing held), `can_` (before a verb), or a third-person verb phrase
(`contains`, `starts_with`, `allows`). A bare adjective, participle or noun (`enabled`, `pending`,
`read_only`) reads as a getter. Exempt: a function that acts and reports whether it worked
(`insert`, `push`, `claim`, a `take_`), test functions, fields, a trait method or a name another
project owns, and an `extern` symbol. Prior art and the hard cases:
[functions-that-answer-yes-or-no.md](naming/functions-that-answer-yes-or-no.md).

## Command-line names

- The command line abbreviates, and the code behind it does not: `--mem` is the option and
  `memory_grant` the identifier. Each abbreviation is still a name to ratify.
- `jig` takes apt's verb where apt has one, and a nife spelling that says so where it has none.
- A shell builtin takes Unix's word where one exists (`cd`, `ls`, `apropos`), and never shares its
  first word with a program, since the shell matches builtins first.
- The shell is `swish`: shell names are identities, and `capsh` is Linux libcap's.

The `jig` verbs, their prior art and the builtins:
[command-line-names.md](naming/command-line-names.md).

## Crates

- A crate takes `snake_case`: one word where one will do, an underscore where the name qualifies a
  thing. Programs follow the same rule, with no short tier for programs a person types.
- A nife wire-contract crate ends in `_contract`; an external standard keeps its word
  (`network_time_protocol`); `abi` takes no suffix.
- A crate that holds a component's logic takes the component's name (`line_editor`); where the two
  are different things they take different names (`video_terminal`, `display_terminal`).
- A network card transport is `<device family>_transport`, with no vendor prefix.
- A page contract crate spells its constants `PAGE_CONTENT_BYTES`, `PAGE_BYTES` and
  `PAGE_VIRTUAL_ADDRESS`.
- In `file_allocation_table`, host tests live in `<module>/tests.rs` and Kani proofs in
  `<module>/verification.rs`.
- `nifefs` caps a program's archive name at 32 bytes. Do not let the limit pick a name.

Reasons and what `crates/` holds: [crates.md](naming/crates.md).

## Programs, scripts and directories

| Domain | Form |
|---|---|
| Crates, programs, modules | `snake_case` |
| `script/` and `helpers/` entry points | `hyphens` |
| Ordinary markdown (`notes/`, `design/`) | `hyphens` |
| Repo-root markdown | `SCREAMING_SNAKE_CASE` |
| A directory holding a Rust package | exactly the package's name |
| Any other directory | `hyphens` if it needs two words |

- Never `-d`, which claims a Unix daemon. "Server" is a fine role word.
- A program's binary, source file and archive entry share one string.
- `c_` means written in C; `c_shim`, `c_seam`, `c_confiner` and `c_swappable` are not one family.
- An `_exerciser` loads a capability of the system; a `_test_client` exercises a service contract
  from outside. Plain names like `fs_client` stay free for real clients.
- Fixtures and benchmarks live in `fixtures/` and take program names; a test fixture names its role.
- `script/` is typed by people: no extension, hyphenated, Scripts to Rule Them All names kept.
  `helpers/` holds what scripts call, with an extension. A one-action script takes a verb, a
  standing tool a noun, and a gate ends `-check`. Every `script/` entry needs a row in
  [notes/scripts.md](../notes/scripts.md).

Reasons: [programs-scripts-and-directories.md](naming/programs-scripts-and-directories.md).

## Documents, numbers and branches

- `design/` holds the option space, `design/decisions/` the decision, `design/tenets/` why a rule in
  `AGENTS.md` holds, and `notes/` what exists. Every concept and finding gets a note.
- Write `§N` only for a decision and "milestone N" in full, named on first mention. Two files on one
  number is fatal; a gap passes; the later lander of a collision takes the next free number.
- A branch that looks like a milestone claim parses as `milestone/<N>-<slug>`. Other prefixes are
  convention; write `feature/`, never `feat/`.
- An appendix takes a slug of the heading it continues; one that continues no single section takes
  its own title, cut to a noun. A dated record takes its subject and date. A rule carries no date in
  its file name.

Reasons and the appendix file-name cases:
[documents-numbers-and-branches.md](naming/documents-numbers-and-branches.md).

## Rules a script can check

A revised rule lands with its check in `helpers/name_checks.py`. A new or renamed unratified name
passes every check; an existing violator sits in a baseline that only shrinks, and lanes rename it,
still `provisional`. A ratified name is exempt. The gates, the worklist and what no script can
judge: [rules-a-script-can-check.md](naming/rules-a-script-can-check.md).

## Where a name's provenance lives

Every name carries its own `Name:` block at the name, in one of four states (`provisional`,
`unrecorded`, `recorded`, `ratified`), with what was refused. A lane never writes `ratified`; the
maintainer writes it in the commit that applies the name. `script/names` computes the table on
demand and checks presence and form, never `ratified`. A rule that ratifies a class of names lives
in [convention-rulings.md](naming/convention-rulings.md), which `script/names` reads; a ruled name
nobody renamed fails `--check` after 45 days. The forms and commands:
[provenance.md](naming/provenance.md); what the record cannot see:
[provenance-limits.md](naming/provenance-limits.md).

## Performing a ratified rename

Enumerate before sweeping, hold the refusal count, and let status decide what moves: a quotation and
a closed record keep their names, and a path is fixed everywhere. The checklist and its incidents:
[rename-what-moves.md](naming/rename-what-moves.md),
[rename-where-names-hide.md](naming/rename-where-names-hide.md) and
[rename-traps.md](naming/rename-traps.md).

## Appendices

[naming/README.md](naming/README.md) indexes them.

## BUGS

- An item without a marker is invisible to the worklist, and nothing gates that.
  [provenance-limits.md](naming/provenance-limits.md) says which kinds of name are uncovered.
- `helpers/` helpers are outside the worklist on purpose, per milestone 446 (the naming worklist
  says what it covers).
- A `ratified` is never checked against an architect, and a `recorded` citation is never followed.
- `helpers/unit_symbols.py` does not count `ns`, `ms`, `hz`, `khz` or `mhz`, and still respells
  `nanos` as `ns`; `helpers/name_checks.py` does not hold `rx`, `tx`, `phy`, `init`, `irq`, `fifo`,
  `min`, `max`, `os`, `rgmii` or `mdio`. The gates pass a new name that breaks those rows.
- `design/capsicum-and-the-retrofit-question.md` still names `worker`, `heeder` and `spinner` in a
  present-tense claim.

# Naming things in nife

What the tree's names mean, which conventions are rules, and which of those a machine checks.

This file is the authority for naming conventions, per §155 (the naming conventions move out of the
constitution, and the note becomes the rule). `AGENTS.md` keeps only the authority: naming is an
architect's call, ship a provisional one and say so, never rename on your own initiative. Where the
two disagree, this file is the rule and `AGENTS.md` is the bug. The headline rule is
[§39 (a component is named for what it is)](decisions/0039-component-names.md).

The argument, examples and refusals live in the [appendices](#appendices).

## Who names things

An architect names the crates, the programs, the shared modules and the public functions. A name is
global to the tree, so an architect, who can see the whole tree, decides it. In a capability system
a name is often the only thing that says what a program may do.

- A lane ships a provisional name, marks its `Name:` block `provisional`, and says so in its report.
  The integrator surfaces it.
- Nobody renames on their own initiative. A rename is a naming decision with extra steps.
- A function name is more reversible than a crate's, so the "recommend on reversible forks" latitude
  applies more freely there.
- A lane minting a provisional name need not read this; `script/lint` names a broken rule.

## A name is a claim

A name is a claim, made before a reader sees a line of code. A wrong one is a stale comment nobody
can skip. `netd` claimed a Unix daemon, a model this OS rejected. `linedisc` claimed a reader who
does not exist: calef did not recognise it, and he built the system. So name a component for what it
is, and prefer a word that parses without prior Unix exposure.

Name against three failure modes:

- Abbreviations that need a decoder: `capsh`, `uheap`, `vt`. Ask whether a competent stranger who
  has never read this tree would recognise the name.
- Generic words that could name almost anything in an operating system: `compose`, `measure`,
  `regions`, `slots`, `caps`, `frames`.
- On the other side of the line, standard terms a reader already knows are the best names
  available: `elf`, `pci`, `paging`, `glob`. That covers shape too, so `supply-chain` keeps its
  hyphen. This rule is not a licence to rename everything.

### The acronym test

An acronym is spelled out where its expansion is a phrase people actually say, and stays whole where
nobody says it. That is §154 (the acronym test is whether the phrase is spoken, applied
recursively), ruled 2026-09-18. Ask it again of any acronym inside the expansion. So `dtb` became
`device_tree_blob` and `pcie` becomes `pci_express`, while `pci` and `elf` stay. §154 deratified
`dma_validator`, `nvme`, `gpt`, `dtb`, `ipc` and `asid`.

The test applies to names this tree authors. A name that arrives across somebody else's interface
keeps their spelling, with an expansion written where the reader meets it. The foreign spelling
stops there: inside, `initrd` is `initial_ramdisk` (#1842).

### Nouns, not verbs

A crate, a program or a module is a thing, so it takes a thing's name (calef, 2026-08-01).
`line_edit::expand_output` reads as an instruction; `line_editor::expand_output` reads as a
location. A term of art that happens to be a verb is the exception: `bind` is Plan 9's word, per §50
(namespace composition).

### A shared name says something

When a crate and a program share a name, the crate is that program's logic, lifted out to be
host-tested while the program keeps the IO: `line_editor`, `compositor`, `coremark`. A note tells
them apart as "the `line_editor` crate" and "the `line_editor` binary". The crate `video_terminal`
(named for its protocol) and the program `display_terminal` (named for its role) differ on purpose.

## Functions that answer yes or no

Follow Rust (calef, ratified 2026-09-24, reviewing `tick_pending`). A function that answers a
question and changes nothing is named in one of four shapes:

- `is_` before an adjective, participle or noun phrase: `is_empty`, `is_tick_pending`.
- `has_` before a thing held (`has_unclosed_fence`), or `can_` before a verb (`can_read`).
- A third-person verb phrase, std's shape for a relation: `contains`, `starts_with`, `overlaps`,
  `allows`, `needs_drop`.

A bare adjective, participle or noun (`enabled`, `pending`, `present`, `truncated`) reads as a
getter, and std spells those `is_enabled`. Exempt: a function that acts and reports whether it
worked (`insert`, `push`, `claim`, a `take_` that clears a flag). Also exempt: test functions, which are
sentences here; fields; a trait method or a name another project owns (`eq`, `readonly`); and an
`extern` symbol. The prior art and the exceptions std keeps are in
[functions-that-answer-yes-or-no.md](naming/functions-that-answer-yes-or-no.md), and the pass that applied the rule is
[functions-that-answer-yes-or-no-worklist.md](naming/functions-that-answer-yes-or-no-worklist.md).

## Spelling, per domain

| Domain | Form | Because |
|---|---|---|
| Crates, programs, modules | `snake_case` | Rust's own convention |
| `script/` and `helpers/` entry points | `hyphens` | shell commands are hyphenated everywhere (`apt-get`, `pkg-config`) |
| Ordinary markdown (`notes/`, `design/`) | `hyphens` | filenames become URL slugs, where a hyphen separates words |
| Repo-root markdown | `SCREAMING_SNAKE_CASE` | GitHub recognises `README.md`, `SECURITY.md` and `CONTRIBUTING.md` by name |
| A directory holding a Rust package | exactly the package's name | the directory and the package are one thing |
| Any other directory | `hyphens` if it needs two words | a directory is a path element |

The splits run across domains on a stable property, and there is no second tier within one. Short
names for programs a person types were refused as a rule, because `wc` went from plumbing to a typed
pipeline stage in a day. A short name is its author's choice.

Every name and every word is spelled American: `color`, `catalog` (calef, 2026-10-06 UTC).
`helpers/house_style.py`, in `script/lint`, refuses new British forms.

`nifefs` caps a program's archive name at `NAME_LEN = 32` bytes. Crates are unbounded. Do not let
the limit pick a name.

## Programs, crates and scripts

- Never `-d`, which claims a Unix daemon. The vocabulary is component (the shippable unit), service
  (what it offers) and contract (the wire protocol). "Server" is a fine role word.
- A program's binary, source file and archive entry share one string.
- Wire-contract crates end in `_contract` (calef, 2026-10-18 UTC); a standard keeps its word
  (`network_time_protocol`). `abi`
  predates the suffix. `compositor` and `line_editor` hold protocol modules but are logic crates.
- A crate that is a component's engine takes the component's name.
- An `_exerciser` loads a capability of the system, with no contract probed. A `_test_client`
  exercises a service contract from outside. Plain names like `fs_client` stay free for real
  clients.
- `c_` means written in C. `c_shim`, `c_seam`, `c_confiner` and `c_swappable` are not one family.
- Fixtures and benchmarks live in `fixtures/`, per milestone 175 (split `user/`), and take program
  names.
- The shell is `swish`, since shell names are identities (`bash`, `zsh`). `capsh` is Linux libcap's.
- A builtin takes Unix's word where one exists (`cd`, `ls`, `apropos`). It must not share its first
  word with a program, since builtins match first.
- `script/` is typed by people: no extension, hyphenated, and the Scripts to Rule Them All set keeps
  its standard names. `helpers/` holds helpers other scripts call, with an extension. Every
  `script/` entry needs a row in [notes/scripts.md](../notes/scripts.md).
- `target/` is build output, and `targets/` holds the tracked target JSON.

## Words calef has ruled on

Each sets a test that reaches past the word. The arguments are in
[vocabulary-rulings.md](naming/vocabulary-rulings.md).

| Ruling | The test it sets |
|---|---|
| A half implies two; a third of anything is an arm | "half" is for a two-way split; a branch of three or more is an arm, part, piece or leg |
| A terminus that is structural, or one that is merely current | name a program for an end of stream only if no grant could end it. `audit_sink` became `login_audit_receiver` |
| An identity is what you present; a principal is what you become | `principal` is the authenticated actor holding a capability set |
| The `login` stem stays | ratified 2026-09-15 for the whole family |
| The casing of `nife` | lowercase everywhere, prose included |
| `cpu` is the scheduling unit; `core` is the physical core | `cpu` is what the scheduler runs a thread on; a GPU is named `gpu`, never `cpu` (2026-10-03) |

### Precedents set by example

The 2026-10-18 rulings, as rules: [precedents-set-by-example.md](naming/precedents-set-by-example.md).

## Where a name's provenance lives

Provenance lives at the name, per milestone 115 (the names that were ratified, and the ones that
were refused). A crate's `lib.rs` header, a program's module doc, a `script/` entry's comment and a
Cargo package's manifest each carry one `Name:` block. Adding a name touches one file, so lanes
cannot collide.

Four more kinds are read by marker (2026-09-24), in the same grammar:

- `item`: a `/// Name:` paragraph in the doc of a function, constant, type, field or variant.
- `module`: a `//! Name:` block in a Rust file that is not a crate root or a program.
- `directory`: the `Name:` paragraph of a documentation directory's `README.md`, per §75.
- `document`: the document's own `Name:` paragraph, else its directory's block.

A lane minting any of these writes the marker, so the name joins `--unratified`. An unmarked item
never fails.

A document's block is an italic paragraph under its title by convention and read wherever it
sits (calef, 2026-10-24 UTC), not frontmatter
([why](naming/provenance.md#a-documents-block-is-a-paragraph-not-frontmatter)). A README
`` `stem` Name: `` paragraph counts until `helpers/document_name_migrate.py` moves the last.

The refusals are the valuable half. A refused name is visible nowhere else, and the person who most
needs it is the one about to propose it again.

### The four states

| State | Means | What clears it |
|---|---|---|
| `provisional` | whoever chose it expects it to change, per §89 (`provisional` becomes the fourth provenance state) | a ruling |
| `unrecorded` | nothing in the tree or its history says why | research, then a ruling |
| `recorded` | something outside the block argues the name; calef never ruled | a ruling |
| `ratified` | calef ruled, with the date and what was refused | done |

### The forms

```
Name: ratified <YYYY-MM-DD> (<who>, <where>). Refused `x` (why), `y` (why).
Name: recorded (<where>). <what the tree already argues, and what it does not settle>
Name: unrecorded. <what the history does and does not say>
Name: provisional. <what was considered>
```

- The date on `ratified` sits outside the parentheses, and `recorded` names its citation. Both are
  checked as form, never as truth.
- A block is never its own evidence. "It got here first" is not a reason.
- A reason goes in parentheses, and a refusal clause ends at its sentence, so the parser does not
  read a citation such as `capsh(1)` as a refusal.
- `unrecorded` is a first-class answer. Cite the commit that introduced the name.
- One block per file, in plain spelling, per milestone 283 (one provenance block per file). The
  parse reads the first block only and cannot see a `Name:` line wearing markdown.
- A lane never writes `ratified`.
- The maintainer writes the block at ratification, in the commit that applies the name.

### The commands

```
script/names <name>        has this name been refused, and where does the reason live?
script/names --refused     everything turned down, and what holds each refusal
script/names --unratified  the worklist, ordered by exposure; provisional first; an item or
                           module another Cargo package reaches comes before an internal one
script/names --check       the gate; script/lint runs it
```

The gate checks presence and form, and never keys on `ratified`, so an unratified name never fails
a build.

The order of `--unratified` is ruled in [convention-rulings.md](naming/convention-rulings.md). That
page also holds the one gate that keys on a ruling: a name calef ruled and nobody renamed fails
`--check` after 45 days.

A rule calef ratified for a class of names is recorded with his words in
[convention-rulings.md](naming/convention-rulings.md) (2026-10-17). `script/names` counts every name
a rule covers as ratified and `script/names <name>` cites the rule. A name the rule pulls aside keeps
the status its own block claims.

## Where a document goes

| | holds | shape |
|---|---|---|
| `design/` | the option space, before a decision | "here are four answers and three are bad" |
| `design/decisions/` | the decision, and the argument that settled it | numbered `§N`, append-only |
| `design/tenets/` | why a rule in `AGENTS.md` holds | one file per theme, linked from the rule |
| `notes/` | what exists, and what building it taught us | a glossary, indexed in `notes/README.md` |

What decides it is what a document is for. `design/roadmap/` sits in `design/` because a milestone
block argues for doing something. Every concept and finding gets a note, indexed in
[notes/README.md](../notes/README.md).

## `§N` is not milestone N

Decision numbers and milestone numbers are separate schemes over the same integers.

- Write `§N` only for a decision, and "milestone N" in full, never a bare `N`.
- Name it on first mention: "milestone 31 (the capability shell)". `script/citations` checks it.
- Two files claiming one number is fatal. A gap is reported and passes.
- When sessions collide, the later lander takes the next free number and leaves the hole, per §194
  (sessions interleave rather than serialize).
- A number is provisional until the merge queue lands it.

## Branches

A branch that looks like a milestone claim must parse as `milestone/<N>-<slug>`, because
`script/lint` check 4b reads it. `helpers/branch-name-check.sh` refuses a near-miss such as
`milestone-126-pgrep`, per §77 (the branch-prefix list now describes the tree). Other prefixes are
convention: `maintainer/`, `fix/`, `bench/`, `audit/`. Write `feature/`, never `feat/`.

## Performing a ratified rename

A rename is cheap to type and expensive in every other way. A sweep edits a record as cheaply as
code, and a revert cannot restore a refusal it rewrote. The incidents behind each step are in
[rename-what-moves.md](naming/rename-what-moves.md),
[rename-where-names-hide.md](naming/rename-where-names-hide.md) and
[rename-traps.md](naming/rename-traps.md).

1. Enumerate before sweeping. Renaming `crates/manual` matched 89 files, and 37 were the crate. List
   the true positives, read them, then edit.
2. Take `script/names | tail -3` before and after. The refusal count must not move.
3. Status decides what moves, not directory. Status belongs to the passage, not the file.

   | Kind | Moves? |
   |---|---|
   | `BUILT` block, dated audit report, closed decision | No: an account keeps the names it used |
   | `PROPOSED` proposal, a `PARTIAL` block's open scope | Yes: live intent |
   | A dated measurement inside a live document | No: keep the name, add a clause saying why |
   | Any quotation | Never |

4. A quotation never moves. Put a note beside it instead. A `Name:` block's `replacing` clause is a
   quotation. So is a citation of another project: its file name, URL, version string and symbols.
5. The ownership test: if the thing keeps its name when our crate is deleted, the name is not ours.
   That keeps `satp.ASID`, PCI's `CLASS_NVME` and QEMU's `-device nvme`.
6. A crate drags its directory, package, dependency entries, fuzz targets, public types named for
   the acronym (`Gpt`, though `Guid` stays) and a note named for it. Notes named for a concept stay,
   as do roadmap slugs.
7. A path is navigation, so fix a broken one even in a `BUILT` block. Grep links and prose paths.
8. A program's name is a string literal nothing type-checks. Grep the quoted name too: shell tables,
   `xtask` archive tuples, gate rows, `.conf` files, tests and derived identifiers.
9. A crate's name also lives outside Rust: `build.rs` paths, `--exclude` lists, mutation globs,
   baselines, gate globs and each workspace's `Cargo.lock`. Build all eight workspaces (every
   tracked `Cargo.toml` with a `[workspace]` table, outside `vendor/`).
10. Run a gate whose subject you renamed against a deliberately wrong name once. It may have stopped
    looking.
11. Resolve a pointer before rewriting it, or the sweep fabricates one.
12. Take the census twice and classify every line of the second, in the crate's provenance block.
13. Tells: `git status` shows `R`, not `A`. Backtick `crates/<name>` in doc comments. Grep
    `](<name>)`. macOS `git grep` ignores `\b`, so re-check a zero. Width-check a rewrap.

This is rung three of the ladder: no gate can tell an account from an intention in prose.

## What is checked, and what cannot be

`script/lint`'s naming block, whose count the script owns:

1. No name ending in `-d` (a real word goes in `naming_allow`, with a reason).
2. No "daemon" outside `design/` and `script/lint`.
3. Contract crates spelled `*_contract`.
4. A milestone-claim branch that parses.
5. No `#[path]` module shared by two binaries.
6. Lowercase, hyphenated `notes/` and `design/` filenames.
7. One `Name:` block per name, in a known state.

A separate check refuses a count above two before "halves". `linedisc` would pass every check. What
caught it was a person not knowing the word, and that remains the test.

Names meet the current rules (calef, 2026-10-18 UTC). A revised rule lands with its check, a new
or renamed name passes each check, and lanes rename unratified violators, still
`provisional`. [rules-a-script-can-check.md](naming/rules-a-script-can-check.md) has the gate.

## Appendices

Every former section kept its heading, so a citation by section name resolves in the file listed.
The directory and stems are provisional. The table is in [naming/README.md](naming/README.md).

## BUGS

- An item without a marker is invisible to the worklist, and nothing gates that. Only the lane
  that minted a name can say it is new.
  [provenance-limits.md](naming/provenance-limits.md) says which kinds of name are uncovered.
- `helpers/` helpers are outside the worklist on purpose, per milestone 446 (the naming worklist
  says what it covers). None of their paragraphs recorded a refusal when that was priced.
- A `ratified` is never checked against calef, and a `recorded` citation is never followed.
- The boot mode is still `shell` (`cargo xtask shell`) while the program is `swish`.
- `design/capsicum-and-the-retrofit-question.md` still names `worker` in a present-tense claim.

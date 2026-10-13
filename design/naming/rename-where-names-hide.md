# Performing a rename: where names hide

*An appendix to [`design/naming.md`](../naming.md), which is the rule. This file holds the sites a
compiler cannot see when a program or crate is renamed, and why the census is taken twice. It exists
to verify or challenge the main page. A reader who only needs to name, ratify or rename something
should not have to open it. The directory `design/naming/` and this file's stem are provisional
names; naming is an architect's.*

## Renaming a crate is compiler-checked; renaming a program is not

This is the clause the other three do not cover, and it is a different error. (The other three are
[status](rename-what-moves.md#status-decides-what-moves-not-directory),
[quotation](rename-what-moves.md#a-quotation-never-moves) and
[enumeration](rename-what-moves.md#enumerate-before-sweeping).) "Enumerate before sweeping" guards
false *positives*: most matches are not the name. This guards false *negatives*, and the two want
opposite habits. One says do not trust the match count. The other says the match count is not the
whole set, and nothing will tell you.

Change `crates/manual` and `cargo check` finds every site missed. Change the program `doc` and the
compiler is silent. A program's name reaches the running system as a string literal in tables
nothing type-checks.

Where it hides, from the two renames that found it:

| Site | Example |
|---|---|
| The shell's command table | `b"doc" => Some(Prog::Doc)` in `crates/grant_plan` |
| ...and its reverse map | `Prog::Doc => "doc"` |
| Archive tuples in `xtask` | `("doc", "doc")`, **once per architecture** |
| A gate's expectation row | `("doc", &["name a file"])` |
| `program(...)` lookups | `user::program("jh7110_trng")` in `kernel/src/main.rs` and the entropy tests |
| `[[bin]]` name and path | `components/Cargo.toml`, `fixtures/Cargo.toml` |
| Shell command strings inside tests | `parse(b"heeder report.txt")` |
| Fixture strings in other crates | `crates/timetable`'s `"every 5s heeder"` |
| A configuration file the tree ships | `components/timetable.conf`'s `at-boot budgeter --mem 4` |
| Identifiers derived from the program's name | `saw_budgeter_grant`, `budgeter_reports`, four test function names |
| A provenance block's "replacing" clause | `Name: ... replacing the provisional jh7110_trng` |
| Another project's file name, URL, version string or identifier | `$NetBSD: jh7110_trng.c,v 1.2 ...`, the fetch URL beside it, and `jh7110_trng_init` in its text |

### Two rows that hide in their own way

The configuration-file and derived-identifier rows were added by the `budgeter` rename on
2026-09-13. A `.conf` is invisible to the habit that makes this technique cheap. Most of these
sweeps are scoped with `git grep` narrowed by `--include=*.rs --include=*.md --include=*.toml`, and
that misses a shipped configuration file entirely. Yet `crates/timetable` compiles that one in with
`include_str!`, and the kernel asserts on it firing.

Derived identifiers hide for the opposite reason: they are *not* string literals, and the compiler
does find them. `saw_budgeter_grant` and `budgeter_reports` would have compiled fine under the old
spelling. They would have left the tree naming a program that no longer exists, in the one place a
sweep's own grep still finds them. Neither is exotic. Both were hit by the `worker` rename earlier
the same day and recorded only in its commit message, which is rung four.

### Two rows a sweep must not touch

The last two rows, the provenance clause and the foreign citation, are the first that mark a site a
sweep must *not* touch (the `jh7110` rename's repair, 2026-09-13). Every row above them is a false
negative, a place the sweep missed. These two are false positives. They belong here because the
same technique catches both: enumerating the matches and reading them rather than counting them.

The provenance sentence is the worst place in the tree to sweep blind. It is the one occurrence of
the old name the standard exists to protect. `95db4a3e` renamed `jh7110_trng` to
`jh7110_entropy_source` and `jh7110_crg` to `jh7110_clock_and_reset`. In both crates it rewrote the
clause naming the predecessor. Each block came out saying it replaced itself, and the old name was
then unrecoverable from the block: it had to be read back out of `git log`.

That was caught only because the resulting sentence is self-referentially absurd. "Replacing the
provisional `jh7110_entropy_source`" inside `jh7110_entropy_source` reads as nonsense to anyone who
looks at it. A rename between two less similar words produces a sentence that reads perfectly and is
false, and nothing here would say so. `script/names` parses the block and prints it, and has no
opinion about whether the name inside is the one being replaced. So treat the `replacing` clause
exactly as [a quotation](rename-what-moves.md#a-quotation-never-moves) is treated, because that is
what it is. It quotes a decision.

A citation to another project is a quotation wearing a path. The same commit rewrote
`sys/arch/riscv/starfive/jh7110_trng.c` to `jh7110_entropy_source.c` in three places in one file:
the source bullet, its `raw.githubusercontent.com` fetch URL, and the reference-link definition at
the bottom. One of them carried NetBSD's own RCS keyword string, `$NetBSD: jh7110_trng.c,v 1.2
2025/02/09 09:09:49 skrll Exp $`, which is a verbatim line out of somebody else's source file. The
tree then cited a file that does not exist upstream and a version string nothing ever printed. That
is the fabricated-quote failure this project has already carried once for twelve days. The seven
questions say prior art is read rather than recalled; a swept citation is a citation recalled, with
`sed` doing the recalling.

The sibling that survived shows it was luck rather than care. `crates/jh7110_clock_and_reset` cites
Linux's `starfive%2Cjh7110-crg.h`. It is still right only because upstream spells that one with a
hyphen where the sweep matched an underscore.

There was a fourth site in that same file, and it outlived the repair. It was found 2026-09-14 by
the rename that replaced `jh7110_entropy_source` with `jh7110_entropy`. `95db4a3e` also rewrote the
function name inside NetBSD's driver. So the crate's bring-up section credited the sequence to
`[netbsd]'s jh7110_entropy_source_init`, a symbol that exists in no tree anywhere. `0cfb6f63`
restored the two `replacing` clauses and did not look for this. The three sites it knew about were
all paths, and this one is an identifier. It was repaired by fetching the file
(`raw.githubusercontent.com/NetBSD/src/trunk/sys/arch/riscv/starfive/jh7110_trng.c`, which also
re-confirmed the `$NetBSD: jh7110_trng.c,v 1.2 2025/02/09 09:09:49 skrll Exp $` line the crate
quotes) and reading the name out of it: `jh7110_trng_init`.

So the row above reads file name, URL, version string, or identifier. A foreign name does not have
to look like a path to be somebody else's. The sweep's own pattern decides which of them it eats.
An underscore-spelled rename matches an underscore-spelled foreign symbol and leaves a
hyphen-spelled one standing. That is why the survivor above survived and this one did not.

### One failure and one success, a commit apart

Renaming `doc` to `mdr` left `grant_plan` still saying `doc`. The shell could not spawn the binary,
and the archive did not hold what the gate looked for. `cargo check` passed and three CI jobs failed
for that one cause. The `jh7110` rename the same day enumerated strings first, found all four sites,
and pushed green. That success was real and partial, which is why the paragraphs above exist. The
same commit broke four records, fabricated three external citations, and left a whole crate behind.
Enumerating the *program* strings is one clause of this standard and not the standard.

So: for a program, grep the quoted name as well as the identifier, and treat `cargo check` passing
as no evidence at all.

## A crate copied rather than moved is invisible to every gate but one

`95db4a3e` moved `kernel/src/drivers/jh7110_crg.rs` and `user/src/jh7110_trng.rs` properly, and git
records both as renames. `crates/jh7110_crg` it copied: the new directory was added and the old one
was never deleted. That left 757 lines of duplicate source behind with no `Cargo.toml` at all.

Nothing compiled it. It was not in `Cargo.toml`'s workspace members, and no manifest referenced it.
This tree's gates are compile-driven almost everywhere. So there was no clippy over it, no test, no
coverage, no mutation sweep, and no `cargo check` that could notice the duplicate at all. A
directory outside the workspace is outside all of them at once.

The one gate that did see it is `script/names`, because it walks `crates/*/src/lib.rs` on disk
rather than the package graph. So the defect surfaced as a worklist entry: `jh7110_crg` went on
`script/names --unratified`. That queued a name nobody could compile into the one queue whose entire
purpose is to spend an architect's attention well. Nothing red happened anywhere. The cost was paid
in the scarcest thing in the project rather than in a build.

The general fact is worth more than the incident: an on-disk walker and a package-graph walker
disagree, and the disagreement is information. `script/verify` and `script/falsifications` both
moved to `cargo metadata` because a hand-kept list went stale silently. `script/names` walks the
disk because a name exists whether or not it compiles. Neither is wrong. A name present to one and
absent to the other is a thing to go and look at rather than reconcile.

So, after any rename that moves a directory: `git status` showing an add where you expected a rename
is the whole tell, and `git diff --stat -M` on the commit says which it was.

## A crate is compiler-checked only where a compiler is looking

The clause above says renaming a crate is the easy case, and milestone 285 (`user_` meant a person)
found the sentence too generous. `cargo check` finds every `use` and every `[dependencies]` key,
which is most of the work and all of the reassurance. It finds nothing where the crate's name has
left Rust and become a path on disk or an argument to a build tool. Those sites fail at link time,
at gate time, or not at all.

The tree already had the scar and had not generalised it. When milestone 175 (split `user/`) moved
`user/link.ld` to `crates/user_rt/link.ld`, two `build.rs` files were missed because they live in
separate Cargo workspaces. Nothing errored until a cross-compiling link, and CI's QEMU leg went red
with nothing before it saying a word. That is the same shape as everything below. (`crates/user_rt`
is `crates/user_mode_runtime` since the rename that wrote the table below; the sentence keeps the
name the move happened under.)

| Site | Example | Why the compiler is blind to it |
|---|---|---|
| A linker script referenced by path from a `build.rs` | `../crates/user_mode_runtime/link.ld`, in **eight** build scripts, six of them in separate workspaces (`git grep` lists them) | it is a string handed to `rust-lld`, and the main workspace never builds the separate ones |
| `--exclude <crate>` in a gate | `script/lint`, `script/coverage`, and two lists in `xtask/src/main.rs` | cargo takes an unknown `--exclude` name silently, so the gate keeps passing while covering less |
| A mutation-testing exclusion glob | `.cargo/mutants.toml`'s `"crates/user_mode_runtime/**"` | a glob that matches nothing is not an error |
| A crate-keyed row in a measurement baseline | `.cargo/mutants-baseline.txt`'s `user_mode_heap 20 3 5 7` | a plain data file, keyed by crate name, that no build reads |
| An identifier derived from the crate name inside a gate's embedded script | `reaches_user_mode_runtime()` in `script/lint`'s python | it compiles and runs either way; only the reader is misled |
| A shell script that derives an artifact from the crate's directory | `helpers/build-ripgrep.sh` seds `crates/user_mode_runtime/link.ld` into a high-load variant | shell, and it runs only when somebody builds ripgrep |
| A generated module in the patched-`std` overlay | `sys/alloc/nife/user_mode_heap.rs`, written by `xtask` from the crate and declared `mod user_mode_heap;` in the overlay | it compiles only when the `std` farm is rebuilt, in a source tree outside every workspace |
| `Cargo.lock` in each separate workspace | `redoxfs_server/Cargo.lock`, `tools/redoxfs_host/Cargo.lock` | regenerated on their own next build, not on the main workspace's |
| A **glob that selects the set a gate then judges** | `script/lint` check 3 looped over `crates/*proto` and rejected any name not ending `_proto` | after milestone 265 that glob matches no directory, so the loop body never runs and the check passes by checking zero crates |

The glob row is worse than the `--exclude` row above it and belongs beside it anyway. Both fail by
going quiet. But an `--exclude` that has gone stale still covers everything else; a selector that
has gone stale covers nothing, and the gate's whole subject vanishes at once. The tell is the same,
and it is not a failure. A gate that passed before your change and passes after it, on a change that
is precisely its subject, has probably stopped looking. Run it against a deliberately wrong name
once and confirm it still says no.

The habit that catches all of them is the one the program clause asks for, applied a directory
wider. Grep the path (`crates/<name>`) as well as the identifier. Then build every workspace, not
the one `cargo build` means by default. `find . -name Cargo.toml -maxdepth 3 | xargs grep -l
'\[workspace\]'` is the enumeration, outside `vendor/` and `target/`; it lists twelve on
2026-10-13.

## A suffix rename is one decision and N provenance blocks

The cost of a rename scales with the names it touches. The cost of *repairing* one scales with the
records those names appear in, and a family rename makes the second number much larger than the
first. Milestone 265 (`_proto` is a truncation) renamed fifteen crates by changing one suffix, which
is one decision. It then had to read every one of their provenance blocks, because a provenance
block's job is to record what the name was.

Two shapes recur and are worth expecting:

- The same account, copied into several crates. Four crates (`clock_proto`, `entropy_proto`,
  `supervision_proto`, `swap_proto`) carried a byte-identical sentence about the wire contract
  having been spelled four ways on 2026-07-30. A sweep breaks all four the same way, so the repair
  is also four copies. Finding one is no evidence you have found them all. Grep the sentence, not
  the name.
- An account whose subject is the spelling itself. Those four sentences list four spellings in
  order to contrast them, and two of the four ended in the suffix being renamed. Swept, the sentence
  still parses and is now nonsense: it contrasts four spellings, two of which no longer contain the
  thing being contrasted. Nothing catches that but reading it.

The general rule: when the thing being renamed is a *convention* rather than a single name, every
passage arguing the convention is an account. There are as many of them as there were names.

## The census is the checklist, and it is taken twice

Contributed by the fourth performed rename (`nvme` to `non_volatile_memory_express`, 2026-09-18).
It is the first one large enough that no person could hold the file list in their head: 615
occurrences across 93 files, against the `asid` rename's 446 and the two before it in the dozens.

Take the full census before and after, and classify every line of the second one. Not the count, the
lines. The after-census is the only artifact that asks "why is this one still here?" of each
survivor individually. On this rename it caught three sites every other check had passed over:

- a module header still calling itself "the volatile half of the `nvme` crate" and still saying
  `Provisional` after the ratification
- a `println!` prefix naming the driver in the one place a user actually reads it
- `kernel/src/nvme.rs` written into two QEMU runner scripts as a comment

`script/lint`, `script/names` and a green `script/test` on four legs had all passed with those in
the tree. None of them is a link, a symbol, or a name a gate parses.

The classification is also the report and the block's own evidence. Sorting the survivors into kinds
takes one script: the standard as a proper noun, the emulator's device name, an account, a spec
citation, a slug, a neighbouring crate's identifiers. It turns "most occurrences are not the crate"
from a thing a brief asserts into a number the next lane can check. Here it was 241 / 71 / 68 / 30 /
26 / 26, and that distribution is now in the crate's own provenance block.

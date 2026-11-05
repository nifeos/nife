---
status: NOT-STARTED
raised: 2026-09-13
milestone_dependencies: none
decision_dependencies: 182
machine_requirements: none
specific_machine: none
needs_person: no
---
# 398. Provenance for the names two programs agree on

Premise re-read against the tree on 2026-09-19 and still true. `crates/measured_boot/src/lib.rs` still carries the record on `PROGRAM_MEASUREMENTS`, saying in its
own prose that the entry name is provisional and that it "sits outside the four surfaces
`script/names` enumerates", and `script/names` still enumerates exactly those four. Two figures
moved: `--unratified` is 72 deep rather than 97, and the program surface is `components/` and
`fixtures/` rather than `user/src/` since milestone 175. **The cheap half this block asks for first
has a first answer**: `pub const <NAME>: &str` matches 77 times across `crates/`, `components/`,
`fixtures/` and `kernel/`, which is the order of magnitude that decides between a list in
`design/naming.md` and a declared out-of-scope. It is an upper bound rather than the count,
since not every one of the 77 is agreed between two binaries.

The decision is
§182 (is a string two binaries agree on a name for `script/names`' purposes, or is it data?), written up
2026-09-19 by milestone 435's slice-c lane because this gate named no section. What counts as a
surface is a scope question, and scope decides how large the worklist calef is handed becomes.
Whether to widen at all is his call, not a lane's.

## In brief

`script/names` enumerates four kinds: a crate (`crates/*/src/lib.rs`), a program (`user/src/*.rs`),
a `script/` entry point, and a Cargo package (any `Cargo.toml` outside `crates/`). The `package`
kind was added on 2026-08-18 because `script/names std_exerciser` had been answering *"neither a
name in the tree nor a recorded refusal"* for weeks about eight real packages. The note written
at the time is the whole argument for this one too: **a registry with a hole is worse than no
registry, because it answers confidently about the names it happens to cover.**

`crates/measured_boot/src/lib.rs` carries this, on the constant rather than on the crate:

```
/// Name: **provisional**. Under `nifefs`'s `NAME_LEN = 32` with room to spare.
pub const PROGRAM_MEASUREMENTS: &str = "program_measurements";
```

`"program_measurements"` is an archive entry name. The kernel's trust root names it and init reads
it out of the archive at boot. So it is a string two programs agree on, which `AGENTS.md` puts
in the expensive, hard-to-reverse category alongside a wire format and an opcode number. It has a
provisional name, its author said so, and `script/names --unratified` has never listed it because
the surface does not exist.

## Why this is the same defect the `package` kind fixed, and why it is worse

A package name is read in a `[dependencies]` list, which is at least inside this repository. A wire
string is agreed between two binaries and is the category `AGENTS.md` says cannot be un-shipped.
The names with the least reversibility are the ones with no record.

It is also not one instance. Candidates, unenumerated, which is the point:

- Archive entry names (`program_measurements`, and whatever else the loader agrees with init on).
- The `*_proto` crates' operation names and error codes, which are already crates and already carry
  blocks *for the crate*, not for the words on the wire.
- Public function and method names, calef's since 2026-08-23, counted by nothing. Milestone 276's
  `BUGS` already records that its naming columns miss them.

## What would have to be decided first

1. Is a wire string a "name" for this purpose, or is it data? The argument for yes is that a
   reader meets it and cannot change it; the argument for no is that the worklist would grow by an
   amount nobody has counted, and `script/names --unratified` is already 97 deep.
2. **Where does its block live?** Not in the file's header, because milestone 283 reserved that
   spelling for the file's own one block, and a second one would be unreachable by construction. So
   either a distinct spelling (`Value:`? `Wire:`?) or a different mechanism entirely.
3. **How is the set enumerated?** The four existing kinds are a directory listing or a manifest
   walk. A wire string is a `const` in an arbitrary file, and a scheme that cannot enumerate its own
   surface is the hole this proposal is about, one level in.

The counting is the cheap half and should come first. Before widening anything, a sweep for
`pub const <NAME>: &str` and the `*_proto` operation tables would say how many names this actually
is. If it is a dozen, the answer is probably a list in `design/naming.md` and no new machinery. If it
is two hundred, the answer is probably that wire strings are out of scope and the record says so on
purpose, the way `unrecorded` is a first-class answer rather than a gap.

## What milestone 283 did in the meantime

Reworded the `measured_boot` record to say the same things in prose, and to say why it does not wear
the `Name:` header spelling. The record is preserved and is one hop from the constant; what it does
not have is a gate, and this proposal is where that goes rather than into a `BUGS` entry nobody is
measured against.

## Index row

`script/names` enumerates four kinds of named thing (a crate, a program, a `script/` entry point and
a Cargo package). Milestone 283 (one provenance block per file, in the spelling the parse reads)'s
gate fired on a fifth, `measured_boot`'s `PROGRAM_MEASUREMENTS`, an archive entry name that nothing
has ever listed. It is a string two programs agree on, which AGENTS.md puts in the expensive,
hard-to-reverse category beside a wire format and an opcode number. Its name is provisional and
`--unratified` has never listed it. The names with the least reversibility are the ones with no
record. Three things must be decided first. Is a wire string a name for this purpose, or data? Where does
its block live, given that the file header is the one 283 reserved? How is the set enumerated, since
a `const` in an arbitrary file is neither a directory listing nor a manifest walk?
Counting is the cheap half and comes first. If it is a dozen names, a list in `design/naming.md`
answers it. If it is two hundred, wire strings are probably out of scope, and the record says so on
purpose, the way `unrecorded` is an answer.

---
status: NOT-STARTED
raised: 2026-09-19
promoted_from: a-const-in-a-proof-module-escapes-its-exclusion
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 492. A const inside a proof module is not excluded, because its mutant has no module path

*(Number provisional until the merge queue lands it.)* Promoted from the
proposal `a-const-in-a-proof-module-escapes-its-exclusion`, filed 2026-09-19, on calef's instruction
of 2026-09-20 to give every proposal on `main` a number. The text below is the proposal's own,
unedited except for this paragraph: the argument is its author's and promotion is not the moment to
improve it. Found by milestone 438 (would a diff-scoped mutation check have caught) while replaying `cargo mutants --in-diff` against milestone 319 (the crate that parses firmware had no)'s
pull request. Two of the four survivors it reported are mutants that `cargo test` can never kill, in
a module `cargo test` never compiles.

One entry in `.cargo/mutants.toml` and a paragraph saying why, which is the same
shape milestone 326 already used for `**/src/proofs.rs`. Nothing has to build or boot first.

**What is wrong.** `.cargo/mutants.toml` excludes the Kani and loom harness modules by
`exclude_re = ["verification::", "proofs::", "interleavings::"]`, matched against the mutant's
name. A function inside `#[cfg(kani)] mod verification` gets `verification::` in its name and is
excluded. A `const` or `static` inside the same module does not: cargo-mutants names its mutant by
the file and the operator alone, with no module path at all, so the regex has nothing to match.

```console
$ cargo mutants --list --workspace | grep 'x86_64.rs:314'
crates/machine_discovery/src/x86_64.rs:314:29: replace + with -
crates/machine_discovery/src/x86_64.rs:314:29: replace + with *
```

Line 314 is `const N: usize = V1_LEN + 1;`, four lines inside `#[cfg(kani)] mod verification`.

How big it is: three mutants, today. Cross-referencing the 9,656 mutants
`cargo mutants --list --workspace` generates against the line spans of every `#[cfg(kani)]` and
`#[cfg(all(test, loom))]` module in `crates/` and `components/` finds exactly three: the two above
and `crates/network_time_protocol/src/lib.rs:1323`, which is `const PROVED_NS: u64 = 1 << 16;`.
Most such items are plain literals or struct values with no operator to mutate, which is why the
number is small; it grows the next time a harness needs a computed bound.

Why it is worth an entry rather than a shrug. It is the third appearance of one defect.
Milestone 244 found `system_initializer` scored 191-mutants-0-caught because `--exclude` does not
remove a package from the dependency graph. Milestone 280 (unexplained holes in the published score)
found `uefi_loader`'s `[[bin]]` half scored on a file nothing compiles. Milestone 326 (nobody has
been assigned to turn a mutation score upward) found 36 of `timetable`'s 48 survivors were its own
proof harnesses, because the module-path regex cannot see a module that is its own file. Each time
the number moved for a reason that was arithmetic rather than a finding, and each time it was found
by somebody triaging by hand. This one is the same defect keyed on the item kind instead of the
file layout.

The shape of the fix, for whoever takes it. A regex over the mutant name cannot express "inside
a `cfg`-gated module", because the name does not carry that. What the tree can express is the file
and line, which `exclude_globs` cannot reach either. So the honest options are a narrow name regex
per case (cheap, and it rots). Or a `script/lint` check that does the cross-reference above and
fails when a mutant's line falls inside a `cfg(kani)` or `cfg(loom)` module span (rung two, and it
cannot rot). The second is what milestone 244's bare-metal gate already does one level over:
derive the list rather than remember it.

**What it blocks: nothing.** Three mutants is not a score. It is on the list because the same defect
has now cost four triages, and because a proof lane that trips it reads as a lane with untested code.

## Index row

What is wrong. `.cargo/mutants.toml` excludes the Kani and loom harness modules by `exclude_re =
["verification::", "proofs::", "interleavings::"]`, matched against the mutant's name.

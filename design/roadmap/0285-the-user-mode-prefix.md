---
status: BUILT
raised: 2026-09-13
built: 2026-09-13
---
# 285. `user_` meant a person in three hundred places, so the two crates that meant a privilege level took a longer prefix

Built 2026-09-13. calef ratified `crates/user_rt` to `crates/user_mode_runtime` and
`crates/user_heap` to `crates/user_mode_heap` the same day, working the unratified worklist, and
ruled the prefix itself in the same breath. *(Number provisional until the merge queue lands it.)*

Both together on purpose: renaming one would have left the family split, and the prefix is the half
that had never been argued at all.

## The two halves, argued separately

`rt` was an abbreviation that needs a decoder, the first of the three failure modes AGENTS.md
lists, and the crate's own first line had always spelled it out ("the tiny EL0 runtime shared by
nife userspace programs"). The precedent is exact: `cred_proto` became `credential_proto` on
2026-08-23 for "spell out the contraction fully". Nothing outside this tree owns the spelling,
and that is what decides it. The 2026-09-05 acronym test spares a term the field itself uses, which
is why `pci`, `elf` and `gpt` survive: a reader arrives already knowing them, so the expansion
teaches nothing. `rt` has no owner of that kind. No specification uses it, no wire format carries
it, no command-line flag spells it; it is a shortening somebody typed, and the test applies at full
force.

`user_` was the harder half, and a count is why the answer is `user_mode_` rather than `user_`.
In this tree `user` overwhelmingly means *a person*. At 32fccda8, `git grep -oiw` over the whole
tree answers `a user` 291, `the user` 254, `users` 68 and `per-user` 19, which is 632 occurrences of
the word doing that job. Milestone 49 is users and attribution, `identity_provisioner` creates them,
`login` authenticates them, and `crates/schedule_store` calls itself "the on-disk, per-user schedule
store". So `user_runtime` would have fixed the abbreviation and left the ambiguity exactly where it
was: a reader meets it and cannot tell whether the runtime belongs to a person or to a privilege
level.

"User mode" is this tree's architecture-neutral phrase for that level, and the counts beside it
say why it is the right one rather than the commonest one. The same grep answers `EL0` 1375,
`U-mode` 252 and `ring 3` 123, each of which is **one** architecture's word for the level, against
`user mode` 59, which is what this tree writes when a sentence has to cover all three. So
**`el0_runtime` was refused** for naming one architecture's privilege level on a crate that ships on
three, which is what rule 5 exists to catch, and the plain phrase wins on reach rather than on
frequency. The same defect was live one level down
and is fixed here: the crate's first line described a three-architecture crate as "the tiny EL0
runtime", and `.cargo/mutants.toml` and `script/coverage` each said "EL0 system call runtime" about it.

## What the ruling closed, which is more than two names

Milestone 63 ratified `user_heap` over `uheap` on 2026-08-01 partly on the ground that "`user_rt`
already establishes `user_` as the prefix for it". The prefix was therefore on the record because
this crate had established it, and this crate's name had never been argued for itself. Milestone
264 called it "a circle rather than a reason" in the crate's own provenance block, and milestone 63's
table is where the circle is written down.

Both provenance blocks now record the ruling rather than the precedent. The 2026-08-01
ratification keeps its own reasoning in the spelling it used, because a refusal record whose reason
has been swept into today's vocabulary is a record that lies about what was argued.

## The hazard that decided the work, and it was already a scar

**A crate name that has left Rust is not compiler-checked, and this tree learned it eight hours
earlier.** When milestone 175 moved `user/link.ld` to `crates/user_rt/link.ld`, two `build.rs` files
were missed because they live in separate Cargo workspaces; nothing errored until a
cross-compiling link, and CI's QEMU leg went red with nothing before it saying a word.

There are **four** build scripts holding that path and **two** of them are outside the main
workspace, so `cargo build` at the root proves nothing about half of them:

| Build script | Workspace |
|---|---|
| `components/build.rs` | main |
| `fixtures/build.rs` | main |
| `redoxfs_server/build.rs` | **separate** |
| `std_exerciser/build.rs` | **separate** |

Seven more sites hide the same way, and they are listed with the reason each is invisible in
[design/naming.md](../naming.md)'s new "a crate is compiler-checked only where a compiler is
looking" table: `--exclude <crate>` arguments in `script/lint`, `script/coverage` and two lists in
`xtask`, where cargo takes an unknown name in silence. `.cargo/mutants.toml`'s exclusion glob and
`.cargo/mutants-baseline.txt`'s crate-keyed row. A `reaches_user_rt()` identifier inside
`script/lint`'s embedded python. `helpers/build-ripgrep.sh`, which seds the linker script into a
high-load variant by path. The module `xtask` generates into the patched-`std` overlay
(`sys/alloc/nife/user_heap.rs`, declared `mod user_heap;`), which compiles only when the farm is
rebuilt. And the `Cargo.lock` of each separate workspace.

That table is this milestone's real deliverable beside the names. The existing clause said
renaming a crate is the easy case because `cargo check` finds every site missed, and it is too
generous by exactly the sites above.

## The enumeration

At 32fccda8, `user_rt` was 818 occurrences in 223 files and `user_heap` 79 in 37, and the forms were
not one string: 301 bare, 69 more as `user_rt::` in front of a brace or a type. Then
`::panic_handler` (82), `::mapped_window` (59), `::exit` (54), `::heap` (43), `::trap` (31), `::now`
(23), `::cntfrq` (23), `::initrd` (22), `::map_page_frame` (18) and a long tail. Eight manifests
declare one of the two as a dependency and a ninth names `dep:user_rt` in a feature
(`redoxfs_server`'s `el0`), which is the site a `[dependencies]` sweep walks past. Nothing was swept
blind; the sets were listed, read, and then edited, which is what caught the records below.

## What did not move, and why each

- The `uheap` refusal and its reason. `uheap` was refused on 2026-08-01 because the `u` was
  "userspace" and had to be decoded, "while `user_rt` already establishes `user_`". Rewriting that
  sentence in today's spelling would make the record claim an argument nobody made. It is the tree's
  oldest naming scar: a blind `sed` once rewrote the very row recording that a name had been refused.
- A quoted commit subject. `notes/unsafe-obligations.md` quotes `d5a969a2`, "user_rt: one trap
  instruction, not forty-eight". The quotation keeps its bytes and gains a note beside it saying the
  crate is spelled differently now, so the number stays traceable to the run that produced it.
- A quotation from milestone 68's block, inside what is now
  `design/roadmap/0345-pure-halves-of-the-user-rt-crates.md`, for the same reason. That block's own
  prose moved, because it is live intent a reader picks up and goes looking with.
- `design/decisions/` and every other milestone's roadmap block, which are outside a lane's
  reach. The count and the rows that now mislead are in the Follow-on section.

## BUGS

- The slug is still `pure-halves-of-the-user-rt-crates`, now as
  `design/roadmap/0345-pure-halves-of-the-user-rt-crates.md`. The obstacle this entry named is gone:
  milestone 433 numbered it on 2026-09-19 and milestone 68's bullet became `**Milestone 345.**` with
  no path in it, so a rename no longer reaches into another block. The body is current; only the
  slug is stale, and a slug is a name, so it is an architect's.
- **Four `NOT-STARTED` and eight `PARTIAL` roadmap blocks still spell the crate `user_rt`.** They
  are live intent pointing at a directory that no longer exists, and they are listed below rather
  than fixed, for the same reason. Four other blocks had to be touched despite that rule, because
  `script/roadmap --check` gates on a `## Follow-on` citation resolving to a real path and theirs
  stopped resolving the moment the directory moved. 80, 228, 230 took a one-path edit, and 264's
  entry, which is the one this milestone performed, became `**Milestone 285.**`.
- The `user_mode_` prefix has exactly two members and no rule says what else joins it. Nothing
  in the tree currently needs a third, and inventing the rule before the third case would be
  building an abstraction ahead of its requirements.

## Follow-on

- **Recorded.** In this block's BUGS section above, in
  `design/roadmap/0285-the-user-mode-prefix.md`: thirty-seven roadmap blocks and ten sections under
  `design/decisions/` spell `user_rt` or `user_heap`, 112 occurrences and 25. Twelve of the blocks
  are unbuilt (four `NOT-STARTED`, eight `PARTIAL`) and are therefore live intent pointing at a
  directory that no longer exists; the rest are dated narrative and correctly keep the old spelling.
  The two that mislead hardest are index rows rather than blocks. Milestone 185 says two of the
  programs it will sweep "call `user_rt::trap()`". Milestone 278 says the crate "is excluded from
  the host pass, coverage, mutation and Kani", under its old directory, and closes with "does not
  settle whether `user_rt` keeps its name", which this milestone settled. One edit each, for whoever
  has the standing.
- **Recorded.** **A crate name outside Rust is not compiler-checked**, in
  [design/naming.md](../naming.md)'s "Performing a ratified rename" section, beside the
  clause it corrects. Eight kinds of site, each with why the compiler is blind to it and the habit
  that finds it: grep the path as well as the identifier, and build every workspace rather than the
  one `cargo build` means by default.

## Index row

calef ratified `crates/user_rt` to `crates/user_mode_runtime` and `crates/user_heap` to `crates/user_mode_heap` on 2026-09-13, and ruled the prefix itself in the same breath, which is
why both moved together. `rt` was an abbreviation with no owner outside this tree, the `cred_proto`/`credential_proto` case again. `user_` was the harder half, because `git grep -oiw`
answers `a user` 291, `the user` 254, `users` 68 and `per-user` 19, so the prefix read as *a
person* rather than as a privilege level. `user mode` is the architecture-neutral phrase (59
uses) where `EL0` (1375), `U-mode` (252) and `ring 3` (123) are each one architecture's, so `el0_runtime` was refused under rule 5 for a crate that ships on three. Closes milestone 63's
circle: `user_heap` was ratified partly because "`user_rt` already establishes `user_`", so the
prefix rested on a crate nobody had argued. 818 occurrences in 223 files plus 79 in 37, swept by
enumeration; the `uheap` refusal record, a quoted commit subject and a milestone 68 quotation kept
their spelling. Found that the "renaming a crate is compiler-checked" clause is too generous
and wrote the eight sites it misses into design/naming.md. Four `build.rs` linker-script paths, two
of them in separate workspaces. Plus `--exclude` arguments, two mutation-testing files, a derived
identifier in a gate's python, a shell script that seds the script by path, the generated module
in the patched-`std` overlay, and each separate workspace's lockfile.

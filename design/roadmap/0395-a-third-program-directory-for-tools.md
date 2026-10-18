---
status: NOT-STARTED
raised: 2026-09-13
milestone_dependencies: none
decision_dependencies: 180
machine_requirements: none
specific_machine: none
needs_person: no
---
# 395. A third program directory, for the tools a person invokes

Filed 2026-09-13 as an unnumbered proposal by milestone 175's
classification pass, which its own block asked for in as many words; numbered 2026-09-19 by
milestone 433's drain of the proposal pile. **Premise re-read against the tree on 2026-09-19. The
argument is unchanged and two of its numbers are not**, both moved by work that landed the same day
this was filed. `watch` was cut by milestone 281, on calef's *"We can cut `ps` with the `watch` and simplify."*,
and `doc` was ratified as `mdr` in the same afternoon's worklist. So the list below is
fifteen non-service programs rather than sixteen, and nine of them are typeable at the
prompt rather than ten: `grant_plan::Program::from_name` resolves `date`, `rm`, `wc`, `mdr`, `ps`,
`pgrep`, `uptime`, `printenv` and `uuid`. `components/Cargo.toml` now carries 47 `[[bin]]` entries
and `fixtures/Cargo.toml` 40. Two entries in the table below are stale as written and are left as
written, because a reader needs to see what was counted; this paragraph is the correction.
*(Number provisional until the merge queue lands it.)*

The decision is
§180 (whether `components/` splits again), written up 2026-09-19
by milestone 435's slice-c lane because this gate named no section.
A top-level directory holding a Cargo package is named exactly as the package.
So this is a name, and names are an architect's. It is also the most expensive kind of name in this
tree: it lands in `xtask`, in four exclusion lists, in `script/lint`, `script/names` and
`script/verify`, and in every citation that points into it.

## What milestone 175 found

175 split 74 programs into `components/` (49) and `fixtures/` (23, plus two `#[path]` modules). The
rule it used was one question, *would a distribution ship this because somebody wants its function?*,
and it works: no program was genuinely hard to place under it.

**But sixteen of the forty-nine components are not services**, and 39's option A already listed
`tools/` beside `components/` for exactly them:

| | |
|---|---|
| `date`, `printenv`, `uptime`, `uuid` | read one thing, print it, exit |
| `ps`, `pgrep`, `pmap`, `watch` | look at the running system |
| `wc`, `rm`, `doc`, `rmle` | operate on what the command line designates |
| `disk_surveyor`, `disk_partitioner`, `identity_provisioner`, `swapper` | operator tools, run once on purpose |

Ten of the sixteen are already typeable at the prompt: `grant_plan::Program::from_name` resolves
`date`, `rm`, `wc`, `doc`, `ps`, `pgrep`, `watch`, `uptime`, `printenv` and `uuid` and nothing else
in that list, which is the closest thing this tree has to a `/bin`.

The remaining thirty-three are what the word was coined for: long-running servers and drivers
(`net_stack`, `gpu_driver`, `clock`, `login`, `credentialer`, the caretakers), and the init and
supervision spine (`progenitor`, `root_supervisor`, `spawner`, `builder`, `job_undertaker`).

## Why 175 did not take it

**The two-way split is not a forced fit, and that is the honest case against a third directory.**
Milestone 39's vocabulary says a component is *the shippable unit, a binary plus its manifest*.
A service is what a component offers. `wc` is a shippable unit with a manifest. It offers no
service, and nothing in 39's definition requires it to. So `components/` holding `wc` is coarse
rather than wrong, and a coarse-but-correct directory is a much smaller defect than a name taken
without the person who owns names.

And the cost of a third directory is real but bounded, which is the other half of the argument.
Sixteen `git mv`s, sixteen `[[bin]]` blocks moved between manifests, one more dependency set to
prune, a third `build.rs` (four lines, pointing at `crates/user_mode_runtime/link.ld` like the other two), and
roughly 150 path citations to repoint. 175 measured that cost by doing the same work at four times
the scale, and it is about an hour.

## The argument for doing it anyway, and for doing it soon

A reader meets a directory before they meet anything in it. `components/` with `wc.rs` next to
`net_stack.rs` tells a newcomer that those two things are the same kind of thing, and the tree's own
`notes/adding-a-program.md` now has to spend a paragraph saying they are not.

Doing it later costs the repoint twice. That is milestone 175's own argument for performing the
`interrupt_heeder`/`interrupt_ignorer` renames in the same change that moved the files, quoted from
its block: *"this milestone moves both files anyway and doing it twice is the cost of doing it
early."* The same sentence applies here one level out.

## What a decision needs to settle

1. **Whether the third category exists at all**, or whether "component" is deliberately the broad
   word and `tools/` would be inventing a distinction the capability model does not have.
2. The name. `tools/` is 39's own suggestion and is already spent once in this tree
   (`tools/redoxfs_host`, a host build tool, which is a different meaning of the word and would make
   `tools/` ambiguous). `bin/` is Unix's and says nothing about authority. Others worth refusing on
   the record rather than silently: `commands/`, `programs/`, `utilities/`.
3. Where `swish` goes. It is the shell, so it is what *runs* the tools rather than one of them,
   and it is as plausibly the spine as it is a tool.

## What it does not decide

Whether `virtio_net_transport` gets lifted into a crate (milestone 175 (split `user/`)'s block
leaves that open), and whether `crates/` gets 39's three-audience split (39 named that as a
separate strain and 175 explicitly refused to fold it in).

## Index row

Milestone 175 split 74 programs into `components/` and `fixtures/` on one question, *would a
distribution ship this because somebody wants its function*, and asked in its own block whether a
third directory would be more honest than forcing a fit. This is that answer written down instead of
taken. Fifteen of the components are not services. Four that read one thing and print it (`date`,
`printenv`, `uptime`, `uuid`), three that look at the running system (`ps`, `pgrep`, `pmap`), and four
that operate on what the command line designates (`wc`, `rm`, `mdr`, `rmle`). Four operator tools
run once on purpose (`disk_surveyor`, `disk_partitioner`, `identity_provisioner`, `swapper`). Nine of
them are already typeable at the prompt through `grant_plan::Program::from_name`, which is the
closest thing this tree has to a `/bin`. The honest case against is that the two-way split is not a
forced fit. Milestone 39 (repository structure for a loosely-coupled OS, and the road to a
distribution) defines a component as the shippable unit, a binary plus its manifest, and
`wc` is one. So `components/` holding it is coarse rather than wrong. A coarse-but-correct
directory is a much smaller defect than a name taken without the person who owns names. The cost is
bounded and measured against 175 doing the same work at four times the scale: about an hour, sixteen
moves, a third four-line `build.rs`, and roughly 150 path citations. The case for doing it soon is
that a reader meets a directory before they meet anything in it, and that doing it later pays the
repoint twice. A decision has to settle three things: whether the third category exists at all, and where `swish`
goes, since the shell is as plausibly the spine as a tool. The name is its own question: `tools/`
is already spent on `tools/redoxfs_host`, a host build tool, and `bin/` says nothing
about authority.

---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-28
ratified_by: calef
---

# 180. Whether `components/` splits again, for the tools a person invokes

Ruled 2026-10-28 (UTC): No third directory. `components/` stays the broad word; no third directory
for interactive tools. First-party programs move to package-specific repos under §297, so a directory
split in `components/` is busy work rather than an answered question.

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice-c lane, which found milestone 395 (a third program directory, for the tools a person invokes)'s
`DECISION` gate naming no section. Milestone 175 asked this question in its own block and correctly
did not answer it, because the answer is a top-level directory name. *(Section number provisional
until the merge queue lands it.)*

## What is being decided

Three things, and the first decides whether the other two are asked at all:

1. Does a third category exist, or is "component" deliberately the broad word and a `tools/`
   directory would invent a distinction the capability model does not have?
2. What it is called, if it exists.
3. Where `swish` goes, since a shell runs the tools rather than being one of them, and is as
   plausibly the spine.

## Is the premise true

Checked 2026-09-19 in this worktree. `components/Cargo.toml` carries 47 `[[bin]]` entries and
`fixtures/Cargo.toml` 40, so the split milestone 175 performed is real and current. Fifteen of
the components are not services:

| | |
|---|---|
| read one thing, print it, exit | `date`, `printenv`, `uptime`, `uuid` |
| look at the running system | `ps`, `pgrep`, `pmap` |
| operate on what the command line designates | `wc`, `rm`, `mdr`, `rmle` |
| operator tools, run once on purpose | `disk_surveyor`, `disk_partitioner`, `identity_provisioner`, `swapper` |

Nine of the fifteen are typeable at the prompt. `grant_plan::Prog::from_name` resolves `date`,
`rm`, `wc`, `mdr`, `ps`, `pgrep`, `uptime`, `printenv` and `uuid`, which is the closest thing this
tree has to a `/bin`. Milestone 395's own block corrects two earlier figures here: `watch` was cut
by milestone 281 and `doc` was ratified as `mdr`.

*(Dated note, 2026-09-26, decisions-hygiene lane, recounted against `main` at `508333ebe`.)* The
figures above have moved and the premise has not. `components/Cargo.toml` now carries 50
`[[bin]]` entries and `fixtures/Cargo.toml` 43. Two of the new components belong in the table
above: `top` (look at the running system, 2026-09-21) and `installer` (an operator tool, 2026-09-21),
which makes seventeen that are not services. `grant_plan`'s program table now resolves ten of
them by name, the nine above plus `top`. The third new component, `framebuffer_driver`, is a driver,
and `audit_sink` became `login_audit_receiver` (calef's 2026-09-13 ruling, performed 2026-09-24).
`tools/` still holds only `redoxfs_host`.

## What this tree already does in the analogous case

**§39 defines the vocabulary and does not force this.** A component is the shippable unit, a
binary plus its manifest; a service is what it offers; a contract is the wire protocol.
`wc` is a shippable unit with a manifest. It offers no service, and nothing in §39 requires it to.
So `components/` holding `wc` is coarse rather than wrong, which is the honest case against a
third directory: a coarse-but-correct directory is a much smaller defect than a name taken without
the person who owns names.

§63 draws the tree's other line in this area (a crate is the logic, a program is the authority)
and it cuts across this one rather than along it, so it neither supports nor refuses a third
directory.

§75 is the mechanism a new directory would owe: a directory under `design/` or `notes/` carries
its provenance in its own `README.md`. A top-level program directory is outside that rule's scope
today, which is itself worth noticing: `components/` and `fixtures/` carry no such record.

## Whether the name is available, read rather than recalled

`tools/` is already spent. `tools/redoxfs_host` exists in this worktree and is a host build
tool, which is a different meaning of the word. Taking `tools/` for EL0 programs would give one
directory name two senses, which is the ground §154 and naming.md's recognition argument already
refuse elsewhere (milestone 440's block makes the same refusal of `runner` on a measured 163 files).

The alternatives, so they are refused on the record rather than silently: `bin/` is Unix's and says
nothing about authority, which is the §39 (a component is named for what it is) failure wearing a
different hat. `commands/`, `programs/` and `utilities/` are each one of the generic words naming.md
names as a failure mode.

## What it costs, measured

Milestone 175 performed the same work at four times the scale and priced it: about an hour.
Fifteen `git mv`s, fifteen `[[bin]]` blocks moved between manifests, one more dependency set to
prune, a third four-line `build.rs` pointing at `crates/user_mode_runtime/link.ld` like the other
two, and roughly 150 path citations to repoint.

Doing it later costs the repoint twice, which is milestone 175's own argument for folding the
`interrupt_heeder`/`interrupt_ignorer` renames into the change that moved the files.

## Recommendation

None on question 1, because it is a name, and a directory name is the most expensive kind in this
tree. It lands in `xtask`, in four exclusion lists, in `script/lint`, `script/names` and
`script/verify`, and in every citation that points into it. AGENTS.md's rule is that a fork this
irreversible arrives with options rather than with a winner.

What this section does recommend is the sequencing: if question 1 is yes, it should happen soon
rather than eventually. The reason is that a reader meets a directory before they meet anything in
it, and `notes/adding-a-program.md` already spends a paragraph saying that `wc.rs` and
`net_stack.rs` are not the same kind of thing.

## What this does not decide

Whether `net_transport` is lifted into a crate (milestone 175 left it open), and whether `crates/`
takes §39's three-audience split, which §39 named as a separate strain and 175 refused to fold in.

## What is blocked until this is answered

Milestone 395, and nothing else. The tree is correct as it stands; what it is not is legible to
a newcomer at the directory level.

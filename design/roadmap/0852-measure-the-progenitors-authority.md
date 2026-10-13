---
status: NOT-STARTED
raised: 2026-10-06
promoted_from: measure-the-progenitors-authority
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 852. Measure the progenitor's authority

*(The title and slug are provisional.)*

calef asked for this on 2026-10-06 (UTC): "Record a separate proposal to measure the progenitor's
authority." He was ruling on #1783, milestone 805 (`reboot` at the prompt). That build had the
progenitor flush the filesystem on `reboot`'s behalf and keep one more capability for the life of
the boot, 33 of 64 slots. He sent it back so that `reboot` holds a sync-only capability of its
own, and gave the reason: "My concern is progenitor is turning into a god process."

The pattern behind the worry is an easy path. A program needs something sensitive, and the
progenitor already holds it, so the progenitor does it for them. Each step is small and locally
sensible. Nothing in the tree shows the sum. Written by lane `progenitor-authority`, which built
nothing but this file.

Reuse: the live-table print reuses `kernel::cap::report_peak`'s shape and its `swish-check`
hook. seL4's CapDL describes a system's initial capability distribution as a spec a loader
realizes (recalled, not read). It is prior art for the table's shape, not code to take. The table
is a Rust struct in a crate the progenitor already builds from.

## The question the data answers

What does the progenitor hold for the whole boot, and what does it do on other programs' behalf?
For each item: why, and could a narrower holder or a narrower capability do the job?

The reader is calef, deciding remediation. The second reader is every reviewer after him, who
should see the progenitor's authority grow as a diff and not discover it at a slot wall.

## Where the tree starts

A quick read on 2026-10-06, not a measurement. Every number here is a rough count.

- The kernel grants the progenitor about 31 named slots at boot (`BootEndowment` in
  `crates/system_initializer`, mirrored by the `GRANTS` table in `components/src/progenitor.rs`).
  Several are empty when a device is absent. Slot 28, its own address space, is the newest.
- `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` records the peak as a count, and
  `script/swish-check` fails when a boot passes it. It says how many. It does not say which, and
  its own doc admits that for the graphical arm: "Which grants sit on the peak is not traced."
- The spawn service performs five package verbs for the shell (`spawnproto::Activation`: install,
  remove, rollback, fetch, vouch).
- At spawn it endows a child from its own holdings for about a dozen `grant_plan::Manifest`
  fields. Entropy, the network stack and the clock are among them. The entropy and network
  endpoints are kept for the life of the boot for exactly this reason. The peak record says so in
  its paragraphs for milestone 111 (a shell that can endow a child with entropy) and milestone
  590 (the booted system starts its network stack).
- The login block mints and places capabilities for `login`, such as the durable window
  (milestone 152 (durable delegation)).

So the facts exist, scattered through doc comments that each explain one slot well. No artifact
lists them together, and no gate fires when one is added. That is the rung-four state the
mechanisms ladder warns about.

## The measurement

The milestone produces a generated, checked-in inventory, and a gate that fails when it is
incomplete. Every name below is provisional.

### The source table

A table in `crates/system_initializer` (provisional `authority.rs`), one row per item. A row is a
struct whose fields have no defaults, so a row without a reason does not compile:

| field | holds |
|---|---|
| `slot` | the slot, or "first free" for a capability placed by the progenitor itself |
| `object` | the kernel object type |
| `rights` | the rights held |
| `acquired` | kernel grant at boot, minted during boot, or received from a child |
| `released` | never, or the point it is deleted (after a build, after a hand-off) |
| `present_when` | always, or the device or configuration that makes the slot non-empty |
| `reason` | why the progenitor and not a narrower holder |
| `narrower` | what a narrower holder or capability would need, or "none known" |

A second table lists proxied actions: what the progenitor does, for whom, through which held
capability, and why.

### The generator and the lint

`cargo xtask progenitor-authority` (provisional) renders both tables to
`notes/progenitor-authority.md` (provisional). A new `script/lint` check runs it with `--check` and
fails when the checked-in file differs. The generated note is what a reviewer reads in a diff.

### Proof that it is complete

Two gates, on two rungs.

1. Capabilities, rung 2. When the progenitor reaches the prompt, it prints one console line per
   occupied slot: slot, object type and rights. `script/swish-check` compares that set against the
   rows whose `released` is never, and fails on any slot the table does not name. This is
   `report_peak`'s shape, which has caught three additions on their first run. The same line taken
   at the peak would cover transient holdings too. The milestone decides whether that is worth the
   second print; until it does, transient rows are checked by the existing peak count only.
2. Proxied actions through the two enumerable surfaces, rung 1. The proxied-action table is built
   by an exhaustive `match` over `spawnproto::Activation` and a destructuring of
   `grant_plan::Manifest` with no `..`. A new verb or a new endowed field does not compile until it
   has a row.

What this does not catch, said plainly. A proxied action that uses a capability the progenitor
already holds, through no new verb or field, adds no slot and no variant. #1783's flush was that
shape: `fs::SYNC` on an endpoint the progenitor already had. The milestone should measure how many
such sends exist, and say whether a mechanism can find them. If none can, the gap is a `BUGS`
entry beside the table, and review is the catch.

## The rule it proposes

Each new lifetime capability or proxied action needs a row with a written reason. The gate
enforces that the row exists, and the struct enforces that the reason is not empty. A reviewer
then sees growth as a diff to one generated note, with the reason beside it. Whether the reason is
good stays a review question, as it must.

## What it does not decide

No capability is removed and no action moves in this milestone. Remediation is decided later,
with the inventory in hand, per measure first. Some candidate remediations, as examples only:

- A narrower capability minted by the service that owns the object, as #1783's rework did with a
  sync-only filesystem capability.
- A separate small server that holds one authority, so the progenitor hands out a client view and
  forgets the original.
- Releasing an item after its last use, where the inventory shows a lifetime capability with one
  use at boot.
- Moving the installer out of the progenitor. Milestone 809 (the package client becomes a program)
  keeps the installer there, because the progenitor alone reads the activation set; the inventory
  would say what that costs.

## Parity

The slot layout is already one table on all three architectures (milestone 166 (one boot loader, reached two inconsistent ways)). The one
per-architecture difference is the shape of slot 1's console authority: a device page on aarch64
and riscv64, a port range on x86_64 (milestone 299 (the x86 port-range capability)). The `object` field for that row takes a
`cfg`-selected value, and nothing else in the table varies by ISA.

The gate runs where `report_peak` already runs: `script/swish-check`, whose legs boot aarch64,
riscv64 and x86_64. Device rows carry `present_when`, so a leg without a NIC or a gpu checks their
absence, not their presence. A row that holds on one architecture and is silently absent on
another fails on the leg that disagrees.

## Forks for calef, both ruled

calef ruled on both on 2026-10-06 (UTC), in comments on #1785. No fork is open, so
`decision_dependencies` is `none`. The section Fork 1 calls for is minted at promotion, and the
milestone writes it.

### Fork 1: does the rule need a `design/decisions/` section?

Ruled: "Approve A" (calef, 2026-10-06 UTC). The rule gets a short section, numbered at promotion.
It cites calef's concern that the progenitor is "turning into a god process".

Recommendation: yes, a short one, minted by the integrator when this lands. The rule binds what
every future milestone may ask of the progenitor. That is a constraint on the process model
§10 (the capability-based microkernel process model) set up, and the tree records constraints of
that kind as sections. The section would say three things: the progenitor's authority is
inventoried, growth needs a written reason, and least authority applies to it as to any other
program.

If no: the rule lives only in the inventory note's header and the gate. That works mechanically.
It leaves the principle unstated where a designer looks for principles. Reversible either way.

### Fork 2: is the gate a hard fail?

Ruled: "Approve A" (calef, 2026-10-06 UTC). The check fails hard in `script/lint` when the
generated inventory is stale, and in `script/swish-check` when the progenitor holds a slot the
table does not name.

Recommendation: hard fail, in `script/lint` for the generated note and in `script/swish-check` for
the live table. A warning nobody must act on is rung 4, and #1783 shows the growth arrives one
reasonable step at a time. The cost to a lane that adds a capability is one row with a reason,
which is the point. The peak count is the precedent: it hard-fails, and it has fired as designed.

If no: a warning in the job summary, and the inventory decays at the rate nobody reads warnings.

## Appendix: what the milestone owes

- The two tables, filled for every current holding and proxied action, with `narrower` answered
  for each.
- The generator, the lint check, and the `swish-check` comparison on all three legs.
- A count of sends the progenitor makes on another program's behalf through no enumerated surface,
  and the gap's home if no mechanism can find them.
- A summary for calef: which items have a narrower candidate and which have none. No changes.

## Index row

The progenitor's authority is inventoried: a checked-in table of every lifetime capability and proxied action with its reason, a generator, and gates that fail when a row is missing. Growth becomes a diff a reviewer reads rather than a slot count nobody can explain, which is the drift calef refused on #1783.

---
status: BUILT
raised: 2026-09-08
built: 2026-09-08
---
# 266. One progenitor, on all three architectures, and `init` stops being a role

Built 2026-09-08. Minted the same day by calef, who refused the guard rail that had been
quoted at him: *"init still isn't a good name even though it comes with history. This is the first process."*
*(Number provisional until the merge queue lands it.)*

It gated on nothing while it ran: the rename needed nobody's permission. The sequencing below was
not optional, and milestone 265's block carries the same constraint for the same reason.

## The argument, which turns on a word in the rule rather than against it

AGENTS.md protects standard terms that are genuinely right and says a name a reader already knows
from outside is the best available. The maintainer quoted that at `init` and calef's answer was that
`init` is not genuinely right, only familiar:

- It names an action for a thing. `init` is short for *initialize*, a verb, and AGENTS.md's own
  rule is *name things with nouns*, because a verb names an action and a process is not one.
- The thing is the first process, the one the kernel starts and from which every other descends,
  and the name says none of that.
- Familiarity is not correctness. `elf` is genuinely right because that is the format's name.
  `pci` is genuinely right because the expansion teaches nothing better. `init` is a truncated verb
  that a reader recognizes from Unix, which is a different thing from a name that is right.

And in this tree the role is larger than Unix's, which sharpens it. `crates/system_initializer`:
*"The kernel loads this as the boot process, maps the initrd, and grants it the capabilities...
From those, and nothing else, it builds the whole interactive system out of its own budget."* So
the first process here is not merely first in time; it holds the machine's entire authority at boot
and hands slices away. Milestone 22 is called trusted init for that reason, and
[§32](../decisions/0032-reap-without-build.md)'s fork about whether a supervisor can restart without
regaining construction authority is the same subject. *(That citation named decision 148 when this
block was minted, a number no decision has, and `script/decisions --check` failed on it from the
moment the block landed. Corrected by this milestone's lane. Written without the section sigil on
purpose: spelling it would make this sentence itself a dangling citation, which is the gate working
as designed.)*

## The name

`progenitor`. Ratified by calef 2026-09-08.

- It is the ancestor every process descends from, which is the actual relationship rather than a
  position in a sequence.
- It matches the house style, which is overwhelming: `builder`, `spawner`, `supervisor`,
  `caretaker`, `undertaker`, `provisioner`, `reviver`, `surveyor`, `responder`, `editor`,
  `initializer`. Thirty-odd agent nouns. A role here is named for who does the thing.

`prime` was calef's own first suggestion and he set it aside on the argument against it. In
English it usually means *chief* or *best* (prime minister, prime rib) rather than first in sequence,
and in a tree carrying Argon2, GUIDs and CRC polynomials a systems reader meets `prime` and thinks
number theory. That is a decoder problem, which is the failure mode the naming rule refuses.

`origin` and `first` were considered. `origin` captures what makes the role load-bearing here and
is not an agent noun; `first` is unambiguous and generic enough to sit with `compose` and `measure`
in the words that could name anything.

## What is in scope: one program, not a relabelling

calef, 2026-09-08: one progenitor for all three architectures. The rename falls out of that
rather than preceding it, and the reason is that the role exists to abstract over several
implementations and there should not be several.

Today three programs stand behind one archive entry:

| | what it is |
|---|---|
| `hello` | *"one binary, two roles"* that grew into the milestone 7-19 catalogue. On aarch64 the archive packs it **as** `init`, because that is where the PL011 wiring already was |
| `builder` | *"a minimal init... deliberately small and fully portable"*, which exists to prove on a second ISA that **userspace, not the kernel, composes the system** |
| `system_initializer` | the full one: console, input driver, line discipline, shell, sink adapter and job undertaker, then resident as the spawn service |

The alias layer is what makes them look like one thing, and it is already wrong.
`crates/system_initializer` says the entry `init` *"is `fixtures/src/hello.rs`'s `init_boot` role on
aarch64 and this program on riscv64"*, while `xtask`'s `portable_archive_entries()` maps
`("init", "builder")` and lists `system_initializer` separately. One of those is stale, and the
contradiction exists because there is an alias to be stale about. Collapse the alias and the class
goes with it: the kernel looks up `progenitor`, and `progenitor` is the program.

### The parity violation, stated as the tree states it

`hello` is not a one-platform program: it is in the riscv64 and x86_64 archives under its own
name. What is aarch64-only is its init role, and `fixtures/src/hello.rs` records what that cost in
its own comment:

> aarch64 packs `hello` as `init`, because there hello *is* [init]... This was a hardcoded `"init"`,
> which is right on aarch64 and silently wrong on RISC-V

That is [§19](../decisions/0019-architectural-parity.md)'s own failure mode, *a feature that works on
one ISA and silently not another*, and the bill was paid once already:
`crates/system_initializer` records a fix landing in one and not the other presenting as "a boot
that reached userspace and printed nothing at all, with no fault and no message."

So the violation is not that `hello` exists. It is that one architecture's first process is a role
bolted inside a demo binary while the others get a purpose-built one.

### What survives

- `hello` stays, on all three, as the demo catalog it actually is. One role moves out of it,
  on one architecture. Milestone 96 already lifted the shared construction into
  `crates/system_initializer`, so what moves is a boot entry rather than logic.
- `builder` stays as itself if it still earns its keep: it is a demonstrator artifact with its
  own argument, not a third init. The lane should say whether that argument still holds once one
  progenitor exists, and not delete it silently if it does not.

## The brief, and what it asked

The block as minted carried its starting measurement, the rules a lane had to follow, the
difference and the contradiction to resolve, and the limits it predicted, all now in
[`the-brief.md`](0266-init-is-an-action-and-the-thing-is-a-process/the-brief.md).

## What was built

One program, three architectures, one name. `components/src/progenitor.rs` is the first process on
aarch64, riscv64 and x86_64. The kernel looks up the archive entry `progenitor` and enters it; there
is no alias and no class of thing for an alias to be stale about.

Two commits, in that order, and the separation is the point rather than tidiness. The first makes
`system_initializer` the boot program on all three boards under the old entry name, so a boot failure
belongs to the structure. The second renames. `script/swish-check` is green on both legs at each of
them, which is the only way that separation is worth anything.

### The contradiction, settled

`crates/system_initializer`'s header was the stale one, and the second half of its claim was
wrong. `xtask`'s `portable_archive_entries()` mapped `("init", "builder")` and listed
`system_initializer` as its own entry, which is what the archive actually contained. On riscv64 the
entry `init` was milestone 20's `builder` demo, and `system_initializer` was reached by its own
name from `riscv_shell_boot`. So `init` did not mean two things, it meant three: hello's boot role,
the builder demo, and (in the header's telling) the system builder.

The reason the header could be wrong for that long is the reason the milestone exists. The claim was
prose beside a program, and nothing compared it to the table; the alias is what made the two
describable separately.

### The one genuine difference

A `cfg` over data, not over code, in one file, beside the other board's table. Gone since
2026-09-15, when milestone 166 (one boot loader, reached two inconsistent ways) unified the loaders
(see Follow-on). Why it was right, and the three alternatives that lost, are in
[`the-grant-order.md`](0266-init-is-an-action-and-the-thing-is-a-process/the-grant-order.md).

### `builder` survives, and the argument holds

It is packed under its own name now, on riscv64 and x86_64, and the RISC-V boot tour enters it
directly. Its claim is *"userspace, not the kernel, composes the system"*, proven on a second ISA in
the smallest form that can prove it, and one progenitor does not retire that claim: the progenitor
proves the same thing while also being the interactive system, so it proves it less cleanly. A
reader who wants to see the composition model with nothing else in the frame reads `builder`.

What it stops being is a boot program with a boot program's name. It was the entry called `init` on
two of three archives, which is most of how `init` came to mean different binaries on different
boards.

## The prose count, which was the milestone's own first deliverable

The block predicted 1,011 occurrences of the word `init` in `notes/` and `design/` and said the
count of *roles* among them was the lane's first honest deliverable. Measured on the base commit:
1,005 as a whole word, and 1,988 as a substring.

| what | count |
|---|---|
| the role, in prose | **748** |
| the archive entry, as a literal (`` `init` ``, `"init"`) | 116 |
| kernel functions that are not this role at all (`mmu::init`, `arch::iommu::init`, ...) | 76 |
| titles, filenames and paths (`notes/trusted-init.md`, `target/init-measure-<arch>.txt`, five roadmap slugs) | 65 |
| **changed by this milestone** | **18** |

The block's third bucket does not exist, and that is worth recording rather than quietly
dropping. It predicted "a great many are the ordinary English sense of initialisation". None are:
`initrd`, `initial`, `initialize` and `initializer` are not matched by a word-boundary search for
`init`, and they are exactly the 983 occurrences that separate the substring count from the word
count. The English sense is real, it is large, and it was never in the 1,011 to begin with. The
1,011 was measured with a different tool than the sentence describing it assumed.

Eighteen changed of 748 roles, and the policy is the deliverable rather than the number. The rule
applied, stated so it can be disagreed with:

- A pointer changes. An identifier, a path, an archive entry name, a transcript string: if a
  reader would grep it and find nothing, it is a dangling reference and a defect. `spawn_init`,
  `boot_via_init`, `INIT_ROLES_ENTRY`, `INIT_BOOT_ROLE` and `user/src/system_initializer.rs` were
  swept everywhere they appear, including inside dated logs, because a dead pointer in a log is
  still dead.
- A present-tense claim about the system changes, because it is now false. Three were:
  `notes/grant-expression.md`'s *"There are still two inits"*, `notes/trusted-init.md`'s
  *"`user::initrd()` loads the archive entry `init`, which is..."*, and `design/naming.md`'s *"the one
  deliberate exception"*.
- A record of what was true on a date does not change. This is the `cred`-to-`credentialer`
  precedent from the same week, and the reason is that rewriting it makes the record lie. Milestone
  22's title is *trusted init*; `notes/pipes/second-stream.md`'s section heading *"A correction:
  there are two
  inits"* is the correction it records; `notes/trusted-init.md` carries captured boot transcripts
  whose bytes are evidence.
- A file a developer may not edit does not change, and that is a larger share of the 748 than any
  judgment call: 371 of the remaining occurrences are in other milestones' roadmap blocks and 100
  are in `design/decisions/`, both of which AGENTS.md puts outside a lane's reach.

## Follow-on

- **Milestone 301.** `design/roadmap/0301-one-grant-order-for-the-progenitor.md`, promoted to
  milestone 301 and SUPERSEDED there by milestone 166. This entry once read: the `cfg` above is
  honest and it is still two orders for one endowment, and unifying them means renumbering aarch64's
  19d test roles, which is its own milestone with its own gate. 166 did exactly that, splitting the
  19d roles out of the boot path into `spawn_hello`, so the two tables in
  `components/src/progenitor.rs` became one and the `cfg` is gone. The proposal path is kept as
  history, the way milestone 296's retired proposal is cited below.
- **Milestone 296.** `design/roadmap/proposals/what-the-boot-path-is-called.md` (retired with that
  milestone: calef ruled the feature deleted rather than renamed, so neither candidate below
  won and the table is kept as the argument that was made). Held out of this milestone deliberately (`script/initboot` to
  `init-boot` was staged and backed out pending it), and it is three strings in two naming domains:

  | thing | today | domain | mechanical answer |
  |---|---|---|---|
  | `script/initboot` | `initboot` | shell command, hyphens | `progenitor-boot` |
  | `cargo xtask initboot` | `initboot` | shell command, hyphens | `progenitor-boot` |
  | the kernel Cargo feature | `initboot` | Rust identifier, `snake_case` | `progenitor_boot` |

  The recommendation is not the mechanical answer. The boot path is named after the role it hands
  to, and once the role has a nine-letter name that reads as a mouthful three times over. What the
  flag actually selects is *skip the milestone tour and hand off immediately*, which is a property of
  the boot rather than of who receives it: its sibling `shell` is named that way already, for the
  thing you get rather than for the program that builds it. So `handoff` / `handoff` / `handoff` is
  worth calef's consideration beside `progenitor-boot`. Either way it is his call, and `jobmix` has
  the same unratified-squish defect beside it.
- **Refused.** `crates/system_initializer` keeps its name. calef, 2026-09-13, asked whether the
  crate should follow the program: *"init is the issue not initializer."* The proposal that raised
  it is deleted rather than kept, because a proposal whose question has been answered is a second
  reading of the tree that disagrees with it.

  The premise this block put in that proposal was wrong, and the correction is the part worth
  keeping. It said calef's argument for `progenitor` *"applies unchanged to `initializer`"*. It
  does not. `init` is a truncated verb; `initializer` is an agent noun, the thing that
  initializes, which is precisely what *name things with nouns* asks for. This block's own
  house-style list names `initializer` in the same breath as `builder`, `spawner`, `supervisor` and
  `provisioner`, so the block argued against itself one section later and nobody noticed for five
  days.

  The proposal's own *argument against* had already reached the same place without the vocabulary
  for it: *"the crate genuinely is the initialisation, as distinct from the process that runs it:
  the crate does not descend anything, so `progenitor` would be a worse fit for it than for the
  program."* That is now the recorded reason.
- **Recorded.** *`HELLO_ENTRY` and `PROGENITOR_ROLE` are provisional*, marked as such at their
  definitions. `HELLO_ENTRY` was already the spelling `kernel::user::tests` used.
- **Recorded.** *`notes/trusted-init.md` keeps its name.* Its subject is milestone 22, whose title is
  *trusted init*, and its filename is that title. The loading note did not keep its own: its subject
  is the role rather than a milestone, so it is `notes/progenitor-and-loading.md` now and its six
  citations moved with it.

## BUGS

- `init` was the one name a stranger arrived already knowing, and that cost is now paid rather than
  predicted. Every other rename in this sweep replaced a name nobody outside knew. A reader who has
  used Unix will look for `init` in the archive and in the process list and not find it. Nothing in
  the tree currently redirects them, and the honest mitigation is that `progenitor` appears in the
  boot transcript's first lines, where `init:` used to.
- The boot transcript prefix changed, from `init: ` to `progenitor: `, in
  `crates/system_initializer`. `script/swish-check` matches those strings exactly and moved with
  them. Anyone with a saved transcript, or a script grepping one, sees a different word.
- Dangling references remain in `design/`, and they are not oversights. 471 occurrences of the
  word sit in other milestones' roadmap blocks and in `design/decisions/`, which a developer lane may
  not edit; among them are live citations of `spawn_init`, `boot_via_init`, `INIT_ROLES_ENTRY` and
  `INIT_BOOT_ROLE` (milestones 166, 177, 182, 161, 49 and 105, and decisions 21 and 120). They will
  not resolve against the tree until a maintainer sweeps them.
- `notes/pipes.md` carries a dead path in a fixed-width diagram, `user/src/system_initializer`,
  left because the diagram is aligned and dated. It is the one place a path was knowingly left broken.
- The captured transcripts in `notes/trusted-init.md` still read `init:`. They are evidence from
  a dated run and their bytes are the record, so they were not rewritten; a reader comparing them
  against a boot today will see a different prefix.

## Index row

calef, 2026-09-08: one program, not a relabelling. Three stand behind one archive entry today, and
the alias layer is already self-contradictory. aarch64's first process is a role bolted inside a
demo binary while the others get a purpose-built one, which is §19's own failure mode

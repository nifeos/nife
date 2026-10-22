---
status: BUILT
raised: 2026-08-25
built: 2026-09-13
---
# 175. Split `user/`: `components/` for services, `fixtures/` for test and benchmark programs

When should nife development split into different repositories? [Milestone 39
(repository)](0039-repository-structure.md)'s own analysis already answered the bigger question
(monorepo now, distribution as a separate manifest repo later, gated on milestone 23 (a
capability-routed component OS) forcing it). 23's residual piece was declined for want of a
customer rather than forcing anything, the same shape as [DECISIONS §105 (`std::thread::spawn` stays
declined)](../decisions/0105-thread-spawn-decline-for-now.md). The answer was not yet, by the tree's own stated
trigger ("when a component is first built outside this tree, or a binary is first distributed to
someone who cannot rebuild it"), which has not happened. This milestone is 39's own "cheap first
move, which commits to none of the four options", scoped against the strain as it is now.

## The strain, measured fresh rather than trusted from 39's own numbers

Milestone 39 measured `user/` on 2026-07-30 (28 `[[bin]]` targets, 9,324 lines) and
`design/what-a-distribution-packages.md` re-measured it four days later (48 targets, 16,309 lines),
already close to doubled. Measured again now: 65 `[[bin]]` targets, 25,277 lines. The strain 39
named has not stopped. `user/` is still one crate doing two incompatible jobs, a collection of
programs and a shared library (`net_transport` and others still live as modules beside the programs
that consume them). So no component can depend on part of it without every other program rebuilding
when any shared module changes.

## What "components" and "fixtures" means, per milestone 39's own naming argument

Not "daemons": a Unix daemon is defined by what it detaches from (no controlling terminal, inherited
ambient authority, a pid file), and nife deliberately has none of those
([DECISIONS §10](../decisions/0010-capability-microkernel.md)). Milestone 39's vocabulary, already
argued for and not re-litigated here. A component is the shippable unit (a binary plus its
manifest), a service is what it offers over a contract. "Server" stays a fine role word inside a
component; "daemon" does not appear.

This milestone does not pre-classify all 65 programs, deliberately. Milestone 39 named
illustrative examples on a tree less than half this size (`net_stack`, `display`, `compositor`,
`line_editor` as components; `heeder`, `spinner`, `flaky`, `allocator_exerciser`, `worker`,
`builder`, `coremark`, `os_primitives_benchmarker` as fixtures). A fresh, correct classification
of the current 65 is real work for whoever builds this, not something to guess at from names in a
minting doc. A third category may turn out to be real and worth naming. Several current programs
are neither long-running services nor test-only fixtures but interactive, user-invoked tools
(`wc`, `rm`, `date`, `ps`, `pgrep`, `watch`, among others) closer to what a Unix `/bin` holds.
Whoever builds this should check whether milestone 39's two-way split still fits the tree or whether
a third directory (`tools/`, or similar, not decided here) is honest about what's actually there,
rather than forcing a fit.

## Three names calef ruled on 2026-09-13, which this milestone performs

Working the unratified worklist, calef ruled the §24 (interrupting) interrupt pair. The rulings were recorded here
before the renames were performed, because this milestone moves both files anyway and doing it twice is
the cost of doing it early.

| Today | Becomes |
|---|---|
| `heeder` | `interrupt_heeder` |
| `spinner` | `interrupt_ignorer` |
| `worker` | `least_authority_demo` |

What was wrong with the old pair. `heeder` never said what it heeds, and the answer, §24's
cooperative interrupt flag, was not in the name. `spinner` did carry its meaning, being the field's
ordinary word for a thread that burns cycles, so the pair was asymmetric: one member self-describing
and one not. `interrupt_heeder`/`interrupt_spinner` was refused because the prefix parses as an
object for the first and not the second, since the spinner does not spin the interrupt, it spins
despite it. `interrupt_ignorer` costs the borrowed word and buys a pair that parses the same way.

`worker` was added to this list on the same day, and it settles a classification this milestone
would otherwise have had to guess. Its own provenance left the name open because the answer
depended on whether the file is a fixture or the canonical minimal program. Three records
(`design/naming.md`, milestones 39 and 175) had it in a fixture list by repetition rather than by
ruling. calef ruled it the canonical minimal program, so it goes to `components/` rather than
`fixtures/`, and the name says what it demonstrates. `demo_square` was considered and refused: the
file's own header says *"the squaring is arbitrary and the authority is the point"*, so naming it
after the arithmetic drops the point, and `demo` is a generic word with no precedent in this tree.

`worker` has the same sweep hazard as `spinner`, in a different place: 99 files match, and
`crates/serial_console`'s soak-census uses the English word ("where the kernel placed each worker at
spawn"). Read those rather than sweeping them. `README.md`'s `worker 7` at the prompt is the command
and does move.

All three renames were performed on 2026-09-13 and all three provenance blocks now read
`ratified`. The measured sweeps below are what the performing lane actually did.

The sweep is the reason this waits, and it is measured. `spinner` occurs 116 times and only
about 41 are the program. The rest are the English word, used throughout `kernel/src/sched.rs`'s
scheduler assertions ("the spinner never ran at all", "a leaked spinner starves later tests"), five
times in `crates/virtio`, and across a dozen notes. A pattern sweep would rewrite assertion messages
and test rationale into nonsense. `heeder` is the opposite: 38 occurrences, essentially all the
program, because nobody uses the word otherwise. So `heeder` sweeps mechanically and `spinner` is
read by hand, every occurrence, before anything is edited.

Three further traps, all learned the hard way this week and recorded in `AGENTS.md`'s naming section
and `design/naming.md`. The program's name is a string literal in `crates/grant_plan`'s command
table, `xtask`'s archive tuples and `kernel/src/user/tests.rs`'s `program("spinner")`, where no
compiler reads it. A path citation must resolve whatever the citing record's status. A
`Name:` provenance block ends at the next empty comment line.

## What it needs

- An audit of all 65 current `[[bin]]` targets, classified by what they actually are (checked
  against each program's own module doc, not guessed from its name).
- The directory move itself, plus updating `xtask`'s `--bin` lists and the initrd packing that reads
  them.
- Done as one mechanical commit, audited, not folded into feature work. Milestone 39's own
  warning, kept verbatim because the failure it names already happened once: "a union merge in
  exactly that code dropped a `--bin` flag on 2026-07-29 and duplicated a loop header the same day."
  A rename touching two generated lists (`xtask`'s bin list, the initrd packer) is exactly the shape
  of change that silently drops an entry when it collides with unrelated work landing at the same
  time.
- Re-checking `crates/` against milestone 39's own three-audience split (kernel proof crates, wire
  contracts, userspace runtime) is explicitly out of scope here. 39 named it as a separate strain,
  and folding it into this milestone would undo the "one thing at a time" discipline the split is
  supposed to buy.

## What was built, and the rule the classification used

74 files, not 65, which is the first thing worth recording: the block's own measurement was
eighteen days old when the lane read it, and the directory had grown by nine programs in that time.
72 are `[[bin]]` targets and two are `net_stack`'s single-consumer `#[path]` modules
(`net_transport`, `socket_test_client`), which travel with it.

The rule is one question, and it is not the obvious one. *Would a distribution ship this because
somebody wants its function?* Yes is `components/` (50 programs); no, it exists to exercise or
measure the system, is `fixtures/` (22).

"Who calls it" is emphatically not the rule, and getting that wrong would have produced a very
different and much worse split. Nearly everything in this tree is reached only from a kernel test,
because the system has one user: `disk_partitioner`, `timetable`, `mdns_responder` and
`identity_provisioner` are each spawned by exactly one test and are obviously components anyway.
What separates the two is what the program IS.

**`least_authority_demo` is the case where the ruling did the classifying**, and it is the reason
this section is worth reading rather than skimming. It was `worker`, and three records had it in a
fixture list (`design/naming.md`, milestone 39's illustrative list, and this block's own) by
repetition rather than by ruling. calef ruled it the canonical minimal program on 2026-09-13, so it
is a component. Nothing in the tree would have caught that: the lists agreed with each other, and
they were all copies of one guess.

The three the lane had to decide for itself, and how each went:

- `builder`. Milestone 39's own illustrative list called it a fixture, on a tree where it was
  new. It is not one: `kernel/src/trust.rs` names it a measured boot root beside `progenitor`, for
  riscv64's boot, and a program the trust anchor covers is not a test program. `components/`.
- `hello`. A boot program, packed and measured on all three architectures, which argues the
  other way. But its own header now says what it is, in the sentence milestone 266 left behind:
  *"what is left here is the demo catalogue the name always described."* `fixtures/`.
- **`swapper`.** Reached only from `live_swap_tests`, which by the rule above decides nothing. It is
  *the operator*, the program a person runs to replace a component under a talking client, so it is
  a tool rather than a fixture. The stand-in servers it swaps (`rust_swappable`, `c_swappable`) and
  the client that does not notice (`chatty`) are fixtures. `components/`, and milestone 23's demo is
  therefore split across the two directories on purpose.

**The split is real dependency isolation, not just directories.** `components` keeps 42
dependencies and `fixtures` 22, derived by pruning against `cargo`'s own unused-dependency lint on
all three targets and then against `cargo machete`, which `script/lint` gates on. `smoltcp`, `gpt`,
`calendar`, `swish`, `timetable` and sixteen more no longer reach a fixture; `coremark`, `job_mix`
and `soak_page` no longer reach a component.

**The provisional package name `user` is gone rather than renamed**, which was the cheapest possible
answer to its own manifest's request: that block called it *"the weakest of the four structural
package names"* and asked for a replacement saying what the package holds. Two packages that each
say so is that replacement.

**The linker script is one file, not two** (`crates/user_rt/link.ld`). `cargo:rustc-link-arg` is
per-package so each build script names it, but the layout has one home, beside the runtime that
supplies `_start` and the panic handler to both packages. Milestone 73 refused to *rename*
`user/link.ld` on the ground that it is genuinely shared; two copies of a 73-line script that must
agree about `0x40_0000`, with nothing comparing them, would have been the low rung.

**The C half went to `fixtures/`** with `c_shim` and `c_swappable`, so `components/build.rs` is four
lines of link argument and `fixtures/build.rs` keeps milestone 36's clang resolution.

**All three of calef's 2026-09-13 rulings were performed here**, and none of the three could be
swept. `heeder` was the only mechanical one (59 occurrences, every one the program). `spinner` is
177 occurrences of which about forty are the program, and `worker` is 696 across 110 files, of which
the English word holds `kernel/src/soak.rs`, `crates/soak_page`, `kernel/src/smp.rs`,
`kernel/src/bench.rs`, `crates/serial_console`'s soak census, `kernel/src/testing.rs` and half a dozen
notes. Every occurrence of both was read. Records that narrate the past keep the old name (milestone
39's 2026-07-30 measurement, 264's worklist, the dated findings in 47 and 51, the board sessions in
`notes/visionfive2.md`, the milestone-10 note in `notes/shell.md`). Documentation describing the
tree as it is now takes the new one.

**860 path citations were repointed** across `design/`, `notes/`, `kernel/` and `crates/`. Fifteen
that name files deleted before this milestone (`user/src/virtio.rs`, `user/src/smb_server.rs` and
thirteen more) were left dangling exactly as they already were: repointing a citation to a file that
never moved because it no longer exists would invent a fact. `design/` prose keeps `user/` wherever
it narrates the past, which turned out to be every occurrence but one.

**Verified without QEMU**, which this milestone can afford in a way most cannot. All three initrds
build and pack (a missing program fails that loudly, which is the check the block asks for),
`cargo build -p kernel` is green on aarch64. `script/lint` passes including the four derived
checks that read the program directories: `script/names`' surfaces, the `-d`-suffix name sweep, rule
7's `#[path]`-consumer counter, and the bare-metal host-pass exclusion in all four of its places.
The boot leg is CI's.

## Follow-on

- **Milestone 395.** The third category this block asked whoever built it to check for. It is
  real: sixteen of the forty-nine components were tools a person invokes rather than services, ten
  of them already typeable at the prompt through `grant_plan::Prog`. On 2026-09-19 that is fifteen
  and nine, because milestone 281 (watch holds what ps holds) cut `watch` and `doc` was ratified as `mdr`. It was not taken
  because a top-level package directory is a name and names are an architect's, and because the
  two-way split is coarse rather than wrong: 39's own definition of a component is the shippable
  unit, which `wc` is. 395 carries the list, the cost (about an hour, measured by doing the same
  work at four times the scale) and the three things a decision has to settle. Numbered on
  2026-09-19 by milestone 433 (drain the proposal pile to zero).
- **Recorded.** `notes/user-proofs.md`'s BUGS: `fixtures` is not a row in `script/verify`'s crate
  table, because it carries no Kani harness and a row with none fails the way that file's own
  comment describes. So a harness added to a fixture would run nowhere and nothing would say so,
  which is the exact failure `mdns_proto` and `jh7110_trng` each cost this tree once. Recorded where
  a reader adding a harness meets it rather than fixed, because fixing it means either a row that
  cannot pass or machinery for a case that does not exist yet.
- **Recorded.** `design/naming.md`'s BUGS: one present-tense claim in `design/` still names `worker`,
  `heeder` and `spinner` as programs this system confines, and `AGENTS.md`'s rule 7 section still
  describes the old program directory as a live one. Both were left because a developer lane edits
  its own milestone's roadmap block and nothing else under `design/`, and never `AGENTS.md`. Every
  other occurrence of the retired names in `design/` is a dated narrative and correctly keeps them.
  Neither is load-bearing; both are one line for whoever next has the standing to make the edit.
- **Recorded.** This lane made two edits under `design/` outside its own block, both of them
  citations that would otherwise resolve to nothing. The old program paths in every block that
  cites one (which `script/roadmap --check` enforces, and which is what made it notice). And
  one table cell in `design/roadmap/0266-init-is-an-action-and-the-thing-is-a-process.md` naming
  `hello`'s `init_worker` role, whose constant this milestone renamed. Repointing a citation is
  neither minting nor deciding, but it is worth saying out loud that it happened rather than leaving
  a reader to find it in a diff.
- **Refused.** Lifting `net_transport` and `socket_test_client` out of `components/src/` into crates
  in this change. This block's own "What this does not decide" leaves that open, and rule 7 permits
  a single-consumer `#[path]` module; folding it in would have put a judgment call inside the one
  mechanical commit milestone 39 asked for. `script/lint`'s consumer counter still guards the case
  that matters, and it now reads both directories.
  This refusal is
  milestone 465 (design/roadmap/0465-net-transport-and-socket-test-client-as-crates.md), which
  carries it with the condition that would change it.
- **Refused.** Re-checking `crates/` against 39's three-audience split, which this block already
  put out of scope and which nothing found here changes.

## Why it matters

Directly: ends the crate-is-both-a-program-collection-and-a-library problem 39 named, so a component
can express "I need this dependency but not that one" without handing it to all 65 siblings.
Milestone 39's own §10 (capability-based, microkernel)/§46 (thin primitives or whole subsystems)
packaging observations get a real home to grow into if `basalt` ever moves past being a
placeholder. The manifest and measured-boot hash are already three quarters of a package format.

Indirectly: it is evidence for re-reading milestone 39's bigger recommendation (repo split, gated on
milestone 23) against current numbers, which its own text asks for, without executing that
recommendation itself.

## What this does not decide

Whether nife ever splits into multiple repositories (milestone 39's own question, still correctly
gated and still correctly undecided). The exact directory name for the third category if one turns
out to be real (`tools/`, `bin/`, or something else). And whether `net_transport` and any other
still-module shared code gets lifted into its own crate as part of this move or left for a follow-on,
matching how `virtio`/`socket_proto`/`supervision_proto` were each lifted separately under Rule 7.

## Index row

Milestone 39's own "cheap first move, which commits to none of the four options", performed. 74
files split 49/23 (plus `net_stack`'s two `#[path]` modules) by one question: **would a
distribution ship this because somebody wants its function?** "Who calls it" is deliberately not
the rule, since nearly everything here is reached only from a kernel test. Two Cargo packages
rather than two directories, so the split is real dependency isolation: 43 dependencies for `components`, 22 for `fixtures`, and the provisional package name `user` is gone rather than
renamed. The linker script moved to `crates/user_rt/link.ld` so one file serves both. Performs
calef's three 2026-09-13 rulings: `heeder`/`spinner` become `interrupt_heeder`/`interrupt_ignorer`
and `worker` becomes `least_authority_demo`, all three read by hand rather than swept because the
English words outnumber the programs. **Found a real third category and did not take it**: sixteen
of the forty-nine components are tools a person invokes rather than services, proposed rather than
named because a top-level package directory is an architect's.

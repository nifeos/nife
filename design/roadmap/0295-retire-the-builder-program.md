---
status: BUILT
raised: 2026-09-14
built: 2026-09-14
promoted_from: retire-the-builder-program
---
# 295. Retire `components/src/builder.rs`

Built 2026-09-14. Promoted from `design/roadmap/0295-retire-the-builder-program.md`,
which was written on 2026-09-14 to ask calef one sentence and which carries this block's whole
argument in its git history. *(Number provisional until the merge queue lands it.)*

calef ruled, in his own words: *"Retire builder"*, option (b). That option and its accepted risk
are restated below rather than re-argued; the milestone is the performance of a decision already
made.

## What was retired, and what it proved

`components/src/builder.rs` was the richer-initrd demonstration of milestone 20 (a portable HAL, proven on a second architecture). The RISC-V boot tour
entered it directly. The kernel read it out of the archive, measured it against the trust root,
mapped the whole archive in read-only, and granted it **exactly two capabilities**, an untyped budget
in slot 0 and a report endpoint in slot 1. From those and nothing else it parsed the archive and read
`least_authority_demo` out of it by name. Parsed that ELF in userspace, it built a child from its own
budget through the granular verbs (retype an address space, copy and map each segment, retype a TCB,
endow, configure, start), started it with 9. The child sent back 81. The kernel never touched the
child's bytes.

The claim was **userspace, not the kernel, composes a process**, made in the smallest form that can
make it. It ran on riscv64 only; aarch64's tour has no such step and `x86_64` packs the archive but
has never been able to enter it.

## Why it could go

Milestone 268's item 4 is the whole reason. Nothing halts by default on riscv64 any more. The
tour now ends in `riscv_hand_over`, so the default build and the card reach the progenitor. It
builds the console server, the line discipline, the input driver and `swish` out of its own budget
through the same verbs, on the same boot the retired step ran on. That is the same claim at the scale
of a running system, and a person can type at the result. aarch64 has done this since milestone
28.

Milestone 289 had examined this program on the same day and kept it, correctly for the tree as
it stood. `riscv_shell_boot` was `#[cfg(feature = "shell")]` then, so the **default** riscv64 build, the
one `script/board-image` writes to a card, halted after the tour, and this was the only demonstration
on that ISA that userspace composes anything. 268 made that premise false on purpose.

## The risk taken, restated because it was accepted rather than closed

**`builder` proved composition from exactly two capabilities. The progenitor does not.** It is
granted the NS16550 and the UART's interrupt line as well, because it is building a system rather
than demonstrating a floor. So the tree keeps *userspace composes a process* and drops
*"...from an authority you can count on one hand"*.

Where that half went is the question this milestone was told to answer rather than assume, and the
answer is half-proved, and the missing half is real:

- **Proved.** `fixtures/src/process_composition_witness.rs` holds the *same two capabilities* `builder`
  held, a memory region in slot 0 and a report line in slot 1. From those it retypes an address
  space, retypes a frame, maps the frame into the space it built, and proves the kernel enforces
  break-before-make inside it. `kernel::user::tests::a_process_can_build_an_address_space_from_el0`
  asserts the verdict `0b111` on **both** architectures whose test kernel can load a user ELF, under
  `script/test`. That is more coverage than `builder` ever had: nothing that runs on a pull request
  ever executed `builder` (milestone 406, `design/roadmap/0406-nothing-in-ci-boots-the-riscv-tour.md`).
- Proved, but from the wrong side.
  `kernel::user::tests::a_process_can_build_start_and_run_a_child_thread` drives the whole sequence
  (retype an address space and a TCB, map code and stack, insert the report rendezvous, configure,
  start) and the child runs and sends its word home. It is a kernel-side test: it calls
  `memory_region::create`, `user_address_space_map` and the rest directly, rather than a program
  making those calls across the syscall boundary out of a budget it was granted. So the verbs
  compose; what it does not witness is a *userspace* program driving them from a fixed endowment.
- **Not proved anywhere.** Those two facts joined: a userspace program holding exactly two
  capabilities reading an ELF out of an archive by name, laying its segments down, retyping a TCB,
  endowing, configuring and starting it. That was `builder`'s body.
  `address_space_witness` gets two verbs in from userspace and stops where milestone 19b stopped,
  with nothing running in the space it built, because threads were 19c's object.
  `fixtures/src/os_primitives_benchmarker.rs` starts a child from userspace and is a benchmark
  holding more than two; `crates/supervision_proto`'s `build_child` is the loader every one of them
  shares, and its callers are endowed for their jobs rather than trimmed to a floor. So the gap is
  narrower than "nobody proves this" and real: it is the join that went, not either half.

The proposal's own guess was checked and was wrong, which is worth recording. It suggested
`least_authority_demo` ("named for the property") and `crates/grant_plan` ("reasons about it") as
where to look. Both are about a child's authority: `least_authority_demo` holds one capability
and `grant_plan` decides what a shell grants. `builder`'s claim was about the composer's
authority, which is a different property, and neither carries it. Written up as milestone 404,
`design/roadmap/0404-composing-a-process-from-two-capabilities.md`.

## The eight sites

Surveyed by the proposal on 2026-09-14 and re-verified before editing. Six numbered entries covering
eight files, because entry 4 names two tables that must move together and entry 5 names two crates.
All eight were real; one path in the survey was stale and is corrected here.

1. `kernel/src/main.rs`, the tour's `is_archive` branch and its `init/build` line. The branch
   stays and its body goes: an archive initrd now prints
   `user ELF    : an archive; the progenitor composes this machine's userspace at the handoff below`,
   which points a board reader at where the claim is made. The bare-ELF arm is untouched.
   `note_boot_stage(4)` moved up out of the archive arm, because it had only ever fired there. A
   bare-ELF boot that died in this step used to report the stage before it and read as having died
   one step earlier than it did. The breadcrumb table's rows 4 and 5 are reworded to match.
2. `kernel/src/user.rs`, `riscv_initrd_demo`. Deleted, 146 lines, including the 2026-08-14
   bench-diagnostic watcher and the two `canary_arm_registries`/`canary_disarm` calls that bracketed
   its blocking receive. Those were diagnostics for a hang inside this function and there is no
   function. The canary itself is kept, along with `sched::boot_stage` and `console::tx_bytes`,
   which the watcher was the only reader of; the call sites went and the instrument did not. See
   `BUGS` below for why, and `kernel/src/sched.rs`'s `mod canary` for the same reason written where
   a reader meets it.
3. `components/src/builder.rs` and its `[[bin]]`. Both deleted. Its four `BUGS` entries went with
   it and none was lost: "nothing in CI executes this program" is moot and its underlying cause is
   already a proposal file; "the child is loaded unmeasured" is closed (below); "one child, one
   hardcoded name" and the `components/`-versus-`fixtures/` category question are moot.
4. `xtask/src/main.rs`, `boot_programs` and `portable_archive_entries`. Both moved together, as
   the proposal warned they must. `boot_programs` no longer differs by architecture, so it lost its
   `arch` parameter rather than keeping two identical match arms: an argument nothing reads is a
   claim that something varies when nothing does. The absent-name path in `write_measure_manifest` is
   kept, because lists are still *allowed* to differ and a missing name must not be a panic.
5. `kernel/src/trust.rs` and `crates/user_mode_runtime/src/initrd.rs`. Doc comments.
   The proposal's path was stale: it said `crates/user_rt/`, and milestone 285 renamed that crate
   to `user_mode_runtime`. `initrd.rs`'s "seven programs" count and its quotation of
   `timetable.rs` are *kept*, with a note that six are left, because they are an account of what
   milestone 139 collapsed rather than a description of today.
6. `crates/serial_console/src/progress.rs`, `userspace_ran()`. The one with teeth, and it resolved
   as the proposal predicted: the matcher stays and the live claim moves. See below.

Two sites beyond the eight, found by re-verifying: `components/src/timetable.rs` and
`components/src/pmap.rs` each cite `components/src/builder.rs` from live code, the first as the
sibling its `// SAFETY:` contract matches and the second as one of four sites in DECISIONS §114's
address-space audit. Both repointed; the audit's finding is unchanged, it has one fewer site.

## `userspace_ran()`: a recognizer that now reads history

The live replacement is `boot_ladder::PROMPT` / `Stage::Prompt`, and it says more than the marker
it replaces. `init/build` meant userspace built one child from two capabilities, where a prompt
cannot appear unless userspace built the console server, the line discipline, the input driver and
the shell.

The matcher for `init/build` stays anyway, and this is not sentiment.
`crates/serial_console/tests/fixtures/captured/vf2-2026-09-01-userspace.log` contains the line and
is asserted on. That capture is bytes off real VisionFive 2 silicon and cannot be re-taken with a
different kernel; a recognizer that could no longer read it would be throwing evidence away to tidy
code. So `userspace_ran()`'s doc now says which of the two questions it answers, at the accessor
itself, and the matcher carries a comment saying the same. `notes/board-console.md` says it where a
reader meets the tool.

## What this closed that was not asked for

One of the three unmeasured userspace loaders is gone, and it was the one on the shipped path.
`notes/trusted-init.md`'s "Still not covered" listed three loaders that build children without
consulting `measured_boot::PROGRAM_MEASUREMENTS`, and milestone 289 had added a caveat there saying
the first of them was worse than the framing suggested. `builder` was not a demo somebody runs in
QEMU. It was the program the kernel measured and entered on the build that goes to a card, and the
chain reached the first process and stopped one link short of the only child it built.

That gap is closed by the program going away rather than by the call being added, and the note now
says so in those terms. The default riscv64 boot's userspace is covered by the interactive chain
(`boot_progenitor` measures `progenitor` and hands it the table; `crates/system_initializer` is the
same code aarch64 runs), not by an exception to it. Two loaders are left and both really are QEMU
only.

The measured-boot refusal stays, which is the one thing a board operator will see differently.
The archive is checked against the trust root in `boot_progenitor` at the handoff. A card with the
wrong archive halts with `MEASURED BOOT REFUSED` there, and `board_console` reads that marker.

And the name went without a ruling, which was the point. `builder`'s provisional
`process_builder` was parked by calef on 2026-09-13 (*"Skip this one because it will go away with
parity"*). Deleting the file took the name off `script/names --unratified` for free, which is what
that parking was waiting for. The refusal records that quote the program (`script/names`'s own
header, `crates/system_initializer`'s provenance block, `design/naming.md`) are kept verbatim. A
refusal is an account of why a name lost on the day it lost, and one rewritten every time the tree
moves is one nobody can check.

## Follow-on

- **Milestone 404.** The minimality half of the claim is proved for the first two verbs and for
  none of the rest. This is the risk option (b) accepted, priced after looking rather than before.
  Numbered on 2026-09-19 by milestone 433's drain of the pile.
- **Recorded.** The synthetic fixture
  `crates/serial_console/tests/fixtures/synthetic/qemu-soak-then-silence.log` still contains an
  `init/build` line, describing a boot shape no kernel produces any more. It is a recognizer test
  rather than evidence, so it is left alone and named in `BUGS` below rather than rewritten.

## BUGS

- **The composition claim is now made at one scale, not two.** Every boot that demonstrates userspace
  composing a process is a boot where the composer holds a budget, endpoints, the UART and its
  interrupt line. Nothing demonstrates it from a floor. This is the accepted risk, not a surprise,
  and the follow-on above is what it is priced at.
- `userspace_ran()` is dead on every live board and cannot say so by returning an error. It
  returns `false`, which is also what it returns for a card with no archive. A reader who does not
  read its doc will read a live board's `false` as "userspace did not run". The doc is the mechanism
  and it is rung three; the honest alternative, deleting the accessor, would take the captured
  transcript's assertion with it.
- One synthetic fixture still carries `init/build`, above. A recognizer fed a line no kernel
  prints is testing the recognizer rather than the system, which is what a synthetic fixture is for;
  it is recorded because the next person to read that file will wonder.
- Three diagnostics are now dead code, kept deliberately. `sched::canary` (with
  `canary_arm_registries` and `canary_disarm`), `sched::boot_stage` and `console::tx_bytes` had
  exactly one consumer between them: the hang watcher inside `riscv_initrd_demo`, which armed the
  canary around that function's blocking receive and printed the counters while the boot thread was
  parked. They are `#[allow(dead_code)]` with the reason written at each, which is the `AGENTS.md`
  "an exception is allowed and must say so" case. Kept rather than deleted because the canary is how
  a board hang gets diagnosed, it was the evidence that overturned the VisionFive 2 "hang"
  (`notes/visionfive2.md`, fifth stop). Its own serialization is loom-checked in
  `crates/memory_corruption_canary_gate`, and unarmed it costs one relaxed load on the tick path.
  What retires this entry is pointing the watcher at a window that can still hang, which the
  handoff's blocking prompt is; inventing that diagnostic for a hang nobody has seen was out of this
  milestone's scope. `note_boot_stage` still has ten callers, so the breadcrumb is still written and
  `dump_threads` still prints it.
- **This milestone was measured in QEMU and not on silicon.** radon was not at this lane's bench, so
  every claim about what a VisionFive 2 prints is read from `notes/visionfive2.md`'s captures and
  from the source, the same limitation milestone 289 recorded.

## Index row

Promoted from the 2026-09-14 proposal that asked calef one sentence and got it: *"Retire builder"*, option (b). Milestone 20's richer-initrd demo composed a child from exactly two capabilities and printed the tour's `init/build` line. Milestone 268 (every architecture boots the same way) item 4 made the default riscv64 boot hand over to the progenitor, which makes the same claim at the scale of a whole system on the same boot. So the step was making it twice. Eight surveyed sites, all real, one path stale (`crates/user_rt` is `user_mode_runtime` since 285), plus two live-code citations the survey missed. The accepted risk was priced rather than assumed. The minimality half is half-proved, by `fixtures/src/address_space_witness.rs` holding the same two capabilities on both ISAs under `script/test`, and proved nowhere past the address space, which is milestone 404 (proved for two verbs and no more). The proposal's guess at where the claim went was checked and was wrong: `least_authority_demo` and `grant_plan` are about a child's authority, not a composer's. Closed unasked: the one unmeasured child-loader that ran on the shipped board path. `userspace_ran()` keeps its matcher for the VisionFive 2 capture and its doc now says it reads history, not a live board.

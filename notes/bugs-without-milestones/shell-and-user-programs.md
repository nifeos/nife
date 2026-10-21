# Shell and user programs

Entries from the [survey](README.md) of `BUGS` sections that are real future work with no milestone. Where is the file whose `BUGS` section holds the entry. Size is the reader's guess: S under a day, M a lane, L more than one lane.

| Where | Gap | Proposed milestone | Size |
|---|---|---|---|
|  components/src/swish/main.rs | a child holding the result endpoint can send SPAWN_FAILED or a wrong exit status about itself; a badged copy per sender closes it (0715 records it, no lane) | swish: badged result endpoint per job | S  |
|  components/src/swish/running.rs | set-grant filter race with the shell's shared frame; cites 599 (BUILT, production pool) as the fix, but the shell stays on window 0; verify if stale | Move the shell's own file channel off window 0 (or retire the entry) | S  |
|  components/src/system_log.rs | flush deadline is the first forwarded line's, so a later queued line can flush early and redraw mid-typing; whether it widened swish-check's window is unmeasured | system_log: move the flush deadline to the console's queue | S  |
|  components/src/system_log.rs | stack depth measured only on QEMU test paths; crowded-writer path runs host-only | system_log: stack high-water on all paths | S  |
|  components/src/system_log.rs | at boot serves the kernel only: no unbadged intake, no writer badges, no reader windows, nothing at the prompt reads it (0613 BUILT, 0687 is persistence only) | system_log: writer badges, reader windows, a reader command | M  |
|  components/src/timetable.rs | a hung child stops the whole timetable once the budget runs out; nothing kills a livelocked child | timetable: kill a livelocked child | M  |
|  crates/supervision_protocol/src/lib.rs (last bullet) | The progenitor's scratch-window use is computed, not measured. | Measure the progenitor's scratch window use at runtime | S  |
|  crates/swish/src/bare.rs (bullets 1 and 3) | A bare installed program name is refused in a pipeline; the both-names refusal is proved on the host only and no gate boots a base that adds a colliding name. | Bare installed names in pipelines, and a boot gate for the both-names refusal | M  |
|  crates/swish/src/complete.rs (space bullet) | A completed name containing a space is inserted unquoted, so the parser splits it. | Quote a completion that contains a space | S  |
|  crates/swish/src/sequence.rs (first bullet) | No grouping (`{ }` or `( )`), so `time a && b` times only `a` and a chain cannot be timed whole. | Command grouping in swish | M  |
|  crates/top/src/lib.rs (top N bullet) | `ProgramArgumentDeclaration` cannot declare an optional integer, so no `top N`. Might fold into 498 or 672. | An optional integer argument kind for program declarations | S  |
|  crates/vmstat/src/lib.rs (also slabtop lib.rs, top lib.rs) | vmstat has no interval or count, slabtop and top no live refresh, all blamed on "no timed wait". Milestone 106 is BUILT and gave sleep, so the stated blocker is gone. | vmstat, slabtop and top refresh on a timer sleep | S  |
|  design/roadmap/0040-documentation-service.md | Shell never waits on "caretaker queue drained"; 151 (BUILT) built the primitive and said its BUGS entry would convert, but no milestone builds the shell consumer, so the `$ ` prompt can interleave with page output | The shell waits on the caretaker's drain instead of racing it | S  |
|  design/roadmap/0067-swish-language.md | `$?` readable only in `echo`; no variable substitution in any other word (milestone 47 (Navigation and naming: `cd`, `pwd`, `ls`, `mkdir`, `rm`, paths, and environm) is BUILT and did not build variables) | swish variables and `$?` substitution | M  |
|  design/roadmap/0111-entropy-for-a-child.md | `ArgSpec` has no positional arguments, so `uuidgen -n 10`, `date` selectors and `printenv NAME` have no spelling (same gap named in notes/date.md and 0126) | Positional arity in `grant_plan::ArgSpec` | M  |
|  design/roadmap/0129-scheduled-execution.md | Registration control is still a polled word "until a deadline wait exists"; 106 built the deadline wait | timetable takes registration by message on a deadline wait | M  |
|  design/roadmap/0198-package-manager.md | Renewing `nifeos.org` (2027-10-03) gates every image that names it and nothing enforces it | A dated reminder or gate for the domain renewal | S  |
|  design/roadmap/0251-a-needs-architect-label-outlives-its-question.md | the "What I need from you" comment goes stale when one of three asks is answered | The ask comment tracks which asks are answered | S  |
|  design/roadmap/0280-unexplained-mutation-scores.md | table alignment honored in first chunk of a table only (crates/documentation BUGS) | Table alignment across chunks | S  |
|  design/roadmap/0281-watch-is-cut-not-folded.md | no automated run types ps or pgrep at a real prompt; the domain-grant regression line went with watch | swish-check types ps and pgrep | S  |
|  design/roadmap/0282-per-thread-cpu-time.md | top cannot take an optional N; ArgSpec is Required or Forbidden only | ArgSpec learns an optional integer | S  |
|  design/roadmap/0282-per-thread-cpu-time.md | top does not refresh and has no %CPU; the timed wait it was waiting for (106) is BUILT | top refreshes and shows %CPU | M  |
|  notes/documentation.md | Last page line and next `$ ` prompt interleave; cites milestone 151 (BUILT) but the shell does not yet WAIT on caretaker drain | Shell waits for the screen caretaker to drain before prompting | S  |
|  notes/env-config.md | `caps` cannot show configuration values for a `login` session's shell, nor for `caps <path>` of an unvouched image | `caps` shows configuration values everywhere | S  |
|  notes/packages.md | only a plain line runs an image; pipes and redirections reach the planner as a program name | Run an image in a pipeline | M  |
|  notes/pipes.md | slot 1 is overloaded (clock, input, `--mem`); a numbered slot convention is owed | Numbered manifest slot convention | S  |
|  notes/pipes.md | `rm` and per-file capabilities not reachable from the prompt | Per-invocation caretaker for rm | M  |
|  notes/pipes.md | refusals read badly (redirected filter blames `doc`; `2>` on a builtin says "declares no second output") | Reword pipeline refusals | S  |
|  notes/pipes.md | `wc` has no `-l`, `-w`, `-c` | wc flags | S  |
|  notes/process-view.md | a doomed thread does not say so | Mark doomed threads in the survey | S  |
|  notes/process-view.md | `pmap` cannot tell a device mapping from ordinary memory; a resumed LIST cursor can land on a recycled log slot; `pgrep` selector cannot be typed and has no exit status | Survey correctness: mapping kind, cursor recycling, pgrep | M  |
|  notes/process-view/limitations.md (via notes/process-view.md) | a process has no name so there is no CMD column | Process names in the survey | M  |
|  notes/rm.md (recursion depth bullet) | A deep tree exhausts rm's 4 stack pages and dies as a data abort with no diagnostic | `rm` refuses a too-deep tree with a message | S  |
|  notes/shell-line-editing.md (typed-ahead bullet) | Typed-ahead keys are dropped at a supervised job's edges because `OPERATION_RAWMODE` discards the queue | Keep typed-ahead across a supervised job (change RAWMODE's meaning) | M  |
|  notes/shell-navigation.md (bind bullet) | `bind` / namespace composition unbuilt; a process cannot hold two directory capabilities (§50 (Namespace composition (`bind`), not stored paths)) | Namespace composition: a second directory endowment and `bind` | L  |
|  notes/shell-navigation.md (environment bullet) | No environment; `PATH` is a closed enum; nothing carries config at spawn | A spawn-time environment, and an open `PATH` | L  |
|  notes/state-handoff.md (second paragraph) | A replacement that neither absorbs nor refuses hangs the operator; no detection | Detect a hung replacement in the handoff operator (non-cooperative fallback) | M  |
|  notes/swish-language.md ($? / variables bullet) | `$?` readable only in `echo`; no variable substitution, no conditional | Shell variables and word substitution | L  |
|  notes/swish-language.md (xargs bullet) | `xargs <program>` stops after batch one; shell cannot ask the progenitor for a per-batch caretaker | Per-batch caretakers for `xargs <program>` | M  |
|  notes/trusted-init/chain-past-the-progenitor.md (pipeline hang bullet) | "echo hi / wc" with a refused wc hangs the prompt | An unspawnable pipeline stage fails the line | S  |
|  notes/trusted-init/chain-past-the-progenitor.md (wrong message bullet) | Shell says "progenitor is out of memory" for a refused program (wrong cause) | Distinct spawn-refusal reply code and message | S  |
|  notes/what-a-session-carries.md | the `--effort` second-order claim about context growth is asserted, never measured | Measure context growth at two effort settings | S  |
|  roadmap 0595 | `caps` on a std image names 384 pages, not the region it is built in | `caps` previews a std image's true region | S  |
|  roadmap 0685 | screen-narrowed job behind a directory grant keeps its window for the boot (shell reaps, not undertaker); also 0892:200 resolver badge never forgotten | The undertaker reaps screen-narrowed jobs | M  |
|  roadmap 0715 | a child holding `result_ep` can send `SPAWN_NO_DISPLAY` about itself | Spawn result words are unforgeable by children | S  |

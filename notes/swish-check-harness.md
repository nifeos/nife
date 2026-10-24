# How swish-check reads what the shell answered

Name: provisional. The stem names the program, `script/swish-check`, and the part of it this page
covers: the host side that types the script and reads the transcript back.

`cargo xtask swish-check` boots the whole system, types each row of its script at the swish prompt
over the UART, and checks what came back. The code is `xtask/src/swish_check/`. This page carries the
design behind its readers, its timeouts and the shape of its assertions. The flake history and its
measurements are in [swish-check-flake.md](swish-check-flake.md).

## Two writers share one UART

The kernel prints fault reports with its own UART driver. The userspace `console` server drives the
same device from another address space, and nothing arbitrates between them. The result is a
byte-level shuffle: `the kernel is fine.` once came out as `the kernel iis fnit: constiner.`. No
matcher can be made reliable against that.

So the safety of `boot_claim` (in `transcript.rs`) lives in the order of its questions, and in one
asymmetry a shuffle cannot break: interleaving can destroy a string, but it cannot create one.

1. The progenitor's negative sentence, searched exactly. If `construction budget NOT dropped` is in
   the transcript, the progenitor printed it. That is the check with teeth, and it runs first.
2. The positive sentence, exact or shuffled, passes.
3. Neither, while the kernel was writing during the boot, is unreadable. It passes, and the gate
   says so on stderr.
4. Neither, with nothing else writing, is silent. With one writer there is nothing to shuffle, so
   the progenitor did not report this at all, and the gate fails.

This moves the residual error from false red to false green, on purpose. The failure it guards is
printed on every boot on every leg, so hiding it needs the shuffle to land on that sentence every
time. A false red taxes every lane. The residual hole is named in `script/swish-check`'s BUGS: a
deleted report together with a thread fault in the same boot passes.

`find_marker` answers whether some characters could be a marker, shuffled. That question has false
positives by construction: `budget NOT dropped` answers yes for `budget dropped` at a cost of four
skipped characters. It is not asked to decide anything on its own. Two versions that tried to carry
the safety inside the matcher were each beaten by a shuffle worse than their fixture. Its slack, 400
characters, is one kernel fault report (three lines, about 150 characters) with room to spare.

The kernel fault tokens are six short strings, because any single string can be shuffled apart. Six
independent chances are a much better bet than one, and when all six lose the result is a loud
failure. The first two, `user thread ` and ` killed: `, are used as a pair. The first alone appears
in ordinary prose. In order, they are the aarch64 and riscv64 fault printer's own first line.

The complaint a failed claim produces describes the program's behavior, never the machine's state,
which the gate cannot see. A diagnostic that guessed at capability state once sent a maintainer
hunting a bug that did not exist.

## The gauges

The progenitor-stack and capability-slot gauges speak from the kernel's idle loop once their marks
settle. On a healthy boot that is just after a prompt comes back, so a gauge lands between `$ ` and
the next line's echo. Every reader assumes those two are adjacent. With a gauge between them, a wait
for a prompt already printed times out, and `echo hello world | wc` reads as having answered
nothing. `GaugeFilter` takes both gauges out as the transcript arrives, so no reader sees them. The
raw transcript keeps them for their own checks.

The filter is streaming and prefix-stable. The reader thread sees the UART in arbitrary chunks, and
the waits take the filtered length as a cursor into growing text. So a tail that could start a gauge
line is held until it is one or is not. The one exception is the space of a bare `$ `, since both
needles start with the kernel's two-space indent. Without it, the prompt every wait looks for would
never be emitted whole.

`degauge` handles the case the filter cannot. In one CI run (#1371) the stack gauge landed character
by character inside the shell's echo of `package install`:

```text
package   proinstgenitor sall tack: 31528 of 49152 bytes at peak, 17624 spare
```

No contiguous `progenitor stack:` existed for the filter to find, the exact search for
`package install\n` never matched, and the leg failed as a hung shell. #1420 hit the same race. Any
line typed while a gauge prints can be shuffled this way. `degauge` matches the gauge's own words,
with its numbers as wildcards, inside the same slack `find_marker` uses. It deletes only the matched
characters, and the newline right after them, which is the kernel's. The templates are grepped from
`kernel::progenitor_stack::announce` and `kernel::cap::announce_peak`, longer variants first so a
`BELOW` or `ABOVE` sentence matches whole. The pass is bounded at 64 rounds so a gate cannot hang.

It is interim. §175 (where the kernel's own output goes once userspace owns the console) rules that
the kernel's output goes through a ring a log service drains. Once that is built, the kernel no
longer writes the UART directly and nothing is left to splice.

`without_redraws` handles the console redrawing a partial prompt line beneath a gauge. The flush
timer is `FLUSH_NANOS` in `components/src/system_log.rs`, and the redraw is
`system_log_contract::console::Inserter::flush`. The mechanism and the runs are in
[swish-check-flake.md](swish-check-flake.md), "The noteless signature". Only gauges are taken out
of the redraw's queue. Any other kernel line flushed above a redraw stays and fails the line it lands
in, which is the right default for a line nothing expects.

## Timeouts and what a leg costs

The boot waits 120 s for the banner and 30 s for each echo. Both are generous because a gate that
flakes on a busy laptop is one people learn to ignore. The 30 s is per echo, not per line. On
2026-08-18 the riscv64 leg failed after echoing thirteen characters of `caps date 2> err.txt` on a
machine running four other lanes, and passed on a rerun. A failure of that shape is a load report
until the one leg is rerun. Raising the number would make a real hang take longer to report, and how
long an echo takes under load has not been measured.

The `x86_64` leg's per-line bound under TCG is 45 s. Under KVM it costs what the others do and keeps
their 30 s. Measured typed to prompt-back, one run each, with the boot thread no longer halting on
the run queue (milestone 628):

| where | lines | per line | slowest line |
|---|---|---|---|
| patagonia, TCG | 118 in 125.7 s | 1.07 s | `caps installed/unvouched` 10.8 s |
| CI arm64, TCG (run 37089120726) | 128 in 364.5 s | 2.85 s | `caps installed/unvouched` 14.2 s |
| CI `x86_64`, KVM (same run) | 128 in 33.4 s | 0.26 s | `interrupt_ignorer` 7.0 s |

So 45 s is 3.2 times CI's slowest emulated line. The same shell over TCG answers every line in under
a second on the other two legs, so the difference is the screen path. `display_terminal` paints the
damaged cells, the whole 924x344 surface on every scroll. `framebuffer_driver` copies them into an
uncacheable aperture one word at a time. Both are unoptimized debug builds, and each store goes
through TCG. A real PC pays the same copy in native stores, milliseconds per scroll, which is not
measured on silicon (`framebuffer_driver`'s BUGS). Milestone 400's BUGS records the design half: the
console blocks on the screen, since `components/src/console.rs`'s batcher is off.

Milestone 624 (the paint path) measured on 2026-10-03 UTC, before milestone 628's fix, sharing
patagonia with another session's `x86_64` leg:

| tree | lines | total | per line | slowest line |
|---|---|---|---|---|
| `main` at 4db8c13bf | 119 | 753.5 s | 6.3 s | `caps std_exerciser` 27.5 s |
| milestone 624 at 4124d6390 | 128 | 665.6 s | 5.2 s | `caps /installed/std-grep needle docs` 22.9 s |

Those runs were paced by 10 ms ticks, 7.7 s a line on CI with a 35 s slowest line
([benchmarks/swish-check-x86-leg.md](benchmarks/swish-check-x86-leg.md)). That pacing is what a
90 s bound covered, and what 624's rule of twice the slowest local line at CI's worst 1.8x ratio
(82 s) measured. They are not the evidence for 45 s.

The leg-cost check (milestone 722) fails a leg whose median line costs more than five times its own
baseline row. The rows come from CI's `swish-check` jobs on 2026-10-03 UTC, runs 37141733965 and
37141058488, as the job printed the mean. A median sits at or under the mean, so these are loose,
which is the right side for a tripwire.

| leg | accelerator | source | seconds a line |
|---|---|---|---|
| aarch64 | TCG | CI arm64, 145 lines in 27.5 s | 0.20 |
| riscv64 | TCG | CI arm64, 145 lines in 30.7 s | 0.21 |
| x86_64 | KVM | CI x86_64, 142 lines in 36.8 s | 0.26 |
| x86_64 | TCG | CI arm64, 128 lines in 364.5 s | 2.85 |

A leg is held to its own history, not to its peers. In CI no two legs share KVM, and the legs that
share TCG are not alike: `x86_64` under TCG costs about fourteen times aarch64, legitimately. A peer
rule would fail that leg on every Mac. The defect the check exists for, a runnable boot thread halting
until the next 10 ms tick, ran the KVM leg at 7.7 s a line, about 30 times its row. Under TCG the same
defect was 2.7 times its row and passes, because there the per-line bound does that job. A host far
slower than CI's arm64 runner also fails, and the answer is a measurement and a new row.

## What a leg does after its script

The graphical launch (milestone 632, graphics on demand) boots the normal UART system with a
virtio-gpu attached. The swish prompt must reach the serial console before anything is typed, which
asserts the boot stayed minimal. Then the leg types `graphical_terminal`. The shell asks for
graphics (`spawnproto::GRAPHICS_BIT`), the progenitor builds the session's stack from the devices it
holds, and the session prints its own `$ ` on the screen. One key press must echo back.

Any `$ ` decoded off the scanout is the proof. A capability-slot collision anywhere in the session's
build fails in total silence, so the check looks anywhere in the grid rather than at a predicted row.
There is no known picture to compare against, so this proves the launch and less than a scripted
boot proves.

The key arrives one of two ways. `Keystrokes::Device` attaches a virtio-keyboard and presses the key
with the monitor's `sendkey`. `Keystrokes::Serial` attaches no keyboard and types the same byte down
the UART, the configuration every target machine has (milestone 192, option A). The session then
reads the boot's line discipline raw, the shape of §227 (how Tab reaches the shell). Neither the
screen nor the session can tell which source a byte came by, so a change that made it matter turns
exactly one run red. `video_terminal::script::HOST_KEY` names the key once, and `HOST_KEY_BYTE`
beside it is the same key as a byte.

The decoder is `serial_console::screen::decode_exact`. The leg contributes three things: a geometry
check (a scanout of any other size is a `SET_SCANOUT` bug, and is skipped so the next poll retries),
the colors of `video_terminal::Attr::DEFAULT`, and the font `glyph_atlas::REGULAR` at the terminal's
cell.

The reboot phase (milestone 805) waits for `the kernel was asked to restart the machine` without its
`reboot:` prefix. The shell draws its next prompt once `reboot`'s report ends, and the kernel writes
straight to the UART, so the prompt can land inside the kernel's line. Merge group 37554001967 read
`reboo$t:  the kernel was asked...` on riscv64 on 2026-10-07 UTC.

### The `rg` walk

`rg BUGS crates` is typed in `/search` on the second boot, and its answer must equal the host's as a
multiset of lines. Order is not a claim, because RedoxFS and APFS list a directory differently. Its
bound is 600 s, typed to prompt-back. Measured on patagonia on 2026-10-07 UTC with other lanes
building (milestone 121):

| leg | the walk | load average |
|---|---:|---:|
| aarch64, HVF | 14.9 s, 54.1 s | 24, 38 |
| riscv64, TCG | 40.8 s | about 20 |
| `x86_64`, TCG | 345.8 s, 432.1 s | 15 to 20 |

Why emulated `x86_64` is eight to twenty times the others is not measured. The likeliest cost is the
screen path above, painting each of 349 lines through TCG. CI's legs are in milestone 121's block.

Every other line naming `rg` gets 90 s. Loading a 2.8 MiB image is most of it: under the same load
those lines took 3.8 to 30.1 s on aarch64 and up to 38.7 s on `x86_64`. Fetching `rg` from basalt
over software TLS took 21.0 s on aarch64, 28.7 s on riscv64 and 46.1 s on `x86_64` under TCG, on
patagonia under load on 2026-10-12 UTC.

A pipe read can cut a multi-byte character in two, and decoding each read alone turned `§` into two
replacement characters in one of 349 lines. `whole_characters` holds an incomplete sequence for the
next read.

## Why the script's rows assert what they assert

Each row's `jobs` is how many children the progenitor builds from its bounded job pool: one per
program stage that runs. It is zero for a builtin (`echo`, `ls`, `apropos`, `package`), a `caps` preview, a line refused at the prompt,
an image the installed-programs table refuses, and a supervised job built from the shell's own
untyped. `rm` with its caretaker is one job holding two processes. The field has no default, so a
row cannot be added without saying. The total in the success summary is summed from the rows each
leg typed, so adding a row touches only that row and two lanes adding rows do not conflict. A row
with jobs whose answer says "could not spawn" or "faulted" fails even with no wanted phrases. A tag
that is too low is invisible at the prompt; `a_job_count_names_a_program` bounds it from above.

Several rows pin a shape rather than a value:

- `uuid > id.txt` then `wc < id.txt` must answer `1 1 37`. A predictable version-4 identifier would
  be one drawn from nothing. A `uuid` the progenitor endowed with nothing leaves the file empty, its
  refusal on the second stream, so the count fails exactly then.
- `ps` asserts its header, not a tid, since a tid moves with how many jobs ran before it. `ps | wc`
  is not counted, because whether `ps` walks before or after `wc` exists is a race. The confinement
  claim is asserted deterministically in `kernel::user::survey_tests`.
- `mdr gate.txt | wc` answers `1 4 26`: `gate.txt`'s two lines reflow to one paragraph, plus a
  two-byte body indent. A viewer handed an empty stream answers `0 0 0`, so the count separates
  rendering from silence.
- `std_exerciser`'s phrases are each a slot or a page landing where `std` looks. The heap is at slot
  0 and stdout at 1, with slots 4 and 2 left empty. The clock is at 5, entropy at 6, the
  configuration at 7 and the arguments at 8. The stack has no phrase; too little of it faults partway through.
- The job-pool rows use six distinct arguments, because the transcript is walked with a moving
  cursor and six identical answers would let a missed line pass as its neighbor.

The second boot's generation numbers skip one. The first boot vouched for a build as generation 4
and rolled it back (§221, the boot prompt is the owner's console). A generation is never rewritten,
so the removal is generation 5, and a rollback goes by number, landing on 4. Generation 3 is `greeting`'s
0.2.0, installed before the vouch, and lists both versions beside `noteless`. The removed program is
`noteless` rather than `uptime` because §229 (how a bare name at the prompt reaches an installed program) refuses
installing a package named after an image program.

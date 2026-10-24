# The radon benchmark's limitations, in full

*Name: provisional, minted 2026-10-11 (UTC) by lane/ten-longest-notes.*

An appendix to [notes/footprint-perturbation.md](../footprint-perturbation.md), whose BUGS section
keeps the short list. This is every entry as it was written, sentence-split for the prose limits.

- E3 could not separate footprint from code layout, which is the defect the control was built for.
  The experiment varied one Cargo feature, which changes both the amount of resident text and the
  address of every symbol after it. The 2026-09-04 session found a 193 ns effect with a sign footprint
  cannot produce, so the 19 ns effect it *can* produce is not attributable. No reading taken that way
  is a footprint result, including the 2026-09-04 table. The control landed 2026-09-19 ("The next
  radon evening"). Until an evening is run with it, E3 has no attributable number at all.
- The padding is never executed, so it can only ever act through addresses. A dead sled evicts
  nothing by itself; what it does is push other code apart. So even a clean dose-response measures
  whether `script/fastpath-footprint`'s number predicts latency, not Liedtke's claim about an executed
  footprint. It is said in the procedure as well, because it is the sentence most likely to be dropped
  when a result is quoted. [E5](executed-footprint.md) plans the executed version.
- Four layout images are a small sample of a distribution. Stabilizer (Curtsinger and Berger, ASPLOS
  2013) randomizes layout repeatedly for exactly this reason. Four draws can bound an effect loosely,
  and cannot prove one absent. A pad reading just outside the layout range is weak evidence rather than
  a finding.
- The bench card's kernel is not the kernel the static table measures. `bench` changes the IPC path's
  codegen: `ipc_call_reply` is 5,212 bytes with it against 5,936 without, on riscv64, and the
  normalized instruction stream differs. `single_hart` adds four instructions; `board` alone changes
  nothing. Step 0 therefore measures the card's own feature set rather than the bare release kernel,
  which the 2026-09-04 session did not. Its "5,936 unpadded, 11,070 padded" is a true statement about
  a kernel nobody booted that evening. The optional per-IPC-depth boot of milestone 134 (the register of
  measures) rests on the same assumption, and inherits this.
- `cargo xtask bench --riscv --extra-features <f>` silently ignored the features until 2026-09-19,
  building a plain `bench` kernel and printing its numbers under the flag. This control's QEMU boot
  proof found it, and it was fixed in `xtask` (the x86_64 arm had it too). Any riscv64 E3 rehearsal
  taken through that path before this date measured an un-padded kernel.
- A bench boot takes about 75 seconds, and step 3's twenty-minute window is therefore very generous.
  It was measured over six boots on 2026-09-04, all of which reached `bench: done`. The deadline was
  the thing this page was most worried about being wrong, and it was wrong in the safe direction.
- `single_hart` booted on hardware six times, and nothing noticed, which was this page's second
  unverified claim. Three U74s left parked exactly as OpenSBI handed them over produced no fault, no
  timeout and no skip line. The boot-to-boot spread with one core is 0 to 2 units on a four-figure
  tick count, against the fifteenfold placement spread notes/soak.md records for a four-core card.
- Nobody has repeated the session on a second card or a second board. Everything here is one microSD
  card in one VisionFive 2 on one evening, and the layout confound is the reason to care. A different
  card or a different board would not change the layout. So it would reproduce the same artifact, and
  look like confirmation.
- Nothing in CI compiles this card, or any card. `board`, `soak_test`, `job_mix`, `reboot_soak_test`,
  `single_hart` and `fastpath_pad` are built when a person runs `script/board-image`, minutes before
  walking to the bench. A refactor that breaks a card build leaves the tree green until then, and the
  error arrives at the worst possible moment. Six release builds of one crate would close it:
  `design/roadmap/0373-board-only-features-nothing-compiles.md`.
- The bench card measures fewer things than an ordinary one. `smp_throughput`, `fs_read` and
  `fs_throughput` self-skip under `single_hart`. A session that wants a multi-core number from radon
  builds a second card without the flag, and that card cannot produce E1 or E4.
- The images are still mostly indistinguishable on the card, with one exception since 2026-09-19.
  They have the same three filenames, no build stamp in the payload, and nothing on the board prints
  its feature set. So the mitigation for everything else is the `features:` line in the build output
  and the operator's own log filename, which is rung four of AGENTS.md's ladder. The exception is
  exactly the case an eight-image interleave would break on: a `fastpath_pad` bench boot prints
  `bench-probe: fastpath_pad units <u> shift <s>`, so E3's images name themselves. A kernel that
  printed its whole feature set would be rung three for the rest, and is proposed in
  `design/roadmap/proposals/a-boot-banner-that-names-the-build.md`.
- E3's `black_box` guard is a confound on both shapes, and the dose-response removes it from the
  comparison rather than from the kernel. Every image in the ladder carries the guard, including the
  zero rung, so it cancels between conditions. It remains a difference between any of them and a build
  with the feature off, which is what the 2026-09-04 table compared. `ipc_send` and `ipc_call` each
  carry one untaken compare-and-branch when the feature is on. `kernel/src/fastpath_pad.rs`'s module
  doc prices it at around a nanosecond against a low-microsecond round trip. On radon the round trip
  is longer and the branch is not faster, so the ratio only improves. It is still a real asymmetry
  between the two builds, and it is why an effect at the 1% level should not be believed.
- The riscv64 TCG self-skip is a single-value test. It says "not QEMU `virt`", not "has a 32 KB L1".
  Another riscv64 machine with a different timebase would run E1 and E4 and be believed. Which machine
  a capture came from lives in this page and in the operator's log filename, not in the kernel.
- Nothing here re-takes E2. The thread census (4 new threads on the SMB/FS path) was taken on both
  ISAs under QEMU on 2026-08-22. It is a topology fact rather than a timing one, so it does not need
  the board. If the customer path changes, E2 changes, and E1's reading against it changes with it.

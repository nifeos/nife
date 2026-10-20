# Step 0 under milestone 796's pin: why E3 as specified cannot run, and why it was retired

*An appendix to [notes/footprint-perturbation.md](../footprint-perturbation.md), whose "The next
radon evening" holds the procedure this step belongs to. Name provisional.*

## What step 0 found on 2026-10-20

Step 0 ran on patagonia on 2026-10-20 (UTC), at commit `68647cfc5`. All eight images built and
printed SEALED. Their hashes are in `bench/radon-2026-10-20/e3-payloads.sha256`, and every `pin:`
and `layout:` line is in `bench/radon-2026-10-20/e3-step0-check.log`.

- The code hash passes: `7c518b6586efa75a` over 3,033 instructions in 16 hot symbols, in all eight.
- The sled passes: `fastpath_pad_body` sits at `0xffffffc08020647e` in all eight. It is 4, 5,092,
  10,180 and 15,268 bytes for pads 0 to 3, and 4 bytes in each shift image. The twins pass too,
  trivially: every hot symbol is at one address in all eight images, not just in each pair.
- The sled line fails. Every image reads `the sled precedes 0 of 16 hot symbols, 0% of their
  bytes`, and `pin: .text.hot 9342 B at 0xffffffc080204000` with 16 of 16 inside, in all eight.

The cause is in the linker scripts (`kernel/link-riscv64.ld`, `kernel/link-aarch64.ld`).
Milestone 796 (pin the hot trap path's placement) put every hot symbol in `.text.hot`, aligned to
16 KiB right after the boot stub, and the sled section comes after it. A pad or a shift now moves
the cold text after the block, and moves `.data` and `.bss` in whole pages (`__data_start` sits 0
to 3 pages on across the eight). It moves no instruction the IPC rows execute. The eight images are
one hot-path layout, so the pad ladder and the layout range would measure the same thing. No board
time went to boots, because step 0 is the check that invalidates the whole block.

Milestone 796's own sweep ran the same evening (recorded on #2469): one-task `null_syscall`
`per_job` 121 at shifts 0, 16, 32 and 48, a spread of 0. Those shifts move everything except the
hot block, so the flat line says code outside the pinned block no longer moves that number. It does
not say the block's own position is irrelevant. Unpinned, on 2026-10-05, placement alone moved the
same number between 110 and 118. 121 is from another commit, so it does not compare cleanly with
that range.

## The fork, and calef's ruling

1. **Move the sled ahead of the hot symbols, so E3 moves the pinned block as a unit.** Literally
   ahead of `.text.hot` measures nothing. The block starts at `ALIGN(16384)`, so a sled before it
   is absorbed by the alignment gap until it crosses a boundary, and then it moves the block by a
   whole L1i way, which keeps every set. To move the block, the sled has to sit inside it, after
   `__text_hot_start` and before the hot symbols. It also needs a knob of its own, because 796's CI
   check builds `NIFE_FASTPATH_SHIFT` 0 and 20,520 and requires that the block not move, and 796's
   sweep relies on the same. Cost: both linker scripts, `kernel/build.rs`, the two checks in
   `script/fastpath-footprint`, then the 24-boot evening. It would answer whether the pinned
   block's offset within a way moves the IPC rows. That is a question about 796's pin, not about
   footprint.
2. **Retire E3's question.** Under the control E3 answers two questions
   ([executed-footprint.md](executed-footprint.md)): does the footprint `script/fastpath-footprint`
   prints predict latency, and does moving kernel text cost time? With the pin, the first has no
   channel left: a byte nothing fetches reaches a clock only by moving code that is fetched, and no
   pad moves a hot instruction (the data pages are option 3). The second is 796's sweep. The
   footprint question milestone 188 (the IPC fastpath) leans on is the executed one, E5,
   milestone 708 (an executed footprint ladder for radon). Cost: a rewrite of 134's Outstanding
   item and the register's E3 row, and 134 closes.
3. **Run the eight images as built.** They differ only in cold text and in where `.data` and `.bss`
   start (0 to 3 pages). So they test whether the page placement of the kernel's statics moves
   the IPC rows on radon's 8 KiB-way L1d. No code, one evening. Nobody holds that question, and the
   images were not chosen to answer it: the data moves with the page count of the displacement, so
   pad 0 and both small shifts share one placement.

Recommended: 2. Would I still pick it if all three cost the same? Yes. Free, option 1 answers a
placement question that belongs to milestone 796, beside its BUGS entry that alignment inside the
block is not controlled, and option 3 answers a question nobody asked. The recommendation is not
about effort. If the block's offset is worth measuring, and 121 against 110 to 118 hints it may be,
that is a follow-on for 796 rather than E3.

calef ruled on #2472, relayed and recorded by the maintainer at 19:58 UTC: *calef, 2026-10-20
(UTC): "2". Retire E3's question.* Option 2 is in force. E3 has no attributable number, and
the E3 row of milestone 134 (the register of measures) is retired. Its bench evidence stays in
`bench/radon-2026-10-20/`.

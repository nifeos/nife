# radon, 2026-10-05 (UTC): milestones 761 and 766, then a bisect of the one-task rise

Nineteen boots of the job mix on radon, power-cycled by calef from 09:56 to 10:35 UTC. Boots 1 to 7
follow [`RUN-SHEET.md`](RUN-SHEET.md), written before any boot and committed unedited. Boots 8 to
19 were added the same evening to explain why boot 1's one-task guard failed. Read with
[`notes/job-mix/radon-2026-10-05.md`](https://github.com/nifeos/record/blob/main/notes/job-mix/radon-2026-10-05.md).

`bootN-<payload>.log` are console transcripts cleaned by the run sheet's `tr` (non-printing bytes
dropped); the raw captures are not committed. `bootN-<payload>.netboot.log` is the TFTP server's
log, which names the directory served and the bytes fetched; boots 6 and 7 share one server run.
No console line names a payload, so the netboot log and the hashes below are the record of which
image a boot ran. Every boot printed `one of 7 kinds`, `median of 21 repeats` and `job-mix: done`.

## Boot, payload, commit

`null_syscall` `per_job` in 4 MHz ticks, from `LC_ALL=C grep -a 'kind=null_syscall'`. "Merge" is
the position in the 74 first-parent merges from `b500b3d48` to `c8b5fd09e`. Every payload was built
with `script/board-image --job-mix --tftp` (riscv64, release kernel; see its `BUGS` for the
userspace profile). `nife-vf2.img` hashes are the first 12 hex digits; full hashes of both files are
in [`payloads.sha256`](payloads.sha256) (boots 1 to 7) and
[`bisect-payloads.sha256`](bisect-payloads.sha256) (boots 8 to 19).

| Boot | Payload | Built from | `nife-vf2.img` | 1 task | 4 tasks |
|---|---|---|---|---|---|
| 1, 3, 5 | `main` | `c8b5fd09e` | `abd3f8487dc2` | 110, 110, 110 | 120, 121, 120 |
| 2, 4 | `unaligned` | `c8b5fd09e` + [`unaligned.patch`](unaligned.patch) (766 undone, `PERCPU` at 24 mod 128) | `044d25c6030c` | 110, 110 | 128, 127 |
| 6, 7 | `main-lock-wait` | `c8b5fd09e`, `--extra-features lock_wait` | `96d48d3e27b2` | 116, 116 | 124, 123 |
| 8 | `old` | `b500b3d48` (the 2026-10-04 fix evening's commit, rebuilt) | `94b69a876ed7` | 99 | 148 |
| 9 | `oldstub` | `c8b5fd09e` + [`oldstub.patch`](oldstub.patch) (`b500b3d48`'s five-register riscv64 `invoke`) | `ab929d03f41e` | 107 | 116 |
| 10 | `slot64` | `c8b5fd09e` + [`slot64.patch`](slot64.patch) (`EMPTY_SLOT` 64, outside the table) | `32b4eae40b09` | 109 | 119 |
| 11 | `q50` | `911fc0350`, merge 41 (#1630, milestone 761 (capability lookup off the global lock)) | `600bdbc2c2f3` | 101 | 116 |
| 12 | `q75` | `f2f0bce45`, merge 58 (#1665) | `b77dcabba440` | 120 | 133 |
| 13 | `i17` | `97e6efccf`, merge 51 (#1644) | `2121adafef1b` | 100 | 114 |
| 14 | `i18` | `aee3b5ee9`, merge 52 (#1659, milestone 105 (the two forks)'s sixth register) | `fd703608fce1` | 116 | 131 |
| 15 | `i18-oldstub` | `aee3b5ee9` + [`i18-oldstub.patch`](i18-oldstub.patch) | `d11a600ceebc` | 108 | 122 |
| 16 | `shift0` | `c8b5fd09e`, `--extra-features fastpath_pad`, `NIFE_FASTPATH_PAD=0 NIFE_FASTPATH_SHIFT=0` | `ee5407752e08` | 111 | 122 |
| 17 | `shift32` | the same, `NIFE_FASTPATH_SHIFT=32` | `f68a6ccd544d` | 116 | 126 |
| 18 | `shift48` | the same, `NIFE_FASTPATH_SHIFT=48` | `2c85c57b9f9d` | 110 | 121 |
| 19 | `shift16` | the same, `NIFE_FASTPATH_SHIFT=16` | `1c502bdbf3c6` | 118 | 128 |

Built and never booted: `no1663`, `c8b5fd09e` with #1663's userspace change reverted
([`no1663.patch`](no1663.patch), staged in its worktree; hashes in `bisect-payloads.sha256`).

## Things a reader of the hashes should know

- Identical source in two worktrees did not give identical archives: `main` and `unaligned` differ
  only in `kernel/src/cpu.rs` and their `nife-initrd.img` hashes differ (`4654c6a8...` and
  `c34d848d...`, same size). The hashes identify what was served; they do not prove two builds
  were the same source. The commit and patch columns do that.
- Every patch here was scratch, never committed to a branch. `oldstub.patch` declares `a5`
  clobbered so the five-register stub stays sound against the six-register kernel.

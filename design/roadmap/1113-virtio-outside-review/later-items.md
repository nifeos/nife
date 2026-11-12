# P2: later

An appendix to [the milestone](../1113-virtio-outside-review.md).

## P2

12. `find_file` trusts its caller's name length. `:412-413` reads `entry + name.len()` when
    `name.len() != NAME_LEN`, so a name longer than `nifefs::NAME_LEN` would read the next entry's
bytes. Both callers pass four- and seven-byte literals. The fix: `debug_assert!(name.len() <=
nifefs::NAME_LEN)`, or a `&[u8; N]` parameter. Effort: S. Two-way.

13. `disk_size_bytes` multiplies without a bound and ignores the configuration generation.
`(sectors * 512) as i64` (`:1165`) overflows past 2^55 sectors and goes negative past 2^54, which
the FS server would read as an error code. The code reads the two 32-bit halves of the 64-bit
capacity without the `ConfigGeneration` register (virtio-mmio `0x0fc`; unverified offset), and that register
makes a multi-word configuration read consistent. No QEMU disk reaches either. The fix: `checked_mul`
with a saturating fallback, and a generation-bracketed read when the direct grant lands. Effort:
S. Two-way.

14. One request in flight, one interrupt per request, on the production path. Every submit
reuses descriptor head 0 (`:441`, `:1122`) and the block server waits on the interrupt for each
request (`:986-1008`). `notes/virtio.md` records that the mount's read count stays in the low
hundreds, so this fits the watchdog. It is still the slowest shape a virtio-blk driver can take,
and the first thing a reader from Linux or Redox asks about. This review proposes no change; the
figure
belongs beside the storage benchmarks, so the shape becomes a recorded choice. Effort: S.
Two-way. Inferred from reading.

15. `run_attack` keeps a parameter it does not use. `:341` discards
`direct_memory_access_phys` with `let _ =` after never using it; `run_attack_indirect` (`:386`)
does the same after using it at `:370`. The first should take `_direct_memory_access_phys`; the
second should drop the line. Effort: S. Two-way.

16. `wait_receive` has a bound and `complete_block` has none. The net loop gives up after 64
wakeups (`:803`) and reports `0xE9`; the disk loops forever (`:488`). A one-shot test role on a
device that never completes hangs the suite past the watchdog instead of reporting. The fix, with
item 7's single `complete`, is one bound for both. Effort: S. Two-way.

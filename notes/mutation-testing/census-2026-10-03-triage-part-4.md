# The 2026-10-03 census's survivors, part four

Milestone 637 (triage the crates the 2026-09-21 mutation census measured for the first time),
provisional number, 2026-10-03. Parts [one](census-2026-10-03-triage.md),
[two](census-2026-10-03-triage-part-2.md) and [three](census-2026-10-03-triage-part-3.md) hold the
method and the first crates. Every number here is `script/mutation -p <crate>` on the lane's
worktree against the same run, [37108924347](https://github.com/nifeos/nife/actions/runs/37108924347).
Line numbers in the census list are from the run's tree and have moved since.

## swish

32 missed: 26 killed, 6 equivalent. The sweep afterwards reports 6 missed, and 14 timeouts that the
census did not list (a mutated loop step in `pieces`, `write_found`, `pad`, `completing`,
`command_position` and `split`, which hang rather than survive).

- `PATH_MAX` (4). The longest installed path, three 32-byte fields, fills the array to the last byte.
  A bound too small refuses a legal install; the length check notices one too large.
- `Lister` (8). A row of exactly 78 columns stays one row and one column more wraps. Three names so
  the column carried after the second is tested, and a directory's `/` takes a column.
  An empty listing prints no newline.
- `designation` (5). The first word is the program and an option is the program's: a resolver that
  holds every name makes either mistake show as a grant. `..`, an absolute path and a quoted glob
  are refused before the directory is asked, which a resolver that records its questions proves.
- `write_say`, `write_holdings` and `HEAP_MAX_BYTES` (4). The heap cap prints as 32 KiB in both
  places and the constant is 32 KiB.
- `write_note_asks` (2). A note asking for a word of output or for none says so; bytes is the third
  wording.
- `versions::entry` (1). A comment that reads as two words does not select a program of that name.

The 6 equivalents:

- `write_duration`'s two `%` as `+`. The sum is the remainder plus one whole unit, which adds exactly
  1,000 thousandths, and each digit is taken `% 10`, so the printed fraction is the same.
- `write_refusal`'s empty `NoSuchProgram` arm. Without it the `_` arm runs, and `Prog::from_name` of
  the empty name is `None`, so nothing is written either way.
- `write_preview_rows`'s `n < 10` as `<=`. No slot printed through `cap` is 10: the native ones are
  at most 9 and the `std` ones at most 8. It becomes live if a slot constant moves to 10.
- `command_position`'s `<` or `>` arm. `is_break` counts both as breaks, so the `_` arm scans an
  empty word, which is no prefix word, and answers false as the deleted arm did.
- `Sequence::is_empty` as `false`. A line is always at least one segment, as `len` says.

## line_editor

24 missed: 16 killed, 4 equivalent, 4 recorded gaps. The sweep afterwards reports 8 missed, and 10
timeouts in `pop8` and `feed` that are new, because a mutated `pop8` makes the new test's drain loop
hang, which counts as caught.

- Handoff blob (8). A blob that exactly fills its buffer is accepted (the header alone is eight
  bytes) and one byte short is refused. A field exactly as long as its buffer fits and one more is
  malformed. A restored cursor on the last byte is accepted and one past it refuses the whole blob.
  The history's write position survives a swap, which needs a ring that is neither empty nor full:
  the old round trip typed eight lines, so `hist_next` was 0 and `% HIST` as `/` read the same.
- `RawQueue` (6). Save and restore were never run by any test. A queue whose ring has wrapped
  crosses a swap in order, and an empty one empties the target.
- `resume_line` (1). It paints nothing and still remembers the prompt, seen by a repaint after it.
- `repaint` (1). The cursor goes back `len - cur` columns: two mid-line, none at the end, all of it
  at the start.
- `csi_move` (1). A move of one writes `ESC [ D` and not `ESC [ 1 D`. The test's terminal model reads
  both alike, so this one needs a sink that keeps the bytes.
- `START_ABSORB` (1), the second bit.

The 4 equivalents:

- `START_HANDOFF` and `FLAG_EOF`, `1 << 0` as `1 >> 0`. Both are 1.
- `proto::req`'s `|` as `^`. The opcode sits at bit 56 and above and the length is masked to 32
  bits, so the halves never share a bit.
- `pop8`'s `head = (head + n) % RAW_QUEUE_MAX` as `+`. The head then grows past the buffer, and
  every access reduces it again, so the bytes read are the same (`usize` overflow is out of reach).

The 4 gaps are `BUDGET_PAGES = 2 * INSTANCE_PAGES + 16`. A test could only restate the formula. What
the number protects is a second instance starting beside the incumbent under a supervisor. The swap
test under QEMU does that: a smaller budget fails it, and a larger one only wastes pages.

## package_archive

22 missed, 22 killed. The sweep afterwards reports 0 missed.

The writer and the reader both take their offsets from `HEADER_LEN`, `MEMBER_LEN` and `STEM_LEN`, so
a wrong constant is consistent with itself and every round-trip test passed. The test that kills
them pins the layout from the crate docs by hand: the magic, the three names at 8, 40 and 72, the
count at 104, the first entry at 112 and its offset, length and digest words. The rest:

- The ceiling is inclusive in both directions: 64 members write and parse, the writer refuses 65,
  and the reader refuses a count word raised to 65.
- An empty package is exactly a header, and a table cut one byte short is `Truncated`.
- `member_name(count)` is `None`, as `member(count)` already was.
- The longest stem fills `STEM_LEN` exactly (98 bytes).
- `catalogued_stem` refuses an empty version, a version with a hyphen, and an empty name, each
  written as a catalog line that would otherwise be matched.

## pci

19 missed: 9 killed, 10 equivalent. The sweep afterwards reports 10 missed.

- `BusQueue::enqueue` (2). Every one of the 256 bus numbers, named twice and in reverse order, comes
  out once in arrival order. Bits above 63 are the case: a mask built from `bus ^ 63` shifts by 64
  or more and panics, and `bus | 63` gives every bus below 64 the same bit.
- `msix_cap` (1). A status with other bits set but not the capability-list bit hides a capability
  that is sitting where the list would be.
- `MSIX_ENABLE` and `MSIX_FUNCTION_MASK` (2), pinned to the specification's bits 15 and 14.
- `mem32_window` (4). Three distinct cells with non-zero high halves, so a high half shifted the
  wrong way reads as a different number. A window whose two addresses differ only in the high cell
  is skipped. A whole entry followed by one stray cell is ragged. The old test cut a one-entry
  range, which has no whole chunk left to be misread.

The 10 equivalents are all `|` as `^` where the two operands share no bit:

- `mem32_window`'s three `(hi << 32) | lo`, each cell being 32 bits.
- `read_bars`'s three: `(mask_hi << 32) | (mask_lo & 0xffff_fff0)`, the same with `orig_hi`, and
  `mask | 0xffff_ffff_0000_0000` where the mask is at most 32 bits.
- `Bdf::ecam_offset` and `Bdf::requester_id` (two each). The fields are `u8`, so a device number
  above 31 or a function above 7 would overlap its neighbor, but no enumeration produces one. A
  test would pin what the function does with an invalid address, which nobody wants to rely on.

## manifest_note

17 missed: 9 killed, 8 equivalent. The sweep afterwards reports 8 missed.

- Every value of every enumerated field round-trips, one field at a time: the argument kinds, the
  file, directory and output kinds. An arm of `decode` for a value no round trip named was an arm
  nobody would notice losing. A `should_panic` test checks the second stream's slot.
- The boundaries: a descriptor of four bytes is the wrong length and not "no version", sixteen
  options and an empty memory range are accepted, and the last byte of the layout is checked.
- A subtree option must be a declared letter, including when it is the only letter (the `!=`
  mutant passes for every program that declares two).
- The subtree-grants note's header says the owner is 5 bytes with its NUL.

The 8 equivalents:

- `Note::of` and `SubtreeGrantsNote::of`, six `+` as `-` in the byte-copy loops. `out[4 - i]` writes
  the same bytes as `out[4 + i]` because every word is below 256: the other three bytes are zero and
  land on bytes that are zero already. They would differ the day a descriptor length or a type
  passes 255.
- `Scope::word` returning 1. There is one scope and its word is 1.
- `decode`'s `DESCRIPTOR_LEN - TAIL` as `/`. The tail is the last byte, so 56 minus 55 and 56 over
  55 are both 1.

## file_allocation_table

15 missed, 15 killed. The sweep afterwards reports 0 missed.

- The sector arithmetic is pinned by hand: the data area starts after 32 reserved sectors and two
  copies of the table, a cluster is eight sectors, and the boot file takes `file_clusters` of them.
- `FSInfo` counts three directories and the file, and the next free cluster follows the file.
- Each directory cluster's first sector has contents and the sectors between and before are zero.
  A match guard that is always true hands the boot directory to every unowned sector.
- A directory entry splits its first cluster into halves. Every cluster this crate writes is below
  65,536, so only a direct call with a larger number sees the high half.
- A short name is refused for each reason alone, and the longest one is kept.

## system_log_protocol

14 missed: 10 killed, 4 equivalent. The sweep afterwards reports 4 missed.

- The quoted sizes: a record is 256 bytes at most, a window's data is 4,080, and the four flag bits
  are the four low bits.
- Severity 7 is accepted and 8 refused. A name of 16 bytes is one message and 17 are two. A read
  request is its opcode in the top byte alone.

The 4 equivalents: `KERNEL` as `1 >> 0`, which is 1, and the three `|` as `^` in `control::word`, whose
fields are masked to disjoint bits.

The sweep also reports 42 survivors in `kernel_ring` and `console`, code that landed after the census
ran. They are not among the 1,004 and are untriaged; the next weekly census lists them, and the
mutation inflow check (#1582) is the intended route for them.

## login_protocol

14 missed: 11 killed, 3 equivalent. The sweep afterwards reports 3 missed.

- Each bare word on the front door is its opcode in the top byte alone.
- The durable window is the file service's last, and the session budgets are pinned to 320 and 800
  pages. A change there decides what `login` may spend, so the number is written down twice.
- The two schedule lengths come from their own halves, and skip counts pack one byte each and
  saturate at 255.

The 3 equivalents: `schedule_lengths`'s `|` as `^` (disjoint halves), `sessions_held`'s `<` as `<=`
(equal to the fixed cost gives 0 either way) and `pack_skip_counts`'s `> 255` as `>=` (255 is 255).

## compositor

14 missed, 14 equivalent, no test added.

Every one is a tie that does not matter. `Rect::intersect` and `Rect::union` pick a minimum or a
maximum, and `<` against `<=` picks the same value when the two are equal. An empty rectangle leaves
the intersection empty through its bounds, so the early return is a shortcut. The two `|` as `^` in
the pixel functions join bytes already masked to eight bits.

## Rows for the triage CSV

The inflow check's `notes/project-metrics/mutation-triage.csv` is not on `main` yet (#1582). The 57
survivors of this batch share 42 keys, and those rows are in `census-2026-10-03-triage-batch-6-rows.csv` in the
CSV's own shape, ready to append.

## Tally after batch 6

Of the 1,004: 511 killed, 275 equivalent, 17 recorded gaps, 201 remaining. The next crates, by
survivor count, are `argument_protocol` (13), `boot_slot` (12), `machine_statistics_protocol` (11)
and `elf` (10).

# radon, 2026-11-09 (UTC): milestone 1049's board check

Milestone 1049 (`jh7110_entropy` is ready to publish: the release review's findings), item 4, the
bench session that [notes/jh7110-entropy-board-check.md](../../notes/jh7110-entropy-board-check.md)
drives. The crate commit under test was `nifeos/jh7110_entropy` `ee0bde8feb9f78a67da011c928f9a2a697f3bbc1`.
The payload (`tools/jh7110_entropy_board_check/`, SHA-256
`e421000bc0b66d35ac60315cbe618ba1c8e15fb46d9bf8aac44f3a27b82054e8`) was built from nife
`9a1c55963b` with an uncommitted change to its `Cargo.lock`, and served by `script/board-netboot`.
The person powered the board on from off.

| file | what it is |
|---|---|
| `1049-board-check-results.md` | the filled table from `results.py summarize`, with R4 and 9g filled by hand; `results.py check` printed `PASS, 0 problem(s)` |

Two files from the run are kept by the maintainers, and their SHA-256 sums are the record:

| file | SHA-256 |
|---|---|
| `board.log` (the console, two boots) | `540996ad74682bd9fbe350e897028af2310113a9d210df37590847b40a2c960e` |
| `capture.bin` (1,310,720 bytes, 40,960 generations) | `97fc5325be5ca81d31cfb85b73aaea73191efc474d24d25a0af82f0817079263` |

## Two boots in one log

The first boot fetched `nife-vf2.img`, then asked for `nife-initrd.img`, which was not served, so
the card's boot script fell back to the card's own kernel. The second boot received both files and
ran the payload. `results.py` reads the last run in a log, so the table is the second boot's.

## What the table says

Every judged row passed. The RECORDED rows are answers and carry their limits in
the milestone's item 4 and in the crate's README "Hardware evidence" section: the reset state is
what the bootloader left, `BCONF` bit 3's meaning holds only if nothing wrote `MODE` since reset, and
no `LFSR_LOCKUP` appeared in 1,048,576 generations, so whether a reseed clears it is open. Row 9g
reads "not run": no outside statistical suite was present, and the in-checker battery on one capture
shows no gross defect and says nothing about unpredictability.

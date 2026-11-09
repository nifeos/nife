# The jh7110_entropy board check: a runbook for an agent session

*Name: provisional, minted by lane lane/jh7110-entropy-board-check. The bench session that item 4 of
milestone 1049 (`jh7110_entropy` is ready to publish) asks for, written so that an agent session
on the maintainer's own machine can drive it from this page alone.*

## The prompt to start the session

Paste this into a Claude session started in a checkout of nife on `main`, on the machine that has
the VisionFive 2's serial adapter and shares its local network:

> You are running the `jh7110_entropy` board check on a VisionFive 2. Read
> `notes/jh7110-entropy-board-check.md` in this checkout and follow it step by step, from "Before
> anything else" to "Record the result". Read the off-tree bench README it names before you touch
> anything, and never switch an outlet or unplug a USB hub: ask me for every power cycle and for
> anything physical. Ask before you open the serial port, start the network boot server or write
> any card. Do not claim a check that did not run. Stop and tell me at every stop condition the
> page lists, and finish by showing me the filled results table and the checker's verdict.

## What this decides, and what it does not

The crate `nifeos/jh7110_entropy` holds the JH7110 TRNG's register map and the function that
decides whether a generation succeeded. Its README says that this exact source tree has not run on
a board. This session runs it on one: a small bare-metal payload, built against one commit of the
crate, performs the register sequences below and prints one checksummed line per result, and a
script on the host judges the log. One boot runs every check, in about fifteen minutes.

It answers questions as well as meeting bars. A row of the results table is PASS or FAIL against a
criterion, or RECORDED where the board is answering a question (what the block resets to, whether
two bits are ever set together). The statistics in check 9 are a measurement of one capture, never
a proof that the output is random.

The payload is `tools/jh7110_entropy_board_check/`, and the checker is its `results.py`. Neither
uses the kernel: the kernel pins the crate at an older commit, and the board check tests the commit
about to be released.

## Before anything else

1. **Read the off-tree bench instructions.** The maintainers keep the unit-specific details of the
   bench (which serial device is the board's, which outlet powers it, which outlets and hubs must
   never lose power) in a private lab checkout, not in this repository. Ask the person where that
   checkout is, and read its `README.md` before touching any hardware. Follow its warnings over
   anything here. Never quote it or name anything from it in public text: a commit, a pull
   request, an issue or a note.
2. **The rules for the whole session.**
   - Never switch any outlet, the board's included, and never unplug or power-cycle a USB hub.
     Every power cycle is the person's: ask in one sentence and wait.
   - Ask before opening the serial port, before starting `script/board-netboot`, and before any
     write to a card or a stick. Building on the host and reading a log need no ask.
   - Anything physical (a cable, a DIP switch, a card, typing at the U-Boot prompt) is the
     person's. Say what you need and wait; do not infer the state of the desk.
   - `script/board-console` sends nothing in this session. It is not a keyboard
     ([notes/board-console.md](board-console.md), "Writing to a board").
   - Do not end your turn while the board runs. Poll the log in calls of up to ten minutes.
   - A check that did not run is UNRUN in your report, never PASS.

## Preconditions

What must be true before step 1, and how to confirm each:

| what | how to confirm | if not |
|---|---|---|
| The board's serial console is attached: UART0 on the 40-pin header (pin 6 ground, 8 transmit, 10 receive), 3.3 V, 115200 8N1 | the lab README names the adapter's device; `ls /dev/cu.usb*` on macOS, `/dev/ttyUSB*` or `/dev/ttyACM*` on Linux, shows it | stop and ask |
| No other process holds that port | `lsof <device>` prints nothing | ask the person which session holds it; do not kill it unasked |
| The board's boot-mode switches select QSPI, and its microSD card carries the network-first boot script `script/board-image --tftp` writes | the person confirms; a boot prints `nife: tftp server is <address>` | ask the person; writing a new card is a media write and needs their go |
| The board and this machine share a local network, and the address the card's script asks for is this machine's | compare the `tftp server is` line with this machine's addresses (`script/board-netboot` prints its candidates) | the person types the fallback in step 5, or the card is rewritten with their go |
| UDP port 69 is free here | `lsof -i UDP:69` prints nothing | stop the other server only if the person says it is theirs to stop |
| The toolchain from `script/setup` (the pinned nightly, `rust-src`, `llvm-tools`, the `riscv64imac-unknown-none-elf` target) and `python3` | `rustc --version` in the checkout prints the pinned nightly | run `script/setup` |
| For the rehearsal in step 3 only: `qemu-system-riscv64` | `qemu-system-riscv64 --version` | skip the rehearsal and say so; it is not a board check |

## The steps

### Step 1: choose the crate commit under test

The commit under test is the one the release will tag: the tip of the crate's `main` once every pull
request that changes what the checks exercise has merged.

```
gh pr list -R nifeos/jh7110_entropy --state open
gh api repos/nifeos/jh7110_entropy/commits/main --jq .sha
```

If an open pull request changes `src/`, stop and ask whether to wait for it. Otherwise record the
40-digit commit; it is row R1. The payload's `Cargo.toml` pins the commit this page was verified
against, and `--crate-commit` overrides it for one build.

### Step 2: build the payload

```
tools/jh7110_entropy_board_check/build.sh --crate-commit <commit>
```

It must exit 0 and print three lines: the crate commit, the payload's SHA-256, and the
`script/board-netboot` command. It stages `target/jh7110-entropy-board-check/serve/`, which holds
the payload as `nife-vf2.img` and a small placeholder as `nife-initial-ramdisk.img`: the card's
boot script asks for those two names and falls back to the card unless both transfers succeed. A
build error that names a missing constant means the crate renamed something this page uses; stop
and report it.

### Step 3: rehearse under QEMU

```
tools/jh7110_entropy_board_check/build.sh --crate-commit <commit> --model --rehearse
```

This builds the same payload with the register window replaced by a software model and runs it
under QEMU `virt`. It must end `results.py: PASS, 0 problem(s)`. It proves the payload boots, prints,
and that its log parses; it says nothing about the silicon, and a model log is refused as a board
result. Record its last line in your report.

### Step 4: serve, then watch

With the person's go for both, start the server and the console, each with `nohup ... &` so that
they outlive one tool call. Write the log under `target/` in this checkout: it holds this machine's
addresses and stays local (step 8).

```
nohup script/board-netboot --root target/jh7110-entropy-board-check/serve \
    > target/jh7110-entropy-board-check/netboot.log 2>&1 &
nohup script/board-console --port <device> --until none --for 30m --quiet-after 0 \
    --log target/jh7110-entropy-board-check/board.log \
    > target/jh7110-entropy-board-check/console.log 2>&1 &
```

An empty `board.log` at this point is what you want: the board is off or halted.

### Step 5: ask for a cold power cycle

Ask: "Please power-cycle the VisionFive 2 now, from off." A cold start matters: checks 2 and 3 read
the block as it leaves reset, so a board that ran anything since power-on gives the wrong answer.
Record in row R4 how the board came up (for example "cold power-on by the person").

If the card's script cannot reach this machine, `board.log` shows `nife: payload came from card`
and the board boots whatever kernel the card holds. Ask the person to power-cycle again and, when
U-Boot's countdown appears, interrupt it and type:

```
setenv autoload no
dhcp
setenv serverip <this machine's address>
tftpboot ${kernel_addr_r} nife-vf2.img
booti ${kernel_addr_r} - ${fdtcontroladdr}
```

The payload reads no archive and needs no `fdt move`. Row R3 is then `prompt`.

### Step 6: read the first minute

In this order, with `LC_ALL=C grep -a 'board-check' target/jh7110-entropy-board-check/board.log`:

1. `nife: payload came from net` (or the typed fallback). Row R3.
2. `board-check: begin ... model=0 crate_commit=<the commit from step 1> ...`. A different commit
   means a stale payload: stop.
3. `board-check: window trng=0x1600c000 ... from_tree=1`. `status_okay=0` is expected on the vendor
   tree, which marks the node disabled because U-Boot has no driver for it.
4. Three `clock_step` lines, the last with `released=1`.
5. `reset_state`, `bring_up`, `first_reseed done=1`, and two `generation ... outcome=ready` lines.

Stop on any of these, report the last 50 lines of the log, and leave the console running:

| you see | it means |
|---|---|
| no `board-check:` line within two minutes of power | the payload did not run: check `netboot.log` for the two transfers and the console log for `Bad Linux RISCV Image magic` |
| `refused reason=no_trng_node` | the board's tree names neither TRNG compatible; capture the tree at the prompt with `fdt addr ${fdtcontroladdr}` then `fdt list /soc` (the person types) |
| a `clock_step` with `released=0` or `refused=1` | the reset never read as released; nothing after it is meaningful |
| `reset_state stat=0x00000000 ... features=0x00000000 bconf=0x00000000` | the window reads as nothing: a gated clock, a reset still asserted, or a base that is not the TRNG |
| `fault scause=...` | an exception; the line gives the cause, the instruction address and the faulting address |
| `first_reseed done=0`, or a generation outcome other than `ready` | the block did not answer; finish the run, since later checks still record what it does |

### Step 7: wait for the end

The run prints `long_run_progress` every 131,072 generations and then about six minutes of
`capture` lines at 115200 baud. It is over at `board-check: end complete=1`. Poll for that line in
calls of up to ten minutes, then stop the console and the server (`kill` the two processes you
started). The board then sits parked; it needs a power cycle before anything else boots, which is
the person's.

### Step 8: judge the log

```
python3 -I tools/jh7110_entropy_board_check/results.py summarize \
    target/jh7110-entropy-board-check/board.log \
    --capture target/jh7110-entropy-board-check/capture.bin \
    > target/jh7110-entropy-board-check/results.md
python3 -I tools/jh7110_entropy_board_check/results.py fill \
    target/jh7110-entropy-board-check/results.md R4 "cold power-on by the person"
python3 -I tools/jh7110_entropy_board_check/results.py fill \
    target/jh7110-entropy-board-check/results.md 9g "not run"
python3 -I tools/jh7110_entropy_board_check/results.py check \
    target/jh7110-entropy-board-check/results.md
```

`summarize` fills every row it can from the records whose checksums match, takes the last run in the
log, and writes the capture's bytes. `check` recomputes every verdict from the table's values and
exits 1 when a row fails, is blank, or carries a verdict its values do not support. If an outside
suite is installed (`ent`, or `dieharder -a -g 201 -f capture.bin`), run it on `capture.bin` and put
its name and one-line summary in row 9g instead of "not run".

### Step 9: record the result

- Show the person the filled table and the checker's last line. Report each FAIL row with its
  value, and each RECORDED row of checks 2 to 8 as the answer it gives.
- The raw `board.log` stays on this machine, or goes in the lab checkout: U-Boot prints this
  machine's address and the board's network identity into it. A public record carries the filled
  table, the checker's output, the crate commit and the SHA-256 of `board.log` and `capture.bin`,
  and no address.
- The results then go to two places, each in its own pull request: the crate's README "Hardware
  evidence" section, written as facts about the crate commit that ran and naming no other project,
  and milestone 1049 (`jh7110_entropy` is ready to publish)'s block in this repository. The session
  hands both to the maintainer rather than opening them unasked.

## The checks

Every register offset and bit below is a `jh7110_entropy` constant, and the payload uses the
constant, so the run tests the crate's map. `STAT` is at `0x04`, `CTRL` `0x00`, `MODE` `0x08`, `SMODE`
`0x0C`, `IE` `0x10`, `ISTAT` `0x14`, `FEATURES` `0x1C`, `RAND0..7` `0x20..0x3C`, `AUTO_RQSTS` `0x60`,
`AUTO_AGE` `0x64` and `BCONF` `0x68`, from the window base `0x1600_C000`. `ISTAT` is
write-1-to-clear. Every poll is bounded by one second of the `time` CSR and counts a timeout rather
than hanging. Ticks are of `timebase_hertz` on the `begin` line, 4 MHz on this board.

### Check 0: the run itself (rows R1 to R9)

The payload enables STG clocks 15 and 16 and releases STG reset 3, the TRNG's plan in
`jh7110_clock_and_reset::ENTROPY_BRING_UP`, at the STG window `0x1023_0000`, printing each word
before and after. Pass: the commit is the one under test, the run is not the model, the window is
`0x1600c000` from the tree, both clocks read enabled, the reset reads released, and the run reaches
`end complete=1`. Row R9 counts records dropped for a bad checksum; a few are expected on this link.

### Check 1: a reseed and a generation by polling (rows 1a to 1e)

Sequence: after check 3's bring-up, write `SEED_DONE` (bit 1) to `ISTAT`, write `RESEED` (2) to `CTRL`,
poll `ISTAT` for bit 1 and write it back. Then twice: write `RAND_RDY` (bit 0) to `ISTAT`, write
`GENERATE` (1) to `CTRL`, and hand each `(STAT, ISTAT)` poll to `generation_outcome` until it answers
anything but `NotReady`; on `Ready`, write `RAND_RDY` back. Pass: the reseed completes with
`STAT.SEEDED` (bit 9) set, both outcomes are `ready`, neither 32 bytes are all zero, and the two
differ. Capture: both 64-digit byte strings.

### Check 2: STAT.SEEDED and LAST_RESEED after reset (rows 2a to 2c)

Sequence: the first read of `STAT` after the clock plan, before any write to the block. Recorded:
`SEEDED` and `LAST_RESEED` (bits 16 to 18, `STATUS_LAST_RESEED_MASK`), where the manual gives 7 as
unseeded; and `LAST_RESEED` again after check 1's reseed. These are answers, not bars.

### Check 3: the output width (rows 3a to 3g, and 7a)

Sequence: the same first read gives `STAT.R256` (bit 3), `MODE`, `FEATURES` and `BCONF` before any
write. Then the bring-up the crate documents: write 0 to `AUTO_AGE` and `AUTO_RQSTS`, write
`INTERRUPT_STATUS_DOCUMENTED_BITS_MASK` (`0x1F`) to `ISTAT` and read the residue (row 7a), set
`MODE_256_BIT_OUTPUT` (bit 3) in `MODE`, write 0 to `IE`, and read `STAT`. Pass: `STAT.R256` reads 1
after the write (3e), no documented `ISTAT` bit survives the clear (7a), and `FEATURES` bits 1:0 and
`BCONF` bit 2 both say the block was built for 256 bits (3g). Recorded: which value of `BCONF` bit 3
(`PRNG_LEN_AFTER_RST`) means 256 bits (3f), read off the width the block reset to. That reading holds
only if nothing wrote `MODE` between reset and the payload; U-Boot has no driver for the block.

### Check 4: RAND_RDY and RAND_GENERATING together (rows 4a, 4b)

Each snapshot reads `STAT`, `ISTAT`, `STAT`. A snapshot counts as `together` when both `STAT` reads
show `RAND_GENERATING` (bit 30) and `ISTAT` shows `RAND_RDY`, and as `together_loose` when either
`STAT` read does, since the three reads are not one instant. Scenario 4a, 1000 trials: acknowledge
`RAND_RDY`, write `GENERATE`, snapshot until `RAND_RDY` is set with no generation in flight.
Scenario 4b, 1000 trials: finish a generation, leave its `RAND_RDY` latched, write `GENERATE` again,
and snapshot until a generation was seen and ended, or 64 quiet snapshots pass. 4b also records
whether the block accepted the second command and whether the `RAND` words changed. Pass: no
timeouts. Recorded: the counts, which answer whether the decoder's `NotReady` for this case is ever
exercised by the silicon.

### Check 5: the width of LAST_RESEED (rows 5a to 5c)

Every `STAT` read in the run is folded into one OR and into a set of `LAST_RESEED` values seen.
Recorded: any `STAT` bit outside every field the crate names (5a), the values seen (5b), and whether
any of bits 19 to 23, just above a three-bit field, was ever set (5c). A board can show that the
field is at least as wide as its largest value and that the bits above it stayed clear; it cannot
prove the field ends at bit 18.

### Check 6: how long a generation and a reseed take (rows 6a to 6e)

Generation, 1000 samples: acknowledge `RAND_RDY`, read the time CSR, write `GENERATE`, poll `ISTAT`
alone for `RAND_RDY`, read the time CSR. Reseed, 100 samples: the same around `RESEED` and
`SEED_DONE`. Pass: no timeouts. Recorded: minimum, median, 99th percentile and maximum in ticks, the
median and maximum in microseconds, and the cost of one time CSR read (6e), which bounds the
resolution. These are the measured poll bound the crate's README lacks.

### Check 7: the ISTAT residue and the alarm bits (rows 7a to 7c)

Sequence: clear `ISTAT`, write 1 to `AUTO_RQSTS` and read it back, run four generations, and OR every
`ISTAT` read; write 0 and clear again, and read the residue. Then write 1 to `AUTO_AGE`, poll `ISTAT`
for bit 2 for up to two seconds, write 0, clear, and read the residue. A reseed follows, since an
automatic reseed may have run. Recorded: whether bit 3 (the request alarm) and bit 2 (the age
alarm) latched. Pass: no documented bit survives each clear.

### Check 8: LFSR_LOCKUP over a long run (rows 8a, 8b)

Sequence: 1,048,576 generations on the driver path of check 1, counting each outcome. On the first
`Lockup` (`ISTAT` bit 4), the payload reseeds without acknowledging bit 4, reads `ISTAT`, then writes
bit 4 back, reads `ISTAT` again, and runs one more generation. Pass: no all-zero answer, no
`WrongOutputWidth`, no timeout, and, if a lockup appeared, the next generation is `ready`. Recorded:
how many lockups, and whether a reseed alone cleared the bit. A run with no lockup records "none
seen", and the reseed question stays open.

### Check 9: a capture through a statistical suite (rows 9a to 9g)

Sequence: 40,960 generations on the driver path, each printed as a checksummed line, 1.25 MiB in all.
`results.py` keeps the lines whose checksums match. Pass: at least 1,048,576 verified bytes; at most
3 of the FIPS 140-2 blocks (20,000 bits each) fail the monobit, poker, runs or long-run test, where a
good source fails about one block in 1,250; Shannon entropy of at least 7.99 bits per byte; a
chi-square over byte values with an upper-tail probability between 0.001 and 0.999; a mean byte
within 0.5 of 127.5 and a lag-1 serial correlation under 0.005 in size; and no 32-byte generation
repeated. These say the capture shows no gross defect. They do not say the source is unpredictable,
and the README's "quality unmeasured" line gains a measurement, not a proof.

## The records

Every line the payload prints is `board-check: <record> *<crc32>`, the checksum being zlib's CRC-32
of the record, so `results.py` drops a line the link damaged instead of misreading it. A record is a
kind followed by `key=value` tokens, except `capture <sequence> <64 hex digits>`. The kinds, in
order: `begin`, `window`, `clock_step` (or `clock_plan skipped=model`), `reset_state`, `bring_up`,
`first_reseed`, `generation`, `overlap`, `timing`, `alarm`, `long_run_progress`, `lockup`,
`long_run`, `status_bits`, `capture`, `capture_gap`, `capture_end`, `end`, and on an exception
`fault`. `begin` carries `format=1`; a payload that changes a record raises it.

## What has run, and what has not

Run on a host: the payload builds for `riscv64imac-unknown-none-elf` against the commit the
manifest pins and passes `cargo clippy` with warnings denied; the model build runs the whole sequence
under QEMU `virt` and its log summarizes and checks to PASS; `results.py --selftest` (in
`script/lint`) drives `summarize`, `fill` and `check` over synthetic logs, a damaged line, a model
run, a zero capture and a short one, and holds `results-template.md` to the code.

Not run: anything on a VisionFive 2. The clock plan's register writes, the image's acceptance by
U-Boot's `booti`, the console stride on the board's UART, and every value in checks 1 to 9 are
unrun until a session follows this page.

## BUGS

- The model in `src/model.rs` is this tree's guess wherever the sources are silent, so a passing
  rehearsal says the plumbing works and nothing about the block.
- Check 3f assumes nothing wrote `MODE` between reset and the payload. A firmware that ever gains a
  TRNG driver makes the reading wrong without saying so.
- The time CSR on this board may be emulated by firmware, which makes one read cost more than a
  tick; row 6e records the cost and every timing includes it.
- The payload parks at the end, and nothing but a power cycle frees the board, which is a person.
- `results.py` reads only the last run in a log. A log that holds two boots keeps the second.
- The checker cannot tell a capture taken from the TRNG from one taken from anything else that
  prints the same records; the `begin` line's commit and the payload's SHA-256 are the provenance.

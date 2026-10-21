# radon, 2026-10-21 (UTC): the prompt, one keystroke, and the storage read

One bench session on radon (VisionFive 2), power-cycled by calef on radon's own outlet, from 00:41
to about 01:30 UTC. Each console was captured with `script/board-console --until none` and cleaned
with `briefs/bench-session.md` step 6's `tr`; the raw captures are not committed.

| file | boot | image |
|---|---|---|
| `890-card-fallback.log` | 1 | none fetched: `nothing came over the network, falling back to the card` |
| `890-boot-to-prompt.log` | 2 and 3 | `main` at `cda4ceb55`, `script/board-image --tftp`, SEALED |
| `storage-bench-read.log` | 4 | pull request #2486's head `54c8e773b`, `script/board-image --tftp --extra-features storage_bench`, SEALED |
| `README-225.md` | 5 on | milestone 225 (run the soak on radon, argon and xenon)'s reboot draws |

Before the first boot the same tree's QEMU rehearsal, `script/soak-test --arch riscv64 --for 45s`,
was clean: 760,115 round trips in 40 s, 4,206 crossings, `refused=0 mismatch=0 stalled=0`.

## Boot 1: the card's fallback

The card's boot script asks for TFTP at the address of the TFTP host's USB ethernet adapter. That
adapter had no link, so the board printed `payload came from card` and ran the card's older image.
The capture is kept because it shows what that fallback looks like. The adapter's link came back
when the USB devices behind it re-enumerated at 00:52:45 UTC; the cause of the lost link is not
known. That re-enumeration also ended this capture (`Device not configured`).

## Boots 2 and 3: milestone 890, the prompt and one keystroke

Both reached the `swish` prompt, the first radon boots in `bench/` to do so. Each printed `payload
came from net`, `nife self-test: 11 of 13 passed, 2 absent: blockdev deviceirq`, `uart shape:
registers 4 byte(s) apart, 32-bit access (device tree), passed to the console and input drivers`,
`progenitor: network stack up; DHCP leased 192.168.8.200` and the prompt. `script/board-console
--replay 890-boot-to-prompt.log --board radon --until prompt` reports `reached shell prompt`.

`board-console` sends nothing but the soak's stop byte, so the keystrokes were written by a second,
write-only open of the port while it read. The capture holds the board's echo, not these bytes:

| UTC | boot | sent | echoed |
|---|---|---|---|
| 00:56:34 | 2 | `echo hello` and a carriage return, in one write | `e` |
| 00:56:57 | 2 | `cho hello` and a carriage return, one byte per 300 ms | nothing |
| 01:24:13 | 3 | `e` | `e` |
| 01:24:16 | 3 | `c` | nothing |

So the first receive interrupt of a boot is delivered and no later one is, in a burst or one byte
at a time. Milestone 890 (riscv64 and x86_64 join the shared boot path)'s `BUGS` has what is ruled out.
Milestone 805 (`reboot` at the prompt) cannot be tried until it is fixed.

## Boot 4: milestone 53, the storage read

Step 1 of milestone 53 (the board's own peripherals) in `notes/designware-mobile-storage.md`,
read-only, from pull request #2486 (the card read threshold left off). Nothing was written. The
card identified, and `bench::read_only`'s two single-block reads (CMD17) of block 0 and of the
first partition's block 2048 passed; it reports them only once the whole read succeeds. Its timed
multi-block read from block 2048 failed:

    storage-bench: slot 1 (microSD slot): read of block 2048 failed: Host(Data(18, 164))
    storage-bench: slot 1 (microSD slot): at the failure: STATUS 0x1009409 (FIFO holds 128 words) TCBCNT 1100 TBBCNT 4580
    storage-bench: verdict FAILED: read of block 2048: Host(Data(18, 164))

`Data(18, 164)` is CMD18 (multi-block read) with `RINTSTS 0xa4`: command done, receive data
request and a data CRC error. `TBBCNT` above `TCBCNT` is the readout's underrun signature, the CPU
having read more than the card delivered. A FIFO count of 128 words on a 32-word FIFO suggests the
`STATUS` field is decoded at the wrong width. The eMMC socket answered `NoCard(NoResponse(1))`, as
it holds no part. Step 2, the scratch write, waits on step 1 passing.

The RedoxFS power cut did not run. No boot puts RedoxFS on radon's storage, so there is nothing to
cut power under; milestone 53's outstanding item records the boot it needs.

Milestone 593 (a wedged kernel resets itself)'s radon watchdog did not run: the tree has no JH7110 watchdog driver.

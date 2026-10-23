# serial_console's modules: the design their doc comments link

An appendix to [notes/board-console.md](../board-console.md). The doc comments in
`crates/serial_console/src/` state what each item holds and refuses. This page holds the reasons
behind three of them that the comments link here, plus one reading rule for the lottery.

## Why a sweep point keeps all seven fields

`progress::SweepPoint` is not what decides whether a sweep is moving. A watcher reads the stage and
the latest subrun for that. The point exists to be reported, and the report is the one place a
person reads a sweep's numbers without opening the log. That settles the field list.

The kernel prints the fastest and slowest repeats beside the median so that the spread is never
hidden; `job_mix::REPEATS` carries that argument. A reader that kept only the median would hide the
spread again at the last step. The repeat count stays for the same reason, and it is the field most
easily thought unnecessary. A median of 21 and a median of 3 are different claims, and a struct
that drops the count lets a transcript from either be quoted as the other.

The field names are spelled as the wire spells them. That is the cheapest defense against the
first `BUGS` entry in `progress.rs`, where a field renamed on the kernel side parses as nothing. A
struct that can be diffed against a `job-mix:` line by eye makes such a rename visible.

## The clean-core count is a summary, not a theory

`lottery` counts clean cores per draw and prints the rate beside each one. The count summarizes the
placement census. It does not explain where placement lands; that belongs to §138 (how a saturated
workload is made to hand threads across cores). If a series shows the rate not following the count, that is the result. The count is
still the right thing to have measured, which is why the report never prints the tally alone.

## What reading the screen proves

`cargo xtask uefi-boot` asks QEMU for a screendump and reads the boot tour off it with
`screen::read`. That one read gates the whole framebuffer path on every run: the loader's
`LocateProtocol`, the byte order, the stride, the mapping surviving `mmu::init`, and the glyphs.
The graphical `swish-check` leg reads the display terminal's live screen with
`screen::decode_exact`. There is no picture of a live session to predict, so it searches the
decoded rows for what it typed.

## Why the escape byte is not NUL

`stop::ESCAPE_BYTE` is a carriage return. `NUL` was the other candidate, and it was refused
narrowly. It is the most inert byte there is. U-Boot ignores it and so does a shell's line
discipline, yet it still sets the NS16550's data-ready bit, which is all the kernel's escape reads.

It loses on the failure mode. A carriage return that goes unacknowledged points at the board's
receive path. An unacknowledged `NUL` could also be a USB-serial bridge or a driver treating eight
zero bits as something other than a character. A report that cannot tell those apart is worse
than one that can. The inertness `NUL` would buy is already bought by `stop::Escape`'s gate. It
sends nothing until the board has announced an armed soak, so there is no state left for an inert
byte to protect.

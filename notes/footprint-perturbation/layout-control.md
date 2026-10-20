# The layout control, in detail

An appendix to [notes/footprint-perturbation.md](../footprint-perturbation.md), whose "The next radon
evening" keeps the images and the procedure. This file holds how the knob works, why it is two
build-time variables rather than features, and why the eight images have the sizes they do. It is
milestone 370 (a layout control, because the perturbation experiments cannot tell footprint from
addresses), built 2026-09-19.

## The knob

`fastpath_pad` still turns E3's padding on. Two environment variables, read by `kernel/build.rs` and
only when that feature is on, say how much of it there is:

- `NIFE_FASTPATH_PAD=<units>` is the sled's length, in units of what the feature has always linked
  (5,092 bytes on riscv64, 5,796 on aarch64). Unset is 1, the sled the 2026-09-04 session booted. 0
  is the ladder's zero: the guard and a `ret`. So every rung runs the same instructions, and the
  guard stops being a difference between conditions.
- `NIFE_FASTPATH_SHIFT=<bytes>` appends that many zero bytes after the sled, under a symbol nothing
  references. It is the layout variant: bytes that move code, and that the footprint gate does not
  count.

The linker script places the sled right after `.text.hot`, the block milestone 796 (pin the hot
trap path's placement) aligns to 16 KiB after the boot stub. So both numbers do the same thing: move
all kernel text after that block by that many bytes, and `.data` and `.bss` in whole pages. The boot
stub and every hot symbol keep their addresses, which is why E3 as specified cannot run on this tree
([step-0-under-the-pin.md](step-0-under-the-pin.md)). An ordinary build has no such section, so the
default link order is unchanged. That was checked by comparing default boot images byte for byte
across the change, on both ISAs.

## Why the pin matters more than the knob does

A pad and a shift of equal displacement produce identical binaries, apart from the sled's own bytes.
`PAD=1` and `SHIFT=5088` put every riscv64 text symbol at the same address. So the two ladders
differ in exactly one thing: whether the bytes sit inside the footprint `script/fastpath-footprint`
counts, which is the variable E3 claims to test.

Before the pin, where the linker dropped the sled was redrawn every commit. On 2026-09-04 it landed
ahead of the whole trap path, and by 2026-09-19 past all but 5% of the fastpath's bytes. So how much
of the hot path a pad perturbed was an accident of that day's build.

## Variables, not a feature per size

They are variables rather than a feature per size because it was built that way first, and measured.
A feature's name enters cargo's `-C metadata` hash, which renames every symbol and reorders codegen
units. Seven sibling features moved code linked *ahead* of the sled by up to 11 KB on the card build.
`syscall::dispatch` wandered across 240 of the 256 L1i sets. So each image was an uncontrolled layout
draw. Editing this module's own source does the same thing for the same reason, which is why every
image in an evening must come from one commit.

## Why the images have these sizes

The two matched images are the sharpest comparison available, and cost nothing extra. Each is
byte-identical to its pad outside the sled. So any difference between a pad and its twin is the
counted footprint alone, and the physics says there should be none: the sled is never fetched. A
difference there is the instrument telling you something is wrong with the measurement, rather than a
finding about footprint.

The four un-padded images are the layout distribution. Two of them are matched to pads and two are
small, so the distribution covers both scales the pads reach into.

The two small images sample layout at a scale the pads do not. They move the text by under one page,
and they differ where the U74 is known to care. 820 is 4 mod 8, and 2,460 is 4 mod 8 with different
64-byte line phases (52 and 28) from the pads' (32 and 0). The manual says the BTB predicts a taken
branch or jump with no bubble only when the target is 8-byte aligned (`SiFive` U74-MC Core Complex
Manual 21G3.02.00, section 4.2.6). One leaves `.data` on the page pad 0 has, and the other moves it.
That matters because the L1d is 32 KiB 4-way, an 8 KiB way, so an odd page moves every static to a
new set (section 4.4.1).

## What the step-0 readout carries

Keep the whole readout per image. It carries each hot symbol's address and its L1i set on the U74:
32 KiB, 2-way, 64-byte lines, virtually indexed (section 4.2.2), so 256 sets from address bits 6 to
13. It also carries the line phase, whether each symbol starts 8-byte aligned, and where `.text` ends
and `.data` starts. Those are the pre-registered layout facts each image's numbers get read against.

The code hash in that readout covers every instruction in both IPC closures and the entry set, with
address operands normalized away. That means direct call and branch targets, `auipc`/`adrp` uppers,
and the low-12 immediates that pair with them; the script prints how many of each it touched.

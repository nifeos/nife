# The device tree

The machine describing itself. Where RAM is, where the UART is, where the interrupt
controller lives, how many CPUs exist, what firmware has already claimed.

The alternative is hardcoding, which is what milestone 1 did (`0x0900_0000` for the UART,
read off a `dtc` dump by hand). Hardcoding gives you a kernel that runs on exactly one
board. The device tree gives you a kernel that can be *told* what board it's on.

QEMU hands us a pointer to one in `x0`, but **only because we ship a flat arm64 Image**.
See [boot-protocol.md](boot-protocol.md).

## Everything is big-endian

The FDT format predates the little-endian consensus and never changed. Every integer in
the blob is big-endian, on a machine that is little-endian.

Forget one byte-swap and you get a plausible-looking number that is wrong by a factor of
16 million, which is exactly the kind of bug that survives a code review. Our parser routes
every read through `be32` or `be64` so there is no path that forgets.

Even the magic: `0xd00dfeed`, stored big-endian. The kernel test that validates the pointer
does `u32::from_be(magic)`.

## The layout

```
+------------------+
| header (40 B)    |  magic, totalsize, and offsets to the three blocks below
+------------------+
| memory reserve   |  (address, size) pairs. DON'T TOUCH THESE.
|   block          |  Terminated by an all-zero entry.
+------------------+
| structure block  |  the tree itself, as a token stream
+------------------+
| strings block    |  every property name, deduplicated, null-terminated
+------------------+
```

The reservation block is deliberately dead simple, and it comes first, precisely so a
kernel can honor it without parsing anything. It's the firmware saying "I have things in
here." QEMU's `virt` leaves it empty; real boards often don't, and a kernel that skips it
will happily allocate over the firmware's own tables.

## The structure block is a token stream

Five tokens, all 4-byte aligned:

| Token | Followed by |
|---|---|
| `FDT_BEGIN_NODE` (1) | a null-terminated node name, padded to 4 bytes |
| `FDT_END_NODE` (2) | nothing |
| `FDT_PROP` (3) | `len`, `nameoff` (an index into the strings block), then `len` bytes of value, padded |
| `FDT_NOP` (4) | nothing. Lets a bootloader blank out a node in place, without rewriting the blob. |
| `FDT_END` (9) | nothing. Done. |

Property *names* live in a separate strings block and are referenced by offset, because
`#address-cells` appears hundreds of times in a real tree and storing it once is worth the
indirection.

## The part that will bite you: cells

A `reg` property is a list of (address, size) pairs. But how many 32-bit words each of
those takes is not fixed.

It's declared by `#address-cells` and `#size-cells` on the parent node. So to decode a
`/memory` node's `reg`, you first need the *root's* cell counts.

```dts
/ {
    #address-cells = <0x02>;      // addresses are 2 cells = 64 bits
    #size-cells = <0x02>;         // sizes are 2 cells = 64 bits

    memory@40000000 {
        reg = <0x00 0x40000000 0x00 0x8000000>;
        //     \______________/  \___________/
        //      address (2 cells)  size (2 cells)
        //      = 0x4000_0000      = 128 MiB
    };
};
```

The spec's *defaults* are 2 and 1, which almost nothing uses. Read them rather than assume;
our parser does, and it's one of the two things (with endianness) most likely to be silently
wrong.

## Reading one yourself

```bash
qemu-system-aarch64 -machine virt,dumpdtb=virt.dtb -cpu cortex-a72 -nographic
dtc -I dtb -O dts virt.dtb | less
```

`dtc` ships with QEMU via Homebrew. Genuinely worth ten minutes of scrolling: it is a full
description of the machine we've been booting, and after milestone 3 we're only reading the
first two lines of it.

**One gotcha:** QEMU pads its dump to a full megabyte and *says so in the header*, so the raw
dump is a 1 MB file describing 7 KB of tree. Round-trip it through `dtc -I dtb -O dtb` to
compact it. That's how the test fixture in `crates/device_tree_blob/tests/fixtures/` was made.

## What we read, and what we ignore

Milestone 3 read exactly two things: the `/memory` nodes (where RAM is) and the reservation block
(what not to touch). The list has grown as each subsystem stopped assuming its board:

| Node | Who reads it | Since |
|---|---|---|
| `/memory`, the reservation block, `/reserved-memory` | `kernel/src/memory.rs` | milestone 3 |
| `intc@`, `plic@` | the interrupt controllers | milestone 5 |
| `virtio_mmio@`, the PCIe `reg` and `interrupt-map` | the transports | milestone 8 and the PCIe port |
| `smmuv3@` | the IOMMU | milestone 16b |
| `cpu@`'s `riscv,isa-extensions` and `mmu-type` | `crates/machine_discovery` | milestone 60 |
| `/psci`, and `cpu@`'s `reg` / `status` / `enable-method` | SMP bring-up | milestone 100 |
| `/cpus/timebase-frequency` | the RISC-V timer | milestone 100 |
| `/chosen`'s initrd range | the loader | milestone 12 |
| the console UART node: register shape (`reg-shift`, `reg-io-width`, `clock-frequency`, `compatible`), and its interrupt line (`interrupts`, the inheritable `interrupt-parent`, the parent's `#interrupt-cells`) | `console::configure_from_dtb` (riscv, the shape); `memory::init` via `machine_discovery::interrupt_id` (both ISAs, the line) | the VisionFive 2 prep and its boot-13 fix, 2026-08-14/15 |

The UART's *address* is still hardcoded, and that is correct, for a nice chicken-and-egg
reason: the parser is the thing most likely to have a bug, and `println!` is how you would debug
it. So the console has to come up *before* the device tree is parsed, which means the console
cannot depend on it. What it can do is be checked against the tree afterwards, which
`crates/device_tree_blob/tests/qemu_aarch64_virt.rs` does. Everything else about the UART now does
come from the tree, per the row above. That is the register shape adopted before the first
`println!`, and the interrupt line. Its QEMU constant armed an unrelated PLIC source on the JH7110
until boot 13 proved it (notes/visionfive2.md, BUGS). The constant survives only as the documented
fallback for a tree that does not say, and the boot prints which source won.

The Pi port wants all of it, because none of the addresses will match.

## The parser's bounds

The blob comes from firmware, and firmware is the trust boundary here, so these bounds are boot
robustness rather than confinement. Each one costs a comparison and turns a crash on the boot path
into an `Error`.

- `MAX_TOTALSIZE` is 2 MiB. `DeviceTreeBlob::from_ptr` must trust the header's `totalsize` before
  anything else is checked, and the magic says only that the pointer is at a blob. A `totalsize`
  of `0xffff_ffff` would otherwise become a 4 GiB slice that the walkers read past the end of RAM.
  The figure is Linux's `MAX_FDT_SIZE` for the same slot. The largest blob this tree has met is
  QEMU `virt`'s, under 64 KiB, and a VisionFive 2's U-Boot tree is under 32 KiB.
- `be32` and `be64` add with `checked_add`, because every offset comes out of the blob. The
  verification module proves both total.
- A `reg` pair whose `start + size` wraps is `Error::RegionOverflow`. `Region::end` still
  saturates, because `Region` is `pub` with `pub` fields and anyone can build one. That is the
  argument `elf::Segment::page_range` makes too: a type anyone can construct has to hold on its
  own, whatever the parser that usually builds it checked. The fuzz target
  `fuzz/fuzz_targets/device_tree_blob_walk` found the panic it closes.
- The walkers that keep per-depth state stop at a depth of 16. Past it they refuse to match
  rather than return an `Error`, so they never index past their stacks (BUGS below).

## Finding a node

`node_reg` matches a node-name prefix. It found the interrupt controller on both QEMU boards:
`intc@8000000` on aarch64 `virt`, a direct child of the root, and `plic@c000000` on riscv64
`virt`, nested under `/soc`. It decodes with the parent's cell counts even though QEMU writes 2
and 2 at both levels, because relying on that breaks the first board that differs. A name is only
a convention; a real driver matches `compatible`, such as `"arm,cortex-a15-gic"` or
`"riscv,plic0"`, and the Pi port will need that.

`node_reg_compatible` is that lookup, first needed for the RTC of milestone 51 (wall-clock time).
The RTC is `pl031@9010000` on aarch64 `virt` and `rtc@101000` on riscv64, so a prefix that finds one
finds nothing on the other, while `arm,pl031` and `google,goldfish-rtc` name what a driver can
drive. `compatible` lists the most specific entry first, and a match on any entry counts.
`node_prop_compatible`, `parent_prop_compatible` and `phandle_prop` follow the same pattern: the
deciding property may come after the wanted one in a node, so each open node's candidate is
remembered and the answer is taken when a matching node closes.

`node_prop_inherited` exists because `interrupt-parent` sits in a different place on each machine.
QEMU's riscv64 `virt` writes it on the serial node, QEMU's aarch64 `virt` once on the root, and the
mainline JH7110 dtsi on `/soc`. A read of the device's own node alone works on one of the three.

`node_props` reads every match, because a heterogeneous RISC-V machine describes each hart as its
own `cpu@` node with its own `riscv,isa`. A kernel that read the first one and then scheduled onto
any hart would have read the wrong node, and the JH7110 is that machine.

## BUGS

The blob is the first thing this kernel reads and the last thing it can check against anything
else. The pointer comes from firmware, before there is a frame allocator or any way to report a
failure, so every limitation here is a limitation on the boot path specifically.

- **A node nested deeper than 16 is invisible to `node_reg` and `node_reg_compatible`.** Both carry
  `#address-cells`/`#size-cells` down a fixed 16-entry per-depth stack, and past it they stop
  tracking, so they refuse to match rather than decode a region with cell widths they no longer
  know. Nothing in the trees we boot nests past 4. `node_prop` keeps no per-depth state and so has
  no such limit, which means the two lookups disagree about how deep a tree can be. Until
  2026-08-02 `node_reg` did not refuse: it matched at any depth and then indexed past the stack,
  which was an out-of-bounds panic on a 17-deep tree (milestone 42, notes/fuzzing.md).
- A `reg` pair whose `start + size` wraps 64 bits is `Error::RegionOverflow`, not a clamped
  region. `kernel/src/memory.rs` decides where RAM is from these, and "the firmware's memory map
  is impossible" is something a boot path should be told. `Region::end()` saturates as a backstop,
  because the type is `pub` with `pub` fields.
- **`memory_regions` finds `/memory` nodes by NAME, not by `device_type`.** `device_type = "memory"`
  is the more correct check and arrives after the node name; the name is unambiguous on every board
  we have met. A board that spells it otherwise would have its RAM missed entirely.
- **`node_reg` matches a node-name prefix rather than `compatible`.** That is a deliberate
  simplification for two boards, documented at the function. `node_reg_compatible` is the correct
  one and is what milestone 51's RTC lookup uses.
- Nothing validates the structure block's nesting balance. A blob with more `FDT_END_NODE`
  tokens than `FDT_BEGIN_NODE` is walked rather than refused: the three `usize` walkers saturate
  their depth at zero and the three `i32` ones let it go negative. Neither can index anything (every
  array access is guarded and the negative depths simply match nothing), so it is a wart rather than
  a hole, but an unbalanced tree is not told apart from a well-formed one.

`crates/device_tree_blob/tests/hostile.rs` holds the regressions for the first two, hand-built rather than
fuzzer-minimized, so a reader meets the attack next to the code.

---

*Add to this file as new device tree concepts come up.*

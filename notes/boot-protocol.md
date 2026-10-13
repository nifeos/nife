# The boot protocol, and the arm64 Image header

## The question QEMU is asking

When you say `-kernel foo`, QEMU has to decide what kind of thing `foo` is. It doesn't
ask you. It sniffs the file, and the answer determines how much help you get.

| What you hand it | How QEMU boots it | `x0` at entry |
|---|---|---|
| an **ELF** | bare metal: copy the segments to the addresses in the program headers, set PC to the entry point, release the CPU | **nothing.** Registers are not populated. |
| a **flat binary with an arm64 Image header** | the **Linux boot protocol**: generate a device tree, place it in memory, hand you a pointer to it | **the DTB address** |

Milestone 1 shipped an ELF. We printed `x0` and got zero, which is how we found out the
claim "QEMU passes a device tree pointer in x0" is only true for the second row.

A 64-byte header is the entire difference.

## The header

From Linux's `Documentation/arch/arm64/booting.rst`. Ours is
`kernel/src/arch/aarch64/image_header.s`.

| offset | size | field | ours |
|---|---|---|---|
| `0x00` | 4 | `code0` | `b _boot`: the entry point *is* byte 0, so this jumps over the header |
| `0x04` | 4 | `code1` | 0 |
| `0x08` | 8 | `text_offset` | `0x80000`: load us this far into RAM |
| `0x10` | 8 | `image_size` | how much **memory** we occupy, including `.bss` and stack |
| `0x18` | 8 | `flags` | `2` = little-endian, 4 KiB pages |
| `0x20`–`0x37` | 24 | reserved | 0 |
| `0x38` | 4 | `magic` | `0x644d5241`, which is `"ARM\x64"` little-endian |
| `0x3c` | 4 | reserved | 0 |

You can see all of it:

```bash
cargo xtask image
```

### Three details that are easy to get wrong

**`text_offset` and the linker script must agree.** QEMU loads the image at
`RAM_base + text_offset`. RAM starts at `0x4000_0000` on `virt`, and `text_offset` is
`0x8_0000`, so we land at `0x4008_0000`. That is exactly where `link-aarch64.ld` puts us. These are
two independent numbers that have to match, and nothing checks them for you.

**`image_size` must cover `.bss` and the stack, not just the file.** The flat binary stops
after `.data`, because `.bss` occupies no file bytes ([elf.md](elf.md)). But `image_size` is
a statement about *memory*, not about the file. Understate it and the bootloader will happily
place the device tree blob on top of our `.bss` or our boot stack, and we'll destroy it the
first time we push a stack frame. So `link-aarch64.ld` computes it as `__stack_top - __image_start`.

**`code0` must not touch `x0`.** The entry point is the first byte of the image, so `code0`
executes before anything else in the kernel. Ours is a single `b _boot`, which leaves the
device tree pointer sitting exactly where QEMU left it. `_boot`'s own first instruction stashes
it in `x19`, which the `eret` in `enter_el1` preserves along with every other general register.

**What is verified here and what is only promised.** That QEMU puts a device tree in `x0` is
verified on every run: `device_tree_pointer_was_provided` fails on a zero, and it exists because
milestone 1 printed `x0` and got one. That U-Boot's `booti` does the same is Linux's boot
protocol in the same document as the header above (x0 is the DTB's physical address, x1 to x3
zero), which is a firmware contract this project has not yet held a board to. Milestone 127's
bench list checks it first, for the reason this file exists at all: the last time this tree
believed a boot-register claim without printing it, the claim was wrong.

### The failure mode has no diagnostics

If the magic is wrong, QEMU does not complain. It silently decides the file is an anonymous
blob, boots it anyway, and hands you a zero in `x0`. Which looks exactly like a bug in your
own code.

That is why `cargo xtask image` exists, and why there are two tests.

## The other half of the handoff: which exception level

The header settles what QEMU does with the file. It says nothing about which exception level
the payload starts at, and that is a separate fact from a separate source: the machine, not
the image.

| What starts us | Entry level | `/psci` method |
|---|---|---|
| QEMU `virt` (the default) | **EL1** | `hvc` |
| QEMU `virt,virtualization=on` (`NIFE_EL2=1 script/test`) | **EL2** | `smc` |
| U-Boot on a board, with TF-A's BL31 below it | **EL2** | `smc` |

The two columns are not independent, and the reason is worth holding onto because it looks
like a QEMU quirk and is not. PSCI has to be implemented *below* whoever calls it. When the
kernel runs at EL1 with nothing at EL2, an `hvc` traps up to a vacant EL2 that the emulator
can implement PSCI in, so `hvc` is the conduit. When there is a real EL2 (a board's, or
`virtualization=on`'s) the kernel drops itself into EL1 *underneath* it, an `hvc` would now
arrive at that EL2 rather than at firmware, and the conduit has to be `smc`. The machine says
which in `/psci`, the kernel reads it (milestone 100), and `arch::aarch64::isa`'s test asserts
the pairing rather than either value alone.

**PSCI starts secondaries at the highest implemented non-secure level, whichever level called
`CPU_ON`.** So under an EL2 the secondaries arrive at EL2 even though EL1 asked for them, and
`secondary_boot` takes the same drop `_boot` does. A kernel that dropped only core 0 would come
up single-core-correct and fault on its second core.

`boot.s` reads `CurrentEL` and drops itself rather than being built two ways; `arch::entry_el`
is the record of what the entry was, and the boot banner prints it:

```
  exception level : EL1  (entered at EL2, dropped in boot.s)
```

See `kernel/src/arch/aarch64/boot.s` (`enter_el1`) for which registers the drop configures and
why each one is on the list, and milestone 127 (the seL4 machine) for the board this was built
for.

## Why the tests boot the same way the real thing does

`.cargo/config.toml` points cargo's runner at `helpers/qemu-runner-aarch64.sh`, which strips the ELF
to a flat binary before launching QEMU. So `cargo test` and `cargo xtask run` take the
identical boot path.

That was a deliberate choice. It would have been easier to leave the tests booting the ELF
(they don't need the device tree). But a test harness that exercises a different boot path
than the real kernel is testing a fiction, and the difference would eventually be exactly
where a bug lived.

## The two tests

`device_tree_pointer_was_provided` asserts `x0` was nonzero. A zero means we've silently
regressed to the ELF path, which is easy to do by editing one line of the runner script and
otherwise impossible to notice.

`device_tree_has_the_right_magic` reads the first four bytes at that pointer and expects
`0xd00dfeed`. A nonzero pointer is necessary but not sufficient; this proves it points at an
actual device tree.

Note the byte order: the DTB magic is big-endian, so we `u32::from_be` it. The device tree
format predates the little-endian consensus and never changed. Every field in a DTB is
big-endian, which will matter a lot when we actually parse one.

## What we get from this

The kernel no longer *assumes* what machine it's on. It can be told.

Right now we still hardcode `0x0900_0000` for the UART, which is a fact we looked up. The DTB
is the machine telling us, and it also describes where RAM starts and ends (milestone 3 wants
that), where the interrupt controller lives (milestone 5 wants that), and how many CPUs exist.

## And it moves us toward the Pi

A Raspberry Pi boots a flat `kernel8.img`, not an ELF. It has no use for our ELF at all.

So this wasn't a detour from the Pi port. It was the first step of it.

---

*Add to this file as new boot-protocol details come up.*

# ELF

Executable and Linkable Format. The standard container for compiled code on
Unix-like systems. (macOS uses Mach-O, Windows uses PE, but every bare-metal ARM and
RISC-V toolchain emits ELF, including ours.)

An ELF file is a bag of bytes plus metadata describing what those bytes are and where
they belong.

## One file, two views

The whole design, and what "Executable *and* Linkable" is pointing at. The same bytes are
indexed twice, for two audiences.

| | **Sections** (linking view) | **Segments** (execution view) |
|---|---|---|
| Granularity | fine: dozens | coarse: usually 2-4 |
| Examples | `.text`, `.rodata`, `.data`, `.bss`, `.symtab`, `.debug_info` | "load 8 KB at `0x40080000`, read+execute" |
| Who reads it | the **linker**, `objdump`, GDB | whoever **loads** the file |

The linker thinks in sections because it merges, sorts, and places them (exactly what
our [linker script](linker-scripts.md) does). A loader thinks in segments because it
doesn't care about `.rodata` vs `.text`, only "which contiguous chunks go where, with what
permissions." Sections are grouped into segments by permission before shipping.

Same bytes, two indexes.

## The ELF header

First 64 bytes of the file:

- Magic number `\x7f E L F`, so anything can identify the format instantly
- 64-bit? Little-endian? Machine type (`EM_AARCH64`)?
- **The entry point address** (`e_entry`)

That last one is the payoff. `e_entry` is a single 64-bit number saying "start executing
here," and it is *exactly* what `ENTRY(_start)` in our linker script sets. The linker script
writes an address into the ELF header; QEMU reads it back out and puts it in the program
counter. That's the handoff.

## Magic numbers, and `BadMagic`

A **magic number** is a fixed, known byte sequence at the start of a file that identifies its
format. The format's author picks an arbitrary constant, every file of that format begins with it,
and every parser checks it before trusting a single other byte. Match: "plausibly one of mine,
proceed." Mismatch: stop now, it's the wrong kind of thing. It's called "magic" because the value
means nothing on its own; it's just an agreed sentinel.

ELF's is `7F 45 4C 46` (`␡ELF`), the first four bytes above. Ours are not the only ones in the
tree: `crates/nifefs` tags its superblock with `CRKR0002`. Both crates return `Error::BadMagic`
when the check fails, and it is usually the *first* thing they check, because it is the cheapest
guard there is: one comparison that turns "wrong format" from an unbounded disaster into an
immediate, named refusal.

We hit it for real at milestone 19f. The initrd became a nifefs archive (first bytes `CRKR0001`
then, since the 2026-08-01 name widening, `CRKR0002`), but the milestone tour and several tests
still handed that blob to the *ELF* loader. The parser read
`43 52 4B 52` (`CRKR`) where it wanted `7F 45 4C 46`, and refused: `LoadError::NotLoadable(BadMagic)`,
followed by "the kernel is fine." The bug was upstream (feeding the wrong bytes); the magic check is
what surfaced it cleanly instead of letting the loader read archive bytes as machine code and crash
three steps later somewhere unrelated. See notes/progenitor-and-loading.md for the 19f archive.

## What QEMU does with `-kernel kernel.elf`

Deliberately, almost nothing:

1. Read the ELF header. Confirm it's aarch64.
2. Walk the **program headers** (segments). For each loadable one, copy its bytes to the
   physical address it names.
3. Set the program counter to `e_entry`.
4. Release the CPU.

That is the entire "boot process." No relocation, no address space, no stack, no `argv`.

### Nobody zeroes `.bss`

`.bss` occupies zero bytes in the file (it's just "reserve N bytes here"), so there is
nothing to copy. In a normal program the C runtime zeroes it before calling `main`.

We have no C runtime. Real hardware certainly won't do it, and we don't rely on QEMU to.
So `boot.s` zeroes it by hand. That loop is not paranoia; it is the missing piece of a
runtime we don't have.

## Why ELF and not a flat binary

You could strip all metadata and get a **flat binary** (`objcopy -O binary`): raw bytes,
loaded at a fixed address, no header, no structure. That's what a real Raspberry Pi wants
(`kernel8.img`). Most primitive thing possible.

We use ELF for two practical reasons:

The entry point travels with the file. A flat binary loader has to *assume* execution
starts at byte zero. ELF says so explicitly.

Symbols. ELF carries `.symtab` (names → addresses) and `.debug_*` (DWARF). This is how
GDB knows `0x400800f0` is `kernel_main`, and how it shows the Rust source line you're
stopped on instead of a raw address. Debugging a kernel without symbols is miserable, and
it's a big reason we set up the GDB path early.

Symbols also flow the other way, which we already rely on: `__bss_start` and `__stack_top`
are symbols the linker invents and writes into the ELF, so our assembly can reference
addresses it has no way of knowing at compile time.

## Why don't macOS and Windows use it?

History, not merit. Mach-O came with NeXTSTEP into Mac OS X, and PE extended COFF for Windows NT,
both before ELF became the Unix standard. An executable format is a compatibility boundary with
enormous switching costs and near-zero switching benefits, so it is decided early and frozen. The
punchline is that UEFI firmware boots PE, so a UEFI bootloader is formally a Windows executable,
which this tree now builds for x86_64 (`uefi_loader`, `BOOTX64.EFI`).
[`why-not-mach-o-or-pe.md`](elf/why-not-mach-o-or-pe.md) has the history, the fat-binary argument,
and the punchline.
## Poking at it

Once we build, all of these work on our kernel:

```bash
readelf -h kernel.elf     # the header: entry point, machine type
readelf -l kernel.elf     # program headers (segments) - what QEMU reads
readelf -S kernel.elf     # section headers - what the linker made
nm kernel.elf             # symbols: where did __bss_start land?
```

Running `readelf -l` and seeing `LOAD 0x40080000` is a nice moment. It's the linker
script's decisions, made visible.

---

*Add to this file as new ELF details come up.*

---

# Milestone 7c: the kernel actually loads one

The above was written at milestone 1, when ELF was a thing we *read about*. This is the part
where a file we did not compile becomes a running process.

## How the binary gets in: the initrd, which is how Linux does it

There is no filesystem yet (that is milestone 9). So the program arrives the way Linux's
initramfs does:

1. QEMU is given `-initrd target/.../hello`.
2. QEMU loads the file into RAM, somewhere, and writes the address into the device tree it
   generates, at `/chosen/linux,initrd-start` and `linux,initrd-end`.
3. The kernel reads it there ([device-tree.md](device-tree.md)) and tells the frame allocator
   that region is **forbidden**, or it would hand the program's own bytes out as scratch memory.

That reservation was written at milestone 3, with a comment saying milestones 8 and 10 would
want it. It turned out to be 7c.

Nothing about the binary is known to the kernel at build time. No `include_bytes!`, no build
script reaching into another crate's `target/`. The kernel is handed an address by the firmware
and finds a program there, which is exactly the relationship a real kernel has with its
bootloader.

## The parser is a HOST crate, and that is the whole trick

`crates/static_elf` is pure logic and compiles for the laptop, so its tests run in **milliseconds with
no emulator** ([DECISIONS §7](../design/decisions/0007-testing-harness.md)).

Which means forging a malicious binary is eleven lines:

```rust
let bytes = Builder::new()
    .seg(PF_R | PF_W | PF_X, 0x40_0000, &[0xaa; 16], 16)   // W AND X
    .build();

assert_eq!(Elf::parse(&bytes).err(), Some(Error::WritableAndExecutable));
```

Getting a real toolchain to *emit* that file, packing it into an initrd, and booting QEMU to
watch it be rejected would be a day of work and a twenty-second test. Here it is a microsecond,
and there are fourteen of them.

## What the loader refuses, and why each one is a real file and not a hypothetical

| Refused | Because an ELF can simply **ask** |
|---|---|
| `WritableAndExecutable` | ...for a page that is both. That is the thing every exploit wants, and the file is allowed to request it. `page_tables::Flags` has no constructor that returns one; this is what stops a *file* talking us into building one. |
| `SegmentOutOfBounds` | `p_offset` and `p_filesz` are **attacker-controlled**. A loader that trusts them reads past the buffer and then **maps what it finds into a process**. |
| `NotAarch64` | ...to be an x86 binary. Caught here, rather than as a mystery illegal-instruction fault at EL0. |
| `NeedsRelocation` | ...to be a PIE. It expects a dynamic linker. We are not one, and running it anyway means jumping to an address that means nothing. |
| `EntryNotExecutable` | ...to start in its `.data` segment. |
| `SegmentTruncated` | `p_memsz < p_filesz`. |

And one the parser cannot catch, because it is a kernel policy and not an ELF fact:

## The attack: a binary that asks to be loaded over the kernel

An ELF names its own load address. So a hostile one names `0xffff_0000_4008_0000` and waits
to see whether the loader is credulous.

It is refused by construction, not by a check we remembered to write. The user `Mapper` is
built with `Half::Low`, and a high address is **not a thing it can express**:

```rust
if !self.half.contains(va) {
    return Err(MapError::WrongHalf);
}
```

That guard has been in `page_tables` since milestone 4 (MMU on: page tables, the kernel heap, the high half). A *host test*
discovered that bits 63:48 are not translated ([higher-half.md](higher-half.md)), and we needed a
way to say which table a mapping belongs to. It has been sitting there for three milestones,
waiting for this file.

This is the same move as `TlbFlush`'s `Drop` and the lock ranking: make the bad state
unrepresentable rather than checking for it.

## `memsz > filesz` is `.bss`, and forgetting it is the classic loader bug

```
Type: PT_LOAD    VirtualAddress: 0x402000    FileSize: 8    MemSize: 16
```

The file carries eight bytes. The program expects sixteen, and the other eight must be
zero. Copy `filesz` and stop, and the program's `.bss` holds whatever the previous owner of
that frame left behind. Every uninitialized-memory bug in that program becomes an information
leak from a dead process.

Our loader zeroes every page before copying, so the tail is free. But only because we thought
about it, and the test binary deliberately has a `.bss` variable it checks is zero, and a
`crates/static_elf` test asserts the test binary *has* a `.bss` at all, so the check cannot go vacuous.

## `p_paddr` is not `p_vaddr`, and the trap is that it usually is

A `PT_LOAD` header carries two addresses. `p_vaddr` is where the program wants the segment in its
own address space; `p_paddr` is where it wants the segment in *physical* memory. `crates/static_elf`
exposes both, but they answer different questions, and almost nothing here should ask the second
one.

For every user program in this tree the two are equal. The linker scripts say so, and nothing
we build separates them. That is what makes this dangerous rather than tedious: a loader that
reaches for the wrong field is correct in every test anyone will run, and wrong only on the one
path where the distinction is real.

That path is the kernel image. Measured on the `x86_64-unknown-none` release build of this
tree:

```
PT_LOAD  R X  vaddr=0x101000            paddr=0x101000     .boot, linked low, VA == PA
PT_LOAD  R X  vaddr=0xffffffff80109000  paddr=0x109000     .text, a fixed 0xffffffff80000000 offset
PT_LOAD  R X  vaddr=0x8000              paddr=0x12b000     .ap_trampoline, neither of the above
```

Three different relationships in one file, which is why no caller can recover one address from
the other by subtracting a constant. The trampoline is the interesting one, and it inverts the usual
sense of both words. Its bytes *ship* at `0x12b000`, because `AT()` places them after `.rodata`
where nothing writes them at runtime. They *execute* at `0x8000`, because a STARTUP IPI can only
name a physical page below 1 MiB and the secondary core arrives there in real mode with paging off.
Its VMA was chosen to be its eventual execution address; its LMA is merely where the image carries
it until `ap_boot::prepare` copies it down.

The rule that falls out:

- A loader mapping into a fresh address space wants `vaddr` and never `paddr`. That is
  `kernel/src/user.rs`. It takes frames wherever the allocator gives them and maps them where the
  program asked, so the file's physical address is not merely unused, it is a claim about a
  decision the loader is making itself.
- A loader placing an image at fixed physical addresses wants `paddr`. That is firmware-shaped
  work, and it is why the field is exposed at all (milestone 196 (a physical address on `elf::Segment`)). Without it, such a loader has to
  re-implement the ELF parse, and two readers of one format is what AGENTS.md rule 7 exists to
  prevent.

Nothing in `parse` validates `paddr`. The bounds checks, the overlap check, the entry-point
check and the `vaddr + memsz` overflow guard are all about the virtual layout. A physical address
that is zero, or that collides with another segment's, is a fact about the file rather than an
error, and the loader placing images physically owns that judgment.

### BUGS

- The field is a plain `u64`, so nothing stops a consumer using it as a virtual address, and the
  only guard is its doc comment. That is rung three of AGENTS.md's ladder, chosen deliberately:
  milestone 200 gives virtual and physical addresses different types across the whole tree, and
  typing these two fields alone would claim a distinction the rest of the tree does not make.
  Until 200 lands, this note and that comment are the whole mechanism.
- Every test but one has them equal, which is the hazard restated. `crates/static_elf`'s
  `a_physical_address_may_differ_from_the_virtual_one` is the single case that would catch a
  consumer conflating them, and it only covers the parse, not any consumer.
- The field did not, on its own, retire the second reader, which was milestone 196's other half.
  The blocker was not `p_paddr`: **`crates/static_elf` refused the kernel image** with
  `Error::WritableAndExecutable`, because the `x86_64` linker script folded `.text.boot` and
  `.data.boot` into one output section and the 32-bit trampoline shipped as a single `RWX` `PT_LOAD`
  at `0x101000`. Measured rather than argued, twice. 196's lane patched that one segment's `p_flags`
  to `RX` in a copy and watched the whole file become acceptable. Milestone 208 (the x86_64
  kernel image ships an RWX segment) then split the section and ran the shipped artifact through
  `Elf::parse`, which accepts it: all ten `PT_LOAD`s, the three `NOLOAD` reservations and the
  trampoline's split addresses included. Nothing else in the validating parser objected, so the
  blocker really was one linker-script line.
- The second reader is gone (milestone 208). `uefi_loader/src/image.rs` is now a physical-span
  helper and a refusal-wording table over `static_elf::Elf`, and the loader validates the kernel it places
  instead of trusting it. What made the deletion possible was a security fix rather than an API
  change, which is the part worth remembering. The kernel's own image was the one thing in the tree
  breaking the W^X rule `crates/static_elf` and `page_tables::Flags` enforce everywhere else, and the parser
  refusing it is that rule catching the tree. `script/image-permissions` is what keeps it caught.

## The loader honors permissions and does not widen them

An ELF's `.rodata` segment is `PF_R` alone. The tempting shortcut is to map every
non-executable segment as `user_data()`, which is writable, quietly granting the program
authority its own file never asked for.

`page_tables::Flags` grew a `user_read_only_data()` for exactly this. Three segment shapes, three
constructors, no widening:

| ELF says | We map |
|---|---|
| `PF_R \| PF_X` | `user_code()`: readable and executable at EL0, **PXN** so the kernel can never execute it |
| `PF_R` | `user_read_only_data()`: readable at EL0 and *nothing else* |
| `PF_R \| PF_W` | `user_data()`: readable and writable, **UXN and PXN** |
| `PF_W \| PF_X` | *refused* |

## The program has no syscalls, and says so in the only two words it has

There is no ABI yet ([DECISIONS §10](../design/decisions/0010-capability-microkernel.md): the syscall surface gets designed at 7d,
against a capability table). So the test binary cannot tell the kernel anything.

Instead it checks its own image and speaks with:

- **`svc`**: everything I expected about my own memory is true.
- **`brk`**: it is not. (Which the kernel treats as a fault, and kills it.)

No data crosses the boundary. The kernel counts `svc`s and faults and learns whether its
loader is correct, without either side agreeing on the meaning of a single register. `svc` and no
fault means: `.text` executed, `.rodata` was readable, `.data` was copied from the file, `.bss`
was zeroed, and the stack worked well enough to recurse eight frames.

### And a `brk` from EL0 had to stop being a breakpoint

Writing that program exposed a bug. `exception_dispatch` matched `ec::BRK64` before it checked
which exception level the trap came from, so a `brk` from a *user* program would have been
stepped over as if it were one of ours. A user program could park a `brk` in a loop and be
immortal.

A breakpoint is a debugging affordance for code we trust. From EL0 it is a fault.

## What it prints

```
    initrd : 813656 bytes at 0x44000000, from the device tree
    hello  : a real ELF, loaded from the initrd, ran and verified its own .text/.rodata/.data/.bss

  the machine has run code it does not trust, and taken the CPU back.
  and it did not compile it, or link it, or ever see it before this boot.
```

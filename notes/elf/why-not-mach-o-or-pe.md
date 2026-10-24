# Why don't macOS and Windows use ELF?

*Name: provisional, minted 2026-10-11 (UTC) by lane/seventh-ten-longest-notes, after the parent's
"Why don't macOS and Windows use it?". calef names files.*

An appendix to [`notes/elf.md`](../elf.md). It holds the history of Mach-O and PE, why an
executable format is frozen early, and the punchline that UEFI firmware boots PE.

History, not merit. The three formats are far more alike than different: header, symbol
table, relocations, "load these bytes here with these permissions." Nobody looked at ELF
and found it wanting.

The timing is the whole story. ELF was published ~1988 with System V Release 4 and took
years to become *the* Unix standard (Linux didn't switch from `a.out` to ELF until ~1995).
Both Apple's and Microsoft's formats were locked in before that happened.

Mach-O comes from the Mach kernel (CMU, mid-1980s). NeXT built NeXTSTEP on Mach + BSD
and used Mach-O. Apple bought NeXT in 1997, NeXTSTEP became Mac OS X, and Mach-O came
along. Apple never *chose* Mach-O over ELF; it was already in the building.

One real technical reason it stuck: Mach-O supports *fat binaries* (one file containing
code for multiple architectures). Apple changes CPU architecture roughly every decade
(68k → PowerPC → Intel → Apple Silicon) and each transition was survivable partly because
one `.app` could run natively on both old and new machines. ELF has no equivalent.

> The interesting version of the fat-binary argument is not about ISAs at all, it's about
> microarchitecture variants within one ISA (AVX-512, LSE atomics, SVE). That case is
> live for nife and is written up in
> [design/fat-binaries.md](../../design/fat-binaries.md).

PE (Portable Executable, Windows NT 1993) extends COFF, AT&T's *previous* Unix
object format from the early 1980s: the one ELF was designed to replace. NT development
started ~1988; the team took the well-understood format they had and extended it for DLLs
and Windows' resource system. They had zero incentive to adopt a brand-new, unproven,
competitor's-Unix standard.

The principle: an executable format is a compatibility boundary with enormous switching
costs and near-zero switching benefits. Compiler, assembler, linker, loader, dynamic
linker, debugger, profiler, `nm`, `strip`, and the kernel's `exec` path all have to agree,
and changing it breaks every binary ever compiled. So it gets decided very early, usually
by "what did the builders already have lying around," and then frozen forever.

## The punchline

UEFI firmware uses PE. Microsoft's format is what boots essentially every modern x86
and ARM PC, including Linux machines. A UEFI bootloader is formally a Windows executable.

So had we gone x86_64 + UEFI, we'd have been asking Rust to emit a Windows PE binary to
boot a Unix-flavored kernel from a Mac. Not a joke: that is literally what the
`x86_64-unknown-uefi` target does.

We dodged it by picking aarch64 and QEMU's `-kernel`, which takes ELF directly.

*Corrected 2026-10-11 (UTC), verified against the tree:* the dodge did not last. Milestone 87 (the
x86_64 bare-metal machine) built `uefi_loader`, so the tree does ask Rust for exactly that:
`BOOTX64.EFI` from the `x86_64-unknown-uefi` target, beside `BOOTAA64.EFI`. riscv64 has no UEFI
target in rustc, so `cargo xtask stick` turns its loader's ELF into PE/COFF.
`notes/boot-stick.md` has all three boot files.

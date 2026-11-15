---
status: BUILT
raised: 2026-08-31
built: 2026-09-02
---
# 208. The x86_64 kernel image ships an RWX segment, and it is the reason a second ELF reader exists

Built. It stood between milestone 196 (a physical address on `elf::Segment`) and deleting a
duplicate ELF parser. It needed no ruling: the change was one linker script, and the parser that
refused the image already existed.

In brief. `kernel/link-x86_64.ld` folded `KEEP(*(.text.boot))` and `KEEP(*(.data.boot))` into one
output section, so the 32-bit trampoline shipped as a single `RWX` `PT_LOAD` at `0x101000`.

`crates/elf::Elf::parse` refused that image outright, with `Error::WritableAndExecutable`. So the
kernel's own image violated the W^X rule that `crates/elf` and `page_tables::Flags` enforce everywhere
else in the tree.

## How it was found, which is the useful part

Milestone 196's premise was that `uefi_loader` carries its own forty-line ELF reader because
`elf::Segment` exposed no physical address. That was true and it was not the blocker. The lane
added the field, then measured rather than assumed: it built the kernel for `x86_64-unknown-none`,
dumped the program headers, and ran the real bytes through `Elf::parse`.

`REFUSED: WritableAndExecutable`. Patching that one segment's `p_flags` from `RWX` to `RX` in a copy
made `Elf::parse` accept the whole file: all ten `PT_LOAD`s, the three `NOLOAD` reservations,
and the trampoline's split addresses. Nothing else in the validating parser objected.

So the duplicate reader existed because of a W^X violation, not because of a missing field, and
nobody knew that until somebody tried to delete it.

## What was built

One linker-script split, `.boot` into `.boot_text` (RX) and `.boot_data` (RW), each page-aligned
because a permission boundary is a page boundary. It costs one 4 KiB page of image and shifts
everything after it up by that much.

The program headers, before and after, from the shipped debug artifact:

| | before | after |
|---|---|---|
| `PT_LOAD` at `0x101000` | `RWX`, `0x1000` | `R-X`, `0x1000` (`.boot_text`) |
| `PT_LOAD` at `0x102000` | `RW-`, memsz `0x7000` (`.boot_scratch` alone) | `RW-`, memsz `0x8000` (`.boot_data` + `.boot_scratch`) |
| `PT_LOAD` count | 10 | 10 |
| `Elf::parse` | `REFUSED: WritableAndExecutable` | **`ACCEPTED`**, all ten segments |

The `Elf::parse` result is from the actual shipped artifact, not a copy with patched flags. The
harness was the tree's own `crates/elf/src/lib.rs` with exactly one line changed, the compile-time
`EXPECTED_MACHINE` selection, so a host binary would accept `EM_X86_64`; every check under test is
byte-identical. Run against the pre-fix build it printed `REFUSED: WritableAndExecutable`, and
against the post-fix build `ACCEPTED`, listing all ten `PT_LOAD`s including the trampoline's
`p_vaddr 0x8000` / `p_paddr 0x166000`.

The surprise worth keeping, because reading the source would have found nothing. `boot.s`
declares `.section .data.boot, "a"` with no `w`, and the contents genuinely are read-only: the
boot GDT and its `lgdt` pointer. The assembler overrides it. LLVM gives a section whose name begins
with `.data` its default ELF flags, `SHF_ALLOC | SHF_WRITE`, and the object file measurably carries
`WA`. That `W` is the one that was being unioned with `.text.boot`'s `X`. Renaming the input section
would make `.boot_data` read-only too; that is a `boot.s` change rather than a linker-script one, and
W^X holds either way.

`uefi_loader`'s duplicate reader is deleted, which was this block's stated unlock and which was
re-measured rather than assumed. `uefi_loader/src/image.rs` was a forty-line header walk with its own
`Error` enum; it is now `physical_span` over `elf::Segment::paddr`, plus this loader's own wording for
every way `Elf::parse` can refuse, plus a `parse` that joins them. The premise held with one
correction: `Elf::parse` does not provide `physical_span`, and should not, because a program loader
maps at `p_vaddr` and a firmware loader places at `p_paddr`, and on this image the two are unrelated.
So the module still exists; it is no longer a *reader*.

And the loader now validates. The old module argued it need not, because the kernel comes from
the same build that produced the loader. That made it the one place in the tree where an ELF was
trusted rather than checked, and it is now moot: a kernel image that regressed to `RWX` fails to boot
on real firmware with a printed reason. Proved by booting, not by reasoning: `cargo xtask uefi-boot`
comes up under OVMF and reaches "boot complete", and `script/test --arch x86_64` is 190 passed, 68
skipped with its UEFI leg green.

The other two architectures are clean, and structurally rather than luckily. `link-aarch64.ld`
and `link-riscv64.ld` both fold `KEEP(*(.text.boot))` into `.text`, which is `AX`, and neither has a
`.data.boot` at all. The x86_64 script's low `.boot` section exists only because a multiboot/PVH
kernel is entered in 32-bit protected mode and needs a trampoline linked at its physical address,
which is the one structural divergence its own header already describes. Checked by building both
kernels and dumping their program headers: six `PT_LOAD`s each, `R-X` / `R--` / four `RW-`, no
segment with both bits.

## The gate

**`script/image-permissions`** (name provisional): builds all three kernels and fails on any `PT_LOAD`
that is both writable and executable. `--no-build` checks what is already built. In `script/gates`,
and a CI job of its own.

Proved both ways before it was wired up, which is the only way a gate is worth anything: green on
the three fixed images, and red on the pre-fix x86_64 one, naming the segment
(`vaddr 0x101000, paddr 0x101000, memsz 0x1000`).

**It does not call `Elf::parse`, and the reason is not laziness.** That parser binds its accepted
`e_machine` at **compile** time (`EXPECTED_MACHINE`, chosen by `target_arch`), deliberately, because a
kernel only ever loads binaries for its own architecture. So a host tool built once can accept exactly
one of the three kernels, and a W^X gate covering one architecture would have failed DECISIONS §19
(architectural parity is a tenet) on the day it was written. The script reads two fields, `p_type` and
`p_flags`, at the offsets ELF64 fixes. It is an auditor rather than a reader, it places nothing and
maps nothing, and it reports every violation rather than the first, which a parser returning one
error cannot. `Elf::parse` still does this work on both paths where it can: the kernel's user-program
loader, and now `uefi_loader` at every real-firmware boot.

Its false-positive risk, stated rather than asserted. AGENTS.md's rung two warns about gates whose
only effect is rejecting legitimate work, and `script/lint` has had three checks deleted for exactly
that. This is not that shape, and the test is falsifiable: it asserts a property the three artifacts
genuinely have today, measured, so a red run means the property is gone rather than that the rule
was too strict. There is also no legitimate reason for a nife kernel image to want the combination.
The kernel installs fine-grained W^X page tables over its own image on all three architectures, so a
segment asking for both is asking for something the kernel will not honor anyway. If one is ever
wanted, this script is where the argument has to be made and written down. The failure mode it can
actually have is the opposite one, a false *negative*: see BUGS.

## What this does and does not say about the confinement claims

It says the kernel image no longer contradicts them, and it says nothing about whether they hold.
The claims in `design/fatal-risks/README.md` are about what a confined component can reach at runtime, and
they are enforced by the page tables and the capability system, neither of which this touches. The
kernel already installed fine-grained W^X tables over its own image on all three architectures, so
**the RWX segment was never actually mapped RWX at runtime**. The `PT_LOAD` was a claim in a file that
the kernel then declined to honor.

What it was, and this is the part worth keeping, is the tree's own rule catching the tree. The
verification story here is not "we proved the kernel safe" but "we wrote the rule down once, in a
parser, and then it caught the one artifact nobody had ever run it against". That is an argument for
pointing existing checks at new inputs, which is what the gate now does permanently.

## BUGS

- **The gate reads flags, not semantics.** It cannot tell a segment that is `RX` and *maps* writable
  at runtime from one that does not, so it is a check on the image and not on the kernel. The runtime
  half is `page_tables::Flags`, which has no writable-and-executable constructor, and the two together are
  the mechanism; neither is it alone.
- **It is not a required check in the merge queue's ruleset**, because that is a repository setting
  rather than a file in this tree. A red run is visible and does not block a merge today. `script/gates`
  runs it locally, which is where a developer meets it. Making it required is calef's, and is one
  checkbox.
- **`Elf::parse`'s overlap check is over `p_vaddr`, not `p_paddr`**, so nothing rejects a linker script
  that made two segments' *physical* ranges collide. For a user program the two questions are the same;
  for the kernel image they are not, and `uefi_loader` asks the firmware for one span and copies into
  it, so a later segment would quietly overwrite an earlier one. No linker script in this tree can
  produce that (one script, one contiguous physical layout), which is why it is recorded in
  `uefi_loader/src/image.rs`'s own `BUGS` rather than checked.
- `.boot_data` is `RW` when its contents are read-only. The input section says `"a"`; the assembler
  gives any `.data*` section `SHF_ALLOC | SHF_WRITE` anyway. Making it genuinely read-only means
  renaming the input section in `boot.s`, which is a boot-path change for a hardening improvement
  rather than a correctness one, and it was left out of a milestone whose subject is the linker script.
- The trampoline's addresses are unusual and worth reading before editing: `p_vaddr` `0x8000`
  against `p_paddr` `0x166000` (it was `0x12b000` when this block was minted; the split moved it).
  Its bytes ship high (`AT()` places them after `.rodata`) and execute low (a STARTUP IPI can
  only name a page below 1 MiB). The image has three different vaddr/paddr relationships and this is
  the strangest.

## Follow-on

- **Recorded.** In `design/roadmap/0208-boot-section-wx.md`: the gate reads flags, not semantics, so
  it cannot tell a segment that is `RX` and maps writable at runtime from one that does not. It is a
  check on the image and not on the kernel. The runtime half is `page_tables::Flags`, which has no
  writable-and-executable constructor, and the two together are the mechanism.
- **Recorded.** In `uefi_loader/src/image.rs`'s own `BUGS`: `Elf::parse`'s overlap check is over
  `p_vaddr`, not `p_paddr`, so nothing rejects a linker script whose segments collide physically. No
  script in this tree can produce that, which is why it is recorded rather than checked.
- **Recorded.** In `design/roadmap/0208-boot-section-wx.md`: `.boot_data` is `RW` when its contents
  (the boot GDT and its `lgdt` pointer) are read-only, because LLVM gives any `.data*` section
  `SHF_ALLOC | SHF_WRITE` whatever `boot.s` says. Making it genuinely read-only is a `boot.s`
  rename, a hardening improvement rather than a correctness one, and it was left out of a milestone
  whose subject is the linker script.
- **Recorded.** In `design/roadmap/0208-boot-section-wx.md`: the trampoline's `p_vaddr` `0x8000`
  against `p_paddr` `0x166000` is the strangest of the image's three vaddr/paddr relationships and
  is worth reading before editing, since its bytes ship high and it executes low.
- **Milestone 340.** Make
  `script/image-permissions` a required check in the merge queue's ruleset. It is one checkbox and
  it is calef's, because it is a repository setting rather than a file in this tree. Until it is
  flipped, a red run is visible and merges anyway, so the gate reports rather than gates.

## Index row

Found by milestone 196's lane while trying to delete a duplicate ELF parser: `link-x86_64.ld`
folded `.text.boot` and `.data.boot` into one output section, so the trampoline shipped as a
single RWX `PT_LOAD` and `crates/elf::Elf::parse` refused the kernel's own image. Split in
two; the shipped artifact now parses, `uefi_loader`'s forty-line duplicate reader is deleted and
the loader validates what it places, and `script/image-permissions` gates all three images so a
linker script cannot regress it. aarch64 and riscv64 were already clean, structurally.

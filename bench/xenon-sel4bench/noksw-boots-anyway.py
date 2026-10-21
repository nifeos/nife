"""Make a skim-window-off seL4 kernel boot on a CPU whose microcode reports Meltdown, changing
one instruction (milestone 127 (the seL4 machine), bench/xenon-sel4bench/build.sh).

    python3 -I noksw-boots-anyway.py KERNEL_IN KERNEL_OUT

Name: provisional, minted 2026-10-21 (UTC) with bench/xenon-sel4bench/; not ratified.

seL4's try_boot_sys (src/arch/x86/kernel/boot_sys.c at the kernel sel4bench-manifest 11d0fc2c
pins) refuses to boot a kernel built with KernelSkimWindow off when CPUID enumerates
IA32_ARCH_CAPABILITIES and its RDCL_NO bit is clear. gcc 14.2.0 compiles that test to

    mov $0x10a,%ecx ; rdmsr ; test $0x1,%al ; je <return false>

and this replaces the 6-byte `je rel32` with a 6-byte NOP. Every other byte of the image is
unchanged, so every instruction sel4bench times sits at the same address with the same bytes as
in the published configuration's kernel. A source patch was tried first and refused: removing
the `return false` regrows boot_sys in the MCS build and moves all of .text by 60 bytes, which
changes the cache-line alignment of the very code being measured.

It refuses unless the sequence occurs exactly once, so a different compiler or kernel fails here
rather than producing a kernel patched somewhere else.
"""

import re
import sys

REFUSAL = re.compile(rb"\xb9\x0a\x01\x00\x00\x0f\x32\xa8\x01(\x0f\x84....)", re.S)
NOP6 = b"\x66\x0f\x1f\x44\x00\x00"


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: noksw-boots-anyway.py KERNEL_IN KERNEL_OUT")
    with open(sys.argv[1], "rb") as f:
        image = bytearray(f.read())
    found = list(REFUSAL.finditer(image))
    if len(found) != 1:
        sys.exit(f"noksw-boots-anyway: expected one refusal branch, found {len(found)}; not patched")
    at = found[0].start(1)
    image[at:at + 6] = NOP6
    with open(sys.argv[2], "wb") as f:
        f.write(image)
    print(f"noksw-boots-anyway: {sys.argv[2]}: je at file offset {at:#x} replaced by a 6-byte nop")


if __name__ == "__main__":
    main()

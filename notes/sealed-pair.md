# The sealed pair: checking a kernel against its archive on the host

A kernel image and the userspace archive beside it are one sealed set. The build compiles the
digests of the archive entries the kernel may enter into its `.rodata` (`kernel/build.rs`), and the
kernel refuses userspace whose bytes do not match (`kernel::trust::require`). A mismatched pair
boots, prints `MEASURED BOOT REFUSED` and halts, and that is discovered only after a walk to the
board. `crates/sealed_pair` asks the same question on the host, where the answer costs nothing.

## Searching bytes, not parsing them

`inspect` hashes each entry the kernel would hand over: the boot programs and the measurement table.
It then looks for each digest's 32 raw bytes inside the kernel image. That is what the trust root
compiles to, and a chance match is not worth a sentence. It needs no knowledge of where the array
landed, so one function reads a flat `booti` image, an ELF, and a UEFI boot file with the pair
embedded. Finding a digest proves the kernel vouches for those bytes. Not finding one cannot say
what the kernel expected instead, which is why a refusal names the archive's digest and never the
kernel's; `cargo xtask card-check` recovers the other by comparing against the tree's own build.

`inspect` also checks the archive's measurement table against the archive's own contents. That is
a separate refusal, the one the progenitor would make one level up.

## One list of boot programs

`BOOT_PROGRAMS` has three readers: `xtask::measure` writes the manifest from it, `uefi_loader`'s
build refuses an unsealed pair against it, and `inspect` reads a card with it. With one list, a
kernel cannot measure a name the checkers do not know about, which would be invisible to them.
`progenitor` comes first, and `inspect` reads it as the one program every archive must carry. The
six after it are not boot programs in the ordinary sense: they are milestone 19 (run a real
workload)'s ELF child builders. `kernel::user::spawn_elf_child_builder` enters each of them
directly, so the trust root has to name them, or the kernel could enter a program it never measured.

## What it cannot see

- It duplicates the kernel's check as an early warning. If they disagree, the kernel is right, and
  `Seal::explain` says so.
- It says nothing about a boot script or `extlinux.conf`. Nothing forces anyone to run it on a
  hand-written card; `uefi_loader/build.rs` makes it a build failure where the pair is built.
- It asks whether a digest is present, not whether the kernel checks it, so a build carrying the
  digest without reaching the check would read as sealed. Every build reaches the check today
  through `kernel::trust::require_program`. Option C in
  `design/roadmap/0563-a-seal-check-that-reads-bytes-cannot-see-a-check-that-was-dropped.md` prices
  a build that says so itself.

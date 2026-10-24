# The initial ramdisk archives

An appendix to [`notes/adding-a-program.md`](../adding-a-program.md): how `xtask/src/archive.rs`
turns the declared programs into the three initial ramdisks the kernels boot with.

## One list, three archives

Each architecture has its own archive, because each holds ELFs for its own `e_machine` and the
kernel's loader refuses any other. All three pack the same list: every `[[bin]]` in
`components/Cargo.toml` and `fixtures/Cargo.toml`, less what a package marks `packaged_only`.
[`why-it-works-this-way.md`](why-it-works-this-way.md) has why the list is read from `Cargo.toml`
rather than kept as a table.

The list is not filtered per architecture. Several programs cannot do their job everywhere.
`console`, `input` and `keyboard_driver` need port I/O that a ring-3 process cannot reach on
`x86_64` (DECISIONS §121 (what a device capability is when the device has no page)). `serial_driver` drives riscv64's UART, and `jh7110_entropy` is radon's.
They are packed anyway. An archive entry costs a directory slot and some bytes, nothing spawns a
program by accident, and the tests that would spawn one `skip!()` with the reason. A filter per
architecture would put the same fact in a second place, where it could disagree with the first.

The riscv64 and `x86_64` packers build the whole of both packages rather than a `--bin` list, so a
new `[[bin]]` reaches every archive with no second edit. The aarch64 packer only packs; `user()`
builds first, and every command that boots the aarch64 kernel goes through it.

## What else an archive carries

Beside the `[[bin]]` programs, each archive packs what was built for its target outside those two
packages, each entry present only if its own build ran:

- the `std` programs from the patched farm, of which `jig` and `mdr` are required in every image;
- the ported programs from basalt (`rg`, `ioping`, `hackbench`, `speedtest1`) and the C programs;
- the TLS graph's workloads;
- the FS server and `mkfs`, which are their own workspace and target.

Then come the image's package catalog and index sources, on every architecture: the progenitor
reads them for every install, and `jig` is handed a copy. Last comes the measurement table, so it
measures everything above it. The progenitor refuses to load a program whose bytes the table does
not vouch for. The packer writes the measurement manifest before the kernel is built, because
`kernel/build.rs` reads it to generate `TRUST_ROOT`. With no manifest that root is empty, and the
kernel refuses every boot program as `Unmeasured`.

## Why the archive is a nifefs image

The initial ramdisk is in the same nifefs format the virtio disk uses, so one parser serves both.
It is generated on every build and never checked in: a binary in git is one nobody can review.

## How the kernel finds it

On aarch64, `helpers/qemu-runner-aarch64.sh` passes the packed archive to QEMU as `-initrd`
(milestone 19f (run a real workload)). This is the road Linux's initramfs travels. QEMU loads the
file into RAM and writes its address into `/chosen/linux,initrd-start` in the device tree. The
kernel finds it there with `memory::initial_ramdisk_region`, built at milestone 3 (hand out physical
memory) for this. The kernel knows nothing about the contents at build time beyond the measurement
table's digests; that separation is what milestone 7c (user mode: EL0, capabilities, the ELF loader,
and IPC) set out to build. The raw user ELFs are only the packer's input.

## Checks at packing time

Packing refuses an archive rather than writing a partial one when:

- a `[[bin]]` block has a key the reader does not know (DECISIONS §46 (thin primitives or whole subsystems) keeps a TOML parser out of
  `xtask`, so the reader is strict instead);
- the list lacks `progenitor`, names a program twice, or misses a program that `grant_plan` lets
  the shell spawn or that the kernel measures at boot;
- an ELF falls outside the address-space map's image band, which the kernel's loader would only
  catch for the programs a given boot loads;
- `jig` or `mdr` is missing.

A host test also scans `kernel/src`, `system_tests/src` and `crates/system_initializer/src` for
programs looked up by name, so deleting a `[[bin]]` that something still loads fails in
`script/lint` rather than at boot.

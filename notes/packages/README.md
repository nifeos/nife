# Packages: the appendices to notes/packages.md

[`notes/packages.md`](../packages.md) carries the design of a nife package, its producer and its
installer, and is written so a reader can act without opening anything here. These files carry the
proofs and the measured costs behind it: [measurements.md](measurements.md), and the fetch by
name with the program no image carries: [fetching.md](fetching.md). Running bytes nobody vouched
for, §219 (how the shell names an installed program to the spawner)'s gate D2, is
[running-unvouched.md](running-unvouched.md). The owner's console, vouching for a build and the list
of users who may run new code (§221 (the boot prompt is the owner's console)), is
[vouching.md](vouching.md). Packages over the internet are
[over-the-internet.md](over-the-internet.md). The pinned TLS client those fetches speak through,
and the two programs that prove it under QEMU, are [pinned-tls.md](pinned-tls.md). The index
format survey is
[the-index-format.md](the-index-format.md). A package with several programs is [several-programs.md](several-programs.md).
Four candidate containers for a package (NIFEPKG2, tar, and each compressed with zstd), measured
for size, reader code, tooling and what each forces, are in
[container-format-measurements.md](container-format-measurements.md).
Serving a package as an Open Container Initiative (OCI) registry artifact, as Homebrew does on
ghcr.io, is [oci-artifacts-measurements.md](oci-artifacts-measurements.md).
The distribution's name, basalt, and the names refused for it are in [basalt.md](basalt.md).

Name: provisional (2026-09-26, milestone 198 (a package manager) rung 3a's installer lane). The
directory `notes/packages/` follows §212 (a prose budget)'s siting of an appendix beside its parent,
as `notes/benchmarks/` does.

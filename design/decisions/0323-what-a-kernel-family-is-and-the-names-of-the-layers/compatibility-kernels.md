# §323 appendix: compatibility kernels

Part of the survey behind [§323 (what a kernel family is, and the names of the project's
layers)](../0323-what-a-kernel-family-is-and-the-names-of-the-layers.md). That page states the two
consequences of a compatibility kernel for the family question. This one holds the prior art,
read on 2026-11-12 (UTC), and what a library of components looks like when other projects build
several systems from one.

## Where a foreign interface lives

A system can serve another system's calls in three places: in a user-space program over its own
kernel, inside its own kernel beside the native interface, or as a whole kernel built for that
interface alone.

| system | where Linux's interface lives | what its documentation says |
|---|---|---|
| Starnix on Fuchsia | a user-space program over Zircon | "a Fuchsia user-space program" and "a compatibility layer that translates requests (syscalls) from Linux programs to the appropriate Fuchsia subsystems" (fuchsia.dev, Starnix) |
| gVisor | a user-space application kernel over Linux | "an application kernel that implements a Linux-like interface", "written in a memory-safe language (Go) and runs in userspace"; its Sentry handles the calls and does not pass them to the host (gvisor.dev) |
| WSL 1 | inside the Windows kernel | WSL 1 "used a translation layer that was built by the WSL team"; WSL 2 replaced it with a real Linux kernel in a managed virtual machine for "full system call compatibility" (Microsoft Learn, comparing WSL versions) |
| FreeBSD's Linuxulator | inside the FreeBSD kernel, as loadable modules | "optional binary compatibility with Linux", with a separate Linux userland under `/compat/linux`; "Linux software requires more than just an ABI to work" (FreeBSD Handbook) |
| illumos LX zones | inside the illumos kernel, per zone | "the majority of Linux system calls are provided, along with emulation for a variety of Linux file systems", and a zone sets the Linux version it emulates; no Linux kernel modules (SmartOS lx(7)) |
| Asterinas | a whole kernel for Linux's interface | "a Linux-compatible, general-purpose OS kernel" that "supports 240+ Linux system calls", under MPL-2.0 (its README) |
| Starry OS | a whole kernel for Linux's interface, built on ArceOS components | "An experimental monolithic OS based on ArceOS"; the crate `starry-kernel` describes itself as "A Linux-compatible OS kernel built on ArceOS unikernel" |

The first two rows are what §323 calls a personality: a component over the native kernel, with the
native interface unchanged. The middle three put a second interface into one kernel, which under
(a) would make that kernel a member of two families. The last two are kernels of an adopted
interface. WSL is the one case that went back: Microsoft kept the translation layer as WSL 1 and
made a real Linux kernel the default, because translation never reached full compatibility.

NetBSD's rump kernels go the other way round. They run NetBSD's own kernel drivers in user space,
for example to mount an untrusted file system image so that "any damage" stays in a user-space
process (NetBSD wiki, rump kernels). They show kernel code reused outside its kernel, which is this
project's library idea, rather than a foreign interface served.

## Licensing

Linux's `LICENSES/exceptions/Linux-syscall-note` says its copyright "does *not* cover user programs
that use kernel services by normal system calls". That note concerns programs that call Linux, not
a kernel that serves the same calls. The evidence for the second case is the projects that do it
under other licenses: gVisor (Apache-2.0), Asterinas (MPL-2.0), Starry OS (Apache-2.0). Starry
takes Linux's constants and layouts from `linux-raw-sys`, licensed
`Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` and generated from Linux's user-space headers,
which carry that note. Taking code from Linux itself would bring in the GPL. FreeBSD's
`sys/compat/linux` files read on this date carry `BSD-2-Clause` (`linux_file.c`, `linux_socket.c`)
or `BSD-3-Clause` (`linux_misc.c`) headers. None of this is legal advice.

## Several systems from one library: ArceOS

ArceOS is the nearest prior art for the project's intent: one set of component crates, several
systems built from it. Its README calls it "An experimental modular operating system (or
unikernel) written in Rust", inspired by Unikraft, for x86_64, riscv64, aarch64 and loongarch64.
Programs link `axstd`, a `std`-like library, and platform support lives in separate `axplat-*`
packages. Its license files cover Apache-2.0, GPL-3.0 and MulanPSL-2.0.

Its components left one crate per repository under the `arceos-org` organization and reached
crates.io under plain names, with no brand. crates.io figures on 2026-11-12:

| crate | first published | downloads | reverse dependencies |
|---|---|---|---|
| `memory_addr` | 2023-05-09 | 184,324 | 31 |
| `kernel_guard` | 2023-05-09 | 145,041 | |
| `axerrno` | 2024-07-17 | 190,939 | 37 |
| `page_table_multiarch` | 2024-07-16 | 118,449 | |
| `axcpu` | 2025-05-12 | 57,575 | |
| `axplat` | 2025-07-02 | 53,602 | |

The maintainer's survey of the same day finds most reverse dependencies inside the family: 23 of
31 for `memory_addr` and 28 of 37 for `axerrno`, the rest mostly tutorial exercises. The downloads
come mostly from the family's own builds. This file did not repeat that classification.

Three systems come from those components under names of their own: ArceOS the unikernel, Starry OS
the Linux-compatible monolithic kernel, and Axvisor, a hypervisor in a separate organization.

In 2026 the components moved into one repository, `rcore-os/tgoskits` ("An integrated os-level
crates repository for operating system and virtualization development", Apache-2.0, created
2026-03-13). Its top level holds `components`, `drivers`, `fs`, `memory`, `net`, `platforms`, `os`
and `virtualization`, with one `xtask` and a `release-plz.toml`. Crates published from it carry an
`ax-` prefix: `axhal` (0.3.0-preview.3, from the ArceOS repository) became `ax-hal` (0.8.4, from
tgoskits). `starry-kernel` (0.12.1) and `starryos` (0.5.39) publish from it too. This file found no
stated reason for the prefix.

Two facts for the later rulings, recorded and not recommended. ArceOS named its systems separately,
over a component library that first carried no brand and later took a family prefix. And it moved
from one repository per crate to one repository after about three years.

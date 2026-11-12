---
status: PROPOSED
raised: 2026-11-12
---

# 323. What a kernel family is, and the names of the project's layers

*The section number, the file name and this title are provisional until the merge lands them.*

The maintainer raised this on 2026-11-12 (UTC), on the intent that the crates are the product: a
library of components from which the project builds several operating systems. Counts come from
`nifeos/nife@a3c7d4647` and `nifeos/basalt@ecdfee7`, and each gives its command so a reviewer can
take it again at merge. This file rules nothing. It asks one question now and lays out the options
for each later one, and each layer's name is its own later ruling. The appendix,
[the five layers](0323-what-a-kernel-family-is-and-the-names-of-the-layers/the-five-layers.md),
holds the inventory, the reversibility of each layer and every candidate name with its checks.

## What this decides

Milestone 120 (the rename: the OS becomes `nife`, and the project gets an organization) chose the
names for one system. It named the kernel for the Earth's core, `nife`, distributions for rocks
(`basalt`), and programs and components for minerals. It kept the organization's name apart from
the OS's, so that the distribution and the programs would read as peers of the kernel rather than
as its parts. Since 2026-10-03 (UTC) the organization's name, `nifeos`, carries the kernel's.

The intent now is several operating systems from one library. The plan holds at least four
kernels: a capability microkernel (today's), a capability monokernel, an ambient-authority
microkernel and an ambient-authority monokernel. Names then sit at five layers:

1. the organization (the GitHub organization `nifeos` and the domain `nifeos.org`);
2. the library of components;
3. kernel families;
4. individual kernels;
5. distributions (today one, `basalt`: a manifest that pins each component at a commit, and the
   workflow that builds and tests the image).

One question comes before any name, because it decides which names exist and what each touches:
what a kernel family is. It is the only question this file puts for a ruling now.

## Is the premise true

Mostly. One part of it meets an existing decision.

- The tree builds one kernel for one platform. `targets/` holds three target specifications, all
  with `"os": "nife"` and none with a `target-family`. The tree has one ABI crate (`crates/abi`,
  `publish = false`) and one `std` port (`patches/std-nife`).
- The word "family" appears on 45 lines in 29 files of `design/decisions/` and `design/naming.md`
  (`git grep -c -iw family`), never as a defined term for a set of kernels.
- §10 (process model: capability-based, microkernel) and §82 (ambient authority is the problem;
  replacing the ecosystem, not confining it, is the end state) make ambient authority the problem
  this project exists to solve. Two of the four planned kernels have it. One library under both
  does not contradict §82 by itself, since §82 speaks of one system's end state. A record that
  names an ambient-authority family should still say how it stands beside §82. This file does not
  decide that.
- Milestone 1039 (the kernel leaves nife) moves the kernel to `nifeos/nife-kernel`, a name ruled
  2026-11-03 (UTC). That repository does not exist yet.

## Question 1: what a kernel family is

### Background

A program here targets three things. The compiler builds it for `target_os = "nife"`, it links the
`std` port, and it calls the kernel through the numbers in `crates/abi`. At spawn, the kernel
compares the program's `ABI_VERSION` with its own and refuses a mismatch.

Some bytes in every image also spell the project's name. Every program's capability manifest has
the ELF note owner `b"nife"` (`crates/capability_manifest_note`, ratified 2026-09-26). The boot
slot header reads `NIFESLOT` and the boot stack magic `NIFESTK1`. Packages travel as `.nifepkg`
files. §45 (a nife partition is `EC5CC08B-D749-4434-AC38-A274C50385BA`, and that never changes)
fixes the partition type.

With several kernels, each of these belongs either to every kernel of a family or to one kernel.
The two readings of "family" place them differently.

### (a) A family shares a binary interface (recommended)

Every kernel in a family serves one ABI, so a program built for the family runs on any of its
kernels without a rebuild. The family is then the platform a developer targets. It carries the
Rust `target_os`, the ABI crate, the `std` port, the SDK crates and the in-binary names above. A
program cannot tell which kernel of the family runs it, just as it cannot tell which build of
Zircon runs it.

The cost: the kernels in a family move together. The capability monokernel serves the same
syscall numbers, object methods, error codes and right bits as the microkernel. A new number goes
into both kernels or neither, and one version gate binds them. A kernel that wants a private
syscall leaves the family. The kernels stay free in everything a program cannot observe: where a
service runs, how the scheduler works, what the proofs cover.

The gain: one `target_os`, one published ABI crate and one `std` port per family. A third-party
author writes a `getrandom` backend or a `libc` port once per family. A distribution built for a
family runs on any of its kernels. Kernel names touch only repositories and images, the cheapest
names in the project.

### (b) A family shares a design

Each kernel is its own platform, with its own ABI, `target_os`, ABI crate and `std` port. The word
family then means only kernels that share a design and much code. L4Ka::Pistachio, Fiasco and seL4
all belong to the L4 family in this sense, and none runs the programs of another.

The cost: four kernels mean four `target_os` values, four ABI crates and four `std` ports. Every
third-party library that names the platform names each kernel. Every kernel name then costs what a
family name costs under (a), since each lands on crates.io and in the Rust tree.

The gain: each kernel changes its interface on its own schedule. The capability monokernel can add
a call the microkernel does not serve, with nothing to negotiate.

### An interface the project defines, or one it adopts

The project is also exploring compatibility kernels: whether the components, with other code, could
build a Linux-compatible kernel, a BSD-compatible kernel or both. Two consequences bear on the
question.

First, the ambient-authority family might adopt Linux's system-call interface instead of defining
its own. Under (a), that family's interface then belongs to Linux. Its programs target `linux`, and
its ABI crate describes Linux's interface, so that crate's name says compatibility rather than
minting a brand. Starry OS does this: `starry-kernel` reads Linux's user-space interface from
`linux-raw-sys`, an existing crate, rather than defining one. A family that adopts an interface
mints no `target_os`, no `std` port and no ABI brand. Its own names fall to the kernel and the
distribution, the cheap layers.

Second, the capability family might gain a Linux personality: a user-space program that serves
Linux's calls on top of the capability kernel, as Starnix does on Zircon. A personality changes
neither kernel's interface. Under (a) it is a component of the library, not a family, and takes a
component's name.

So (a) reads in full: a family shares one binary interface, either one the project defines or one
it adopts. (b) fits a compatibility kernel badly, since a Linux-compatible kernel that called
itself its own platform would refuse the programs it exists to run.

Licensing, stated as a fact and not as legal advice: implementing Linux's system-call interface
does not bring in the GPL, and taking Linux code would. gVisor (Apache-2.0), Asterinas (MPL-2.0)
and Starry OS (Apache-2.0) each implement that interface under a license other than the GPL.
FreeBSD's Linux layer, `sys/compat/linux`, carries BSD-2-Clause and BSD-3-Clause headers, so its
code can come in under its own license, as a dependency ruling under §46 (thin primitives or whole
subsystems). The appendix
[compatibility kernels](0323-what-a-kernel-family-is-and-the-names-of-the-layers/compatibility-kernels.md)
holds the prior art for both consequences.

### Prior art

Read for this file unless marked otherwise.

| system | what a program targets | what it shows |
|---|---|---|
| Fuchsia and Zircon | the Fuchsia System Interface, with API levels and an ABI revision (RFC 0002, platform versioning); the vDSO belongs to it | Fuchsia defines the platform apart from the kernel, and Zircon is "the core platform that powers Fuchsia"; the interface is the promise, which is (a) |
| Genode | one component API across kernels | runs on NOVA, seL4, Fiasco.OC, OKL4, L4Ka::Pistachio, Linux and its own `base-hw`; since release 16.08 one dynamically linked binary runs on all of them, which is (a) over kernels that are (b) |
| the L4 family | each kernel's own interface | the name covers "the L4 kernel interface and its different versions" (Wikipedia); V2, X.2 and seL4's interface diverged, and programs do not move between them, which is (b) |
| Darwin and XNU | `macos`, `ios` and other `target_os` values, all on XNU | XNU is "the kernel of the Darwin operating system for use in macOS and iOS" (its README); Rust names the platform a program sees, not the kernel |
| Rust's configuration keys | one `target_os` per target; `target_family` may repeat (`unix`, `wasm`) | the Reference calls `target_family` "a more generic description of a target, such as the family of the operating systems"; a custom target can set it |
| WASI | one interface specification | Wasmtime, WAMR, WasmEdge, wazero and Wasmer implement it, and a program for the interface runs on any of them, which is (a) |
| Redox | `target_os = "redox"`, ABI crate `redox_syscall` (library `syscall`) | one kernel, and the ABI crate names the system |
| Hermit | `target_os = "hermit"`, ABI crate `hermit-abi` | the project renamed itself twice (HermitCore, RustyHermit, Hermit); `hermit-sys` stays on crates.io, marked as renamed; Rust removed the `hermitkernel` targets in rust-lang/rust#95299 |
| Windows 95 and Windows NT | the Win32 API | from memory, not read: two kernel designs ran the same Win32 programs, and Microsoft then retired the 9x line rather than keep two kernels |
| the BSDs | `freebsd`, `netbsd`, `openbsd`, `dragonfly` | "BSD" names a shared design and history; their binaries do not run on each other without a compatibility layer, which is (b) (from memory) |

The pattern: where a project promised an interface (Fuchsia, Genode, WASI, Win32), several
implementations sat under it. Where kernels shared only a design (L4, the BSDs), the family name
stayed a word, and each kernel became its own platform.

### Recommendation, and why

(a), a family shares a binary interface. It makes the stricter promise, and the asymmetry decides
it. A family defined by its ABI can drop the promise later. If the capability monokernel needs an
interface the microkernel will never serve, it leaves the family and takes its own `target_os`,
and nothing built for the family breaks. A family defined by design cannot add the promise later.
Once two kernels each ship numbers the other lacks, programs exist that run on one and not the
other, and no ruling makes them compatible again.

(a) also puts the expensive names in the fewest places. The in-binary names, the `target_os` and
the published ABI crate exist once per family rather than once per kernel.

Would this still be the choice if both cost the same to build? Yes. (a) costs more work, since two
kernels must keep serving one interface. The recommendation does not rest on effort.

On (b), every kernel becomes a platform. The family names in the appendix become prose only, the
kernel names take on everything listed there for families, and kernel names move to the front of
the order below.

## The order of the later rulings

Hardest to change first. The maintainer brings each as its own ruling, one at a time.

1. What a kernel family is, the question above. It decides which layer carries `target_os`, the
   ABI crate and the in-binary names.
2. The capability family's name on (a), or each kernel's on (b). It turns one-way at the first
   crates.io publish and at the first upstream Rust merge, and the published name of `crates/abi`
   waits on it.
3. The organization and the domain. GitHub redirects most of a rename, every image names the
   domain, and anyone may claim an organization's old name. This ruling includes whether `nifeos`
   stays.
4. The library, if its name differs from the organization's, and with it the published names of
   the shared page-contract crates.
5. Distributions, starting with whether `basalt` stays and which family it belongs to.
6. Individual kernels on (a): repositories and images only, the cheapest names here.

## What waits on this answer

- The crates.io name of `crates/abi`, decision A of milestone 1075 (`abi`: what an outside
  maintainer would fix before the move). Discussed, not ruled. `capability_kernel_abi` reads as
  too generic, since seL4, Zircon, Hubris and Genode all have capability kernels with ABIs. Each
  published ABI crate names its system (`sel4-sys`, `fuchsia-zircon-sys`, `hermit-abi`, `wasi`,
  `r-efi`, `redox_syscall`). Both capability kernels would share this one. It has to publish,
  because a crates.io crate cannot depend on a git crate, so a `getrandom` backend needs it. It
  becomes the bottom of the SDK. A rename in code wins over a package-only rename (1,784 uses in
  238 files, none in basalt). It waits on rulings 1 and 2.
- The published names of the 25 page-contract crates (`*_contract`), which every family may share.
  They wait on rulings 1 and 4.
- The kernel's own repository and name. Milestone 1039 (the kernel leaves nife) names it
  `nifeos/nife-kernel`. With a second capability kernel under (a), that name may need to say
  which. It waits on rulings 1, 2 and 6.
- The website's domain, from milestone 1018 (the design record moves to `nifeos/record` and is
  published as a website), and the rule that a leaving crate never mentions nife. That rule means
  something different if `nife` comes to name one family. Both wait on rulings 1, 2 and 3.

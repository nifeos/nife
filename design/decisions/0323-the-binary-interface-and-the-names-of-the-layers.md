---
status: PROPOSED
raised: 2026-11-12
---

# 323. The binary interface, and the names of the project's layers

*The section number, the file name and this title are provisional until the merge lands them.*

The maintainer raised this on 2026-11-12 (UTC), on the intent that the crates are the product: a
library of components from which the project builds several operating systems. Counts come from
`nifeos/nife@a3c7d4647` and `nifeos/basalt@ecdfee7`, and each gives its command so a reviewer can
take it again at merge. The architect ruled question 1, what the layer above a kernel is, and ruling
2, the capability binary interface's name, on 2026-11-14 (UTC), and this file states both rules.
Rulings 3 to 6 are still open, and this file lays out the options for each. The appendix, [the five
layers](0323-the-binary-interface-and-the-names-of-the-layers/the-five-layers.md), holds the
inventory, the reversibility of each layer and every candidate name with its checks.

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
3. binary interfaces;
4. individual kernels;
5. distributions (today one, `basalt`: a manifest that pins each component at a commit, and the
   workflow that builds and tests the image).

One question came before any name, because it decides which names exist and what each touches:
what the layer above a kernel is. The rule follows.

## Is the premise true

Mostly. One part of it meets an existing decision.

- The tree builds one kernel for one platform. `targets/` holds three target specifications, all
  with `"os": "nife"` and none with a `target-family`. The tree has one ABI crate (`crates/abi`,
  `publish = false`) and one `std` port (`patches/std-nife`).
- The word "family" appears on 45 lines in 29 files of `design/decisions/` and `design/naming.md`
  (`git grep -c -iw family`), never as a defined term for a set of kernels. The project does not
  use it for one.
- §10 (process model: capability-based, microkernel) and §82 (ambient authority is the problem;
  replacing the ecosystem, not confining it, is the end state) make ambient authority the problem
  this project exists to solve. Two of the four planned kernels have it. One library under both
  does not contradict §82 by itself, since §82 speaks of one system's end state. A record that
  names an ambient-authority binary interface should still say how it stands beside §82. This file
  does not decide that.
- Milestone 1039 (the kernel leaves nife) moves the kernel to `nifeos/nife-kernel`, a name ruled
  2026-11-03 (UTC). That repository does not exist yet.

## The rule: the layer above a kernel is its binary interface

Ruled 2026-11-14 (UTC).

- The layer above a kernel is its **binary interface**: the system calls, object methods, error
  codes, right bits and version gate a program builds against. A binary interface is one the
  project defines, or one it adopts, such as Linux's.
- Every kernel that serves a binary interface serves all of it, so a program built for that binary
  interface runs on any of those kernels without a rebuild.
- The binary interface carries the operating-system name (Rust `target_os`), the ABI crate, the
  `std` port, the SDK crates and the names inside binaries.
- A kernel's own name touches only its repository, its images and its boot log.
- The project does not use the word "family" for a set of kernels.
- nife already uses "interface" for its wire contracts, each with an interface number. The record
  therefore always writes "binary interface", never "interface" alone, for this layer.

### Background

A program here targets three things. The compiler builds it for `target_os = "nife"`, it links the
`std` port, and it calls the kernel through the numbers in `crates/abi`. At spawn, the kernel
compares the program's `ABI_VERSION` with its own and refuses a mismatch.

Some bytes in every image also spell the project's name. Every program's capability manifest has
the ELF note owner `b"nife"` (`crates/capability_manifest_note`, ratified 2026-09-26). The boot
slot header reads `NIFESLOT` and the boot stack magic `NIFESTK1`. Packages travel as `.nifepkg`
files. §45 (a nife partition is `EC5CC08B-D749-4434-AC38-A274C50385BA`, and that never changes)
fixes the partition type.

With several kernels, each of these belongs either to every kernel that serves one binary
interface or to one kernel. The rule puts them all at the binary interface.

### What the rule costs and what it gains

The binary interface is the platform a developer targets. A program cannot tell which of its
kernels runs it, just as it cannot tell which build of Zircon runs it.

The cost: kernels that serve one binary interface move together. The capability monokernel serves
the same syscall numbers, object methods, error codes and right bits as the microkernel. A new
number goes into both kernels or neither, and one version gate binds them. A kernel that wants a
private syscall takes a binary interface of its own. The kernels stay free in everything a program
cannot observe: where a service runs, how the scheduler works, what the proofs cover.

The gain: one `target_os`, one published ABI crate and one `std` port per binary interface. A
third-party author writes a `getrandom` backend or a `libc` port once per binary interface. A
distribution built for a binary interface runs on any kernel that serves it. Kernel names touch
only repositories and images, the cheapest names in the project.

### A binary interface the project defines, or one it adopts

The project is also exploring compatibility kernels: whether the components, with other code, could
build a Linux-compatible kernel, a BSD-compatible kernel or both. Two consequences follow from the
rule.

First, an ambient-authority kernel might adopt Linux's binary interface instead of the project
defining one. Its programs then target `linux`, and its ABI crate describes Linux's system calls,
so that crate's name says compatibility rather than minting a brand. Starry OS does this:
`starry-kernel` reads Linux's user-space definitions from `linux-raw-sys`, an existing crate,
rather than defining its own. An adopted binary interface mints no `target_os`, no `std` port and
no ABI brand. The names of a kernel that serves one fall to the kernel and the distribution, the
cheap layers.

Second, the capability kernels might gain a Linux personality: a user-space program that serves
Linux's calls on top of a capability kernel, as Starnix does on Zircon. A personality changes no
kernel's binary interface. It is a component of the library, and takes a component's name.

Licensing, stated as a fact and not as legal advice: implementing Linux's binary interface does not
bring in the GPL, and taking Linux code would. gVisor (Apache-2.0), Asterinas (MPL-2.0) and Starry
OS (Apache-2.0) each implement it under a license other than the GPL. FreeBSD's Linux layer,
`sys/compat/linux`, carries BSD-2-Clause and BSD-3-Clause headers, so its code can come in under its
own license, as a dependency ruling under §46 (thin primitives or whole subsystems). The appendix
[compatibility
kernels](0323-the-binary-interface-and-the-names-of-the-layers/compatibility-kernels.md) holds the
prior art for both consequences.

### The alternative not taken

The other reading treated the layer above a kernel as a shared design only, the sense in which
L4Ka::Pistachio, Fiasco and seL4 belong to the L4 family and the BSDs form one, though none runs
the programs of another. Each kernel would then be its own platform, with its own `target_os`, ABI
crate and `std` port, and every kernel name would cost what a binary interface's name costs. Those
outside systems call such a set a family. This project does not use the term.

### Prior art

Read for this file unless marked otherwise.

| system | what a program targets | what it shows |
|---|---|---|
| Fuchsia and Zircon | the Fuchsia System Interface, with API levels and an ABI revision (RFC 0002, platform versioning); the vDSO belongs to it | Fuchsia defines the platform apart from the kernel, and Zircon is "the core platform that powers Fuchsia"; the binary interface is the promise |
| Genode | one component API across kernels | runs on NOVA, seL4, Fiasco.OC, OKL4, L4Ka::Pistachio, Linux and its own `base-hw`; since release 16.08 one dynamically linked binary runs on all of them, one binary interface over kernels that share no design |
| the L4 kernels | each kernel's own binary interface | Wikipedia's L4 family covers "the L4 kernel interface and its different versions"; V2, X.2 and seL4's diverged, and programs do not move between them: a shared design, not a shared binary interface |
| Darwin and XNU | `macos`, `ios` and other `target_os` values, all on XNU | XNU is "the kernel of the Darwin operating system for use in macOS and iOS" (its README); Rust names the platform a program sees, not the kernel |
| Rust's configuration keys | one `target_os` per target; `target_family` may repeat (`unix`, `wasm`) | the Reference calls `target_family` "a more generic description of a target, such as the family of the operating systems"; a custom target can set it |
| WASI | one specification | Wasmtime, WAMR, WasmEdge, wazero and Wasmer implement it, and a program built for it runs on any of them: one binary interface, several implementations |
| Redox | `target_os = "redox"`, ABI crate `redox_syscall` (library `syscall`) | one kernel, and the ABI crate names the system |
| Hermit | `target_os = "hermit"`, ABI crate `hermit-abi` | the project renamed itself twice (HermitCore, RustyHermit, Hermit); `hermit-sys` stays on crates.io, marked as renamed; Rust removed the `hermitkernel` targets in rust-lang/rust#95299 |
| Windows 95 and Windows NT | the Win32 API | from memory, not read: two kernel designs ran the same Win32 programs, and Microsoft then retired the 9x line rather than keep two kernels |
| the BSDs | `freebsd`, `netbsd`, `openbsd`, `dragonfly` | "BSD" names a shared design and history; their binaries do not run on each other without a compatibility layer (from memory) |

The pattern: where a project promised a binary interface (Fuchsia, Genode, WASI, Win32), several
implementations sat under it. Where kernels shared only a design (L4, the BSDs), each kernel became
its own platform.

### Why

The binary interface is the stricter promise, and the asymmetry decides it. The project can drop the
promise later: if the capability monokernel needs calls the microkernel will never serve, it takes a
binary interface of its own and its own `target_os`, and nothing built for the first one breaks. It
cannot add the promise later: once two kernels each ship numbers the other lacks, programs exist
that run on one and not the other, and no ruling makes them compatible again.

The rule also puts the expensive names in the fewest places. The in-binary names, the `target_os`
and the published ABI crate exist once per binary interface rather than once per kernel. It costs
more work, since two kernels must keep serving one binary interface, so it does not rest on effort.

## The capability binary interface is Moho

Ruled 2026-11-14 (UTC).

- The capability binary interface is named Moho, the boundary between the Earth's crust and its
  mantle (the Mohorovičić discontinuity).
- Its published ABI crate is `moho_abi`, and the name in code matches the package name, as it does
  for `byte_glob`.
- `nife` stays the name of today's kernel, the capability microkernel.
- The ambient-authority binary interface takes its name in a later ruling.

The checks, run 2026-11-14 (UTC). On crates.io, `moho_abi`, `moho-abi`, `moho_sys`, `moho-sys`,
`moho_sdk` and `moho-sdk` are free, and the bare `moho` is a game library abandoned in 2017. On
GitHub, the user `moho` exists and holds no repositories. `moho.org` is registered and offered for
sale, and another party holds `moho.dev`. Moho, the animation software, is well known. Two web
searches found no operating system, kernel, ABI or Rust target named Moho. Nobody has yet checked
the rustc target list, LLVM or the Linux sources.

What moves, and when. The tree still spells the capability binary interface `nife` in
`target_os = "nife"`, the `unknown-nife` triples, the `std-nife` port and the `abi` crate
(`crates/abi`). Renaming them waits on a separate plan lane, which sets their order and the rename
of `crates/abi` to `moho_abi`. The names inside binaries stay as they are: the ELF note owner `nife`, section names
such as `.note.nife.manifest`, and magics such as `NIFESLOT` and `NIFESTK1`. They fall under the
wire-literal exemption, ruled 2026-11-14 (UTC): a marked owner string, section name or magic may
spell nife.

## The order of the later rulings

Hardest to change first. The maintainer brings each as its own ruling, one at a time.

1. What the layer above a kernel is. Ruled 2026-11-14 (UTC): its binary interface, which carries
   `target_os`, the ABI crate and the in-binary names.
2. The capability binary interface's name. Ruled 2026-11-14 (UTC): Moho, with the ABI crate
   `moho_abi`, and `nife` stays the capability microkernel's name. The ambient-authority binary
   interface's name stays open.
3. The organization and the domain. GitHub redirects most of a rename, every image names the
   domain, and anyone may claim an organization's old name. This ruling includes whether `nifeos`
   stays.
4. The library, if its name differs from the organization's, and with it the published names of
   the shared page-contract crates.
5. Distributions, starting with whether `basalt` stays and which binary interface it targets.
6. Individual kernels: repositories, images and boot logs only, the cheapest names here.

## What waits on these rulings

- The rename of `crates/abi` to `moho_abi`, decision A of milestone 1075 (`abi`: what an outside
  maintainer would fix before the move), with `target_os`, the triples and the `std` port. Ruling 2
  settles the name, and the rename in code (1,784 uses in 238 files, none in basalt) waits on the
  plan lane above. The crate has to publish, because a crates.io crate cannot depend on a git
  crate, so a `getrandom` backend needs it. Both capability kernels share it.
- The published names of the 25 page-contract crates (`*_contract`), which every binary interface
  may share. They wait on ruling 4.
- The kernel's own repository. Milestone 1039 (the kernel leaves nife) names it
  `nifeos/nife-kernel`. With a second capability kernel, that name may need to say which. It waits
  on ruling 6.
- The website's domain, from milestone 1018 (the design record moves to `nifeos/record` and is
  published as a website), and the rule that a leaving crate never mentions nife (now the name of
  one kernel). Both wait on ruling 3.

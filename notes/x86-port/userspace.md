# The x86_64 port: the userspace and its archive

*An appendix to [`notes/x86-port.md`](../x86-port.md), which is the page to read. This file holds
how the archive reaches the machine, the suite's skips, and the bugs userspace found. It exists to
verify or challenge the main page. A reader who only needs to build, boot or test the x86_64 port
should not have to open it. The directory `notes/x86-port/` and this file's stem are provisional
names; naming is calef's.*

*Records cited below: §121 (a device with no page), milestone 150 (adding a program), milestone 162
(real hardware entropy) and milestone 184 (extend the `std` port).*

## The userspace, and how an archive reaches a machine with no device tree

Item 4's hand-off, 2026-08-24. Five pieces, and only one of them was interesting.

The interesting one, `user_mode_runtime`, has its own appendix: [user-mode-runtime.md](user-mode-runtime.md).
What the userspace still lacks is on the [main page](../x86-port.md), under BUGS.

### Four programs needed an arm, and three of them refuse rather than pretend

`os_primitives_benchmarker` hand-assembles a child stub, which had to become bytes rather than
words because x86 instructions are not a fixed width. One `copy_nonoverlapping` over
`size_of_val` now serves all three element types.

`console::uart_put`, `input`'s `uart` module and `swap_protocol::probe_device` cannot reach a device
from ring 3 at all. Their x86 arms `trap()` (or return a sentinel that nothing observes) rather than
no-op, and that is the whole point. A silent no-op compiles into a console that acknowledges every
byte and prints none. That is a lie told in the one place an operator is looking. Those programs
are packed into the archive anyway, because an archive entry costs a directory slot and nothing
spawns a program by accident. The tests that would spawn them skip with the reason.

### The archive arrives as a PVH module, which is the device tree's `/chosen` one machine over

`hvm_start_info` carries `nr_modules` and `modlist_paddr`, and QEMU's PVH loader turns `-initrd
FILE` into exactly one entry there. `machine_discovery::x86_64::module` decodes it (host-tested,
three tests, including the 32-byte stride that a one-module list cannot catch), and
`arch::x86_64::machine::initial_ramdisk` reads entry zero through the direct map. The x86 front end then does
what `memory::init` does with the tree's answer. It adds the region to the `forbidden` slice so the
allocator cannot hand out the archive it is about to read, and calls `memory::record_initial_ramdisk`. The
evidence it works is arithmetic rather than a print: free memory drops from 254 MiB to 249 with a
4.4 MB archive attached.

The first module is taken and any others ignored. PVH permits several; this kernel wants one;
inventing a policy for a case the machine never produces would be code nobody could test.

### `xtask` grew a third packer, and one latent bug came out with it

There is no table of programs to pack. All three packers, `initial_ramdisk_x86` among them, pack
every `[[bin]]` in `components/Cargo.toml` and `fixtures/Cargo.toml` (`declared_programs()` in
`xtask`, milestone 150), so no list can drift between callers. See notes/adding-a-program.md.

`read_stripped`'s cache tag is the latent bug. It namespaces stripped copies by the target directory
a binary came from, and an `x86_64-unknown-none` path fell through to `"host"`, sharing a filename
with every aarch64 build of the same program. Nothing had noticed because nothing ever asked for
both. The moment an x86 archive is packed in the same run as an aarch64 one, whichever ran second
would read the other's bytes back out of `target/stripped` and measure them. It would be a real digest of a
real program, and the wrong one, in a trust root. The check is anchored on the full triple rather
than a substring, because a host path on an x86 CI runner contains `x86_64-unknown-linux-gnu`.

### The suite: 170 pass, 67 skip

Every skip prints why. Grouped by cause, largest first:

| Count | Reason | What would close it |
|---|---|---|
| 21 | no `fs_server` in the archive | it does not compile for this target; see below |
| 13 | no RTC binding | the discovery seam's wide half (item 0), or §121 for the CMOS ports |
| 11 | no virtio-rng on either bus | the runner attaches none, and PCI is not enumerated here |
| 5 | the console UART is in the I/O port space | DECISIONS §121 |
| 4 | one core online / no core roster | SMP, item 5 |
| 4 | no PCI bus enumerated, so no GPU, keyboard or NVMe | item 0 again, and the runner |
| 1 | no `std_exerciser` | an `x86_64-unknown-nife` target and a `std` farm. Closed by milestone 184: the test now passes on this port |
| 1 | no `mkfs` | `fs_server`'s cause, one binary over |
| 1 | address spaces are not tagged | `CR4.PCIDE`, which is calef's call (item 3) |
| 1 | `hvm_start_info` is not a device tree | nothing; it is a true statement about this machine |
| 1 | no instruction-mode entropy source on this build | milestone 162 |

The `fs_server` group is the one that is not about this machine. It is a toolchain failure and
it is worth stating exactly, because the next person to try will otherwise spend an afternoon on it.
`fs_server` links the vendored RedoxFS engine, which depends on the `aes` crate unconditionally
(its encrypted-volume support is not behind a feature), and building `aes` for
`x86_64-unknown-none` ends in

```
rustc-LLVM ERROR: Do not know how to split the result of this operator!
```

at every optimization level, zero included. The cause is the target spec rather than the crate:
`x86_64-unknown-none` is `-mmx,-sse,+soft-float`, so LLVM has no 128-bit vector register to legalize
an AES block into and no scalar fallback for that operator. Nothing on this side fixes it. The two
routes out are a RedoxFS built without its crypto (a patch against a vendored crate, which
`patches/README.md` is the place for) or an x86 userspace target that keeps SSE. Both are their own
work, and until one of them happens x86 has no filesystem, which also means there is no point
attaching a disk to the runner.

### Four bugs userspace found, and every one had been latent since the day it was written

None of these could have been caught before there were user programs on this architecture, which is
the argument for doing item 4's hand-off rather than deferring it.

1. `arch::x86_64::irq::enable` conflated two numbering schemes. An intid on x86 is either a
   legacy IRQ (0..15, which needs the MADT's override table and a redirection entry) or a local
   APIC vector (0x20..0x2f, raised by writing the ICR, with no controller input to unmask).
   `enable` assumed the first, always. `spawn_hello` enables `user::INIT_TEST_SGI`, which on this
   architecture *is* `SELF_TEST_VECTOR` = 0x22 = 34, and the kernel panicked with
   `gsi 34 is outside the IO APIC's range`. The fix is three lines and the ranges were already
   documented as disjoint in `GSI_VECTOR_BASE`'s own doc comment. Nothing above the arch layer had
   ever called `enable` before.

2. `user_mode_runtime::trap()` cannot use `int3` from ring 3. A *software* interrupt is refused unless the
   IDT gate's DPL admits the caller's privilege, and every gate here is DPL 0. So `int3` from a
   process raises #GP with error code 0x1a (`(3 << 3) | 2`: the vector it was refused, tagged as
   an IDT selector) rather than #BP. The process dies either way, so `int3` looks like it works,
   but what it reports is a general protection fault at address zero, naming neither the
   instruction nor the reason. It is `ud2`: a fault the CPU raises on a permanently invalid
   opcode, so no gate DPL is involved. Opening vector 3 to ring 3 is what Linux does, and Linux has
   ptrace to justify it. This kernel has no debugger, so that would widen what a process may do and
   buy nothing.

3. `user/link.ld` did not name `.got`, and x86 emits one. An unnamed orphan section is appended
   after the last section the script mentions, which was `.bss`. `.got` has file contents and `.bss`
   does not. So the `data` segment's `p_filesz` stretched past `.bss` to cover it, `p_memsz ==
   p_filesz`, and the segment had no zero-fill tail at all. The loader had nothing to zero and
   `.bss` was whatever the file happened to hold. Found by `the_initial_ramdisk_holds_a_native_executable`,
   which asserts `memsz > filesz` precisely so the zero-fill test below it is not vacuous. It shows
   up here and not on the other two because all three targets use the static relocation model and
   none of these programs is position-independent. But LLVM's x86-64 backend still routes a few
   references through a GOT entry where the aarch64 and RISC-V backends produce none.

4. `fs_service::blk_server_image()`'s x86 arm was a `panic!`. It read "x86_64 builds no user
   programs at all yet", which was true when it was written and stopped being true the same day the
   archive existed. It panicked *before* any disk check, so a dozen test files that already degrade
   gracefully on a machine with no disk aborted the suite instead.

### `cfg(initial_ramdisk)`, and the prediction it made

Thirty test modules under `kernel/src/user/` were gated `#[cfg(all(test, initrd))]`, a cfg
`kernel/build.rs` emits for every target that has user programs to pack. They are portable in every
respect except that their fixture is a real ELF binary read out of the initrd archive.

`#[cfg(not(target_arch = "x86_64"))]` would have said the same thing thirty times and been wrong the
day this port could build user programs, in thirty places nobody would think to look. `cfg(initial_ramdisk)`
reads as what it means, and unblocking it was one match arm in the build script, with nothing
else edited. Every one of those modules came back at once. That is the prediction the cfg was
written to make, and it held. It is recorded here as well as in the build script because the
alternative spelling would have needed thirty edits and would have been found by whoever hit the
thirty-first.

Six modules under `user/` ran here before that: `force_kill_tests`, `pmap_tests`, `reap_tests`,
`supervision_tests`, `survey_tests` and `thread_leak_police`. Their fixtures are hand-assembled
programs rather than ELFs, and those live in `kernel/src/user/x86_programs.rs` rather than in a
`#[cfg(test)]` module, because the boot tour needs the same four programs. They stay: a fixture that
needs no initrd is what lets the userspace demo run on a `cargo run` with no `-initrd` at all.

---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1112. `user_mode_runtime`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/user_mode_runtime` at the head of `main`, read the way a
maintainer meeting it for the first time would read it. It changes nothing; it lists what the next
lanes change.

The crate has no review package in `notes/crate-reviews/`, so there is no settled baseline to
inherit. The review treats as settled only what a ruling elsewhere already fixes: the crate name,
`VERSION` and `INTERFACE_ID` (the names every contract crate carries), `INTERFACE_ID = 20` for the
entry registers and link layout, the ratified function names the `Name:` blocks record, and the
order of extraction in milestone 1017 (the contract crates leave in this order), which places this
crate in step 3, after `abi`, `address_space_map`, `current_cpu_contract` and the five contract
crates it links. Nothing here reopens those. Where a later rule contradicts a recorded ruling, the
item says so.

Two pull requests in flight touch `src/socket_frame.rs`: #3517 (a send's length rides in the `CALL`
argument word) and the ruling that `ACCEPT`'s second reply word carries the peer. Both are read
against the items below; neither changes a line of code in this crate, only documentation and one
example.

Reuse: the template `nifeos/crate-workflows` and its scaffold, `script/crate-standalone`,
`script/crate-move --dry-run`, the template's name and prose gates, cargo-llvm-cov, cargo-mutants.
No new tool.

## Goal

A stranger who finds `user_mode_runtime` in its own repository can tell in one screen what it is
for. It holds the one syscall wrapper, the entry macro, the panic handler and the typed windows over
memory the kernel handed a process. It covers three architectures. The same stranger can build it for all three
targets and run its host tests from the README alone. The stranger can also trust that every safe
function is safe to call with any argument. A wrong argument panics or meets a refusal from the
kernel, and it never reads or writes memory the type did not promise. The repository's required checks are green for the
crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1112-user-mode-runtime-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1112-user-mode-runtime-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two, L is a week. A door is
one-way when somebody outside the repository acts on it after the release (a public name, a
published behavior, a layout, a wire value, a dependency); everything else is two-way and the
maintainer decides it. The crate has no release, so every public name is two-way until the first tag
and one-way after it.

### P0: blocks the move or any publish

1. `MappedWindow::read<T: Copy>` and `write<T>` let safe code read or write any type at any
   offset.
   Finding: `read` is `pub fn read<T: Copy>(&self, off: u64) -> T` over `ptr::read_volatile`.
   The only check is the byte range. `T` may be `&'static u8`, `bool`, `char`, a fieldless enum, a
   function pointer or a `NonZeroU64`. A window holding the bytes a device or peer wrote then
   produces a value with an invalid bit pattern from a safe call, which is undefined behavior. The
   same call with `T = u64` at `off = 1` is an unaligned volatile read, which `read_volatile`
   documents as undefined behavior; the `r64` doc says "the caller keeps the offset 8-aligned",
   which a safe function cannot ask. `write<T>` carries no bound at all. Every caller in the tree
   passes `u8`, `u16`, `u32` or `u64`, so no current program is wrong. The hole is in the claim
   the signature makes. An outside reviewer of an `unsafe`-heavy crate looks for it first.
   Evidence: `src/mapped_window.rs` lines 83 to 97 (`read`, `write`), 123 to 132 (`r64`, `w64`),
   75 to 81 (`check`).
   Change: a sealed trait for the integers the window carries (`u8`, `u16`, `u32`, `u64`, and
   their signed twins if a caller wants them), with a private supertrait so no one outside the
   crate implements it; `read<T: Word>` and `write<T: Word>`. Extend `check` with an alignment
   check: `(self.base + off) % align_of::<T>() == 0`, panicking with the offset and alignment
   named, as the bounds panic does. Prior art: `tock-registers` seals `UIntLike`; `zerocopy` and
   `bytemuck` express the same promise as `FromBytes` and `Pod`; `volatile-register` and `vcell`
   avoid the question by reading through `&self` on a typed place. Concrete example: today
   `WINDOW.read::<&u8>(0)` compiles and is undefined behavior. After the change it does not
   compile, and `WINDOW.r64(1)` panics instead of causing undefined behavior.
   Door: two-way now (every caller already uses an integer type); one-way after the first tag,
   because narrowing a generic bound is a breaking change. Effort: S.

2. `doorbell_barrier` and `virtio_ring_barrier` tell the compiler they are not barriers on
   aarch64 and `x86_64`.
   Finding: the `dmb sy`, `dmb ish` and `mfence` arms carry `options(nomem)`. The Rust reference
   says of `nomem`: "The assembly code does not read from or write to any memory accessible
   outside of the assembly code. This allows the compiler to cache the values of modified global
   variables in registers across execution of the assembly code ... The compiler also assumes that
   the assembly code does not perform any kind of synchronization with other threads, e.g. via
   fences." So on two of three architectures the compiler may move an ordinary load or store
   across either barrier. The riscv64 arm omits `nomem` and its comment says why ("ordering memory
   is its job"); the two other arms contradict it inside the same function. No miscompile is
   known. Every current caller pairs the barrier with volatile accesses on both sides, and the
   bulk copies carry their own `compiler_fence`. A caller that writes through `as_mut_slice` and
   then rings a doorbell is the exposed shape, and the `SAFETY` comment ("cannot be unsound; it
   only constrains ordering") is false as written.
   Evidence: `src/mapped_window.rs` lines 253 to 266; `src/virtio.rs` lines 81 to 84.
   Change: delete `nomem` from the four arms, keeping `nostack` and `preserves_flags`, and say in
   each comment that the absence of `nomem` is the compiler half of the barrier. Prior art: the
   Linux `dmb()` and `mb()` macros carry a `"memory"` clobber, which is what an `asm!` block
   without `nomem` is in Rust. Concrete example: a driver that fills a descriptor through
   `as_mut_slice`, calls `doorbell_barrier`, then stores the tail register: with `nomem` the
   compiler may sink the descriptor stores past the `dmb`; without it the order holds.
   Door: two-way. Effort: XS.

3. The manifest and the files the standalone check refuses.
   Finding: `script/crate-standalone` fails the manifest and files steps. The crate inherits
   `license` and `lints` from the nife workspace. It has no `rust-version`, no README, no
   `LICENSE-MIT` and no `LICENSE-APACHE`. The `publish = false` line points at
   `crates/publish-allowlist.txt`, a file the crate's repository does not hold. The `[lints]` the
   template's `standalone` job requires (`unsafe_op_in_unsafe_fn`, `missing_docs`,
   `unexpected_cfgs` naming `cfg(kani)`) need literals in the manifest. The entry macro's
   `cfg(kani)` arms depend on the last one. The manifest
   has no `description`, `repository`, `keywords` or `categories`. `jig/Cargo.toml` depends on the
   crate by path with no `version` key, which the move's `rev` form needs.
   Evidence: `Cargo.toml` lines 5 and 6, 16 and 17; the standalone check's output; the scaffold's
   `README-crate.md`.
   Change: the move lane's ordinary work, which it does in nife before the move so the move commit
   stays mechanical. Write a literal `license` and a literal `[lints]`, and add `rust-version`.
   Add a README from the scaffold with the build lines for all three targets, and both license
   files. Reword the `publish` comment to one sentence a stranger can act on. Add `version` on
   jig's dependency line. The six path
   dependencies the check also reports are the extraction order of milestone 1017 doing its job,
   not a defect: `abi`, `clock_contract`, `counter_frequency_contract`, `current_cpu_contract`,
   `entropy_contract` and `socket_contract` leave first.
   Door: two-way (the crate name and the license are already ruled). Effort: S.

4. The crate has no test target, and the template's required checks assume one.
   Finding: `[lib] test = false, bench = false, doctest = false`, with the reason "aarch64-only
   inline asm, so no host test". The crate has `x86_64` arms for every instruction and builds on
   an `x86_64` Linux host (verified), and its aarch64 arms are valid on an aarch64 host. The
   template's `check / tests`, `check / coverage` (85 percent per file) and `check /
   mutation-diff` run on the host; a crate with no tests gives the first nothing to run, the
   second nothing to measure, and the third a suite that kills no mutant. The mutation-testing
   notes already record that shape for a crate that reaches this one. Meanwhile the `should_panic`
   example in `mapped_window.rs` and the two `ignore` examples in `process.rs` are never compiled,
   and milestone 1017 says the `link.ld` test now in `address_space_map` "moves to
   `user_mode_runtime`", which needs a test target to move into. Much of the crate is pure logic
   that runs anywhere: the window's bounds and alignment checks and its three copies, `Frame`'s
   accessors over a heap page, `Delivered::decode`'s four branches, the three hand-assembled
   stubs, and the nanosecond conversion.
   Evidence: `Cargo.toml` lines 8 to 14; `src/mapped_window.rs` lines 29 to 33; `src/process.rs`
   lines 88 to 92 and 114 to 126.
   Change: turn `test` and `doctest` on. Factor `monotonic_nanoseconds` into a pure
   `ticks_to_nanoseconds(ticks, hertz)` and test its overflow edge (the five-minute wrap the doc
   names). Test `MappedWindow` and `Frame` over a `#[repr(align(4096))]` heap page: bounds, the new
   alignment check, `sub`, `copy_within` with overlap, `copy_to_slice`, `copy_from_slice`. Test
   `Delivered::decode` on all four branches and `Received` on both. Decode each `movz`, `addi` and
   `mov r32, imm32` in `child_stub` back to the `abi` constant it should carry (item 7). Move the
   `link.ld` base test here. Gate everything that traps or reads a system register behind
   `cfg(target_os = "none")` or leave it untested and list those files in the coverage exemptions
   file the template reads, each with its reason. Turn the `ignore` examples into `no_run` so they
   compile. Reword the manifest comment.
   Door: two-way. Effort: M.

5. Eight build scripts in nife reach `link.ld` by a relative path into this crate's directory.
   Finding: `components`, `fixtures`, `jig`, `redoxfs_server`, `std_exerciser`,
   `cryptography_exerciser`, `pinned_tls_exerciser` and `mdr` each pass
   `-T<manifest dir>/../crates/user_mode_runtime/link.ld` to the linker, and
   `address_space_map`'s test does `include_str!("../../user_mode_runtime/link.ld")`. After the
   move the file is in a dependency's source directory, which a consumer's build script has no
   path to. The script is interface 20, "the entry registers and link layout a program starts
   with", so it belongs with the runtime; what is missing is a delivery mechanism.
   Evidence: `jig/build.rs` lines 9 and 10; `components/build.rs` line 21;
   `crates/address_space_map/src/lib.rs` line 358; the dry run's nife-side list.
   Change: a `build.rs` in this crate that copies `link.ld` into `OUT_DIR` and prints
   `cargo:rustc-link-search=<OUT_DIR>`, with `cargo:rerun-if-changed=link.ld`. A consumer then
   passes `-C link-arg=-Tlink.ld` with no path, from its build script or its `.cargo/config.toml`.
   Prior art: `cortex-m-rt` and `riscv-rt` ship `link.x` exactly this way, and every embedded Rust
   binary links with `-Tlink.x`. Concrete example: `jig/build.rs` loses its two path lines and
   gains `println!("cargo::rustc-link-arg=-Tlink.ld")`. The `address_space_map` test moves here
   (item 4) and reads the file by `include_str!("../link.ld")`.
   Door: the mechanism is what a program outside nife uses to link, so it is one-way after the
   first release; see the decisions section. Effort: M, counting the eight consumers.

### P2: later

The P2 items are in [the appendix](1112-user-mode-runtime-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1112-user-mode-runtime-outside-review/before-the-tag.md).

## Decisions for the architect

Only the one-way doors. Each is one decision, with the background first.

### Decision 1: the names a stranger meets

Background. The move rule asks for ratified names before a crate leaves, and 114 of this crate's
names lack ratification. The template's abbreviation gate will run in the new repository with a ceiling, and
today it reports 53 components. The naming rules this review applied are the ones in
`design/naming.md`: abbreviations spelled out, a quantity ending in its unit, the standard
library's spelling and meaning where it has one, and no generic word. A ratified name keeps its
spelling until its own review. The record shows none of the names below as ratified except where
noted.

Recommendation, one row per name, the stricter spelling first:

| today | recommended | why |
|---|---|---|
| `va` (three parameters) | `virtual_address` | the `[spelled_out]` list says so; the sibling contract crates already renamed theirs |
| `aspace`, `aspace_slot` | `address_space_slot` | abbreviation; `address_space_map_into` already spells it |
| `tcb_slot` | `thread_control_block_slot` | the function names already spell it |
| `user_sp` | `user_stack_pointer` | abbreviation |
| `nr` | `number` | abbreviation |
| `val` (three) | `value` | the list says so |
| `num` | `descriptors` | a count of things names the things |
| `x0`, `x1`, `x2` (parameters of `thread_control_block_start`) | `argument0`, `argument1`, `argument2` | register names of one architecture in a three-architecture signature |
| `copy_within(src, dst, len)` | `copy_within(source, destination, bytes)` | the list spells both; the standard library's `copy_within` takes a range and `dest`, so this is not its signature anyway |
| `set_dst`, `dst_ip`, `dst_port` | `set_destination`, `destination_address`, `destination_port` | the list spells `dst`; `ip` has no entry |
| `mapped_window::PAGE` | `PAGE_BYTES` | a quantity carries its unit; the page-crate rule spells it this way |
| `cntfrq`, `cntfrq_checked` | `counter_hertz`, `counter_hertz_checked` | a rate ends in `_hertz` (`tick_hertz` is the in-crate precedent); the ARM mnemonic names nothing on riscv64 or `x86_64` |
| `MappedWindow::sub` | `MappedWindow::window` | `sub` is the standard library's subtraction |
| `list` | `address_space_list` | a generic word; its twin is `survey`, which names its object |
| `r8` ... `w64` | keep, or `read_u8` ... `write_u64` | the architect's call; Linux spells `readb`/`writeq`, OpenBSD `bus_space_read_1`; `read<T>`/`write<T>` already exist as the long form |
| `virtio` | accepted word | a product name, like `pci` |

Prior art: `rustix` spells every parameter out. Its one exception, `fd`, is a name the kernel owns.
`cortex-m` spells `dmb`, `dsb`, `isb` because ARM owns them. This tree applies the same rule to
`cntfrq` on aarch64 and cannot apply it on the other two. Concrete example: a reader on
`x86_64` who sees `cntfrq()` searches for a register that does not exist; one who sees
`counter_hertz()` does not.

### Decision 2: how a program outside nife links against the layout

Background. `link.ld` is interface 20, the entry registers and link layout. Today eight build
scripts in nife reach it by a relative path into this crate's directory, and that path stops
existing at the move. A consumer of the released crate (the SDK archive milestone 607 describes, a
program built outside nife) needs a documented way to link with the script, and that way is a
published behavior from the first release on.

Recommendation: the embedded Rust mechanism. This crate's `build.rs` copies `link.ld` into
`OUT_DIR` and emits `cargo:rustc-link-search`; a program links with `-C link-arg=-Tlink.ld` and
no path. The script's base stays `0x60000000`, and the test that checks it against
`address_space_map::IMAGE_BASE` moves here. The alternative, each consumer vendoring a copy, is
what milestone 73 refused ("two copies would be two places to be wrong about the base") and the
move would make it eight copies.

Prior art: `cortex-m-rt` (`link.x`, 10.6 million downloads) and `riscv-rt` (`link.x`) do exactly
this, and every embedded Rust binary's `.cargo/config.toml` carries `-Tlink.x`. Concrete example:
`jig/build.rs` becomes one `println!("cargo::rustc-link-arg=-Tlink.ld")`.

### Decision 3: the receive surface before the first tag

Background. Ten `receive_*` functions project the same five kernel words ten ways, in three
return shapes. Every one is public, and after the first tag removing one is a breaking change.
The crate's own `Request` and `Delivered` types already carry everything a receive returns except
the sixth word (the death label) and the bound notification's word.

Recommendation: before the first tag, add one `Message` record (`w0`, `w1`, `w2`, `badge`, `label`,
`delivered: Delivered`) and one `Received` enum (`Message(Message)` or `Notification(u64)`). Build
both from one `receive_message(slot)` over `invoke6` and one `receive_capability_message(slot)`.
Keep `receive`, `call` and `receive_request` as the thin hot-path wrappers callers write most, and
mark the other seven for removal in the changelog. The stricter surface is the smaller one; loosening
later is compatible and tightening is not. If the architect prefers to ship the ten, the block
records that the surface is provisional in the README and the first tag is a pre-release, which
the crate-tag ruling already makes it.

Prior art: seL4's `seL4_Recv` returns the badge through one out-parameter on every receive and
has no `_badged` twin; Zircon's `zx_channel_read` returns bytes and handles from one call.
Concrete example: `receive_capability_bound` has no caller in the tree today and would still be a
published function after the tag.

## Done when

The checks are in [the appendix](1112-user-mode-runtime-outside-review/done-when.md).

## BUGS

- Item 6's premise, firmware that leaves `CNTFRQ_EL0` at zero, is from reading, not from a board.
  The fix costs one compare and is worth landing without one.
- Item 14 depends on a kernel property (every object type refuses an unknown method with
  `BadMethod`) that this review did not verify for every type and that no host test can.
- Item 4 says what the coverage and mutation jobs will do with a crate that has no test target
  from reading the template; the review did not run them.
- Two of the dry run's findings are against the template, not the crate: its scaffold files
  mention nife on ten lines, and its pull request template names the architect. Until the
  template fixes them, every move's dry run reports them, and the move lane for this crate cannot
  clear them from inside the crate.
- The template's spelled-out-word and unit-symbol gates are not in release v7, which the dry run
  pins. Item 13's counts come from running the helpers at the template's head; the ceiling they
  enforce arrives with the release that carries them.
- The review ran no mutation census (outside the window) and found none recorded; the crate has
  no row in the mutation baseline.

## Index row

An outside review of `user_mode_runtime` before it leaves. It finds two soundness fixes (a generic
window read safe code can misuse, two barriers declared `nomem`). It finds three move blockers (the
manifest and files, a test target the template's checks need, the link script's delivery). It adds
six items before the first tag and six for later. Three decisions go to the architect: the names,
the link mechanism, and the receive surface.

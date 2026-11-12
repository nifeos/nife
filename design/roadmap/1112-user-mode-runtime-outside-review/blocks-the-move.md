# P0: blocks the move or any publish

An appendix to [the milestone](../1112-user-mode-runtime-outside-review.md).

1. `MappedWindow::read<T: Copy>` and `write<T>` let safe code read or write any type at any
   offset.
   Finding: `read` is `pub fn read<T: Copy>(&self, off: u64) -> T` over `ptr::read_volatile`.
   The only check is the byte range. `T` may be `&'static u8`, `bool`, `char`, a fieldless enum, a
   function pointer or a `NonZeroU64`. A window holding the bytes a device or peer wrote then
   produces a value with an invalid bit pattern from a safe call, which has undefined behavior. The
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
   `WINDOW.read::<&u8>(0)` compiles and has undefined behavior. After the change it does not
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
   is its job"); the two other arms contradict it inside the same function. No one knows of a
   miscompile. Every current caller pairs the barrier with volatile accesses on both sides, and the
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

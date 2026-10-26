# The rules that hold the codebase together, and what each one buys

*Appendix to [`AGENTS.md`](../../AGENTS.md), which carries the seven rules themselves. This file
carries the reason behind each: the Raspberry Pi port that becomes a directory rather than a diff,
and the driver that knows only a base address. Also what §46 (thin primitives or whole subsystems)
means by write it or vendor it, and why a `#[path]` module shared by two binaries is checked by
nothing. Moved here 2026-09-23 (UTC) on calef's authorization, unchanged in substance.*


These come from `design/decisions/`. They are cheap to follow and expensive to retrofit.

1. All architecture-specific code lives under `kernel/src/arch/`. Assembly, `asm!`, system
   registers, CPU-specific behavior. If you're writing `asm!` outside `arch/`, that is the bug.
   This is what makes the Raspberry Pi port a new directory instead of a diff across every file.

2. A driver never reaches into a kernel global. It gets what it needs passed in (a base address,
   later a DMA allocator, later an interrupt registration). See `drivers/pl011.rs`: it takes a base
   address and knows nothing else.

3. The syscall surface stays narrow and explicit. It is a boundary, not a habit.

5. Architectural parity is a gate, not an aspiration: DECISIONS §19 (architectural parity is a
   tenet). The targets are aarch64, riscv64, and x86_64, all three of which now boot on real
   hardware. A kernel capability ships on every supported architecture, proven by the same suite, or
   a scope note records the gap and the plan. If a feature works on one ISA and silently not
   another, that is the bug.

Rules 2, 3 and 7 are what keep the microkernel option open (7 because a contract you cannot test is
a contract you cannot trust to replace a component behind). We are deliberately **not**
speculatively trait-ifying every subsystem, because that builds the wrong abstraction before the
requirements are known.

4. Assume weak memory ordering. We're on ARM, which is the weak one, and that's a gift: we
   cannot develop hidden strong-ordering assumptions the way an x86-first project would. Don't
   squander it.

6. Taking a dependency is a decision, not a convenience: DECISIONS §46 (thin primitives or whole
   subsystems). The tree's shape is thin architectural primitives (`aarch64-cpu`,
   `tock-registers`) or whole subsystems we would never write (`smoltcp`, vendored RedoxFS), with
   nothing in between: most crates here have no external dependencies at all. Write it if it is
   on the verification path, because you cannot restructure someone else's crate to make a model
   checker tractable. Vendor it if correctness is won by *exposure* rather than by reading the spec,
   which is why §46 says write the calendar and vendor the crypto.

7. **Anything two binaries must agree on is a crate, never a `#[path]` module** (calef, 2026-08-01).
   If a constant, an opcode, a layout, or an error code is shared by more than one program, it goes
   in `crates/` and is depended on. `#[path = "x.rs"] mod x;` is not an option.

   Two reasons, and `script/lint` check 5 carries the third: it counts consumers per `#[path]`
   target and fails at two.

   It removes a category that nothing enforces. A `#[path]` module is neither a program nor a crate,
   so a reader meeting `cseam::GRANT_VA` cannot tell what they are looking at, and `user/src/` held
   48 programs and 3 modules with nothing distinguishing them.

      And it makes location self-enforcing for free. Once shared definitions live in `crates/`,
   everything in `components/src/` is a program, with no files moved and nothing to remember.

   This was already the tree's practice for seven crates (`filesystem_contract`,
   `byte_sink_contract`, `credential_contract`, `clock_contract`, `entropy_contract`,
   `network_time_protocol`, `graphics_contract`); `cseam.rs`, now the `c_seam` crate, never
   justified the `#[path]` mechanism.

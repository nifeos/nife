---
status: BUILT
raised: 2026-08-25
built: 2026-09-01
---
# 164. x86_64 userspace can't build `aes` (and therefore `fs_server`): no SSE, no scalar fallback

x86_64 userspace builds `aes`, `redoxfs_server` and `mkfs`, and the x86_64 archive carries the last
two; the title names the blocker this milestone removed. It is a toolchain and dependency problem
rather than a hardware one, so it needs no board. Neither route this block sized is needed; the
section below says why, because the sizing was wrong in an instructive direction.

## What it turned out to need: one build flag

`aes` 0.8.4 selects its backend in `src/lib.rs` with `cfg_if`, and **every architecture-specific
arm is gated `not(aes_force_soft)`**. The crate ships a portable constant-time software backend in
`src/soft/`, reachable by setting that cfg. Measured against this tree's own FS-server build
command, on `x86_64-unknown-none`:

```
(no flag)              exit 101, rustc-LLVM ERROR: Do not know how to split the result of this operator!
--cfg aes_force_soft   exit 0, redoxfs_server and mkfs both linked
```

That is this block's exact error, reproduced and then cleared. The flag lives in
`.cargo/config.toml`'s `[target.x86_64-unknown-none]` block, beside the `relocation-model=static`
the kernel needs.

It is on the target rather than on the one package that needs it, and the coupling was noticed
rather than inherited. This block's own "fact worth knowing" is correct: `x86_64-unknown-none` is
the *same* target the kernel builds under, so a `rustflags` entry there reaches the kernel too.
Cargo has no way to ADD a flag (`RUSTFLAGS` and `--config target.*.rustflags` both REPLACE the
list), so scoping the cfg to `redoxfs_server_build` would have meant restating
`relocation-model=static` in `xtask/src/main.rs`, away from the paragraph explaining why the kernel
needs it. One copy in one place, with the cost written down where a reader meets it: every crate on
this target gets the cfg, and it is inert in all of them but `aes`, which is the only thing in this
tree or its dependency graph that reads the name. `cargo clippy` at the bars `script/lint` already
sets passes for the kernel, for `bench`, and for `user` + `user_rt` with it on.

## Why neither sized route is needed

Route 1 (patch the vendored `aes` for a scalar fallback) is unnecessary, and `patches/` does not
grow a third entry. This block named the right next step ("is it a feature flag this tree isn't
enabling, or a genuine gap in the crate's own portable path") and it took five minutes to answer:
the portable path is not a gap, it is a cfg nobody had set.

**Route 2 (an SSE-enabled x86 userspace target) is not owed for this blocker**, and its sizing was
the accurate half of this block: `kernel/src/arch/x86_64/` saves and restores no FPU/SSE state
anywhere, so enabling SSE would mean an `FXSAVE` area per thread and save/restore in the
context-switch path. None of that was built, because none of it is needed to compile `aes`.

**The honest cost of not taking Route 2 is speed, and this block should not let a reader assume
parity.** The software backend is a bitsliced constant-time implementation; upstream RustCrypto's
own figures put AES-NI roughly an order of magnitude ahead of it. Unmeasured here, and
deliberately so: nothing on x86_64 mounts an encrypted RedoxFS volume yet, so there is no workload
to measure and a synthetic number would be a fact leaving the machine with nothing behind it. When
an x86_64 workload touches the crypto path, that is when the number is owed, and Route 2 is what it
would be weighed against.

## What it delivered, measured

`script/test --arch x86_64`, before and after, on the same tree:

| | passed | skipped |
|---|---|---|
| before | 200 | 55 |
| after | 211 | 44 |

Zero tests were recovered, and reading that table as though eleven were is the mistake this
paragraph exists to prevent. All eleven that moved do not skip through `skip!` at all: they
`println!` a line and `return`, which the harness counts as a pass. They were honestly skipped for
want of an FS server and are now silently green for want of a disk, which is *less* signal than
before. Forty-six sites across sixteen files share that shape; see milestone 214 (a test that
prints "skipping" and returns is counted as passed).

What did change, and is real: the FS server and `mkfs` are in the x86_64 archive, so **the reason
those tests do not run is now a machine fact rather than a toolchain one**, which is what the next
milestone can act on. `kernel::user::disk_tests::the_write_half_...` now skips with "no blank-disk
fixture attached" instead of blaming `aes`.

## What broke after `aes`, and what it costs

The disk. With the server packed, `fs_service::wire_servers` still asks
`virtio::find_block_device_n(1)` for its disk, and `q35` has no virtio-mmio bus at all
(`arch::x86_64::mmu::VIRTIO_SLOTS` is 0). Attaching the fixtures as `virtio-blk-pci` and making the
lookup transport-blind was built in this lane, run, and reverted, because it does not work and
the reason is structural rather than a bug:

- `qemu-system-x86_64: Interrupt Mask set, irq is not generated`
- `qemu-system-x86_64: vtd_iommu_translate: detected translation failure (dev=00:04:00, iova=0xffde000)`
- the suite then wedged with no verdict.

The first line is the whole story and it is already written down in the tree, at
`arch::x86_64::mmu::PCI_IRQ_BASE`, which is `0` and says honestly that it is a marker rather than a
value. `pci::intx_irq(0, 4, 1)` is therefore `0`, and `arch::x86_64::irq::enable(0)` resolves that
through `isa_routing` to the PIT's legacy line: the confined block server was armed on the
timer and waited forever for an interrupt that was never going to be its. Nothing about that is
specific to the FS server; it is the first userspace PCI driver this architecture has ever been
asked to run. See milestone 215, on x86_64 PCI interrupt routing, which fixed it.

## What this unblocks

Milestone 87 (the x86_64 bare-metal machine), which is the Dell OptiPlex, is the one that
matters, and this moves it by
removing the toolchain wall and leaving a machine wall in its place. When calef sits down at the
null modem, the archive that boots carries a real filesystem server rather than nothing above the
kernel; what it still cannot do is find a disk, and that is one bounded piece of interrupt routing
rather than an open-ended port.

Any future x86_64 work needing `fs_server` is likewise no longer blocked on a dependency that will
not compile, which is what fatal risk 9 (`design/fatal-risks/README.md`, "The HAL is a fiction, and an
architecture costs a restructure rather than a port") named this as a piece of.

## BUGS

- `mkfs` is packed but has nothing to format on this architecture. Milestone 57's two
  `disk_service` wirings want the GPT and blank fixtures, which no x86_64 runner attaches; those
  tests skip with an accurate reason and no plan of their own. They ride milestone 215, on x86_64 PCI interrupt routing.
- The soft-AES cost is unmeasured, above.

## Follow-on

- **Milestone 215.** The disk. With the server packed, `fs_service::wire_servers` still asks for a
  virtio-mmio device that `q35` does not have, and the `virtio-blk-pci` route built in this lane was
  reverted because `PCI_IRQ_BASE` is 0 and the block server was armed on the PIT's legacy line. 215
  fixed the routing; `mkfs` and milestone 57's two `disk_service` wirings ride it for their
  fixtures.
- **Milestone 214.** Eleven tests count as passed without running a line of them, because they
  `println!` a line and `return` instead of using `skip!`. Milestone 214 found 80 such sites.
- **Refused.** Route 1, patching the vendored `aes` for a scalar fallback, was unnecessary and
  `patches/` does not grow a third entry. The portable constant-time backend is already in
  `src/soft/` and every architecture arm is gated `not(aes_force_soft)`, so it was a cfg nobody had
  set rather than a gap in the crate.
- **Refused.** Route 2, an SSE-enabled x86 userspace target, is not owed for this blocker.
  `kernel/src/arch/x86_64/` saves and restores no FPU/SSE state anywhere, so it would mean an
  `FXSAVE` area per thread and save/restore in the context-switch path, and none of that is needed
  to compile `aes`.
  This refusal is milestone 461 (design/roadmap/0461-an-sse-enabled-x86-64-userspace-target.md),
  which carries it with the condition that would change it.
- **Refused.** The soft-AES cost stays unmeasured on purpose. Upstream puts AES-NI roughly an order
  of magnitude ahead of the bitsliced backend, but nothing on x86_64 mounts an encrypted RedoxFS
  volume yet, so there is no workload and a synthetic number would be a fact leaving the machine
  with nothing behind it. The number is owed when an x86_64 workload touches the crypto path, and
  Route 2 is what it would be weighed against.
  This refusal is milestone 462 (design/roadmap/0462-the-soft-aes-cost.md), which carries it with the
  condition that would change it.
- **Recorded.** In `design/roadmap/0164-x86-64-fs-server-aes.md`: the `aes_force_soft` cfg sits on
  the target rather than on the one package that needs it, because Cargo can only replace a
  `rustflags` list and never add to it. Every crate on `x86_64-unknown-none` gets the cfg and it is
  inert in all of them but `aes`.

## Index row

The blocker was one build flag, and neither route this block sized is needed. `aes` 0.8.4 gates
every architecture-specific backend `not(aes_force_soft)` and ships a portable software one; `--cfg
aes_force_soft` on `[target.x86_64-unknown-none]` turns exit 101 into exit 0, so `patches/` grows no
third entry and the SSE context-switch state Route 2 priced is not owed. `redoxfs_server` and `mkfs`
now build for x86_64 and ride its archive. **No test was recovered**: the eleven that moved out of
the skip column `println!` and return rather than `skip!`, which is milestone 214. The wall behind
`aes` is the disk, and it is one bounded piece of interrupt routing, not a port: milestone 215 (a
PCI function's interrupt reaches nothing on x86_64).

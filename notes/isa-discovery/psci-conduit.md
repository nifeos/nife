# The PSCI conduit: two failure shapes and the `smc` fixture

*Name: provisional. For the parent's PSCI failure shapes and `smc` fixture. calef names files.*

An appendix of [ISA discovery](../isa-discovery.md). The parent keeps what milestone 100 (read the
machine's PSCI and its CPU list) reads, what it still assumes and what the boot says. This file
keeps why the two hardcoded facts failed differently, and how a QEMU option made the `smc` half
parseable.

## The two failure shapes were not the same, and the plan had to say so

The core list was a guaranteed silent no-op. A machine with more than four cores had cores 4 and
up never started, and there was no error available, because nothing asked about them. That is the
worst shape a failure can take: success reported, work not done, and the symptom arriving later as
a scheduler that never balances.

The conduit was board-specific and untested. `smp.rs` did have a degradation path, written for one
case: a core that is absent, `PSCI {ret}; not present?`. But a machine whose firmware answers on
`smc` is under no obligation to produce a PSCI error code from an `hvc` it never agreed to serve.
It would more likely be an undefined-instruction trap. Calling both "silent" was close enough for a
warning and too loose for a plan.

## The fixture that made the `smc` half testable

QEMU `virt` states `method = "hvc"`, so for a while it looked as though the other branch could only
be exercised by hand-writing a tree. It cannot, quite. `-machine virt,virtualization=on` puts
something at EL2 and QEMU's own PSCI moves to EL3, so the same board, one option different, states
`smc`. That dump is `crates/device_tree_blob/tests/fixtures/qemu-aarch64-virt-smc.dtb`. The host
test that compares the two is the whole finding in one assertion: the conduit is not a property of
aarch64, of QEMU, or of the `virt` board.

It does not make the `smc` call tested. That configuration enters the kernel at EL2 and this kernel
expects EL1, so nothing here boots it. Parsed, not called; the `arch::psci_cpu_on` BUGS block says
so where a reader meets the function.

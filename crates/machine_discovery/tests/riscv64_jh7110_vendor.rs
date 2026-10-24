//! The vendor tree the VisionFive 2 actually serves, and its two lies, on the host.
//!
//! It marks the S7 monitor core, which has no S-mode, `status = "okay"` with
//! `mmu-type = "riscv,sv39"`, so the `status` gate `riscv64_jh7110.rs` proves never fires.
//! Startability, the machine record and the PLIC expectations follow the hart's own `riscv,isa`
//! instead. The two fixtures reach one conclusion through different lies, so a regression in either
//! gate fails exactly one file. See `notes/visionfive2/hart-roster-stops.md`.

use machine_discovery::cpu_list::CpuList;
use machine_discovery::plic::PlicContexts;
use machine_discovery::riscv64::*;

const VENDOR: &[u8] = include_bytes!("fixtures/jh7110-vendor.dtb");

fn tree(bytes: &[u8]) -> device_tree_blob::DeviceTreeBlob<'_> {
    device_tree_blob::DeviceTreeBlob::from_bytes(bytes).expect("fixture is a valid device tree")
}

/// **The S7's lies do not narrow the machine.** Same shape as the mainline fixture's test, reached
/// through the other gate: here `status` says "okay" on every node, and only the supervisor rule
/// keeps the S7's `rv64imacu` out of the intersection. Four U74s speak; F and D survive; the
/// machine is exactly our contract.
#[test]
fn the_okay_s7_still_does_not_narrow_the_machine() {
    let mut cpu = Isa::from_device_tree(&tree(VENDOR)).expect("the CPU nodes parse");
    cpu.sbi.spec_major = 2;
    cpu.sbi.extensions = SBI_REQUIRED;

    assert_eq!(cpu.harts, 5, "five cpu@ nodes, honestly counted");
    assert_eq!(
        cpu.described, 4,
        "only the four supervisor-capable harts speak, status notwithstanding"
    );
    assert_eq!(cpu.base, Base::Rv64);
    assert!(cpu.legacy_isa_string, "the vendor tree is the old spelling");
    assert!(
        cpu.common.contains(F.union(D)),
        "the U74s' FPU survives: the S7's rv64imacu did not narrow the intersection"
    );
    assert!(
        !cpu.is_heterogeneous(),
        "the four harts the kernel can run on are identical; heterogeneity was the S7's"
    );
    assert_eq!(cpu.mmu, MmuType::Sv39);
    assert!(
        !cpu.missing_requirements().any(),
        "the VisionFive 2 can run this kernel, on the vendor tree too"
    );
}

/// **The roster records the status lie and the ISA truth side by side.** `usable` reports what the
/// tree said (okay, recorded not obeyed), `supervisor` reports what the hart's own ISA string
/// admits, and the bring-up predicate (`smp::read_cpu_list` requires both) refuses the S7 on the
/// second. This is the exact decision that was wrong on the bench.
#[test]
fn the_okay_s7_is_still_not_startable() {
    let list = CpuList::from_device_tree(&tree(VENDOR)).expect("cannot read /cpus");

    assert_eq!(list.described, 5);
    let s7 = &list.cpus()[0];
    assert_eq!(s7.hwid, 0);
    assert!(s7.usable, "the status lie, recorded as the tree told it");
    assert!(
        !s7.supervisor,
        "rv64imacu spells user mode and omits supervisor: the node's one truthful property"
    );
    assert!(
        !s7.is_startable(),
        "the bring-up predicate must refuse hart 0, or OpenSBI dies on it again"
    );
    for (i, cpu) in list.cpus().iter().enumerate().skip(1) {
        assert_eq!(cpu.hwid, i as u64);
        assert!(
            cpu.is_startable(),
            "hart {i} is a U74 (rv64imafdcbsux spells its s) the kernel may start"
        );
    }
    // The whole startable set, in one read: 5 harts described, 4 startable, hart 4 among them.
    // Seating is by hart id (smp::read_cpu_list), so the S7's lie costs it only its own seat and
    // the fourth U74 sits at slot 4 instead of falling off a positional truncation.
    let startable: Vec<u64> = list
        .cpus()
        .iter()
        .filter(|c| c.is_startable())
        .map(|c| c.hwid)
        .collect();
    assert_eq!(
        startable,
        [1, 2, 3, 4],
        "the vendor tree's lies must cost the S7 its start, and nobody else theirs"
    );
    assert_eq!(list.timebase_hz, Some(4_000_000));
}

/// **No S context for the S7, and `2h` for the U74s.** The PLIC wiring is silicon, not a cpu-node
/// claim, so it tells the truth in both trees: hart 0 contributes only a machine context. A hart
/// the supervisor rule excludes gets no S-context expectation, and the map agrees on its own.
#[test]
fn the_s7_has_no_s_context_for_the_vendor_tree_either() {
    let ctx = PlicContexts::from_device_tree(&tree(VENDOR)).expect("the PLIC wiring parses");

    assert_eq!(ctx.s_context(0), None, "an M-only hart has no S context");
    for hart in 1..=4 {
        assert_eq!(ctx.s_context(hart), Some(2 * hart));
    }
    assert_eq!(ctx.len(), 4);
}

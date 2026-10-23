//! What the machine says its cores are, and how to start them (milestone 100 (read the machine's
//! PSCI and its CPU list)), on the host; the machine side is the kernel's own tests, the split §7
//! (testing) asks for. Four trees, each a shape the others cannot show:
//!
//! * QEMU aarch64 `virt,gic-version=2,iommu=smmuv3`, `cortex-a72`, `-smp 4`, the machine
//!   `script/test` boots: `hvc`, `0xc4000003`, and cores `0..4`.
//! * QEMU aarch64 `virt` with `virtualization=on`, a real dump and the only `smc` here.
//! * QEMU riscv64 `virt`, from `device_tree_blob`'s fixtures, for `timebase-frequency`.
//! * A hand-written clustered tree: two address cells, a non-contiguous id, a disabled core, a
//!   spin-table core, more cores than the ceiling, and PSCI 0.1 with its own function id.

use machine_discovery::aarch64::{Conduit, PSCI_CPU_ON_64, Psci};
use machine_discovery::cpu_list::{CpuList, EnableMethod, MAX_CPU_NODES};

/// The machine `script/test` boots on aarch64, at the `-smp 4` the runner passes.
const QEMU_VIRT_SMP4: &[u8] =
    include_bytes!("../../device_tree_blob/tests/fixtures/qemu-aarch64-virt-smp4.dtb");
/// The same board with `virtualization=on`, which moves the conduit to `smc`.
const QEMU_VIRT_SMC: &[u8] =
    include_bytes!("../../device_tree_blob/tests/fixtures/qemu-aarch64-virt-smc.dtb");
/// The RISC-V machine, borrowed rather than copied so a regenerated tree cannot leave two crates
/// testing different bytes.
const QEMU_RISCV: &[u8] =
    include_bytes!("../../device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb");
const CLUSTERED: &[u8] = include_bytes!("fixtures/clustered-cpus.dtb");
const NO_PSCI: &[u8] = include_bytes!("fixtures/no-psci.dtb");
/// Eighteen cores, a `/cpus` whose `#address-cells` is two bytes rather than four, one `reg` of
/// the same shape, and a two-cell `timebase-frequency`: the properties that are shorter or wider
/// than the decoder reading them, and the one machine size the record cannot hold.
const MANY_HARTS: &[u8] = include_bytes!("fixtures/many-harts.dtb");
/// A `/psci` node stating only `method`, and a `reg` one cell short of the two `/cpus` declares.
const PARTIAL_PSCI: &[u8] = include_bytes!("fixtures/partial-psci.dtb");

fn cpus(bytes: &[u8]) -> CpuList {
    let dt = device_tree_blob::DeviceTreeBlob::from_bytes(bytes).expect("fixture should parse");
    CpuList::from_device_tree(&dt).expect("cpu list should decode")
}

fn psci(bytes: &[u8]) -> Option<Psci> {
    let dt = device_tree_blob::DeviceTreeBlob::from_bytes(bytes).expect("fixture should parse");
    Psci::from_device_tree(&dt).expect("psci node should decode")
}

/// **The four cores the runner asks for are the four the tree describes**, with the hardware ids the
/// kernel used to assume. This is the assertion that made the old hardcode *look* right: on this
/// machine `reg` really is `0..4`, so nothing the kernel did was wrong here. It is the shape of the
/// check `crates/pci` uses on its own hardcodes, and the reason a second board is where it breaks.
#[test]
fn qemu_virt_describes_the_four_cpus_the_runner_starts() {
    let list = cpus(QEMU_VIRT_SMP4);

    assert_eq!(list.described, 4, "the runner passes -smp 4");
    assert_eq!(list.len, 4);
    assert!(!list.is_truncated());
    assert_eq!(list.address_cells, 1, "QEMU virt numbers cores in one cell");
    assert_eq!(
        list.cpus().iter().map(|c| c.hwid).collect::<Vec<_>>(),
        [0, 1, 2, 3],
    );
    for cpu in list.cpus() {
        assert!(cpu.usable, "QEMU disables no core");
        assert_eq!(cpu.enable_method, EnableMethod::Psci);
    }
    assert_eq!(
        list.timebase_hz, None,
        "aarch64 states its counter rate in CNTFRQ_EL0, not in the tree"
    );
}

/// **The conduit and the function id the kernel used to compile in are what this machine states.**
///
/// Both halves matter and they fail differently. A wrong function id returns a PSCI error, which
/// `smp.rs` already degrades on; a wrong conduit is an undefined instruction on a machine with
/// nothing at that exception level, and there is no error code for that.
#[test]
fn qemu_virt_states_hvc_and_the_standard_cpu_on() {
    let psci = psci(QEMU_VIRT_SMP4).expect("virt has a /psci node");

    assert_eq!(psci.conduit, Some(Conduit::Hvc));
    assert_eq!(psci.cpu_on, Some(PSCI_CPU_ON_64));
    assert_eq!(
        psci.cpu_on,
        Some(0xC400_0003),
        "the number smp.rs used to hold"
    );
    assert!(psci.standard, "QEMU claims arm,psci-0.2 and arm,psci-1.0");
    assert!(
        psci.cpu_on_from_property,
        "and publishes the id anyway, which we take at its word"
    );
    assert!(psci.can_start_a_cpu());
}

/// **The same board, one machine option different, answers on `smc`.**
///
/// `virtualization=on` puts something at EL2, so QEMU's own PSCI moves to EL3 and the tree says so.
/// This is the whole finding in one comparison: the conduit is not a property of aarch64, of QEMU,
/// or of the `virt` board. Nothing boots this configuration here (the kernel expects to be entered
/// at EL1 and this one enters at EL2), so the `smc` **call path** remains untested on any machine;
/// what is proved is that we read the answer rather than assume it.
#[test]
fn the_same_board_with_a_hypervisor_states_smc() {
    let hvc = psci(QEMU_VIRT_SMP4).unwrap();
    let smc = psci(QEMU_VIRT_SMC).unwrap();

    assert_eq!(hvc.conduit, Some(Conduit::Hvc));
    assert_eq!(smc.conduit, Some(Conduit::Smc));
    assert_eq!(
        hvc.cpu_on, smc.cpu_on,
        "the function id is the same; only the instruction that carries it changes"
    );
}

/// **A machine with no `/psci` node is told apart from one we could not parse.** The kernel says
/// different things about the two, and there is nothing in a `None` field to distinguish them.
#[test]
fn a_tree_without_psci_answers_none() {
    assert_eq!(psci(NO_PSCI), None);
    assert_eq!(psci(QEMU_RISCV), None, "RISC-V starts harts over SBI HSM");
}

/// **PSCI 0.1 publishes its own function ids, and they are not the standard ones.**
///
/// This is the case the old `const PSCI_CPU_ON: u64 = 0xC400_0003` could not have survived: 0.1
/// predates the standardised id space entirely, so the only correct source is the property. The
/// fixture's id is deliberately not the standard one, so a decoder that reached for the constant
/// would fail here rather than pass by coincidence.
#[test]
fn psci_0_1_publishes_its_own_function_id() {
    let psci = psci(CLUSTERED).expect("the fixture has a /psci node");

    assert!(!psci.standard, "a bare arm,psci is 0.1");
    assert_eq!(psci.cpu_on, Some(0x95c1_ba5e));
    assert!(psci.cpu_on_from_property);
    assert_eq!(psci.conduit, Some(Conduit::Smc));
    assert!(psci.can_start_a_cpu());
}

/// **A two-cell hardware id decodes as one 64-bit number, not as its high half.**
///
/// The ARM CPU binding requires `#address-cells = <2>` on `/cpus` the moment any core's
/// `MPIDR_EL1.Aff3` is nonzero. A decoder fixed at one cell reads `<0x100 0x200>` as `0x100`, which
/// is a plausible id belonging to a different core, so the failure would be a `CPU_ON` aimed at the
/// wrong place rather than an error.
#[test]
fn a_clustered_machine_decodes_two_cell_ids() {
    let list = cpus(CLUSTERED);

    assert_eq!(list.address_cells, 2);
    assert_eq!(
        list.cpus().iter().map(|c| c.hwid).collect::<Vec<_>>(),
        [0x0, 0x1, 0x100, 0x101, 0x100_0000_0200, 0x201, 0x202],
        "affinity is not a dense range, which is the point",
    );
}

/// **`status` and `enable-method` come back as the tree wrote them.**
///
/// The module records rather than obeys, because whether a `spin-table` core is startable is a fact
/// about the kernel and not about the tree. Both are here so the kernel's refusal has something
/// truthful to refuse on.
#[test]
fn a_disabled_cpu_and_a_spin_table_cpu_are_reported_as_such() {
    let list = cpus(CLUSTERED);

    let disabled = list.cpus()[5];
    assert_eq!(disabled.hwid, 0x201);
    assert!(!disabled.usable, "status = \"disabled\"");
    assert_eq!(disabled.enable_method, EnableMethod::Psci);

    let spinning = list.cpus()[6];
    assert_eq!(spinning.hwid, 0x202);
    assert!(spinning.usable, "no status property means okay");
    assert_eq!(spinning.enable_method, EnableMethod::SpinTable);

    // And the ordinary case, so the two above are read against something.
    assert!(list.cpus()[2].usable);
    assert_eq!(list.cpus()[2].enable_method, EnableMethod::Psci);
}

/// **A machine with more cores than fit is reported as truncated, not as smaller.**
///
/// `described` keeps counting past `MAX_CPU_NODES`, so a caller can print "this machine has more
/// cores than this kernel was built for" instead of quietly reading a prefix and calling it the
/// machine. Seven cores is not enough to overflow sixteen slots, so the equality below is the
/// honest state of the fixture and the inequality is what a bigger machine would hit.
#[test]
fn the_described_count_is_the_machines_and_the_length_is_ours() {
    let list = cpus(CLUSTERED);

    assert_eq!(list.described, 7);
    assert_eq!(list.len, 7);
    assert!(!list.is_truncated());
    assert!(
        list.described <= MAX_CPU_NODES,
        "widen the array if this trips"
    );
}

/// **The RISC-V counter rate comes from `/cpus/timebase-frequency`.**
///
/// 10 MHz is the number `arch/riscv64/timer.rs` held as a `const` with a comment saying it was
/// hardcoded until the device-tree parse landed. The parse landed with milestone 60 and the comment
/// outlived it; this is the value it should have been reading, and the aarch64 side has always read
/// its own from `CNTFRQ_EL0`.
#[test]
fn the_riscv_timebase_comes_from_the_tree() {
    assert_eq!(cpus(QEMU_RISCV).timebase_hz, Some(10_000_000));
}

/// **The `/cpus` decode does not care which architecture wrote the tree.** Same call, same struct,
/// two machines. The only field that differs by architecture is the RISC-V-only timebase.
#[test]
fn both_architectures_answer_the_same_call() {
    let arm = cpus(QEMU_VIRT_SMP4);
    let riscv = cpus(QEMU_RISCV);

    assert_eq!(arm.address_cells, riscv.address_cells);
    assert_eq!(riscv.described, 1, "the riscv fixture was dumped at -smp 1");
    assert_eq!(riscv.cpus()[0].hwid, 0);
    assert_eq!(
        riscv.cpus()[0].enable_method,
        EnableMethod::Unstated,
        "the RISC-V binding has no enable-method: SBI HSM is the only mechanism",
    );
    assert!(riscv.cpus()[0].usable, "the riscv tree says status = okay");
}

/// **Eighteen cores in a sixteen-slot record: `described` is the machine's and `len` is ours.**
///
/// The test above states the same property on a fixture that does not overflow, which proves the
/// two numbers agree and nothing about what happens when they cannot. This is the other half, and
/// it is the only place `is_truncated` is ever true: a predicate that answered `false` for every
/// tree in the suite is a predicate no caller could rely on.
#[test]
fn a_machine_with_more_cpus_than_fit_says_so() {
    let list = cpus(MANY_HARTS);

    assert_eq!(list.described, 18, "the tree describes eighteen");
    assert_eq!(list.len, MAX_CPU_NODES, "the record holds sixteen");
    assert!(
        list.is_truncated(),
        "and a caller has to be able to see that"
    );
    assert_eq!(
        list.cpus().last().map(|c| c.hwid),
        Some(0xf),
        "the prefix that fit is the first sixteen, in tree order",
    );
}

/// **A `#address-cells` shorter than one cell is a malformed tree, and the fallback is one cell.**
///
/// The CPU bindings require the property, so there is no correct reading of two bytes of it; what
/// there is, is a decoder that must not read the two bytes that follow it in the blob. One cell is
/// the overwhelmingly common shape and the documented fallback.
#[test]
fn an_address_cells_property_too_short_to_hold_a_cell_falls_back_to_one() {
    assert_eq!(cpus(MANY_HARTS).address_cells, 1);
}

/// **A `reg` too short for the width `/cpus` declares yields no hardware id, at either width.**
///
/// Zero is the documented direction: it collides with the boot core, which every caller skips,
/// rather than naming some other core to start. The failure this refuses is the opposite one,
/// reading the bytes that follow the property and starting whatever number they spell.
#[test]
fn a_reg_shorter_than_its_declared_width_has_no_hardware_id() {
    let one_cell = cpus(MANY_HARTS);
    assert_eq!(one_cell.address_cells, 1);
    assert_eq!(
        one_cell.cpus()[11].hwid,
        0,
        "cpu@b states two bytes where one cell is four",
    );
    assert_eq!(one_cell.cpus()[10].hwid, 0xa, "and its neighbours decode");
    assert_eq!(one_cell.cpus()[12].hwid, 0xc);

    let two_cells = cpus(PARTIAL_PSCI);
    assert_eq!(two_cells.address_cells, 2);
    assert_eq!(
        two_cells.cpus()[1].hwid,
        0,
        "cpu@1 states one cell where /cpus declares two",
    );
}

/// **A two-cell `timebase-frequency` decodes as one 64-bit number.**
///
/// The RISC-V binding allows either width. QEMU writes one cell, which is what every other fixture
/// here holds, so the wider arm has never been read; a counter above 4 GHz needs it, and a decoder
/// that dropped the arm would report no timebase at all and send the timer back to a constant.
#[test]
fn a_two_cell_timebase_decodes_as_one_number() {
    assert_eq!(cpus(MANY_HARTS).timebase_hz, Some(0x1_0200_0000));
}

/// **A `/psci` node that states only its conduit is a node, not an absent one.**
///
/// The two answers are different and a caller says different things about them: `Ok(None)` is a
/// machine with no PSCI at all (spin-table bring-up, or a uniprocessor), while this is a machine
/// whose firmware published a conduit and no function id, on which starting a core is impossible
/// for a reason a boot line can name. A decoder demanding all three properties before believing in
/// the node would report this machine as the first kind.
#[test]
fn a_psci_node_stating_only_its_conduit_is_still_a_node() {
    let psci = psci(PARTIAL_PSCI).expect("the node exists, incomplete as it is");

    assert_eq!(psci.conduit, Some(Conduit::Hvc));
    assert_eq!(psci.cpu_on, None, "no cpu_on property and no 0.2 claim");
    assert!(!psci.cpu_on_from_property);
    assert!(!psci.standard);
    assert!(
        !psci.can_start_a_cpu(),
        "both halves are needed, and only one is here",
    );
}

/// **The names the boot line prints are the spellings the bindings use.**
///
/// They are the only thing a person reading a transcript has to go on when a core does not start,
/// and a boot line that printed the wrong conduit, or printed nothing, would send whoever is at
/// the bench looking at the wrong exception level. Both enums are here because both reach the same
/// line.
#[test]
fn the_boot_lines_words_are_the_bindings_own() {
    assert_eq!(Conduit::Hvc.name(), "hvc");
    assert_eq!(Conduit::Smc.name(), "smc");

    assert_eq!(EnableMethod::Unstated.name(), "unstated");
    assert_eq!(EnableMethod::Psci.name(), "psci");
    assert_eq!(EnableMethod::SpinTable.name(), "spin-table");
    assert_eq!(EnableMethod::Other.name(), "other");
}

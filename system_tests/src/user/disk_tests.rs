use core::sync::atomic::Ordering;

use block_roster::{
    Roster, TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS, TRANSPORT_VIRTIO_MEMORY_MAPPED_IO,
    TRANSPORT_VIRTIO_PCI,
};

use super::*;
use crate::arch::exceptions::USER_FAULTS;

/// Bounded by wall clock rather than by a yield count, for the reason `ntp_tests` records: work a
/// test spawns runs on other cores, so a yield on an idle core elapses no real time.
fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if cond() {
            return true;
        }
        crate::sched::yield_now();
    }
    cond()
}

fn surveyor_image() -> &'static [u8] {
    program("disk_surveyor").expect("no disk_surveyor program in the initial ramdisk")
}

fn partitioner_image() -> &'static [u8] {
    program("disk_partitioner").expect("no disk_partitioner program in the initial ramdisk")
}

// The report's flag words. Must match components/src/disk_surveyor.rs.
const F_ROSTER: u64 = 1 << 0;
const F_SIZE: u64 = 1 << 1;
const F_MBR: u64 = 1 << 2;
const F_PRIMARY: u64 = 1 << 3;
const F_BACKUP: u64 = 1 << 4;
const F_NIFE: u64 = 1 << 5;
const F_NAMES: u64 = 1 << 6;
const R_PROBING: u64 = 0x50_0B_11_46;
const P_RW_REFUSED: u64 = 1 << 0;
const R_HOLDING: u64 = 0x_48_4F_4C_44_47;

/// The layout `sgdisk` 1.0.10 wrote into the fixture the test image is built from
/// (`crates/globally_unique_identifier_partition_table/tests/real_disks.rs` has the exact
/// commands). Three partitions on a 64 MiB disk, the third of them a nife data partition (DECISIONS
/// §45) starting at block 30720.
const PARTITIONS: u64 = 3;
const NIFE_FIRST_LBA: u64 = 30720;

/// **Message one: the roster, which is the authority that does NOT involve the disk.** The
/// surveyor's per-kind counts, checked against the runner's fixtures and against the kernel's own
/// read of the same frame.
fn roster_message_names_every_disk(w: &disk_service::Wiring) {
    let [mmio, pci, nvme, ..] = crate::sched::ipc_receive(w.report);
    let total = mmio + pci + nvme;
    assert_eq!(
        total as usize, w.devices,
        "the surveyor counted {mmio} virtio-mmio, {pci} virtio-pci and {nvme} nvme; the kernel put \
         {} devices in the page, so some entry is of a kind the surveyor does not count",
        w.devices,
    );
    // **The NVMe disk is listed, once, exactly when one is attached** (milestone 421). The runners
    // attach a controller on all three legs of `cargo xtask test`, and it is the one disk whose
    // driver is a confined process; a roster that leaves it out says the machine has no NVMe.
    assert_eq!(
        nvme,
        u64::from(NON_VOLATILE_MEMORY_EXPRESS_ATTACHED),
        "the surveyor lists {nvme} NVMe disks; NIFE_NVME {} on this leg, so the roster owes {}. \
         An NVMe controller missing from the roster is disk_service::devices not counting \
         pci::count_non_volatile_memory_express_controllers",
        if NON_VOLATILE_MEMORY_EXPRESS_ATTACHED {
            "is set"
        } else {
            "is not set"
        },
        u64::from(NON_VOLATILE_MEMORY_EXPRESS_ATTACHED),
    );
    // The two shapes the same five fixtures take. On the virt boards they are five mmio slots
    // and one PCI function (milestone 215 (a PCI function's interrupt reaches nothing on x86_64
    // so no userspace driver can run there), its nifefs disk); on `q35` there is no virtio-mmio bus
    // at all, so since milestone 420 (x86_64 has one fixture of five and no NIC, GPU, keyboard
    // or RNG) all five are PCI functions, in the same ordinal order. Either way the kernel's own
    // read of the same frame, below, must agree with the program's count.
    if cfg!(target_arch = "x86_64") {
        assert_eq!(
            mmio, 0,
            "q35 has no virtio-mmio bus, yet the roster names {mmio} mmio disks"
        );
    } else {
        assert!(
            mmio >= 4,
            "at least four mmio disks are attached when this test runs (nifefs, RedoxFS, the \
             crash image, the GPT image, and since milestone 57 (partitioning and formatting a \
             real drive and extended attributes)'s write half the blank one); the \
             roster names {mmio}",
        );
        assert_eq!(
            pci, 1,
            "one virtio-blk-pci disk is attached (§18's transport)"
        );
    }

    // And the kernel's own read of the same frame agrees, computed through the same contract crate
    // rather than by trusting the number the program sent back.
    let here = Roster::read(w.roster()).expect("the kernel wrote a roster it cannot read");
    assert_eq!(here.len(), w.devices);
    if X86_FIXTURES_REQUIRED {
        for (i, d) in here.iter().take(5).enumerate() {
            assert_eq!(
                (d.transport, d.ordinal as usize),
                (TRANSPORT_VIRTIO_PCI, i),
                "roster entry {i} is not PCI function {i}: the five fixtures ride in ordinal order"
            );
        }
    }
    assert_eq!(
        here.count_on(TRANSPORT_VIRTIO_MEMORY_MAPPED_IO) as u64,
        mmio
    );
    assert_eq!(here.count_on(TRANSPORT_VIRTIO_PCI) as u64, pci);
    assert_eq!(
        here.count_on(TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS) as u64,
        nvme
    );
    // The NVMe entry comes after every virtio one and is ordinal 0, the controller
    // `non_volatile_memory_express_service` serves, so adding it moved no virtio ordinal.
    if NON_VOLATILE_MEMORY_EXPRESS_ATTACHED {
        let last = here
            .get(w.devices - 1)
            .expect("the roster has an NVMe entry");
        assert_eq!(
            (last.transport, last.ordinal, last.transport_name()),
            (TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS, 0, Some("nvme")),
            "the NVMe controller is not the roster's last entry at ordinal 0",
        );
    }
}

/// **The machine finds a partition table on a disk it did not write, and says what is on it.**
///
/// Milestone 57's non-optional half: an OS that cannot read a GPT cannot find a filesystem on a
/// disk it did not create. The image is built from the committed `sgdisk` fixture, so the table was
/// written by gptfdisk, not by this project. The backup check reads a second, differently aligned
/// run: the 33 logical blocks ending on the last block, which begin partway into a filesystem block
/// (`globally_unique_identifier_partition_table::span`). The roster half holds milestone 421 (the
/// block roster names an NVMe disk): an attached NVMe controller must be listed.
///
/// Falsification: replayable `system_tests/falsifications/user.disk_tests.the_disk_surveyor_reads_a_table_gptfdisk_wrote.patch`
#[test_case]
fn the_disk_surveyor_reads_a_table_gptfdisk_wrote() {
    let Some(w) = disk_service::start(fs_service::blk_server_image(), surveyor_image()) else {
        // No fourth mmio block device: this boot did not build the GPT image. A fact about the
        // machine, which the final line must carry as a skip (milestone 214,
        // design/roadmap/0214-print-and-return-skips.md).
        skip_unless_required!(
            X86_FIXTURES_REQUIRED,
            "no GPT disk attached (this boot did not build the GPT image)"
        );
    };
    // **On the PVH x86_64 leg the roster is exactly the five fixtures, no more and no fewer**
    // (milestone 420 (x86_64 has one fixture of five and no NIC, GPU, keyboard or RNG)). The runner
    // attaches the crash, GPT and blank disks softly (`[ -f img ]`) and the assertions below hold
    // for any count, so this names them: a lost fixture slides every ordinal above it down.
    if X86_FIXTURES_REQUIRED {
        assert_eq!(
            w.devices,
            5 + usize::from(NON_VOLATILE_MEMORY_EXPRESS_ATTACHED),
            "the x86_64 roster names {} block devices; the runner owes five virtio functions \
             (nifefs=0, redoxfs=1, crash=2, gpt=3, blank=4) and one NVMe controller when \
             NIFE_NVME is set. A fixture is missing from helpers/qemu-runner-x86_64.sh, and \
             ordinals above the missing one have slid down",
            w.devices,
        );
    }
    // Past device bring-up, so a hang below is a hang in a read rather than in the driver.
    let [ready, ..] = crate::sched::ipc_receive(w.ready);
    assert_eq!(ready, filesystem_contract::READY);

    roster_message_names_every_disk(&w);

    // Message two: the table, which is the authority that does.
    let [flags, partitions, nife_first_lba, ..] = crate::sched::ipc_receive(w.report);
    for (bit, what) in [
        (F_ROSTER, "the roster page read as a roster"),
        (
            F_SIZE,
            "the block service reported a whole number of blocks",
        ),
        (F_MBR, "LBA 0 holds a protective MBR covering the disk"),
        (F_PRIMARY, "the primary header and entry array parsed"),
        (F_BACKUP, "the backup table agrees with the primary"),
        (F_NIFE, "a nife data partition is on the disk"),
        (F_NAMES, "every partition name decoded"),
    ] {
        assert!(flags & bit != 0, "{what} (flags {flags:#x})");
    }
    assert_eq!(partitions, PARTITIONS);
    assert_eq!(
        nife_first_lba, NIFE_FIRST_LBA,
        "the nife partition starts where sgdisk put it",
    );
}

/// **A listing is not a lever.**
///
/// The roster answers "what drives are attached" and nothing else. The same binary, mapping and
/// roster address, with no disk: it writes there, and the write is refused because the mapping is
/// read-only. The boundary is a page permission, not a check the program could be argued out of.
/// This is the claim `lsblk` plus `parted` cannot make: there the listing is world-readable and
/// naming a device to a destructive tool is a matter of typing its path. Here the enumeration is a
/// read-only mapping and the disk is a rendezvous somebody else was handed (notes/block-devices.md).
#[test_case]
fn the_roster_is_a_listing_and_not_a_lever() {
    let faults = USER_FAULTS.load(Ordering::Relaxed);
    let report = disk_service::start_probe(surveyor_image());

    let [tag, va, how, ..] = crate::sched::ipc_receive(report);
    assert_eq!(tag, R_PROBING, "the probe never reached its write");
    assert_eq!(va, disk_service::ROSTER_VA);

    // **The rung the migration added** (milestone 108). The probe holds the roster as a `PageFrame`
    // with `READ` alone, so it asked `PageFrame::MAP` for a writable window first and was refused
    // before a page-table entry was written. When the roster was a spawn-time mapping there was
    // nothing to refuse: the only boundary was the permission bit the fault below trips.
    assert!(
        how & P_RW_REFUSED != 0,
        "a frame held READ-only produced a WRITABLE mapping of the roster",
    );

    assert!(
        wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > faults),
        "a program wrote the block-device roster at {va:#x} and was NOT stopped",
    );
    // The exact address, on both ISAs (milestone 19's portable fault record). The *kind* is not
    // asserted: what is being claimed here is where the write was aimed and that it did not land.
    assert_eq!(
        crate::arch::exceptions::last_user_fault().map(|(_, addr)| addr),
        Some(disk_service::ROSTER_VA),
    );
}

/// **A page a driver holds can be taken back; a page it was handed at birth cannot.**
///
/// Milestone 108's claim. A spawn-time mapping (`Spawn::maps`) is recorded nowhere, so
/// `revoke::revoke_page_frame`, which works off the mapping database, cannot reach it; the roster
/// is a held `PageFrame` instead. The program maps the frame, reads and reports the first word, and
/// parks. The kernel checks that word against its own direct-map read of the same page, which says
/// the mapping was real and of this page. Then it revokes the frame and resumes the program: the
/// second read faults at that address and "the read landed" never arrives. Against a spawn-time
/// mapping the second read would return the same word as the first.
#[test_case]
fn the_roster_can_be_revoked_out_from_under_its_holder() {
    let faults = USER_FAULTS.load(Ordering::Relaxed);
    let w = disk_service::start_holder(surveyor_image());

    let [tag, word, ..] = crate::sched::ipc_receive(w.report);
    assert_eq!(tag, R_HOLDING, "the holder never mapped its roster frame");
    assert_eq!(
        word,
        w.first_word(),
        "the holder read something other than the roster frame the kernel granted it",
    );
    // The roster's first word is its magic. Asserting it (rather than "not zero") is what makes the
    // second read's absence meaningful: a page of zeroes would read the same before and after.
    assert_eq!(word, block_roster::MAGIC);

    // Take the page back. Every capability to it is deleted and it is unmapped from every address
    // space that recorded a mapping of it, the holder's included.
    crate::revoke::revoke_page_frame(w.roster_phys);
    crate::sched::ipc_send(w.resume, [0, 0, 0]);

    assert!(
        wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > faults),
        "a program read a frame that had been revoked out from under it, at {:#x}, and was NOT \
         stopped: the mapping outlived the capability",
        disk_service::ROSTER_VA,
    );
    assert_eq!(
        crate::arch::exceptions::last_user_fault().map(|(_, addr)| addr),
        Some(disk_service::ROSTER_VA),
        "the fault was somewhere other than the revoked page",
    );
}

// =============================================================================================
// Milestone 57's write half: partitioning and formatting on the target
// =============================================================================================

// The partitioner's roles and verdicts. Must match components/src/disk_partitioner.rs.
const ROLE_PARTITION: u64 = 0;
const ROLE_VERIFY: u64 = 1;
const R_PARTITIONED: u64 = 0x_50_41_52_54_44;
const P_NO_ENTROPY: u64 = 0x_4E_4F_52_4E_47;
const PF_MBR: u64 = 1 << 0;
const PF_PRIMARY: u64 = 1 << 1;
const PF_BACKUP: u64 = 1 << 2;
const PF_NIFE: u64 = 1 << 3;
const PF_NAMES: u64 = 1 << 4;
const PF_UNIQUE: u64 = 1 << 5;

// `mkfs`'s roles and verdicts. Must match redoxfs_server/src/bin/mkfs.rs.
const ROLE_MAKE: u64 = 0;
const ROLE_CHECK: u64 = 1;
const R_MADE: u64 = 0x_4D_4B_46_53_44;
const M_NO_ENTROPY: u64 = 0x_4E_4F_52_4E_47;
const M_NO_DISK: u64 = 0x_4E_4F_44_53_4B;
const R_FOUND: u64 = 0x_46_4F_55_4E_44;
const R_NO_FS: u64 = 0x_4E_4F_46_53_00;

/// The entropy service's request rendezvous, wiring the service if this is the first test to ask.
///
/// The bus this machine has, because this test is not about the bus (`entropy_tests` runs the
/// same service over both): the virt boards offer virtio-mmio with nothing in the way, and `q35`
/// has no virtio-mmio bus at all, so it asks for the PCIe transport there (milestone 420's
/// `virtio-rng-pci`).
///
/// Returns `None` if the bus asked for has no virtio-rng device (milestone 145 (a test that needs
/// hardware the boot doesn't have can say so): correct on a bare board boot with no
/// `NIFE_RNG`-equivalent).
fn entropy_rendezvous() -> Option<crate::sched::RendezvousId> {
    let image = program("entropy").expect("no entropy program in the initial ramdisk");
    let bus = if cfg!(target_arch = "x86_64") {
        entropy_service::Bus::Pci
    } else {
        entropy_service::Bus::Mmio
    };
    let w = entropy_service::ensure(image, bus)?;
    if let Some(report) = w.wait_for_ready() {
        assert_eq!(
            report[0],
            entropy_contract::READY,
            "the entropy service did not come up (it reported {:#x})",
            report[0],
        );
    }
    Some(w.request)
}

/// **The machine partitions a disk and puts a filesystem on it, and can do neither without the
/// capability that makes an identifier unique.**
///
/// ```text
///   1  partition, no entropy   -> refused          5  mkfs, no disk       -> refused
///   2  read the table          -> nothing there    6  read the partition  -> still no filesystem
///   3  partition, with entropy -> three partitions 7  mkfs, both          -> a filesystem
///   4  mkfs, no entropy        -> refused          8  read the partition  -> the file is in it
/// ```
///
/// Milestone 57's write half, on one disk and in order, because only a later read shows that a
/// refused run left the disk as it found it. The two capabilities are jointly sufficient and
/// separately necessary; why entropy is a real need is in notes/block-devices.md, "The disk tests".
#[test_case]
fn the_write_half_needs_a_disk_and_an_entropy_rendezvous_and_holds_nothing_else() {
    if fs_service::mkfs_image().is_none() {
        crate::testing::skip!(fs_service::NO_MKFS);
    }
    let Some(disk) = disk_service::blank_disk(fs_service::blk_server_image()) else {
        // No fifth mmio block device: this boot did not build the blank image. A fact about the
        // machine, not a failure (milestone 145: reported as skipped rather than a silent "ok").
        skip_unless_required!(
            X86_FIXTURES_REQUIRED,
            "no blank-disk fixture attached (this boot did not build one)"
        );
    };
    if let Some(ready) = disk.blk_ready {
        // Past device bring-up, so a hang below is a hang in a write rather than in the driver.
        let [word, ..] = crate::sched::ipc_receive(ready);
        assert_eq!(word, filesystem_contract::READY);
    }
    let Some(entropy) = entropy_rendezvous() else {
        skip_unless_required!(
            X86_RNG_ATTACHED,
            "no virtio-rng device on the bus this fixture asks for (NIFE_RNG not set?)"
        );
    };
    let maker = program("mkfs").expect("no mkfs program in the initial ramdisk");
    partition_phase(&disk, entropy);
    mkfs_phase(maker, &disk, entropy);
}

/// Steps 1 to 3 of the write half: the partitioner refuses without entropy, leaves the disk
/// untouched, then succeeds with it and the table reads back.
fn partition_phase(disk: &disk_service::BlankDisk, entropy: crate::sched::RendezvousId) {
    use filesystem_fixture::blank;

    // 1. The partitioner, with the disk and NO entropy rendezvous. It must refuse, and it must refuse
    //    before writing anything, which is why the program draws all four ids first and lays out
    //    the table second.
    let report = disk_service::start_partitioner(partitioner_image(), disk, ROLE_PARTITION, None);
    let [verdict, ..] = crate::sched::ipc_receive(report);
    assert_eq!(
        verdict, P_NO_ENTROPY,
        "a partitioner with no entropy rendezvous reported {verdict:#x}; it must refuse rather than \
         invent a GUID, because an identifier that is not random is not unique",
    );

    // 2. And the disk still has no table on it, which is what makes step 1 mean something: "it
    //    reported a refusal" and "it wrote nothing" are different claims.
    let report = disk_service::start_partitioner(partitioner_image(), disk, ROLE_VERIFY, None);
    let [flags, partitions, ..] = crate::sched::ipc_receive(report);
    assert_eq!(
        (flags, partitions),
        (0, 0),
        "the refused partitioner left something on the disk (flags {flags:#x})",
    );

    // 3. The same binary, the same role, the same disk, one capability more.
    let report =
        disk_service::start_partitioner(partitioner_image(), disk, ROLE_PARTITION, Some(entropy));
    let [verdict, written, step, ..] = crate::sched::ipc_receive(report);
    assert_eq!(
        verdict, R_PARTITIONED,
        "the partitioner failed at step {written} ({step})",
    );
    assert_eq!(written, blank::PARTITIONS as u64);

    // And the table is really there, read back by a process that cannot write: both copies, their
    // CRCs, the protective MBR, the names, and three distinct version-4 GUIDs.
    let report = disk_service::start_partitioner(partitioner_image(), disk, ROLE_VERIFY, None);
    let [flags, partitions, data_lba, ..] = crate::sched::ipc_receive(report);
    for (bit, what) in [
        (PF_MBR, "LBA 0 holds a protective MBR covering the disk"),
        (PF_PRIMARY, "the primary header and entry array parsed"),
        (PF_BACKUP, "the backup table agrees with the primary"),
        (PF_NIFE, "a nife data partition is on the disk"),
        (PF_NAMES, "every partition name decoded and matched"),
        (
            PF_UNIQUE,
            "every unique GUID is distinct, non-zero and version 4",
        ),
    ] {
        assert!(flags & bit != 0, "{what} (flags {flags:#x})");
    }
    assert_eq!(partitions, blank::PARTITIONS as u64);
    assert_eq!(
        data_lba,
        blank::DATA.0,
        "the data partition starts where it was placed",
    );
}

/// Steps 4 to 8 of the write half: `mkfs` refuses without entropy or without a disk, neither
/// refusal creates anything, and with both it makes a filesystem another process can open.
fn mkfs_phase(
    maker: &'static [u8],
    disk: &disk_service::BlankDisk,
    entropy: crate::sched::RendezvousId,
) {
    use filesystem_fixture::blank;

    // 4. `mkfs` with the disk and no entropy. The same refusal one layer up: a filesystem's uuid has
    //    the GPT's problem, and the engine now takes it as an argument for exactly this reason.
    let report = disk_service::start_maker(maker, disk, ROLE_MAKE, None, true);
    let [verdict, ..] = crate::sched::ipc_receive(report);
    assert_eq!(
        verdict, M_NO_ENTROPY,
        "mkfs with no entropy rendezvous reported {verdict:#x}; it must refuse",
    );

    // 5. And with entropy and no disk: nothing to read a table off, nothing to write to.
    let report = disk_service::start_maker(maker, disk, ROLE_MAKE, Some(entropy), false);
    let [verdict, ..] = crate::sched::ipc_receive(report);
    assert_eq!(
        verdict, M_NO_DISK,
        "mkfs with no block rendezvous reported {verdict:#x}; there is nothing it can write to",
    );

    // 6. Neither refusal created anything. A partition that has been *described* is not a partition
    //    that has been *formatted*, and this is where that difference is read off the platter.
    let report = disk_service::start_maker(maker, disk, ROLE_CHECK, None, true);
    let [verdict, ..] = crate::sched::ipc_receive(report);
    assert_eq!(
        verdict, R_NO_FS,
        "something put a filesystem in the partition before any run that was allowed to",
    );

    // 7. Both capabilities, and they are the whole authority needed to create a filesystem.
    let report = disk_service::start_maker(maker, disk, ROLE_MAKE, Some(entropy), true);
    let [verdict, blocks, first, ..] = crate::sched::ipc_receive(report);
    assert_eq!(
        verdict, R_MADE,
        "mkfs failed at step {blocks} with errno {first}",
    );
    let want_blocks = (blank::DATA.1 - blank::DATA.0 + 1) * blank::LBA
        / filesystem_contract::blk::BLOCK_SIZE as u64;
    assert_eq!(blocks, want_blocks, "the filesystem covers the partition");
    assert_eq!(
        first,
        blank::DATA.0 * blank::LBA / filesystem_contract::blk::BLOCK_SIZE as u64,
        "and starts at the partition's first block",
    );

    // 8. And it is a filesystem a *different* process can open, holding the file that run wrote.
    //    The host tool reads the same image from outside the guest after the run, which is the half
    //    a cache cannot fake (`cargo xtask test`'s blank-image check).
    let report = disk_service::start_maker(maker, disk, ROLE_CHECK, None, true);
    let [verdict, n, ..] = crate::sched::ipc_receive(report);
    assert_eq!(
        verdict, R_FOUND,
        "the filesystem mkfs created does not hold the file it wrote",
    );
    assert_eq!(n, blank::MADE_BODY.len() as u64);
}

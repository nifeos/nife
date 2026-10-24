//! **A confined EL0 process drives the machine's NVMe disk** (milestone 261 (the NVMe driver leaves
//! the kernel); DECISIONS §86 (whether an NVMe driver can leave the kernel)'s option 2a,
//! notes/non-volatile-memory-express.md).
//!
//! The sequence milestone 53 (the board's own peripherals) proved: find the controller over §18
//! (the PCIe transport), confine it behind the IOMMU, bring it from reset through identify to an
//! I/O queue pair, and serve the blk verbs. Here the queue half runs at EL0, in a process holding
//! two endpoints, a page of BAR0 and DMA pages, and no capability naming any of it. It is fatal
//! risk 6's prerequisite, not its experiment, and a number measured under QEMU is about QEMU; see
//! the note's "What the test proves, and where".

use super::*;

/// Wire the server, or `None` when this machine has no NVMe controller (a bare board boot, or a
/// leg the runner did not attach one on). Callers `skip!()` at their own call site rather than
/// have this helper do it, because `skip!()` returns from its immediate caller and a helper is
/// not the test.
fn start() -> Option<non_volatile_memory_express_service::Wiring> {
    let image = program("non_volatile_memory_express")
        .expect("no non_volatile_memory_express program in the initial ramdisk");
    // A controller that is there and refused is a failure, never a skip (milestone 261's bench
    // rehearsal): only `NoController` may skip.
    let w = match non_volatile_memory_express_service::ensure_or_why(image) {
        Ok(w) => w,
        Err(crate::non_volatile_memory_express::Absent::NoController) => return None,
        Err(crate::non_volatile_memory_express::Absent::Refused { rid, why }) => panic!(
            "an NVMe controller at requester id {rid:#06x} is on the bus and this driver refused              it: {why:?}. Not a skip: see notes/risk-6-bench-evening.md"
        ),
    };
    if let Some(report) = w.wait_for_ready() {
        assert_eq!(
            report[0],
            filesystem_contract::READY,
            "the NVMe server did not come up (it reported {:#x}, first-read status {:#x}; see \
             components/src/non_volatile_memory_express.rs for the step words)",
            report[0],
            report[2],
        );
        // **The geometry survived the handoff into ring 3.** Compared against what the kernel's
        // admin plane read from IDENTIFY, not against a constant: on the runner's image that is
        // 8 MiB and on xenon's Micron it is 256 GB, and this assertion is about the *handoff*
        // rather than about either number (milestone 318).
        assert_eq!(
            report[1], w.size_bytes,
            "the namespace size did not survive the spawn handoff into ring 3 (the kernel read \
             {} bytes from IDENTIFY)",
            w.size_bytes,
        );
    }
    // A namespace of no blocks would make every assertion below vacuously true, and
    // `kernel/src/non_volatile_memory_express.rs::bring_up` already refuses one; say so here rather
    // than let a silent zero pass for a pass.
    assert!(
        w.size_bytes >= 2 * crate::non_volatile_memory_express::BLOCK_SIZE as u64,
        "the namespace is {} bytes, too small for this test's two blocks",
        w.size_bytes,
    );
    Some(w)
}

/// **The headline.** An unprivileged process drives a real NVMe controller: it answers the disk's
/// size, persists two blocks, brings each back byte for byte, and keeps each write off the other's
/// block, every answer crossing a rendezvous from a client with no device, doorbell or DMA page.
///
/// Every assertion is written against the geometry this boot was handed, never a constant, so it
/// holds on QEMU's 8 MiB image and xenon's 256 GB namespace (milestone 318), and none assumes the
/// disk's prior contents, so it re-runs on a used disk. One test on purpose: bring-up is not
/// idempotent (a second wiring would reset the controller and recreate the queues under the first).
///
/// Falsification: replayable `system_tests/falsifications/user.non_volatile_memory_express_tests.a_confined_el0_process_serves_the_block_interface_end_to_end.patch`
#[test_case]
fn a_confined_el0_process_serves_the_block_interface_end_to_end() {
    let Some(disk) = start() else {
        crate::testing::skip!("no NVMe controller came up (NIFE_NVME not set on this leg?)");
    };

    // Every test boot fronts the PCIe bus with an IOMMU on all three ISAs, so this run proved the
    // confined configuration; say so if that stops being true. With the driver at EL0 the IOMMU is
    // the whole of what stops a compromised server reaching memory it was not given. This asks
    // whether the unit that is up owns the controller, not whether any is: with two VT-d units
    // those differ.
    assert!(
        disk.confined_by_iommu,
        "the NVMe server was wired without an IOMMU that owns it ({}); the confinement claim is \
         untested",
        disk.scope,
    );

    // SIZE: the server was told the geometry rather than allowed to ask, since asking means
    // IDENTIFY and IDENTIFY is admin. What it answers must be what the kernel read from IDENTIFY,
    // whatever disk this leg attached: 8 MiB for xtask's `mknvmedisk`, 256 GB on xenon.
    assert_eq!(
        disk.blk(filesystem_contract::blk::SIZE, 0) as u64,
        disk.size_bytes,
        "the server answered the wrong size"
    );

    // Two blocks, two patterns, each reading back its own: the second pattern, not zeros expected
    // from a neighbour, shows a write lands where it said, so nothing assumes the disk's prior
    // contents (milestone 318 (the NVMe boot test on real geometry)). Block 37 is far enough in
    // that a server confusing block and LBA units (the classic factor of 8) lands visibly
    // elsewhere; 38, its neighbour, catches a smear.
    const BLOCK: u64 = 37;
    // Both functions of the byte's offset, so a server that returns a constant, a shifted copy, or
    // the other block fails. 251 and 241 are prime to 4096, so neither aliases a page period.
    let first: fn(usize) -> u8 = |i| (i as u64 % 251) as u8;
    let second: fn(usize) -> u8 = |i| ((i as u64 % 241) as u8) ^ 0x5a;

    for (block, pattern) in [(BLOCK, first), (BLOCK + 1, second)] {
        // SAFETY: the blk contract's turn-taking. A request is a CALL, so this thread holds the
        // buffer exactly while the server is not running, and vice versa.
        for (i, b) in unsafe { disk.transfer_block() }.iter_mut().enumerate() {
            *b = pattern(i);
        }
        assert_eq!(
            disk.blk(filesystem_contract::blk::WRITE, block),
            0,
            "the server refused the write to block {block}"
        );
    }

    // Both writes are done before either read, which is what makes the first read load-bearing: if
    // block 38's write had gone everywhere, block 37 would come back holding `second`.
    for (block, pattern) in [(BLOCK, first), (BLOCK + 1, second)] {
        // Clobber the buffer first, so a read that never reached the device fails rather than
        // passing on what this thread just staged. SAFETY: as above.
        unsafe { disk.transfer_block() }.fill(0xaa);
        assert_eq!(
            disk.blk(filesystem_contract::blk::READ, block),
            0,
            "the server refused the read of block {block}"
        );
        // SAFETY: as above.
        for (i, b) in unsafe { disk.transfer_block() }.iter().enumerate() {
            assert_eq!(*b, pattern(i), "byte {i} of block {block} came back wrong");
        }
    }

    // A block outside the namespace is refused by the driver's own arithmetic
    // (`non_volatile_memory_express::Handoff`'s Kani-proved range check), not left to a controller
    // status nobody can attribute. `size / BLOCK_SIZE` is the first block past the end for any
    // size: it starts at the end or its transfer runs off it, and `holds_block` refuses both.
    let past_end = disk.size_bytes / crate::non_volatile_memory_express::BLOCK_SIZE as u64;
    assert!(
        disk.blk(filesystem_contract::blk::READ, past_end) < 0,
        "the server read block {past_end}, which a namespace of {} bytes does not have",
        disk.size_bytes,
    );

    // **FLUSH answers a count, and the count moves.** The contract's own falsifiability argument:
    // two syncs returning the same number would mean the second never reached the device. Unlike
    // the virtio server this one never answers EOPNOTSUPP, because NVMe 1.4 §6.8 makes Flush
    // mandatory.
    let first = disk.blk(filesystem_contract::blk::FLUSH, 0);
    assert!(first >= 1, "the server refused a flush: {first}");
    assert_eq!(
        disk.blk(filesystem_contract::blk::FLUSH, 0),
        first + 1,
        "a second flush returned the same count, so it never reached the controller"
    );

    // **An opcode the server has no verb for is refused, not guessed at.**
    assert!(disk.blk(0xfe, 0) < 0, "the server answered an unknown verb");
}

/// **Each client stages in the window its badge names, and the disk tree's cannot write**
/// (milestone 834 (ioping on nife and Linux); calef, 2026-10-18 23:06 UTC, "Per-client window").
/// One transfer window per client lets the filesystem server, the installer and the disk tree each
/// keep a request in flight; what it prevents is a write carrying another client's bytes. The
/// installer's write leaves window 0 untouched, and the disk tree's read lands only in its window.
/// Named to run before `a_confined_el0_server_cannot_dma_outside_its_region` (name order), whose
/// attacker leaves the controller silent so later requests time out `EIO`; `ensure` wires once.
///
/// Falsification: replayable `system_tests/falsifications/user.non_volatile_memory_express_tests.a_client_stages_in_the_window_its_badge_names.patch`
#[test_case]
fn a_client_stages_in_the_window_its_badge_names() {
    use filesystem_contract::blk;

    use crate::non_volatile_memory_express::{
        CLIENT_WINDOWS, DISK_TREE_WINDOW, FILESYSTEM_WINDOW, INSTALLER_WINDOW,
    };
    let Some(disk) = start() else {
        crate::testing::skip!("no NVMe controller came up (NIFE_NVME not set on this leg?)");
    };
    const BLOCK: u64 = 41;
    let staged = |w: u64| {
        // SAFETY: each window is a block or more of the server's transfer buffer, which the direct
        // map covers, and the server touches it only inside a CALL this thread is not making.
        unsafe {
            &mut *(crate::arch::mmu::phys_to_virt(disk.window_phys(w))
                as *mut [u8; crate::non_volatile_memory_express::BLOCK_SIZE])
        }
    };
    let call = |op, badge| {
        crate::sched::ipc_call_badged(disk.request, [blk::req(op, 1), BLOCK], badge)[0] as i64
    };
    let pattern = |i: usize| (i as u64 % 239) as u8 ^ 0x3c;

    staged(FILESYSTEM_WINDOW).fill(0x11);
    for (i, b) in staged(INSTALLER_WINDOW).iter_mut().enumerate() {
        *b = pattern(i);
    }
    assert_eq!(
        call(blk::WRITE, INSTALLER_WINDOW),
        0,
        "the installer's write was refused"
    );
    assert!(
        staged(FILESYSTEM_WINDOW).iter().all(|&b| b == 0x11),
        "the installer's write touched window 0's staging"
    );

    staged(DISK_TREE_WINDOW).fill(0xaa);
    assert_eq!(
        call(blk::READ, DISK_TREE_WINDOW),
        0,
        "the disk tree's read was refused"
    );
    for (i, b) in staged(DISK_TREE_WINDOW).iter().enumerate() {
        assert_eq!(
            *b,
            pattern(i),
            "byte {i} of the disk tree's read came back wrong"
        );
    }
    assert!(
        staged(FILESYSTEM_WINDOW).iter().all(|&b| b == 0x11),
        "the disk tree's read landed in window 0"
    );

    // **Read-only is the capability's, not the disk tree's code** (`can_write`).
    let erofs = filesystem_contract::reply_err(30);
    assert_eq!(
        call(blk::WRITE, DISK_TREE_WINDOW),
        erofs,
        "the disk tree's window wrote"
    );
    assert_eq!(
        call(blk::FLUSH, DISK_TREE_WINDOW),
        erofs,
        "the disk tree's window flushed"
    );
    // **A badge no client was given is refused**, not folded onto window 0.
    assert_eq!(
        call(blk::READ, CLIENT_WINDOWS as u64),
        filesystem_contract::reply_err(22),
        "a badge past the last window was served"
    );
}

/// **A confined EL0 NVMe server cannot DMA outside its region** (milestone 261's Outstanding item;
/// milestone 202 (every confinement test is a ritual until somebody breaks the confinement and
/// watches it fail)'s convention). An unprivileged server aims a PRP at a frame it was never given,
/// in both directions, and the IOMMU, not the server's arithmetic, is all that refuses it.
///
/// The proof is the IOMMU fault and an untouched canary, not the completion status; the patch
/// widens the domain over the victim and the escape lands. See the note's "The DMA escape test".
///
/// Falsification: replayable `system_tests/falsifications/user.non_volatile_memory_express_tests.a_confined_el0_server_cannot_dma_outside_its_region.patch`
#[test_case]
fn a_confined_el0_server_cannot_dma_outside_its_region() {
    let image = program("non_volatile_memory_express")
        .expect("no non_volatile_memory_express program in the initial ramdisk");

    // Drain any stale fault first, so what we observe is this test's.
    while crate::iommu::take_fault().is_some() {}

    let Some(esc) = non_volatile_memory_express_service::start_dma_escape(image) else {
        crate::testing::skip!("no NVMe controller came up (NIFE_NVME not set on this leg?)");
    };

    // The IOMMU must own the controller, or nothing refuses the escape and a pass is a fiction.
    assert!(
        esc.confined_by_iommu,
        "the NVMe controller is not owned by the IOMMU unit this kernel programmed ({}); the \
         escape would be refused by nothing, so the test cannot prove anything",
        esc.scope,
    );

    // The attacker reports once it has tried both escapes: word 0 the disk->victim (READ) status,
    // word 1 the victim->disk (WRITE) status. A confined controller never completes either, so
    // both are nonzero; they ride in the transcript, not the assertions.
    let [rd, wr, ..] = esc.wait();

    // **A server that served instead of attacking** sends the honest readiness word. That is an
    // initial ramdisk built without `components/confinement_attackers` (a boot outside `cargo xtask
    // test`), and it would otherwise surface below as "the IOMMU recorded no fault", blaming the
    // IOMMU for a build that never tried.
    assert_ne!(
        rd,
        filesystem_contract::READY,
        "the NVMe server reported ready rather than attacking: this initial ramdisk was built without \
         `components/confinement_attackers`, so it has no DMA-escape role. Run the suite through \
         `cargo xtask test`, which builds it in",
    );

    // **The evidence the controller actually tried the out-of-region DMA.** QEMU records the fault
    // as it processes the doorbell under TCG, so a bounded spin is plenty; the bound turns "no
    // fault ever" into a failure rather than a hang. Without this a pass would be consistent with
    // the controller never reaching the address at all, which is the hollow pass milestone 202
    // forbids.
    let mut fault = None;
    for _ in 0..2_000_000 {
        if let Some(f) = crate::iommu::take_fault() {
            fault = Some(f);
            break;
        }
        core::hint::spin_loop();
    }
    let f = fault.unwrap_or_else(|| {
        panic!(
            "an EL0 server aimed a PRP at {:#x}, outside its DMA region (read status {rd:#x}, \
             write status {wr:#x}), and the IOMMU recorded no fault: it is not confining the \
             controller in hardware",
            esc.victim,
        )
    });
    match f.addr {
        Some(addr) => assert_eq!(
            addr & !0xfff,
            esc.victim & !0xfff,
            "the IOMMU faulted, but on {addr:#x} (code {:#x}, rid {:#x}), not the victim frame \
             {:#x}",
            f.code,
            f.rid,
            esc.victim,
        ),
        // **AMD-Vi under QEMU 11.1.1 reports no address** (`arch/x86_64/iommu.rs`'s
        // `Fault::addr`). The fault is tied to the controller by its requester id instead, and the
        // canary below, which never depended on the fault queue, is what proves the READ escape
        // did not land.
        None => assert_eq!(
            f.rid, esc.rid,
            "the IOMMU faulted (code {:#x}) for requester {:#x}, not the NVMe controller {:#x}",
            f.code, f.rid, esc.rid,
        ),
    }

    // **And the frame is verifiably untouched.** A READ escape the IOMMU had failed to refuse would
    // have copied the disk's bytes over the canary. This is the direct memory proof, independent of
    // the fault queue, and it is the assertion the falsification's widened domain flips.
    assert!(
        esc.is_canary_intact(),
        "the victim frame at {:#x} was overwritten: the controller's DMA reached a frame outside \
         its region",
        esc.victim,
    );

    // Leave the fault queue as we found it: the RISC-V IOMMU's queue holds records a later test
    // would otherwise read as its own (the gpu escape test records the same).
    while crate::iommu::take_fault().is_some() {}

    // And the frames: the controller's DMA region, the victim and the report endpoint's region,
    // which the suite's frame ledger would otherwise carry to the end of the boot.
    assert!(
        esc.release(),
        "the DMA-escape wiring could not give its frames back: the attacker never exited, the \
         controller would not disable, or its endpoint region would not reclaim",
    );
}

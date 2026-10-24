use entropy_service::Bus;

use super::*;

/// Reach the service on `bus`, wiring it if this is the first test to ask, and check its
/// startup report when this call is the one that wired it.
///
/// Returns `None` if the bus has no virtio-rng device (milestone 145: correct on a bare board
/// boot, which has no equivalent of the runner's `NIFE_RNG`-gated `-device virtio-rng` line);
/// callers must `skip!()` at their own call site rather than have this helper do it, because
/// `skip!()` returns from its immediate caller and a helper is not the test.
fn start(bus: Bus) -> Option<entropy_service::Wiring> {
    let image = program("entropy").expect("no entropy program in the initial ramdisk");
    let w = entropy_service::ensure(image, bus)?;
    if let Some(report) = w.wait_for_ready() {
        assert_eq!(
            report[0],
            entropy_contract::READY,
            "the entropy service did not come up on {bus:?} (it reported {:#x}; a 0xDEAD_.. \
             word's low byte names the step, see components/src/entropy.rs)",
            report[0],
        );
        assert_eq!(
            report[1], 1,
            "the entropy service came up on {bus:?} but the device gave it no bytes at all",
        );
    }
    Some(w)
}

/// Draw `WORDS` eight-byte words through the request endpoint, asserting every draw is full.
/// Deliberately more than one bufferful (the service fetches 256 bytes per device request), so
/// this crosses the refill boundary and a cursor that wrapped instead of refilling shows up as
/// a repeat below.
const WORDS: usize = 64;

fn draw(w: &entropy_service::Wiring) -> [u64; WORDS] {
    let mut words = [0u64; WORDS];
    for (i, slot) in words.iter_mut().enumerate() {
        let mut buf = [0u8; 8];
        let n = w.get(8, &mut buf);
        assert_eq!(
            n, 8,
            "draw {i} of {WORDS} on {:?} returned {n} bytes, not 8: the device ran dry, or the \
             service failed to refill",
            w.bus,
        );
        *slot = u64::from_le_bytes(buf);
    }
    words
}

/// Every word distinct, and none of them zero. With a real source a collision among 64 draws is
/// a 2^-58 event, so a failure here is a bug rather than bad luck: a stuck device, a buffer
/// served twice, or a used ring the driver never re-read all present exactly this way.
fn assert_unpredictable(words: &[u64; WORDS], what: &str) {
    for (i, &a) in words.iter().enumerate() {
        assert_ne!(
            a, 0,
            "{what}: draw {i} is all zeros, which is the DMA page unwritten"
        );
        for (j, &b) in words.iter().enumerate().skip(i + 1) {
            assert_ne!(a, b, "{what}: draws {i} and {j} are identical ({a:#018x})");
        }
    }
}

/// **The headline, over virtio-mmio.** A client that holds one endpoint and no device gets
/// bytes off a real random-number generator, 512 of them, across a refill, all different.
#[test_case]
fn a_client_obtains_unpredictable_bytes_from_a_virtio_rng_over_mmio() {
    let Some(w) = start(Bus::Mmio) else {
        crate::testing::skip!("no virtio-rng device on the mmio bus (NIFE_RNG not set?)");
    };
    let words = draw(&w);
    assert_unpredictable(&words, "mmio");
}

/// **The same service, the same binary, over PCIe** (DECISIONS §18), and behind the IOMMU while
/// it is there. An entropy source's buffer is the one page in memory whose contents must not be
/// guessable, so an unconfined device writing it is worth asserting against rather than hoping.
#[test_case]
fn a_client_obtains_unpredictable_bytes_from_a_virtio_rng_over_pcie() {
    let Some(w) = start(Bus::Pci) else {
        skip_unless_required!(
            X86_RNG_ATTACHED,
            "no virtio-rng device on the pcie bus (NIFE_RNG not set?)"
        );
    };
    assert!(
        w.confined_by_iommu,
        "the PCIe RNG is present but not behind the IOMMU: the buffer the device writes the \
         system's key material into is unconfined (is iommu_platform=on missing from the \
         runner's virtio-rng-pci line?)",
    );
    let words = draw(&w);
    assert_unpredictable(&words, "pcie");
}

/// **The JH7110 backend refuses to wire on a machine that has no JH7110** (milestone 159), which
/// is every machine this suite boots. That is worth a test rather than a comment because the
/// failure it guards is silent: a wiring that spawned the driver anyway would hand a userspace
/// process a device mapping of an address nobody described, and the first symptom would be a load
/// fault (or worse, a read of some unrelated peripheral) on the one machine nobody can run this
/// suite on.
///
/// On a `StarFive` VisionFive 2 (radon) this test skips and the boot tour's `hw entropy` line is
/// what speaks instead; see `design/roadmap/0159-jh7110-trng-driver/bench-procedure.md` for the
/// bench procedure.
#[test_case]
fn the_jh7110_backend_refuses_to_wire_where_there_is_no_jh7110() {
    if entropy_service::jh7110_trng_device().is_some() {
        crate::testing::skip!(
            "this machine's tree describes a JH7110 TRNG (under either spelling): the boot \
             tour's hw entropy line is the test that matters here, not this one"
        );
    }
    let Some(image) = program("jh7110_entropy") else {
        crate::testing::skip!(
            "no jh7110_entropy program in this archive (every archive packs one since milestone 150)"
        );
    };
    assert!(
        entropy_service::ensure(image, Bus::Jh7110).is_none(),
        "the JH7110 entropy backend wired itself on a machine whose device tree describes no \
         such device: something spawned a driver holding a device mapping of an address nobody \
         named",
    );
}

/// No clock-and-reset window is mapped on a machine that is not a JH7110 (milestone 220).
///
/// `jh7110_clock_and_reset::discover` always produces an address, falling back to `0x1023_0000`,
/// so an unconditional caller would store there on every board; `memory::init` records the window
/// only when the tree names a JH7110, and this pins it. The failure is silent: a boot fault nobody
/// would blame on the clock driver. QEMU's `virt` has no clock or reset controller, so CI runs
/// only the absence path; the sequence itself has never run against silicon. See
/// `notes/jh7110-clock-and-reset.md`.
#[test_case]
fn no_clock_window_is_mapped_where_there_is_no_jh7110() {
    if entropy_service::jh7110_trng_device().is_some() {
        crate::testing::skip!(
            "this machine's tree describes a JH7110 TRNG, so a mapped clock window is correct \
             here; the boot tour's hw clock line is what speaks on that machine"
        );
    }
    assert!(
        entropy_service::jh7110_crg_window().is_none(),
        "a JH7110 clock-and-reset window was recorded on a machine whose device tree names no \
         JH7110: the constant fallback escaped its guard, and the next boot stores to an address \
         nobody described",
    );
}

/// **Two independent sources do not agree**, which is what says the bytes came from the devices
/// rather than from anything shared underneath them (a fixed seed, a counter, the DMA page's
/// previous contents). Also the cheapest proof that two services can hold two devices at once.
#[test_case]
fn two_entropy_services_on_two_devices_do_not_produce_the_same_bytes() {
    let Some(mmio) = start(Bus::Mmio) else {
        crate::testing::skip!("no virtio-rng device on the mmio bus (NIFE_RNG not set?)");
    };
    let Some(pci) = start(Bus::Pci) else {
        skip_unless_required!(
            X86_RNG_ATTACHED,
            "no virtio-rng device on the pcie bus (NIFE_RNG not set?)"
        );
    };
    let a = draw(&mmio);
    let b = draw(&pci);
    for (i, (&x, &y)) in a.iter().zip(b.iter()).enumerate() {
        assert_ne!(
            x, y,
            "draw {i} is identical on both devices ({x:#018x}): these are not two sources",
        );
    }
}

/// **The count in a reply is the truth about the reply.** A short request gets exactly that
/// many bytes and leaves the rest of the caller's buffer alone, so a caller cannot be handed
/// padding it mistakes for entropy; an oversized one is clamped and answered rather than
/// refused; and an opcode the service does not implement is answered with nothing rather than
/// killing the service, which the draw afterwards proves.
#[test_case]
fn a_reply_never_delivers_more_bytes_than_it_says() {
    let Some(w) = start(Bus::Mmio) else {
        crate::testing::skip!("no virtio-rng device on the mmio bus (NIFE_RNG not set?)");
    };

    let mut buf = [0xAAu8; 8];
    assert_eq!(w.get(3, &mut buf), 3, "asked for three bytes");
    assert_eq!(
        &buf[3..],
        &[0xAA; 5],
        "the service wrote past the count it reported",
    );

    let mut big = [0u8; 8];
    assert_eq!(
        w.get(200, &mut big),
        entropy_contract::MAX_BYTES as usize,
        "an oversized request should be clamped and answered, not refused",
    );

    let r = crate::sched::ipc_call(w.request, [entropy_contract::req(0xff, 8), 0]);
    assert_eq!(
        r[0],
        entropy_contract::NO_ENTROPY,
        "an unknown opcode should be answered with no bytes",
    );

    let mut after = [0u8; 8];
    assert_eq!(
        w.get(8, &mut after),
        8,
        "the service stopped serving after an unknown opcode",
    );
}

/// A fill of more than one exchange gathers across round trips (milestone 159 (a real hardware
/// entropy source: the JH7110's TRNG)).
///
/// `entropy_contract` carries [`entropy_contract::MAX_BYTES`] bytes per exchange, so a 32-byte
/// buffer takes four; `Wiring::fill` is the loop, run here against the backend CI has. A full
/// buffer comes back full, and two consecutive fills differ, which for 32 bytes means the service
/// crossed an internal refill boundary without repeating itself.
#[test_case]
fn a_fill_gathers_across_round_trips() {
    let Some(w) = start(Bus::Mmio) else {
        crate::testing::skip!("no virtio-rng device on the mmio bus (NIFE_RNG not set?)");
    };

    let mut a = [0u8; 32];
    let mut b = [0u8; 32];
    assert_eq!(
        w.fill(&mut a),
        32,
        "a 32-byte fill should gather four replies, not stop at the first",
    );
    assert_eq!(w.fill(&mut b), 32, "the second fill came up short");
    assert!(
        a.iter().any(|&x| x != 0),
        "a full fill of zeros is a dead source, not entropy",
    );
    assert_ne!(a, b, "two fills returned identical bytes");
}

/// The instruction backend (milestone 162 (real hardware entropy on x86_64 and aarch64: RDSEED
/// and RNDRRS)): `RNDRRS` on aarch64, with no virtio device at all.
///
/// It needs `ID_AA64ISAR0_EL1.RNDR` (`FEAT_RNG`, ARMv8.5), which the default `cortex-a72` lacks, so
/// it skips there; run it under TCG with `script/test --arch aarch64 --cpu neoverse-n2` or
/// `--cpu max`. Under HVF `helpers/qemu-runner-aarch64.sh` refuses `NIFE_CPU` (so Apple Silicon's
/// `FEAT_RNG` is not established), and CI's CPU matrix sweeps riscv64 only, so CI always takes the
/// skip. riscv64 has neither instruction ("Follow-on" in
/// `design/roadmap/0162-cpu-instruction-entropy.md`); `x86_64` checks
/// `arch::isa::get().has_random_seed_instruction()`. See notes/entropy.md, "Honest limits".
#[test_case]
fn a_client_obtains_unpredictable_bytes_from_rndrrs_with_no_device_at_all() {
    let Some(w) = start(Bus::Instruction) else {
        crate::testing::skip!(
            "no instruction-mode entropy source on this build (aarch64: ID_AA64ISAR0_EL1.RNDR is \
             clear, so no FEAT_RNG. riscv64: the ISA has neither instruction, refused on purpose \
             in design/roadmap/0162-cpu-instruction-entropy.md). --cpu neoverse-n2 reaches the \
             passing path under TCG only, no CI leg takes it, and the HVF leg cannot boot this \
             kernel at all today (milestone 227). Read this test's doc comment first"
        );
    };
    assert!(
        !w.confined_by_iommu,
        "an instruction has no device for an IOMMU to confine",
    );
    let words = draw(&w);
    assert_unpredictable(&words, "rndrrs");
}

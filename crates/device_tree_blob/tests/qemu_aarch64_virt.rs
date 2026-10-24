//! Parse a real device tree, dumped from the machine we actually boot on, on the host (§7
//! (testing: QEMU harness + host-testable crates)).
//!
//! Regenerate the fixture with the following; the `dtc` round-trip compacts QEMU's dump, which is
//! padded to a full megabyte:
//!
//!     qemu-system-aarch64 -machine virt,dumpdtb=f.dtb -cpu cortex-a72 -nographic
//!     dtc -I dtb -O dtb -o crates/device_tree_blob/tests/fixtures/qemu-aarch64-virt.dtb f.dtb

use device_tree_blob::{DeviceTreeBlob, Error, Region};

const QEMU_VIRT: &[u8] = include_bytes!("fixtures/qemu-aarch64-virt.dtb");

#[test]
fn parses_the_header() {
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT).expect("should parse");
    assert_eq!(dtb.total_size(), QEMU_VIRT.len());
}

#[test]
fn finds_the_ram() {
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT).unwrap();
    let mut regions = [Region { start: 0, size: 0 }; 8];
    let n = dtb.memory_regions(&mut regions).unwrap();

    assert_eq!(n, 1, "virt has one memory node");

    // The RAM base and size QEMU `virt` declares.
    assert_eq!(
        regions[0],
        Region {
            start: 0x4000_0000,
            size: 0x800_0000, // 128 MiB, QEMU's default
        }
    );
}

#[test]
fn qemu_virt_reserves_nothing() {
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT).unwrap();
    let mut regions = [Region { start: 0, size: 0 }; 8];

    // QEMU's virt leaves the reservation block empty; real firmware often does not.
    assert_eq!(dtb.reserved_regions(&mut regions).unwrap(), 0);
}

#[test]
fn rejects_junk() {
    // A pointer to something that isn't a device tree: the magic check is all that stops
    // the parser walking into random memory.
    match DeviceTreeBlob::from_bytes(&[0xffu8; 64]) {
        Err(Error::BadMagic(m)) => assert_eq!(m, 0xffff_ffff),
        other => panic!("expected BadMagic, got {other:?}"),
    }
    match DeviceTreeBlob::from_bytes(&[0u8; 64]) {
        Err(Error::BadMagic(0)) => {}
        other => panic!("expected BadMagic(0), got {other:?}"),
    }
}

#[test]
fn rejects_a_truncated_blob() {
    // Correct magic, but the blob is cut short. The header says it's 7566 bytes and
    // we only have 32. A parser that trusts the header here walks off the end of the
    // buffer, which in a kernel means reading whatever memory happens to follow.
    match DeviceTreeBlob::from_bytes(&QEMU_VIRT[..32]) {
        Err(Error::Truncated) => {}
        other => panic!("expected Truncated, got {other:?}"),
    }
}

#[test]
fn refuses_to_overflow_the_callers_slice() {
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT).unwrap();
    let mut too_small: [Region; 0] = [];
    match dtb.memory_regions(&mut too_small) {
        Err(Error::TooManyRegions) => {}
        other => panic!("expected TooManyRegions, got {other:?}"),
    }
}

// --- /chosen and the initial ramdisk ---

const QEMU_VIRT_INITIAL_RAMDISK: &[u8] =
    include_bytes!("fixtures/qemu-aarch64-virt-initial-ramdisk.dtb");

#[test]
fn finds_the_initial_ramdisk() {
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT_INITIAL_RAMDISK).unwrap();
    let initial_ramdisk = dtb
        .initial_ramdisk()
        .unwrap()
        .expect("this fixture has an initial ramdisk");

    // Nothing else protects the ramdisk the bootloader loaded: the kernel must reserve this range
    // before the frame allocator hands it out.
    assert_eq!(initial_ramdisk.start, 0x4400_0000);
    assert_eq!(initial_ramdisk.end(), 0x4403_0d40);
    assert_eq!(
        initial_ramdisk.size, 200_000,
        "the fixture was made from a 200 KB file"
    );
}

#[test]
fn no_initial_ramdisk_is_not_an_error() {
    // The common case: nobody passed -initrd. `None`, not an error, and definitely not a
    // zero-length region that the allocator then tries to reason about.
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT).unwrap();
    assert_eq!(dtb.initial_ramdisk().unwrap(), None);
}

#[test]
fn an_initial_ramdisk_does_not_disturb_the_memory_map() {
    // Regression guard: the /chosen walk and the /memory walk are separate passes over
    // the same token stream. A bug in one must not corrupt the other.
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT_INITIAL_RAMDISK).unwrap();
    let mut regions = [Region { start: 0, size: 0 }; 8];
    assert_eq!(dtb.memory_regions(&mut regions).unwrap(), 1);
    assert_eq!(regions[0].start, 0x4000_0000);
}

// --- finding a device by name ---

#[test]
fn finds_the_interrupt_controller() {
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT).unwrap();
    let mut regs = [Region { start: 0, size: 0 }; 4];

    let n = dtb.node_reg(b"intc@", &mut regs).unwrap();

    // The GIC has TWO register blocks, and the order is part of the binding.
    assert_eq!(
        n, 2,
        "the GIC should have a distributor and a CPU interface"
    );

    assert_eq!(
        regs[0],
        Region {
            start: 0x0800_0000,
            size: 0x1_0000
        }
    ); // GICD, distributor
    assert_eq!(
        regs[1],
        Region {
            start: 0x0801_0000,
            size: 0x1_0000
        }
    ); // GICC, CPU interface
}

#[test]
fn finds_the_uart_the_console_hardcodes() {
    // The console hardcodes 0x0900_0000 on purpose (it must come up before the DTB parser
    // exists to debug it). But now we can *check* the hardcode against what the machine says,
    // which is the cross-check that catches the day it changes.
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT).unwrap();
    let mut regs = [Region { start: 0, size: 0 }; 2];

    assert_eq!(dtb.node_reg(b"pl011@", &mut regs).unwrap(), 1);
    assert_eq!(regs[0].start, 0x0900_0000);
}

#[test]
fn a_missing_node_is_zero_regions_not_an_error() {
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT).unwrap();
    let mut regs = [Region { start: 0, size: 0 }; 2];
    assert_eq!(dtb.node_reg(b"nonesuch@", &mut regs).unwrap(), 0);
}

#[test]
fn from_ptr_agrees_with_from_bytes() {
    // The boot path's actual entry: the machine hands the kernel a bare pointer, and from_ptr
    // must learn the blob's length from the blob itself before it can build the checked slice
    // that everything else parses. Prove the pointer path lands on the same tree.
    // SAFETY: `QEMU_VIRT` is a `'static` blob compiled into the test binary, so the pointer is valid for any `'a`, which is exactly what `from_ptr` asks for.
    let via_ptr = unsafe { DeviceTreeBlob::from_ptr(QEMU_VIRT.as_ptr()) }.expect("should parse");
    assert_eq!(via_ptr.total_size(), QEMU_VIRT.len());

    let mut a = [Region { start: 0, size: 0 }; 4];
    let mut b = [Region { start: 0, size: 0 }; 4];
    DeviceTreeBlob::from_bytes(QEMU_VIRT)
        .unwrap()
        .memory_regions(&mut a)
        .unwrap();
    via_ptr.memory_regions(&mut b).unwrap();
    assert_eq!(a, b);
}

#[test]
fn no_reserved_memory_node_is_zero_regions() {
    // aarch64 virt has no /reserved-memory node (that path matters on riscv, where OpenSBI
    // adds one; see qemu_riscv64_virt.rs). Walking the whole tree and finding none must be
    // zero regions, not an error: this is the answer the aarch64 boot gets every time.
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT).unwrap();
    let mut regs = [Region { start: 0, size: 0 }; 2];
    assert_eq!(dtb.reserved_memory_regions(&mut regs).unwrap(), 0);
}

/// **The RTC, found by binding rather than by label** (milestone 51). The aarch64 `virt` board
/// calls the node `pl031@9010000` and the RISC-V one calls its RTC `rtc@101000`, so no single name
/// prefix finds both and a driver that matched on the name would silently find nothing on the other
/// ISA. `compatible` is what the two boards agree to disagree about honestly.
///
/// Two properties of the parser ride along here. The node declares
/// `compatible = "arm,pl031", "arm,primecell"`, so matching it at all proves the NUL-separated list
/// is split rather than compared whole. And its `reg` is written **before** its `compatible`, which
/// is why the decode has to wait for the node to close instead of deciding at the `reg` property.
#[test]
fn finds_the_rtc_by_compatible() {
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT).unwrap();
    let mut regs = [Region { start: 0, size: 0 }; 2];

    assert_eq!(dtb.node_reg_compatible(b"arm,pl031", &mut regs).unwrap(), 1);
    assert_eq!(
        regs[0],
        Region {
            start: 0x0901_0000,
            size: 0x1000,
        }
    );
}

/// The RISC-V RTC is not on this board, and asking for it must be an honest zero rather than an
/// error or a stray match. This is the answer the aarch64 boot gets when it probes for both.
#[test]
fn the_goldfish_rtc_is_absent_on_aarch64() {
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT).unwrap();
    let mut regs = [Region { start: 0, size: 0 }; 2];
    assert_eq!(
        dtb.node_reg_compatible(b"google,goldfish-rtc", &mut regs)
            .unwrap(),
        0
    );
}

/// **`interrupt-parent` lives on the ROOT of this tree**, one declaration for every device on the
/// machine; the pl011 itself does not carry it. So `node_prop` honestly answers `None`, and only
/// the inherited read can decode any `interrupts` on this board at all. The parent it finds is
/// the GIC, whose `#interrupt-cells = <3>` is what makes the pl011's `<0 1 4>` mean SPI 1.
#[test]
fn inherits_the_interrupt_parent_from_the_root() {
    let dtb = DeviceTreeBlob::from_bytes(QEMU_VIRT).unwrap();

    assert_eq!(
        dtb.node_prop(b"pl011@", b"interrupt-parent").unwrap(),
        None,
        "the node itself does not say; the root does"
    );

    let parent = dtb
        .node_prop_inherited(b"pl011@", b"interrupt-parent")
        .unwrap()
        .expect("inherited from the root");
    let phandle = u32::from_be_bytes(parent[..4].try_into().unwrap());

    let cells = dtb
        .phandle_prop(phandle, b"#interrupt-cells")
        .unwrap()
        .expect("the GIC declares its interrupt cells");
    assert_eq!(
        u32::from_be_bytes(cells[..4].try_into().unwrap()),
        3,
        "a GIC entry is <type number flags>"
    );
}

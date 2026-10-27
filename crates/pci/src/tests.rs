use super::*;

/// A fake config space: a virtio-blk at 00:01.0 with two BARs and a capability list, a
/// multifunction device at 00:02, a device whose first BAR is 64-bit at 00:03, and empty
/// slots everywhere else. Offsets are byte addresses into per-function 4 KB pages.
struct FakeCfg {
    space: std::collections::HashMap<(u8, u8, u8, u64), u32>,
}

impl FakeCfg {
    fn new() -> Self {
        let mut s = std::collections::HashMap::new();
        let f = (0u8, 1u8, 0u8);
        // vendor 0x1af4, device 0x1042 (modern virtio-blk)
        s.insert((f.0, f.1, f.2, 0x00), 0x1042_1af4u32);
        // command 0, status: capability list present
        s.insert((f.0, f.1, f.2, 0x04), (0x0010u32) << 16);
        // header type 0, single-function
        s.insert((f.0, f.1, f.2, 0x0c), 0);
        // BAR0: 32-bit memory at 0x4000_0000 (assigned), size 0x1000
        s.insert((f.0, f.1, f.2, 0x10), 0x4000_0000);
        // BAR4: 64-bit memory, unassigned (base 0), size 0x4000
        s.insert((f.0, f.1, f.2, 0x20), 0b100);
        s.insert((f.0, f.1, f.2, 0x24), 0);
        // capability list: 0x40 -> vendor cap (common, bar 4, off 0x0, len 0x1000)
        //                  0x50 -> vendor cap (notify, bar 4, off 0x3000, len 0x1000, mult 4;
        //                          the notify cap is 20 bytes, so its multiplier sits at 0x60)
        //                  0x70 -> MSI-X cap (id 0x11), to prove non-vendor caps are skipped
        s.insert((f.0, f.1, f.2, 0x34), 0x40);
        s.insert(
            (f.0, f.1, f.2, 0x40),
            u32::from_le_bytes([0x09, 0x50, 16, VIRTIO_CAPABILITY_COMMON]),
        );
        s.insert((f.0, f.1, f.2, 0x44), 4); // bar 4
        s.insert((f.0, f.1, f.2, 0x48), 0x0); // offset
        s.insert((f.0, f.1, f.2, 0x4c), 0x1000); // length
        s.insert(
            (f.0, f.1, f.2, 0x50),
            u32::from_le_bytes([0x09, 0x70, 20, VIRTIO_CAPABILITY_NOTIFY]),
        );
        s.insert((f.0, f.1, f.2, 0x54), 4);
        s.insert((f.0, f.1, f.2, 0x58), 0x3000);
        s.insert((f.0, f.1, f.2, 0x5c), 0x1000);
        s.insert((f.0, f.1, f.2, 0x60), 4); // notify_off_multiplier
        // 0x70 -> MSI-X (id 0x11), end of list. Message Control 0x0002, which is a table of
        // THREE entries: the field holds the count minus one. Table BIR 1, offset 0x2000.
        s.insert(
            (f.0, f.1, f.2, 0x70),
            u32::from_le_bytes([0x11, 0x00, 0x02, 0x00]),
        );
        s.insert((f.0, f.1, f.2, 0x74), 0x2000 | 1);
        s.insert((f.0, f.1, f.2, 0x78), 0x2800 | 1); // the pending-bit array, same BAR

        // 00:01.1 answers, and must never be enumerated: the header at 00:01.0 says
        // single-function. A device that aliases its config space across all eight functions
        // is why that bit is checked instead of probing all eight and believing the answers.
        s.insert((0, 1, 1, 0x00), 0x1042_1af4);

        // 00:02.0, multifunction (header type bit 7 set): virtio-net at function 0 and
        // virtio-rng at function 1, functions 2..8 empty so the per-function "nobody home"
        // check has something to reject.
        s.insert((0, 2, 0, 0x00), 0x1041_1af4);
        s.insert((0, 2, 0, 0x0c), 0x0080_0000); // header type is byte 0x0e of this dword
        s.insert((0, 2, 1, 0x00), 0x1044_1af4);

        // 00:03.0, a virtio-gpu whose FIRST BAR is 64-bit and assigned. The blk device's
        // 64-bit BAR is the last pair and unassigned, so nothing there shows the walk
        // resuming two slots on, or a base whose high half is nonzero.
        s.insert((0, 3, 0, 0x00), 0x1050_1af4);
        s.insert((0, 3, 0, 0x0c), 0);
        s.insert((0, 3, 0, 0x10), 0xc000_0004); // 64-bit memory BAR, base 0x1_c000_0000
        s.insert((0, 3, 0, 0x14), 0x0000_0001);

        FakeCfg { space: s }
    }

    fn read32(&mut self, bdf: Bdf, off: u64) -> u32 {
        // Empty slots float high: all-ones, the hardware's "nobody home".
        *self
            .space
            .get(&(bdf.bus, bdf.dev, bdf.func, off & !3))
            .unwrap_or(&u32::MAX)
    }

    /// BAR writes emulate the size probe: all-ones writes read back as the size mask.
    fn write32(&mut self, bdf: Bdf, off: u64, v: u32) {
        let key = (bdf.bus, bdf.dev, bdf.func, off & !3);
        if !self.space.contains_key(&key) {
            return;
        }
        // Keyed on the device too: 00:01 and 00:03 both have a BAR at 0x10 and they are
        // different sizes.
        let v = match (bdf.dev, off & !3) {
            (1, 0x10) if v == u32::MAX => !(0x1000u32 - 1), // BAR0: size 0x1000
            (1, 0x20) if v == u32::MAX => (!(0x4000u32 - 1)) | 0b100, // BAR4 low: size 0x4000, keep type bits
            (1, 0x24) if v == u32::MAX => u32::MAX, // BAR4 high: all bits writable
            (3, 0x10) if v == u32::MAX => (!(0x2000u32 - 1)) | 0b100, // BAR0 low: size 0x2000
            (3, 0x14) if v == u32::MAX => u32::MAX, // BAR0 high
            _ => v,
        };
        self.space.insert(key, v);
    }
}

/// ECAM addressing is a pure shift-and-or, and these three points pin the layout: function
/// stride 4 KB, device stride 32 KB, bus stride 1 MB.
#[test]
fn ecam_offsets_have_the_specified_strides() {
    assert_eq!(
        Bdf {
            bus: 0,
            dev: 0,
            func: 1
        }
        .ecam_offset(),
        0x1000
    );
    assert_eq!(
        Bdf {
            bus: 0,
            dev: 1,
            func: 0
        }
        .ecam_offset(),
        0x8000
    );
    assert_eq!(
        Bdf {
            bus: 1,
            dev: 0,
            func: 0
        }
        .ecam_offset(),
        0x10_0000
    );
}

/// The requester id is `bus:8 | dev:5 | fn:3`, the id the IOMMU keys per-device tables on
/// (milestone 16b). A different packing than `ecam_offset`, so it gets its own witness.
#[test]
fn requester_id_packs_bus_dev_fn() {
    assert_eq!(
        Bdf {
            bus: 0,
            dev: 0,
            func: 0
        }
        .requester_id(),
        0
    );
    assert_eq!(
        Bdf {
            bus: 0,
            dev: 1,
            func: 0
        }
        .requester_id(),
        0x08
    );
    assert_eq!(
        Bdf {
            bus: 0,
            dev: 0,
            func: 1
        }
        .requester_id(),
        0x01
    );
    assert_eq!(
        Bdf {
            bus: 0,
            dev: 2,
            func: 3
        }
        .requester_id(),
        0x13
    );
    assert_eq!(
        Bdf {
            bus: 1,
            dev: 0,
            func: 0
        }
        .requester_id(),
        0x100
    );
}

/// Enumeration reports every function that answers and nothing else. The three properties
/// come as one walk because they only exist together: empty slots (vendor 0xffff) are
/// skipped, the multifunction device's function 1 is found, and the single-function device's
/// function 1 is never probed even though this fixture would answer for it.
#[test]
fn enumeration_follows_the_multifunction_bit_and_skips_what_does_not_answer() {
    let mut cfg = FakeCfg::new();
    let mut found = Vec::new();
    enumerate(
        256,
        &mut |b, o| cfg.read32(b, o),
        &mut |bdf, vendor, device| found.push((bdf, vendor, device)),
    );
    assert_eq!(
        found,
        vec![
            (
                Bdf {
                    bus: 0,
                    dev: 1,
                    func: 0
                },
                VIRTIO_VENDOR,
                VIRTIO_BLK_MODERN
            ),
            (
                Bdf {
                    bus: 0,
                    dev: 2,
                    func: 0
                },
                VIRTIO_VENDOR,
                VIRTIO_NET_MODERN
            ),
            (
                Bdf {
                    bus: 0,
                    dev: 2,
                    func: 1
                },
                VIRTIO_VENDOR,
                VIRTIO_RNG_MODERN
            ),
            (
                Bdf {
                    bus: 0,
                    dev: 3,
                    func: 0
                },
                VIRTIO_VENDOR,
                VIRTIO_GPU_MODERN
            ),
        ],
    );
}

/// **A machine shaped like xenon**: a host bridge and a PCIe root port on bus 0, and the NVMe
/// controller behind the root port on bus 1. This is the topology milestone 320 exists for, and
/// it is the one QEMU's `q35` does not have by default, so it is built here.
///
/// The ids are the real ones off `bench/xenon-2026-09-17/`: an Intel Kaby Lake host bridge, an
/// Intel 200-series root port, and a Micron 2450 in the M.2 slot. Nothing depends on them being
/// those ids; they are here so a reader meets the machine rather than a fixture.
struct BridgedCfg {
    space: std::collections::HashMap<(u8, u8, u8, u64), u32>,
}

impl BridgedCfg {
    /// `class` is the 24-bit class triple; `header` the raw header-type byte (bit 7 sets
    /// multifunction).
    fn function(&mut self, bdf: (u8, u8, u8), ids: u32, class: u32, header: u32) {
        self.space.insert((bdf.0, bdf.1, bdf.2, VENDOR_ID), ids);
        self.space
            .insert((bdf.0, bdf.1, bdf.2, CLASS_REVISION), class << 8);
        self.space
            .insert((bdf.0, bdf.1, bdf.2, HEADER_TYPE & !3), header << 16);
    }

    fn new() -> Self {
        let mut cfg = BridgedCfg {
            space: std::collections::HashMap::new(),
        };
        cfg.function((0, 0, 0), 0x591f_8086, 0x06_00_00, 0x00);
        cfg.function((0, 0x1d, 0), 0xa334_8086, 0x06_04_00, 0x01);
        // primary 0, secondary 1, subordinate 2: the subtree is buses 1 and 2, and only bus 1
        // has anything on it, which is the ordinary shape (firmware leaves headroom).
        cfg.space.insert((0, 0x1d, 0, BUS_NUMBERS), 0x00_02_01_00);
        cfg.function((1, 0, 0), 0x51b2_1344, CLASS_NVME, 0x00);
        cfg
    }

    fn read32(&self, bdf: Bdf, off: u64) -> u32 {
        *self
            .space
            .get(&(bdf.bus, bdf.dev, bdf.func, off & !3))
            .unwrap_or(&u32::MAX)
    }
}

/// **The walk follows a root port onto the bus behind it and finds the controller there.**
/// This is milestone 320's whole claim in one test: before it, enumeration scanned bus 0 and
/// reported the machine as having no NVMe at all.
#[test]
fn the_walk_follows_a_root_port_onto_the_bus_behind_it() {
    let cfg = BridgedCfg::new();
    let mut found = Vec::new();
    walk(256, &mut |b, o| cfg.read32(b, o), &mut |f| found.push(f));

    let buses: Vec<u8> = found.iter().map(|f| f.bdf.bus).collect();
    assert_eq!(buses, vec![0, 0, 1], "bus 1 was reached through the bridge");

    let port = found[1];
    assert_eq!(port.header_type, HEADER_TYPE_BRIDGE);
    assert_eq!(
        port.bridge,
        Some(BridgeBuses {
            primary: 0,
            secondary: 1,
            subordinate: 2,
        })
    );

    let nvme = found[2];
    assert_eq!(nvme.class, CLASS_NVME, "the controller is on bus 1");
    assert_eq!(
        nvme.bdf,
        Bdf {
            bus: 1,
            dev: 0,
            func: 0
        }
    );
    assert_eq!(nvme.bridge, None, "an endpoint states no buses behind it");
}

/// **The ceiling stops the descent without hiding the bridge.** A kernel that has mapped only
/// bus 0 of configuration space must not read bus 1, and must still be able to *say* that there
/// is a subtree it cannot see: the bridge's own `Function` carries the bus numbers either way,
/// which is what lets `kernel/src/pci.rs` print the gap rather than enumerate silently short.
#[test]
fn a_bus_above_the_ceiling_is_reported_but_never_read() {
    let cfg = BridgedCfg::new();
    let mut found = Vec::new();
    let mut touched_bus_1 = false;
    walk(
        1,
        &mut |b, o| {
            touched_bus_1 |= b.bus != 0;
            cfg.read32(b, o)
        },
        &mut |f| found.push(f),
    );

    assert!(!touched_bus_1, "the walk read a bus outside the ceiling");
    assert_eq!(found.len(), 2, "the two functions on bus 0");
    assert_eq!(
        found[1].bridge.unwrap().secondary,
        1,
        "the bridge still says where its subtree is"
    );
}

/// **Bus numbers come from firmware and can name anything, including a loop.** A bridge on bus
/// 1 that claims bus 1 is behind it, and one on bus 1 claiming bus 0, are both firmware bugs
/// this kernel cannot fix and must not hang on. The visited set is what makes each bus scanned
/// once; without it this recurses until the stack ends.
#[test]
fn a_cycle_in_the_bus_numbering_terminates() {
    let mut cfg = BridgedCfg::new();
    // 01:01.0: a bridge whose secondary bus is 1, the bus it is itself on.
    cfg.function((1, 1, 0), 0xdead_beef, 0x06_04_00, 0x01);
    cfg.space.insert((1, 1, 0, BUS_NUMBERS), 0x00_01_01_00);
    // 01:02.0: a bridge pointing back at bus 0, the root.
    cfg.function((1, 2, 0), 0xdead_beee, 0x06_04_00, 0x01);
    cfg.space.insert((1, 2, 0, BUS_NUMBERS), 0x00_02_00_01);

    let mut n = 0usize;
    walk(256, &mut |b, o| cfg.read32(b, o), &mut |_| n += 1);
    assert_eq!(n, 5, "each bus scanned once: 2 on bus 0, 3 on bus 1");
}

/// **A bridge firmware never configured reads zeros, and zero is not a route to the root.**
/// An unconfigured bridge states secondary 0; treating that as a subtree would report bus 0's
/// functions a second time if the visited set ever changed, and reports a subtree that does not
/// exist either way. It is an endpoint-shaped answer to a bridge-shaped question, so the walk
/// reports the bridge and descends nowhere.
#[test]
fn an_unconfigured_bridge_states_no_subtree() {
    let mut cfg = BridgedCfg::new();
    cfg.space.insert((0, 0x1d, 0, BUS_NUMBERS), 0);
    let mut found = Vec::new();
    walk(256, &mut |b, o| cfg.read32(b, o), &mut |f| found.push(f));
    assert_eq!(found.len(), 2, "nothing behind an unconfigured bridge");
    assert_eq!(found[1].bridge.unwrap().secondary, 0);
}

/// An empty slot costs exactly one config read, which is the entire job of the vendor-id
/// check on function 0. Without it every empty slot on the bus also pays a header-type read
/// and eight per-function reads, and the walk still returns the right answer, so read count
/// is the only thing that can see the guard.
#[test]
fn an_empty_slot_costs_one_config_read() {
    let mut cfg = FakeCfg::new();
    let mut reads = 0usize;
    enumerate(
        1,
        &mut |b, o| {
            reads += 1;
            cfg.read32(b, o)
        },
        &mut |_, _, _| {},
    );
    // 29 empty slots at one read each, and then the three devices that answer. A device costs
    // two reads before its functions (the function-0 vendor id, then the header type that says
    // whether functions 1..8 exist), and a function that answers costs three (vendor id, header
    // type, class); a function that does not answer costs only its vendor id.
    //
    // So: blk 2 + 3, the multifunction device 2 + 3 + 3 for its two functions and 6 for the six
    // empty ones, gpu 2 + 3.
    assert_eq!(reads, 29 + (2 + 3) + (2 + 3 + 3 + 6) + (2 + 3));
}

/// The BAR probe decodes an assigned 32-bit BAR and an UNassigned 64-bit BAR (base 0), sizes
/// both by the write-ones dance, marks widths, and restores the original values (the probe
/// must not leave the device reprogrammed).
#[test]
fn bars_are_sized_and_restored() {
    let cfg = std::cell::RefCell::new(FakeCfg::new());
    let bdf = Bdf {
        bus: 0,
        dev: 1,
        func: 0,
    };
    let bars = read_bars(
        bdf,
        &mut |b, o| cfg.borrow_mut().read32(b, o),
        &mut |b, o, v| cfg.borrow_mut().write32(b, o, v),
    );
    let mut cfg = cfg.into_inner();

    assert_eq!(
        bars[0],
        Some(Bar {
            base: 0x4000_0000,
            size: 0x1000,
            is_64: false
        })
    );
    assert_eq!(
        bars[4],
        Some(Bar {
            base: 0,
            size: 0x4000,
            is_64: true
        })
    );
    assert_eq!(bars[1], None);
    assert_eq!(
        bars[5], None,
        "the upper half of a 64-bit BAR is not its own BAR"
    );

    // Restored: originals read back, not the probe's all-ones.
    assert_eq!(cfg.read32(bdf, 0x10), 0x4000_0000);
    assert_eq!(cfg.read32(bdf, 0x20), 0b100);
}

/// A 64-bit BAR in the *first* slot: the base is assembled from both halves, the upper half
/// is not a BAR of its own, and the walk stops after six slots. None of the three is visible
/// on the blk device, whose 64-bit BAR is unassigned and sits in the last pair, so a base
/// that ignored its high half and a walk that ran one slot long both read as correct there.
#[test]
fn a_64_bit_bar_in_the_first_slot_keeps_its_high_half() {
    let cfg = std::cell::RefCell::new(FakeCfg::new());
    let bdf = Bdf {
        bus: 0,
        dev: 3,
        func: 0,
    };
    // A high-water mark rather than a log of every access: the mutants that make this walk
    // run forever are caught by a timeout, and a growing Vec would turn that into an OOM.
    let highest = std::cell::Cell::new(0u64);
    let bars = read_bars(
        bdf,
        &mut |b, o| {
            highest.set(highest.get().max(o));
            cfg.borrow_mut().read32(b, o)
        },
        &mut |b, o, v| {
            highest.set(highest.get().max(o));
            cfg.borrow_mut().write32(b, o, v);
        },
    );

    assert_eq!(
        bars[0],
        Some(Bar {
            base: 0x1_c000_0000,
            size: 0x2000,
            is_64: true
        })
    );
    assert_eq!(
        bars[1], None,
        "the upper half of a 64-bit BAR is not its own BAR"
    );

    // A type-0 header has six BAR slots, 0x10 through 0x24. 0x28 is the Cardbus CIS pointer,
    // and a walk that ran one slot long would size that pointer as if it were a BAR.
    assert_eq!(
        highest.get(),
        BAR0 + 20,
        "the probe touched a register outside the six BAR slots"
    );
}

/// The capability walk yields the two virtio vendor capabilities with their (bar, offset,
/// length) and the notify multiplier, and skips the MSI-X capability in the same list.
#[test]
fn the_capability_walk_finds_virtio_vendor_capabilities_only() {
    let mut cfg = FakeCfg::new();
    let bdf = Bdf {
        bus: 0,
        dev: 1,
        func: 0,
    };
    let mut capabilities = Vec::new();
    virtio_capabilities(bdf, &mut |b, o| cfg.read32(b, o), &mut |c| {
        capabilities.push(c);
    });

    assert_eq!(
        capabilities.len(),
        2,
        "exactly the two vendor caps, MSI-X skipped"
    );
    assert_eq!(capabilities[0].cfg_type, VIRTIO_CAPABILITY_COMMON);
    assert_eq!(
        (
            capabilities[0].bar,
            capabilities[0].offset,
            capabilities[0].length
        ),
        (4, 0, 0x1000)
    );
    assert_eq!(capabilities[1].cfg_type, VIRTIO_CAPABILITY_NOTIFY);
    assert_eq!((capabilities[1].bar, capabilities[1].offset), (4, 0x3000));
    assert_eq!(capabilities[1].notify_off_multiplier, 4);
}

/// A function whose status register does not advertise a capability list is not walked. The
/// capability pointer at 0x34 is undefined on such a function, so following it decodes
/// whatever bytes happen to sit there as a capability chain. This fixture keeps its real
/// chain in place and only clears the status bit, which is the case that tells the two
/// apart.
#[test]
fn a_function_without_a_capability_list_is_not_walked() {
    let mut cfg = FakeCfg::new();
    let bdf = Bdf {
        bus: 0,
        dev: 1,
        func: 0,
    };
    cfg.space.insert((0, 1, 0, COMMAND), 0);
    let mut capabilities = Vec::new();
    virtio_capabilities(bdf, &mut |b, o| cfg.read32(b, o), &mut |c| {
        capabilities.push(c);
    });
    assert!(
        capabilities.is_empty(),
        "walked a list the device does not claim"
    );
}

/// A capability list that loops must terminate, not hang: hostile or broken hardware gets a
/// bounded walk, the same discipline as the virtqueue chain walk.
#[test]
fn a_cyclic_capability_list_terminates() {
    let mut cfg = FakeCfg::new();
    let bdf = Bdf {
        bus: 0,
        dev: 1,
        func: 0,
    };
    // Point the second cap's `next` back at the first.
    cfg.space.insert(
        (0, 1, 0, 0x50),
        u32::from_le_bytes([0x09, 0x40, 20, VIRTIO_CAPABILITY_NOTIFY]),
    );
    let mut n = 0;
    virtio_capabilities(bdf, &mut |b, o| cfg.read32(b, o), &mut |_| n += 1);
    assert!(n <= 64, "the walk did not terminate promptly on a cycle");
}

/// The INTx swizzle, pinned: on QEMU virt the PCI interrupt base is 32 (riscv) and the four lines
/// rotate by device. Device 1 pin INTA is base+1; device 4 wraps back to base+0.
#[test]
fn the_intx_swizzle_rotates_by_device() {
    assert_eq!(intx_interrupt_number(32, 1, 1), 33);
    assert_eq!(intx_interrupt_number(32, 2, 1), 34);
    assert_eq!(intx_interrupt_number(32, 4, 1), 32);
    assert_eq!(
        intx_interrupt_number(32, 1, 2),
        34,
        "pin B advances the rotation too"
    );
}

/// The two command-register bits, pinned to their spec positions. This crate never reads
/// them back (the kernel writes them into the register), so a wrong value here surfaces as a
/// device that silently never decodes memory or never gets to DMA, with nothing in the decode
/// path to catch it.
#[test]
fn the_command_bits_are_the_specified_positions() {
    assert_eq!(CMD_MEMORY_SPACE, 0x0002);
    assert_eq!(CMD_BUS_MASTER, 0x0004);
}

/// **A modern virtio PCI device id is `0x1040 + the virtio device type`**, which is what lets
/// the kernel hand a driver a truthful `DeviceID` over the PCI transport instead of a
/// hardcoded one. Every id we drive is pinned against that derivation, so a typo in one
/// of them is a build-time-cheap test failure rather than a device we quietly never find.
/// Build a `ranges` blob from 7-cell entries, the way a device tree stores it.
fn ranges(entries: &[[u32; 7]]) -> Vec<u8> {
    entries
        .iter()
        .flatten()
        .flat_map(|c| c.to_be_bytes())
        .collect()
}

/// The QEMU riscv `virt` ranges verbatim (IO, 32-bit memory, 64-bit memory): the parser must
/// step past the IO entry, take the mem32 one, and never reach the mem64 one. The fixture
/// test in `tests/qemu_virt_dtb.rs` holds the same claim against the real tree.
#[test]
fn the_mem32_entry_is_found_among_its_neighbours() {
    let r = ranges(&[
        [0x0100_0000, 0, 0, 0, 0x0300_0000, 0, 0x1_0000],
        [0x0200_0000, 0, 0x4000_0000, 0, 0x4000_0000, 0, 0x4000_0000],
        [0x0300_0000, 4, 0, 4, 0, 4, 0],
    ]);
    assert_eq!(mem32_window(&r), Some((0x4000_0000, 0x4000_0000)));
}

/// A prefetchable 32-bit entry is not the window BARs are placed in; a tree that states only
/// that answers `None` rather than a window the non-prefetchable BARs should not land in.
#[test]
fn a_prefetchable_window_is_not_taken() {
    let r = ranges(&[[0x4200_0000, 0, 0x4000_0000, 0, 0x4000_0000, 0, 0x4000_0000]]);
    assert_eq!(mem32_window(&r), None);
}

/// A translated window (PCI address != CPU address) is refused, per the doc: the kernel
/// writes one number into both the BAR and its page tables, so it cannot honor one yet.
#[test]
fn a_translated_window_is_refused() {
    let r = ranges(&[[0x0200_0000, 0, 0x0000_0000, 0, 0x4000_0000, 0, 0x4000_0000]]);
    assert_eq!(mem32_window(&r), None);
}

/// A ragged length is not the shape this parser knows; `None`, not a partial read. Empty is
/// the same answer for the same reason.
#[test]
fn a_ragged_or_empty_ranges_is_refused() {
    let r = ranges(&[[0x0200_0000, 0, 0x4000_0000, 0, 0x4000_0000, 0, 0x4000_0000]]);
    assert_eq!(mem32_window(&r[..r.len() - 4]), None);
    assert_eq!(mem32_window(&[]), None);
}

#[test]
fn a_modern_virtio_pci_id_is_0x1040_plus_the_device_type() {
    assert_eq!(VIRTIO_NET_MODERN as u32, 0x1040 + VIRTIO_TYPE_NET);
    assert_eq!(VIRTIO_BLK_MODERN as u32, 0x1040 + VIRTIO_TYPE_BLOCK);
    assert_eq!(VIRTIO_RNG_MODERN as u32, 0x1040 + VIRTIO_TYPE_ENTROPY);
    assert_eq!(VIRTIO_GPU_MODERN as u32, 0x1040 + VIRTIO_TYPE_GPU);
    assert_eq!(VIRTIO_INPUT_MODERN as u32, 0x1040 + VIRTIO_TYPE_INPUT);
}

/// **The MSI-X capability decodes, and the table size is the field plus one.** The fake's
/// function declares Message Control 0x0002, which is three entries; a decoder that read the
/// field as the count would say two, and would say zero for the very common one-entry table,
/// which reads exactly like a device with no MSI-X at all.
#[test]
fn the_msix_capability_decodes_with_its_table_size_off_by_one_corrected() {
    let mut cfg = FakeCfg::new();
    let bdf = Bdf {
        bus: 0,
        dev: 1,
        func: 0,
    };
    let capability =
        msix_capability(bdf, &mut |b, o| cfg.read32(b, o)).expect("the fake declares MSI-X");
    assert_eq!(
        capability,
        MsixCapability {
            capability_offset: 0x70,
            table_size: 3,
            table_bar: 1,
            table_offset: 0x2000,
            pba_bar: 1,
            pba_offset: 0x2800,
        }
    );
}

/// **An MSI-only function, the shape of an Intel PCH's xHCI** (milestone 242 (USB host and
/// HID)): a power-management capability, then MSI with a 64-bit address, and no MSI-X. The
/// data register moves with the address width, and writing the address's high half into the
/// data register is the bug a 32-bit assumption would make.
#[test]
fn an_msi_only_function_decodes_and_places_its_data_by_address_width() {
    let bdf = Bdf {
        bus: 0,
        dev: 20,
        func: 0,
    };
    let mut read = |_: Bdf, o: u64| match o {
        COMMAND => u32::from(STATUS_CAPABILITIES_LIST) << 16,
        CAPABILITIES_POINTER => 0x70,
        0x70 => 0x01 | 0x80 << 8, // power management, next 0x80
        0x80 => u32::from(CAPABILITY_ID_MSI) | u32::from(MSI_64_BIT | 0x6) << 16, // 64-bit, eight vectors
        _ => 0,
    };
    let msi = msi_capability(bdf, &mut read).expect("the function declares MSI");
    assert_eq!(
        msi,
        MsiCapability {
            capability_offset: 0x80,
            is_64: true
        }
    );
    assert_eq!((msi.address_low(), msi.data()), (0x84, 0x8c));
    assert_eq!(msix_capability(bdf, &mut read), None);
    let narrow = MsiCapability {
        capability_offset: 0x50,
        is_64: false,
    };
    assert_eq!(narrow.data(), 0x58);
    // A 32-bit MSI capability says so, and a function whose status has no capability list
    // is not walked even when its pointer byte happens to lead to one.
    let mut read32 = |_: Bdf, o: u64| match o {
        COMMAND => u32::from(STATUS_CAPABILITIES_LIST) << 16,
        CAPABILITIES_POINTER => 0x50,
        0x50 => u32::from(CAPABILITY_ID_MSI),
        _ => 0,
    };
    assert_eq!(msi_capability(bdf, &mut read32), Some(narrow));
    let mut unlisted = |_: Bdf, o: u64| match o {
        CAPABILITIES_POINTER => 0x50,
        0x50 => u32::from(CAPABILITY_ID_MSI),
        _ => 0,
    };
    assert_eq!(msi_capability(bdf, &mut unlisted), None);
}

/// A function with no capability list at all answers `None` rather than walking from a
/// floating-high capability pointer. 00:02.1 (the fake's virtio-rng) has no status bit set.
#[test]
fn a_function_without_a_capability_list_declares_no_msix() {
    let mut cfg = FakeCfg::new();
    let bdf = Bdf {
        bus: 0,
        dev: 2,
        func: 1,
    };
    assert_eq!(msix_capability(bdf, &mut |b, o| cfg.read32(b, o)), None);
}

/// **A capability list that points at itself terminates.** The walk is a device-supplied
/// linked list read by the kernel during enumeration, so a broken or hostile function must
/// not be able to hang the machine that is bringing it up.
#[test]
fn a_cyclic_capability_list_terminates_instead_of_hanging() {
    let bdf = Bdf {
        bus: 0,
        dev: 4,
        func: 0,
    };
    let mut reads = 0usize;
    let answer = msix_capability(bdf, &mut |_, off| {
        reads += 1;
        match off {
            COMMAND => 0x0010 << 16,      // capability list present
            CAPABILITIES_POINTER => 0x40, // first entry at 0x40
            0x40 => 0x0000_4009,          // a vendor cap whose `next` points back at 0x40
            _ => 0,
        }
    });
    assert_eq!(answer, None);
    assert!(reads < 128, "the walk did not terminate in 64 hops");
}

/// A function whose status says "no capability list" has none, even with a pointer and a
/// capability sitting where the list would be, and even with other status bits set.
#[test]
fn a_status_without_the_list_bit_hides_a_capability_that_is_there() {
    let bdf = Bdf {
        bus: 0,
        dev: 5,
        func: 0,
    };
    let read = |present: bool| {
        move |_: Bdf, off: u64| -> u32 {
            match off {
                // Status bit 3 (an interrupt is pending) is set; bit 4 is the list.
                COMMAND => (if present { 0x0018u32 } else { 0x0008 }) << 16,
                CAPABILITIES_POINTER => 0x40,
                0x40 => 0x0001_0011, // MSI-X, control 1, no next
                _ => 0,
            }
        }
    };
    assert!(msix_capability(bdf, &mut read(true)).is_some());
    assert_eq!(msix_capability(bdf, &mut read(false)), None);
}

/// The two MSI-X control bits are bits 15 and 14 of the Message Control word, as the
/// specification numbers them.
#[test]
fn the_msix_control_bits_are_the_specifications() {
    assert_eq!(MSIX_ENABLE, 0x8000);
    assert_eq!(MSIX_FUNCTION_MASK, 0x4000);
}

/// **A window's three 64-bit values are each two cells, high first.** Every cell here is
/// distinct and the high ones are non-zero, so a value built from the wrong half, or a high
/// half shifted the wrong way, comes out as a different number.
#[test]
fn a_window_reads_its_addresses_and_size_as_high_then_low_cells() {
    let r = ranges(&[[
        0x0200_0000,
        0x0000_0001,
        0x8000_0000,
        0x0000_0001,
        0x8000_0000,
        0x0000_0002,
        0x1000_0000,
    ]]);
    assert_eq!(
        mem32_window(&r),
        Some((0x1_8000_0000, 0x2_1000_0000)),
        "{r:?}"
    );
    // The addresses must agree in their high cells too: a translated window is skipped.
    let translated = ranges(&[[
        0x0200_0000,
        0x0000_0001,
        0x8000_0000,
        0x0000_0000,
        0x8000_0000,
        0,
        0x1000_0000,
    ]]);
    assert_eq!(mem32_window(&translated), None);
}

/// A whole entry followed by four stray bytes is ragged, not "the first entry and some noise".
#[test]
fn a_whole_entry_plus_a_stray_cell_is_ragged() {
    let mut r = ranges(&[[0x0200_0000, 0, 0x4000_0000, 0, 0x4000_0000, 0, 0x4000_0000]]);
    assert!(mem32_window(&r).is_some());
    r.extend_from_slice(&[0; 4]);
    assert_eq!(mem32_window(&r), None);
}

/// Every bus number is queued once and handed out once, in order of arrival, whatever the
/// order and however often it is named: the bit for a bus is its own, in the word for its
/// range of 64.
#[test]
fn every_bus_number_is_its_own_bit_in_the_seen_set() {
    let mut q = BusQueue::from_the_root();
    for bus in (0..=255u8).rev() {
        q.enqueue(bus);
        q.enqueue(bus);
    }
    let mut out = std::vec::Vec::new();
    while let Some(b) = q.next_bus() {
        out.push(b);
    }
    let mut expect: std::vec::Vec<u8> = vec![0];
    expect.extend((1..=255u8).rev());
    assert_eq!(out, expect);
}

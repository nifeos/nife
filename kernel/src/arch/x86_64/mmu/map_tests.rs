//! **The 17 GiB machine, without the 17 GiB machine.**
//!
//! These run in the `x86_64` kernel leg under QEMU, which boots with 2 GiB and can never produce
//! the memory map that killed the first boot on real hardware. [`super::firmware_fill_ceiling`] is
//! a function of the map alone, so the map goes in as a value, transcribed from
//! `art/bench/xenon-2026-09-05-first-light.jpg`: 33 of the 38 entries the machine reports, every
//! one the photograph shows.

use super::{LOCAL_APIC_PHYS, firmware_fill_ceiling};

/// xenon's memory map: `(start, end, usable)`, physical, in the order the firmware wrote it.
/// **Not sorted**, and that is faithful rather than sloppy: the low `0xa0000..0x100000`
/// reservation comes after a 16 GiB RAM region on this machine, which is why the ceiling walk
/// takes more than one pass.
const XENON: &[(u64, u64, bool)] = &[
    (0x0, 0xa_0000, true),
    (0x10_0000, 0x200_0000, true),
    (0x200_0000, 0x214_c000, false),
    (0x214_c000, 0xacfc_0000, true),
    (0xacfc_0000, 0xad00_0000, false),
    (0xad00_0000, 0xb7e1_b000, true),
    (0xb7e1_b000, 0xb82c_5000, false),
    (0xb82c_5000, 0xb9ef_d000, true),
    (0xb9ef_d000, 0xb9ef_e000, false),
    (0xb9ef_e000, 0xb9ef_f000, false),
    (0xb9ef_f000, 0xb9fb_0000, true),
    (0xb9fb_0000, 0xb9fb_a000, true),
    (0xb9fb_a000, 0xb9fb_b000, true),
    (0xb9fb_b000, 0xb9fb_f000, false),
    (0xb9fb_f000, 0xc894_0000, true),
    (0xc894_0000, 0xc8d3_e000, true),
    (0xc8d3_e000, 0xc97c_4000, true),
    (0xc97c_4000, 0xcadf_b000, false),
    (0xcadf_b000, 0xcae4_1000, false),
    (0xcae4_1000, 0xcb77_5000, false),
    (0xcb77_5000, 0xcbd1_8000, false),
    (0xcbd1_8000, 0xcbdf_f000, false),
    (0xcbdf_f000, 0xcbe0_0000, true),
    // The last entry below the 32-bit MMIO hole: firmware's own reservation at the top of low
    // DRAM. Above it the map describes nothing until 0xf0000000.
    (0xcbe0_0000, 0xd000_0000, false),
    (0x1_0000_0000, 0x1_4000_0000, true),
    (0x1_4000_0000, 0x1_408c_0000, true),
    (0x1_408c_0000, 0x4_2e00_0000, true),
    (0xa_0000, 0x10_0000, false),
    (0xf000_0000, 0xf800_0000, false),
    (0xfe00_0000, 0xfe01_1000, false),
    (0xfec0_0000, 0xfec0_1000, false),
    (0xfee0_0000, 0xfee0_1000, false),
    (0xff00_0000, 0x1_0000_0000, false),
];

fn ceiling(map: &[(u64, u64, bool)]) -> u64 {
    firmware_fill_ceiling(|span| {
        for &(start, end, _) in map {
            span(start, end);
        }
    })
}

/// **The panic, reproduced as an assertion.** The old bound was the top of RAM, which on this
/// machine is `0x42e000000`: every MMIO window it has is below that, so the cacheable
/// reservation fill claimed the local APIC's page a few lines before the device mapping asked
/// for it, and the mapper refused. The chain rule stops at the floor of the MMIO hole instead.
#[test_case]
fn the_fill_stops_below_the_devices_on_a_17_gib_machine() {
    let top_of_ram = XENON
        .iter()
        .filter(|(.., usable)| *usable)
        .map(|(_, end, _)| *end)
        .max()
        .expect("the fixture has RAM");
    assert_eq!(top_of_ram, 0x4_2e00_0000, "the fixture is not xenon's map");
    assert!(
        top_of_ram > LOCAL_APIC_PHYS,
        "the old bound would not have reached the local APIC, so this is not the failure"
    );

    assert_eq!(
        ceiling(XENON),
        0xd000_0000,
        "the fill should stop where the firmware's map stops describing memory"
    );
    assert!(
        ceiling(XENON) <= LOCAL_APIC_PHYS,
        "the local APIC is inside the cacheable fill again"
    );
}

/// **The hole xenon's BARs have to go in**, both ends of it, from the map alone.
///
/// The floor is the same number [`the_fill_stops_below_the_devices_on_a_17_gib_machine`]
/// asserts and that is the point: one walk of the map answers "how far may the cacheable fill
/// follow memory" and "where does the MMIO hole start", because they are the same boundary
/// asked from the two sides. The ceiling is the next thing the firmware describes above it,
/// which on this machine is the reserved window at `0xf0000000`.
#[test_case]
fn the_mmio_hole_on_a_17_gib_machine_is_bounded_at_both_ends() {
    let hole = super::firmware_mmio_hole(|span| {
        for &(start, end, _) in XENON {
            span(start, end);
        }
    });
    assert_eq!(hole, (0xd000_0000, 0xf000_0000), "xenon's 32-bit MMIO hole");
    assert!(
        hole.0 > 0xc000_0000,
        "the retired constant was 0xc0000000, and this machine's hole starts above it, \
         which is the whole reason it is gone"
    );
}

/// **The window steps over the screen**, which on this machine is at the floor of the hole.
///
/// xenon's framebuffer is at `0xd0000000`, byte for byte where its MMIO hole begins, so a
/// derivation that took the floor and asked nothing else would put the first BAR it placed on
/// top of the console the panic would have been printed on. 1920x1080 at 4 bytes is `0x7e9000`,
/// and the next 2 MiB boundary above that is `0xd0800000`.
///
/// **This is not only a prediction about a machine nobody here can boot.** The same collision
/// reproduces under OVMF on QEMU, whose framebuffer also sits at the floor of its hole, and the
/// boot line there reads `bar window 0x80400000..0x80600000` for the same reason.
#[test_case]
fn the_window_steps_over_a_framebuffer_at_the_floor_of_the_hole() {
    const FB: u64 = 0xd000_0000;
    const FB_END: u64 = FB + 1920 * 1080 * 4;
    let at = super::window_in_hole(0xd000_0000, 0xf000_0000, 0x20_0000, |avoid| {
        avoid(FB, FB_END);
    });
    assert_eq!(at, Some(0xd080_0000));
    let at = at.expect("checked above");
    assert!(at >= FB_END, "the window overlaps the screen");
    assert_eq!(
        at % 0x20_0000,
        0,
        "a BAR window must be aligned to its size"
    );
}

/// With nothing in the way the window is the floor itself, which is what both QEMU paths do
/// when no aperture is in the first megabytes of the hole.
#[test_case]
fn an_empty_hole_gives_up_its_floor() {
    let at = super::window_in_hole(0x1000_0000, 0xb000_0000, 0x20_0000, |_| {});
    assert_eq!(at, Some(0x1000_0000));
}

/// **A hole with no room in it is refused rather than squeezed.** The caller panics on this,
/// which is the posture the whole milestone turns on: there is no arm that falls back to a
/// constant, because the constant is what put device registers on top of RAM.
#[test_case]
fn a_hole_with_no_room_answers_none() {
    assert_eq!(
        super::window_in_hole(0xd000_0000, 0xd010_0000, 0x20_0000, |_| {}),
        None,
        "a 1 MiB hole cannot hold a 2 MiB window"
    );
    assert_eq!(
        super::window_in_hole(0xd000_0000, 0xd040_0000, 0x20_0000, |avoid| {
            avoid(0xd000_0000, 0xd030_0000);
        }),
        None,
        "the only aligned slot left starts past the end"
    );
}

/// The two sources' disagreement is a sentence a person reads off a photograph, so it names
/// both numbers. Asserted for the same reason [`the_failure_names_both_ranges`] is.
#[test_case]
fn the_disagreement_names_both_sources() {
    let mut s = Line::default();
    core::fmt::write(
        &mut s,
        format_args!(
            "{}",
            super::MemoryMappedIoWindowError::Disagreement {
                map: 0xd000_0000,
                tolud: 0xc000_0000,
            }
        ),
    )
    .expect("formatting cannot fail");
    let line = s.as_str();
    assert!(line.contains("0xd0000000"), "{line}");
    assert!(line.contains("0xc0000000"), "{line}");
}

/// A machine whose RAM ends below the hole, which is every machine this tree had booted before
/// xenon: the new rule has to answer what the old one did, or it is a different bug.
#[test_case]
fn a_small_machine_is_unchanged() {
    let qemu: &[(u64, u64, bool)] = &[
        (0x0, 0x9_fc00, true),
        (0x9_fc00, 0xa_0000, false),
        (0xf_0000, 0x10_0000, false),
        (0x10_0000, 0xffd_d000, true),
        (0xffd_d000, 0x1000_0000, false),
        (0xfffc_0000, 0x1_0000_0000, false),
    ];
    assert_eq!(ceiling(qemu), 0x1000_0000);
}

/// The failure message names both ranges, which is the whole point of it. Asserted rather than
/// eyeballed, because the reader it is written for is holding a camera.
#[test_case]
fn the_failure_names_both_ranges() {
    let f = super::MapFailure {
        what: "local apic",
        lo: 0xfee0_0000,
        hi: 0xfee0_1000,
        err: paging::MapError::AlreadyMapped,
        conflict: Some((
            0xfee0_0000,
            "firmware reservation",
            0xfee0_0000,
            0xfee0_1000,
        )),
    };
    let mut s = Line::default();
    core::fmt::write(&mut s, format_args!("{f}")).expect("formatting a failure cannot fail");
    let line = s.as_str();
    assert!(line.contains("local apic 0xfee00000..0xfee01000"), "{line}");
    assert!(
        line.contains("firmware reservation 0xfee00000..0xfee01000"),
        "{line}"
    );
}

/// A fixed line of text to format into. The kernel has no allocator, and one panic line is the
/// only thing anything here needs to build.
struct Line {
    bytes: [u8; 256],
    len: usize,
}

impl Default for Line {
    fn default() -> Self {
        Line {
            bytes: [0; 256],
            len: 0,
        }
    }
}

impl Line {
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).expect("we only wrote &str into it")
    }
}

impl core::fmt::Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = (self.len + s.len()).min(self.bytes.len());
        let take = end - self.len;
        self.bytes[self.len..end].copy_from_slice(&s.as_bytes()[..take]);
        self.len = end;
        Ok(())
    }
}

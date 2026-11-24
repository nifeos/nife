//! Where the JH7110's two PCI Express root ports are, as the tree describes them, for
//! `jh7110_pci_express`.
//!
//! Mainline Linux's `arch/riscv/boot/dts/starfive/jh7110.dtsi` (`torvalds/linux` at
//! `cbbddb6b08cc`) has `pcie0: pcie@940000000` and `pcie1: pcie@9c0000000`, each
//! `compatible = "starfive,jh7110-pcie"`, with two `reg` entries named by `reg-names = "cfg", "apb"`
//! (the configuration window and the bridge's registers, in that order), `ranges` holding a 32-bit and a 64-bit
//! prefetchable memory range, `linux,pci-domain` 0 or 1, and `perst-gpios` naming the fundamental
//! reset pin in the SYS general-purpose input/output (GPIO) controller (`jh7110-common.dtsi`). The chip's tree leaves both `disabled`,
//! and `jh7110-starfive-visionfive-2.dtsi` marks both `okay`.
//!
//! [`discover`] finds the node named for a port in [`NODE_NAMES`] and hands [`RootPort::from_node`]
//! its `reg` and its properties.
//!
//! # BUGS
//!
//! - Only mainline's node shape is read. Whether the vendor U-Boot tree a `VisionFive` 2's
//!   firmware hands over carries these nodes under these names and `reg-names` is a board step in
//!   `partsfoundry/record:notes/jh7110-pci-express-bench.md`; a tree without them reads as no port.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring, after
//! the crate it serves. calef has not named it.

use device_tree_blob::{DeviceTreeBlob, Error, Region};
use jh7110_pci_express::{Port, PortNumber, Window};

/// Mainline's `compatible`.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
pub const COMPATIBLE: &[u8] = b"starfive,jh7110-pcie";

/// The node name of each port, indexed by port number.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
pub const NODE_NAMES: [&[u8]; 2] = [b"pcie@940000000", b"pcie@9c0000000"];

/// One root port as the tree describes it.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootPort {
    /// The windows and the reset pin, ready for `jh7110_pci_express::bring_up`.
    pub port: Port,
    /// `status` is `okay` or absent.
    pub status_okay: bool,
}

fn cell(bytes: &[u8], index: usize) -> Option<u32> {
    let start = index.checked_mul(4)?;
    Some(u32::from_be_bytes(
        bytes.get(start..start.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// The 64-bit prefetchable memory range in a PCI `ranges` property (three address cells for the
/// bus, two for the processor, two for the size), when its bus and processor addresses agree.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring, beside
/// `pci_configuration_space::memory_window_32_bit`.
#[must_use]
pub fn prefetch_window(ranges: &[u8]) -> Option<Window> {
    const ENTRY: usize = 7 * 4;
    if !ranges.len().is_multiple_of(ENTRY) {
        return None;
    }
    ranges.as_chunks::<ENTRY>().0.iter().find_map(|entry| {
        let word = |i| cell(entry, i).map(u64::from);
        let high = word(0)?;
        let space = (high >> 24) & 0b11;
        if space != 0b11 || high & (1 << 30) == 0 {
            return None;
        }
        let bus = (word(1)? << 32) | word(2)?;
        let processor = (word(3)? << 32) | word(4)?;
        let size_bytes = (word(5)? << 32) | word(6)?;
        (bus == processor && size_bytes != 0).then_some(Window {
            physical_address: processor,
            size_bytes,
        })
    })
}

impl RootPort {
    /// **Port `number`, from the node named for it**: `regions` is its decoded `reg`, and
    /// `property(name)` is the node's property `name`.
    ///
    /// `None` when the `compatible` is not [`COMPATIBLE`], when `linux,pci-domain` names the other
    /// port, when `reg-names` does not name both `apb` and `cfg`, or when `ranges` holds no 32-bit
    /// memory range. Without those the bring-up has nowhere to write, and a guess at a window is how
    /// a write lands on the wrong device. The `perst-gpios` pin is its second cell, the specifier's
    /// pin number; an absent property leaves the pin `None`.
    ///
    /// # Errors
    ///
    /// Propagates the first error `property` returns.
    ///
    /// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
    pub fn from_node<'t, E>(
        number: PortNumber,
        regions: &[Region],
        mut property: impl FnMut(&[u8]) -> Result<Option<&'t [u8]>, E>,
    ) -> Result<Option<RootPort>, E> {
        let Some(compatible) = property(b"compatible")? else {
            return Ok(None);
        };
        if !compatible.split(|&b| b == 0).any(|s| s == COMPATIBLE) {
            return Ok(None);
        }
        let domain = property(b"linux,pci-domain")?.and_then(|v| cell(v, 0));
        if domain.is_some_and(|d| PortNumber::from_domain(d) != Some(number)) {
            return Ok(None);
        }
        let names = property(b"reg-names")?.unwrap_or_default();
        let named = |wanted: &[u8]| {
            names
                .split(|&b| b == 0)
                .position(|n| n == wanted)
                .and_then(|i| regions.get(i))
                .map(|r| Window {
                    physical_address: r.start,
                    size_bytes: r.size_bytes,
                })
        };
        let (Some(bridge), Some(configuration)) = (named(b"apb"), named(b"cfg")) else {
            return Ok(None);
        };
        let ranges = property(b"ranges")?.unwrap_or_default();
        let Some((memory_physical_address, memory_size)) =
            pci_configuration_space::memory_window_32_bit(ranges)
        else {
            return Ok(None);
        };
        let status_okay =
            property(b"status")?.is_none_or(|v| matches!(v, b"okay\0" | b"okay" | b"ok\0" | b"ok"));
        let reset_pin = property(b"perst-gpios")?.and_then(|v| cell(v, 1));
        Ok(Some(RootPort {
            port: Port {
                number,
                bridge,
                configuration,
                memory_32_bit: Window {
                    physical_address: memory_physical_address,
                    size_bytes: memory_size,
                },
                prefetch_memory: prefetch_window(ranges),
                reset_pin,
            },
            status_okay,
        }))
    }
}

/// **Find port `number` in `tree`**, or `None` when the tree has no node under its name or the
/// node does not describe the port (see [`RootPort::from_node`]).
///
/// No constant fallback, unlike the clock and reset windows: QEMU's tree and any board without the
/// node must not see the JH7110's windows written.
///
/// # Errors
///
/// Propagates [`Error`] if the blob is malformed.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.
pub fn discover(tree: &DeviceTreeBlob<'_>, number: PortNumber) -> Result<Option<RootPort>, Error> {
    let name = NODE_NAMES[match number {
        PortNumber::Zero => 0,
        PortNumber::One => 1,
    }];
    let mut regions = [Region {
        start: 0,
        size_bytes: 0,
    }; 2];
    let found = tree.node_register_regions(name, &mut regions)?;
    if found == 0 {
        return Ok(None);
    }
    RootPort::from_node(number, &regions[..found], |p| tree.node_property(name, p))
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec;
    use std::vec::Vec;

    use super::*;

    fn node<'t>(
        table: &'t [(&'static [u8], &'t [u8])],
    ) -> impl FnMut(&[u8]) -> Result<Option<&'t [u8]>, ()> + 't {
        move |name| Ok(table.iter().find(|(n, _)| *n == name).map(|(_, v)| *v))
    }

    fn cells(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }

    /// Mainline's `pcie1`, as `jh7110.dtsi` and `jh7110-common.dtsi` give it: `cfg` first, and
    /// `perst-gpios = <&sysgpio 28 GPIO_ACTIVE_LOW>`.
    #[allow(clippy::type_complexity)]
    fn mainline_port_1() -> (Vec<Region>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        let regions = vec![
            Region {
                start: 0x9_c000_0000,
                size_bytes: 0x100_0000,
            },
            Region {
                start: 0x2c00_0000,
                size_bytes: 0x10_0000,
            },
        ];
        let ranges = cells(&[
            0x8200_0000,
            0,
            0x3800_0000,
            0,
            0x3800_0000,
            0,
            0x0800_0000,
            0xc300_0000,
            9,
            0x8000_0000,
            9,
            0x8000_0000,
            0,
            0x4000_0000,
        ]);
        (
            regions,
            ranges,
            cells(&[1]),
            cells(&[0x55, 28, 1]),
            b"cfg\0apb\0".to_vec(),
        )
    }

    #[test]
    fn mainline_port_one_reads_as_the_crate_s_own_description_of_it() {
        let (regions, ranges, domain, reset_specifier, names) = mainline_port_1();
        let table: &[(&[u8], &[u8])] = &[
            (b"compatible", b"starfive,jh7110-pcie\0"),
            (b"reg-names", &names),
            (b"ranges", &ranges),
            (b"linux,pci-domain", &domain),
            (b"perst-gpios", &reset_specifier),
            (b"status", b"okay\0"),
        ];
        let found = RootPort::from_node(PortNumber::One, &regions, node(table))
            .unwrap()
            .unwrap();
        assert!(found.status_okay);
        assert_eq!(found.port, PortNumber::One.mainline());
    }

    #[test]
    fn register_names_order_the_windows() {
        let (mut regions, ranges, _, _, _) = mainline_port_1();
        regions.reverse();
        let table: &[(&[u8], &[u8])] = &[
            (b"compatible", b"starfive,jh7110-pcie\0"),
            (b"reg-names", b"apb\0cfg\0"),
            (b"ranges", &ranges),
        ];
        let found = RootPort::from_node(PortNumber::One, &regions, node(table))
            .unwrap()
            .unwrap();
        assert_eq!(found.port.bridge.physical_address, 0x2c00_0000);
        assert_eq!(found.port.configuration.physical_address, 0x9_c000_0000);
        assert_eq!(found.port.reset_pin, None);
    }

    #[test]
    fn a_node_that_does_not_describe_the_port_reads_as_none() {
        let (regions, ranges, _, _, names) = mainline_port_1();
        let other_domain = cells(&[0]);
        let cases: [&[(&[u8], &[u8])]; 5] = [
            &[],
            &[(b"compatible", b"vendor,other\0")],
            &[
                (b"compatible", b"starfive,jh7110-pcie\0"),
                (b"reg-names", &names),
                (b"ranges", &ranges),
                (b"linux,pci-domain", &other_domain),
            ],
            &[
                (b"compatible", b"starfive,jh7110-pcie\0"),
                (b"reg-names", b"reg\0config\0"),
                (b"ranges", &ranges),
            ],
            &[
                (b"compatible", b"starfive,jh7110-pcie\0"),
                (b"reg-names", &names),
            ],
        ];
        for table in cases {
            assert_eq!(
                RootPort::from_node(PortNumber::One, &regions, node(table)).unwrap(),
                None
            );
        }
        let failing = |_: &[u8]| -> Result<Option<&'static [u8]>, u8> { Err(7) };
        assert_eq!(
            RootPort::from_node(PortNumber::One, &regions, failing),
            Err(7)
        );
    }

    #[test]
    fn a_disabled_node_still_reads_but_says_so() {
        let (regions, ranges, _, _, names) = mainline_port_1();
        let table: &[(&[u8], &[u8])] = &[
            (b"compatible", b"starfive,jh7110-pcie\0"),
            (b"reg-names", &names),
            (b"ranges", &ranges),
            (b"status", b"disabled\0"),
        ];
        let found = RootPort::from_node(PortNumber::One, &regions, node(table))
            .unwrap()
            .unwrap();
        assert!(!found.status_okay);
    }

    #[test]
    fn prefetch_window_skips_ranges_that_are_not_64_bit_prefetch_memory() {
        assert_eq!(prefetch_window(&[]), None);
        assert_eq!(prefetch_window(&[0; 27]), None);
        let not_prefetch = cells(&[0x8300_0000, 9, 0, 9, 0, 0, 0x1000]);
        assert_eq!(prefetch_window(&not_prefetch), None);
        let translated = cells(&[0xc300_0000, 0, 0, 9, 0, 0, 0x1000]);
        assert_eq!(prefetch_window(&translated), None);
    }

    /// A flattened device tree with one node under the root, for the tests that need a tree QEMU
    /// will not emit: the root declares two address and two size cells, as `jh7110.dtsi` does.
    /// Written out by hand because the crate has no tree writer.
    fn tree_with_node(name: &[u8], properties: &[(&[u8], &[u8])]) -> Vec<u8> {
        let mut strings = Vec::new();
        let mut structure = Vec::new();
        let push = |v: &mut Vec<u8>, w: u32| v.extend_from_slice(&w.to_be_bytes());
        let pad = |v: &mut Vec<u8>| v.resize(v.len().next_multiple_of(4), 0);
        let mut property = |st: &mut Vec<u8>, key: &[u8], value: &[u8]| {
            let offset = u32::try_from(strings.len()).unwrap();
            strings.extend_from_slice(key);
            strings.push(0);
            push(st, 3);
            push(st, u32::try_from(value.len()).unwrap());
            push(st, offset);
            st.extend_from_slice(value);
            pad(st);
        };
        push(&mut structure, 1);
        push(&mut structure, 0);
        property(&mut structure, b"#address-cells", &cells(&[2]));
        property(&mut structure, b"#size-cells", &cells(&[2]));
        push(&mut structure, 1);
        structure.extend_from_slice(name);
        structure.push(0);
        pad(&mut structure);
        for (key, value) in properties {
            property(&mut structure, key, value);
        }
        for token in [2, 2, 9] {
            push(&mut structure, token);
        }
        let structure_offset = 40 + 16;
        let strings_offset = structure_offset + structure.len();
        let total = strings_offset + strings.len();
        let mut out = Vec::new();
        for word in [
            0xd00d_feed,
            total,
            structure_offset,
            strings_offset,
            40,
            17,
            16,
            0,
            strings.len(),
            structure.len(),
        ] {
            push(&mut out, u32::try_from(word).unwrap());
        }
        out.extend_from_slice(&[0; 16]);
        out.extend_from_slice(&structure);
        out.extend_from_slice(&strings);
        out
    }

    #[test]
    fn discover_finds_mainline_port_one_under_its_own_name_and_not_port_zero() {
        let (_, ranges, domain, reset_specifier, names) = mainline_port_1();
        let reg = cells(&[9, 0xc000_0000, 0, 0x100_0000, 0, 0x2c00_0000, 0, 0x10_0000]);
        let blob = tree_with_node(
            NODE_NAMES[1],
            &[
                (b"compatible", b"starfive,jh7110-pcie\0"),
                (b"reg", &reg),
                (b"reg-names", &names),
                (b"ranges", &ranges),
                (b"linux,pci-domain", &domain),
                (b"perst-gpios", &reset_specifier),
            ],
        );
        let tree = DeviceTreeBlob::from_bytes(&blob).unwrap();
        let found = discover(&tree, PortNumber::One).unwrap().unwrap();
        assert_eq!(found.port, PortNumber::One.mainline());
        assert!(found.status_okay);
        assert_eq!(discover(&tree, PortNumber::Zero).unwrap(), None);
    }

    #[test]
    fn a_node_complete_but_for_its_compatible_reads_as_none() {
        let (regions, ranges, _, _, names) = mainline_port_1();
        let table: &[(&[u8], &[u8])] = &[
            (b"compatible", b"vendor,other\0"),
            (b"reg-names", &names),
            (b"ranges", &ranges),
        ];
        assert_eq!(
            RootPort::from_node(PortNumber::One, &regions, node(table)).unwrap(),
            None
        );
    }

    #[test]
    fn a_prefetch_window_of_four_gibibytes_or_more_keeps_its_size_s_high_word() {
        let large = cells(&[0xc300_0000, 9, 0, 9, 0, 1, 0]);
        assert_eq!(
            prefetch_window(&large),
            Some(Window {
                physical_address: 0x9_0000_0000,
                size_bytes: 0x1_0000_0000,
            })
        );
    }

    #[test]
    fn qemu_s_tree_has_no_port() {
        let blob = include_bytes!("../../tests/fixtures/qemu-riscv64-virt.dtb");
        let tree = DeviceTreeBlob::from_bytes(blob).unwrap();
        for number in [PortNumber::Zero, PortNumber::One] {
            assert_eq!(discover(&tree, number).unwrap(), None);
        }
    }
}

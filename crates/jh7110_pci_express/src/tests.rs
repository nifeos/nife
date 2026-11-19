//! Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
use pci_configuration_space::{Bdf, CLASS_NVME, Function, HEADER_TYPE_BRIDGE, walk};

use crate::simulation::{Event, Simulated};
use crate::{
    Block, Failure, LINK_POLLS, PortNumber, Window, bring_up, glue, root_port, bridge,
};

fn walk_all(simulated: &Simulated) -> Vec<Function> {
    let mut found = Vec::new();
    walk(
        2,
        &mut |bdf: Bdf, offset| simulated.configuration_read(bdf.ecam_offset() + offset),
        &mut |function| found.push(function),
    );
    found
}

fn configure(simulated: &mut Simulated, memory: Window) -> root_port::Configured {
    let shared = std::cell::RefCell::new(simulated);
    root_port::configure(
        &mut |bdf: Bdf, offset| {
            shared
                .borrow()
                .configuration_read(bdf.ecam_offset() + offset)
        },
        &mut |bdf: Bdf, offset, value| {
            shared
                .borrow_mut()
                .configuration_write(bdf.ecam_offset() + offset, value);
        },
        memory,
    )
}

#[test]
fn configuration_window_encodes_as_linux_programs_it() {
    // 16 MiB at 0x9_4000_0000: log2 is 24, so the size field is 23 and the low word is
    // 0x4000_0000 | 23 << 1 | 1.
    let entry = bridge::outbound_window(0, 0x9_4000_0000, 0, 0x100_0000).unwrap();
    assert_eq!(
        entry,
        [
            (0x810, bridge::INTERFACE_CONFIGURATION),
            (0x800, 0x4000_002f),
            (0x804, 0x9),
            (0x808, 0),
            (0x80c, 0),
        ]
    );
    assert_eq!(
        bridge::translated_size_bytes(0x4000_002f),
        Some(0x100_0000)
    );
}

#[test]
fn memory_window_uses_the_next_entry_and_the_memory_interface() {
    let entry = bridge::outbound_window(1, 0x3800_0000, 0x3800_0000, 0x800_0000).unwrap();
    assert_eq!(entry[0], (0x830, bridge::INTERFACE_TRANSMIT_RECEIVE));
    assert_eq!(entry[1], (0x820, 0x3800_0000 | (26 << 1) | 1));
    assert_eq!(entry[3], (0x828, 0x3800_0000));
}

#[test]
fn window_encoder_refuses_what_an_entry_cannot_hold() {
    assert_eq!(bridge::outbound_window(8, 0, 0, 0x1000), None);
    assert_eq!(bridge::outbound_window(0, 0, 0, 0x800), None);
    assert_eq!(bridge::outbound_window(0, 0, 0, 0x3000), None);
    assert_eq!(bridge::outbound_window(0, 0, 0, 0), None);
}

#[test]
fn bring_up_trains_the_link_and_programs_three_windows() {
    let mut simulated = Simulated::new(PortNumber::One);
    let brought_up = bring_up(&mut simulated, &PortNumber::One.mainline()).unwrap();
    assert!(brought_up.link_up);
    assert_eq!(brought_up.link_polls, 1);
    assert_eq!(brought_up.windows, 3);
    assert_eq!(
        brought_up.link_status & glue::DATA_LINK_ACTIVE,
        glue::DATA_LINK_ACTIVE
    );
}

#[test]
fn bring_up_follows_linux_order() {
    let mut simulated = Simulated::new(PortNumber::One);
    let port = PortNumber::One.mainline();
    bring_up(&mut simulated, &port).unwrap();
    let base = PortNumber::One.system_top_group_controller_base();
    let position = |wanted: &dyn Fn(&Event) -> bool| {
        simulated
            .events
            .iter()
            .position(wanted)
            .expect("event happened")
    };
    let root_port_glue = position(&|event| {
        matches!(event, Event::Write(Block::SystemTopGroupController, offset, _)
            if *offset == base + glue::ROOT_PORT)
    });
    let clocks = position(&|event| *event == Event::ClocksAndResets);
    let first_pin_word =
        position(&|event| matches!(event, Event::Write(Block::GeneralPurposeInputOutput, _, _)));
    let root_port_enable = position(&|event| {
        matches!(
            event,
            Event::Write(Block::Bridge, bridge::GENERAL_SETTINGS, _)
        )
    });
    let configuration_entry = position(&|event| {
        matches!(
            event,
            Event::Write(
                Block::Bridge,
                bridge::OUTBOUND_SOURCE_LOW,
                _
            )
        )
    });
    assert!(root_port_glue < clocks);
    assert!(clocks < first_pin_word);
    assert!(first_pin_word < root_port_enable);
    assert!(root_port_enable < configuration_entry);
    // Three functions disabled through the bridge's miscellaneous word.
    let disables = simulated
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::Write(Block::Bridge, bridge::MISCELLANEOUS, _)
            )
        })
        .count();
    assert_eq!(disables, 3);
    // The function selects end cleared.
    assert_eq!(
        simulated.words[&(Block::SystemTopGroupController, base + glue::READ_CHANNEL)]
            & glue::READ_CHANNEL_MASK,
        0
    );
    let write_channel =
        simulated.words[&(Block::SystemTopGroupController, base + glue::WRITE_CHANNEL)];
    assert_eq!(write_channel & glue::WRITE_CHANNEL_MASK, 0);
    assert_eq!(
        write_channel & (glue::CLOCK_REQUEST | glue::REFERENCE_CLOCK_SOURCE_MASK),
        glue::CLOCK_REQUEST | glue::REFERENCE_CLOCK_SOURCE
    );
    // Latency tolerance reporting is off, and the class reads as a bridge with the revision kept.
    assert_eq!(
        simulated.words[&(Block::Bridge, bridge::MESSAGE_RECEPTION)]
            & bridge::LATENCY_TOLERANCE_REPORTING_RECEPTION,
        0
    );
    assert_eq!(
        simulated.words[&(Block::Bridge, bridge::IDENTIFICATION_CLASS_AND_REVISION)],
        bridge::CLASS_PCI_TO_PCI_BRIDGE | 2
    );
}

#[test]
fn port_zero_also_switches_its_physical_layer_out_of_usb_mode() {
    let mut simulated = Simulated::new(PortNumber::Zero);
    let brought_up = bring_up(&mut simulated, &PortNumber::Zero.mainline()).unwrap();
    assert!(brought_up.link_up);
    assert!(
        simulated
            .events
            .iter()
            .any(|event| matches!(event, Event::Write(Block::SystemController, _, _)))
    );
}

#[test]
fn port_one_writes_no_physical_layer_mode_bits() {
    let mut simulated = Simulated::new(PortNumber::One);
    bring_up(&mut simulated, &PortNumber::One.mainline()).unwrap();
    assert!(!simulated.events.iter().any(|event| matches!(
        event,
        Event::Write(Block::SystemController, _, _)
            | Event::Write(Block::SystemTopGroupController, crate::physical_layer::MODE, _)
    )));
}

#[test]
fn an_empty_slot_reports_link_down_after_every_poll_and_still_programs_windows() {
    let mut simulated = Simulated::new(PortNumber::One);
    simulated.device_fitted = false;
    let brought_up = bring_up(&mut simulated, &PortNumber::One.mainline()).unwrap();
    assert!(!brought_up.link_up);
    assert_eq!(brought_up.link_polls, LINK_POLLS);
    assert_eq!(brought_up.windows, 3);
    // The root port still answers, and nothing is behind it.
    let found = walk_all(&simulated);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].header_type, HEADER_TYPE_BRIDGE);
}

#[test]
fn a_stuck_reset_stops_before_the_fundamental_reset() {
    let mut simulated = Simulated::new(PortNumber::One);
    simulated.resets_stick = true;
    assert_eq!(
        bring_up(&mut simulated, &PortNumber::One.mainline()),
        Err(Failure::ClocksAndResets)
    );
    assert!(
        !simulated
            .events
            .iter()
            .any(|event| matches!(event, Event::Write(Block::GeneralPurposeInputOutput, _, _)))
    );
}

#[test]
fn a_window_that_is_not_a_power_of_two_is_refused_by_index() {
    let mut simulated = Simulated::new(PortNumber::One);
    let mut port = PortNumber::One.mainline();
    port.memory_32_bit.size_bytes = 0x600_0000;
    assert_eq!(bring_up(&mut simulated, &port), Err(Failure::Window(1)));
}

#[test]
fn nothing_answers_before_bring_up() {
    let simulated = Simulated::new(PortNumber::One);
    assert!(walk_all(&simulated).is_empty());
}

#[test]
fn the_non_volatile_memory_express_controller_is_hidden_until_the_root_port_forwards_bus_one() {
    let mut simulated = Simulated::new(PortNumber::One);
    let port = PortNumber::One.mainline();
    bring_up(&mut simulated, &port).unwrap();

    let before = walk_all(&simulated);
    assert_eq!(before.len(), 1, "only the root port, with zero bus numbers");
    assert_eq!(before[0].vendor, 0x1556);

    let configured = configure(&mut simulated, port.memory_32_bit);
    assert_eq!(
        configured,
        root_port::Configured::Forwarding {
            bus_numbers: 0x0001_0100,
            memory_base_and_limit: 0x3ff0_3800,
        }
    );

    let after = walk_all(&simulated);
    assert_eq!(after.len(), 2);
    assert_eq!(after[0].bdf, root_port::LOCATION);
    let bridge = after[0].bridge.unwrap();
    assert_eq!(
        (bridge.primary, bridge.secondary, bridge.subordinate),
        (0, 1, 1)
    );
    assert_eq!(
        after[1].bdf,
        Bdf {
            bus: 1,
            dev: 0,
            func: 0
        }
    );
    assert_eq!(after[1].class, CLASS_NVME);
}

#[test]
fn the_non_volatile_memory_express_bar_sizes_through_the_configured_port() {
    let mut simulated = Simulated::new(PortNumber::One);
    let port = PortNumber::One.mainline();
    bring_up(&mut simulated, &port).unwrap();
    configure(&mut simulated, port.memory_32_bit);
    let bar0 = Bdf {
        bus: 1,
        dev: 0,
        func: 0,
    }
    .ecam_offset()
        + 0x10;
    simulated.configuration_write(bar0, 0xffff_ffff);
    let sized = simulated.configuration_read(bar0);
    assert_eq!(!(sized & !0xf) + 1, crate::simulation::ENDPOINT_BAR_BYTES);
}

#[test]
fn configure_refuses_an_absent_root_port() {
    let mut simulated = Simulated::new(PortNumber::One);
    let port = PortNumber::One.mainline();
    assert_eq!(
        configure(&mut simulated, port.memory_32_bit),
        root_port::Configured::Absent
    );
}

#[test]
fn configure_refuses_a_window_the_word_cannot_express() {
    let mut simulated = Simulated::new(PortNumber::One);
    let port = PortNumber::One.mainline();
    bring_up(&mut simulated, &port).unwrap();
    let above_four_gibibytes = Window {
        physical_address: 0x9_8000_0000,
        size_bytes: 0x10_0000,
    };
    assert_eq!(
        configure(&mut simulated, above_four_gibibytes),
        root_port::Configured::WindowUnusable
    );
}

#[test]
fn memory_base_and_limit_counts_in_mebibytes() {
    let word = root_port::memory_base_and_limit(Window {
        physical_address: 0x3800_0000,
        size_bytes: 0x800_0000,
    });
    assert_eq!(word, Some(0x3ff0_3800));
    assert_eq!(
        root_port::memory_base_and_limit(Window {
            physical_address: 0x3800_8000,
            size_bytes: 0x10_0000,
        }),
        None
    );
    assert_eq!(
        root_port::memory_base_and_limit(Window {
            physical_address: 0xfff0_0000,
            size_bytes: 0x20_0000,
        }),
        None
    );
}

#[test]
fn bus_numbers_keep_the_latency_timer() {
    assert_eq!(root_port::bus_numbers(0x4000_0000), 0x4001_0100);
    assert_eq!(root_port::bus_numbers(0xffff_ffff), 0xff01_0100);
}

#[test]
fn reset_pin_fields_land_in_their_byte() {
    use crate::general_purpose_input_output::word_and_shift;
    assert_eq!(word_and_shift(26), (24, 16));
    assert_eq!(word_and_shift(28), (28, 0));
}

#[test]
fn ports_map_from_their_domain() {
    assert_eq!(PortNumber::from_domain(0), Some(PortNumber::Zero));
    assert_eq!(PortNumber::from_domain(1), Some(PortNumber::One));
    assert_eq!(PortNumber::from_domain(2), None);
}

//! Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
use pci_configuration_space::{
    Bdf, CLASS_NON_VOLATILE_MEMORY_EXPRESS, Function, HEADER_TYPE_BRIDGE, walk,
};

use crate::simulation::{Event, Simulated};
use crate::{Block, Failure, LINK_POLLS, PortNumber, Window, bridge, bring_up, glue, root_port};

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
    assert_eq!(bridge::translated_size_bytes(0x4000_002f), Some(0x100_0000));
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
            Event::Write(Block::Bridge, bridge::OUTBOUND_SOURCE_LOW, _)
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
        .filter(|event| matches!(event, Event::Write(Block::Bridge, bridge::MISCELLANEOUS, _)))
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
            | Event::Write(
                Block::SystemTopGroupController,
                crate::physical_layer::MODE,
                _
            )
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
            device: 0,
            function: 0
        }
    );
    assert_eq!(after[1].class, CLASS_NON_VOLATILE_MEMORY_EXPRESS);
}

#[test]
fn the_non_volatile_memory_express_bar_sizes_through_the_configured_port() {
    let mut simulated = Simulated::new(PortNumber::One);
    let port = PortNumber::One.mainline();
    bring_up(&mut simulated, &port).unwrap();
    configure(&mut simulated, port.memory_32_bit);
    let bar0 = Bdf {
        bus: 1,
        device: 0,
        function: 0,
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

#[test]
fn register_fields_hold_the_values_linux_names() {
    // Linux's spellings, from `pcie-plda.h` and `pcie-starfive.c` at the cited commit, as numbers.
    // The simulation reads these same constants, so only a literal here pins them.
    use crate::physical_layer;
    assert_eq!(bridge::ROOT_PORT_ENABLE, 0x1);
    assert_eq!(bridge::CLASS_PCI_TO_PCI_BRIDGE, 0x0604_0000);
    assert_eq!(bridge::PHYSICAL_FUNCTION_DISABLE, 0x8000);
    assert_eq!(bridge::PREFETCH_WINDOW_64_BIT, 0x8);
    assert_eq!(bridge::LATENCY_TOLERANCE_REPORTING_RECEPTION, 0x4);
    assert_eq!(bridge::LEGACY_INTERRUPTS, 0x0f00_0000);
    assert_eq!(bridge::MESSAGE_SIGNALED_INTERRUPT, 0x1000_0000);
    assert_eq!(bridge::ROOT_PORT_BAR0, 0x1010);
    assert_eq!(bridge::ROOT_PORT_BAR1, 0x1014);
    assert_eq!(bridge::TRANSLATION_ENABLE, 0x1);
    assert_eq!(glue::READ_CHANNEL_MASK, 0x007f_ff00);
    assert_eq!(glue::CLOCK_REQUEST, 0x0040_0000);
    assert_eq!(glue::REFERENCE_CLOCK_SOURCE, 0x0008_0000);
    assert_eq!(physical_layer::USB3_PHYSICAL_LAYER_ENABLE, 0x10);
    assert_eq!(physical_layer::MODE_MASK, 0x0030_0000);
    assert_eq!(physical_layer::USB3_BUS_WIDTH, 0x8);
    assert_eq!(physical_layer::USB_POWER_DOWN_RESET_SPLIT, 0x0002_0000);
}

#[test]
fn each_port_has_its_own_glue_block_and_physical_layer() {
    assert_eq!(PortNumber::Zero.system_top_group_controller_base(), 0x48);
    assert_eq!(PortNumber::One.system_top_group_controller_base(), 0x1f8);
    assert_eq!(
        PortNumber::Zero.physical_layer_physical_address(),
        0x1021_0000
    );
    assert_eq!(
        PortNumber::One.physical_layer_physical_address(),
        0x1022_0000
    );
}

#[test]
fn window_encoder_takes_the_smallest_window_and_keeps_only_whole_pages_of_the_source() {
    let entry = bridge::outbound_window(2, 0x9_8000_0abc, 0x9_8000_0000, 0x1000).unwrap();
    assert_eq!(
        entry,
        [
            (0x850, bridge::INTERFACE_TRANSMIT_RECEIVE),
            (0x840, 0x8000_0000 | (11 << 1) | 1),
            (0x844, 0x9),
            (0x848, 0x8000_0000),
            (0x84c, 0x9),
        ]
    );
}

#[test]
fn a_disabled_entry_translates_nothing() {
    assert_eq!(bridge::translated_size_bytes(0), None);
    assert_eq!(bridge::translated_size_bytes(0x4000_002e), None);
    assert_eq!(bridge::translated_size_bytes(0x1), Some(0x2));
}

#[test]
fn a_window_may_end_exactly_at_four_gibibytes() {
    assert_eq!(
        root_port::memory_base_and_limit(Window {
            physical_address: 0xfff0_0000,
            size_bytes: 0x10_0000,
        }),
        Some(0xfff0_fff0)
    );
}

#[test]
fn configure_keeps_the_command_bits_and_writes_no_status_bit() {
    use pci_configuration_space::{BUS_NUMBERS_REGISTER, COMMAND_REGISTER};
    let mut writes = Vec::new();
    let mut read = |_: Bdf, offset: u64| match offset {
        0 => 0x0000_1556,
        // Header type 1 in bits 23:16 of the word at 0x0c.
        0x0c => 0x0001_0000,
        // A status bit set in the high half, which a write of one would clear.
        o if o == COMMAND_REGISTER => 0x0010_0004,
        o if o == BUS_NUMBERS_REGISTER => 0,
        _ => 0,
    };
    let configured = root_port::configure(
        &mut read,
        &mut |_, offset, value| writes.push((offset, value)),
        Window {
            physical_address: 0x3800_0000,
            size_bytes: 0x800_0000,
        },
    );
    assert!(matches!(
        configured,
        root_port::Configured::Forwarding { .. }
    ));
    assert_eq!(writes.last(), Some(&(COMMAND_REGISTER, 0x0000_0006)));
}

#[test]
fn port_one_selects_each_function_in_its_own_glue_block() {
    let mut simulated = Simulated::new(PortNumber::One);
    bring_up(&mut simulated, &PortNumber::One.mainline()).unwrap();
    // 0x1f8 + 0x78 and 0x1f8 + 0x7c: the read and write channel words of port 1's glue.
    let channel_writes: Vec<(u32, u32)> = simulated
        .events
        .iter()
        .filter_map(|event| match *event {
            Event::Write(Block::SystemTopGroupController, offset @ (0x270 | 0x274), value) => {
                Some((offset, value))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        channel_writes,
        [
            (0x274, 0x0008_0000),
            (0x274, 0x0048_0000),
            (0x270, 0x0002_0000),
            (0x274, 0x0048_0200),
            (0x270, 0x0004_0000),
            (0x274, 0x0048_0400),
            (0x270, 0x0006_0000),
            (0x274, 0x0048_0600),
            (0x270, 0),
            (0x274, 0x0048_0000),
        ]
    );
}

#[test]
fn port_zero_drives_its_reset_pin_in_the_third_byte() {
    // Port 0's PERST# is pin 26: the word at 24, bits 23:16. Every other bit starts set, and the
    // bring-up must leave each one as it found it.
    let mut simulated = Simulated::new(PortNumber::Zero);
    for word in [0x18, 0x58] {
        simulated
            .words
            .insert((Block::GeneralPurposeInputOutput, word), 0xffff_ffff);
    }
    let _ = bring_up(&mut simulated, &PortNumber::Zero.mainline());
    let pin_writes: Vec<(u32, u32)> = simulated
        .events
        .iter()
        .filter_map(|event| match *event {
            Event::Write(Block::GeneralPurposeInputOutput, offset, value) => Some((offset, value)),
            _ => None,
        })
        .collect();
    assert_eq!(
        pin_writes,
        [
            (0x58, 0xff80_ffff),
            (0x18, 0xffc0_ffff),
            (0x58, 0xff81_ffff),
            (0x18, 0xffc0_ffff),
        ]
    );
}

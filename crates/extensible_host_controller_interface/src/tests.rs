//! The crate's host tests, kept out of `lib.rs` so the crate root stays an index of its modules.

extern crate std;
use super::*;
use crate::trb::*;

fn qemu() -> Capabilities {
    Capabilities::decode(0x0100_0040, 0x0400_0840, 0, 0x0000_7000, 0x2000, 0x1000)
}

#[test]
fn qemus_window_withholds_the_msix_page_and_nothing_else() {
    assert_eq!(
        register_window(&qemu(), 0x4000, &[(0x3000, 0x3100), (0x3800, 0x3808)]),
        Ok(0b0111)
    );
    // No MSI-X at all (a function delivering INTx): every page.
    assert_eq!(register_window(&qemu(), 0x4000, &[]), Ok(0b1111));
    // A withheld range is half open: one that ends where page 4 starts leaves page 4 mapped.
    assert_eq!(
        register_window(&qemu(), 0x5000, &[(0x3000, 0x4000)]),
        Ok(0b1_0111)
    );
}

/// **The refusal that makes the rule checkable**: a controller whose doorbells share a page
/// with its MSI-X table cannot be handed to a driver at all, because mapping the doorbells
/// would map the table.
#[test]
fn a_needed_register_on_the_interrupt_page_is_refused() {
    assert_eq!(
        register_window(&qemu(), 0x4000, &[(0x2000, 0x2100)]),
        Err(WindowRefusal::SharesTheInterruptPage { offset: 0x2000 })
    );
}

#[test]
fn a_needed_register_beyond_the_bar_is_refused() {
    assert_eq!(
        register_window(&qemu(), 0x2000, &[]),
        Err(WindowRefusal::BeyondWindow { offset: 0x2000 })
    );
}

/// An Intel-shaped controller: a 64 KiB BAR, runtime at 0x2000, doorbells at 0x3000, the
/// MSI-X table well above them. Values from the xHCI capability layout Intel's datasheets
/// describe, not read from xenon (a bench step on xenon reads its own).
#[test]
fn a_sixty_four_kibibyte_bar_maps_sixteen_pages_less_the_table() {
    let caps = Capabilities::decode(0x0100_0080, 0x1a00_0840, 0, 0x0000_7000, 0x3000, 0x2000);
    let mask = register_window(&caps, 0x1_0000, &[(0xa000, 0xa000 + 16 * 8)]).unwrap();
    assert_eq!(mask, 0xffff & !(1 << 10));
}

#[test]
fn the_handoff_round_trips_and_refuses_what_no_kernel_sends() {
    let h = Handoff {
        dma_phys: 0x4000_0000,
        dma_pages: 12,
        registers: 0b0111,
    };
    assert_eq!(Handoff::unpack(h.pack()), Some(h));
    assert_eq!(Handoff::unpack([0x4000_0001, 12 | 7 << 32, 0]), None);
    assert_eq!(Handoff::unpack([0, 12 | 7 << 32, 0]), None);
    assert_eq!(Handoff::unpack([0x4000_0000, 3 | 7 << 32, 0]), None);
    assert_eq!(Handoff::unpack([0x4000_0000, 12 | 6 << 32, 0]), None);
}

#[test]
fn events_decode_by_type() {
    // A command completion for Enable Slot, success, slot 1.
    let t = Trb([0x1000, 0, 1 << 24, 33 << 10 | 1 << 24 | 1]);
    assert_eq!(
        Event::decode(t),
        Event::Command {
            trb: 0x1000,
            code: completion::SUCCESS,
            slot: 1
        }
    );
    // A transfer event on endpoint 3 (EP1 IN), short by 2.
    let t = Trb([0x7000, 0, 13 << 24 | 2, 32 << 10 | 3 << 16 | 1 << 24 | 1]);
    assert_eq!(
        Event::decode(t),
        Event::Transfer {
            trb: 0x7000,
            residual: 2,
            code: completion::SHORT_PACKET,
            slot: 1,
            endpoint: 3
        }
    );
    let t = Trb([5 << 24, 0, 1 << 24, 34 << 10 | 1]);
    assert_eq!(Event::decode(t), Event::PortStatus { port: 5 });
    // A TRB above 4 GiB keeps its high half.
    let t = Trb([0x1000, 0x2, 1 << 24, 33 << 10 | 1 << 24 | 1]);
    assert!(matches!(
        Event::decode(t),
        Event::Command {
            trb: 0x2_0000_1000,
            ..
        }
    ));
}

/// The event ring's consumer flips its cycle at the wrap, so last lap's events read as stale.
#[test]
fn the_event_ring_laps() {
    let mut ring = EventRing::new(0x2000, 2);
    let fresh = Trb([0, 0, 0, 1]);
    assert!(ring.is_ready(&fresh));
    ring.advance();
    assert_eq!(ring.index(), 1);
    assert_eq!(ring.dequeue_pointer(), 0x2010);
    ring.advance();
    assert_eq!(ring.dequeue_pointer(), 0x2000);
    assert!(!ring.is_ready(&fresh));
    assert_eq!(ring.segment_table_entry(), [0x2000, 0, 2, 0]);
    // A ring above 4 GiB: the table entry's second dword is the base's high half.
    let high = EventRing::new(0x3_0000_2000, 4);
    assert_eq!(high.segment_table_entry(), [0x2000, 3, 4, 0]);
}

#[test]
fn a_setup_trb_carries_the_packet_and_the_transfer_type() {
    let t = Trb::setup(0x0008_0000_0100_0680, Some(true));
    assert_eq!(t.0[0], 0x0100_0680);
    assert_eq!(t.0[1], 0x0008_0000);
    assert_eq!(t.0[2], 8);
    assert_eq!(t.kind(), trb_type::SETUP);
    assert_eq!((t.0[3] >> 16) & 3, 3);
    assert!(t.0[3] & IDT != 0);
    // The status stage of an IN transfer flows OUT, and of a no-data transfer flows IN.
    assert_eq!((Trb::status(true).0[3] >> 16) & 1, 0);
    assert_eq!((Trb::status(false).0[3] >> 16) & 1, 1);
}

#[test]
fn the_interval_field_follows_the_speed() {
    // Full speed, 10 ms: 80 microframes, so 2^6 = 64 is the largest period not over it.
    assert_eq!(interrupt_interval(port::speed::FULL, 10), 6);
    assert_eq!(interrupt_interval(port::speed::LOW, 1), 3);
    assert_eq!(interrupt_interval(port::speed::FULL, 255), 10);
    assert_eq!(interrupt_interval(port::speed::FULL, 0), 3);
    // High speed states an exponent plus one.
    assert_eq!(interrupt_interval(port::speed::HIGH, 4), 3);
    assert_eq!(interrupt_interval(port::speed::HIGH, 0), 0);
    assert_eq!(interrupt_interval(port::speed::SUPER, 200), 15);
}

/// **A simulated controller consuming what the driver produces**: it reads from its dequeue
/// index while the cycle bit matches its own, follows a link TRB and toggles its cycle when the
/// link says so (xHCI 1.2 section 4.9.2), and returns what it ran. Three laps of a four-entry
/// ring, so every wrap and every link is exercised: the controller must run exactly the TRBs
/// produced, in order, and stop where production stops.
#[test]
fn a_simulated_controller_runs_exactly_what_was_produced_across_laps() {
    const ENTRIES: u16 = 4;
    let base = 0x8000;
    let mut memory = [Trb([0; 4]); ENTRIES as usize];
    let mut ring = ProducerRing::new(base, ENTRIES);
    let (mut dequeue, mut ccs) = (0u16, true);
    let run = |memory: &[Trb; ENTRIES as usize], dequeue: &mut u16, ccs: &mut bool| {
        let mut ran = std::vec::Vec::new();
        for _ in 0..16 {
            let t = memory[*dequeue as usize];
            if t.cycle() != *ccs {
                break;
            }
            if t.kind() == trb_type::LINK {
                let target = u64::from(t.0[0]) | u64::from(t.0[1]) << 32;
                assert_eq!(target, base, "a link leads back to the ring's start");
                if t.0[3] & 1 << 1 != 0 {
                    *ccs = !*ccs;
                }
                *dequeue = 0;
                continue;
            }
            ran.push(t.0[0]);
            *dequeue += 1;
        }
        ran
    };
    let mut next = 1u32;
    // Never more than the ring's three usable entries between two runs: a fourth would overwrite
    // a TRB the controller has not reached, which is the producer's rule, not the ring's.
    for burst in [2u32, 3, 1, 3, 2, 3] {
        let mut produced = std::vec::Vec::new();
        for _ in 0..burst {
            let p = ring.enqueue(Trb::normal(u64::from(next), 8));
            memory[p.index as usize] = p.trb;
            if let Some((at, link)) = p.link {
                memory[at as usize] = link;
            }
            produced.push(next);
            next += 1;
        }
        assert_eq!(run(&memory, &mut dequeue, &mut ccs), produced);
        // Where the controller stopped is where the driver will write next.
        assert_eq!(ring.dequeue_pointer() & !1, base + u64::from(dequeue) * 16);
        assert_eq!(ring.dequeue_pointer() & 1 != 0, ccs);
    }
}

/// **The three shapes of a control transfer**, as a controller reads them: the setup stage's
/// transfer type must agree with the data stage's direction, and the status stage flows the
/// other way (xHCI 1.2 section 4.11.2.2). Only the status stage asks for an event on success,
/// which is what the driver waits for; the data stage asks only on a short packet.
#[test]
fn control_transfers_agree_on_direction_in_all_three_shapes() {
    for (data_in, trt, status_in) in [
        (None, 0, true),
        (Some(false), 2, true),
        (Some(true), 3, false),
    ] {
        let setup = Trb::setup(0, data_in);
        assert_eq!((setup.0[3] >> 16) & 3, trt);
        let status = Trb::status(data_in == Some(true));
        assert_eq!(status.kind(), trb_type::STATUS);
        assert_eq!((status.0[3] >> 16) & 1 != 0, status_in);
        assert!(status.0[3] & IOC != 0);
        if let Some(inbound) = data_in {
            let data = Trb::data(0x1_2345_6000, 18, inbound);
            assert_eq!(data.kind(), trb_type::DATA);
            assert_eq!((data.0[0], data.0[1], data.0[2]), (0x2345_6000, 1, 18));
            assert_eq!((data.0[3] >> 16) & 1 != 0, inbound);
            assert_eq!((data.0[3] & IOC, data.0[3] & ISP != 0), (0, true));
        }
    }
    let normal = Trb::normal(0x4000, 8);
    assert_eq!((normal.kind(), normal.0[2]), (trb_type::NORMAL, 8));
    assert!(normal.0[3] & IOC != 0 && normal.0[3] & ISP != 0);
}

/// **Every command names its slot where a controller looks for it** (bits 31:24 of the last
/// dword), the pointer commands carry their input context, and the endpoint commands their
/// endpoint (bits 20:16). A simulated command dispatcher decodes each one back.
#[test]
fn every_command_decodes_back_to_its_slot_endpoint_and_pointer() {
    let decode = |t: Trb| {
        (
            t.kind(),
            (t.0[3] >> 24) as u8,
            ((t.0[3] >> 16) & 0x1f) as u8,
            u64::from(t.0[0]) | u64::from(t.0[1]) << 32,
        )
    };
    assert_eq!(decode(Trb::enable_slot()), (trb_type::ENABLE_SLOT, 0, 0, 0));
    assert_eq!(
        decode(Trb::disable_slot(7)),
        (trb_type::DISABLE_SLOT, 7, 0, 0)
    );
    assert_eq!(
        decode(Trb::address_device(0x5000, 1)),
        (trb_type::ADDRESS_DEVICE, 1, 0, 0x5000)
    );
    assert_eq!(
        decode(Trb::configure_endpoint(0x5000, 2)),
        (trb_type::CONFIGURE_ENDPOINT, 2, 0, 0x5000)
    );
    assert_eq!(
        decode(Trb::evaluate_context(0x5000, 3)),
        (trb_type::EVALUATE_CONTEXT, 3, 0, 0x5000)
    );
    assert_eq!(
        decode(Trb::reset_endpoint(1, 3)),
        (trb_type::RESET_ENDPOINT, 1, 3, 0)
    );
    assert_eq!(
        decode(Trb::set_dequeue(0x7001, 1, 3)),
        (trb_type::SET_TR_DEQUEUE, 1, 3, 0x7001)
    );
    // An event type this driver does not act on decodes as itself, never as a known one.
    assert_eq!(
        Event::decode(Trb([0, 0, 0, 37 << 10])),
        Event::Other { kind: 37 }
    );
    let ring = EventRing::new(0x2000, 4);
    assert_eq!(ring.index(), 0);
}

/// A doorbell range that runs onto a withheld page is refused at the first needed byte on it,
/// and so is a needed range that starts on one.
#[test]
fn a_needed_range_crossing_onto_a_withheld_page_is_refused() {
    // 64 slots of doorbells from 0x2f00 run to 0x3003, onto page 3.
    let caps = Capabilities::decode(0x0100_0040, 0x0400_0840, 0, 0, 0x2f00, 0x1000);
    assert_eq!(
        register_window(&caps, 0x4000, &[(0x3000, 0x3100)]),
        Err(WindowRefusal::SharesTheInterruptPage { offset: 0x3000 })
    );
}

#[test]
fn a_producer_ring_of_two_links_every_time() {
    let mut ring = ProducerRing::new(0x9000, 2);
    let p = ring.enqueue(Trb::enable_slot());
    assert_eq!(p.index, 0);
    let (at, link) = p.link.unwrap();
    assert_eq!(at, 1);
    assert_eq!(link.kind(), trb_type::LINK);
    assert_eq!(link.0[0], 0x9000);
    assert_eq!(ring.dequeue_pointer(), 0x9000); // back at 0, cycle now 0
}

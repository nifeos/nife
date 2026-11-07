//! The crate's host tests, kept out of `lib.rs` so the crate root stays an index of its modules as
//! the tests grow.
//!
//! Name: ratified 2026-11-03 (UTC), #3135. The crate's host tests.

use crate::card;
use crate::host_controller::{Error, HostController};
use crate::partition::MasterBootRecord;
use crate::sd::{self, BLOCK_BYTES, BlockTransferError, InitializationError};
use crate::simulator::{CardKind, SimulatedHostController};

fn host(kind: CardKind, blocks: usize) -> HostController<SimulatedHostController> {
    let (h, id) = HostController::new(
        SimulatedHostController::new(kind, blocks, 32),
        50_000_000,
        Some(32),
    )
    .unwrap();
    assert_eq!(id.version_identification, crate::simulator::VERID);
    h
}

fn master_boot_record_card(blocks: usize, first: u32) -> SimulatedHostController {
    let mut s = SimulatedHostController::new(CardKind::SdHighCapacity, blocks, 32);
    let m = &mut s.storage[0];
    m[446 + 4] = 0x0b;
    m[446 + 8..446 + 12].copy_from_slice(&first.to_le_bytes());
    m[446 + 12..446 + 16].copy_from_slice(&(blocks as u32 - first).to_le_bytes());
    m[510] = 0x55;
    m[511] = 0xaa;
    let fat = &mut s.storage[first as usize];
    fat[82..90].copy_from_slice(b"FAT32   ");
    fat[510] = 0x55;
    fat[511] = 0xaa;
    s
}

#[test]
fn an_sdhc_card_is_identified_selected_and_widened_in_the_specification_order() {
    let mut h = host(CardKind::SdHighCapacity, 8192);
    let card = sd::identify(&mut h, 4).unwrap();
    assert_eq!(card.kind, sd::CardKind::SdHighCapacity);
    assert!(card.block_addressed);
    assert_eq!(card.blocks, 8192);
    assert_eq!((card.bus_width, card.clock_hertz), (4, 25_000_000));
    assert_eq!(&card.card_identification.name[..5], b"TEST1");
    let sim = h.into_registers();
    let order: Vec<u8> = sim.commands.iter().map(|c| c.0).collect();
    // CMD0, CMD8, then ACMD41 until ready (three busy polls), CID, RCA, CSD, select, ACMD6,
    // block length.
    assert_eq!(
        order,
        [0, 8, 55, 41, 55, 41, 55, 41, 55, 41, 2, 3, 9, 7, 55, 6, 16]
    );
    assert!(
        !sim.clockless_command,
        "a command went out with the card clock off"
    );
}

#[test]
fn an_old_sd_card_is_byte_addressed_and_an_emmc_is_told_apart_by_its_silence() {
    let mut h = host(CardKind::SdV1, 4096);
    let card = sd::identify(&mut h, 4).unwrap();
    assert_eq!(card.kind, sd::CardKind::SdV1);
    assert!(!card.block_addressed);
    assert_eq!(card.block_address_argument(3), 3 * 512);

    let mut h = host(CardKind::MultimediaCard, 16_384);
    let card = sd::identify(&mut h, 8).unwrap();
    assert_eq!(card.kind, sd::CardKind::MultimediaCard);
    assert_eq!(
        card.relative_card_address,
        sd::MULTIMEDIA_CARD_RELATIVE_CARD_ADDRESS
    );
    assert!(card.block_addressed);
    // The size came from EXT_CSD, not the CSD's placeholder.
    assert_eq!(card.blocks, 16_384);
    assert_eq!(card.bus_width, 1);
}

#[test]
fn an_empty_slot_is_no_card_not_a_hang() {
    let mut h = host(CardKind::Empty, 1024);
    assert!(matches!(
        sd::identify(&mut h, 4),
        Err(InitializationError::NoCard(Error::NoResponse(1)))
    ));
}

#[test]
fn blocks_written_read_back_and_multi_block_transfers_stop_by_themselves() {
    let mut h = host(CardKind::SdHighCapacity, 8192);
    let card = sd::identify(&mut h, 4).unwrap();
    let data: Vec<u8> = (0..8 * BLOCK_BYTES).map(|i| (i * 7 % 251) as u8).collect();
    sd::write_blocks(&mut h, &card, 100, &data).unwrap();
    let mut back = vec![0u8; 8 * BLOCK_BYTES];
    sd::read_blocks(&mut h, &card, 100, &mut back).unwrap();
    assert_eq!(back, data);
    let mut one = [0u8; BLOCK_BYTES];
    sd::read_blocks(&mut h, &card, 103, &mut one).unwrap();
    assert_eq!(one[..], data[3 * BLOCK_BYTES..4 * BLOCK_BYTES]);
    let sim = h.into_registers();
    assert_eq!(sim.written, (100..108).collect::<Vec<u64>>());
    // CMD25 and CMD18 each followed by the controller's own CMD12; CMD17 not.
    let tail: Vec<u8> = sim.commands.iter().rev().take(6).map(|c| c.0).collect();
    assert_eq!(tail, [17, 12, 18, 13, 12, 25]);
}

#[test]
fn a_transfer_that_does_not_fit_the_card_or_the_buffer_is_refused_before_anything_is_sent() {
    let mut h = host(CardKind::SdHighCapacity, 8192);
    let card = sd::identify(&mut h, 4).unwrap();
    let sent = h.registers_mut().commands.len();
    let mut buffer = [0u8; BLOCK_BYTES];
    assert_eq!(
        sd::read_blocks(&mut h, &card, 8192, &mut buffer),
        Err(BlockTransferError::Range)
    );
    assert_eq!(
        sd::read_blocks(&mut h, &card, 0, &mut buffer[..100]),
        Err(BlockTransferError::Length)
    );
    assert_eq!(
        sd::write_blocks(&mut h, &card, u64::MAX, &buffer),
        Err(BlockTransferError::Range)
    );
    assert_eq!(h.registers_mut().commands.len(), sent);
}

#[test]
fn a_controller_before_2_40a_reads_through_its_queue_at_0x100_and_gets_no_threshold_write() {
    let sim = SimulatedHostController::new(CardKind::SdHighCapacity, 8192, 32)
        .with_version_identification(0x5342_230a);
    let (mut h, id) = HostController::new(sim, 50_000_000, Some(32)).unwrap();
    assert_eq!(
        crate::registers::queue_offset(id.version_identification),
        0x100
    );
    let card = sd::identify(&mut h, 4).unwrap();
    let data: Vec<u8> = (0..2 * BLOCK_BYTES).map(|i| (i % 253) as u8).collect();
    sd::write_blocks(&mut h, &card, 7, &data).unwrap();
    let mut back = vec![0u8; 2 * BLOCK_BYTES];
    // A threshold write to 0x100 here would land in the FIFO and shift every word read.
    sd::read_blocks(&mut h, &card, 7, &mut back).unwrap();
    assert_eq!(back, data);
    assert!(!h.into_registers().stray_queue_write);
}

#[test]
fn a_read_whose_tail_is_below_the_watermark_is_drained_on_transfer_over() {
    // 24 words deep: a 128-word block arrives as five fills of 24 and a tail of 8, below the
    // receive watermark of 11, so the tail raises DTO and never RXDR.
    let mut sim = SimulatedHostController::new(CardKind::SdHighCapacity, 8192, 24);
    sim.strict_watermark = true;
    sim.storage[5] = core::array::from_fn(|i| (i % 249) as u8);
    let (mut h, _) = HostController::new(sim, 50_000_000, Some(24)).unwrap();
    let card = sd::identify(&mut h, 4).unwrap();
    let mut one = [0u8; BLOCK_BYTES];
    sd::read_blocks(&mut h, &card, 5, &mut one).unwrap();
    assert_eq!(one, h.registers_mut().storage[5]);
}

#[test]
fn a_data_crc_error_is_reported_with_its_command() {
    let mut h = host(CardKind::SdHighCapacity, 8192);
    let card = sd::identify(&mut h, 4).unwrap();
    h.registers_mut().fail_next_read_crc = true;
    let mut buffer = [0u8; BLOCK_BYTES];
    assert!(matches!(
        sd::read_blocks(&mut h, &card, 0, &mut buffer),
        Err(BlockTransferError::Host(Error::Data(17, rint))) if rint & crate::registers::RINTSTS_DCRC != 0
    ));
}

/// Identification, sector 0 and the first partition's boot sector are read, a long sequential read
/// follows, and no write command is sent: the order a read-only look at a card takes.
#[test]
fn a_card_is_identified_and_read_through_its_partition_table_without_a_write() {
    let (mut h, _) =
        HostController::new(master_boot_record_card(8192, 2048), 50_000_000, Some(32)).unwrap();
    let card = sd::identify(&mut h, 4).unwrap();
    let mut sector = [0u8; BLOCK_BYTES];
    sd::read_blocks(&mut h, &card, 0, &mut sector).unwrap();
    let table = MasterBootRecord::parse(&sector).unwrap();
    assert_eq!(table.entries[0].start, 2048);
    sd::read_blocks(&mut h, &card, 2048, &mut sector).unwrap();
    assert_eq!(sector[510..512], [0x55, 0xaa]);
    assert_eq!(&sector[82..90], b"FAT32   ");
    let mut buffer = vec![0u8; 128 * BLOCK_BYTES];
    for k in 0..8u64 {
        sd::read_blocks(&mut h, &card, 2048 + 128 * k, &mut buffer).unwrap();
    }
    let sim = h.into_registers();
    assert!(sim.written.is_empty());
    assert!(
        !sim.commands.iter().any(|c| matches!(c.0, 24 | 25)),
        "a read-only sequence sent a write command"
    );
}

/// Reading the same blocks at each card clock and multi-block length, identifying the card again
/// before every read, returns the right bytes every time and writes nothing.
#[test]
fn the_same_blocks_read_back_at_every_clock_and_length_after_a_fresh_identification() {
    let sim = master_boot_record_card(8192, 2048);
    let want = sim.storage[2048..2048 + 128].concat();
    let (mut h, _) = HostController::new(sim, 50_000_000, Some(32)).unwrap();
    let mut buffer = vec![0u8; 128 * BLOCK_BYTES];
    let mut reads = 0;
    for hertz in [
        sd::TRANSFER_HERTZ,
        sd::TRANSFER_HERTZ / 2,
        sd::TRANSFER_HERTZ / 4,
    ] {
        for blocks in [2usize, 8, 32, 128] {
            let card = sd::identify(&mut h, 4).unwrap();
            let card_hertz = h.set_clock(hertz).unwrap();
            assert!(card_hertz <= hertz, "{card_hertz} Hz for {hertz} Hz");
            let dst = &mut buffer[..blocks * BLOCK_BYTES];
            sd::read_blocks(&mut h, &card, 2048, dst).unwrap();
            assert_eq!(dst, &want[..blocks * BLOCK_BYTES]);
            reads += 1;
        }
    }
    assert_eq!(reads, 12);
    assert!(h.into_registers().written.is_empty());
}

/// A multi-block write confined to the sectors before the first partition, read back, then
/// restored: the gap ends as it began and no sector outside it is written.
#[test]
fn a_write_confined_to_the_gap_is_read_back_and_restored_exactly() {
    let mut sim = master_boot_record_card(8192, 2048);
    // Something already in the gap: the restore must put it back, not zero it.
    sim.storage[2043][0] = 0x42;
    let before = sim.storage.clone();
    let (mut h, _) = HostController::new(sim, 50_000_000, Some(32)).unwrap();
    let card = sd::identify(&mut h, 4).unwrap();
    let mut sector = [0u8; BLOCK_BYTES];
    sd::read_blocks(&mut h, &card, 0, &mut sector).unwrap();
    let table = MasterBootRecord::parse(&sector).unwrap();
    let (first, count) = table.scratch_sector_range(8).unwrap();
    assert_eq!((first, count), (2040, 8));
    let bytes = count as usize * BLOCK_BYTES;
    let first = u64::from(first);
    let (mut original, mut pattern, mut back) =
        (vec![0u8; bytes], vec![0u8; bytes], vec![0u8; bytes]);
    sd::read_blocks(&mut h, &card, first, &mut original).unwrap();
    for (i, b) in pattern.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(31) ^ 0x5a;
    }
    sd::write_blocks(&mut h, &card, first, &pattern).unwrap();
    sd::read_blocks(&mut h, &card, first, &mut back).unwrap();
    assert_eq!(back, pattern);
    sd::write_blocks(&mut h, &card, first, &original).unwrap();
    sd::read_blocks(&mut h, &card, first, &mut back).unwrap();
    assert_eq!(back, original);
    let sim = h.into_registers();
    assert!(sim.written.iter().all(|&lba| (2040..2048).contains(&lba)));
    assert_eq!(sim.written.len(), 16);
    assert_eq!(sim.storage, before);
}

#[test]
fn a_sixteen_bit_queue_is_refused_at_the_door() {
    // A controller whose HCON says 16-bit, over the sim.
    struct Narrow(SimulatedHostController);
    impl crate::host_controller::RegisterWindowAndClock for Narrow {
        fn read(&mut self, o: u32) -> u32 {
            if o == crate::registers::HCON {
                0
            } else {
                self.0.read(o)
            }
        }
        fn write(&mut self, o: u32, v: u32) {
            self.0.write(o, v);
        }
        fn now_microseconds(&mut self) -> u64 {
            self.0.now_microseconds()
        }
    }
    let narrow = Narrow(SimulatedHostController::new(
        CardKind::SdHighCapacity,
        1024,
        32,
    ));
    assert!(matches!(
        HostController::new(narrow, 50_000_000, None),
        Err(Error::QueueWidth(0))
    ));
}

#[test]
fn without_a_tree_depth_the_queue_depth_is_read_from_its_reset_watermark() {
    let (h, _) = HostController::new(
        SimulatedHostController::new(CardKind::SdHighCapacity, 1024, 16),
        50_000_000,
        None,
    )
    .unwrap();
    assert_eq!(h.queue_depth(), 16);
}

// **The error paths.** Each test below injects one fault the sim can produce and checks that
// the driver names the step it happened at, because a board transcript that stops is only as
// useful as what its last line says.

use crate::simulator::Fault;

fn faulty(
    kind: CardKind,
    blocks: usize,
    faults: &[(u8, usize, Fault)],
) -> HostController<SimulatedHostController> {
    let mut sim = SimulatedHostController::new(kind, blocks, 32);
    sim.faults = faults.to_vec();
    HostController::new(sim, 50_000_000, Some(32)).unwrap().0
}

#[test]
fn an_sd_2_0_standard_capacity_card_answers_the_echo_and_is_still_byte_addressed() {
    let mut h = host(CardKind::SdV2, 4096);
    let card = sd::identify(&mut h, 4).unwrap();
    assert_eq!(card.kind, sd::CardKind::SdV2);
    assert!(!card.block_addressed);
    // It answered CMD8 but did not set CCS in its OCR, so it is not mistaken for SDHC.
    assert_eq!(card.block_address_argument(2), 1024);
}

#[test]
fn a_wrong_interface_echo_is_refused_rather_than_guessed_at() {
    let mut h = faulty(CardKind::SdHighCapacity, 8192, &[(8, 0, Fault::EchoWrong)]);
    assert!(matches!(
        sd::identify(&mut h, 4),
        Err(InitializationError::InterfaceCondition(r)) if r & 0xfff != 0x1aa
    ));
}

/// A worker with its own deadline turns a spin into a named failure: a mutant that removes the
/// power-up loop's exit makes this test fail instead of hanging the run.
#[test]
#[cfg_attr(
    miri,
    ignore = "a ten-second wall-clock watchdog; the interpreted spin to the simulated deadline outlasts it"
)]
fn a_card_that_never_finishes_power_up_is_reported_with_its_last_operation_conditions() {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for kind in [CardKind::SdHighCapacity, CardKind::MultimediaCard] {
            let mut sim = SimulatedHostController::new(kind, 16_384, 32);
            sim.busy_polls = u32::MAX;
            let mut h = HostController::new(sim, 50_000_000, Some(32)).unwrap().0;
            let busy_done = card::OPERATION_CONDITIONS_BUSY_DONE;
            let reported = matches!(
                sd::identify(&mut h, 4),
                Err(InitializationError::PowerUp(conditions)) if conditions & busy_done == 0
            );
            let _ = sender.send(reported);
        }
    });
    for _ in 0..2 {
        assert_eq!(
            receiver.recv_timeout(std::time::Duration::from_secs(5)),
            Ok(true),
            "power-up neither finished nor gave up"
        );
    }
}

#[test]
fn an_sd_2_0_card_silent_to_its_first_app_command_is_no_card_not_an_emmc() {
    let mut h = faulty(
        CardKind::SdHighCapacity,
        8192,
        &[(55, 0, Fault::NoResponse)],
    );
    assert!(matches!(
        sd::identify(&mut h, 4),
        Err(InitializationError::NoCard(Error::NoResponse(55)))
    ));
}

#[test]
fn each_identification_step_names_itself_when_the_card_goes_silent() {
    for index in [2u8, 3, 9, 7] {
        let mut h = faulty(
            CardKind::SdHighCapacity,
            8192,
            &[(index, 0, Fault::NoResponse)],
        );
        assert_eq!(
            sd::identify(&mut h, 4),
            Err(InitializationError::Identify(
                index,
                Error::NoResponse(index)
            )),
            "CMD{index}"
        );
    }
    let mut h = faulty(
        CardKind::MultimediaCard,
        16_384,
        &[(3, 0, Fault::NoResponse)],
    );
    assert_eq!(
        sd::identify(&mut h, 8),
        Err(InitializationError::Identify(3, Error::NoResponse(3)))
    );
}

#[test]
fn each_configuration_step_names_itself_when_the_card_goes_silent() {
    // ACMD41 takes four CMD55s (three busy polls), so the fifth is ACMD6's.
    for (index, nth) in [(55u8, 4usize), (6, 0), (16, 0)] {
        let mut h = faulty(
            CardKind::SdHighCapacity,
            8192,
            &[(index, nth, Fault::NoResponse)],
        );
        assert_eq!(
            sd::identify(&mut h, 4),
            Err(InitializationError::Configure(
                index,
                Error::NoResponse(index)
            )),
            "CMD{index}"
        );
    }
    // An eMMC's first CMD8 is the interface probe it never answers; its second is EXT_CSD.
    let mut h = faulty(
        CardKind::MultimediaCard,
        16_384,
        &[(8, 1, Fault::NoResponse)],
    );
    assert_eq!(
        sd::identify(&mut h, 8),
        Err(InitializationError::Configure(8, Error::NoResponse(8)))
    );
}

#[test]
fn an_error_bit_in_a_status_answer_stops_identification_at_that_command() {
    let mut h = faulty(
        CardKind::SdHighCapacity,
        8192,
        &[(7, 0, Fault::ErrorStatus)],
    );
    let unknown_error = 1 << 19;
    assert!(matches!(
        sd::identify(&mut h, 4),
        Err(InitializationError::Status(7, r)) if r & unknown_error != 0
    ));
    let mut h = faulty(
        CardKind::MultimediaCard,
        16_384,
        &[(3, 0, Fault::ErrorStatus)],
    );
    assert!(matches!(
        sd::identify(&mut h, 8),
        Err(InitializationError::Status(3, _))
    ));
    let mut h = faulty(
        CardKind::SdHighCapacity,
        8192,
        &[(16, 0, Fault::ErrorStatus)],
    );
    assert!(matches!(
        sd::identify(&mut h, 4),
        Err(InitializationError::Status(16, _))
    ));
}

#[test]
fn a_card_specific_data_this_driver_cannot_size_is_refused_with_its_structure() {
    let mut sim = SimulatedHostController::new(CardKind::SdHighCapacity, 8192, 32);
    sim.zero_capacity = true;
    let mut h = HostController::new(sim, 50_000_000, Some(32)).unwrap().0;
    assert_eq!(
        sd::identify(&mut h, 4),
        Err(InitializationError::Capacity(3))
    );
}

#[test]
fn a_status_error_on_a_transfer_is_reported_not_swallowed() {
    let mut h = faulty(
        CardKind::SdHighCapacity,
        8192,
        &[(17, 0, Fault::ErrorStatus)],
    );
    let card = sd::identify(&mut h, 4).unwrap();
    let mut one = [0u8; BLOCK_BYTES];
    assert!(matches!(
        sd::read_blocks(&mut h, &card, 0, &mut one),
        Err(BlockTransferError::Status(_))
    ));
    // A single-block write is CMD24; its R1, then the status after programming, are each
    // checked.
    let mut h = faulty(
        CardKind::SdHighCapacity,
        8192,
        &[(24, 0, Fault::ErrorStatus)],
    );
    let card = sd::identify(&mut h, 4).unwrap();
    assert!(matches!(
        sd::write_blocks(&mut h, &card, 9, &one),
        Err(BlockTransferError::Status(_))
    ));
    let mut h = faulty(
        CardKind::SdHighCapacity,
        8192,
        &[(13, 0, Fault::ErrorStatus)],
    );
    let card = sd::identify(&mut h, 4).unwrap();
    assert!(matches!(
        sd::write_blocks(&mut h, &card, 9, &one),
        Err(BlockTransferError::Status(_))
    ));
    let mut h = host(CardKind::SdHighCapacity, 8192);
    let card = sd::identify(&mut h, 4).unwrap();
    one[0] = 0x77;
    sd::write_blocks(&mut h, &card, 9, &one).unwrap();
    assert_eq!(h.registers_mut().storage[9][0], 0x77);
    assert_eq!(
        h.registers_mut().commands.iter().rev().nth(1).map(|c| c.0),
        Some(24)
    );
}

#[test]
fn a_byte_addressed_card_stops_where_a_32_bit_byte_address_does() {
    let last = (1u64 << 32) / BLOCK_BYTES as u64;
    assert!(sd::can_address_blocks(1 << 40, false, last - 1, 1));
    assert!(!sd::can_address_blocks(1 << 40, false, last, 1));
    assert!(sd::can_address_blocks(1 << 40, true, last, 1));
}

/// Sequential reads in whole 256-block commands walk up from a partition's start, one command
/// per step, and stop where the next would pass the card's end.
#[test]
fn sequential_long_reads_walk_up_from_the_partition_and_stop_at_the_card_end() {
    let (mut h, _) =
        HostController::new(master_boot_record_card(2048, 16), 50_000_000, Some(32)).unwrap();
    let card = sd::identify(&mut h, 4).unwrap();
    let mut buffer = vec![0u8; 256 * BLOCK_BYTES];
    let mut at = 16u64;
    while at + 256 <= card.blocks {
        sd::read_blocks(&mut h, &card, at, &mut buffer).unwrap();
        at += 256;
    }
    // Blocks 16 to 2047 fit seven 256-block reads and not eight.
    let reads: Vec<u32> = h
        .into_registers()
        .commands
        .iter()
        .filter(|c| c.0 == 18)
        .map(|c| c.1)
        .collect();
    assert_eq!(reads, (0..7).map(|k| 16 + 256 * k).collect::<Vec<u32>>());
}

/// An SD card's relative address goes in the high half of every addressed command's argument,
/// the `APP_CMD` before `ACMD6` among them; its CID is decoded with the SD layout and its
/// ACMD41 argument asks for high capacity only of a card that answered CMD8.
#[test]
fn an_sd_card_is_addressed_by_the_relative_card_address_it_chose_and_decoded_as_sd() {
    let mut h = host(CardKind::SdHighCapacity, 8192);
    let card = sd::identify(&mut h, 4).unwrap();
    assert_eq!(
        card.card_identification,
        card::CardIdentification {
            manufacturer: 0x1b,
            name: *b"TEST1\0",
            serial: 0x1234_5678,
        }
    );
    let sim = h.into_registers();
    let acmd6 = sim.commands.iter().position(|c| c.0 == 6).unwrap();
    assert_eq!(sim.commands[acmd6 - 1], (55, 0xb368 << 16));
    assert!(sim.commands.contains(&(41, 0x4030_0000)));
    let mut h = host(CardKind::SdV1, 4096);
    sd::identify(&mut h, 4).unwrap();
    assert!(h.into_registers().commands.contains(&(41, 0x0030_0000)));
}

/// An eMMC of 2 GB or less is byte addressed and sized by its CSD; `EXT_CSD`'s count, which
/// such a part need not fill in, is not trusted for it. A larger one is sized by `EXT_CSD`.
#[test]
fn an_emmc_is_sized_by_extended_card_specific_data_only_when_it_is_sector_addressed() {
    let mut sim = SimulatedHostController::new(CardKind::MultimediaCard, 4096, 32);
    sim.multimedia_card_byte_addressed = true;
    sim.extended_card_specific_data_sectors = Some(99_999);
    let mut h = HostController::new(sim, 50_000_000, Some(32)).unwrap().0;
    let card = sd::identify(&mut h, 8).unwrap();
    assert!(!card.block_addressed);
    assert_eq!(card.blocks, 4096);
    let mut sim = SimulatedHostController::new(CardKind::MultimediaCard, 4096, 32);
    sim.extended_card_specific_data_sectors = Some(99_999);
    let mut h = HostController::new(sim, 50_000_000, Some(32)).unwrap().0;
    let card = sd::identify(&mut h, 8).unwrap();
    assert!(card.block_addressed);
    assert_eq!(card.blocks, 99_999);
}

/// The power-up deadline is a comparison past it, not an equality with it: on a clock that
/// steps seven microseconds a read the elapsed time never equals the deadline, as a real
/// clock's need not. A worker with its own deadline turns a spin into a named failure.
#[test]
#[cfg_attr(
    miri,
    ignore = "a ten-second wall-clock watchdog; the interpreted spin to the simulated deadline outlasts it"
)]
fn an_emmc_that_never_powers_up_is_given_up_on_whatever_the_clock_steps() {
    assert_ne!(sd::POWER_UP_DEADLINE_MICROSECONDS % 7, 0);
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut sim = SimulatedHostController::new(CardKind::MultimediaCard, 4096, 32);
        sim.busy_polls = u32::MAX;
        sim.clock_step = 7;
        let mut h = HostController::new(sim, 50_000_000, Some(32)).unwrap().0;
        let busy_done = card::OPERATION_CONDITIONS_BUSY_DONE;
        let _ = sender.send(matches!(
            sd::identify(&mut h, 8),
            Err(InitializationError::PowerUp(conditions)) if conditions & busy_done == 0
        ));
    });
    assert_eq!(
        receiver.recv_timeout(std::time::Duration::from_secs(10)),
        Ok(true),
        "power-up neither finished nor gave up"
    );
}

/// The SD twin of the test above: `ACMD41` polled for a card that stays busy gives up whatever
/// the clock steps, so the loop's deadline is `>` and never `==`.
#[test]
#[cfg_attr(
    miri,
    ignore = "a ten-second wall-clock watchdog; the interpreted spin to the simulated deadline outlasts it"
)]
fn an_sd_card_that_never_powers_up_is_given_up_on_whatever_the_clock_steps() {
    assert_ne!(sd::POWER_UP_DEADLINE_MICROSECONDS % 7, 0);
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut sim = SimulatedHostController::new(CardKind::SdHighCapacity, 4096, 32);
        sim.busy_polls = u32::MAX;
        sim.clock_step = 7;
        let mut h = HostController::new(sim, 50_000_000, Some(32)).unwrap().0;
        let busy_done = card::OPERATION_CONDITIONS_BUSY_DONE;
        let _ = sender.send(matches!(
            sd::identify(&mut h, 8),
            Err(InitializationError::PowerUp(conditions)) if conditions & busy_done == 0
        ));
    });
    assert_eq!(
        receiver.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(true),
        "power-up neither finished nor gave up"
    );
}

/// Every error enum prints a sentence and is a `core::error::Error`, so a caller can box it or
/// print it with `{}`.
#[test]
fn the_error_enums_display_and_implement_the_error_trait() {
    use crate::host_controller::Error;
    use crate::internal_direct_memory_access::Refusal;
    use crate::sd::{BlockTransferError, InitializationError};
    fn text<E: core::error::Error>(e: E) -> std::string::String {
        std::format!("{e}")
    }
    assert_eq!(text(Error::Busy), "the card stayed busy past the deadline");
    assert_eq!(
        text(Refusal::Alignment(0x1001)),
        "the region at 0x1001 does not start on a page"
    );
    assert_eq!(
        text(InitializationError::GoIdle(Error::NoResponse(0))),
        "CMD0 failed: no card answered command 0"
    );
    assert_eq!(
        text(BlockTransferError::Host(Error::ClockUpdate)),
        "the transfer failed: the controller did not take a clock update"
    );
}

//! The crate's host tests, moved out of `lib.rs` on 2026-10-12 (UTC) by lane
//! maintainer/mutation-triage-2026w42, so the crate root stays an index under §282 (when a file is
//! split, the parent becomes an index) as its tests grow.

use crate::bench::{self, WriteVerdict};
use crate::card;
use crate::host::{Error, Host};
use crate::sd::{self, BLOCK, InitError, IoError, Kind};
use crate::sim::{CardKind, Sim};

fn host(kind: CardKind, blocks: usize) -> Host<Sim> {
    let (h, id) = Host::new(Sim::new(kind, blocks, 32), 50_000_000, Some(32)).unwrap();
    assert_eq!(id.verid, crate::sim::VERID);
    h
}

fn mbr_card(blocks: usize, first: u32) -> Sim {
    let mut s = Sim::new(CardKind::SdHighCapacity, blocks, 32);
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
fn an_sdhc_card_is_identified_selected_and_widened_in_the_spec_order() {
    let mut h = host(CardKind::SdHighCapacity, 8192);
    let card = sd::identify(&mut h, 4).unwrap();
    assert_eq!(card.kind, Kind::SdHighCapacity);
    assert!(card.block_addressed);
    assert_eq!(card.blocks, 8192);
    assert_eq!((card.bus_width, card.clock_hz), (4, 25_000_000));
    assert_eq!(&card.cid.name[..5], b"NIFE1");
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
    assert_eq!(card.kind, Kind::SdV1);
    assert!(!card.block_addressed);
    assert_eq!(card.address(3), 3 * 512);

    let mut h = host(CardKind::Mmc, 16_384);
    let card = sd::identify(&mut h, 8).unwrap();
    assert_eq!(card.kind, Kind::Mmc);
    assert_eq!(card.rca, sd::MMC_RCA);
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
        Err(InitError::NoCard(Error::NoResponse(1)))
    ));
}

#[test]
fn blocks_written_read_back_and_multi_block_transfers_stop_by_themselves() {
    let mut h = host(CardKind::SdHighCapacity, 8192);
    let card = sd::identify(&mut h, 4).unwrap();
    let data: Vec<u8> = (0..8 * BLOCK).map(|i| (i * 7 % 251) as u8).collect();
    sd::write_blocks(&mut h, &card, 100, &data).unwrap();
    let mut back = vec![0u8; 8 * BLOCK];
    sd::read_blocks(&mut h, &card, 100, &mut back).unwrap();
    assert_eq!(back, data);
    let mut one = [0u8; BLOCK];
    sd::read_blocks(&mut h, &card, 103, &mut one).unwrap();
    assert_eq!(one[..], data[3 * BLOCK..4 * BLOCK]);
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
    let sent = h.registers().commands.len();
    let mut buf = [0u8; BLOCK];
    assert_eq!(
        sd::read_blocks(&mut h, &card, 8192, &mut buf),
        Err(IoError::Range)
    );
    assert_eq!(
        sd::read_blocks(&mut h, &card, 0, &mut buf[..100]),
        Err(IoError::Length)
    );
    assert_eq!(
        sd::write_blocks(&mut h, &card, u64::MAX, &buf),
        Err(IoError::Range)
    );
    assert_eq!(h.registers().commands.len(), sent);
}

#[test]
fn a_controller_before_2_40a_reads_through_its_fifo_at_0x100_and_gets_no_threshold_write() {
    let sim = Sim::new(CardKind::SdHighCapacity, 8192, 32).with_verid(0x5342_230a);
    let (mut h, id) = Host::new(sim, 50_000_000, Some(32)).unwrap();
    assert_eq!(crate::regs::fifo_offset(id.verid), 0x100);
    let card = sd::identify(&mut h, 4).unwrap();
    let data: Vec<u8> = (0..2 * BLOCK).map(|i| (i % 253) as u8).collect();
    sd::write_blocks(&mut h, &card, 7, &data).unwrap();
    let mut back = vec![0u8; 2 * BLOCK];
    // A threshold write to 0x100 here would land in the FIFO and shift every word read.
    sd::read_blocks(&mut h, &card, 7, &mut back).unwrap();
    assert_eq!(back, data);
    assert!(!h.into_registers().stray_fifo_write);
}

#[test]
fn a_read_whose_tail_is_below_the_watermark_is_drained_on_transfer_over() {
    // 24 words deep: a 128-word block arrives as five fills of 24 and a tail of 8, below the
    // receive watermark of 11, so the tail raises DTO and never RXDR.
    let mut sim = Sim::new(CardKind::SdHighCapacity, 8192, 24);
    sim.strict_watermark = true;
    sim.storage[5] = core::array::from_fn(|i| (i % 249) as u8);
    let (mut h, _) = Host::new(sim, 50_000_000, Some(24)).unwrap();
    let card = sd::identify(&mut h, 4).unwrap();
    let mut one = [0u8; BLOCK];
    sd::read_blocks(&mut h, &card, 5, &mut one).unwrap();
    assert_eq!(one, h.registers().storage[5]);
}

#[test]
fn a_data_crc_error_is_reported_with_its_command() {
    let mut h = host(CardKind::SdHighCapacity, 8192);
    let card = sd::identify(&mut h, 4).unwrap();
    h.registers().fail_next_read_crc = true;
    let mut buf = [0u8; BLOCK];
    assert!(matches!(
        sd::read_blocks(&mut h, &card, 0, &mut buf),
        Err(IoError::Host(Error::Data(17, rint))) if rint & crate::regs::INT_DCRC != 0
    ));
}

#[test]
fn the_read_only_probe_reads_and_never_writes() {
    let (mut h, _) = Host::new(mbr_card(8192, 2048), 50_000_000, Some(32)).unwrap();
    let mut buf = vec![0u8; 128 * BLOCK];
    let r = bench::read_only(&mut h, 4, &mut buf, 1024).unwrap();
    assert_eq!(r.mbr.unwrap().first_start(), Some(2048));
    assert!(r.partition_boot_signature);
    assert_eq!(&r.fs_type, b"FAT32   ");
    assert_eq!(r.timed_blocks, 1024);
    let sim = h.into_registers();
    assert!(sim.written.is_empty());
    assert!(
        !sim.commands.iter().any(|c| matches!(c.0, 24 | 25)),
        "the read-only probe sent a write command"
    );
}

/// The read matrix is radon's diagnosis for its failing `CMD18`, and a bench boot that wedged on
/// its second row would waste a power cycle: every row must read the right bytes, write nothing,
/// and see the FIFO fill.
#[test]
fn the_read_matrix_reads_every_row_and_never_writes() {
    let sim = mbr_card(8192, 2048);
    let want = sim.storage[2048..2048 + 128].concat();
    let (mut h, _) = Host::new(bench::Watch::new(sim), 50_000_000, Some(32)).unwrap();
    let mut buf = vec![0u8; 128 * BLOCK];
    let mut rows = 0;
    bench::read_matrix(&mut h, 4, 2048, &mut buf, |r| {
        rows += 1;
        assert_eq!(r.result, Ok(()), "{r:?}");
        assert!(r.max_fill > 0, "{r:?}");
    })
    .unwrap();
    assert_eq!(rows, bench::MATRIX_HZ.len() * bench::MATRIX_BLOCKS.len());
    assert_eq!(buf, want);
}

#[test]
fn the_write_test_touches_only_the_gap_and_puts_back_what_was_there() {
    let mut sim = mbr_card(8192, 2048);
    // Something already in the gap: the test must restore it, not zero it.
    sim.storage[2043][0] = 0x42;
    let before = sim.storage.clone();
    let (mut h, _) = Host::new(sim, 50_000_000, Some(32)).unwrap();
    let mut buf = vec![0u8; 8 * BLOCK];
    let r = bench::read_only(&mut h, 4, &mut buf, 0).unwrap();
    let (mut a, mut b) = (vec![0u8; 8 * BLOCK], vec![0u8; 8 * BLOCK]);
    let v = bench::scratch_write(&mut h, &r.card, r.mbr.as_ref(), &mut a, &mut b);
    assert_eq!(v, WriteVerdict::Verified(2040, 8, false));
    let sim = h.into_registers();
    assert!(sim.written.iter().all(|&lba| (2040..2048).contains(&lba)));
    assert_eq!(sim.written.len(), 16);
    assert_eq!(sim.storage, before);
}

#[test]
fn a_card_with_no_gap_is_refused_and_not_written() {
    let (mut h, _) = Host::new(mbr_card(8192, 1), 50_000_000, Some(32)).unwrap();
    let mut buf = vec![0u8; 8 * BLOCK];
    let r = bench::read_only(&mut h, 4, &mut buf, 0).unwrap();
    let (mut a, mut b) = (vec![0u8; 8 * BLOCK], vec![0u8; 8 * BLOCK]);
    let v = bench::scratch_write(&mut h, &r.card, r.mbr.as_ref(), &mut a, &mut b);
    assert_eq!(v, WriteVerdict::Refused);
    assert!(h.into_registers().written.is_empty());
}

#[test]
fn a_sixteen_bit_fifo_is_refused_at_the_door() {
    // A controller whose HCON says 16-bit, over the sim.
    struct Narrow(Sim);
    impl crate::host::Registers for Narrow {
        fn read(&mut self, o: u32) -> u32 {
            if o == crate::regs::HCON {
                0
            } else {
                self.0.read(o)
            }
        }
        fn write(&mut self, o: u32, v: u32) {
            self.0.write(o, v);
        }
        fn now_us(&mut self) -> u64 {
            self.0.now_us()
        }
    }
    let narrow = Narrow(Sim::new(CardKind::SdHighCapacity, 1024, 32));
    assert!(matches!(
        Host::new(narrow, 50_000_000, None),
        Err(Error::FifoWidth(0))
    ));
}

#[test]
fn without_a_tree_depth_the_fifo_depth_is_read_from_its_reset_watermark() {
    let (h, _) = Host::new(
        Sim::new(CardKind::SdHighCapacity, 1024, 16),
        50_000_000,
        None,
    )
    .unwrap();
    assert_eq!(h.fifo_depth(), 16);
}

// **The error paths.** Each test below injects one fault the sim can produce and checks that
// the driver names the step it happened at, because a bench transcript that stops is only as
// useful as what its last line says.

use crate::sim::Fault;

fn faulty(kind: CardKind, blocks: usize, faults: &[(u8, usize, Fault)]) -> Host<Sim> {
    let mut sim = Sim::new(kind, blocks, 32);
    sim.faults = faults.to_vec();
    Host::new(sim, 50_000_000, Some(32)).unwrap().0
}

#[test]
fn an_sd_2_0_standard_capacity_card_answers_the_echo_and_is_still_byte_addressed() {
    let mut h = host(CardKind::SdV2, 4096);
    let card = sd::identify(&mut h, 4).unwrap();
    assert_eq!(card.kind, Kind::SdV2);
    assert!(!card.block_addressed);
    // It answered CMD8 but did not set CCS in its OCR, so it is not mistaken for SDHC.
    assert_eq!(card.address(2), 1024);
}

#[test]
fn a_wrong_interface_echo_is_refused_rather_than_guessed_at() {
    let mut h = faulty(CardKind::SdHighCapacity, 8192, &[(8, 0, Fault::EchoWrong)]);
    assert!(matches!(
        sd::identify(&mut h, 4),
        Err(InitError::InterfaceCondition(r)) if r & 0xfff != 0x1aa
    ));
}

#[test]
fn a_card_that_never_finishes_power_up_is_reported_with_its_last_ocr() {
    for kind in [CardKind::SdHighCapacity, CardKind::Mmc] {
        let mut sim = Sim::new(kind, 16_384, 32);
        sim.busy_polls = u32::MAX;
        let mut h = Host::new(sim, 50_000_000, Some(32)).unwrap().0;
        assert!(matches!(
            sd::identify(&mut h, 4),
            Err(InitError::PowerUp(ocr)) if ocr & card::OCR_BUSY_DONE == 0
        ));
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
        Err(InitError::NoCard(Error::NoResponse(55)))
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
            Err(InitError::Identify(index, Error::NoResponse(index))),
            "CMD{index}"
        );
    }
    let mut h = faulty(CardKind::Mmc, 16_384, &[(3, 0, Fault::NoResponse)]);
    assert_eq!(
        sd::identify(&mut h, 8),
        Err(InitError::Identify(3, Error::NoResponse(3)))
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
            Err(InitError::Configure(index, Error::NoResponse(index))),
            "CMD{index}"
        );
    }
    // An eMMC's first CMD8 is the interface probe it never answers; its second is EXT_CSD.
    let mut h = faulty(CardKind::Mmc, 16_384, &[(8, 1, Fault::NoResponse)]);
    assert_eq!(
        sd::identify(&mut h, 8),
        Err(InitError::Configure(8, Error::NoResponse(8)))
    );
}

#[test]
fn an_error_bit_in_a_status_answer_stops_identification_at_that_command() {
    let mut h = faulty(
        CardKind::SdHighCapacity,
        8192,
        &[(7, 0, Fault::ErrorStatus)],
    );
    assert!(matches!(sd::identify(&mut h, 4), Err(InitError::Status(7, r)) if r & (1 << 19) != 0));
    let mut h = faulty(CardKind::Mmc, 16_384, &[(3, 0, Fault::ErrorStatus)]);
    assert!(matches!(
        sd::identify(&mut h, 8),
        Err(InitError::Status(3, _))
    ));
    let mut h = faulty(
        CardKind::SdHighCapacity,
        8192,
        &[(16, 0, Fault::ErrorStatus)],
    );
    assert!(matches!(
        sd::identify(&mut h, 4),
        Err(InitError::Status(16, _))
    ));
}

#[test]
fn a_csd_this_driver_cannot_size_is_refused_with_its_structure() {
    let mut sim = Sim::new(CardKind::SdHighCapacity, 8192, 32);
    sim.zero_capacity = true;
    let mut h = Host::new(sim, 50_000_000, Some(32)).unwrap().0;
    assert_eq!(sd::identify(&mut h, 4), Err(InitError::Capacity(3)));
}

#[test]
fn a_status_error_on_a_transfer_is_reported_not_swallowed() {
    let mut h = faulty(
        CardKind::SdHighCapacity,
        8192,
        &[(17, 0, Fault::ErrorStatus)],
    );
    let card = sd::identify(&mut h, 4).unwrap();
    let mut one = [0u8; BLOCK];
    assert!(matches!(
        sd::read_blocks(&mut h, &card, 0, &mut one),
        Err(IoError::Status(_))
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
        Err(IoError::Status(_))
    ));
    let mut h = faulty(
        CardKind::SdHighCapacity,
        8192,
        &[(13, 0, Fault::ErrorStatus)],
    );
    let card = sd::identify(&mut h, 4).unwrap();
    assert!(matches!(
        sd::write_blocks(&mut h, &card, 9, &one),
        Err(IoError::Status(_))
    ));
    let mut h = host(CardKind::SdHighCapacity, 8192);
    let card = sd::identify(&mut h, 4).unwrap();
    one[0] = 0x77;
    sd::write_blocks(&mut h, &card, 9, &one).unwrap();
    assert_eq!(h.registers().storage[9][0], 0x77);
    assert_eq!(
        h.registers().commands.iter().rev().nth(1).map(|c| c.0),
        Some(24)
    );
}

#[test]
fn a_byte_addressed_card_stops_where_a_32_bit_byte_address_does() {
    let last = (1u64 << 32) / BLOCK as u64;
    assert!(sd::in_range(1 << 40, false, last - 1, 1));
    assert!(!sd::in_range(1 << 40, false, last, 1));
    assert!(sd::in_range(1 << 40, true, last, 1));
}

#[test]
fn the_read_only_probe_says_which_block_it_could_not_read() {
    // The second CMD17 is partition 1's boot sector; the first CMD18 is the timed read.
    let mut sim = mbr_card(8192, 2048);
    sim.faults = vec![(17, 1, Fault::ErrorStatus)];
    let mut h = Host::new(sim, 50_000_000, Some(32)).unwrap().0;
    let mut buf = vec![0u8; 8 * BLOCK];
    assert!(matches!(
        bench::read_only(&mut h, 4, &mut buf, 64),
        Err(bench::ReadFailure::Read(2048, IoError::Status(_)))
    ));
    let mut sim = mbr_card(8192, 2048);
    sim.faults = vec![(18, 1, Fault::ErrorStatus)];
    let mut h = Host::new(sim, 50_000_000, Some(32)).unwrap().0;
    assert!(matches!(
        bench::read_only(&mut h, 4, &mut buf, 64),
        Err(bench::ReadFailure::Read(2056, IoError::Status(_)))
    ));
}

/// Run the write test on a card prepared by `setup`, after a read-only probe that times
/// nothing, and return its verdict and the card.
fn write_test(setup: impl FnOnce(&mut Sim), len: usize) -> (WriteVerdict, Sim) {
    let mut sim = mbr_card(8192, 2048);
    setup(&mut sim);
    let mut h = Host::new(sim, 50_000_000, Some(32)).unwrap().0;
    let mut buf = vec![0u8; 8 * BLOCK];
    let r = bench::read_only(&mut h, 4, &mut buf, 0).unwrap();
    let (mut a, mut b) = (vec![0u8; len], vec![0u8; len]);
    let v = bench::scratch_write(&mut h, &r.card, r.mbr.as_ref(), &mut a, &mut b);
    (v, h.into_registers())
}

#[test]
fn the_write_test_refuses_buffers_too_small_for_its_range_before_touching_the_card() {
    let (v, sim) = write_test(|_| {}, 7 * BLOCK);
    assert_eq!(v, WriteVerdict::Failed(0, IoError::Length));
    assert!(sim.written.is_empty());
}

#[test]
fn the_write_test_names_the_phase_that_failed() {
    // Phase 0 reads with CMD18, 1 writes with CMD25, 2 reads back block by block with CMD17
    // (the probe already sent two), 3 writes the original back, 4 reads it back.
    for (index, nth, phase) in [
        (18u8, 0usize, 0u8),
        (25, 0, 1),
        (17, 2, 2),
        (25, 1, 3),
        (18, 1, 4),
    ] {
        let (v, _) = write_test(
            |s| s.faults = vec![(index, nth, Fault::ErrorStatus)],
            8 * BLOCK,
        );
        assert!(
            matches!(v, WriteVerdict::Failed(p, IoError::Status(_)) if p == phase),
            "CMD{index}#{nth}: {v:?}"
        );
    }
}

#[test]
fn a_card_that_drops_writes_is_a_pattern_mismatch_and_one_that_drops_the_restore_says_so() {
    let (v, sim) = write_test(|s| s.writes_stored = Some(0), 8 * BLOCK);
    assert_eq!(v, WriteVerdict::PatternMismatch(2040));
    assert!(sim.storage[2040].iter().all(|&b| b == 0));
    let (v, sim) = write_test(|s| s.writes_stored = Some(1), 8 * BLOCK);
    assert_eq!(v, WriteVerdict::RestoreMismatch(2040));
    // The pattern stayed, inside the gap, and partition 1 was never touched.
    assert!(sim.storage[2040].iter().any(|&b| b != 0));
    assert!(sim.written.iter().all(|&lba| lba < 2048));
}

/// What the write test put on the card is the pattern for each sector's own address: seen when
/// a card drops the restore and the pattern stays.
#[test]
fn the_pattern_left_behind_is_each_sectors_own() {
    let (v, sim) = write_test(|s| s.writes_stored = Some(1), 8 * BLOCK);
    assert_eq!(v, WriteVerdict::RestoreMismatch(2040));
    for lba in 2040..2048u64 {
        for (i, &b) in sim.storage[lba as usize].iter().enumerate() {
            assert_eq!(b, bench::pattern(lba, i), "sector {lba} byte {i}");
        }
    }
}

/// A mismatch is named by the sector it is in, not the first of the range: here the second.
#[test]
fn a_mismatch_names_its_own_sector() {
    let (v, _) = write_test(|s| s.mangle_write = Some((0, 2041)), 8 * BLOCK);
    assert_eq!(v, WriteVerdict::PatternMismatch(2041));
    let (v, _) = write_test(|s| s.mangle_write = Some((1, 2041)), 8 * BLOCK);
    assert_eq!(v, WriteVerdict::RestoreMismatch(2041));
}

/// Buffers longer than the range are fine (only their first eight sectors are used), either
/// one too short is refused, and the verdict says whether the gap was blank.
#[test]
fn the_write_test_takes_long_buffers_refuses_either_short_one_and_reports_a_blank_gap() {
    let (v, _) = write_test(|_| {}, 9 * BLOCK);
    assert_eq!(v, WriteVerdict::Verified(2040, 8, true));
    let (v, _) = write_test(|s| s.storage[2047][511] = 1, 8 * BLOCK);
    assert_eq!(v, WriteVerdict::Verified(2040, 8, false));
    for (a, b) in [(7, 8), (8, 7)] {
        let mut h = Host::new(mbr_card(8192, 2048), 50_000_000, Some(32))
            .unwrap()
            .0;
        let mut buf = vec![0u8; 8 * BLOCK];
        let r = bench::read_only(&mut h, 4, &mut buf, 0).unwrap();
        let (mut a, mut b) = (vec![0u8; a * BLOCK], vec![0u8; b * BLOCK]);
        let v = bench::scratch_write(&mut h, &r.card, r.mbr.as_ref(), &mut a, &mut b);
        assert_eq!(v, WriteVerdict::Failed(0, IoError::Length));
    }
}

/// The pattern, worked by hand for a few bytes: `i * 31 ^ lba * 7 ^ 0x5a ^ i >> 8`, each in
/// eight bits. The `i >> 8` term is what makes a sector's two halves differ.
#[test]
fn the_write_pattern_is_the_formula_and_its_halves_differ() {
    assert_eq!(bench::pattern(0, 0), 0x5a);
    assert_eq!(bench::pattern(0, 1), 31 ^ 0x5a);
    assert_eq!(bench::pattern(1, 0), 7 ^ 0x5a);
    assert_eq!(bench::pattern(3, 5), 155 ^ 21 ^ 0x5a);
    assert_eq!(bench::pattern(0, 256), 0x5a ^ 1);
    assert_ne!(
        (0..256).map(|i| bench::pattern(9, i)).collect::<Vec<_>>(),
        (256..512).map(|i| bench::pattern(9, i)).collect::<Vec<_>>()
    );
}

/// The timed reads stop at the card's end, in whole buffers, and a buffer under one block
/// times nothing rather than reading nothing forever.
#[test]
fn the_timed_reads_stop_at_the_end_of_the_card() {
    let (mut h, _) = Host::new(mbr_card(2048, 16), 50_000_000, Some(32)).unwrap();
    let mut buf = vec![0u8; 256 * BLOCK];
    let r = bench::read_only(&mut h, 4, &mut buf, 100_000).unwrap();
    // Partition 1 starts at 16, so blocks 16 to 2047 fit seven 256-block reads and not eight.
    assert_eq!(r.timed_blocks, 7 * 256);
    // Each read was of the next 256 blocks up from the partition, not below it.
    let reads: Vec<u32> = h
        .into_registers()
        .commands
        .iter()
        .filter(|c| c.0 == 18)
        .map(|c| c.1)
        .collect();
    assert_eq!(reads, (0..7).map(|k| 16 + 256 * k).collect::<Vec<u32>>());
    let (mut h, _) = Host::new(mbr_card(2048, 1024), 50_000_000, Some(32)).unwrap();
    let r = bench::read_only(&mut h, 4, &mut [], 8).unwrap();
    assert_eq!(r.timed_blocks, 0);
}

/// An SD card's relative address goes in the high half of every addressed command's argument,
/// the `APP_CMD` before `ACMD6` among them; its CID is decoded with the SD layout and its
/// ACMD41 argument asks for high capacity only of a card that answered CMD8.
#[test]
fn an_sd_card_is_addressed_by_the_rca_it_chose_and_decoded_as_sd() {
    let mut h = host(CardKind::SdHighCapacity, 8192);
    let card = sd::identify(&mut h, 4).unwrap();
    assert_eq!(
        card.cid,
        card::Cid {
            manufacturer: 0x1b,
            name: *b"NIFE1\0",
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
fn an_emmc_is_sized_by_ext_csd_only_when_it_is_sector_addressed() {
    let mut sim = Sim::new(CardKind::Mmc, 4096, 32);
    sim.mmc_byte_addressed = true;
    sim.ext_csd_sectors = Some(99_999);
    let mut h = Host::new(sim, 50_000_000, Some(32)).unwrap().0;
    let card = sd::identify(&mut h, 8).unwrap();
    assert!(!card.block_addressed);
    assert_eq!(card.blocks, 4096);
    let mut sim = Sim::new(CardKind::Mmc, 4096, 32);
    sim.ext_csd_sectors = Some(99_999);
    let mut h = Host::new(sim, 50_000_000, Some(32)).unwrap().0;
    let card = sd::identify(&mut h, 8).unwrap();
    assert!(card.block_addressed);
    assert_eq!(card.blocks, 99_999);
}

/// The power-up deadline is a comparison past it, not an equality with it: on a clock that
/// steps seven microseconds a read the elapsed time never equals the deadline, as a real
/// clock's need not. A worker with its own deadline turns a spin into a named failure.
#[test]
fn an_emmc_that_never_powers_up_is_given_up_on_whatever_the_clock_steps() {
    assert_ne!(sd::POWER_UP_DEADLINE_US % 7, 0);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut sim = Sim::new(CardKind::Mmc, 4096, 32);
        sim.busy_polls = u32::MAX;
        sim.clock_step = 7;
        let mut h = Host::new(sim, 50_000_000, Some(32)).unwrap().0;
        let _ = tx.send(matches!(
            sd::identify(&mut h, 8),
            Err(InitError::PowerUp(ocr)) if ocr & card::OCR_BUSY_DONE == 0
        ));
    });
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(10)),
        Ok(true),
        "power-up neither finished nor gave up"
    );
}

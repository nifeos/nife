//! Host tests of the checks the driver makes before it touches the DMA page or the device, and of
//! the logic it runs on the page's bytes: the accessors' window check, the block server's range
//! check, the descriptor chain's links, the completion wait's bound, the directory lookup, the
//! write test's pattern, the DHCP DISCOVER and its checksum, and the DHCP OFFER parse.
//!
//! Name: provisional, minted 2026-11-11 (UTC) by maintainer/virtio-block-server-robustness.

extern crate std;

use std::vec;
use std::vec::Vec;

use super::*;

/// The accessors' check admits an aligned access that ends at the last byte of the window, and
/// refuses one byte further, a misaligned one, and an offset whose end would overflow.
#[test]
fn window_access_fits_holds_the_access_inside_the_window_and_aligned() {
    let window = DIRECT_MEMORY_ACCESS_WINDOW_BYTES;
    assert!(window_access_fits(0, 8, 8, window));
    assert!(window_access_fits(window - 8, 8, 8, window));
    assert!(window_access_fits(window - 1, 1, 1, window));
    assert!(!window_access_fits(window - 7, 8, 1, window));
    assert!(!window_access_fits(window, 1, 1, window));
    assert!(!window_access_fits(
        REQUEST_HEADER_OFFSET_BYTES + 2,
        4,
        4,
        window
    ));
    assert!(!window_access_fits(
        USED_RING_OFFSET_BYTES + 1,
        2,
        2,
        window
    ));
    assert!(!window_access_fits(u64::MAX, 1, 1, window));
    assert!(!window_access_fits(u64::MAX - 3, 8, 1, window));
}

/// A transfer is admitted only when every block of it lies on the disk; a block index from a
/// client that would overflow the sector arithmetic is refused, not wrapped.
#[test]
fn transfer_first_sector_refuses_a_request_past_the_end_of_the_disk() {
    let sectors_per_block = blk::SECTORS_PER_BLOCK;
    let capacity = 100 * sectors_per_block; // a disk of 100 filesystem blocks
    assert_eq!(transfer_first_sector(0, 1, capacity), Some(0));
    assert_eq!(
        transfer_first_sector(99, 1, capacity),
        Some(99 * sectors_per_block)
    );
    assert_eq!(
        transfer_first_sector(84, 16, capacity),
        Some(84 * sectors_per_block)
    );
    assert_eq!(transfer_first_sector(100, 1, capacity), None);
    assert_eq!(transfer_first_sector(85, 16, capacity), None);
    assert_eq!(transfer_first_sector(0, 0, capacity), None);
    assert_eq!(transfer_first_sector(u64::MAX, 1, capacity), None);
    assert_eq!(
        transfer_first_sector(u64::MAX / sectors_per_block, 1, u64::MAX),
        None
    );
    assert_eq!(transfer_first_sector(0, 1, 0), None);
    // A disk whose size is not a whole number of blocks: the partial block at the end is refused.
    assert_eq!(transfer_first_sector(1, 1, 2 * sectors_per_block - 1), None);
}

/// `blk::SIZE` never answers a negative number, which a client would read as an error.
#[test]
fn disk_size_bytes_saturates_rather_than_turn_negative() {
    assert_eq!(disk_size_bytes(0), 0);
    assert_eq!(disk_size_bytes(8), 4096);
    assert_eq!(disk_size_bytes(u64::MAX / 512), i64::MAX);
    assert_eq!(disk_size_bytes(u64::MAX), i64::MAX);
}

/// A DHCP OFFER for our transaction, as an Ethernet frame: 14 bytes of Ethernet, a 20-byte IPv4
/// header, 8 bytes of UDP and the BOOTP body with its options. `options` follows the magic cookie.
fn offer_frame(options: &[u8]) -> Vec<u8> {
    let bootstrap_bytes = 240 + options.len();
    let transport_bytes = 8 + bootstrap_bytes;
    let packet_bytes = 20 + transport_bytes;
    let mut f = vec![0u8; 14 + packet_bytes];
    f[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
    let packet = 14;
    f[packet] = 0x45;
    f[packet + 2..packet + 4].copy_from_slice(&(packet_bytes as u16).to_be_bytes());
    f[packet + 9] = 17;
    let transport = packet + 20;
    f[transport..transport + 2].copy_from_slice(&67u16.to_be_bytes());
    f[transport + 2..transport + 4].copy_from_slice(&68u16.to_be_bytes());
    f[transport + 4..transport + 6].copy_from_slice(&(transport_bytes as u16).to_be_bytes());
    let reply_body = transport + 8;
    f[reply_body] = 2; // BOOTREPLY
    f[reply_body + 4..reply_body + 8].copy_from_slice(&NET_XID.to_be_bytes());
    f[reply_body + 16..reply_body + 20].copy_from_slice(&[10, 0, 2, 15]);
    f[reply_body + 236..reply_body + 240].copy_from_slice(&0x6382_5363u32.to_be_bytes());
    f[reply_body + 240..].copy_from_slice(options);
    f
}

/// Parse `bytes`, of which the device reported writing `len`.
fn parse(bytes: &[u8], len: usize) -> Option<u32> {
    parse_offer_in_frame(Frame {
        length_bytes: len as u64,
        byte: |at| bytes[at as usize],
    })
}

const OFFERED: u32 = u32::from_be_bytes([10, 0, 2, 15]);

#[test]
fn an_offer_for_our_transaction_parses() {
    let f = offer_frame(&[53, 1, 2, 255]);
    assert_eq!(parse(&f, f.len()), Some(OFFERED));
    // Padding before the message type is skipped.
    let f = offer_frame(&[0, 0, 53, 1, 2, 255]);
    assert_eq!(parse(&f, f.len()), Some(OFFERED));
}

#[test]
fn a_reply_that_is_not_an_offer_or_not_ours_is_skipped() {
    let f = offer_frame(&[53, 1, 5, 255]); // an ACK, not an OFFER
    assert_eq!(parse(&f, f.len()), None);
    let mut f = offer_frame(&[53, 1, 2, 255]);
    f[14 + 20 + 8 + 4] ^= 1; // another transaction id
    assert_eq!(parse(&f, f.len()), None);
}

/// The bytes past what the device wrote are a previous frame's, and are never read: an OFFER whose
/// message-type option lies past the reported length is not an OFFER.
#[test]
fn bytes_past_the_device_length_are_never_read() {
    let f = offer_frame(&[53, 1, 2, 255]);
    for len in 0..f.len() - 1 {
        // The closure panics on a read at or past `len`, so a read past the device length fails
        // the test rather than going unseen.
        let got = parse_offer_in_frame(Frame {
            length_bytes: len as u64,
            byte: |at| {
                assert!((at as usize) < len, "read at {at} past device length {len}");
                f[at as usize]
            },
        });
        assert_eq!(got, None, "a frame cut to {len} bytes parsed");
    }
}

/// The IPv4 total length and the UDP length bound the parse as well: an option past either is
/// trailing garbage, not part of the reply.
#[test]
fn the_packet_and_transport_lengths_bound_the_option_scan() {
    let mut f = offer_frame(&[53, 1, 2, 255]);
    let packet_bytes = u16::from_be_bytes([f[16], f[17]]);
    f[16..18].copy_from_slice(&(packet_bytes - 4).to_be_bytes());
    assert_eq!(parse(&f, f.len()), None);

    let mut f = offer_frame(&[53, 1, 2, 255]);
    let transport_bytes = u16::from_be_bytes([f[38], f[39]]);
    f[38..40].copy_from_slice(&(transport_bytes - 4).to_be_bytes());
    assert_eq!(parse(&f, f.len()), None);
}

/// An option whose length runs past the end of the frame ends the scan instead of reading on: here
/// the frame ends one byte after an option tag and its length byte, the case the old guard let
/// through.
#[test]
fn an_option_running_past_the_frame_ends_the_scan() {
    let f = offer_frame(&[53, 1]);
    assert_eq!(parse(&f, f.len()), None);
    let f = offer_frame(&[53, 4, 2]);
    assert_eq!(parse(&f, f.len()), None);
}

/// A header length that points the UDP header past the frame is refused, not read through.
#[test]
fn a_header_length_past_the_frame_is_refused() {
    let mut f = offer_frame(&[53, 1, 2, 255]);
    f[14] = 0x4f; // a header length of 15 words, 60 bytes, which runs the UDP header into the BOOTP body
    assert_eq!(parse(&f, f.len()), None);
    let f = offer_frame(&[53, 1, 2, 255]);
    assert_eq!(parse(&f[..30], 30), None);
}

/// A reset is complete when `Status` reads back as 0, and not when the write returns. A device that
/// takes a few reads to answer is waited for, one that answers at once is not read more than once,
/// and one that never answers is given up on after the bound rather than waited for forever.
#[test]
fn a_reset_waits_for_status_to_read_back_as_zero() {
    use core::cell::Cell;

    // Answers 0 only on the fourth read.
    let reads = Cell::new(0u32);
    let written = Cell::new(None);
    let slow = reset_until_zero(
        |value| written.set(Some(value)),
        || {
            reads.set(reads.get() + 1);
            if reads.get() >= 4 { 0 } else { 0x3 }
        },
    );
    assert!(slow);
    assert_eq!(written.get(), Some(0), "the reset is a write of 0");
    assert_eq!(reads.get(), 4, "it stops at the first 0 it reads");

    // Already 0: one read.
    let reads = Cell::new(0u32);
    assert!(reset_until_zero(
        |_| {},
        || {
            reads.set(reads.get() + 1);
            0
        }
    ));
    assert_eq!(reads.get(), 1);

    // Never 0: the bound, and a false answer.
    let reads = Cell::new(0u32);
    assert!(!reset_until_zero(
        |_| {},
        || {
            reads.set(reads.get() + 1);
            0x3
        }
    ));
    assert_eq!(reads.get(), RESET_POLLS);
}

/// A nifefs image of `files`, written by `nifefs` itself, so the lookup is tested against the format
/// and not against this file's idea of it.
fn image(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = vec![0u8; nifefs::image_size(files)];
    nifefs::write_image(files, &mut out).expect("the test image fits");
    out
}

/// The start block `nifefs` itself records for `name` in `image`.
fn start_block_of(image: &[u8], name: &str) -> u32 {
    nifefs::Fs::parse(image)
        .expect("the test image parses")
        .entries()
        .find(|entry| entry.name_eq(name))
        .expect("the file is in the image")
        .start_block
}

/// The lookup finds every file in the first block at the start block `nifefs` recorded, and a
/// name that is only a prefix of an entry, or that an entry is a prefix of, finds nothing.
#[test]
fn find_file_returns_the_start_block_nifefs_recorded() {
    let longest = "a-name-of-exactly-thirty-two-byt";
    assert_eq!(longest.len(), nifefs::NAME_LEN);
    let disk = image(&[
        ("motd", b"hello"),
        ("scratch", &[0u8; 512]),
        (longest, b"x"),
    ]);
    let directory = &disk[..BLOCK];
    for name in ["motd", "scratch", longest] {
        assert_eq!(
            find_file(directory, name.as_bytes()),
            Some(start_block_of(&disk, name)),
            "{name}"
        );
    }
    assert_eq!(find_file(directory, b"mot"), None);
    assert_eq!(find_file(directory, b"motdx"), None);
    assert_eq!(find_file(directory, b"absent"), None);
}

/// A name longer than the field names no entry, even when its first `NAME_LEN` bytes are an
/// entry's whole name: the lookup answers `None` rather than reading past the field.
#[test]
fn find_file_refuses_a_name_longer_than_the_field() {
    let longest = "a-name-of-exactly-thirty-two-byt";
    let disk = image(&[(longest, b"x")]);
    let longer = [longest.as_bytes(), b"!"].concat();
    assert_eq!(find_file(&disk[..BLOCK], &longer), None);
}

/// A directory that claims more entries than the block holds, or a buffer shorter than the
/// header, answers `None` rather than reading past the slice.
#[test]
fn find_file_reads_nothing_past_its_slice() {
    let mut disk = image(&[("motd", b"hello")]);
    disk[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    let block = &disk[..BLOCK];
    assert_eq!(
        find_file(block, b"motd"),
        Some(start_block_of(&image(&[("motd", b"hello")]), "motd"))
    );
    assert_eq!(find_file(block, b"absent"), None);
    // Cut inside the first entry's start block: the entry is not visible.
    let cut = nifefs::HEADER_LEN + nifefs::NAME_LEN + 2;
    assert_eq!(find_file(&block[..cut], b"motd"), None);
    assert_eq!(find_file(&block[..4], b"motd"), None);
}

/// The write test's pattern: the literal the kernel test asserts, then a position-dependent byte, so
/// a block that came back shifted by one byte matches nowhere past the head.
#[test]
fn the_write_pattern_opens_with_the_literal_and_moves_with_position() {
    let pattern: Vec<u8> = (0..BLOCK).map(pattern_byte).collect();
    assert_eq!(&pattern[..8], b"CRKWRIT1");
    assert_eq!(pattern[8], 8u8.wrapping_mul(31) ^ 0x5A);
    assert!((8..BLOCK - 1).all(|i| pattern[i] != pattern[i + 1]));
}

/// The checksum of RFC 1071's own example words, and of a header with its checksum written in,
/// which sums to zero; an odd trailing byte is summed as the high byte of a padded word.
#[test]
fn ip_checksum_matches_rfc_1071() {
    // RFC 1071 section 3: the words 0001 f203 f4f5 f6f7 sum to ddf2 with the carries folded.
    assert_eq!(
        ip_checksum(&[0x00, 0x01, 0xf2, 0x03, 0xf4, 0xf5, 0xf6, 0xf7]),
        !0xddf2
    );
    // A worked IPv4 header, UDP from 192.168.0.1 to 192.168.0.199: checksum b861.
    let mut header = [
        0x45, 0x00, 0x00, 0x73, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0x00, 0x00, 0xc0, 0xa8, 0x00,
        0x01, 0xc0, 0xa8, 0x00, 0xc7,
    ];
    assert_eq!(ip_checksum(&header), 0xb861);
    header[10..12].copy_from_slice(&0xb861u16.to_be_bytes());
    assert_eq!(ip_checksum(&header), 0);
    assert_eq!(ip_checksum(&[0x12]), !0x1200);
    assert_eq!(ip_checksum(&[]), 0xffff);
}

/// The DISCOVER is a broadcast BOOTREQUEST for our transaction from our MAC, with a valid IPv4
/// checksum and lengths that agree with the frame, and asks for DISCOVER in option 53.
#[test]
fn the_dhcp_discover_is_a_well_formed_broadcast_request() {
    let f = dhcp_discover();
    assert_eq!(f.len(), 291);
    assert_eq!(&f[0..6], &[0xff; 6]);
    assert_eq!(&f[6..12], &NET_MAC);
    assert_eq!(&f[12..14], &[0x08, 0x00]);
    let ip = &f[14..34];
    assert_eq!(ip[0], 0x45);
    assert_eq!(
        usize::from(u16::from_be_bytes([ip[2], ip[3]])),
        f.len() - 14
    );
    assert_eq!((ip[8], ip[9]), (64, 17));
    assert_eq!(&ip[12..16], &[0; 4]);
    assert_eq!(&ip[16..20], &[0xff; 4]);
    assert_eq!(ip_checksum(ip), 0, "the header checksum is valid");
    let udp = &f[34..42];
    assert_eq!(u16::from_be_bytes([udp[0], udp[1]]), 68);
    assert_eq!(u16::from_be_bytes([udp[2], udp[3]]), 67);
    assert_eq!(
        usize::from(u16::from_be_bytes([udp[4], udp[5]])),
        f.len() - 34
    );
    assert_eq!(&udp[6..8], &[0, 0]);
    let dhcp = &f[42..];
    assert_eq!(&dhcp[0..4], &[1, 1, 6, 0]);
    assert_eq!(&dhcp[4..8], &NET_XID.to_be_bytes());
    assert_eq!(&dhcp[8..12], &[0, 0, 0x80, 0]);
    assert!(dhcp[12..28].iter().all(|&b| b == 0));
    assert_eq!(&dhcp[28..34], &NET_MAC);
    assert!(dhcp[34..236].iter().all(|&b| b == 0));
    assert_eq!(&dhcp[236..240], &[0x63, 0x82, 0x53, 0x63]);
    assert_eq!(&dhcp[240..], &[53, 1, 1, 55, 3, 1, 3, 6, 255]);
}

/// Every descriptor of a chain but the last carries NEXT and links to the one after it; the last
/// keeps only its own flags and links nowhere, and a one-descriptor chain is its own last.
#[test]
fn a_chain_links_each_descriptor_to_the_next_and_ends_at_the_last() {
    assert_eq!(chained(0, 3, 0), (VIRTQ_DESC_F_NEXT, 1));
    assert_eq!(
        chained(1, 3, VIRTQ_DESC_F_WRITE),
        (VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2)
    );
    assert_eq!(chained(2, 3, VIRTQ_DESC_F_WRITE), (VIRTQ_DESC_F_WRITE, 0));
    assert_eq!(
        chained(0, 1, VIRTQ_DESC_F_INDIRECT),
        (VIRTQ_DESC_F_INDIRECT, 0)
    );
    assert_eq!(chained(0, 2, 0), (VIRTQ_DESC_F_NEXT, 1));
    assert_eq!(chained(1, 2, VIRTQ_DESC_F_WRITE), (VIRTQ_DESC_F_WRITE, 0));
}

/// The completion wait skips wakeups that leave the used index where it was, returns on the first
/// that moves it, and with a bound gives up after exactly that many wakeups.
#[test]
fn the_completion_wait_counts_wakeups_and_stops_at_the_bound() {
    // Moves on the third wakeup: found within a bound of three, and the wait stops there.
    let mut wakeups = 0;
    let moved = used_ring_moved(Some(COMPLETION_WAKEUPS), 7, || {
        wakeups += 1;
        if wakeups >= 3 { 8 } else { 7 }
    });
    assert!(moved);
    assert_eq!(wakeups, 3);

    // Never moves: gives up after exactly the bound.
    let mut wakeups = 0;
    let moved = used_ring_moved(Some(COMPLETION_WAKEUPS), 7, || {
        wakeups += 1;
        7
    });
    assert!(!moved);
    assert_eq!(wakeups, COMPLETION_WAKEUPS);

    // Unbounded: waits past any bound, and a wrap from u16::MAX to 0 is a move.
    let mut wakeups = 0u32;
    let moved = used_ring_moved(None, u16::MAX, || {
        wakeups += 1;
        if wakeups > 1000 { 0 } else { u16::MAX }
    });
    assert!(moved);
    assert_eq!(wakeups, 1001);
}

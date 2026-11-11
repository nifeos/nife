extern crate std;

use std::vec;
use std::vec::Vec;

use super::*;

/// The accessors' check admits an aligned access that ends at the last byte of the window, and
/// refuses one byte further, a misaligned one, and an offset whose end would overflow.
#[test]
fn window_access_fits_holds_the_access_inside_the_window_and_aligned() {
    let window = DMA_WINDOW_BYTES;
    assert!(window_access_fits(0, 8, 8, window));
    assert!(window_access_fits(window - 8, 8, 8, window));
    assert!(window_access_fits(window - 1, 1, 1, window));
    assert!(!window_access_fits(window - 7, 8, 1, window));
    assert!(!window_access_fits(window, 1, 1, window));
    assert!(!window_access_fits(OFF_HEADER + 2, 4, 4, window));
    assert!(!window_access_fits(OFF_USED + 1, 2, 2, window));
    assert!(!window_access_fits(u64::MAX, 1, 1, window));
    assert!(!window_access_fits(u64::MAX - 3, 8, 1, window));
}

/// A transfer is admitted only when every block of it lies on the disk; a block index from a
/// client that would overflow the sector arithmetic is refused, not wrapped.
#[test]
fn transfer_first_sector_refuses_a_request_past_the_end_of_the_disk() {
    let spb = blk::SECTORS_PER_BLOCK;
    let capacity = 100 * spb; // a disk of 100 filesystem blocks
    assert_eq!(transfer_first_sector(0, 1, capacity), Some(0));
    assert_eq!(transfer_first_sector(99, 1, capacity), Some(99 * spb));
    assert_eq!(transfer_first_sector(84, 16, capacity), Some(84 * spb));
    assert_eq!(transfer_first_sector(100, 1, capacity), None);
    assert_eq!(transfer_first_sector(85, 16, capacity), None);
    assert_eq!(transfer_first_sector(0, 0, capacity), None);
    assert_eq!(transfer_first_sector(u64::MAX, 1, capacity), None);
    assert_eq!(transfer_first_sector(u64::MAX / spb, 1, u64::MAX), None);
    assert_eq!(transfer_first_sector(0, 1, 0), None);
    // A disk whose size is not a whole number of blocks: the partial block at the end is refused.
    assert_eq!(transfer_first_sector(1, 1, 2 * spb - 1), None);
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
    let dhcp_len = 240 + options.len();
    let udp_len = 8 + dhcp_len;
    let ip_total = 20 + udp_len;
    let mut f = vec![0u8; 14 + ip_total];
    f[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
    let ip = 14;
    f[ip] = 0x45;
    f[ip + 2..ip + 4].copy_from_slice(&(ip_total as u16).to_be_bytes());
    f[ip + 9] = 17;
    let udp = ip + 20;
    f[udp..udp + 2].copy_from_slice(&67u16.to_be_bytes());
    f[udp + 2..udp + 4].copy_from_slice(&68u16.to_be_bytes());
    f[udp + 4..udp + 6].copy_from_slice(&(udp_len as u16).to_be_bytes());
    let dhcp = udp + 8;
    f[dhcp] = 2; // BOOTREPLY
    f[dhcp + 4..dhcp + 8].copy_from_slice(&NET_XID.to_be_bytes());
    f[dhcp + 16..dhcp + 20].copy_from_slice(&[10, 0, 2, 15]);
    f[dhcp + 236..dhcp + 240].copy_from_slice(&0x6382_5363u32.to_be_bytes());
    f[dhcp + 240..].copy_from_slice(options);
    f
}

/// Parse `bytes`, of which the device reported writing `len`.
fn parse(bytes: &[u8], len: usize) -> Option<u32> {
    parse_dhcp_offer_frame(Frame {
        len: len as u64,
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
        let got = parse_dhcp_offer_frame(Frame {
            len: len as u64,
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
fn the_ip_and_udp_lengths_bound_the_option_scan() {
    let mut f = offer_frame(&[53, 1, 2, 255]);
    let ip_total = u16::from_be_bytes([f[16], f[17]]);
    f[16..18].copy_from_slice(&(ip_total - 4).to_be_bytes());
    assert_eq!(parse(&f, f.len()), None);

    let mut f = offer_frame(&[53, 1, 2, 255]);
    let udp_len = u16::from_be_bytes([f[38], f[39]]);
    f[38..40].copy_from_slice(&(udp_len - 4).to_be_bytes());
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

/// An IHL that points the UDP header past the frame is refused, not read through.
#[test]
fn a_header_length_past_the_frame_is_refused() {
    let mut f = offer_frame(&[53, 1, 2, 255]);
    f[14] = 0x4f; // IHL 15: a 60-byte header, which runs the UDP header into the BOOTP body
    assert_eq!(parse(&f, f.len()), None);
    let f = offer_frame(&[53, 1, 2, 255]);
    assert_eq!(parse(&f[..30], 30), None);
}

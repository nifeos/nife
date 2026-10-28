//! **The rings and their descriptors**: how big each ring and buffer is, the descriptor bits the
//! driver writes and reads, and what a receive descriptor says. The descriptor bits keep
//! OpenBSD's `dwqereg.h` names, and each one's documentation opens with its full name.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

pub(crate) const PAGE: u64 = 4096;

/// Receive descriptors. Sixteen 16-byte descriptors are 256 bytes, one page with room to grow.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `rx` and `tx` spelled out, replacing `RX_ENTRIES`.
pub const RECEIVE_ENTRIES: u16 = 16;

/// Transmit descriptors.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `rx` and `tx` spelled out, replacing `TX_ENTRIES`.
pub const TRANSMIT_ENTRIES: u16 = 16;

/// Bytes per descriptor: four little-endian words.
pub const DESCRIPTOR_BYTES: u64 = 16;

/// Bytes per buffer. Written into the receive control register's buffer size field, so the
/// device never writes past it, and large enough that a 1518-byte frame never spans two.
pub const BUFFER_SIZE: u64 = 2048;

/// The largest frame either direction carries: 1500 bytes of payload and the 14-byte header, CRC
/// excluded (the device appends it on transmit, and the driver strips it on receive).
///
/// Name: provisional. Ruled 2026-10-26 (UTC): renamed by the crate's rename table, replacing
/// `MAX_FRAME`.
pub const MAXIMUM_FRAME: usize = 1514;

/// The frame check sequence the device leaves on every received frame.
pub const CRC_BYTES: usize = 4;

// Receive and transmit, read and write-back formats: descriptor word 3 is the one that carries
// ownership, and the only one the data plane judges.

/// **Receive descriptor word 3, owned by the device** (`RDES3_OWN`, `dwqereg.h`). Set by the
/// driver to hand a descriptor over, cleared by the device's write-back.
#[doc(alias = "owned by the device")]
pub const RDES3_OWN: u32 = 1 << 31;

/// **Receive descriptor word 3, interrupt on completion** (`RDES3_IC`, `dwqereg.h`). Receive, read
/// format: interrupt on completion. Interrupts are masked at the channel; set anyway so the channel
/// status's receive bit reports completions, which the coherence probe reads.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): Decision 37c, the source's name copied strictly,
/// replacing `RDES3_IOC`.
#[doc(alias = "receive descriptor word 3, interrupt on completion")]
pub const RDES3_IC: u32 = 1 << 30;

/// **Receive descriptor word 3, buffer 1 address valid** (`RDES3_BUF1V`, `dwqereg.h`). Receive,
/// read format: buffer 1's address is valid.
#[doc(alias = "receive descriptor word 3, buffer 1 address valid")]
pub const RDES3_BUF1V: u32 = 1 << 24;

/// **Receive descriptor word 3, context descriptor** (`RDES3_CTXT`, `dwqereg.h`). Receive,
/// write-back: this is a context descriptor (a timestamp), not a frame.
#[doc(alias = "receive descriptor word 3, context descriptor")]
pub const RDES3_CTXT: u32 = 1 << 30;

/// **Receive descriptor word 3, first descriptor** (`RDES3_FD`, `dwqereg.h`). Receive, write-back:
/// first descriptor of a frame.
#[doc(alias = "receive descriptor word 3, first descriptor")]
pub const RDES3_FD: u32 = 1 << 29;

/// **Receive descriptor word 3, last descriptor** (`RDES3_LD`, `dwqereg.h`). Receive, write-back:
/// last descriptor of a frame.
#[doc(alias = "receive descriptor word 3, last descriptor")]
pub const RDES3_LD: u32 = 1 << 28;

/// **Receive descriptor word 3, error summary** (`RDES3_ES`, `dwqereg.h`). Receive, write-back:
/// error summary.
#[doc(alias = "receive descriptor word 3, error summary")]
pub const RDES3_ES: u32 = 1 << 15;

/// **Receive descriptor word 3, packet length** (`RDES3_LENGTH`, `dwqereg.h`). Receive, write-back:
/// the frame length including the CRC, valid when [`RDES3_LD`] is set.
#[doc(alias = "receive descriptor word 3, packet length")]
pub const RDES3_LENGTH: u32 = 0x7fff;

/// **Transmit descriptor word 2, interrupt on completion** (`TDES2_IC`, `dwqereg.h`). Transmit,
/// read format: interrupt on completion (in word 2).
#[doc(alias = "transmit descriptor word 2, interrupt on completion")]
pub const TDES2_IC: u32 = 1 << 31;

/// **Transmit descriptor word 2, buffer 1 length** (`TDES2_BUF1_LEN`, `dwqereg.h`). Transmit, read
/// format: buffer 1's length (in word 2).
#[doc(alias = "transmit descriptor word 2, buffer 1 length")]
pub const TDES2_BUF1_LEN: u32 = 0x3fff;

/// **Transmit descriptor word 3, first segment** (`TDES3_FS`, `dwqereg.h`). Transmit: first
/// descriptor of a frame.
#[doc(alias = "transmit descriptor word 3, first segment")]
pub const TDES3_FS: u32 = 1 << 29;

/// **Transmit descriptor word 3, last segment** (`TDES3_LS`, `dwqereg.h`). Transmit: last
/// descriptor of a frame.
#[doc(alias = "transmit descriptor word 3, last segment")]
pub const TDES3_LS: u32 = 1 << 28;

/// **Transmit descriptor word 3, owned by the device** (`TDES3_OWN`, `dwqereg.h`). Transmit: the
/// device owns it ([`RDES3_OWN`]'s bit, in the transmit ring).
#[doc(alias = "transmit descriptor word 3, owned by the device")]
pub const TDES3_OWN: u32 = 1 << 31;

/// **Transmit descriptor word 3, frame length** (`TDES3_FRAME_LEN`, `dwqereg.h`). Transmit, read
/// format: the whole frame's length.
#[doc(alias = "transmit descriptor word 3, frame length")]
pub const TDES3_FRAME_LEN: u32 = 0x7fff;

/// **Transmit descriptor word 3, error summary** (`TDES3_ES`, `dwqereg.h`). Transmit, write-back:
/// error summary.
#[doc(alias = "transmit descriptor word 3, error summary")]
pub const TDES3_ES: u32 = 1 << 15;

/// **A receive descriptor handed to the device**: the buffer's address in words 0 and 1 (low and
/// high 32 bits, as OpenBSD and Linux's `dwmac4_set_addr` both write it), word 2 clear, and word 3
/// owned, interrupt-on-completion, buffer 1 valid. As two little-endian `u64`s: words 0 and 1,
/// then words 2 and 3.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `rx` and `tx` spelled out, replacing
/// `rx_descriptor_words`.
pub const fn receive_descriptor_words(buffer_physical_address: u64) -> [u64; 2] {
    [
        buffer_physical_address,
        ((RDES3_OWN | RDES3_IC | RDES3_BUF1V) as u64) << 32,
    ]
}

/// The three bits the receive word sets are distinct single bits, so the `|`s are their `^`s,
/// which the compiler checks here; the mutation exclusion for `receive_descriptor_words` names
/// this constant. The compiler is the gate's only reader, and it reads at const evaluation, so
/// `dead_code` is allowed here for exactly that reason.
///
/// Name: provisional, minted 2026-10-28 (UTC) by lane/954-ethernet-assertion-names.
#[allow(dead_code)]
pub const THE_RECEIVE_DESCRIPTOR_SET_BITS_SHARE_NO_BIT: () =
    assert!(RDES3_OWN | RDES3_IC | RDES3_BUF1V == RDES3_OWN ^ RDES3_IC ^ RDES3_BUF1V);

/// **A transmit descriptor for one whole frame in one buffer**: the address, the length with
/// interrupt-on-completion, and word 3 owned, first and last, with the frame length. The device
/// inserts the CRC and pads a short frame (checksum and padding control zero).
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `rx` and `tx` spelled out, replacing
/// `tx_descriptor_words`.
pub const fn transmit_descriptor_words(buffer_physical_address: u64, length: u16) -> [u64; 2] {
    let length = length as u32 & TDES3_FRAME_LEN;
    let des2 = (length & TDES2_BUF1_LEN) | TDES2_IC;
    let descriptor_word_3 = TDES3_OWN | TDES3_FS | TDES3_LS | length;
    [
        buffer_physical_address,
        des2 as u64 | (descriptor_word_3 as u64) << 32,
    ]
}

/// Word 2's length field and interrupt bit share no bit; word 3's three set bits are distinct
/// single bits and none of them reaches the length field; and word 2 sits a whole word below the
/// shifted word 3, for every value either can hold. The compiler checks it here, and the
/// mutation exclusion for `transmit_descriptor_words` names this constant. The compiler is the
/// gate's only reader, and it reads at const evaluation, so `dead_code` is allowed here for
/// exactly that reason.
///
/// Name: provisional, minted 2026-10-28 (UTC) by lane/954-ethernet-assertion-names.
#[allow(dead_code)]
pub const THE_TRANSMIT_DESCRIPTOR_BITS_SHARE_NO_BIT: () = assert!(
    TDES2_IC & TDES2_BUF1_LEN == 0
        && (TDES3_OWN | TDES3_FS | TDES3_LS == TDES3_OWN ^ TDES3_FS ^ TDES3_LS)
        && TDES3_OWN & TDES3_FRAME_LEN == 0
        && TDES3_FS & TDES3_FRAME_LEN == 0
        && TDES3_LS & TDES3_FRAME_LEN == 0
        && ((u32::MAX as u64) << 32) & (u32::MAX as u64) == 0
);

/// What a receive descriptor's word 3 says, judged.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `rx` and `tx` spelled out, replacing `RxVerdict`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiveVerdict {
    /// The device still owns it.
    NotYet,
    /// A whole, good frame of this many bytes (CRC removed), which fits the buffer and the room.
    Frame(usize),
    /// The device wrote it and it is not a frame to deliver: an error, a context descriptor, a
    /// frame spanning descriptors, or a length it could not have meant. Consumed and reposted.
    Drop,
}

/// **Judge a receive descriptor's word 3**, where `room` is how many bytes the caller can take.
///
/// Every field is the device's statement and none is trusted, since a device that lies about a
/// length would lead a trusting driver past its buffer: a `Frame(n)` always has `n <= room`,
/// `n <= MAXIMUM_FRAME` and `n + CRC_BYTES <= BUFFER_SIZE`, which the Kani harness below proves for
/// every word.
///
/// # Examples
///
/// ```
/// use designware_ethernet::{ReceiveVerdict, receive_verdict, RDES3_FD, RDES3_LD, RDES3_OWN};
///
/// // 64 bytes on the wire is 60 of frame and 4 of CRC, which the driver strips.
/// assert_eq!(receive_verdict(RDES3_FD | RDES3_LD | 64, 1514), ReceiveVerdict::Frame(60));
/// assert_eq!(receive_verdict(RDES3_OWN | RDES3_FD | RDES3_LD | 64, 1514), ReceiveVerdict::NotYet);
/// assert_eq!(receive_verdict(RDES3_FD | RDES3_LD | 0x7fff, 1514), ReceiveVerdict::Drop);
/// ```
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `rx` and `tx` spelled out, replacing `rx_verdict`.
pub fn receive_verdict(descriptor_word_3: u32, room: usize) -> ReceiveVerdict {
    if descriptor_word_3 & RDES3_OWN != 0 {
        return ReceiveVerdict::NotYet;
    }
    let whole = descriptor_word_3 & (RDES3_FD | RDES3_LD) == RDES3_FD | RDES3_LD;
    if descriptor_word_3 & RDES3_CTXT != 0 || !whole || descriptor_word_3 & RDES3_ES != 0 {
        return ReceiveVerdict::Drop;
    }
    let wire = (descriptor_word_3 & RDES3_LENGTH) as usize;
    let Some(length) = wire.checked_sub(CRC_BYTES) else {
        return ReceiveVerdict::Drop;
    };
    let fits =
        (14..=MAXIMUM_FRAME).contains(&length) && length <= room && wire as u64 <= BUFFER_SIZE;
    if !fits {
        return ReceiveVerdict::Drop;
    }
    ReceiveVerdict::Frame(length)
}

/// First and last are distinct single bits, so the mask and the compared value read the same
/// under `|` and under `^`, which the compiler checks here; the mutation exclusion for
/// `receive_verdict` names this constant. The compiler is the gate's only reader, and it reads at
/// const evaluation, so `dead_code` is allowed here for exactly that reason.
///
/// Name: provisional, minted 2026-10-28 (UTC) by lane/954-ethernet-assertion-names.
#[allow(dead_code)]
pub const THE_FIRST_AND_LAST_BITS_SHARE_NO_BIT: () =
    assert!(RDES3_FD | RDES3_LD == RDES3_FD ^ RDES3_LD);

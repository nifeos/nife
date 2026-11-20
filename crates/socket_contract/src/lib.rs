#![cfg_attr(not(test), no_std)]
//! The socket contract wire format, shared by a network stack and its clients. Each socket is its
//! own capability: a process holds a front door that can
//! only make sockets, and one capability per open socket. Opening and `LISTEN` are `CALL`s on the
//! front door answered with the socket's capability; every other operation is a `CALL` on the
//! socket, with an opcode, one argument and one reply word. The stack names a socket only by its
//! kernel-stamped [`SOCKET_BADGE_BIT`], so a closed socket's copies reach nothing. A port claim is
//! granted at spawn ([`listen_grant`], [`udp_bind_grant`]), never assumed.
//!
//! # Vocabulary
//!
//! The contract assumes a small capability kernel. A *capability* is an unforgeable handle to one
//! object, and holding it is the right to use it. A *`CALL`* is the one synchronous request on a
//! capability: an opcode, one argument word, and a reply of one or two words (and sometimes a new
//! capability). A *badge* is a number the kernel stamps on every request that arrived through a
//! capability, so the receiver learns which object was meant without trusting the caller. The
//! *shared frame* is one page of memory a socket's holder and the stack both map, carrying the
//! destination and the payload (see [`PAYLOAD_OFFSET_BYTES`]); `SEND_CAPABILITY` delegates it. The kernel's
//! interface crate defines the operations and rights themselves.

//! # BUGS
//!
//! - A grant is the stack's, shared by every holder of its front door; a per-client grant would
//!   ride on a badged front door, which nothing builds yet.
//! - A listener's backlog is one connection: a second arriving before `ACCEPT` is refused by the
//!   Transmission Control Protocol (TCP).
//! - No operation carries [`VERSION`]: a client and a stack built from different versions of this
//!   crate are not told apart when the client binds.
//!
//! Name: ratified 2026-08-01 (calef, the naming tenet); `_contract` per calef's 2026-10-18 ruling.

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

// Operations. The opcode is the whole request word.

/// The protocol's version, one for the operation set and the reply shapes together: one constant
/// per wire format. A break bumps this; an addition (a new opcode, an optional field) does not. No
/// operation carries it yet, so a client and a stack built from different versions are not told
/// apart at bind (see BUGS).
///
/// Name: ratified 2026-11-20 (UTC).
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 19, naming the socket protocol and
/// its shared frame. The number is this interface's for good: no other interface takes it, and a
/// format change is a new version, never a new number. The project that links these crates keeps the table of numbers and holds it to that rule. A
/// program declares the pair of this id and [`VERSION`].
///
/// Name: ratified 2026-11-20 (UTC).
pub const INTERFACE_ID: u64 = 19;

/// `SEND_CAPABILITY` on a socket's capability: delegate that socket's shared frame. Not a `CALL`:
/// it has no argument word, and a `CALL` with this opcode is answered [`REPLY_ERROR`].
///
/// Name: ratified 2026-11-19 (UTC), the same spelling as the resolver contract's operation for the
/// same act; the earlier page-frame spelling is refused, since `frame` also means a network frame
/// that a stack takes off a network card.
pub const OPERATION_ATTACH_SHARED_PAGE: u64 = 1;
/// `CALL` on the front door: create a User Datagram Protocol (UDP) socket and bind an ephemeral local port. Answered
/// [`REPLY_OK`] with the socket's capability.
pub const OPERATION_OPEN_UDP: u64 = 2;
/// `CALL` on the front door: create a TCP socket. Answered [`REPLY_OK`] with the socket's capability.
pub const OPERATION_OPEN_TCP: u64 = 3;
/// `CALL(udp socket, OPERATION_SEND_TO, length)`: UDP send. The argument word is the payload length
/// in bytes, at most [`MAXIMUM_PAYLOAD_BYTES`]; a larger one is answered [`REPLY_ERROR`]. The destination is in
/// the frame header and the payload at [`PAYLOAD_OFFSET_BYTES`]. The frame's [`PAYLOAD_LENGTH_OFFSET_BYTES`] is not read. Replies
/// [`REPLY_OK`], or [`REPLY_ERROR`].
///
/// Name: ratified 2026-11-20 (UTC).
#[doc(alias = "sendto")]
pub const OPERATION_SEND_TO: u64 = 4;
/// `CALL(socket, OPERATION_RECEIVE, 0)`: block until a datagram or segment arrives, and write it to
/// the frame. The argument word is ignored: pass 0. Replies the payload length in bytes, which is
/// also written at [`PAYLOAD_LENGTH_OFFSET_BYTES`]; 0 means the stream has closed, and [`REPLY_ERROR`] a failure. A UDP
/// reply also writes the sender's address and port at [`DESTINATION_IPV4_ADDRESS_OFFSET_BYTES`] and [`DESTINATION_PORT_OFFSET_BYTES`].
pub const OPERATION_RECEIVE: u64 = 5;
/// `CALL(tcp socket, OPERATION_CONNECT, 0)`: connect to the destination in the frame header. The
/// argument word is ignored: pass 0. Replies [`CONNECT_ESTABLISHED`] or [`CONNECT_REFUSED`], or
/// [`REPLY_ERROR`].
pub const OPERATION_CONNECT: u64 = 6;
/// `CALL(tcp socket, OPERATION_SEND, length)`: TCP send. The argument word is the payload length in
/// bytes, at most [`MAXIMUM_PAYLOAD_BYTES`]; a larger one is answered [`REPLY_ERROR`]. The payload is at
/// [`PAYLOAD_OFFSET_BYTES`]. The frame's [`PAYLOAD_LENGTH_OFFSET_BYTES`] is not read. Replies the count of bytes the stack
/// accepted, which may be fewer than `length` when its buffer is short: send the rest.
///
/// ```
/// use socket_contract::{MAXIMUM_PAYLOAD_BYTES, PAYLOAD_LENGTH_OFFSET_BYTES, PAYLOAD_OFFSET_BYTES, OPERATION_SEND};
///
/// // A client's side of a send: the payload goes in the frame, and the length goes in the
/// // argument word of the `CALL`. It never writes `PAYLOAD_LENGTH_OFFSET_BYTES`.
/// let mut frame = [0u8; 4096];
/// let payload = b"hello out!";
/// let at = PAYLOAD_OFFSET_BYTES as usize;
/// frame[at..at + payload.len()].copy_from_slice(payload);
/// let (operation, argument) = (OPERATION_SEND, payload.len() as u64);
///
/// // The stack's side: the argument is the whole length, checked against `MAXIMUM_PAYLOAD_BYTES`.
/// assert_eq!(operation, 7);
/// assert!(argument <= MAXIMUM_PAYLOAD_BYTES as u64);
/// let sent = &frame[at..at + argument as usize];
/// assert_eq!(sent, payload);
/// // Nothing was written at `PAYLOAD_LENGTH_OFFSET_BYTES`.
/// assert_eq!(frame[PAYLOAD_LENGTH_OFFSET_BYTES as usize..PAYLOAD_LENGTH_OFFSET_BYTES as usize + 2], [0, 0]);
/// ```
pub const OPERATION_SEND: u64 = 7;
/// `CALL(socket, OPERATION_CLOSE, 0)`: close the socket and drop its frame mapping. The argument
/// word is ignored: pass 0. Replies [`REPLY_OK`].
pub const OPERATION_CLOSE: u64 = 8;
/// `CALL(front door, OPERATION_LISTEN, port)`: start listening on `port`. Replies one of the
/// [`PORT_CLAIM_GRANTED`] outcomes, and a granted one carries the listener's capability.
pub const OPERATION_LISTEN: u64 = 9;
/// `CALL(listener, OPERATION_ACCEPT, 0)`: block until a connection arrives on this listener, then
/// answer [`REPLY_OK`] carrying the connection's capability, or [`REPLY_ERROR`]. The listener keeps
/// listening, and the connection wants a frame attached before it carries bytes. The argument word
/// is ignored: pass 0. The reply's second word is the peer that connected, packed by [`peer_word`]
/// and read back by [`word_peer`]: the Internet Protocol version 4 (IPv4) address in bits 47 to 16
/// and the port in bits 15 to 0. The word is the IPv4 answer only, and a peer of another family
/// would need its own route.
///
/// ```
/// use socket_contract::{peer_word, word_peer};
///
/// // The second reply word of an ACCEPT from 10.0.2.2, port 50161.
/// assert_eq!(peer_word([10, 0, 2, 2], 50161), 0x0a00_0202_c3f1);
/// assert_eq!(word_peer(0x0a00_0202_c3f1), Some(([10, 0, 2, 2], 50161)));
/// ```
pub const OPERATION_ACCEPT: u64 = 10;
/// `CALL(front door, OPERATION_BIND_UDP, port)`: create a UDP socket bound to the fixed
/// `port`, subject to the stack's [`udp_bind_grant`]. Replies the [`PORT_CLAIM_GRANTED`] vocabulary,
/// which is the port-claim vocabulary rather than a TCP one, and a granted one carries the socket's
/// capability.
pub const OPERATION_BIND_UDP: u64 = 11;

/// A connection's peer as the second reply word of `OPERATION_ACCEPT`: the IPv4 address in bits 47
/// to 16, big-endian as an address is written, and the port in bits 15 to 0.
///
/// Name: ratified 2026-11-20 (UTC).
pub const fn peer_word(octets: [u8; 4], port: u16) -> u64 {
    (u32::from_be_bytes(octets) as u64) << 16 | port as u64
}

/// The peer a [`peer_word`] names, or `None` for the word 0 that a reply without a peer carries
/// and for a word with any of the bits 63 to 48 set, which no stack here produces.
///
/// Name: ratified 2026-11-20 (UTC).
pub const fn word_peer(word: u64) -> Option<([u8; 4], u16)> {
    if word == 0 || word >> 48 != 0 {
        return None;
    }
    Some((((word >> 16) as u32).to_be_bytes(), word as u16))
}

/// The badge bit that marks a socket's capability. The stack mints every
/// socket's badge with this bit set, so a request whose badge has it names a socket and any other
/// badge is a front door. A closed socket's badge stays
/// in the socket range and leaves the stack's table, so it reaches nothing at all.
///
/// Name: ratified 2026-11-20 (UTC).
pub const SOCKET_BADGE_BIT: u64 = 1 << 63;

/// Does a request that arrived with `badge` name a socket? `false` means it came through a front
/// door.
///
/// # Examples
///
/// A badge in the socket range names a socket; any other badge is a front door:
///
/// ```
/// use socket_contract::{SOCKET_BADGE_BIT, names_a_socket};
///
/// assert!(names_a_socket(SOCKET_BADGE_BIT | 1));
/// assert!(!names_a_socket(0)); // the unbadged front door a spawner hands out
/// assert!(!names_a_socket(7)); // a front door a spawner badged for its own reasons
/// ```
pub const fn names_a_socket(badge: u64) -> bool {
    badge & SOCKET_BADGE_BIT != 0
}

// Reply words. The connect outcomes are their own small vocabulary, so a client can tell
// "refused" from "connected".

/// Success. Non-negative is success, and `RECEIVE` and `SEND` reply a byte count here instead: from
/// `RECEIVE`, 0 is the end of the stream, and from `SEND`, a count below the length sent means send
/// the rest.
///
/// Name: ratified 2026-11-20 (UTC).
pub const REPLY_OK: u64 = 0;
/// `OPERATION_CONNECT` succeeded.
pub const CONNECT_ESTABLISHED: u64 = 0;
/// The peer sent a reset (RST), or the connect otherwise failed or closed.
pub const CONNECT_REFUSED: u64 = 1;
/// A failure sentinel (a capability that names no live socket, a bad operation, or a server-side
/// timeout). Bit 32 set, so it is never mistaken for a length or a connect outcome.
///
/// Name: ratified 2026-11-20 (UTC).
pub const REPLY_ERROR: u64 = 1 << 32;

// `LISTEN`'s outcomes, its own small vocabulary because "never granted this port" and "somebody
// else holds it" call for opposite responses: ask for authority, or pick another port.

/// The port claim was granted; the reply carries the new capability.
///
/// Name: ratified 2026-11-20 (UTC).
pub const PORT_CLAIM_GRANTED: u64 = 0;
/// The port is outside this stack's [`listen_grant`]. The capability refusal: no retry, no
/// other port in range will help unless the caller was granted it.
///
/// Name: ratified 2026-11-20 (UTC).
pub const PORT_CLAIM_DENIED: u64 = 1;
/// The port is inside the grant but another socket already listens there. Exclusivity is the whole
/// reason a port is a grantable thing; this is what enforcing it looks like from the client side.
///
/// Name: ratified 2026-11-20 (UTC).
pub const PORT_CLAIM_IN_USE: u64 = 2;

/// No inbound authority at all, and the default a net server is spawned with.
///
/// Name: ratified 2026-11-20 (UTC).
pub const NO_PORT_GRANT: u64 = 0;

/// Pack an inclusive listen-port range into the one word a net server is spawned with. `start == 0` is
/// [`NO_PORT_GRANT`] whatever `end` says, because port 0 is not a port and a grant that starts at
/// "no port" grants nothing; that keeps the "no grant" case a single value a reader can recognize.
///
/// # Examples
///
/// The degenerate cases collapse to one value a reader can recognize:
///
/// ```
/// use socket_contract::{NO_PORT_GRANT, listen_grant};
///
/// assert_eq!(listen_grant(0, 65535), NO_PORT_GRANT); // port 0 is not a port
/// assert_eq!(listen_grant(8080, 80), NO_PORT_GRANT); // an inverted range grants nothing
/// ```
pub const fn listen_grant(start: u16, end: u16) -> u64 {
    if start == 0 || end < start {
        return NO_PORT_GRANT;
    }
    (start as u64) | ((end as u64) << 16)
}

/// May a holder of a stack carrying `grant` listen on `port`? The whole authority check, in one
/// place both the server and its tests read, so a server-side widening cannot pass a test that
/// keeps its own copy of the range.
///
/// Name: ratified 2026-11-20 (UTC).
pub const fn listen_grant_allows(grant: u64, port: u16) -> bool {
    let lo = (grant & 0xffff) as u16;
    let hi = ((grant >> 16) & 0xffff) as u16;
    lo != 0 && port >= lo && port <= hi
}

/// Pack an inclusive UDP bind port range into the high half of the spawn word (the
/// [`listen_grant`] holds the low half, and the two compose with `|`). The same degenerate cases as the listen grant, for the same reasons: `start == 0` grants nothing, and
/// an inverted range is a spawn-site mistake whose safe reading is "grants nothing".
///
/// # Examples
///
/// The two inbound authorities share one word and are independent: UDP 5353 is not TCP 5353.
///
/// ```
/// use socket_contract::{
///     NO_PORT_GRANT, listen_grant_allows, listen_grant, udp_bind_grant, udp_bind_grant_allows,
/// };
///
/// // What an mDNS responder is spawned with: one UDP port, no TCP listen authority at all.
/// let responder = udp_bind_grant(5353, 5353);
/// assert!(udp_bind_grant_allows(responder, 5353));
/// assert!(!listen_grant_allows(responder, 5353)); // the same number, and not the same authority
///
/// // What a file server is spawned with: SMB's port, and nothing on the UDP side.
/// let smb = listen_grant(445, 445);
/// assert!(listen_grant_allows(smb, 445));
/// assert!(!udp_bind_grant_allows(smb, 445));
///
/// // Both at once is one word, and neither half leaks into the other.
/// let both = smb | responder;
/// assert!(listen_grant_allows(both, 445) && udp_bind_grant_allows(both, 5353));
/// assert!(!listen_grant_allows(both, 5353) && !udp_bind_grant_allows(both, 445));
///
/// // The default, and what every outbound-only client is spawned with: nothing inbound.
/// assert!(!listen_grant_allows(NO_PORT_GRANT, 445));
/// assert!(!udp_bind_grant_allows(NO_PORT_GRANT, 5353));
/// ```
pub const fn udp_bind_grant(start: u16, end: u16) -> u64 {
    if start == 0 || end < start {
        return NO_PORT_GRANT;
    }
    ((start as u64) << 32) | ((end as u64) << 48)
}

/// May a holder of a stack carrying `grant` bind a UDP socket to the fixed `port`? [`listen_grant_allows`]
/// for the word's UDP half; the two halves are independent authorities, so granting a TCP listen
/// range grants no UDP port and the reverse.
///
/// Name: ratified 2026-11-20 (UTC).
pub const fn udp_bind_grant_allows(grant: u64, port: u16) -> bool {
    let lo = ((grant >> 32) & 0xffff) as u16;
    let hi = ((grant >> 48) & 0xffff) as u16;
    lo != 0 && port >= lo && port <= hi
}

/// Where a network stack finds the two capabilities that per-socket capabilities added to its
/// spawn, which its spawners and the stack itself must agree on, so they are here rather than
/// spelled per program.
pub mod stack_slots {
    /// The stack's own endpoint again, `WRITE | GRANT`: what each socket's capability is minted
    /// from with `BADGE`, which keeps the source's rights. Not the `READ` copy the stack serves on,
    /// because a socket minted from that could take other clients' requests.
    ///
    /// Name: ratified 2026-11-20 (UTC).
    pub const MINTING_ENDPOINT_SLOT: u64 = 7;
    /// The stack's own address space, `WRITE`: so `CLOSE` can `UNMAP` a socket's page before its
    /// table entry takes another client's.
    ///
    /// Name: ratified 2026-11-20 (UTC).
    pub const OWN_ADDRESS_SPACE_SLOT: u64 = 8;
}

/// The destination address's offset in the `PageFrame`: the first header field. The frame is
/// one region reused per synchronous `CALL`, not a ring.
///
/// ```text
///   +0x000  u8[4]  destination_address    destination (SENDTO / CONNECT); source on a UDP RECEIVE reply
///   +0x004  u16    destination_port  little-endian, the same two roles
///   +0x006  u16    len       payload length, written by RECEIVE only
///   +0x008  ...    payload   up to MAXIMUM_PAYLOAD_BYTES bytes
/// ```
///
/// Name: ratified 2026-11-14 (UTC) (#3571).
pub const DESTINATION_IPV4_ADDRESS_OFFSET_BYTES: u64 = 0x000;
/// The destination port, right after the destination Internet Protocol (IP) address.
///
/// Name: ratified 2026-11-14 (UTC) (#3571).
pub const DESTINATION_PORT_OFFSET_BYTES: u64 = 0x004;
/// The payload length of a received datagram or segment. Every multi-byte frame field is
/// little-endian; the lease word, by contrast, carries its address big-endian in its low 32 bits,
/// because that is the order an address is read in. Only `RECEIVE` writes it,
/// and nothing in this contract reads it: a send carries its length in the argument word of its
/// `CALL`, checked against [`MAXIMUM_PAYLOAD_BYTES`], and ignores this field. A client may clear it before
/// a `RECEIVE` to watch for the reply.
///
/// Name: ratified 2026-11-14 (UTC) (#3571).
pub const PAYLOAD_LENGTH_OFFSET_BYTES: u64 = 0x006;
/// Where the payload bytes start.
///
/// Name: ratified 2026-11-14 (UTC) (#3571).
pub const PAYLOAD_OFFSET_BYTES: u64 = 0x008;

/// The shared frame's size in bytes: one 4 KiB page. [`MAXIMUM_PAYLOAD_BYTES`] derives from it, and a client
/// runtime that maps the frame can import it.
///
/// Name: ratified 2026-11-20 (UTC).
pub const FRAME_BYTES: usize = 4096;

/// The largest payload the frame carries (a 4 KiB frame minus the header).
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_PAYLOAD_BYTES: usize = FRAME_BYTES - PAYLOAD_OFFSET_BYTES as usize;

/// The lease report the network stack sends its spawner once the Dynamic Host Configuration Protocol (DHCP) completes. Word 0 is the leased Internet Protocol version 4
/// (IPv4) address, big-endian in the low 32 bits; word 1 the first Domain Name System (DNS) server the lease named, for the
/// resolver's spawner (the name resolver is its own confined program), the same encoding or
/// [`lease::NO_NAME_SERVER`]; word 2 is zero and reserved.
///
/// ```
/// use socket_contract::lease;
///
/// let word = lease::ipv4_to_word([10, 0, 2, 3]);
/// assert_eq!(lease::word_to_ipv4(word), Some([10, 0, 2, 3]));
/// assert_eq!(lease::word_to_ipv4(lease::NO_NAME_SERVER), None);
/// ```
pub mod lease {
    /// Word 1 when the lease named no DNS server. 0.0.0.0 is never a nameserver, so it cannot
    /// collide with one.
    ///
    /// Name: ratified 2026-11-20 (UTC).
    pub const NO_NAME_SERVER: u64 = 0;

    /// An IPv4 address as a report word: big-endian in the low 32 bits.
    ///
    /// Name: ratified 2026-11-20 (UTC).
    pub const fn ipv4_to_word(octets: [u8; 4]) -> u64 {
        u32::from_be_bytes(octets) as u64
    }

    /// A report word back to an address, or `None` for [`NO_NAME_SERVER`] or a word with any of the
    /// high 32 bits set, which no encoder here produces.
    ///
    /// Name: ratified 2026-11-20 (UTC).
    pub const fn word_to_ipv4(word: u64) -> Option<[u8; 4]> {
        if word == NO_NAME_SERVER || word > u32::MAX as u64 {
            return None;
        }
        Some((word as u32).to_be_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_peer_word_round_trips_and_refuses_what_no_stack_writes() {
        assert_eq!(peer_word([10, 0, 2, 2], 50161), 0x0a00_0202_c3f1);
        assert_eq!(word_peer(0x0a00_0202_c3f1), Some(([10, 0, 2, 2], 50161)));
        assert_eq!(
            word_peer(peer_word([255; 4], u16::MAX)),
            Some(([255; 4], u16::MAX))
        );
        assert_eq!(
            word_peer(peer_word([0, 0, 0, 1], 0)),
            Some(([0, 0, 0, 1], 0))
        );
        assert_eq!(word_peer(0), None);
        assert_eq!(word_peer(1 << 48), None);
    }

    #[test]
    fn a_lease_word_round_trips_and_refuses_what_no_encoder_writes() {
        assert_eq!(lease::ipv4_to_word([10, 0, 2, 3]), 0x0a00_0203);
        assert_eq!(lease::word_to_ipv4(0x0a00_0203), Some([10, 0, 2, 3]));
        assert_eq!(lease::word_to_ipv4(lease::NO_NAME_SERVER), None);
        assert_eq!(lease::word_to_ipv4(1 << 32), None);
        assert_eq!(lease::word_to_ipv4(u32::MAX as u64), Some([255; 4]));
    }

    /// Every opcode the contract defines, in one place: a list repeated per property will one day
    /// be missing an entry in exactly the property that would have caught it.
    const OPERATIONS: &[u64] = &[
        OPERATION_ATTACH_SHARED_PAGE,
        OPERATION_OPEN_UDP,
        OPERATION_OPEN_TCP,
        OPERATION_SEND_TO,
        OPERATION_RECEIVE,
        OPERATION_CONNECT,
        OPERATION_SEND,
        OPERATION_CLOSE,
        OPERATION_LISTEN,
        OPERATION_ACCEPT,
        OPERATION_BIND_UDP,
    ];

    /// Opcodes are distinct from each other and from zero, and stay small. The request word is the
    /// opcode alone now, so a wide opcode costs nothing on the wire, but zero is what an empty
    /// word reads as and must never mean an operation.
    #[test]
    fn every_opcode_is_nonzero_and_small() {
        for &operation in OPERATIONS {
            assert!(operation != 0 && operation <= 0xff, "opcode {operation}");
        }
    }

    /// Opcodes are distinct. Two sharing a number is one operation silently performing another.
    #[test]
    fn opcodes_are_distinct() {
        for (i, a) in OPERATIONS.iter().enumerate() {
            for b in &OPERATIONS[i + 1..] {
                assert_ne!(a, b, "two opcodes share a number");
            }
        }
    }

    /// The socket range and the front door never overlap. A badge either names a socket or is
    /// a front door; if one value could be both, a closed socket's capability would open new
    /// sockets, or a front door would reach someone's socket.
    #[test]
    fn a_badge_is_a_socket_or_a_front_door_and_never_both() {
        assert!(!names_a_socket(0), "the unbadged front door names a socket");
        for b in [1u64, 7, 1 << 62, (1 << 63) - 1] {
            assert!(!names_a_socket(b), "front door badge {b:#x} names a socket");
        }
        for b in [SOCKET_BADGE_BIT, SOCKET_BADGE_BIT | 1, u64::MAX] {
            assert!(
                names_a_socket(b),
                "socket badge {b:#x} reads as a front door"
            );
        }
    }

    /// The shared frame's header fields do not overlap, and the payload starts after all of them.
    /// An overlap here means a destination port written over an address, which would send bytes to
    /// the wrong host rather than failing.
    #[test]
    fn the_frame_header_fields_do_not_overlap() {
        let mut offs: [u64; 3] = [
            DESTINATION_IPV4_ADDRESS_OFFSET_BYTES,
            DESTINATION_PORT_OFFSET_BYTES,
            PAYLOAD_LENGTH_OFFSET_BYTES,
        ];
        offs.sort_unstable();
        for w in offs.windows(2) {
            assert_ne!(w[0], w[1], "two header fields share an offset");
        }
        for o in offs {
            assert!(
                o < PAYLOAD_OFFSET_BYTES,
                "header field {o} runs into the payload"
            );
        }
    }

    /// The payload is exactly the rest of the frame: the header, then bytes to the last one on the
    /// page. Larger and a full-size send writes past the end of the frame the client was granted;
    /// smaller and part of a granted page is unreachable, which nothing anywhere would report.
    #[test]
    fn a_full_payload_fills_the_shared_frame_exactly() {
        assert_eq!(
            PAYLOAD_OFFSET_BYTES as usize + MAXIMUM_PAYLOAD_BYTES,
            4096,
            "the payload is not the whole frame minus its header"
        );
    }

    /// The connect outcomes are distinguishable from each other and from the generic reply codes,
    /// which is the whole reason they have their own vocabulary.
    #[test]
    fn connect_outcomes_are_their_own_vocabulary() {
        assert_ne!(CONNECT_ESTABLISHED, CONNECT_REFUSED);
        assert_ne!(CONNECT_ESTABLISHED, REPLY_ERROR);
        assert_ne!(CONNECT_REFUSED, REPLY_ERROR);
    }

    /// The same for `LISTEN`'s three outcomes, and for the same reason: a client that cannot tell
    /// a refusal of *authority* from a collision on a port cannot do the right thing about either.
    #[test]
    fn listen_outcomes_are_their_own_vocabulary() {
        let outcomes = [
            PORT_CLAIM_GRANTED,
            PORT_CLAIM_DENIED,
            PORT_CLAIM_IN_USE,
            REPLY_ERROR,
        ];
        for (i, a) in outcomes.iter().enumerate() {
            for b in &outcomes[i + 1..] {
                assert_ne!(a, b, "two listen outcomes share a value");
            }
        }
    }

    /// A grant admits exactly its range and nothing else. This is the authority check itself,
    /// so an off-by-one here is a server binding a port it was never granted, which no other test
    /// in the tree would see: the on-device gate asks for one allowed port and one denied one.
    #[test]
    fn a_listen_grant_admits_its_range_and_refuses_everything_outside_it() {
        let g = listen_grant(7778, 7780);
        assert!(
            !listen_grant_allows(g, 7777),
            "one below the range was allowed"
        );
        for p in 7778..=7780 {
            assert!(
                listen_grant_allows(g, p),
                "port {p} inside the range was refused"
            );
        }
        assert!(
            !listen_grant_allows(g, 7781),
            "one above the range was allowed"
        );
        assert!(
            !listen_grant_allows(g, 80),
            "a well-known port outside was allowed"
        );

        // A single-port grant is the common case (one server, one port) and the tightest one.
        let one = listen_grant(80, 80);
        assert!(listen_grant_allows(one, 80));
        assert!(!listen_grant_allows(one, 81));
    }

    /// No grant means no port. The default a server is spawned with must admit nothing, or
    /// "inbound authority is granted, never assumed" is a sentence in a doc comment and not a
    /// property of the system. Port 0 is refused under every grant, including one that names it,
    /// because it is not a port anything can listen on.
    #[test]
    fn the_default_grant_admits_no_port_at_all() {
        for p in [0u16, 1, 80, 7778, 65535] {
            assert!(
                !listen_grant_allows(NO_PORT_GRANT, p),
                "port {p} was allowed with no grant"
            );
        }
        assert_eq!(listen_grant(0, 65535), NO_PORT_GRANT);
        assert!(!listen_grant_allows(listen_grant(0, 65535), 7778));
        assert!(!listen_grant_allows(listen_grant(1, 65535), 0));
        // An inverted range is a mistake at the spawn site, and the safe reading of a mistake in a
        // grant is "grants nothing", never "grants the ports between them in the other order".
        assert_eq!(listen_grant(7780, 7778), NO_PORT_GRANT);
    }

    /// The UDP bind grant admits exactly its range, the twin of the listen-grant test above
    /// and for the same reason: this is the authority check itself, and the on-device gate asks
    /// for one allowed port and one denied one, so an off-by-one here is a server binding a UDP
    /// port it was never granted with no other test to see it.
    #[test]
    fn a_udp_bind_grant_admits_its_range_and_refuses_everything_outside_it() {
        let g = udp_bind_grant(5353, 5355);
        assert!(
            !udp_bind_grant_allows(g, 5352),
            "one below the range was allowed"
        );
        for p in 5353..=5355 {
            assert!(
                udp_bind_grant_allows(g, p),
                "port {p} inside the range was refused"
            );
        }
        assert!(
            !udp_bind_grant_allows(g, 5356),
            "one above the range was allowed"
        );

        // The single-port grant is the one the multicast name service actually spawns with (5353).
        let one = udp_bind_grant(5353, 5353);
        assert!(udp_bind_grant_allows(one, 5353));
        assert!(!udp_bind_grant_allows(one, 5354));

        // The degenerate cases read as "grants nothing", exactly as the listen grant's do.
        assert_eq!(udp_bind_grant(0, 65535), NO_PORT_GRANT);
        assert_eq!(udp_bind_grant(5355, 5353), NO_PORT_GRANT);
        for p in [0u16, 1, 5353, 65535] {
            assert!(
                !udp_bind_grant_allows(NO_PORT_GRANT, p),
                "port {p} allowed with no grant"
            );
        }
    }

    /// The two grants share one word and neither leaks into the other. They are different
    /// authorities (receive TCP connections on a port, versus claim a fixed UDP port), so a spawn
    /// that grants one must not have granted the other, and the composed word must answer each
    /// question from its own half only. A packing overlap here is a stack granting mDNS's port to
    /// anything that was granted a TCP listen range, silently.
    #[test]
    fn the_listen_and_udp_bind_grants_compose_without_leaking() {
        let listen_only = listen_grant(7778, 7778);
        assert!(listen_grant_allows(listen_only, 7778));
        assert!(
            !udp_bind_grant_allows(listen_only, 7778),
            "a listen grant granted a UDP bind"
        );

        let udp_only = udp_bind_grant(5353, 5353);
        assert!(udp_bind_grant_allows(udp_only, 5353));
        assert!(
            !listen_grant_allows(udp_only, 5353),
            "a UDP bind grant granted a TCP listen"
        );

        let both = listen_grant(7778, 7778) | udp_bind_grant(5353, 5353);
        assert!(listen_grant_allows(both, 7778));
        assert!(udp_bind_grant_allows(both, 5353));
        assert!(
            !listen_grant_allows(both, 5353),
            "the UDP half answered a listen question"
        );
        assert!(
            !udp_bind_grant_allows(both, 7778),
            "the listen half answered a UDP question"
        );

        // The top of the port space survives the high half's packing.
        let top = udp_bind_grant(65535, 65535);
        assert!(udp_bind_grant_allows(top, 65535));
        assert!(!udp_bind_grant_allows(top, 65534));
    }

    /// A grant round-trips through the one word a spawn argument carries, up to the top of the
    /// port space. It rides in a `u64` beside nothing else, so the only way to lose a bit is to
    /// pack it wrong.
    #[test]
    fn a_grant_survives_the_word_it_is_spawned_with() {
        for (lo, hi) in [
            (1u16, 65535u16),
            (49152, 65535),
            (65535, 65535),
            (7778, 7778),
        ] {
            let g = listen_grant(lo, hi);
            assert!(
                listen_grant_allows(g, lo),
                "the low end was lost for {lo}..={hi}"
            );
            assert!(
                listen_grant_allows(g, hi),
                "the high end was lost for {lo}..={hi}"
            );
        }
    }
}

/// Machine-checked proofs, run with `cargo kani`. The port grants are the authority checks, so each
/// harness ranges over every pair of bounds and every port rather than the cases a test lists.
#[cfg(kani)]
mod verification {
    use super::*;

    /// **A listen grant admits exactly its range**: for any bounds and any port, a word packed by
    /// [`listen_grant`] lets [`listen_grant_allows`] admit the port exactly when the low bound is a port and
    /// the port lies between the bounds, and it grants no UDP port at all.
    /// Falsification: replayable `crates/socket_contract/falsifications/verification.a_listen_grant_admits_exactly_its_range.patch`
    #[kani::proof]
    fn a_listen_grant_admits_exactly_its_range() {
        let lowest: u16 = kani::any();
        let highest: u16 = kani::any();
        let port: u16 = kani::any();
        let grant = listen_grant(lowest, highest);
        assert_eq!(
            listen_grant_allows(grant, port),
            lowest != 0 && lowest <= port && port <= highest
        );
        assert!(!udp_bind_grant_allows(grant, port));
    }

    /// **A UDP bind grant admits exactly its range**: the same property for [`udp_bind_grant`] and
    /// [`udp_bind_grant_allows`], and such a word grants no listen port at all.
    /// Falsification: replayable `crates/socket_contract/falsifications/verification.a_bind_grant_admits_exactly_its_range.patch`
    #[kani::proof]
    fn a_bind_grant_admits_exactly_its_range() {
        let lowest: u16 = kani::any();
        let highest: u16 = kani::any();
        let port: u16 = kani::any();
        let grant = udp_bind_grant(lowest, highest);
        assert_eq!(
            udp_bind_grant_allows(grant, port),
            lowest != 0 && lowest <= port && port <= highest
        );
        assert!(!listen_grant_allows(grant, port));
    }

    /// **The two halves of a composed grant are independent**: for any listen range, any UDP range
    /// and any port, the word the two make with `|` answers each question exactly as its own half
    /// alone does.
    /// Falsification: replayable `crates/socket_contract/falsifications/verification.a_composed_grant_answers_each_question_from_its_own_half.patch`
    #[kani::proof]
    fn a_composed_grant_answers_each_question_from_its_own_half() {
        let listen = listen_grant(kani::any(), kani::any());
        let bind = udp_bind_grant(kani::any(), kani::any());
        let port: u16 = kani::any();
        let both = listen | bind;
        assert_eq!(
            listen_grant_allows(both, port),
            listen_grant_allows(listen, port)
        );
        assert_eq!(
            udp_bind_grant_allows(both, port),
            udp_bind_grant_allows(bind, port)
        );
    }

    /// **A peer word round-trips for every address and port**: for any four octets and any port,
    /// [`word_peer`] of [`peer_word`] gives them back, and a word with a bit above 47 set names no peer.
    /// Falsification: replayable `crates/socket_contract/falsifications/verification.a_peer_word_round_trips_for_every_host_and_port.patch`
    #[kani::proof]
    fn a_peer_word_round_trips_for_every_host_and_port() {
        let octets: [u8; 4] = kani::any();
        let port: u16 = kani::any();
        let word = peer_word(octets, port);
        assert_eq!(word >> 48, 0);
        if word != 0 {
            assert_eq!(word_peer(word), Some((octets, port)));
        } else {
            assert_eq!(word_peer(word), None);
        }
        let stray: u64 = kani::any();
        kani::assume(stray >> 48 != 0);
        assert_eq!(word_peer(stray), None);
    }
}

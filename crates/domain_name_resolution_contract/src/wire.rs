//! **What a client and the name resolver put on the wire**: the client's page and words, alone, so
//! the file can be copied verbatim into a standard library that speaks this contract, which cannot
//! name this crate's dependency. It names
//! nothing outside itself, which is the condition of that copy. The spawner's control words, which
//! no client sends, are in `control`; the grant table and the reject codes need
//! `domain_name_system`, and are in their own modules.

/// The longest host name a client may write, in dotted text without a trailing dot. RFC 1035's
/// 255-byte wire limit is 253 characters of dotted text.
///
/// Name: provisional, decided by the maintainer on 2026-11-14 (UTC) at the crate's outside review:
/// a limit opens `MAXIMUM_` and ends with its unit.
pub const MAXIMUM_NAME_TEXT_BYTES: usize = 253;

/// The most bytes the name field holds: a host name of [`MAXIMUM_NAME_TEXT_BYTES`] and the
/// trailing dot a fully qualified name may end with. The one bound on a resolve's name length and
/// on a grant's zone text, so the client, the resolver and the grant table read the same number.
///
/// Name: provisional, minted 2026-11-16 (UTC) by milestone/1086-resolver-contract-outside-review-items.
pub const MAXIMUM_NAME_FIELD_BYTES: usize = MAXIMUM_NAME_TEXT_BYTES + 1;

/// How many IPv4 addresses one answer carries back, the same bound `domain_name_system::Answer`
/// keeps.
///
/// Name: provisional, decided by the maintainer on 2026-11-14 (UTC) at the crate's outside review:
/// an address that is not a memory location says which kind it is, as
/// `domain_name_system::MAXIMUM_IPV4_ADDRESSES` does.
pub const MAXIMUM_IPV4_ADDRESSES: usize = 8;

/// Where the client writes the host name in its page.
///
/// Name: ratified 2026-11-14 (UTC) (#3571).
pub const NAME_OFFSET_BYTES: usize = 0x000;
/// Where the resolver writes the addresses in the client's page, four bytes each, in the order the
/// name server gave them.
///
/// Name: provisional, decided by the maintainer on 2026-11-14 (UTC) at the crate's outside review:
/// the `<FIELD>_OFFSET_BYTES` shape is the family-wide one ratified 2026-11-14 (UTC) (#3571), and
/// the field is spelled `IPV4_ADDRESSES` as `MAXIMUM_IPV4_ADDRESSES` is.
pub const IPV4_ADDRESSES_OFFSET_BYTES: usize = 0x100;

// The name field ends before the addresses begin, and the addresses end inside the page.
const _: () = assert!(NAME_OFFSET_BYTES + MAXIMUM_NAME_FIELD_BYTES <= IPV4_ADDRESSES_OFFSET_BYTES);
const _: () = assert!(IPV4_ADDRESSES_OFFSET_BYTES + 4 * MAXIMUM_IPV4_ADDRESSES <= 4096);

// =================================================================================================
// The words.
// =================================================================================================

/// **A client delegates its page.** A `SEND_CAPABILITY` of a writable page frame with this request word.
/// The resolver maps it in the window its badge's grant names; a page from a badge with no grant
/// is deleted unmapped.
pub const OPERATION_ATTACH_PAGE_FRAME: u64 = 1;
/// **A client resolves the name in its page.** A `CALL` whose second word is the name's length in
/// bytes. The reply is an [`Outcome`] word and the answer's TTL in seconds.
pub const OPERATION_RESOLVE: u64 = 2;
/// A client's request word.
pub const fn request(operation: u64) -> u64 {
    operation << 56
}

/// The operation in any first word, client or control.
pub const fn operation(first_word: u64) -> u64 {
    first_word >> 56
}

// =================================================================================================
// The reply.
// =================================================================================================

/// What a [`OPERATION_RESOLVE`] reply's first word says. Each is a distinct reason, because "no
/// address" is a different fact from "you may not ask", and a client that collapses them cannot
/// tell its own misconfiguration from the network's.
pub mod status {
    /// Resolved. The count is how many addresses are in the page.
    pub const OK: u8 = 0;
    /// **This client may not ask about this name**: no grant for its badge, or the name is outside
    /// the zone it was granted. Nothing was sent to the network.
    pub const DENIED: u8 = 1;
    /// The name server says the name does not exist (NXDOMAIN).
    pub const NO_SUCH_NAME: u8 = 2;
    /// A reply came back and `domain_name_system::Query::accept` refused it. The detail byte is
    /// the protocol crate's `reject_code` of the reason.
    pub const REFUSED: u8 = 3;
    /// No reply the resolver could believe arrived before it gave up.
    pub const NO_ANSWER: u8 = 4;
    /// The name server answered with a failure other than NXDOMAIN. The detail byte is its response
    /// code.
    pub const SERVER_ERROR: u8 = 5;
    /// The page did not hold a host name `domain_name_system::Name::from_dotted` accepts.
    pub const BAD_NAME: u8 = 6;
    /// The client has a grant and has not attached its page yet.
    pub const NOT_ATTACHED: u8 = 7;
    /// The resolver has no source of unguessable transaction ids, so it asks nothing.
    pub const NO_ENTROPY: u8 = 8;
    /// The socket contract refused the resolver. The detail byte names the step.
    pub const NETWORK: u8 = 9;
    /// **The request was refused as malformed**: an operation this protocol does not have, a
    /// resolve from the spawner's own badge, or a control message the grant table refused. For a
    /// control message the detail byte is the reason, one of the crate's `refusal` numbers.
    pub const BAD_REQUEST: u8 = 10;
}

/// Which step of the socket contract refused the resolver: [`status::NETWORK`]'s detail byte. In
/// the client half, so a standard library can say which step failed instead of one error for all
/// four.
///
/// Name: provisional, the riscv-cpu-net-flake lane, 2026-10-13 (UTC). `UNKNOWN_TRANSPORT` is
/// provisional too, ruled in the milestone 1086 rulings lane, 2026-11-19 (UTC).
pub mod network_step {
    /// The resolver has no socket page to attach.
    pub const NO_SOCKET_PAGE: u8 = 1;
    /// The stack refused to open a socket.
    pub const OPEN: u8 = 2;
    /// The stack refused a send.
    pub const SEND: u8 = 3;
    /// The TCP connection to the name server was not established.
    pub const CONNECT: u8 = 4;
    /// The spawner started the resolver with a transport word that is neither `TRANSPORT_UDP` nor
    /// `TRANSPORT_TCP`. The resolver refuses it at start and answers every resolve with this step.
    pub const UNKNOWN_TRANSPORT: u8 = 5;
}

/// A resolve reply's first word, decoded: `status | detail << 8 | count << 16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    /// One of [`status`].
    pub status: u8,
    /// What else the status carries: a reject code, a response code, a network step, or 0.
    pub detail: u8,
    /// How many addresses the resolver wrote at [`IPV4_ADDRESSES_OFFSET_BYTES`]: 0 unless the status is OK.
    pub count: u8,
}

impl Outcome {
    /// A status with no detail and no addresses.
    pub const fn of(status: u8) -> Outcome {
        Outcome {
            status,
            detail: 0,
            count: 0,
        }
    }

    /// The reply word.
    pub const fn word(self) -> u64 {
        self.status as u64 | (self.detail as u64) << 8 | (self.count as u64) << 16
    }

    /// A reply word back to its fields. The resolver never sets the high 40 bits, so a word with any
    /// of them set is not its reply: it is the kernel's error from a `CALL` that failed (a negative
    /// number, read as an enormous one), and decodes to `None`. So does a word the resolver cannot
    /// send: a status the [`status`] module does not define, or a count above
    /// [`MAXIMUM_IPV4_ADDRESSES`]. The bound belongs here, where every client gets it.
    pub const fn from_word(word: u64) -> Option<Outcome> {
        if word >> 24 != 0 {
            return None;
        }
        let status = word as u8;
        let count = (word >> 16) as u8;
        if status > status::BAD_REQUEST || count as usize > MAXIMUM_IPV4_ADDRESSES {
            return None;
        }
        Some(Outcome {
            status,
            detail: (word >> 8) as u8,
            count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_word_carries_its_operation() {
        for operation_code in [OPERATION_ATTACH_PAGE_FRAME, OPERATION_RESOLVE] {
            assert_eq!(operation(request(operation_code)), operation_code);
        }
    }

    #[test]
    fn an_outcome_word_round_trips_and_a_kernel_error_is_not_one() {
        let o = Outcome {
            status: status::OK,
            detail: 3,
            count: 2,
        };
        assert_eq!(Outcome::from_word(o.word()), Some(o));
        assert_eq!(Outcome::of(status::DENIED).word(), status::DENIED as u64);
        // A failed CALL returns a negative number, which no reply word looks like.
        assert_eq!(Outcome::from_word(-1i64 as u64), None);
    }

    /// A word the resolver cannot send is refused by the contract, so no client has to clamp it.
    #[test]
    fn an_outcome_word_the_resolver_cannot_send_is_refused() {
        let count_too_large = status::OK as u64 | (MAXIMUM_IPV4_ADDRESSES as u64 + 1) << 16;
        assert_eq!(Outcome::from_word(count_too_large), None);
        assert_eq!(Outcome::from_word(255 << 16), None);
        let at_the_limit = status::OK as u64 | (MAXIMUM_IPV4_ADDRESSES as u64) << 16;
        assert!(Outcome::from_word(at_the_limit).is_some());
        assert!(Outcome::from_word(status::BAD_REQUEST as u64).is_some());
        assert_eq!(Outcome::from_word(status::BAD_REQUEST as u64 + 1), None);
        assert_eq!(Outcome::from_word(200), None);
    }
}

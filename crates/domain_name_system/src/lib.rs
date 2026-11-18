//! **The Domain Name System (DNS) wire format a stub resolver needs** (Request for Comments (RFC)
//! 1035; RFC 7766 for the Transmission Control Protocol, TCP), as pure
//! computation: the bytes of a query for one name's Internet Protocol version 4 (IPv4) addresses, and the checks a reply must pass
//! before any address in it is believed. No socket, timer or policy; which program sends these
//! bytes, and which names a client may ask about, are the caller's policy.
//!
//! Every byte parsed here came from a server the client may not trust, so the parser is total (Kani
//! proves the name decoder, the header codec and the TCP reassembly, and this crate's `fuzz/`
//! target covers the rest), and
//! [`Query::accept`] believes as little as it can: [`Reject`] lists its nine refusals, in check
//! order. Only A records owned by the end of the canonical name chain from our name are taken, so a reply
//! about `example.com` cannot plant an address for `bank.test`; authority and additional go unread.

//! # BUGS
//!
//! - IPv4 only: a query asks for address records alone, so it reads no address longer than four bytes.
//! - No Extension Mechanisms for DNS (EDNS) version 0, so UDP (the User Datagram Protocol) replies stop at [`MAXIMUM_UDP_MESSAGE_BYTES`] and a larger answer costs a TCP
//!   retry.
//! - No DNS Security Extensions (DNSSEC): an on-path attacker can forge the reply, id and all. No "0x20" case randomization.
//! - At most [`MAXIMUM_IPV4_ADDRESSES`] addresses are kept from one answer; the rest are dropped.
//! - Labels hold only letters, digits, `-` and `_` ([`Name::from_dotted`]): no escape syntax.
//!
//! # Sizes
//!
//! [`Name`] is 256 bytes and `Copy`, and [`Reader::record`] returns a [`Record`] by value, so
//! [`Query::accept`] copies a name once per record on each of its three passes over the answer
//! section. [`Answer`] is 296 bytes and `Result<Answer, Reject>` is 300, so a caller on a small
//! stack should count them beside its 514-byte buffer.
//!
//! Name: ratified 2026-10-18 (calef, a crate-naming session). Kept: the external DNS wire format,
//! named for the standard's full name, spelled out under §154 (the acronym test). His answer was
//! "`domain_name_system`". Refused `dns` (the acronym test) and `name_resolver` (it names a program
//! rather than this wire format).

#![no_std]
#![forbid(unsafe_code)]

#[cfg(kani)]
mod proofs;

/// The port a name server listens on, over UDP and TCP alike (RFC 1035 section 4.2).
pub const PORT: u16 = 53;

/// The largest message RFC 1035 lets travel over the User Datagram Protocol (UDP) without EDNS
/// (section 4.2.1). A reply that would be larger arrives truncated, with [`FLAG_TRUNCATED`] set.
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_UDP_MESSAGE_BYTES: usize = 512;

/// The largest message RFC 7766 lets travel over TCP: its two-byte length prefix counts up to
/// 65,535 bytes, and the prefix itself comes on top.
///
/// Name: provisional, minted 2026-11-15 (UTC) by lane/1087-outside-review-fixes.
pub const MAXIMUM_TCP_MESSAGE_BYTES: usize = 65_535;

/// A name in wire form is at most 255 bytes, length bytes and the root label included (section 2.3.4).
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_NAME_BYTES: usize = 255;
/// A label is at most 63 bytes; the two top bits of a longer count are the compression-pointer tag.
///
/// Name: provisional, minted 2026-11-15 (UTC) by lane/1087-outside-review-fixes; the spelling ratified
/// in #2797 was `MAXIMUM_LABEL_LEN`.
pub const MAXIMUM_LABEL_BYTES: usize = 63;

/// The header every DNS message starts with.
///
/// Name: provisional, minted 2026-11-15 (UTC) by lane/1087-outside-review-fixes.
pub const HEADER_BYTES: usize = 12;

/// The response bit: set in a reply.
///
/// Name: provisional, minted 2026-11-15 (UTC) by lane/1087-outside-review-fixes.
#[doc(alias = "QR")]
pub const FLAG_RESPONSE: u16 = 0x8000;
/// The operation code field. A standard query has operation code 0.
///
/// Name: provisional, minted 2026-11-16 (UTC) by lane/dns-ttl-seconds-canonical-name.
#[doc(alias = "OPCODE")]
pub const OPERATION_CODE_MASK: u16 = 0x7800;
/// Truncated: the server had more to say than the transport carried.
///
/// Name: provisional, minted 2026-11-15 (UTC) by lane/1087-outside-review-fixes.
#[doc(alias = "TC")]
pub const FLAG_TRUNCATED: u16 = 0x0200;
/// Recursion desired. A stub resolver always sets it, because it asks a recursive server to do the
/// walk from the root that a stub does not.
///
/// Name: provisional, minted 2026-11-15 (UTC) by lane/1087-outside-review-fixes.
#[doc(alias = "RD")]
pub const FLAG_RECURSION_DESIRED: u16 = 0x0100;
/// The response code field, the low four bits of the flags.
///
/// Name: provisional, minted 2026-11-15 (UTC) by lane/1087-outside-review-fixes.
#[doc(alias = "RCODE")]
pub const RESPONSE_CODE_MASK: u16 = 0x000f;

/// The response code a server sends for a name that does not exist (RFC 1035's "Name Error").
///
/// Name: provisional, minted 2026-11-15 (UTC) by lane/1087-outside-review-fixes.
#[doc(alias = "RCODE")]
#[doc(alias = "NXDOMAIN")]
pub const RESPONSE_CODE_NAME_ERROR: u8 = 3;

/// Record types this crate reads.
pub mod record_type {
    /// An address record, whose data is one IPv4 address.
    ///
    /// Name: provisional, minted 2026-11-16 (UTC) by lane/dns-ttl-seconds-canonical-name.
    #[doc(alias = "A")]
    #[doc(alias = "address record")]
    pub const IPV4_ADDRESS: u16 = 1;
    /// A canonical name record: the owner is an alias, and the record's data is the
    /// canonical name.
    ///
    /// Name: provisional, minted 2026-11-16 (UTC) by lane/dns-ttl-seconds-canonical-name.
    #[doc(alias = "canonical name record")]
    #[doc(alias = "CNAME")]
    pub const CANONICAL_NAME: u16 = 5;
}

/// The Internet class, the only one a host-name lookup uses.
#[doc(alias = "Internet class")]
pub const CLASS_IN: u16 = 1;

/// How many canonical name links [`Query::accept`] follows before giving up. Real chains are one or two
/// links (a CDN alias, sometimes a second); eight leaves room for that and stops a reply that chains
/// a name through itself from costing more than eight scans of its own answer section.
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_CHAIN: usize = 8;

/// How many IPv4 addresses an [`Answer`] keeps (see the crate's BUGS).
///
/// Name: provisional, minted 2026-11-15 (UTC) by lane/1087-outside-review-fixes; the spelling ratified
/// in #2797 was `MAXIMUM_ADDRESSES`.
pub const MAXIMUM_IPV4_ADDRESSES: usize = 8;

/// What is wrong with some bytes, as a parser sees them.
///
/// Decoding errors mean a message is not valid DNS and the right response is to drop it; building
/// errors are a caller's mistake (a buffer too small, a name that cannot be one) and are reported
/// rather than panicking, because a resolver must not be crashable by its own client's input.
///
/// Non-exhaustive: Longer addresses or EDNS(0) would add variants, so a caller matches with a
/// wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The input ends before the structure it promised.
    Truncated,
    /// A label length byte uses the reserved `0x40` or `0x80` tag bits, or a dotted name has an
    /// empty label.
    BadLabel,
    /// A label in a dotted name exceeds 63 bytes.
    LabelTooLong,
    /// A name exceeds 255 bytes in wire form.
    NameTooLong,
    /// A dotted name holds a byte outside letters, digits, `-` and `_`.
    BadCharacter,
    /// A compression pointer that does not point strictly backwards. Every message a correct encoder
    /// produces satisfies that (a pointer names an earlier name), and enforcing it is also what makes
    /// the decoder terminate on a pointer loop.
    PointerForward,
    /// A record's data is the wrong shape for its type: an A record that is not four bytes, or a
    /// canonical name whose name runs past its own record.
    BadRecordData,
    /// The output buffer is too small, or a TCP reply declares more than its buffer holds.
    Overflow,
    /// A TCP reply carried bytes after the message its length announced.
    Trailing,
}

/// This crate's results.
pub type Result<T> = core::result::Result<T, Error>;

// =================================================================================================
// The header.
// =================================================================================================

/// The 12-byte header, decoded. Judging the flags is the caller's business, so decoding it is
/// total on any input long enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Header {
    /// The transaction id a reply must echo.
    pub id: u16,
    /// QR, the opcode, AA, TC, RD, RA and the response code, packed.
    pub flags: u16,
    /// How many questions follow.
    #[doc(alias = "QDCOUNT")]
    pub question_count: u16,
    /// How many answer records follow the questions.
    #[doc(alias = "ANCOUNT")]
    pub answer_count: u16,
    /// How many authority records follow the answers.
    #[doc(alias = "NSCOUNT")]
    pub authority_count: u16,
    /// How many additional records follow the authority records.
    #[doc(alias = "ARCOUNT")]
    pub additional_count: u16,
}

impl Header {
    /// Decode the first 12 bytes of `msg`.
    pub fn parse(msg: &[u8]) -> Result<Header> {
        let b = msg.get(..HEADER_BYTES).ok_or(Error::Truncated)?;
        let f = |i: usize| u16::from_be_bytes([b[i], b[i + 1]]);
        Ok(Header {
            id: f(0),
            flags: f(2),
            question_count: f(4),
            answer_count: f(6),
            authority_count: f(8),
            additional_count: f(10),
        })
    }

    /// Encode into the first 12 bytes of `out`.
    pub fn write(&self, out: &mut [u8]) -> Result<()> {
        let out = out.get_mut(..HEADER_BYTES).ok_or(Error::Overflow)?;
        let words = [
            self.id,
            self.flags,
            self.question_count,
            self.answer_count,
            self.authority_count,
            self.additional_count,
        ];
        for (pair, w) in out.as_chunks_mut::<2>().0.iter_mut().zip(words) {
            *pair = w.to_be_bytes();
        }
        Ok(())
    }

    /// The response code, the flags' low four bits.
    ///
    /// Name: provisional, minted 2026-11-15 (UTC) by lane/1087-outside-review-fixes.
    #[doc(alias = "RCODE")]
    pub fn response_code(&self) -> u8 {
        (self.flags & RESPONSE_CODE_MASK) as u8
    }
}

// =================================================================================================
// Names.
// =================================================================================================

/// A name in uncompressed wire form (length-prefixed labels, ending in the empty root label), in a
/// fixed buffer. 255 bytes is the protocol's own maximum, so this type holds every valid name.
///
/// **Equality is DNS equality**: ASCII letters compare without regard to case (RFC 4343), because a
/// server may answer `Packages.Example.Test` to a question about `packages.example.test` and mean the
/// same name. `Debug` and `Display` print the dotted form, and [`FromStr`](core::str::FromStr)
/// reads it ([`Name::from_dotted`]).
///
/// There is no `Hash`: with case-insensitive equality a hash would have to fold case first, and a
/// caller that wants a key folds the bytes itself.
#[derive(Clone, Copy)]
pub struct Name {
    bytes: [u8; MAXIMUM_NAME_BYTES],
    length_bytes: u8,
}

impl Name {
    /// The root, the name every other name ends in: one empty label.
    pub const fn root() -> Name {
        Name {
            bytes: [0; MAXIMUM_NAME_BYTES],
            length_bytes: 1,
        }
    }

    /// The wire bytes, root label included.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.length_bytes as usize]
    }

    /// Encode a dotted host name (`"packages.example.test"`, a trailing dot optional) into wire form.
    ///
    /// Each label is 1 to 63 bytes of ASCII letters, digits, `-` and `_` (see the crate's BUGS), and
    /// the whole is at most 255 bytes in wire form. `""` and `"."` are the root.
    ///
    /// This is the DNS label alphabet the crate chose, not RFC 1123 host-name syntax: `-a.test`,
    /// `a-.test` and `_acme-challenge.test` all pass. A caller that wants host-name rules checks
    /// them itself.
    ///
    /// ```
    /// use domain_name_system::{Error, Name};
    ///
    /// let name = Name::from_dotted("example.test.").unwrap();
    /// assert_eq!(name.as_bytes(), b"\x07example\x04test\x00");
    /// assert_eq!(Name::from_dotted("example..test"), Err(Error::BadLabel));
    /// assert_eq!(Name::from_dotted("example test"), Err(Error::BadCharacter));
    /// ```
    pub fn from_dotted(text: &str) -> Result<Name> {
        Name::from_dotted_bytes(text.as_bytes())
    }

    /// [`Name::from_dotted`] over the text's bytes, by index rather than with `str::split`, so
    /// that the parse a resolver's grants rest on is one Kani can prove (`proofs`): the search
    /// behind `split` and `strip_suffix` did not finish on two free bytes in five minutes. A dot is
    /// one byte in UTF-8 and never part of another character, so the bytes split where the text
    /// does, and any byte outside the label alphabet is refused as the text's was.
    fn from_dotted_bytes(text: &[u8]) -> Result<Name> {
        let mut out = Name {
            bytes: [0; MAXIMUM_NAME_BYTES],
            length_bytes: 0,
        };
        let body = match text {
            [rest @ .., b'.'] => rest,
            _ => text,
        };
        if !body.is_empty() {
            let mut rest = body;
            loop {
                // The label up to the next dot, and what follows the dot, if there is one. Split
                // rather than counted, so no edit to the arithmetic can make the walk stand still.
                let (label, after) = match rest.iter().position(|&b| b == b'.') {
                    Some(dot) => {
                        let (label, from_dot) = rest.split_at(dot);
                        (label, Some(&from_dot[1..]))
                    }
                    None => (rest, None),
                };
                if label.is_empty() {
                    return Err(Error::BadLabel);
                }
                if label.len() > MAXIMUM_LABEL_BYTES {
                    return Err(Error::LabelTooLong);
                }
                if !label
                    .iter()
                    .all(|&b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                {
                    return Err(Error::BadCharacter);
                }
                out.push(label.len() as u8)?;
                for &b in label {
                    out.push(b)?;
                }
                match after {
                    Some(after) => rest = after,
                    None => break,
                }
            }
        }
        out.push(0)?;
        Ok(out)
    }

    /// **Is this name `zone` or a name under it**, label by label? `a.example.test` is within
    /// `example.test` and so is `example.test` itself; `evilexample.test` is not, though its text ends the
    /// same way, because the comparison only ever starts at a label boundary. Every name is within
    /// the root.
    ///
    /// A resolver grant scoped to a zone uses it: a caller holding a grant for `example.test` may
    /// ask about `example.test` and every name under it, and about nothing else.
    ///
    /// ```
    /// use domain_name_system::Name;
    ///
    /// let zone = Name::from_dotted("example.test").unwrap();
    /// assert!(Name::from_dotted("packages.EXAMPLE.test").unwrap().is_within(&zone));
    /// assert!(!Name::from_dotted("evilexample.test").unwrap().is_within(&zone));
    /// ```
    pub fn is_within(&self, zone: &Name) -> bool {
        let name = self.as_bytes();
        let zone = zone.as_bytes();
        let mut at = 0usize;
        loop {
            let rest = &name[at..];
            if rest.len() == zone.len() {
                return bytes_equal_ignoring_case(rest, zone);
            }
            if rest.len() < zone.len() {
                return false;
            }
            // `rest` starts at a label boundary and is longer than the zone, so it is not the root
            // and its first byte is a label length that stays inside it.
            at += 1 + rest[0] as usize;
        }
    }

    fn push(&mut self, b: u8) -> Result<()> {
        let slot = self
            .bytes
            .get_mut(self.length_bytes as usize)
            .ok_or(Error::NameTooLong)?;
        *slot = b;
        self.length_bytes += 1;
        Ok(())
    }
}

/// ASCII case-insensitive equality over wire bytes. Length bytes are below 64 and so are never
/// letters, which is why folding them along with the labels is harmless.
fn bytes_equal_ignoring_case(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
}

impl PartialEq for Name {
    fn eq(&self, other: &Name) -> bool {
        bytes_equal_ignoring_case(self.as_bytes(), other.as_bytes())
    }
}

impl Eq for Name {}

impl core::fmt::Debug for Name {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self, f)
    }
}

impl core::str::FromStr for Name {
    type Err = Error;

    fn from_str(text: &str) -> Result<Name> {
        Name::from_dotted(text)
    }
}

/// The dotted form, with master-file escapes: a byte that is not printable ASCII, a dot or a
/// backslash inside a label prints as a backslash and three decimal digits.
impl core::fmt::Display for Name {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut rest = self.as_bytes();
        let mut first = true;
        while let Some((&n, tail)) = rest.split_first() {
            if n == 0 {
                break;
            }
            if !first {
                f.write_str(".")?;
            }
            first = false;
            let (label, after) = tail.split_at(usize::min(n as usize, tail.len()));
            for &b in label {
                if b.is_ascii_graphic() && b != b'.' && b != b'\\' {
                    write!(f, "{}", b as char)?;
                } else {
                    write!(f, "\\{b:03}")?;
                }
            }
            rest = after;
        }
        if first {
            f.write_str(".")?;
        }
        Ok(())
    }
}

/// Decode one possibly compressed name starting at `off` in the whole message `msg`.
///
/// Returns the name and the offset just past it in the original stream (past the first pointer if
/// there was one, else past the root label), which is where the caller keeps reading.
///
/// Compression pointers must point **strictly backwards**, each target earlier than the last. That
/// is what a correct encoder always produces, and it is the termination argument: the pointer
/// budget strictly decreases and every label consumes bytes, so a pointer loop is an
/// [`Error::PointerForward`] rather than a hang. `src/proofs.rs` checks that claim with Kani.
pub fn decode_name(msg: &[u8], off: usize) -> Result<(Name, usize)> {
    let mut out = Name {
        bytes: [0; MAXIMUM_NAME_BYTES],
        length_bytes: 0,
    };
    let (len, resume) = decode_name_into(msg, off, &mut out.bytes)?;
    // The engine wrote at most the array's 255 bytes, so the length fits its `u8`.
    out.length_bytes = len as u8;
    Ok((out, resume))
}

/// The decoder's engine, over a caller-supplied output. [`decode_name`] runs it at the protocol's
/// 255-byte maximum, and the capacity decides nothing except where [`Error::NameTooLong`] fires. It
/// is separate so the Kani harness can prove the same loop against a small output, where the solver
/// is fast (the proof at the full 255 bytes measured twenty-plus minutes). It is both proved with
/// Kani and fuzzed.
pub(crate) fn decode_name_into(msg: &[u8], off: usize, out: &mut [u8]) -> Result<(usize, usize)> {
    let mut len = 0usize;
    let mut position = off;
    let mut resume: Option<usize> = None;
    // Every pointer must target an offset strictly below this.
    let mut fence = off;
    loop {
        let b = *msg.get(position).ok_or(Error::Truncated)?;
        match b & 0xc0 {
            0xc0 => {
                let b2 = *msg.get(position + 1).ok_or(Error::Truncated)?;
                let target = ((b as usize & 0x3f) << 8) | b2 as usize;
                if resume.is_none() {
                    resume = Some(position + 2);
                }
                if target >= fence {
                    return Err(Error::PointerForward);
                }
                fence = target;
                position = target;
            }
            0x00 => {
                if b == 0 {
                    if len >= out.len() {
                        return Err(Error::NameTooLong);
                    }
                    out[len] = 0;
                    return Ok((len + 1, resume.unwrap_or(position + 1)));
                }
                let l = b as usize;
                let label = msg
                    .get(position + 1..position + 1 + l)
                    .ok_or(Error::Truncated)?;
                let end = len + 1 + l;
                if end > out.len() {
                    return Err(Error::NameTooLong);
                }
                out[len] = b;
                out[len + 1..end].copy_from_slice(label);
                len = end;
                position += 1 + l;
            }
            // 0x40 and 0x80 are reserved (RFC 1035 section 4.1.4); nothing valid emits them.
            _ => return Err(Error::BadLabel),
        }
    }
}

// =================================================================================================
// Reading a message.
// =================================================================================================

/// A decoded question.
#[derive(Debug, Clone, Copy)]
pub struct Question {
    /// The name asked about.
    pub name: Name,
    /// The record type asked for.
    #[doc(alias = "QTYPE")]
    pub record_type: u16,
    /// The class asked in.
    #[doc(alias = "QCLASS")]
    pub class: u16,
}

/// A decoded resource record: its fixed fields and its raw data. Data that holds a name (a canonical name's)
/// may be compressed against the whole message (a canonical name's is; an A record's is four plain bytes),
/// so [`canonical_name_target`] takes the message too, and `data_offset` says where the data sits in it.
#[derive(Debug, Clone, Copy)]
pub struct Record<'a> {
    /// The record's owner.
    pub name: Name,
    /// The record type.
    #[doc(alias = "TYPE")]
    pub record_type: u16,
    /// The class.
    #[doc(alias = "CLASS")]
    pub class: u16,
    /// The time to live: seconds a cache may keep the record.
    ///
    /// Name: provisional, minted 2026-11-16 (UTC) by lane/dns-ttl-seconds-canonical-name.
    #[doc(alias = "TTL")]
    #[doc(alias = "time to live")]
    pub ttl_seconds: u32,
    /// The record data, raw.
    #[doc(alias = "RDATA")]
    pub data: &'a [u8],
    /// Where `data` starts in the message.
    pub data_offset: usize,
}

/// Walks one message in wire order: the header, then [`Reader::question`] `question_count` times, then
/// [`Reader::record`] for each record. The reader does not count for you. It is `Clone`, so a caller
/// can rescan a section from a saved position.
#[derive(Clone)]
pub struct Reader<'a> {
    /// The message's header, decoded on construction.
    pub header: Header,
    msg: &'a [u8],
    off: usize,
}

impl<'a> Reader<'a> {
    /// Start reading `msg`, decoding its header.
    pub fn new(msg: &'a [u8]) -> Result<Reader<'a>> {
        Ok(Reader {
            header: Header::parse(msg)?,
            msg,
            off: HEADER_BYTES,
        })
    }

    fn u16_at(&self, off: usize) -> Result<u16> {
        let b = self.msg.get(off..off + 2).ok_or(Error::Truncated)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    /// Read the next question.
    pub fn question(&mut self) -> Result<Question> {
        let (name, off) = decode_name(self.msg, self.off)?;
        let record_type = self.u16_at(off)?;
        let class = self.u16_at(off + 2)?;
        self.off = off + 4;
        Ok(Question {
            name,
            record_type,
            class,
        })
    }

    /// Read the next resource record.
    pub fn record(&mut self) -> Result<Record<'a>> {
        let (name, off) = decode_name(self.msg, self.off)?;
        let record_type = self.u16_at(off)?;
        let class = self.u16_at(off + 2)?;
        let ttl_hi = self.u16_at(off + 4)? as u32;
        let ttl_lo = self.u16_at(off + 6)? as u32;
        let rdlen = self.u16_at(off + 8)? as usize;
        let data_offset = off + 10;
        let data = self
            .msg
            .get(data_offset..data_offset + rdlen)
            .ok_or(Error::Truncated)?;
        self.off = data_offset + rdlen;
        Ok(Record {
            name,
            record_type,
            class,
            ttl_seconds: (ttl_hi << 16) | ttl_lo,
            data,
            data_offset,
        })
    }
}

/// A canonical name record's canonical name, decompressed against the whole message. The name must end
/// inside the record's own data, though its pointers may reach back anywhere earlier.
///
/// Name: provisional, minted 2026-11-16 (UTC) by lane/dns-ttl-seconds-canonical-name.
#[doc(alias = "canonical name")]
#[doc(alias = "CNAME")]
pub fn canonical_name_target(msg: &[u8], record: &Record<'_>) -> Result<Name> {
    let (name, end) = decode_name(msg, record.data_offset)?;
    if end != record.data_offset + record.data.len() {
        return Err(Error::BadRecordData);
    }
    Ok(name)
}

// =================================================================================================
// One query and its answer.
// =================================================================================================

/// One question in flight: a host name, and the transaction id its reply must carry.
///
/// The id is the caller's to choose, and over UDP it should be unguessable: it is the only thing an
/// off-path forger has to guess. This crate has no randomness of its own, deliberately, for the
/// reason `network_time_protocol::Query::with_nonce` takes its nonce as an argument.
#[derive(Debug, Clone, Copy)]
pub struct Query {
    id: u16,
    name: Name,
}

/// Why a reply was not believed. Each variant is one check in [`Query::accept`], declared in the
/// order the checks run, so a log or a test can say which check refused it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    /// Not a DNS message, or its answer section is not.
    Malformed(Error),
    /// A query rather than a response, or a response to some other opcode.
    NotAResponse,
    /// Not our transaction id: over UDP, the sixteen bits an off-path forger must guess, so the
    /// caller draws it from the entropy service.
    IdMismatch,
    /// The server truncated it. Over UDP, ask again over TCP.
    Truncated,
    /// It does not echo exactly our question: one question, our name (ASCII case aside), type A,
    /// class IN.
    QuestionMismatch,
    /// The name does not exist (NXDOMAIN).
    NoSuchName,
    /// Any other nonzero response code, carried so a caller can report it.
    ServerError(u8),
    /// The canonical name chain, followed only from our name, is longer than [`MAXIMUM_CHAIN`] links, or
    /// loops.
    ChainTooLong,
    /// The name exists and has no A record, or the answer holds addresses only for other names.
    NoIpv4Address,
}

/// A TTL as a receiver believes it: RFC 2181 section 8 says one with its most significant bit set
/// is treated as zero.
fn believable_time_to_live(time_to_live: u32) -> u32 {
    if time_to_live & 0x8000_0000 != 0 {
        0
    } else {
        time_to_live
    }
}

/// The addresses a reply gave for our name, after every check passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answer {
    ipv4_addresses: [[u8; 4]; MAXIMUM_IPV4_ADDRESSES],
    count: u8,
    ttl_seconds: u32,
    canonical: Name,
}

impl Answer {
    /// The IPv4 addresses, in the order the server gave them. Never empty.
    ///
    /// Name: provisional, minted 2026-11-15 (UTC) by lane/1087-outside-review-fixes.
    pub fn ipv4_addresses(&self) -> &[[u8; 4]] {
        &self.ipv4_addresses[..self.count as usize]
    }

    /// The smallest TTL along the chain: how long the whole answer may be believed, in seconds.
    /// A record whose TTL has its most significant bit set counts as zero, as RFC 2181 section 8
    /// directs, so an answer carrying one is good for no time.
    ///
    /// Name: provisional, minted 2026-11-16 (UTC) by lane/dns-ttl-seconds-canonical-name.
    #[doc(alias = "TTL")]
    #[doc(alias = "time to live")]
    pub fn ttl_seconds(&self) -> u32 {
        self.ttl_seconds
    }

    /// The name the addresses belong to: ours, or the end of the canonical name chain from it.
    pub fn canonical(&self) -> &Name {
        &self.canonical
    }
}

impl Query {
    /// A query for `host`'s IPv4 addresses under transaction id `id`. Fails only when `host` is not
    /// a host name [`Name::from_dotted`] accepts.
    pub fn new(id: u16, host: &str) -> Result<Query> {
        Ok(Query {
            id,
            name: Name::from_dotted(host)?,
        })
    }

    /// A query for a name already in wire form. The resolver parses a client's text once, checks it
    /// against the client's grant, and asks about exactly the name it checked, rather than parsing
    /// the text a second time and trusting the two parses to agree.
    pub fn for_name(id: u16, name: Name) -> Query {
        Query { id, name }
    }

    /// The transaction id.
    pub fn id(&self) -> u16 {
        self.id
    }

    /// The name asked about.
    pub fn name(&self) -> &Name {
        &self.name
    }

    /// Write the query message (header with RD set, one question, type A, class IN) into `out`, and
    /// return its length. At most [`HEADER_BYTES`] + [`MAXIMUM_NAME_BYTES`] + 4 = 271 bytes, so it
    /// always fits a UDP datagram.
    pub fn request(&self, out: &mut [u8]) -> Result<usize> {
        let name = self.name.as_bytes();
        let len = HEADER_BYTES + name.len() + 4;
        let out = out.get_mut(..len).ok_or(Error::Overflow)?;
        Header {
            id: self.id,
            flags: FLAG_RECURSION_DESIRED,
            question_count: 1,
            ..Header::default()
        }
        .write(out)?;
        let (q, tail) = out[HEADER_BYTES..].split_at_mut(name.len());
        q.copy_from_slice(name);
        tail[..2].copy_from_slice(&record_type::IPV4_ADDRESS.to_be_bytes());
        tail[2..].copy_from_slice(&CLASS_IN.to_be_bytes());
        Ok(len)
    }

    /// The same query framed for TCP (RFC 7766): a two-byte big-endian length, then the message.
    /// Returns the framed length.
    pub fn request_tcp(&self, out: &mut [u8]) -> Result<usize> {
        let body = out.get_mut(2..).ok_or(Error::Overflow)?;
        let n = self.request(body)?;
        out[..2].copy_from_slice(&(n as u16).to_be_bytes());
        Ok(n + 2)
    }

    /// **Judge a reply**, and return the addresses it gives for our name only if every check
    /// [`Reject`] lists passes. Total on any input: a reply cannot panic this, and that is fuzzed.
    ///
    /// # Examples
    ///
    /// A reply as a recursive server sends it, the owner a compression pointer to the question:
    ///
    /// ```
    /// use domain_name_system::{Query, Reject};
    ///
    /// let query = Query::new(0x4e1f, "packages.example.test").unwrap();
    /// let mut request = [0u8; 512];
    /// let n = query.request(&mut request).unwrap();
    /// assert_eq!(&request[..2], &[0x4e, 0x1f]); // our transaction id
    ///
    /// // The reply: our header with QR and RA set, our question back, then one A record.
    /// let mut reply = request[..n].to_vec();
    /// reply[2] |= 0x80; // QR: a response
    /// reply[3] |= 0x80; // RA
    /// reply[7] = 1; // ANCOUNT = 1
    /// reply.extend_from_slice(&[0xc0, 0x0c]); // owner: a pointer to the question's name
    /// reply.extend_from_slice(&[0, 1, 0, 1]); // type A, class IN
    /// reply.extend_from_slice(&300u32.to_be_bytes()); // TTL
    /// reply.extend_from_slice(&[0, 4, 10, 0, 2, 9]); // RDLENGTH 4, then the address
    ///
    /// let answer = query.accept(&reply).unwrap();
    /// assert_eq!(answer.ipv4_addresses(), &[[10, 0, 2, 9]]);
    /// assert_eq!(answer.ttl_seconds(), 300);
    ///
    /// // The same bytes under somebody else's transaction id are not an answer to us.
    /// reply[1] ^= 1;
    /// assert_eq!(query.accept(&reply), Err(Reject::IdMismatch));
    /// ```
    pub fn accept(&self, reply: &[u8]) -> core::result::Result<Answer, Reject> {
        let mut reader = Reader::new(reply).map_err(Reject::Malformed)?;
        let h = reader.header;
        if h.flags & FLAG_RESPONSE == 0 || h.flags & OPERATION_CODE_MASK != 0 {
            return Err(Reject::NotAResponse);
        }
        if h.id != self.id {
            return Err(Reject::IdMismatch);
        }
        if h.flags & FLAG_TRUNCATED != 0 {
            return Err(Reject::Truncated);
        }
        if h.question_count != 1 {
            // A server that could not parse the query may send back no question at all, and then
            // its response code is the only thing it said.
            return Err(match (h.question_count, h.response_code()) {
                (0, 0) | (2.., _) => Reject::QuestionMismatch,
                (_, code) => response_code_reject(code),
            });
        }
        let question = reader.question().map_err(Reject::Malformed)?;
        if question.name != self.name
            || question.record_type != record_type::IPV4_ADDRESS
            || question.class != CLASS_IN
        {
            return Err(Reject::QuestionMismatch);
        }
        if h.response_code() != 0 {
            return Err(response_code_reject(h.response_code()));
        }

        // Read every answer once before believing any of them: one malformed record and the whole
        // reply is refused, rather than half of it being used.
        let answers = reader;
        let mut scan = answers.clone();
        for _ in 0..h.answer_count {
            let record = scan.record().map_err(Reject::Malformed)?;
            if record.class != CLASS_IN {
                continue;
            }
            match record.record_type {
                record_type::IPV4_ADDRESS if record.data.len() != 4 => {
                    return Err(Reject::Malformed(Error::BadRecordData));
                }
                record_type::CANONICAL_NAME => {
                    canonical_name_target(reply, &record).map_err(Reject::Malformed)?;
                }
                _ => {}
            }
        }

        // Follow the chain from our name. Every record was read above, so these rescans cannot fail.
        let mut owner = self.name;
        let mut ttl_seconds = u32::MAX;
        // A bounded loop rather than a counter beside an endless one: `links` is how many links
        // were followed before this pass, and no pass after the last can start, so a reply that
        // chains in a circle ends here however the count is kept.
        'chain: for links in 0..=MAXIMUM_CHAIN {
            let mut scan = answers.clone();
            for _ in 0..h.answer_count {
                let Ok(record) = scan.record() else { break };
                if record.class == CLASS_IN
                    && record.record_type == record_type::CANONICAL_NAME
                    && record.name == owner
                {
                    if links == MAXIMUM_CHAIN {
                        return Err(Reject::ChainTooLong);
                    }
                    ttl_seconds = ttl_seconds.min(believable_time_to_live(record.ttl_seconds));
                    owner = canonical_name_target(reply, &record).map_err(Reject::Malformed)?;
                    continue 'chain;
                }
            }
            break;
        }

        let mut answer = Answer {
            ipv4_addresses: [[0; 4]; MAXIMUM_IPV4_ADDRESSES],
            count: 0,
            ttl_seconds,
            canonical: owner,
        };
        let mut scan = answers;
        for _ in 0..h.answer_count {
            let Ok(record) = scan.record() else { break };
            if record.class != CLASS_IN
                || record.record_type != record_type::IPV4_ADDRESS
                || record.name != owner
            {
                continue;
            }
            answer.ttl_seconds = answer
                .ttl_seconds
                .min(believable_time_to_live(record.ttl_seconds));
            if let (Some(slot), Ok(address)) = (
                answer.ipv4_addresses.get_mut(answer.count as usize),
                <[u8; 4]>::try_from(record.data),
            ) {
                *slot = address;
                answer.count += 1;
            }
        }
        if answer.count == 0 {
            return Err(Reject::NoIpv4Address);
        }
        Ok(answer)
    }
}

fn response_code_reject(code: u8) -> Reject {
    if code == RESPONSE_CODE_NAME_ERROR {
        Reject::NoSuchName
    } else {
        Reject::ServerError(code)
    }
}

// =================================================================================================
// TCP framing.
// =================================================================================================

/// Reassembles one reply that arrives over TCP in pieces: a two-byte length, then that many bytes
/// (RFC 7766 section 8). One reply per connection, which is all a stub resolver sends for; bytes past the
/// announced length are [`Error::Trailing`], not the start of a second message.
///
/// # Examples
///
/// ```
/// use domain_name_system::{Query, TcpReply};
///
/// let query = Query::new(7, "example.test").unwrap();
/// let mut out = [0u8; 514];
/// let n = query.request_tcp(&mut out).unwrap();
/// assert_eq!(u16::from_be_bytes([out[0], out[1]]) as usize, n - 2);
///
/// let mut buf = [0u8; 600];
/// let mut reply = TcpReply::new(&mut buf);
/// assert_eq!(reply.feed(&[0, 3, 1]), Ok(false)); // a length, and one byte of three
/// assert_eq!(reply.feed(&[2, 3]), Ok(true));
/// assert_eq!(reply.message(), Some(&[1u8, 2, 3][..]));
/// ```
pub struct TcpReply<'a> {
    buf: &'a mut [u8],
    have: usize,
}

impl<'a> TcpReply<'a> {
    /// Collect into `buf`, which bounds the reply: one announcing more than `buf.len() - 2` bytes is
    /// refused when its length arrives, before any of its body is read.
    ///
    /// A buffer no larger than [`MAXIMUM_UDP_MESSAGE_BYTES`] cannot hold the TCP reply that a
    /// truncated UDP reply asks for, which defeats the retry. Size it for the largest reply the
    /// caller accepts, up to [`MAXIMUM_TCP_MESSAGE_BYTES`] + 2.
    pub fn new(buf: &'a mut [u8]) -> TcpReply<'a> {
        TcpReply { buf, have: 0 }
    }

    fn wanted(&self) -> Option<usize> {
        if self.have < 2 {
            return None;
        }
        Some(2 + u16::from_be_bytes([self.buf[0], self.buf[1]]) as usize)
    }

    /// Take the next piece. `Ok(true)` once the message is whole.
    pub fn feed(&mut self, mut bytes: &[u8]) -> Result<bool> {
        while !bytes.is_empty() {
            let limit = match self.wanted() {
                Some(w) if self.have == w => return Err(Error::Trailing),
                Some(w) if w > self.buf.len() => return Err(Error::Overflow),
                Some(w) => w,
                None => 2.min(self.buf.len()),
            };
            if self.have >= limit {
                return Err(Error::Overflow);
            }
            let take = (limit - self.have).min(bytes.len());
            self.buf[self.have..self.have + take].copy_from_slice(&bytes[..take]);
            self.have += take;
            bytes = &bytes[take..];
        }
        match self.wanted() {
            Some(w) if w > self.buf.len() => Err(Error::Overflow),
            Some(w) => Ok(self.have == w),
            None => Ok(false),
        }
    }

    /// The message, without its length, once [`TcpReply::feed`] has said it is whole.
    pub fn message(&self) -> Option<&[u8]> {
        match self.wanted() {
            Some(w) if self.have == w => Some(&self.buf[2..w]),
            _ => None,
        }
    }
}

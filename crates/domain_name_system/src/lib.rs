//! **The DNS wire format a stub resolver needs** (RFC 1035; RFC 7766 for TCP), as pure
//! computation: the bytes of a query for one name's IPv4 addresses, and the checks a reply must pass
//! before any address in it is believed. No socket, timer or policy; which program sends these
//! bytes, and which names a client may ask about, are in `notes/name-resolution.md`.
//!
//! Every byte parsed here came from a server the client may not trust, so the parser is total (Kani
//! checks the name decoder, `fuzz/fuzz_targets/domain_name_system_reply.rs` the rest), and
//! [`Query::accept`] believes as little as it can: [`Reject`] lists its seven checks in order. Only
//! A records owned by the end of the CNAME chain from our name are taken, so a reply about
//! `example.com` cannot plant an address for `bank.test`; authority and additional are never read.

//! # BUGS
//!
//! - IPv4 only: `net_stack` builds `smoltcp` with `proto-ipv4` alone.
//! - No EDNS(0), so UDP replies stop at [`UDP_MESSAGE_MAX`] and a larger answer costs a TCP retry.
//! - No DNSSEC: an on-path attacker can forge the reply, id and all. No "0x20" case randomisation.
//! - At most [`MAX_ADDRESSES`] addresses are kept from one answer; the rest are dropped.
//! - Labels hold only letters, digits, `-` and `_` ([`Name::from_dotted`]): no escape syntax.
//!
//! Name: ratified 2026-10-18 (calef, a crate-naming session). Kept: the external DNS wire format,
//! named for the standard's full name, spelled out under §154 (the acronym test). His answer was
//! "`domain_name_system`". Refused `dns` (the acronym test) and `name_resolver` (it names a program
//! rather than this wire format).

#![no_std]

#[cfg(kani)]
mod proofs;

/// The port a name server listens on, over UDP and TCP alike (RFC 1035 section 4.2).
pub const PORT: u16 = 53;

/// The largest message RFC 1035 lets travel over UDP without EDNS (section 4.2.1). A reply that would be
/// larger arrives truncated, with [`FLAG_TC`] set.
pub const UDP_MESSAGE_MAX: usize = 512;

/// A name in wire form is at most 255 bytes, length bytes and the root label included (section 2.3.4).
pub const MAX_NAME_LEN: usize = 255;
/// A label is at most 63 bytes; the two top bits of a longer count are the compression-pointer tag.
pub const MAX_LABEL_LEN: usize = 63;

/// The header every DNS message starts with.
pub const HEADER_LEN: usize = 12;

/// The response bit.
pub const FLAG_QR: u16 = 0x8000;
/// The opcode field. A standard query is opcode 0.
pub const OPCODE_MASK: u16 = 0x7800;
/// Truncated: the server had more to say than the transport carried.
pub const FLAG_TC: u16 = 0x0200;
/// Recursion desired. A stub resolver always sets it, because it asks a recursive server to do the
/// walk from the root that a stub does not.
pub const FLAG_RD: u16 = 0x0100;
/// The response code field, the low four bits of the flags.
pub const RCODE_MASK: u16 = 0x000f;

/// The response code a server sends for a name that does not exist (RFC 1035's "Name Error").
pub const RCODE_NAME_ERROR: u8 = 3;

/// Record types this crate reads.
pub mod record_type {
    /// An IPv4 address.
    pub const A: u16 = 1;
    /// The owner is an alias, and the record's data is the canonical name.
    pub const CNAME: u16 = 5;
}

/// The Internet class, the only one a host-name lookup uses.
pub const CLASS_IN: u16 = 1;

/// How many CNAME links [`Query::accept`] follows before giving up. Real chains are one or two
/// links (a CDN alias, sometimes a second); eight leaves room for that and stops a reply that chains
/// a name through itself from costing more than eight scans of its own answer section.
pub const MAX_CHAIN: usize = 8;

/// How many addresses an [`Answer`] keeps (see the crate's BUGS).
pub const MAX_ADDRESSES: usize = 8;

/// What is wrong with some bytes, as a parser sees them.
///
/// Decoding errors mean a message is not valid DNS and the right response is to drop it; building
/// errors are a caller's mistake (a buffer too small, a name that cannot be one) and are reported
/// rather than panicking, because a resolver must not be crashable by its own client's input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// CNAME whose name runs past its own record.
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
    pub qdcount: u16,
    /// How many answer records follow the questions.
    pub ancount: u16,
    /// How many authority records follow the answers.
    pub nscount: u16,
    /// How many additional records follow the authority records.
    pub arcount: u16,
}

impl Header {
    /// Decode the first 12 bytes of `msg`.
    pub fn parse(msg: &[u8]) -> Result<Header> {
        let b = msg.get(..HEADER_LEN).ok_or(Error::Truncated)?;
        let f = |i: usize| u16::from_be_bytes([b[i], b[i + 1]]);
        Ok(Header {
            id: f(0),
            flags: f(2),
            qdcount: f(4),
            ancount: f(6),
            nscount: f(8),
            arcount: f(10),
        })
    }

    /// Encode into the first 12 bytes of `out`.
    pub fn write(&self, out: &mut [u8]) -> Result<()> {
        let out = out.get_mut(..HEADER_LEN).ok_or(Error::Overflow)?;
        let words = [
            self.id,
            self.flags,
            self.qdcount,
            self.ancount,
            self.nscount,
            self.arcount,
        ];
        for (pair, w) in out.as_chunks_mut::<2>().0.iter_mut().zip(words) {
            *pair = w.to_be_bytes();
        }
        Ok(())
    }

    /// The response code, the flags' low four bits.
    pub fn rcode(&self) -> u8 {
        (self.flags & RCODE_MASK) as u8
    }
}

// =================================================================================================
// Names.
// =================================================================================================

/// A name in uncompressed wire form (length-prefixed labels, ending in the empty root label), in a
/// fixed buffer. 255 bytes is the protocol's own maximum, so this type holds every valid name.
///
/// **Equality is DNS equality**: ASCII letters compare without regard to case (RFC 4343), because a
/// server may answer `Packages.Nife.Test` to a question about `packages.nife.test` and mean the
/// same name. `Debug` prints the dotted form.
#[derive(Clone, Copy)]
pub struct Name {
    bytes: [u8; MAX_NAME_LEN],
    len: u8,
}

impl Name {
    /// The root, the name every other name ends in: one empty label.
    pub const fn root() -> Name {
        Name {
            bytes: [0; MAX_NAME_LEN],
            len: 1,
        }
    }

    /// The wire bytes, root label included.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    /// Encode a dotted host name (`"packages.nife.test"`, a trailing dot optional) into wire form.
    ///
    /// Each label is 1 to 63 bytes of ASCII letters, digits, `-` and `_` (see the crate's BUGS), and
    /// the whole is at most 255 bytes in wire form. `""` and `"."` are the root.
    ///
    /// ```
    /// use domain_name_system::{Error, Name};
    ///
    /// let name = Name::from_dotted("nife.test.").unwrap();
    /// assert_eq!(name.as_bytes(), b"\x04nife\x04test\x00");
    /// assert_eq!(Name::from_dotted("nife..test"), Err(Error::BadLabel));
    /// assert_eq!(Name::from_dotted("nife test"), Err(Error::BadCharacter));
    /// ```
    pub fn from_dotted(text: &str) -> Result<Name> {
        let mut out = Name {
            bytes: [0; MAX_NAME_LEN],
            len: 0,
        };
        let body = text.strip_suffix('.').unwrap_or(text);
        if !body.is_empty() {
            for label in body.split('.') {
                if label.is_empty() {
                    return Err(Error::BadLabel);
                }
                if label.len() > MAX_LABEL_LEN {
                    return Err(Error::LabelTooLong);
                }
                if !label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                {
                    return Err(Error::BadCharacter);
                }
                out.push(label.len() as u8)?;
                for b in label.bytes() {
                    out.push(b)?;
                }
            }
        }
        out.push(0)?;
        Ok(out)
    }

    /// **Is this name `zone` or a name under it**, label by label? `a.nife.test` is within
    /// `nife.test` and so is `nife.test` itself; `evilnife.test` is not, though its text ends the
    /// same way, because the comparison only ever starts at a label boundary. Every name is within
    /// the root.
    ///
    /// Nothing applies it yet: a resolver grant scoped to a domain is an open question of milestone
    /// 384 (in a capability system the resolver is a grant; notes/name-resolution.md).
    ///
    /// ```
    /// use domain_name_system::Name;
    ///
    /// let zone = Name::from_dotted("nife.test").unwrap();
    /// assert!(Name::from_dotted("packages.NIFE.test").unwrap().is_within(&zone));
    /// assert!(!Name::from_dotted("evilnife.test").unwrap().is_within(&zone));
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
            .get_mut(self.len as usize)
            .ok_or(Error::NameTooLong)?;
        *slot = b;
        self.len += 1;
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
        bytes: [0; MAX_NAME_LEN],
        len: 0,
    };
    let (len, resume) = decode_name_into(msg, off, &mut out.bytes)?;
    // The engine wrote at most the array's 255 bytes, so the length fits its `u8`.
    out.len = len as u8;
    Ok((out, resume))
}

/// The decoder's engine, over a caller-supplied output. [`decode_name`] runs it at the protocol's
/// 255-byte maximum, and the capacity decides nothing except where [`Error::NameTooLong`] fires. It
/// is separate so the Kani harness can prove the same loop against a small output, where the solver
/// is fast (the multicast DNS crate this was lifted from measured twenty-plus minutes at 255).
///
/// Lifted unchanged from `crates/multicast_dns_protocol` (commit `0652c981`), which milestone 298
/// (retire the multicast DNS responder and its two crates) removed along with the responder. It
/// was Kani-proved there and never fuzzed; here it is both.
pub(crate) fn decode_name_into(msg: &[u8], off: usize, out: &mut [u8]) -> Result<(usize, usize)> {
    let mut len = 0usize;
    let mut pos = off;
    let mut resume: Option<usize> = None;
    // Every pointer must target an offset strictly below this.
    let mut fence = off;
    loop {
        let b = *msg.get(pos).ok_or(Error::Truncated)?;
        match b & 0xc0 {
            0xc0 => {
                let b2 = *msg.get(pos + 1).ok_or(Error::Truncated)?;
                let target = ((b as usize & 0x3f) << 8) | b2 as usize;
                if resume.is_none() {
                    resume = Some(pos + 2);
                }
                if target >= fence {
                    return Err(Error::PointerForward);
                }
                fence = target;
                pos = target;
            }
            0x00 => {
                if b == 0 {
                    if len >= out.len() {
                        return Err(Error::NameTooLong);
                    }
                    out[len] = 0;
                    return Ok((len + 1, resume.unwrap_or(pos + 1)));
                }
                let l = b as usize;
                let label = msg.get(pos + 1..pos + 1 + l).ok_or(Error::Truncated)?;
                let end = len + 1 + l;
                if end > out.len() {
                    return Err(Error::NameTooLong);
                }
                out[len] = b;
                out[len + 1..end].copy_from_slice(label);
                len = end;
                pos += 1 + l;
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
    pub qtype: u16,
    /// The class asked in.
    pub qclass: u16,
}

/// A decoded resource record: its fixed fields and its raw data. Data that holds a name (a CNAME's)
/// may be compressed against the whole message, so [`cname_target`] takes the message too, and
/// `rdata_off` says where the data sits in it.
#[derive(Debug, Clone, Copy)]
pub struct Record<'a> {
    /// The record's owner.
    pub name: Name,
    /// The record type.
    pub rtype: u16,
    /// The class.
    pub class: u16,
    /// Seconds a cache may keep it.
    pub ttl: u32,
    /// The data, raw.
    pub rdata: &'a [u8],
    /// Where `rdata` starts in the message.
    pub rdata_off: usize,
}

/// Walks one message in wire order: the header, then [`Reader::question`] `qdcount` times, then
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
            off: HEADER_LEN,
        })
    }

    fn u16_at(&self, off: usize) -> Result<u16> {
        let b = self.msg.get(off..off + 2).ok_or(Error::Truncated)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    /// Read the next question.
    pub fn question(&mut self) -> Result<Question> {
        let (name, off) = decode_name(self.msg, self.off)?;
        let qtype = self.u16_at(off)?;
        let qclass = self.u16_at(off + 2)?;
        self.off = off + 4;
        Ok(Question {
            name,
            qtype,
            qclass,
        })
    }

    /// Read the next resource record.
    pub fn record(&mut self) -> Result<Record<'a>> {
        let (name, off) = decode_name(self.msg, self.off)?;
        let rtype = self.u16_at(off)?;
        let class = self.u16_at(off + 2)?;
        let ttl_hi = self.u16_at(off + 4)? as u32;
        let ttl_lo = self.u16_at(off + 6)? as u32;
        let rdlen = self.u16_at(off + 8)? as usize;
        let rdata_off = off + 10;
        let rdata = self
            .msg
            .get(rdata_off..rdata_off + rdlen)
            .ok_or(Error::Truncated)?;
        self.off = rdata_off + rdlen;
        Ok(Record {
            name,
            rtype,
            class,
            ttl: (ttl_hi << 16) | ttl_lo,
            rdata,
            rdata_off,
        })
    }
}

/// A CNAME record's canonical name, decompressed against the whole message. The name must end
/// inside the record's own data, though its pointers may reach back anywhere earlier.
pub fn cname_target(msg: &[u8], record: &Record<'_>) -> Result<Name> {
    let (name, end) = decode_name(msg, record.rdata_off)?;
    if end != record.rdata_off + record.rdata.len() {
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
    /// The CNAME chain, followed only from our name, is longer than [`MAX_CHAIN`] links, or loops.
    ChainTooLong,
    /// The name exists and has no A record, or the answer holds addresses only for other names.
    NoAddress,
}

/// The addresses a reply gave for our name, after every check passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answer {
    addresses: [[u8; 4]; MAX_ADDRESSES],
    count: u8,
    ttl: u32,
    canonical: Name,
}

impl Answer {
    /// The IPv4 addresses, in the order the server gave them. Never empty.
    pub fn addresses(&self) -> &[[u8; 4]] {
        &self.addresses[..self.count as usize]
    }

    /// The smallest TTL along the chain: how long the whole answer may be believed.
    pub fn ttl(&self) -> u32 {
        self.ttl
    }

    /// The name the addresses belong to: ours, or the end of the CNAME chain from it.
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
    /// return its length. At most 17 + 255 bytes, so it always fits a UDP datagram.
    pub fn request(&self, out: &mut [u8]) -> Result<usize> {
        let name = self.name.as_bytes();
        let len = HEADER_LEN + name.len() + 4;
        let out = out.get_mut(..len).ok_or(Error::Overflow)?;
        Header {
            id: self.id,
            flags: FLAG_RD,
            qdcount: 1,
            ..Header::default()
        }
        .write(out)?;
        let (q, tail) = out[HEADER_LEN..].split_at_mut(name.len());
        q.copy_from_slice(name);
        tail[..2].copy_from_slice(&record_type::A.to_be_bytes());
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
    /// let query = Query::new(0x4e1f, "packages.nife.test").unwrap();
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
    /// assert_eq!(answer.addresses(), &[[10, 0, 2, 9]]);
    /// assert_eq!(answer.ttl(), 300);
    ///
    /// // The same bytes under somebody else's transaction id are not an answer to us.
    /// reply[1] ^= 1;
    /// assert_eq!(query.accept(&reply), Err(Reject::IdMismatch));
    /// ```
    pub fn accept(&self, reply: &[u8]) -> core::result::Result<Answer, Reject> {
        let mut reader = Reader::new(reply).map_err(Reject::Malformed)?;
        let h = reader.header;
        if h.flags & FLAG_QR == 0 || h.flags & OPCODE_MASK != 0 {
            return Err(Reject::NotAResponse);
        }
        if h.id != self.id {
            return Err(Reject::IdMismatch);
        }
        if h.flags & FLAG_TC != 0 {
            return Err(Reject::Truncated);
        }
        if h.qdcount != 1 {
            // A server that could not parse the query may send back no question at all, and then
            // its response code is the only thing it said.
            return Err(match (h.qdcount, h.rcode()) {
                (0, 0) | (2.., _) => Reject::QuestionMismatch,
                (_, code) => rcode_reject(code),
            });
        }
        let question = reader.question().map_err(Reject::Malformed)?;
        if question.name != self.name
            || question.qtype != record_type::A
            || question.qclass != CLASS_IN
        {
            return Err(Reject::QuestionMismatch);
        }
        if h.rcode() != 0 {
            return Err(rcode_reject(h.rcode()));
        }

        // Read every answer once before believing any of them: one malformed record and the whole
        // reply is refused, rather than half of it being used.
        let answers = reader;
        let mut scan = answers.clone();
        for _ in 0..h.ancount {
            let record = scan.record().map_err(Reject::Malformed)?;
            if record.class != CLASS_IN {
                continue;
            }
            match record.rtype {
                record_type::A if record.rdata.len() != 4 => {
                    return Err(Reject::Malformed(Error::BadRecordData));
                }
                record_type::CNAME => {
                    cname_target(reply, &record).map_err(Reject::Malformed)?;
                }
                _ => {}
            }
        }

        // Follow the chain from our name. Every record was read above, so these rescans cannot fail.
        let mut owner = self.name;
        let mut ttl = u32::MAX;
        let mut links = 0usize;
        'chain: loop {
            let mut scan = answers.clone();
            for _ in 0..h.ancount {
                let Ok(record) = scan.record() else { break };
                if record.class == CLASS_IN
                    && record.rtype == record_type::CNAME
                    && record.name == owner
                {
                    if links == MAX_CHAIN {
                        return Err(Reject::ChainTooLong);
                    }
                    links += 1;
                    ttl = ttl.min(record.ttl);
                    owner = cname_target(reply, &record).map_err(Reject::Malformed)?;
                    continue 'chain;
                }
            }
            break;
        }

        let mut answer = Answer {
            addresses: [[0; 4]; MAX_ADDRESSES],
            count: 0,
            ttl,
            canonical: owner,
        };
        let mut scan = answers;
        for _ in 0..h.ancount {
            let Ok(record) = scan.record() else { break };
            if record.class != CLASS_IN || record.rtype != record_type::A || record.name != owner {
                continue;
            }
            answer.ttl = answer.ttl.min(record.ttl);
            if let (Some(slot), Ok(address)) = (
                answer.addresses.get_mut(answer.count as usize),
                <[u8; 4]>::try_from(record.rdata),
            ) {
                *slot = address;
                answer.count += 1;
            }
        }
        if answer.count == 0 {
            return Err(Reject::NoAddress);
        }
        Ok(answer)
    }
}

fn rcode_reject(code: u8) -> Reject {
    if code == RCODE_NAME_ERROR {
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
/// let query = Query::new(7, "nife.test").unwrap();
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

//! `Query::accept` against real replies and against the replies a forger or a broken server sends.
//!
//! The three `REAL_*` fixtures (`tests/fixtures/*.dns`, one raw reply each, which `script/fuzz` also
//! hands the `domain_name_system_reply` target as seeds) were captured on 2026-10-04 (UTC) from
//! 1.1.1.1 with a twenty-line Python stub sending exactly the query `Query::request` writes, so they carry what a recursive
//! server really does: compression pointers into the question, a CNAME pointing into its own owner,
//! TTLs that differ along a chain. The rest are built by hand below, one per check, because nobody
//! captures a forgery.

use domain_name_system::{
    CLASS_IN, Error, FLAG_TC, Header, MAXIMUM_ADDRESSES, MAXIMUM_CHAIN, Name, Query, Reader,
    Reject, TcpReply, cname_target, decode_name, record_type,
};

/// `example.com` A, id 0x1111: two A records, owners compressed to the question.
const REAL_EXAMPLE: &[u8] = include_bytes!("fixtures/example.com.dns");
/// `www.github.com` A, id 0x2222: a CNAME to `github.com` (a pointer into the question's own name)
/// with TTL 1952, then github.com's A with TTL 33.
const REAL_GITHUB: &[u8] = include_bytes!("fixtures/www.github.com.dns");
/// `nosuch.invalid` A, id 0x3333: NXDOMAIN, no records.
const REAL_NXDOMAIN: &[u8] = include_bytes!("fixtures/nosuch.invalid.dns");

#[test]
fn a_real_reply_with_two_addresses_gives_both_in_order() {
    let query = Query::new(0x1111, "example.com").unwrap();
    let answer = query.accept(REAL_EXAMPLE).unwrap();
    assert_eq!(
        answer.addresses(),
        &[[104, 20, 23, 154], [172, 66, 147, 243]]
    );
    assert_eq!(answer.ttl(), 0x31);
    assert_eq!(
        answer.canonical(),
        &Name::from_dotted("example.com").unwrap()
    );
}

#[test]
fn a_real_cname_is_followed_and_the_shortest_ttl_wins() {
    let query = Query::new(0x2222, "www.github.com").unwrap();
    let answer = query.accept(REAL_GITHUB).unwrap();
    assert_eq!(answer.addresses(), &[[140, 82, 116, 3]]);
    assert_eq!(
        answer.canonical(),
        &Name::from_dotted("github.com").unwrap()
    );
    assert_eq!(answer.ttl(), 33, "the A's 33 seconds, not the CNAME's 1952");
}

#[test]
fn a_real_name_error_is_no_such_name() {
    let query = Query::new(0x3333, "nosuch.invalid").unwrap();
    assert_eq!(query.accept(REAL_NXDOMAIN), Err(Reject::NoSuchName));
}

#[test]
fn the_request_is_byte_for_byte_what_a_real_server_echoed() {
    // The server echoes header id and question verbatim, so our request must equal the reply's first
    // bytes with the reply's flags and counts put back to a query's.
    let reply = REAL_GITHUB;
    let query = Query::new(0x2222, "www.github.com").unwrap();
    let mut out = [0u8; 512];
    let n = query.request(&mut out).unwrap();
    let mut expected = reply[..n].to_vec();
    expected[2..4].copy_from_slice(&[0x01, 0x00]); // RD only
    expected[6..8].copy_from_slice(&[0, 0]); // no answers
    assert_eq!(&out[..n], &expected[..]);
}

// =================================================================================================
// A hand builder for the replies nobody captures.
// =================================================================================================

const OUR_ID: u16 = 0xbeef;
const OURS: &str = "packages.nife.test";

fn wire(name: &str) -> Vec<u8> {
    Name::from_dotted(name).unwrap().as_bytes().to_vec()
}

struct Reply {
    bytes: Vec<u8>,
}

impl Reply {
    /// A successful response to our query, no records yet.
    fn to(name: &str) -> Reply {
        let mut bytes = vec![0u8; 12];
        Header {
            id: OUR_ID,
            flags: 0x8180,
            qdcount: 1,
            ..Header::default()
        }
        .write(&mut bytes)
        .unwrap();
        bytes.extend(wire(name));
        bytes.extend(record_type::A.to_be_bytes());
        bytes.extend(CLASS_IN.to_be_bytes());
        Reply { bytes }
    }

    fn header(&self) -> Header {
        Header::parse(&self.bytes).unwrap()
    }

    fn set_header(mut self, f: impl FnOnce(&mut Header)) -> Reply {
        let mut h = self.header();
        f(&mut h);
        h.write(&mut self.bytes).unwrap();
        self
    }

    fn record(mut self, owner: &[u8], rtype: u16, class: u16, ttl: u32, rdata: &[u8]) -> Reply {
        self.bytes.extend(owner);
        self.bytes.extend(rtype.to_be_bytes());
        self.bytes.extend(class.to_be_bytes());
        self.bytes.extend(ttl.to_be_bytes());
        self.bytes.extend((rdata.len() as u16).to_be_bytes());
        self.bytes.extend(rdata);
        self.set_header(|h| h.ancount += 1)
    }

    fn a(self, owner: &str, address: [u8; 4]) -> Reply {
        self.record(&wire(owner), record_type::A, CLASS_IN, 60, &address)
    }

    fn cname(self, owner: &str, target: &str) -> Reply {
        self.record(
            &wire(owner),
            record_type::CNAME,
            CLASS_IN,
            60,
            &wire(target),
        )
    }

    fn judged(&self) -> Result<Vec<[u8; 4]>, Reject> {
        Query::new(OUR_ID, OURS)
            .unwrap()
            .accept(&self.bytes)
            .map(|a| a.addresses().to_vec())
    }
}

#[test]
fn the_hand_builder_agrees_with_the_real_replies() {
    assert_eq!(
        Reply::to(OURS).a(OURS, [10, 0, 2, 9]).judged(),
        Ok(vec![[10, 0, 2, 9]])
    );
}

#[test]
fn another_transaction_id_is_not_our_answer() {
    let r = Reply::to(OURS)
        .a(OURS, [10, 0, 2, 9])
        .set_header(|h| h.id ^= 0x0100);
    assert_eq!(r.judged(), Err(Reject::IdMismatch));
}

#[test]
fn a_query_or_another_opcode_is_not_a_response() {
    let query = Reply::to(OURS)
        .a(OURS, [10, 0, 2, 9])
        .set_header(|h| h.flags &= !0x8000);
    assert_eq!(query.judged(), Err(Reject::NotAResponse));
    let notify = Reply::to(OURS)
        .a(OURS, [10, 0, 2, 9])
        .set_header(|h| h.flags |= 4 << 11);
    assert_eq!(notify.judged(), Err(Reject::NotAResponse));
}

#[test]
fn a_truncated_reply_says_so_rather_than_answering_from_half() {
    let r = Reply::to(OURS)
        .a(OURS, [10, 0, 2, 9])
        .set_header(|h| h.flags |= FLAG_TC);
    assert_eq!(r.judged(), Err(Reject::Truncated));
}

#[test]
fn a_reply_to_some_other_question_is_refused() {
    let other_name = Reply::to("mirror.nife.test").a("mirror.nife.test", [10, 0, 2, 9]);
    assert_eq!(other_name.judged(), Err(Reject::QuestionMismatch));

    let mut other_type = Reply::to(OURS).a(OURS, [10, 0, 2, 9]);
    let qtype_at = 12 + wire(OURS).len();
    other_type.bytes[qtype_at + 1] = 28; // AAAA
    assert_eq!(other_type.judged(), Err(Reject::QuestionMismatch));

    let mut other_class = Reply::to(OURS).a(OURS, [10, 0, 2, 9]);
    other_class.bytes[qtype_at + 3] = 3; // CHAOS
    assert_eq!(other_class.judged(), Err(Reject::QuestionMismatch));

    let two_questions = Reply::to(OURS).set_header(|h| h.qdcount = 2);
    assert_eq!(two_questions.judged(), Err(Reject::QuestionMismatch));

    let no_question = Reply::to(OURS).set_header(|h| h.qdcount = 0);
    assert_eq!(no_question.judged(), Err(Reject::QuestionMismatch));
}

#[test]
fn a_question_echoed_in_another_case_is_still_ours() {
    let r = Reply::to("Packages.NIFE.test").a("PACKAGES.nife.TEST", [10, 0, 2, 9]);
    assert_eq!(r.judged(), Ok(vec![[10, 0, 2, 9]]));
}

#[test]
fn response_codes_are_named_and_carried() {
    let servfail = Reply::to(OURS).set_header(|h| h.flags |= 2);
    assert_eq!(servfail.judged(), Err(Reject::ServerError(2)));
    let refused = Reply::to(OURS).set_header(|h| h.flags |= 5);
    assert_eq!(refused.judged(), Err(Reject::ServerError(5)));
    let nxdomain = Reply::to(OURS).set_header(|h| h.flags |= 3);
    assert_eq!(nxdomain.judged(), Err(Reject::NoSuchName));
    // A server that could not read the query at all may send back no question, only a code.
    let formerr = Reply::to(OURS).set_header(|h| {
        h.qdcount = 0;
        h.flags |= 1;
    });
    assert_eq!(formerr.judged(), Err(Reject::ServerError(1)));
    let header = formerr.header();
    assert_eq!(header.rcode(), 1);
}

#[test]
fn an_address_for_another_name_is_never_taken() {
    // The cache-poisoning shape: a reply about our name that also "helpfully" carries an address
    // for somebody else's. Only ours comes out, and a reply carrying only theirs gives nothing.
    let mixed = Reply::to(OURS)
        .a("bank.test", [6, 6, 6, 6])
        .a(OURS, [10, 0, 2, 9]);
    assert_eq!(mixed.judged(), Ok(vec![[10, 0, 2, 9]]));
    let theirs_only = Reply::to(OURS).a("bank.test", [6, 6, 6, 6]);
    assert_eq!(theirs_only.judged(), Err(Reject::NoAddress));
}

#[test]
fn an_empty_answer_section_is_no_address() {
    assert_eq!(Reply::to(OURS).judged(), Err(Reject::NoAddress));
}

#[test]
fn a_chain_is_followed_in_any_order_and_only_from_our_name() {
    // Records out of order, as some servers send them, and a stray CNAME for a name not on the chain.
    let r = Reply::to(OURS)
        .a("origin.nife.test", [10, 0, 2, 9])
        .cname("cdn.nife.test", "origin.nife.test")
        .cname("elsewhere.test", "bank.test")
        .a("bank.test", [6, 6, 6, 6])
        .cname(OURS, "cdn.nife.test");
    let answer = Query::new(OUR_ID, OURS).unwrap().accept(&r.bytes).unwrap();
    assert_eq!(answer.addresses(), &[[10, 0, 2, 9]]);
    assert_eq!(
        answer.canonical(),
        &Name::from_dotted("origin.nife.test").unwrap()
    );
}

#[test]
fn an_address_for_our_name_beside_its_cname_is_not_taken() {
    // A name with a CNAME has no other data (RFC 1034 section 3.6.2), so the chain's end is what counts.
    let r = Reply::to(OURS)
        .a(OURS, [6, 6, 6, 6])
        .cname(OURS, "origin.nife.test")
        .a("origin.nife.test", [10, 0, 2, 9]);
    assert_eq!(r.judged(), Ok(vec![[10, 0, 2, 9]]));
}

#[test]
fn a_cname_loop_stops_at_the_chain_limit() {
    let r = Reply::to(OURS)
        .cname(OURS, "b.nife.test")
        .cname("b.nife.test", OURS);
    assert_eq!(r.judged(), Err(Reject::ChainTooLong));
}

#[test]
fn the_chain_limit_is_exactly_maximum_chain_links() {
    let names: Vec<String> = (0..=MAXIMUM_CHAIN + 1)
        .map(|i| {
            if i == 0 {
                OURS.to_string()
            } else {
                format!("hop{i}.nife.test")
            }
        })
        .collect();
    let chain = |links: usize| {
        let mut r = Reply::to(OURS);
        for i in 0..links {
            r = r.cname(&names[i], &names[i + 1]);
        }
        r.a(&names[links], [10, 0, 2, 9])
    };
    assert_eq!(chain(MAXIMUM_CHAIN).judged(), Ok(vec![[10, 0, 2, 9]]));
    assert_eq!(chain(MAXIMUM_CHAIN + 1).judged(), Err(Reject::ChainTooLong));
}

#[test]
fn records_in_another_class_are_ignored() {
    let r = Reply::to(OURS)
        .record(&wire(OURS), record_type::A, 3, 60, &[6, 6, 6, 6])
        .record(&wire(OURS), record_type::CNAME, 3, 60, &wire("bank.test"))
        .a(OURS, [10, 0, 2, 9]);
    assert_eq!(r.judged(), Ok(vec![[10, 0, 2, 9]]));
}

#[test]
fn other_record_types_are_ignored() {
    let r = Reply::to(OURS)
        .record(&wire(OURS), 16, CLASS_IN, 60, b"\x05hello")
        .a(OURS, [10, 0, 2, 9]);
    assert_eq!(r.judged(), Ok(vec![[10, 0, 2, 9]]));
}

#[test]
fn addresses_past_the_cap_are_dropped_and_the_rest_kept() {
    let mut r = Reply::to(OURS);
    for i in 0..(MAXIMUM_ADDRESSES as u8 + 2) {
        r = r.a(OURS, [10, 0, 2, i]);
    }
    let got = r.judged().unwrap();
    assert_eq!(got.len(), MAXIMUM_ADDRESSES);
    assert_eq!(
        got[MAXIMUM_ADDRESSES - 1],
        [10, 0, 2, MAXIMUM_ADDRESSES as u8 - 1]
    );
}

#[test]
fn the_ttl_is_the_smallest_on_the_chain_and_among_the_addresses() {
    let r = Reply::to(OURS)
        .record(
            &wire(OURS),
            record_type::CNAME,
            CLASS_IN,
            500,
            &wire("o.nife.test"),
        )
        .record(
            &wire("o.nife.test"),
            record_type::A,
            CLASS_IN,
            90,
            &[10, 0, 2, 9],
        )
        .record(
            &wire("o.nife.test"),
            record_type::A,
            CLASS_IN,
            70,
            &[10, 0, 2, 8],
        );
    let answer = Query::new(OUR_ID, OURS).unwrap().accept(&r.bytes).unwrap();
    assert_eq!(answer.ttl(), 70);
    let short_chain = Reply::to(OURS)
        .record(
            &wire(OURS),
            record_type::CNAME,
            CLASS_IN,
            5,
            &wire("o.nife.test"),
        )
        .record(
            &wire("o.nife.test"),
            record_type::A,
            CLASS_IN,
            90,
            &[10, 0, 2, 9],
        );
    let answer = Query::new(OUR_ID, OURS)
        .unwrap()
        .accept(&short_chain.bytes)
        .unwrap();
    assert_eq!(answer.ttl(), 5);
}

#[test]
fn one_malformed_answer_refuses_the_whole_reply() {
    let long_a = Reply::to(OURS).a(OURS, [10, 0, 2, 9]).record(
        &wire(OURS),
        record_type::A,
        CLASS_IN,
        60,
        &[1, 2, 3, 4, 5],
    );
    assert_eq!(
        long_a.judged(),
        Err(Reject::Malformed(Error::BadRecordData))
    );

    // A CNAME whose name runs past its own record's data.
    let mut spill = wire("o.nife.test");
    spill.truncate(3);
    let spilling = Reply::to(OURS).a(OURS, [10, 0, 2, 9]).record(
        &wire("x.test"),
        record_type::CNAME,
        CLASS_IN,
        60,
        &spill,
    );
    assert!(matches!(spilling.judged(), Err(Reject::Malformed(_))));

    // An answer count that promises one more record than the bytes hold.
    let short = Reply::to(OURS)
        .a(OURS, [10, 0, 2, 9])
        .set_header(|h| h.ancount += 1);
    assert_eq!(short.judged(), Err(Reject::Malformed(Error::Truncated)));
}

#[test]
fn a_pointer_loop_in_an_answer_is_malformed_not_a_hang() {
    // The owner is a pointer to itself.
    let at = Reply::to(OURS).bytes.len() as u16;
    let ptr = (0xc000 | at).to_be_bytes();
    let r = Reply::to(OURS).record(&ptr, record_type::A, CLASS_IN, 60, &[10, 0, 2, 9]);
    assert_eq!(r.judged(), Err(Reject::Malformed(Error::PointerForward)));
}

#[test]
fn a_reply_shorter_than_a_header_is_malformed() {
    let q = Query::new(OUR_ID, OURS).unwrap();
    assert_eq!(q.accept(&[0xbe]), Err(Reject::Malformed(Error::Truncated)));
    let mut cut = Reply::to(OURS).bytes;
    cut.truncate(14);
    assert_eq!(q.accept(&cut), Err(Reject::Malformed(Error::Truncated)));
}

// =================================================================================================
// Names.
// =================================================================================================

#[test]
fn dotted_names_encode_and_refuse() {
    assert_eq!(Name::from_dotted("").unwrap().as_bytes(), b"\0");
    assert_eq!(Name::from_dotted(".").unwrap(), Name::root());
    assert_eq!(
        Name::from_dotted("a-b_c.d9").unwrap().as_bytes(),
        b"\x05a-b_c\x02d9\0"
    );
    let label63 = "a".repeat(63);
    assert!(Name::from_dotted(&label63).is_ok());
    assert_eq!(Name::from_dotted(&"a".repeat(64)), Err(Error::LabelTooLong));
    assert_eq!(Name::from_dotted(".nife"), Err(Error::BadLabel));
    assert_eq!(Name::from_dotted("nife.."), Err(Error::BadLabel));
    assert_eq!(Name::from_dotted("ni/fe"), Err(Error::BadCharacter));
    assert_eq!(Name::from_dotted("nifé"), Err(Error::BadCharacter));

    // 255 bytes in wire form is the most there is: four 63-byte labels and the root is 257.
    let fits = format!("{l}.{l}.{l}.{}", "a".repeat(61), l = label63);
    assert_eq!(Name::from_dotted(&fits).unwrap().as_bytes().len(), 255);
    let over = format!("{l}.{l}.{l}.{}", "a".repeat(62), l = label63);
    assert_eq!(Name::from_dotted(&over), Err(Error::NameTooLong));
}

#[test]
fn a_name_prints_dotted_and_escapes_what_is_not_text() {
    assert_eq!(
        format!("{:?}", Name::from_dotted("nife.test").unwrap()),
        "nife.test"
    );
    assert_eq!(format!("{:?}", Name::root()), ".");
    let (odd, _) = decode_name(b"\x03a.b\x00", 0).unwrap();
    assert_eq!(format!("{odd:?}"), "a\\046b");
}

#[test]
fn within_is_label_by_label() {
    let zone = Name::from_dotted("nife.test").unwrap();
    let n = |s| Name::from_dotted(s).unwrap();
    assert!(n("nife.test").is_within(&zone));
    assert!(n("a.b.NIFE.TEST").is_within(&zone));
    assert!(!n("evilnife.test").is_within(&zone));
    assert!(!n("test").is_within(&zone));
    assert!(!n("nife.test.evil").is_within(&zone));
    assert!(!n("x.nife.text").is_within(&zone));
    assert!(n("anything.at.all").is_within(&Name::root()));
    assert!(!Name::root().is_within(&zone));
}

#[test]
fn the_name_decoder_resumes_after_the_first_pointer() {
    // "nife" at 0, then "a" + pointer to 0 at 6.
    let msg = b"\x04nife\x00\x01a\xc0\x00";
    let (name, resume) = decode_name(msg, 6).unwrap();
    assert_eq!(name, Name::from_dotted("a.nife").unwrap());
    assert_eq!(resume, msg.len());
    let (_, resume) = decode_name(msg, 0).unwrap();
    assert_eq!(resume, 6);
}

#[test]
fn the_name_decoder_refuses_what_is_not_a_name() {
    assert_eq!(
        decode_name(b"\xc0\x00", 0).err(),
        Some(Error::PointerForward)
    );
    assert_eq!(
        decode_name(b"\x00\xc0\x02", 1).err(),
        Some(Error::PointerForward)
    );
    assert_eq!(decode_name(b"\x40", 0).err(), Some(Error::BadLabel));
    assert_eq!(decode_name(b"\x80", 0).err(), Some(Error::BadLabel));
    assert_eq!(decode_name(b"\x05ab", 0).err(), Some(Error::Truncated));
    assert_eq!(decode_name(b"\xc0", 0).err(), Some(Error::Truncated));
    assert_eq!(decode_name(b"", 0).err(), Some(Error::Truncated));
}

#[test]
fn a_name_longer_than_255_by_pointers_is_refused() {
    // Each 63-byte label points back at the one before; five of them decode past 255 bytes.
    let mut msg = vec![0u8];
    let mut prev = 0u16;
    for _ in 0..5 {
        let here = msg.len() as u16;
        msg.push(63);
        msg.extend([b'a'; 63]);
        msg.extend((0xc000 | prev).to_be_bytes());
        prev = here;
    }
    assert_eq!(
        decode_name(&msg, prev as usize).err(),
        Some(Error::NameTooLong)
    );
}

#[test]
fn a_cname_target_reads_through_the_message() {
    let reply = REAL_GITHUB;
    let mut reader = Reader::new(reply).unwrap();
    reader.question().unwrap();
    let record = reader.record().unwrap();
    assert_eq!(record.rtype, record_type::CNAME);
    assert_eq!(record.ttl, 1952);
    assert_eq!(
        cname_target(reply, &record).unwrap(),
        Name::from_dotted("github.com").unwrap()
    );
}

// =================================================================================================
// Building, and TCP.
// =================================================================================================

#[test]
fn a_buffer_too_small_is_overflow_not_a_panic() {
    let q = Query::new(1, "nife.test").unwrap();
    let mut small = [0u8; 20];
    assert_eq!(q.request(&mut small), Err(Error::Overflow));
    assert_eq!(q.request_tcp(&mut small[..1]), Err(Error::Overflow));
    assert_eq!(q.request_tcp(&mut small), Err(Error::Overflow));
    assert_eq!(
        Header::default().write(&mut small[..11]),
        Err(Error::Overflow)
    );
    let mut exact = [0u8; 12 + 11 + 4];
    assert_eq!(q.request(&mut exact), Ok(27));
    assert_eq!(q.id(), 1);
    assert_eq!(q.name(), &Name::from_dotted("nife.test").unwrap());
}

#[test]
fn the_tcp_request_is_the_udp_request_with_its_length_in_front() {
    let q = Query::new(0x0102, "nife.test").unwrap();
    let mut udp = [0u8; 512];
    let n = q.request(&mut udp).unwrap();
    let mut tcp = [0u8; 514];
    let m = q.request_tcp(&mut tcp).unwrap();
    assert_eq!(m, n + 2);
    assert_eq!(&tcp[..2], &(n as u16).to_be_bytes());
    assert_eq!(&tcp[2..m], &udp[..n]);
}

#[test]
fn a_tcp_reply_reassembles_a_byte_at_a_time() {
    let real = REAL_EXAMPLE.to_vec();
    let mut framed = (real.len() as u16).to_be_bytes().to_vec();
    framed.extend(&real);
    let mut buf = [0u8; 600];
    let mut reply = TcpReply::new(&mut buf);
    for (i, b) in framed.iter().enumerate() {
        assert_eq!(reply.message(), None);
        assert_eq!(reply.feed(&[*b]), Ok(i == framed.len() - 1));
    }
    assert_eq!(reply.message(), Some(&real[..]));
    assert_eq!(reply.feed(&[]), Ok(true));
    assert_eq!(reply.feed(&[0]), Err(Error::Trailing));
}

#[test]
fn a_tcp_reply_refuses_more_than_it_holds_and_more_than_it_announced() {
    let mut buf = [0u8; 8];
    let mut reply = TcpReply::new(&mut buf);
    assert_eq!(reply.feed(&[0, 7]), Err(Error::Overflow));

    let mut buf = [0u8; 8];
    let mut reply = TcpReply::new(&mut buf);
    assert_eq!(reply.feed(&[0, 2, 9, 9, 9]), Err(Error::Trailing));

    let mut buf = [0u8; 8];
    let mut reply = TcpReply::new(&mut buf);
    assert_eq!(reply.feed(&[0, 0]), Ok(true));
    assert_eq!(reply.message(), Some(&[][..]));

    let mut tiny = [0u8; 1];
    let mut reply = TcpReply::new(&mut tiny);
    assert_eq!(reply.feed(&[0]), Ok(false));
    assert_eq!(reply.feed(&[0]), Err(Error::Overflow));

    let mut buf = [0u8; 8];
    let mut reply = TcpReply::new(&mut buf);
    assert_eq!(reply.feed(&[0, 6, 1, 2, 3, 4, 5, 6]), Ok(true));
}

/// A length prefix announcing more than the buffer holds is refused even when the body arrives in
/// the same piece as the prefix, before any of it is copied.
#[test]
fn an_oversized_tcp_reply_is_refused_when_its_body_comes_with_its_length() {
    let mut buf = [0u8; 8];
    let mut reply = TcpReply::new(&mut buf);
    let mut piece = vec![0, 16];
    piece.extend(1..=10);
    assert_eq!(reply.feed(&piece), Err(Error::Overflow));
}

/// A record's TTL is two 16-bit halves on the wire, high half first; one past 65,535 seconds
/// needs both.
#[test]
fn a_ttl_wider_than_sixteen_bits_reads_whole() {
    let bytes = Reply::to("nife.test")
        .record(
            &wire("nife.test"),
            record_type::A,
            CLASS_IN,
            0x0001_0002,
            &[10, 0, 0, 1],
        )
        .bytes;
    let mut reader = Reader::new(&bytes).unwrap();
    reader.question().unwrap();
    assert_eq!(reader.record().unwrap().ttl, 0x0001_0002);
}

/// A compression pointer is fourteen bits, so a target past offset 255 uses the high six.
#[test]
fn a_pointer_past_offset_255_reaches_its_target() {
    let mut msg = vec![0u8; 300];
    msg.extend(wire("nife.test"));
    let at = msg.len();
    msg.extend((0xc000u16 | 300).to_be_bytes());
    assert_eq!(
        decode_name(&msg, at).unwrap().0,
        Name::from_dotted("nife.test").unwrap()
    );
}

/// Labels that fill the 255 bytes exactly are read in full; the name fails at its missing end,
/// as truncated, and only a label past the limit is too long.
#[test]
fn labels_that_fill_the_limit_exactly_are_not_too_long() {
    let mut msg = Vec::new();
    for len in [63u8, 63, 63, 62] {
        msg.push(len);
        msg.extend(vec![b'a'; len as usize]);
    }
    assert_eq!(decode_name(&msg, 0).err(), Some(Error::Truncated));
}

#[test]
fn a_query_keeps_its_transaction_id() {
    assert_eq!(Query::new(0x1234, "nife.test").unwrap().id(), 0x1234);
}

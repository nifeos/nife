//! Fuzz a stub resolver's reply checks with arbitrary bytes as the reply.
//!
//! A forged reply that panics `crates/domain_name_system` takes name resolution down, and one that
//! slips an address past `Query::accept` sends a client to the wrong host (milestone 384 (in a
//! capability system the resolver is a grant)). Kani proves the name decoder and the TCP reassembly
//! on small inputs; `accept` is too large for the solver. The input is the raw reply, so the
//! captures in `crates/domain_name_system/tests/fixtures` seed it unmodified, and the query is
//! built from the reply's own id and question so a mutation reaches the answer section.
//! notes/fuzzing/targets.md has more.

//! # What is asserted, beyond "it returned"
//!
//! 1. **An accepted address is really there.** Each address `accept` returns is a class-IN A
//!    record's data in the answer section, owned by the canonical name `accept` reports, which is
//!    ours or reached from ours by a CNAME there; a second, independent walk checks it. Falsified on
//!    2026-10-04: with `accept`'s owner check deleted, it fired within two minutes of the captures.
//! 2. **TCP framing does not change the bytes.** Framed and fed in pieces of one, three and all
//!    bytes, the reply comes back whole, and a buffer one byte too small refuses it.
//! 3. **The name decoder is total at every offset**, with its resume offset inside the message.

#![no_main]

use domain_name_system::{
    CLASS_IN, Header, MAX_ADDRESSES, Name, Query, Reader, TcpReply, cname_target, decode_name,
    record_type,
};
use libfuzzer_sys::fuzz_target;

/// The dotted text of a decoded name, if every label is one `Name::from_dotted` accepts.
fn dotted(name: &Name) -> Option<String> {
    let mut out = String::new();
    let mut rest = name.as_bytes();
    while let Some((&n, tail)) = rest.split_first() {
        if n == 0 {
            break;
        }
        let label = tail.get(..n as usize)?;
        if !out.is_empty() {
            out.push('.');
        }
        out.push_str(core::str::from_utf8(label).ok()?);
        rest = &tail[n as usize..];
    }
    Some(out)
}

/// Every class-IN record of `rtype` in the answer section, as (owner, data offset, data).
fn answers(reply: &[u8], rtype: u16) -> Vec<(Name, usize, Vec<u8>)> {
    let mut found = Vec::new();
    let Ok(mut reader) = Reader::new(reply) else {
        return found;
    };
    let header = reader.header;
    for _ in 0..header.qdcount {
        if reader.question().is_err() {
            return found;
        }
    }
    for _ in 0..header.ancount {
        let Ok(r) = reader.record() else {
            return found;
        };
        if r.class == CLASS_IN && r.rtype == rtype {
            found.push((r.name, r.rdata_off, r.rdata.to_vec()));
        }
    }
    found
}

fuzz_target!(|reply: &[u8]| {
    // Property 3, first, because everything else rests on it.
    for off in 0..=reply.len() {
        if let Ok((_, resume)) = decode_name(reply, off) {
            assert!(resume > off && resume <= reply.len());
        }
    }

    // The query this reply would answer if it were honest.
    let id = Header::parse(reply).map(|h| h.id).unwrap_or(0);
    let asked = Reader::new(reply)
        .and_then(|mut r| r.question())
        .ok()
        .and_then(|q| dotted(&q.name))
        .and_then(|text| Query::new(id, &text).ok())
        .unwrap_or_else(|| Query::new(id, "packages.nife.test").unwrap());

    if let Ok(answer) = asked.accept(reply) {
        // Property 1.
        assert!(!answer.addresses().is_empty() && answer.addresses().len() <= MAX_ADDRESSES);
        let canonical = *answer.canonical();
        let a_records = answers(reply, record_type::A);
        for address in answer.addresses() {
            assert!(
                a_records
                    .iter()
                    .any(|(owner, _, data)| *owner == canonical && data[..] == address[..]),
                "an address that no A record for the canonical name carries"
            );
        }
        if canonical != *asked.name() {
            let reached = answers(reply, record_type::CNAME)
                .iter()
                .any(|(owner, at, data)| {
                    *owner == *asked.name()
                        && cname_target(
                            reply,
                            &domain_name_system::Record {
                                name: *owner,
                                rtype: record_type::CNAME,
                                class: CLASS_IN,
                                ttl: 0,
                                rdata: data,
                                rdata_off: *at,
                            },
                        )
                        .is_ok()
                });
            assert!(reached, "a canonical name our name has no CNAME to");
        }
        assert!(canonical.is_within(&Name::root()));
    }

    // Property 2. A reply longer than a TCP length can say is not a TCP reply at all.
    if reply.len() <= usize::from(u16::MAX) {
        let mut framed = (reply.len() as u16).to_be_bytes().to_vec();
        framed.extend_from_slice(reply);
        for step in [1, 3, framed.len()] {
            let mut buf = vec![0u8; framed.len()];
            let mut tcp = TcpReply::new(&mut buf);
            let mut whole = false;
            for piece in framed.chunks(step) {
                whole = tcp.feed(piece).expect("a well-framed reply was refused");
            }
            assert!(whole);
            assert_eq!(tcp.message(), Some(reply));
        }
        let mut short = vec![0u8; framed.len() - 1];
        let mut tcp = TcpReply::new(&mut short);
        assert!(framed.chunks(3).any(|piece| tcp.feed(piece).is_err()));
    }
});

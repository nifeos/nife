//! Fuzz the decoders for a program's manifest notes with arbitrary descriptor bytes.
//!
//! `crates/capability_manifest_note` reads the descriptor of an ELF note that the builder of a
//! foreign program wrote ("a file anyone could have written", in the crate's words). The
//! progenitor decodes it to decide what authority the program is endowed with, and the progenitor
//! holds all userspace authority, so nothing confines it from these bytes. Three decoders read
//! three note types: [`decode`] (the manifest, versions 1, 2 and 3), [`decode_subtree_grants`] and
//! [`decode_verbs`]. Every input goes to all three, so a seed is a descriptor as it sits in the
//! note, and the input needs no framing.
//!
//! What this adds over the proofs. Kani proves `decode` total and proves `a_decoded_manifest_
//! encodes_back_to_its_bytes`, but only up to 57 bytes, which ends at the zone count byte. The
//! version 2 and 3 walk (`check_zones`, `check_serviced`, then `zones` and `service`) is past that
//! bound, and `decode_subtree_grants` and `decode_verbs` have no harness. The committed seeds are
//! encoded valid manifests of every version, and the mutator below builds more, so the fuzzer
//! starts inside the zone walk rather than hoping to guess a version word and 56 canonical bytes.

//! # What is asserted, beyond "it returned"
//!
//! 1. **One manifest has exactly one encoding.** When `decode` accepts, `encode` of the manifest
//!    (and, for version 2 and 3, the zones and the service byte the accessors report) written back
//!    gives the input byte for byte, and decoding that gives the same manifest. The same for a
//!    scope and a verb table.
//! 2. **A refusal is whole.** When `decode` refuses, `zones` and `service` refuse too.
//! 3. **Nothing rides on a note that the owner has not granted.** An accepted manifest never holds
//!    `reboot`, `sync`, `installer`, `catalog`, `indexes`, `activation_directory` or
//!    `console_input`: no note can ask for them.
//! 4. **The zones are canonical and strictly ascending**, at most eight, at least one in version 2;
//!    a version 1 or 2 descriptor never reports a service; the service byte of a version 3 is the
//!    last byte.
//! 5. **A valid descriptor with a byte cut off or added is refused.**
//! 6. **A verb table lists strictly ascending, nonempty words with no zero, and `grants_for` finds
//!    each.**

#![no_main]

use capability_manifest_note::{
    MAXIMUM_VERBS, MAXIMUM_ZONES, SERVICED_VERSION, SUBTREE_GRANTS_LEN, SubtreeGrantsNote,
    VERB_RECORD, VERBS_VERSION, ZONED_VERSION, decode, decode_subtree_grants, decode_verbs, encode,
    service, zones,
};
use grant_plan::{IndexStoreDeclaration, MAXIMUM_VERB, ManifestAuthorities};
use libfuzzer_sys::{fuzz_mutator, fuzz_target};

fuzz_target!(|data: &[u8]| {
    manifest(data);
    subtree_grants(data);
    verbs(data);
});

fn word(d: &[u8]) -> u32 {
    u32::from_le_bytes([d[0], d[1], d[2], d[3]])
}

fn manifest(d: &[u8]) {
    let Ok(m) = decode(d) else {
        // The accessors check the descriptor whole, as `decode` does.
        assert!(zones(d).is_err());
        assert!(service(d).is_err());
        return;
    };
    // What no note carries (property 3).
    assert!(!m.reboot && !m.sync && !m.installer && !m.catalog);
    assert!(!m.activation_directory && !m.console_input);
    assert!(matches!(m.indexes, IndexStoreDeclaration::Forbidden));

    // The first 56 bytes are version 1's encoding with the version word replaced (property 1).
    let mut again = encode(&m).to_vec();
    let version = word(d);
    let zs: Vec<&[u8]> = zones(d).expect("decode accepted").iter().collect();
    let served = service(d).expect("decode accepted");
    match version {
        1 => {
            assert!(zs.is_empty() && !served);
        }
        ZONED_VERSION | SERVICED_VERSION => {
            assert!(zs.len() <= MAXIMUM_ZONES);
            assert!(version == SERVICED_VERSION || !zs.is_empty());
            assert!(version == SERVICED_VERSION || !served);
            again[..4].copy_from_slice(&version.to_le_bytes());
            again.push(zs.len() as u8);
            for (i, zone) in zs.iter().enumerate() {
                assert!(domain_name_resolution_contract::is_canonical_zone(zone));
                assert!(i == 0 || zs[i - 1] < *zone, "zones ascend, each once");
                again.push(zone.len() as u8);
                again.extend_from_slice(zone);
            }
            if version == SERVICED_VERSION {
                assert_eq!(served, d[d.len() - 1] == 1);
                again.push(served as u8);
            }
        }
        other => panic!("decode accepted version {other}"),
    }
    assert_eq!(again, d, "one manifest, one encoding");
    assert_eq!(decode(&again), Ok(m));

    // Property 5: a descriptor is its exact length, whatever the version.
    assert!(decode(&d[..d.len() - 1]).is_err());
    let mut longer = d.to_vec();
    longer.push(0);
    assert!(decode(&longer).is_err());
}

fn subtree_grants(d: &[u8]) {
    let Ok(scope) = decode_subtree_grants(d) else {
        return;
    };
    let note = SubtreeGrantsNote::of(scope);
    assert_eq!(&note.0[note.0.len() - SUBTREE_GRANTS_LEN..], d);
    assert!(decode_subtree_grants(&d[..d.len() - 1]).is_err());
}

fn verbs(d: &[u8]) {
    let Ok(table) = decode_verbs(d) else {
        return;
    };
    let count = u32::from_le_bytes([d[4], d[5], d[6], d[7]]) as usize;
    assert!(count <= MAXIMUM_VERBS);
    assert_eq!(table.len(), count);
    assert_eq!(table.is_empty(), count == 0);
    let mut again = Vec::new();
    again.extend_from_slice(&VERBS_VERSION.to_le_bytes());
    again.extend_from_slice(&(count as u32).to_le_bytes());
    let mut previous: Option<&[u8]> = None;
    let mut seen = 0;
    for (verb, grants) in table.iter() {
        assert!(!verb.is_empty() && verb.len() <= MAXIMUM_VERB && !verb.contains(&0));
        assert!(previous.is_none_or(|p| p < verb), "verbs ascend, each once");
        previous = Some(verb);
        assert_eq!(table.grants_for(verb), Some(grants));
        let mut record = [0u8; VERB_RECORD];
        record[..verb.len()].copy_from_slice(verb);
        record[MAXIMUM_VERB..MAXIMUM_VERB + 2].copy_from_slice(&grants.bits().to_le_bytes());
        again.extend_from_slice(&record);
        seen += 1;
    }
    assert_eq!(seen, count);
    assert_eq!(again, d, "one table, one encoding");
}

// The structure-aware half. libFuzzer's byte mutations almost never keep a zone walk valid: one
// flipped length byte shifts every later zone, so mutants die at the first check. This mutator
// builds descriptors that are valid to the end and then, half the time, breaks one thing, so the
// fuzzer spends its inputs on the walk's edges. A descriptor that is not one of these shapes gets
// libFuzzer's own mutation.

/// Zone texts, sorted before use. Every one is canonical, including the root (empty), the longest
/// a zone may be, and names that are prefixes of each other, which the ordering check must place.
const ZONE_POOL: &[&str] = &[
    "",
    "a",
    "ab",
    "org",
    "nifeos.org",
    "basalt.nifeos.org",
    "example.com",
    "a.example.com",
    "xn--80ak6aa92e.com",
    "0",
    "9.9.9.9.in-addr.arpa",
    // 63 + 1 + 63 + 1 + 63 + 1 + 61 = 253 bytes.
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.\
     bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.\
     ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc.\
     ddddddddddddddddddddddddddddddddddddddddddddddddddddd",
];

/// Verb texts: a word, or a word, a space and a word.
const VERB_POOL: &[&[u8]] = &[
    b"a",
    b"ab",
    b"b",
    b"list",
    b"list -u",
    b"install",
    b"remove",
    b"0123456789abcdef",
];

struct Random(u32);

impl Random {
    fn next(&mut self) -> u32 {
        // xorshift32; the seed libFuzzer passes may be zero.
        let mut x = self.0 | 1;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        self.next() as usize % n
    }
}

fn build_zoned(base: &[u8], random: &mut Random) -> Vec<u8> {
    let mut out = if base.len() >= 56 {
        base[..56].to_vec()
    } else {
        encode(&grant_plan::NO_NOTE_MANIFEST).to_vec()
    };
    let mut picks: Vec<&str> = (0..random.below(MAXIMUM_ZONES + 1))
        .map(|_| ZONE_POOL[random.below(ZONE_POOL.len())])
        .collect();
    picks.sort();
    picks.dedup();
    let version = if picks.is_empty() || random.below(2) == 0 {
        SERVICED_VERSION
    } else {
        ZONED_VERSION
    };
    out[..4].copy_from_slice(&version.to_le_bytes());
    out.push(picks.len() as u8);
    for zone in &picks {
        out.push(zone.len() as u8);
        out.extend_from_slice(zone.as_bytes());
    }
    if version == SERVICED_VERSION {
        out.push(random.below(2) as u8);
    }
    out
}

fn build_verbs(random: &mut Random) -> Vec<u8> {
    let mut picks: Vec<&[u8]> = (0..random.below(VERB_POOL.len() + 1))
        .map(|_| VERB_POOL[random.below(VERB_POOL.len())])
        .collect();
    picks.sort();
    picks.dedup();
    let mut out = Vec::new();
    out.extend_from_slice(&VERBS_VERSION.to_le_bytes());
    out.extend_from_slice(&(picks.len() as u32).to_le_bytes());
    for verb in picks {
        let mut record = [0u8; VERB_RECORD];
        record[..verb.len()].copy_from_slice(verb);
        let bits = (random.next() as u16) & ManifestAuthorities::ALL.bits();
        record[MAXIMUM_VERB..MAXIMUM_VERB + 2].copy_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(&record);
    }
    out
}

/// Break one thing in a descriptor that was valid.
fn break_one(d: &mut Vec<u8>, random: &mut Random) {
    if d.is_empty() {
        return;
    }
    match random.below(5) {
        // Any byte at all, which for a zoned descriptor is mostly a length, a letter or the count.
        0 => {
            let at = random.below(d.len());
            d[at] = random.next() as u8;
        }
        // The count byte (zoned) or count word (verbs) off by a little.
        1 => {
            let at = if word(&pad4(d)) == VERBS_VERSION && d.len() >= 8 && d.len() != 56 {
                4
            } else {
                56.min(d.len() - 1)
            };
            d[at] = d[at].wrapping_add(random.below(3) as u8).wrapping_sub(1);
        }
        2 => {
            d.pop();
        }
        3 => d.push(random.next() as u8),
        // One byte of the fixed 56, where the manifest's own checks live.
        _ => {
            let at = random.below(56.min(d.len()));
            d[at] ^= 1 << random.below(8);
        }
    }
}

fn pad4(d: &[u8]) -> [u8; 4] {
    let mut w = [0u8; 4];
    let n = d.len().min(4);
    w[..n].copy_from_slice(&d[..n]);
    w
}

fuzz_mutator!(|data: &mut [u8], size: usize, max_size: usize, seed: u32| {
    let mut random = Random(seed);
    let current = &data[..size];
    let mut next = match random.below(4) {
        // A new zoned descriptor, on the manifest the input already carries when it has one.
        0 => build_zoned(current, &mut random),
        1 => build_verbs(&mut random),
        // The input as it is, to break one thing in.
        2 => current.to_vec(),
        _ => return libfuzzer_sys::fuzzer_mutate(data, size, max_size),
    };
    if random.below(2) == 0 {
        break_one(&mut next, &mut random);
    }
    if next.len() > max_size {
        next.truncate(max_size);
    }
    data[..next.len()].copy_from_slice(&next);
    next.len()
});


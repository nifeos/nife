//! The credential service's clients (milestone 56, the credential half; notes/credentials.md).
//!
//! One binary, three roles, because a program that shares the honest path is a fairer test of a
//! boundary than a different program failing for its own reasons. Every role holds exactly the
//! same endowment: an endpoint of the service, a report endpoint and the page it writes its request
//! into. No store, no provision endpoint, no entropy, no budget. The roles differ in what they send
//! and on which endpoint, never in whose password they present, so there is no role-to-credential
//! lookup here; [`PEOPLE`] lives in `credential_contract::fixture`.

//! # Roles
//!
//! - [`ROLE_HONEST`] is what any authenticating server would be: it asks three questions and
//!   reports the three answers, plus whether the shared page came back clean.
//! - [`ROLE_ATTACKER`] holds the identical endowment and tries to *write* the store through it:
//!   `PUT`, `SEAL`, an opcode nobody defined, and lengths outside the contract. Then it asks
//!   whether the credential it tried to install works. It must not.
//! - [`ROLE_PROVISIONER`] runs *before* either, on the **provision** endpoint. It is a role rather
//!   than a binary, so provisioning and verifying differ by one field of a `Spawn` literal: slot 0.
//!

//! # What the attacker proves, and what it does not
//!
//! It does not prove that a permission check works, because there is no permission check. It
//! proves the shape: by the time this program runs, the provision endpoint has been deleted at
//! both ends, so `PUT` is not a request that gets refused, it is a word with no object behind it.
//! The serve loop it reaches implements one opcode. See components/src/credentialer.rs.

//! # Capability contract (notes/abi.md §4)
//!
//! - slot 0: the credential service's endpoint (WRITE). **Which one is the whole story**: the
//!   provision endpoint for [`ROLE_PROVISIONER`], the verify endpoint for the other two.
//! - slot 1: the report endpoint (WRITE)
//! - mapped: one shared page, at [`PAGE_VA`] for a client and [`PROV_VA`] for the provisioner
//! - `a0`: the role
//!

//! Name: ratified 2026-08-01 (calef, milestone 63), replacing `credcli`. Refused `credcli` (a
//! squished abbreviation) and `credentialer_client` (that name belongs to the real client milestone
//! 55 needs for SMB authentication, and giving it to a test program squats it). `witness` was
//! considered for the family and set aside: it is standard in proof theory, model checking and
//! cryptography, but `client` carries information about what the program is that `witness` does
//! not.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use credential_contract as proto;
/// The identities and secrets this milestone's tests use, from the one place they live
/// (milestone 293). This file held the definition until then and two other files held their own
/// copies of it, one of which said in a comment that it had *chosen* to match this one; a fact
/// three files agree on by hand is a fact waiting to disagree.
use proto::fixture::PEOPLE;
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{call, exit, send};

/// The credential service's endpoint (slot 0). Verify or provision, depending on the role, and
/// this program cannot tell which it was given: that is the point of an unforgeable reference.
const SERVICE: u64 = 0;
/// The report endpoint (slot 1).
const REPORT: u64 = 1;

/// A client's shared page. Must match components/src/credentialer.rs `VERIFY_VA`.
const PAGE_VA: u64 = address_space_map::pair_page(0x0000_0000_00e1_0000);
// SAFETY: the wiring maps one page read/write at PAGE_VA before this program runs (milestone 139
// round 6).
const PAGE_WINDOW: MappedWindow = unsafe { MappedWindow::new(PAGE_VA, proto::PAGE as u64) };
/// The provisioner's shared page. Must match components/src/credentialer.rs `PROV_VA`.
const PROV_VA: u64 = address_space_map::pair_page(0x0000_0000_00e0_0000);
// SAFETY: as PAGE_WINDOW's.
const PROV_WINDOW: MappedWindow = unsafe { MappedWindow::new(PROV_VA, proto::PAGE as u64) };

/// An SMB adapter's shape: ask, and believe the answer.
pub const ROLE_HONEST: u64 = 0;
/// The same endowment, used to try to write the store.
pub const ROLE_ATTACKER: u64 = 1;
/// Phase one: fill the store and seal it.
pub const ROLE_PROVISIONER: u64 = 2;
/// The report's first word, so the kernel test knows who is speaking.
pub const RPT_DONE: u64 = 0x_c2ed_c11e_0000_0001;

/// Bit 0 of the report's third word: the shared page came back empty after the last reply.
pub const F_CLEAN: u64 = 1 << 0;
/// One share: the resource name, the password, the account name, and the domain. A named type
/// rather than a bare tuple because four `&[u8]`s in a row are a puzzle at the use site, which is
/// exactly what clippy's `type_complexity` is for.
type Share = (&'static [u8], &'static [u8], &'static [u8], &'static [u8]);

/// **Three shares, one per family member**, each with its own account name and password, which is
/// what "a secret is scoped to a resource rather than to an identity" means in practice
/// (design/roadmap/0065-secrets-service.md). A leaked key here authenticates to one share and to
/// nothing else, because there is nothing else it is the credential for.
///
/// The **first** uses [MS-NLMP] §4.2.1's account (`Domain\User`, password `Password`), which is
/// where it came from: milestone 65 stored it under that account because Microsoft publishes every
/// intermediate NTLMv2 value for it. That path was removed on 2026-08-30 (notes/smb.md) and these
/// are now three ordinary password records; the values are kept because nothing is served by
/// changing them and `credential_contract::fixture` is still where they live.
const SHARES: [Share; 3] = [
    (
        proto::fixture::SMB_RESOURCE,
        proto::fixture::SMB_PASSWORD,
        proto::fixture::SMB_USER,
        proto::fixture::SMB_DOMAIN,
    ),
    (
        b"backups-bob",
        b"another share secret",
        b"bob",
        b"WORKGROUP",
    ),
    (
        b"backups-carol",
        b"a third share secret",
        b"carol",
        b"WORKGROUP",
    ),
];

/// The identity the attacker tries to install for itself.
const IMPOSTOR: &[u8] = b"impostor";
const IMPOSTOR_SECRET: &[u8] = b"let me in";

#[unsafe(no_mangle)]
pub extern "C" fn _start(role: u64, _a1: u64, _a2: u64) -> ! {
    match role {
        ROLE_PROVISIONER => provisioner(),
        ROLE_ATTACKER => attacker(),
        _ => honest(),
    }
}

/// **Phase one**: three identities, then the seal, then a `PUT` that must arrive at a deleted
/// endpoint. That last one is deliberately *not* sent: a `CALL` on an endpoint whose receiver is
/// gone blocks forever, which would hang the test rather than fail it, and "the provisioner would
/// hang if it tried" is a worse thing to encode in a test than in a sentence. The proof that the
/// store is closed is the attacker's, which reaches a live endpoint that does not implement `PUT`.
fn provisioner() -> ! {
    let mut codes = Codes::new();
    for (identity, secret) in PEOPLE {
        codes.push(request(
            PROV_WINDOW,
            identity,
            secret,
            proto::provision::PUT,
        ));
    }
    // Then the three shares. A share's secret is scoped to the resource, so these are six
    // independent secrets in one store rather than three people with two credentials each. The
    // account name and domain each share carries stay in the table because they are what a share
    // is *for*; nothing stores them (notes/smb.md).
    for (resource, password, _user, _domain) in SHARES {
        codes.push(request(
            PROV_WINDOW,
            resource,
            password,
            proto::provision::PUT,
        ));
    }
    // A seventh secret in a six-slot store: FULL, not a silent overwrite of somebody.
    codes.push(request(
        PROV_WINDOW,
        b"nobody",
        b"no room",
        proto::provision::PUT,
    ));
    // Seal. `place` needs a non-empty identity and secret even for an opcode that reads neither;
    // the words carry the opcode and the page is wiped by the service either way.
    codes.push(request(
        PROV_WINDOW,
        b"seal",
        b"seal",
        proto::provision::SEAL,
    ));
    done(codes, u64::from(page_is_clean(PROV_WINDOW)))
}

/// **The honest client**: the right secret, the wrong secret, and an identity nobody provisioned.
fn honest() -> ! {
    let mut codes = Codes::new();
    let (identity, secret) = PEOPLE[0];
    codes.push(request(
        PAGE_WINDOW,
        identity,
        secret,
        proto::verify::VERIFY,
    ));
    codes.push(request(
        PAGE_WINDOW,
        identity,
        proto::fixture::NOBODYS_SECRET,
        proto::verify::VERIFY,
    ));
    codes.push(request(
        PAGE_WINDOW,
        b"nobody-at-all",
        secret,
        proto::verify::VERIFY,
    ));
    // The pairing is what is checked, not the secret alone: one person's password must not open
    // another person's account.
    let (other, _) = PEOPLE[1];
    codes.push(request(PAGE_WINDOW, other, secret, proto::verify::VERIFY));
    done(codes, u64::from(page_is_clean(PAGE_WINDOW)))
}

/// **The attacker**: the same slot 0, used for everything the contract does not offer.
fn attacker() -> ! {
    let mut codes = Codes::new();
    // Write the store through the endpoint we were given. The answer is `MISMATCH`, not a refusal,
    // and that is the model working rather than a hole in it: `provision::PUT` and `verify::VERIFY`
    // are both opcode 1, because the *endpoint* gives a number its meaning and a client cannot
    // choose which serve loop reads it. So this is a verify of an identity nobody provisioned, and
    // the honest answer is no. See `credential_contract`'s "an opcode is not an authority".
    codes.push(request(
        PAGE_WINDOW,
        IMPOSTOR,
        IMPOSTOR_SECRET,
        proto::provision::PUT,
    ));
    // Re-seal it, in case a service that had not sealed would accept one. `SEAL` is 2, and this
    // endpoint implements no opcode 2, so the answer is MALFORMED: a number's meaning is the
    // endpoint's rather than its own (`credential_contract`'s module docs, notes/smb.md).
    codes.push(request(
        PAGE_WINDOW,
        IMPOSTOR,
        IMPOSTOR_SECRET,
        proto::provision::SEAL,
    ));
    // An opcode nobody defined, in case the dispatch falls through to something.
    codes.push(request(PAGE_WINDOW, IMPOSTOR, IMPOSTOR_SECRET, 0x7f));
    // A length outside the contract, in case the service indexes before it checks. `place` will
    // not build this request, so the word is hand-made and the page is left as it was.
    let (r0, r1) = call(
        SERVICE,
        proto::req(proto::verify::VERIFY, 0xffff, 0xffff),
        0,
    );
    codes.push(if r1 == proto::NO_DATA {
        r0
    } else {
        // A reply that carried a second word at all is a finding, whatever it said.
        u64::MAX
    });
    // Opcode 3, which no serve loop here implements, so it is MALFORMED rather than a refusal:
    // there is still nothing to refuse. The probe is "an opcode from the other space", the case it
    // exercises (notes/smb.md).
    codes.push(request(PAGE_WINDOW, IMPOSTOR, IMPOSTOR_SECRET, 3));
    // And now the question that matters: did any of that install a credential?
    codes.push(request(
        PAGE_WINDOW,
        IMPOSTOR,
        IMPOSTOR_SECRET,
        proto::verify::VERIFY,
    ));
    done(codes, u64::from(page_is_clean(PAGE_WINDOW)))
}

/// Place a request in the shared page and `CALL`. Returns the service's reply code, or
/// [`u64::MAX`] if the reply carried data in its second word, which nothing here ever should.
fn request(window: MappedWindow, identity: &[u8], secret: &[u8], operation: u64) -> u64 {
    // SAFETY: forwarded from `window`'s own contract, and this process is its only user.
    let page = unsafe { window.as_mut_slice() };
    let Some(w0) = proto::place(page, identity, secret, operation) else {
        return u64::MAX;
    };
    let (r0, r1) = call(SERVICE, w0, 0);
    if r1 != proto::NO_DATA { u64::MAX } else { r0 }
}

/// **Is the shared page empty?** The service wipes it after reading every request, so after the
/// last `CALL` this frame should hold neither the secret this program presented nor anything the
/// service put there. Checking the whole page and not just the request area, because a byte of a
/// salt or a tag landing past the request area would be the interesting kind of leak.
fn page_is_clean(window: MappedWindow) -> bool {
    // SAFETY: forwarded from `window`'s own contract.
    let page = unsafe { window.as_slice() };
    page.iter().all(|&b| b == 0)
}

/// Up to eight reply codes, one per byte, so a run of requests fits in one report word. A code
/// wider than a byte (only [`u64::MAX`], which means "the reply was malformed") is recorded as
/// `0xff`, which no real code can be.
struct Codes {
    packed: u64,
    n: u32,
}

impl Codes {
    fn new() -> Self {
        Codes { packed: 0, n: 0 }
    }

    fn push(&mut self, code: u64) {
        if self.n >= 8 {
            return;
        }
        let byte = if code <= proto::MAX_CODE { code } else { 0xff };
        self.packed |= byte << (8 * self.n);
        self.n += 1;
    }
}

/// Report: who spoke, the packed reply codes, and a **flag word**. Bit 0 is "the shared page came
/// back empty", which every role sets. The NTLM role set two more bits until 2026-08-30
/// (notes/smb.md). A flag word rather than a bool because the third report word is the only one
/// left and a role that learns two things has to say both.
fn done(codes: Codes, flags: u64) -> ! {
    send(REPORT, RPT_DONE, codes.packed, flags);
    exit()
}

user_mode_runtime::panic_handler!();

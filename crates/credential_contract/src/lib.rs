//! The credential contract: the two request shapes a credential service serves, defined once for
//! the service, its provisioner, its clients and the kernel-side tests. The provision endpoint
//! (`PUT`, then `SEAL`, then deleted at both ends) writes the store, and the verify endpoint uses
//! it forever, so a verify holder can use a credential and cannot read one.
//!
//! [`provision::PUT`] and [`verify::VERIFY`] are both opcode 1, because the endpoint gives a number
//! its meaning. Every reply is one word with [`NO_DATA`] second, and [`MISMATCH`] answers a wrong
//! secret and an unknown identity alike, so the verify endpoint is neither a decryption oracle nor
//! an identity oracle. See notes/credentials.md and notes/credentials/contract.md.
//!
//! Name: ratified 2026-08-23 (calef), renamed from `cred_proto` to spell out the contraction;
//! `_contract` per calef's 2026-10-18 ruling.

#![cfg_attr(not(test), no_std)]

/// The shared-page size, in bytes. One host page, the same unit `filesystem_contract` moves, and far more
/// than the [`MAX_IDENTITY`] + [`MAX_SECRET`] a request can fill.
pub const PAGE: usize = 4096;

/// Where a request packs its opcode: bits 63:56 of the first `CALL` word, the same position
/// `filesystem_contract`, `entropy_contract` and `line_editor::proto` use, so the contracts read alike.
pub const OPERATION_SHIFT: u32 = 56;

/// The longest identity, in bytes. An identity here is **an opaque byte string and nothing more**:
/// no user id, no group, no home directory, no session. milestone 49 (users, login, attribution)
/// is a different milestone and this crate deliberately does not start it. 64 bytes is longer than
/// any SMB account name and short enough that the whole store is a small fixed array.
pub const MAX_IDENTITY: usize = 64;

/// The longest secret a request may present, in bytes. Long enough for a passphrase nobody would
/// type twice; the bound exists so the service's parse is total and its page layout is fixed.
pub const MAX_SECRET: usize = 256;

/// Where the identity starts in the shared page.
pub const ID_OFF: usize = 0;

/// Where the secret starts in the shared page. [`MAX_IDENTITY`] bytes after the identity, so the
/// two never overlap and a short identity does not shift the secret.
pub const SECRET_OFF: usize = MAX_IDENTITY;

/// The whole area this contract owns in the shared page, and therefore what [`wipe`] clears: the
/// identity and the secret. Everything past it belongs to whoever mapped the frame, and a wipe
/// that ran past this bound would scribble on state it was never told about.
pub const LAYOUT_LEN: usize = SECRET_OFF + MAX_SECRET;

/// The layout has to fit in the frame both parties map, and a compile-time assertion is the only
/// check that cannot be skipped by a build that never runs a test.
const _: () = assert!(LAYOUT_LEN <= PAGE, "the layout must fit in one shared page");

/// The second word of every reply. See the module docs: the reply channel carries a verdict and
/// carries no data, because there is no fact about the store a caller is entitled to.
pub const NO_DATA: u64 = 0;

/// Phase one, on the provision endpoint. These opcodes exist only while the store is being
/// written, which is before any client has been handed anything.
pub mod provision {
    /// Store `identity` with `secret`. The service draws the salt itself, from the entropy
    /// service, so a provisioner cannot choose a weak one or reuse one. Reply [`super::OK`],
    /// [`super::FULL`], or [`super::MALFORMED`].
    pub const PUT: u64 = 1;

    /// No more writing. The service replies [`super::OK`] and then deletes its receive end of
    /// this endpoint; the provisioner deletes its send end. After that the capability is gone from
    /// both sides and the store cannot be changed by anything short of restarting the service.
    /// The page's contents are not the seal's business; the service zeroes it after every PUT.
    pub const SEAL: u64 = 2;
}

/// Phase two, on the verify endpoint. One opcode: the store holds one kind of secret and answers
/// one question about it.
pub mod verify {
    /// Is `secret` the secret for `identity`? Reply [`super::MATCH`] or [`super::MISMATCH`], and
    /// [`super::MALFORMED`] if the lengths in the request word are out of range.
    pub const VERIFY: u64 = 1;
}

/// The request was accepted (a `PUT` landed, or a `SEAL` took effect).
pub const OK: u64 = 1;
/// The presented secret is the identity's secret.
pub const MATCH: u64 = 2;
/// It is not, or there is no such identity; see the module docs for why those are one answer.
pub const MISMATCH: u64 = 3;
/// The request word's lengths are out of range, or the opcode is not one this phase serves. Not an
/// authentication outcome: a client that gets this learns nothing about the store.
pub const MALFORMED: u64 = 4;
/// A `PUT` with no slot left. Provisioning only.
pub const FULL: u64 = 5;
/// The service could not obtain a salt and therefore stored nothing. Provisioning only. Distinct
/// from [`MALFORMED`], which is the provisioner's bug; serving anyway with a made-up salt is the
/// silent degradation DECISIONS §42 (a filesystem declares what it offers and must be truthful)
/// forbids. See notes/credentials/contract.md.
pub const NO_ENTROPY: u64 = 6;

/// The largest reply code. Everything at or below this is a reply; see [`code`].
pub const MAX_CODE: u64 = NO_ENTROPY;

/// Build a request's first word: the opcode, the identity's length, and the secret's length.
///
/// The lengths ride in the word rather than in the page so the page is payload only, which is what
/// lets the service zero it unconditionally after reading.
pub const fn req(operation: u64, id_len: usize, secret_len: usize) -> u64 {
    (operation << OPERATION_SHIFT) | ((id_len as u64 & 0xffff) << 16) | (secret_len as u64 & 0xffff)
}

/// The opcode of a request word.
pub const fn operation(w0: u64) -> u64 {
    w0 >> OPERATION_SHIFT
}

/// The identity length a request word claims. Not yet checked against [`MAX_IDENTITY`]; that is
/// [`read`]'s job, and it is the reason [`read`] exists rather than two accessors.
pub const fn id_len(w0: u64) -> usize {
    ((w0 >> 16) & 0xffff) as usize
}

/// The secret length a request word claims. Same caveat as [`id_len`].
pub const fn secret_len(w0: u64) -> usize {
    (w0 & 0xffff) as usize
}

/// Client side: write `identity` and `secret` into the shared page and return the request word to
/// `CALL` with. `None` if either is empty or over its bound, or the page is too small: a caller
/// bug, so it never becomes a message. The empty identity would collide with an unwritten slot in
/// the service's store, and `credentialer::Store` refuses it too.
///
/// # Examples
///
/// A verify is one page write and one `CALL`; client and server read the same two functions in
/// opposite directions:
///
/// ```
/// use credential_contract::{PAGE, place, read, verify, wipe};
///
/// // Client: put the identity and the presented secret in the page both parties map.
/// let mut page = [0u8; PAGE];
/// let w0 = place(&mut page, b"bob", b"hunter2", verify::VERIFY).expect("within bounds");
///
/// // Server: every `u64` either names a well-formed request or is refused, so the serve loop has
/// // no arithmetic that could run off the page.
/// let (identity, secret) = read(&page, w0).expect("a well-formed request");
/// assert_eq!(identity, b"bob");
/// assert_eq!(secret, b"hunter2");
///
/// // Then the service wipes the request area, so the frame holds no secret once the reply lands.
/// wipe(&mut page);
/// assert!(!page.windows(7).any(|w| w == b"hunter2"));
///
/// // An empty identity is a caller bug and never becomes a message: it is the one string that
/// // would collide with an unwritten slot in the service's fixed-size store.
/// assert_eq!(place(&mut page, b"", b"hunter2", verify::VERIFY), None);
/// ```
pub fn place(page: &mut [u8], identity: &[u8], secret: &[u8], operation: u64) -> Option<u64> {
    if page.len() < SECRET_OFF + MAX_SECRET {
        return None;
    }
    if identity.is_empty() || identity.len() > MAX_IDENTITY {
        return None;
    }
    if secret.is_empty() || secret.len() > MAX_SECRET {
        return None;
    }
    page[ID_OFF..SECRET_OFF].fill(0);
    page[ID_OFF..ID_OFF + identity.len()].copy_from_slice(identity);
    page[SECRET_OFF..SECRET_OFF + MAX_SECRET].fill(0);
    page[SECRET_OFF..SECRET_OFF + secret.len()].copy_from_slice(secret);
    Some(req(operation, identity.len(), secret.len()))
}

/// Server side: the identity and the secret a request word points at, or `None` if the lengths
/// are not ones this contract allows. Total: every `u64` either names a well-formed request or is
/// rejected, so the serve loop has no arithmetic that could run off the page.
pub fn read(page: &[u8], w0: u64) -> Option<(&[u8], &[u8])> {
    let (i, s) = (id_len(w0), secret_len(w0));
    if i == 0 || i > MAX_IDENTITY || s == 0 || s > MAX_SECRET {
        return None;
    }
    if page.len() < SECRET_OFF + MAX_SECRET {
        return None;
    }
    Some((&page[ID_OFF..ID_OFF + i], &page[SECRET_OFF..SECRET_OFF + s]))
}

/// Zero the request area of a shared page. The service calls this after every request it reads,
/// so the shared frame holds no presented secret once the reply lands; a client that keeps running
/// should call it too. `write_volatile` rather than `fill`, because a compiler that can prove
/// nobody reads these bytes again is entitled to delete a plain store.
pub fn wipe(page: &mut [u8]) {
    let n = page.len().min(LAYOUT_LEN);
    for b in &mut page[..n] {
        // SAFETY: `b` is a live, unique, aligned reference to one byte of the caller's slice.
        unsafe { core::ptr::write_volatile(b, 0) };
    }
    // PAIR: none. A compiler fence emits no instruction and orders nothing between cores, so it
    // has no acquire side. It stops the optimiser sinking the wipe below what the caller does next;
    // `write_volatile` stops it being deleted. The `CALL` rendezvous orders this page against the
    // other party, not this line, which is not a `fence(SeqCst)`.
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

/// Read a reply's first word. `Some(code)` when the credential service answered; `None` when the
/// word did not come from it at all. Every reply code is a small positive, and every kernel failure
/// from a `CALL` is a small negative (`abi::Error`, -1 to -8), which reads as an enormous `u64`,
/// so "no credential service" never reads as an answer (`entropy_contract::delivered`'s shape).
pub const fn code(r0: u64) -> Option<u64> {
    if r0 >= OK && r0 <= MAX_CODE {
        Some(r0)
    } else {
        None
    }
}

/// The one question this contract exists to answer: anything that is not an unambiguous [`MATCH`]
/// is a refusal. A missing capability, a malformed request, a dead service and a wrong password
/// all return `false`, so no caller has to remember which codes were the good ones.
///
/// # Examples
///
/// ```
/// use credential_contract::{MALFORMED, MATCH, MISMATCH, OK, code, is_authenticated};
///
/// assert!(is_authenticated(MATCH));
///
/// assert!(!is_authenticated(MISMATCH)); // the wrong secret, or an identity not in the store
/// assert!(!is_authenticated(OK)); // a provisioning success is not an authentication
/// assert!(!is_authenticated(MALFORMED));
///
/// // `abi::Error` is -1 to -8. As a `u64` each is enormous, so no reply code collides with one,
/// // and an empty capability slot is distinguishable from an answer without a probe request.
/// assert_eq!(code(-4i64 as u64), None);
/// assert!(!is_authenticated(-4i64 as u64));
/// ```
pub const fn is_authenticated(r0: u64) -> bool {
    matches!(code(r0), Some(MATCH))
}

/// The one login the gates share, so several programs agree on an account down to the byte. It is
/// [MS-NLMP] §4.2.1's published account, so a gate asserts against numbers Microsoft printed. A
/// fixture, not a deployment: a real share's account must never be these (notes/smb.md's BUGS).
/// See notes/credentials/contract.md.
pub mod fixture {
    /// The resource the SMB gate's share authenticates against: the name its NTLM key is stored
    /// under. A *resource* rather than an account, which is milestone 65's model
    /// (design/roadmap/0065-secrets-service.md): a secret is scoped to the thing it opens.
    pub const SMB_RESOURCE: &[u8] = b"backups-alice";
    /// The password behind it. Published by Microsoft; secret to nobody.
    pub const SMB_PASSWORD: &[u8] = b"Password";
    /// The account name bound into the stored key at provisioning time.
    pub const SMB_USER: &[u8] = b"User";
    /// The domain bound into it. Not uppercased anywhere, which is [MS-NLMP] §3.3.2's asymmetry
    /// and the detail a reimplementation gets wrong silently.
    pub const SMB_DOMAIN: &[u8] = b"Domain";

    /// The three people every credential fixture in this tree authenticates (milestone 293), as
    /// identity and secret pairs rather than parallel arrays, so an insertion cannot pair `bob`
    /// with `carol`'s password. `carol` has no home subtree in `kernel::user::login_tests`. See
    /// notes/credentials/contract.md.
    pub const PEOPLE: [(&[u8], &[u8]); 3] = [
        (b"alice", b"correct horse battery staple"),
        (b"bob", b"a different secret entirely"),
        (b"carol", b"and a third one"),
    ];

    /// Index of `alice` in [`PEOPLE`], for both [`identity`] and [`secret`].
    ///
    /// Name: ratified 2026-10-12 (calef, renamed by calef's ruling of 2026-10-12 (UTC) removing
    /// family names from the public tree).
    pub const ALICE: u64 = 0;
    /// Index of `bob` in [`PEOPLE`].
    ///
    /// Name: ratified 2026-10-12 (calef, renamed by calef's ruling of 2026-10-12 (UTC) removing
    /// family names from the public tree).
    pub const BOB: u64 = 1;
    /// Index of `carol` in [`PEOPLE`].
    ///
    /// Name: ratified 2026-10-12 (calef, renamed by calef's ruling of 2026-10-12 (UTC) removing
    /// family names from the public tree).
    pub const CAROL: u64 = 2;

    /// A secret that is nobody's, for the refusal every credential gate owes: a real identity
    /// presenting a real-looking secret that was never provisioned for anyone.
    ///
    /// Not another person's secret, deliberately. "One person's password must not open another
    /// person's account" is a different property, and `credentialer_test_client`'s honest role
    /// already tests it; a wrong-secret refusal that reused `bob`'s password would be testing
    /// that one twice and the plain wrong-secret case never.
    pub const NOBODYS_SECRET: &[u8] = b"not-the-password";

    /// The secret index that means [`NOBODYS_SECRET`]. One past the last person, derived from
    /// [`PEOPLE`] rather than written as `3`, so growing the roster cannot silently turn this into
    /// a fourth person's real secret.
    pub const WRONG: u64 = PEOPLE.len() as u64;

    /// No credential at all, for a caller that presents neither. Both [`identity`] and
    /// [`secret`] answer `None`, so a program handed this and asked to authenticate anyway reports
    /// a malformed request instead of quietly logging in as whoever index zero happens to be.
    pub const NONE: u64 = u64::MAX;

    /// The identity at `i`, or `None` for [`NONE`] and any other index outside [`PEOPLE`].
    ///
    /// An index rather than a string because this is what a spawn argument can carry: a process is
    /// started with three integer registers and no environment, so the bytes have to be somewhere
    /// both sides already link, and the register names *which*. See `fixtures/src/login_test_client.rs`.
    pub fn identity(i: u64) -> Option<&'static [u8]> {
        PEOPLE.get(usize::try_from(i).ok()?).map(|p| p.0)
    }

    /// The secret at `i`: that person's own for an index into [`PEOPLE`], [`NOBODYS_SECRET`] for
    /// [`WRONG`], and `None` for [`NONE`] or anything else.
    pub fn secret(i: u64) -> Option<&'static [u8]> {
        if i == WRONG {
            return Some(NOBODYS_SECRET);
        }
        PEOPLE.get(usize::try_from(i).ok()?).map(|p| p.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The roster's indices name who they say they name. A fixture whose `ALICE` had drifted to
    /// `bob` would still authenticate, still pass every login test, and quietly stop proving
    /// that two identities land in two subtrees, which is the one thing those tests exist for.
    #[test]
    fn the_roster_indices_name_the_people_they_are_named_for() {
        assert_eq!(fixture::identity(fixture::ALICE), Some(b"alice".as_slice()));
        assert_eq!(fixture::identity(fixture::BOB), Some(b"bob".as_slice()));
        assert_eq!(fixture::identity(fixture::CAROL), Some(b"carol".as_slice()));
        for i in [fixture::ALICE, fixture::BOB, fixture::CAROL] {
            assert_eq!(fixture::secret(i), Some(fixture::PEOPLE[i as usize].1));
        }
    }

    /// `WRONG` is nobody's, which is the property the refusal tests rest on: a wrong-secret run
    /// that happened to present a real person's real secret would be authenticated, and the test
    /// asserting a denial would fail in a way that reads as a service bug.
    #[test]
    fn the_wrong_secret_belongs_to_no_one_on_the_roster() {
        assert_eq!(
            fixture::secret(fixture::WRONG),
            Some(fixture::NOBODYS_SECRET)
        );
        assert!(fixture::identity(fixture::WRONG).is_none());
        assert!(
            fixture::PEOPLE
                .iter()
                .all(|&(_, s)| s != fixture::NOBODYS_SECRET)
        );
    }

    /// `NONE` is neither, so a behaviour that takes no credential cannot accidentally be handed
    /// person zero's.
    #[test]
    fn no_credential_at_all_reads_as_absent_on_both_halves() {
        assert!(fixture::identity(fixture::NONE).is_none());
        assert!(fixture::secret(fixture::NONE).is_none());
    }

    #[test]
    fn a_request_round_trips_its_opcode_and_both_lengths() {
        let w = req(verify::VERIFY, 7, 260);
        assert_eq!(operation(w), verify::VERIFY);
        assert_eq!(id_len(w), 7);
        assert_eq!(secret_len(w), 260);
    }

    /// The wire layout, pinned as one exact number. The serve loop on the far side of the
    /// endpoint decodes this word with its own copy of these shifts, so the bit positions are the
    /// contract, not an implementation detail a refactor may move. The opcode is `SEAL` on
    /// purpose: `PUT` and `VERIFY` are both 1, and a test that only ever builds opcode 1 cannot
    /// tell a working `operation` from one that returns a constant.
    #[test]
    fn the_request_word_is_the_documented_bit_layout() {
        let w = req(provision::SEAL, MAX_IDENTITY, MAX_SECRET);
        assert_eq!(w, 0x0200_0000_0040_0100);
        assert_eq!(operation(w), provision::SEAL);
        assert_eq!(id_len(w), MAX_IDENTITY);
        assert_eq!(secret_len(w), MAX_SECRET);
    }

    #[test]
    fn place_and_read_are_inverses() {
        let mut page = [0xAAu8; PAGE];
        let w = place(&mut page, b"bob", b"hunter2", verify::VERIFY).unwrap();
        let (id, secret) = read(&page, w).unwrap();
        assert_eq!(id, b"bob");
        assert_eq!(secret, b"hunter2");
    }

    /// A short identity must not shift the secret, and must not leave the previous caller's
    /// identity visible past its end. Both are the kind of thing a fixed-offset layout gets right
    /// by construction and a packed one gets wrong once.
    #[test]
    fn a_second_request_leaves_nothing_of_the_first() {
        let mut page = [0u8; PAGE];
        place(
            &mut page,
            b"a-very-long-identity-name",
            b"a-long-secret-value",
            verify::VERIFY,
        )
        .unwrap();
        let w = place(&mut page, b"cd", b"ef", verify::VERIFY).unwrap();
        let (id, secret) = read(&page, w).unwrap();
        assert_eq!(id, b"cd");
        assert_eq!(secret, b"ef");
        assert!(
            page[..SECRET_OFF + MAX_SECRET]
                .windows(4)
                .all(|w| w != b"very"),
            "the previous identity survived into the next request",
        );
    }

    /// The parse is total. Nothing a client can put in the request word makes the server read
    /// outside the page or believe a length it should not.
    #[test]
    fn every_out_of_range_length_is_refused() {
        let page = [0u8; PAGE];
        for (i, s) in [
            (0, 8),
            (8, 0),
            (MAX_IDENTITY + 1, 8),
            (8, MAX_SECRET + 1),
            (0xffff, 0xffff),
        ] {
            assert!(
                read(&page, req(verify::VERIFY, i, s)).is_none(),
                "id_len {i}, secret_len {s} should be refused",
            );
        }
        assert!(read(&page, req(verify::VERIFY, MAX_IDENTITY, MAX_SECRET)).is_some());
    }

    /// A page too small to hold the fixed layout is refused rather than indexed into. The service
    /// maps a whole frame, so this is a contract that cannot be violated in the tree today; the
    /// check is here so a future caller with a smaller buffer fails loudly instead of panicking.
    #[test]
    fn a_page_too_small_for_the_layout_is_refused_at_both_ends() {
        let mut small = [0u8; SECRET_OFF + MAX_SECRET - 1];
        assert!(place(&mut small, b"x", b"y", verify::VERIFY).is_none());
        assert!(read(&small, req(verify::VERIFY, 1, 1)).is_none());
    }

    /// A page of exactly `SECRET_OFF + MAX_SECRET` bytes is the smallest the fixed layout fits,
    /// and both ends must accept it: the size check refuses a page the layout runs off, not a
    /// page with no slack after it.
    #[test]
    fn the_smallest_page_the_layout_fits_is_accepted_at_both_ends() {
        let mut page = [0u8; SECRET_OFF + MAX_SECRET];
        let w = place(&mut page, b"id", b"secret", verify::VERIFY).unwrap();
        let (id, secret) = read(&page, w).unwrap();
        assert_eq!(id, b"id");
        assert_eq!(secret, b"secret");
    }

    #[test]
    fn an_empty_identity_or_secret_never_becomes_a_request() {
        let mut page = [0u8; PAGE];
        assert!(place(&mut page, b"", b"secret", verify::VERIFY).is_none());
        assert!(place(&mut page, b"id", b"", verify::VERIFY).is_none());
    }

    #[test]
    fn place_refuses_what_will_not_fit() {
        let mut page = [0u8; PAGE];
        assert!(place(&mut page, &[b'x'; MAX_IDENTITY + 1], b"s", verify::VERIFY).is_none());
        assert!(place(&mut page, b"i", &[b'x'; MAX_SECRET + 1], verify::VERIFY).is_none());
        assert!(
            place(
                &mut page,
                &[b'x'; MAX_IDENTITY],
                &[b'x'; MAX_SECRET],
                verify::VERIFY
            )
            .is_some()
        );
    }

    /// The wipe covers everything this contract owns, which is why [`LAYOUT_LEN`] is one name
    /// rather than an expression repeated in three places: growing the layout is then one edit.
    #[test]
    fn wipe_clears_the_whole_layout_and_nothing_past_it() {
        let mut page = [0xFFu8; PAGE];
        place(&mut page, b"identity", b"secret", verify::VERIFY).unwrap();
        wipe(&mut page);
        assert!(page[..LAYOUT_LEN].iter().all(|&b| b == 0));
        // The rest of the page is not wipe's to clear: the frame is shared, and a wipe whose
        // bound has grown is a wipe scribbling on state it was never told about.
        assert!(page[LAYOUT_LEN..].iter().all(|&b| b == 0xFF));
    }

    /// The layout is disjoint, in the strong sense: every field this contract names occupies bytes
    /// no other field does, and all of them fit in a page. Written as arithmetic on the constants
    /// rather than as literals, so it stays true when one of them moves, and stated at all because
    /// two overlapping offsets is a bug that shows up as one field mysteriously containing
    /// another's bytes.
    #[test]
    fn the_page_layout_does_not_overlap_itself() {
        // Kept with two fields: the property is about the layout, and the next field is what it
        // is for.
        let fields = [(ID_OFF, MAX_IDENTITY), (SECRET_OFF, MAX_SECRET)];
        for (i, &(a, alen)) in fields.iter().enumerate() {
            assert!(a + alen <= LAYOUT_LEN, "field {i} runs past the layout");
            for &(b, blen) in &fields[i + 1..] {
                assert!(
                    a + alen <= b || b + blen <= a,
                    "fields at {a} (+{alen}) and {b} (+{blen}) overlap",
                );
            }
        }
        // That the layout fits in a page is a `const _: () = assert!(..)` next to LAYOUT_LEN,
        // because it is a property of the constants and does not need a test run to be true.
    }

    /// The property the whole error story rests on: no reply code collides with any error the
    /// kernel can return, so a caller holding no credential capability cannot mistake a refusal
    /// for a `MATCH`. `abi::Error` is -1..-8; those are the words a `CALL` on an empty slot puts
    /// in the first register.
    #[test]
    fn a_kernel_refusal_is_never_mistaken_for_a_verdict() {
        for c in OK..=MAX_CODE {
            assert_eq!(code(c), Some(c));
        }
        for err in 1i64..=8 {
            assert_eq!(code((-err) as u64), None, "kernel error -{err}");
            assert!(!is_authenticated((-err) as u64), "kernel error -{err}");
        }
        assert_eq!(code(0), None);
        assert_eq!(code(MAX_CODE + 1), None);
    }

    /// Only [`MATCH`] authenticates. Written as an exhaustive sweep rather than a handful of cases
    /// because the failure this guards against is a *new* code being added later and quietly
    /// falling on the permissive side.
    #[test]
    fn nothing_but_match_authenticates() {
        for w in 0u64..=MAX_CODE + 4 {
            assert_eq!(is_authenticated(w), w == MATCH, "reply word {w}");
        }
        assert!(!is_authenticated(u64::MAX));
    }
}

/// Proofs, for the properties a test can only sample (`script/verify`). Both are about what an
/// adversary can send or receive, so they cover every one of the 2^64 request and reply words.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// The server's parse is total. For *any* first word a client can send, `read` either
    /// refuses it or hands back two slices that lie inside the page and have exactly the lengths
    /// the word claimed. Kani proves the memory safety (no index runs off the page for any input);
    /// the assertions pin the meaning, so a future rewrite that stayed in bounds while returning
    /// the wrong bytes would still fail.
    ///
    /// This is the property that lets the credential service's serve loop have no arithmetic in it
    /// that could go wrong. It is also the one an attacker probes first.
    /// Falsification: replayable `crates/credential_contract/falsifications/proofs.no_request_word_makes_the_parse_read_outside_the_page.patch`
    #[kani::proof]
    fn no_request_word_makes_the_parse_read_outside_the_page() {
        let page = [0u8; SECRET_OFF + MAX_SECRET];
        let w0: u64 = kani::any();
        match read(&page, w0) {
            None => {}
            Some((identity, secret)) => {
                // The shifts as literals, the wire format (milestone 211): asserting only
                // against `id_len` and `secret_len` would prove that `read` used them.
                assert!(identity.len() == ((w0 >> 16) & 0xffff) as usize);
                assert!(secret.len() == (w0 & 0xffff) as usize);
                assert!(identity.len() == id_len(w0));
                assert!(secret.len() == secret_len(w0));
                assert!(!identity.is_empty() && identity.len() <= MAX_IDENTITY);
                assert!(!secret.is_empty() && secret.len() <= MAX_SECRET);
                assert!(ID_OFF + identity.len() <= SECRET_OFF);
                assert!(SECRET_OFF + secret.len() <= page.len());
            }
        }
    }

    /// Nothing but `MATCH` authenticates, for every word in the space. The failure this rules
    /// out is the worst one this contract could permit: a caller holding no credential capability,
    /// or talking to a service that died, reading the kernel's refusal as a successful login. The
    /// host test sweeps a few dozen values around the boundary; this sweeps all of them.
    /// Falsification: replayable `crates/credential_contract/falsifications/proofs.no_reply_word_but_match_ever_authenticates.patch`
    #[kani::proof]
    fn no_reply_word_but_match_ever_authenticates() {
        let r0: u64 = kani::any();
        assert!(is_authenticated(r0) == (r0 == MATCH));
        // And the discrimination itself: a code is a code exactly when it is in range, so no
        // arithmetic on a kernel error word can land inside the reply space.
        match code(r0) {
            Some(c) => assert!(c == r0 && (OK..=MAX_CODE).contains(&c)),
            None => assert!(!(OK..=MAX_CODE).contains(&r0)),
        }
    }

    /// A request word round-trips its three fields for every combination `place` can produce;
    /// outside those ranges the packing is deliberately lossy. On its own it is blind to a swap of
    /// both length fields in encoder and decoder (milestone 213); the parse proof above catches
    /// that, so weakening it uncovers the wire format. See notes/credentials/contract.md.
    /// Falsification: replayable `crates/credential_contract/falsifications/proofs.a_request_word_round_trips_every_field.patch`
    #[kani::proof]
    fn a_request_word_round_trips_every_field() {
        let operation_in: u64 = kani::any();
        let i: usize = kani::any();
        let s: usize = kani::any();
        kani::assume(operation_in <= 0xff);
        kani::assume(i <= MAX_IDENTITY);
        kani::assume(s <= MAX_SECRET);
        let w = req(operation_in, i, s);
        assert!(operation(w) == operation_in);
        assert!(id_len(w) == i);
        assert!(secret_len(w) == s);
    }
}

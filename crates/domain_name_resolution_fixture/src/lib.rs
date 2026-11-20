#![no_std]
//! **The name resolver test fixture**: the zone, the name server peer and the cases of the QEMU
//! runners' resolver tests, in one place for the programs that must agree on them. The test client
//! (`fixtures/src/name_resolver_test_client.rs`) asks each name in order, the kernel's test
//! (`system_tests/src/user/name_resolver_tests.rs`) judges each reply by its index, and the
//! standard library exerciser (`std_exerciser/src/bin/std_resolve.rs`) asks the same names through
//! `std`. They are a test harness's facts, not part of the resolver contract. The peer that answers,
//! `helpers/name-server-peer`, spells its zone again in Python, so the two sides are written
//! independently, as `socket_fixture` splits the inbound gate.
//!
//! Not published: nothing outside this tree has a reason to read it.
//!
//! Name: provisional, minted 2026-11-20 (UTC) by lane 1086-resolver-contract-before-the-move; the
//! crate holds what `domain_name_resolution_contract` held as its `fixture` module. Considered
//! `name_resolver_fixture`, after the program, and followed `filesystem_fixture` and
//! `socket_fixture`, which take their contract's name. Calef names crates.

use domain_name_resolution_contract::{reject_code, status};
use domain_name_system::{Error, Reject};

/// The zone the test grants its client.
///
/// Name: provisional; the contract crate's `fixture` module held it under the same name.
pub const ZONE: &str = "nife.test";
/// The runners' name server peer, a `guestfwd` to `helpers/name-server-peer` on the Domain Name System (DNS) port, and
/// the address its zone answers with.
///
/// Name: provisional; the contract crate's `fixture` module held it under the same name.
pub const SERVER: [u8; 4] = [10, 0, 2, 9];

/// The test client's `arg0` when it was given a stack and ends by connecting to what it
/// resolved.
///
/// Name: provisional; the contract crate's `fixture` module held it under the same name.
pub const WITH_NETWORK: u64 = 1;
/// The test client's last report when the echo through the resolved address came back. The last
/// report carries one of the four `ECHO_` codes.
///
/// Name: provisional; the contract crate's `fixture` module held it under the same name.
pub const ECHO_OK: u64 = 1;
/// The test client's last report when it was given no stack.
///
/// Name: provisional; the contract crate's `fixture` module held it under the same name.
pub const ECHO_SKIPPED: u64 = 2;
/// The test client's last report when the first name did not resolve, so there was nowhere to
/// connect.
///
/// Name: provisional, minted 2026-11-20 (UTC) by lane 1086-resolver-contract-before-the-move.
/// Refused `ECHO_NO_ADDRESS`, the module's spelling, which the template's register check reads as a
/// memory address that says neither virtual nor physical.
pub const ECHO_NOTHING_RESOLVED: u64 = 3;
/// The test client's last report when the connection or the echo failed.
///
/// Name: provisional; the contract crate's `fixture` module held it under the same name.
pub const ECHO_FAILED: u64 = 4;

/// One name, and the reply the client must get for it.
///
/// Name: provisional; the contract crate's `fixture` module held it under the same name.
pub struct Case {
    /// The host name the client writes in its page.
    pub name: &'static str,
    /// The status the reply must carry.
    pub status: u8,
    /// Its detail byte.
    pub detail: u8,
}

const fn case(name: &'static str, status: u8, detail: u8) -> Case {
    Case {
        name,
        status,
        detail,
    }
}

/// The cases, in the order the client asks them. The first resolves, and is the address the
/// client then connects to.
///
/// Name: provisional; the contract crate's `fixture` module held it under the same name.
pub const CASES: [Case; 10] = [
    case("packages.nife.test", status::OK, 0),
    // A compressed CNAME to the name above, the way a recursive server answers.
    case("mirror.nife.test", status::OK, 0),
    case("nosuch.nife.test", status::NO_SUCH_NAME, 0),
    // The peer's catch-all answer is REFUSED, response code 5.
    case("unlisted.nife.test", status::SERVER_ERROR, 5),
    // The right answer under the wrong transaction id.
    case(
        "forged.nife.test",
        status::REFUSED,
        reject_code(Reject::IdMismatch),
    ),
    // An address for bank.nife.test, and none for the name asked.
    case(
        "poisoned.nife.test",
        status::REFUSED,
        reject_code(Reject::NoIpv4Address),
    ),
    // An owner that is a compression pointer to itself.
    case(
        "loop.nife.test",
        status::REFUSED,
        reject_code(Reject::Malformed(Error::PointerForward)),
    ),
    // Outside the zone, so never asked.
    case("example.com", status::DENIED, 0),
    // Ends in the zone's text and is not under it.
    case("evilnife.test", status::DENIED, 0),
    // Not a host name at all.
    case("two words.nife.test", status::BAD_NAME, 0),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// **The table's builder keeps what it is given, and the table names each case once.** `CASES`
    /// is built in a `const`, which line coverage cannot see, so this runs `case` at runtime; and a
    /// name listed twice would have the boot test check one answer twice and another not at all.
    #[test]
    fn every_case_is_built_as_written_and_named_once() {
        let built = core::hint::black_box(case)("packages.nife.test", status::OK, 7);
        assert_eq!(
            (built.name, built.status, built.detail),
            ("packages.nife.test", status::OK, 7)
        );
        for (i, a) in CASES.iter().enumerate() {
            for b in &CASES[i + 1..] {
                assert_ne!(a.name, b.name);
            }
        }
    }
}

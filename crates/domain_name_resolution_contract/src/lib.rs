//! **What a name resolver and the programs around it agree on** (§248 (the name resolver is its
//! own confined program)). The spawner grants each client badge a zone ([`grant_messages`]), and
//! only badge 0 is read as a grant; a client attaches a page, writes a host name into it and
//! `CALL`s [`OPERATION_RESOLVE`]; the resolver judges the name against its [`Grants`] before it
//! sends anything. See notes/name-resolution/the-contract.md.
//!
//! | bytes | field | written by |
//! |---|---|---|
//! | [`OFF_NAME`].. | the host name, dotted ASCII, at most [`NAME_TEXT_MAX`] bytes | the client |
//! | [`OFF_ADDRESSES`].. | up to [`MAX_ADDRESSES`] IPv4 addresses, four bytes each | the resolver |

//! # BUGS
//!
//! - A grant lasts until the spawner sends [`OPERATION_FORGET`]; at most [`GRANTS_MAX`] at once.
//! - One zone per badge: two unrelated domains need two badges, or their common parent.
//! - Grant messages carry eight bytes each (`RECEIVE_CAP` returns two words, one the control word),
//!   so a 253-byte zone is 32 messages, once per client.
//!
//! Name: ratified 2026-10-18 (calef): `domain_name_resolution_contract`, to pair with the program
//! `domain_name_resolver`. Refused `resolver_protocol` (the program, not the act), `dns_protocol`
//! (§154 (the acronym test)) and `domain_name_resolution_protocol` (the suffix is `_contract`).

#![no_std]

mod wire;

pub use wire::*;

/// The resolver's own copy of the bound, checked against the parser's, because `wire` is copied
/// into std's PAL where `domain_name_system` cannot be named.
const _: () = assert!(MAX_ADDRESSES == domain_name_system::MAX_ADDRESSES);

// =================================================================================================
// The resolver's endowment.
// =================================================================================================

/// **What the resolver is told at spawn**, in its three start words. Which name server to ask is
/// the spawner's decision, made from the DHCP lease (`socket_contract::lease`) or from a
/// configuration, never the resolver's, the way the network time client is told its server.
pub mod endowment;

/// **The boot test's cases**, in one place for the two programs that must agree on them: the test
/// client asks each name in order, and the kernel's test (`system_tests/src/user/name_resolver_tests.rs`)
/// judges each reply by its index. `socket_contract::fixture`'s split: the peer that answers,
/// `helpers/name-server-peer`, spells its zone again in Python, so the two sides are written
/// independently.
pub mod fixture;

mod grants;
pub use grants::*;

#[cfg(test)]
mod tests;

mod messages;

pub use messages::{forget_message, grant_messages, reject_code};

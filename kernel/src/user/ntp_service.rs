//! The wiring for the three network time programs: `components/src/network_time_client.rs`,
//! `fixtures/src/network_time_test_server.rs` and `fixtures/src/unwritable_clock_witness.rs`.
//!
//! The client and the witness are endowed by one function, [`spawn_with_client_endowment`], so
//! `an_ntp_client_holds_no_writable_clock_page` tests the client's real capability set. Do not grow
//! a second list of grants here. See notes/ntp.md.

use super::*;
use crate::cap::{Rights, memory_region_cap, memory_region_lending_capability, rendezvous_cap};
use crate::sched::RendezvousId;
use crate::user::holding::Holding;

/// The words the three network time programs report. Must match `RPT_*` in
/// `components/src/network_time_client.rs`, `fixtures/src/network_time_test_server.rs` and
/// `fixtures/src/unwritable_clock_witness.rs`.
///
/// One numbering space across all three. Each program declares only the words it sends; the whole
/// vocabulary is here, so a failing test that prints `3` has a name for it on this side.
#[allow(dead_code)]
pub mod rpt {
    pub const SYNCED: u64 = 1;
    pub const REJECTED: u64 = 2;
    pub(crate) const NO_REPLY: u64 = 3;
    pub const NO_ENTROPY: u64 = 4;
    pub(crate) const NET_ERROR: u64 = 5;
    pub const PROBING: u64 = 6;
    pub const SERVED: u64 = 7;
    pub(crate) const BAD_LOCAL_TIME: u64 = 8;
}

/// Which reply the test server sends. Must match `fixtures/src/network_time_test_server.rs`'s `srv`.
pub mod srv {
    pub const GOOD: u64 = 0;
    pub const BAD_ORIGIN: u64 = 1;
    pub const KISS_OF_DEATH: u64 = 2;
    pub const SHORT: u64 = 3;
}

/// The small integers the client reports a `network_time_protocol::Reject` as. Must match
/// `components/src/network_time_client.rs`'s `reject_code`; kept as distinct values rather than a
/// bool because which check refused a packet is the difference between a broken server and an
/// attack.
pub mod reject {
    pub const LENGTH: u64 = 1;
    pub const KISS_OF_DEATH: u64 = 4;
    pub const ORIGIN_MISMATCH: u64 = 7;
}

/// How many requests the client makes before giving up. Must match `ATTEMPTS` in
/// `components/src/network_time_client.rs`.
pub const ATTEMPTS: u64 = 3;

/// Each of the three mints or maps at most one shared frame and pays for its page tables. Small
/// and fixed: none of them links a heap.
const BUDGET_PAGES: u64 = 16;
/// Extra stack pages: the client builds a 48-byte packet and does IPC, and three is plenty.
const STACK_PAGES: u64 = 3;

/// The endpoints one exchange retypes from the region its caller passes: the server's two and the
/// client's report. Size a region with this and pass the same one to [`start_server`] and
/// [`start_client`] (or [`start_witness`]).
///
/// Name: provisional, minted 2026-10-19 (UTC) by lane milestone/608-service-rendezvous-regions.
pub const EXCHANGE_PAGES: u64 = 3;

/// A running test server: the endpoint a client speaks the socket contract on, and the endpoint
/// it reports the first request it saw on.
pub struct Server {
    /// The socket contract's endpoint. The server holds `READ`; a client is given `WRITE`.
    pub stack: RendezvousId,
    /// Drain this before waiting on the client: the server reports once, with a blocking `send`,
    /// and the client's `RECEIVE` is queued behind it.
    pub report: RendezvousId,
}

/// Extra stack for one of the three, allocated and zeroed. Returned by value so the spawn closure
/// owns it.
fn stack_pages() -> [Mapping; STACK_PAGES as usize] {
    let mut maps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_data(),
    }; STACK_PAGES as usize];
    for (k, m) in maps.iter_mut().enumerate() {
        let phys = crate::memory::alloc_zeroed()
            .expect("no frame for a network time program's stack")
            .addr();
        m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
        m.phys = phys;
    }
    maps
}

/// Spawn `network_time_test_server`: `variant` selects the reply it builds, and `claimed_nanos` is
/// the wall-clock time it claims, supplied by the kernel because the server holds no clock.
///
/// Four slots and no propose or entropy endpoint, so the peer answering the client cannot reach the
/// clock. Both endpoints are retyped from `region` and the thread goes into `held`; the server
/// serves until killed, so the caller hands `region` to the same holding.
pub fn start_server(
    image: &'static [u8],
    variant: u64,
    claimed_nanos: u64,
    region: u64,
    held: &mut Holding,
) -> Server {
    let stack = crate::sched::create_rendezvous_from(region).expect("no ntp server endpoint");
    let report = crate::sched::create_rendezvous_from(region).expect("no ntp server report");
    let budget = crate::memory_region::create(BUDGET_PAGES).expect("no untyped for the ntp server");
    let maps = stack_pages();

    let thread = crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: variant,
                arg1: claimed_nanos,
                arg2: 0,
                grants: &[
                    rendezvous_cap(report, Rights::WRITE), // slot 0: the one report
                    rendezvous_cap(stack, Rights::READ),   // slot 1: serve the socket contract
                    memory_region_cap(budget),             // slot 2: map the client's frame
                    // slot 3: what an opened socket is minted from (§255 (each socket is its own
                    // capability))
                    rendezvous_cap(stack, Rights::WRITE.union(Rights::GRANT)),
                ],
                maps: &maps,
            },
        )
    })
    .expect("could not spawn network_time_test_server");
    held.add_thread(thread);

    Server { stack, report }
}

/// The client's endowment, the one list of grants both `network_time_client` and
/// `unwritable_clock_witness` (`image`) are spawned with, so the witness holds the client's five
/// slots by construction. Returns the report endpoint, retyped from `region`; the thread goes into
/// `held`.
///
/// `a0` and `a1` are the program's two arguments. `entropy` is an `Option` because a client wired
/// without it is the test that the refusal is loud: the empty slot is "no entropy capability".
#[allow(clippy::too_many_arguments)]
fn spawn_with_client_endowment(
    image: &'static [u8],
    a0: u64,
    a1: u64,
    stack: RendezvousId,
    propose: RendezvousId,
    entropy: Option<RendezvousId>,
    region: u64,
    held: &mut Holding,
) -> RendezvousId {
    let report = crate::sched::create_rendezvous_from(region).expect("no ntp client report");
    let budget = crate::memory_region::create(BUDGET_PAGES).expect("no untyped for the ntp client");
    let maps = stack_pages();
    // Slot 4 is granted or it is not; there is no third state and no flag inside it. An
    // ungranted slot answers a `CALL` with `NoSuchSlot`, which is how the client tells "there
    // is no entropy service" from "the service has none".
    let n_grants = if entropy.is_some() { 5 } else { 4 };
    let entropy = entropy.unwrap_or(0);

    let thread = crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: a0,
                arg1: a1,
                arg2: 0,
                grants: &[
                    rendezvous_cap(report, Rights::WRITE),    // slot 0: the verdict
                    rendezvous_cap(stack, Rights::WRITE),     // slot 1: the network
                    memory_region_lending_capability(budget), // slot 2: the shared frame
                    rendezvous_cap(propose, Rights::WRITE),   // slot 3: ask, never tell
                    rendezvous_cap(entropy, Rights::WRITE),   // slot 4: the nonce
                ][..n_grants],
                maps: &maps,
            },
        )
    })
    .expect("could not spawn a network time program");
    held.add_thread(thread);

    report
}

/// Spawn `network_time_client` against `stack`, told which server to ask (`ip` packed big-endian,
/// `port`). Returns its report endpoint.
#[allow(clippy::too_many_arguments)]
pub fn start_client(
    image: &'static [u8],
    stack: RendezvousId,
    propose: RendezvousId,
    entropy: Option<RendezvousId>,
    ip: u32,
    port: u16,
    region: u64,
    held: &mut Holding,
) -> RendezvousId {
    spawn_with_client_endowment(
        image,
        ip as u64,
        port as u64,
        stack,
        propose,
        entropy,
        region,
        held,
    )
}

/// Spawn `unwritable_clock_witness`: the client's endowment, pointed at `va`, through the same
/// function, so the witness cannot drift from what the client holds.
pub fn start_witness(
    image: &'static [u8],
    stack: RendezvousId,
    propose: RendezvousId,
    entropy: Option<RendezvousId>,
    va: u64,
    region: u64,
    held: &mut Holding,
) -> RendezvousId {
    spawn_with_client_endowment(image, va, 0, stack, propose, entropy, region, held)
}

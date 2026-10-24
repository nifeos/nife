#![cfg_attr(not(test), no_std)]
//! The socket contract wire format, shared by `net_stack` and its clients. Each socket is its own
//! capability (§255 (each socket is its own capability)): a process holds a front door that can
//! only make sockets, and one capability per open socket. Opening and `LISTEN` are `CALL`s on the
//! front door answered with the socket's capability; every other operation is a `CALL` on the
//! socket, with an opcode, one argument and one reply word. The stack names a socket only by its
//! kernel-stamped [`SOCKET_BADGE`], so a closed socket's copies reach nothing. A port claim is
//! granted at spawn ([`listen_grant`], [`udp_bind_grant`]), never assumed. See notes/net.md and
//! notes/net/the-socket-contract.md.

//! # BUGS
//!
//! - A grant is the stack's, shared by every holder of its front door; a per-client grant would
//!   ride on a badged front door, which nothing builds yet.
//! - A listener's backlog is one connection: a second arriving before `ACCEPT` is refused by TCP.
//!
//! Name: ratified 2026-08-01 (calef, the naming tenet); `_contract` per calef's 2026-10-18 ruling.

// Operations. The opcode is the whole request word.

/// `SEND_CAP` on a socket's capability: delegate that socket's shared frame.
pub const OPERATION_ATTACH_PAGE_FRAME: u64 = 1;
/// `CALL` on the front door: create a UDP socket and bind an ephemeral local port. Answered
/// [`REP_OK`] with the socket's capability.
pub const OPERATION_OPEN_UDP: u64 = 2;
/// `CALL` on the front door: create a TCP socket. Answered [`REP_OK`] with the socket's capability.
pub const OPERATION_OPEN_TCP: u64 = 3;
/// `CALL`: UDP send; dst in the frame header, payload in the frame.
pub const OPERATION_SENDTO: u64 = 4;
/// `CALL`: block until a datagram/segment arrives, write it to the frame.
pub const OPERATION_RECEIVE: u64 = 5;
/// `CALL`: TCP connect to the frame's dst; reply the outcome.
pub const OPERATION_CONNECT: u64 = 6;
/// `CALL`: TCP send; payload in the frame.
pub const OPERATION_SEND: u64 = 7;
/// `CALL`: close the socket and drop its frame mapping.
pub const OPERATION_CLOSE: u64 = 8;
/// `CALL(front door, OPERATION_LISTEN, port)`: start listening on `port`. Replies one of the
/// [`LISTEN_GRANTED`] outcomes, and a granted one carries the listener's capability. Names
/// provisional (milestone 107 (the socket contract learns to accept)).
pub const OPERATION_LISTEN: u64 = 9;
/// `CALL(listener, OPERATION_ACCEPT, 0)`: block until a connection arrives on this listener, then
/// answer [`REP_OK`] carrying the connection's capability, or [`REP_ERR`]. The listener keeps
/// listening, and the connection wants a frame attached before it carries bytes.
pub const OPERATION_ACCEPT: u64 = 10;
/// `CALL(front door, OPERATION_BIND_UDP, port)`: create a UDP socket bound to the fixed
/// `port`, subject to the stack's [`udp_bind_grant`]. Replies the [`LISTEN_GRANTED`] vocabulary,
/// which is the port-claim vocabulary rather than a TCP one, and a granted one carries the socket's
/// capability. Name provisional (milestone 55 (Time Machine: SMB3 with Apple's extensions, and
/// mDNS)'s mDNS stack half).
pub const OPERATION_BIND_UDP: u64 = 11;

/// The badge bit that marks a socket's capability (§255 (each socket is its own capability);
/// name provisional). The stack mints every socket's badge with this bit set, so a request whose
/// badge has it names a socket and any other badge is a front door. A closed socket's badge stays
/// in the socket range and leaves the stack's table, so it reaches nothing at all.
pub const SOCKET_BADGE: u64 = 1 << 63;

/// Does a request that arrived with `badge` name a socket? `false` means it came through a front
/// door. Name provisional.
///
/// # Examples
///
/// A badge in the socket range names a socket; any other badge is a front door:
///
/// ```
/// use socket_contract::{SOCKET_BADGE, names_a_socket};
///
/// assert!(names_a_socket(SOCKET_BADGE | 1));
/// assert!(!names_a_socket(0)); // the unbadged front door a spawner hands out
/// assert!(!names_a_socket(7)); // a front door a spawner badged for its own reasons
/// ```
pub const fn names_a_socket(badge: u64) -> bool {
    badge & SOCKET_BADGE != 0
}

// Reply words. The connect outcomes are their own small vocabulary, so a client can tell
// "refused" from "connected".

/// Success. Non-negative is success, and `RECEIVE` replies its length here instead.
pub const REP_OK: u64 = 0;
/// `OPERATION_CONNECT` succeeded.
pub const CONNECT_ESTABLISHED: u64 = 0;
/// The peer sent RST, or the connect otherwise failed or closed.
pub const CONNECT_REFUSED: u64 = 1;
/// A failure sentinel (a capability that names no live socket, a bad operation, or a server-side
/// timeout). Bit 32 set, so it is never mistaken for a length or a connect outcome.
pub const REP_ERR: u64 = 1 << 32;

// `LISTEN`'s outcomes, its own small vocabulary because "never granted this port" and "somebody
// else holds it" call for opposite responses: ask for authority, or pick another port.

/// The port claim was granted; the reply carries the new capability.
pub const LISTEN_GRANTED: u64 = 0;
/// The port is outside this stack's [`listen_grant`]. The capability refusal: no retry, no
/// other port in range will help unless the caller was granted it.
pub const LISTEN_DENIED: u64 = 1;
/// The port is inside the grant but another socket already listens there. Exclusivity is the whole
/// reason a port is a grantable thing; this is what enforcing it looks like from the client side.
pub const LISTEN_IN_USE: u64 = 2;

/// No inbound authority at all, and the default a net server is spawned with.
pub const NO_LISTEN_GRANT: u64 = 0;

/// Pack an inclusive listen-port range into the one word a net server is spawned with. `lo == 0` is
/// [`NO_LISTEN_GRANT`] whatever `hi` says, because port 0 is not a port and a grant that starts at
/// "no port" grants nothing; that keeps the "no grant" case a single value a reader can recognise.
///
/// # Examples
///
/// The degenerate cases collapse to one value a reader can recognise:
///
/// ```
/// use socket_contract::{NO_LISTEN_GRANT, listen_grant};
///
/// assert_eq!(listen_grant(0, 65535), NO_LISTEN_GRANT); // port 0 is not a port
/// assert_eq!(listen_grant(8080, 80), NO_LISTEN_GRANT); // an inverted range grants nothing
/// ```
pub const fn listen_grant(lo: u16, hi: u16) -> u64 {
    if lo == 0 || hi < lo {
        return NO_LISTEN_GRANT;
    }
    (lo as u64) | ((hi as u64) << 16)
}

/// May a holder of a stack carrying `grant` listen on `port`? The whole authority check, in one
/// place both the server and its tests read, so a server-side widening cannot pass a test that
/// keeps its own copy of the range.
pub const fn grant_allows(grant: u64, port: u16) -> bool {
    let lo = (grant & 0xffff) as u16;
    let hi = ((grant >> 16) & 0xffff) as u16;
    lo != 0 && port >= lo && port <= hi
}

/// Pack an inclusive UDP bind port range into the high half of the spawn word (milestone 55's
/// mDNS stack half; the [`listen_grant`] holds the low half, and the two compose with `|`). The
/// same degenerate cases as the listen grant, for the same reasons: `lo == 0` grants nothing, and
/// an inverted range is a spawn-site mistake whose safe reading is "grants nothing".
///
/// # Examples
///
/// The two inbound authorities share one word and are independent: UDP 5353 is not TCP 5353.
///
/// ```
/// use socket_contract::{
///     NO_LISTEN_GRANT, grant_allows, listen_grant, udp_bind_grant, udp_grant_allows,
/// };
///
/// // What an mDNS responder is spawned with: one UDP port, no TCP listen authority at all.
/// let responder = udp_bind_grant(5353, 5353);
/// assert!(udp_grant_allows(responder, 5353));
/// assert!(!grant_allows(responder, 5353)); // the same number, and not the same authority
///
/// // What a file server is spawned with: SMB's port, and nothing on the UDP side.
/// let smb = listen_grant(445, 445);
/// assert!(grant_allows(smb, 445));
/// assert!(!udp_grant_allows(smb, 445));
///
/// // Both at once is one word, and neither half leaks into the other.
/// let both = smb | responder;
/// assert!(grant_allows(both, 445) && udp_grant_allows(both, 5353));
/// assert!(!grant_allows(both, 5353) && !udp_grant_allows(both, 445));
///
/// // The default, and what every outbound-only client is spawned with: nothing inbound.
/// assert!(!grant_allows(NO_LISTEN_GRANT, 445));
/// assert!(!udp_grant_allows(NO_LISTEN_GRANT, 5353));
/// ```
pub const fn udp_bind_grant(lo: u16, hi: u16) -> u64 {
    if lo == 0 || hi < lo {
        return NO_LISTEN_GRANT;
    }
    ((lo as u64) << 32) | ((hi as u64) << 48)
}

/// May a holder of a stack carrying `grant` bind a UDP socket to the fixed `port`? [`grant_allows`]
/// for the word's UDP half; the two halves are independent authorities, so granting a TCP listen
/// range grants no UDP port and the reverse.
pub const fn udp_grant_allows(grant: u64, port: u16) -> bool {
    let lo = ((grant >> 32) & 0xffff) as u16;
    let hi = ((grant >> 48) & 0xffff) as u16;
    lo != 0 && port >= lo && port <= hi
}

pub mod polled_wait {
    //! Whether `net_stack` may sleep while it waits on a NIC that raises no interrupt (the
    //! `e1000e` and radon's `DesignWare` port; `components/src/nic_wait.rs`). For one scheduler
    //! tick after the last frame the stack yields and looks at the ring again, and it sleeps only
    //! once a whole tick has passed with nothing, because a timed wait ends at a tick and a sleep
    //! would add up to a tick to every round trip. Here so its claim is a host test; only
    //! `net_stack` uses it. The module's name is provisional (milestone 53 (the board's own
    //! peripherals)). See notes/net/the-socket-contract.md.

    //! # BUGS
    //!
    //! - A core is busy while a stream flows, and for a tick after it ends: each look yields, but
    //!   an idle core does not idle. A NIC interrupt or a finer timed wait would fix it.

    /// What a polled NIC's wait does next.
    ///
    /// Name: provisional, minted 2026-10-21 (UTC) by milestone 53's receive-rate lane.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Next {
        /// Yield, then look at the ring again: a frame came within the last tick, so another is
        /// likely sooner than a sleep could end.
        Look,
        /// Sleep this many microseconds (the stack's own next timer or the poll interval, whichever
        /// is sooner), then look. Zero means look now.
        Sleep(u64),
    }

    /// The wait at counter time `now`, when the last frame was taken at `last_frame` and a
    /// scheduler tick is `tick` counter ticks. `stack_due_micros` is when the stack next needs
    /// polling for its own timers, if ever; `poll_micros` is the longest a polled NIC goes
    /// unlooked-at.
    ///
    /// Name: provisional, minted 2026-10-21 (UTC) by milestone 53's receive-rate lane.
    pub const fn next(
        now: u64,
        last_frame: u64,
        tick: u64,
        stack_due_micros: Option<u64>,
        poll_micros: u64,
    ) -> Next {
        if now.wrapping_sub(last_frame) < tick {
            return Next::Look;
        }
        match stack_due_micros {
            Some(due) if due < poll_micros => Next::Sleep(due),
            _ => Next::Sleep(poll_micros),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const TICK: u64 = 40_000; // radon: a 4 MHz counter at 100 Hz

        /// The regression: any wait inside a tick of the last frame sleeps through a tick instead.
        #[test]
        fn within_a_tick_of_a_frame_the_stack_never_sleeps() {
            let last = 1_000_000;
            for since in [0, 1, TICK / 2, TICK - 1] {
                assert_eq!(next(last + since, last, TICK, None, 1000), Next::Look);
            }
        }

        /// The cost side: an idle link must reach a sleep, or the stack spins a core forever while
        /// a client waits on a listener nobody connects to.
        #[test]
        fn a_tick_after_the_last_frame_the_stack_sleeps_no_longer_than_its_timers_allow() {
            let last = 1_000_000;
            assert_eq!(next(last + TICK, last, TICK, None, 1000), Next::Sleep(1000));
            assert_eq!(
                next(last + TICK, last, TICK, Some(300), 1000),
                Next::Sleep(300)
            );
            assert_eq!(next(last + TICK, last, TICK, Some(0), 1000), Next::Sleep(0));
            // Before any frame at all, a stack that has been up a tick sleeps too.
            assert_eq!(next(5 * TICK, 0, TICK, Some(5000), 1000), Next::Sleep(1000));
        }
    }
}

pub mod ephemeral {
    //! Which local port `net_stack` gives an outbound socket: the private ephemeral range, walked
    //! by a cursor that never falls behind the boot clock. An allocation takes the later of the
    //! cursor and a clock advancing [`PER_SECOND`] ports a second, so a fresh stack starts past the
    //! ports an earlier one used, which a peer may still hold in `TIME_WAIT`. Here so its claim is
    //! a host test; only `net_stack` uses it. The module's name is provisional. See
    //! notes/net/the-socket-contract.md and notes/net/the-outbound-gates.md.

    //! # BUGS
    //!
    //! - The bound is a rate, not a proof: a stack replaced quickly can hand its successor a port.
    //! - The range wraps every 64 s, inside a longer `TIME_WAIT` such as Windows' 240 s.
    //! - The clock is boot-relative, so a rebooted machine's first ports repeat the last boot's.
    //! - The sequence-number half stays smoltcp's: a reused 4-tuple still fails half the time.

    /// The lowest port of the private ephemeral range (RFC 6335).
    pub const LO: u16 = 49152;
    /// The highest.
    pub const HI: u16 = 65535;
    /// How many ports the range holds.
    pub const SPAN: u64 = (HI - LO) as u64 + 1;
    /// How far the clock moves the cursor each second. 256 makes one tick about 4 ms, so a stack
    /// started a few milliseconds after another's last open already starts past it, and keeps the
    /// wrap at 64 s, above the 60 s `TIME_WAIT` of Linux and the BSDs.
    pub const PER_SECOND: u64 = 256;

    // The wrap stays at or above the 60 s `TIME_WAIT` the BUGS section names, checked at compile
    // time.
    const _: () = assert!(SPAN * 1000 / PER_SECOND >= 60_000);

    /// The cursor. It counts positions without wrapping; a position's port is its remainder in the
    /// range. A new one has handed out nothing, so its first allocation starts at the clock.
    pub struct Ports {
        next: u64,
    }

    impl Ports {
        /// A cursor that has handed out nothing.
        pub const fn new() -> Self {
            Self { next: 0 }
        }

        /// The next port at `millis` on the boot clock that `held` does not say a live socket
        /// holds. Bounded by the range, so it always returns; a stack holds far fewer sockets than
        /// the range has ports, so in practice the first or second candidate is free.
        pub fn next(&mut self, millis: u64, held: impl Fn(u16) -> bool) -> u16 {
            self.next = self.next.max(clock(millis));
            for _ in 0..SPAN {
                let port = port_at(self.next);
                self.next += 1;
                if !held(port) {
                    return port;
                }
            }
            port_at(self.next)
        }
    }

    impl Default for Ports {
        fn default() -> Self {
            Self::new()
        }
    }

    /// The clock's position at `millis`.
    fn clock(millis: u64) -> u64 {
        millis.saturating_mul(PER_SECOND) / 1000
    }

    fn port_at(position: u64) -> u16 {
        LO + (position % SPAN) as u16
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Hand out `n` ports from a fresh cursor, `gap_ms` apart, starting at `start_ms`. Returns
        /// the ports and the time of the last open.
        fn run(start_ms: u64, n: u64, gap_ms: u64) -> (std::vec::Vec<u16>, u64) {
            let mut ports = Ports::new();
            let mut at = start_ms;
            let mut got = std::vec::Vec::new();
            for _ in 0..n {
                got.push(ports.next(at, |_| false));
                at += gap_ms;
            }
            (got, at - gap_ms)
        }

        /// A stack started after another hands out none of its ports. The shapes are the name
        /// resolver tests': fourteen opens 50 ms apart, then a new stack whose first open comes
        /// every whole number of milliseconds from one tick (4 ms) to 2 s later (the measured gap
        /// is about 230 ms on riscv64), at a spread of boot times. Seeding the cursor once from
        /// the counter fails this.
        ///
        /// Falsification: replayable `crates/socket_contract/falsifications/ephemeral.tests.a_stack_started_after_another_reuses_none_of_its_ports.patch`
        #[test]
        fn a_stack_started_after_another_reuses_none_of_its_ports() {
            for start in [0, 1, 7_919, 123_457, 3_600_001] {
                let (first, last) = run(start, 14, 50);
                for gap in 4..=2000 {
                    let (second, _) = run(last + gap, 14, 50);
                    for p in &second {
                        assert!(
                            !first.contains(p),
                            "boot time {start} ms, gap {gap} ms: port {p} reused from {first:?}",
                        );
                    }
                }
            }
        }

        /// One stack never hands out a port twice before the range has cycled, however fast it
        /// opens: a burst runs ahead of the clock rather than repeating it.
        #[test]
        fn a_burst_runs_ahead_of_the_clock_and_repeats_nothing() {
            let mut ports = Ports::new();
            let mut seen = std::collections::BTreeSet::new();
            for _ in 0..SPAN {
                assert!(seen.insert(ports.next(1_000, |_| false)));
            }
        }

        /// The cursor walks the whole private range and nothing outside it (RFC 6335's
        /// 49152 to 65535), in order, and wraps to its bottom. Spelled as numbers, because the
        /// other tests compare ports only with each other and would pass on any range.
        #[test]
        fn the_cursor_walks_49152_to_65535_and_wraps() {
            let mut ports = Ports::new();
            for want in 49152..=65535u16 {
                assert_eq!(ports.next(0, |_| false), want);
            }
            assert_eq!(ports.next(0, |_| false), 49152);
        }

        /// A port a live socket holds is skipped, and the cursor moves past it.
        #[test]
        fn a_held_port_is_skipped() {
            let mut ports = Ports::new();
            let at = clock(10_000);
            let held = port_at(at);
            assert_eq!(ports.next(10_000, |p| p == held), port_at(at + 1));
            assert_eq!(ports.next(10_000, |p| p == held), port_at(at + 2));
        }
    }
}

/// Where a network stack finds the two capabilities §255 (each socket is its own capability)
/// added to its spawn, which its spawners (the progenitor, and the kernel's test harness) and the
/// stack itself must agree on, so they are here under rule 7 rather than spelled per program. Names
/// provisional.
pub mod stack_slots {
    /// The stack's own endpoint again, `WRITE | GRANT`: what each socket's capability is minted
    /// from with `BADGE`, which keeps the source's rights. Not the `READ` copy the stack serves on,
    /// because a socket minted from that could take other clients' requests.
    pub const MINT: u64 = 7;
    /// The stack's own address space, `WRITE`: so `CLOSE` can `UNMAP` a socket's page before its
    /// table entry takes another client's.
    pub const OWN_SPACE: u64 = 8;
}

/// The destination address's offset in the `PageFrame`: the first header field. The frame is
/// one region reused per synchronous `CALL`, not a ring.
///
/// ```text
///   +0x000  u8[4]  dst_ip    destination (SENDTO / CONNECT); source on a UDP RECEIVE reply
///   +0x004  u16    dst_port  little-endian, the same two roles
///   +0x006  u16    len       payload length, in for SEND*, out for RECEIVE
///   +0x008  ...    payload   up to DATA_MAX bytes
/// ```
pub const OFF_DST_IP: u64 = 0x000;
/// The destination port, right after the destination IP.
pub const OFF_DST_PORT: u64 = 0x004;
/// The payload length.
pub const OFF_LEN: u64 = 0x006;
/// Where the payload bytes start.
pub const OFF_PAYLOAD: u64 = 0x008;

/// The largest payload the frame carries (a 4 KiB frame minus the header).
pub const DATA_MAX: usize = 4096 - OFF_PAYLOAD as usize;

/// The inbound gate's shared facts, for the four programs that must agree on them (rule 7). The
/// host's copy is deliberately not here: `xtask`'s prober and `helpers/qemu-runner-*.sh` spell
/// them again, so the gate proves two independently written sides agree. See
/// notes/net/the-socket-contract.md.
pub mod fixture {
    /// The port the inbound gate listens on, and the guest side of the runners' `hostfwd`. Both
    /// ISA legs use the same number because they run one after the other and never hold it at once.
    pub const LISTEN_PORT: u16 = 7778;
    /// A port deliberately outside every listen grant the tests hand out, so asking for it
    /// proves the grant *refuses* rather than that nothing happened to bind. It is the whole of the
    /// authority claim: a client that could bind this bound a port nothing granted.
    pub const DENIED_PORT: u16 = 8080;

    /// What the host sends in, and what the guest answers with. Different strings on purpose: an
    /// echo would pass even if the guest were only reflecting the host's own bytes, and the point
    /// of the gate is that the guest *composed* an answer to a connection it did not make.
    pub const IN_MSG: &[u8] = b"nife-in!";
    /// What the guest answers with, composed rather than echoed (see the field-group docs).
    pub const OUT_MSG: &[u8] = b"nife-out!";

    /// How many connections one listener serves before the client stops. Two, and the second is
    /// the load-bearing one: a listener that accepts exactly one connection would pass a
    /// one-round gate and is precisely what a file server cannot use.
    pub const ROUNDS: usize = 2;

    /// The runners' TCP echo peer: a `guestfwd` inside QEMU's user-mode network that pipes every
    /// connection to `10.0.2.9:7777` into a fresh `/bin/cat` (`helpers/qemu-runner-aarch64.sh`,
    /// and its riscv64 twin). Two binaries dial it, so it is here; the runners spell it a third
    /// time for [`LISTEN_PORT`]'s reason.
    pub const ECHO_PEER_IP: [u8; 4] = [10, 0, 2, 9];
    /// The echo peer's port; see [`ECHO_PEER_IP`].
    pub const ECHO_PEER_PORT: u16 = 7777;

    /// The runners' package source: a `guestfwd` that runs `helpers/package-http-peer` once per
    /// connection to `10.0.2.9:8080`, serving packages over HTTP/1.0 (milestone 198 (a package
    /// manager) rung 3a). `socket_test_client` and `jig install <name>` dial it (rule 7). Only
    /// reachable inside a QEMU runner (notes/packages.md's BUGS).
    pub const PACKAGE_PEER_IP: [u8; 4] = [10, 0, 2, 9];
    /// The package source's port; see [`PACKAGE_PEER_IP`]. The same number as [`DENIED_PORT`], and
    /// unrelated to it: that is a port a *guest* may not listen on, this is one on a *host* the
    /// guest dials.
    pub const PACKAGE_PEER_PORT: u16 = 8080;
    /// The `Host` header a request to the package source carries: [`PACKAGE_PEER_IP`], spelled.
    pub const PACKAGE_PEER_HOST: &str = "10.0.2.9";
}

/// The lease report `net_stack` sends its spawner once DHCP completes, provisional, as is the name
/// (milestone 384 (in a capability system the resolver is a grant)). Word 0 is the leased IPv4
/// address, big-endian in the low 32 bits; word 1 the first DNS server the lease named, for the
/// resolver's spawner (§248 (the name resolver is its own confined program)), the same encoding or
/// [`lease::NO_NAMESERVER`]; word 2 is zero and reserved. See notes/net/the-socket-contract.md.
///
/// ```
/// use socket_contract::lease;
///
/// let word = lease::ipv4_word([10, 0, 2, 3]);
/// assert_eq!(lease::word_ipv4(word), Some([10, 0, 2, 3]));
/// assert_eq!(lease::word_ipv4(lease::NO_NAMESERVER), None);
/// ```
pub mod lease {
    /// Word 1 when the lease named no DNS server. 0.0.0.0 is never a nameserver, so it cannot
    /// collide with one.
    pub const NO_NAMESERVER: u64 = 0;

    /// An IPv4 address as a report word: big-endian in the low 32 bits.
    pub const fn ipv4_word(octets: [u8; 4]) -> u64 {
        u32::from_be_bytes(octets) as u64
    }

    /// A report word back to an address, or `None` for [`NO_NAMESERVER`] or a word with any of the
    /// high 32 bits set, which no encoder here produces.
    pub const fn word_ipv4(word: u64) -> Option<[u8; 4]> {
        if word == NO_NAMESERVER || word > u32::MAX as u64 {
            return None;
        }
        Some((word as u32).to_be_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lease_word_round_trips_and_refuses_what_no_encoder_writes() {
        assert_eq!(lease::ipv4_word([10, 0, 2, 3]), 0x0a00_0203);
        assert_eq!(lease::word_ipv4(0x0a00_0203), Some([10, 0, 2, 3]));
        assert_eq!(lease::word_ipv4(lease::NO_NAMESERVER), None);
        assert_eq!(lease::word_ipv4(1 << 32), None);
        assert_eq!(lease::word_ipv4(u32::MAX as u64), Some([255; 4]));
    }

    /// Every opcode the contract defines, in one place: a list repeated per property will one day
    /// be missing an entry in exactly the property that would have caught it.
    const OPERATIONS: &[u64] = &[
        OPERATION_ATTACH_PAGE_FRAME,
        OPERATION_OPEN_UDP,
        OPERATION_OPEN_TCP,
        OPERATION_SENDTO,
        OPERATION_RECEIVE,
        OPERATION_CONNECT,
        OPERATION_SEND,
        OPERATION_CLOSE,
        OPERATION_LISTEN,
        OPERATION_ACCEPT,
        OPERATION_BIND_UDP,
    ];

    /// Opcodes are distinct from each other and from zero, and stay small. The request word is the
    /// opcode alone now, so a wide opcode costs nothing on the wire, but zero is what an empty
    /// word reads as and must never mean an operation.
    #[test]
    fn every_opcode_is_nonzero_and_small() {
        for &operation in OPERATIONS {
            assert!(operation != 0 && operation <= 0xff, "opcode {operation}");
        }
    }

    /// Opcodes are distinct. Two sharing a number is one operation silently performing another.
    #[test]
    fn opcodes_are_distinct() {
        for (i, a) in OPERATIONS.iter().enumerate() {
            for b in &OPERATIONS[i + 1..] {
                assert_ne!(a, b, "two opcodes share a number");
            }
        }
    }

    /// The socket range and the front door never overlap. A badge either names a socket or is
    /// a front door; if one value could be both, a closed socket's capability would open new
    /// sockets, or a front door would reach someone's socket.
    #[test]
    fn a_badge_is_a_socket_or_a_front_door_and_never_both() {
        assert!(!names_a_socket(0), "the unbadged front door names a socket");
        for b in [1u64, 7, 1 << 62, (1 << 63) - 1] {
            assert!(!names_a_socket(b), "front door badge {b:#x} names a socket");
        }
        for b in [SOCKET_BADGE, SOCKET_BADGE | 1, u64::MAX] {
            assert!(
                names_a_socket(b),
                "socket badge {b:#x} reads as a front door"
            );
        }
    }

    /// The shared frame's header fields do not overlap, and the payload starts after all of them.
    /// An overlap here means a destination port written over an address, which would send bytes to
    /// the wrong host rather than failing.
    #[test]
    fn the_frame_header_fields_do_not_overlap() {
        let mut offs: [u64; 3] = [OFF_DST_IP, OFF_DST_PORT, OFF_LEN];
        offs.sort_unstable();
        for w in offs.windows(2) {
            assert_ne!(w[0], w[1], "two header fields share an offset");
        }
        for o in offs {
            assert!(o < OFF_PAYLOAD, "header field {o} runs into the payload");
        }
    }

    /// The payload is exactly the rest of the frame: the header, then bytes to the last one on the
    /// page. Larger and a full-size send writes past the end of the frame the client was granted;
    /// smaller and part of a granted page is unreachable, which nothing anywhere would report.
    #[test]
    fn a_full_payload_fills_the_shared_frame_exactly() {
        assert_eq!(
            OFF_PAYLOAD as usize + DATA_MAX,
            4096,
            "the payload is not the whole frame minus its header"
        );
    }

    /// The connect outcomes are distinguishable from each other and from the generic reply codes,
    /// which is the whole reason they have their own vocabulary.
    #[test]
    fn connect_outcomes_are_their_own_vocabulary() {
        assert_ne!(CONNECT_ESTABLISHED, CONNECT_REFUSED);
        assert_ne!(CONNECT_ESTABLISHED, REP_ERR);
        assert_ne!(CONNECT_REFUSED, REP_ERR);
    }

    /// The same for `LISTEN`'s three outcomes, and for the same reason: a client that cannot tell
    /// a refusal of *authority* from a collision on a port cannot do the right thing about either.
    #[test]
    fn listen_outcomes_are_their_own_vocabulary() {
        let outcomes = [LISTEN_GRANTED, LISTEN_DENIED, LISTEN_IN_USE, REP_ERR];
        for (i, a) in outcomes.iter().enumerate() {
            for b in &outcomes[i + 1..] {
                assert_ne!(a, b, "two listen outcomes share a value");
            }
        }
    }

    /// A grant admits exactly its range and nothing else. This is the authority check itself,
    /// so an off-by-one here is a server binding a port it was never granted, which no other test
    /// in the tree would see: the on-device gate asks for one allowed port and one denied one.
    #[test]
    fn a_listen_grant_admits_its_range_and_refuses_everything_outside_it() {
        let g = listen_grant(7778, 7780);
        assert!(!grant_allows(g, 7777), "one below the range was allowed");
        for p in 7778..=7780 {
            assert!(grant_allows(g, p), "port {p} inside the range was refused");
        }
        assert!(!grant_allows(g, 7781), "one above the range was allowed");
        assert!(
            !grant_allows(g, 80),
            "a well-known port outside was allowed"
        );

        // A single-port grant is the common case (one server, one port) and the tightest one.
        let one = listen_grant(80, 80);
        assert!(grant_allows(one, 80));
        assert!(!grant_allows(one, 81));
    }

    /// No grant means no port. The default a server is spawned with must admit nothing, or
    /// "inbound authority is granted, never assumed" is a sentence in a doc comment and not a
    /// property of the system. Port 0 is refused under every grant, including one that names it,
    /// because it is not a port anything can listen on.
    #[test]
    fn the_default_grant_admits_no_port_at_all() {
        for p in [0u16, 1, 80, 7778, 65535] {
            assert!(
                !grant_allows(NO_LISTEN_GRANT, p),
                "port {p} was allowed with no grant"
            );
        }
        assert_eq!(listen_grant(0, 65535), NO_LISTEN_GRANT);
        assert!(!grant_allows(listen_grant(0, 65535), 7778));
        assert!(!grant_allows(listen_grant(1, 65535), 0));
        // An inverted range is a mistake at the spawn site, and the safe reading of a mistake in a
        // grant is "grants nothing", never "grants the ports between them in the other order".
        assert_eq!(listen_grant(7780, 7778), NO_LISTEN_GRANT);
    }

    /// The UDP bind grant admits exactly its range, the twin of the listen-grant test above
    /// and for the same reason: this is the authority check itself, and the on-device gate asks
    /// for one allowed port and one denied one, so an off-by-one here is a server binding a UDP
    /// port it was never granted with no other test to see it.
    #[test]
    fn a_udp_bind_grant_admits_its_range_and_refuses_everything_outside_it() {
        let g = udp_bind_grant(5353, 5355);
        assert!(
            !udp_grant_allows(g, 5352),
            "one below the range was allowed"
        );
        for p in 5353..=5355 {
            assert!(
                udp_grant_allows(g, p),
                "port {p} inside the range was refused"
            );
        }
        assert!(
            !udp_grant_allows(g, 5356),
            "one above the range was allowed"
        );

        // The single-port grant is the one milestone 55 actually spawns (mDNS's 5353).
        let one = udp_bind_grant(5353, 5353);
        assert!(udp_grant_allows(one, 5353));
        assert!(!udp_grant_allows(one, 5354));

        // The degenerate cases read as "grants nothing", exactly as the listen grant's do.
        assert_eq!(udp_bind_grant(0, 65535), NO_LISTEN_GRANT);
        assert_eq!(udp_bind_grant(5355, 5353), NO_LISTEN_GRANT);
        for p in [0u16, 1, 5353, 65535] {
            assert!(
                !udp_grant_allows(NO_LISTEN_GRANT, p),
                "port {p} allowed with no grant"
            );
        }
    }

    /// The two grants share one word and neither leaks into the other. They are different
    /// authorities (receive TCP connections on a port, versus claim a fixed UDP port), so a spawn
    /// that grants one must not have granted the other, and the composed word must answer each
    /// question from its own half only. A packing overlap here is a stack granting mDNS's port to
    /// anything that was granted a TCP listen range, silently.
    #[test]
    fn the_listen_and_udp_bind_grants_compose_without_leaking() {
        let listen_only = listen_grant(7778, 7778);
        assert!(grant_allows(listen_only, 7778));
        assert!(
            !udp_grant_allows(listen_only, 7778),
            "a listen grant granted a UDP bind"
        );

        let udp_only = udp_bind_grant(5353, 5353);
        assert!(udp_grant_allows(udp_only, 5353));
        assert!(
            !grant_allows(udp_only, 5353),
            "a UDP bind grant granted a TCP listen"
        );

        let both = listen_grant(7778, 7778) | udp_bind_grant(5353, 5353);
        assert!(grant_allows(both, 7778));
        assert!(udp_grant_allows(both, 5353));
        assert!(
            !grant_allows(both, 5353),
            "the UDP half answered a listen question"
        );
        assert!(
            !udp_grant_allows(both, 7778),
            "the listen half answered a UDP question"
        );

        // The top of the port space survives the high half's packing.
        let top = udp_bind_grant(65535, 65535);
        assert!(udp_grant_allows(top, 65535));
        assert!(!udp_grant_allows(top, 65534));
    }

    /// The inbound fixture's own claim, checked: the port the gate is granted is inside the
    /// grant the tests hand out and the port it probes is outside it. Both device legs spend
    /// minutes proving the refusal on real hardware-shaped paths, and neither of them would notice
    /// if somebody edited [`fixture::DENIED_PORT`] to a number inside the range: the client would
    /// bind it, print that it bound it, and every assertion would still pass. This is the check
    /// that a "denied" port is denied.
    #[test]
    fn the_inbound_fixture_probes_a_port_no_test_grants() {
        let g = listen_grant(fixture::LISTEN_PORT, fixture::LISTEN_PORT);
        assert!(grant_allows(g, fixture::LISTEN_PORT));
        assert!(
            !grant_allows(g, fixture::DENIED_PORT),
            "the port the gate calls denied is inside the grant it is handed",
        );
        // And the wider range the combined inbound boot hands out (the SMB port rides beside the
        // listen port) must not reach it either.
        let wide = listen_grant(fixture::LISTEN_PORT, fixture::LISTEN_PORT + 1);
        assert!(!grant_allows(wide, fixture::DENIED_PORT));
        // The default admits neither, which is what "granted, never assumed" means for the fixture.
        assert!(!grant_allows(NO_LISTEN_GRANT, fixture::LISTEN_PORT));
    }

    /// A grant round-trips through the one word a spawn argument carries, up to the top of the
    /// port space. It rides in a `u64` beside nothing else, so the only way to lose a bit is to
    /// pack it wrong.
    #[test]
    fn a_grant_survives_the_word_it_is_spawned_with() {
        for (lo, hi) in [
            (1u16, 65535u16),
            (49152, 65535),
            (65535, 65535),
            (7778, 7778),
        ] {
            let g = listen_grant(lo, hi);
            assert!(grant_allows(g, lo), "the low end was lost for {lo}..={hi}");
            assert!(grant_allows(g, hi), "the high end was lost for {lo}..={hi}");
        }
    }
}

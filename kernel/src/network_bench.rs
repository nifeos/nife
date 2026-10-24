//! The network bench boot, behind the `network_bench` feature (name provisional), for exit
//! criterion 2 of milestone 494 (a driver for the network card a PC actually has).
//!
//! [`run`] replaces the hand-over: it brings up the NIC (or, on a JH7110, the Ethernet port),
//! preflights the condition that makes its numbers mean anything, waits for a DHCP lease, times one
//! TCP transfer from the peer the image was built for, prints a `network-bench:` verdict and halts.
//! See notes/bench-boots.md.

//! # BUGS
//! - The DHCP wait has no bound; a boot with no lease ends on the `waiting` line.
//! - The peer is baked into the image (`NIFE_NETWORK_BENCH_PEER`); another peer is a rebuild.

use crate::user::e1000e_service::{self as service, Absent};
use crate::{arch, println};

/// The peer, `a.b.c.d:port`, from the build's environment; `None` builds a boot that leases and
/// stops.
const PEER: Option<&str> = option_env!("NIFE_NETWORK_BENCH_PEER");

/// Run the bench and never come back.
pub fn run() -> ! {
    println!();
    println!(
        "network-bench: milestone 494 bench boot. {} build, {} Hz counter, peer {}.",
        if cfg!(debug_assertions) {
            "DEBUG"
        } else {
            "release"
        },
        arch::timer::frequency(),
        PEER.unwrap_or("none"),
    );
    let verdict = measure();
    println!("network-bench: verdict {verdict}");
    println!("network-bench: done, halting.");
    arch::halt(arch::HaltReason::measurement_boot());
}

fn measure() -> &'static str {
    let Some(image) = crate::trust::require_program("net_stack") else {
        return "FAILED: no net_stack in the initial ramdisk, or its bytes are not the measured ones";
    };
    let peer = match PEER.map(parse_peer) {
        Some(Some(p)) => Some(p),
        Some(None) => return "FAILED: NIFE_NETWORK_BENCH_PEER is not a.b.c.d:port",
        None => None,
    };
    #[cfg(target_arch = "riscv64")]
    if crate::memory::jh7110_ethernet().is_some() {
        return radon::measure(image, peer);
    }
    let started = match peer {
        Some(word) => service::start_drain(image, word).map(|(cli, w)| (Some(cli), w)),
        None => {
            service::start_net_server(image, socket_contract::NO_LISTEN_GRANT).map(|w| (None, w))
        }
    };
    let (client, w) = match started {
        Ok(v) => v,
        Err(Absent::NoController) => return "FAILED: no NIC this driver claims is on the bus",
        Err(Absent::Refused { rid, why }) => {
            println!("network-bench: nic       : rid {rid:#06x} refused at bring-up: {why:?}");
            return "FAILED: the NIC is on the bus and bring-up refused it";
        }
        Err(Absent::Busy) => return "FAILED: a server already holds the NIC",
        Err(Absent::NoLink { device }) => {
            println!(
                "network-bench: nic       : 8086:{device:04x} {}, link NOT UP",
                ::e1000e::model(device).unwrap_or("?"),
            );
            return "FAILED: no link (autonegotiation did not finish in the link wait; check the \
                    cable and the port's LEDs)";
        }
    };
    let m = w.mac;
    println!(
        "network-bench: nic       : 8086:{:04x} {}, link up, mac {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        w.device,
        ::e1000e::model(w.device).unwrap_or("?"),
        m[0],
        m[1],
        m[2],
        m[3],
        m[4],
        m[5],
    );
    println!("network-bench: phy       : id {:#010x}", w.phy_id);
    println!(
        "network-bench: preflight dmar scope : {}",
        if w.confined_by_iommu { "PASS" } else { "FAIL" }
    );
    // With a client, `start_drain` drained the lease before returning; without one, take it here.
    let Some(cli) = client else {
        println!(
            "network-bench: dhcp      : waiting for a lease (no bound: a last line here is the answer)"
        );
        let o = (crate::sched::ipc_receive(w.report)[0] as u32).to_be_bytes();
        println!(
            "network-bench: dhcp      : leased {}.{}.{}.{}",
            o[0], o[1], o[2], o[3]
        );
        return if w.confined_by_iommu {
            "NO-PEER: leased, nothing to measure (build with NIFE_NETWORK_BENCH_PEER)"
        } else {
            "UNCONFINED: leased, but no IOMMU unit this kernel programmed owns the NIC"
        };
    };
    let o = w.lease.to_be_bytes();
    println!(
        "network-bench: dhcp      : leased {}.{}.{}.{}",
        o[0], o[1], o[2], o[3]
    );
    let [code, bytes, ticks, ..] = crate::sched::ipc_receive(cli);
    if code != 1 {
        println!("network-bench: transfer  : client stopped with code {code:#x}");
        return "FAILED: the transfer did not complete (see socket_test_client for the code)";
    }
    print_transfer(bytes, ticks);
    if bytes == 0 {
        "FAILED: connected and received nothing"
    } else if w.confined_by_iommu {
        "LEASED-AND-MEASURED"
    } else {
        "UNCONFINED: measured, but no IOMMU unit this kernel programmed owns the NIC"
    }
}

/// The `transfer` line, the same on every NIC so two machines' transcripts compare line for line.
fn print_transfer(bytes: u64, ticks: u64) {
    let hz = arch::timer::frequency();
    let micros = ticks.saturating_mul(1_000_000) / hz.max(1);
    let rate = bytes.saturating_mul(8).saturating_mul(1_000_000) / micros.max(1);
    println!(
        "network-bench: transfer  : {bytes} bytes in {}.{:03} s = {}.{:03} Mbit/s",
        micros / 1_000_000,
        (micros / 1000) % 1000,
        rate / 1_000_000,
        (rate / 1000) % 1000,
    );
}

/// `a.b.c.d:port` as the drain client's word, `ipv4 << 16 | port`.
fn parse_peer(s: &str) -> Option<u64> {
    let (ip, port) = s.split_once(':')?;
    let port: u16 = port.parse().ok()?;
    let mut word: u64 = 0;
    let mut n = 0;
    for part in ip.split('.') {
        word = word << 8 | u64::from(part.parse::<u8>().ok()?);
        n += 1;
    }
    (n == 4).then_some(word << 16 | u64::from(port))
}

/// The radon arm: the same boot over the JH7110's Ethernet port, its compared lines spelled as the
/// `e1000e` arm spells them.
#[cfg(target_arch = "riscv64")]
mod radon {
    use ::designware_ethernet::coherence::Verdict;
    use ::designware_ethernet::jh7110;

    use crate::designware_ethernet::{Absent, Report, last_report};
    use crate::println;
    use crate::user::designware_ethernet_service::{self as service, NotStarted};

    pub(super) fn measure(image: &'static [u8], peer: Option<u64>) -> &'static str {
        let started = match peer {
            Some(word) => service::start_drain(image, word).map(|(cli, w)| (Some(cli), w)),
            None => service::start_net_server(image, socket_contract::NO_LISTEN_GRANT)
                .map(|w| (None, w)),
        };
        let (client, w) = match started {
            Ok(v) => v,
            Err(NotStarted::Absent(Absent::NoController)) => {
                return "FAILED: the tree names no JH7110 Ethernet port";
            }
            Err(NotStarted::Absent(Absent::Refused(why))) => {
                print(last_report().as_ref());
                println!("network-bench: nic       : refused at bring-up: {why:?}");
                return "FAILED: the port is described and bring-up refused it";
            }
            Err(NotStarted::NoLink) => {
                print(last_report().as_ref());
                return "FAILED: no link (the port came up; check the cable and the port it is in)";
            }
            Err(NotStarted::Busy) => return "FAILED: a server already holds the port",
        };
        let report = last_report();
        print(report.as_ref());
        let coherent = matches!(
            report.and_then(|r| r.coherence),
            Some((Verdict::Coherent, _))
        );
        let Some(cli) = client else {
            println!(
                "network-bench: dhcp      : waiting for a lease (no bound: a last line here is the answer)"
            );
            let o = (crate::sched::ipc_receive(w.report)[0] as u32).to_be_bytes();
            println!(
                "network-bench: dhcp      : leased {}.{}.{}.{}",
                o[0], o[1], o[2], o[3]
            );
            return if coherent {
                "NO-PEER: leased, nothing to measure (build with NIFE_NETWORK_BENCH_PEER)"
            } else {
                "INCONCLUSIVE-COHERENCE: leased, but the probe did not run to a verdict"
            };
        };
        let o = w.lease.to_be_bytes();
        println!(
            "network-bench: dhcp      : leased {}.{}.{}.{}",
            o[0], o[1], o[2], o[3]
        );
        let [code, bytes, ticks, ..] = crate::sched::ipc_receive(cli);
        if code != 1 {
            println!("network-bench: transfer  : client stopped with code {code:#x}");
            return "FAILED: the transfer did not complete (see socket_test_client for the code)";
        }
        super::print_transfer(bytes, ticks);
        if bytes == 0 {
            "FAILED: connected and received nothing"
        } else if coherent {
            "LEASED-AND-MEASURED"
        } else {
            "INCONCLUSIVE-COHERENCE: measured, but the probe did not run to a verdict"
        }
    }

    /// Every line of the bring-up's own account, in the order it happened.
    fn print(r: Option<&Report>) {
        let Some(r) = r else {
            println!("network-bench: nic       : no bring-up report was kept");
            return;
        };
        println!(
            "network-bench: nic       : JH7110 port at {:#x} ({}), clock and syscon windows {}",
            r.base,
            core::str::from_utf8(r.compatible).unwrap_or("?"),
            if r.windows_from_tree {
                "from the tree"
            } else {
                "from the constants (the tree named at least one of them nowhere)"
            },
        );
        let words = |c: &jh7110_clock_and_reset::Report| {
            let mut s = [(0u32, 0u32); jh7110_clock_and_reset::MAX_RECORDED_CLOCKS];
            for (k, w) in s.iter_mut().enumerate().take(c.clocks) {
                *w = (c.clock_before[k], c.clock_after[k]);
            }
            (s, c.clocks)
        };
        for (name, c) in [("sys", &r.sys), ("aon", &r.aon)] {
            let (w, n) = words(c);
            for (k, (before, after)) in w.iter().take(n).enumerate() {
                println!(
                    "network-bench: clocks    : {name} clock step {k}: {before:#010x} -> {after:#010x}"
                );
            }
        }
        if r.aon.had_mux {
            println!(
                "network-bench: clocks    : aon tx mux {:#010x} -> {:#010x}",
                r.aon.mux_before, r.aon.mux_after
            );
        }
        println!(
            "network-bench: resets    : {} of {} released, last assert {:#010x} -> {:#010x}, status {:#010x}, {} polls",
            r.aon.resets_released,
            r.aon.resets,
            r.aon.reset_assert_before,
            r.aon.reset_assert_after,
            r.aon.reset_status_after,
            r.aon.polls,
        );
        println!(
            "network-bench: syscon    : {:#010x} -> {:#010x} (gmac0 interface {} before, {} after; 1 is RGMII)",
            r.syscon.0,
            r.syscon.1,
            jh7110::interface(r.syscon.0),
            jh7110::interface(r.syscon.1),
        );
        println!(
            "network-bench: mac core  : version {:#010x}, {}-bit DMA",
            r.version, r.dma_bits
        );
        match r.phy {
            Some((mv, source)) => println!(
                "network-bench: phy       : id {:#010x}, {mv} mV I/O, settings from {source:?}",
                r.phy_id
            ),
            None => println!("network-bench: phy       : id {:#010x}", r.phy_id),
        }
        if let Some((m, source)) = r.mac {
            println!(
                "network-bench: address   : {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} from {source:?}",
                m[0], m[1], m[2], m[3], m[4], m[5]
            );
        }
        println!(
            "network-bench: reset     : {} us, then {} us",
            r.resets_us[0], r.resets_us[1]
        );
        match r.coherence {
            Some((v, o)) => println!(
                "network-bench: coherence : {v:?} (rx des3 {:#010x}, tx des3 {:#010x}, channel status {:#010x}, payload {:?})",
                o.rx_des3, o.tx_des3, o.status, o.payload
            ),
            None => println!("network-bench: coherence : not reached"),
        }
        match r.link {
            Some(l) => println!(
                "network-bench: link      : {} Mbit/s {} duplex, {} ms after autonegotiation restarted",
                l.speed.mbps(),
                if l.full_duplex { "full" } else { "half" },
                r.link_wait_ms
            ),
            None => println!(
                "network-bench: link      : NONE after {} ms",
                r.link_wait_ms
            ),
        }
    }
}
